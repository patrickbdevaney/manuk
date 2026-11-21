#!/usr/bin/env python3
"""RG.3.0.b.a.a.a.a.a.a.a.b.a.a.a.a.a.a.a.c: is Chrome's ordinal #13 miss (the 11/13 chrome match) a replay regression, or
the recorded transcript's collapsed state no longer holding once step #7 opens the time dropdown?

Hypothesis under test: the transcript was recorded when the click on 'Select time' (replay step #7 = 0-based index 7 of the
flattened replay) did nothing, so the nameless combobox ordinal #13 read `[collapsed]`; on Chrome the same click now opens
the dropdown, so the same ordinal resolves to the expanded combobox.

Checks (each failure makes the gate red):
  1. every run in DIR has chrome_ver == $MANUK_CHROME_PIN;
  2. in every run the chrome opendining-4 #13 LIVE line is `[expanded]` (the explanation's prediction);
  3. the transcript's #13 reference line is `[collapsed]` (the state the replay was recorded against);
  4. the transcript's own click on 'Select time' (raw action act h7 click) reports NO EFFECT (the dropdown never opened);
  5. in every run chrome's step #7 resolved, was not occluded, and listed a time option (the dropdown opened).
Usage (run with `python3 -I`; needs MANUK_CHROME_PIN):
  rg3baaaaaaaaaaaaaaaac_verdict.py [--dir DIR] [--transcript PATH] [--mutate collapsed]
  exit 0 = GATE PASS (explanation holds); exit 1 = GATE FAIL.
  --mutate collapsed: the in-memory live #13 chrome line is replaced by `[collapsed]` (no file is touched); the gate must
  exit 1 on it. Pre-registered MIN_CHROME/MIN_MANUK in rg3baaaaaaa_spread.py are NOT used or edited here."""
import argparse, json, os, re, sys
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import rg0_ledger as lg

ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
RG = os.path.join(ROOT, "docs/sprint/rg/RG3baaaaaaaaaaaaaa")
TR = os.path.expanduser("~/.cache/manuk-rgb2/transcripts/manuk_opendining-4_r1.jsonl")
ROW = re.compile(r"^\| opendining-4 \| (manuk|chrome) \| #(\d+) \| `([^`]*)` \| `([^`]*)` \| (yes|no) \|$", re.M)
STEP = 7
ORD13 = 13
TIME_OPT = re.compile(r'option "\d{1,2}:\d{2} [AP]M"')


def chrome13(text):
    for side, n, tr, live, m in ROW.findall(text):
        if side == "chrome" and int(n) == ORD13:
            return tr, live
    return None, None


def check(d, pin, transcript, mutate, errs, notes):
    for n in (1, 2, 3):
        jp = os.path.join(d, f"RG3baaaaaaa-match-run{n}.json")
        mp = os.path.join(d, f"RG3baaaaaaa-match-run{n}.md")
        sp = os.path.join(d, f"step7-run{n}.json")
        if not all(os.path.exists(p) for p in (jp, mp, sp)):
            errs.append(f"run{n}: missing json/md/step7 file in {d}")
            continue
        cv = json.load(open(jp)).get("chrome_ver")
        if cv != pin:
            errs.append(f"run{n}: chrome_ver={cv!r} != MANUK_CHROME_PIN={pin!r}")
        tr, live = chrome13(open(mp).read())
        if live is None:
            errs.append(f"run{n}: no chrome #13 row in table")
            continue
        if mutate == "collapsed":
            live = "combobox [collapsed]"
        if "[expanded]" not in live:
            errs.append(f"run{n}: chrome #13 live line is {live!r}, not [expanded]")
        if "[collapsed]" not in (tr or ""):
            errs.append(f"run{n}: chrome #13 transcript line is {tr!r}, not [collapsed]")
        s7 = json.load(open(sp)).get("chrome", {})
        if not (s7.get("resolved") and not s7.get("occluded") and TIME_OPT.search(s7.get("result", ""))):
            errs.append(f"run{n}: chrome step #7 not a resolved, unoccluded dropdown open: {s7.get('result', '')[:120]!r}")
        notes.append(f"run{n}: chrome #13 transcript {tr} | live {live}")
    st, _ = lg.steps(transcript)
    dead = [r for r in st if r[1] == "act" and json.loads(r[2]).get("handle") == "h7"
            and json.loads(r[2]).get("action") == "click" and "NO EFFECT" in r[3]]
    if not dead:
        errs.append("transcript: no recorded click on h7 ('Select time') reports NO EFFECT")
    else:
        notes.append(f"transcript step {dead[0][0]}: act h7 click -> {dead[0][3][:90]!r}")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dir", default=RG)
    ap.add_argument("--transcript", default=TR)
    ap.add_argument("--mutate", choices=["collapsed"])
    a = ap.parse_args()
    pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
    errs, notes = [], []
    check(a.dir, pin, a.transcript, a.mutate, errs, notes)
    print("\n".join("  " + x for x in notes))
    if errs:
        print("GATE FAIL: the explanation does not hold"); [print("  " + e) for e in errs]
        sys.exit(1)
    print(f"GATE PASS: chrome #13 is [expanded] in all 3 runs at chrome_ver={pin}; transcript recorded [collapsed] "
          "and its Select-time click did nothing")


if __name__ == "__main__":
    main()
