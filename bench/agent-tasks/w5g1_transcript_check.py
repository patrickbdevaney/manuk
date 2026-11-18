#!/usr/bin/env python3
"""W5.G1.b.a: every G1 row must still have its own transcript.

  w5g1_transcript_check.py [--runs-dir target/real-runs] [--dir docs/sprint/w5/G1] [--tsv F ...]

For each row of the G1 TSVs (default cdp-layer.tsv, cdp-layer+pwfmt.tsv), the row's transcript must
exist and its final `result` event's text, flattened as real_run.flat_answer does, must start with the row's `answer`
column (first 80 chars). Exit 1 and list each row that fails (missing, no result event, or answer differs: the dir was
overwritten by a later run).
"""
import argparse, csv, datetime, json, os, re, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
G1 = os.path.join(ROOT, "docs/sprint/w5/G1")
# The run timeout real_run.py uses by default (--timeout 420). A run killed by it writes no result event.
RUN_TIMEOUT_S = 420


def transcript_path(runs_dir, row, tol=5):
    """W5.G1.b.b.a.a: a later run into the same arm/task/run dir archives the earlier one as <dir>.<YYYYMMDDTHHMMSS>, a few
    seconds from the earlier row's ts column. A row's own transcript is the archive within `tol` seconds of its ts when one
    exists (the nearest, if several), and the live dir otherwise."""
    stem = f"{row['arm']}_{row['task']}_r{row['run']}"
    base = os.path.join(runs_dir, stem)
    best = None
    if row.get("ts"):
        want = datetime.datetime.fromisoformat(row["ts"])
        for name in os.listdir(runs_dir):
            m = re.fullmatch(re.escape(stem) + r"\.(\d{8}T\d{6})", name)
            if not m:
                continue
            gap = abs((datetime.datetime.strptime(m.group(1), "%Y%m%dT%H%M%S") - want).total_seconds())
            if gap <= tol and (best is None or gap < best[0]):
                best = (gap, name)
    return os.path.join(runs_dir, best[1] if best else os.path.basename(base), "transcript.jsonl")


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


def is_timeout_row(r):
    """A row killed by the run timeout: seconds at least RUN_TIMEOUT_S, pass 0, and the TIMEOUT marker in answer."""
    try:
        secs = float(r.get("seconds") or "nan")
    except ValueError:
        return False
    return secs >= RUN_TIMEOUT_S and r.get("pass") == "0" and r.get("answer", "").strip() == "TIMEOUT"


def check(runs_dir, tsvs):
    bad, n = [], 0
    for tsv in tsvs:
        for r in csv.DictReader(open(tsv), delimiter="\t"):
            n += 1
            key = f"{r['arm']} {r['task']} r{r['run']}"
            p = transcript_path(runs_dir, r)
            if not os.path.exists(p):
                bad.append(f"{key}: no transcript")
                continue
            res = final_result(p)
            if res is None:
                # W5.G1.b.a.a.a.a.b: a timed-out run (seconds at least the run timeout, pass 0, answer TIMEOUT) has no
                # result event by construction; its transcript is tied as a timeout. Any other row without one is untied.
                if is_timeout_row(r):
                    continue
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
