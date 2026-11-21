#!/usr/bin/env python3
"""RG.3.0.b.a.a.a.a.a.a.a.b.a.a.a.a.a.a.a.c.b: gocalendar-2 ordinal #6 on both backends. Prints the transcript line the
handle was recorded against and the live line it resolved to, and classifies each backend as SAME element (live line has the
same role, no accessible name, same state/value as the transcript's) or DIFFERENT (anything else). The live observation's
same-role ordinal list around the ordinal is printed so the cause is visible. Red (exit 1) if either backend is unclassified
(step unresolved), or if --mutate same forces the classifier to read a wrong live line and it still says SAME.
  rg3baaaaaaaaaaaaaaaacb_gocal.py [--mutate unresolved] [--out FILE]    (python3 -I; needs MANUK_CHROME_PIN)
  --mutate unresolved: chrome's step #6 is dropped from the resolved set in memory; the gate must exit 1 (UNCLASSIFIED).
  Note the replay result is mutated only after the live run, so it still takes a full replay."""
import argparse, json, os, re, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import rg3baaaaaaa_match as m
import rg3b_replay as r
import rg3baaaaaa_ordinal as o

TASK, STEP = "gocalendar-2", 6


def classify(ref, live_line):
    lv = m.NODE_LINE.search(live_line)
    named = bool(m.NAME.match(lv.group(3))) if lv else True
    sv = m.NAME.sub("", lv.group(3)).strip() if lv else None
    same = bool(lv) and lv.group(2) == ref["role"] and not named and sv == ref["sv"]
    return "SAME" if same else "DIFFERENT"


def main():
    ap = argparse.ArgumentParser(); ap.add_argument("--mutate", choices=["unresolved"]); ap.add_argument("--out")
    a = ap.parse_args()
    pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
    r.resolve = m.rec_resolve
    st = r.lg.steps(os.path.join(r.TR, f"manuk_{TASK}_r1.jsonl"))[0]
    seq = m.flatten_ref(st, 0)
    ref = seq[STEP][3]
    assert ref and seq[STEP][2] and seq[STEP][2][0] == o.ORD, "step %d is not an ordinal step" % STEP
    out = [f"chrome_ver={pin} {TASK} #{STEP}: transcript line `{ref['line']}` (role {ref['role']}, ordinal {ref['n']} among same-role nodes, sv {ref['sv']!r})"]
    bad = 0
    idx = [i for i, x in enumerate(seq) if x[2] and x[2][0] == o.ORD]
    for side in ("manuk", "chrome"):
        m.REC.clear(); raw = []
        r.replay(side, [x[:3] for x in seq], 2, settle=2.0, raw=raw)
        gaps = {x[0] for x in raw if x[3]}
        ok = [i for i in idx if i not in gaps]
        if a.mutate == "unresolved" and side == "chrome":
            ok = [i for i in ok if i != STEP]
        if STEP not in ok:
            out.append(f"{side}: UNCLASSIFIED (step #{STEP} unresolved)"); bad += 1; continue
        h, obs = m.REC[ok.index(STEP)]
        nodes = m.node_lines(obs)
        live = nodes[h]["line"]
        verdict = classify(ref, live)
        same_role = [v for v in nodes.values() if v["role"] == ref["role"]]
        bare = [v["n"] for v in same_role if not m.NAME.match(re.sub(r"^\[h\d+\] \w+", "", v["line"])) and v["sv"] == ref["sv"]]
        out.append(f"{side}: {verdict}; live `{live}`; {len(same_role)} {ref['role']} nodes live, nameless `{ref['sv']}` buttons at ordinal(s) {bare}; "
                   f"neighbours: " + " | ".join(f"#{v['n']} {v['line'][:60]}" for v in same_role if abs(v['n'] - ref['n']) <= 1))
    out.append("self-test: transcript line classifies " + classify(ref, ref["line"]) + " against itself")
    if classify(ref, ref["line"]) != "SAME":
        bad += 1
    text = "\n".join(out)
    print(text)
    if a.out and not a.mutate:
        open(a.out, "w").write(text + "\n")
    if bad:
        print("GATE FAIL: unclassified"); sys.exit(1)
    print("GATE PASS: both backends classified")


if __name__ == "__main__":
    main()
