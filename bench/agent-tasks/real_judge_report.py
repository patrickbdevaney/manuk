#!/usr/bin/env python3
"""W3.18.a.a.b: per-arm pass for the judge-graded REAL suite (real-judge-gptoss120b), with 3-run bands.

  real_judge_report.py [--runs TSV] [--out TSV]

A task passes when it passes in >=2 of its runs (the SUCCESS.md rule). Band = min..max of per-run-index pass counts
across the 3 run indices. Rows are read only from suite real-judge-gptoss120b; deterministic rows are never mixed in.
Run with `python3 -I`.
"""
import argparse, collections, csv, os

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
SUITE = "real-judge-gptoss120b"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--runs", default=os.path.join(ROOT, "docs/sprint/w318/real-judge-runs.tsv"))
    ap.add_argument("--out", default=os.path.join(ROOT, "docs/sprint/w318/real-judge-report.tsv"))
    a = ap.parse_args()
    rows = [r for r in csv.DictReader(open(a.runs), delimiter="\t") if r["suite"] == SUITE]
    chrome = sorted({r["chrome_ver"] for r in rows})
    assert chrome == ["145.0.7632.116"], f"mixed or wrong chrome_ver: {chrome}"
    by = collections.defaultdict(dict)  # (arm, task) -> {run: pass}
    for r in rows:
        by[(r["arm"], r["task"])][int(r["run"])] = int(r["pass"])
    out = ["chrome_ver\tsuite\tarm\ttasks\tfull_runs\ttask_pass_2of3\trun1\trun2\trun3\tband\tllm_judge_rows\tjudge_model"]
    for arm in sorted({a for a, _ in by}):
        tasks = {t: runs for (a2, t), runs in by.items() if a2 == arm}
        full = {t: r for t, r in tasks.items() if len(r) == 3}
        per_run = [sum(1 for r in full.values() if r.get(i) == 1) for i in (1, 2, 3)]
        p2 = sum(1 for r in full.values() if sum(r.values()) >= 2)
        band = f"{min(per_run)}-{max(per_run)}" if full else "n/a"
        judged = sum(1 for r in rows if r["arm"] == arm)
        model = sorted({r["judge_model"] for r in rows if r["arm"] == arm})
        out.append(f"{chrome[0]}\t{SUITE}\t{arm}\t{len(tasks)}\t{len(full)}\t{p2}\t{per_run[0]}\t{per_run[1]}\t{per_run[2]}\t{band}\t{judged}\t{','.join(model)}")
    open(a.out, "w").write("\n".join(out) + "\n")
    print("\n".join(out))


if __name__ == "__main__":
    main()
