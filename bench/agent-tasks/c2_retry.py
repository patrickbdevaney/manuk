#!/usr/bin/env python3 -I
"""W4.C2: layer-applied retry vs the same recovery done by the model, on dead-click fixtures. No LLM.

A scripted agent follows the ladder a model would: click; if the token is not in the result, observe if the tool
did not already show the page; then keyboard Enter; then the target's inner link. Metric = tool calls until the
fixture's token is visible (cap 8). Arms: manuk (retry on), manuk-noretry (MANUK_NO_RETRY=1), cdp-layer (retry on,
--backend chrome), playwright (@playwright/mcp@0.0.83). Output TSV; GATE: manuk recovers keydown-only in 1 call.
Usage: python3 -I c2_retry.py [--runs 3] [--out FILE]   (needs MANUK_CHROME_PIN; MANUK_C2_MUTATE=1 sets MANUK_NO_RETRY on every layer arm)
"""
import argparse, json, os, re, statistics, subprocess, sys, threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
SET = "document.getElementById('o').textContent="
PAGES = {
    "keydown-only": f"""<div role="button" tabindex="0" aria-label="Target" onkeydown="if(event.key==='Enter'){SET}'DONE-keydown-only'">Target</div>""",
    "inner-link": f"""<div role="button" tabindex="0" aria-label="Target" style="width:400px;padding:20px"><a role="link" href="#" style="float:left" onclick="event.preventDefault();{SET}'DONE-inner-link'">Open</a></div>""",
    "error-page": f"""<div role="button" tabindex="0" aria-label="Target" onclick="location='/gone'" onkeydown="if(event.key==='Enter'){SET}'DONE-error-page'">Target</div>""",
    "plain-button": f"""<button onclick="{SET}'DONE-plain-button'">Target</button>""",  # control: first click works, no retry may fire
}


class H(BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def do_GET(self):
        path = self.path.split("?")[0].strip("/")
        if path == "gone":
            self.send_response(404); b = b"<html><body><h1>404 Not Found</h1></body></html>"
        elif path in PAGES:
            self.send_response(200)
            b = f"<!doctype html><html><head><title>{path}</title></head><body><h1>{path}</h1>{PAGES[path]}<h2 id=o>idle</h2></body></html>".encode()
        else:
            self.send_response(404); b = b""
        self.send_header("Content-Type", "text/html"); self.send_header("Content-Length", str(len(b))); self.send_header("Connection", "close")
        self.end_headers(); self.wfile.write(b)


class Mcp:
    def __init__(self, cmd, env=None):
        e = dict(os.environ); e.update(env or {})
        self.p = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, env=e)
        self.id = 0; self.calls = 0
        self.rpc("initialize", {"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "c2", "version": "0"}})
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n"); self.p.stdin.flush()
    def rpc(self, method, params):
        self.id += 1
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": self.id, "method": method, "params": params}) + "\n"); self.p.stdin.flush()
        while True:
            line = self.p.stdout.readline()
            if not line: raise RuntimeError("server closed")
            v = json.loads(line)
            if v.get("id") == self.id: return v
    def tool(self, name, args, count=True):
        if count: self.calls += 1
        r = self.rpc("tools/call", {"name": name, "arguments": args}).get("result", {})
        return "\n".join(b.get("text", "") for b in r.get("content", []))
    def close(self): self.p.kill()


def chrome_exe():
    pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
    exe = os.path.expanduser(f"~/.local/chrome-cft/{pin}/chrome-linux64/chrome")
    os.path.exists(exe) or sys.exit(f"refusing: {exe} missing")
    return exe


def ref(text, pat):
    m = re.search(pat, text); return m.group(1) if m else None


def run_one(arm, name, base):
    """-> (tool calls used to see the token or CAP, ok, layer_retried)."""
    tok = f"DONE-{name}"; CAP = 8
    bin_ = os.environ.get("MANUK_MCP_BIN") or os.path.join(ROOT, "target/release/manuk-mcp")
    off = {"MANUK_NO_RETRY": "1"} if os.environ.get("MANUK_C2_MUTATE") else {}
    if arm == "manuk": s = Mcp([bin_], off)
    elif arm == "manuk-noretry": s = Mcp([bin_], {"MANUK_NO_RETRY": "1"})
    elif arm == "cdp-layer": s = Mcp([bin_, "--backend", "chrome"], {"MANUK_CHROME": chrome_exe(), **off})
    else: s = Mcp(["npx", "-y", "@playwright/mcp@0.0.83", "--headless", "--isolated", "--executable-path", chrome_exe()])
    try:
        pw = arm == "playwright"
        if pw:
            snap = s.tool("browser_navigate", {"url": base + name}, count=False)
            snap = s.tool("browser_snapshot", {}, count=False)
            t = ref(snap, r'(?:button|generic)[^\n]*"?Target"?[^\n]*\[ref=(\w+)\]')
            inner = ref(snap, r'link "Open"[^\n]*\[ref=(\w+)\]')
            click = lambda r_: s.tool("browser_click", {"element": "Target", "target": r_})
            key = lambda: s.tool("browser_press_key", {"key": "Enter"})
            seen = lambda: s.tool("browser_snapshot", {})
            back = lambda: s.tool("browser_navigate_back", {})
        else:
            nav = s.tool("navigate", {"url": base + name}, count=False)
            def h(label):
                l = next((l for l in nav.splitlines() if label in l), None)
                return ref(l or "", r"\[(h\d+)\]")
            t = h("Target") or h("button")
            inner = h("Open")
            click = lambda r_: s.tool("act", {"handle": r_, "action": "click"})
            key = lambda: s.tool("act", {"handle": t, "action": "focus"})  # focus then key
            seen = lambda: s.tool("extract", {})
            back = lambda: s.tool("back", {})
        if not t: return CAP, 0, 0
        out = click(t)
        retried = int("layer retry" in out)
        def done(o): return tok in o and "404 Not Found" not in o.split("\n")[0]
        if done(out): return s.calls, 1, retried
        if pw or "no change" in out or True:
            # the agent reads the page only if the click result did not already say
            if "no change" not in out and "url: unchanged" not in out:
                o = seen()
                if tok in o: return s.calls, 1, retried
        # fallback ladder, as a model would climb it
        if "404" in out or "Not Found" in out:
            back()
        steps = ["key", "inner"]
        for st in steps:
            if s.calls >= CAP: break
            if st == "key":
                if not pw: s.tool("act", {"handle": t, "action": "focus"})
                o = s.tool("key", {"key": "Enter"}) if not pw else key()
            else:
                if not inner: continue
                o = click(inner)
            if tok in o: return s.calls, 1, retried
            o = seen()
            if tok in o: return s.calls, 1, retried
        return min(s.calls, CAP), 0, retried
    finally:
        s.close()


def main():
    ap = argparse.ArgumentParser(); ap.add_argument("--runs", type=int, default=3); ap.add_argument("--out", default="docs/sprint/w4/C2/retry.tsv")
    a = ap.parse_args()
    pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
    srv = ThreadingHTTPServer(("127.0.0.1", 0), H); threading.Thread(target=srv.serve_forever, daemon=True).start()
    base = f"http://127.0.0.1:{srv.server_address[1]}/"
    os.makedirs(os.path.dirname(a.out), exist_ok=True)
    rows = ["chrome_ver\tarm\tfixture\trun\ttool_calls\trecovered\tlayer_retried"]
    for name in PAGES:
        for arm in ("manuk", "manuk-noretry", "cdp-layer", "playwright"):
            for run in range(1, a.runs + 1):
                c, ok, rt = run_one(arm, name, base)
                rows.append(f"{pin}\t{arm}\t{name}\t{run}\t{c}\t{ok}\t{rt}"); print(rows[-1], flush=True)
    open(a.out, "w").write("\n".join(rows) + "\n")
    g = [r.split("\t") for r in rows[1:] if r.split("\t")[1] == "manuk" and r.split("\t")[2] == "keydown-only"]
    good = all(int(r[4]) == 1 and r[5] == "1" for r in g)
    print(f"GATE keydown-only manuk recovers in 1 tool call x{a.runs}: {'PASS' if good else 'FAIL'}")
    sys.exit(0 if good else 1)


if __name__ == "__main__":
    main()
