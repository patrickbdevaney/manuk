#!/usr/bin/env python3
"""RG.3.0.b.a.a.a.a.a.a.a.b.a.a.a.a: replay opendining-4 to step #7 (the click on 'Select time') on manuk and chrome and
print, per backend, combobox/option counts before and after the click and the act result text (listener count, events).
Exit 0 = divergence shown (before counts equal, after counts differ); 1 = identical after counts or not shown.
  --mutate same   the manuk side is replaced by the chrome side's numbers (deliberately-broken input; must exit 1)
Writes docs/sprint/rg/RG3baaaaaaaaaa-click.md. Run with `python3 -I`; needs MANUK_CHROME_PIN."""
import os, sys, re, argparse
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import rg3b_replay as r, rg3baaaaaaa_match as m, rg3baaaaaa_ordinal as o
ap = argparse.ArgumentParser(); ap.add_argument("--mutate"); ap.add_argument("--step", type=int, default=7); a = ap.parse_args()
pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
st = r.lg.steps(os.path.join(r.TR, "manuk_opendining-4_r1.jsonl"))[0]
seq = m.flatten_ref(st, 0)
r.resolve = o.resolve
def cnt(obs, role): return len(re.findall(r'^\s*\[h\d+\] ' + role + r'\b', obs or "", re.M))
res = {}
out = [f"## RG.3.0.b.a.a.a.a.a.a.a.b.a.a.a.a", "", f"chrome_ver={pin}. opendining-4 replay, step #{a.step} = `{seq[a.step][0]} {seq[a.step][2]}`.", ""]
for side in ("manuk", "chrome"):
    raw = []; log = r.replay(side, [x[:3] for x in seq[:a.step + 1]], 2, settle=2.0, raw=raw)
    b = next(x for x in reversed(raw) if x[0] < a.step); c = next((x for x in raw if x[0] == a.step), None)
    ct = log[-1][2] if log and log[-1][0] == a.step else "(step unresolved)"
    res[side] = dict(cb0=cnt(b[2], "combobox"), op0=cnt(b[2], "option"), cb1=cnt(c[2], "combobox") if c else -1, op1=cnt(c[2], "option") if c else -1, act=ct)
if a.mutate == "same": res["manuk"] = dict(res["chrome"])
for s, v in res.items():
    out.append(f"- {s}: comboboxes before/after {v['cb0']}/{v['cb1']}, options before/after {v['op0']}/{v['op1']}; act result: `{v['act']}`")
    print(out[-1], flush=True)
mk, ch = res["manuk"], res["chrome"]
shown = (mk["cb0"], mk["op0"]) == (ch["cb0"], ch["op0"]) and (mk["cb1"], mk["op1"]) != (ch["cb1"], ch["op1"])
out.append(f"- divergence shown: {shown}")
if not a.mutate: open(os.path.join(r.ROOT, "docs/sprint/rg/RG3baaaaaaaaaa-click.md"), "w").write("\n".join(out) + "\n")
print("GATE", "PASS" if shown else "FAIL"); sys.exit(0 if shown else 1)
