#!/usr/bin/env python3
"""PERF.1.1 gate: sum per-URL median subres_fetch / doc_fetch (manuk) and count URLs requested twice.

Usage: python3 -I scripts/perf1-gate.py <runs.jsonl> [traces_dir]
Exit 1 when sum subres_fetch > 24 or doc_fetch > 1.10*4.72 or any URL is on the wire twice.
"""
import collections, glob, json, os, statistics, sys

rows = [json.loads(l) for l in open(sys.argv[1]) if l.strip()]
rows = [r for r in rows if r["engine"] == "manuk" and r["status"] == "OK"]
by = collections.defaultdict(list)
for r in rows:
    by[r["url"]].append(r)
tot = {"subres_fetch": 0.0, "doc_fetch": 0.0}
for u, rs in by.items():
    for k in tot:
        tot[k] += statistics.median(r["phases_s"][k] for r in rs)
print(f"urls={len(by)} sum_subres_fetch={tot['subres_fetch']:.2f}s sum_doc_fetch={tot['doc_fetch']:.2f}s")
dupes = 0
if len(sys.argv) > 2:
    for f in sorted(glob.glob(os.path.join(sys.argv[2], "*.tsv"))):
        c = collections.Counter()
        for l in open(f):
            p = l.rstrip("\n").split("\t")
            if len(p) > 2 and p[1] == "req" and (len(p) < 9 or p[8] == "GET"):
                c[p[2]] += 1
        d = {u: n for u, n in c.items() if n > 1}
        if d:
            dupes += sum(n - 1 for n in d.values())
            print("DUP", os.path.basename(f), len(d), list(d)[:2])
    print(f"duplicate wire requests (all traces) = {dupes}")
bad = tot["subres_fetch"] > 24 or tot["doc_fetch"] > 4.72 * 1.10 or dupes
print("GATE", "RED" if bad else "GREEN")
sys.exit(1 if bad else 0)
