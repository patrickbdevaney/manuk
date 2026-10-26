#!/usr/bin/env python3 -I
"""R2.1.3 gate: page async work started by input keeps running under manuk-mcp.
 1. /banner: click 'Reject all'  -> /__state cookie_choice == reject
 2. /feed:   scroll to the end   -> extract contains 'Item 45: amber'
 3. probe:   click handler fetches /api/g?z=1 -> the server logged it
Exit 0 only if all three hold."""
import json, os, subprocess, sys, threading, time, re, urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "bench/agent-tasks"))
import fixtures

HITS = []
class P(BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def do_GET(self):
        if self.path.startswith("/api/g"):
            HITS.append(self.path); b = b"{}"
        else:
            b = b'<button onclick="fetch(\'/api/g?z=1\')">Go</button>'
        self.send_response(200); self.send_header("Content-Length", str(len(b))); self.end_headers(); self.wfile.write(b)

class C:
    def __init__(s):
        s.p = subprocess.Popen([os.path.join(ROOT, "target/release/manuk-mcp")], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
        s.i = 0
        s.rpc("initialize", {"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "g", "version": "0"}})
        s.p.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n"); s.p.stdin.flush()
    def rpc(s, m, params):
        s.i += 1
        s.p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": s.i, "method": m, "params": params}) + "\n"); s.p.stdin.flush()
        while True:
            v = json.loads(s.p.stdout.readline())
            if v.get("id") == s.i: return v
    def tool(s, n, **a):
        r = s.rpc("tools/call", {"name": n, "arguments": a})["result"]
        return "\n".join(b.get("text", "") for b in r.get("content", []))
    def click(s, name_re, page_text):
        m = re.search(r"\[?h(\d+)\]?[^\n]*" + name_re, page_text)
        assert m, (name_re, page_text[:600])
        return s.tool("act", handle="h" + m.group(1), action="click")

def state(base): return json.load(urllib.request.urlopen(base + "/__state"))

def main():
    srv, base = fixtures.serve(0)
    ps = ThreadingHTTPServer(("127.0.0.1", 0), P); threading.Thread(target=ps.serve_forever, daemon=True).start()
    pbase = f"http://127.0.0.1:{ps.server_address[1]}"
    ok = {}
    c = C()
    t = c.tool("navigate", url=base + "/banner"); c.click("Reject all", t)
    ok["banner"] = state(base).get("cookie_choice") == "reject"
    c.tool("navigate", url=base + "/feed")
    for _ in range(12):
        c.tool("scroll", dy=4000)
        time.sleep(0.6)
        if "Item 45: amber" in c.tool("extract", format="text"): ok["feed"] = True; break
    ok.setdefault("feed", False)
    t = c.tool("navigate", url=pbase + "/"); c.click("Go", t); time.sleep(0.2)
    ok["probe"] = any("z=1" in h for h in HITS)
    c.p.kill()
    print(json.dumps(ok))
    sys.exit(0 if all(ok.values()) else 1)
main()
