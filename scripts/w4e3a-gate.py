#!/usr/bin/env python3
"""W4.E3.a gate: stencil cache for page scripts. Cold (empty cache) vs warm (stencil hit) script time
(compile+run, from MANUK_SCRIPT_TIMING) on 3 script-heavy pages, 3 runs each. Passes when every warm
run is below every cold run (bands do not overlap), the a11y digest is identical cold/warm/plain, and
a changed script body misses. Usage: w4e3a-gate.py <site-dir with three.js ethers.js axe.js + .html>"""
import hashlib, http.server, os, re, shutil, subprocess, sys, tempfile, threading, functools, json

site = os.path.abspath(sys.argv[1])
BIN = os.path.join(os.path.dirname(__file__), "..", "target", "release", "a11y-dump")
PAGES = ["three", "ethers", "axe"]
h = functools.partial(http.server.SimpleHTTPRequestHandler, directory=site)
class Q(http.server.ThreadingHTTPServer):
    def log_message(self, *a): pass
srv = http.server.ThreadingHTTPServer(("127.0.0.1", 0), h)
h.func.log_message = lambda *a, **k: None
threading.Thread(target=srv.serve_forever, daemon=True).start()
port = srv.server_address[1]

def run(page, cache):
    env = dict(os.environ, MANUK_SCRIPT_TIMING="1")
    env.pop("MANUK_HTTP_CACHE_DIR", None)
    if cache: env["MANUK_HTTP_CACHE_DIR"] = cache
    p = subprocess.run([BIN, f"http://127.0.0.1:{port}/{page}.html"], env=env, capture_output=True, text=True, timeout=120)
    rows = [l.split() for l in p.stderr.splitlines() if l.startswith("manuk-script-ms")]
    kinds = {r[1] for r in rows}
    ms = sum(float(r[2]) for r in rows)
    run.rows = rows
    return ms, kinds, hashlib.sha256(p.stdout.encode()).hexdigest()[:16]

res, ok = {}, True
for pg in PAGES:
    colds, warms, dig = [], [], set()
    for _ in range(3):
        d = tempfile.mkdtemp(); ms, k, dg = run(pg, d); colds.append(ms); dig.add(dg); assert k == {"miss"}, k
        ms2, k2, dg2 = run(pg, d); warms.append(ms2); dig.add(dg2); assert k2 == {"hit"}, k2
        shutil.rmtree(d)
    _, kp, dgp = run(pg, None); dig.add(dgp)
    res[pg] = dict(cold=colds, warm=warms)
    good = max(warms) < min(colds) and len(dig) == 1
    ok &= good
    print(f"{pg}: cold ms {sorted(colds)} warm ms {sorted(warms)} digests_equal={len(dig)==1} {'PASS' if good else 'FAIL'}")
# changed body never hits
d = tempfile.mkdtemp(); run("axe", d)
src = os.path.join(site, "axe.js"); orig = open(src).read()
try:
    open(src, "w").write(orig + "\n;void 0;")
    _, k, _ = run("axe", d)
finally:
    open(src, "w").write(orig)
shutil.rmtree(d)
big = max(run.rows, key=lambda r: float(r[2]))[1]  # the changed (large) script; the 1-line inline one is unchanged
print("changed-body script:", big, "PASS" if big == "miss" else "FAIL")
ok &= big == "miss"
print("GATE", "PASS" if ok else "FAIL")
sys.exit(0 if ok else 1)
