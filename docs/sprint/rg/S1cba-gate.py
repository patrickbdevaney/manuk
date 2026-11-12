#!/usr/bin/env python3
"""RG.S1.c.b.a gate: does manuk expose as many restaurant cards as Chrome on the OpenDining home page?

  python3 -I docs/sprint/rg/S1cba-gate.py docs/sprint/rg/S1cba-homepage.tsv <chrome_ver>

GREEN (exit 0) only when, for every GATED surface, manuk_cards == chrome_cards on the pinned chrome_ver.
RED (exit 1) when any gated surface shows fewer cards than Chrome. Exit 2 when the file is for another chrome_ver
or has no gated rows. Gated surfaces: extract-text (the harness's manuk read, default format) and
extract-text-hidden (include_hidden=true). The other rows are reported, not gated.
"""
import csv, sys

GATED = {"extract-text", "extract-text-hidden"}


def main():
    path, pin = sys.argv[1], sys.argv[2]
    rows = list(csv.DictReader(open(path), delimiter="\t"))
    if any(r["chrome_ver"] != pin for r in rows):
        sys.exit(2)
    gated = [r for r in rows if r["surface"] in GATED]
    if not gated:
        sys.exit(2)
    red = [r for r in gated if int(r["manuk_cards"]) < int(r["chrome_cards"])]
    for r in rows:
        tag = "GATED" if r["surface"] in GATED else "info"
        short = int(r["manuk_cards"]) < int(r["chrome_cards"])
        verdict = "RED" if r in red else ("short (not gated)" if short else "ok")
        print(f"{tag}\t{r['surface']}\tchrome={r['chrome_cards']}\tmanuk={r['manuk_cards']}\t{verdict}")
    print("GATE " + ("RED" if red else "GREEN"))
    sys.exit(1 if red else 0)


if __name__ == "__main__":
    main()
