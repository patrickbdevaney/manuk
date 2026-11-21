#!/usr/bin/env python3
"""RG.3.0.b.a.a.a.a.a.a.a.b.a.a.a.a.a: print, per backend, the observation lines for calendar days built like the opendining date
dialog (MUI shape: <div role=grid aria-label><div role=row><button role=gridcell tabindex=-1>N</button>...). Also prints a plain
<td onclick> control. Exit 0 = both backends give the day the SAME role; 1 = they differ (the divergence).
  --mutate same   the chrome lines are replaced by the manuk lines (deliberately-broken input; must exit 0, i.e. flips the gate)
Run with `python3 -I`; needs MANUK_CHROME_PIN."""
import os, sys, re, urllib.parse
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import real_boot as rb
pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
mut = "--mutate" in sys.argv
HTML = ('<dialog open aria-label="Select date"><div role=grid aria-label="November 2024"><div role=row>'
        '<button role=gridcell tabindex=-1 id=d1 onclick="1">1</button><button role=gridcell tabindex=0 id=d2 onclick="1">2</button></div></div>'
        '<table role=grid aria-label="T"><tr><td role=gridcell tabindex=0 onclick="1">7</td></tr></table></dialog>')
url = "data:text/html," + urllib.parse.quote(HTML)
out = {}
for side in ("manuk", "chrome"):
    env, args = dict(os.environ), []
    if side == "chrome": args = ["--backend", "chrome"]; env["MANUK_CHROME"] = os.path.expanduser("~/.local/chrome-cft/bin/google-chrome")
    m = rb.M(args, env)
    try:
        import time; m.call("navigate", {"url": url}); time.sleep(1.5)
        obs = m.call("observe", {"mode": "full"})
        out[side] = [l for l in obs.splitlines() if re.search(r'^\s*(\[h\d+\] |- )?(\w+ "[127]"|grid|table|row|gridcell|cell)', l)]
    finally: m.p.kill()
if mut: out["chrome"] = list(out["manuk"])
for s, ls in out.items():
    print("==", s); [print("  ", l) for l in ls]
role = lambda ls: [re.sub(r'^\s*\[h\d+\] (\w+) "(\d)".*', r'\1 \2', l) for l in ls if re.match(r'\s*\[h\d+\] \w+ "\d"', l)]
same = role(out["manuk"]) == role(out["chrome"]) and len(role(out["manuk"])) == 3
print("roles manuk:", role(out["manuk"]), "chrome:", role(out["chrome"]))
print("GATE", "SAME" if same else "DIFFER", f"chrome_ver={pin}")
sys.exit(0 if same else 1)
