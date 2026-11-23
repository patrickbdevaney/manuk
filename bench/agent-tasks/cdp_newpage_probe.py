#!/usr/bin/env python3 -I
"""W4.P0.f.a.a.a: does the cdp-layer shim (manuk-mcp --backend chrome) isolate `page:"new"` from the default page?
Serves a local origin, sets localStorage on the default page, then reads it back (a) from the default page, (b) from
a page:"new" page. --mutate same-page reads (b) with no `page` arg (must see the value). Exit 0 iff the flow's verdict
holds: normal flow => default sees it AND new page does NOT; --mutate => new-page read sees it (gate red if not)."""
import json, os, re, subprocess, sys, threading, http.server
ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
PIN = os.environ.get("MANUK_CHROME_PIN") or sys.exit("MANUK_CHROME_PIN unset")
CHROME = os.path.expanduser(f"~/.local/chrome-cft/{PIN}/chrome-linux64/chrome")
mut = "--mutate" in sys.argv
class H(http.server.BaseHTTPRequestHandler):
    n = 0
    def do_GET(s):
        s.send_response(200); s.send_header("content-type", "text/html"); s.end_headers(); H.n += 1; s.wfile.write(("<title>t</title><body></body><script>var k=localStorage.getItem('k')||'id%d';localStorage.setItem('k',k);document.body.textContent='K='+k;</script>" % H.n).encode())
    def log_message(*a): pass
srv = http.server.HTTPServer(("127.0.0.1", 0), H); threading.Thread(target=srv.serve_forever, daemon=True).start()
URL = f"http://127.0.0.1:{srv.server_port}/"
p = subprocess.Popen([os.path.join(ROOT, "target/release/manuk-mcp"), "--backend", "chrome"], stdin=subprocess.PIPE,
                     stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, env=dict(os.environ, MANUK_CHROME=CHROME))
n = 0
def rpc(method, params=None, note=False):
    global n
    m = {"jsonrpc": "2.0", "method": method, "params": params or {}}
    if not note:
        n += 1; m["id"] = n
    p.stdin.write(json.dumps(m) + "\n"); p.stdin.flush()
    while not note:
        r = json.loads(p.stdout.readline())
        if r.get("id") == n: return r
def tool(name, **a):
    r = rpc("tools/call", {"name": name, "arguments": a})
    return json.dumps(r.get("result", r))
def pids():  # chrome processes whose --user-data-dir is a distinct profile, under this server
    out = subprocess.run(["pgrep", "-P", str(p.pid)], capture_output=True, text=True).stdout.split()
    return out
rpc("initialize", {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "probe", "version": "0"}})
rpc("notifications/initialized", note=True)
def dirs():
    ps = subprocess.run(["ps", "-eo", "args"], capture_output=True, text=True).stdout.splitlines()
    return sorted({a.split("--user-data-dir=")[1].split()[0] for a in ps if "--user-data-dir=" in a and CHROME in a and "--type=" not in a})
try:
    tool("navigate", url=URL)
    d1 = dirs()
    r_def = tool("evaluate", expression="document.body.textContent")
    nav = tool("navigate", page="new", url=URL) if not mut else tool("navigate", url=URL)
    d2 = dirs()
    if True:
        h = re.search(r"\bp[0-9a-f]{6}:\d+", nav)
        if not h: sys.exit("no page handle in navigate reply: " + nav[:200])
        r_new = tool("evaluate", page=h.group(0), expression="document.body.textContent") if not mut else tool("evaluate", expression="document.body.textContent")
    row = {"chrome_ver": PIN, "mutate": mut, "default_sees": "K=id1" in r_def,
           "new_page_sees": "K=id1" in r_new, "chrome_profiles_before": len(d1), "chrome_profiles_after_page_new": len(d2),
           "new_page_raw": r_new[:200]}
    print(json.dumps(row))
    ok = row["default_sees"] and (row["new_page_sees"] if mut else (not row["new_page_sees"] and row["chrome_profiles_after_page_new"] == row["chrome_profiles_before"] + 1))
    sys.exit(0 if ok else 1)
finally:
    p.stdin.close()
    try: p.wait(10)
    except subprocess.TimeoutExpired: p.kill()
    srv.shutdown()
