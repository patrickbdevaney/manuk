#!/usr/bin/env python3
"""W5.G1.b.b.a.a.a.a: per-checkpoint view of the gomail-8 rows in a G1 arm TSV.

For every cdp-layer gomail-8 row it prints evals_vec, the failed checkpoint indices with their
description from the frozen task JSON, and the grader's updated-email count from the archived
run state (target/real-runs/<arm>_gomail-8_r<run>.<ts>/state.json, matched within 5 s of the row ts).

usage: python3 -I bench/agent-tasks/w5g1_gomail8_grader.py [--tsv F] [--task-json F] [--runs-dir D]
exit 1 if any such row has an all-ones evals_vec (the grader would pass that run), else 0.
"""
import argparse
import csv
import datetime
import glob
import json
import os
import sys


def archived_state(runs_dir, run, ts):
    """Return the archived state.json path whose timestamp is within 5 s of the row ts."""
    want = datetime.datetime.fromisoformat(ts)
    best = None
    for path in glob.glob(os.path.join(runs_dir, f"cdp-layer_gomail-8_r{run}.*", "state.json")):
        stamp = os.path.basename(os.path.dirname(path)).split(".", 1)[1]
        got = datetime.datetime.strptime(stamp, "%Y%m%dT%H%M%S")
        gap = abs((got - want).total_seconds())
        if gap <= 5 and (best is None or gap < best[0]):
            best = (gap, path)
    return best[1] if best else None


def updated_emails(path):
    with open(path) as f:
        emails = json.load(f)["state"]["differences"].get("emails", {})
    return [(e.get("id"), e.get("trash")) for e in emails.get("updated", [])]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--tsv", default="docs/sprint/w5/G1/cdp-layer-v2.tsv")
    ap.add_argument("--task-json", default="bench/agent-tasks/real/tasks/gomail-8.json")
    ap.add_argument("--runs-dir", default="target/real-runs")
    a = ap.parse_args()

    with open(a.task_json) as f:
        evals = json.load(f)["evals"]
    with open(a.tsv) as f:
        rows = [r for r in csv.DictReader(f, delimiter="\t") if r["arm"] == "cdp-layer" and r["task"] == "gomail-8"]

    all_ones = False
    for r in rows:
        vec = json.loads(r["evals_vec"])
        failed = [i for i, v in enumerate(vec) if not v]
        all_ones = all_ones or all(vec)
        print(f"run {r['run']} ts {r['ts']} pass {r['pass']} evals_vec {r['evals_vec']} chrome_ver {r['chrome_ver']}")
        for i in failed:
            print(f"  failed checkpoint {i}: {evals[i]['description']} (expected {evals[i]['expected_value']!r})")
        state = archived_state(a.runs_dir, r["run"], r["ts"])
        if state:
            up = updated_emails(state)
            print(f"  archived state {os.path.basename(os.path.dirname(state))}: {len(up)} updated, (id, trash) = {up}")
        else:
            print("  archived state: not found (run dir absent)")

    if not rows:
        print("no cdp-layer gomail-8 rows", file=sys.stderr)
        return 1
    if all_ones:
        print("RED: a gomail-8 cdp-layer row has an all-ones evals_vec", file=sys.stderr)
        return 1
    print("GREEN: no gomail-8 cdp-layer row is all-ones")
    return 0


if __name__ == "__main__":
    sys.exit(main())
