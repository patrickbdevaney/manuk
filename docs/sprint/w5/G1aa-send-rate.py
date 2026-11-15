#!/usr/bin/env python3
"""W5.G1.a.a.a.a: gomail-3 on cdp-layer, send-vs-draft per run.

Usage: python3 -I G1aa-send-rate.py [RUNS_DIR]   (default: target/real-runs under the repo root)

For each fresh run cdp-layer_gomail-3_r{4..8} it reads state.json (the /finish diff) and transcript.jsonl and
checks that the two agree:
  * a Send click = an act(click) whose handle was last observed as button "Send" in the observation text;
  * a sent email = differences.emails.added is non-empty.
Prints one line per run and the send rate. Exits 1 if a run's Send click and added email disagree, or if a
run folder or file is missing (the check fails closed).
"""
import json, os, re, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(HERE)))
RUNS = [f"cdp-layer_gomail-3_r{n}" for n in range(4, 9)]
OBS = re.compile(r'\[(h\d+)\] (\w+)(?: "([^"]*)")?')


def tool_results(o):
    for c in o.get("message", {}).get("content", []) or []:
        if isinstance(c, dict) and c.get("type") == "tool_result":
            body = c.get("content")
            yield body if isinstance(body, str) else " ".join(x.get("text", "") for x in body or [])


def scan(path):
    """Return (send_clicks, click_count) from a transcript, reading each handle's label from the latest observation."""
    names, sends, clicks = {}, 0, 0
    for line in open(path, encoding="utf-8"):
        try:
            o = json.loads(line)
        except ValueError:
            continue
        if o.get("type") == "user":
            for text in tool_results(o):
                for h, _role, name in OBS.findall(text):
                    names[h] = name
        if o.get("type") != "assistant":
            continue
        for c in o.get("message", {}).get("content", []) or []:
            if isinstance(c, dict) and c.get("type") == "tool_use" and c.get("name", "").endswith("__act"):
                inp = c.get("input", {})
                if inp.get("action") == "click":
                    clicks += 1
                    if names.get(inp.get("handle")) == "Send":
                        sends += 1
    return sends, clicks


def main():
    root = sys.argv[1] if len(sys.argv) > 1 else os.path.join(ROOT, "target/real-runs")
    bad, sent = [], 0
    for run in RUNS:
        d = os.path.join(root, run)
        try:
            state = json.load(open(os.path.join(d, "state.json")))["state"]
            sends, clicks = scan(os.path.join(d, "transcript.jsonl"))
        except (OSError, KeyError, TypeError) as e:
            print(f"{run}: MISSING or unreadable ({e})")
            bad.append(run)
            continue
        added = state["differences"]["emails"]["added"]
        ok = bool(added) == (sends > 0)
        sent += bool(added)
        print(f"{run}: send_clicks={sends} act_clicks={clicks} added={len(added)} "
              f"{'AGREE' if ok else 'MISMATCH'}")
        if not ok:
            bad.append(run)
    print(f"sent {sent}/{len(RUNS)} runs (draft-only {len(RUNS) - sent})")
    if bad:
        print("FAIL:", ", ".join(bad))
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
