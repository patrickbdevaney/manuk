#!/usr/bin/env python3
"""W6.F1: does the REAL-fast subset (12 GAP + 5 AHEAD from docs/sprint/rg/B2.md) reproduce the full-102 picture?

  python3 -I bench/agent-tasks/fast_subset.py             # report + verdict (exit 1 when the falsifier fires)
  python3 -I bench/agent-tasks/fast_subset.py --self-test # a mutant subset must turn the agreement check red
  python3 -I bench/agent-tasks/fast_subset.py --spec      # print the per-commit x1 queue spec for the subset

Units are every (arm-round, run) with a full-102 pass total and a subset total. Rank agreement = pairwise concordance of
the two totals over all unit pairs where both differ. Sign check = RG.B -> RG.2.B manuk change in per-run mean, subset vs full.
Resolution check: sd of the subset total across runs of one arm-round; a x1 paired diff has sd*sqrt(2); a 2-task flip must be >= 2 sd of that.
Caveat printed in the report: GAP/AHEAD were defined from W5.B1 vs RG.2.B, so agreement on those rounds is partly by construction.
"""
import csv, collections, itertools, json, os, statistics, sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "../.."))
S = {'W5.B1 manuk': 'docs/sprint/w5/B1-real-manuk.tsv', 'W5.B1 playwright': 'docs/sprint/w5/B1-real-playwright.tsv',
     'W5.B1 cdp-layer': 'docs/sprint/w5/B1-real-cdp-layer.tsv', 'RG.B manuk': 'docs/sprint/rg/B-real-manuk.tsv',
     'RG.B playwright': 'docs/sprint/rg/B-real-playwright.tsv', 'RG.2.B manuk': 'docs/sprint/rg/B2-real-manuk.tsv',
     'RG.2.B playwright': 'docs/sprint/rg/B2-real-playwright.tsv', 'W6.B0 cdp-layer': 'bench-queue/out/w6b0-cdplayer-real.tsv'}
GAP = "gocalendar-2 gomail-3 gomail-5 opendining-10 opendining-4 opendining-7 staynb-4 udriver-10 udriver-7 udriver-9 zilloft-2 zilloft-5".split()
AHEAD = "dashdish-3 gomail-8 networkin-3 opendining-2 opendining-8".split()
SUBSET = GAP + AHEAD  # the 15 dialog_modal tasks are left out: W6.X1 showed them flat (0-1 of 15 in every arm), they add no discrimination


def load():
    D = {}
    for k, p in S.items():
        d = collections.defaultdict(dict)
        with open(os.path.join(ROOT, p), newline="") as f:
            for r in csv.DictReader(f, delimiter="\t"):
                if r["pass"] in ("0", "1"):
                    d[r["task"]][int(r["run"])] = int(r["pass"])
        D[k] = d
    return D


def units(D, sub):
    U = []
    for k, d in D.items():
        for r in sorted({r for t in d for r in d[t]}):
            U.append((k, r, sum(d[t].get(r, 0) for t in d), sum(d[t].get(r, 0) for t in sub)))
    return U


def concordance(U):
    c = dc = 0
    for a, b in itertools.combinations(U, 2):
        if a[2] != b[2] and a[3] != b[3]:
            if (a[2] - b[2]) * (a[3] - b[3]) > 0: c += 1
            else: dc += 1
    return c / (c + dc) if c + dc else 0.0, c, dc


def evaluate(D, sub):
    U = units(D, sub)
    conc, c, dc = concordance(U)
    mean = lambda k, i: statistics.mean(u[i] for u in U if u[0] == k)
    full_d, sub_d = mean('RG.2.B manuk', 2) - mean('RG.B manuk', 2), mean('RG.2.B manuk', 3) - mean('RG.B manuk', 3)
    sign_ok = (full_d > 0) == (sub_d > 0) or full_d == 0
    sds = [statistics.pstdev([u[3] for u in U if u[0] == k]) for k in D if sum(1 for u in U if u[0] == k) > 1]
    sd_diff = statistics.mean(sds) * 2 ** 0.5
    can_see_flip = 2 >= 2 * sd_diff
    return dict(concordance=conc, concordant=c, discordant=dc, full_delta=full_d, subset_delta=sub_d, sign_ok=sign_ok,
                mean_subset_sd=statistics.mean(sds), sd_paired_diff_x1=sd_diff, can_see_2task_flip=can_see_flip, units=U)


def verdict(e):
    return e["concordance"] >= 0.8 and e["sign_ok"] and e["can_see_2task_flip"]


def main():
    D = load()
    if "--spec" in sys.argv:
        base = ["env", "MANUK_MCP_BIN=target/release/manuk-mcp", "python3", "-I", "bench/agent-tasks/real_run.py", "run", "--arm", "manuk",
                "--runs", "1", "--run-base", "0", "--judge", "--jobs", "2", "--tasks", ",".join(sorted(SUBSET)),
                "--out", "bench-queue/out/real-fast-HEAD.tsv", "--transcripts", os.path.expanduser("~/.cache/manuk-bq/transcripts-realfast")]
        print(json.dumps({"id": "real-fast", "note": "W6.F1 REAL-fast: 17 tasks x1; enqueue once per commit (parent then HEAD, paired). UNDERPOWERED for 2-task flips, see docs/sprint/w6/F1.md",
                          "inputs": ["target/release/manuk-mcp", "bench/agent-tasks/real_run.py", "bench/agent-tasks/run_arm.py"],
                          "steps": [{"name": "real-fast x1", "argv": base}],
                          "reconcile": {"tsv_glob": "bench-queue/out/real-fast-HEAD*.tsv", "primary_judge": "openai/gpt-oss-120b"},
                          "band": {"tsv_glob": "bench-queue/out/real-fast-HEAD.tsv", "runs": 1, "tasks": sorted(SUBSET), "task_pass_2of3": [0, 17], "per_run": [0, 17]}}, indent=1))
        return 0
    if "--self-test" in sys.argv:
        good = evaluate(D, SUBSET)
        # mutant: the 17 tasks the full set solves most reliably in EVERY arm (non-discriminating) instead of GAP+AHEAD
        sc = collections.Counter()
        for d in D.values():
            for t, v in d.items(): sc[t] += sum(v.values())
        mut = [t for t, _ in sc.most_common(17)]
        bad = evaluate(D, mut)
        print(f"real subset concordance {good['concordance']:.3f}; mutant (17 always-pass tasks) {bad['concordance']:.3f}")
        ok = bad["concordance"] < good["concordance"] and bad["concordance"] < 0.8
        print("self-test", "GREEN" if ok else "RED (mutant not detected)")
        return 0 if ok else 1
    e = evaluate(D, SUBSET)
    for u in e["units"]: print("unit", *u)
    print(f"rank agreement {e['concordance']:.3f} ({e['concordant']} concordant / {e['discordant']} discordant), bar >=0.8")
    print(f"RG.B->RG.2.B manuk per-run mean: full {e['full_delta']:+.2f}, subset {e['subset_delta']:+.2f}, sign_ok={e['sign_ok']}")
    print(f"subset sd across runs {e['mean_subset_sd']:.2f}; x1 paired-diff sd {e['sd_paired_diff_x1']:.2f}; 2-task flip visible (2 >= 2sd): {e['can_see_2task_flip']}")
    ok = verdict(e)
    print("verdict:", "subset USABLE" if ok else "FALSIFIER FIRES (subset not usable as a per-commit gate)")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
