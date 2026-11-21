#!/usr/bin/env python3
"""RG.3.0.b.a.a.a.a.a.a.a.b.a.a.a.a.a.a: per backend, print (a) the matchMedia values a page can read (pointer/hover/any-pointer/
any-hover, max-width 600/900/1200, min-width 600, prefers-reduced-motion) via a data: probe page that renders them as text, and
(b) the opendining-4 date picker after replay step #6: dialog name, Cancel/OK presence.
Exit 0 = DIVERGENCE EXPLAINED-FREE state is NOT reached; semantics: exit 1 (red) iff media values are identical yet the picker
variants differ (the gate's red condition: unexplained). exit 0 otherwise (media differ -> the media query explains the variant, or
variants equal).
  same-picker   chrome's picker replaced by manuk's (variants equal -> must exit 0)
  diff-picker   chrome's picker gets a Cancel/OK modal while media stay identical (deliberately broken input -> must exit 1)
Run with `python3 -I`; needs MANUK_CHROME_PIN."""
import os, sys, re, time, urllib.parse
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import real_boot as rb
import rg3b_replay as r, rg3baaaaaaa_match as m, rg3baaaaaa_ordinal as o
pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
mut = next((a for a in sys.argv[1:] if a in ("same-picker", "diff-picker")), None)
Q = ["(pointer: fine)", "(pointer: coarse)", "(pointer: none)", "(hover: hover)", "(hover: none)", "(any-pointer: fine)",
     "(any-pointer: coarse)", "(any-hover: hover)", "(max-width: 600px)", "(max-width: 900px)", "(max-width: 1200px)",
     "(min-width: 600px)", "(prefers-reduced-motion: reduce)"]
HTML = ("<body><p id=o></p><script>var q=%s;var s=q.map(function(x){return x+'='+matchMedia(x).matches}).join(' ; ');"
        "s+=' ; innerWidth='+innerWidth+' ; devicePixelRatio='+devicePixelRatio+' ; maxTouchPoints='+navigator.maxTouchPoints+' ; ontouchstart='+('ontouchstart' in window);"
        "document.getElementById('o').innerHTML=s.split(' ; ').map(function(x){return '<p>MEDIA '+x+'</p>'}).join('')</script></body>") % repr(Q)
url = "data:text/html," + urllib.parse.quote(HTML)
def media(side):
    env, args = dict(os.environ), []
    if side == "chrome": args = ["--backend", "chrome"]; env["MANUK_CHROME"] = os.path.expanduser("~/.local/chrome-cft/bin/google-chrome")
    mm = rb.M(args, env)
    try:
        mm.call("navigate", {"url": url}); time.sleep(1.5)
        t = mm.call("observe", {"mode": "full"})
        return " ; ".join(re.findall(r"(\([\w-]+: \w+\)=\w+|\b(?:innerWidth|devicePixelRatio|maxTouchPoints|ontouchstart)=\w+)", t)[-(len(Q) + 4):]) or "(none)"
    finally: mm.p.kill()
st = r.lg.steps(os.path.join(r.TR, "manuk_opendining-4_r1.jsonl"))[0]
seq = m.flatten_ref(st, 0)
r.resolve = o.resolve
def picker(side):
    raw = []; r.replay(side, [x[:3] for x in seq[:7]], 2, settle=2.0, raw=raw)
    obs = next((x[2] for x in reversed(raw) if x[0] == 6), None) or ""
    dlg = re.findall(r'(?:dialog|alertdialog)\s+"([^"]*)"', obs)
    btn = lambda n: bool(re.search(r'\bbutton "%s"' % n, obs))
    return dict(dialog=dlg, cancel=btn("Cancel"), ok=btn("OK"), month=bool(re.search(r'"November 2024"', obs)))
res = {s: dict(media=media(s), picker=picker(s)) for s in ("manuk", "chrome")}
if mut == "same-picker": res["chrome"]["picker"] = dict(res["manuk"]["picker"])
if mut == "diff-picker": res["chrome"]["media"] = res["manuk"]["media"]; res["chrome"]["picker"] = dict(res["manuk"]["picker"], cancel=True, ok=True, dialog=["Select date"])
for s, v in res.items():
    print("==", s); print("  media :", v["media"]); print("  picker:", v["picker"])
same_media = res["manuk"]["media"] == res["chrome"]["media"]
same_pick = res["manuk"]["picker"] == res["chrome"]["picker"]
red = same_media and not same_pick
print("media identical:", same_media, "| picker identical:", same_pick)
print("GATE", "RED (identical media, different picker: unexplained)" if red else "OK", f"chrome_ver={pin}")
sys.exit(1 if red else 0)
