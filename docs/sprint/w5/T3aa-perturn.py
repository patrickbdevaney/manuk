#!/usr/bin/env python3
"""W5.T3.a.a: per-turn input context of a real `claude -p --model haiku` task, manuk before vs after W5.T3 (n=9 each).

Each run is the W5.T3.a deferred-tool task: a local fixture page, then `storage_state` (deferred at W5.T3, so the
after-arm must load it through ToolSearch; the before-arm has it always loaded). Every assistant message of the
stream is one model turn. Its context = input + cache_read + cache_create (cache state does not change the sum).

Output: one row per (arm, run, turn) with the tools that turn called, its context tokens and its output tokens.
The summary (per arm: total task context, first-turn context, and the ToolSearch-turn delta for the after-arm) is
printed and is what docs/sprint/w5/T3aa-join.py re-reads with binary shas checked.

Usage: python3 docs/sprint/w5/T3aa-perturn.py --before BIN --after BIN --out TSV [--runs 9]
Refuses to run without MANUK_CHROME_PIN (every row carries chrome_ver).
"""
import argparse, hashlib, http.server, json, os, socketserver, statistics, subprocess, sys, tempfile, threading

CLAUDE = os.path.expanduser("~/.local/bin/claude")
MARKER = "QUOKKA-7741"
PAGE = f"<!doctype html><html><head><title>{MARKER}</title></head><body><h1>Fixture</h1></body></html>"
PROMPTS = {
    # W5.T3.a.a: a deferred tool (storage_state) is needed, so ToolSearch must run in the after-arm.
    "deferred": ("Navigate to {url} with the browser tools. Then call the storage_state tool (it is a browser tool; "
                 "load it first if it is not loaded). Finally reply with the exact page title you saw."),
    # W5.T3.a.a.a: a navigate+observe task that never needs a deferred tool.
    "navigate": ("Navigate to {url} with the browser tools, then observe the page. "
                 "Finally reply with the exact page title you saw."),
}
COLS = ["chrome_ver", "arm", "run", "turn", "tools", "toolsearch", "context_tokens", "output_tokens",
        "bin_sha256", "claude_version", "rc"]


def sha(path):
    with open(path, "rb") as f:
        return hashlib.sha256(f.read()).hexdigest()[:16]


def serve(root):
    class Quiet(http.server.SimpleHTTPRequestHandler):
        def log_message(self, *a):
            pass
    handler = lambda *a, **k: Quiet(*a, directory=root, **k)  # noqa: E731
    srv = socketserver.TCPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv


def one_run(cwd, cfg, url, prompt):
    """Return a list of turns: (tools called, ToolSearch calls, context tokens, output tokens), plus rc."""
    cmd = [CLAUDE, "-p", "--model", "haiku", "--output-format", "stream-json", "--verbose",
           "--mcp-config", cfg, "--strict-mcp-config", "--allowedTools", "mcp__browser",
           "--permission-mode", "dontAsk", prompt.format(url=url)]
    p = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, timeout=600, stdin=subprocess.DEVNULL)
    turns, order = {}, []  # message id -> accumulated turn, in first-seen order
    for line in p.stdout.splitlines():
        try:
            e = json.loads(line)
        except ValueError:
            continue
        if e.get("type") != "assistant":
            continue
        msg = e.get("message") or {}
        mid = msg.get("id")
        if mid not in turns:
            turns[mid] = {"tools": [], "usage": None, "out": 0}
            order.append(mid)
        t = turns[mid]
        for block in msg.get("content") or []:
            if isinstance(block, dict) and block.get("type") == "tool_use":
                t["tools"].append(block.get("name", "?"))
        u = msg.get("usage")
        if u:  # the usage of a message repeats on each of its stream events; the last one is complete
            t["usage"] = u
    out = []
    for mid in order:
        t, u = turns[mid], turns[mid]["usage"] or {}
        ctx = u.get("input_tokens", 0) + u.get("cache_read_input_tokens", 0) + u.get("cache_creation_input_tokens", 0)
        out.append((t["tools"], ctx, u.get("output_tokens", 0)))
    return out, p.returncode


def claude_version():
    return subprocess.run([CLAUDE, "--version"], capture_output=True, text=True).stdout.strip()


def summarize(arm, rows):
    per_run = {}
    for r in rows:
        per_run.setdefault(r["run"], []).append(r)
    totals = [sum(r["context_tokens"] for r in rs) for rs in per_run.values()]
    firsts = [rs[0]["context_tokens"] for rs in per_run.values()]
    deltas = []
    for rs in per_run.values():
        ts = [r["turn"] for r in rs if r["toolsearch"] > 0]
        if ts:  # context of the turn after the ToolSearch result minus the turn that called it
            k = ts[0]
            after = [r["context_tokens"] for r in rs if r["turn"] == k + 1]
            here = [r["context_tokens"] for r in rs if r["turn"] == k]
            if after and here:
                deltas.append(after[0] - here[0])
    def band(xs):
        if not xs:
            return "n/a"
        sd = statistics.stdev(xs) if len(xs) > 1 else 0.0
        return f"mean={statistics.mean(xs):.0f} sd={sd:.0f} min={min(xs)} max={max(xs)} n={len(xs)}"
    print(f"{arm}: task context total  {band(totals)}")
    print(f"{arm}: first-turn context  {band(firsts)}")
    print(f"{arm}: ToolSearch-turn delta {band(deltas)}")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--before", required=True)
    ap.add_argument("--after", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--runs", type=int, default=9)
    ap.add_argument("--task", choices=sorted(PROMPTS), default="deferred")
    a = ap.parse_args()
    pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
    root = tempfile.mkdtemp(prefix="w5t3aa-site-")
    with open(os.path.join(root, "index.html"), "w") as f:
        f.write(PAGE)
    cwd = tempfile.mkdtemp(prefix="w5t3aa-cwd-")  # empty, so no CLAUDE.md or project context leaks into either arm
    srv = serve(root)
    url = f"http://127.0.0.1:{srv.server_address[1]}/index.html"
    ver = claude_version()
    rows, shas = [], {}
    for arm, binp in [("before-W5.T3", a.before), ("after-W5.T3", a.after)]:
        cfg = os.path.join(cwd, f"{arm}.json")
        with open(cfg, "w") as f:
            json.dump({"mcpServers": {"browser": {"command": binp, "args": []}}}, f)
        shas[arm] = sha(binp)
        for run in range(1, a.runs + 1):
            turns, rc = one_run(cwd, cfg, url, PROMPTS[a.task])
            if not turns:
                sys.exit(f"refusing: {arm} run {run} produced no assistant turns (rc={rc})")
            for i, (tools, ctx, out) in enumerate(turns, 1):
                ts = sum(1 for t in tools if t == "ToolSearch")
                row = {"run": run, "turn": i, "tools": "|".join(tools) or "-", "toolsearch": ts,
                       "context_tokens": ctx, "output_tokens": out, "rc": rc}
                rows.append(dict(row, arm=arm))
                print("\t".join(map(str, [pin, arm, run, i, row["tools"], ts, ctx, out, shas[arm], ver, rc])),
                      flush=True)
    srv.shutdown()
    with open(a.out, "w") as f:
        f.write("\t".join(COLS) + "\n")
        for r in rows:
            f.write("\t".join(map(str, [pin, r["arm"], r["run"], r["turn"], r["tools"], r["toolsearch"],
                                        r["context_tokens"], r["output_tokens"], shas[r["arm"]], ver, r["rc"]])) + "\n")
    for arm in ["before-W5.T3", "after-W5.T3"]:
        summarize(arm, [r for r in rows if r["arm"] == arm])
    print("wrote", a.out, len(rows), "turn rows")


if __name__ == "__main__":
    main()
