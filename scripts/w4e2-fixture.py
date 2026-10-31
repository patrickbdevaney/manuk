#!/usr/bin/env python3
"""W4.E2 fixture server: 12 noisy ~60 KB PNGs in a gallery page that also names tracker hosts.
Honours `Range`. Usage: w4e2-fixture.py PORT   (serves /index.html)."""
import http.server, os, random, struct, sys, zlib

def png(w, h, seed):
    r = random.Random(seed)
    raw = b"".join(b"\0" + bytes(r.getrandbits(8) for _ in range(w * 3)) for _ in range(h))
    def ch(t, d):
        return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d))
    return b"\x89PNG\r\n\x1a\n" + ch(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)) + ch(b"IDAT", zlib.compress(raw, 1)) + ch(b"IEND", b"")

IMGS = {f"/i{i}.png": png(100 + i, 120, i) for i in range(12)}
HTML = ("<html><head><title>fx</title></head><body><h1>Gallery</h1><a href='/x'>next</a>"
        + "".join(f"<img src='i{i}.png' alt='pic {i}'>" for i in range(12))
        + "<img src='https://www.google-analytics.com/collect?v=1' width='1' height='1' alt=''>"
        + "<script src='https://www.googletagmanager.com/gtm.js'></script>"
        + "<button>Buy</button></body></html>").encode()

class H(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def do_GET(self):
        body, ct = (HTML, "text/html") if self.path in ("/", "/index.html") else (IMGS.get(self.path), "image/png")
        if body is None:
            self.send_response(404); self.end_headers(); return
        rng = self.headers.get("Range")
        if rng and rng.startswith("bytes="):
            a, _, b = rng[6:].partition("-")
            a, b = int(a or 0), min(int(b) if b else len(body) - 1, len(body) - 1)
            part = body[a:b + 1]
            self.send_response(206)
            self.send_header("Content-Range", f"bytes {a}-{b}/{len(body)}")
        else:
            part = body
            self.send_response(200)
        self.send_header("Content-Type", ct)
        self.send_header("Content-Length", str(len(part)))
        self.end_headers()
        try: self.wfile.write(part)
        except BrokenPipeError: pass

http.server.ThreadingHTTPServer(("127.0.0.1", int(sys.argv[1])), H).serve_forever()
