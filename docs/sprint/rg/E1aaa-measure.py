#!/usr/bin/env python3
"""RG.2.E1.a.a.a: does manuk's observe flag the zilloft-2 results landmark as an overlay when pinned Chrome does not?

  MANUK_CHROME_PIN=145.0.7632.116 python3 -I docs/sprint/rg/E1aaa-measure.py docs/sprint/rg/E1aaa-consent.tsv [runs=3]

Chrome side: E1aaa-chrome.js (Playwright, pinned CfT 145): after navigate, type "San Francisco", Start search, it reads the
"Search results for homes" main: role, aria-modal on it or an ancestor, dialog ancestors, and how many controls outside it
Chrome's hit test lands inside it. Chrome calls the landmark modal only if one of those holds.
Manuk side: target/release/manuk-mcp over JSON-RPC, same flow, then observe full; the run's overlay kind is the
"CONSENT OVERLAY" / "MODAL OVERLAY" / "BOT WALL" line that names "Search results for homes", or none.
Writes one TSV row per manuk run; E1aaa-gate.py compares them. chrome_ver is on every row.
"""
import json, os, re, subprocess, sys, time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(HERE)))
EXE = os.path.join(ROOT, "target/release/manuk-mcp")
URL = "https://evals-zilloft.vercel.app"
LINE = re.compile(r'(CONSENT|MODAL|BOT WALL) OVERLAY? ?\[h\d+\] "Search results for homes"')


class M:
    def __init__(s):
        s.p = subprocess.Popen([EXE], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, env=dict(os.environ))
        s.i = 0
        s.rpc("initialize", {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "E1aaa", "version": "0"}})
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

    def handle(s, text, pat):
        for l in text.split("\n"):
            if re.search(pat, l) and "[h" in l:
                return l[l.index("[h") + 1:l.index("]", l.index("[h"))]
        sys.exit(f"no handle matching {pat}")


def chrome():
    out = subprocess.run(["node", os.path.join(HERE, "E1aaa-chrome.js")], capture_output=True, text=True, timeout=300, check=True).stdout
    return json.loads(out)


def manuk_run():
    m = M()
    m.call("navigate", {"url": URL})
    m.call("type", {"handle": m.handle(m.call("observe", {"mode": "interactive"}), "textbox"), "text": "San Francisco"})
    m.call("act", {"handle": m.handle(m.call("observe", {"mode": "interactive"}), r'Start search'), "action": "click"})
    time.sleep(4)
    full = m.call("observe", {"mode": "full"})
    m.p.kill()
    hit = LINE.search(full)
    return hit.group(0) if hit else "none"


def main():
    pin = os.environ.get("MANUK_CHROME_PIN")
    if not pin:
        sys.exit("refusing: MANUK_CHROME_PIN unset")
    dest = sys.argv[1]
    runs = int(sys.argv[2]) if len(sys.argv) > 2 else 3
    c = chrome()
    if c["chrome_ver"] != pin:
        sys.exit(f"refusing: chrome reports {c['chrome_ver']}, pin is {pin}")
    if not c.get("found"):
        sys.exit("refusing: no 'Search results for homes' landmark in Chrome")
    chrome_modal = bool(c["dialog_ancestors"] or c["aria_modal_self_or_ancestor"] or
                        (c["covers_30pct_viewport"] and c["controls_hit_inside_main"] > 0))
    cols = ["chrome_ver", "run", "chrome_role", "chrome_aria_modal", "chrome_dialog_ancestors", "chrome_covers_30pct",
            "chrome_controls_hit_inside", "chrome_modal", "manuk_overlay"]
    rows = []
    for k in range(1, runs + 1):
        rows.append({"chrome_ver": pin, "run": k, "chrome_role": c["role"], "chrome_aria_modal": c["aria_modal_self_or_ancestor"],
                     "chrome_dialog_ancestors": c["dialog_ancestors"], "chrome_covers_30pct": c["covers_30pct_viewport"],
                     "chrome_controls_hit_inside": c["controls_hit_inside_main"], "chrome_modal": chrome_modal,
                     "manuk_overlay": manuk_run()})
    with open(dest, "w") as f:
        f.write("\t".join(cols) + "\n")
        for r in rows:
            f.write("\t".join(str(r[k]) for k in cols) + "\n")
    print(open(dest).read())


if __name__ == "__main__":
    main()
