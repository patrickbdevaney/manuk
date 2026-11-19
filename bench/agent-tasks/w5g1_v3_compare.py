#!/usr/bin/env python3
"""W5.G1.b.a.a.a: the v2 vs v3 band for the cdp-layer arm, at one tree (HEAD) and one chrome_ver.
W5.G1.b.a.a.a.a: the same report for a pre-77b97d40 set (--v2 cdp-layer-pre77.tsv), plus the band check and the build check.
W5.G1.b.a.a.a.a.a.b.a: --v2 and --v3 take comma-separated lists; each side is pooled across its files.

  w5g1_v3_compare.py [--dir docs/sprint/w5/G1] [--v2 A.tsv[,B.tsv...]] [--v3 C.tsv[,D.tsv...]] [--report F]
                     [--band-pair cdp-layer-v2.tsv,cdp-layer-v3.tsv] [--builds pre77-build.tsv]

Per task: passes in each side (pooled over that side's files), |v2-v3|. Exit 1 (RED) if:
  - a file has a row count other than its expected --runs-per-task for some task it carries, or the rows of one
    task in one file differ in count from the other tasks' counts in that same file (no mixed files);
  - a run number repeats for a task within one file;
  - the same (ts, task, run) row appears in two different files (a copy renamed as another set);
  - the TSVs carry different chrome_ver values;
  - --min-gap is given and a named gap task falls below it (or is missing from a side).

--runs-per-task: one integer (every file expects that many rows per task), or a comma list with one integer per
file in the order --v2 files then --v3 files (e.g. "3,9,3,9" for the pooled gomail-3 sets). Default 3.

--min-gap N --gap-task T (repeatable, or comma-separated): the pre-registered gap clause. For each named task, the
pooled per-task gap |passes(v2) - passes(v3)| must be at least N, else RED. A named task missing from either side is RED.
Without --min-gap the gap is reported but not checked.

--task T (repeatable, or comma-separated): keep only rows of these tasks, before every check (e.g. gomail-3).

--band-pair (default: the HEAD v2/v3 pair): exactly two single files, band = |total(first) - total(second)| in runs.
The --v2 total is "outside the band" iff it differs from BOTH HEAD totals by more than that band. This is the
pre-registered reading for W5.G1.b.a.a.a.a; it is reported, and it does not change the exit code.

--builds (default docs/sprint/w5/G1/pre77-build.tsv when it exists): one row per build with columns
role, path, sha256, marker_count, commit. RED if the pre-77 and HEAD shas are equal, if the pre-77 binary carries the
77b97d40 marker string, or if the HEAD binary does not. Pool only sets with a hashed build record; the report names
the files so a reader can see which side each came from.
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


def split_list(spec):
    return [s for s in (spec or "").split(",") if s]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dir", default=G1)
    ap.add_argument("--v2", default="cdp-layer-v2.tsv")
    ap.add_argument("--v3", default="cdp-layer-v3.tsv")
    ap.add_argument("--band-pair", default="cdp-layer-v2.tsv,cdp-layer-v3.tsv")
    ap.add_argument("--builds", default=None)
    ap.add_argument("--report", default=None)
    ap.add_argument("--runs-per-task", default="3")
    ap.add_argument("--min-gap", type=int, default=None)
    ap.add_argument("--gap-task", action="append", default=[])
    ap.add_argument("--task", action="append", default=[], help="keep only these tasks (repeatable or comma-separated)")
    a = ap.parse_args()
    gap_tasks = [t for g in a.gap_task for t in g.split(",") if t]
    if a.min_gap is not None and not gap_tasks:
        ap.error("--min-gap needs at least one --gap-task")

    names2, names3 = split_list(a.v2), split_list(a.v3)
    if not names2 or not names3:
        ap.error("--v2 and --v3 need at least one file each")
    files = [(n, "v2") for n in names2] + [(n, "v3") for n in names3]
    per_file = split_list(a.runs_per_task)
    if len(per_file) == 1:
        expect = [int(per_file[0])] * len(files)
    elif len(per_file) == len(files):
        expect = [int(x) for x in per_file]
    else:
        ap.error(f"--runs-per-task needs 1 or {len(files)} values, got {len(per_file)}")

    problems = []
    loaded = []  # (name, side, expected, rows)
    keep = set(split_list(",".join(a.task)))
    for (name, side), n in zip(files, expect):
        rows = load(os.path.join(a.dir, name))
        if keep:
            rows = [r for r in rows if r["task"] in keep]
        loaded.append((name, side, n, rows))
    if keep and not any(rows for *_, rows in loaded):
        problems.append(f"--task {sorted(keep)} matches no rows in any file")

    pins = {r["chrome_ver"] for _, _, _, rows in loaded for r in rows}
    if len(pins) != 1:
        problems.append(f"chrome_ver differs between the sets: {sorted(pins)}")

    # Per-file checks: expected count, equal counts per task within the file, no repeated run number.
    seen_rows = {}
    names_all = [name for name, _ in files]
    for name in sorted({x for x in names_all if names_all.count(x) > 1}):
        problems.append(f"{name} is listed {names_all.count(name)} times (a copy, not a new run)")
    for idx, (name, side, n, rows) in enumerate(loaded):
        per_task = tally(rows)
        for t, (cnt, _, runs) in per_task.items():
            if cnt != n:
                problems.append(f"{name}: {t} has {cnt} rows, need {n}")
            if len(set(runs)) != len(runs):
                problems.append(f"{name}: {t} repeats a run number {runs}")
        for r in rows:
            key = (r["ts"], r["task"], r["run"])
            if key in seen_rows and seen_rows[key] != idx:
                problems.append(f"row {key} appears in both {loaded[seen_rows[key]][0]} and {name} (a copy, not a new run)")
            seen_rows.setdefault(key, idx)

    def pooled(side):
        rows = [r for _, s, _, rs in loaded if s == side for r in rs]
        return tally(rows), rows

    t2, v2rows = pooled("v2")
    t3, v3rows = pooled("v3")
    tot2, tot3 = sum(p for _, p, _ in t2.values()), sum(p for _, p, _ in t3.values())
    len2, len3 = len(v2rows), len(v3rows)
    tasks = list(t2) + [t for t in t3 if t not in t2]
    lines = ["| task | v2 passes | v3 passes | abs(v2-v3) | v2 runs | v3 runs |", "|---|---|---|---|---|---|"]
    for t in tasks:
        n2, p2, _ = t2.get(t, (0, 0, []))
        n3, p3, _ = t3.get(t, (0, 0, []))
        lines.append(f"| {t} | {p2}/{n2} | {p3}/{n3} | {abs(p2 - p3)} | {n2} | {n3} |")
    lines.append(f"| **total** | {tot2}/{len2} | {tot3}/{len3} | {abs(tot2 - tot3)} | | |")

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
        pair = split_list(a.band_pair)
        if len(pair) != 2:
            ap.error("--band-pair takes exactly two single files")
        fr, sr = load(os.path.join(a.dir, pair[0])), load(os.path.join(a.dir, pair[1]))
        if keep:
            fr, sr = [r for r in fr if r["task"] in keep], [r for r in sr if r["task"] in keep]
        pins |= {r["chrome_ver"] for r in fr + sr}
        if len(pins) != 1:
            problems.append(f"chrome_ver differs between the band pair and the sets: {sorted(pins)}")
        bt1 = sum(p for _, p, _ in tally(fr).values())
        bt2 = sum(p for _, p, _ in tally(sr).values())
        band = abs(bt1 - bt2)
        outside = abs(tot2 - bt1) > band and abs(tot2 - bt2) > band
        band_lines = [f"Band ({pair[0]} {bt1}/{len(fr)} vs {pair[1]} {bt2}/{len(sr)}): {band} runs. "
                      f"The --v2 total {tot2}/{len2} is {'OUTSIDE' if outside else 'INSIDE'} the band "
                      f"(outside = more than {band} runs from both HEAD totals)."]

    build_lines = []
    builds = a.builds or (os.path.join(a.dir, "pre77-build.tsv") if os.path.exists(os.path.join(a.dir, "pre77-build.tsv")) else None)
    if builds:
        bp = check_builds(builds)
        problems += bp
        build_lines = [f"Build check ({os.path.basename(builds)}): " + ("ok, the two binaries differ and carry the right marker" if not bp else "failed")]

    src_lines = ["Sources (pooled per side):"] + [
        f"- {side} `{name}`: {len(rows)} rows, {sum(r['pass'] == '1' for r in rows)} passes, expects {n} per task"
        for name, side, n, rows in loaded]

    verdict = "GATE PASS" if not problems else "GATE RED"
    shown_pin = sorted(pins)[0] if len(pins) == 1 else pins
    body = [f"# W5.G1.b.a.a.a.a: cdp-layer pre-77b97d40 vs HEAD v2/v3 at one pin ({verdict})", "",
            f"chrome_ver={shown_pin}. Set A = `{','.join(names2)}`, set B = `{','.join(names3)}` (pooled per side).", "",
            *lines, "", *band_lines, *gap_lines, *build_lines, "", *src_lines, ""]
    body += ["Problems:"] + [f"- {p}" for p in problems] if problems else ["No problems: every file has the expected rows per task, and no row is shared between files."]
    text = "\n".join(body) + "\n"
    print(text)
    if a.report:
        with open(a.report, "w", encoding="utf-8") as f:
            f.write(text)
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
