#!/usr/bin/env python3 -I
"""SHIP.2 smoke: a manuk-mcp binary lists its tools and completes one Chrome task.

usage: mcp-smoke.py /path/to/manuk-mcp
Runs against the DEFAULT backend (pinned Chrome for Testing; MANUK_CHROME overrides). Serves
scripts/mcp-smoke/page.html on loopback, navigates, finds the button, clicks it, and requires the page
to say 'clicked-ok'. Exit 0 = pass. Any other outcome prints why and exits 1.
"""
import http.server, os, socketserver, sys, threading, json
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "python"))
from manuk_client import Manuk

REQUIRED = {"navigate", "observe", "act", "find", "extract", "screenshot", "scroll", "type"}

def main():
    binp = sys.argv[1]
    class H(http.server.SimpleHTTPRequestHandler):
        def __init__(s, *a, **k): super().__init__(*a, directory=os.path.join(HERE, "mcp-smoke"), **k)
        def log_message(s, *a): pass
    srv = socketserver.TCPServer(("127.0.0.1", 0), H)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    url = f"http://127.0.0.1:{srv.server_address[1]}/page.html"
    env = dict(os.environ); env.pop("MANUK_BACKEND", None)
    os.environ.clear(); os.environ.update(env)
    with Manuk([binp]) as m:
        names = {t["name"] for t in m.tools()}
        print("tools:", len(names), sorted(names)[:12])
        missing = REQUIRED - names
        if missing:
            print("FAIL: tools missing", sorted(missing)); return 1
        print(m.call("navigate", url=url)[:300])
        obs = m.call("observe")
        print(obs[:600])
        found = m.call("find", query="Press me")
        print(found[:300])
        import re
        handle = None
        for src in (found, obs):
            for line in src.splitlines():
                if "Press me" in line:
                    mm = re.search(r"\b(h\d+)\b", line)
                    if mm: handle = mm.group(1); break
            if handle: break
        if not handle:
            print("FAIL: no handle for the button"); return 1
        print("click handle:", handle)
        print(m.call("act", handle=handle, action="click")[:300])
        after = m.call("observe")
        if "clicked-ok" not in after:
            print("FAIL: page did not change after click\n", after[:600]); return 1
    print("PASS: tools listed and one Chrome task completed")
    return 0

sys.exit(main())
