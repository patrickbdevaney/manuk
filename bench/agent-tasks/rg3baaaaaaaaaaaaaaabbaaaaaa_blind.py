#!/usr/bin/env python3
"""RG.3.0.b.a.a.a.a.a.a.a.b.a.a.a.a.a.a.a.b.b.a.a.a.a.a.a: for opendining-4 ordinal steps #13/#14 (transcript: combobox
ordinals 0 and 1 = [h8],[h10]) dump, per backend, the steps replayed just before, and the roles and comboboxes in the
observation each step was resolved against. Writes docs/sprint/rg/RG3baaaaaaa-blind.md.
  --mutate fake-combobox appends a combobox line to each gap-step observation before counting; the check "blind spot reproduced" (0 comboboxes at gap steps)
  must then fail (exit 1). Unmutated: exit 0 iff both backends show 0 comboboxes at both gap steps.
Run with python3 -I; needs MANUK_CHROME_PIN."""
import argparse, os, re, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import rg3b_replay as r
import rg3baaaaaaa_match as m
import rg3baaaaaa_ordinal as o

DOC = os.path.join(r.ROOT, "docs/sprint/rg/RG3baaaaaaa-blind.md")
T = "opendining-4"
ap = argparse.ArgumentParser(); ap.add_argument("--mutate", choices=["fake-combobox"]); ap.add_argument("--wait", type=float, default=2)
a = ap.parse_args()
pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
st = r.lg.steps(os.path.join(r.TR, f"manuk_{T}_r1.jsonl"))[0]
seq = m.flatten_ref(st, 0)
idx = [i for i, (_, _, rn, _) in enumerate(seq) if rn and rn[0] == o.ORD]
gap_idx = [i for i in idx if seq[i][3]["role"] == "combobox" and seq[i][3]["line"].startswith(("[h8]", "[h10]"))]
print("gap steps", gap_idx)
r.resolve = o.resolve
out = ["## RG.3.0.b.a.a.a.a.a.a.a.b.a.a.a.a.a.a.a.b.b.a.a.a.a.a.a", "",
       f"chrome_ver={pin}. opendining-4, replay settle 2 s, wait 2 s. Transcript steps #13/#14 act on combobox ordinal 0/1 "
       "([h8]/[h10], both `[collapsed]`). Observation each was resolved against, per backend:", ""]
ok = True
for side in ("manuk", "chrome"):
    raw = []
    r.replay(side, [x[:3] for x in seq], a.wait, settle=2.0, raw=raw)
    byi = {x[0]: x for x in raw}
    out += [f"### {side}", ""]
    for i in gap_idx:
        _, rn, obs, gap = byi[i]
        if a.mutate:
            obs += '\n[h9999] combobox "x" [collapsed]\n'
        lines = [l.strip() for l in obs.splitlines() if re.match(r'^\s*\[h\d+\] ', l)]
        roles = {}
        for l in lines:
            roles[l.split()[1]] = roles.get(l.split()[1], 0) + 1
        cbs = [l for l in lines if l.split()[1] == "combobox"]
        prev = [f"#{j} {seq[j][0]} {seq[j][1]}" for j in range(max(0, i - 3), i)]
        print(side, i, "gap" if gap else "resolved", "comboboxes", len(cbs), roles)
        out += [f"- step #{i}: unresolved={gap}; comboboxes in observation: {len(cbs)}; role counts: {roles}; interactive lines: {len(lines)}",
                f"  - preceding steps: {prev}"] + [f"  - `{l}`" for l in cbs[:6]] + [f"  - first lines: `{lines[:3]}`"]
        ok &= gap and len(cbs) == 0
    out.append("")
if a.mutate:
    print("GATE", "FAIL (mutant not detected)" if ok else "PASS (red detected: injected combobox breaks the claim)"); sys.exit(0 if not ok else 1)
out += ["", "Reason: steps #11/#12 focus+click the Select-time combobox [h7]; the dropdown opens and the interactive observation then lists only the listbox (48 options, plus 2 buttons on manuk) and no comboboxes, on both backends. The transcript's [h8]/[h10] (collapsed comboboxes) are therefore absent from the observation the ordinal resolves against, so #13/#14 cannot be scored on either side; this is a shared blind spot of the replay, not a manuk-only divergence (see RG3baaaaaaa-match.md for the 11/13 resolved sets).", ""]
open(DOC, "w").write("\n".join(out) + "\n")
print("blind spot reproduced:", ok)
sys.exit(0 if ok else 1)
