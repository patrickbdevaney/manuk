#!/usr/bin/env python3 -I
"""CONC.5: CONC.2 (before) vs CONC.5 (after) side by side. usage: compare.py GRID2 GRID5 > out.md
Per (arm, N): mean success, mean peak PSS MB, mean ok traces/min, mean p95 ms over surviving runs; 'x' = no surviving run (OOM-killed cage)."""
import csv, statistics as st, sys
NS = [1, 5, 10, 25, 50, 100]
ARMS = ["engine-proc-full", "engine-proc-lean", "engine-fork", "chrome-mcp", "pw-proc", "pw-shared"]
G = [list(csv.DictReader(open(p), delimiter="\t")) for p in sys.argv[1:3]]
def m(rows, a, n, k):
    v = [float(r[k]) for r in rows if r["arm"] == a and int(r["n"]) == n and r[k] not in ("", "None")]
    return st.mean(v) if v else None
for title, key, fmt in (("Success", "success", "{:.2f}"), ("Peak PSS MB", "peak_pss_mb", "{:.0f}"),
                        ("Successful traces per minute", "ok_per_min", "{:.1f}"), ("p95 ms per call", "p95_ms", "{:.0f}"), ("CPU seconds", "cpu_s", "{:.0f}")):
    print(f"\n#### {title}: CONC.2 -> CONC.5 (mean over surviving runs)\n")
    print("| arm | " + " | ".join(f"N={n}" for n in NS) + " |\n|---|" + "---|" * len(NS))
    for a in ARMS:
        c = []
        for n in NS:
            x = [m(g, a, n, key) for g in G]
            c.append(" -> ".join("x" if v is None else fmt.format(v) for v in x))
        print(f"| {a} | " + " | ".join(c) + " |")
