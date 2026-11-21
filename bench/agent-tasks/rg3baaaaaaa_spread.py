#!/usr/bin/env python3
"""RG.3.0.b.a.a.a.a.a.a.a.a: run-to-run spread of the RG.3.0.b.a.a.a.a.a.a.a ordinal match count.

Reads the three unmutated runs docs/sprint/rg/RG3baaaaaaa-match-runN.json (+ .md for resolved counts), checks each
carries chrome_ver == $MANUK_CHROME_PIN, and writes docs/sprint/rg/RG3baaaaaaa-spread.md with per-run counts and the
min and max of manuk/chrome match and resolved counts.
  rg3baaaaaaa_spread.py [--dir DIR]   exit 0 = GATE PASS, 1 = GATE FAIL (missing run, wrong or missing chrome_ver,
                                      counts not readable from the run's table)
Run with `python3 -I`; needs MANUK_CHROME_PIN."""
import argparse, json, os, re, sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
ROW = re.compile(r"^\| (\S+) \| (manuk|chrome) \| #(\d+) \| `[^`]*` \| `([^`]*)` \| (yes|no) \|$", re.M)


def read_run(d, n, pin, errs):
    jp = os.path.join(d, f"RG3baaaaaaa-match-run{n}.json")
    mp = os.path.join(d, f"RG3baaaaaaa-match-run{n}.md")
    if not os.path.exists(jp) or not os.path.exists(mp):
        errs.append(f"run{n}: missing {os.path.basename(jp)} or {os.path.basename(mp)}")
        return None
    j = json.load(open(jp))
    cv = j.get("chrome_ver")
    if cv != pin:
        errs.append(f"run{n}: chrome_ver={cv!r} != MANUK_CHROME_PIN={pin!r}")
    counts = j.get("counts", {})
    if not all(isinstance(counts.get(s), int) for s in ("manuk", "chrome")):
        errs.append(f"run{n}: match counts missing from json")
        return None
    rows = ROW.findall(open(mp).read())
    res = {"manuk": 0, "chrome": 0}
    tot = {"manuk": 0, "chrome": 0}
    match = {"manuk": 0, "chrome": 0}
    for _, side, _, live, m in rows:
        tot[side] += 1
        if live != "-":
            res[side] += 1
        match[side] += m == "yes"
    for s in ("manuk", "chrome"):
        if tot[s] != 12:
            errs.append(f"run{n}: {s} has {tot[s]} table rows, expected 12")
        if match[s] != counts[s]:
            errs.append(f"run{n}: {s} table shows {match[s]} matches, json says {counts[s]}")
    return {"chrome_ver": cv, "match": {s: counts[s] for s in ("manuk", "chrome")}, "resolved": res}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dir", default=os.path.join(ROOT, "docs/sprint/rg"))
    a = ap.parse_args()
    pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
    errs, runs = [], []
    for n in (1, 2, 3):
        r = read_run(a.dir, n, pin, errs)
        if r is not None:
            runs.append((n, r))
    if errs:
        print("GATE FAIL:"); [print("  " + e) for e in errs]
        sys.exit(1)
    md = ["## RG.3.0.b.a.a.a.a.a.a.a.a", "",
          f"chrome_ver={pin}. Three unmutated runs of `bench/agent-tasks/rg3baaaaaaa_match.py` (same chrome_ver, "
          "each run's output copied before the next run overwrote it). Resolved = ordinal steps whose handle resolved "
          "(live line not `-`), out of 12 per backend. Match = role and state/value agree.", "",
          "| run | manuk match | manuk resolved | chrome match | chrome resolved |", "|---|---|---|---|---|"]
    for n, r in runs:
        md.append(f"| run{n} | {r['match']['manuk']}/12 | {r['resolved']['manuk']}/12 | {r['match']['chrome']}/12 | {r['resolved']['chrome']}/12 |")
    mn = lambda k, s: min(r[k][s] for _, r in runs)
    mx = lambda k, s: max(r[k][s] for _, r in runs)
    md += ["", "| count | min manuk | max manuk | min chrome | max chrome |", "|---|---|---|---|---|",
           f"| match | {mn('match','manuk')}/12 | {mx('match','manuk')}/12 | {mn('match','chrome')}/12 | {mx('match','chrome')}/12 |",
           f"| resolved | {mn('resolved','manuk')}/12 | {mx('resolved','manuk')}/12 | {mn('resolved','chrome')}/12 | {mx('resolved','chrome')}/12 |",
           ""]
    out = os.path.join(a.dir, "RG3baaaaaaa-spread.md")
    open(out, "w").write("\n".join(md) + "\n")
    print("\n".join(md))
    print("GATE PASS: wrote", os.path.relpath(out, ROOT))


if __name__ == "__main__":
    main()
