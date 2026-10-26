#!/usr/bin/env python3 -I
"""Gate for docs/sprint/SECURITY.md: gate files exist; <2 layers or UNPINNED => a SEC.DID.* node exists."""
import os, re, sys
root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sec = sys.argv[1] if len(sys.argv) > 1 else os.path.join(root, "docs/sprint/SECURITY.md")
graph = sys.argv[2] if len(sys.argv) > 2 else os.path.join(root, "docs/sprint/GRAPH.tsv")
ids = {l.split("\t")[0] for l in open(graph) if l.strip()}
bad, rows = [], 0
for line in open(sec):
    if not re.match(r"\| P\w+ ", line):
        continue
    rows += 1
    c = [x.strip() for x in line.strip().strip("|").split("|")]
    pid, layers, node = c[0], c[2], c[3]
    entries = re.findall(r"([^;@]+)@([\w/.-]+\.rs|UNPINNED)", layers)
    gates = [g for _, g in entries if g != "UNPINNED"]
    for g in gates:
        if not os.path.exists(os.path.join(root, g)):
            bad.append(f"{pid}: gate file missing: {g}")
    n = len(entries)
    unpinned = any(g == "UNPINNED" for _, g in entries)
    if (n < 2 or unpinned) and not (node.startswith("SEC.DID.") and node in ids):
        bad.append(f"{pid}: {n} layers/unpinned={unpinned} but node {node!r} not in GRAPH")
if rows < 10:
    bad.append(f"only {rows} rows parsed")
for b in bad: print("RED", b)
print(f"{rows} rows, {len(bad)} problems")
sys.exit(1 if bad else 0)
