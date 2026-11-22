#!/usr/bin/env python3
"""W6.B1: does the driving model change the arm ordering on the REAL-fast subset?

Two drivers, same subset (fast_subset.SUBSET, 17 tasks), same HEAD, same pinned Chrome, same primary judge:
  haiku  : the default driver (run_arm.py `--model haiku`)
  sonnet : claude-sonnet-5-5 (MANUK_DRIVER_MODEL=claude-sonnet-5-5), the owner-delegated driver arm
Arms: manuk, cdp-layer, playwright. Inputs are the queue outputs bench-queue/out/w6b1-<driver>-<arm>.tsv.

Pre-registered rule (docs/sprint/w6/B1.md, written before any sonnet row existed):
  M(arm, driver) = mean over the 3 runs of the per-run subset pass count (0..17).
  T = 2 tasks (the smallest move W6.F1 names as resolvable).
  A pair of arms is DECISIVE under a driver when |M(A) - M(B)| >= T.
  FLIP = a pair decisive under BOTH drivers with opposite sign, or a different rank-1 arm under the two drivers.
  Falsifier (W6.B1 gate): no FLIP -> the sonnet arm does not change the ordering -> not needed per commit.
  Any FLIP -> the sonnet arm stays in the per-commit plan.

Refuses (exit 2) on: a chrome_ver other than $MANUK_CHROME_PIN, a driver column that is not the expected model id,
a non-primary judge row, or a missing (task, run) key. Only a complete, correctly labelled table yields a verdict.

  python3 -I bench/agent-tasks/b1_driver.py                    # the verdict from bench-queue/out/w6b1-*.tsv
  python3 -I bench/agent-tasks/b1_driver.py --self-test        # a flip mutant must go red; broken inputs must refuse
"""
import copy, csv, itertools, os, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "../.."))
sys.path.insert(0, HERE)
from fast_subset import SUBSET  # the 17 REAL-fast tasks (W6.F1), frozen there

JUDGE = "openai/gpt-oss-120b"
ARMS = ["manuk", "cdp-layer", "playwright"]
DRIVERS = {"haiku": "haiku", "sonnet": "claude-sonnet-5-5"}  # short name -> model column value
RUNS = 3
T = 2


class Refuse(Exception):
    pass


def path(driver, arm):
    return os.path.join(ROOT, f"bench-queue/out/w6b1-{driver}-{arm}.tsv")


def load(p):
    if not os.path.exists(p):
        raise Refuse(f"missing table {os.path.relpath(p, ROOT)}")
    with open(p, newline="") as f:
        return list(csv.DictReader(f, delimiter="\t"))


def table(rows, model, tasks=SUBSET, runs=RUNS):
    pin = os.environ.get("MANUK_CHROME_PIN", "")
    cv = sorted({r["chrome_ver"] for r in rows})
    if not pin or cv != [pin]:
        raise Refuse(f"chrome_ver {cv} != pin {pin!r}")
    md = sorted({r["model"] for r in rows})
    if md != [model]:
        raise Refuse(f"model column {md} != {model!r}")
    bad = sorted({r["judge_model"] for r in rows if r.get("judge_model") not in ("", JUDGE)})
    if bad:
        raise Refuse(f"non-primary judge rows: {bad}")
    got = {(r["task"], int(r["run"])): int(r["pass"]) for r in rows if r.get("pass") in ("0", "1")}
    missing = [(t, k) for t in tasks for k in range(1, runs + 1) if (t, k) not in got]
    if missing:
        raise Refuse(f"{len(missing)} (task,run) keys unscored, e.g. {missing[:3]}")
    return got


def metric(got, tasks=SUBSET, runs=RUNS):
    per_run = [sum(got[(t, k)] for t in tasks) for k in range(1, runs + 1)]
    pass2 = sum(1 for t in tasks if sum(got[(t, k)] for k in range(1, runs + 1)) >= 2)
    return {"per_run": per_run, "mean": sum(per_run) / runs, "task_pass_2of3": pass2}


def gather(paths=None):
    """paths: {(driver, arm): tsv}. Returns {driver: {arm: metric}} after every refusal check."""
    out = {}
    for d, model in DRIVERS.items():
        out[d] = {}
        for a in ARMS:
            p = (paths or {}).get((d, a), path(d, a))
            rows = load(p) if isinstance(p, str) else p
            out[d][a] = metric(table(rows, model))
    return out


def verdict(res, T=T):
    flips = []
    for a, b in itertools.combinations(ARMS, 2):
        gh = res["haiku"][a]["mean"] - res["haiku"][b]["mean"]
        gs = res["sonnet"][a]["mean"] - res["sonnet"][b]["mean"]
        if abs(gh) >= T and abs(gs) >= T and (gh > 0) != (gs > 0):
            flips.append(f"{a} vs {b}: haiku {gh:+.1f}, sonnet {gs:+.1f}")
    top = {d: max(ARMS, key=lambda a: res[d][a]["mean"]) for d in DRIVERS}
    if top["haiku"] != top["sonnet"]:
        flips.append(f"rank-1 arm differs: haiku={top['haiku']} sonnet={top['sonnet']}")
    return ("FLIP" if flips else "SAME"), flips


def report(res):
    for d in DRIVERS:
        for a in ARMS:
            m = res[d][a]
            print(f"{d:6s} {a:10s} per_run={m['per_run']} mean={m['mean']:.2f} task_pass_2of3={m['task_pass_2of3']}/{len(SUBSET)}")
    v, flips = verdict(res)
    for f in flips:
        print("  flip:", f)
    print(f"verdict: {v} (T={T} tasks; {'sonnet arm changes the ordering -> keep per commit' if v == 'FLIP' else 'ordering identical within band -> driver arm not needed per commit'})")
    return v


def self_test():
    fails = []
    # a healthy table for each (driver, arm): haiku and sonnet agree -> SAME
    rows = {}
    for d, model in DRIVERS.items():
        for a in ARMS:
            rs = []
            npass = 4 + 4 * ARMS.index(a)  # arm i passes the first 4+4i tasks in every run: gaps of 4 tasks
            for k in range(1, RUNS + 1):
                for t in SUBSET:
                    rs.append({"task": t, "run": str(k), "pass": "1" if t in SUBSET[:npass] else "0", "model": model,
                               "chrome_ver": os.environ.get("MANUK_CHROME_PIN", ""), "judge_model": JUDGE})
            rows[(d, a)] = rs
    base = gather(rows)
    v0, _ = verdict(base)
    if v0 != "SAME":
        fails.append(f"identical drivers gave {v0}")
    # mutant: sonnet swaps the ordering of two arms by 4 tasks per run -> must be FLIP
    ranked = sorted(ARMS, key=lambda a: base["haiku"][a]["mean"])
    lo, hi = ranked[0], ranked[-1]
    mut = copy.deepcopy(rows)
    for r in mut[("sonnet", lo)]:
        r["pass"] = "1"  # the weakest arm passes every task
    for r in mut[("sonnet", hi)]:
        r["pass"] = "0"  # the strongest arm fails every task
    v1, flips = verdict(gather(mut))
    if v1 != "FLIP":
        fails.append(f"mutant swap {lo}/{hi} did not flip (got {v1})")
    # refusals: foreign chrome_ver, non-primary judge, wrong driver label, missing key
    cases = {
        "chrome drift": lambda rs: [r.__setitem__("chrome_ver", "146.0.0.0") for r in rs],
        "fallback judge": lambda rs: rs[0].__setitem__("judge_model", "claude-haiku (claude -p)"),
        "wrong driver label": lambda rs: rs[0].__setitem__("model", "haiku"),
        "missing row": lambda rs: rs.pop(),
    }
    for name, edit in cases.items():
        m = copy.deepcopy(rows)
        edit(m[("sonnet", "manuk")])
        try:
            gather(m)
            fails.append(f"{name}: not refused")
        except Refuse:
            pass
    if fails:
        for f in fails:
            print("SELF-TEST FAIL:", f)
        return 1
    print("self-test PASS: identical -> SAME; swap mutant -> FLIP; chrome drift, fallback judge, wrong driver label, missing row -> refused")
    return 0


def main():
    if "--self-test" in sys.argv:
        sys.exit(self_test())
    try:
        res = gather()
    except Refuse as e:
        print("REFUSED:", e)
        sys.exit(2)
    report(res)


if __name__ == "__main__":
    main()
