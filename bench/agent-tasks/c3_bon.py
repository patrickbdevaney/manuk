#!/usr/bin/env python3 -I
"""W4.C3: best-of-3 at a submit-type act, 3 arms. The submit hits a flaky server (GET /roll, 1 in 3 attempts succeed).
single = first attempt only. BoN-3 = checkpoint before the submit, run it in 3 branches, a Haiku judge (given only the task
and the 3 resulting page texts) picks one. Arms: manuk (fork tool, in-process engine), cdp-layer (same layer code, --backend
chrome, fork tool), playwright (@playwright/mcp@0.0.83, prefix replayed by hand: navigate + type + click per branch).
Roll schedules are seeded per (run, trial) and identical across arms, so arms are paired. Truth = the success text.
Usage: python3 -I c3_bon.py [--runs 3] [--trials 8] [--out FILE] [--arms manuk,cdp-layer,playwright]
Needs MANUK_CHROME_PIN. MANUK_C3_MUTATE=1 makes the judge pick branch 1 unconditionally (the gate must then fail).
"""
import argparse, json, os, random, re, shutil, statistics, subprocess, sys, threading, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
CLAUDE = shutil.which("claude") or os.path.expanduser("~/.local/bin/claude")
FIX = {  # name -> (task, success text, failure text)
    "clear": ("Buy one ticket for Ada and get a confirmed order.", "Order confirmed for Ada", "Error: payment declined. Try again."),
    "ambiguous": ("Book the room for Ada so that the booking is confirmed.", "Booking confirmed for Ada", "Booking request received for Ada, awaiting payment, not confirmed yet."),
}
SCHED = {}  # (fixture, run, trial) -> rolls, attempt counter
LOCK = threading.Lock()


def rolls(run, trial):
    r = random.Random(run * 1000 + trial)
    first = r.random() < 1 / 3
    return [first] + [r.random() < 1 / 3 for _ in range(2)]


class H(BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def do_GET(self):
        p = self.path.split("?")[0].strip("/").split("/")
        ct = "text/html"
        if p[0] == "roll":  # /roll/<fixture>/<run>/<trial>
            fx, run, trial, arm = p[1], int(p[2]), int(p[3]), p[4]
            with LOCK:
                k = SCHED.get((fx, run, trial, arm), 0); SCHED[(fx, run, trial, arm)] = k + 1
            ok = rolls(run, trial)[min(k, 2)]
            b = (FIX[fx][1] if ok else FIX[fx][2]).encode(); ct = "text/plain"
        elif p[0] == "form":
            fx, run, trial, arm = p[1], p[2], p[3], p[4]
            b = (f"<!doctype html><html><head><title>checkout</title></head><body><h1>Checkout</h1>"
                 f"<input type=text aria-label=Name><button onclick=\"fetch('/roll/{fx}/{run}/{trial}/{arm}').then(r=>r.text()).then(t=>{{document.getElementById('o').textContent=t}})\">Submit</button>"
                 f"<p id=o>not submitted</p></body></html>").encode()
        else:
            self.send_response(404); self.send_header("Content-Length", "0"); self.end_headers(); return
        self.send_response(200); self.send_header("Content-Type", ct); self.send_header("Content-Length", str(len(b))); self.send_header("Connection", "close")
        self.end_headers(); self.wfile.write(b)


class Mcp:
    def __init__(self, cmd, env=None):
        e = dict(os.environ); e.update(env or {})
        self.p = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, env=e)
        self.id = 0
        self.rpc("initialize", {"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "c3", "version": "0"}})
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


JUDGE_CACHE = {}
def judge(task, obs):
    """Haiku, given only the task and the 3 observations. -> (index 0..2, model calls)."""
    if os.environ.get("MANUK_C3_MUTATE"): return 0, 0
    key = (task, tuple(obs))
    if key in JUDGE_CACHE: return JUDGE_CACHE[key], 1
    body = "\n".join(f"Branch {i+1} page text: {o}" for i, o in enumerate(obs))
    prompt = (f"Task: {task}\nThree copies of the page each ran the final action. Which branch best completes the task?\n{body}\n"
              "Answer with only the branch number: 1, 2 or 3.")
    out = subprocess.run([CLAUDE, "-p", "--model", "haiku", "--no-session-persistence", "--tools", ""], input=prompt, capture_output=True, text=True, timeout=120, cwd="/tmp").stdout
    m = re.search(r"[123]", out); i = int(m.group()) - 1 if m else 0
    JUDGE_CACHE[key] = i
    return i, 1


def result_text(o):
    m = re.search(r"((?:Order|Booking|Error)[^\n]*)", o)
    return (m.group(1) if m else re.sub(r"\s+", " ", o))[:300]


def trial(arm, fx, run, tr, base):
    """-> (single_ok, bon_ok, judge_correct, model_calls, wall_s)."""
    task, good, bad = FIX[fx]
    url = f"{base}form/{fx}/{run}/{tr}/{arm}"
    bin_ = os.environ.get("MANUK_MCP_BIN") or os.path.join(ROOT, "target/release/manuk-mcp")
    if arm == "manuk": s = Mcp([bin_])
    elif arm == "cdp-layer": s = Mcp([bin_, "--backend", "chrome"], {"MANUK_CHROME": chrome_exe()})
    else: s = Mcp(["npx", "-y", "@playwright/mcp@0.0.83", "--headless", "--isolated", "--executable-path", chrome_exe()])
    t0 = time.time()
    try:
        obs = []
        if arm == "playwright":
            for _ in range(3):  # prefix replay by hand: load, fill, then the candidate action
                snap = s.tool("browser_navigate", {"url": url}); snap = s.tool("browser_snapshot", {})
                b = re.search(r'button "Submit"[^\n]*\[ref=(\w+)\]', snap); t = re.search(r'textbox[^\n]*\[ref=(\w+)\]', snap)
                if t: s.tool("browser_type", {"element": "Name", "target": t.group(1), "text": "Ada"})
                s.tool("browser_click", {"element": "Submit", "target": b.group(1)})
                time.sleep(0.3); obs.append(result_text(s.tool("browser_snapshot", {})))
        else:
            nav = s.tool("navigate", {"url": url})
            h = lambda lab: (lambda m: m.group(1) if m else None)(re.search(r"\[(h\d+)\][^\n]*" + lab, nav))
            name, sub = h("Name") or h("textbox"), h("Submit")
            s.tool("type", {"handle": name, "text": "Ada"})
            ck = s.tool("checkpoint", {}); cid = re.search(r"checkpoint (c\w+)", ck).group(1)
            for _ in range(3):
                f = s.tool("fork", {"checkpoint": cid}); pg = re.search(r"page: (\S+)", f).group(1)
                cur = lambda o, d: (re.search(r"page: (\S+)", o) or [None, d])[1]
                pg_nav = s.tool("observe", {"page": pg}); pg = cur(pg_nav, pg)
                sub_h = re.search(r"\[(h\d+)\][^\n]*Submit", pg_nav).group(1)
                a = s.tool("act", {"handle": sub_h, "action": "click", "page": pg}); pg = cur(a, pg)
                time.sleep(0.3)
                ex = s.tool("extract", {"page": pg}); obs.append(result_text(ex))
        single_ok = good in obs[0]
        pick, calls = judge(task, obs)
        bon_ok = good in obs[pick]
        return int(single_ok), int(bon_ok), int(bon_ok == any(good in o for o in obs)), calls, time.time() - t0
    finally:
        s.close()


def main():
    ap = argparse.ArgumentParser(); ap.add_argument("--runs", type=int, default=3); ap.add_argument("--trials", type=int, default=8)
    ap.add_argument("--out", default="docs/sprint/w4/C3/bon.tsv"); ap.add_argument("--arms", default="manuk,cdp-layer,playwright")
    a = ap.parse_args()
    pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
    srv = ThreadingHTTPServer(("127.0.0.1", 0), H); threading.Thread(target=srv.serve_forever, daemon=True).start()
    base = f"http://127.0.0.1:{srv.server_address[1]}/"
    rows = ["chrome_ver\tarm\tfixture\trun\ttrial\tsingle_ok\tbon_ok\tjudge_correct\tmodel_calls\twall_s\tany_branch_ok"]
    arms = a.arms.split(",")
    for arm in arms:
        for fx in FIX:
            for run in range(1, a.runs + 1):
                for tr in range(a.trials):
                    try: r = trial(arm, fx, run, tr, base)
                    except Exception as e: r = (0, 0, 0, 0, -1.0); print("ERR", arm, fx, run, tr, e, file=sys.stderr)
                    anyok = int(any(rolls(run, tr)))
                    rows.append(f"{pin}\t{arm}\t{fx}\t{run}\t{tr}\t{r[0]}\t{r[1]}\t{r[2]}\t{r[3]}\t{r[4]:.2f}\t{anyok}"); print(rows[-1], flush=True)
    os.makedirs(os.path.dirname(a.out), exist_ok=True); open(a.out, "w").write("\n".join(rows) + "\n")
    print("arm\tfixture\tsingle%\tbon3%\tjudge-correct%\tcalls/trial\twall_s")
    gate = None
    for arm in arms:
        for fx in FIX:
            R = [r.split("\t") for r in rows[1:] if r.split("\t")[1] == arm and r.split("\t")[2] == fx]
            if not R: continue
            n = len(R); f = lambda i: 100 * sum(int(x[i]) for x in R) / n
            print(f"{arm}\t{fx}\t{f(5):.0f}\t{f(6):.0f}\t{f(7):.0f}\t{sum(int(x[8]) for x in R)/n:.1f}\t{statistics.mean(float(x[9]) for x in R):.1f}")
            if arm == "manuk" and fx == "clear": gate = f(6) - f(5)
    good = gate is not None and gate >= 5
    print(f"GATE manuk clear BoN-3 beats single by >=5 pt: {'PASS' if good else 'FAIL'} ({gate})")
    sys.exit(0 if good else 1)


if __name__ == "__main__":
    main()
