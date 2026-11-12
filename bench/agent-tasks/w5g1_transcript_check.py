#!/usr/bin/env python3
"""W5.G1.b.a: every G1 row must still have its own transcript.

  w5g1_transcript_check.py [--runs-dir target/real-runs] [--dir docs/sprint/w5/G1] [--tsv F ...]

For each row of the G1 TSVs (default cdp-layer.tsv, cdp-layer+pwfmt.tsv), RUNS/<arm>_<task>_r<run>/transcript.jsonl must
exist and its final `result` event's text, flattened as real_run.flat_answer does, must start with the row's `answer`
column (first 80 chars). Exit 1 and list each row that fails (missing, no result event, or answer differs: the dir was
overwritten by a later run).
"""
import argparse, csv, json, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
G1 = os.path.join(ROOT, "docs/sprint/w5/G1")


def flat(t):
    return (t or "").replace("\t", " ").replace("\n", " ")


def final_result(path):
    res = None
    for line in open(path, encoding="utf-8", errors="replace"):
        try:
            e = json.loads(line)
        except ValueError:
            continue
        if e.get("type") == "result":
            res = e.get("result")
    return res


def check(runs_dir, tsvs):
    bad, n = [], 0
    for tsv in tsvs:
        for r in csv.DictReader(open(tsv), delimiter="\t"):
            n += 1
            key = f"{r['arm']} {r['task']} r{r['run']}"
            p = os.path.join(runs_dir, f"{r['arm']}_{r['task']}_r{r['run']}", "transcript.jsonl")
            if not os.path.exists(p):
                bad.append(f"{key}: no transcript")
                continue
            res = final_result(p)
            if res is None:
                bad.append(f"{key}: no result event")
            elif flat(res)[:80] != r["answer"][:80]:
                bad.append(f"{key}: transcript answer differs from the row (overwritten by a later run)")
    return n, bad


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--runs-dir", default=os.path.join(ROOT, "target/real-runs"))
    ap.add_argument("--tsv", action="append")
    a = ap.parse_args()
    tsvs = a.tsv or [os.path.join(G1, "cdp-layer.tsv"), os.path.join(G1, "cdp-layer+pwfmt.tsv")]
    n, bad = check(a.runs_dir, tsvs)
    print(f"{n - len(bad)}/{n} rows tied to their transcript")
    if bad:
        print("CHECK FAIL:\n  " + "\n  ".join(bad))
        sys.exit(1)
    print("CHECK PASS")


if __name__ == "__main__":
    main()
