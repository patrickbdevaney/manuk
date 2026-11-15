#!/usr/bin/env python3
"""W5.G1.b.b.a: every failure-class count and the top-2 claim in docs/sprint/w5/G1.md must equal what
docs/sprint/w5/G1/causes.tsv says. Exit 1 on any mismatch, so an edited count goes red.

  w5g1_counts_check.py [--md docs/sprint/w5/G1.md] [--causes docs/sprint/w5/G1/causes.tsv]
                       [--causes-a docs/sprint/w5/G1/causes-v2.tsv]

(b) is counted from --causes; (a) from --causes-a, the causes file derived from cdp-layer-v2.tsv (W5.G1.b.b.a.a).

Checked in G1.md:
- one bullet per class, "- `<class>`: ... **N** in (a), **M** in (b)." (exactly two bold numbers per bullet);
- the "**Top 2 failure classes" line: "(a) ... ; (b) `X` (n) then `Y` (m)", or "(a) none" when (a) has no verified rows.
"""
import argparse, collections, csv, os, re, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
CLASSES = ["perceive", "target", "act", "tool-choice", "turn-limit"]
ARMS = {"a": "cdp-layer", "b": "cdp-layer+pwfmt"}


def causes_counts(path):
    counts = {arm: collections.Counter() for arm in ARMS.values()}
    with open(path) as fh:
        for row in csv.DictReader(fh, delimiter="\t"):
            if row["class"] not in CLASSES:
                sys.exit(f"FAIL: causes.tsv row has unknown class {row['class']!r} ({row['arm']} {row['task']} r{row['run']})")
            counts[row["arm"]][row["class"]] += 1
    return counts


def md_bullets(text, cls):
    # a bullet is its "- `cls`:" line plus the indented continuation lines under it
    out, lines = [], text.splitlines()
    for i, ln in enumerate(lines):
        if ln.startswith(f"- `{cls}`:"):
            body = [ln]
            for nxt in lines[i + 1:]:
                if not nxt.startswith("  "):
                    break
                body.append(nxt)
            out.append(" ".join(body))
    return out


def md_class_counts(text):
    found = {}
    for cls in CLASSES:
        bullets = md_bullets(text, cls)
        if len(bullets) != 1:
            return None, f"class `{cls}`: expected one bullet, found {len(bullets)}"
        nums = [int(n) for n in re.findall(r"\*\*(\d+)\*\*", bullets[0])]
        if len(nums) != 2:
            return None, f"class `{cls}`: expected two bold counts (a),(b), found {len(nums)}"
        found[cls] = {"a": nums[0], "b": nums[1]}
    return found, None


def top_two(counter):
    # ties broken by CLASSES order, the same order the report prints
    ranked = sorted(CLASSES, key=lambda c: (-counter[c], CLASSES.index(c)))
    return [(c, counter[c]) for c in ranked[:2] if counter[c] > 0]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--md", default=os.path.join(ROOT, "docs/sprint/w5/G1.md"))
    ap.add_argument("--causes", default=os.path.join(ROOT, "docs/sprint/w5/G1/causes.tsv"))
    ap.add_argument("--causes-a", default=os.path.join(ROOT, "docs/sprint/w5/G1/causes-v2.tsv"))
    a = ap.parse_args()
    counts = causes_counts(a.causes)
    counts[ARMS["a"]] = causes_counts(a.causes_a)[ARMS["a"]]  # (a) is the v2 rows; causes.tsv has no (a) rows
    text = open(a.md).read()
    bad = []

    found, err = md_class_counts(text)
    if err:
        sys.exit("FAIL: " + err)
    for cls in CLASSES:
        for key, arm in ARMS.items():
            want, got = counts[arm][cls], found[cls][key]
            if want != got:
                bad.append(f"`{cls}` ({key}) {arm}: G1.md says {got}, causes.tsv says {want}")

    top = [ln for ln in text.splitlines() if ln.startswith("**Top 2 failure classes")]
    if len(top) != 1:
        bad.append(f"top-2 line: expected one '**Top 2 failure classes' line, found {len(top)}")
    else:
        for key, arm in ARMS.items():
            pairs = top_two(counts[arm])
            if not pairs:
                if f"({key}) none" not in top[0]:
                    bad.append(f"top-2 ({key}): causes.tsv has no rows for {arm}, G1.md must say '({key}) none'")
                continue
            want = " then ".join(f"`{c}` ({n})" for c, n in pairs)
            if want not in top[0]:
                bad.append(f"top-2 ({key}): causes.tsv gives {want}, not found in G1.md")

    if bad:
        print("FAIL: G1.md does not match causes.tsv:\n  " + "\n  ".join(bad))
        sys.exit(1)
    print("PASS: every class count and the top-2 claim in G1.md match causes.tsv")


if __name__ == "__main__":
    main()
