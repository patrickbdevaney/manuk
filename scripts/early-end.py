#!/usr/bin/env python3
"""W4.P0.g.b.a.a: how the cdp-layer arm ended on every own (task,run) where manuk makes a further call.

W4.P0.g.b.a read the ending from the turn log, which has no per-call error flag, so tool_error could not be assigned.
This node re-runs those 56 own (task,run) pairs for cdp-layer at the pin with run_arm.py --transcripts
(docs/sprint/w4p0g/rerun-cdp-layer/, run1..3.tsv + run1..3.turns.tsv + transcripts/cdp-layer_<task>_r<run>.jsonl), and
classifies each ending from the raw stream-json, where tool results carry is_error.

Input rows: docs/sprint/w4p0g/DIVERGENCE.tsv rows with slice=own and kind=manuk_extra (the 56 (task,run) keys). Their
transcripts are read in full. Each transcript is one `claude -p` stream-json run: assistant messages are turns (deduped
by message id, as in run_arm.py turn_log), tool_use blocks are calls, user tool_result blocks carry is_error.

Output: docs/sprint/w4p0g/EARLY-END.tsv, one row per manuk_extra own (task,run):
  cdp_last_turn      tool_error  the last tool_result of the run has is_error=true: the arm's last call failed. This
                                 takes precedence over a text answer that follows it (the arm answered after an error).
                     text_answer the run's final assistant turn is text-only and its last tool_result is not an error
                     other       the final turn carries a tool call whose result is ok or missing (budget, timeout)
  cdp_last_turn_no   turn number of the final assistant turn
  cdp_calls          flattened tool_use count of the rerun (parallel calls counted individually)
  cdp_is_error_results  number of tool_result blocks with is_error=true anywhere in the rerun (the tool_error count)
  orig_cdp_len       len_cdp_layer of the original DIVERGENCE row (the turn-log run the pair came from)

The rerun is a new cdp-layer sample under the same (task,run) label; its call count can differ from orig_cdp_len. That is
recorded, not forced: the class is computed from the rerun transcript only.

Usage:
  python3 -I scripts/early-end.py                  # write EARLY-END.tsv (exit 1 on any missing transcript)
  python3 -I scripts/early-end.py --check [PATH]   # exit 1 unless PATH matches the transcripts exactly
  python3 -I scripts/early-end.py --selftest       # red controls on temp copies: a dropped row, a changed class and
                                                   # one flipped is_error flag must each make --check fail
"""
import argparse
import csv
import json
import os
import shutil
import sys
import tempfile
from collections import Counter

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
DIV = os.path.join(ROOT, "docs/sprint/w4p0g/DIVERGENCE.tsv")
TRANS = os.path.join(ROOT, "docs/sprint/w4p0g/rerun-cdp-layer/transcripts")
OUT = os.path.join(ROOT, "docs/sprint/w4p0g/EARLY-END.tsv")
PIN = "145.0.7632.116"  # same pin as trajectory-agree.py; MANUK_CHROME_PIN is checked against it when set
COLS = ["chrome_ver", "task", "run", "orig_cdp_len", "cdp_calls", "cdp_last_turn_no", "cdp_last_turn",
        "cdp_is_error_results"]
CLASSES = {"text_answer", "tool_error", "other"}


def read_tsv(path):
    with open(path, newline="") as f:
        return list(csv.reader(f, delimiter="\t"))


def trans_name(task, run):
    return f"cdp-layer_{task}_r{run}.jsonl"


def parse_run(path):
    """Read one stream-json transcript in full. Returns a dict (see module docstring for the fields)."""
    turns = []          # per assistant message id: list of tool_use ids
    seen = {}
    results = []       # (tool_use_id, is_error) in stream order
    run_is_error = None
    with open(path) as f:
        for line in f:
            if not line.startswith("{"):
                continue
            e = json.loads(line)
            m = e.get("message") or {}
            if e.get("type") == "assistant":
                mid = m.get("id") or f"anon{len(turns)}"
                if mid not in seen:
                    seen[mid] = len(turns)
                    turns.append([])
                for b in m.get("content") or []:
                    if b.get("type") == "tool_use" and b.get("id") not in [x for t in turns for x in t]:
                        turns[seen[mid]].append(b.get("id"))
            elif e.get("type") == "user" and isinstance(m.get("content"), list):
                for b in m["content"]:
                    if b.get("type") == "tool_result":
                        results.append((b.get("tool_use_id"), b.get("is_error") is True))
            elif e.get("type") == "result":
                run_is_error = bool(e.get("is_error"))
    if not turns:
        return {"calls": 0, "turns": 0, "last_turn_no": 0, "cls": "text_answer", "errs": 0, "run_is_error": run_is_error}
    calls = sum(len(t) for t in turns)
    errs = sum(1 for _, x in results if x)
    last_tools = turns[-1]
    if results and results[-1][1]:
        cls = "tool_error"
    elif last_tools:
        cls = "other"
    else:
        cls = "text_answer"
    return {"calls": calls, "turns": len(turns), "last_turn_no": len(turns), "cls": cls, "errs": errs,
            "run_is_error": run_is_error}


def pair_rows(div_path=DIV):
    """The 56 own manuk_extra (chrome_ver, task, run, len_cdp_layer) keys from DIVERGENCE."""
    if not os.path.exists(div_path):
        return None, [f"{div_path} missing; run scripts/first-divergence.py first"]
    div = read_tsv(div_path)
    head, body = div[0], div[1:]
    ix = {c: i for i, c in enumerate(head)}
    want = [r for r in body if r[ix["slice"]] == "own" and r[ix["kind"]] == "manuk_extra"]
    return [(r[ix["chrome_ver"]], r[ix["task"]], int(r[ix["run"]]), r[ix["len_cdp_layer"]]) for r in want], []


def expected_rows(div_path=DIV, trans_dir=TRANS):
    """Rebuild EARLY-END rows from DIVERGENCE + the transcripts. Returns (rows, errs)."""
    pairs, errs = pair_rows(div_path)
    if pairs is None:
        return [], errs
    rows = []
    for cv, task, run, orig in pairs:
        path = os.path.join(trans_dir, trans_name(task, run))
        if not os.path.exists(path):
            errs.append(f"own {task} run {run}: transcript {trans_name(task, run)} missing")
            continue
        p = parse_run(path)
        rows.append([cv, task, str(run), orig, str(p["calls"]), str(p["last_turn_no"]), p["cls"], str(p["errs"])])
    return rows, errs


def check(path, div_path=DIV, trans_dir=TRANS):
    """Returns a list of problems (empty = pass)."""
    if not os.path.exists(path):
        return [f"{path} missing"]
    got = read_tsv(path)
    if not got or got[0] != COLS:
        return [f"header must be {COLS}"]
    want, errs = expected_rows(div_path, trans_dir)
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
            problems.append(f"row {k} disagrees with transcript on {diff}: got {got_by[k]} want {want_by[k]}")
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

        # the gate's mutation: flip one is_error flag in one transcript; exactly one class must change
        tdir = os.path.join(tmp, "transcripts")
        shutil.copytree(TRANS, tdir)
        target = os.path.join(tdir, trans_name(base[1][1], int(base[1][2])))
        lines = open(target).read().split("\n")
        flipped = False
        for n in range(len(lines) - 1, -1, -1):
            if not lines[n].startswith("{"):
                continue
            e = json.loads(lines[n])
            if e.get("type") != "user" or not isinstance(e.get("message", {}).get("content"), list):
                continue
            for b in e["message"]["content"]:
                if b.get("type") == "tool_result" and not flipped and not b.get("is_error"):
                    b["is_error"] = True
                    flipped = True
            if flipped:
                lines[n] = json.dumps(e, separators=(",", ":"))
                break
        open(target, "w").write("\n".join(lines))
        probs = check(OUT, trans_dir=tdir)
        changed = [p for p in probs if p.startswith("row ")]
        p3 = flipped and len(changed) == 1
        print(f"  mutation flipped is_error in {os.path.basename(target)}: "
              f"{len(changed)} row(s) disagree -> {'ONE CLASS CHANGED as required' if p3 else 'FAIL'}")
        if changed:
            print("    " + changed[0][:200])
        ok = ok and p3
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
    print(f"wrote {os.path.relpath(OUT, ROOT)} ({len(rows)} rows) classes={dict(Counter(r[6] for r in rows))} "
          f"is_error_results={sum(int(r[7]) for r in rows)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
