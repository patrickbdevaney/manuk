#!/usr/bin/env python3
"""RG.0.a.a: why the evals clones' 'Version 2.0 launches soon' banner is in Chrome's tree but not manuk's.
Mechanism: the clone arms setTimeout(dismiss, 10000) on the banner. manuk's virtual clock runs ahead 60 s after `load`
(dom_bindings.rs __VCLOCK_HORIZON_MS), so the 10 s dismissal has fired by the first observe; Chrome (real time) dismisses
at 10 s wall. Network-free fixture, no site-specific code. Checks (exit 1 on any miss):
  manuk, 10 s dismiss timer  -> banner ABSENT at first observe   (inside the 60 s horizon)
  manuk, 90 s dismiss timer  -> banner PRESENT                   (beyond the horizon; discriminates the check)
  chrome (MANUK_CHROME_PIN), 10 s timer -> PRESENT at 2 s, ABSENT at 13 s
Run with `python3 -I`. Requires MANUK_CHROME_PIN.  --mutate: expect the 10 s manuk case PRESENT (must exit 1)."""
import os, sys, tempfile, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import real_boot as rb

def page(ms):
    d = tempfile.mkdtemp(prefix="rg0aa-")
    p = os.path.join(d, "banner.html")
    open(p, "w").write(f"<!doctype html><body><div id=b role=alert>Version 2.0 launches soon</div><button>Create</button>"
                       f"<script>setTimeout(function(){{document.getElementById('b').remove()}},{ms})</script>")
    return "file://" + p

def observe(backend, url, waits):
    env, args = dict(os.environ), []
    if backend == "chrome":
        args = ["--backend", "chrome"]; env["MANUK_CHROME"] = os.path.expanduser("~/.local/chrome-cft/bin/google-chrome")
    m = rb.M(args, env)
    try:
        m.call("navigate", {"url": url}); t = time.time(); out = []
        for w in waits:
            time.sleep(max(0, w - (time.time() - t)))
            out.append("Version 2.0" in m.call("extract", {"include_hidden": True}))
        return out
    finally:
        m.p.kill()

def main():
    mut = "--mutate" in sys.argv
    pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("MANUK_CHROME_PIN unset")
    r = {"manuk10": observe("manuk", page(10000), [0])[0], "manuk90": observe("manuk", page(90000), [0])[0]}
    c = observe("chrome", page(10000), [2, 13]); r["chrome2"], r["chrome13"] = c
    print(f"chrome_ver={pin} banner_present: {r}")
    want = {"manuk10": mut, "manuk90": True, "chrome2": True, "chrome13": False}
    ok = r == want
    print("GATE PASS" if ok else "GATE FAIL"); sys.exit(0 if ok else 1)

main()
