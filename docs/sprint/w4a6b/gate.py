#!/usr/bin/env python3
"""W4.A6.b gate: re-measure the cdp-layer own suite x3 at HEAD (post viewport fix).
Reads the TSV written by bench/agent-tasks/run_arm.py (arm=cdp-layer, suite own, 40 tasks x 3 runs).
Pass criteria (GRAPH row W4.A6.b): every row on the single chrome_ver; suite 69903c7d; 40 tasks x 3 runs
present with no duplicates; red if any run has < 39 passes or the file is truncated. The 2026-10-31
cdp-layer run (40/40/40, suite 69903c7d, pin 145.0.7632.116) is printed as the comparison, with per-task moves.
  python3 -I gate.py [TSV] [--min-pass N]   (--min-pass 41 is the red check: impossible bar, must exit 1)
"""
import csv, sys, os
from collections import defaultdict
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(HERE)))
PIN = "145.0.7632.116"
SUITE = "69903c7d53b2856e5f993f8e510a32173584da3bfadd2b099693419bfd6d5f5e"
argv = sys.argv[1:]
bar = int(argv[argv.index("--min-pass") + 1]) if "--min-pass" in argv else 39
pos = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--min-pass")]
path = pos[0] if pos else os.path.join(HERE, "own3-cdp.tsv")
rows = [r for r in csv.DictReader(open(path), delimiter="\t") if r["arm"] == "cdp-layer"]
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
base = defaultdict(dict)
for r in csv.DictReader(open(os.path.join(ROOT, "docs/sprint/SCOREBOARD.tsv")), delimiter="\t"):
    if r["arm"] == "cdp-layer" and r["suite_hash"] == SUITE and r["ts"].startswith("2026-10-31") and r["slice"] == "own":
        base[r["run"]][r["task"]] = int(r["pass"])
print("run  pass/40  (2026-10-31 baseline)")
per_run = []
for run in runs:
    p = sum(int(cell[(run, t)]["pass"]) for t in tasks)
    per_run.append(p)
    b = sum(base[run].values()) if base.get(run) else "n/a"
    print(f"{run}    {p}/40   ({b})")
print("band:", min(per_run), "-", max(per_run))
moved = []
for t in tasks:
    now = [int(cell[(r, t)]["pass"]) for r in runs]
    was = [base[r].get(t) for r in runs]
    if None not in was and sum(now) != sum(was):
        moved.append(f"{t} now {now} vs baseline {was}")
print("tasks whose pass count moved vs 2026-10-31:", "; ".join(moved) if moved else "none")
ok = min(per_run) >= bar
print(f"min run {min(per_run)}/40 (bar {bar}/40)")
print("PASS" if ok else "RED")
sys.exit(0 if ok else 1)
