#!/usr/bin/env python3
"""W4.B4.a.a gate: navigate result bytes per task, lean (manuk) vs legacy observe (revert mutant), own suite x3.

  python3 -I b4aa_gate.py DIR [--mutate swap]   (DIR holds manuk.tsv(.turns.tsv) and manuk-legacy-observe.tsv(.turns.tsv))
Pass: navigate bytes/task of manuk is >=30% below the legacy arm AND below the B4a baseline (7035 B/task, n=10),
and manuk's pass count is within 4 of 120-run-equivalent legacy (same band rule as w5t1b: |passes diff| <= 10% of runs).
--mutate swap feeds the legacy file as the lean arm: must exit 1 (the revert returns the bytes).
chrome_ver=145.0.7632.116
"""
import csv, sys, os
BASE = 7035.0
def load(d, arm):
    p = os.path.join(d, arm + ".tsv")
    rows = [r for r in csv.DictReader(open(p), delimiter="\t") if r.get("task")]
    turns = [r for r in csv.DictReader(open(p + ".turns.tsv"), delimiter="\t") if r.get("task")]
    nav = sum(int(t["result_bytes"] or 0) for t in turns if t["tool"].split("+")[0].endswith("navigate"))
    key = {(r["task"], r["run"]) for r in rows}
    return len(key), nav / len(key), sum(int(r["pass"] or 0) for r in rows), rows
a, b = sys.argv[1], ("manuk-legacy-observe", "manuk")
if "--mutate" in sys.argv: b = b[::-1]
n1, lean, p1, r1 = load(a, b[1]); n0, leg, p0, r0 = load(a, b[0])
cut = 1 - lean / leg
print(f"chrome_ver=145.0.7632.116 runs lean={n1} legacy={n0}")
print(f"navigate bytes/task lean={lean:.0f} legacy={leg:.0f} cut={cut:.1%} B4a-baseline={BASE:.0f}")
print(f"passes lean={p1}/{n1} legacy={p0}/{n0}")
ok = cut >= 0.30 and lean < BASE and abs(p1 / n1 - p0 / n0) <= 0.10
print("VERDICT", "PASS" if ok else "FAIL"); sys.exit(0 if ok else 1)
