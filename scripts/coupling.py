#!/usr/bin/env python3
"""Change-coupling matrix, mined from THIS repo's own history (METHODOLOGY 3.3).

Which files actually change together? Not which files *look* related — which ones the commit
history says are. That is the empirical partition: work may be parallelised only *within* a
low-coupling cluster, and gates need only be re-run for the files a diff historically drags along.

Guessing this partition is how you get a merge conflict on every tick. Chromium's architecture is
not ours; neither is our own intuition about our architecture, which is a year out of date the
moment it is written down.

    scripts/coupling.py [--since <rev>] [--top N]

Output: the highest-coupled file pairs, and the low-coupling clusters that are safe to fan out.
"""
import subprocess
import sys
from collections import defaultdict
from itertools import combinations

INTERESTING = (".rs",)
# A commit touching more than this many files is a sweep (a rename, a lint pass, a reformat). It
# tells you nothing about coupling and would make everything look coupled to everything.
MAX_FILES_PER_COMMIT = 25


def commits(since=None):
    rng = [f"{since}..HEAD"] if since else []
    out = subprocess.run(
        ["git", "log", "--name-only", "--pretty=format:---%H---", *rng],
        capture_output=True, text=True, check=True,
    ).stdout
    cur = []
    for line in out.splitlines():
        if line.startswith("---") and line.endswith("---"):
            if cur:
                yield cur
            cur = []
        elif line.strip() and line.endswith(INTERESTING):
            cur.append(line.strip())
    if cur:
        yield cur


def main():
    since = None
    top = 20
    args = sys.argv[1:]
    if "--since" in args:
        since = args[args.index("--since") + 1]
    if "--top" in args:
        top = int(args[args.index("--top") + 1])

    pair = defaultdict(int)
    solo = defaultdict(int)
    n = 0
    for files in commits(since):
        files = sorted(set(files))
        if not files or len(files) > MAX_FILES_PER_COMMIT:
            continue
        n += 1
        for f in files:
            solo[f] += 1
        for a, b in combinations(files, 2):
            pair[(a, b)] += 1

    print(f"# change-coupling over {n} commits\n")
    print("## Most-coupled pairs — a change to one has historically dragged the other along")
    print(f"{'coupling':>9}  {'both':>5} {'a':>5} {'b':>5}  files")
    rows = []
    for (a, b), c in pair.items():
        # A pair that has only co-occurred once or twice is noise, not coupling. Filter FIRST,
        # then rank — ranking first lets a single accidental co-commit (Jaccard 1.0) crowd out
        # every real signal.
        if c < 3:
            continue
        # Jaccard: how often they change together, out of the times either changes at all.
        j = c / (solo[a] + solo[b] - c)
        rows.append((j, c, a, b))
    rows.sort(reverse=True)
    for j, c, a, b in rows[:top]:
        print(f"{j:>8.2f}  {c:>5} {solo[a]:>5} {solo[b]:>5}  {a}\n{'':>29}{b}")

    print("\n## Files that mostly change ALONE — safe to fan out in parallel")
    lone = []
    for f, c in solo.items():
        if c < 3:
            continue
        # Highest coupling this file has with anything.
        worst = max((pair.get(tuple(sorted((f, g))), 0) / (c + solo[g] - pair.get(tuple(sorted((f, g))), 0))
                     for g in solo if g != f), default=0.0)
        lone.append((worst, c, f))
    lone.sort()
    for worst, c, f in lone[:top]:
        print(f"  max-coupling {worst:.2f}  ({c} commits)  {f}")

    print("\nRule (METHODOLOGY 5.4): a change may batch into one verify cycle with another IFF")
    print("their file sets have coupling ~0 here AND neither touches a frozen shared type.")


if __name__ == "__main__":
    main()
