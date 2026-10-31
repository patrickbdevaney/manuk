#!/usr/bin/env python3 -I
"""W4.C4: do action receipts let an agent verify a state change? Fixture: a shop whose "Add Lamp" button SOMETIMES silently
fails (seeded per run/trial, identical across arms) while the UI shows an optimistic "Added Lamp!" either way, as real
shops do. "Add Mug" always works. A scripted agent clicks both; a Haiku verifier sees ONLY the task and the two click
results and answers whether the cart now holds both items. Truth = localStorage.cart holds both (read by the harness).
Arms: manuk / manuk-no-receipts (MANUK_NO_RECEIPTS=1) / cdp-layer / cdp-layer-no-receipts / playwright (@playwright/mcp 0.0.83).
Usage: python3 -I c4_receipts.py [--runs 3] [--trials 8] [--out FILE] [--arms ...]; needs MANUK_CHROME_PIN.
MANUK_C4_MUTATE=1: verifier always answers "yes" (accuracy must then equal the base rate, so the gate fails).
"""
import argparse, json, os, random, re, shutil, subprocess, sys, threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
CLAUDE = shutil.which("claude") or os.path.expanduser("~/.local/bin/claude")
TASK = "Add the Mug and the Lamp to the cart, then confirm the cart holds both."
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))


def lamp_fails(run, trial):
    return random.Random(run * 7919 + trial).random() < 0.5


class H(BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def do_POST(self):
        self.rfile.read(int(self.headers.get("Content-Length") or 0)); self.reply(b"{}", "application/json")
    def do_GET(self):
        p = self.path.split("?")[0].strip("/").split("/")
        if p[0] != "shop": self.send_response(404); self.send_header("Content-Length", "0"); self.end_headers(); return
        fail = "true" if lamp_fails(int(p[1]), int(p[2])) else "false"
        add = ("function add(n,bad){document.getElementById('o').textContent='Added '+n+'!';if(bad)return;"
               "var c=JSON.parse(localStorage.getItem('cart')||'[]');c.push(n);localStorage.setItem('cart',JSON.stringify(c));"
               "fetch('/api/cart',{method:'POST',body:n})}")
        self.reply((f"<!doctype html><html><head><title>Shop</title></head><body><h1>Shop</h1><script>{add}</script>"
                    f"<button onclick=\"add('Mug',false)\">Add Mug</button><button onclick=\"add('Lamp',{fail})\">Add Lamp</button>"
                    f"<p id=o>nothing yet</p></body></html>").encode(), "text/html")
    def reply(self, b, ct):
        self.send_response(200); self.send_header("Content-Type", ct); self.send_header("Content-Length", str(len(b)))
        self.send_header("Connection", "close"); self.end_headers(); self.wfile.write(b)


class Mcp:
    def __init__(self, cmd, env=None):
        e = dict(os.environ); e.update(env or {})
        self.p = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, env=e)
        self.id = 0
        self.rpc("initialize", {"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "c4", "version": "0"}})
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
    def close(self): self.p.kill()


def chrome_exe():
    pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
    exe = os.path.expanduser(f"~/.local/chrome-cft/{pin}/chrome-linux64/chrome")
    os.path.exists(exe) or sys.exit(f"refusing: {exe} missing")
    return exe


CACHE = {}
def verify(obs):
    """Haiku sees the task and the two click results. -> True if it says the cart holds both."""
    if os.environ.get("MANUK_C4_MUTATE"): return True
    if obs in CACHE: return CACHE[obs]
    prompt = (f"Task: {TASK}\nYou clicked 'Add Mug' then 'Add Lamp'. The browser tool returned:\n--- click 1 ---\n{obs[0]}\n--- click 2 ---\n{obs[1]}\n---\n"
              "Using only this, does the cart now hold BOTH items? Answer with only YES or NO.")
    out = subprocess.run([CLAUDE, "-p", "--model", "haiku", "--no-session-persistence", "--tools", ""], input=prompt, capture_output=True, text=True, timeout=120, cwd="/tmp").stdout
    CACHE[obs] = "YES" in out.upper().split("\n")[0].upper() or out.strip().upper().startswith("YES")
    return CACHE[obs]


def trial(arm, run, tr, base):
    """-> (verifier_correct, claimed_yes, truth_both, obs_len)."""
    url = f"{base}shop/{run}/{tr}"
    bin_ = os.environ.get("MANUK_MCP_BIN") or os.path.join(ROOT, "target/release/manuk-mcp")
    nr = {"MANUK_NO_RECEIPTS": "1"} if arm.endswith("no-receipts") else {}
    if arm.startswith("manuk"): s = Mcp([bin_], nr)
    elif arm.startswith("cdp-layer"): s = Mcp([bin_, "--backend", "chrome"], dict(nr, MANUK_CHROME=chrome_exe()))
    else: s = Mcp(["npx", "-y", "@playwright/mcp@0.0.83", "--headless", "--isolated", "--executable-path", chrome_exe()])
    try:
        obs = []
        if arm == "playwright":
            snap = s.tool("browser_navigate", {"url": url}); snap += s.tool("browser_snapshot", {})
            for lab in ("Add Mug", "Add Lamp"):
                m = re.search(r'button "' + lab + r'"[^\n]*\[ref=(\w+)\]', snap)
                obs.append(s.tool("browser_click", {"element": lab, "target": m.group(1)}))
            truth = not lamp_fails(run, tr)
        else:
            nav = s.tool("navigate", {"url": url})
            for lab in ("Add Mug", "Add Lamp"):
                h = re.search(r"\[(h\d+)\][^\n]*" + lab, nav).group(1)
                obs.append(s.tool("act", {"handle": h, "action": "click"}))
            truth = not lamp_fails(run, tr)
        obs = tuple(o[:1500] for o in obs)
        yes = verify(obs)
        return int(yes == truth), int(yes), int(truth), sum(len(o) for o in obs)
    finally:
        s.close()


def main():
    ap = argparse.ArgumentParser(); ap.add_argument("--runs", type=int, default=3); ap.add_argument("--trials", type=int, default=8)
    ap.add_argument("--out", default="docs/sprint/w4/C4/receipts.tsv")
    ap.add_argument("--arms", default="manuk,manuk-no-receipts,cdp-layer,cdp-layer-no-receipts,playwright")
    a = ap.parse_args()
    pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
    srv = ThreadingHTTPServer(("127.0.0.1", 0), H); threading.Thread(target=srv.serve_forever, daemon=True).start()
    base = f"http://127.0.0.1:{srv.server_address[1]}/"
    rows = ["chrome_ver\tarm\trun\ttrial\tcorrect\tclaimed_both\ttruth_both\tobs_chars"]
    arms = a.arms.split(",")
    for arm in arms:
        for run in range(1, a.runs + 1):
            for tr in range(a.trials):
                try: r = trial(arm, run, tr, base)
                except Exception as e: r = (0, -1, int(not lamp_fails(run, tr)), 0); print("ERR", arm, run, tr, e, file=sys.stderr)
                rows.append(f"{pin}\t{arm}\t{run}\t{tr}\t" + "\t".join(map(str, r))); print(rows[-1], flush=True)
    os.makedirs(os.path.dirname(a.out), exist_ok=True); open(a.out, "w").write("\n".join(rows) + "\n")
    print("arm\tverification-correct%\tfalse-YES (claimed both, lamp missing)\tn")
    acc = {}
    for arm in arms:
        R = [r.split("\t") for r in rows[1:] if r.split("\t")[1] == arm]
        n = len(R); acc[arm] = 100 * sum(int(x[4]) for x in R) / n
        fy = sum(1 for x in R if x[5] == "1" and x[6] == "0")
        print(f"{arm}\t{acc[arm]:.0f}\t{fy}/{sum(1 for x in R if x[6]=='0')}\t{n}")
    good = "manuk" in acc and "manuk-no-receipts" in acc and acc["manuk"] - acc["manuk-no-receipts"] >= 20
    print(f"GATE manuk receipts lift verification accuracy over no-receipts by >=20 pt: {'PASS' if good else 'FAIL'}")
    sys.exit(0 if good else 1)


if __name__ == "__main__":
    main()
