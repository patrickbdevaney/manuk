#!/usr/bin/env python3
"""Gate for W5.L2.a.a.a.a: checks L2a-a-a-why-no-restore.md against the raw transcripts.

Exit 0 only if: six table rows; each run is a failure run in L2a-a-a-runs.tsv; each quoted final text
appears verbatim in that run's assistant text blocks; the class counts match the rows.
Usage: python3 -I L2a-a-a-why-check.py [path/to/why.md] [dir with runs.tsv and transcripts/]
"""
import csv
import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
md_path = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "L2a-a-a-why-no-restore.md")
base = sys.argv[2] if len(sys.argv) > 2 else HERE
runs_tsv = os.path.join(base, "L2a-a-a-runs.tsv")
tx_dir = os.path.join(base, "L2a-a-a-transcripts")

errors = []

# failure runs = runs where the declined-charge failure was hit (undo line shown, not success)
failure_runs = set()
with open(runs_tsv) as f:
    for row in csv.DictReader(f, delimiter="\t"):
        if row["success"] == "0" and row["undo_lines"] == "1":
            failure_runs.add(int(row["run"]))

rows = []
for line in open(md_path, encoding="utf-8"):
    if not line.startswith("| ") or line.startswith("| run ") or line.startswith("|---"):
        continue
    cells = [c.strip() for c in line.strip().strip("|").split("|")]
    if len(cells) != 4:
        continue
    rows.append(cells)

if len(rows) != 6:
    errors.append(f"expected 6 table rows, found {len(rows)}")

counts = {}
seen_runs = []
for run_s, tool, quote_cell, cls in rows:
    run = int(run_s)
    seen_runs.append(run)
    counts[cls] = counts.get(cls, 0) + 1
    if run not in failure_runs:
        errors.append(f"run {run} is not a failure run in runs.tsv")
    m = re.fullmatch(r'"(.+)"', quote_cell)
    if not m:
        errors.append(f"run {run}: no quoted final text")
        continue
    quote = m.group(1)
    path = os.path.join(tx_dir, f"manuk_L2a_r{run}.jsonl")
    texts = []
    for raw in open(path, encoding="utf-8"):
        d = json.loads(raw)
        if d.get("type") == "assistant":
            for c in d["message"]["content"]:
                if c.get("type") == "text":
                    texts.append(c["text"])
    if not any(quote in t for t in texts):
        errors.append(f"run {run}: quote not found verbatim in transcript: {quote!r}")
    if run not in failure_runs or not m:
        continue

if sorted(seen_runs) != sorted(failure_runs):
    errors.append(f"rows cover runs {sorted(seen_runs)}, failure runs are {sorted(failure_runs)}")

expected = {"C1": 6, "C2": 0, "C3": 0}
for k, v in expected.items():
    if counts.get(k, 0) != v:
        errors.append(f"class {k}: table has {counts.get(k, 0)}, document claims {v}")

if errors:
    for e in errors:
        print("RED:", e)
    sys.exit(1)
print(f"GREEN: {len(rows)} rows, runs {sorted(seen_runs)}, classes {dict(sorted(counts.items()))}")
sys.exit(0)
