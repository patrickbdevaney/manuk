#!/usr/bin/env python3
"""W5.G1.b.a.a.a: the v2 vs v3 band for the cdp-layer arm, at one tree (HEAD) and one chrome_ver.
W5.G1.b.a.a.a.a: the same report for a pre-77b97d40 set (--v2 cdp-layer-pre77.tsv), plus the band check and the build check.

  w5g1_v3_compare.py [--dir docs/sprint/w5/G1] [--v2 cdp-layer-v2.tsv] [--v3 cdp-layer-v3.tsv] [--report F]
                     [--band-pair cdp-layer-v2.tsv,cdp-layer-v3.tsv] [--builds pre77-build.tsv]

Per task: passes in v2, passes in v3, |v2-v3|. Exit 1 (RED) if a task has a row count other than --runs-per-task
(default 3) in either TSV, if the TSVs carry different chrome_ver values, or if a row's run number repeats within a task.

--runs-per-task N (default 3): the expected row count per task in each TSV.

--min-gap N --gap-task T (repeatable, or comma-separated): the pre-registered gap clause. For each named task, the
per-task gap |passes(v2) - passes(v3)| must be at least N, else RED. A named task missing from either set is RED.
Without --min-gap the gap is reported but not checked.

--band-pair (default: the HEAD v2/v3 pair): band = |total(first) - total(second)| in runs. The --v2 total is
"outside the band" iff it differs from BOTH HEAD totals by more than that band. This is the pre-registered reading
for W5.G1.b.a.a.a.a; it is reported, and it does not change the exit code.

--builds (default docs/sprint/w5/G1/pre77-build.tsv when it exists): one row per build with columns
role, path, sha256, marker_count, commit. RED if the pre-77 and HEAD shas are equal, if the pre-77 binary carries the
77b97d40 marker string, or if the HEAD binary does not.
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


def check_builds(path):
    """Problems with the build record: the pre-77 binary must differ from HEAD's and carry no 77b97d40 marker."""
    rows = {r["role"]: r for r in load(path)}
    if set(rows) != {"pre77", "head"}:
        return [f"{path}: need one row each for pre77 and head, got {sorted(rows)}"]
    out = []
    if rows["pre77"]["sha256"] == rows["head"]["sha256"]:
        out.append("pre-77 manuk-mcp sha equals HEAD's: the two arms are the same binary")
    if int(rows["pre77"]["marker_count"]) != 0:
        out.append("pre-77 binary carries the 77b97d40 marker string (drop-form-containers)")
    if int(rows["head"]["marker_count"]) < 1:
        out.append("HEAD binary lacks the 77b97d40 marker string")
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dir", default=G1)
    ap.add_argument("--v2", default="cdp-layer-v2.tsv")
    ap.add_argument("--v3", default="cdp-layer-v3.tsv")
    ap.add_argument("--band-pair", default="cdp-layer-v2.tsv,cdp-layer-v3.tsv")
    ap.add_argument("--builds", default=None)
    ap.add_argument("--report", default=None)
    ap.add_argument("--runs-per-task", type=int, default=3)
    ap.add_argument("--min-gap", type=int, default=None)
    ap.add_argument("--gap-task", action="append", default=[])
    a = ap.parse_args()
    gap_tasks = [t for g in a.gap_task for t in g.split(",") if t]
    if a.min_gap is not None and not gap_tasks:
        ap.error("--min-gap needs at least one --gap-task")
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
            if n != a.runs_per_task:
                problems.append(f"{t}: {name} has {n} rows, need {a.runs_per_task}")
            if len(set(s)) != len(s):
                problems.append(f"{t}: {name} repeats a run number {s}")
    lines.append(f"| **total** | {tot2}/{len(v2)} | {tot3}/{len(v3)} | {abs(tot2 - tot3)} | | |")
    gap_lines = []
    for t in gap_tasks:
        if t not in t2 or t not in t3:
            problems.append(f"gap task {t} is missing from {'v2' if t not in t2 else 'v3'}")
            continue
        gap = abs(t2[t][1] - t3[t][1])
        gap_lines.append(f"Gap {t}: |{t2[t][1]} - {t3[t][1]}| = {gap} runs (min {a.min_gap}).")
        if a.min_gap is not None and gap < a.min_gap:
            problems.append(f"{t}: per-task gap {gap} is below --min-gap {a.min_gap}")
    band_lines = []
    if a.band_pair:
        first, second = a.band_pair.split(",")
        fr, sr = load(os.path.join(a.dir, first)), load(os.path.join(a.dir, second))
        pins |= {r["chrome_ver"] for r in fr + sr}
        if len(pins) != 1:
            problems.append(f"chrome_ver differs between the band pair and the sets: {sorted(pins)}")
        bt1 = sum(p for _, p, _ in tally(fr).values())
        bt2 = sum(p for _, p, _ in tally(sr).values())
        band = abs(bt1 - bt2)
        outside = abs(tot2 - bt1) > band and abs(tot2 - bt2) > band
        band_lines = [f"Band ({first} {bt1}/{len(fr)} vs {second} {bt2}/{len(sr)}): {band} runs. "
                      f"The --v2 total {tot2}/{len(v2)} is {'OUTSIDE' if outside else 'INSIDE'} the band "
                      f"(outside = more than {band} runs from both HEAD totals)."]
    build_lines = []
    builds = a.builds or (os.path.join(a.dir, "pre77-build.tsv") if os.path.exists(os.path.join(a.dir, "pre77-build.tsv")) else None)
    if builds:
        bp = check_builds(builds)
        problems += bp
        build_lines = [f"Build check ({os.path.basename(builds)}): " + ("ok, the two binaries differ and carry the right marker" if not bp else "failed")]
    verdict = "GATE PASS" if not problems else "GATE RED"
    body = [f"# W5.G1.b.a.a.a.a: cdp-layer pre-77b97d40 vs HEAD v2/v3 at one pin ({verdict})", "",
            f"chrome_ver={sorted(pins)[0] if len(pins) == 1 else pins}. Set A = `{a.v2}`, set B = `{a.v3}`. Same 10 tasks x 3 runs.", "",
            *lines, "", *band_lines, *gap_lines, *build_lines, ""]
    body += ["Problems:"] + [f"- {p}" for p in problems] if problems else ["No problems: every task has 3 rows in each set."]
    text = "\n".join(body) + "\n"
    print(text)
    if a.report:
        with open(a.report, "w", encoding="utf-8") as f:
            f.write(text)
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
