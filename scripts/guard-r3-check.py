#!/usr/bin/env python3
"""GUARD.R3: compare the GUARD.R3 trend-corpus runs to the GUARD.R0 3-run bands in docs/sprint/GUARD.tsv.

A metric below the lower edge of the GUARD.R0 run1-3 band on any R3 run is a REGRESSION (exit 1).
Values above the band are reported but not flagged: the rule is one-sided by design. Bands are not widened.
Rows: 'band run' rows are the R0 baseline (run1-3); rows whose note starts with 'R3 run' are the new runs.

usage: python3 -I scripts/guard-r3-check.py [docs/sprint/GUARD.tsv]
"""
import csv
import os
import sys

METRICS = ["cov", "shape", "m2_full", "m2_meas", "ax_full", "ax_meas"]
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
path = sys.argv[1] if len(sys.argv) > 1 else os.path.join(ROOT, "docs", "sprint", "GUARD.tsv")
rows = list(csv.DictReader(open(path, newline=""), delimiter="\t"))
base = [r for r in rows if (r["note"] or "").startswith("band run")]
new = [r for r in rows if (r["note"] or "").startswith("R3 run")]
if len(base) != 3 or not new:
    print(f"FAIL: need 3 'band run' rows and >=1 'R3 run' row, got {len(base)} and {len(new)}")
    sys.exit(1)
red = 0
for m in METRICS:
    lo = min(float(r[m]) for r in base)
    hi = max(float(r[m]) for r in base)
    for r in new:
        v = float(r[m])
        tag = "BELOW" if v < lo else ("above" if v > hi else "inside")
        if tag == "BELOW":
            red += 1
        print(f"{r['row']}\t{m}\t{v:.2f}\tband[{lo:.2f},{hi:.2f}]\t{tag}")
print("RED" if red else "GREEN", f"below-band={red}")
sys.exit(1 if red else 0)
