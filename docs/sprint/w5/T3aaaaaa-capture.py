#!/usr/bin/env python3
"""W5.T3.a.a.a.a.a.a: record the ToolSearch calls a navigate-quiet task makes on one manuk binary.

W5.T3.a.a.a.a.a found the after-W5.T3 arm calls ToolSearch in 8 of 9 navigate-quiet runs, and the transcript was not
kept. This script reruns the same task (PROMPTS["navigate-quiet"] from T3aa-perturn.py, same fixture, same empty cwd,
same claude flags) and keeps the stream-json tool_use input of every ToolSearch call and the tool_result that answered
it, so the query string and the tool it selects are on record.

Refuses (exit 2, nothing written) when a ToolSearch tool_use has no query recorded, or when a tool_use has no matching
tool_result. Those are the only ways the capture could report a ToolSearch that it did not see.

Usage: python3 docs/sprint/w5/T3aaaaaa-capture.py --bin BIN --out TSV [--runs 9]
       python3 docs/sprint/w5/T3aaaaaa-capture.py --parse-fixture STREAM_JSON   (offline check of the parser)
Requires MANUK_CHROME_PIN (every row carries chrome_ver).
"""
import argparse, importlib.util, json, os, sys, tempfile, threading

HERE = os.path.dirname(os.path.abspath(__file__))
_spec = importlib.util.spec_from_file_location("perturn", os.path.join(HERE, "T3aa-perturn.py"))
perturn = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(perturn)

COLS = ["chrome_ver", "run", "call", "query", "max_results", "selected_tools", "tool_result_head",
        "bin_sha256", "claude_version"]


class Refused(Exception):
    pass


def parse_stream(text):
    """Return the ToolSearch calls of one stream-json run as dicts, in order.

    Each dict: query, max_results (str or ""), selected (tool names found in the tool_result), result_head.
    Raises Refused on a ToolSearch tool_use with no query, or a tool_use whose tool_result never arrives.
    """
    uses, results = [], {}
    for line in text.splitlines():
        try:
            e = json.loads(line)
        except ValueError:
            continue
        msg = e.get("message") or {}
        for block in msg.get("content") or []:
            if not isinstance(block, dict):
                continue
            if block.get("type") == "tool_use" and e.get("type") == "assistant" and block.get("name") == "ToolSearch":
                uses.append(block)
            if block.get("type") == "tool_result":
                results[block.get("tool_use_id")] = block.get("content")
    calls = []
    for u in uses:
        inp = u.get("input") or {}
        query = inp.get("query")
        if not isinstance(query, str) or not query.strip():
            raise Refused(f"ToolSearch tool_use {u.get('id')} has no query recorded (input={inp!r})")
        if u.get("id") not in results:
            raise Refused(f"ToolSearch tool_use {u.get('id')} has no tool_result in the stream")
        body = results[u["id"]]
        if isinstance(body, list):
            body = " ".join(b.get("text", "") for b in body if isinstance(b, dict))
        body = str(body or "")
        selected = sorted(set(t for t in _names(body)))
        calls.append({"query": query.strip(), "max_results": str(inp.get("max_results", "")),
                      "selected": "|".join(selected) or "-", "result_head": " ".join(body.split())[:120]})
    return calls


def _names(body):
    # ToolSearch results list the loaded tools as "mcp__<server>__<name>" tokens
    import re
    return re.findall(r"mcp__[A-Za-z0-9_\-]+", body)


def run_once(cwd, cfg, url, prompt):
    import subprocess
    cmd = [perturn.CLAUDE, "-p", "--model", "haiku", "--output-format", "stream-json", "--verbose",
           "--mcp-config", cfg, "--strict-mcp-config", "--allowedTools", "mcp__browser",
           "--permission-mode", "dontAsk", prompt.format(url=url)]
    p = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, timeout=600, stdin=subprocess.DEVNULL)
    return p.stdout, p.returncode


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--bin")
    ap.add_argument("--out")
    ap.add_argument("--runs", type=int, default=9)
    ap.add_argument("--parse-fixture")
    a = ap.parse_args()
    if a.parse_fixture:
        with open(a.parse_fixture) as f:
            try:
                print(json.dumps(parse_stream(f.read()), indent=1))
            except Refused as e:
                sys.exit(f"refused: {e}")
        return
    if not (a.bin and a.out):
        sys.exit("--bin and --out are required")
    pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
    root = tempfile.mkdtemp(prefix="w5t3aaaaaa-site-")
    with open(os.path.join(root, "index.html"), "w") as f:
        f.write(perturn.PAGE)
    cwd = tempfile.mkdtemp(prefix="w5t3aaaaaa-cwd-")
    srv = perturn.serve(root)
    url = f"http://127.0.0.1:{srv.server_address[1]}/index.html"
    ver = perturn.claude_version()
    binsha = perturn.sha(a.bin)
    cfg = os.path.join(cwd, "after.json")
    with open(cfg, "w") as f:
        json.dump({"mcpServers": {"browser": {"command": a.bin, "args": []}}}, f)
    rows = []
    try:
        for run in range(1, a.runs + 1):
            text, rc = run_once(cwd, cfg, url, perturn.PROMPTS["navigate-quiet"])
            calls = parse_stream(text)
            if not calls:
                print(f"run {run}: no ToolSearch call (rc={rc})", flush=True)
            for i, c in enumerate(calls, 1):
                rows.append([pin, run, i, c["query"], c["max_results"], c["selected"], c["result_head"],
                             binsha, ver])
                print("\t".join(map(str, rows[-1])), flush=True)
    except Refused as e:
        srv.shutdown()
        sys.exit(f"refused: {e}")
    srv.shutdown()
    with open(a.out, "w") as f:
        f.write("\t".join(COLS) + "\n")
        for r in rows:
            f.write("\t".join(map(str, r)) + "\n")
    runs_with = len({r[1] for r in rows})
    print(f"runs={a.runs} runs_with_ToolSearch={runs_with} calls={len(rows)} written={a.out}")


if __name__ == "__main__":
    main()
