#!/usr/bin/env python3
"""W5.G1.b.c.a.a.a.a.a: on the v4 hand-labelled rows, a run hand-labelled none/grader-issue must not be classed perceive,
and every hand-labelled perceive row must stay perceive (the new `unexplained` class replaces the blanket fallback).
Exit 1 on a violation. --mutate perceive-fallback restores the old fallback (must exit 1).
  w5g1_unexplained_check.py [--mutate perceive-fallback]"""
import argparse, csv, os, sys
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import w5g1_report as r
ROOT = os.path.join(HERE, "..", "..")
ap = argparse.ArgumentParser()
ap.add_argument("--mutate", choices=["perceive-fallback"])
ap.add_argument("--labels", default=os.path.join(ROOT, "docs/sprint/w5/G1/hand-labels-v4.tsv"))
a = ap.parse_args()
SKIP = {("gomail-8", "1"), ("gomail-8", "2")}  # grader-issue comes from GRADER_ISSUE in main(), not classify()
bad, n = [], 0
for x in csv.DictReader(open(a.labels), delimiter="\t"):
    arm = x["arm"].replace("-v2", "")
    p = os.path.join(ROOT, "target/real-runs", f"{arm}_{x['task']}_r{x['run']}", "transcript.jsonl")
    c = r.classify(p, a.mutate)
    n += 1
    if x["hand_label"] in ("none", "grader-issue") and c == "perceive" and (x["task"], x["run"]) not in SKIP:
        bad.append(f"{x['task']} r{x['run']}: hand {x['hand_label']} but classed perceive")
    if x["hand_label"] == "perceive" and c not in ("perceive", "target"):  # target over-trigger is a separate follow-up
        bad.append(f"{x['task']} r{x['run']}: hand perceive but classed {c}")
print(f"{n} rows, {len(bad)} violations")
print("\n".join(bad))
sys.exit(1 if bad or n < 10 else 0)
