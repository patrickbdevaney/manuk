#!/usr/bin/env python3
"""RG.B tally: manuk's REAL re-bench at HEAD against the W5.B1 Playwright baseline.

Reads REAL rows (real_run.py TSV format), keeps rows with pass in {0,1} (judge_unavailable rows are
counted and reported, never as PASS/FAIL), and reports per-run pass totals, per-task pass>=2 of 3, and the
WIN / PARITY / GAP verdict against the Playwright band (W5.B1 3 runs + the RG.B drift-control run).

  python3 -I bench/agent-tasks/rgb_tally.py                  # report
  python3 -I bench/agent-tasks/rgb_tally.py --mutate TASK    # flips TASK's rows to pass=1 (gate mutant)
  python3 -I bench/agent-tasks/rgb_tally.py --self-test      # the verdict must change under the mutant
  python3 -I bench/agent-tasks/rgb_tally.py --round B2       # RG.2.B: reads docs/sprint/rg/B2-real-*.tsv
"""
import argparse, collections, csv, glob, os, sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "../.."))
RG = os.path.join(ROOT, "docs/sprint/rg")
W5 = os.path.join(ROOT, "docs/sprint/w5")
JUDGE = "openai/gpt-oss-120b"
PREFIX = "B"  # the round's TSV prefix: B-real-* (RG.B) or B2-real-* (RG.2.B); set by --round


def load(paths):
    rows = []
    for p in paths:
        if not os.path.exists(p):
            continue
        with open(p, newline="") as f:
            rows += list(csv.DictReader(f, delimiter="\t"))
    return rows


def passes(rows, mutate=None):
    """Return {(task, run): 0|1} for scored rows; excluded rows are counted separately."""
    scored, excluded = {}, 0
    for r in rows:
        if r["pass"] not in ("0", "1"):
            excluded += 1
            continue
        v = int(r["pass"])
        if mutate and r["task"] == mutate:
            v = 1
        scored[(r["task"], int(r["run"]))] = v
    return scored, excluded


def per_run(scored):
    tot = collections.Counter()
    for (_, run), v in scored.items():
        tot[run] += v
    return [tot[k] for k in sorted(tot)]


def per_task(scored):
    by = collections.defaultdict(list)
    for (task, _), v in scored.items():
        by[task].append(v)
    return {t: sum(vs) >= 2 for t, vs in by.items()}


def verdict(manuk_runs, pw_runs):
    lo_m, hi_m = min(manuk_runs), max(manuk_runs)
    lo_p, hi_p = min(pw_runs), max(pw_runs)
    if lo_m > hi_p:
        return "WIN"
    if hi_m >= lo_p and hi_p >= lo_m:
        return "PARITY"
    return "GAP"


def report(mutate=None, quiet=False):
    manuk_rows = load(sorted(glob.glob(os.path.join(RG, f"{PREFIX}-real-manuk*.tsv"))))
    pw_rows = load(sorted(glob.glob(os.path.join(RG, f"{PREFIX}-real-playwright*.tsv"))))
    b1_pw = load([os.path.join(W5, "B1-real-playwright.tsv")])
    b1_m = load([os.path.join(W5, "B1-real-manuk.tsv")])
    for rows in (manuk_rows, pw_rows):
        bad = {r["judge_model"] for r in rows if r["judge_model"] and r["judge_model"] != JUDGE}
        if bad:
            raise SystemExit(f"judge_model not {JUDGE}: {sorted(bad)}")
    chrome = {r["chrome_ver"] for r in manuk_rows + pw_rows}
    if chrome - {os.environ.get("MANUK_CHROME_PIN", "")}:
        raise SystemExit(f"chrome_ver mismatch: {sorted(chrome)}")

    m, m_ex = passes(manuk_rows, mutate)
    p, p_ex = passes(pw_rows)
    bp, _ = passes(b1_pw)
    bm, _ = passes(b1_m)
    m_runs, p_runs = per_run(m), per_run(p)
    b1p_runs, b1m_runs = per_run(bp), per_run(bm)
    m_task, p_task = per_task(m), per_task(p)
    b1p_task, b1m_task = per_task(bp), per_task(bm)
    tasks = sorted(set(m_task) | set(b1p_task))
    gap = [t for t in tasks if b1p_task.get(t) and not m_task.get(t)]
    ahead = [t for t in tasks if b1m_task.get(t) is not None and m_task.get(t) and not b1p_task.get(t)]
    # the band is the Playwright per-run spread of W5.B1 plus the RG.B drift control
    pw_band = b1p_runs + p_runs
    v = verdict(m_runs, pw_band) if m_runs and pw_band else "INCOMPLETE"
    out = {
        "verdict": v, "manuk_runs": m_runs, "playwright_band": pw_band, "b1_playwright_runs": b1p_runs,
        "b1_manuk_runs": b1m_runs, "manuk_task_pass2of3": sum(m_task.values()), "tasks_scored": len(m_task),
        "excluded_rows": m_ex + p_ex, "gap_tasks": gap, "ahead_tasks": ahead,
        "playwright_drift_pass2of3": sum(p_task.values()),
    }
    if not quiet:
        for k, val in out.items():
            print(f"{k}: {val}")
    return out


def self_test():
    # mutant: force one manuk task that currently fails (<2 of 3) to pass in every run; the tally must move by 1
    base = report(quiet=True)
    manuk_rows = load(sorted(glob.glob(os.path.join(RG, f"{PREFIX}-real-manuk*.tsv"))))
    scored, _ = passes(manuk_rows)
    failing = sorted(t for t, ok in per_task(scored).items() if not ok)
    if not failing:
        raise SystemExit("self-test: no failing manuk task to mutate")
    mutant = report(mutate=failing[0], quiet=True)
    moved = mutant["manuk_task_pass2of3"] - base["manuk_task_pass2of3"]
    if moved != 1:
        print(f"self-test RED: mutant on {failing[0]} moved pass2of3 by {moved}, expected 1")
        return 1
    print(f"self-test GREEN: mutant on {failing[0]} moved pass2of3 {base['manuk_task_pass2of3']} -> {mutant['manuk_task_pass2of3']}")
    return 0


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--mutate")
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--round", default="B", choices=["B", "B2"])
    a = ap.parse_args()
    PREFIX = a.round
    if a.self_test:
        sys.exit(self_test())
    report(mutate=a.mutate)
