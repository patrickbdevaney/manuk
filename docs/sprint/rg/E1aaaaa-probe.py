#!/usr/bin/env python3
"""RG.2.E1.a.a.a.a.a: the zilloft-2 "Map view of search results" box, manuk vs pinned Chrome.

  MANUK_CHROME_PIN=145.0.7632.116 python3 -I docs/sprint/rg/E1aaaaa-probe.py [out.tsv]

Chrome: E1aaaaa-chrome.js. Manuk: target/release/manuk-mcp over JSON-RPC, same flow; the box is read by a
scripted handoff (MANUK_HANDOFF_SCRIPT) that writes the region's getBoundingClientRect into the DOM, then
observe. Exit 1 if the widths differ by more than 2px, 2 if the pin is wrong/unset. Prints one TSV row.
"""
import json, os, re, subprocess, sys, tempfile, time
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(HERE)))
EXE = os.path.join(ROOT, "target/release/manuk-mcp")
JS = ("(function(){var e=[].slice.call(document.querySelectorAll('[aria-label]')).filter(function(e){return /Map view/.test(e.getAttribute('aria-label'))})[0];"
      "var r=e?e.getBoundingClientRect():null;var d=document.createElement('h1');"
      "d.textContent='MKBOX '+(r?[r.left,r.top,r.width,r.height].map(Math.round).join(','):'none');document.body.insertBefore(d,document.body.firstChild);return 1})()")

def main():
    pin = os.environ.get("MANUK_CHROME_PIN")
    if not pin:
        sys.exit(2)
    c = json.loads(subprocess.run(["node", os.path.join(HERE, "E1aaaaa-chrome.js")], capture_output=True, text=True, timeout=300, check=True).stdout)
    if c["chrome_ver"] != pin or not c["found"]:
        print(f"refusing: chrome {c['chrome_ver']} pin {pin} found {c['found']}", file=sys.stderr); sys.exit(2)
    with tempfile.NamedTemporaryFile("w", suffix=".js", delete=False) as f:
        f.write(JS)
    p = subprocess.Popen([EXE], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True,
                         env=dict(os.environ, MANUK_HANDOFF_SCRIPT=f.name))
    n = [0]
    def rpc(m, pa):
        n[0] += 1
        p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": n[0], "method": m, "params": pa}) + "\n"); p.stdin.flush()
        while True:
            j = json.loads(p.stdout.readline())
            if j.get("id") == n[0]:
                return j
    def call(nm, a):
        return "\n".join(x.get("text", "") for x in rpc("tools/call", {"name": nm, "arguments": a})["result"]["content"])
    def handle(t, pat):
        for l in t.split("\n"):
            if re.search(pat, l) and "[h" in l:
                return l[l.index("[h") + 1:l.index("]", l.index("[h"))]
        sys.exit(f"no handle {pat}")
    rpc("initialize", {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "E1aaaaa", "version": "0"}})
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n"); p.stdin.flush()
    call("navigate", {"url": "https://evals-zilloft.vercel.app"})
    call("type", {"handle": handle(call("observe", {"mode": "interactive"}), "textbox"), "text": "San Francisco"})
    call("act", {"handle": handle(call("observe", {"mode": "interactive"}), "Start search"), "action": "click"})
    time.sleep(4)
    h = call("handoff", {"reason": "probe", "timeout_ms": 5000}) if True else ""
    full = call("observe", {"mode": "viewport"})
    p.kill(); os.unlink(f.name)
    if os.environ.get("DBG"): print(h[:300], full[:700])
    m = re.search(r"MKBOX ([\d,\-none]+)", full + h)
    mk = [int(x) for x in m.group(1).split(",")] if m and m.group(1) != "none" else None
    row = [pin, "chrome=" + ",".join(map(str, c["box"])), "manuk=" + (",".join(map(str, mk)) if mk else "none")]
    out = "\t".join(row)
    print(out)
    if len(sys.argv) > 1:
        open(sys.argv[1], "w").write("chrome_ver\tchrome_box\tmanuk_box\n" + out + "\n")
    sys.exit(0 if mk and abs(mk[2] - c["box"][2]) <= 2 else 1)
main()
