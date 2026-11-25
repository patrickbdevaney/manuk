#!/usr/bin/env python3 -I
"""CONC.2.b: registered cost bar at N=50. usage: cost_n50.py GRID.tsv CHROME_N50.tsv > out.md
Engine rows: 8 GB cage grid. Chrome rows: raised-cage deviation file (cage_gb column). PSS per successful task = peak_pss_mb / traces_pass."""
import csv, statistics as st, sys
grid = [r for r in csv.DictReader(open(sys.argv[1]), delimiter="\t") if r["n"] == "50"]
ch = [r for r in csv.DictReader(open(sys.argv[2]), delimiter="\t") if r["n"] == "50"]
def pps(rows, arm):
    x = [float(r["pss_per_ok_mb"]) for r in rows if r["arm"] == arm and r["pss_per_ok_mb"] not in ("None", "")]
    return (st.mean(x), len(x)) if x else (None, 0)
cages = sorted({r["cage_gb"] for r in ch}); cg = ",".join(cages)
assert all(r["caged"] == "False" for r in ch), "Chrome rows must be flagged caged=False (not the registered 8 GB cage)"
print(f"### CONC.2.b cost bar at N=50 (DEVIATION: Chrome arms measured in a {cg} GB cage, engine rows in the registered 8 GB cage; chrome_ver={ch[0]['chrome_ver']})\n")
print("| arm | cage GB | runs | mean PSS/ok MB | success mean |\n|---|---|---|---|---|")
best = None
for a in ["chrome-mcp", "pw-proc", "pw-shared"]:
    m, n = pps(ch, a); s = st.mean(float(r["success"]) for r in ch if r["arm"] == a)
    print(f"| {a} | {cg} | {n} | {m:.1f} | {s:.2f} |")
    if best is None or m < best[0]: best = (m, a)
for a in ["engine-proc-full", "engine-proc-lean", "engine-fork"]:
    m, n = pps(grid, a); s = st.mean(float(r["success"]) for r in grid if r["arm"] == a)
    print(f"| {a} | 8 | {n} | {m:.1f} | {s:.2f} |")
print(f"\nbest Chrome arm: {best[1]} {best[0]:.1f} MB/ok")
for a in ["engine-proc-full", "engine-proc-lean", "engine-fork"]:
    m, _ = pps(grid, a); lower = 100 * (1 - m / best[0])
    print(f"- **Cost win ({a}) {m:.1f} vs {best[0]:.1f} MB/ok: {lower:.0f}% lower (need >=30%): {'PASS' if m <= 0.7 * best[0] else 'FAIL'}** (deviation: cage {cg} GB for Chrome vs 8 GB for engine)")
