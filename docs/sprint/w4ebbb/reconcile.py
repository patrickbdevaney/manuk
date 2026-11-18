#!/usr/bin/env python3 -I
"""W4.E0.c.b.b: reconcile the 0.30x URL-sweep PSS figure with the process-per-page figure.
Reads a runs-urls.tsv (one page per process, one URL per process, both arms) and prints, per run, the
statistic that produced 0.30x (median of per-URL manuk/chrome ratios) next to the pooled ratio (sum/sum)
on the same URLs. --selftest: a swapped-arm input and a truncated input must both go red."""
import csv, statistics as st, sys

def load(path):
    return [r for r in csv.DictReader(open(path), delimiter="\t") if r["axis"] == "urls"]

def paired(rows, paired_set=None):
    runs = sorted({int(r["run"]) for r in rows})
    d = {(r["arm"], r["target"], int(r["run"])): int(r["peak_pss_kb"]) / 1024 for r in rows if r["status"] == "OK"}
    urls = sorted({k[1] for k in d})
    ok = [u for u in urls if all((a, u, i) in d for a in ("manuk-proc", "chrome-proc") for i in runs)]
    if paired_set is not None:
        ok = [u for u in ok if u.split("/")[2] in paired_set]
    return runs, ok, d

def stats(rows, paired_set=None, expect_runs=3):
    runs, ok, d = paired(rows, paired_set)
    if len(runs) != expect_runs or not ok:
        raise SystemExit(f"RED: runs={len(runs)} (want {expect_runs}) paired_urls={len(ok)}")
    out = []
    for run in runs:
        m = [d[("manuk-proc", u, run)] for u in ok]; c = [d[("chrome-proc", u, run)] for u in ok]
        out.append((run, st.median(a / b for a, b in zip(m, c)), sum(m) / sum(c), st.mean(m), st.mean(c)))
    return ok, out

def main(argv):
    path = argv[1]
    ps = set(argv[2].split(",")) if len(argv) > 2 else None
    ok, out = stats(load(path), ps)
    print(f"{len(ok)} URLs: {' '.join(u.split('/')[2] for u in ok)}")
    for run, med, pooled, mm, cm in out:
        print(f"run {run}: median per-URL {med:.3f}  pooled sum/sum {pooled:.3f}  mean/page manuk {mm:.1f} MB chrome {cm:.1f} MB")

def selftest():
    path = sys.argv[2]
    ok, out = stats(load(path))
    assert all(0.1 < o[1] < 0.6 for o in out)
    swapped = [dict(r, arm={"manuk-proc": "chrome-proc", "chrome-proc": "manuk-proc"}.get(r["arm"], r["arm"])) for r in load(path)]
    _, sw = stats(swapped)
    assert sw[0][1] > 1.5, "swapped arms must invert the ratio"
    try:
        stats([r for r in load(path) if r["run"] != "3"])
    except SystemExit:
        print("selftest: OK (swapped arms invert, truncated runs RED)"); return
    raise AssertionError("truncated input did not go red")

if __name__ == "__main__":
    selftest() if sys.argv[1] == "--selftest" else main(sys.argv)
