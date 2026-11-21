#!/usr/bin/env python3
"""RG.3.0.b.a.a.a.a.a.a.a.b.a.a.a.a.a.a.a.b.b: one observation contract for content scrolled out of a scroll container.
Contract: content a scroll container has scrolled out of its view (above OR below) stays listed and actionable on both
backends -- it is reachable by scrolling (a click scrolls it in), so it is not hidden text. The shield's 'offscreen'
rule judges a run where it sits with every scroll container unscrolled (the in-process shield's fragments; the Chrome
backend now adds back each container's scrollRects offset), so only content no scroll can reveal is withheld.
Probe: the same data: listbox as rg3baaaaaaaaaaaaaaab_scroll.py (400px overflow:auto, 48 x 36px options), modes none,
scrollTop=504, focus() on option 26. Per backend prints scrollTop and listed count/first/last.
exit 0 iff in every mode both backends list the same count, first and last option (and the probe listed something);
exit 1 otherwise. Red with the change reverted: MANUK_CDP_MUTATE=scrolled-offscreen (chrome then drops opts above).
  --mutate drop-above   chrome's scrolled modes replaced by 'first = opt 13' (deliberately broken input -> exit 1)
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
if mut == "drop-above":
    for md in ("scrolltop", "focus"):
        res["chrome"][md] = dict(res["chrome"][md], first="13", n=35)
for s, v in res.items():
    for md, x in v.items():
        print(f"{s:6} {md:9} scrollTop={x['scrollTop']} listed={x['n']} first=opt {x['first']} last=opt {x['last']}  chrome_ver={pin}")
if any(x["n"] == 0 for v in res.values() for x in v.values()):
    print("GATE RED (probe broken: no options listed)"); sys.exit(1)
if res["manuk"]["scrolltop"]["scrollTop"] != 504 or res["chrome"]["scrolltop"]["scrollTop"] != 504:
    print("GATE RED (probe broken: scrolltop mode did not scroll)"); sys.exit(1)
diff = [md for md in JS if any(res["manuk"][md][k] != res["chrome"][md][k] for k in ("n", "first", "last"))]
print("modes where the listing differs:", diff or "none")
print("GATE", f"RED (listing differs in {diff})" if diff else "OK (same count/first/last in every mode)")
sys.exit(1 if diff else 0)
