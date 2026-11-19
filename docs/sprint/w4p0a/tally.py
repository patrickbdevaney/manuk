#!/usr/bin/env python3
"""W4.P0.a.a.a: per-task REAL pass counts for the dashdish-2/7/10 manuk measurement.

  python3 -I docs/sprint/w4p0a/tally.py [FILE.tsv]     table: passes per run, SCOREBOARD 2-of-3 verdict, chrome_ver
  python3 -I docs/sprint/w4p0a/tally.py --selftest     mutants: a flipped pass and a task forced to 1/3 must go red

Exit 0 only when every task passes in >=2 of 3 runs on ONE chrome_ver (the SUCCESS.md rule). Rows are the latest per
(task, run). Numbers are read from the TSV; nothing is typed in.
"""
import csv, sys, os, io, collections

TASKS = ("dashdish-2", "dashdish-7", "dashdish-10")
HERE = os.path.dirname(os.path.abspath(__file__))
DEFAULT = os.path.join(HERE, "dashdish-activate-manuk.tsv")


def tally(text):
    rows = [r for r in csv.DictReader(io.StringIO(text), delimiter="\t") if r.get("arm") == "manuk" and r["task"] in TASKS]
    latest = {}
    for r in sorted(rows, key=lambda r: r["ts"]):
        latest[(r["task"], r["run"])] = r
    per = collections.defaultdict(dict)
    for (t, run), r in latest.items():
        per[t][run] = r["pass"] == "1"
    chrome = {r["chrome_ver"] for r in latest.values()}
    ok = len(chrome) == 1 and all(t in per and sum(per[t].values()) >= 2 for t in TASKS)
    return per, chrome, ok


def main():
    if "--selftest" in sys.argv:
        text = open(DEFAULT).read()
        per, chrome, ok = tally(text)
        assert ok, "real rows must pass the 2-of-3 rule"
        # mutant 1: flip one passing run of dashdish-2 to a fail -> still 2/3, must stay green
        m1 = text.replace("\t1\t1\t", "\t1\t0\t", 1)
        _, _, ok1 = tally(m1)
        # mutant 2: force dashdish-7 to 1/3 -> the verdict must go red
        lines = text.splitlines()
        out = []
        for ln in lines:
            f = ln.split("\t")
            if len(f) > 9 and f[4] == "dashdish-7" and f[6] in ("2", "3"):
                f[8] = "0"
                ln = "\t".join(f)
            out.append(ln)
        _, _, ok2 = tally("\n".join(out) + "\n")
        print(f"selftest: real={ok} mutant_one_flip={ok1} mutant_task_1of3={ok2}")
        sys.exit(0 if ok and ok1 and not ok2 else 1)
    path = next((a for a in sys.argv[1:] if not a.startswith("--")), DEFAULT)
    per, chrome, ok = tally(open(path).read())
    for t in TASKS:
        runs = per.get(t, {})
        print(f"{t}\t{sum(runs.values())}/{len(runs)}\t" + " ".join(f"r{k}={'PASS' if v else 'fail'}" for k, v in sorted(runs.items())))
    print(f"chrome_ver={','.join(sorted(chrome))}")
    print("VERDICT", "GREEN (each task >=2 of 3 on one chrome_ver)" if ok else "RED")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
