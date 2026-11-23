#!/usr/bin/env python3
"""W4.P0.g.b: first divergence per non-identical (slice, task, run) between the manuk and cdp-layer arms.

Reads the same turns files as scripts/trajectory-agree.py (imported, so the flattening is identical) and, for
every (slice, task, run) that docs/sprint/w4p0g/TRAJECTORY.tsv marks tool_seq_identical=0, finds the first
position where the two flattened tool sequences part:

  first_diff_index  1-based position in the flattened sequence (parallel '+' calls are flattened in order)
  kind              substitute   both arms have a call here and the names differ
                    manuk_extra  cdp-layer's sequence ended first; manuk makes a further call (tool_cdp_layer=<end>)
                    cdp_extra    manuk's sequence ended first; cdp-layer makes a further call (tool_manuk=<end>)
  turn_*            the agent turn number that the call at first_diff_index came from (empty at <end>)

Output: docs/sprint/w4p0g/DIVERGENCE.tsv (one row per non-identical (slice, task, run); 116 rows at pin 145).

Usage:
  python3 -I scripts/first-divergence.py                 # write DIVERGENCE.tsv
  python3 -I scripts/first-divergence.py --check [PATH]  # exit 1 unless PATH matches TRAJECTORY + turns exactly
  python3 -I scripts/first-divergence.py --selftest      # red controls on temp copies: a swapped tool name and a
                                                         # dropped row must each make --check fail; then exit 0 if so
"""
import argparse
import csv
import importlib.util
import os
import shutil
import sys
import tempfile

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
TRAJ = os.path.join(ROOT, "docs/sprint/w4p0g/TRAJECTORY.tsv")
OUT = os.path.join(ROOT, "docs/sprint/w4p0g/DIVERGENCE.tsv")
COLS = ["chrome_ver", "slice", "task", "run", "first_diff_index", "kind",
        "turn_manuk", "tool_manuk", "turn_cdp_layer", "tool_cdp_layer", "len_manuk", "len_cdp_layer"]
ARM_A, ARM_B = "manuk", "cdp-layer"
END = "<end>"


def _trajectory_agree():
    spec = importlib.util.spec_from_file_location("trajectory_agree", os.path.join(ROOT, "scripts/trajectory-agree.py"))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


TA = _trajectory_agree()


def flatten_turns(turn_rows):
    """Same ordering and '+' splitting as trajectory-agree.flatten, but keeps the turn number of each call."""
    seq = []
    for turn, tool in sorted(turn_rows):
        for t in tool.split("+"):
            t = t.strip()
            if t:
                seq.append((turn, t))
    return seq


def first_divergence(a, b):
    """a, b: flattened [(turn, tool)] lists. Returns (index1, kind, (turn_a, tool_a), (turn_b, tool_b)) or None."""
    for i in range(max(len(a), len(b))):
        ea = a[i] if i < len(a) else None
        eb = b[i] if i < len(b) else None
        if ea is not None and eb is not None and ea[1] == eb[1]:
            continue
        if ea is not None and eb is not None:
            kind = "substitute"
        elif eb is None:
            kind = "manuk_extra"  # cdp-layer ended; manuk still calls
        else:
            kind = "cdp_extra"  # manuk ended; cdp-layer still calls
        return (i + 1, kind,
                ("" if ea is None else ea[0], END if ea is None else ea[1]),
                ("" if eb is None else eb[0], END if eb is None else eb[1]),
                len(a), len(b))
    return None  # identical sequences


def expected_rows(traj_path, cdp_external=TA.EXT_CDP_TURNS):
    """Rebuild the DIVERGENCE rows from TRAJECTORY.tsv keys (identical=0) and the turns files. Returns (rows, errs)."""
    errs = []
    with open(traj_path, newline="") as f:
        traj = list(csv.DictReader(f, delimiter="\t"))
    want = [r for r in traj if r["tool_seq_identical"] == "0"]
    slices = TA.slices(cdp_external)
    out = []
    for r in want:
        got = slices.get(r["slice"])
        if got is None:
            errs.append(f"slice {r['slice']} UNPAIRED in turns files; cannot locate {r['task']} run {r['run']}")
            continue
        rows, cv = got
        run = int(r["run"])
        ka = (cv, ARM_A, r["task"], run)
        kb = (cv, ARM_B, r["task"], run)
        if ka not in rows or kb not in rows:
            errs.append(f"{r['slice']} {r['task']} run {run}: missing turns for an arm")
            continue
        d = first_divergence(flatten_turns(rows[ka]), flatten_turns(rows[kb]))
        if d is None:
            errs.append(f"{r['slice']} {r['task']} run {run}: TRAJECTORY says non-identical but turns are identical")
            continue
        idx, kind, (ta, toola), (tb, toolb), la, lb = d
        out.append([cv, r["slice"], r["task"], str(run), str(idx), kind, str(ta), toola, str(tb), toolb,
                    str(la), str(lb)])
    return out, errs


def read_tsv(path):
    with open(path, newline="") as f:
        rd = csv.reader(f, delimiter="\t")
        rows = list(rd)
    return rows


def check(path, traj_path=TRAJ):
    """Exit-status style: returns list of problems (empty = pass)."""
    problems = []
    if not os.path.exists(path):
        return [f"{path} missing"]
    got_rows = read_tsv(path)
    if not got_rows or got_rows[0] != COLS:
        return [f"header must be {COLS}"]
    want, errs = expected_rows(traj_path)
    problems += errs
    if len(got_rows) - 1 != len(want):
        problems.append(f"row count {len(got_rows) - 1} != count of tool_seq_identical=0 rows {len(want)}")
    key = lambda r: tuple(r[:4])
    got_by = {key(r): r for r in got_rows[1:] if len(r) == len(COLS)}
    want_by = {key(r): r for r in want}
    for k in sorted(set(want_by) - set(got_by)):
        problems.append(f"missing row for {k}")
    for k in sorted(set(got_by) - set(want_by)):
        problems.append(f"extra row for {k}")
    for k in sorted(set(got_by) & set(want_by)):
        if got_by[k] != want_by[k]:
            diff = [c for c, a, b in zip(COLS, got_by[k], want_by[k]) if a != b]
            problems.append(f"row {k} disagrees with turns on {diff}: got {got_by[k]} want {want_by[k]}")
    return problems


def selftest():
    ok = True
    if check(OUT):
        print("selftest: real DIVERGENCE.tsv does not pass --check; fix that first", file=sys.stderr)
        return 1
    tmp = tempfile.mkdtemp(prefix="divergence-selftest-")
    try:
        base = read_tsv(OUT)
        # Red control 1: swap the two tool names in one row (positions are unchanged, only names move).
        mut = os.path.join(tmp, "swapped.tsv")
        rows = [list(r) for r in base]
        i = next(j for j in range(1, len(rows)) if rows[j][5] == "substitute")
        rows[i][7], rows[i][9] = rows[i][9], rows[i][7]
        with open(mut, "w", newline="") as f:
            csv.writer(f, delimiter="\t", lineterminator="\n").writerows(rows)
        p1 = check(mut)
        print(f"  red control swapped names (row {i}): {'FAILS as required' if p1 else 'PASSES (FAIL)'}")
        ok = ok and bool(p1)
        # Red control 2: drop one row; the count check must fail.
        mut2 = os.path.join(tmp, "dropped.tsv")
        with open(mut2, "w", newline="") as f:
            csv.writer(f, delimiter="\t", lineterminator="\n").writerows(base[:-1])
        p2 = check(mut2)
        print(f"  red control dropped row: {'FAILS as required' if p2 else 'PASSES (FAIL)'}")
        ok = ok and bool(p2)
    finally:
        shutil.rmtree(tmp, ignore_errors=True)
    print("SELFTEST " + ("PASS" if ok else "FAIL"))
    return 0 if ok else 1


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", nargs="?", const=OUT, default=None)
    ap.add_argument("--selftest", action="store_true")
    args = ap.parse_args()
    if args.selftest:
        return selftest()
    if args.check is not None:
        probs = check(args.check)
        for p in probs:
            print("FAIL: " + p)
        print(("CHECK PASS " if not probs else "CHECK FAIL ") + args.check)
        return 0 if not probs else 1
    rows, errs = expected_rows(TRAJ)
    if errs:
        for e in errs:
            print("FAIL: " + e, file=sys.stderr)
        return 1
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with open(OUT, "w", newline="") as f:
        w = csv.writer(f, delimiter="\t", lineterminator="\n")
        w.writerow(COLS)
        w.writerows(rows)
    print(f"wrote {os.path.relpath(OUT, ROOT)} ({len(rows)} rows)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
