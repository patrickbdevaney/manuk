#!/usr/bin/env python3
"""W3.18.a.a.b: REAL's 59 llm_boolean rubrics graded by the owner-provided OpenAI-compatible judge.

  real_judge.py controls [--out TSV]     validity controls (a) negative, (b) positive, (c) two-pass agreement
  real_judge.py verdict RUBRIC ANSWER    one verdict (debug; prints JSON)
  real_judge.py agree                    (c) two fresh passes vs the committed controls TSV -> judge-agreement.tsv
MANUK_JUDGE_MUTATE=1 makes every verdict true (the gate must then go red).

The judge is read from ~/manuk/.env (REAL_JUDGE_BASE_URL, REAL_JUDGE_API_KEY, REAL_JUDGE_MODEL) inside this process
only. The key is never printed, logged or written to a row. Temperature 0. The judge sees the rubric, the expected
boolean, the agent's final answer and the final clone state, and nothing else. Run with `python3 -I`.
"""
import argparse, glob, json, os, re, sys, time, urllib.error, urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
TASKS = os.path.join(HERE, "real", "tasks")
STATE_CAP = 8000  # chars of the final clone state passed to the judge
PROMPT = (
    "You are grading an AI agent's run on a web task. You are given a rubric (a yes/no question about the outcome), "
    "the agent's final answer, and the final state of the web app as JSON. Decide whether the rubric holds for this run. "
    "Judge only from the answer and the state; do not assume anything that is not shown.\n\n"
    "RUBRIC: {rubric}\n\nFINAL ANSWER:\n{answer}\n\nFINAL STATE (JSON):\n{state}\n\n"
    'Reply with only a JSON object: {{"verdict": true or false, "reason": "<one short sentence>"}}'
)


def load_env():
    env = {}
    for l in open(os.path.join(ROOT, ".env")):
        l = l.strip()
        if "=" in l and not l.startswith("#"):
            k, v = l.split("=", 1)
            env[k.strip()] = v.strip().strip('"').strip("'")
    for k in ("REAL_JUDGE_BASE_URL", "REAL_JUDGE_API_KEY", "REAL_JUDGE_MODEL"):
        if not env.get(k):
            sys.exit(f"refusing: {k} missing from .env")
    return env


def _post(env, body, tries=8):
    data = json.dumps(body).encode()
    url = env["REAL_JUDGE_BASE_URL"].rstrip("/") + "/chat/completions"
    hdr = {"Authorization": "Bearer " + env["REAL_JUDGE_API_KEY"], "Content-Type": "application/json",
           "User-Agent": "manuk-real-judge/1.0"}  # the default Python UA is refused by the gateway (HTTP 403, Cloudflare 1010)
    for i in range(tries):
        try:
            with urllib.request.urlopen(urllib.request.Request(url, data=data, headers=hdr), timeout=120) as r:
                return json.load(r)
        except urllib.error.HTTPError as e:
            if e.code not in (408, 429, 500, 502, 503, 504):
                raise RuntimeError(f"judge HTTP {e.code}")
            wait = float(e.headers.get("retry-after") or 0) or min(60, 2 ** i)
        except (urllib.error.URLError, TimeoutError):
            wait = min(60, 2 ** i)
        time.sleep(wait)
    raise RuntimeError("judge: retries exhausted")


def verdict(env, rubric, answer, state=None):
    """-> (bool or None, reason). None = the judge's reply did not parse (counts as a failed eval, never as a pass)."""
    if os.environ.get("MANUK_JUDGE_MUTATE"):  # mutant: a judge that always says yes; the controls must then fail
        return True, "mutant: always true"
    st = json.dumps(state or {}, ensure_ascii=False)[:STATE_CAP]
    body = {"model": env["REAL_JUDGE_MODEL"], "temperature": 0,
            "messages": [{"role": "user", "content": PROMPT.format(rubric=rubric, answer=answer or "(empty)", state=st)}]}
    txt = _post(env, body)["choices"][0]["message"]["content"]
    m = re.search(r"\{.*\}", txt, re.S)
    try:
        j = json.loads(m.group(0))
        v = j["verdict"]
        return (v if isinstance(v, bool) else None), str(j.get("reason", ""))[:200]
    except (AttributeError, ValueError, KeyError, TypeError):
        return None, "unparsed: " + txt[:120].replace("\t", " ").replace("\n", " ")


def llm_rubrics():
    """Every llm_boolean eval of every non-omnizon task: (task_id, rubric, expected)."""
    out = []
    for f in sorted(glob.glob(os.path.join(TASKS, "*.json"))):
        t = json.load(open(f))
        if t["website"]["id"] == "omnizon":
            continue
        for e in t["evals"]:
            if e["type"] == "llm_boolean":
                out.append((t["id"], e["rubric"], e["expected_value"]))
    return out


# Positive control: correct answers written by hand against each rubric, each checked by reading the rubric text.
POSITIVE = {
    "Does the answer include \"Gambinos New York Subs\", \"Wingstop\", and \"Man vs Fries\" as the first three restaurants?":
        "The first three restaurants on the homepage are Gambinos New York Subs, Wingstop, and Man vs Fries.",
    "Does the answer indicate that the price is $17.11?": "The price is $17.11.",
    "Does the answer indicate that the earliest flight is at 8:00 AM?": "The earliest flight departs at 8:00 AM.",
    "Does the answer indicate that the price for Premium Economy is $453?": "The price for Premium Economy is $453.",
    "Does the answer reflect that there are 17 unread emails in the Inbox?": "There are 17 unread emails in the Inbox.",
    "Does the answer reflect that there are 13 emails in the \"Starred\" category?": "There are 13 emails in the Starred category.",
    "Does the answer contain the price difference of $982?": "The price difference is $982.",
    "Does the answer reflect that the last Thai restaurant visted was 'Phat Thai'?": "The last Thai restaurant visited was Phat Thai.",
    "Does the answer reflect that the cheapest ride price is equivalant to $28.50?": "The cheapest ride costs $28.50.",
    "Does the answer reflect that six rides were taken in June?": "Six rides were taken in June.",
    "Does the answer reflect a license plate of \"DLOD90\"?": "The license plate is DLOD90.",
    "Does the answer indicate that there are 4 flights available from Dallas to Fresno on December 4th, 2024?":
        "There are 4 flights available from Dallas to Fresno on December 4th, 2024.",
}
# Negative control: one deliberately wrong answer, carrying no task facts, for every rubric.
WRONG = "I could not complete this task. I did not find any of that information on the page and have nothing to report."


def controls(env, out):
    rub = llm_rubrics()
    rows = ["judge_model\tcontrol\ttask\trubric\tverdict\treason"]
    pos = [(r, a) for r, a in POSITIVE.items() if a]
    # (a) negative: must be false on >=90% of all rubrics
    na, a_true = 0, 0
    for tid, r, exp in rub:
        v, why = verdict(env, r, WRONG)
        na += (v is False); a_true += (v is True)
        rows.append(f"{env['REAL_JUDGE_MODEL']}\tneg\t{tid}\t{r}\t{v}\t{why}")
    # (b) positive: must be true on >=90% of the hand-checked answers
    pb = 0
    for r, ans in pos:
        tid = next(t for t, rr, _ in rub if rr == r)
        v, why = verdict(env, r, ans)
        pb += (v is True)
        rows.append(f"{env['REAL_JUDGE_MODEL']}\tpos\t{tid}\t{r}\t{v}\t{why}")
    # (c) two passes over the same (control) inputs must agree on >=95%
    inputs = [(r, WRONG) for _, r, _ in rub] + pos
    agree = 0
    for r, ans in inputs:
        v1, _ = verdict(env, r, ans)
        v2, _ = verdict(env, r, ans)
        agree += (v1 == v2)
    res = {"neg_false": f"{na}/{len(rub)}", "neg_pass": na / len(rub) >= 0.90,
           "pos_true": f"{pb}/{len(pos)}", "pos_pass": pb / len(pos) >= 0.90,
           "agree": f"{agree}/{len(inputs)}", "agree_pass": agree / len(inputs) >= 0.95}
    open(out, "w").write("\n".join(rows) + "\n")
    print(json.dumps(res))
    return all(res[k] for k in ("neg_pass", "pos_pass", "agree_pass"))


def agreement(env, committed, out):
    """(c) two fresh judge passes over the committed control inputs; an input agrees only when pass 1, pass 2 and the
    committed verdict are all the same boolean (None never agrees). Writes one row per input to `out`.
    Resumable: rows already in `out + ".partial"` are kept and not re-judged (the gateway rate-limits with 429s)."""
    rub = {r: t for t, r, _ in llm_rubrics()}
    rows = [l.rstrip("\n").split("\t") for l in open(committed)][1:]
    inputs = []
    for model, control, task, rubric, verdict_s, _reason in rows:
        ans = WRONG if control == "neg" else POSITIVE[rubric]
        inputs.append((model, control, task, rubric, verdict_s == "True", ans))
    assert len(inputs) == 71 and all(i[3] in rub for i in inputs)
    part = out + ".partial"
    done = [l.rstrip("\n").split("\t") for l in open(part)] if os.path.exists(part) else []
    fh = open(part, "a")
    if not done:
        fh.write("judge_model\tcontrol\ttask\trubric\tcommitted\tpass1\tpass2\tagree3\n"); fh.flush()
    for k, (model, control, task, rubric, committed_v, ans) in enumerate(inputs):
        if k < len(done) - 1:
            continue
        v1, _ = verdict(env, rubric, ans)
        v2, _ = verdict(env, rubric, ans)
        line = f"{model}\t{control}\t{task}\t{rubric}\t{committed_v}\t{v1}\t{v2}\t{int(v1 is not None and v1 == v2 == committed_v)}"
        fh.write(line + "\n"); fh.flush()
        print(f"{k + 1}/{len(inputs)} {control} {task} committed={committed_v} p1={v1} p2={v2}", file=sys.stderr, flush=True)
    fh.close()
    body = [l.rstrip("\n").split("\t") for l in open(part)][1:]
    assert len(body) == 71
    a3 = sum(r[7] == "1" for r in body)
    a_pair = sum(r[5] == r[6] != "None" for r in body)
    a_commit = sum(r[5] == r[4] != "None" for r in body)
    head = ["judge_model", "control", "task", "rubric", "committed", "pass1", "pass2", "agree3"]
    open(out, "w").write("\n".join("\t".join(r) for r in [head] + body) + "\n")
    os.remove(part)
    res = {"n": len(body), "agree3": f"{a3}/{len(body)}", "pass_vs_pass": f"{a_pair}/{len(body)}",
           "pass1_vs_committed": f"{a_commit}/{len(body)}", "agree_pass": a3 / len(body) >= 0.95}
    print(json.dumps(res))
    return res["agree_pass"]

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=["controls", "verdict", "agree"])
    ap.add_argument("rest", nargs="*")
    ap.add_argument("--out", default=os.path.join(ROOT, "docs/sprint/w318/judge-controls.tsv"))
    a = ap.parse_args()
    env = load_env()
    if a.cmd == "verdict":
        print(json.dumps(verdict(env, a.rest[0], a.rest[1])))
        return
    if a.cmd == "agree":
        ok = agreement(env, os.path.join(ROOT, "docs/sprint/w318/judge-controls.tsv"),
                       os.path.join(ROOT, "docs/sprint/w318/judge-agreement.tsv"))
        sys.exit(0 if ok else 1)
    ok = controls(env, a.out)
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
