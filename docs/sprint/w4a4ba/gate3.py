#!/usr/bin/env python3
"""W4.A4.b.a.a gate: own-suite x3, interleaved per run, manuk (text ON) vs manuk-textoff (text OFF).
Same thresholds as gate.py (W4.A4.b.a), applied to the 3-run pooled means:
  exit 0 iff mean tokens/task (ON) <= 1.10 x mean (OFF) AND total pass(ON) >= total pass(OFF),
  over the same 40 tasks x 3 runs per arm, every row on one chrome_ver.
  python3 -I gate3.py [TSV] [--off-scale F]   (--off-scale is the red-check: scales the OFF arm's tokens)
"""
import csv, statistics as st, sys
from collections import defaultdict
path = next((a for a in sys.argv[1:] if not a.startswith("--") and not a.replace('.', '').isdigit()), "tokens3.tsv")
scale = float(sys.argv[sys.argv.index("--off-scale") + 1]) if "--off-scale" in sys.argv else 1.0
PIN = "145.0.7632.116"
rows = list(csv.DictReader(open(path), delimiter="\t"))
rows = [r for r in rows if r["arm"] in ("manuk", "manuk-textoff")]
if len({r["chrome_ver"] for r in rows}) != 1 or any(r["chrome_ver"] != PIN for r in rows):
    print("FAIL: chrome_ver not the single pin"); sys.exit(1)
tok = lambda r: int(r["input_tokens"]) + int(r["output_tokens"]) + int(r["cache_read_tokens"]) + int(r["cache_create_tokens"])
cell = defaultdict(dict)  # (arm, run) -> task -> row
for r in rows:
    if r["task"] in cell[(r["arm"], r["run"])]:
        print("FAIL: duplicate row", r["arm"], r["run"], r["task"]); sys.exit(1)
    cell[(r["arm"], r["run"])][r["task"]] = r
runs = sorted({k[1] for k in cell})
tasks = None
for k, v in cell.items():
    if len(v) != 40 or (tasks is not None and set(v) != tasks):
        print("FAIL: arm/run", k, "is not the same 40 tasks", len(v)); sys.exit(1)
    tasks = set(v)
if len(runs) != 3 or {k[0] for k in cell} != {"manuk", "manuk-textoff"}:
    print("FAIL: need 3 runs of both arms, got", sorted(cell)); sys.exit(1)
on = [r for k, v in cell.items() if k[0] == "manuk" for r in v.values()]
off = [r for k, v in cell.items() if k[0] == "manuk-textoff" for r in v.values()]
m_on = st.mean(tok(r) for r in on)
m_off = st.mean(tok(r) for r in off) * scale
p_on = sum(int(r["pass"]) for r in on); p_off = sum(int(r["pass"]) for r in off)
for run in runs:
    a = st.mean(tok(r) for r in cell[("manuk", run)].values())
    b = st.mean(tok(r) for r in cell[("manuk-textoff", run)].values())
    pa = sum(int(r["pass"]) for r in cell[("manuk", run)].values())
    pb = sum(int(r["pass"]) for r in cell[("manuk-textoff", run)].values())
    print(f"  run{run}: tokens/task ON={a:.0f} OFF={b:.0f} ratio={a/b:.3f}  pass ON={pa} OFF={pb}")
print(f"3-run pooled: tokens/task ON={m_on:.0f} OFF={m_off:.0f} ratio={m_on/m_off:.3f} (limit 1.10)  pass ON={p_on} OFF={p_off} (of 120 each)")
ok = True
if m_on > 1.10 * m_off: print("FAIL: tokens/task more than +10%"); ok = False
if p_on < p_off: print("FAIL: pass count lower with text on"); ok = False
print("PASS" if ok else "RED")
sys.exit(0 if ok else 1)
