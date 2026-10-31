#!/usr/bin/env python3
"""W4.E4 gate: warm pre-forked pool (manuk-mcp --zygote) vs a cold manuk-mcp spawn.

  cold     spawn manuk-mcp -> initialize -> navigate a local fixture -> first observation (ms)
  zygote   connect to the running pool -> initialize -> navigate the same fixture (ms)
Arms are interleaved, N runs each (default 10). Pass condition for the speed claim: the zygote band
(min..max) lies strictly below the cold band. Isolation: session A sets a cookie and a localStorage
key; a later session B (a fresh fork) must see neither. `--leak-mutant` starts the pool with a
shared --profile, which MUST turn the isolation check red. Also asserts the pool parent had one
thread at fork (it refuses otherwise) and that it stays at one thread while serving.
Exit 1 on any failed check. Prints chrome_ver (no Chrome arm here).
"""
import argparse, http.server, importlib.util, json, os, select, socket, subprocess, sys, tempfile, threading, time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
MCP = os.path.join(ROOT, "target", "release", "manuk-mcp")
_spec = importlib.util.spec_from_file_location("fp", os.path.join(HERE, "w4e0-footprint.py"))
fp = importlib.util.module_from_spec(_spec); _spec.loader.exec_module(fp)


class SockMcp(fp.Mcp):
    """The footprint Mcp client, over a unix-socket connection to the pool instead of a child's pipes."""
    def __init__(self, path):
        self.s = socket.socket(socket.AF_UNIX); self.s.connect(path)
        self.fd = self.s.fileno(); self.buf, self.id = b"", 0
        class P:  # minimal Popen-like for _send/kill
            pass
        self.p = P(); self.p.stdin = self.s.makefile("wb"); self.p.kill = lambda: None; self.p.wait = lambda: None
        r = self.rpc("initialize", {"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "w4e4", "version": "0"}})
        if "result" not in r: raise RuntimeError(r)
        self._send({"jsonrpc": "2.0", "method": "notifications/initialized"})
    def kill(self):
        self.s.close()


def fixture():
    class H(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            ck = self.headers.get("Cookie", "")
            if self.path.startswith("/iso"):
                body = ("<!doctype html><title>iso</title><h1 id=o>pending</h1><script>"
                        "var v=localStorage.getItem('k');document.getElementById('o').textContent="
                        "'ls='+v+' ck='+(document.cookie||'none');localStorage.setItem('k','A');"
                        "document.cookie='sess=A; path=/';</script>").encode()
            else:
                body = b"<!doctype html><title>fx</title><h1>fixture</h1><button>Go</button>"
            self.send_response(200); self.send_header("Content-Type", "text/html")
            if self.path.startswith("/iso"): self.send_header("Set-Cookie", "srv=A; Path=/")
            self.send_header("Content-Length", str(len(body))); self.end_headers(); self.wfile.write(body)
        def log_message(self, *a): pass
    srv = http.server.ThreadingHTTPServer(("127.0.0.1", 0), H)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv, f"http://127.0.0.1:{srv.server_address[1]}"


def threads_of(pid):
    for ln in open(f"/proc/{pid}/status"):
        if ln.startswith("Threads:"): return int(ln.split()[1])


def main():
    ap = argparse.ArgumentParser(); ap.add_argument("--runs", type=int, default=10)
    ap.add_argument("--leak-mutant", action="store_true"); a = ap.parse_args()
    pin = os.environ.get("MANUK_CHROME_PIN", "-")
    print(f"chrome_ver={pin} (no Chrome arm: cold start vs the pool is manuk vs manuk)")
    srv, base = fixture()
    tmp = tempfile.mkdtemp(prefix="w4e4-"); sock = os.path.join(tmp, "z.sock")
    env = dict(os.environ); env["MANUK_ACTLOG_DIR"] = os.path.join(tmp, "log"); os.makedirs(env["MANUK_ACTLOG_DIR"])
    argv = [MCP, "--zygote", sock] + (["--profile", os.path.join(tmp, "shared")] if a.leak_mutant else [])
    z = subprocess.Popen(argv, env=env, stderr=subprocess.DEVNULL)
    for _ in range(200):
        if os.path.exists(sock): break
        time.sleep(0.02)
    fail = 0
    try:
        cold, warm = [], []
        for i in range(a.runs + 1):  # run 0 is a discarded warm-up of caches for both arms
            fp.touch()
            t0 = time.monotonic(); m = fp.Mcp(env); e, _, _ = m.call("navigate", {"url": base + "/fx"}); c = (time.monotonic() - t0) * 1000; m.kill()
            if e: fail += 1; print("FAIL cold navigate errored")
            t0 = time.monotonic(); m = SockMcp(sock); e, _, _ = m.call("navigate", {"url": base + "/fx"}); w = (time.monotonic() - t0) * 1000; m.kill()
            if e: fail += 1; print("FAIL zygote navigate errored")
            if i: cold.append(c); warm.append(w)
        band = lambda v: f"{min(v):.1f}-{max(v):.1f} ms (median {sorted(v)[len(v)//2]:.1f}, n={len(v)})"
        print("cold   :", band(cold)); print("zygote :", band(warm))
        ok = max(warm) < min(cold)
        print("speed  :", "PASS zygote band strictly below cold band" if ok else "FAIL bands overlap")
        fail += 0 if ok else 1
        tc = threads_of(z.pid)
        print(f"pool parent threads while serving: {tc}")
        if tc != 1: fail += 1; print("FAIL pool parent is not single-threaded")
        # isolation: A then B, fresh forks
        seen = []
        for who in "AB":
            for _ in range(20):  # a child may still be exiting (profile lock, if any)
                try:
                    m = SockMcp(sock); break
                except OSError:
                    time.sleep(0.3)
            _, t, _ = m.call("navigate", {"url": base + "/iso"})
            seen.append(t); m.kill()
        print("iso A:", seen[0][100:260].replace("\n", " ")); print("iso B:", seen[1][100:260].replace("\n", " "))
        want = 'ls=null ck=srv=A"'  # B may see only the cookie its own response set
        iso_ok = want in seen[0] and want in seen[1]
        print("isolation:", "PASS session B saw no cookie or localStorage from A" if iso_ok else "FAIL session B saw state from A")
        fail += 0 if iso_ok else 1
    finally:
        z.kill(); z.wait(); srv.shutdown()
    sys.exit(1 if fail else 0)

main()
