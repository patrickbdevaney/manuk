#!/usr/bin/env python3
"""W4.C4.b.a: what the W4.C4.b text capture costs per act.

The W4.C4.b commit made every snapshot call `page_text` (the effect diff needs the visible text). This bench
times `act(click)` on a ~2000-node page through two manuk-mcp binaries built from the parent commit and from
HEAD, interleaved run by run, each run a fresh process. Overhead = HEAD mean - parent mean, per act, in ms.

Usage (stdlib only, -I so no local module shadows it):
  MANUK_CHROME_PIN=145.0.7632.116 python3 -I scripts/w4c4ba-act-cost.py \
      --arm parent=/path/manuk-mcp-parent --arm head=/path/manuk-mcp-head --out docs/sprint/W4C4BA-ACT-COST.tsv
  python3 -I scripts/w4c4ba-act-cost.py --selftest --arm a=P1 --arm b=P2   # red proofs: short page, identical arms, no pin

Gate red cases (exit 2, nothing measured): fewer than --min-rows rows visible in the page text, a button
missing, the two arms byte-identical, or MANUK_CHROME_PIN unset. Every row carries chrome_ver.
"""
import argparse
import hashlib
import http.server
import json
import os
import re
import statistics
import subprocess
import sys
import threading
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
from mcp_readline import readline  # noqa: E402  (fd-level reader with a deadline)


def touch():
    """The watchdog reads this mtime to see the sprint agent is alive."""
    try:
        os.utime(os.path.join(ROOT, ".git", "sprint-working"), None)
    except OSError:
        pass


def page(rows):
    body = "".join(f"<p>row {i} of the bench list, with a little text</p>" for i in range(rows))
    return ("<!doctype html><html><head><title>Bench</title></head><body><h1>Bench</h1>"
            "<button onclick=\"var n=(window.n=(window.n||0)+1);"
            "document.getElementById('o').textContent='Order placed: '+n\">Place order</button>"
            f"<div id='o'>Your cart is empty.</div>{body}</body></html>").encode()


def serve(body):
    class H(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, *a):
            pass

    srv = http.server.ThreadingHTTPServer(("127.0.0.1", 0), H)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv, f"http://127.0.0.1:{srv.server_address[1]}/"


class Mcp:
    def __init__(self, binary):
        self.p = subprocess.Popen([binary], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                  stderr=subprocess.DEVNULL)
        self.id = 0
        self.rpc("initialize", {"protocolVersion": "2025-03-26", "capabilities": {},
                                "clientInfo": {"name": "bench", "version": "0"}})
        self.send({"jsonrpc": "2.0", "method": "notifications/initialized"})

    def send(self, msg):
        self.p.stdin.write((json.dumps(msg) + "\n").encode())
        self.p.stdin.flush()

    def rpc(self, method, params):
        self.id += 1
        self.send({"jsonrpc": "2.0", "id": self.id, "method": method, "params": params})
        while True:
            v = json.loads(readline(self.p, 120))
            if v.get("id") == self.id:
                return v

    def tool(self, name, args):
        r = self.rpc("tools/call", {"name": name, "arguments": args})
        res = r["result"]
        text = "\n".join(b.get("text", "") for b in res.get("content", []))
        return text, bool(res.get("isError", False))

    def close(self):
        self.p.kill()


def sha(path):
    with open(path, "rb") as f:
        return hashlib.sha256(f.read()).hexdigest()[:16]


def one_run(binary, rows, acts):
    """Fresh process, navigate, find the button handle, then `acts` timed clicks. Returns [(ms, first_line)]."""
    srv, url = serve(page(rows))
    m = Mcp(binary)
    try:
        touch()
        m.tool("navigate", {"url": url})
        # mode "full" exists on both arms (the parent commit has no "content" mode).
        text, _ = m.tool("observe", {"mode": "full", "budget_tokens": 400000})
        visible = text.count("row ")
        if visible < rows * 0.95:
            raise SystemExit(f"RED: only {visible} of {rows} rows visible in the observation")
        btn = [ln for ln in text.splitlines() if "Place order" in ln]
        if not btn:
            raise SystemExit("RED: no 'Place order' button in the observation")
        handle = re.search(r"\[(h\d+)\]", btn[0]).group(1)
        out = []
        for _ in range(acts):
            touch()
            t0 = time.perf_counter()
            text, err = m.tool("act", {"handle": handle, "action": "click"})
            ms = (time.perf_counter() - t0) * 1000.0
            if err:
                raise SystemExit(f"RED: act failed: {text[:200]}")
            lines = text.splitlines()
            effect = next((ln for ln in lines if "dispatched" in ln or "change" in ln), lines[0] if lines else "")
            out.append((ms, effect))
        return out
    finally:
        m.close()
        srv.shutdown()


def selftest(arms):
    """Red proofs: each deliberately-broken input must exit 2 before anything is measured."""
    import tempfile
    (la, pa), (lb, pb) = arms
    cases = [
        ("short page (20 rows)", ["--arm", f"{la}={pa}", "--arm", f"{lb}={pb}", "--rows", "20"], {}),
        ("identical arms", ["--arm", f"{la}={pa}", "--arm", f"{lb}={pa}"], {}),
        ("no pin", ["--arm", f"{la}={pa}", "--arm", f"{lb}={pb}"], {"UNSET_PIN": "1"}),
    ]
    ok = True
    for name, args, extra in cases:
        env = {k: v for k, v in os.environ.items() if k != "MANUK_CHROME_PIN"}
        if not extra:
            env["MANUK_CHROME_PIN"] = os.environ.get("MANUK_CHROME_PIN", "145.0.7632.116")
        with tempfile.TemporaryDirectory() as d:
            r = subprocess.run([sys.executable, "-I", __file__, "--out", os.path.join(d, "x.tsv")] + args,
                               env=env, capture_output=True, text=True)
        red = r.returncode == 2 and "RED" in r.stderr
        print(f"selftest {name}: rc={r.returncode} red={red} :: {r.stderr.strip()[-120:]}")
        ok &= red
    print("SELFTEST", "PASS" if ok else "FAIL")
    return 0 if ok else 1


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--arm", action="append", default=[], help="LABEL=PATH to a manuk-mcp binary")
    ap.add_argument("--runs", type=int, default=3)
    ap.add_argument("--acts", type=int, default=20)
    ap.add_argument("--rows", type=int, default=1990, help="<p> rows: ~2000 DOM nodes with the fixed elements")
    ap.add_argument("--min-rows", type=int, default=1900)
    ap.add_argument("--out", default=os.path.join(ROOT, "docs", "sprint", "W4C4BA-ACT-COST.tsv"))
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args()
    if a.selftest:
        given = [x.split("=", 1) for x in a.arm]
        sys.exit(selftest(given if len(given) == 2 else [["a", "/bin/true"], ["b", "/bin/cat"]]))
    pin = os.environ.get("MANUK_CHROME_PIN", "")
    if not pin:
        print("RED: MANUK_CHROME_PIN unset; every measurement row must carry chrome_ver", file=sys.stderr)
        sys.exit(2)
    arms = [x.split("=", 1) for x in a.arm]
    if len(arms) != 2:
        print("RED: give exactly two --arm LABEL=PATH", file=sys.stderr)
        sys.exit(2)
    (la, pa), (lb, pb) = arms
    if open(pa, "rb").read() == open(pb, "rb").read():
        print("RED: the two arms are byte-identical; the overhead would be zero by construction", file=sys.stderr)
        sys.exit(2)
    if a.rows < a.min_rows:
        print(f"RED: --rows {a.rows} < --min-rows {a.min_rows}", file=sys.stderr)
        sys.exit(2)
    hashes = {la: sha(pa), lb: sha(pb)}
    rows_out = ["chrome_ver\tarm\trun\tact\tms\tfirst_line"]
    per_run = {la: [], lb: []}
    for r in range(a.runs):
        order = [(la, pa), (lb, pb)] if r % 2 == 0 else [(lb, pb), (la, pa)]
        for label, path in order:
            res = one_run(path, a.rows, a.acts)
            per_run[label].append([ms for ms, _ in res])
            for i, (ms, line) in enumerate(res):
                rows_out.append(f"{pin}\t{label}\t{r + 1}\t{i + 1}\t{ms:.1f}\t{line[:90]}")
            print(f"run {r + 1} {label}: mean {statistics.mean(ms for ms, _ in res):.1f} ms/act", flush=True)
    os.makedirs(os.path.dirname(a.out), exist_ok=True)
    with open(a.out, "w") as f:
        f.write("\n".join(rows_out) + "\n")
    means = {k: [statistics.mean(x) for x in v] for k, v in per_run.items()}
    med = {k: statistics.median(x for run in v for x in run) for k, v in per_run.items()}
    diff = statistics.mean(means[lb]) - statistics.mean(means[la])
    lo = min(means[lb]) - max(means[la])
    hi = max(means[lb]) - min(means[la])
    print(f"chrome_ver={pin} rows={a.rows} runs={a.runs} acts/run={a.acts}")
    for k in (la, lb):
        print(f"  {k} sha={hashes[k]} per-run mean ms/act={[round(x, 1) for x in means[k]]} "
              f"all-acts median={med[k]:.1f} ms")
    print(f"overhead ({lb} - {la}) per act: mean {diff:.1f} ms, run-to-run band [{lo:.1f}, {hi:.1f}] ms")
    print(f"written {a.out}")


if __name__ == "__main__":
    main()
