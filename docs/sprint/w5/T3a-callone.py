#!/usr/bin/env python3
"""W5.T3.a: call-1 context tokens of `claude -p --model haiku`, manuk before vs after W5.T3 (n=9 each), plus a no-MCP control.

Each run sends one trivial prompt that must not call a tool, so the first API call's input (system prompt, tool
definitions, MCP instructions) is what is measured: context = input + cache_read + cache_create of the first
assistant message. Cache state does not change the sum, so runs are comparable across cache hits and misses.

Usage: python3 docs/sprint/w5/T3a-callone.py --before BIN --after BIN --out docs/sprint/w5/T3a-callone.tsv
Refuses to run without MANUK_CHROME_PIN (every row carries chrome_ver).
"""
import argparse, hashlib, json, os, subprocess, sys, tempfile

CLAUDE = os.path.expanduser("~/.local/bin/claude")
PROMPT = "Reply with the single word OK and call no tools."
COLS = ["chrome_ver", "arm", "run", "context_tokens", "input_tokens", "cache_read_tokens", "cache_create_tokens",
        "init_tools", "mcp_tools_listed", "bin_sha256", "claude_version", "rc"]


def sha(path):
    if not path:
        return "none"
    with open(path, "rb") as f:
        return hashlib.sha256(f.read()).hexdigest()[:16]


def one_run(cwd, cfg_path):
    cmd = [CLAUDE, "-p", "--model", "haiku", "--output-format", "stream-json", "--verbose",
           "--allowedTools", "mcp__browser", "--permission-mode", "dontAsk"]
    cmd += ["--mcp-config", cfg_path, "--strict-mcp-config"]
    cmd.append(PROMPT)
    p = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, timeout=300, stdin=subprocess.DEVNULL)
    init_tools, mcp_listed, usage = None, None, None
    for line in p.stdout.splitlines():
        try:
            e = json.loads(line)
        except ValueError:
            continue
        if e.get("type") == "system" and e.get("subtype") == "init":
            tools = e.get("tools", [])
            init_tools = len(tools)
            mcp_listed = sum(1 for t in tools if t.startswith("mcp__browser__"))
        if e.get("type") == "assistant" and usage is None:
            usage = e["message"].get("usage") or {}
    if usage is None:
        return None, p.returncode, init_tools, mcp_listed, p.stderr[-400:]
    i = usage.get("input_tokens", 0)
    r = usage.get("cache_read_input_tokens", 0)
    c = usage.get("cache_creation_input_tokens", 0)
    return (i, r, c), p.returncode, init_tools, mcp_listed, ""


def claude_version():
    return subprocess.run([CLAUDE, "--version"], capture_output=True, text=True).stdout.strip()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--before", required=True)
    ap.add_argument("--after", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--runs", type=int, default=9)
    a = ap.parse_args()
    pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
    cwd = tempfile.mkdtemp(prefix="w5t3a-cwd-")  # empty, so no CLAUDE.md or project context leaks into either arm
    arms = [("manuk-before-W5.T3", a.before), ("manuk-after-W5.T3", a.after), ("no-mcp-control", None)]
    ver = claude_version()
    rows = []
    for arm, binp in arms:
        cfg_path = os.path.join(cwd, f"{arm}.json")
        servers = {"browser": {"command": binp, "args": []}} if binp else {}
        with open(cfg_path, "w") as f:
            json.dump({"mcpServers": servers}, f)
        for run in range(1, a.runs + 1):
            got, rc, it, ml, err = one_run(cwd, cfg_path)
            if got is None:
                sys.exit(f"refusing: {arm} run {run} produced no assistant usage (rc={rc}): {err}")
            i, r, c = got
            rows.append([pin, arm, run, i + r + c, i, r, c, it, ml, sha(binp), ver, rc])
            print("\t".join(map(str, rows[-1])), flush=True)
    with open(a.out, "w") as f:
        f.write("\t".join(COLS) + "\n")
        for row in rows:
            f.write("\t".join(map(str, row)) + "\n")
    print("wrote", a.out, len(rows), "rows")


if __name__ == "__main__":
    main()
