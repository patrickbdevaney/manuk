#!/usr/bin/env python3
"""W4.A4.b.a gate: own-suite x1, manuk backend, text ON (arm manuk) vs text OFF (arm manuk-textoff).
Exit 0 iff mean tokens/task (ON) <= 1.10 x mean (OFF) AND pass(ON) >= pass(OFF), over the same 40 tasks, one chrome_ver.
  python3 -I gate.py [TSV] [--off-scale F]   (--off-scale is the red-check: scales the OFF arm's tokens)
"""
import csv, statistics as st, sys
path = next((a for a in sys.argv[1:] if not a.startswith("--")), "tokens.tsv")
scale = float(sys.argv[sys.argv.index("--off-scale") + 1]) if "--off-scale" in sys.argv else 1.0
rows = list(csv.DictReader(open(path), delimiter="\t"))
arms = {}
for r in rows:
    arms.setdefault(r["arm"], {})[r["task"]] = r
on, off = arms.get("manuk", {}), arms.get("manuk-textoff", {})
ok = True
if set(on) != set(off) or len(on) != 40:
    print("FAIL: task sets differ or not 40", len(on), len(off)); sys.exit(1)
if len({r["chrome_ver"] for r in rows}) != 1 or not all(r["chrome_ver"] == "145.0.7632.116" for r in rows):
    print("FAIL: chrome_ver not the single pin"); sys.exit(1)
tok = lambda r: int(r["input_tokens"]) + int(r["output_tokens"]) + int(r["cache_read_tokens"]) + int(r["cache_create_tokens"])
m_on = st.mean(tok(r) for r in on.values())
m_off = st.mean(tok(r) for r in off.values()) * scale
p_on = sum(int(r["pass"]) for r in on.values()); p_off = sum(int(r["pass"]) for r in off.values())
print(f"tokens/task ON={m_on:.0f} OFF={m_off:.0f} ratio={m_on/m_off:.3f} (limit 1.10)  pass ON={p_on} OFF={p_off}")
if m_on > 1.10 * m_off: print("FAIL: tokens/task more than +10%"); ok = False
if p_on < p_off: print("FAIL: pass count lower with text on"); ok = False
print("PASS" if ok else "RED")
sys.exit(0 if ok else 1)
