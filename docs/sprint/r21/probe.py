#!/usr/bin/env python3
"""R2.1 probe: drive manuk-mcp against the bench fixtures and print per-call response bytes.
Usage: python3 -I probe.py <scenario>   (scenarios: tools, f01, f06, f07, f08, f10, l11, l12)"""
import json, os, select, subprocess, sys
HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", "..", ".."))
sys.path.insert(0, os.path.join(REPO, "bench", "agent-tasks"))
import fixtures
BIN = os.path.join(REPO, "target", "release", "manuk-mcp")

class Mcp:
    def __init__(s):
        s.p = subprocess.Popen([BIN], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, bufsize=1)
        s.id = 0
        s.rpc("initialize", {"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "r21", "version": "0"}})
        s.p.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n"); s.p.stdin.flush()
    def rpc(s, m, params, timeout=120):
        s.id += 1
        s.p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": s.id, "method": m, "params": params}) + "\n"); s.p.stdin.flush()
        while True:
            if not select.select([s.p.stdout], [], [], timeout)[0]: raise TimeoutError(m)
            v = json.loads(s.p.stdout.readline())
            if v.get("id") == s.id: return v
    def call(s, name, **a):
        r = s.rpc("tools/call", {"name": name, "arguments": a})["result"]
        t = r["content"][0]["text"]
        print(f"--- {name} {json.dumps(a)} -> {len(t)} bytes err={bool(r.get('isError'))}\n{t[:int(os.environ.get('SHOW','600'))]}")
        return t

if __name__ == "__main__":
    sc = sys.argv[1]
    m = Mcp()
    if sc == "tools":
        for t in m.rpc("tools/list", {})["result"]["tools"]:
            print(t["name"], json.dumps(t.get("inputSchema", {}).get("properties", {}))[:1500])
        sys.exit()
    _, base = fixtures.serve()
    path = {"f01": "/login", "f06": "/promo", "f07": "/banner", "f08": "/feed", "f10": "/product"}
    m.call("navigate", url="https://quotes.toscrape.com/js/" if sc in ("l11", "l12") else base + path[sc])
    m.call("observe")
    for step in sys.argv[2:]:  # tool:json-args
        name, _, a = step.partition(":")
        m.call(name, **(json.loads(a) if a else {}))
    if sc not in ("l11", "l12"):
        import urllib.request
        print("STATE", urllib.request.urlopen(base + "/__state").read().decode()[:1500])
