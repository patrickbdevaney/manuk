#!/usr/bin/env python3
"""W5.G1 agreement gate: hand labels vs causes.tsv. Exit 0 iff agreement >= THRESH of n, else 1.
  w5g1_agree_check.py HAND_LABELS.tsv [--causes causes.tsv] [--thresh 9]"""
import argparse, csv, os, sys
ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..")
ap = argparse.ArgumentParser()
ap.add_argument("labels")
ap.add_argument("--causes", default=os.path.join(ROOT, "docs/sprint/w5/G1/causes.tsv"))
ap.add_argument("--thresh", type=int, default=9)
a = ap.parse_args()
rd = lambda f: list(csv.DictReader(open(f), delimiter="\t"))
c = {(x["arm"], x["task"], x["run"]): x["class"] for x in rd(a.causes)}
h = rd(a.labels)
n = sum(c.get((x["arm"], x["task"], x["run"])) == x["hand_label"] for x in h)
print(f"agreement {n}/{len(h)} (threshold {a.thresh}/10)")
sys.exit(0 if n >= a.thresh and len(h) >= 10 else 1)
