#!/usr/bin/env python3
"""GUARD.R2: reduce a tri-sweep persisted TSV to one docs/sprint/GUARD.tsv row (human-view guard metrics).

Definitions are the ones GUARD.R1 rows were written with (reproduced exactly on R1run1, see --self-check):
  reach  = M1 status in {OK, LOW_SAMPLE, NO_STRUCT}
  cov    = mean M1 cov over reachable rows with a value
  shape  = mean M1 SHAPE over reachable rows with a value
  m2_full= mean M2 drive rate over reachable rows with a value
  m2_meas= mean M2 drive rate over rows tagged M2 OK
  ax_full= mean a11y F1 over reachable rows with a value
  ax_meas= mean a11y F1 over rows tagged a11y OK
m2_meas and ax_meas are not the GUARD.R0 formulas (see TAIL.1.f.b); they are reported as the GUARD.R1 rows report them.

usage: python3 -I scripts/guard-r2-rows.py <tri-sweep.tsv> <note> [--self-check]
"""
import csv
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
R1_SOURCE = "tri-sweep-20261102-041112.tsv"
R1_EXPECT = {"reach": 51, "cov": 86.89, "shape": 78.98, "m2_full": 79.06, "m2_meas": 79.06, "ax_full": 80.94, "ax_meas": 82.56}


def metrics(path):
    rows = list(csv.DictReader(open(path, newline=""), delimiter="\t"))
    reach = [r for r in rows if r["m1_status"] in ("OK", "LOW_SAMPLE", "NO_STRUCT")]

    def mean(vals):
        vals = [float(v) for v in vals if v not in ("", None)]
        return sum(vals) / len(vals) if vals else float("nan")

    out = {
        "reach": len(reach),
        "cov": mean(r["cov"] for r in reach),
        "shape": mean(r["shape"] for r in reach),
        "m2_full": mean(r["m2_rate"] for r in reach),
        "m2_meas": mean(r["m2_rate"] for r in rows if r["m2_tag"] == "OK"),
        "ax_full": mean(r["a11y_f1"] for r in reach),
        "ax_meas": mean(r["a11y_f1"] for r in rows if r["a11y_tag"] == "OK"),
        "sites": len(rows),
        "chrome_ver": sorted({r["chrome_ver"] for r in rows}),
    }
    return out


def main():
    args = [a for a in sys.argv[1:] if a != "--self-check"]
    if "--self-check" in sys.argv:
        m = metrics(os.path.join(ROOT, "docs", "bench", R1_SOURCE))
        bad = []
        for k, want in R1_EXPECT.items():
            got = m[k]
            if abs(got - want) > 0.006:
                bad.append(f"{k} got {got:.2f} want {want}")
        if bad:
            print("SELF-CHECK RED: " + "; ".join(bad))
            sys.exit(1)
        print("SELF-CHECK GREEN: formulas reproduce GUARD.tsv R1run1")
        return
    if len(args) != 3:
        print(__doc__)
        sys.exit(2)
    src, row, note = args
    m = metrics(src)
    if m["chrome_ver"] != [os.environ.get("MANUK_CHROME_PIN", "")]:
        print(f"FAIL: chrome_ver in {src} is {m['chrome_ver']}, MANUK_CHROME_PIN is {os.environ.get('MANUK_CHROME_PIN')}")
        sys.exit(1)
    commit = os.popen("git -C %s rev-parse --short HEAD" % ROOT).read().strip()
    vals = [row, os.path.basename(src), commit, m["chrome_ver"][0], m["sites"], m["reach"]]
    vals += [f"{m[k]:.2f}" for k in ("cov", "shape", "m2_full", "m2_meas", "ax_full", "ax_meas")] + [note]
    print("\t".join(str(v) for v in vals))


if __name__ == "__main__":
    main()
