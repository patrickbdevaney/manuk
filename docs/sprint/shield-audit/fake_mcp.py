#!/usr/bin/env python3
"""Stand-in for manuk-mcp in test_mcp_client.py: stdio JSON-RPC that answers like the real server.

Like the real server (mcp/src/server.rs), a mutating tools/call sends `notifications/tools/list_changed`
BEFORE its response. Both lines go out in one write, so a client reading the pipe can get them in one chunk.
"""
import json, sys

for line in sys.stdin:
    msg = json.loads(line)
    if "id" not in msg:
        continue
    if msg["method"] == "initialize":
        out = {"protocolVersion": "2025-03-26", "capabilities": {}, "serverInfo": {"name": "fake", "version": "0"}}
        sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": msg["id"], "result": out}) + "\n")
    else:
        note = {"jsonrpc": "2.0", "method": "notifications/tools/list_changed"}
        res = {"jsonrpc": "2.0", "id": msg["id"], "result": {"content": [{"type": "text", "text": "ok " + msg["params"]["name"]}]}}
        sys.stdout.write(json.dumps(note) + "\n" + json.dumps(res) + "\n")
    sys.stdout.flush()
