#!/usr/bin/env python3
"""W4.E1.b.b.a.a gate: TRUE bytes the server wrote for the lean image-size probe, pre-fix vs fixed binary.
Loopback HTTP/1.1 server (Range-aware) logs every byte it hands the kernel per image request; a page of N <img>
(sizes 20-300 KB, a quarter with a >16 KB EXIF block before the dims) is loaded with MANUK_AGENT_PROFILE=lean.
--sndbuf N emulates a bounded in-flight window (SO_SNDBUF) so an early stop can actually truncate the send.
Exit 0 iff fixed wire bytes <= 1.10 x pre-fix wire bytes AND pre-fix arm reproduces its own count (2 runs within 5%).
Usage: python3 -I scripts/w4e1bbaa-wire-check.py --a BIN_PREFIX --b BIN_FIXED [--sndbuf 65536] [--n 40] | --self-test"""
import argparse, os, socket, struct, subprocess, sys, threading, zlib, re
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

def png(w, h, size):
    def ch(t, d): return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xffffffff)
    ihdr = struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)
    pad = os.urandom(max(0, size - 60))
    return b"\x89PNG\r\n\x1a\n" + ch(b"IHDR", ihdr) + ch(b"IDAT", pad) + ch(b"IEND", b"")

def jpeg_big_exif(w, h, size):  # APP1 of ~20 KB before SOF0, so the dims sit past the old 16 KB step
    app = b"Exif\0\0" + os.urandom(20000)
    sof = b"\xff\xc0" + struct.pack(">HBHHB", 11, 8, h, w, 1) + b"\x01\x11\x00"
    return b"\xff\xd8" + b"\xff\xe1" + struct.pack(">H", len(app) + 2) + app + sof + os.urandom(max(0, size - 20100)) + b"\xff\xd9"

def make_site(n):
    files = {}
    for i in range(n):
        size = 20000 + (i * 7919) % 280000
        files[f"/i{i}.png"] = jpeg_big_exif(100 + i, 80, size + 25000) if i % 4 == 3 else png(100 + i, 80, size)
        if i % 4 == 3: files[f"/i{i}.png"] = files[f"/i{i}.png"]  # served as image/jpeg below
    return files

def serve(files, sndbuf, stats):
    class H(BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.1"
        def log_message(self, *a): pass
        def do_GET(self):
            if self.path in ("/", "/index.html"):
                body = ("<html><body>" + "".join(f'<img src="/i{i}.png">' for i in range(len(files))) + "</body></html>").encode()
                self.send_response(200); self.send_header("Content-Type", "text/html"); self.send_header("Content-Length", str(len(body)))
                self.end_headers(); self.wfile.write(body); return
            data = files.get(self.path)
            if data is None: self.send_error(404); return
            if sndbuf: self.connection.setsockopt(socket.SOL_SOCKET, socket.SO_SNDBUF, sndbuf)
            m = re.match(r"bytes=(\d+)-(\d*)", self.headers.get("Range", ""))
            lo, hi, code = 0, len(data) - 1, 200
            if m:
                lo = int(m.group(1)); hi = min(len(data) - 1, int(m.group(2) or len(data) - 1)); code = 206
            part = data[lo:hi + 1]
            self.send_response(code); self.send_header("Content-Type", "image/jpeg" if data[:2] == b"\xff\xd8" else "image/png")
            self.send_header("Content-Length", str(len(part)))
            if code == 206: self.send_header("Content-Range", f"bytes {lo}-{hi}/{len(data)}")
            self.end_headers()
            sent = 0
            try:
                for k in range(0, len(part), 8192):
                    self.wfile.write(part[k:k + 8192]); self.wfile.flush(); sent += len(part[k:k + 8192])
            except OSError: pass
            with stats["lock"]: stats["bytes"] += sent; stats["reqs"] += 1
    return ThreadingHTTPServer(("127.0.0.1", 0), H)

def run_arm(binp, n, sndbuf):
    stats = {"lock": threading.Lock(), "bytes": 0, "reqs": 0}
    srv = serve(make_site(n), sndbuf, stats)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    env = dict(os.environ, MANUK_AGENT_PROFILE="lean")
    subprocess.run([binp, f"http://127.0.0.1:{srv.server_port}/"], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=120)
    import time; time.sleep(0.5); srv.shutdown()
    return stats["bytes"], stats["reqs"]

if __name__ == "__main__":
    ap = argparse.ArgumentParser(); ap.add_argument("--a"); ap.add_argument("--b"); ap.add_argument("--n", type=int, default=40)
    ap.add_argument("--sndbuf", type=int, default=0); ap.add_argument("--self-test", action="store_true")
    a = ap.parse_args()
    if a.self_test:
        s = make_site(8); assert len(s["/i3.png"]) > 20000 and s["/i0.png"][:8] == b"\x89PNG\r\n\x1a\n"; print("SELF-TEST OK"); sys.exit(0)
    A1 = run_arm(a.a, a.n, a.sndbuf); A2 = run_arm(a.a, a.n, a.sndbuf); B = run_arm(a.b, a.n, a.sndbuf)
    print(f"pre-fix run1 bytes={A1[0]} reqs={A1[1]}; run2 bytes={A2[0]} reqs={A2[1]}; fixed bytes={B[0]} reqs={B[1]}  sndbuf={a.sndbuf}")
    repro = abs(A1[0] - A2[0]) <= 0.05 * max(A1[0], 1) and A1[0] > 0
    ratio = B[0] / max(A1[0], 1); print(f"ratio fixed/pre-fix={ratio:.3f} repro={repro}")
    sys.exit(0 if (repro and ratio <= 1.10) else 1)
