#!/usr/bin/env python3 -I
"""W4.C4.a: REAL-47 action receipts ON vs OFF, per layer (manuk engine, cdp-layer), 3 runs per task.
  c4a_report.py --on ON.tsv --off OFF.tsv [--off ...]     (ON = real_run rows for manuk/cdp-layer; OFF = the *-no-receipts arms)
Per layer: per-run pass counts (the 3-run band = min..max) on / off, tasks passing >=2/3, and 'false-done' runs
(the run ended without error but failed the grader = the agent believed it finished and the end state said otherwise).
GREEN for a layer iff the ON band lies wholly above the OFF band (outside it). Exit 0 if any layer green, 1 otherwise, 2 incomplete.
MANUK_C4A_MUTATE=1 swaps on and off (the gate must then go red). Rows must share one chrome_ver.
"""
import argparse, csv, collections, os, sys

LAYERS = {"manuk": "manuk-no-receipts", "cdp-layer": "cdp-layer-no-receipts"}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--on", required=True); ap.add_argument("--off", action="append", required=True)
    ap.add_argument("--runs", type=int, default=3)
    a = ap.parse_args()
    cols = open(a.off[0]).readline().rstrip("\n").split("\t")  # real_run's file may have its header mid-file or missing
    rows = []
    for f in [a.on] + a.off:
        rows += [r for r in csv.DictReader(open(f), fieldnames=cols, delimiter="\t") if r["arm"] != "arm"]
    vers = {r["chrome_ver"] for r in rows}
    if len(vers) != 1:
        sys.exit(f"refusing: rows span chrome_ver {sorted(vers)}")
    g = collections.defaultdict(lambda: collections.defaultdict(dict))
    for r in rows:
        g[r["arm"]][r["task"]][int(r["run"])] = r
    mut = bool(os.environ.get("MANUK_C4A_MUTATE"))
    print(f"REAL-47 receipts on vs off  chrome_ver={vers.pop()}" + ("  [MUTATED: on/off swapped]" if mut else ""))
    print("layer\tarm\tper-run pass\tband\tpass>=2/3\tfalse-done\tmean tok")
    runs = range(1, a.runs + 1)
    green, incomplete = False, False
    for layer, off in LAYERS.items():
        on_arm, off_arm = (off, layer) if mut else (layer, off)
        tasks = sorted(set(g[on_arm]) & set(g[off_arm]))
        if any(len(g[x][t]) < a.runs for x in (on_arm, off_arm) for t in tasks) or not tasks:
            print(f"{layer}: INCOMPLETE"); incomplete = True; continue
        band = {}
        for x in (on_arm, off_arm):
            pr = [sum(int(g[x][t][k]["pass"]) for t in tasks) for k in runs]
            rs = [g[x][t][k] for t in tasks for k in runs]
            fd = sum(1 for r in rs if not int(r["pass"]) and not int(r["is_error"]))
            tok = sum(int(r["input_tokens"] or 0) + int(r["output_tokens"] or 0) for r in rs) / len(rs)
            two = sum(sum(int(g[x][t][k]["pass"]) for k in runs) >= 2 for t in tasks)
            band[x] = (min(pr), max(pr))
            print(f"{layer}\t{x}\t{pr}\t{band[x][0]}-{band[x][1]}\t{two}/{len(tasks)}\t{fd}/{len(rs)}\t{tok:.0f}")
        outside = band[on_arm][0] > band[off_arm][1]
        print(f"{layer}: n={len(tasks)} tasks; ON band {band[on_arm]} vs OFF band {band[off_arm]}: {'OUTSIDE (on higher)' if outside else 'overlapping/not higher'}")
        green |= outside
    if incomplete:
        sys.exit(2)
    print("GATE GREEN" if green else "GATE RED")
    sys.exit(0 if green else 1)


if __name__ == "__main__":
    main()
