#!/usr/bin/env python3
"""RG.2.S1.b.a: per-task pass tally for an AHEAD x3 TSV (real_run.py --judge output).

  python3 -I bench/agent-tasks/rg_ahead_tally.py FILE.tsv [--mutate-flip TASK]

Prints each task's passes out of its rows, the count of tasks with >=2 of 3 passes, and
the chrome_ver / judge_model values seen (a mixed set means the file must not be diffed).
--mutate-flip TASK flips that task's first row's pass value in memory; the printed count
must change, which is the gate's red control.
"""
import collections, csv, sys

args = sys.argv[1:]
path = args[0]
flip = args[args.index("--mutate-flip") + 1] if "--mutate-flip" in args else None
rows = list(csv.DictReader(open(path), delimiter="\t"))
per = collections.defaultdict(list)
for r in rows:
    per[r["task"]].append([r["run"], r["pass"] == "1"])
if flip:
    per[flip][0][1] = not per[flip][0][1]
for t in sorted(per):
    runs = sorted(per[t])
    print(f"{t}\t{sum(p for _, p in runs)}/{len(runs)}")
hits = sum(1 for t in per if sum(p for _, p in per[t]) >= 2)
print(f"tasks_pass_ge_2_of_3\t{hits}/{len(per)}")
print("chrome_ver\t" + ",".join(sorted({r["chrome_ver"] for r in rows})))
print("judge_model\t" + ",".join(sorted({r["judge_model"] for r in rows})))
