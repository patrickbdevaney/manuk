#!/usr/bin/env python3
"""W4.A6.a.a gate: re-measure manuk own suite x3 on the viewport-fixed binary.
Reads the own3 TSV written by bench/agent-tasks/run_arm.py (arm=manuk, suite own, 40 tasks x 3 runs).
Pass criteria (from GRAPH row W4.A6.a.a): every row on the single chrome_ver; 40 tasks x 3 runs present;
L12 passes in >=2 of 3 runs; F08 reported as-is (no threshold); tasks whose pass differs from the
2026-10-31 SCOREBOARD baseline (suite 69903c7d) are named. Exit 0 iff the structure is sound AND L12 >= 2/3.
  python3 -I gate.py [TSV] [--l12-bar N]   (--l12-bar 4 is the red check: impossible bar, must exit 1)
"""
import csv, sys, os
from collections import defaultdict
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(HERE)))
PIN = "145.0.7632.116"
SUITE = "69903c7d53b2856e5f993f8e510a32173584da3bfadd2b099693419bfd6d5f5e"
args = [a for i, a in enumerate(sys.argv[1:]) if not a.startswith("--") and (i == 0 or sys.argv[i] != "--l12-bar")]
path = args[0] if args else os.path.join(HERE, "own3.tsv")
bar = int(sys.argv[sys.argv.index("--l12-bar") + 1]) if "--l12-bar" in sys.argv else 2
rows = [r for r in csv.DictReader(open(path), delimiter="\t") if r["arm"] == "manuk"]
if not rows or any(r["chrome_ver"] != PIN for r in rows):
    print("FAIL: missing rows or chrome_ver not the single pin"); sys.exit(1)
if any(r["suite_hash"] != SUITE for r in rows):
    print("FAIL: suite hash differs from own suite 69903c7d"); sys.exit(1)
cell = {}
for r in rows:
    k = (r["run"], r["task"])
    if k in cell:
        print("FAIL: duplicate row", k); sys.exit(1)
    cell[k] = r
runs = sorted({k[0] for k in cell}); tasks = sorted({k[1] for k in cell})
if len(runs) != 3 or len(tasks) != 40 or len(cell) != 120:
    print(f"FAIL: need 40 tasks x 3 runs, got runs={runs} tasks={len(tasks)} rows={len(cell)}"); sys.exit(1)
# Baseline: the 2026-10-31 manuk own-suite rows in SCOREBOARD (same suite, same chrome_ver)
base = defaultdict(dict)
for r in csv.DictReader(open(os.path.join(ROOT, "docs/sprint/SCOREBOARD.tsv")), delimiter="\t"):
    if r["arm"] == "manuk" and r["suite_hash"] == SUITE and r["ts"].startswith("2026-10-31") and r["slice"] == "own":
        base[r["run"]][r["task"]] = int(r["pass"])
print("run  pass/40  (2026-10-31 baseline)")
for run in runs:
    p = sum(int(cell[(run, t)]["pass"]) for t in tasks)
    b = sum(base[run].values()) if base.get(run) else "n/a"
    print(f"{run}    {p}/40   ({b})")
print("per-task pass (of 3 runs) for tasks F08, L12, and any task whose pass count moved vs baseline:")
moved = []
for t in tasks:
    now = [int(cell[(r, t)]["pass"]) for r in runs]
    was = [base[r].get(t) for r in runs]
    if t in ("F08", "L12") or (None not in was and sum(now) != sum(was)):
        print(f"  {t}: now {now} ({sum(now)}/3)  baseline {was}")
    if None not in was and sum(now) != sum(was):
        moved.append(t)
print("moved vs baseline:", ", ".join(moved) if moved else "none")
l12 = sum(int(cell[(r, "L12")]["pass"]) for r in runs)
print(f"L12 passes: {l12}/3 (bar {bar}/3)")
ok = l12 >= bar
print("PASS" if ok else "RED")
sys.exit(0 if ok else 1)
