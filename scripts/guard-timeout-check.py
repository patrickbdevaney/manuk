#!/usr/bin/env python3
"""GUARD.R0.a.b.a: did the 41-site frozen M1 pass time out under a 120 s limit, and does run4 still self-agree?

Four persisted tri-sweep files (run1..run4, in order), each produced with --m1-timeout 120 on the same frozen corpus
(scripts/guard-frozen-fresh-run.sh 4 120). Green only if (a) m1_status is TIMEOUT on zero rows in EVERY run, and
(b) run4 cov and shape lie inside the [min,max] band of runs 1-3. Anything else is red (exit 1), and the sites that
caused it are listed. Writes docs/sprint/GUARD-TIMEOUT-RUNS.tsv (one row per run, one band row, one verdict row).

Stdlib only; run as: python3 -I scripts/guard-timeout-check.py RUN1.tsv RUN2.tsv RUN3.tsv RUN4.tsv
"""
import csv
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DEST = os.environ.get("GUARD_TIMEOUT_DEST") or os.path.join(ROOT, "docs", "sprint", "GUARD-TIMEOUT-RUNS.tsv")
CHROME_VER = os.environ.get("MANUK_CHROME_PIN") or "145.0.7632.116"
NOTE = os.environ.get("GUARD_TIMEOUT_NOTE") or "frozen F, M1 fresh, --m1-timeout 120"
GATED = ["cov", "shape"]


def read_rows(path):
    with open(path, newline="") as fh:
        return list(csv.DictReader(fh, delimiter="\t"))


def run_stats(path):
    rows = read_rows(path)
    timeouts = [r["url"] for r in rows if r["m1_status"] == "TIMEOUT"]
    n = len(rows)
    return {
        "n": n,
        "timeouts": timeouts,
        # a failed M1 scores 0 (not dropped), matching guard-frozen-check.py
        "cov": sum(float(r["cov"] or 0) for r in rows) / n,
        "shape": sum(float(r["shape"] or 0) for r in rows) / n,
    }


def main(paths):
    if len(paths) != 4:
        print("usage: guard-timeout-check.py RUN1 RUN2 RUN3 RUN4")
        return 2
    runs = [run_stats(p) for p in paths]
    red = []
    band = {}
    for m in GATED:
        lo, hi = min(r[m] for r in runs[:3]), max(r[m] for r in runs[:3])
        band[m] = (lo, hi)
        if not lo <= runs[3][m] <= hi:
            red.append("run4 %s %.2f outside [%.2f,%.2f]" % (m, runs[3][m], lo, hi))
    for i, r in enumerate(runs, 1):
        if r["timeouts"]:
            red.append("run%d has %d TIMEOUT rows: %s" % (i, len(r["timeouts"]), ", ".join(r["timeouts"][:8])))
    verdict = "FAIL" if red else "PASS"
    out = [["row", "n", "m1_timeouts", "cov", "shape", "chrome_ver", "note"]]
    for i, r in enumerate(runs, 1):
        out.append(["run%d" % i, r["n"], len(r["timeouts"]), "%.2f" % r["cov"], "%.2f" % r["shape"], CHROME_VER, NOTE])
    out.append(["band", "", "", "[%.2f,%.2f]" % band["cov"], "[%.2f,%.2f]" % band["shape"], CHROME_VER, "min..max of runs 1-3"])
    out.append(["verdict", "", "", "", "", CHROME_VER, verdict + ("" if not red else ": " + "; ".join(red))])
    with open(DEST, "w", newline="") as fh:
        csv.writer(fh, delimiter="\t", lineterminator="\n").writerows(out)
    for line in out:
        print("\t".join(str(c) for c in line))
    return 1 if red else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
