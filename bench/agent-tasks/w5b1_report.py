#!/usr/bin/env python3
"""W5.B1: wave-5 re-bench report. Four arms (manuk, cdp-layer, playwright, playwright+preload), two slices, Haiku, 3 runs.

  python3 -I bench/agent-tasks/w5b1_report.py [--root docs/sprint/w5] [--transcripts DIR] [--summary OUT.tsv]
  python3 -I bench/agent-tasks/w5b1_report.py --self-test

Inputs (written by docs/sprint/w5/b1-run.sh and docs/sprint/w5/b1a-run.sh):
  B1-real-<arm>.tsv   REAL judged slice: every non-omnizon task (102), real_run.py --judge
  B1-full-<arm>.tsv   own 40 + external 14 (the suite run_arm.py --suite full runs), run_arm.py
  B1-preload-real.tsv / B1-preload-full.tsv   the playwright+preload arm (W5.B1.a), same two slices

Integrity (exit 1 on any miss, the gate): each (arm, slice) has exactly one row per (task, run 1..3) for the expected
task set; every row carries chrome_ver == docs/sprint/CHROME_PIN; one judge_model per REAL slice; every row's
transcript exists under --transcripts (the tokens are only bucketable from transcripts; W5.T0.b).
Exit 2 = incomplete (a run is still in progress); the report is printed but the node is not DONE.

Verdicts (read against docs/sprint/SUCCESS.md Bar 2, rule copied from the W4.B4 report):
  2.1 success: |pass>=2of3 rate(manuk) - rate(playwright)| <= 10 points on the full slice AND the per-run bands overlap.
  2.2 tokens:  in+out tokens/task for manuk vs playwright (the incumbent; lower of playwright and playwright+preload
       when that arm exists, see W5.B1.a); PASS needs >= 30% fewer at equal-or-better success. cdp-layer is shown too.
"""
import argparse, csv, glob, json, os, sys, tempfile, shutil

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
ARMS = ["manuk", "cdp-layer", "playwright", "playwright+preload"]
SLICES = {"real": "B1-real-{arm}.tsv", "full": "B1-full-{arm}.tsv"}
PRELOAD_FILES = {"real": "B1-preload-real.tsv", "full": "B1-preload-full.tsv"}  # W5.B1.a: playwright+preload
RUNS = (1, 2, 3)


def fname(slice_, arm):
    return PRELOAD_FILES[slice_] if arm == "playwright+preload" else SLICES[slice_].format(arm=arm)


def expected_tasks(slice_):
    if slice_ == "real":
        out = []
        for f in sorted(glob.glob(os.path.join(HERE, "real", "tasks", "*.json"))):
            t = json.load(open(f))
            if t["website"]["id"] != "omnizon":  # REAL's HTTP 451 DMCA site, excluded for every arm
                out.append(t["id"])
        return out
    ids = []
    for name in ("suite-v1.json", "suite-v1-external.json"):
        ids += [t["id"] for t in json.load(open(os.path.join(HERE, name)))["tasks"]]
    return ids


def norm(row, slice_):
    g = lambda k: row.get(k) or "0"
    inp, out = int(g("input_tokens")), int(g("output_tokens"))
    cr, cc = int(g("cache_read_tokens")), int(g("cache_create_tokens"))
    return {"task": row["task"], "run": int(row["run"]), "pass": int(row["pass"] or 0), "chrome_ver": row["chrome_ver"],
            "judge": row.get("judge_model", ""), "in_out": inp + out, "total": inp + out + cr + cc,
            "cost": float(g("cost_usd")), "transcript": row.get("_tr", "")}


def load_slice(root, slice_, arm):
    path = os.path.join(root, fname(slice_, arm))
    if not os.path.exists(path):
        return None, path
    rows = list(csv.DictReader(open(path), delimiter="\t"))
    return rows, path


def check(root, transcripts, pin, errors, incomplete):
    data = {}
    for slice_ in SLICES:
        exp = set(expected_tasks(slice_))
        for arm in ARMS:
            rows, path = load_slice(root, slice_, arm)
            if rows is None:
                incomplete.append(f"{slice_}/{arm}: {os.path.basename(path)} missing")
                continue
            seen = {}
            for r in rows:
                if not r.get("task"):
                    continue
                r["_tr"] = os.path.join(transcripts, f"{arm}_{r['task']}_r{r['run']}.jsonl")
                k = (r["task"], r["run"])
                if k in seen:
                    errors.append(f"{slice_}/{arm}: duplicate row {k}")
                seen[k] = r
                if r["chrome_ver"] != pin:
                    errors.append(f"{slice_}/{arm}: {k} chrome_ver {r['chrome_ver']} != pin {pin}")
                if not os.path.exists(r["_tr"]):
                    errors.append(f"{slice_}/{arm}: {k} transcript missing ({r['_tr']})")
            want = {(t, str(n)) for t in exp for n in RUNS}
            missing = want - set(seen)
            extra = {k for k in seen if k[0] not in exp}
            if extra:
                errors.append(f"{slice_}/{arm}: {len(extra)} rows for tasks outside the expected set, e.g. {sorted(extra)[0]}")
            if missing:
                incomplete.append(f"{slice_}/{arm}: {len(missing)} of {len(want)} (task, run) rows not yet written")
            if slice_ == "real":
                if any(not r.get("judge_model") for r in seen.values()):
                    errors.append(f"real/{arm}: a row has no judge_model")
            data[(slice_, arm)] = [norm(r, slice_) for r in seen.values()]
    return data


def stats(rows, exp_n):
    per_task = {}
    for r in rows:
        per_task.setdefault(r["task"], []).append(r)
    passed = sum(1 for rs in per_task.values() if sum(x["pass"] for x in rs) >= 2)
    band = []
    for n in RUNS:
        band.append(sum(x["pass"] for x in rows if x["run"] == n))
    tok_io = [sum(x["in_out"] for x in rows if x["run"] == n) / max(1, sum(1 for x in rows if x["run"] == n)) for n in RUNS]
    tok_tot = [sum(x["total"] for x in rows if x["run"] == n) / max(1, sum(1 for x in rows if x["run"] == n)) for n in RUNS]
    return {"n": exp_n, "pass2": passed, "rate": 100.0 * passed / max(1, exp_n), "run_pass": band,
            "run_pass_band": (min(band), max(band)), "io_per_task": sum(tok_io) / 3,
            "io_band": (min(tok_io), max(tok_io)), "total_per_task": sum(tok_tot) / 3,
            "cost_per_task": sum(x["cost"] for x in rows) / max(1, len(rows))}


def report(data, summary_path):
    lines, out = [], []
    for slice_ in SLICES:
        lines.append(f"\n## slice {slice_}")
        lines.append("| arm | tasks | pass>=2of3 | rate | pass/run | in+out tok/task (run band) | total tok/task | USD/run |")
        lines.append("|---|---|---|---|---|---|---|---|")
        for arm in ARMS:
            rows = data.get((slice_, arm))
            if not rows:
                continue
            s = stats(rows, len({x["task"] for x in rows}))
            data[(slice_, arm, "stats")] = s
            lines.append(f"| {arm} | {s['n']} | {s['pass2']} | {s['rate']:.1f}% | {s['run_pass']} | {s['io_per_task']:.0f} "
                         f"[{s['io_band'][0]:.0f}, {s['io_band'][1]:.0f}] | {s['total_per_task']:.0f} | {s['cost_per_task']:.4f} |")
            out.append(("tokens_io_per_task", slice_, arm, s["io_per_task"], s["io_band"][0], s["io_band"][1]))
            out.append(("tokens_total_per_task", slice_, arm, s["total_per_task"], "", ""))
            out.append(("pass2of3_rate", slice_, arm, s["rate"], s["run_pass_band"][0], s["run_pass_band"][1]))
    judges = {}
    for arm in ARMS:
        for x in data.get(("real", arm), []):
            judges.setdefault(x["task"], set()).add(x["judge"])
    if judges:
        counts = {}
        for arm in ARMS:
            for x in data.get(("real", arm), []):
                counts[x["judge"]] = counts.get(x["judge"], 0) + 1
        mixed = sorted(t for t, js in judges.items() if len(js) > 1)
        lines.append(f"\nREAL judge rows: {counts}; tasks judged by more than one judge: {len(mixed)} {mixed[:12]}")
    fm, fp = data.get(("full", "manuk", "stats")), data.get(("full", "playwright", "stats"))
    fpp = data.get(("full", "playwright+preload", "stats"))
    if fpp:
        lines.append(f"Playwright+preload (full slice): {fpp['rate']:.1f}%, in+out {fpp['io_per_task']:.0f} per task")
    if fm and fp:
        d = fm["rate"] - fp["rate"]
        overlap = fm["run_pass_band"][0] <= fp["run_pass_band"][1] and fp["run_pass_band"][0] <= fm["run_pass_band"][1]
        ok21 = abs(d) <= 10 and overlap
        lines.append(f"\nBar 2.1 (full slice): manuk {fm['rate']:.1f}% vs playwright {fp['rate']:.1f}% -> diff {d:+.1f} pts, "
                     f"bands overlap={overlap} -> {'MET' if ok21 else 'NOT MET'}")
    rm, rp = data.get(("real", "manuk", "stats")), data.get(("real", "playwright", "stats"))
    if rm and rp:
        lines.append(f"REAL slice (not a Bar 2 input by itself): manuk {rm['rate']:.1f}% vs playwright {rp['rate']:.1f}%")
    tm = data.get(("full", "manuk", "stats"))
    tp = data.get(("full", "playwright", "stats"))
    tc = data.get(("full", "cdp-layer", "stats"))
    # W5.B1.a: the incumbent is the cheaper (in+out per task) of the two Playwright arms, as in W4.B4
    inc = min((x for x in (tp, fpp) if x), key=lambda x: x["io_per_task"], default=None)
    inc_name = "playwright+preload" if inc is fpp and fpp else "playwright"
    if tm and inc:
        red = 100.0 * (1 - tm["io_per_task"] / inc["io_per_task"])
        ok22 = red >= 30 and tm["rate"] >= inc["rate"]
        lines.append(f"Bar 2.2 tokens (full slice, in+out per task): manuk {tm['io_per_task']:.0f} vs {inc_name} {inc['io_per_task']:.0f} "
                     f"-> {abs(red):.1f}% {'fewer' if red >= 0 else 'more'} (need >=30% fewer at equal-or-better success) -> {'MET' if ok22 else 'NOT MET'}")
        if tc:
            lines.append(f"  (K1 arm 3, cdp-layer: {tc['io_per_task']:.0f} in+out per task; manuk uses {abs(100.0 * (1 - tm['io_per_task'] / tc['io_per_task'])):.1f}% {'fewer' if tm['io_per_task'] < tc['io_per_task'] else 'more'})")
        if tm["total_per_task"] and inc["total_per_task"]:
            lines.append(f"  (total incl. cache, manuk vs {inc_name}: {100.0 * (1 - tm['total_per_task'] / inc['total_per_task']):.1f}% fewer)")
    print("\n".join(lines))
    if summary_path:
        with open(summary_path, "w") as f:
            f.write("metric\tslice\tarm\tvalue\tband_lo\tband_hi\n")
            for row in out:
                f.write("\t".join(str(c) if not isinstance(c, float) else f"{c:.2f}" for c in row) + "\n")


def run(root, transcripts, summary, pin):
    errors, incomplete = [], []
    data = check(root, transcripts, pin, errors, incomplete)
    report(data, summary)
    for e in errors:
        print("INTEGRITY:", e)
    for m in incomplete:
        print("INCOMPLETE:", m)
    return 1 if errors else (2 if incomplete else 0)


def self_test(pin):
    """The gate must go red: a dropped row, a wrong chrome_ver and a deleted transcript each make the check fail.
    The unmutated copy must be clean (no integrity error and no missing row)."""
    src = os.path.join(ROOT, "docs/sprint/w5")
    if not all(os.path.exists(os.path.join(src, fname(s, a))) for s in SLICES for a in ARMS):
        print("self-test: B1 rows not complete yet"); return 2
    rows = {(s, a): [r for r in csv.DictReader(open(os.path.join(src, fname(s, a))), delimiter="\t") if r.get("task")]
            for s in SLICES for a in ARMS}
    results = []
    with tempfile.TemporaryDirectory(prefix="w5b1-") as tmp:
        tr = os.path.join(tmp, "tr")
        os.makedirs(tr)
        for (s, a), rs in rows.items():
            for r in rs:
                open(os.path.join(tr, f"{a}_{r['task']}_r{r['run']}.jsonl"), "w").write("{}\n")

        def build(name, mutate=None, drop_transcript=None):
            d, t = os.path.join(tmp, name), os.path.join(tmp, name + "-tr")
            shutil.copytree(tr, t)
            os.makedirs(d)
            for (s, a), rs in rows.items():
                out = mutate(s, a, [dict(r) for r in rs]) if mutate else [dict(r) for r in rs]
                with open(os.path.join(d, fname(s, a)), "w", newline="") as f:
                    w = csv.DictWriter(f, fieldnames=list(rows[(s, a)][0].keys()), delimiter="\t")
                    w.writeheader(); w.writerows(out)
            if drop_transcript:
                os.remove(os.path.join(t, drop_transcript))
            errs, inc = [], []
            check(d, t, pin, errs, inc)
            results.append((name, bool(errs or inc)))

        build("clean")
        build("drop-row", mutate=lambda s, a, rs: rs[1:] if (s, a) == ("real", "manuk") else rs)
        build("wrong-chrome", mutate=lambda s, a, rs: [dict(r, chrome_ver="0.0.0") for r in rs] if (s, a) == ("full", "cdp-layer") else rs)
        first = rows[("real", "playwright")][0]
        build("no-transcript", drop_transcript=f"playwright_{first['task']}_r{first['run']}.jsonl")
    ok = results[0][1] is False and all(red for _, red in results[1:])
    for name, red in results:
        print(f"self-test {name}: {'RED' if red else 'green'}")
    print("self-test:", "PASS" if ok else "FAIL")
    return 0 if ok else 1


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", default=os.path.join(ROOT, "docs/sprint/w5"))
    ap.add_argument("--transcripts", default=os.path.expanduser("~/.cache/manuk-b1/transcripts"))
    ap.add_argument("--summary", default=None)
    ap.add_argument("--self-test", action="store_true")
    a = ap.parse_args()
    pin = open(os.path.join(ROOT, "docs/sprint/CHROME_PIN")).read().strip()
    if a.self_test:
        sys.exit(self_test(pin))
    sys.exit(run(a.root, a.transcripts, a.summary, pin))


if __name__ == "__main__":
    main()
