#!/usr/bin/env python3
"""RG.3.0.b.a.a.a.a.a.a.a.b.a.a.a.a.a.a.a.b (part 2): opendining's time list is a static 48-entry array
("12:00 AM".."11:30 PM", see the site's page chunk), so the first *listed* option depends only on which options each
backend's observation keeps once MUI's menu scrolls to the selected value. This probe isolates that on a data: page: a
400px-high overflow:auto listbox of 48 x 36px options, in three modes: no scroll, scrollTop = 14*36 (= 504, the index
of '7:00 AM'), and focus() on option 26 (MUI's autoFocusItem path). Per backend it prints the listbox scrollTop the
page reads and the first/last/count of listed options.
exit 1 (red) iff both backends list the same first option in every mode (then scrolling does not explain the
opendining difference); exit 0 when they differ in at least one scrolled mode.
  --mutate same   chrome's listing replaced by manuk's (deliberately broken input -> must exit 1)
Run with `python3 -I`; needs MANUK_CHROME_PIN."""
import os, re, sys, time, urllib.parse
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import real_boot as rb

pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
mut = sys.argv[sys.argv.index("--mutate") + 1] if "--mutate" in sys.argv else None
JS = {"none": "", "scrolltop": "document.getElementById('l').scrollTop=14*36",
      "focus": "document.getElementById('i26').focus()"}


def page(mode):
    items = "".join(f'<li role=option tabindex=-1 id=i{k} style="height:36px">opt {k}</li>' for k in range(48))
    h = (f'<body><p id=s></p><ul id=l role=listbox style="height:400px;overflow:auto;margin:0;padding:0;'
         f'list-style:none">{items}</ul><script>{JS[mode]};setTimeout(function(){{document.getElementById("s")'
         f'.textContent="ST "+document.getElementById("l").scrollTop}},50)</script></body>')
    return "data:text/html," + urllib.parse.quote(h)


def probe(side, mode):
    env, args = dict(os.environ), []
    if side == "chrome":
        args = ["--backend", "chrome"]; env["MANUK_CHROME"] = os.path.expanduser("~/.local/chrome-cft/bin/google-chrome")
    m = rb.M(args, env)
    try:
        m.call("navigate", {"url": page(mode)}); time.sleep(1.5)
        full = m.call("observe", {"mode": "full"})
        inter = m.call("observe", {"mode": "interactive"})
    finally:
        m.p.kill()
    st = re.findall(r"ST (\d+)", full)
    o = re.findall(r'option "opt (\d+)"', inter)
    return {"scrollTop": int(st[0]) if st else None, "n": len(o), "first": o[0] if o else None, "last": o[-1] if o else None}


res = {s: {md: probe(s, md) for md in JS} for s in ("manuk", "chrome")}
if mut == "same":
    res["chrome"] = {md: dict(v) for md, v in res["manuk"].items()}
for s, v in res.items():
    for md, x in v.items():
        print(f"{s:6} {md:9} scrollTop={x['scrollTop']} listed={x['n']} first=opt {x['first']} last=opt {x['last']}  chrome_ver={pin}")
if any(x["n"] == 0 for v in res.values() for x in v.values()):
    print("GATE RED (probe broken: no options listed)"); sys.exit(1)
diff = [md for md in JS if res["manuk"][md]["first"] != res["chrome"][md]["first"]]
print("modes where the first listed option differs:", diff or "none")
print("GATE", "OK (scroll position explains a first-option difference)" if diff else "RED (same first option in every mode)")
sys.exit(0 if diff else 1)
