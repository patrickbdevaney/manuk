#!/usr/bin/env python3
"""PERF.1.2 gate: sum per-URL median settle (manuk), and the a11y node-count drift vs a baseline binary.

Usage: python3 -I scripts/perf1-settle-gate.py <runs.jsonl> [--nodes BASE_BIN NEW_BIN]
Exit 1 when sum settle > 11 s, or (with --nodes) any URL's a11y node count differs by > 1 % between
the two binaries (a binary that skips settle work shows up here: the tree it leaves is smaller).
"""
import collections, json, statistics, subprocess, sys

rows = [json.loads(l) for l in open(sys.argv[1]) if l.strip()]
rows = [r for r in rows if r["engine"] == "manuk" and r["status"] == "OK"]
by = collections.defaultdict(list)
for r in rows:
    by[r["url"]].append(r)
tot = sum(statistics.median(r["phases_s"]["settle"] for r in rs) for rs in by.values())
print(f"urls={len(by)} sum_settle={tot:.2f}s (bar <= 11)")
bad = tot > 11
if "--nodes" in sys.argv:
    i = sys.argv.index("--nodes")
    base, new = sys.argv[i + 1], sys.argv[i + 2]

    def count(binary, url):
        p = subprocess.run([binary, url], capture_output=True, timeout=120)
        try:
            return len(json.loads(p.stdout))
        except Exception:
            return None

    for u in sorted(by):
        a, b = count(base, u), count(new, u)
        if a is None or b is None:
            print(f"NODES {u}: unreadable base={a} new={b}")
            bad = True
            continue
        d = abs(a - b) / max(a, 1)
        flag = "RED" if d > 0.01 else "ok"
        bad |= d > 0.01
        print(f"NODES {flag} {u} base={a} new={b} drift={d*100:.2f}%")
print("GATE", "RED" if bad else "GREEN")
sys.exit(1 if bad else 0)
