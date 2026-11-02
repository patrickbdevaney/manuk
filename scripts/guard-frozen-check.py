#!/usr/bin/env python3
"""GUARD.R0.a: self-agreement check on the frozen site list F (docs/sprint/guard-frozen-m1.tsv).

Takes four tri-sweep persisted files (run1..run4, in order) that were all produced by
scripts/guard-frozen-run.sh on the SAME frozen list. Computes six metrics per run over F:
  cov, shape      mean of the M1 columns over F (a failed M1 scores 0; constant by design if M1 is imported)
  m2_full         mean m2_rate over all F rows (non-OK drive tags score 0)
  m2_meas         mean m2_rate over F rows with m2_tag OK
  ax_full         mean a11y_f1 over all F rows (non-OK a11y tags score 0)
  ax_meas         mean a11y_f1 over F rows with a11y_tag OK
The 3-run band is [min, max] of runs 1-3; run4 must lie inside it on all six, else exit 1.
Writes docs/sprint/GUARD-FROZEN-RUNS.tsv (one row per run, one band row per metric).

Stdlib only; run as: python3 -I scripts/guard-frozen-check.py RUN1.tsv RUN2.tsv RUN3.tsv RUN4.tsv
"""
import csv
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FROZEN = os.path.join(ROOT, "docs", "sprint", "guard-frozen-m1.tsv")
# GUARD_FROZEN_DEST / GUARD_FROZEN_NOTE let a run write its own file (the fresh-M1 runs of GUARD.R0.a.b)
DEST = os.environ.get("GUARD_FROZEN_DEST") or os.path.join(ROOT, "docs", "sprint", "GUARD-FROZEN-RUNS.tsv")
NOTE = os.environ.get("GUARD_FROZEN_NOTE") or "frozen F, M1 imported"
CHROME_VER = "145.0.7632.116"
METRICS = ["cov", "shape", "m2_full", "m2_meas", "ax_full", "ax_meas"]


def read_tsv(path):
    with open(path, newline="") as fh:
        return list(csv.DictReader(fh, delimiter="\t"))


def mean(vals):
    return sum(vals) / len(vals) if vals else float("nan")


def frozen_urls():
    with open(FROZEN, newline="") as fh:
        return [r[1] for r in csv.reader(fh, delimiter="\t")]


def run_metrics(path, urls):
    rows = {r["url"]: r for r in read_tsv(path)}
    missing = [u for u in urls if u not in rows]
    if missing:
        raise SystemExit("FAIL: %s lacks frozen sites: %s" % (path, ", ".join(missing[:5])))
    fr = [rows[u] for u in urls]
    m2_ok = [r for r in fr if r["m2_tag"] == "OK"]
    ax_ok = [r for r in fr if r["a11y_tag"] == "OK"]
    return {
        "n": len(fr),
        # a site whose M1 fails in this run has an empty cov/shape: it scores 0, it is not dropped (GUARD.R0.a.b)
        "cov": mean([float(r["cov"] or 0) for r in fr]),
        "shape": mean([float(r["shape"] or 0) for r in fr]),
        "m2_full": mean([float(r["m2_rate"] or 0) for r in fr]),
        "m2_meas": mean([float(r["m2_rate"]) for r in m2_ok]),
        "ax_full": mean([float(r["a11y_f1"] or 0) for r in fr]),
        "ax_meas": mean([float(r["a11y_f1"]) for r in ax_ok]),
        "n_m2_ok": len(m2_ok),
        "n_ax_ok": len(ax_ok),
    }


def main(paths):
    if len(paths) != 4:
        print("usage: guard-frozen-check.py RUN1 RUN2 RUN3 RUN4")
        return 2
    urls = frozen_urls()
    runs = [run_metrics(p, urls) for p in paths]
    band = {}
    inside = {}
    for m in METRICS:
        lo, hi = min(r[m] for r in runs[:3]), max(r[m] for r in runs[:3])
        band[m] = (lo, hi)
        inside[m] = lo <= runs[3][m] <= hi
    out = [["row", "n", "cov", "shape", "m2_full", "m2_meas", "ax_full", "ax_meas",
            "n_m2_ok", "n_ax_ok", "chrome_ver", "note"]]
    for i, r in enumerate(runs, 1):
        out.append(["run%d" % i, r["n"]] + ["%.2f" % r[m] for m in METRICS]
                   + [r["n_m2_ok"], r["n_ax_ok"], CHROME_VER, NOTE])
    out.append(["band", "", *["[%.2f,%.2f]" % band[m] for m in METRICS], "", "", CHROME_VER,
                "min..max of runs 1-3"])
    out.append(["run4_inside", "", *[str(inside[m]) for m in METRICS], "", "", CHROME_VER,
                "self-agreement"])
    verdict = all(inside.values())
    out.append(["verdict", "", "PASS" if verdict else "FAIL", "", "", "", "", "", "", "", CHROME_VER,
                "run4 inside 3-run band on all six" if verdict else
                "outside: " + ", ".join(m for m in METRICS if not inside[m])])
    with open(DEST, "w", newline="") as fh:
        csv.writer(fh, delimiter="\t", lineterminator="\n").writerows(out)
    for line in out:
        print("\t".join(str(c) for c in line))
    return 0 if verdict else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
