"""W4.E0.d.a: wall-clock manuk-proc vs chrome-proc on LOCAL fixtures sized like the 7 paired URLs.
Sizes (html bytes, script/css/img counts) come from sizes.tsv (curl of each URL on 2026-11-18, tag counts of the
fetched HTML); each subresource is a SUB_BYTES placeholder file. Wall = cold process start to page loaded, both arms.
  MANUK_CHROME_PIN=... python3 -I docs/sprint/w4eda/measure.py [--runs 10] [--mutant] > summary
  python3 -I docs/sprint/w4eda/measure.py --selftest
--mutant forces every manuk wall to equal its chrome wall (ratio 1.0): verdict must be NOT_SETTLED."""
import importlib.util, os, sys, statistics, tempfile, threading, time, http.server, argparse
HERE = os.path.dirname(os.path.abspath(__file__)); ROOT = os.path.dirname(os.path.dirname(os.path.dirname(HERE)))
spec = importlib.util.spec_from_file_location("fp", os.path.join(ROOT, "scripts", "w4e0-footprint.py"))
fp = importlib.util.module_from_spec(spec); spec.loader.exec_module(fp)
SUB_BYTES = 20000

def load_sizes():
    rows = [l.split("\t") for l in open(os.path.join(HERE, "sizes.tsv")).read().split("\n")[1:] if l]
    return [(r[0], int(r[1]), int(r[2]), int(r[3]), int(r[4])) for r in rows]

def build_page(i, html_b, ns, nc, ni):
    head = "".join(f'<link rel="stylesheet" href="/s/{i}/c{k}.css">' for k in range(nc))
    head += "".join(f'<script src="/s/{i}/j{k}.js"></script>' for k in range(ns))
    imgs = "".join(f'<img src="/s/{i}/i{k}.png" width="10" height="10" alt="i{k}">' for k in range(ni))
    pre = f"<!doctype html><html><head><title>fixture {i}</title>{head}</head><body><h1>fixture {i}</h1>{imgs}"
    fill = "<p>" + "Lorem ipsum dolor sit amet. " * 4 + "</p>"
    body = pre
    while len(body) < html_b - 20:
        body += fill
    return (body + "</body></html>").encode()

def serve(sizes):
    pages = {i: build_page(i, *s[1:]) for i, s in enumerate(sizes)}
    class H(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            parts = self.path.strip("/").split("/")
            if parts[0] not in ("p", "s"):  # e.g. /favicon.ico from Chrome
                self.send_response(404); self.send_header("Content-Length", "0"); self.end_headers(); return
            if parts[0] == "p":
                b, ct = pages[int(parts[1])], "text/html; charset=utf-8"
            else:
                ext = parts[-1].rsplit(".", 1)[-1]
                ct = {"css": "text/css", "js": "application/javascript", "png": "image/png"}[ext]
                b = (b"/* x */\n" * (SUB_BYTES // 8)) if ext != "png" else b"\x89PNG" + b"\0" * SUB_BYTES
            self.send_response(200); self.send_header("Content-Type", ct)
            self.send_header("Content-Length", str(len(b))); self.end_headers(); self.wfile.write(b)
        def log_message(self, *a): pass
    srv = http.server.ThreadingHTTPServer(("127.0.0.1", 0), H)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv, f"http://127.0.0.1:{srv.server_address[1]}/p/"

def manuk_wall(url, log_root):
    t0 = time.monotonic()
    m = fp.Mcp(fp.mcp_env(tempfile.mkdtemp(prefix="w4eda-log-", dir=log_root)))
    try:
        err, text, _ = m.call("navigate", {"url": url})
        w = time.monotonic() - t0
    finally:
        m.kill()
    return (w, "OK") if not err and "fixture" in text else (w, "FAIL")

def chrome_wall(url):
    html = []
    w, _, _, _, st = fp.r01.run_chrome(fp.CHROME, url, fp.PAGE_TIMEOUT_S, html)
    return (w, "OK") if st == "OK" and html and "fixture" in html[0] else (w, "FAIL")

def per_run_ratios(rows, mutant=False):
    """rows: dict (run,url,arm)->wall. Returns per-run median over URLs of manuk/chrome."""
    out = []
    for run in sorted({k[0] for k in rows}):
        rs = []
        for u in sorted({k[1] for k in rows}):
            c = rows[(run, u, "chrome-proc")]; m = c if mutant else rows[(run, u, "manuk-proc")]
            rs.append(m / c)
        out.append(statistics.median(rs))
    return out

def selftest():
    f = []
    sz = load_sizes()
    if len(sz) != 7: f.append("sizes.tsv not 7 rows")
    pg = build_page(0, 10992, 1, 3, 2)
    if abs(len(pg) - 10992) > 200 or pg.count(b"<script") != 1 or pg.count(b"stylesheet") != 3: f.append("page builder")
    d = {(r, u, a): (2.0 if a == "manuk-proc" else 1.0) + 0.01 * r for r in range(10) for u in "ab" for a in ("manuk-proc", "chrome-proc")}
    if not fp.wall_verdict(per_run_ratios(d, True)).startswith("NOT_SETTLED"): f.append("mutant not NOT_SETTLED")
    if not fp.wall_verdict(per_run_ratios({k: v for k, v in d.items() if k[0] < 3}, False)).startswith("NOT_SETTLED"): f.append("3 runs not NOT_SETTLED")
    print("selftest", "FAIL " + "; ".join(f) if f else "OK"); return 1 if f else 0

def main():
    ap = argparse.ArgumentParser(); ap.add_argument("--runs", type=int, default=10)
    ap.add_argument("--mutant", action="store_true"); ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args()
    if a.selftest: return selftest()
    sizes = load_sizes(); srv, base = serve(sizes); log_root = tempfile.mkdtemp(prefix="w4eda-")
    rows, bad = {}, 0
    print("run\turl\tarm\twall_s\tstatus\tchrome_ver")
    for run in range(1, a.runs + 1):
        for i, s in enumerate(sizes):
            for arm, fn in (("manuk-proc", lambda u: manuk_wall(u, log_root)), ("chrome-proc", chrome_wall)):
                w, st = fn(f"{base}{i}")
                print(f"{run}\t{s[0]}\t{arm}\t{w:.3f}\t{st}\t{fp.PIN}", flush=True)
                if st != "OK": bad += 1
                rows[(run, s[0], arm)] = w
                fp.touch()
    pr = per_run_ratios(rows, a.mutant)
    v = fp.wall_verdict(pr)
    print(f"# per-run median ratio manuk/chrome: {[round(x,2) for x in pr]}\n# VERDICT {v} bad_rows={bad} chrome_ver={fp.PIN}")
    return 1 if bad else 0
sys.exit(main())
