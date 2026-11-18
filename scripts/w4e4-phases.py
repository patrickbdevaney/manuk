#!/usr/bin/env python3
"""W4.E4.a.a.b: attribute the first-navigate-in-a-pool-child cost to phases.

Starts `manuk-mcp --zygote` with MANUK_PHASE_TIMING=1 (children print `phase <name> <ms>` on stderr),
then for N forks: connect, navigate twice (nav1 = first in the child, nav2 = second), recording the
client wall time and the child's phase lines for each. Prints a per-phase table (median nav1, nav2,
delta) and checks: sum of per-phase deltas + unattributed residual == wall gap, and that the
attributed phases explain the gap to within --tol ms (default 2). Exit 1 if not.
`--break` drops one phase from the sum (a deliberately broken input: the check must go red).
"""
import argparse, importlib.util, os, statistics as st, subprocess, sys, tempfile, time
HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location("g", os.path.join(HERE, "w4e4-gate.py").replace("w4e4-gate.py", "w4e4-gate.py"))
src = open(os.path.join(HERE, "w4e4-gate.py")).read().replace("\nmain()", "\n")  # reuse helpers, do not run main
g = {"__file__": os.path.join(HERE, "w4e4-gate.py"), "__name__": "gate"}
exec(compile(src, "w4e4-gate.py", "exec"), g)
SockMcp, fixture, MCP = g["SockMcp"], g["fixture"], g["MCP"]
SUB = ["prewarm.engine_init","prewarm.runtime_new","prewarm.global_eval","prewarm.precompile"]
ORDER = ["js_realm_prewarm", "fetch_join_total", "load_async", "finish_loading", "b_navigate_total", "settled", "tree_seed_snapshot"]
# fetch_join_total and b_navigate_total CONTAIN other phases; attribute only the leaf phases:
LEAVES = ["fetch_join_total", "load_async", "finish_loading", "settled", "tree_seed_snapshot"]  # fetch_join includes prewarm; split below

def main():
    ap = argparse.ArgumentParser(); ap.add_argument("--forks", type=int, default=8); ap.add_argument("--tol", type=float, default=2.0)
    ap.add_argument("--max-nav1", type=float, default=None, help="fail unless median nav1 wall <= this (ms)"); ap.add_argument("--break", dest="brk", action="store_true"); ap.add_argument("--think-ms", type=float, default=0, help="agent think time between initialize and the first navigate"); ap.add_argument("--blank-preload", action="store_true"); ap.add_argument("--env", action="append", default=[])
    a = ap.parse_args()
    print(f"chrome_ver={os.environ.get('MANUK_CHROME_PIN','-')}")
    srv, base = fixture()
    tmp = tempfile.mkdtemp(prefix="w4e4p-"); sock = os.path.join(tmp, "z.sock"); errp = os.path.join(tmp, "err.txt")
    env = dict(os.environ, MANUK_PHASE_TIMING="1", MANUK_ACTLOG_DIR=os.path.join(tmp, "log"))
    for kv in a.env: k, v = kv.split("=", 1); env[k] = v
    os.makedirs(env["MANUK_ACTLOG_DIR"])
    ef = open(errp, "w")
    argv = [MCP, "--zygote-after-load", sock, "data:text/html,hi"] if a.blank_preload else [MCP, "--zygote", sock]
    z = subprocess.Popen(argv, env=env, stderr=ef)
    for _ in range(300):
        if os.path.exists(sock): break
        time.sleep(0.02)
    rows = []
    try:
        for i in range(a.forks + 1):
            m = SockMcp(sock); w = []; time.sleep(a.think_ms / 1000)
            for url in ("/fx", "/fx2"):
                ef.flush(); n0 = os.path.getsize(errp)
                t0 = time.monotonic(); m.call("navigate", {"url": base + url}); w.append((time.monotonic() - t0) * 1000)
                time.sleep(0.05); ef.flush()
                ph = {}
                for ln in open(errp).read()[n0:].splitlines():
                    p = ln.split()
                    if len(p) == 3 and p[0] == "phase": ph[p[1]] = float(p[2])
                ph["_wall"] = w[-1]; rows.append(ph)
            m.kill(); time.sleep(0.1)
    finally:
        z.kill(); z.wait(); srv.shutdown()
    rows = rows[2:]  # discard fork 0 (warm-up)
    n1, n2 = rows[0::2], rows[1::2]
    med = lambda rs, k: st.median(r.get(k, 0.0) for r in rs)
    # leaf decomposition of one navigate (ms): prewarm | fetch(rest of join) | load_async | finish_loading | ws+misc (b_navigate - above) | settled | tree_seed_snapshot | residual
    def parts(r):
        pre = r.get("js_realm_prewarm", 0); fj = r.get("fetch_join_total", 0)
        la, fl = r.get("load_async", 0), r.get("finish_loading", 0)
        bn = r.get("b_navigate_total", 0)
        d = {"js_realm_prewarm": pre, "first_fetch (join - prewarm)": max(fj - pre, 0), "load_async (parse/style/layout/scripts)": la,
             "finish_loading": fl, "ws_service+other in navigate": max(bn - fj - la - fl, 0),
             "settled": r.get("settled", 0), "tree_seed_snapshot": r.get("tree_seed_snapshot", 0)}
        d["residual (rpc, text fit, header)"] = r["_wall"] - sum(d.values())
        for k in SUB: d["  . " + k] = r.get(k, 0)  # sub-phases of js_realm_prewarm: shown, never summed
        return d
    P1 = [parts(r) for r in n1]; P2 = [parts(r) for r in n2]
    keys = list(P1[0].keys())
    print(f"{'phase':46s} {'nav1':>8s} {'nav2':>8s} {'delta':>8s}   (median ms over {len(n1)} forks)")
    sd = 0.0; sd_attr = 0.0
    for k in keys:
        a1, a2 = st.median(p[k] for p in P1), st.median(p[k] for p in P2)
        print(f"{k:46s} {a1:8.2f} {a2:8.2f} {a1-a2:8.2f}")
        if k.startswith("  ."): continue
        sd += a1 - a2
        if not k.startswith("residual") and not (a.brk and k == "js_realm_prewarm"): sd_attr += a1 - a2
    w1, w2 = med(n1, "_wall"), med(n2, "_wall"); gap = w1 - w2
    print(f"{'wall (client)':46s} {w1:8.2f} {w2:8.2f} {gap:8.2f}")
    print(f"sum of attributed phase deltas = {sd_attr:.2f} ms; wall gap = {gap:.2f} ms; |diff| = {abs(sd_attr-gap):.2f} ms (tol {a.tol})")
    ok = abs(sd_attr - gap) <= a.tol
    print("attribution:", "PASS" if ok else "FAIL phases do not explain the gap")
    if a.max_nav1 is not None:
        fast = w1 <= a.max_nav1
        print(f"nav1 median {w1:.2f} ms vs bar {a.max_nav1:.2f} ms:", "PASS" if fast else "FAIL nav1 did not fall")
        ok = fast
    sys.exit(0 if ok else 1)
main()
