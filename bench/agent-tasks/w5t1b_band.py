#!/usr/bin/env python3
"""W5.T1.b: lean observe vs the pre-W5.T1 format (MANUK_OBSERVE_MUTATE=legacy) on the own 40-task suite x3, Haiku.

  python3 -I bench/agent-tasks/w5t1b_band.py LEAN.tsv LEGACY.tsv

Rule, fixed before the runs (SUCCESS.md Bar 2 margin = 10 points = 4 of 40 tasks):
  - per-run pass band: min..max of passes in each run (run 1..3) per arm;
  - PASS iff the two bands overlap AND |pass>=2of3(lean) - pass>=2of3(legacy)| <= 4.
Exit 0 on PASS, 1 on FAIL. Prints tokens per task (in+out, cache excluded) for both arms.
"""
import csv, sys

def load(path):
    with open(path) as f:
        rows = [r for r in csv.DictReader(f, delimiter="\t") if r.get("task")]
    runs, per_task, tok = {}, {}, {}
    for r in rows:
        p = int(r["pass"] or 0)
        runs.setdefault(int(r["run"]), 0)
        runs[int(r["run"])] += p
        per_task.setdefault(r["task"], []).append(p)
        tok.setdefault(r["task"], []).append(int(r["input_tokens"] or 0) + int(r["output_tokens"] or 0))
    pass2 = sum(1 for ps in per_task.values() if sum(ps) >= 2)
    mean_tok = sum(sum(v) for v in tok.values()) / max(1, sum(len(v) for v in tok.values()))
    return runs, pass2, len(per_task), mean_tok, rows

def main(lean_path, legacy_path):
    lr, lp2, ln, lt, lrows = load(lean_path)
    gr, gp2, gn, gt, grows = load(legacy_path)
    lband = (min(lr.values()), max(lr.values()))
    gband = (min(gr.values()), max(gr.values()))
    overlap = lband[0] <= gband[1] and gband[0] <= lband[1]
    diff = abs(lp2 - gp2)
    ok = overlap and diff <= 4
    chrome = {r["chrome_ver"] for r in lrows + grows}
    print(f"chrome_ver\t{','.join(sorted(chrome))}")
    print("arm\tn_rows\ttasks\tper-run pass\tband\tpass>=2of3\tmean in+out tok/run")
    print(f"lean\t{len(lrows)}\t{ln}\t{[lr[k] for k in sorted(lr)]}\t{lband[0]}-{lband[1]}\t{lp2}/{ln}\t{lt:.0f}")
    print(f"legacy\t{len(grows)}\t{gn}\t{[gr[k] for k in sorted(gr)]}\t{gband[0]}-{gband[1]}\t{gp2}/{gn}\t{gt:.0f}")
    print(f"bands overlap={overlap} pass>=2of3 diff={diff} (margin 4)")
    print("VERDICT", "PASS" if ok else "FAIL")
    return 0 if ok else 1

if __name__ == "__main__":
    sys.exit(main(sys.argv[1], sys.argv[2]))
