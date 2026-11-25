#!/usr/bin/env python3 -I
"""CONC.2 analysis: grid tsv (+ .infra) -> bands, ladder capacity, pss/ok at N=50, verdicts exactly per CONCURRENCY-PLAN.md.
usage: analyze.py GRID.tsv > out.md.  Cells with no surviving run count as success 0 for capacity (as harness capacity does)."""
import csv, json, os, sys, statistics as st
g = sys.argv[1]
rows = list(csv.DictReader(open(g), delimiter="\t"))
infra = [json.loads(l) for l in open(g + ".infra")] if os.path.exists(g + ".infra") else []
NS = [1, 5, 10, 25, 50, 100]
ARMS = ["engine-proc-full", "engine-proc-lean", "engine-fork", "chrome-mcp", "pw-proc", "pw-shared"]
CHROME = ["chrome-mcp", "pw-proc", "pw-shared"]
def cell(a, n): return [r for r in rows if r["arm"] == a and int(r["n"]) == n]
def inf(a, n): return [i for i in infra if i["arm"] == a and i["n"] == n]
def band(v):
    return "-" if not v else f"{st.mean(v):.2f} [{min(v):.2f}-{max(v):.2f}] n={len(v)}"
def fl(r, k): return float(r[k])
out = []
out.append("## Success (mean [min-max] over surviving runs; infra = cells OOM-killed by the 8 GB cage; counted as success 0 in capacity)\n")
out.append("| arm | " + " | ".join(f"N={n}" for n in NS) + " |\n|---|" + "---|" * len(NS))
succ = {}
for a in ARMS:
    cs = []
    for n in NS:
        v = [fl(r, "success") for r in cell(a, n)]
        succ[a, n] = st.mean(v) if v else 0.0
        cs.append(band(v) + (f" (+{len(inf(a,n))} infra)" if inf(a, n) else ""))
    out.append(f"| {a} | " + " | ".join(cs) + " |")
def metric(title, key, fmt="{:.0f}"):
    out.append(f"\n## {title} (mean [min-max])\n")
    out.append("| arm | " + " | ".join(f"N={n}" for n in NS) + " |\n|---|" + "---|" * len(NS))
    for a in ARMS:
        cs = []
        for n in NS:
            v = [fl(r, key) for r in cell(a, n) if r[key] not in ("None", "")]
            cs.append("-" if not v else f"{fmt.format(st.mean(v))} [{fmt.format(min(v))}-{fmt.format(max(v))}]")
        out.append(f"| {a} | " + " | ".join(cs) + " |")
metric("Peak PSS of process tree, MB", "peak_pss_mb")
metric("Successful traces per minute", "ok_per_min", "{:.1f}")
metric("p95 ms per tool call", "p95_ms")
metric("CPU seconds", "cpu_s", "{:.0f}")
metric("PSS per successful trace, MB", "pss_per_ok_mb", "{:.0f}")
out.append("\n## Agent capacity (largest ladder N with mean success within 5 points of the arm's N=1; ladder only, no bisection)\n")
cap = {}
for a in ARMS:
    base = succ[a, 1]; c = 1
    for n in NS[1:]:
        if succ[a, n] >= base - 0.05: c = n
        else: break
    cap[a] = c
    out.append(f"- {a}: capacity {c} (N=1 success {base:.2f})")
REAL = ("rfc", "w3c", "mdn")
def rsucc(a, n):
    rs = cell(a, n)
    if not rs: return 0.0
    x = []
    for r in rs:
        pt = json.loads(r["per_trace"]); k = sum(int(pt[t].split("/")[0]) for t in REAL); d = sum(int(pt[t].split("/")[1]) for t in REAL); x.append(k / d)
    return st.mean(x)
out.append("\n## Agent capacity on real-page traces only (rfc, w3c, mdn; the registered basis)\n")
rcap = {}
for a in ARMS:
    base = rsucc(a, 1); c = 1
    for n in NS[1:]:
        if rsucc(a, n) >= base - 0.05: c = n
        else: break
    rcap[a] = c; out.append(f"- {a}: capacity {c} (N=1 {base:.2f}; by N: " + ", ".join(f"{n}:{rsucc(a,n):.2f}" for n in NS) + ")")
cap_all = cap; cap = rcap
# bisection rows (CONC.2.a): optional 2nd arg = capacity.tsv (+ .infra); infra (OOM-killed) runs count as success 0 per run
brows = list(csv.DictReader(open(sys.argv[2]), delimiter="\t")) if len(sys.argv) > 2 else []
binfra = [json.loads(l) for l in open(sys.argv[2] + ".infra")] if len(sys.argv) > 2 and os.path.exists(sys.argv[2] + ".infra") else []
def rreal(r):
    pt = json.loads(r["per_trace"]); return sum(int(pt[t].split("/")[0]) for t in REAL) / sum(int(pt[t].split("/")[1]) for t in REAL)
def bsucc(a, n):
    xs = [rreal(r) for r in rows + brows if r["arm"] == a and int(r["n"]) == n] + [0.0] * len([i for i in infra + binfra if i["arm"] == a and i["n"] == n])
    return st.mean(xs) if xs else None
def brackets(a):
    """-> (lo, hi): lo = largest tested N holding success within 5 points of N=1 (scanning up, stop at first miss), hi = that first miss (None if none)."""
    base = bsucc(a, 1); lo, hi = 1, None
    for n in sorted({int(r["n"]) for r in rows + brows if r["arm"] == a} | {i["n"] for i in infra + binfra if i["arm"] == a}):
        if n == 1: continue
        if bsucc(a, n) >= base - 0.05: lo = n
        else: hi = n; break
    return lo, hi
if brows:
    out.append("\n## Bisection (CONC.2.a): real-page success by tested N, OOM-killed runs counted 0; bracket = [largest ok N, first bad N)\n")
    cap = {}
    for a in ARMS:
        lo, hi = brackets(a); cap[a] = (lo, hi)
        ns = sorted({int(r["n"]) for r in rows + brows if r["arm"] == a} | {i["n"] for i in infra + binfra if i["arm"] == a})
        out.append(f"- {a}: capacity in [{lo},{(hi - 1) if hi else 'inf'}] ({'width %d' % (hi - 1 - lo) if hi else 'open'}); by N: " + ", ".join(f"{n}:{bsucc(a, n):.2f}" for n in ns))
    bestc = max(CHROME, key=lambda a: cap[a][0])
    out.append(f"\n## Pre-registered bars (after bisection)\n\nbest Chrome arm by capacity: {bestc} (in [{cap[bestc][0]},{cap[bestc][1] - 1}]).")
    for a in ["engine-proc-full", "engine-proc-lean", "engine-fork"]:
        lo_a, hi_a, lo_b, hi_b = cap[a][0], (cap[a][1] - 1 if cap[a][1] else None), cap[bestc][0], cap[bestc][1] - 1
        sure = lo_a / hi_b >= 1.5; possible = hi_a is None or hi_a / lo_b >= 1.5
        verdict = "PASS" if sure else ("UNRESOLVED (bracket still straddles 1.5x)" if possible else "FAIL")
        out.append(f"- **Capacity win ({a}): capacity in [{lo_a},{hi_a}] vs {bestc} in [{lo_b},{hi_b}]; ratio between {lo_a/hi_b:.2f}x and {(hi_a or 999)/lo_b:.2f}x (need >=1.5x): {verdict}**")
else:
    bestc = max(CHROME, key=lambda a: cap[a])
    out.append(f"\n## Pre-registered bars\n\nbest Chrome arm by capacity: {bestc} ({cap[bestc]}). capacity above uses real-page traces; ladder resolution only.")
    def hi(c): return (NS[NS.index(c) + 1] - 1) if c != NS[-1] else None  # largest N the true capacity could be (ladder only)
    for a in ["engine-proc-full", "engine-proc-lean", "engine-fork"]:
        lo_a, hi_a, lo_b, hi_b = cap[a], hi(cap[a]), cap[bestc], hi(cap[bestc])
        sure = hi_b is not None and lo_a / hi_b >= 1.5
        possible = hi_a is None or hi_a / lo_b >= 1.5
        verdict = "PASS" if sure else ("UNRESOLVED (ladder bracket straddles 1.5x; bisection needed)" if possible else "FAIL")
        out.append(f"- **Capacity win ({a}): capacity in [{lo_a},{hi_a}] vs {bestc} in [{lo_b},{hi_b}]; ratio between {lo_a/hi_b if hi_b else 0:.2f}x and {(hi_a or 999)/lo_b:.2f}x (need >=1.5x): {verdict}**")
def v(name, ok): out.append(f"- **{name}: {'PASS' if ok else 'FAIL'}**")
def pps(a):
    x = [fl(r, "pss_per_ok_mb") for r in cell(a, 50) if r["pss_per_ok_mb"] not in ("None", "")]
    return st.mean(x) if x else None
bc = [(pps(a), a) for a in CHROME if pps(a)]
bc = [x for x in bc if x]
if bc:
    b = min(bc); out.append(f"\nbest Chrome arm by PSS/ok at N=50: {b[1]} {b[0]:.0f} MB")
    for a in ["engine-proc-full", "engine-proc-lean", "engine-fork"]:
        p = pps(a)
        if p is None: out.append(f"- Cost win ({a}): no N=50 data -> FAIL"); continue
        v(f"Cost win ({a}) {p:.0f} MB/ok vs {b[0]:.0f}: {100*(1-p/b[0]):.0f}% lower (need >=30%)", p <= 0.7 * b[0])
else: out.append("- **Cost win (registered, N=50): NOT EVALUABLE - no Chrome arm survived N=50 in the 8 GB cage (24 cells OOM-killed; journal shows 24 oom-kill scopes). Not scored as a pass.**")
out.append("\nExploratory, NOT the registered test (registered N=50 is unmeasurable: every Chrome arm is OOM-killed at N=50 in the 8 GB cage). Largest N where a Chrome arm survived all runs (pw-shared N=25):")
for a in ["engine-proc-full", "engine-proc-lean", "engine-fork"]:
    p, q = pps(a) if False else None, None
def pps_n(a, n):
    x = [fl(r, "pss_per_ok_mb") for r in cell(a, n) if r["pss_per_ok_mb"] not in ("None", "")]
    return st.mean(x) if x else None
for a in ["engine-proc-full", "engine-proc-lean", "engine-fork"]:
    out.append(f"- {a} N=25: {pps_n(a,25):.0f} MB/ok vs pw-shared {pps_n('pw-shared',25):.0f}: {100*(1-pps_n(a,25)/pps_n('pw-shared',25)):.0f}% lower")
out.append("\nSuccess within 5 points of best Chrome arm at the same N (n/a where every Chrome run was OOM-killed):")
for a in ["engine-proc-full", "engine-proc-lean", "engine-fork"]:
    for n in NS:
        if not any(cell(c, n) for c in CHROME): out.append(f"- {a} N={n}: {succ[a,n]:.2f} vs best Chrome n/a (all Chrome arms OOM-killed)"); continue
        bs = max(succ[c, n] for c in CHROME)
        out.append(f"- {a} N={n}: {succ[a,n]:.2f} vs best Chrome {bs:.2f} -> {'ok' if succ[a,n] >= bs-0.05 else 'BELOW'}")
out.append("\n## Infra / died cells (listed, not dropped)\n")
for i in infra: out.append(f"- {i['arm']} N={i['n']} run={i['run']}: {i['infra'][:120]}")
print("\n".join(out))
