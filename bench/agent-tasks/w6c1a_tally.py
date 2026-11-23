#!/usr/bin/env python3
"""W6.C1.a tally: does the W6.C1 escalation ladder move pass counts on the REAL-fast 17 + the 15 dialog_modal tasks?

Ladder arm (NEW): cdp-layer at HEAD ee2f09e4 (contains W6.C1 51b8d4f2 and W6.C2 499e5fba), 32 tasks x3,
rows bench-queue/out/w6c1a-ladder.tsv, transcripts ~/.cache/manuk-bq/transcripts-w6c1a.
Baseline (OLD): W6.B0 cdp-layer rows for the same 32 tasks (bench-queue/out/w6b0-cdplayer-real.tsv, HEAD 6a7acb8f,
i.e. before C1 AND before C2), transcripts ~/.cache/manuk-bq/transcripts-w6b0. Both pinned to 145.0.7632.116.

Caveat stated up front: OLD lacks C2, so any movement on the REAL-fast 17 is C1+C2, not C1 alone.

Pre-registered rule (written before any W6.C1.a row existed; do not edit after results). It is the W6.X1 rule over 32 tasks:
  - per task: passes an arm if >=2 of its 3 runs pass (task_pass_2of3).
  - per-task band: the OLD passing-run count for that task, +/-1 run. A task MOVES if |NEW - OLD| >= 2 runs.
  - verdict:
      moved <= 1                  -> INSIDE_BAND: passes stay inside the off-arm band; ladder worth ~0; keep as hygiene.
      moved >= 2, all upward      -> LADDER_PASSES: ladder moves passes up; credit C1 (plus C2 on the fast 17, see caveat).
      otherwise                   -> MIXED: not decided by this rule; report and escalate.
  - the dialog_modal 15 are reported separately (the ladder's target set) with the same per-task rule.
  - mechanism check (must hold, else the measurement is void): the OLD arm's transcripts contain OCCLUDED refusals on
    the dialog tasks, and the NEW arm's transcripts show the ladder ran (`tried:` text from the typed refusal, or
    rung names) on at least one task. A run with no OCCLUDED text in either arm cannot test the ladder.

Refuses (exit 2) on: a non-primary judge, a chrome_ver other than $MANUK_CHROME_PIN, or a missing (task, run) key.

  python3 -I bench/agent-tasks/w6c1a_tally.py
  python3 -I bench/agent-tasks/w6c1a_tally.py --mutate N   # gate mutant: flips N NEW runs (must change the verdict)
  python3 -I bench/agent-tasks/w6c1a_tally.py --self-test  # refusals and the verdict rule can go red
"""
import argparse, csv, glob, os, sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "../.."))
NEW = os.path.join(ROOT, "bench-queue/out/w6c1a-ladder.tsv")
OLD = os.path.join(ROOT, "bench-queue/out/w6b0-cdplayer-real.tsv")
NEW_T = os.path.expanduser("~/.cache/manuk-bq/transcripts-w6c1a")
OLD_T = os.path.expanduser("~/.cache/manuk-bq/transcripts-w6b0")
JUDGE = "openai/gpt-oss-120b"
RUNS = 3
DIALOG = ["topwork-1", "topwork-2", "topwork-3", "topwork-4", "fly-unified-2", "fly-unified-4", "fly-unified-5",
          "fly-unified-9", "dashdish-2", "dashdish-4", "dashdish-5", "staynb-6", "staynb-8", "zilloft-3", "zilloft-6"]
FAST = ["gocalendar-2", "gomail-3", "gomail-5", "gomail-8", "networkin-3", "opendining-10", "opendining-2",
        "opendining-4", "opendining-7", "opendining-8", "staynb-4", "udriver-10", "udriver-7", "udriver-9", "zilloft-2",
        "zilloft-5", "dashdish-3"]
TASKS = FAST + DIALOG


class Refuse(Exception):
    pass


def load(p):
    if not os.path.exists(p):
        raise Refuse(f"missing table {os.path.relpath(p, ROOT)}")
    with open(p, newline="") as f:
        return list(csv.DictReader(f, delimiter="\t"))


def table(rows, tasks=TASKS, runs=RUNS, label="arm"):
    bad = sorted({r.get("judge_model", "") for r in rows if r.get("judge_model", "") not in ("", JUDGE)})
    if bad:
        raise Refuse(f"{label}: non-primary judge rows {bad}")
    pin = os.environ.get("MANUK_CHROME_PIN", "")
    cv = sorted({r.get("chrome_ver", "") for r in rows})
    if not pin or cv != [pin]:
        raise Refuse(f"{label}: chrome_ver {cv} != pin {pin!r}")
    got = {}
    for r in rows:
        if r.get("task") in tasks and r.get("pass") in ("0", "1"):
            got[(r["task"], int(r["run"]))] = int(r["pass"])
    missing = [(t, k) for t in tasks for k in range(1, runs + 1) if (t, k) not in got]
    if missing:
        raise Refuse(f"{label}: {len(missing)} (task,run) keys unscored, e.g. {missing[:3]}")
    return got


def per_task(new, old, tasks, runs=RUNS):
    per = {}
    for t in tasks:
        n = sum(new[(t, k)] for k in range(1, runs + 1))
        o = sum(old[(t, k)] for k in range(1, runs + 1))
        per[t] = {"new_runs": n, "old_runs": o, "new_pass": int(n >= 2), "old_pass": int(o >= 2),
                  "moved": abs(n - o) >= 2}
    return per


def call_for(per, tasks):
    moved = [t for t in tasks if per[t]["moved"]]
    up = [t for t in moved if per[t]["new_runs"] > per[t]["old_runs"]]
    if len(moved) <= 1:
        return moved, "INSIDE_BAND"
    if len(up) == len(moved):
        return moved, "LADDER_PASSES"
    return moved, "MIXED"


def mechanism(tdir, tasks, arm):
    """OCCLUDED / tried: hits per task across the 3 runs (text evidence that the guard path was live)."""
    hits = {}
    for t in tasks:
        occ = tri = 0
        for k in range(1, RUNS + 1):
            for p in glob.glob(os.path.join(tdir, f"cdp-layer_{t}_r{k}.jsonl")):
                with open(p, errors="replace") as f:
                    s = f.read()
                occ += s.count("OCCLUDED")
                tri += s.count("tried:")
        hits[t] = (occ, tri)
    return hits


def run(new_rows, old_rows, mutate_note="", out=sys.stdout, mech=True):
    new = table(new_rows, label="ladder")
    old = table(old_rows, label="baseline")
    per = per_task(new, old, TASKS)
    print("task\tset\tnew_runs\told_runs\tnew_pass2of3\told_pass2of3\tmoved", file=out)
    for t in TASKS:
        p = per[t]
        s = "dialog" if t in DIALOG else "fast"
        print(f"{t}\t{s}\t{p['new_runs']}\t{p['old_runs']}\t{p['new_pass']}\t{p['old_pass']}\t{int(p['moved'])}", file=out)
    moved, call = call_for(per, TASKS)
    dmoved, dcall = call_for(per, DIALOG)
    print(f"all32\tnew_runs={sum(p['new_runs'] for p in per.values())}/96\told_runs={sum(p['old_runs'] for p in per.values())}/96"
          f"\ttask_pass_2of3 new={sum(per[t]['new_pass'] for t in TASKS)}/32 old={sum(per[t]['old_pass'] for t in TASKS)}/32",
          file=out)
    print(f"dialog15\ttask_pass_2of3 new={sum(per[t]['new_pass'] for t in DIALOG)}/15 old={sum(per[t]['old_pass'] for t in DIALOG)}/15"
          f"\tmoved={len(dmoved)} {dmoved} -> {dcall}", file=out)
    print(f"moved={len(moved)} {moved}", file=out)
    print(f"VERDICT: {call}{mutate_note}", file=out)
    if mech:
        oh, nh = mechanism(OLD_T, DIALOG, "old"), mechanism(NEW_T, DIALOG, "new")
        print("mechanism (OCCLUDED hits / tried: hits, dialog15): old=" + str(sum(v[0] for v in oh.values())) +
              "/" + str(sum(v[1] for v in oh.values())) + " new=" + str(sum(v[0] for v in nh.values())) + "/" +
              str(sum(v[1] for v in nh.values())), file=out)
    return call


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--mutate", type=int, default=0, help="gate mutant: flip N NEW runs")
    ap.add_argument("--self-test", action="store_true")
    a = ap.parse_args()
    if a.self_test:
        return self_test()
    new_rows, old_rows = load(NEW), load(OLD)
    note = ""
    if a.mutate:
        n = 0
        for r in new_rows:
            if r.get("task") in TASKS and r.get("pass") in ("0", "1") and n < a.mutate:
                r["pass"] = str(1 - int(r["pass"])); n += 1
        note = f" [mutant: {n} NEW runs flipped]"
    run(new_rows, old_rows, mutate_note=note, mech=not a.mutate)
    return 0


def _synthetic(new_pat, old_pat):
    def rows(pat):
        out = []
        for i, t in enumerate(TASKS):
            for k in range(1, RUNS + 1):
                out.append({"task": t, "run": str(k), "pass": str(pat[i][k - 1]), "judge_model": JUDGE,
                            "chrome_ver": os.environ["MANUK_CHROME_PIN"]})
        return out
    return rows(new_pat), rows(old_pat)


def self_test():
    os.environ.setdefault("MANUK_CHROME_PIN", "145.0.7632.116")
    fails = 0

    def expect(name, cond):
        nonlocal fails
        print(("ok   " if cond else "RED  ") + name)
        fails += 0 if cond else 1

    same = [[1, 1, 0]] * 32
    n, o = _synthetic(same, same)
    nt, ot = table(n, label="n"), table(o, label="o")
    expect("identical arms -> INSIDE_BAND", call_for(per_task(nt, ot, TASKS), TASKS)[1] == "INSIDE_BAND")
    # gate must be able to go red: 3 tasks gain 3 runs under the ladder -> LADDER_PASSES
    np_ = [[1, 1, 1]] * 3 + [[1, 1, 0]] * 29
    op = [[0, 0, 0]] * 3 + [[1, 1, 0]] * 29
    n, o = _synthetic(np_, op)
    expect("3 upward moves -> LADDER_PASSES",
           call_for(per_task(table(n, label="n"), table(o, label="o"), TASKS), TASKS)[1] == "LADDER_PASSES")
    # a mixed pattern (2 up, 1 down) is not decided by the rule
    np2 = [[1, 1, 1]] * 2 + [[0, 0, 0]] + [[1, 1, 0]] * 29
    op2 = [[0, 0, 0]] * 2 + [[1, 1, 1]] + [[1, 1, 0]] * 29
    n, o = _synthetic(np2, op2)
    expect("2 up + 1 down -> MIXED",
           call_for(per_task(table(n, label="n"), table(o, label="o"), TASKS), TASKS)[1] == "MIXED")
    # a single moved task stays inside the band (threshold is >=2 moved)
    np1 = [[1, 1, 1]] + [[1, 1, 0]] * 31
    op1 = [[0, 0, 0]] + [[1, 1, 0]] * 31
    n, o = _synthetic(np1, op1)
    expect("1 moved task -> INSIDE_BAND",
           call_for(per_task(table(n, label="n"), table(o, label="o"), TASKS), TASKS)[1] == "INSIDE_BAND")
    bad = _synthetic(same, same)[0]
    for r in bad:
        r["chrome_ver"] = "144.0.0.0"
    try:
        table(bad, label="n"); expect("wrong chrome_ver refuses", False)
    except Refuse:
        expect("wrong chrome_ver refuses", True)
    short = _synthetic(same, same)[0][:-1]
    try:
        table(short, label="n"); expect("missing (task,run) refuses", False)
    except Refuse:
        expect("missing (task,run) refuses", True)
    jb = _synthetic(same, same)[0]
    jb[0]["judge_model"] = "other/judge"
    try:
        table(jb, label="n"); expect("non-primary judge refuses", False)
    except Refuse:
        expect("non-primary judge refuses", True)
    print(f"self-test: {'PASS' if fails == 0 else f'{fails} RED'}")
    return 1 if fails else 0


if __name__ == "__main__":
    try:
        sys.exit(main() or 0)
    except Refuse as e:
        print(f"refuse: {e}", file=sys.stderr)
        sys.exit(2)
