#!/usr/bin/env python3
"""W4.E2.d gate: `manuk-mcp --agent-profile=lean` navigate vs the default, on the W4.E2 fixture.

Usage: w4e2d-flag-gate.py [--bin target/release/manuk-mcp] [--port 8742]
Starts scripts/w4e2-fixture.py, then per arm runs manuk-mcp over stdio (initialize, navigate
to /index.html, EOF) and reads the `manuk-mcp: profile=... wire_bytes=... blocked=...` line the
server prints on stderr at exit. Also checks the banner line `agent profile=` on startup.
Exit 1 if: lean wire bytes are not < 10% of default's, the banner does not name the profile,
the flag arm does not report profile=lean, or the default arm reports profile=lean (env leaked).
Run with MANUK_AGENT_PROFILE unset in the environment (the script strips it).
"""
import argparse, json, os, re, subprocess, sys, time, urllib.request

ap = argparse.ArgumentParser()
ap.add_argument("--bin", default="target/release/manuk-mcp")
ap.add_argument("--port", type=int, default=8742)
ap.add_argument("--flag", default="--agent-profile=lean")
a = ap.parse_args()
here = os.path.dirname(os.path.abspath(__file__))

fx = subprocess.Popen([sys.executable, "-I", os.path.join(here, "w4e2-fixture.py"), str(a.port)])
url = f"http://127.0.0.1:{a.port}/index.html"
try:
    for _ in range(50):
        try:
            urllib.request.urlopen(url, timeout=1).read()
            break
        except Exception:
            time.sleep(0.1)

    def arm(extra):
        env = dict(os.environ)
        env.pop("MANUK_AGENT_PROFILE", None)
        args = [a.bin] + extra
        msgs = [
            {"jsonrpc": "2.0", "id": 1, "method": "initialize",
             "params": {"protocolVersion": "2025-03-26", "capabilities": {},
                        "clientInfo": {"name": "gate", "version": "0"}}},
            {"jsonrpc": "2.0", "method": "notifications/initialized"},
            {"jsonrpc": "2.0", "id": 2, "method": "tools/call",
             "params": {"name": "navigate", "arguments": {"url": url}}},
        ]
        inp = "".join(json.dumps(m) + "\n" for m in msgs)
        p = subprocess.run(args, input=inp, capture_output=True, text=True, env=env, timeout=180)
        err = p.stderr
        banner = re.findall(r"manuk-mcp: agent profile=(\w+)", err)
        stats = re.findall(r"manuk-mcp: profile=(\w+) wire_bytes=(\d+) blocked=(\d+)", err)
        nav_ok = '"id":2' in p.stdout.replace(" ", "") and '"isError":true' not in p.stdout.replace(" ", "")
        return banner, stats, nav_ok

    d_banner, d_stats, d_ok = arm([])
    l_banner, l_stats, l_ok = arm([a.flag])
    bad = 0
    for name, banner, stats, ok, want in (("default", d_banner, d_stats, d_ok, "full"),
                                          ("lean", l_banner, l_stats, l_ok, "lean")):
        print(f"{name}\tbanner={banner}\tstats={stats}\tnavigate_ok={ok}")
        if banner != [want] or len(stats) != 1 or stats[0][0] != want or not ok:
            print(f"FAIL: {name} arm did not report profile={want} with a navigate result")
            bad += 1
    if bad == 0:
        dw, lw = int(d_stats[0][1]), int(l_stats[0][1])
        ratio = lw / dw if dw else float("inf")
        print(f"wire_bytes default={dw} lean={lw} ratio={ratio:.3f} (gate: < 0.10)")
        if not ratio < 0.10:
            print("FAIL: lean did not fetch under 10% of default's subresource bytes")
            bad += 1
    sys.exit(1 if bad else 0)
finally:
    fx.terminate()
