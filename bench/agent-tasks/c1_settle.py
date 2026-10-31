#!/usr/bin/env python3 -I
"""W4.C1: engine-exact settle vs cdp-layer's heuristic vs Playwright's wait, on late-content fixtures. No LLM.

Each arm is driven over raw MCP stdio: navigate, click #go, read the act's own returned observation.
A false settle = the act returned before the fixture's content landed (expected token absent from the act result).
Arms: manuk (manuk-mcp), cdp-layer (manuk-mcp --backend chrome), playwright (@playwright/mcp@0.0.83 --isolated).
Usage: python3 -I c1_settle.py [--runs 3] [--out FILE]      (needs MANUK_CHROME_PIN; MANUK_C1_MUTATE=no-wait breaks the oracle)
"""
import argparse, json, os, re, statistics, subprocess, sys, threading, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

# name -> (delay of the content after the click in ms, page script). Token LANDED-<name> must appear in the act result.
def page(name, js):
    return f"""<!doctype html><html><head><title>{name}</title></head><body><h1>{name}</h1>
<button id="go" onclick="go()">Go</button><h2 id="o">idle</h2>
<script>function w(){{document.getElementById('o').textContent='LANDED-{name}'}}
function go(){{{js}}}</script></body></html>"""

FIXTURES = {
    "late-fetch-1400": (1400, "fetch('/d?ms=1400').then(function(r){return r.text()}).then(w)"
                        if os.environ.get("MANUK_C1_MUTATE") != "late" else "setTimeout(w,30000)"),  # mutation: lands beyond any wait
    "fetch-then-timer": (1900, "fetch('/d?ms=400').then(function(r){return r.text()}).then(function(){setTimeout(w,1500)})"),
    "chained-timers": (1200, "setTimeout(function(){setTimeout(function(){setTimeout(function(){w()},400)},400)},400)"),
    "font-swap": (900, "new FontFace('X','url(/f?ms=900)').load().then(w,w)"),
    "font-css": (900, "var st=document.createElement('style');st.textContent='@font-face{font-family:X;src:url(/f?ms=900)}#o{font-family:X}';document.head.appendChild(st);document.fonts.load('12px X').then(w,w)"),
    "microtask-100k": (0, "var n=0;(function s(){if(++n<100000)Promise.resolve().then(s);else w()})()"),
    "microtask-storm": (0, "var n=0;(function s(){if(++n<3000000)Promise.resolve().then(s);else w()})()"),
}
CONTROL = ("never-10s", (10000, "setTimeout(w,10000)"))  # must NOT be waited for; token must be absent, wall < 6 s


class H(BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def do_GET(self):
        m = re.match(r"/(\w[\w-]*)", self.path)
        path = self.path.split("?")[0]
        ms = int(re.search(r"ms=(\d+)", self.path).group(1)) if "ms=" in self.path else 0
        time.sleep(ms / 1000)
        if path == "/d": body, ct = b"ok", "text/plain"
        elif path == "/f": body, ct = b"not-a-font", "font/woff2"
        else:
            name = path.strip("/")
            allf = dict(FIXTURES); allf[CONTROL[0]] = CONTROL[1]
            if name not in allf:
                self.send_response(404); self.send_header("Content-Length", "0"); self.end_headers(); return
            body, ct = page(name, allf[name][1]).encode(), "text/html"
        self.send_response(200); self.send_header("Content-Type", ct); self.send_header("Content-Length", str(len(body)))
        self.send_header("Connection", "close"); self.end_headers(); self.wfile.write(body)


class Mcp:
    def __init__(self, cmd, env=None):
        e = dict(os.environ); e.update(env or {})
        self.p = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, env=e)
        self.id = 0
        self.rpc("initialize", {"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "c1", "version": "0"}})
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n"); self.p.stdin.flush()
    def rpc(self, method, params):
        self.id += 1
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": self.id, "method": method, "params": params}) + "\n"); self.p.stdin.flush()
        while True:
            line = self.p.stdout.readline()
            if not line: raise RuntimeError("server closed")
            v = json.loads(line)
            if v.get("id") == self.id: return v
    def tool(self, name, args):
        r = self.rpc("tools/call", {"name": name, "arguments": args}).get("result", {})
        return "\n".join(b.get("text", "") for b in r.get("content", []))
    def close(self):
        self.p.kill()


def chrome_exe():
    pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
    exe = os.path.expanduser(f"~/.local/chrome-cft/{pin}/chrome-linux64/chrome")
    os.path.exists(exe) or sys.exit(f"refusing: {exe} missing")
    return exe


def run_one(arm, url):
    """-> (act wall ms, act result text)."""
    bin_ = os.environ.get("MANUK_MCP_BIN") or os.path.join(ROOT, "target/release/manuk-mcp")
    if arm == "manuk": s = Mcp([bin_])
    elif arm == "cdp-layer": s = Mcp([bin_, "--backend", "chrome"], {"MANUK_CHROME": chrome_exe()})
    else: s = Mcp(["npx", "-y", "@playwright/mcp@0.0.83", "--headless", "--isolated", "--executable-path", chrome_exe()])
    try:
        if arm == "playwright":
            s.tool("browser_navigate", {"url": url})
            snap = s.tool("browser_snapshot", {})
            m = re.search(r'button "Go"[^\n]*\[ref=(\w+)\]', snap)
            if not m: sys.exit("playwright snapshot has no Go button:\n" + snap)
            ref = m.group(1)
            t0 = time.time(); r = s.tool("browser_click", {"element": "Go", "ref": ref})
        else:
            nav = s.tool("navigate", {"url": url})
            line = next(l for l in nav.splitlines() if 'button "Go"' in l)
            h = re.search(r"\[(h\d+)\]", line).group(1)
            t0 = time.time(); r = s.tool("act", {"handle": h, "action": "click"})
        return (time.time() - t0) * 1000, r
    finally:
        s.close()


def main():
    ap = argparse.ArgumentParser(); ap.add_argument("--runs", type=int, default=3); ap.add_argument("--out", default="docs/sprint/w4/C1/settle.tsv")
    a = ap.parse_args()
    pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
    srv = ThreadingHTTPServer(("127.0.0.1", 0), H); threading.Thread(target=srv.serve_forever, daemon=True).start()
    base = f"http://127.0.0.1:{srv.server_address[1]}/"
    os.makedirs(os.path.dirname(a.out), exist_ok=True)
    rows = ["chrome_ver\tarm\tfixture\trun\tlands_ms\tact_wall_ms\tcontent_in_act_result\tfalse_settle"]
    allf = list(FIXTURES.items()) + [CONTROL]
    for name, (ms, _) in allf:
        if os.environ.get("MANUK_C1_ONLY") and name != os.environ["MANUK_C1_ONLY"]: continue
        for arm in ("manuk", "cdp-layer", "playwright"):
            for run in range(1, a.runs + 1):
                wall, r = run_one(arm, base + name)
                has = f"LANDED-{name}" in r
                ctl = name == CONTROL[0]
                fs = (not has) if not ctl else has  # control: content landing in the act = it waited on a 10 s timer
                rows.append(f"{pin}\t{arm}\t{name}\t{run}\t{ms}\t{wall:.0f}\t{int(has)}\t{int(fs)}")
                print(rows[-1], flush=True)
    open(a.out, "w").write("\n".join(rows) + "\n")
    # GATE: the engine arm reads the 1.4 s fixture correctly in every run (rows: arm, fixture, ..., content)
    ok = sum(int(r.split("\t")[6]) for r in rows[1:] if r.split("\t")[1] == "manuk" and r.split("\t")[2] == "late-fetch-1400")
    print(f"GATE late-fetch-1400 manuk content {ok}/{a.runs}: {'PASS' if ok == a.runs else 'FAIL'}")
    sys.exit(0 if ok == a.runs else 1)


if __name__ == "__main__":
    main()
