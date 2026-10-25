#!/usr/bin/env python3
"""PERF.1.1.a paired A/B: the same binary with and without the early font/image scan, interleaved
per URL and run (arm order flips each run) so remote-origin noise hits both arms in the same minute.

Arm A = default. Arm B = MANUK_NO_EARLY_SCAN=1 (the PERF.1.1 behaviour, and the gate's mutant).
Usage: python3 -I scripts/perf1-ab.py --runs 3 --out /tmp/perf1-ab [--urls file]
Writes <out>/{early,base}/runs.jsonl + traces; judge with scripts/perf1-gate.py per arm and the
paired report printed at the end.
"""
import argparse, collections, importlib.util, json, os, statistics, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location("p1", os.path.join(HERE, "perf1-profile.py"))
p1 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p1)

ap = argparse.ArgumentParser()
ap.add_argument("--runs", type=int, default=3)
ap.add_argument("--out", default="/tmp/perf1-ab")
ap.add_argument("--manuk", default="target/release/a11y-dump")
ap.add_argument("--urls", default="docs/sprint/perf1/urls.txt")
ap.add_argument("--timeout", type=float, default=60.0)
a = ap.parse_args()
pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("MANUK_CHROME_PIN unset")
urls = [l.strip() for l in open(a.urls) if l.strip()]
commit = subprocess.run(["git", "rev-parse", "--short", "HEAD"], capture_output=True, text=True).stdout.strip()
files = {}
for arm in ("early", "base"):
    os.makedirs(os.path.join(a.out, arm), exist_ok=True)
    files[arm] = open(os.path.join(a.out, arm, "runs.jsonl"), "a")
for run in range(1, a.runs + 1):
    for url in urls:
        order = ("early", "base") if run % 2 else ("base", "early")
        for arm in order:
            if arm == "base":
                os.environ["MANUK_NO_EARLY_SCAN"] = "1"
            else:
                os.environ.pop("MANUK_NO_EARLY_SCAN", None)
            r = p1.run_manuk(a.manuk, url, a.timeout, os.path.join(a.out, arm, "traces"), run)
            r.update(run=run, chrome_ver=pin, commit=commit, arm=arm)
            files[arm].write(json.dumps(r) + "\n")
            files[arm].flush()
            print(f"run{run} {arm:5} {r['status']:7} subres={r.get('phases_s', {}).get('subres_fetch', 0):6.2f} {url}", file=sys.stderr)
os.environ.pop("MANUK_NO_EARLY_SCAN", None)
med = collections.defaultdict(lambda: collections.defaultdict(list))
for arm in ("early", "base"):
    for l in open(os.path.join(a.out, arm, "runs.jsonl")):
        r = json.loads(l)
        if r["status"] == "OK" and r.get("chrome_ver") == pin:
            med[arm][r["url"]].append(r["phases_s"]["subres_fetch"])
sums = {}
for arm in med:
    sums[arm] = sum(statistics.median(v) for v in med[arm].values())
print(f"chrome_ver={pin} paired Σ subres_fetch: early={sums['early']:.2f}s base={sums['base']:.2f}s ratio={sums['early']/sums['base']:.3f}")
