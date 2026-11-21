#!/usr/bin/env python3
"""RG.3.0.b.a.a.a.a.a.a.a.b.a.a.a: for each manuk-unresolved or mismatched ordinal step (opendining-4 #13 #14, gocalendar-2 #6 #7),
print the wanted (role, ordinal), how many nodes of that role the live observation had (manuk and chrome), and the
role counts at the step. Writes docs/sprint/rg/RG3baaaaaaaaa-trace.md. Run with `python3 -I`; needs MANUK_CHROME_PIN."""
import os, sys, re
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import rg3b_replay as r, rg3baaaaaaa_match as m, rg3baaaaaa_ordinal as o
pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
WANT = {"opendining-4": [13, 14], "gocalendar-2": [6, 7]}
out = [f"## RG.3.0.b.a.a.a.a.a.a.a.b.a.a.a", "", f"chrome_ver={pin}. Per ordinal step: wanted (role, n) vs live count of that role, as the replay saw it before the step.", ""]
for t, steps in WANT.items():
    st = r.lg.steps(os.path.join(r.TR, f"manuk_{t}_r1.jsonl"))[0]
    seq = m.flatten_ref(st, 0)
    for side in ("manuk", "chrome"):
        raw = []
        r.resolve = o.resolve
        r.replay(side, [x[:3] for x in seq], 2, settle=2.0, raw=raw)
        for i in steps:
            rn = seq[i][2]; ref = seq[i][3]
            prev = next((x for x in reversed(raw) if x[0] < i), None)  # observation after the previous step
            cur = next((x for x in raw if x[0] == i), None)
            def cnt(obs, role): return len(re.findall(r'^\s*\[h\d+\] ' + role + r'\b', obs or "", re.M))
            role = rn[1]
            out.append(f"- {t} {side} #{i}: tool={seq[i][0]} wanted {role} n={rn[2]} (transcript `{ref['line']}`); "
                       f"live {role} count after step #{prev[0] if prev else '-'}: {cnt(prev[2], role) if prev else '-'}; "
                       f"at step: {cnt(cur[2], role) if cur else '-'} gap={cur[3] if cur else '-'}; all roles at step: "
                       + ", ".join(f"{k}={v}" for k, v in sorted(__import__('collections').Counter(re.findall(r'^\s*\[h\d+\] (\w+)', (cur[2] if cur else '') or '', re.M)).items())))
        print(out[-1], flush=True)
open(os.path.join(r.ROOT, "docs/sprint/rg/RG3baaaaaaaaa-trace.md"), "w").write("\n".join(out) + "\n")
