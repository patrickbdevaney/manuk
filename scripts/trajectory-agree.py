#!/usr/bin/env python3
"""W4.P0.g: trajectory agreement between manuk and cdp-layer on the same tasks.

Reads the per-turn action log (docs/sprint/SCOREBOARD.tsv.turns.tsv) and compares the
tool sequence of the manuk arm against the cdp-layer arm, task by task and run by run.
A '+' in the tool column is a parallel call in one turn; it is flattened in order.
Text-only turns (empty tool) are dropped.

Two numbers per run index r in {1,2,3}, over every task both arms ran:
  exact  = share of tasks whose flattened tool sequences are identical
  lcs    = mean of 2*LCS(a,b)/(len(a)+len(b)) over the same tasks (1.0 when both empty)
The 3-run band is [min, max] of those numbers over r.

The external slice has no cdp-layer turns rows in the turns file, so only the own
suite is paired here; the script reports that instead of guessing.

Usage:
  python3 -I scripts/trajectory-agree.py                 # write TSV + print summary
  python3 -I scripts/trajectory-agree.py --mutate shuffle  # shuffle manuk turn order (red control)
  python3 -I scripts/trajectory-agree.py --selftest        # exit 1 unless shuffle lowers the rate
"""
import argparse
import csv
import os
import random
import sys
from collections import defaultdict

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
TURNS = os.path.join(ROOT, "docs/sprint/SCOREBOARD.tsv.turns.tsv")
SCOREBOARD = os.path.join(ROOT, "docs/sprint/SCOREBOARD.tsv")
OUT = os.path.join(ROOT, "docs/sprint/w4p0g/TRAJECTORY.tsv")
ARM_A, ARM_B = "manuk", "cdp-layer"
RUNS = (1, 2, 3)
SEED = 424242


def load_trajectories(path):
    """(chrome_ver, arm, task, run) -> (turn_no, tool) rows, sorted by turn."""
    rows = defaultdict(list)
    chrome = set()
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


def shuffled(seq, rng):
    seq = list(seq)
    rng.shuffle(seq)
    return seq


def compare(rows, chrome, mutate):
    rng = random.Random(SEED)
    results = {}  # run -> list of (task, exact, sim)
    tasks_a = {k[2] for k in rows if k[1] == ARM_A}
    tasks_b = {k[2] for k in rows if k[1] == ARM_B}
    both = sorted(tasks_a & tasks_b)
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
                a = shuffled(a, rng)
            out.append((task, a == b, similarity(a, b), ka[0]))
        results[run] = out
    return both, results


def band(values):
    return min(values), max(values)


def summarize(both, results):
    per_run = {}
    for run, out in results.items():
        n = len(out)
        exact = sum(1 for _, e, _, _ in out if e) / n if n else 0.0
        lcs = sum(s for _, _, s, _ in out) / n if n else 0.0
        per_run[run] = (n, exact, lcs)
    ex_lo, ex_hi = band([v[1] for v in per_run.values()])
    lc_lo, lc_hi = band([v[2] for v in per_run.values()])
    return per_run, (ex_lo, ex_hi), (lc_lo, lc_hi)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--mutate", choices=["shuffle"], default=None)
    ap.add_argument("--selftest", action="store_true")
    ap.add_argument("--no-write", action="store_true")
    args = ap.parse_args()

    rows, chrome = load_trajectories(TURNS)
    if len(chrome) != 1:
        print(f"FAIL: turns file mixes chrome_ver {sorted(chrome)}", file=sys.stderr)
        return 1
    chrome_ver = chrome.pop()

    both, results = compare(rows, chrome, args.mutate)
    per_run, (ex_lo, ex_hi), (lc_lo, lc_hi) = summarize(both, results)

    if args.selftest:
        # Red control: shuffling manuk's turn order must lower BOTH rates on EVERY run index.
        _, real = compare(rows, chrome, None)
        r_per, _, _ = summarize(both, real)
        _, shuf = compare(rows, chrome, "shuffle")
        s_per, _, _ = summarize(both, shuf)
        for run in RUNS:
            print(f"  run {run}: exact real={r_per[run][1]:.3f} shuffled={s_per[run][1]:.3f}  "
                  f"lcs real={r_per[run][2]:.3f} shuffled={s_per[run][2]:.3f}")
        ok = all(s_per[r][1] < r_per[r][1] and s_per[r][2] < r_per[r][2] for r in RUNS)
        print("SELFTEST " + ("PASS" if ok else "FAIL: shuffle did not lower the rate on every run"))
        return 0 if ok else 1

    print(f"chrome_ver={chrome_ver} mutate={args.mutate or 'none'} tasks_paired={len(both)} "
          f"(own suite; external has no cdp-layer turns rows)")
    for run in RUNS:
        n, ex, lc = per_run[run]
        print(f"  run {run}: n={n} exact={ex:.3f} lcs={lc:.3f}")
    print(f"  exact band [{ex_lo:.3f}, {ex_hi:.3f}]  lcs band [{lc_lo:.3f}, {lc_hi:.3f}]")

    if not args.no_write and args.mutate is None:
        os.makedirs(os.path.dirname(OUT), exist_ok=True)
        with open(OUT, "w", newline="") as f:
            w = csv.writer(f, delimiter="\t", lineterminator="\n")
            w.writerow(["chrome_ver", "slice", "task", "run", "tool_seq_identical",
                        "lcs_similarity", "len_manuk", "len_cdp_layer"])
            for run in RUNS:
                for task, exact, sim, _ in results[run]:
                    la = len(flatten(rows[(chrome_ver, ARM_A, task, run)]))
                    lb = len(flatten(rows[(chrome_ver, ARM_B, task, run)]))
                    w.writerow([chrome_ver, "own", task, run, int(exact), f"{sim:.4f}", la, lb])
        print(f"wrote {os.path.relpath(OUT, ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
