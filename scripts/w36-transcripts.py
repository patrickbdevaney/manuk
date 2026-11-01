#!/usr/bin/env python3
"""W3.6.a: drive manuk-mcp over stdio against 10 fixture pages (one element each), click each element twice,
and write the transcripts to docs/sprint/w36/transcripts/<task>_r<run>.txt. The hand labels in
docs/sprint/w36/hand-labels.tsv were written from each page's design BEFORE the classifier was run.
Usage: python3 -I scripts/w36-transcripts.py   (needs target/release/manuk-mcp; MANUK_CHROME_PIN is recorded)
"""
import http.server, json, os, select, subprocess, sys, tempfile, threading

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, ".."))
BIN = os.path.join(REPO, "target", "release", "manuk-mcp")
OUT = os.path.join(REPO, "docs", "sprint", "w36", "transcripts")
CHROME = os.environ.get("MANUK_CHROME_PIN", "")

# task -> (element label, element html)
PAGES = {
    "H1": ("Broken save", '<button onclick="callAnUnimplementedApi()">Broken save</button>'),
    "H2": ("Null read", '<button onclick="document.getElementById(\'nope\').value">Null read</button>'),
    "H3": ("Boom box", '<div onclick="throw new Error(\'boom\')">Boom box</div>'),
    "H4": ("Draft", '<span onclick="saveDraft()">Draft</span>'),
    "H5": ("Parse", '<button onclick="JSON.parse(\'{\')">Parse</button>'),
    "T1": ("Just a label", '<h2>Just a label</h2>'),
    "T2": ("Plain paragraph", '<p>Plain paragraph</p>'),
    "T3": ("Plain heading", '<h3>Plain heading</h3>'),
    "T4": ("Caption text", '<h4>Caption text</h4>'),
    "T5": ("Item one", '<h5>Item one</h5>'),
}


def page(body):
    return ('<!doctype html><html><head><title>Lab</title></head><body><h1>Lab</h1>'
            + body + '<div style="height:3000px">tall</div></body></html>')


class Mcp:
    def __init__(s, logdir):
        env = dict(os.environ, MANUK_ACTLOG_DIR=logdir)
        s.p = subprocess.Popen([BIN], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                               stderr=subprocess.DEVNULL, text=True, bufsize=1, env=env)
        s.id = 0
        s.rpc("initialize", {"protocolVersion": "2025-03-26", "capabilities": {},
                             "clientInfo": {"name": "w36", "version": "0"}})
        s.p.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n")
        s.p.stdin.flush()

    def rpc(s, m, params, timeout=120):
        s.id += 1
        s.p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": s.id, "method": m, "params": params}) + "\n")
        s.p.stdin.flush()
        while True:
            if not select.select([s.p.stdout], [], [], timeout)[0]:
                raise TimeoutError(m)
            v = json.loads(s.p.stdout.readline())
            if v.get("id") == s.id:
                return v

    def tool(s, name, **a):
        r = s.rpc("tools/call", {"name": name, "arguments": a})["result"]
        if r.get("isError"):
            raise RuntimeError(f"{name}: {r['content'][0]['text'][:300]}")
        return "\n".join(c.get("text", "") for c in r["content"])

    def close(s):
        s.p.kill()


def handle_of(text, label):
    for line in text.splitlines():
        if label in line and "[h" in line:
            a = line.index("[h")
            return line[a + 1:line.index("]", a)]
    raise RuntimeError(f"no handle for {label!r} in:\n{text[:800]}")


class Quiet(http.server.BaseHTTPRequestHandler):
    body = b""

    def do_GET(self):
        self.send_response(200)
        self.send_header("Content-Type", "text/html")
        self.send_header("Content-Length", str(len(self.body)))
        self.end_headers()
        self.wfile.write(self.body)

    def log_message(self, *a):
        pass


def main():
    os.makedirs(OUT, exist_ok=True)
    logdir = tempfile.mkdtemp(prefix="w36-actlog-")
    srv = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Quiet)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    base = f"http://127.0.0.1:{srv.server_address[1]}/"
    commit = subprocess.run(["git", "-C", REPO, "rev-parse", "--short", "HEAD"],
                            capture_output=True, text=True).stdout.strip()
    for task, (label, html) in PAGES.items():
        Quiet.body = page(html).encode()
        m = Mcp(logdir)
        try:
            nav = m.tool("navigate", url=base)
            h = handle_of(nav, label)
            acts = [m.tool("act", handle=h, action="click") for _ in range(2)]
        finally:
            m.close()
        with open(os.path.join(OUT, f"{task}_r1.txt"), "w") as f:
            f.write(f"# task={task} run=1 chrome_ver={CHROME} head_at_run={commit} backend=manuk\n")
            f.write(f"# element: {html}\n")
            for i, a in enumerate(acts, 1):
                f.write(f"--- act {i} (click {h})\n{a}\n")
        print(task, "->", [l for l in acts[0].splitlines() if "NO EFFECT" in l or "listener" in l])
    srv.shutdown()


if __name__ == "__main__":
    main()
