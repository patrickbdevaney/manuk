#!/usr/bin/env python3
"""RG.3.0.b.a.a.a.a.a.a.a.b.a.a.a.a.a.a.a.b: why does opendining's time dropdown start at '12:00 AM' on manuk but '7:00 AM'
on Chrome? Per backend, print (a) the clock/zone/locale a page can read: Date.now(), the local wall time, new
Date().getTimezoneOffset(), Intl resolved timeZone/locale and navigator.language, via a data: probe page that renders them
as text, and (b) the first time options after opendining-4 replay step #7 (the click on 'Select time').
exit 1 (red) iff the backends print identical clock/TZ/locale (same zone, offset, locale, language and local wall time
to the minute after subtracting the probe-time gap) yet different first options: an unexplained divergence. exit 0
otherwise (clock/TZ/locale differ -> they explain the options, or the options are equal).
  --mutate same-env   chrome's clock/TZ/locale replaced by manuk's, options untouched (deliberately broken -> must exit 1
                      while the options differ)
  --mutate same-opts  chrome's options replaced by manuk's (must exit 0)
Run with `python3 -I`; needs MANUK_CHROME_PIN."""
import json, os, re, sys, time, urllib.parse
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import real_boot as rb
import rg3b_replay as r, rg3baaaaaaa_match as m, rg3baaaaaa_ordinal as o

pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
mut = sys.argv[sys.argv.index("--mutate") + 1] if "--mutate" in sys.argv else None
JS = ("var d=new Date(),ro=Intl.DateTimeFormat().resolvedOptions();var v={tz:ro.timeZone,offset:d.getTimezoneOffset(),"
      "locale:ro.locale,lang:navigator.language,now:Date.now(),local:d.getFullYear()+'-'+(d.getMonth()+1)"
      "+'-'+d.getDate()+'T'+d.getHours()+':'+d.getMinutes(),tls:d.toLocaleTimeString(),str:d.toString()};"
      "document.getElementById('o').innerHTML=Object.keys(v).map(function(k){return '<p>CLOCK '+k+'='+v[k]+';</p>'}).join('')")
url = "data:text/html," + urllib.parse.quote("<body><p id=o></p><script>%s</script></body>" % JS)


def env_of(side):
    env, args = dict(os.environ), []
    if side == "chrome":
        args = ["--backend", "chrome"]; env["MANUK_CHROME"] = os.path.expanduser("~/.local/chrome-cft/bin/google-chrome")
    return args, env


def clock(side):
    args, env = env_of(side)
    mm = rb.M(args, env)
    try:
        t_host = time.time() * 1000
        mm.call("navigate", {"url": url}); time.sleep(1.5)
        t = mm.call("observe", {"mode": "full"})
        v = {}
        for k, x in re.findall(r"CLOCK (\w+)=([^;\n\"…]*);", t):
            v.setdefault(k, int(x) if re.fullmatch(r"-?\d+", x) else x.strip())
        v["host_ms_at_navigate"] = int(t_host)
        return v
    finally:
        mm.p.kill()


st = r.lg.steps(os.path.join(r.TR, "manuk_opendining-4_r1.jsonl"))[0]
seq = m.flatten_ref(st, 0)
r.resolve = o.resolve


def options(side):
    raw = []
    r.replay(side, [x[:3] for x in seq[:8]], 2, settle=2.0, raw=raw)
    obs = next((x[2] for x in reversed(raw) if x[0] == 7), None) or ""
    allo = re.findall(r'\boption "([^"]*)"', obs)
    ALL[side] = allo
    return allo[:3]


res, ALL = {}, {}
for s in ("manuk", "chrome"):
    res[s] = {"clock": clock(s), "options": options(s)}
    try: os.utime(os.path.join(r.ROOT, ".git/sprint-working"), None)
    except OSError: pass
if mut == "same-env":
    res["chrome"]["clock"] = dict(res["manuk"]["clock"])
if mut == "same-opts":
    res["chrome"]["options"] = list(res["manuk"]["options"])
K = ("tz", "offset", "locale", "lang")
for s, v in res.items():
    c = v["clock"]
    print(f"== {s} chrome_ver={pin}")
    print("  " + " ".join(f"{k}={c.get(k)!r}" for k in ("now", "local", "offset", "tz", "locale", "lang", "str", "tls")))
    print("  page clock - host clock at navigate: %+.0f s" % ((c.get("now", 0) - c["host_ms_at_navigate"]) / 1000))
    print("  first options after step #7:", v["options"])
    al = ALL.get(s, [])
    print(f"  options listed after step #7: {len(al)} ({al[0] if al else '-'} .. {al[-1] if al else '-'})")
a, b = res["manuk"]["clock"], res["chrome"]["clock"]
skew = lambda c: round((c.get("now", 0) - c["host_ms_at_navigate"]) / 60000)
same_env = bool(a.get("now")) and all(a.get(k) == b.get(k) for k in K) and skew(a) == skew(b)
same_opts = res["manuk"]["options"] == res["chrome"]["options"]
print(f"clock/TZ/locale identical: {same_env} | first options identical: {same_opts}")
if not a.get("now") or not b.get("now") or not res["manuk"]["options"] or not res["chrome"]["options"]:
    print("GATE RED (probe broken: no clock or no options read)"); sys.exit(1)
red = same_env and not same_opts
print("GATE", "RED (identical clock/TZ/locale, different first option: unexplained)" if red else "OK")
sys.exit(1 if red else 0)
