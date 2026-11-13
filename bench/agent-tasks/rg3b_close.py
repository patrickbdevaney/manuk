#!/usr/bin/env python3
"""RG.3.0.b.a.a.b: the step-0 difference. For each of the 7 round-3 'agent' tasks, navigate to the task's start URL under
manuk and Chrome (chrome_ver=$MANUK_CHROME_PIN), observe at t=0/3/8 s, and tabulate the Chrome `button "close"` control:
is it in the interactive observation (visible) and does extract(include_hidden=true) show it (present but hidden)?
  rg3b_close.py [--tasks a,b] [--mutate drop-row] [--out FILE]   exit 0 = GATE PASS (one row per task), 1 = FAIL
Writes docs/sprint/rg/RG3b-close.md. Run with `python3 -I`; needs MANUK_CHROME_PIN."""
import argparse, os, re, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import real_boot as rb
import rg3b_replay as rp

URL = {t: f"https://evals-{t.split('-')[0]}.vercel.app" for t in rp.TASKS}
CLOSE = re.compile(r'\[h\d+\] button "close"[^\n]*')
TIMES = (0, 3, 8)


def probe(backend, url):
    env, args = dict(os.environ), []
    if backend == "chrome":
        args = ["--backend", "chrome"]; env["MANUK_CHROME"] = os.path.expanduser("~/.local/chrome-cft/bin/google-chrome")
    m = rb.M(args, env)
    try:
        m.call("navigate", {"url": url}); t0 = time.time(); out = []
        for t in TIMES:
            time.sleep(max(0, t - (time.time() - t0)))
            o = m.call("observe", {"mode": "interactive"})
            out.append(("%ds" % t, CLOSE.findall(o)[:1], len(o.splitlines())))
        hid = m.call("extract", {"include_hidden": True})
        return out, len(re.findall(r'(?i)\bclose\b', hid))
    finally:
        m.p.kill()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--tasks", default=",".join(rp.TASKS)); ap.add_argument("--mutate", choices=["drop-row"])
    ap.add_argument("--out", default=os.path.join(rp.ROOT, "docs/sprint/rg/RG3b-close.md"))
    a = ap.parse_args()
    pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
    rows = []
    for t in a.tasks.split(","):
        mo, mh = probe("manuk", URL[t]); co, ch = probe("chrome", URL[t])
        f = lambda o: " / ".join(f"{s}: {'`'+c[0].split('] ',1)[1]+'`' if c else 'absent'}" for s, c, _ in o)
        rows.append(f"| {t} | {URL[t]} | {f(co)} | {f(mo)} | {ch} | {mh} |")
        print(rows[-1], flush=True)
    if a.mutate == "drop-row":
        rows = rows[:-1]
    md = [f"# RG.3.0.b.a.a.b: Chrome `button \"close\"` at step 0", "",
          f"chrome_ver={pin}. Fresh `observe(interactive)` at 0/3/8 s after `navigate` (`bench/agent-tasks/rg3b_close.py`). 'absent' = no `button \"close\"` line in the interactive observation (which lists visible controls); last two columns count `close` words in `extract(include_hidden=true)` (hidden or visible).", "",
          "| task | start url | chrome close (visible in interactive obs) | manuk close | chrome 'close' in include_hidden extract | manuk 'close' in include_hidden extract |", "|---|---|---|---|---|---|", *rows, ""]
    ok = len(rows) == len(a.tasks.split(","))
    if a.mutate is None:
        open(a.out, "w").write("\n".join(md))
    print("GATE", "PASS" if ok else "FAIL"); sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
