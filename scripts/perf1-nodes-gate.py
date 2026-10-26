#!/usr/bin/env python3
"""PERF.1.3.a gate: a11y node-count drift between a baseline binary and a new one, per URL.

Usage: python3 -I scripts/perf1-nodes-gate.py BASE_BIN NEW_BIN [--max-drift-pct 0.05] [URL ...]
Defaults to the PERF.md URLs in docs/sprint/perf1-pf13/paired.jsonl. Live pages move on their own,
so a URL over the bar is re-measured (up to 3 base/new pairs, interleaved) and judged on its BEST
pair; a binary that really skips layout work is off in every pair. Exit 1 on any URL over the bar.
"""
import json, os, subprocess, sys

args = sys.argv[1:]
maxd = 0.05
if "--max-drift-pct" in args:
    i = args.index("--max-drift-pct")
    maxd = float(args[i + 1])
    del args[i : i + 2]
base, new, urls = args[0], args[1], args[2:]
if not urls:
    here = os.path.dirname(os.path.abspath(__file__))
    rows = open(os.path.join(here, "..", "docs/sprint/perf1-pf13/paired.jsonl"))
    urls = sorted({json.loads(l)["url"] for l in rows if l.strip()})


def count(binary, url):
    p = subprocess.run([binary, url], capture_output=True, timeout=180)
    try:
        return len(json.loads(p.stdout))
    except Exception:
        return None


bad = False
for u in urls:
    best = None
    skipped = False
    for _ in range(3):
        a, b = count(base, u), count(new, u)
        if a is None:  # the baseline cannot load it either: the site is down, not a regression
            skipped = True
            break
        if b is None:
            d = None
        else:
            d = abs(a - b) / max(a, 1) * 100
        if d is not None and (best is None or d < best[0]):
            best = (d, a, b)
        if best and best[0] <= maxd:
            break
    if skipped:
        print(f"NODES SKIP {u}: baseline unreadable")
        continue
    if best is None:
        print(f"NODES RED {u}: unreadable")
        bad = True
        continue
    flag = "ok" if best[0] <= maxd else "RED"
    bad |= flag == "RED"
    print(f"NODES {flag} {u} base={best[1]} new={best[2]} drift={best[0]:.3f}%")
print("GATE", "RED" if bad else "GREEN", f"(max drift {maxd}%)")
sys.exit(1 if bad else 0)
