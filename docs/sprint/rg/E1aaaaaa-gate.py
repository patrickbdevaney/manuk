#!/usr/bin/env python3
"""RG.2.E1.a.a.a.a.a.a: an <img> a click inserts after load gets its size and naturalWidth in a manuk-mcp session.

  python3 -I docs/sprint/rg/E1aaaaaa-gate.py     # exit 0 = GREEN, 1 = RED

Serves a page whose button, once clicked, inserts <img src=a.png> (120x80) into a flex pane; reads
naturalWidth and the pane's width through a scripted handoff. naturalWidth must be 120 and the box non-zero. The real-site probe is
E1aaaaa-probe.py (zilloft-2 map pane, chrome_ver=$MANUK_CHROME_PIN).
"""
import http.server, json, os, re, struct, subprocess, sys, tempfile, threading, time, zlib
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
EXE = os.path.join(ROOT, "target/release/manuk-mcp")

def png(w, h):
    raw = b"".join(b"\0" + bytes([200, 50, 50]) * w for _ in range(h))
    def ch(t, d):
        return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d))
    return b"\x89PNG\r\n\x1a\n" + ch(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)) + ch(b"IDAT", zlib.compress(raw)) + ch(b"IEND", b"")

PAGE = b"""<!doctype html><html><body><h1>t</h1><button id=b>Show map</button>
<div id=r style="display:flex"></div>
<script>document.getElementById('b').onclick=function(){var i=document.createElement('img');i.src='a.png';
i.style.width='100%';document.getElementById('r').appendChild(i);};</script></body></html>"""

class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        body, ct = (png(120, 80), "image/png") if self.path.startswith("/a.png") else (PAGE, "text/html")
        self.send_response(200); self.send_header("Content-Type", ct); self.send_header("Content-Length", str(len(body))); self.end_headers(); self.wfile.write(body)
    def log_message(self, *a): pass

JS = ("(function(){var i=document.images[0];var d=document.createElement('h1');d.textContent='MK nat='+(i?i.naturalWidth:'noimg')"
      "+' w='+(i?Math.round(i.getBoundingClientRect().width):'-');document.body.insertBefore(d,document.body.firstChild);return 1})()")

def main():
    srv = http.server.ThreadingHTTPServer(("127.0.0.1", 0), H)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    with tempfile.NamedTemporaryFile("w", suffix=".js", delete=False) as f:
        f.write(JS)
    p = subprocess.Popen([EXE], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True,
                         env=dict(os.environ, MANUK_HANDOFF_SCRIPT=f.name))
    n = [0]
    def rpc(m, pa):
        n[0] += 1
        p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": n[0], "method": m, "params": pa}) + "\n"); p.stdin.flush()
        while True:
            j = json.loads(p.stdout.readline())
            if j.get("id") == n[0]:
                return j
    def call(nm, a):
        return "\n".join(x.get("text", "") for x in rpc("tools/call", {"name": nm, "arguments": a})["result"]["content"])
    rpc("initialize", {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "g", "version": "0"}})
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n"); p.stdin.flush()
    call("navigate", {"url": f"http://127.0.0.1:{srv.server_address[1]}/"})
    t = call("observe", {"mode": "interactive"})
    h = next(l[l.index("[h") + 1:l.index("]", l.index("[h"))] for l in t.split("\n") if "Show map" in l and "[h" in l)
    call("act", {"handle": h, "action": "click"})
    time.sleep(1)
    call("handoff", {"reason": "g", "timeout_ms": 3000})
    out = call("observe", {"mode": "viewport"})
    p.kill(); os.unlink(f.name)
    m = re.search(r"MK nat=(\S+) w=(\S+)", out)
    print(m.group(0) if m else "no MK line:\n" + out[:300])
    sys.exit(0 if m and m.group(1) == "120" and int(m.group(2)) > 0 else 1)
main()
