#!/usr/bin/env python3
"""W5.T3.a.a: join the before/after per-turn TSVs written by T3aa-perturn.py, refusing a join whose binary does not match.

Each TSV row carries bin_sha256 (the MCP binary the arm ran). This script recomputes the sha of the binary file it is
given and refuses unless (a) every row of the TSV carries that sha, (b) the sha is the one docs/sprint/w5/T3aa-binaries.tsv
records for the commit the arm claims (before = bb497243, after = 3c0d499d), and (c) one chrome_ver and one claude
version run through both files. Any mismatch exits 2 with no numbers printed.

Usage: python3 docs/sprint/w5/T3aa-join.py --before-tsv A --after-tsv B --before-bin BIN --after-bin BIN
"""
import argparse, collections, csv, hashlib, os, statistics, sys

HERE = os.path.dirname(os.path.abspath(__file__))
MANIFEST = os.path.join(HERE, "T3aa-binaries.tsv")


def sha(path):
    with open(path, "rb") as f:
        return hashlib.sha256(f.read()).hexdigest()[:16]


def manifest(path=MANIFEST):
    with open(path) as f:
        return {r["arm"]: r for r in csv.DictReader(f, delimiter="\t")}


def load(path, arm, bin_sha):
    with open(path) as f:
        rows = [r for r in csv.DictReader(f, delimiter="\t") if r["arm"] == arm]
    if not rows:
        sys.exit(f"refuse: {path} has no rows for arm {arm}")
    for r in rows:
        if r["bin_sha256"] != bin_sha:
            sys.exit(f"refuse: {path} row sha {r['bin_sha256']} != binary sha {bin_sha} for arm {arm}")
    return rows


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--before-tsv", required=True)
    ap.add_argument("--after-tsv", required=True)
    ap.add_argument("--before-bin", required=True)
    ap.add_argument("--after-bin", required=True)
    ap.add_argument("--manifest", default=MANIFEST, help="arm/commit/sha table (default T3aa-binaries.tsv)")
    a = ap.parse_args()
    m = manifest(a.manifest)
    arms = {"before-W5.T3": (a.before_tsv, a.before_bin), "after-W5.T3": (a.after_tsv, a.after_bin)}
    rows = {}
    for arm, (tsv, binp) in arms.items():
        got = sha(binp)
        if got != m[arm]["bin_sha256"]:
            sys.exit(f"refuse: {binp} sha {got} is not the {arm} binary ({m[arm]['commit']} = {m[arm]['bin_sha256']})")
        rows[arm] = load(tsv, arm, got)
    pins = {r["chrome_ver"] for rs in rows.values() for r in rs}
    vers = {r["claude_version"] for rs in rows.values() for r in rs}
    if len(pins) != 1 or len(vers) != 1:
        sys.exit(f"refuse: mixed chrome_ver {pins} or claude version {vers}")
    print(f"chrome_ver={pins.pop()} claude={vers.pop()}")
    for arm, rs in rows.items():
        per = collections.defaultdict(list)
        for r in rs:
            per[int(r["run"])].append(r)
        totals = [sum(int(r["context_tokens"]) for r in v) for v in per.values()]
        firsts = [int(min(v, key=lambda r: int(r["turn"]))["context_tokens"]) for v in per.values()]
        ts_runs = sum(1 for v in per.values() if any(int(r["toolsearch"]) for r in v))
        sd = lambda xs: statistics.stdev(xs) if len(xs) > 1 else 0.0  # noqa: E731
        observe_runs = sum(1 for v in per.values() if any("observe" in r["tools"] for r in v))
        print(f"{arm} n={len(per)} runs_with_ToolSearch={ts_runs} runs_with_observe={observe_runs}")
        print(f"  task context total  mean={statistics.mean(totals):.0f} sd={sd(totals):.0f} "
              f"min={min(totals)} max={max(totals)}")
        print(f"  first-turn context  mean={statistics.mean(firsts):.0f} sd={sd(firsts):.0f}")
        deltas = []
        for v in per.values():  # context of the turn after the ToolSearch-calling turn, minus that turn
            ctx = {int(r["turn"]): int(r["context_tokens"]) for r in v}
            k = min((int(r["turn"]) for r in v if int(r["toolsearch"])), default=None)
            if k is not None and k + 1 in ctx:
                deltas.append(ctx[k + 1] - ctx[k])
        if deltas:
            print(f"  ToolSearch-turn delta mean={statistics.mean(deltas):.0f} sd={sd(deltas):.0f} "
                  f"min={min(deltas)} max={max(deltas)} n={len(deltas)}")


if __name__ == "__main__":
    main()
