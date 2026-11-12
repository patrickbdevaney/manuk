#!/usr/bin/env python3
"""RG.S1.c.b.a: how many restaurant cards does the OpenDining home page expose, in Chrome and in manuk?

  MANUK_CHROME_PIN=145.0.7632.116 python3 -I docs/sprint/rg/S1cba-measure.py docs/sprint/rg/S1cba-homepage.tsv

Chrome side: S1cba-chrome-home.js (Playwright DOM, pinned CfT 145). Manuk side: target/release/manuk-mcp over
JSON-RPC, navigate then one read per surface: extract text (default and include_hidden=true), extract markdown
(default and include_hidden=true), observe full (default and include_hidden=true). Distinct restaurant names are
counted against the Chrome name list, so a count is only ever a subset of the same 20 names.
Writes one TSV row per surface with chrome_ver; S1cba-gate.py reads that file.
"""
import json, os, subprocess, sys, time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(HERE)))
EXE = os.path.join(ROOT, "target/release/manuk-mcp")
URL = "https://evals-opendining.vercel.app"
WAIT = float(os.environ.get("S1CBA_WAIT", "6"))
NAMES = ["The Royal Dine", "Vintage Bites", "Evening Delight", "River View Café", "Fancy Lights Bistro", "Bar Central",
         "Spacious Eats", "Yellowy Bistro", "Crowded Corner", "Expensive Taste", "Ocean Breeze", "Bistro Soleil", "Sushi Zen",
         "La Trattoria", "Taco Loco", "The Vegan Table", "Steakhouse 101", "Dim Sum Delight", "Pizza Palace", "BBQ Haven"]


class M:
    def __init__(s):
        s.p = subprocess.Popen([EXE], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, env=dict(os.environ))
        s.i = 0
        s.rpc("initialize", {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "S1cba", "version": "0"}})
        s.p.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n"); s.p.stdin.flush()

    def rpc(s, m, pa):
        s.i += 1
        s.p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": s.i, "method": m, "params": pa}) + "\n"); s.p.stdin.flush()
        while True:
            j = json.loads(s.p.stdout.readline())
            if j.get("id") == s.i:
                return j

    def call(s, n, a):
        r = s.rpc("tools/call", {"name": n, "arguments": a})
        return "\n".join(c.get("text", "") for c in r["result"]["content"])


def chrome():
    out = subprocess.run(["node", os.path.join(HERE, "S1cba-chrome-home.js")], capture_output=True, text=True, timeout=300,
                         env=dict(os.environ, S1CBA_WAIT=str(WAIT)), check=True).stdout
    return json.loads(out)


def main():
    pin = os.environ.get("MANUK_CHROME_PIN")
    if not pin:
        sys.exit("refusing: MANUK_CHROME_PIN unset")
    dest = sys.argv[1]
    c = chrome()
    if c["chrome_ver"] != pin:
        sys.exit(f"refusing: chrome reports {c['chrome_ver']}, pin is {pin}")
    chrome_n = len(c["distinct_names"])
    m = M()
    m.call("navigate", {"url": URL}); time.sleep(WAIT)
    reads = [("extract-text", "extract", {"format": "text"}), ("extract-text-hidden", "extract", {"format": "text", "include_hidden": True}),
             ("extract-markdown", "extract", {"format": "markdown"}),
             ("extract-markdown-hidden", "extract", {"format": "markdown", "include_hidden": True}),
             ("observe-full", "observe", {"mode": "full", "budget_tokens": 60000}),
             ("observe-full-hidden", "observe", {"mode": "full", "budget_tokens": 60000, "include_hidden": True})]
    rows = []
    for label, tool, args in reads:
        txt = m.call(tool, args)
        seen = [n for n in NAMES if n in txt]
        rows.append({"surface": label, "chrome_ver": pin, "chrome_cards": chrome_n, "manuk_cards": len(seen), "names": "|".join(seen)})
    m.p.kill()
    with open(dest, "w") as f:
        cols = ["surface", "chrome_ver", "chrome_cards", "manuk_cards", "names"]
        f.write("\t".join(cols) + "\n")
        for r in rows:
            f.write("\t".join(str(r[k]) for k in cols) + "\n")
    print(open(dest).read())


if __name__ == "__main__":
    main()
