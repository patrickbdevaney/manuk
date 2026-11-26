#!/usr/bin/env python3
"""Whose failure was it? Attribute every scored failure of an agent run to the ENGINE or the MODEL, from evidence.

  python3 -I bench/agent-tasks/h1_attrib.py OUT.tsv [--golden]      # per-driver table + one line per failure
  python3 -I bench/agent-tasks/h1_attrib.py self-test               # every rule can fire, and a mutant goes red

A pass rate measured with a small model (qwen3.8-27b on Groq's free tier) mixes two things: what the ENGINE let the
agent see and do, and what the MODEL made of it. Only the first is the engine's score. So a failure counts against the
engine only when the evidence says the engine was at fault; otherwise it is the model's, or it is UNRESOLVED and says
what would resolve it. Nothing is attributed by default.

Evidence (written by h1_ab.py per run, under <out>.traces/): the relay trace (every message the model saw and every
reply), the agent's stdout/stderr, and the fixture end state.

Rules, first match wins:
  HARNESS grant       an action was refused by the invocation's capability grant: the eval was misconfigured, the
                      run is excluded from scoring (neither the engine's nor the model's)
  ENGINE  crash       the agent process panicked / was killed by a signal
  ENGINE  slow        a timeout where the ENGINE's share of active time (active - model generation) exceeds the budget
  MODEL   slow        a timeout where model generation is the larger share
  MODEL   injection   the run followed a prompt injection (the model obeyed page text)
  ENGINE  no-effect   a state task: the model issued every wanted value in its actions, the end state lacks one
  MODEL   not-issued  a state task: some wanted value never appears in any of the model's actions
  MODEL   ignored     an answer task: every missing fact WAS in an observation the model received
  ENGINE  act-error   a missing fact never reached the model AND an action the model took came back as an error
  UNRESOLVED unseen   a missing fact never reached the model, no action errored: either the engine did not surface it
                      or the model never went to where it is. The model-free control resolves it (--golden).

The model-free control (--golden): bench/agent-tasks/golden/<task>.<driver>.json is a reply script (the replies a
correct agent gives) run through `manuk agent-task --script`, with NO model. If it passes, the engine CAN do the task,
so an `unseen` failure is the MODEL's (it did not get there); if it fails, the engine cannot, and the failure is the
ENGINE's. Without a golden script the row stays UNRESOLVED and is reported as such, never folded into either side.

Headline per driver: pass rate on scored runs, and the ENGINE-attributable failure rate with its band:
  lower = confirmed engine failures / scored,  upper = (engine + unresolved) / scored.
"""
import csv, json, os, re, subprocess, sys, collections

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "../.."))
sys.path.insert(0, HERE)
BUDGET_S = 240
ERR = re.compile(r"\b(error|not found|no such|no element|could not|failed to|cannot|unknown (target|element|ref)|"
                 r"out of range|stale|detached|not (visible|interactable|clickable))\b", re.I)
CRASH = re.compile(r"panicked at|SIGSEGV|SIGABRT|stack overflow|fatal runtime error", re.I)


def norm(s):
    return re.sub(r"\s+", " ", (s or "").replace(" ", " ")).lower()


def msg_text(m):
    c = m.get("content")
    if isinstance(c, str):
        return c
    if isinstance(c, list):
        return " ".join(p.get("text", "") for p in c if isinstance(p, dict))
    return ""


def load_trace(base):
    calls = []
    p = base + ".jsonl"
    if os.path.exists(p):
        with open(p) as f:
            calls = [json.loads(l) for l in f if l.strip()]
    rd = lambda ext: open(base + ext).read() if os.path.exists(base + ext) else ""
    state = None
    if os.path.exists(base + ".state.json"):
        state = json.load(open(base + ".state.json"))
    return calls, rd(".out"), rd(".err"), state


def observations(calls, goal):
    """Texts the model was SHOWN by the agent: every non-system, non-assistant message, normalised, with the goal text
    cut out (a goal that quotes a value must not count as the page showing it). Union over all calls also covers
    drivers that truncate history."""
    seen, ng = [], norm(goal)
    for c in calls:
        for m in c.get("messages") or []:
            if m.get("role") in ("system", "assistant"):
                continue
            t = norm(msg_text(m)).replace(ng, " ") if ng else norm(msg_text(m))
            if t.strip():
                seen.append(t)
    return seen


def replies(calls):
    return [c.get("reply") or "" for c in calls]


def flatten(v):
    if isinstance(v, dict):
        out = []
        for x in v.values():
            out += flatten(x)
        return out
    return [str(v)]


def classify(task, row, calls, out, err, state):
    """-> (side, rule, evidence). side in ENGINE / MODEL / UNRESOLVED."""
    v = task["verifier"]
    reasons = row.get("reasons") or ""
    if CRASH.search(err) or (row.get("rc") not in ("", None) and int(row["rc"]) < 0):
        return "ENGINE", "crash", ((CRASH.search(err) or [""])[0] or "signal rc=%s" % row["rc"])
    if reasons.startswith("timeout"):
        active, model_s = float(row.get("active_s") or 0), float(row.get("model_s") or 0)
        engine_s = active - model_s
        if engine_s > BUDGET_S / 2 and engine_s >= model_s:
            return "ENGINE", "slow", "engine %.0fs vs model %.0fs of %.0fs active" % (engine_s, model_s, active)
        return "MODEL", "slow", "model %.0fs vs engine %.0fs of %.0fs active" % (model_s, engine_s, active)
    blocked = [c for c in calls for m in c.get("messages") or [] if "was not granted to this invocation" in msg_text(m)]
    if blocked:
        return "HARNESS", "grant", "an action was refused by the invocation's grant (run without --grant all?)"
    if "injection followed" in reasons:
        return "MODEL", "injection", "the run acted on injected page text"
    obs = observations(calls, task["goal"])
    obs_n = norm("\n".join(obs))
    said = norm("\n".join(replies(calls)))
    if v.get("state") and "state[" in reasons:
        wanted = [w for w in flatten(v["state"]) if w and w.lower() not in ("true", "false", "none")]
        missing = [w for w in wanted if norm(w) not in said]
        if not missing:
            return "ENGINE", "no-effect", "model issued %s; end state %s" % (wanted, json.dumps(state)[:160])
        return "MODEL", "not-issued", "never issued %s" % missing
    missing = [s for s in v.get("answer_all", []) if norm(s) not in norm(row.get("answer") or "")]
    if not missing:
        return "UNRESOLVED", "other", reasons[:160]
    unseen = [s for s in missing if norm(s) not in obs_n]
    if not unseen:
        return "MODEL", "ignored", "%s was in an observation; the answer omitted it" % missing
    errs = [m.group(0) for t in obs for m in [re.search(r".{0,60}" + ERR.pattern + r".{0,60}", t, re.I)] if m]
    if errs:
        return "ENGINE", "act-error", "%s never shown; action error: %s" % (unseen, errs[0])
    return "UNRESOLVED", "unseen", "%s never reached the model; needs the model-free control" % unseen


def golden(task, driver, base_url):
    """Run the model-free control. Returns True/False, or None when no golden script exists."""
    g = os.path.join(HERE, "golden", "%s.%s.json" % (task["id"], driver))
    if not os.path.exists(g):
        return None
    import verify as V
    import fixtures
    fixtures.reset()
    url = task["start_url"].replace("{BASE}", base_url)
    p = subprocess.run([os.path.join(ROOT, "target/release/manuk"), "agent-task", url, task["goal"], "--driver",
                        driver, "--script", g, "--max-steps", "40"], capture_output=True, text=True, timeout=600,
                       cwd=ROOT)
    ans = next((l[8:] for l in p.stdout.splitlines() if l.startswith("ANSWER: ")), "")
    state = None
    if "{BASE}" in task["start_url"]:
        import urllib.request
        state = json.load(urllib.request.urlopen(base_url + "/__state", timeout=10))
    return bool(V.verify(task, ans, state)["pass"])


def final_rows(path):
    rows = list(csv.DictReader(open(path), delimiter="\t"))
    by = {}
    for r in rows:
        k = (r["task"], r["driver"])
        if k not in by or int(r["attempt"]) > int(by[k]["attempt"]):
            by[k] = r
    return by


def attribute(path, use_golden=False):
    tasks = {t["id"]: t for t in json.load(open(os.path.join(HERE, "suite-v1.json")))["tasks"]}
    by = final_rows(path)
    srv = base = None
    if use_golden:
        from fixtures import serve
        srv, base = serve(0)
    out = []
    try:
        for (tid, d), r in sorted(by.items()):
            if r["pass"] == "1":
                out.append((tid, d, "PASS", "", ""))
                continue
            if r["pass"] == "":
                out.append((tid, d, "UNSCORED", r.get("outcome") or "infra", r.get("reasons", "")[:80]))
                continue
            calls, so, se, state = load_trace(os.path.join(ROOT, r["trace"])) if r.get("trace") else ([], "", "", None)
            side, rule, ev = classify(tasks[tid], r, calls, so, se, state)
            if side == "UNRESOLVED" and rule == "unseen" and use_golden:
                g = golden(tasks[tid], d, base)
                if g is True:
                    side, rule, ev = "MODEL", "unseen+golden-pass", ev + "; the model-free control PASSES"
                elif g is False:
                    side, rule, ev = "ENGINE", "unseen+golden-fail", ev + "; the model-free control FAILS"
            out.append((tid, d, side, rule, ev))
    finally:
        if srv:
            srv.shutdown()
    return out


def summary(res):
    drivers = sorted({d for _, d, *_ in res})
    lines = ["| driver | scored | pass | MODEL | ENGINE | UNRESOLVED | harness | unscored | engine-attributable failure rate |",
             "|---|---|---|---|---|---|---|---|---|"]
    for d in drivers:
        c = collections.Counter(side for _, dd, side, *_ in res if dd == d)
        scored = c["PASS"] + c["MODEL"] + c["ENGINE"] + c["UNRESOLVED"]  # HARNESS and UNSCORED are excluded
        lo = c["ENGINE"] / scored if scored else 0
        hi = (c["ENGINE"] + c["UNRESOLVED"]) / scored if scored else 0
        lines.append("| %s | %d | %d (%.0f%%) | %d | %d | %d | %d | %d | %.0f%%..%.0f%% |" % (
            d, scored, c["PASS"], 100 * c["PASS"] / scored if scored else 0, c["MODEL"], c["ENGINE"],
            c["UNRESOLVED"], c["HARNESS"], c["UNSCORED"], 100 * lo, 100 * hi))
    return lines


def self_test():
    fails = []

    def check(name, cond):
        print(("ok   " if cond else "RED  ") + name)
        if not cond:
            fails.append(name)
    ans_task = {"id": "T", "goal": "What is item 45?", "verifier": {"answer_all": ["Item 45: amber"]}}
    st_task = {"id": "S", "goal": "Sign up as Ada", "verifier": {"answer_all": ["CONF"], "state": {
        "submitted": {"name": "Ada Lovelace", "plan": "Team"}}}}
    row = lambda **k: dict(dict(rc="0", reasons="answer missing 'Item 45: amber'", answer="Item 44", active_s="60",
                                model_s="40"), **k)
    call = lambda obs, reply="": {"messages": [{"role": "system", "content": "sys"},
                                               {"role": "user", "content": obs}], "reply": reply}
    c = classify(ans_task, row(), [call("list: Item 44: red\nItem 45: amber")], "", "", None)
    check("a fact the model was shown and omitted is the MODEL's", c[:2] == ("MODEL", "ignored"))
    c = classify(ans_task, row(), [call("list: Item 44: red\nscroll: error: no element 9")], "", "", None)
    check("an unseen fact behind an action error is the ENGINE's", c[:2] == ("ENGINE", "act-error"))
    c = classify(ans_task, row(), [call("list: Item 44: red")], "", "", None)
    check("an unseen fact with no error is UNRESOLVED, not the engine's", c[:2] == ("UNRESOLVED", "unseen"))
    c = classify(ans_task, row(), [], "", "thread 'main' panicked at layout.rs", None)
    check("a panic is the ENGINE's", c[:2] == ("ENGINE", "crash"))
    c = classify(ans_task, row(reasons="timeout: active 300s > 240s; x", active_s="300", model_s="250"), [], "", "", None)
    check("a timeout spent in the model is the MODEL's", c[:2] == ("MODEL", "slow"))
    c = classify(ans_task, row(reasons="timeout: active 300s > 240s; x", active_s="300", model_s="40"), [], "", "", None)
    check("a timeout spent in the engine is the ENGINE's", c[:2] == ("ENGINE", "slow"))
    srow = row(reasons="state[submitted]={'name': ''} want {...}")
    c = classify(st_task, srow, [call("form", 'fill_form {"name":"Ada Lovelace","plan":"Team"}')], "", "", {})
    check("values the model typed that did not land are the ENGINE's", c[:2] == ("ENGINE", "no-effect"))
    c = classify(st_task, srow, [call("form", 'fill_form {"plan":"Team"}')], "", "", {})
    check("values the model never typed are the MODEL's", c[:2] == ("MODEL", "not-issued"))
    g = {"id": "G", "goal": "Item 45: amber", "verifier": {"answer_all": ["Item 45: amber"]}}
    c = classify(g, row(), [call("Item 45: amber")], "", "", None)
    check("MUTANT GUARD: a goal that quotes the fact does not count as showing it", c[0] == "UNRESOLVED")
    c = classify(st_task, srow, [call("BLOCKED: action \"click\" was not granted to this invocation", "Ada Lovelace Team")],
                 "", "", {})
    check("a grant refusal is the HARNESS's, not the engine's", c[0] == "HARNESS")
    res = [("A", "hub", "PASS", "", ""), ("F", "hub", "HARNESS", "grant", ""), ("B", "hub", "ENGINE", "crash", ""), ("C", "hub", "UNRESOLVED", "unseen", ""),
           ("D", "hub", "MODEL", "ignored", ""), ("E", "hub", "UNSCORED", "infra", "")]
    s = summary(res)[-1]
    check("the headline band is engine/scored .. (engine+unresolved)/scored", "25%..50%" in s and "| 4 |" in s)
    if fails:
        print("self-test RED:", fails)
        return 1
    print("self-test green")
    return 0


if __name__ == "__main__":
    a = sys.argv[1:]
    if a == ["self-test"]:
        sys.exit(self_test())
    if not a:
        sys.exit(__doc__)
    res = attribute(a[0], "--golden" in a)
    for l in summary(res):
        print(l)
    print()
    for tid, d, side, rule, ev in res:
        if side not in ("PASS",):
            print("%-4s %-8s %-10s %-20s %s" % (tid, d, side, rule, ev))
