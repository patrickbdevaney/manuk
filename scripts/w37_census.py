#!/usr/bin/env python3
"""W3.7 step 1: how much of the interactive surface sits inside an iframe? Drives manuk-mcp --backend chrome over
a URL list, observes each page, and counts frames and the interactive lines nested under them.
usage: w37_census.py URLLIST OUT.tsv   (URLLIST lines: `<kind>\t<url>` or `<url>`)"""
import json, subprocess, sys, re, time, urllib.parse

BIN = "target/release/manuk-mcp"
INTERACTIVE = re.compile(r"\] (textbox|searchbox|button|checkbox|radio|combobox|link|switch|slider|spinbutton|listbox|tab|menuitem)\b")

class C:
    def __init__(self):
        self.p = subprocess.Popen([BIN, "--backend", "chrome"], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                  stderr=subprocess.DEVNULL, text=True, bufsize=1)
        self.id = 0
        self.rpc("initialize", {"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "c", "version": "0"}})
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n"); self.p.stdin.flush()
    def rpc(self, m, params):
        self.id += 1
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": self.id, "method": m, "params": params}) + "\n"); self.p.stdin.flush()
        while True:
            l = self.p.stdout.readline()
            if not l: raise EOFError
            v = json.loads(l)
            if v.get("id") == self.id: return v
    def tool(self, n, a):
        r = self.rpc("tools/call", {"name": n, "arguments": a}).get("result", {})
        return "\n".join(b.get("text", "") for b in r.get("content", []))
    def close(self): self.p.kill()

def origin(u):
    p = urllib.parse.urlsplit(u); return f"{p.scheme}://{p.netloc}"

rows = []
for line in open(sys.argv[1]):
    line = line.strip()
    if not line: continue
    kind, url = (line.split("\t", 1) + [""])[:2] if "\t" in line else ("", line)
    c = C()
    try:
        nav = c.tool("navigate", {"url": url}); time.sleep(3)
        obs = c.tool("observe", {})
        top = origin(url)
        frames = re.findall(r'region "iframe ([^"]+)"', obs)
        same = sum(1 for f in frames if f == top)
        cross = len(frames) - same
        inter = [l for l in obs.splitlines() if INTERACTIVE.search(l)]
        nested = [l for l in inter if "iframe " in l.split("]", 1)[-1] and ' in ' in l]
        rows.append((kind, url, len(frames), same, cross, len(inter), len(nested), ""))
    except Exception as e:
        rows.append((kind, url, "", "", "", "", "", f"ERR {str(e)[:60]}"))
    finally:
        c.close()
    print(rows[-1], flush=True)
with open(sys.argv[2], "w") as f:
    f.write("kind\turl\tframes\tsame_origin\tcross_origin\tinteractive\tin_frame\tnote\tchrome_ver\n")
    for r in rows: f.write("\t".join(map(str, r)) + "\t145.0.7632.116\n")
