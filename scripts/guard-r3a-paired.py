#!/usr/bin/env python3
"""GUARD.R3.a: paired a11y F1, GUARD.R3 run k vs GUARD.R0 run k, on sites OK in both (k=1..3).

Gate (GRAPH.tsv row GUARD.R3.a, fixed beforehand): GREEN only if every run's mean paired delta >= -2.0 points AND
every site moving >= 10 points has a class in CLASSES below (INSTRUMENT/ORIGIN/ENGINE + evidence). RED (exit 1) otherwise.
usage: python3 -I scripts/guard-r3a-paired.py [r0_prefix_override r3_prefix_override]  (override = mutation use)
"""
import csv, os, statistics as S, sys
R0 = ["20261024-225412", "20261024-232725", "20261024-235659"]
R3 = ["20261103-231900", "20261103-234426", "20261104-001110"]
if len(sys.argv) == 3:
    R3 = [sys.argv[2]] * 3
    R0 = [sys.argv[1]] * 3
MIN_MEAN = -2.0
CLASSES = {
 "https://hipmiluwuutara.org/": ("INSTRUMENT", "R0 F1=100 was a Chrome AX snapshot with 2 kept nodes (GUARD.R2.a: 5/5 fresh runs F1 0.0, chrome_n=14, oracle+fetch sha identical); R3 oracle sha 6a7482 and fetch sha 0e1efd identical in all 3 runs, F1 0.0 x3"),
 "https://allticketscol.com/": ("ENGINE", "improvement +16.5: R0 83.1 x3 vs R3 99.6 x3, engine fetch sha 52429b stable across R3 runs, F1 stable despite oracle sha varying; not a regression"),
 "https://admin.zoomph.com/": ("INSTRUMENT", "R0 itself flapped (0, 66.7, 66.7); R3 66.7 x3 equals the stable R0 value; oracle sha 2cf570 stable, engine fetch sha differs per run"),
 "https://www.paypal.com/": ("ORIGIN", "R3 F1 empty in 2/3 runs and 1.0 in one, oracle sha '-' in 2/3; engine fetch sha differs every run (c38d3b/133b88/1f500b): bot-gated, varying origin response"),
 "https://booking.directferries.com/": ("ORIGIN", "R0 25.0 x3 vs R3 100.0 x2 (+ one empty); engine fetch sha differs every R3 run (78e646/757827/e46f89): varying origin response; direction is up"),
}

def load(s):
    with open(f"docs/bench/tri-sweep-{s}.tsv", newline="") as f:
        return {r["url"]: r for r in csv.DictReader(f, delimiter="\t")}

bad = []; rows = []; moved = {}
for k in range(3):
    b, n = load(R0[k]), load(R3[k])
    d = {}
    for u in b:
        if u in n and b[u]["m1_status"] == "OK" and n[u]["m1_status"] == "OK" and b[u]["a11y_f1"] and n[u]["a11y_f1"]:
            d[u] = float(n[u]["a11y_f1"]) - float(b[u]["a11y_f1"])
    m = S.mean(d.values())
    rows.append((k + 1, len(d), round(m, 2)))
    print(f"run{k+1} paired_n={len(d)} mean_delta={m:.2f}")
    if m < MIN_MEAN:
        bad.append(f"run{k+1} mean {m:.2f} < {MIN_MEAN}")
    for u, x in d.items():
        if abs(x) >= 10:
            moved.setdefault(u, []).append((k + 1, round(x, 1)))
for u, mv in sorted(moved.items()):
    c = CLASSES.get(u)
    print(u, mv, c[0] if c else "UNCLASSIFIED", "|", c[1] if c else "")
    if not c:
        bad.append(f"unclassified mover {u}")
print("GREEN" if not bad else "RED " + "; ".join(bad))
sys.exit(1 if bad else 0)
