#!/usr/bin/env python3
"""W4.B4.a.a.a.a report: cache-inclusive total tokens and USD per task, manuk vs playwright+preload, own suite x3, haiku.

  MANUK_CHROME_PIN=145.0.7632.116 python3 -I b4aaaa_report.py DIR [--mutate swap]

DIR holds manuk.tsv and playwright+preload.tsv from run_arm.py (the W4.B4.a.a.a rows, docs/sprint/w4/B4aaa).
total = input + output + cache_read + cache_create tokens per task; usd = cost_usd per task. Each arm gets a per-run
mean over its 40 tasks for run 1, 2, 3; the band is min..max across those three runs.
Exit 1 on any failure: a row whose chrome_ver != $MANUK_CHROME_PIN; a file whose arm column is not the arm its name
says (so a swapped file is refused); not 40 tasks x 3 runs per arm. Exit 0 when the table is complete and valid.
The verdict ("below" = manuk band entirely under playwright+preload band) is reported, not gated: SUCCESS.md owns
the thresholds. --mutate swap feeds the files under each other's arm name: must exit 1.
"""
import csv, os, sys, collections
PIN = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
if len(sys.argv) < 2: sys.exit("usage: b4aaaa_report.py DIR [--mutate swap]")
d = sys.argv[1]; swap = "--mutate" in sys.argv
files = {"manuk": "manuk.tsv", "playwright+preload": "playwright+preload.tsv"}
if swap: files = {"manuk": files["playwright+preload"], "playwright+preload": files["manuk"]}
fail = []

def num(r, k):
    return float(r[k] or 0)

def load(arm):
    rows = list(csv.DictReader(open(os.path.join(d, files[arm])), delimiter="\t"))
    rows = list({(r["task"], r["run"]): r for r in rows}.values())
    if any(r["chrome_ver"] != PIN for r in rows): fail.append(f"{arm}: chrome_ver != {PIN}")
    if any(r["arm"] != arm for r in rows): fail.append(f"{arm}: file holds rows of another arm")
    if len(rows) != 120 or len({r["task"] for r in rows}) != 40 or {r["run"] for r in rows} != {"1", "2", "3"}:
        fail.append(f"{arm}: {len(rows)} rows, need 40 tasks x 3 runs")
    return rows

def per_run(rows, f):
    """mean per task for each run, keyed by run id."""
    by = collections.defaultdict(list)
    for r in rows: by[r["run"]].append(r)
    return {k: sum(f(r) for r in v) / len(v) for k, v in sorted(by.items())}

total = lambda r: num(r, "input_tokens") + num(r, "output_tokens") + num(r, "cache_read_tokens") + num(r, "cache_create_tokens")
usd = lambda r: num(r, "cost_usd")
cache = lambda r: num(r, "cache_read_tokens") + num(r, "cache_create_tokens")

res = {}
print(f"chrome_ver={PIN} own suite x3 haiku: cache-inclusive total tokens and USD per task")
print("| arm | pass per run (of 40) | total tokens/task per run | mean total | cache share | USD/task per run | mean USD |")
print("|---|---|---|---|---|---|---|")
for arm in files:
    rows = load(arm)
    if fail: continue
    ps = [sum(int(r["pass"] or 0) for r in rows if r["run"] == k) for k in sorted({r["run"] for r in rows})]
    t = per_run(rows, total); u = per_run(rows, usd); c = per_run(rows, cache)
    mt = sum(t.values()) / 3; mu = sum(u.values()) / 3; share = sum(c.values()) / sum(t.values())
    res[arm] = dict(t=t, u=u, mt=mt, mu=mu, tmin=min(t.values()), tmax=max(t.values()),
                    umin=min(u.values()), umax=max(u.values()))
    print(f"| {arm} | {ps} | {[round(v) for v in t.values()]} | {mt:.0f} | {share:.1%} | "
          f"{['%.5f' % v for v in u.values()]} | {mu:.5f} |")
for e in fail: print("FAIL", e)
if not fail:
    m, p = res["manuk"], res["playwright+preload"]
    def verdict(lo_a, hi_a, lo_b, hi_b):
        if hi_a < lo_b: return "BELOW (bands disjoint, manuk under playwright+preload)"
        if lo_a > hi_b: return "ABOVE (bands disjoint, manuk over playwright+preload)"
        return "NOT BELOW (bands overlap)"
    print(f"total gap manuk/playwright+preload = {m['mt']/p['mt']-1:+.1%} (mean); "
          f"band manuk {m['tmin']:.0f}-{m['tmax']:.0f} vs playwright+preload {p['tmin']:.0f}-{p['tmax']:.0f}")
    print(f"USD gap manuk/playwright+preload = {m['mu']/p['mu']-1:+.1%} (mean); "
          f"band manuk {m['umin']:.5f}-{m['umax']:.5f} vs playwright+preload {p['umin']:.5f}-{p['umax']:.5f}")
    print("manuk total below playwright+preload:", verdict(m['tmin'], m['tmax'], p['tmin'], p['tmax']))
    print("manuk USD below playwright+preload:", verdict(m['umin'], m['umax'], p['umin'], p['umax']))
print("VERDICT", "FAIL" if fail else "VALID"); sys.exit(1 if fail else 0)
