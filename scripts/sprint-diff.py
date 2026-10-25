#!/usr/bin/env python3
"""Diff two sprint measurement TSVs (each with a chrome_ver column).

Refuses (exit 2) when the two sets carry different chrome_ver values, or when
either set mixes versions or lacks the column: numbers taken under different
oracle Chromes are never compared.  Usage: sprint-diff.py OLD.tsv NEW.tsv METRIC [KEY...]
Default KEY columns: url engine.  Prints per-key mean(metric) old/new/delta.
"""
import csv, sys, collections

def load(path):
    with open(path, newline="") as f:
        rows = list(csv.DictReader(f, delimiter="\t"))
    if not rows or "chrome_ver" not in rows[0]:
        sys.exit(f"REFUSED: {path} has no chrome_ver column")
    vers = {r["chrome_ver"] for r in rows}
    if "" in vers or len(vers) != 1:
        sys.exit(f"REFUSED: {path} chrome_ver not a single non-empty value: {sorted(vers)}")
    return vers.pop(), rows

def main(argv):
    if len(argv) < 4:
        sys.exit(__doc__)
    old_p, new_p, metric, *keys = argv[1:]
    keys = keys or ["url", "engine"]
    (ov, orows), (nv, nrows) = load(old_p), load(new_p)
    if ov != nv:
        print(f"REFUSED: chrome_ver differs ({ov} vs {nv}); re-baseline instead of diffing", file=sys.stderr)
        return 2
    def agg(rows):
        d = collections.defaultdict(list)
        for r in rows:
            try:
                d[tuple(r[k] for k in keys)].append(float(r[metric]))
            except ValueError:
                pass
        return {k: sum(v) / len(v) for k, v in d.items()}
    a, b = agg(orows), agg(nrows)
    print(f"chrome_ver={ov}\t{'\t'.join(keys)}\told\tnew\tdelta")
    for k in sorted(a.keys() & b.keys()):
        print("\t".join(["", *k, f"{a[k]:.3f}", f"{b[k]:.3f}", f"{b[k]-a[k]:+.3f}"]))
    return 0

if __name__ == "__main__":
    sys.exit(main(sys.argv))
