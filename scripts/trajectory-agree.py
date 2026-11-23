#!/usr/bin/env python3
"""W4.P0.g / W4.P0.g.a: trajectory agreement between manuk and cdp-layer on the same tasks.

Reads the per-turn action logs and compares the tool sequence of the manuk arm against the cdp-layer arm,
task by task and run by run, per slice:
  own       docs/sprint/SCOREBOARD.tsv.turns.tsv                       (manuk + cdp-layer, own suite)
  external  docs/sprint/w4p0ga/manuk-external.turns.tsv (manuk, X01..X10, W4.P0.g.a.a; same-day)  +
            docs/sprint/w4p0ga/cdp-layer-external.turns.tsv (cdp-layer, X01..X10, W4.P0.g.a)
            (the 2026-10-31 manuk file docs/sprint/w4e2a-default-ext.tsv.turns.tsv is still readable via
             --manuk-external for the before/after comparison)

A '+' in the tool column is a parallel call in one turn; it is flattened in order.
Text-only turns (empty tool) are dropped.

Two numbers per run index r in {1,2,3}, over every task both arms ran in that slice:
  exact  = share of tasks whose flattened tool sequences are identical
  lcs    = mean of 2*LCS(a,b)/(len(a)+len(b)) over the same tasks (1.0 when both empty)
The 3-run band is [min, max] of those numbers over r.

A slice whose turns file(s) are missing or unreadable is reported as UNPAIRED with no numbers; it is never
reported as tasks_paired=0 with rates of 0.

Usage:
  python3 -I scripts/trajectory-agree.py                    # write TSV + print summary (both slices)
  python3 -I scripts/trajectory-agree.py --mutate shuffle   # shuffle manuk turn order (red control, no write)
  python3 -I scripts/trajectory-agree.py --cdp-external PATH  # override the cdp-layer external turns file
  python3 -I scripts/trajectory-agree.py --manuk-external PATH  # override the manuk external turns file (a missing
                                                            # file makes the external slice UNPAIRED)
  python3 -I scripts/trajectory-agree.py --selftest         # exit 1 unless shuffle lowers the rates on every
                                                            # paired slice and a removed external file is UNPAIRED
"""
import argparse
import csv
import os
import random
import sys
from collections import defaultdict

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
OWN_TURNS = os.path.join(ROOT, "docs/sprint/SCOREBOARD.tsv.turns.tsv")
EXT_MANUK_TURNS = os.path.join(ROOT, "docs/sprint/w4p0ga/manuk-external.turns.tsv")
EXT_CDP_TURNS = os.path.join(ROOT, "docs/sprint/w4p0ga/cdp-layer-external.turns.tsv")
OUT = os.path.join(ROOT, "docs/sprint/w4p0g/TRAJECTORY.tsv")
ARM_A, ARM_B = "manuk", "cdp-layer"
RUNS = (1, 2, 3)
SEED = 424242
PIN = "145.0.7632.116"
OWN_SUITE = os.path.join(ROOT, "bench/agent-tasks/suite-v1.json")  # the frozen own slice: 40 task ids


def load_turns(paths):
    """(chrome_ver, arm, task, run) -> [(turn_no, tool)] over all paths; returns (rows, chrome_set)."""
    rows = defaultdict(list)
    chrome = set()
    for path in paths:
        with open(path, newline="") as f:
            for r in csv.DictReader(f, delimiter="\t"):
                chrome.add(r["chrome_ver"])
                rows[(r["chrome_ver"], r["arm"], r["task"], int(r["run"]))].append(
                    (int(r["turn"]), r["tool"]))
    return rows, chrome


def flatten(turn_rows):
    seq = []
    for _, tool in sorted(turn_rows):
        for t in tool.split("+"):
            t = t.strip()
            if t:
                seq.append(t)
    return seq


def lcs_len(a, b):
    prev = [0] * (len(b) + 1)
    for x in a:
        cur = [0]
        for j, y in enumerate(b, 1):
            cur.append(prev[j - 1] + 1 if x == y else max(prev[j], cur[j - 1]))
        prev = cur
    return prev[-1]


def similarity(a, b):
    if not a and not b:
        return 1.0
    return 2.0 * lcs_len(a, b) / (len(a) + len(b))


def compare(rows, mutate):
    """Per run: list of (task, exact, sim). Also returns the sorted list of tasks both arms ran."""
    rng = random.Random(SEED)
    tasks_a = {k[2] for k in rows if k[1] == ARM_A}
    tasks_b = {k[2] for k in rows if k[1] == ARM_B}
    both = sorted(tasks_a & tasks_b)
    results = {}
    for run in RUNS:
        out = []
        for task in both:
            ka = [k for k in rows if k[1] == ARM_A and k[2] == task and k[3] == run]
            kb = [k for k in rows if k[1] == ARM_B and k[2] == task and k[3] == run]
            if not ka or not kb:
                continue
            ka, kb = ka[0], kb[0]
            a = flatten(rows[ka])
            b = flatten(rows[kb])
            if mutate == "shuffle":
                a = list(a)
                rng.shuffle(a)
            out.append((task, a == b, similarity(a, b), ka))
        results[run] = out
    return both, results


def summarize(results):
    per_run = {}
    for run, out in results.items():
        n = len(out)
        exact = sum(1 for _, e, _, _ in out if e) / n if n else 0.0
        lcs = sum(s for _, _, s, _ in out) / n if n else 0.0
        per_run[run] = (n, exact, lcs)
    ex = [v[1] for v in per_run.values()]
    lc = [v[2] for v in per_run.values()]
    return per_run, (min(ex), max(ex)), (min(lc), max(lc))


def load_slice(name, paths, keep=None):
    """Returns (rows, chrome_ver) or None when a file is missing (UNPAIRED). keep = task ids to retain."""
    missing = [p for p in paths if not os.path.exists(p)]
    if missing:
        print(f"slice={name} UNPAIRED: turns file missing: {', '.join(os.path.relpath(p, ROOT) for p in missing)}")
        return None
    rows, chrome = load_turns(paths)
    if keep is not None:
        rows = defaultdict(list, {k: v for k, v in rows.items() if k[2] in keep})
    if len(chrome) != 1:
        print(f"FAIL: slice {name} mixes chrome_ver {sorted(chrome)}", file=sys.stderr)
        sys.exit(1)
    cv = chrome.pop()
    if cv != PIN:
        print(f"FAIL: slice {name} chrome_ver {cv} != pin {PIN}", file=sys.stderr)
        sys.exit(1)
    return rows, cv


def report(name, rows, chrome_ver, mutate):
    both, results = compare(rows, mutate)
    per_run, (ex_lo, ex_hi), (lc_lo, lc_hi) = summarize(results)
    print(f"chrome_ver={chrome_ver} slice={name} mutate={mutate or 'none'} tasks_paired={len(both)}")
    for run in RUNS:
        n, ex, lc = per_run[run]
        print(f"  run {run}: n={n} exact={ex:.3f} lcs={lc:.3f}")
    print(f"  exact band [{ex_lo:.3f}, {ex_hi:.3f}]  lcs band [{lc_lo:.3f}, {lc_hi:.3f}]")
    return both, results, per_run


def own_ids():
    import json
    return {t["id"] for t in json.load(open(OWN_SUITE))["tasks"]}


def slices(cdp_external, manuk_external=EXT_MANUK_TURNS):
    own = load_slice("own", [OWN_TURNS], keep=own_ids())
    ext = load_slice("external", [manuk_external, cdp_external])
    return {"own": own, "external": ext}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--mutate", choices=["shuffle"], default=None)
    ap.add_argument("--selftest", action="store_true")
    ap.add_argument("--no-write", action="store_true")
    ap.add_argument("--cdp-external", default=EXT_CDP_TURNS,
                    help="cdp-layer external turns file (a missing file makes the external slice UNPAIRED)")
    ap.add_argument("--manuk-external", default=EXT_MANUK_TURNS,
                    help="manuk external turns file (a missing file makes the external slice UNPAIRED)")
    args = ap.parse_args()

    loaded = slices(args.cdp_external, args.manuk_external)

    if args.selftest:
        # Red control 1: shuffling manuk's turn order must lower the rates on EVERY run index, on every paired slice.
        ok = True
        for name, got in loaded.items():
            if got is None:
                print(f"slice={name} UNPAIRED (selftest: {name} not checked)")
                ok = False
                continue
            rows, _ = got
            _, real = compare(rows, None)
            r_per = summarize(real)[0]
            _, shuf = compare(rows, "shuffle")
            s_per = summarize(shuf)[0]
            for run in RUNS:
                print(f"  {name} run {run}: exact real={r_per[run][1]:.3f} shuffled={s_per[run][1]:.3f}  "
                      f"lcs real={r_per[run][2]:.3f} shuffled={s_per[run][2]:.3f}")
            # LCS must fall on every run. Exact must fall on every run where it is non-zero; where the real
            # exact rate is already 0 (external slice) it cannot fall, so it must stay 0.
            ok = ok and all(s_per[r][2] < r_per[r][2] for r in RUNS)
            ok = ok and all((s_per[r][1] < r_per[r][1]) if r_per[r][1] > 0 else (s_per[r][1] == 0) for r in RUNS)
        # Red control 2: a removed external cdp-layer turns file must report the slice UNPAIRED, not zero.
        # Red control 3 (W4.P0.g.a.a): a missing manuk external file must also report UNPAIRED.
        for which, kw in (("cdp-layer", {"cdp_external": os.path.join(ROOT, "docs/sprint/w4p0ga/__removed__.turns.tsv")}),
                          ("manuk", {"cdp_external": EXT_CDP_TURNS,
                                     "manuk_external": os.path.join(ROOT, "docs/sprint/w4p0ga/__removed__.turns.tsv")})):
            dropped = slices(**kw)["external"]
            unpaired = dropped is None
            print(f"  removed {which} external turns file -> " + ("UNPAIRED" if unpaired else "PAIRED (FAIL)"))
            ok = ok and unpaired
        print("SELFTEST " + ("PASS" if ok else "FAIL"))
        return 0 if ok else 1

    if args.mutate:
        for name, got in loaded.items():
            if got is not None:
                report(name, got[0], got[1], args.mutate)
        return 0

    out_rows = []
    for name, got in loaded.items():
        if got is None:
            continue
        rows, cv = got
        both, results, _ = report(name, rows, cv, None)
        for run in RUNS:
            for task, exact, sim, _ in results[run]:
                la = len(flatten(rows[(cv, ARM_A, task, run)]))
                lb = len(flatten(rows[(cv, ARM_B, task, run)]))
                out_rows.append([cv, name, task, run, int(exact), f"{sim:.4f}", la, lb])

    if not args.no_write:
        os.makedirs(os.path.dirname(OUT), exist_ok=True)
        with open(OUT, "w", newline="") as f:
            w = csv.writer(f, delimiter="\t", lineterminator="\n")
            w.writerow(["chrome_ver", "slice", "task", "run", "tool_seq_identical",
                        "lcs_similarity", "len_manuk", "len_cdp_layer"])
            w.writerows(out_rows)
        print(f"wrote {os.path.relpath(OUT, ROOT)} ({len(out_rows)} rows)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
