"""CONC.1 fixture site: own-suite pages (served from 127.0.0.1) and frozen real-page snapshots (demo/www/pages)."""
import os, re, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
FROZEN = os.path.join(ROOT, "demo/www/pages")

def _page(title, body, script=""):
    return f"<!doctype html><html><head><meta charset=utf-8><title>{title}</title></head><body><h1>{title}</h1>{body}<script>{script}</script></body></html>"

def heavy():
    rows = "".join(f"<tr><td>Row {i}</td><td><button onclick=\"pick({i})\">Pick {i}</button></td></tr>" for i in range(1500))
    return _page("Heavy", f"<h2 id=o>none</h2><table>{rows}</table>", "function pick(i){document.getElementById('o').textContent='PICKED-'+i}")

PAGES = {
    "/form": _page("Form", '<label>Name <input id=n aria-label="Name"></label><button onclick="document.getElementById(\'o\').textContent=\'SUBMITTED-\'+document.getElementById(\'n\').value">Submit</button><h2 id=o>idle</h2>'),
    "/late": _page("Late", '<button onclick="go()">Go</button><h2 id=o>idle</h2>',
                   "function go(){fetch('/d?ms=400').then(function(r){return r.text()}).then(function(){document.getElementById('o').textContent='LANDED-late'})}"),
    "/p/1": _page("Page 1", '<a href="/p/2">Next</a>'),
    "/p/2": _page("Page 2", '<a href="/p/3">Next</a>'),
    "/p/3": _page("Page 3", "<p>PAGE-THREE-REACHED</p>"),
    "/iso": _page("Iso", "<h2 id=o>?</h2>",
                  "var q=location.search.match(/id=(\\w+)/);if(!localStorage.getItem('who'))localStorage.setItem('who','agent-'+(q?q[1]:'x'));"
                  "document.cookie='who='+localStorage.getItem('who');document.getElementById('o').textContent='WHO='+localStorage.getItem('who')+';'"),
    "/broken": _page("Broken", "<p>nothing here</p>"),
}
FROZEN_TRACES = {"/f/rfc": "rfc-editor.html", "/f/w3c": "w3c.html", "/f/mdn": "mdn-css.html"}

class H(BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def do_GET(self):
        path = self.path.split("?")[0]
        if path == "/d":
            m = re.search(r"ms=(\d+)", self.path); time.sleep(int(m.group(1)) / 1000 if m else 0)
            body, ct = b"ok", "text/plain"
        elif path == "/heavy": body, ct = heavy().encode(), "text/html"
        elif path in PAGES: body, ct = PAGES[path].encode(), "text/html"
        elif path in FROZEN_TRACES:
            with open(os.path.join(FROZEN, FROZEN_TRACES[path]), "rb") as f: body, ct = f.read(), "text/html; charset=utf-8"
        else:
            self.send_response(404); self.send_header("Content-Length", "0"); self.end_headers(); return
        self.send_response(200); self.send_header("Content-Type", ct); self.send_header("Content-Length", str(len(body)))
        self.send_header("Connection", "close"); self.end_headers(); self.wfile.write(body)

def serve():
    srv = ThreadingHTTPServer(("127.0.0.1", 0), H); srv.request_queue_size = 512; srv.daemon_threads = True
    return srv

# Traces: ordered steps (kind, target, arg). nav: target=path, arg=token that must appear; click/type: target=(role,label).
# `expect` after click/type is polled via snapshot up to 6 s (an agent that sees a stale page looks again).
TRACES = {
    "form":   [("nav", "/form", "Submit"), ("type", ("textbox", "Name"), "Ada"), ("click", ("button", "Submit"), "SUBMITTED-Ada")],
    "late":   [("nav", "/late", "Go"), ("click", ("button", "Go"), "LANDED-late")],
    "walk":   [("nav", "/p/1", "Next"), ("click", ("link", "Next"), "Page 2"), ("click", ("link", "Next"), "PAGE-THREE-REACHED")],
    "heavy":  [("nav", "/heavy", "Heavy"), ("click", ("button", "Pick 1337"), "PICKED-1337")],
    "rfc":    [("nav", "/f/rfc", "RFC")],
    "w3c":    [("nav", "/f/w3c", "W3C")],
    "mdn":    [("nav", "/f/mdn", "display")],
}
# Self-test only: expects a token the page never shows. Must be reported failed on every arm.
BROKEN = {"broken": [("nav", "/broken", "TOKEN-THAT-IS-NEVER-THERE")]}
ISO = {"iso": [("nav", "/iso?id={id}", "WHO="), ("barrier", None, None), ("nav", "/iso?id={id}", "WHO=agent-{id};")]}
