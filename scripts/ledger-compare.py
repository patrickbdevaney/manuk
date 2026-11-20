#!/usr/bin/env python3
"""ledger-compare — before/after check of the W6.L1 loop plumbing (node W6.L1.a).

  ledger-compare.py [ledger] [loop-log]
  ledger-compare.py --first-turn [ledger] [loop-log]   (node W6.L1.a.b: same 5+5 check on first_turn_input)

Reads docs/sprint/LEDGER.tsv (one row per agent attempt, written by sprint-ledger.py) and takes the first
5 `full`-prompt rows and the first 5 `pack`-prompt rows. For each group it prints the mean input_total
(prompt-side tokens: input + cache_creation + cache_read) and the DONE rate. An attempt counts as DONE when
docs/sprint/loop.log has a `✓ node <id> DONE` line 0-120 s after the attempt's ledger row (the loop writes
that line right after the row).

Exit codes:
  0  PASS    pack mean input_total < full mean AND pack DONE rate >= full DONE rate
  1  FAIL    the pack mode is not cheaper, or it lands fewer nodes (the W6.L1 claim is falsified)
  2  MISSING fewer than 5 rows in a mode, a row with no usage numbers, or no loop log (incomplete data;
             nothing is claimed)

Every line it prints carries chrome_ver (from $MANUK_CHROME_PIN, else docs/sprint/CHROME_PIN).
"""
import csv
import os
import sys
from collections import Counter
from datetime import datetime
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
N_PER_MODE = 5
DONE_WINDOW_S = 120


def chrome_ver() -> str:
    v = os.environ.get("MANUK_CHROME_PIN", "")
    if not v and (ROOT / "docs/sprint/CHROME_PIN").exists():
        v = (ROOT / "docs/sprint/CHROME_PIN").read_text().strip()
    return v or "unknown"


def done_times(log: Path) -> dict[str, list[datetime]]:
    """node id -> timestamps of its `✓ node <id> DONE` lines in loop.log."""
    out: dict[str, list[datetime]] = {}
    for line in log.read_text(errors="replace").splitlines():
        if "✓ node " not in line or " DONE " not in line:
            continue
        try:
            ts = datetime.strptime(line[:19], "%Y-%m-%d %H:%M:%S")
        except ValueError:
            continue
        node = line.split("✓ node ", 1)[1].split(" DONE ", 1)[0].strip()
        out.setdefault(node, []).append(ts)
    return out


def is_done(row: dict, dones: dict[str, list[datetime]]) -> bool:
    try:
        t = datetime.strptime(row["ts"], "%Y-%m-%d %H:%M:%S")
    except (KeyError, ValueError):
        return False
    return any(0 <= (d - t).total_seconds() <= DONE_WINDOW_S for d in dones.get(row.get("node", ""), []))


def first_turn(a: list[str]) -> int:
    """--first-turn (node W6.L1.a.b): the same 5+5 check, on turn-1 prompt size (first_turn_input, which only
    stream-json rows carry). The window starts at the first row that has first_turn_input; from there on every row
    must carry it (a missing one is the falsifier). Thresholds are the W6.L1.a ones: 5 per mode, pack must be lower
    on first_turn_input and must not land fewer nodes. Each mode's node mix is printed so a skew stays visible."""
    ledger = Path(a[0]) if len(a) > 0 else ROOT / "docs/sprint/LEDGER.tsv"
    log = Path(a[1]) if len(a) > 1 else ROOT / "docs/sprint/loop.log"
    cv = chrome_ver()
    print(f"chrome_ver={cv} check=first_turn_input")
    if not ledger.exists():
        print(f"MISSING: {ledger} does not exist (0 of {2 * N_PER_MODE} attempts recorded)")
        return 2
    if not log.exists():
        print(f"MISSING: {log} does not exist; DONE cannot be read")
        return 2
    rows = list(csv.DictReader(ledger.open(), delimiter="\t"))
    start = next((i for i, r in enumerate(rows) if (r.get("first_turn_input") or "").strip()), None)
    if start is None:
        print(f"MISSING: no row carries first_turn_input yet (of {len(rows)} ledger rows; json-era rows have no turn-1 column)")
        return 2
    window = rows[start:]
    bare = [f"{r.get('ts')} {r.get('node')} {r.get('prompt')}" for r in window
            if not (r.get("first_turn_input") or "").isdigit()]
    if bare:
        print(f"FAIL: {len(bare)} row(s) from {rows[start].get('ts')} on lack first_turn_input (falsifier):")
        for b in bare:
            print(f"  {b}")
        return 1
    dones = done_times(log)
    stats = {}
    for mode in ("full", "pack"):
        grp = [r for r in window if r.get("prompt") == mode][:N_PER_MODE]
        if len(grp) < N_PER_MODE:
            print(f"MISSING: {mode}: {len(grp)} of {N_PER_MODE} live rows with first_turn_input")
            return 2
        vals = [int(r["first_turn_input"]) for r in grp]
        d = sum(is_done(r, dones) for r in grp)
        mix = Counter(r.get("node", "?") for r in grp)
        stats[mode] = (sum(vals) / len(vals), d, len(grp))
        print(f"{mode:5s}: n={len(grp)} mean_first_turn_input={stats[mode][0]:.1f} "
              f"DONE={d}/{len(grp)} ({100 * d / len(grp):.0f}%) chrome_ver={cv}")
        print(f"       node mix: {', '.join(f'{k} x{v}' for k, v in sorted(mix.items()))}")
    (fm, fd, fn), (pm, pd, pn) = stats["full"], stats["pack"]
    cheaper = pm < fm
    lands = pd / pn >= fd / fn
    verdict = "PASS" if cheaper and lands else "FAIL"
    print(f"pack vs full: first_turn_input {'lower' if cheaper else 'NOT lower'}; "
          f"DONE rate {'not lower' if lands else 'LOWER'} -> {verdict} chrome_ver={cv}")
    return 0 if verdict == "PASS" else 1


def main(a: list[str]) -> int:
    if "--first-turn" in a:
        return first_turn([x for x in a if x != "--first-turn"])
    ledger = Path(a[0]) if len(a) > 0 else ROOT / "docs/sprint/LEDGER.tsv"
    log = Path(a[1]) if len(a) > 1 else ROOT / "docs/sprint/loop.log"
    cv = chrome_ver()
    print(f"chrome_ver={cv}")
    if not ledger.exists():
        print(f"MISSING: {ledger} does not exist (0 of {2 * N_PER_MODE} attempts recorded)")
        return 2
    if not log.exists():
        print(f"MISSING: {log} does not exist; DONE cannot be read")
        return 2
    rows = list(csv.DictReader(ledger.open(), delimiter="\t"))
    dones = done_times(log)
    stats = {}
    for mode in ("full", "pack"):
        grp = [r for r in rows if r.get("prompt") == mode][:N_PER_MODE]
        if len(grp) < N_PER_MODE:
            print(f"MISSING: {mode}: {len(grp)} of {N_PER_MODE} ledger rows")
            return 2
        bad = [r.get("node", "?") for r in grp if not (r.get("input_total") or "").isdigit()]
        if bad:
            print(f"MISSING: {mode}: no usage numbers for {', '.join(bad)} (unparseable result)")
            return 2
        tot = [int(r["input_total"]) for r in grp]
        d = sum(is_done(r, dones) for r in grp)
        stats[mode] = (sum(tot) / len(tot), d, len(grp))
        print(f"{mode:5s}: n={len(grp)} mean_input_total={stats[mode][0]:.1f} "
              f"DONE={d}/{len(grp)} ({100 * d / len(grp):.0f}%) chrome_ver={cv}")
    (fm, fd, fn), (pm, pd, pn) = stats["full"], stats["pack"]
    cheaper = pm < fm
    lands = pd / pn >= fd / fn
    verdict = "PASS" if cheaper and lands else "FAIL"
    print(f"pack vs full: input_total {'lower' if cheaper else 'NOT lower'}; "
          f"DONE rate {'not lower' if lands else 'LOWER'} -> {verdict} chrome_ver={cv}")
    return 0 if verdict == "PASS" else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
