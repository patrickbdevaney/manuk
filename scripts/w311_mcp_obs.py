#!/usr/bin/env python3
"""W3.11 helper: one fresh manuk-mcp process (--deterministic), navigate each URL, print the raw
observation text of every page separated by a marker. Used by w311-xproc-gate.sh."""
import json, subprocess, sys
bin_, urls = sys.argv[1], sys.argv[2:]
p = subprocess.Popen([bin_, "--deterministic"], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                     stderr=subprocess.DEVNULL, text=True)
n = 0
def rpc(method, params, notify=False):
    global n
    m = {"jsonrpc": "2.0", "method": method}
    if params is not None: m["params"] = params
    if not notify:
        n += 1; m["id"] = n
    p.stdin.write(json.dumps(m) + "\n"); p.stdin.flush()
    if notify: return None
    while True:
        r = json.loads(p.stdout.readline())
        if r.get("id") == n: return r
rpc("initialize", {"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "w311", "version": "0"}})
rpc("notifications/initialized", None, notify=True)
for u in urls:
    r = rpc("tools/call", {"name": "navigate", "arguments": {"url": u}})
    text = "\n".join(b.get("text", "") for b in r["result"]["content"])
    print("=== " + u.rsplit("/", 1)[-1]); print(text)
p.kill()
