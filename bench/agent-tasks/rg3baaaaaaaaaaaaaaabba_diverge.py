#!/usr/bin/env python3
"""RG.3.0.b.a.a.a.a.a.a.a.b.a.a.a.a.a.a.a.b.b.a.a: why does manuk leave opendining-4 ordinal steps #13/#14 unresolved
(live '-') while chrome resolves them? One replay per backend of the first 15 steps (settle 2 s, wait 2 s). For each
ordinal step: the transcript node (role, ordinal n) it was recorded against, how many same-role nodes each backend's
fresh observation had, and the nodes around that ordinal. The first diverging step = first ordinal step resolved on
chrome but not on manuk.
  rg3baaaaaaaaaaaaaaabba_diverge.py [--mutate role-button]   exit 0 = a divergence was found and printed, 1 = none
--mutate role-button: every ordinal step in the transcript is re-pointed at button ordinal 0 (a mutated transcript); both backends resolve it, exit 1.
Writes docs/sprint/rg/RG3baaaaaaaaaaaaaaabba_diverge.md on an unmutated run. Run with `python3 -I`; needs MANUK_CHROME_PIN."""
import argparse, os, re, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import rg3b_replay as r
import rg3baaaaaa_ordinal as o
import rg3baaaaaaa_match as mm

TASK, LAST = "opendining-4", 15
DOC = os.path.join(r.ROOT, "docs/sprint/rg/RG3baaaaaaaaaaaaaaabba_diverge.md")


def lines(obs):
    return [l.strip() for l in (obs or "").splitlines() if re.match(r'^\s*\[h\d+\] ', l)]


def around(obs, role, n, k=2):
    """Same-role lines of obs centred on ordinal n, plus the count of same-role nodes."""
    ls = [l for l in lines(obs) if re.match(r'\[h\d+\] ' + role + r'\b', l)]
    return len(ls), ls[max(0, n - k): n + k + 1]


def main():
    ap = argparse.ArgumentParser(); ap.add_argument("--mutate", choices=["role-button"]); a = ap.parse_args()
    pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
    r.resolve = o.resolve
    st = r.lg.steps(os.path.join(r.TR, f"manuk_{TASK}_r1.jsonl"))[0]
    seq = mm.flatten_ref(st, 0)[:LAST]
    if a.mutate:
        seq = [(t, x, (rn[0], "button", 0) if rn and rn[0] == o.ORD else rn, ref) for t, x, rn, ref in seq]
    ords = [i for i, s in enumerate(seq) if s[2] and s[2][0] == o.ORD]
    res = {}
    for side in ("manuk", "chrome"):
        raw = []; r.replay(side, [s[:3] for s in seq], 2, settle=2.0, raw=raw)
        res[side] = {x[0]: x for x in raw}
    out = [f"## RG.3.0.b.a.a.a.a.a.a.a.b.a.a.a.a.a.a.a.b.b.a.a", "",
           f"chrome_ver={pin}. 1 replay per backend of the first {LAST} {TASK} steps; ordinal steps: {ords}.", ""]
    first = None
    for i in ords:
        _, role, n = seq[i][2]; ref = seq[i][3]
        out += [f"### step #{i}: transcript `{ref['line']}` = {role} ordinal {n}", ""]
        resolved = {}
        for side in ("manuk", "chrome"):
            _, _, obs, gap = res[side][i]  # gap: obs is the fresh observation the step failed against; else the one after it
            cnt, near = around(obs, role, n)
            resolved[side] = not gap
            out += [f"- {side}: {'UNRESOLVED' if gap else 'resolved'}; {cnt} {role} nodes in the observation "
                    f"{'it failed against' if gap else 'after the step'}; around ordinal {n}:"] + [f"    {l}" for l in near]
        out.append("")
        if first is None and resolved["chrome"] and not resolved["manuk"]:
            first = i
    prev = [i for i in range(LAST) if i in res["manuk"] and res["manuk"][i][3]]
    if first is None:
        out.append("NO DIVERGENCE: no ordinal step is resolved on chrome and unresolved on manuk.")
    else:
        i = first; mo = res["manuk"][i][2]; co = res["chrome"][i][2]
        out += [f"FIRST DIVERGING STEP: #{i} (`{seq[i][0]} {seq[i][2]}`); transcript ordinal {seq[i][2][2]} of {seq[i][2][1]}; "
                f"manuk has {around(mo, seq[i][2][1], 0)[0]} nodes of that role, chrome {around(co, seq[i][2][1], 0)[0]}.", "",
                "manuk interactive observation at that step:", "```"] + lines(mo) + ["```", "", "chrome interactive observation at that step:", "```"] + lines(co) + ["```"]
    txt = "\n".join(out) + "\n"
    print(txt)
    if not a.mutate:
        open(DOC, "w").write(txt)
    sys.exit(0 if first is not None else 1)


if __name__ == "__main__":
    main()
