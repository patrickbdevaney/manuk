#!/usr/bin/env python3
"""W3.18.a.a.b: REAL's 59 llm_boolean rubrics graded by the owner-provided OpenAI-compatible judge.

  real_judge.py controls [--out TSV]     validity controls (a) negative, (b) positive, (c) two-pass agreement
  real_judge.py verdict RUBRIC ANSWER    one verdict (debug; prints JSON)
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


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=["controls", "verdict"])
    ap.add_argument("rest", nargs="*")
    ap.add_argument("--out", default=os.path.join(ROOT, "docs/sprint/w318/judge-controls.tsv"))
    a = ap.parse_args()
    env = load_env()
    if a.cmd == "verdict":
        print(json.dumps(verdict(env, a.rest[0], a.rest[1])))
        return
    ok = controls(env, a.out)
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
