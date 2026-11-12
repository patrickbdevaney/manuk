#!/usr/bin/env python3
"""RG.E1.b.a.a: re-judge the stored final answers of the three E1ba networkin-4 manuk runs with the SAME judge
(openai/gpt-oss-120b, via bench/agent-tasks/real_judge.py), three times each, and write one row per re-judging.

  python3 -I docs/sprint/rg/E1baa-rejudge.py run     # 9 judge calls -> docs/sprint/rg/E1baa-rejudge.tsv
  python3 -I docs/sprint/rg/E1baa-rejudge.py check [TSV]   # gate: exit 1 on any broken row

Inputs are the archived E1ba run dirs (target/real-runs/manuk_networkin-4_r{1,2,3}.20261112T*): the raw final answer
from transcript.jsonl (the exact text the judge saw) and the state.json the judge saw. The rubric is read from the
task file, never edited. MANUK_JUDGE_MUTATE=1 (real_judge.py) makes every verdict true: the gate must then go red.
The judge is never swapped: no fallback model is used here, a judge error is written as judge_error, not a verdict.
"""
import csv, hashlib, json, os, sys, time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(HERE)))
sys.path.insert(0, os.path.join(ROOT, "bench", "agent-tasks"))
import real_judge  # noqa: E402

TSV = os.environ.get("E1BAA_TSV") or os.path.join(HERE, "E1baa-rejudge.tsv")  # override: mutation tests only
RUNS = os.path.join(ROOT, "target", "real-runs")
RUN_DIRS = {1: "manuk_networkin-4_r1.20261112T112141", 2: "manuk_networkin-4_r2.20261112T112624",
            3: "manuk_networkin-4_r3.20261112T113115"}
RECORDED = {1: "0", 2: "0", 3: "1"}  # llm_ok of the E1ba rows (docs/sprint/rg/E1ba-real-manuk.tsv), same order
TASK = os.path.join(ROOT, "bench", "agent-tasks", "real", "tasks", "networkin-4.json")
COLS = ["arm", "task", "source_run", "rejudge", "recorded_llm_ok", "verdict", "reason", "answer_sha256", "state_sha256",
        "chrome_ver", "judge_model", "rubric_sha256"]
PIN = os.environ.get("MANUK_CHROME_PIN", "")


def sha(s):
    return hashlib.sha256(s.encode()).hexdigest()[:16]


def load_inputs():
    t = json.load(open(TASK))
    rubric = next(e["rubric"] for e in t["evals"] if e["type"] == "llm_boolean")
    out = []
    for run, d in RUN_DIRS.items():
        base = os.path.join(RUNS, d)
        ev = [json.loads(l) for l in open(os.path.join(base, "transcript.jsonl")) if l.startswith("{")]
        res = next(e for e in reversed(ev) if e.get("type") == "result")
        answer = res.get("result") or ""
        st = json.load(open(os.path.join(base, "state.json")))
        state = st["state"] if st.get("ok") else None
        out.append({"run": run, "answer": answer, "state": state, "rubric": rubric})
    return out


def run():
    env = real_judge.load_env()  # read inside this process only; never printed
    rows = []
    for inp in load_inputs():
        for k in (1, 2, 3):
            try:
                v, reason = real_judge.verdict(env, inp["rubric"], inp["answer"], inp["state"])
                verdict = "unparsed" if v is None else str(v).lower()
            except (RuntimeError, OSError) as x:
                verdict, reason = "judge_error", str(x)[:120]
            rows.append({"arm": "manuk", "task": "networkin-4", "source_run": inp["run"], "rejudge": k,
                         "recorded_llm_ok": RECORDED[inp["run"]], "verdict": verdict,
                         "reason": reason.replace("\t", " ").replace("\n", " "),
                         "answer_sha256": sha(inp["answer"]),
                         "state_sha256": sha(json.dumps(inp["state"], sort_keys=True)),
                         "chrome_ver": PIN, "judge_model": env["REAL_JUDGE_MODEL"],
                         "rubric_sha256": sha(inp["rubric"])})
            print(f"run{inp['run']} r{k}: {verdict}", flush=True)
    with open(TSV, "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=COLS, delimiter="\t")
        w.writeheader()
        w.writerows(rows)
    print("wrote", TSV, len(rows), "rows")


def check(path):
    """Gate. Returns the list of problems (empty = pass)."""
    bad = []
    rows = list(csv.DictReader(open(path), delimiter="\t"))
    if len(rows) != 9:
        bad.append(f"expected 9 rows, got {len(rows)}")
    pin = os.environ.get("MANUK_CHROME_PIN", "")
    if not pin:
        bad.append("MANUK_CHROME_PIN unset")
    for r in rows:
        if r["chrome_ver"] != pin:
            bad.append(f"chrome_ver {r['chrome_ver']} != pin on run{r['source_run']} r{r['rejudge']}")
        if r["judge_model"] != "openai/gpt-oss-120b":
            bad.append(f"judge_model {r['judge_model']} on run{r['source_run']} r{r['rejudge']}")
        if r["reason"].startswith("mutant"):
            bad.append(f"mutant judge verdict on run{r['source_run']} r{r['rejudge']} (MANUK_JUDGE_MUTATE set)")
        if r["verdict"] not in ("true", "false"):
            bad.append(f"no verdict on run{r['source_run']} r{r['rejudge']}: {r['verdict']}")
    by_run = {}
    for r in rows:
        by_run.setdefault(r["source_run"], []).append(r)
    for run_id, rs in by_run.items():
        if len(rs) != 3 or sorted(x["rejudge"] for x in rs) != ["1", "2", "3"]:
            bad.append(f"run{run_id} does not have re-judgings 1,2,3")
    # the stored answers must still be the answers the E1ba rows were graded on (hash of the raw transcript text)
    for inp in load_inputs():
        for r in by_run.get(str(inp["run"]), []):
            if r["answer_sha256"] != sha(inp["answer"]):
                bad.append(f"answer hash changed for run{inp['run']}")
            if r["rubric_sha256"] != sha(inp["rubric"]):
                bad.append("rubric changed")
    return bad


if __name__ == "__main__":
    cmd = sys.argv[1] if len(sys.argv) > 1 else "check"
    if cmd == "run":
        run()
    else:
        problems = check(sys.argv[2] if len(sys.argv) > 2 else TSV)
        for p in problems:
            print("FAIL:", p)
        print("PASS" if not problems else f"RED: {len(problems)} problem(s)")
        sys.exit(1 if problems else 0)
