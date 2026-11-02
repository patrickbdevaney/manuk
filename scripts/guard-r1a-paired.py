#!/usr/bin/env python3
"""GUARD.R1.a: paired per-site shift between the GUARD.R0 run1 baseline and the GUARD.R1 run1 tri-sweep.

Pairs every site whose m1_status is OK in BOTH sweeps (keyed by url), then writes one row per paired site with
the before/after/delta of each metric, the sites that are OK in only one sweep, and SUMMARY rows (paired n and
mean per metric) to docs/sprint/GUARD-PAIRED.tsv. Also reports which corpus file each sweep's URLs came from,
because a site-set change is the first suspect for a reachable-set shift.

Gate (GRAPH.tsv row GUARD.R1.a, fixed before reading the table): GREEN only if paired n >= 30; RED otherwise
(exit 1). Metric list and threshold are fixed here and not tuned to the result.

usage: python3 -I scripts/guard-r1a-paired.py [base.tsv new.tsv [out.tsv]]
"""
import csv
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BASE = os.path.join(ROOT, "docs", "bench", "tri-sweep-20261024-225412.tsv")  # GUARD.R0 run1
NEW = os.path.join(ROOT, "docs", "bench", "tri-sweep-20261102-041112.tsv")  # GUARD.R1 run1
OUT = os.path.join(ROOT, "docs", "sprint", "GUARD-PAIRED.tsv")
CORPORA = [os.path.join(ROOT, "docs", "bench", f) for f in
           ("corpus-crux-trend.txt", "oracle-corpus.txt", "corpus-v2.tsv")]
METRICS = ["cov", "shape", "vis", "paths", "m2_rate", "a11y_prec", "a11y_rec", "a11y_f1"]
MIN_PAIRED = 30
CHROME = "145.0.7632.116"

if len(sys.argv) >= 3:  # mutation / ad-hoc use only; the gate itself runs with the defaults
    BASE, NEW = sys.argv[1], sys.argv[2]
if len(sys.argv) >= 4:
    OUT = sys.argv[3]


def norm(u):
    return re.sub(r"^https?://(www\.)?", "", u).rstrip("/").lower()


def load(path):
    with open(path, newline="") as f:
        rows = list(csv.DictReader(f, delimiter="\t"))
    return {r["url"]: r for r in rows}


def num(v):
    try:
        return float(v)
    except (TypeError, ValueError):
        return None


def corpus_overlap(urls):
    out = []
    for p in CORPORA:
        if not os.path.exists(p):
            continue
        cu = set()
        for line in open(p):
            if line.strip() and not line.startswith("#"):
                cu.add(norm(line.split()[-1]))
        out.append(f"{os.path.basename(p)}={len({norm(u) for u in urls} & cu)}/{len(urls)}")
    return " ".join(out)


base = load(BASE)
new = load(NEW)
chrome = {r["chrome_ver"] for r in list(base.values()) + list(new.values()) if r["chrome_ver"]}
if chrome != {CHROME}:
    print(f"FAIL: chrome_ver mismatch across sweeps: {sorted(chrome)}")
    sys.exit(1)

ok_base = {u for u, r in base.items() if r["m1_status"] == "OK"}
ok_new = {u for u, r in new.items() if r["m1_status"] == "OK"}
paired = sorted(ok_base & ok_new)
only_base = sorted(ok_base - ok_new)
only_new = sorted(ok_new - ok_base)

print("corpus overlap base:", corpus_overlap(list(base)))
print("corpus overlap new: ", corpus_overlap(list(new)))
print(f"url overlap (all rows) base/new = {len(set(base) & set(new))}/{len(base)}")

summary = {}
for m in METRICS:
    deltas = []
    for u in paired:
        a, b = num(base[u][m]), num(new[u][m])
        if a is not None and b is not None:
            deltas.append((a, b))
    n = len(deltas)
    if n:
        mb = sum(a for a, _ in deltas) / n
        mn = sum(b for _, b in deltas) / n
        summary[m] = (n, mb, mn, mn - mb)
    else:
        summary[m] = (0, None, None, None)

header = ["row", "cat", "url", "status_base", "status_new"]
for m in METRICS:
    header += [f"{m}_base", f"{m}_new", f"{m}_delta"]


def fmt(v, spec="+.2f"):
    return "n/a" if v is None else format(v, spec)


with open(OUT, "w", newline="") as f:
    w = csv.writer(f, delimiter="\t", lineterminator="\n")
    w.writerow(header)
    for u in paired:
        rb, rn = base[u], new[u]
        row = ["site", rb["cat"], u, rb["m1_status"], rn["m1_status"]]
        for m in METRICS:
            a, b = num(rb[m]), num(rn[m])
            row += [rb[m] or "", rn[m] or "", f"{b - a:+.2f}" if a is not None and b is not None else ""]
        w.writerow(row)
    for u in only_base:
        w.writerow(["only_base", base[u]["cat"], u, base[u]["m1_status"], "absent" if u not in new else new[u]["m1_status"]])
    for u in only_new:
        w.writerow(["only_new", new[u]["cat"], u, "absent" if u not in base else base[u]["m1_status"], new[u]["m1_status"]])
    for m in METRICS:
        n, mb, mn, md = summary[m]
        w.writerow(["SUMMARY", f"{m} n={n}", "", f"mean_base={fmt(mb, '.2f')}", f"mean_new={fmt(mn, '.2f')}",
                    f"mean_delta={fmt(md)}"])
    w.writerow(["SUMMARY", "paired_n", "", str(len(paired)), "", f"min_required={MIN_PAIRED}"])
    w.writerow(["SUMMARY", "chrome_ver", "", CHROME, CHROME, ""])

print(f"ok_base={len(ok_base)} ok_new={len(ok_new)} paired_n={len(paired)} "
      f"only_base={len(only_base)} only_new={len(only_new)}")
for m in METRICS:
    n, mb, mn, md = summary[m]
    print(f"{m:10s} n={n:3d} mean_base={fmt(mb, '7.2f'):>7s} mean_new={fmt(mn, '7.2f'):>7s} mean_delta={fmt(md, '+7.2f'):>8s}")
verdict = "GREEN" if len(paired) >= MIN_PAIRED else "RED"
print(verdict, f"paired_n={len(paired)} min_required={MIN_PAIRED}")
sys.exit(0 if verdict == "GREEN" else 1)
