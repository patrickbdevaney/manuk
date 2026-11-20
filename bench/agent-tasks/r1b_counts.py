#!/usr/bin/env python3
"""W6.R1.b — Chrome (and manuk) target-count distribution across repeated R1 captures.

  python3 -I r1b_counts.py [--ref SITE=COUNT ...] DIR [DIR ...]

Reads capture JSON (one per site and run, written by r1_agf.py capture) and prints one row per (site, run) with the
Chrome and manuk scorable-target counts, then per-site distributions and the falsifier verdict.

Falsifier (W6.R1.b node row): "a count that never returns to 49 means the run-1 count is the outlier". Computed, not
read by hand: for each site given a reference count with --ref (the node names fly-unified=49), run1_outlier is True iff
run 1's Chrome count equals the reference and no later run returns to it (run 1 is the only reference-count run).
Exit status 1 when any site has run1_outlier=True, so the check can go red. Sites without --ref are reported only.
"""
import collections, glob, json, os, sys


def load(dirs):
    recs = []
    for d in dirs:
        for p in glob.glob(os.path.join(d, "*_r*.json")):
            with open(p) as f:
                r = json.load(f)
            r["_path"] = p
            recs.append(r)
    recs.sort(key=lambda r: (r["site"], r["run"]))
    return recs


def verdicts(recs, refs):
    out = {}
    by_site = collections.defaultdict(list)
    for r in recs:
        by_site[r["site"]].append(r)
    for site, rs in sorted(by_site.items()):
        counts = [len(r["chrome"]) for r in rs]
        modal, _ = collections.Counter(counts).most_common(1)[0]
        ref = refs.get(site)
        run1_outlier = ref is not None and counts[0] == ref and ref not in counts[1:]
        out[site] = {"runs": len(rs), "chrome_seq": counts, "modal": modal, "ref": ref,
                     "dist": dict(sorted(collections.Counter(counts).items())),
                     "chrome_ver": sorted({r["chrome_ver"] for r in rs}),
                     "chrome_pin": sorted({r["chrome_pin"] for r in rs}),
                     "run1_outlier": run1_outlier}
    return out


def main(argv):
    refs = {}
    while len(argv) >= 2 and argv[0] == "--ref":
        site, _, n = argv[1].partition("=")
        refs[site] = int(n)
        argv = argv[2:]
    if not argv:
        sys.exit("usage: r1b_counts.py [--ref SITE=COUNT ...] DIR [DIR ...]")
    recs = load(argv)
    if not recs:
        sys.exit("no captures found")
    print("site\trun\tchrome_count\tmanuk_count\tchrome_ver\tchrome_pin")
    for r in recs:
        print(f"{r['site']}\t{r['run']}\t{len(r['chrome'])}\t{len(r['manuk'])}\t{r['chrome_ver']}\t{r['chrome_pin']}")
    v = verdicts(recs, refs)
    print()
    print("site\truns\tchrome_seq\tmodal\tref\tdistribution\trun1_outlier")
    red = False
    for site, x in v.items():
        print(f"{site}\t{x['runs']}\t{x['chrome_seq']}\t{x['modal']}\t{x['ref']}\t{x['dist']}\t{x['run1_outlier']}")
        red = red or x["run1_outlier"]
    print("VERDICT:", "RED (run-1 count is the outlier)" if red else "GREEN (no site has a run-1 outlier)")
    sys.exit(1 if red else 0)


if __name__ == "__main__":
    main(sys.argv[1:])
