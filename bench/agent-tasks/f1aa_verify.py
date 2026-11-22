#!/usr/bin/env python3
"""W6.F1.a.a: confirm the N=14 REAL-fast subset on FRESH rounds at one commit (chrome_ver pinned).

Subset = first 14 of fast_subset2.order() on the SELECTION rounds only (rule fixed in W6.F1.a; recomputed here, not copied).
Fresh rounds (written by bench-queue spec w6f1aa): manuk x3 runs and playwright x3 runs, all 102 judged tasks each,
primary judge only.

Gate (pre-registered in docs/sprint/GRAPH.tsv row W6.F1.a.a; falsifier = any clause fails):
  (a) rank:  pairwise concordance over the 6 (arm,run) units of subset total vs full-102 total >= 0.8
  (b) sign:  sign of (manuk - playwright) on the run-mean agrees between subset and full-102
  (c) sd:    every arm's per-round sd (pstdev over its 3 runs) of the subset total <= 0.7
Exit: 0 = PASS, 1 = gate red (falsifier fires), 2 = refused (missing rows, wrong chrome_ver/judge, duplicates).

--self-test: (1) a mutant subset of the 14 most extreme-rate tasks on the fresh data must be RED;
             (2) a TSV with one row dropped must be REFUSED (exit 2 path raises).
"""
import csv, itertools, os, statistics, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "../.."))
sys.path.insert(0, HERE)
import fast_subset2 as fs  # noqa: E402  (rule + selection rounds live there; -I is used when invoked)

ARMS = {"manuk": "bench-queue/out/w6f1aa-manuk.tsv", "playwright": "bench-queue/out/w6f1aa-playwright.tsv"}
RUNS = (1, 2, 3)  # real_run.py writes run = run_base + r, r in 1..runs (1-based)
JUDGE = "openai/gpt-oss-120b"
PIN = os.environ.get("MANUK_CHROME_PIN", "145.0.7632.116")
SPEC = os.path.join(ROOT, "bench-queue/specs/w6f1aa.json")
SD_MAX = 0.7
CONC_MIN = 0.8


class Refused(Exception):
    pass


def judged_tasks():
    import json
    return json.load(open(SPEC))["band"]["tasks"]


def load_arm(path, tasks):
    full = os.path.join(ROOT, path)
    if not os.path.exists(full):
        raise Refused(f"missing {path}")
    cells = {}
    for r in csv.DictReader(open(full, newline=""), delimiter="\t"):
        if r["pass"] not in ("0", "1"):
            continue
        if r["chrome_ver"] != PIN:
            raise Refused(f"{path}: chrome_ver {r['chrome_ver']} != pin {PIN}")
        if r.get("judge_model") not in ("", JUDGE):
            raise Refused(f"{path}: judge {r['judge_model']} != primary {JUDGE}")
        key = (r["task"], int(r["run"]))
        if key in cells and cells[key] != int(r["pass"]):
            raise Refused(f"{path}: conflicting duplicate {key}")
        cells[key] = int(r["pass"])
    for run in RUNS:
        for t in tasks:
            if (t, run) not in cells:
                raise Refused(f"{path}: missing row task={t} run={run} (have {len(cells)})")
    return cells


def totals(cells, tasks, sub):
    return {run: sum(cells[(t, run)] for t in sub if t in tasks) for run in RUNS}


def evaluate(data, tasks, sub):
    units = []
    for arm in ARMS:
        f, s = totals(data[arm], tasks, tasks), totals(data[arm], tasks, sub)
        for run in RUNS:
            units.append((arm, run, f[run], s[run]))
    c = dc = 0
    for a, b in itertools.combinations(units, 2):
        if a[2] != b[2] and a[3] != b[3]:
            if (a[2] - b[2]) * (a[3] - b[3]) > 0:
                c += 1
            else:
                dc += 1
    conc = c / (c + dc) if c + dc else 0.0
    mean = lambda arm, idx: statistics.mean(u[idx] for u in units if u[0] == arm)
    d_full = mean("manuk", 2) - mean("playwright", 2)
    d_sub = mean("manuk", 3) - mean("playwright", 3)
    sign_ok = (d_full > 0) == (d_sub > 0) and (d_full != 0) == (d_sub != 0)
    sds = {arm: statistics.pstdev([u[3] for u in units if u[0] == arm]) for arm in ARMS}
    sd_full = {arm: statistics.pstdev([u[2] for u in units if u[0] == arm]) for arm in ARMS}
    ok = conc >= CONC_MIN and sign_ok and max(sds.values()) <= SD_MAX
    return dict(units=units, conc=conc, d_full=d_full, d_sub=d_sub, sign_ok=sign_ok, sds=sds, sd_full=sd_full, ok=ok)


def report(e, sub, label):
    print(f"[{label}] subset N={len(sub)}")
    for arm, run, f, s in e["units"]:
        print(f"  {arm:10s} run{run}: full-102={f:3d} subset={s:2d}")
    print(f"  (a) concordance = {e['conc']:.3f}  (need >= {CONC_MIN})  {'ok' if e['conc'] >= CONC_MIN else 'RED'}")
    print(f"  (b) sign manuk-playwright: full {e['d_full']:+.2f} subset {e['d_sub']:+.2f}  {'ok' if e['sign_ok'] else 'RED'}")
    print("  (c) per-round sd of subset totals: " + ", ".join(f"{a}={v:.2f}" for a, v in e["sds"].items())
          + f"  (need <= {SD_MAX}; full-102 sd for reference: " + ", ".join(f"{a}={v:.2f}" for a, v in e["sd_full"].items()) + ")")
    print(f"  verdict: {'PASS' if e['ok'] else 'RED (falsifier fires)'}")


def pooled_rates(data, tasks):
    n, s = {}, {}
    for arm in ARMS:
        for (t, _r), v in data[arm].items():
            if t in tasks:
                n[t] = n.get(t, 0) + 1
                s[t] = s.get(t, 0) + v
    return {t: s[t] / n[t] for t in n}


def self_test(data, tasks, subset):
    ok = True
    p = pooled_rates(data, tasks)
    mutant = sorted(p, key=lambda t: (-abs(p[t] - .5), t))[:len(subset)]
    e = evaluate(data, tasks, mutant)
    red = not e["ok"]
    print(f"self-test 1: mutant (14 most extreme-rate tasks) -> conc={e['conc']:.3f} sign_ok={e['sign_ok']} "
          f"max_sd={max(e['sds'].values()):.2f} -> {'red (good)' if red else 'PASS (BAD)'}")
    ok &= red
    # broken input: drop one row from a copy of the manuk table; load must refuse
    import tempfile
    src = open(os.path.join(ROOT, ARMS["manuk"]), newline="").read().splitlines()
    dropped = [src[0]] + src[2:]  # drop the first data row
    tmp = tempfile.mkdtemp(prefix="f1aa-selftest-")
    p2 = os.path.join(tmp, "dropped.tsv")
    open(p2, "w").write("\n".join(dropped) + "\n")
    try:
        load_arm(p2, tasks)  # load_arm joins ROOT with an absolute path, which yields p2
        refused = False
    except Refused as ex:
        refused = True
        print(f"self-test 2: dropped row -> refused: {ex}")
    finally:
        import shutil
        shutil.rmtree(tmp, ignore_errors=True)
    print(f"self-test 2: dropped row -> {'refused (good)' if refused else 'LOADED (BAD)'}")
    ok &= refused
    print("SELF-TEST", "GREEN" if ok else "RED")
    return 0 if ok else 1


def main():
    tasks = judged_tasks()
    sel = fs.load(fs.SEL)
    sub = fs.order(fs.rates(sel))[:14]
    print("subset (N=14, recomputed from selection rounds):", " ".join(sorted(sub)))
    try:
        data = {arm: load_arm(path, tasks) for arm, path in ARMS.items()}
    except Refused as ex:
        print(f"REFUSED: {ex}")
        return 2
    if "--self-test" in sys.argv:
        return self_test(data, tasks, sub)
    e = evaluate(data, tasks, sub)
    report(e, sub, "fresh rounds w6f1aa, same commit")
    return 0 if e["ok"] else 1


if __name__ == "__main__":
    sys.exit(main())
