#!/usr/bin/env python3
"""RG.3.0.b.a.a.a.a.a.a.a.b.a.a.a.a.a.a.a.c.b.a: why does Chrome expose 123 buttons where manuk exposes 51 on gocalendar-2?

Replays the gocalendar-2 ordinal steps on both backends (same machinery as the .c.b gate), takes the interactive
observation that step #6 resolved against, and counts its button nodes. It prints, for each backend, the button count,
the nameless button count and the accessible names, then the names present on one backend and not the other (set
difference) and the multiset difference (count per name). Measure only: nothing in the engine or the match rule changes.

Gate: GREEN (exit 0) when the button counts are equal, or when the name sets differ (the count gap is explained by
names). RED (exit 1) when the counts differ and the name sets are identical (a gap no name explains), or when either
backend's step #6 is unresolved, or when the chrome_ver pin is unset.

  rg3baaaaaaaaaaaaaaaacba_buttons.py [--mutate samenames] [--out FILE]   (python3 -I; needs MANUK_CHROME_PIN)
  --mutate samenames: chrome's name set is replaced by manuk's before the check (a gap with no name difference); must exit 1.
"""
import argparse, collections, os, re, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import rg3baaaaaaa_match as m
import rg3b_replay as r
import rg3baaaaaa_ordinal as o

TASK, STEP = "gocalendar-2", 6
BUTTON = re.compile(r'^\s*\[(h\d+)\] button(.*)$', re.M)
NAME = re.compile(r'^ "([^"]*)"')


def buttons(obs):
    """-> (count, nameless_count, Counter of accessible names) for every button line in one observation."""
    names, nameless = collections.Counter(), 0
    for mo in BUTTON.finditer(obs):
        nm = NAME.match(mo.group(2))
        if nm:
            names[nm.group(1)] += 1
        else:
            nameless += 1
    return sum(names.values()) + nameless, nameless, names


def main():
    ap = argparse.ArgumentParser(); ap.add_argument("--mutate", choices=["samenames"]); ap.add_argument("--out")
    a = ap.parse_args()
    pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
    r.resolve = m.rec_resolve
    st = r.lg.steps(os.path.join(r.TR, f"manuk_{TASK}_r1.jsonl"))[0]
    seq = m.flatten_ref(st, 0)
    assert seq[STEP][2] and seq[STEP][2][0] == o.ORD, "step %d is not an ordinal step" % STEP
    idx = [i for i, x in enumerate(seq) if x[2] and x[2][0] == o.ORD]
    res, bad, out = {}, 0, [f"chrome_ver={pin} {TASK} ordinal step #{STEP}: count of button nodes in the interactive observation step #{STEP} resolved against"]
    for side in ("manuk", "chrome"):
        m.REC.clear(); raw = []
        r.replay(side, [x[:3] for x in seq], 2, settle=2.0, raw=raw)
        gaps = {x[0] for x in raw if x[3]}
        ok = [i for i in idx if i not in gaps]
        if STEP not in ok:
            out.append(f"{side}: UNCLASSIFIED (step #{STEP} unresolved)"); bad += 1; continue
        _, obs = m.REC[ok.index(STEP)]
        res[side] = buttons(obs)
    if bad:
        print("\n".join(out)); print("GATE FAIL: unresolved"); sys.exit(1)
    if a.mutate == "samenames":
        res["chrome"] = (res["chrome"][0], res["chrome"][1], collections.Counter(res["manuk"][2]))
    cm, nm, names_m = res["manuk"]; cc, nc, names_c = res["chrome"]
    only_c = sorted(set(names_c) - set(names_m)); only_m = sorted(set(names_m) - set(names_c))
    multi = collections.Counter(names_c); multi.subtract(names_m)
    multi_diff = sorted(((k, v) for k, v in multi.items() if v), key=lambda kv: -abs(kv[1]))
    out.append(f"manuk: {cm} button nodes ({nm} nameless), {len(names_m)} distinct names")
    out.append(f"chrome: {cc} button nodes ({nc} nameless), {len(names_c)} distinct names")
    out.append(f"names on chrome only ({len(only_c)}): " + (", ".join(f'"{x}"' for x in only_c[:40]) or "none"))
    out.append(f"names on manuk only ({len(only_m)}): " + (", ".join(f'"{x}"' for x in only_m[:40]) or "none"))
    out.append("count per name, chrome minus manuk (top 20): " + (", ".join(f'"{k}" {v:+d}' for k, v in multi_diff[:20]) or "none"))
    def cats(names, nl, total):
        date = sum(v for k, v in names.items() if re.fullmatch(r"\d{2}", k))
        ev = sum(v for k, v in names.items() if k.startswith("Event color indicator"))
        return f"date cells \"01\"-\"31\" {date}, event-indicator {ev}, nameless {nl}, other {total - date - ev - nl}"
    out.append("by category, manuk: " + cats(names_m, nm, cm))
    out.append("by category, chrome: " + cats(names_c, nc, cc))
    names_equal = not only_c and not only_m
    if cm != cc and names_equal:
        out.append("RED: button counts differ and the name sets are identical; no name explains the gap")
        bad += 1
    elif cm != cc:
        out.append(f"counts differ ({cc - cm:+d} on chrome); name sets differ by {len(only_c)} chrome-only and {len(only_m)} manuk-only names")
    else:
        out.append("counts equal")
    text = "\n".join(out)
    print(text)
    if a.out and not a.mutate:
        open(a.out, "w").write(text + "\n")
    if bad:
        print("GATE FAIL"); sys.exit(1)
    print("GATE PASS")


if __name__ == "__main__":
    main()
