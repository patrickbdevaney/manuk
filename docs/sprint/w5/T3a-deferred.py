#!/usr/bin/env python3
"""W5.T3.a: a deferred tool (not in the six always-load set) is reached and succeeds in a real `claude -p --model haiku` session.

A local fixture page is served on 127.0.0.1; the agent is told to call `storage_state` (deferred at W5.T3) and to
report the page title. A run passes when the stream shows a successful tool_result for mcp__browser__storage_state
and the final answer contains the marker word. Every row carries chrome_ver (MANUK_CHROME_PIN).

Usage: python3 docs/sprint/w5/T3a-deferred.py --bin BIN --out TSV [--runs 3]
"""
import argparse, http.server, json, os, socketserver, subprocess, sys, tempfile, threading

CLAUDE = os.path.expanduser("~/.local/bin/claude")
MARKER = "QUOKKA-7741"
PAGE = f"<!doctype html><html><head><title>{MARKER}</title></head><body><h1>Fixture</h1></body></html>"
PROMPT = ("Navigate to {url} with the browser tools. Then call the storage_state tool (it is a browser tool; "
          "load it first if it is not loaded). Finally reply with the exact page title you saw.")
COLS = ["chrome_ver", "arm", "run", "passed", "deferred_tool_ok", "toolsearch_calls", "is_error", "bin_sha256", "rc"]


def serve(root):
    class Quiet(http.server.SimpleHTTPRequestHandler):
        def log_message(self, *a):
            pass
    handler = lambda *a, **k: Quiet(*a, directory=root, **k)  # noqa: E731
    srv = socketserver.TCPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv


def run_once(cwd, cfg, url):
    cmd = [CLAUDE, "-p", "--model", "haiku", "--output-format", "stream-json", "--verbose",
           "--mcp-config", cfg, "--strict-mcp-config", "--allowedTools", "mcp__browser", "--permission-mode", "dontAsk",
           PROMPT.format(url=url)]
    p = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, timeout=600, stdin=subprocess.DEVNULL)
    ok_deferred, toolsearch, answer, is_error = False, 0, "", None
    pending = {}
    for line in p.stdout.splitlines():
        try:
            e = json.loads(line)
        except ValueError:
            continue
        msg = e.get("message") or {}
        for block in msg.get("content") or []:
            if not isinstance(block, dict):
                continue
            if block.get("type") == "tool_use":
                pending[block.get("id")] = block.get("name")
                if block.get("name") == "ToolSearch":
                    toolsearch += 1
            if block.get("type") == "tool_result" and pending.get(block.get("tool_use_id")) == "mcp__browser__storage_state":
                if not block.get("is_error"):
                    ok_deferred = True
        if e.get("type") == "result":
            answer = e.get("result", "")
            is_error = e.get("is_error")
    passed = ok_deferred and MARKER in answer
    return passed, ok_deferred, toolsearch, is_error, p.returncode


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--bin", required=True)
    ap.add_argument("--arm", default="manuk-after-W5.T3")
    ap.add_argument("--out", required=True)
    ap.add_argument("--runs", type=int, default=3)
    a = ap.parse_args()
    pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
    root = tempfile.mkdtemp(prefix="w5t3a-site-")
    with open(os.path.join(root, "index.html"), "w") as f:
        f.write(PAGE)
    cwd = tempfile.mkdtemp(prefix="w5t3a-deferred-cwd-")
    cfg = os.path.join(cwd, "mcp.json")
    with open(cfg, "w") as f:
        json.dump({"mcpServers": {"browser": {"command": a.bin, "args": []}}}, f)
    srv = serve(root)
    url = f"http://127.0.0.1:{srv.server_address[1]}/index.html"
    sha = subprocess.run(["sha256sum", a.bin], capture_output=True, text=True).stdout.split()[0][:16]
    rows = []
    for run in range(1, a.runs + 1):
        passed, dok, ts, ie, rc = run_once(cwd, cfg, url)
        rows.append([pin, a.arm, run, int(passed), int(dok), ts, ie, sha, rc])
        print("\t".join(map(str, rows[-1])), flush=True)
    srv.shutdown()
    with open(a.out, "w") as f:
        f.write("\t".join(COLS) + "\n")
        for row in rows:
            f.write("\t".join(map(str, row)) + "\n")
    print("passed", sum(r[3] for r in rows), "of", len(rows))


if __name__ == "__main__":
    main()
