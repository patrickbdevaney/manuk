#!/usr/bin/env python3
"""W5.G1.b.a.a.a: the v2 vs v3 band for the cdp-layer arm, at one tree (HEAD) and one chrome_ver.

  w5g1_v3_compare.py [--dir docs/sprint/w5/G1] [--v2 cdp-layer-v2.tsv] [--v3 cdp-layer-v3.tsv] [--report F]

Per task: passes in v2, passes in v3, |v2-v3|. Exit 1 (RED) if a task has a row count other than 3 in either TSV,
if the two TSVs carry different chrome_ver values, or if a row's run number repeats within a task.
"""
import argparse, collections, csv, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
G1 = os.path.join(ROOT, "docs/sprint/w5/G1")


def load(path):
    with open(path, encoding="utf-8") as f:
        return list(csv.DictReader(f, delimiter="\t"))


def tally(rows):
    """task -> (runs, passes, run numbers)."""
    out = collections.OrderedDict()
    for r in rows:
        runs, passes, seen = out.get(r["task"], (0, 0, []))
        out[r["task"]] = (runs + 1, passes + (r["pass"] == "1"), seen + [r["run"]])
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dir", default=G1)
    ap.add_argument("--v2", default="cdp-layer-v2.tsv")
    ap.add_argument("--v3", default="cdp-layer-v3.tsv")
    ap.add_argument("--report", default=None)
    a = ap.parse_args()
    v2, v3 = load(os.path.join(a.dir, a.v2)), load(os.path.join(a.dir, a.v3))
    problems = []
    pins = {r["chrome_ver"] for r in v2 + v3}
    if len(pins) != 1:
        problems.append(f"chrome_ver differs between the sets: {sorted(pins)}")
    t2, t3 = tally(v2), tally(v3)
    tasks = list(t2) + [t for t in t3 if t not in t2]
    lines = ["| task | v2 passes | v3 passes | abs(v2-v3) | v2 runs | v3 runs |", "|---|---|---|---|---|---|"]
    tot2 = tot3 = 0
    for t in tasks:
        n2, p2, s2 = t2.get(t, (0, 0, []))
        n3, p3, s3 = t3.get(t, (0, 0, []))
        tot2, tot3 = tot2 + p2, tot3 + p3
        lines.append(f"| {t} | {p2}/{n2} | {p3}/{n3} | {abs(p2 - p3)} | {n2} | {n3} |")
        for name, n, s in (("v2", n2, s2), ("v3", n3, s3)):
            if n != 3:
                problems.append(f"{t}: {name} has {n} rows, need 3")
            if len(set(s)) != len(s):
                problems.append(f"{t}: {name} repeats a run number {s}")
    lines.append(f"| **total** | {tot2}/{len(v2)} | {tot3}/{len(v3)} | {abs(tot2 - tot3)} | | |")
    verdict = "GATE PASS" if not problems else "GATE RED"
    body = [f"# W5.G1.b.a.a.a: cdp-layer v2 vs v3 at one tree and one pin ({verdict})", "",
            f"chrome_ver={sorted(pins)[0] if len(pins) == 1 else pins}. v2 = `{a.v2}`, v3 = `{a.v3}`. Same 10 tasks x 3 runs.", "",
            *lines, ""]
    body += ["Problems:"] + [f"- {p}" for p in problems] if problems else ["No problems: every task has 3 rows in each set."]
    text = "\n".join(body) + "\n"
    print(text)
    if a.report:
        with open(a.report, "w", encoding="utf-8") as f:
            f.write(text)
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
