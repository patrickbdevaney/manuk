#!/usr/bin/env python3
"""RG.2.E1.a.a.a gate: manuk must not call the zilloft-2 results landmark an overlay unless pinned Chrome does too.

  python3 -I docs/sprint/rg/E1aaa-gate.py docs/sprint/rg/E1aaa-consent.tsv <chrome_ver>

GREEN (exit 0) when every run agrees: manuk_overlay is "none" iff chrome_modal is False.
RED (exit 1) when any run disagrees (the flag on a landmark Chrome does not treat as modal, or a missed modal).
Exit 2 when the file is for another chrome_ver or has no rows.
"""
import csv, sys


def main():
    path, pin = sys.argv[1], sys.argv[2]
    rows = list(csv.DictReader(open(path), delimiter="\t"))
    if not rows or any(r["chrome_ver"] != pin for r in rows):
        sys.exit(2)
    red = []
    for r in rows:
        chrome_modal = r["chrome_modal"] == "True"
        flagged = r["manuk_overlay"] != "none"
        ok = flagged == chrome_modal
        print(f"run {r['run']}\tchrome_modal={chrome_modal}\tmanuk={r['manuk_overlay']}\t{'agree' if ok else 'DISAGREE'}")
        if not ok:
            red.append(r["run"])
    print("GATE " + ("RED" if red else "GREEN"))
    sys.exit(1 if red else 0)


if __name__ == "__main__":
    main()
