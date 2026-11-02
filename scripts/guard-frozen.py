#!/usr/bin/env python3
"""GUARD.R0.a/TAIL.1.f: frozen reachable set for the human-view guard.

Reads the four GUARD.R0 tri-sweep files named in docs/sprint/GUARD.tsv, takes the
run-1 M1 OK rows as the frozen set F, removes the sites listed in
docs/sprint/GUARD-ORIGIN.tsv (origin reachability variance shown in TAIL.md),
and writes docs/sprint/GUARD-FROZEN.tsv with the old per-run means (copied from
GUARD.tsv) next to the recomputed means over F. Exits 1 if any site named in
REQUIRED_ORIGIN is still in F, or if the ORIGIN file is missing one of them.

Stdlib only; run as: python3 -I scripts/guard-frozen.py
"""
import csv
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SPRINT = os.path.join(ROOT, "docs", "sprint")
BENCH = os.path.join(ROOT, "docs", "bench")
REQUIRED_ORIGIN = [
    "https://esaj.tjsp.jus.br/",
    "https://www.amazon.com.mx/",
    "https://webfenix.movilidadbogota.gov.co/",
    "https://linxonline.co.pierce.wa.us/",
]
CHROME_VER = "145.0.7632.116"


def read_tsv(path):
    with open(path, newline="") as fh:
        return list(csv.DictReader(fh, delimiter="\t"))


def mean(vals):
    return round(sum(vals) / len(vals), 2) if vals else ""


def main():
    guard = read_tsv(os.path.join(SPRINT, "GUARD.tsv"))
    origin = {r["url"]: r for r in read_tsv(os.path.join(SPRINT, "GUARD-ORIGIN.tsv"))}
    missing = [u for u in REQUIRED_ORIGIN if u not in origin]
    if missing:
        print("FAIL: GUARD-ORIGIN.tsv lacks required origin sites:", *missing, sep="\n  ")
        return 1

    guard_runs = [g for g in guard if g["row"].startswith("run")]
    runs = {}
    for g in guard_runs:
        rows = read_tsv(os.path.join(BENCH, g["source"]))
        runs[g["row"]] = {r["url"]: r for r in rows}

    first = guard_runs[0]["row"]
    frozen = sorted(u for u, r in runs[first].items() if r["m1_status"] == "OK" and u not in origin)
    leaked = [u for u in REQUIRED_ORIGIN if u in frozen]
    if leaked:
        print("FAIL: origin sites still in the frozen set:", *leaked, sep="\n  ")
        return 1

    out = [
        ["row", "set", "n", "cov", "shape", "m2_full", "ax_full", "chrome_ver", "note"]
    ]
    for g in guard_runs:
        out.append([g["row"], "old-GUARD.tsv", g["reach"], g["cov"], g["shape"],
                    g["m2_full"], g["ax_full"], CHROME_VER,
                    "mean over reachable rows incl. LOW_SAMPLE, as recorded"])
    for g in guard_runs:
        rows = [runs[g["row"]][u] for u in frozen]
        ok = [r for r in rows if r["m1_status"] == "OK"]
        out.append([
            g["row"], "frozen-F", len(ok),
            mean([float(r["cov"]) for r in ok]),
            mean([float(r["shape"]) for r in ok]),
            mean([float(r["m2_rate"]) for r in ok]),
            mean([float(r["a11y_f1"]) for r in ok if r["a11y_f1"]]),
            CHROME_VER,
            "F = run1 OK minus ORIGIN; n = sites of F reachable (OK) in this run of %d" % len(frozen),
        ])

    dest = os.path.join(SPRINT, "GUARD-FROZEN.tsv")
    with open(dest, "w", newline="") as fh:
        csv.writer(fh, delimiter="\t", lineterminator="\n").writerows(out)
    for line in out:
        print("\t".join(str(c) for c in line))
    print("frozen set F: %d sites; excluded ORIGIN: %s" % (
        len(frozen), ", ".join(sorted(origin))))
    return 0


if __name__ == "__main__":
    sys.exit(main())
