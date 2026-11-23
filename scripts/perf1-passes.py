#!/usr/bin/env python3
"""PERF.1.2.a.a: how many full cascade+layout passes does each load phase run?

manuk arm only. Runs target/release/a11y-dump on each URL with MANUK_NET_TRACE=<file>, then groups the
`cascade` and `layout` lines by the `phase` line they fall in.

Attribution rule: every `phase` line is written when its phase ENDS and carries the phase's duration, and
the phases run back to back, so a cascade/layout line whose timestamp t lies in (end of previous phase,
end of this phase] ran inside this phase. Lines after the last phase line are reported as
`<after last phase>` (the `mark` lines are written there).

One restyle_and_layout call writes one `cascade` line and one `layout` line (`layout 0` when the
pass-memo answered). So for each run:  passes == cascade lines == layout lines, and the table's pass
totals must equal those counts exactly; the script refuses to print a table that does not reconcile.

Classes (a trace cannot tell a forced reflow from a script mutation; this only says WHERE the pass ran):
  script-window : inside a phase where page scripts run (blocking/deferred/dynamic scripts, DOMContentLoaded,
                  load event). A pass here is either a forced reflow answering a geometry read or a
                  restyle after a script mutation.
  loader-window : any other phase (CSS, images, subframes, page fetches). No script runs there, so a pass
                  is a restyle driven by a finished resource.

Usage (run with -I):
  python3 -I scripts/perf1-passes.py --runs 3 --out docs/sprint/perf1-passes
  python3 -I scripts/perf1-passes.py --self-test
Env: MANUK_CHROME_PIN is recorded in the output (every measurement row carries chrome_ver).
"""
import argparse
import bisect
import json
import os
import statistics
import subprocess
import sys
import tempfile
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
BINARY = os.path.join(ROOT, "target", "release", "a11y-dump")
URLS = [
    ("comix.to", "https://comix.to/"),
    ("puentedemando.com", "https://www.puentedemando.com/"),
    ("Alan_Turing", "https://en.wikipedia.org/wiki/Alan_Turing"),
]
SCRIPT_WINDOWS = {
    "external scripts",
    "cascade+layout+blocking scripts",
    "deferred scripts",
    "DOMContentLoaded",
    "load event",
    "finish_loading: dynamic scripts",
}
AFTER = "<after last phase>"


def parse_trace(text):
    """-> (phases, passes): phases = [(t, name, dur_us)] sorted by t; passes = [(t, kind, us)]."""
    phases, passes = [], []
    for ln in text.splitlines():
        f = ln.split("\t")
        if len(f) < 3:
            continue
        try:
            t = int(f[0])
        except ValueError:
            continue
        kind = f[1]
        if kind == "phase" and len(f) >= 4:
            try:
                phases.append((t, f[2], int(f[3])))
            except ValueError:
                pass
        elif kind == "cascade":
            passes.append((t, "cascade", int(f[2])))
        elif kind == "layout":
            passes.append((t, "layout", int(f[2])))  # "layout 0" = pass-memo hit
        elif kind == "reflow":
            passes.append((t, "reflow", 0))  # forced_reflow tags the pass written just before it
    phases.sort()
    return phases, passes


def group(text):
    """-> (buckets, counts). buckets: list of dicts in trace order, one per phase occurrence (plus
    AFTER if it has lines). counts: lines seen by kind, for reconciliation."""
    phases, passes = parse_trace(text)
    ends = [p[0] for p in phases]
    buckets = [
        {"seq": i, "phase": p[1], "class": "script-window" if p[1] in SCRIPT_WINDOWS else "loader-window",
         "cascade_n": 0, "layout_n": 0, "memo_hits": 0, "cascade_us": 0, "layout_us": 0, "forced_n": 0}
        for i, p in enumerate(phases)
    ]
    after = {"seq": len(phases), "phase": AFTER, "class": "-", "cascade_n": 0, "layout_n": 0,
             "memo_hits": 0, "cascade_us": 0, "layout_us": 0, "forced_n": 0}
    counts = {"cascade": 0, "layout": 0, "reflow": 0, "orphan_reflow": 0}
    prev = None
    for t, kind, us in passes:
        counts[kind] += 1
        if kind == "reflow" and prev != "layout":
            counts["orphan_reflow"] += 1  # a tag that does not follow a layout line tags no pass
        prev = kind
        i = bisect.bisect_left(ends, t)  # first phase whose end is >= t
        b = buckets[i] if i < len(phases) else after
        if kind == "reflow":
            b["forced_n"] += 1
        elif kind == "cascade":
            b["cascade_n"] += 1
            b["cascade_us"] += us
        else:
            b["layout_n"] += 1
            b["layout_us"] += us
            if us == 0:
                b["memo_hits"] += 1
    out = [b for b in buckets if b["cascade_n"] or b["layout_n"]]
    if after["cascade_n"] or after["layout_n"] or after["forced_n"]:
        out.append(after)
    return out, counts


def reconcile(buckets, counts):
    """Pass totals from the table must equal the trace line counts exactly. Returns a list of errors."""
    errs = []
    c_tot = sum(b["cascade_n"] for b in buckets)
    l_tot = sum(b["layout_n"] for b in buckets)
    if c_tot != counts["cascade"]:
        errs.append(f"cascade: buckets {c_tot} != trace lines {counts['cascade']}")
    if l_tot != counts["layout"]:
        errs.append(f"layout: buckets {l_tot} != trace lines {counts['layout']}")
    if counts["cascade"] != counts["layout"]:
        errs.append(f"trace has {counts['cascade']} cascade but {counts['layout']} layout lines")
    f_tot = sum(b["forced_n"] for b in buckets)
    if f_tot != counts["reflow"]:
        errs.append(f"forced: buckets {f_tot} != trace reflow lines {counts['reflow']}")
    if counts["orphan_reflow"]:
        errs.append(f"{counts['orphan_reflow']} reflow tag(s) do not follow a layout line")
    if counts["reflow"] > counts["cascade"]:
        errs.append(f"{counts['reflow']} forced tags for {counts['cascade']} passes")
    # forced + mutation == passes, per bucket and in total (mutation = passes - forced, never negative)
    for b in buckets:
        if b["forced_n"] > b["cascade_n"]:
            errs.append(f"{b['phase']}: forced {b['forced_n']} > passes {b['cascade_n']}")
    if sum(b["forced_n"] + (b["cascade_n"] - b["forced_n"]) for b in buckets) != counts["cascade"]:
        errs.append("forced + mutation != cascade lines")
    return errs


FIXTURES = [("forced-read.html", 1), ("no-read.html", 0)]


def fixture_check():
    """The known-geometry fixtures: forced-read.html reads geometry once in DOMContentLoaded (exactly 1
    forced pass), no-read.html does the same mutation without the read (0). Returns a list of errors."""
    errs = []
    for name, want in FIXTURES:
        path = os.path.join(ROOT, "docs", "sprint", "perf1-fixtures", name)
        tf = tempfile.NamedTemporaryFile(prefix="perf1-fx-", suffix=".tsv", delete=False)
        tf.close()
        subprocess.run([BINARY, "file://" + path], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                       env=dict(os.environ, MANUK_NET_TRACE=tf.name), timeout=60)
        text = open(tf.name, encoding="utf-8", errors="replace").read()
        os.unlink(tf.name)
        b, c = group(text)
        errs += [f"{name}: {e}" for e in reconcile(b, c)]
        if c["reflow"] != want:
            errs.append(f"{name}: {c['reflow']} forced passes, fixture has exactly {want} geometry read(s)")
    return errs


def run_one(url, timeout):
    tf = tempfile.NamedTemporaryFile(prefix="perf1-passes-", suffix=".tsv", delete=False)
    tf.close()
    env = dict(os.environ, MANUK_NET_TRACE=tf.name)
    t0 = time.monotonic()
    try:
        p = subprocess.run([BINARY, url], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                           env=env, timeout=timeout)
        status = "OK" if p.returncode == 0 else f"FAIL rc={p.returncode}"
    except subprocess.TimeoutExpired:
        status = "TIMEOUT"
    wall = time.monotonic() - t0
    text = open(tf.name, encoding="utf-8", errors="replace").read()
    os.unlink(tf.name)
    return status, wall, text


def fmt_ms(us):
    return f"{us / 1000:.1f}"


def self_test():
    # Phases run back to back: html parse ends at 100, scripts ends at 500, load ends at 600.
    good = "\n".join([
        "0\tepoch\t1",
        "100\tphase\thtml parse\t100",
        "150\tcascade\t20000",
        "170\tlayout\t30000",          # ends after 'html parse' (t=100), before 'scripts' (t=500)
        "380\tcascade\t40000",
        "420\tlayout\t0",
        "500\tphase\tcascade+layout+blocking scripts\t400",
        "600\tphase\tload event\t100",
        "650\tmark\tnavigate_returned",
        "700\tcascade\t9000",
        "710\tlayout\t9000",
    ])
    b, c = group(good)
    got = [(x["phase"], x["cascade_n"], x["layout_n"], x["memo_hits"]) for x in b]
    want = [("cascade+layout+blocking scripts", 2, 2, 1), (AFTER, 1, 1, 0)]
    assert got == want, f"grouping wrong: {got} != {want}"
    assert reconcile(b, c) == [], reconcile(b, c)
    # Mutant: a pass lost in the table (one bucket short of the trace's own line count) must go red.
    short = [dict(x) for x in b]
    short[0]["cascade_n"] -= 1
    assert reconcile(short, c), "a short table reconciled: the gate cannot go red"
    # Mutant: layout lines ignored by the grouping -> layout total is 0 vs trace count.
    no_layout = [dict(x, layout_n=0) for x in b]
    assert reconcile(no_layout, c), "missing layout lines reconciled: the gate cannot go red"
    # Forced/mutation split: a reflow tag follows the layout line of the pass it tags.
    tagged = good.replace("420\tlayout\t0", "420\tlayout\t0\n421\treflow\tforced")
    b2, c2 = group(tagged)
    assert reconcile(b2, c2) == [] and c2["reflow"] == 1 and b2[0]["forced_n"] == 1, (b2, c2)
    # Mutant: a tag written for no pass (after a cascade line, not a layout line) must go red.
    stray = good.replace("380\tcascade\t40000", "380\tcascade\t40000\n381\treflow\tforced")
    b3, c3 = group(stray)
    assert reconcile(b3, c3), "a reflow tag that follows no layout line reconciled"
    # Mutant: a tag for every pass (3 passes, 4 tags) must go red.
    every = tagged + "\n711\treflow\tforced\n712\treflow\tforced\n713\treflow\tforced"
    b4, c4 = group(every)
    assert reconcile(b4, c4), "more forced tags than passes reconciled"
    # Mutant: bucket loses its forced count -> disagrees with the trace's reflow lines.
    lost = [dict(x, forced_n=0) for x in b2]
    assert reconcile(lost, c2), "lost forced counts reconciled"
    print("self-test OK: grouping, reconciliation, forced split, and five red mutants")
    return 0


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--runs", type=int, default=3)
    ap.add_argument("--timeout", type=int, default=180)
    ap.add_argument("--out", default=os.path.join(ROOT, "docs", "sprint", "perf1-passes"))
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--fixture-check", action="store_true", help="run the known-geometry-read fixtures (needs the binary)")
    ap.add_argument("--from-json", help="re-render the table from a saved passes.json (no browser run)")
    a = ap.parse_args()
    if a.self_test:
        return self_test()
    if a.fixture_check:
        e = fixture_check()
        print("fixture-check", "OK" if not e else f"RED: {e}")
        return 1 if e else 0
    if a.from_json:
        with open(a.from_json) as fh:
            print(render(json.load(fh)))
        return 0
    pin = os.environ.get("MANUK_CHROME_PIN")
    if not pin:
        sys.exit("MANUK_CHROME_PIN is not set: every measurement row must carry chrome_ver")
    if not os.path.exists(BINARY):
        sys.exit(f"missing {BINARY}: cargo build --release -p manuk-agent --bin a11y-dump")
    os.makedirs(a.out, exist_ok=True)
    arm = {k: os.environ[k] for k in ("MANUK_NO_ADOPT_REFLOW", "MANUK_NO_PASS_MEMO") if k in os.environ}
    result = {"chrome_ver": pin, "binary": os.path.relpath(BINARY, ROOT), "runs": a.runs, "arm_env": arm,
              "urls": {}}
    bad = []
    fx = fixture_check()
    if fx:
        bad.append(("fixtures", 0, fx))
    print("fixture-check", "OK" if not fx else f"RED: {fx}", file=sys.stderr)
    for label, url in URLS:
        result["urls"][label] = {"url": url, "runs": []}
        for r in range(a.runs):
            status, wall, text = run_one(url, a.timeout)
            buckets, counts = group(text)
            errs = reconcile(buckets, counts)
            if not text.strip():
                errs.append("empty trace")
            if errs:
                bad.append((label, r, errs))
            result["urls"][label]["runs"].append({
                "run": r, "status": status, "wall_s": round(wall, 3), "counts": counts,
                "reconciled": not errs, "errors": errs, "buckets": buckets,
            })
            print(f"{label} run{r}: {status} wall={wall:.2f}s cascade={counts['cascade']} "
                  f"layout={counts['layout']} forced={counts['reflow']} mutation={counts['cascade'] - counts['reflow']} {'OK' if not errs else 'MISMATCH ' + '; '.join(errs)}",
                  file=sys.stderr)
    with open(os.path.join(a.out, "passes.json"), "w") as fh:
        json.dump(result, fh, indent=1)
    print(render(result))
    if bad:
        print(f"RECONCILIATION FAILED: {bad}", file=sys.stderr)
        return 1
    return 0


def occurrences(buckets):
    """(name, nth occurrence of that name) per bucket: the stable key across runs. Matching by sequence
    index would shift whenever one run emits a phase the others do not."""
    seen, out = {}, []
    for b in buckets:
        k = seen.get(b["phase"], 0)
        seen[b["phase"]] = k + 1
        out.append(((b["phase"], k), b))
    return out


def render(result):
    lines = [f"chrome_ver={result['chrome_ver']} binary={result['binary']} runs={result['runs']} "
             f"arm_env={result.get('arm_env', {})}", ""]
    for label, d in result["urls"].items():
        runs = d["runs"]
        lines += [f"### {label} ({d['url']})", ""]
        keyed = [dict(occurrences(r["buckets"])) for r in runs]
        keys = []
        for k in keyed:
            for key in k:
                if key not in keys:
                    keys.append(key)
        # trace order: the first run that has the phase defines its position
        order = {}
        for k in keyed:
            for pos, (key, b) in enumerate(sorted(k.items(), key=lambda kv: kv[1]["seq"])):
                order.setdefault(key, (len(order) if key not in order else order[key]))
        keys.sort(key=lambda key: min((k[key]["seq"] for k in keyed if key in k), default=0))
        hdr = "| phase (trace order) | window | " + " | ".join(f"passes r{r['run']}" for r in runs) + \
              " | total | forced | mutation | ms per pass (cascade+layout) | memo hits |"
        lines += [hdr, "|" + "---|" * (2 + len(runs) + 5)]
        for key in keys:
            name = key[0]
            per, tot_n, tot_us, memo, win, forced = [], 0, 0, 0, "-", 0
            for k in keyed:
                b = k.get(key)
                per.append(str(b["cascade_n"] if b else 0))
                if b:
                    tot_n += b["cascade_n"]
                    tot_us += b["cascade_us"] + b["layout_us"]
                    memo += b["memo_hits"]
                    forced += b["forced_n"]
                    win = b["class"]
            if not tot_n:
                continue
            ms = f"{tot_us / 1000 / tot_n:.1f}"
            label_name = name if key[1] == 0 else f"{name} (#{key[1] + 1})"
            lines.append(f"| {label_name} | {win} | " + " | ".join(per) + f" | {tot_n} | {forced} | {tot_n - forced} | {ms} | {memo} |")
        cells = [str(sum(b["cascade_n"] for b in r["buckets"])) for r in runs]
        lines.append("| **pass total** | | " + " | ".join(cells) + f" | {sum(int(c) for c in cells)} | {sum(r['counts']['reflow'] for r in runs)} | {sum(int(c) for c in cells) - sum(r['counts']['reflow'] for r in runs)} | | |")
        lines.append("| trace `cascade` lines | | " + " | ".join(str(r["counts"]["cascade"]) for r in runs) + " | | | | | |")
        lines.append("| trace `layout` lines | | " + " | ".join(str(r["counts"]["layout"]) for r in runs) + " | | | | | |")
        lines.append("| trace `reflow` (forced) lines | | " + " | ".join(str(r["counts"]["reflow"]) for r in runs) + " | | | | | |")
        lines.append("")
    return "\n".join(lines)


if __name__ == "__main__":
    sys.exit(main())
