#!/usr/bin/env python3
"""W4.B4 token re-measure report: Bar 2.2 tokens branch, read as docs/sprint/SUCCESS.md writes it.

  b4_token_report.py [--dir docs/sprint/w4/B4] [--out FILE]

Reads one TSV per arm (run_arm.py output, full suite = own 40 + external 10, 3 runs) and prints, per arm:
pass per run, the 3-run band, and tokens per task under THREE metrics, each named:
  in+out      = input_tokens + output_tokens          (the metric the earlier VERDICT used)
  total       = in+out + cache_read + cache_create     (everything billed as tokens)
  USD         = cost_usd                               (what Claude Code reported)
plus output tokens per task and turns per task. A per-task value is the mean over the 3 runs of the sum over that run's tasks.
Bar 2.2 tokens: manuk must use >= 30% fewer tokens/task than the BEST Playwright arm (the lower of playwright and
playwright+preload, per metric), with the manuk band entirely below the Playwright band, and manuk's full-suite pass
band must not be more than 10 points below Playwright's (equal or better success).
Every row is checked for chrome_ver == $MANUK_CHROME_PIN; other pins are refused, never mixed.
Run with `python3 -I`.
"""
import argparse, collections, csv, glob, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
PIN = os.environ.get("MANUK_CHROME_PIN")
ARMS = ["manuk", "cdp-layer", "playwright", "playwright+preload"]
PLAYWRIGHT = ["playwright", "playwright+preload"]


def num(r, k):
    return int(r[k] or 0)


METRICS = {
    "in+out": lambda r: num(r, "input_tokens") + num(r, "output_tokens"),
    "total": lambda r: num(r, "input_tokens") + num(r, "output_tokens") + num(r, "cache_read_tokens") + num(r, "cache_create_tokens"),
    "USD": lambda r: float(r["cost_usd"] or 0),
    "out": lambda r: num(r, "output_tokens"),
}


def load(d):
    arms = {}
    for arm in ARMS:
        path = os.path.join(d, arm.replace("+", "_") + ".tsv")
        if not os.path.exists(path):
            continue
        rows = list(csv.DictReader(open(path), delimiter="\t"))
        bad = [r for r in rows if r["chrome_ver"] != PIN]
        if bad:
            sys.exit(f"refusing: {path} has {len(bad)} rows at a chrome_ver other than {PIN}")
        # a re-run of the same (task, run) replaces the earlier row; the latest is the measurement
        last = {(r["task"], r["run"]): r for r in rows}
        arms[arm] = list(last.values())
    return arms


def per_run(rows, f):
    """{run: sum of f over that run's rows / number of tasks in that run}, i.e. a per-task mean for each run."""
    by = collections.defaultdict(list)
    for r in rows:
        by[r["run"]].append(r)
    return {k: sum(f(r) for r in v) / len(v) for k, v in sorted(by.items())}


def band(vals):
    return min(vals), max(vals)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dir", default=os.path.join(ROOT, "docs/sprint/w4/B4"))
    ap.add_argument("--out")
    a = ap.parse_args()
    if not PIN:
        sys.exit("refusing: MANUK_CHROME_PIN unset")
    arms = load(a.dir)
    L = []
    P = L.append
    P(f"chrome_ver={PIN}; suite = full (own 40 + external 10), Haiku, 3 runs; source {os.path.relpath(a.dir, ROOT)}/\n")
    P("| arm | tasks x runs | pass per run (of tasks) | pass band | tasks passing >=2/3 | turns/task | out tok/task | in+out tok/task (band) | total tok/task (band) | USD/task (band) |")
    P("|---|---|---|---|---|---|---|---|---|---|")
    stats = {}
    for arm, rows in arms.items():
        ntask = len({r["task"] for r in rows})
        passes = per_run(rows, lambda r: int(r["pass"]))
        pr = {k: v * ntask for k, v in passes.items()}  # passing tasks per run
        ok = collections.defaultdict(int)
        for r in rows:
            ok[r["task"]] += int(r["pass"])
        two = sum(1 for t in ok if ok[t] >= 2)
        pct = [100 * v for v in passes.values()]  # passes is already the per-task mean of each run
        s = {"pct": pct, "two": two, "n": ntask}
        for m, f in METRICS.items():
            vals = per_run(rows, f)
            s[m] = (sum(vals.values()) / len(vals), band(list(vals.values())))
        stats[arm] = s
        turns = per_run(rows, lambda r: int(r["turns"] or 0))
        t_mean = sum(turns.values()) / len(turns)
        P(f"| {arm} | {ntask} x {len(passes)} | " + ", ".join(f"{pr[k]:.0f}" for k in sorted(pr)) +
          f" | {min(pct):.1f}-{max(pct):.1f}% | {two}/{ntask} | {t_mean:.2f} | {s['out'][0]:,.0f} | "
          f"{s['in+out'][0]:,.0f} [{s['in+out'][1][0]:,.0f}, {s['in+out'][1][1]:,.0f}] | "
          f"{s['total'][0]:,.0f} [{s['total'][1][0]:,.0f}, {s['total'][1][1]:,.0f}] | "
          f"{s['USD'][0]:.4f} [{s['USD'][1][0]:.4f}, {s['USD'][1][1]:.4f}] |")
    P("\nBar 2.2 tokens branch (manuk vs the best Playwright arm, per metric):")
    pw = [x for x in PLAYWRIGHT if x in stats]
    if "manuk" not in stats or not pw:
        P("- NOT EVALUABLE: manuk or a Playwright arm has no rows")
    else:
        for m in METRICS:
            if m == "out":
                continue
            best = min(pw, key=lambda x: stats[x][m][0])
            mm, pb = stats["manuk"][m][1], stats[best][m][1]
            cut = 1 - stats["manuk"][m][0] / stats[best][m][0]
            outside = mm[1] < pb[0]  # manuk's highest run below Playwright's lowest run
            succ = min(stats["manuk"]["pct"]) >= min(stats[best]["pct"]) - 10
            verdict = "MET" if cut >= 0.30 and outside and succ else "NOT MET"
            P(f"- {m}: manuk {stats['manuk'][m][0]:,.4g} vs {best} {stats[best][m][0]:,.4g} -> "
              f"{100 * cut:+.1f}% (need >=30% fewer); bands {'disjoint' if outside else 'overlap'}; "
              f"success within 10 pts: {'yes' if succ else 'no'} -> {verdict}")
        # the per-arm best-Playwright reference is printed so a reader can see which incumbent set the bar
        P(f"- incumbent per metric is the lower of {', '.join(pw)}; per-arm numbers are in the table above")
    text = "\n".join(L)
    print(text)
    if a.out:
        open(a.out, "w").write(text + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
