#!/usr/bin/env python3
"""RG.2.0.a: replay the fly-unified 'Find flights' submit under manuk and under Chrome (chrome_ver=$MANUK_CHROME_PIN)
and name the first differing step (request/navigation/DOM). Steps are resolved by role+name per backend (handles differ).
  rg2a_replay.py [--wait 4] [--mutate identical] [--out FILE]   exit 0 = GATE PASS, 1 = GATE FAIL
Writes docs/sprint/rg/LEDGER-2.md section 'RG.2.0.a'. Run with `python3 -I`; requires MANUK_CHROME_PIN.
"""
import argparse, os, re, sys, time, difflib
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import real_boot as rb
import rg0a_domdiff as dd

ROOT = dd.ROOT
URL = "https://evals-fly-unified.vercel.app"


def h(obs, role, name):
    m = re.search(r'\[(h\d+)\] %s "?%s' % (role, re.escape(name)), obs)
    return m.group(1) if m else None


def run(backend, wait):
    env, args = dict(os.environ), []
    if backend == "chrome":
        args = ["--backend", "chrome"]; env["MANUK_CHROME"] = os.path.expanduser("~/.local/chrome-cft/bin/google-chrome")
    m = rb.M(args, env); log = []
    try:
        m.call("navigate", {"url": URL}); time.sleep(wait)
        o = m.call("observe", {"mode": "interactive"})
        o = m.call("observe", {"mode": "interactive"})
        x = h(o, "button", "Find flights")
        r = m.call("act", {"handle": x, "action": "click"}); log.append(("click Find flights", r[:700])); time.sleep(wait)
        full = m.call("observe", {"mode": "content"})
        return log, full
    finally:
        m.p.kill()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--wait", type=float, default=4); ap.add_argument("--mutate")
    ap.add_argument("--out", default=os.path.join(ROOT, "docs/sprint/rg/rg2a-replay.md"))
    a = ap.parse_args()
    pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
    ml, mf = run("manuk", a.wait)
    cl, cf = (ml, mf) if a.mutate == "identical" else run("chrome", a.wait)
    mu = re.findall(r"https://evals-fly-unified\S*", mf); cu = re.findall(r"https://evals-fly-unified\S*", cf)
    mpath = (mu[0] if mu else "?"); cpath = (cu[0] if cu else "?")
    same = ml == cl and dd.norm(mf) == dd.norm(cf)
    md = [f"chrome_ver={pin}", "", "### manuk", *[f"- {k}: {v}" for k, v in ml], f"- page after: {mpath}", "```", *dd.norm(mf)[:25], "```",
          "### chrome", *[f"- {k}: {v}" for k, v in cl], f"- page after: {cpath}", "```", *dd.norm(cf)[:25], "```"]
    open(a.out, "w").write("\n".join(md))
    print("IDENTICAL" if same else "DIFFERENT"); print("manuk:", ml[-1][1][:400]); print("chrome:", cl[-1][1][:400])
    print("manuk page:", mpath); print("chrome page:", cpath)
    sys.exit(1 if same else 0)


if __name__ == "__main__":
    main()
