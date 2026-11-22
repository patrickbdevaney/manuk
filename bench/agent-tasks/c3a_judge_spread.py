#!/usr/bin/env python3 -I
"""W4.C3.a.a.a: judge repeatability. Reads the --out summaries of N judge passes over the SAME candidates
(c3a_bon_real.py, same --group, same chrome_ver) and reports, per arm, the BoN-3 pass count in each pass and the
spread (max - min). Gate (pre-registered in GRAPH.tsv W4.C3.a.a.a): PASS if every arm's spread <= 2 pt; RED if any
arm's spread >= 4 pt; a 3 pt spread is neither and is reported as GREY (not citable as PASS).
Usage: python3 -I c3a_judge_spread.py SUMMARY1.tsv SUMMARY2.tsv SUMMARY3.tsv [...]
"""
import sys

ARMS = ["manuk", "cdp-layer", "playwright"]


def read(path):
    pin, out = None, {}
    for l in open(path):
        if l.startswith("# chrome_ver="):
            pin = l.strip().split("=", 1)[1]
            continue
        f = l.rstrip("\n").split("\t")
        if f and f[0] in ARMS:
            # the BoN CI95 field ("lo-hi") sits right after BoN-3 and single; older summaries have no group column
            k = next(i for i, x in enumerate(f) if i > 2 and "-" in x and x.replace("-", "").isdigit())
            out[f[0]] = (int(f[k - 1]), int(f[k - 2]))  # (BoN-3 pass count, single pass count)
    return pin, out


def main(paths):
    if len(paths) < 2:
        sys.exit("need >= 2 summary files")
    passes = [read(p) for p in paths]
    pins = {p for p, _ in passes}
    if len(pins) != 1 or None in pins:
        sys.exit(f"refusing: chrome_ver differs or missing across passes: {pins}")
    print(f"chrome_ver={pins.pop()} passes={len(paths)}")
    print("arm\tBoN per pass\tBoN spread\tsingle per pass\tverdict")
    worst = 0
    for arm in ARMS:
        bon = [o[arm][0] for _, o in passes if arm in o]
        sing = [o[arm][1] for _, o in passes if arm in o]
        if not bon:
            continue
        spread = max(bon) - min(bon)
        worst = max(worst, spread)
        verdict = "PASS" if spread <= 2 else ("RED" if spread >= 4 else "GREY")
        print(f"{arm}\t{bon}\t{spread}\t{sing}\t{verdict}")
    verdict = "PASS" if worst <= 2 else ("RED" if worst >= 4 else "GREY")
    print(f"GATE\tworst spread {worst} pt\t{verdict}")


if __name__ == "__main__":
    main(sys.argv[1:])
