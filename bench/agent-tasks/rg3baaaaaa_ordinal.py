#!/usr/bin/env python3
"""RG.3.0.b.a.a.a.a.a.a: resolve the nameless nodes of the replay by role + ordinal. A nameless handle (no quoted
name in the transcript) is recorded as (role, n) = its position among the nodes of that role in the last transcript
observation that carried it; the replay resolves it to the nth node of that role in the live observation.
  rg3baaaaaa_ordinal.py [--mutate shift-ordinal]   exit 0 = GATE PASS, 1 = GATE FAIL
--mutate shift-ordinal moves every ordinal by one; the gate needs the resolved count or first gap to change.
Writes docs/sprint/rg/RG3baaaaaa-ordinal.md. Run with `python3 -I`; needs MANUK_CHROME_PIN."""
import argparse, json, os, re, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import rg0_ledger as lg
import rg3b_replay as r

TASKS = "opendining-4 opendining-7 gocalendar-2".split()
LINE = re.compile(r'^\s*\[(h\d+)\] (\w+)', re.M)
ORD = "#ord"


def ordinals(text):
    cnt, out = {}, {}
    for m in LINE.finditer(text):
        out[m.group(1)] = (m.group(2), cnt.get(m.group(2), 0)); cnt[m.group(2)] = cnt.get(m.group(2), 0) + 1
    return out


def flatten_ord(st, shift=0):
    """r.flatten, but a nameless handle gets rn = (ORD, role, n) from the last observation before the step that carried it."""
    base = r.flatten(st)
    seen_ord, k, out = {}, 0, []
    # walk calls in the same order as flatten so the k-th step lines up with base[k]
    for n, tool, inp, res in st:
        try:
            a = json.loads(inp)
        except ValueError:
            a = {}
        subs = a.get("steps") if isinstance(a.get("steps"), list) else None
        items = [("act" if s.get("tool") == "click" else s.get("tool"), s) for s in subs] if subs else [(tool, a)]
        for t, args in items:
            if t in ("act", "type", "key", "navigate", "scroll", "select", "check", "press", "commit", "back"):
                bt, ba, rn = base[k]; k += 1
                h = args.get("handle")
                if h and rn is None and h in seen_ord:
                    role, o = seen_ord[h]
                    rn = (ORD, role, o + shift)
                out.append((bt, ba, rn))
        seen_ord.update(ordinals(res))
    assert k == len(base)
    return out


_resolve = r.resolve


def resolve(obs, rn, text=False, force=False):
    if rn is not None and rn[0] == ORD:
        _, role, o = rn
        hs = [m.group(1) for m in LINE.finditer(obs) if m.group(2) == role]
        return hs[o] if 0 <= o < len(hs) else None
    return _resolve(obs, rn, text, force)


def main():
    ap = argparse.ArgumentParser(); ap.add_argument("--mutate", choices=["shift-ordinal"]); ap.add_argument("--wait", type=float, default=2)
    a = ap.parse_args()
    pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
    r.resolve = resolve
    shift = 1 if a.mutate else 0
    rows, tot = [], {"manuk": [0, 0], "chrome": [0, 0]}
    for t in TASKS:
        seq = flatten_ord(lg.steps(os.path.join(r.TR, f"manuk_{t}_r1.jsonl"))[0], shift)
        idx = [i for i, (_, _, rn) in enumerate(seq) if rn and rn[0] == ORD]
        for side in ("manuk", "chrome"):
            raw = []
            r.replay(side, seq, a.wait, settle=2.0, raw=raw)
            gaps = {x[0] for x in raw if x[3]}
            ok = [i for i in idx if i not in gaps]
            first = next((x[0] for x in raw if x[3]), None)
            tot[side][0] += len(ok); tot[side][1] += len(idx)
            rows.append((t, side, len(idx), len(ok), sorted(gaps & set(idx)), first))
            print(rows[-1], flush=True)
    tab = ["| task | backend | ordinal steps | resolved | unresolved ordinal steps | first gap (any) |", "|---|---|---|---|---|---|"]
    tab += [f"| {t} | {s} | {n} | {o} | {', '.join('#%d' % i for i in u) or '-'} | {'#%d' % f if f is not None else '-'} |" for t, s, n, o, u, f in rows]
    tab.append(f"| all | manuk | {tot['manuk'][1]} | {tot['manuk'][0]} | | |")
    tab.append(f"| all | chrome | {tot['chrome'][1]} | {tot['chrome'][0]} | | |")
    print("\n".join(tab[-2:]))
    sig = [(x[3], x[5]) for x in rows]
    if a.mutate is None:
        md = ["## RG.3.0.b.a.a.a.a.a.a", "", f"chrome_ver={pin}. Replay (settle 2 s, wait {a.wait:g} s) with each nameless handle resolved to the nth node of its role "
              "(n = its position among that role's nodes in the last transcript observation carrying it). The 12 ordinal steps are counted per backend (24 in the classify table).", "", *tab, ""]
        open(os.path.join(r.ROOT, "docs/sprint/rg/RG3baaaaaa-ordinal.md"), "w").write("\n".join(md) + "\n")
        open("/tmp/rg3_ord_sig.json", "w").write(json.dumps(sig))
        sys.exit(0)
    base = json.load(open("/tmp/rg3_ord_sig.json"))
    changed = base != [list(x) for x in sig]
    print("mutant changes resolved count or first gap:", changed, "GATE", "PASS (red detected)" if changed else "FAIL (mutant not detected)")
    sys.exit(0 if changed else 1)


if __name__ == "__main__":
    main()
