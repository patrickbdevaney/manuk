#!/usr/bin/env python3
"""W6.X1 tally: no-code occlusion test on the 15 dialog_modal tasks, cdp-layer arm, x3.

Mutant arm: MANUK_LANDING_MUTATE=1 + MANUK_CDP_MUTATE=no-revalidate (both overlay guards off), rows in
bench-queue/out/w6x1-occlusion-mutant.tsv. Unmutated arm: the same 15 tasks x3 from the W6.B0 cdp-layer rows
(bench-queue/out/w6b0-cdplayer-real.tsv), same binary (HEAD 6a7acb8f), same chrome pin.

Pre-registered rule (written before any X1 row existed; do not edit after results):
  - per task: a task passes an arm if >=2 of its 3 runs pass (task_pass_2of3).
  - per task band: the unmutated passing-run count for that task, +/-1 run. A task MOVES if the mutant's
    passing-run count is outside that band (|diff| >= 2 runs).
  - verdict: MOVED tasks >= 2 (all upward, mutant better) -> the overlay guards are blockers of real passes ->
    keep W6.C1 (overlay handling) prioritised. MOVED tasks <= 1 -> passes stay inside the per-task band ->
    overlays are real blockers the guards do not cause; deprioritise W6.C1.

Refuses (exit 2) on: a non-primary judge, a chrome_ver other than $MANUK_CHROME_PIN, or a missing
(task, run) key on either arm. An incomplete table never yields a verdict.

  python3 -I bench/agent-tasks/w6x1_tally.py
  python3 -I bench/agent-tasks/w6x1_tally.py --mutate N   # gate mutant: flips N mutant runs (must turn verdict/exit red)
  python3 -I bench/agent-tasks/w6x1_tally.py --self-test  # refusals and the verdict rule can go red
"""
import argparse, csv, json, os, sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "../.."))
MUT = os.path.join(ROOT, "bench-queue/out/w6x1-occlusion-mutant.tsv")
UNM = os.path.join(ROOT, "bench-queue/out/w6b0-cdplayer-real.tsv")
SPEC = os.path.join(ROOT, "bench-queue/specs/w6x1-occlusion-mutant.json")
JUDGE = "openai/gpt-oss-120b"
RUNS = 3
TASKS = ["topwork-1", "topwork-2", "topwork-3", "topwork-4", "fly-unified-2", "fly-unified-4", "fly-unified-5",
         "fly-unified-9", "dashdish-2", "dashdish-4", "dashdish-5", "staynb-6", "staynb-8", "zilloft-3", "zilloft-6"]


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


def verdict(mut, unm, tasks=TASKS, runs=RUNS):
    per = {}
    for t in tasks:
        m = sum(mut[(t, k)] for k in range(1, runs + 1))
        u = sum(unm[(t, k)] for k in range(1, runs + 1))
        per[t] = {"mut_runs": m, "unm_runs": u, "mut_pass": int(m >= 2), "unm_pass": int(u >= 2),
                  "moved": abs(m - u) >= 2}
    moved = [t for t in tasks if per[t]["moved"]]
    up = [t for t in moved if per[t]["mut_runs"] > per[t]["unm_runs"]]
    if len(moved) >= 2 and len(up) == len(moved):
        call = "GUARDS_BLOCK_PASSES: keep W6.C1 prioritised"
    elif len(moved) <= 1:
        call = "INSIDE_BAND: overlays are real blockers; deprioritise W6.C1"
    else:
        call = "MIXED: not decided by the pre-registered rule; report and escalate"
    return per, moved, call


def run(mut_rows, unm_rows, out=sys.stdout):
    mut = table(mut_rows, label="mutant")
    unm = table(unm_rows, label="unmutated")
    per, moved, call = verdict(mut, unm)
    print("task\tmut_runs\tunm_runs\tmut_pass2of3\tunm_pass2of3\tmoved", file=out)
    for t in TASKS:
        p = per[t]
        print(f"{t}\t{p['mut_runs']}\t{p['unm_runs']}\t{p['mut_pass']}\t{p['unm_pass']}\t{int(p['moved'])}", file=out)
    print(f"total_runs\tmut={sum(p['mut_runs'] for p in per.values())}/45\tunm={sum(p['unm_runs'] for p in per.values())}/45",
          file=out)
    print(f"task_pass_2of3\tmut={sum(p['mut_pass'] for p in per.values())}/15\tunm={sum(p['unm_pass'] for p in per.values())}/15",
          file=out)
    print(f"moved={len(moved)} {moved}", file=out)
    print(f"VERDICT: {call}", file=out)
    return call


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--mutate", type=int, default=0, help="gate mutant: flip N mutant runs")
    ap.add_argument("--self-test", action="store_true")
    a = ap.parse_args()
    if a.self_test:
        return self_test()
    mut_rows, unm_rows = load(MUT), load(UNM)
    if a.mutate:
        n = 0
        for r in mut_rows:
            if r.get("task") in TASKS and r.get("pass") in ("0", "1") and n < a.mutate:
                r["pass"] = str(1 - int(r["pass"])); n += 1
        print(f"[mutate] flipped {n} mutant runs", file=sys.stderr)
    run(mut_rows, unm_rows)
    return 0


def _synthetic(mut_pat, unm_pat):
    def rows(pat, arm):
        out = []
        for i, t in enumerate(TASKS):
            for k in range(1, RUNS + 1):
                out.append({"task": t, "run": str(k), "pass": str(pat[i][k - 1]), "judge_model": JUDGE,
                            "chrome_ver": os.environ["MANUK_CHROME_PIN"]})
        return out
    return rows(mut_pat, "mut"), rows(unm_pat, "unm")


def self_test():
    os.environ.setdefault("MANUK_CHROME_PIN", "145.0.7632.116")
    fails = 0

    def expect(name, cond):
        nonlocal fails
        print(("ok   " if cond else "RED  ") + name)
        fails += 0 if cond else 1

    same = [[1, 1, 0]] * 15
    m, u = _synthetic(same, same)
    expect("identical arms -> INSIDE_BAND", verdict(table(m), table(u))[2].startswith("INSIDE_BAND"))
    # gate must be able to go red: 3 tasks gain 3 runs under the mutant -> GUARDS_BLOCK_PASSES
    mp = [[1, 1, 1]] * 3 + [[1, 1, 0]] * 12
    up = [[0, 0, 0]] * 3 + [[1, 1, 0]] * 12
    m, u = _synthetic(mp, up)
    expect("3 upward moves -> GUARDS_BLOCK_PASSES", verdict(table(m), table(u))[2].startswith("GUARDS_BLOCK"))
    # refusal: wrong chrome pin
    bad = _synthetic(same, same)[0]
    for r in bad:
        r["chrome_ver"] = "144.0.0.0"
    try:
        table(bad, label="mutant"); expect("wrong chrome_ver refuses", False)
    except Refuse:
        expect("wrong chrome_ver refuses", True)
    # refusal: missing key
    short = _synthetic(same, same)[0][:-1]
    try:
        table(short, label="mutant"); expect("missing (task,run) refuses", False)
    except Refuse:
        expect("missing (task,run) refuses", True)
    # refusal: non-primary judge
    jb = _synthetic(same, same)[0]
    jb[0]["judge_model"] = "other/judge"
    try:
        table(jb, label="mutant"); expect("non-primary judge refuses", False)
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
