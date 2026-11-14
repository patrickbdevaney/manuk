#!/usr/bin/env python3
"""W5.T3.a.a.a.a.a.a.a: does the parent Claude Code environment decide whether the navigate-quiet task calls ToolSearch?

T3aaaaaa found 0 of 12 runs call ToolSearch today, while the T3aaaaa run found 8 of 9. The claude child inherits the
CLAUDE_CODE_* variables of the session that launched it. This probe runs the same navigate-quiet task (same prompt,
fixture, cwd rules and claude flags as T3aa-perturn.py) under two environments on the same binary:

  inherit  the environment as launched (CLAUDE_CODE_*, CLAUDECODE, AI_AGENT, CLAUDE_EFFORT, CLAUDE_PID kept)
  stripped the same environment with those variables removed

Per run it records whether ToolSearch was in the init tool list (so it could be called at all) and whether it was
called. A run with no init event is refused (exit 2), because the init list is the only evidence the probe has.

Usage: python3 docs/sprint/w5/T3aaaaaaa-envprobe.py --bin BIN --out TSV [--runs 5]
Requires MANUK_CHROME_PIN (every row carries chrome_ver).
"""
import argparse, importlib.util, json, os, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
_spec = importlib.util.spec_from_file_location("perturn", os.path.join(HERE, "T3aa-perturn.py"))
perturn = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(perturn)

COLS = ["chrome_ver", "env", "run", "init_tools", "init_has_toolsearch", "toolsearch_calls", "claude_version",
        "bin_sha256"]
PREFIXES = ("CLAUDE_CODE_", "CLAUDECODE", "AI_AGENT", "CLAUDE_EFFORT", "CLAUDE_PID")


class Refused(Exception):
    pass


def env_for(kind):
    env = dict(os.environ)
    if kind == "stripped":
        for k in list(env):
            if k.startswith(PREFIXES):
                del env[k]
    return env


def probe(stdout):
    """Return (init tool names, number of ToolSearch tool_use blocks) from one stream-json run."""
    init, calls = None, 0
    for line in stdout.splitlines():
        try:
            e = json.loads(line)
        except ValueError:
            continue
        if e.get("type") == "system" and e.get("subtype") == "init":
            init = e.get("tools") or []
        if e.get("type") == "assistant":
            for block in (e.get("message") or {}).get("content") or []:
                if isinstance(block, dict) and block.get("type") == "tool_use" and block.get("name") == "ToolSearch":
                    calls += 1
    if init is None:
        raise Refused("no system/init event in the stream; the tool list is unknown")
    return sorted(init), calls


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--bin")
    ap.add_argument("--out")
    ap.add_argument("--runs", type=int, default=5)
    a = ap.parse_args()
    if not (a.bin and a.out):
        sys.exit("--bin and --out are required")
    pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
    root = tempfile.mkdtemp(prefix="w5envprobe-site-")
    with open(os.path.join(root, "index.html"), "w") as f:
        f.write(perturn.PAGE)
    cwd = tempfile.mkdtemp(prefix="w5envprobe-cwd-")
    srv = perturn.serve(root)
    url = f"http://127.0.0.1:{srv.server_address[1]}/index.html"
    ver = perturn.claude_version()
    binsha = perturn.sha(a.bin)
    cfg = os.path.join(cwd, "after.json")
    with open(cfg, "w") as f:
        json.dump({"mcpServers": {"browser": {"command": a.bin, "args": []}}}, f)
    prompt = perturn.PROMPTS["navigate-quiet"].format(url=url)
    rows = []
    try:
        for kind in ("inherit", "stripped"):
            for run in range(1, a.runs + 1):
                cmd = [perturn.CLAUDE, "-p", "--model", "haiku", "--output-format", "stream-json", "--verbose",
                       "--mcp-config", cfg, "--strict-mcp-config", "--allowedTools", "mcp__browser",
                       "--permission-mode", "dontAsk", prompt]
                p = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, timeout=600,
                                   stdin=subprocess.DEVNULL, env=env_for(kind))
                tools, calls = probe(p.stdout)
                has = int("ToolSearch" in tools)
                rows.append([pin, kind, run, len(tools), has, calls, ver, binsha])
                print("\t".join(map(str, rows[-1])), flush=True)
    except Refused as e:
        srv.shutdown()
        sys.exit(f"refused: {e}")
    srv.shutdown()
    with open(a.out, "w") as f:
        f.write("\t".join(COLS) + "\n")
        for r in rows:
            f.write("\t".join(map(str, r)) + "\n")
    for kind in ("inherit", "stripped"):
        sub = [r for r in rows if r[1] == kind]
        print(f"{kind}: n={len(sub)} init_has_toolsearch={sum(r[4] for r in sub)} "
              f"runs_with_toolsearch_call={sum(1 for r in sub if r[5] > 0)} written={a.out}")


if __name__ == "__main__":
    main()
