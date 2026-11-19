#!/usr/bin/env python3
"""W4.P0.a.b.c: real pass counts per task (manuk arm) for gocalendar-1/3/4 and gomail-5, 3 runs each.

  python3 -I docs/sprint/w4p0c/tally.py [FILE.tsv]    per-task pass counts, chrome_ver, verdict
  python3 -I docs/sprint/w4p0c/tally.py --selftest    mutants must turn the verdict red
  python3 -I docs/sprint/w4p0c/tally.py --scoreboard  append per-run rows to docs/sprint/SCOREBOARD.tsv (once)

Pass = the REAL jmespath verdict column `pass` (state_ok and every eval). A task counts as passing when it passes in
>=2 of 3 runs (SUCCESS.md). Exit 0 only when all four tasks have 3 runs each on ONE chrome_ver and each task >=2/3.
"""
import csv, sys, os, io, collections

TASKS = ("gocalendar-1", "gocalendar-3", "gocalendar-4", "gomail-5")
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(HERE)))
DEFAULT = os.path.join(HERE, "real-runs-manuk.tsv")
SCOREBOARD = os.path.join(ROOT, "docs/sprint/SCOREBOARD.tsv")
SB_COLS = ["ts", "arm", "suite_hash", "chrome_ver", "task", "slice", "kind", "run", "model", "pass", "injection_followed",
           "input_tokens", "output_tokens", "cache_read_tokens", "cache_create_tokens", "cost_usd", "seconds",
           "peak_rss_mb", "turns", "is_error", "reasons", "answer"]


def tally(text):
    rows = [r for r in csv.DictReader(io.StringIO(text), delimiter="\t") if r.get("arm") == "manuk" and r["task"] in TASKS]
    latest = {}
    for r in sorted(rows, key=lambda r: r["ts"] if "ts" in r else ""):
        latest[(r["task"], r["run"])] = r
    per = collections.defaultdict(dict)
    for (t, run), r in latest.items():
        per[t][run] = r["pass"] == "1"
    chrome = {r["chrome_ver"] for r in latest.values()}
    ok = (len(chrome) == 1 and "145.0.7632.116" in chrome
          and all(t in per and len(per[t]) == 3 and sum(per[t].values()) >= 2 for t in TASKS))
    return per, chrome, ok


def selftest(text):
    per, chrome, ok = tally(text)
    assert ok, "real rows must pass the 2-of-3 rule on one pin"
    # mutant 1: one passing run of gocalendar-1 flipped to a fail -> still 2/3, must stay green
    m1 = text.replace("gocalendar-1\t1\t1\t", "gocalendar-1\t1\t0\t", 1)
    _, _, ok1 = tally(m1)
    # mutant 2: gomail-5 forced to 1/3 -> red
    rows = list(csv.DictReader(io.StringIO(text), delimiter="\t"))
    cols = list(rows[0].keys()) if rows else []
    def forced(task_name, keep_run):
        rs = []
        for r in rows:
            if r["task"] == task_name and r["run"] != keep_run:
                r = dict(r); r["pass"] = "0"
            rs.append(r)
        buf = io.StringIO(); w = csv.DictWriter(buf, fieldnames=cols, delimiter="\t", lineterminator="\n")
        w.writeheader(); w.writerows(rs); return buf.getvalue()
    _, _, ok2 = tally(forced("gomail-5", "1"))  # only run 1 passes: 1/3 -> red
    # mutant 3: runs from two chrome pins -> red
    m3 = text.replace("145.0.7632.116", "145.0.7632.999", 1)
    _, _, ok3 = tally(m3)
    print(f"selftest: real={ok} mutant_flip_stays_green={ok1} mutant_task_1of3={ok2} mutant_mixed_pin={ok3}")
    return ok and ok1 and not ok2 and not ok3


def main():
    if "--selftest" in sys.argv:
        sys.exit(0 if selftest(open(DEFAULT).read()) else 1)
    if "--scoreboard" in sys.argv:
        text = open(DEFAULT).read()
        rows = [r for r in csv.DictReader(io.StringIO(text), delimiter="\t") if r.get("arm") == "manuk" and r["task"] in TASKS]
        import hashlib
        h = hashlib.sha256()
        for t in TASKS:
            h.update(open(os.path.join(ROOT, "bench/agent-tasks/real/tasks", t + ".json"), "rb").read())
        suite = h.hexdigest()
        out = []
        for r in rows:
            out.append([r["ts"], "manuk", suite, r["chrome_ver"], r["task"], "real", "live", r["run"], "haiku",
                        r["pass"], "0", r["input_tokens"], r["output_tokens"], r["cache_read_tokens"],
                        r["cache_create_tokens"], r["cost_usd"], r["seconds"], "", r["turns"], r["is_error"],
                        "W4.P0.a.b.c real_run.py jmespath (gocalendar/gomail task file sha256 in suite_hash)", ""])
        with open(SCOREBOARD, "a") as f:
            for o in out:
                f.write("\t".join(o) + "\n")
        print(f"appended {len(out)} rows to SCOREBOARD")
        return
    path = next((a for a in sys.argv[1:] if not a.startswith("--")), DEFAULT)
    per, chrome, ok = tally(open(path).read())
    for t in TASKS:
        runs = per.get(t, {})
        print(f"{t}\t{sum(runs.values())}/{len(runs)}\t" + " ".join(f"r{k}={'PASS' if v else 'fail'}" for k, v in sorted(runs.items())))
    print(f"chrome_ver={','.join(sorted(chrome))}")
    print("VERDICT", "GREEN" if ok else "RED")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
