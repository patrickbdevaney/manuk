#!/usr/bin/env python3
"""W4.B4.a.a.a gate: in+out tokens/task, manuk (lean observe) vs playwright+preload, own suite x3, haiku.

  python3 -I b4aaa_gate.py DIR [--mutate swap]   (DIR holds manuk.tsv and playwright+preload.tsv from run_arm.py)
Checks, exit 1 on any failure: every row is at chrome_ver == $MANUK_CHROME_PIN; each file's arm column is the arm its
name says (so a swapped file is refused); 40 tasks x 3 runs each. Prints the per-run table (pass, in+out, out) and the
gap manuk/playwright+preload - 1 against the B4 reference +40%. Exit 0 when the table is complete and valid; the
gap itself is reported, not gated (SUCCESS.md owns the Bar 2.2 thresholds).
--mutate swap feeds the files under each other's arm name: must exit 1.
"""
import csv, os, sys, collections
PIN = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
d = sys.argv[1]; swap = "--mutate" in sys.argv
files = {"manuk": "manuk.tsv", "playwright+preload": "playwright+preload.tsv"}
if swap: files = {"manuk": files["playwright+preload"], "playwright+preload": files["manuk"]}
fail = []
def load(arm):
    rows = list(csv.DictReader(open(os.path.join(d, files[arm])), delimiter="\t"))
    rows = list({(r["task"], r["run"]): r for r in rows}.values())
    if any(r["chrome_ver"] != PIN for r in rows): fail.append(f"{arm}: chrome_ver != {PIN}")
    if any(r["arm"] != arm for r in rows): fail.append(f"{arm}: file holds rows of another arm")
    if len(rows) != 120 or len({r["task"] for r in rows}) != 40: fail.append(f"{arm}: {len(rows)} rows, need 40 tasks x 3 runs")
    return rows
def per_run(rows, f):
    by = collections.defaultdict(list)
    for r in rows: by[r["run"]].append(r)
    return {k: sum(f(r) for r in v) / len(v) for k, v in sorted(by.items())}
io = lambda r: int(r["input_tokens"] or 0) + int(r["output_tokens"] or 0)
out = lambda r: int(r["output_tokens"] or 0)
res = {}
print(f"chrome_ver={PIN} own suite x3 haiku")
print("| arm | pass per run (of 40) | in+out/task per run | mean in+out | out/task mean |\n|---|---|---|---|---|")
for arm in files:
    rows = load(arm)
    ps = [sum(int(r["pass"] or 0) for r in rows if r["run"] == k) for k in sorted({r["run"] for r in rows})]
    t = per_run(rows, io); m = sum(t.values()) / max(len(t), 1)
    o = per_run(rows, out); res[arm] = (m, sum(o.values()) / max(len(o), 1), min(t.values(), default=0), max(t.values(), default=0))
    print(f"| {arm} | {ps} | {[round(v) for v in t.values()]} | {m:.0f} | {res[arm][1]:.0f} |")
for e in fail: print("FAIL", e)
if not fail:
    m, p = res["manuk"], res["playwright+preload"]
    print(f"gap in+out manuk/playwright+preload = {m[0]/p[0]-1:+.1%} (B4 reference +40%); out gap {m[1]/p[1]-1:+.1%}; "
          f"manuk band {m[2]:.0f}-{m[3]:.0f} vs playwright+preload band {p[2]:.0f}-{p[3]:.0f}")
print("VERDICT", "FAIL" if fail else "VALID"); sys.exit(1 if fail else 0)
