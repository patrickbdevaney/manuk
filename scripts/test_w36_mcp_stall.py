#!/usr/bin/env python3
"""R1.4.a.d.b.a: w36-transcripts.py's MCP client must not stall when a server notification and a response share one pipe read.

manuk-mcp announces `notifications/tools/list_changed` just before a mutating tools/call's response. The old rpc()
waited on select() for the pipe, but readline() had already buffered the response, so select saw an empty pipe and
waited out the timeout. The fake server (docs/sprint/shield-audit/fake_mcp.py) writes both lines in one write.

Run:  python3 -I scripts/test_w36_mcp_stall.py   (exit 1 when a call stalls)
"""
import importlib.util, os, sys, time

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, ".."))
FAKE = os.path.join(REPO, "docs", "sprint", "shield-audit", "fake_mcp.py")

CALLS = 20
TIMEOUT = 5


def load_w36():
    spec = importlib.util.spec_from_file_location("w36", os.path.join(HERE, "w36-transcripts.py"))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def main():
    w36 = load_w36()
    w36.BIN = FAKE
    mcp = w36.Mcp(logdir=os.devnull)
    stalled, answered = [], 0
    for i in range(CALLS):
        t0 = time.time()
        try:
            r = mcp.rpc("tools/call", {"name": "navigate", "arguments": {"url": f"http://x/{i}"}}, timeout=TIMEOUT)
            text = r["result"]["content"][0]["text"]
            assert text == "ok navigate", text
            answered += 1
        except TimeoutError as e:
            stalled.append(f"call {i}: {e} (after {time.time() - t0:.1f}s)")
            break
    mcp.close()
    for s in stalled:
        print("STALLED " + s)
    print(f"{answered}/{CALLS} calls answered; GATE " + ("GREEN" if not stalled else "RED"))
    sys.exit(1 if stalled else 0)


if __name__ == "__main__":
    main()
