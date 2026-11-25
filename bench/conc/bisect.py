#!/usr/bin/env python3 -I
"""CONC.2.a: capacity bisection between ladder brackets (docs/sprint/CONCURRENCY.md). Same cells/cage/rule as harness.py:
3 caged runs per N, success = real-page traces (rfc,w3c,mdn) mean, 'ok' = within 5 points of the arm's N=1 real success (1.00 in the grid);
a cell with no surviving run counts as success 0. Stops when bracket width <= max(1, lo//10). Arms are interleaved round-robin.
usage: bisect.py OUT.tsv   (env MANUK_CHROME_PIN, MANUK_MCP_BIN)"""
import json, os, statistics as st, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import harness as H
REAL = ("rfc", "w3c", "mdn")
# arm -> (lo known ok, hi known bad) from the ladder (real-page success per N in analysis.md)
BR = {"engine-proc-full": (50, 100), "engine-fork": (50, 100), "engine-proc-lean": (25, 50),
      "pw-shared": (25, 50), "chrome-mcp": (10, 25), "pw-proc": (10, 25)}
TOL = 0.05

def real(r):
    pt = r["per_trace"]; return sum(int(pt[t].split("/")[0]) for t in REAL) / sum(int(pt[t].split("/")[1]) for t in REAL)

def arm_gen(arm, out):
    lo, hi = BR[arm]
    while hi - lo > max(1, lo // 10):
        mid = (lo + hi) // 2; vals = []
        for run in range(1, 4):
            r = H.caged_cell(arm, mid, run)
            if "infra" in r:
                open(out + ".infra", "a").write(json.dumps(r) + "\n"); vals.append(0.0); print("INFRA", arm, mid, run, flush=True)
            else:
                open(out, "a").write(H.tsv(r) + "\n"); vals.append(real(r))
            print(f"{arm} N={mid} run={run} real_success={vals[-1]:.3f}", flush=True)
            yield
        ok = st.mean(vals) >= 1.0 - TOL
        print(f"== {arm} N={mid} mean {st.mean(vals):.3f} -> {'ok' if ok else 'bad'}", flush=True)
        if ok: lo = mid
        else: hi = mid
        open(out + ".brackets", "a").write(json.dumps({"arm": arm, "n": mid, "mean_real_success": round(st.mean(vals), 4), "ok": ok, "lo": lo, "hi": hi, "chrome_ver": H.pin()}) + "\n")

out = sys.argv[1]
if not os.path.exists(out): open(out, "w").write("\t".join(H.COLS) + "\n")
gens = {a: arm_gen(a, out) for a in BR}
while gens:
    for a in list(gens):
        try: next(gens[a])
        except StopIteration: del gens[a]
print("DONE", flush=True)
