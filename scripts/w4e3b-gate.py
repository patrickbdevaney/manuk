#!/usr/bin/env python3
"""W4.E3.b gate: warm-visit wire bytes on books.toscrape.com with the HTTP cache on, plus a11y digest parity.

Usage: w4e3b-gate.py [--bin target/release/manuk-mcp] [--runs 3] [--url https://books.toscrape.com/]
Per run: a fresh cache dir, a cold navigate (empty cache), then a warm navigate in a NEW process reusing
that dir (the flag `--http-cache-dir`). Reads `manuk-mcp: profile=... wire_bytes=N` from stderr and the
navigate result text from stdout. Exit 1 if: any warm run's wire bytes are not below the cold minimum,
the warm a11y text differs from the cold text, or the cache dir stayed empty (no entries written).
The measurement must be run with chrome_ver recorded alongside (see GRAPH row W4.E3.b).
Note: manuk-mcp currently segfaults at process exit (SpiderMonkey teardown, pre-existing, also with no
cache flag); the gate reads output from before the crash, and the exit code is not checked.
"""
import argparse, json, os, re, shutil, subprocess, sys, tempfile

ap = argparse.ArgumentParser()
ap.add_argument("--bin", default="target/release/manuk-mcp")
ap.add_argument("--runs", type=int, default=3)
ap.add_argument("--url", default="https://books.toscrape.com/")
a = ap.parse_args()

BAND = 700055  # the W4.E3 books.toscrape.com warm-visit band (GRAPH row W4.E3.b gate)

MSGS = [
    {"jsonrpc": "2.0", "id": 1, "method": "initialize",
     "params": {"protocolVersion": "2025-03-26", "capabilities": {},
                "clientInfo": {"name": "gate", "version": "0"}}},
    {"jsonrpc": "2.0", "method": "notifications/initialized"},
    {"jsonrpc": "2.0", "id": 2, "method": "tools/call",
     "params": {"name": "navigate", "arguments": {"url": a.url}}},
]

def run(cache_dir):
    env = dict(os.environ)
    env.pop("MANUK_AGENT_PROFILE", None)
    env.pop("MANUK_HTTP_CACHE_DIR", None)
    inp = "".join(json.dumps(m) + "\n" for m in MSGS)
    p = subprocess.run([a.bin, "--http-cache-dir", cache_dir], input=inp, capture_output=True,
                       text=True, env=env, timeout=240)
    wire = re.findall(r"wire_bytes=(\d+)", p.stderr)
    text = ""
    for line in p.stdout.splitlines():
        try:
            msg = json.loads(line)
        except ValueError:
            continue
        if msg.get("id") == 2:
            text = "".join(c.get("text", "") for c in msg.get("result", {}).get("content", []))
    # The `page: p<hex>:<n>` line is a per-process id, not page content: it differs run to run by design.
    text = "\n".join(l for l in text.splitlines() if not l.startswith("page: "))
    return int(wire[0]) if wire else None, text

bad = 0
cold_w, warm_w = [], []
root = tempfile.mkdtemp(prefix="w4e3b-gate-")
try:
    for r in range(a.runs):
        cache = os.path.join(root, f"run{r}")
        os.makedirs(cache)
        cw, ct = run(cache)
        ww, wt = run(cache)
        entries = len(os.listdir(cache))
        print(f"run{r}\tcold_wire={cw}\twarm_wire={ww}\tcache_entries={entries}\ta11y_equal={ct == wt and bool(ct)}")
        if cw is None or ww is None or entries == 0:
            print("FAIL: missing wire_bytes line or an empty cache dir (cache writes were refused?)")
            bad += 1
        if ct != wt or not ct:
            print("FAIL: warm a11y text differs from cold (or is empty)")
            bad += 1
        cold_w.append(cw or 0)
        warm_w.append(ww or 0)
finally:
    shutil.rmtree(root, ignore_errors=True)

if cold_w and warm_w:
    print(f"cold_wire={min(cold_w)}-{max(cold_w)} warm_wire={min(warm_w)}-{max(warm_w)}")
    if max(warm_w) >= min(cold_w):
        print("FAIL: warm wire bytes are not below cold")
        bad += 1
    if max(warm_w) >= BAND:
        print(f"FAIL: warm wire bytes are not below the W4.E3 band ({BAND} B)")
        bad += 1
sys.exit(1 if bad else 0)
