#!/usr/bin/env python3
"""W5.B1: re-judge every REAL llm_boolean rubric of the B1 slice with ONE judge (the primary gpt-oss-120b).

  python3 -I bench/agent-tasks/w5b1_rejudge.py            (rewrites docs/sprint/w5/B1-real-<arm>.tsv; primary gpt-oss-120b)
  MANUK_REAL_JUDGE=claude python3 -I bench/agent-tasks/w5b1_rejudge.py   (the W5.J1 Haiku fallback for every row)

Why: the run-time judge switched from gpt-oss-120b to the W5.J1 Haiku fallback mid-slice (the endpoint was rate
limited), so most tasks had rows from both judges. The agent runs are not repeated here. Each row is re-graded from
its saved inputs, exactly as real_run.one() does: the final answer (the transcript's `result`), the final clone state
(target/real-runs/<arm>_<task>_r<run>/state.json, ok only) and the jmespath counts already in the row.
The original TSV is kept beside it as B1-real-<arm>.mixed-judge.tsv (audit only, not read by w5b1_report.py).
Agreement check: rows the gpt-oss judge scored at run time must reproduce their original llm_ok; the count is printed.
"""
import csv, json, os, shutil, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
sys.path.insert(0, HERE)
import real_judge, real_run, run_arm  # noqa: E402

OUT = os.path.join(ROOT, "docs/sprint/w5")
TR = os.path.expanduser("~/.cache/manuk-b1/transcripts")
STATES = os.path.join(ROOT, "target/real-runs")
ARMS = ["manuk", "cdp-layer", "playwright"]


def main():
    if os.environ.get("MANUK_REAL_JUDGE") == "claude":  # the W5.J1 fallback for every row (primary endpoint rate-limited)
        env = {"backend": "claude", "REAL_JUDGE_MODEL": real_judge.FALLBACK_MODEL}
    else:
        env = real_judge.load_env()  # read inside this process only; never printed
    tasks = {t["id"]: t for t in real_run.load_tasks()}
    for arm in ARMS:
        src = os.path.join(OUT, f"B1-real-{arm}.tsv")
        rows = list(csv.DictReader(open(src), delimiter="\t"))
        cols = list(rows[0].keys())
        shutil.copy(src, os.path.join(OUT, f"B1-real-{arm}.mixed-judge.tsv"))
        same = diff = 0
        new_rows = []
        for r in rows:
            task = tasks[r["task"]]
            run = r["run"]
            out = open(os.path.join(TR, f"{arm}_{r['task']}_r{run}.jsonl")).read()
            j = run_arm.parse(out) or {}
            ans = j.get("result") or ""
            sp = os.path.join(STATES, f"{arm}_{r['task']}_r{run}", "state.json")
            st = json.load(open(sp)) if os.path.exists(sp) else None
            state = st["state"] if st and st["ok"] else None
            llm = [e for e in task["evals"] if e["type"] == "llm_boolean"]
            llm_ok = 0
            for e in llm:
                v, _ = real_judge.verdict(env, e["rubric"], ans, state)
                llm_ok += (v == e["expected_value"])
            if r["judge_model"] == env["REAL_JUDGE_MODEL"]:  # scored by the primary judge at run time: must agree
                same += int(r["llm_ok"]) == llm_ok
                diff += int(r["llm_ok"]) != llm_ok
            jm_ok = r["evals_ok"] == r["evals_n"] and (r["state_ok"] == "1" or r["evals_n"] == "0")
            new = dict(r, llm_ok=llm_ok, llm_n=len(llm), judge_model=env["REAL_JUDGE_MODEL"],
                       **{"pass": int(jm_ok and llm_ok == len(llm) and r["is_error"] == "0")})
            new_rows.append(new)
        with open(src, "w", newline="") as f:
            w = csv.DictWriter(f, fieldnames=cols, delimiter="\t")
            w.writeheader(); w.writerows(new_rows)
        print(f"{arm}: {len(new_rows)} rows re-judged by {env['REAL_JUDGE_MODEL']}; "
              f"primary-judge rows reproduced {same}, changed {diff}")


if __name__ == "__main__":
    main()
