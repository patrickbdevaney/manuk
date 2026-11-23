#!/usr/bin/env python3
"""W4.P0.g.b.a: how the cdp-layer arm ended on every own (task,run) where manuk makes a further call.

Input rows: docs/sprint/w4p0g/DIVERGENCE.tsv rows with slice=own and kind=manuk_extra (cdp-layer's flattened tool
sequence ended first). For each, the cdp-layer turn log in docs/sprint/SCOREBOARD.tsv.turns.tsv (arm=cdp-layer,
same chrome_ver, task, run) is read in full, text-only turns included. Those turns are kept in the turns file;
trajectory-agree.py only drops them in the comparison.

Output: docs/sprint/w4p0g/EARLY-END.tsv, one row per manuk_extra own (task,run):
  cdp_last_turn      text_answer  the last turn of the cdp-layer run is text-only (tool empty): it answered
                     tool_error   reserved: the turn log has no per-call error flag and the raw stream-json for
                                  these pin-145 own cdp-layer runs is not in the repo, so this class is never
                                  assigned here (the run-level is_error column of SCOREBOARD.tsv is 0 on all 360
                                  cdp-layer own rows at pin 145)
                     other        the last turn carries a tool call (run stopped on a tool result, budget or
                                  timeout; cannot be told apart from the log)
  cdp_last_turn_no   turn number of that last turn
  cdp_last_output_tokens  output_tokens of that last turn

Every row is also checked against the turn log: the number of flattened tool calls in the cdp-layer turns (parallel
'+' calls split, as in trajectory-agree) must equal len_cdp_layer in DIVERGENCE.tsv.

Usage:
  python3 -I scripts/early-end.py                  # write EARLY-END.tsv (exit 1 on any mismatch)
  python3 -I scripts/early-end.py --check [PATH]   # exit 1 unless PATH matches DIVERGENCE + turns exactly
  python3 -I scripts/early-end.py --selftest       # red controls on temp copies: a dropped row and a changed
                                                   # class must each make --check fail; exit 0 if both do
"""
import argparse
import csv
import os
import shutil
import sys
import tempfile
from collections import defaultdict

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
DIV = os.path.join(ROOT, "docs/sprint/w4p0g/DIVERGENCE.tsv")
TURNS = os.path.join(ROOT, "docs/sprint/SCOREBOARD.tsv.turns.tsv")
OUT = os.path.join(ROOT, "docs/sprint/w4p0g/EARLY-END.tsv")
PIN = "145.0.7632.116"  # same pin as trajectory-agree.py; MANUK_CHROME_PIN is checked against it when set
COLS = ["chrome_ver", "task", "run", "first_diff_index", "cdp_len", "cdp_last_turn_no", "cdp_last_turn",
        "cdp_last_output_tokens"]
CLASSES = {"text_answer", "tool_error", "other"}
ARM_B = "cdp-layer"


def read_tsv(path):
    with open(path, newline="") as f:
        return list(csv.reader(f, delimiter="\t"))


def cdp_turns(turns_path=TURNS):
    """(chrome_ver, task, run) -> [(turn_no, tool, output_tokens)] for arm=cdp-layer, pin only."""
    out = defaultdict(list)
    with open(turns_path, newline="") as f:
        for r in csv.DictReader(f, delimiter="\t"):
            if r["arm"] != ARM_B or r["chrome_ver"] != PIN:
                continue
            out[(r["chrome_ver"], r["task"], int(r["run"]))].append(
                (int(r["turn"]), r["tool"], int(r["output_tokens"])))
    return out


def flat_calls(turns):
    n = 0
    for _, tool, _ in turns:
        n += len([t for t in tool.split("+") if t.strip()])
    return n


def expected_rows(div_path=DIV, turns_path=TURNS):
    """Rebuild EARLY-END rows from DIVERGENCE + turns. Returns (rows, errs)."""
    errs = []
    if not os.path.exists(div_path):
        return [], [f"{div_path} missing; run scripts/first-divergence.py first"]
    div = read_tsv(div_path)
    head, body = div[0], div[1:]
    ix = {c: i for i, c in enumerate(head)}
    want = [r for r in body if r[ix["slice"]] == "own" and r[ix["kind"]] == "manuk_extra"]
    turns = cdp_turns(turns_path)
    rows = []
    for r in want:
        cv, task, run = r[ix["chrome_ver"]], r[ix["task"]], int(r[ix["run"]])
        key = (cv, task, run)
        if key not in turns:
            errs.append(f"own {task} run {run}: no cdp-layer turns at {cv}")
            continue
        seq = sorted(turns[key])
        if flat_calls(seq) != int(r[ix["len_cdp_layer"]]):
            errs.append(f"own {task} run {run}: cdp-layer has {flat_calls(seq)} calls, DIVERGENCE says "
                        f"{r[ix['len_cdp_layer']]}")
            continue
        last_no, last_tool, last_out = seq[-1]
        cls = "other" if last_tool.strip() else "text_answer"
        rows.append([cv, task, str(run), r[ix["first_diff_index"]], str(len(seq)), str(last_no), cls,
                     str(last_out)])
    return rows, errs


def check(path, div_path=DIV, turns_path=TURNS):
    """Returns a list of problems (empty = pass)."""
    if not os.path.exists(path):
        return [f"{path} missing"]
    got = read_tsv(path)
    if not got or got[0] != COLS:
        return [f"header must be {COLS}"]
    want, errs = expected_rows(div_path, turns_path)
    problems = list(errs)
    if len(got) - 1 != len(want):
        problems.append(f"row count {len(got) - 1} != manuk_extra own rows {len(want)}")
    key = lambda r: tuple(r[:3])
    got_by = {key(r): r for r in got[1:] if len(r) == len(COLS)}
    want_by = {key(r): r for r in want}
    for k in sorted(set(want_by) - set(got_by)):
        problems.append(f"missing row for {k}")
    for k in sorted(set(got_by) - set(want_by)):
        problems.append(f"extra row for {k}")
    for k in sorted(set(got_by) & set(want_by)):
        if got_by[k] != want_by[k]:
            diff = [c for c, a, b in zip(COLS, got_by[k], want_by[k]) if a != b]
            problems.append(f"row {k} disagrees with turns on {diff}: got {got_by[k]} want {want_by[k]}")
    for r in got[1:]:
        if len(r) == len(COLS) and r[COLS.index("cdp_last_turn")] not in CLASSES:
            problems.append(f"class {r[COLS.index('cdp_last_turn')]!r} not in {sorted(CLASSES)}")
    pin = os.environ.get("MANUK_CHROME_PIN")
    if pin and pin != PIN:
        problems.append(f"MANUK_CHROME_PIN={pin} differs from the pin this file is computed at ({PIN})")
    return problems


def selftest():
    ok = True
    if check(OUT):
        print("selftest: real EARLY-END.tsv does not pass --check; fix that first", file=sys.stderr)
        return 1
    tmp = tempfile.mkdtemp(prefix="early-end-selftest-")
    try:
        base = read_tsv(OUT)
        mut = os.path.join(tmp, "dropped.tsv")
        with open(mut, "w", newline="") as f:
            csv.writer(f, delimiter="\t", lineterminator="\n").writerows(base[:-1])
        p1 = check(mut)
        print(f"  red control dropped row: {'FAILS as required' if p1 else 'PASSES (FAIL)'}")
        ok = ok and bool(p1)
        mut2 = os.path.join(tmp, "reclassed.tsv")
        rows = [list(r) for r in base]
        i = next(j for j in range(1, len(rows)) if rows[j][COLS.index("cdp_last_turn")] == "text_answer")
        rows[i][COLS.index("cdp_last_turn")] = "tool_error"
        with open(mut2, "w", newline="") as f:
            csv.writer(f, delimiter="\t", lineterminator="\n").writerows(rows)
        p2 = check(mut2)
        print(f"  red control changed class (row {i}): {'FAILS as required' if p2 else 'PASSES (FAIL)'}")
        ok = ok and bool(p2)
    finally:
        shutil.rmtree(tmp, ignore_errors=True)
    print("SELFTEST " + ("PASS" if ok else "FAIL"))
    return 0 if ok else 1


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", nargs="?", const=OUT, default=None)
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args()
    if a.selftest:
        return selftest()
    if a.check is not None:
        probs = check(a.check)
        for p in probs:
            print("FAIL: " + p)
        print(("CHECK PASS " if not probs else "CHECK FAIL ") + a.check)
        return 0 if not probs else 1
    rows, errs = expected_rows()
    if errs:
        for e in errs:
            print("FAIL: " + e, file=sys.stderr)
        return 1
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with open(OUT, "w", newline="") as f:
        w = csv.writer(f, delimiter="\t", lineterminator="\n")
        w.writerow(COLS)
        w.writerows(rows)
    from collections import Counter
    print(f"wrote {os.path.relpath(OUT, ROOT)} ({len(rows)} rows) classes={dict(Counter(r[6] for r in rows))}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
