#!/usr/bin/env python3
"""PERF.1.1 gate: sum per-URL median subres_fetch / doc_fetch (manuk) and count URLs requested twice.

Usage: python3 -I scripts/perf1-gate.py <runs.jsonl> [traces_dir]
Exit 1 when sum subres_fetch > 24 or doc_fetch > 1.10*4.72 or any URL is on the wire twice.

Paired mode (PERF.1.1.b): python3 -I scripts/perf1-gate.py --paired <paired.jsonl> [--phase subres_fetch]
  perf1-profile.py --manuk-b writes arm A (engine "manuk") and arm B (engine "manuk-b") interleaved
  per URL and run. Each run's Σ over URLs is taken for both arms in the same minute, and the
  judgement is the per-run ratio B/A: its median and min-max band. Exit 1 when the median ratio
  is outside --max-ratio or the band straddles it. Null (A and B the same binary and env) reads
  ratio ~1.00; a B with MANUK_MAX_CONNS_PER_HOST=1 must read > 1.30.
  python3 -I scripts/perf1-gate.py --self-test
"""
import argparse, collections, glob, json, os, statistics, sys


def paired(path, phase, max_ratio, min_ratio):
    rows = [json.loads(l) for l in open(path) if l.strip()]
    ok = {}
    for r in rows:
        if r["status"] == "OK" and r.get("engine") in ("manuk", "manuk-b"):
            ok[(r["run"], r["url"], r["engine"])] = r["phases_s"][phase]
    runs = sorted({k[0] for k in ok})
    urls = sorted({k[1] for k in ok})
    per_run = []
    for run in runs:
        pairs = [(ok[(run, u, "manuk")], ok[(run, u, "manuk-b")]) for u in urls
                 if (run, u, "manuk") in ok and (run, u, "manuk-b") in ok]
        if not pairs:
            continue
        a = sum(x for x, _ in pairs)
        b = sum(y for _, y in pairs)
        per_run.append((run, a, b, len(pairs)))
    if not per_run:
        print("PAIRED no complete pairs"); return 1
    ratios = [b / a for _, a, b, _ in per_run]
    med = statistics.median(ratios)
    med_a = statistics.median(a for _, a, _, _ in per_run)
    med_b = statistics.median(b for _, _, b, _ in per_run)
    for run, a, b, n in per_run:
        print(f"run{run} pairs={n} sum_A={a:.2f}s sum_B={b:.2f}s B/A={b / a:.3f}")
    print(f"PAIRED phase={phase} median_sum_A={med_a:.2f}s median_sum_B={med_b:.2f}s "
          f"ratio_B/A median={med:.3f} band=[{min(ratios):.3f}-{max(ratios):.3f}] n_runs={len(per_run)}")
    verdict = "GREEN" if min_ratio <= med <= max_ratio else "RED"
    print("PAIRED", verdict, f"(bar {min_ratio:.2f} <= B/A <= {max_ratio:.2f})")
    return 0 if verdict == "GREEN" else 1


def self_test():
    def row(run, url, eng, sub, status="OK"):
        return {"run": run, "url": url, "engine": eng, "status": status, "phases_s": {"subres_fetch": sub}}
    urls = ["u1", "u2"]
    null = [row(r, u, e, 1.0 + 0.1 * (r % 2) + (0.05 if e == "manuk-b" else 0.0)) for r in (1, 2, 3) for u in urls for e in ("manuk", "manuk-b")]
    mut = [row(r, u, e, 1.0 * (2.0 if e == "manuk-b" else 1.0)) for r in (1, 2, 3) for u in urls for e in ("manuk", "manuk-b")]
    import tempfile
    def judge(rows):
        fd, path = tempfile.mkstemp(suffix=".jsonl")
        with os.fdopen(fd, "w") as f:
            for r in rows:
                f.write(json.dumps(r) + "\n")
        try:
            return paired(path, "subres_fetch", 1.10, 0.90)
        finally:
            os.unlink(path)
    assert judge(null) == 0, "null pair must be GREEN"
    assert judge(mut) == 1, "B with 2x subres must be RED against the 1.10 ceiling"
    assert judge([row(1, "u1", "manuk", 1.0)]) == 1, "no complete pairs must not be GREEN"
    print("self-test OK: null pair GREEN, 2x B RED, unpaired RED")
    return 0


if "--self-test" in sys.argv:
    sys.exit(self_test())
if "--paired" in sys.argv:
    ap = argparse.ArgumentParser()
    ap.add_argument("--paired", action="store_true")
    ap.add_argument("path")
    ap.add_argument("--phase", default="subres_fetch")
    ap.add_argument("--max-ratio", type=float, default=1.10)
    ap.add_argument("--min-ratio", type=float, default=0.90)
    a = ap.parse_args()
    sys.exit(paired(a.path, a.phase, a.max_ratio, a.min_ratio))

rows = [json.loads(l) for l in open(sys.argv[1]) if l.strip()]
rows = [r for r in rows if r["engine"] == "manuk" and r["status"] == "OK"]
by = collections.defaultdict(list)
for r in rows:
    by[r["url"]].append(r)
tot = {"subres_fetch": 0.0, "doc_fetch": 0.0}
for u, rs in by.items():
    for k in tot:
        tot[k] += statistics.median(r["phases_s"][k] for r in rs)
print(f"urls={len(by)} sum_subres_fetch={tot['subres_fetch']:.2f}s sum_doc_fetch={tot['doc_fetch']:.2f}s")
dupes = 0
if len(sys.argv) > 2:
    for f in sorted(glob.glob(os.path.join(sys.argv[2], "*.tsv"))):
        c = collections.Counter()
        for l in open(f):
            p = l.rstrip("\n").split("\t")
            if len(p) > 2 and p[1] == "req" and (len(p) < 9 or p[8] == "GET"):
                c[p[2]] += 1
        d = {u: n for u, n in c.items() if n > 1}
        if d:
            dupes += sum(n - 1 for n in d.values())
            print("DUP", os.path.basename(f), len(d), list(d)[:2])
    print(f"duplicate wire requests (all traces) = {dupes}")
bad = tot["subres_fetch"] > 24 or tot["doc_fetch"] > 4.72 * 1.10 or dupes
print("GATE", "RED" if bad else "GREEN")
sys.exit(1 if bad else 0)
