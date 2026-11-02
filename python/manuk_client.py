"""Thin MCP stdio client for manuk-mcp (standard library only).

    with Manuk(["manuk-mcp", "--backend", "chrome"]) as m:
        print([t["name"] for t in m.tools()])
        print(m.call("navigate", url="https://example.com/"))
"""
import json, subprocess


class Manuk:
    def __init__(self, argv=("manuk-mcp",)):
        self.p = subprocess.Popen(list(argv), stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
        self._id = 0
        self._rpc("initialize", {"protocolVersion": "2024-11-05", "capabilities": {},
                                 "clientInfo": {"name": "manuk-client-py", "version": "0"}})
        self._send({"jsonrpc": "2.0", "method": "notifications/initialized"})

    def _send(self, msg):
        self.p.stdin.write(json.dumps(msg) + "\n")
        self.p.stdin.flush()

    def _rpc(self, method, params):
        self._id += 1
        self._send({"jsonrpc": "2.0", "id": self._id, "method": method, "params": params})
        while True:
            line = self.p.stdout.readline()
            if not line:
                raise RuntimeError("manuk-mcp closed stdout")
            msg = json.loads(line)
            if msg.get("id") == self._id:  # skip notifications such as tools/list_changed
                if "error" in msg:
                    raise RuntimeError(msg["error"])
                return msg["result"]

    def tools(self):
        return self._rpc("tools/list", {})["tools"]

    def call(self, name, **arguments):
        r = self._rpc("tools/call", {"name": name, "arguments": arguments})
        return "".join(c.get("text", "") for c in r.get("content", []))

    def close(self):
        self.p.stdin.close()
        self.p.wait(timeout=10)

    def __enter__(self):
        return self

    def __exit__(self, *a):
        self.close()
