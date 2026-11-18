#!/usr/bin/env python3
"""W4.E0.b.a.a gate: chrome-proc PSS drift on the 7 paired URLs.
Two 3-run sets of chrome-proc PSS, taken in different sessions, both at chrome_ver = pin.
Per URL: median PSS (OK rows) of set A vs set B must agree within 15%, else the drift is 'unexplained'.
  python3 -I gate.py chrome-now.tsv chrome-next.tsv     -> exit 0 only if both sets are complete and agree
  python3 -I gate.py chrome-now.tsv                     -> exit 1: second session set missing (gate not run)
  python3 -I gate.py chrome-now.tsv chrome-next.tsv --tol 0.01 -> exit 1 (red check, impossible tolerance)
Context (not gated): the 2026-11-17 10-run sweep and the 2026-10-31 10-run sweep (git 23182449), medians printed.
"""
import csv, os, statistics, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(HERE)))
PIN = "145.0.7632.116"
PAIRED = {"https://admin.zoomph.com/", "https://app.ordertime.com/", "https://client.dkitrxmdwoqruvsi.net/",
          "https://comix.to/", "https://ebanking.bdm-sa.com/", "https://www.mobile.bg/", "https://www.puentedemando.com/"}
args = sys.argv[1:]
tol = float(args[args.index("--tol") + 1]) if "--tol" in args else 0.15
pos = [a for i, a in enumerate(args) if not a.startswith("--") and (i == 0 or args[i - 1] != "--tol")]
def load(path):
    rows = [r for r in csv.DictReader(open(path), delimiter="\t") if not r.get("axis", "").startswith("#")]
    if any(r["chrome_ver"] != PIN for r in rows):
        print(f"FAIL {path}: chrome_ver not the single pin"); sys.exit(1)
    per = {}
    for r in rows:
        per.setdefault(r["target"], []).append(r)
    return per
def med_ok(rows):
    ok = [int(r["pss_kb"]) for r in rows if r["status"] == "OK"]
    return statistics.median(ok) / 1024 if len(ok) >= 2 else None
A = load(pos[0])
if set(A) != PAIRED or any(len(v) != 3 for v in A.values()):
    print("FAIL set A: need 7 URLs x 3 runs"); sys.exit(1)
if len(pos) < 2:
    print("NOT RUN: second session set (3 runs at next session start) not supplied"); sys.exit(1)
B = load(pos[1])
if set(B) != PAIRED or any(len(v) != 3 for v in B.values()):
    print("FAIL set B: need 7 URLs x 3 runs"); sys.exit(1)
bad = []
for u in sorted(PAIRED):
    a, b = med_ok(A[u]), med_ok(B[u])
    if a is None or b is None:
        bad.append(f"{u} <2 OK runs"); continue
    d = abs(a - b) / min(a, b)
    print(f"{u:40} A={a:7.1f}MB B={b:7.1f}MB diff={d*100:5.1f}%")
    if d > tol:
        bad.append(f"{u} diff {d*100:.1f}% > {tol*100:.0f}%")
if bad:
    print("UNEXPLAINED DRIFT:", "; ".join(bad)); sys.exit(1)
print(f"PASS: all 7 URLs agree within {tol*100:.0f}% across the two sessions")
