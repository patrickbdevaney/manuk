#!/usr/bin/env python3
"""W6.R1.a.a: localhost proxy of evals-udriver.vercel.app; --mutate rewrites the Alert auto-dismiss timer delay
(setTimeout(()=>{i(!1)},1e4) -> 1e9) so it lies beyond manuk's 60 s virtual-clock horizon.
usage: r1aa_proxy.py PORT [none|beyond-horizon]"""
import http.server, sys, urllib.error, urllib.request
PORT = int(sys.argv[1]); MODE = sys.argv[2] if len(sys.argv) > 2 else "none"
UP = "https://evals-udriver.vercel.app"
NEEDLE = b"setTimeout(()=>{i(!1)},1e4)"
class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        try: r = urllib.request.urlopen(urllib.request.Request(UP + self.path, headers={"User-Agent": "Mozilla/5.0"}))
        except urllib.error.HTTPError as e: self.send_response(e.code); self.send_header("Content-Length", "0"); self.end_headers(); return
        b = r.read()
        if MODE == "beyond-horizon": b = b.replace(NEEDLE, b"setTimeout(()=>{i(!1)},1e9)")
        self.send_response(r.status)
        self.send_header("Content-Type", r.headers.get("Content-Type", "application/octet-stream"))
        self.send_header("Content-Length", str(len(b))); self.end_headers(); self.wfile.write(b)
    def log_message(self, *a): pass
http.server.ThreadingHTTPServer(("127.0.0.1", PORT), H).serve_forever()
