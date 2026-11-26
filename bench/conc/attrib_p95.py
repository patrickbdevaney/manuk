#!/usr/bin/env python3 -I
"""AXES.1.a: per-call trace TSV (harness CONC_CALLS_OUT) -> p95 attribution table.
usage: attrib_p95.py CALLS.tsv > out.md
cols: chrome_ver arm n run agent trace step_kind phase op ms.  p95 uses the harness's pct() (nearest rank)."""
import csv, sys, collections
COLS = "chrome_ver arm n run agent trace step_kind phase op ms".split()
rows = [dict(zip(COLS, r)) for r in csv.reader(open(sys.argv[1]), delimiter="\t") if r]
def pct(xs, q):
    xs = sorted(xs); return xs[min(len(xs) - 1, int(round(q * (len(xs) - 1))))]
vers = {r["chrome_ver"] for r in rows}
cells = sorted({(r["n"], r["arm"]) for r in rows}, key=lambda c: (int(c[0]), c[1]))
out = [f"# AXES.1.a p95 attribution (chrome_ver={','.join(sorted(vers))})\n"]
for n, arm in cells:
    cr = [r for r in rows if r["n"] == n and r["arm"] == arm]
    allms = [float(r["ms"]) for r in cr]; p95 = pct(allms, .95)
    tail = [r for r in cr if float(r["ms"]) >= p95]
    out.append(f"## {arm} N={n}: {len(cr)} calls, p50 {pct(allms,.5):.1f} ms, p95 {p95:.1f} ms\n")
    out.append("| op | calls | p50 ms | p95 ms | share of calls >= overall p95 |\n|---|---|---|---|---|")
    for op in sorted({r["op"] for r in cr}):
        v = [float(r["ms"]) for r in cr if r["op"] == op]
        out.append(f"| {op} | {len(v)} | {pct(v,.5):.1f} | {pct(v,.95):.1f} | {sum(1 for r in tail if r['op']==op)}/{len(tail)} |")
    out.append("\n| trace:op:phase | calls | p95 ms | share of tail |\n|---|---|---|---|")
    groups = collections.defaultdict(list)
    for r in cr: groups[f"{r['trace']}:{r['op']}:{r['phase']}"].append(float(r["ms"]))
    top = sorted(groups.items(), key=lambda kv: -sum(1 for r in tail if f"{r['trace']}:{r['op']}:{r['phase']}" == kv[0]))[:6]
    for k, v in top:
        out.append(f"| {k} | {len(v)} | {pct(v,.95):.1f} | {sum(1 for r in tail if f'{r['trace']}:{r['op']}:{r['phase']}' == k)}/{len(tail)} |")
    out.append("")
print("\n".join(out))
