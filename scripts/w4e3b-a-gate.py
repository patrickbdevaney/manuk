#!/usr/bin/env python3
"""W4.E3.b.a gate: `wire_bytes` counts only bytes that crossed the wire, checked against a network trace.

Usage: w4e3b-a-gate.py [--bin target/release/manuk-mcp] [--runs 3] [--url https://books.toscrape.com/]

Per run: a fresh cache dir, a cold navigate (empty cache), then a warm navigate in a NEW process reusing that
dir (`--http-cache-dir`). Each process writes a per-request trace (`MANUK_NET_TRACE`). The reference is the sum of
the trace's `body` events, which record the decoded length of every body the net layer actually read off the
wire, and the HTTP status of that response (a `304` reads no body, so it contributes 0). The reported
`wire_bytes=N` (stderr) must equal that sum exactly, in both the cold and the warm process.

Exit 1 if, in any run: `wire_bytes` differs from the trace sum (either process); the warm a11y text differs from
the cold text or is empty; or the cache dir stayed empty. A warm process with no `304` is reported, not failed: on
books.toscrape.com every warm response is fresh, so the 304 accounting is covered by tests/g_wire_bytes.rs instead. The exit code of manuk-mcp itself is not checked (pre-existing
SIGSEGV at exit, W4.E3.b.b).

Baseline (chrome_ver=145.0.7632.116, pre-fix binary): cold wire_bytes 792684 vs trace body sum 843978; the 51294 B
difference is exactly the top-level document, which was never counted. The gate is red on that.
"""
import argparse, json, os, re, shutil, subprocess, sys, tempfile, threading

ap = argparse.ArgumentParser()
ap.add_argument("--bin", default="target/release/manuk-mcp")
ap.add_argument("--runs", type=int, default=3)
ap.add_argument("--url", default="https://books.toscrape.com/")
a = ap.parse_args()

MSGS = [
    {"jsonrpc": "2.0", "id": 1, "method": "initialize",
     "params": {"protocolVersion": "2025-03-26", "capabilities": {},
                "clientInfo": {"name": "gate", "version": "0"}}},
    {"jsonrpc": "2.0", "method": "notifications/initialized"},
    {"jsonrpc": "2.0", "id": 2, "method": "tools/call",
     "params": {"name": "navigate", "arguments": {"url": a.url}}},
]


def trace_sum(path):
    """(sum of body lengths, count of 304 bodies) from a MANUK_NET_TRACE file."""
    total, nots = 0, 0
    if not os.path.exists(path):
        return None, None
    with open(path, encoding="utf-8", errors="replace") as f:
        for line in f:
            parts = line.rstrip("\n").split("\t")
            if len(parts) < 3 or parts[1] != "body":
                continue
            # `t \t body \t url \t len [\t status]`; the URL never contains a tab.
            try:
                n = int(parts[3])
            except (IndexError, ValueError):
                continue
            total += n
            if len(parts) >= 5 and parts[4] == "304":
                nots += 1
    return total, nots


def run(cache_dir, trace_path):
    env = dict(os.environ)
    env.pop("MANUK_AGENT_PROFILE", None)
    env.pop("MANUK_HTTP_CACHE_DIR", None)
    env["MANUK_NET_TRACE"] = trace_path
    # The trace file is outside the cache dir, so it needs an explicit write grant (confined process).
    p = subprocess.Popen([a.bin, "--http-cache-dir", cache_dir, "--allow-write", os.path.dirname(trace_path)],
                         stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                         text=True, env=env)
    # Keep stdin open until the navigate answer arrives: closing it first lets the server shut down with the
    # tool call still in flight, and the answer is lost.
    kill = threading.Timer(240, p.kill)
    kill.start()
    out_lines = []
    try:
        for m in MSGS:
            p.stdin.write(json.dumps(m) + "\n")
        p.stdin.flush()
        for line in p.stdout:
            out_lines.append(line)
            try:
                if json.loads(line).get("id") == 2:
                    break
            except ValueError:
                continue
        p.stdin.close()
        err = p.stderr.read()
        p.wait()
    finally:
        kill.cancel()
    wire = re.findall(r"wire_bytes=(\d+)", err)
    text = ""
    for line in out_lines:
        try:
            msg = json.loads(line)
        except ValueError:
            continue
        if msg.get("id") == 2:
            text = "".join(c.get("text", "") for c in msg.get("result", {}).get("content", []))
    # The `page: p<hex>:<n>` line is a per-process id, not page content: it differs run to run by design.
    text = "\n".join(l for l in text.splitlines() if not l.startswith("page: "))
    return (int(wire[0]) if wire else None), text


bad = 0
cold_w, warm_w = [], []
root = tempfile.mkdtemp(prefix="w4e3b-a-gate-")
try:
    for r in range(a.runs):
        cache = os.path.join(root, f"run{r}")
        os.makedirs(cache)
        tdir = os.path.join(root, f"traces{r}")
        os.makedirs(tdir)
        tc, tw = os.path.join(tdir, "cold.trace"), os.path.join(tdir, "warm.trace")
        cw, ct = run(cache, tc)
        ww, wt = run(cache, tw)
        csum, _ = trace_sum(tc)
        wsum, wnots = trace_sum(tw)
        entries = len(os.listdir(cache))
        print(f"run{r}\tcold_wire={cw}\tcold_trace={csum}\twarm_wire={ww}\twarm_trace={wsum}"
              f"\twarm_304={wnots}\tcache_entries={entries}\ta11y_equal={ct == wt and bool(ct)}")
        if cw is None or ww is None or entries == 0:
            print("FAIL: missing wire_bytes line or an empty cache dir (cache writes were refused?)")
            bad += 1
        if csum is None or wsum is None:
            print("FAIL: missing network trace (MANUK_NET_TRACE not written)")
            bad += 1
        else:
            if cw != csum:
                print(f"FAIL: cold wire_bytes {cw} != trace body sum {csum}")
                bad += 1
            if ww != wsum:
                print(f"FAIL: warm wire_bytes {ww} != trace body sum {wsum} (304s counted as bodies?)")
                bad += 1
            if not wnots:
                # books.toscrape.com's warm visit is all fresh cache hits (no conditional request leaves the
                # process), so the 304 path is not exercised live here; tests/g_wire_bytes.rs claim B covers it.
                print("NOTE: warm run issued no conditional request (every response fresh from cache)")
        if ct != wt or not ct:
            print("FAIL: warm a11y text differs from cold (or is empty)")
            bad += 1
        cold_w.append(cw or 0)
        warm_w.append(ww or 0)
finally:
    shutil.rmtree(root, ignore_errors=True)

if cold_w and warm_w:
    print(f"cold_wire={min(cold_w)}-{max(cold_w)} warm_wire={min(warm_w)}-{max(warm_w)}")
sys.exit(1 if bad else 0)
