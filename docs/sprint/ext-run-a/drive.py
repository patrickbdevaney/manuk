#!/usr/bin/env python3
"""Drive manuk-mcp over stdio: navigate to a URL, print the observation. usage: drive.py BIN URL [extra-args...]"""
import json, subprocess, sys, time
binp, url = sys.argv[1], sys.argv[2]
p = subprocess.Popen([binp] + sys.argv[3:], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
def send(m): p.stdin.write(json.dumps(m) + "\n"); p.stdin.flush()
def recv(): return json.loads(p.stdout.readline())
send({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"d","version":"0"}}})
recv(); send({"jsonrpc":"2.0","method":"notifications/initialized"})
send({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"navigate","arguments":{"url":url}}})
r = recv()
print("".join(c.get("text","") for c in r["result"]["content"]))
import os
time.sleep(float(os.environ.get("WAIT","8")))
send({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"extract","arguments":{"format":"text"}}})
r = recv()
print("=== EXTRACT ===")
print("".join(c.get("text","") for c in r["result"]["content"]))
p.stdin.close(); p.terminate()
print("STDERR:", p.stderr.read()[-1500:], file=sys.stderr)
