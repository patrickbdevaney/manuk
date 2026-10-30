#!/usr/bin/env python3
"""W3.18.a.a gate 2: score the REAL deterministic subset (real_run.py rows) per arm, and the rollback branch.

  real_report.py [--tsv docs/sprint/w318/real-runs.tsv] [--runs 3]

Per arm: per-run pass counts (the 3-run band = min..max), and tasks passing >=2 of 3 runs (the SUCCESS.md pass rule).
Rollback branch (SUCCESS.md Bar 2.2, same task list, same model):
  N = tasks that fail the pass rule on the manuk arm (no rollback tools named);
  M = those that pass the rule on manuk+rollback.v2 AND whose passing runs include one that called restore
      (a pass that never restored is not a rollback win).
GREEN iff M/N >= 15% AND the rollback arm's per-run pass band lies wholly above the manuk band (outside the band).
Exit 0 green, 1 red, 2 incomplete (an arm x task lacks --runs rows). Rows must share one chrome_ver.
Run with `python3 -I`.
"""
import argparse, collections, csv, os, sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
ARMS = ["manuk", "manuk+rollback.v2", "cdp-layer", "playwright"]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--tsv", default=os.path.join(ROOT, "docs/sprint/w318/real-runs.tsv"))
    ap.add_argument("--runs", type=int, default=3)
    a = ap.parse_args()
    rows = list(csv.DictReader(open(a.tsv), delimiter="\t"))
    vers = {r["chrome_ver"] for r in rows}
    if len(vers) != 1:
        sys.exit(f"refusing: rows span chrome_ver {sorted(vers)}")
    g = collections.defaultdict(lambda: collections.defaultdict(dict))  # arm -> task -> run -> row
    for r in rows:
        g[r["arm"]][r["task"]][int(r["run"])] = r
    tasks = sorted(set.intersection(*(set(g[x]) for x in ARMS if x in g)))
    runs = range(1, a.runs + 1)
    incomplete = [(x, t) for x in ARMS for t in tasks if len(g[x][t]) < a.runs]
    passed = lambda x, t, k: int(g[x][t].get(k, {}).get("pass", 0) or 0)
    rule = lambda x, t: sum(passed(x, t, k) for k in runs) >= 2
    print(f"suite real-v1-det  chrome_ver={vers.pop()}  tasks={len(tasks)} (jmespath-only evals, omnizon excluded)")
    print("arm\tper-run pass\tband\tpass>=2/3\tstate_ok\tharness_err\tmean in+out tok\tmean cost")
    band = {}
    for x in ARMS:
        if x not in g:
            continue
        pr = [sum(passed(x, t, k) for t in tasks) for k in runs]
        band[x] = (min(pr), max(pr))
        rs = [g[x][t][k] for t in tasks for k in runs if k in g[x][t]]
        tok = sum(int(r["input_tokens"] or 0) + int(r["output_tokens"] or 0) for r in rs) / max(len(rs), 1)
        cost = sum(float(r["cost_usd"] or 0) for r in rs) / max(len(rs), 1)
        print(f"{x}\t{pr}\t{band[x][0]}-{band[x][1]}\t{sum(rule(x, t) for t in tasks)}/{len(tasks)}\t"
              f"{sum(int(r['state_ok']) for r in rs)}/{len(rs)}\t{sum(int(r['is_error']) for r in rs)}\t{tok:.0f}\t{cost:.3f}")
    if incomplete:
        print(f"INCOMPLETE: {len(incomplete)} arm x task cells lack {a.runs} runs, e.g. {incomplete[:5]}")
        sys.exit(2)
    rb = "manuk+rollback.v2"
    failed = [t for t in tasks if not rule("manuk", t)]
    flipped = [t for t in failed if rule(rb, t)]
    counted = [t for t in flipped if any(passed(rb, t, k) and int(g[rb][t][k]["restores"]) > 0 for k in runs)]
    rr = [g[rb][t][k] for t in tasks for k in runs]
    print(f"rollback arm capability use: runs calling checkpoint {sum(int(r['checkpoints']) > 0 for r in rr)}/{len(rr)}, "
          f"restore {sum(int(r['restores']) > 0 for r in rr)}/{len(rr)}, fork {sum(int(r['forks']) > 0 for r in rr)}/{len(rr)}")
    print(f"N (manuk fails the 2/3 rule) = {len(failed)}")
    print(f"flipped to pass on {rb} = {len(flipped)} {flipped}")
    print(f"M (flipped AND a passing run called restore) = {len(counted)} {counted}")
    frac = len(counted) / len(failed) if failed else 0.0
    outside = band[rb][0] > band["manuk"][1]
    print(f"M/N = {len(counted)}/{len(failed)} = {100 * frac:.1f}% (needs >= 15%); "
          f"band {rb} {band[rb][0]}-{band[rb][1]} vs manuk {band['manuk'][0]}-{band['manuk'][1]}: "
          f"{'outside' if outside else 'overlapping'}")
    green = frac >= 0.15 and outside
    print("GATE 2 GREEN" if green else "GATE 2 RED")
    sys.exit(0 if green else 1)


if __name__ == "__main__":
    main()
