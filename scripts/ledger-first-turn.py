#!/usr/bin/env python3
"""ledger-first-turn — first-turn prompt size, full vs pack, over LIVE stream-json ledger rows (node W6.L1.a.a.a).

  ledger-first-turn.py [ledger] [--since 'YYYY-MM-DD HH:MM:SS']

A "live" row is a LEDGER.tsv row whose ts is at or after --since (default: the W6.L1.a.a commit that switched the
loop to `claude -p --output-format stream-json`, 2026-11-20 12:22:23 local). Only those rows can carry
first_turn_input, so the claim is checked on them alone; older json-era rows are not compared.

Prints, per prompt mode, n and mean first_turn_input (turn-1 prompt tokens: input + cache_creation + cache_read).
chrome_ver is read from each live row's own chrome_ver cell (written by sprint-ledger.py, node W6.L1.a.a.a.b), never
from the current environment. Every distinct pin among the live rows is printed; mixed pins are flagged MIXED.

Exit codes:
  0  PASS     >=3 full and >=3 pack live rows, all live rows carry first_turn_input, pack mean < full mean
  1  FAIL     a live row lacks first_turn_input (falsifier), OR a live row has an empty chrome_ver cell
             (falsifier), OR pack mean is not lower than full mean
  2  MISSING  fewer than 3 live rows in a mode (nothing is claimed)
"""
import csv
import os
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
N_MIN = 3
DEFAULT_SINCE = "2026-11-20 12:22:23"


def main(a: list[str]) -> int:
    since = DEFAULT_SINCE
    if "--since" in a:
        i = a.index("--since")
        since = a[i + 1]
        del a[i:i + 2]
    ledger = Path(a[0]) if a else ROOT / "docs/sprint/LEDGER.tsv"
    print(f"since={since}")
    if not ledger.exists():
        print(f"MISSING: {ledger} does not exist")
        return 2
    rows = [r for r in csv.DictReader(ledger.open(), delimiter="\t") if (r.get("ts") or "") >= since]
    bare = [f"{r.get('ts')} {r.get('node')}" for r in rows if not (r.get("first_turn_input") or "").isdigit()]
    if bare:
        print(f"FAIL: {len(bare)} live row(s) lack first_turn_input (falsifier):")
        for b in bare:
            print(f"  {b}")
        return 1
    nopin = [f"{r.get('ts')} {r.get('node')}" for r in rows if not (r.get("chrome_ver") or "").strip()]
    if nopin:
        print(f"FAIL: {len(nopin)} live row(s) have an empty chrome_ver cell (falsifier):")
        for b in nopin:
            print(f"  {b}")
        return 1
    pins = sorted({r["chrome_ver"].strip() for r in rows})
    cv = ",".join(pins) if pins else "none"
    print(f"chrome_ver={cv}" + ("  MIXED: rows span more than one pin" if len(pins) > 1 else ""))
    stats = {}
    for mode in ("full", "pack"):
        grp = [r for r in rows if r.get("prompt") == mode]
        if len(grp) < N_MIN:
            print(f"MISSING: {mode}: {len(grp)} of >={N_MIN} live rows with first_turn_input")
            return 2
        vals = [int(r["first_turn_input"]) for r in grp]
        stats[mode] = sum(vals) / len(vals)
        print(f"{mode:5s}: n={len(grp)} mean_first_turn_input={stats[mode]:.1f} chrome_ver={cv}")
    lower = stats["pack"] < stats["full"]
    verdict = "PASS" if lower else "FAIL"
    print(f"pack vs full: first_turn_input {'lower' if lower else 'NOT lower'} -> {verdict} chrome_ver={cv}")
    return 0 if lower else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
