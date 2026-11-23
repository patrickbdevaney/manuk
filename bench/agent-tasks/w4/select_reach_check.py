#!/usr/bin/env python3 -I
"""W4.P0.f.a.a.a.a.a.a.a.a: read select_reach_probe.py TSV output and go red unless it says what --expect requires.

  select_reach_check.py FILE.tsv --expect reachable --target "7:00 PM" [--runs 3]
  select_reach_check.py FILE.tsv --expect missing

reachable : exactly --runs rows, every row has in_dom True and reachable True, target matches, chrome_ver is one value.
missing   : every row has in_dom False and reachable False (a probe that finds a missing option reachable is broken).
Exit 0 = the file meets the expectation; exit 1 = red, with the offending rows on stderr.
"""
import csv, sys


def arg(name, default):
    return sys.argv[sys.argv.index(name) + 1] if name in sys.argv else default


def main():
    path = sys.argv[1]
    expect = arg("--expect", "reachable")
    target = arg("--target", "7:00 PM")
    want = int(arg("--runs", "3"))
    with open(path) as f:
        rows = list(csv.DictReader(f, delimiter="\t"))
    bad = []
    if expect == "reachable":
        if len(rows) != want:
            bad.append(f"expected {want} rows, got {len(rows)}")
        for r in rows:
            if r["target"] != target or r["in_dom"] != "True" or r["reachable"] != "True":
                bad.append(f"run {r['run']}: target={r['target']} in_dom={r['in_dom']} reachable={r['reachable']}")
    elif expect == "missing":
        if not rows:
            bad.append("no rows")
        for r in rows:
            if r["in_dom"] != "False" or r["reachable"] != "False":
                bad.append(f"run {r['run']}: in_dom={r['in_dom']} reachable={r['reachable']} for target {r['target']!r}")
    else:
        bad.append(f"unknown --expect {expect!r}")
    if len({r["chrome_ver"] for r in rows}) > 1:
        bad.append("mixed chrome_ver in one file")
    if bad:
        print("RED", *bad, sep="\n  ", file=sys.stderr)
        return 1
    print(f"OK {expect}: {len(rows)} rows, target={rows[0]['target']!r}, chrome_ver={rows[0]['chrome_ver']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
