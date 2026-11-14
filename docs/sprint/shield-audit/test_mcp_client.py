#!/usr/bin/env python3
"""R1.4.a.d.b: the audit's Mcp client must not stall when a server notification and a response share one pipe read.

manuk-mcp announces `notifications/tools/list_changed` just before a mutating tools/call's response. The old
rpc() waited on select() for the pipe, but readline() had already buffered the response, so select saw an
empty pipe and waited out the 180 s timeout (pages.tsv: "manuk-mcp silent for 180s on tools/call").

Run:  /tmp/audit-venv/bin/python -I test_mcp_client.py      (exit 1 when a call stalls)
"""
import os, sys, time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import audit  # noqa: E402

CALLS = 20
TIMEOUT = 5


def main():
    audit.MCP_BIN = os.path.join(HERE, "fake_mcp.py")
    mcp = audit.Mcp()
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
