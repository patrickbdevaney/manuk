#!/usr/bin/env python3
"""RG.3.0.b.a.b: how often does a click right after navigate land before the page handlers are attached?
Per trial: fresh backend, navigate to the opendining home, sleep WAIT, observe, click the 'Select time' combobox,
classify the act result: opened (new option nodes), no_change, no_handle (combobox not yet in the observation), other.
  rg3ba_hydrate.py [--trials 20] [--waits 0.3,1,2] [--out FILE]      measure both backends, write TSV
  rg3ba_hydrate.py --from FILE [--mutate worse]                       re-judge a TSV; exit 0 = EQUAL (manuk within 10 points
                                                                      of Chrome, pooled over waits), 1 = manuk WORSE
miss = no_change + no_handle + other. Run with `python3 -I`; needs MANUK_CHROME_PIN.
"""
import argparse, os, re, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import real_boot as rb

URL = "https://evals-opendining.vercel.app"
HB = os.path.join(rb.os.path.dirname(rb.EXE), "../../.git/sprint-working")


def trial(backend, wait):
    env, args = dict(os.environ), []
    if backend == "chrome":
        args = ["--backend", "chrome"]; env["MANUK_CHROME"] = os.path.expanduser("~/.local/chrome-cft/bin/google-chrome")
    m = rb.M(args, env)
    try:
        m.call("navigate", {"url": URL}); time.sleep(wait)
        o = m.call("observe", {"mode": "interactive"})
        h = re.search(r'\[(h\d+)\] combobox "Select time"', o)
        if not h:
            return "no_handle", ""
        r = m.call("act", {"handle": h.group(1)})
        out = "opened" if 'option "' in r else "no_change" if "no change" in r else "other"
        return out, r[:160].replace("\n", " ")
    finally:
        m.p.kill()


def verdict(rows):
    rate = {}
    for be in ("manuk", "chrome"):
        rs = [r for r in rows if r[0] == be]
        rate[be] = 100.0 * sum(r[3] != "opened" for r in rs) / max(1, len(rs))
    return rate, rate["manuk"] - rate["chrome"] > 10


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--trials", type=int, default=20); ap.add_argument("--waits", default="0.3,1,2")
    ap.add_argument("--out"); ap.add_argument("--from", dest="src"); ap.add_argument("--mutate")
    a = ap.parse_args()
    pin = os.environ.get("MANUK_CHROME_PIN")
    if not pin:
        sys.exit("refusing: MANUK_CHROME_PIN unset")
    if a.src:
        rows = [l.rstrip("\n").split("\t") for l in open(a.src) if not l.startswith("backend")]
    else:
        rows = []
        for w in a.waits.split(","):
            for be in ("manuk", "chrome"):
                for t in range(1, a.trials + 1):
                    out, r = trial(be, float(w)); os.utime(HB, None)
                    rows.append([be, w, str(t), out, pin, r]); print("\t".join(rows[-1]), flush=True)
        if a.out:
            with open(a.out, "w") as f:
                f.write("backend\twait_s\ttrial\toutcome\tchrome_ver\tact_result\n")
                for r in rows: f.write("\t".join(r) + "\n")
    if a.mutate == "worse":
        rows = [[r[0], r[1], r[2], "no_change" if r[0] == "manuk" else r[3], *r[4:]] for r in rows]
    for w in sorted({r[1] for r in rows}, key=float):
        s = {be: sum(1 for r in rows if r[0] == be and r[1] == w and r[3] != "opened") for be in ("manuk", "chrome")}
        n = {be: sum(1 for r in rows if r[0] == be and r[1] == w) for be in ("manuk", "chrome")}
        print(f"wait {w}s: manuk miss {s['manuk']}/{n['manuk']}  chrome miss {s['chrome']}/{n['chrome']}")
    rate, worse = verdict(rows)
    print(f"pooled miss rate: manuk {rate['manuk']:.1f}%  chrome {rate['chrome']:.1f}%  -> {'WORSE' if worse else 'EQUAL'}")
    sys.exit(1 if worse else 0)


main()
