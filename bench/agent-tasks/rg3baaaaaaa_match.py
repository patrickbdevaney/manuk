#!/usr/bin/env python3
"""RG.3.0.b.a.a.a.a.a.a.a: for the 12 ordinal steps per backend of the RG.3.0.b.a.a.a.a.a.a replay, print the live
observation line of the handle each step resolved to beside the transcript line of the node it was recorded against,
and count a match when role and state/value agree. The accessible name is not compared: an ordinal step has no name.
Unresolved ordinal steps count as non-matches. The denominator is the number of ordinal steps the replay produces
(13 per backend once fill_form is expanded; 12 before).
  rg3baaaaaaa_match.py [--mutate shift-ordinal]
  Unmutated run: measures, writes docs/sprint/rg/RG3baaaaaaa-match.md and the baseline counts
  docs/sprint/rg/RG3baaaaaaa-match.json, exit 0.
  --mutate shift-ordinal: every ordinal is moved by one for resolution only (the transcript reference stays put).
  exit 0 = GATE PASS (the mutant lowered the match count, i.e. the check is red on the mutant); exit 1 = GATE FAIL.
Run with `python3 -I`; needs MANUK_CHROME_PIN and the baseline JSON before a mutated run."""
import argparse, json, os, re, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import rg3b_replay as r
import rg3baaaaaa_ordinal as o

TASKS = o.TASKS
DOC = os.path.join(r.ROOT, "docs/sprint/rg/RG3baaaaaaa-match.md")
BASE = os.path.join(r.ROOT, "docs/sprint/rg/RG3baaaaaaa-match.json")
NODE_LINE = re.compile(r'^\s*\[(h\d+)\] (\w+)(.*)$', re.M)
NAME = re.compile(r'^ "[^"]*"')
STALE_RULE = os.environ.get("RG_STALE_RULE", "1") != "0"  # decision doc RG3baaaaaaaaaaaaaaaaca-decision.md
REC = []  # (handle, observation) for each successful ordinal resolution, in call order


def node_lines(text):
    """handle -> {role, sv (state/value: the line after role and name), n (ordinal among same-role nodes), line}."""
    cnt, out = {}, {}
    for m in NODE_LINE.finditer(text):
        h, role, rest = m.group(1), m.group(2), m.group(3)
        n = cnt.get(role, 0); cnt[role] = n + 1
        out[h] = {"role": role, "sv": NAME.sub("", rest).strip(), "n": n, "line": m.group(0).strip()}
    return out


def flatten_ref(st, shift):
    """r.flatten with each nameless step tagged: (tool, args, rn, ref) where rn is the resolution key (ordinal moved by
    `shift`) and ref is the transcript node (unmoved) the handle was recorded against."""
    base = r.flatten(st)
    seen, k, out, stale = {}, 0, [], False
    for n, tool, inp, res in st:
        try:
            a = json.loads(inp)
        except ValueError:
            a = {}
        for t, args in r.items_of(tool, a):
            if t in ("act", "type", "key", "navigate", "scroll", "select", "check", "press", "commit", "back"):
                bt, ba, rn = base[k]; k += 1
                h = args.get("handle")
                ref = None
                if h and rn is None and h in seen:
                    ref = dict(seen[h], stale=stale)
                    rn = (o.ORD, ref["role"], ref["n"] + shift)
                out.append((bt, ba, rn, ref))
        seen.update(node_lines(res))
        stale = stale or "NO EFFECT" in res
    assert k == len(base)
    return out


def rec_resolve(obs, rn, text=False, force=False):
    h = o.resolve(obs, rn, text, force)
    if h and rn is not None and rn[0] == o.ORD:
        REC.append((h, obs))
    return h


def main():
    ap = argparse.ArgumentParser(); ap.add_argument("--mutate", choices=["shift-ordinal"]); ap.add_argument("--wait", type=float, default=2)
    a = ap.parse_args()
    pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
    if a.mutate and not os.path.exists(BASE):
        sys.exit("refusing: no baseline at " + BASE + "; run the unmutated gate first")
    r.resolve = rec_resolve
    shift = 1 if a.mutate else 0
    detail, counts, n_ord = [], {"manuk": 0, "chrome": 0}, 0
    strict = {"manuk": 0, "chrome": 0}
    for t in TASKS:
        st = r.lg.steps(os.path.join(r.TR, f"manuk_{t}_r1.jsonl"))[0]
        seq = flatten_ref(st, shift)
        ref_nodes = {i: seq[i][3] for i in range(len(seq))}
        idx = [i for i, (_, _, rn, _) in enumerate(seq) if rn and rn[0] == o.ORD]
        n_ord += len(idx)
        for side in ("manuk", "chrome"):
            REC.clear(); raw = []
            r.replay(side, [x[:3] for x in seq], a.wait, settle=2.0, raw=raw)
            gaps = {x[0] for x in raw if x[3]}
            ok = [i for i in idx if i not in gaps]
            assert len(ok) == len(REC), (t, side, len(ok), len(REC))
            for i, (h, obs) in zip(ok, REC):
                live = node_lines(obs)[h]; want = ref_nodes[i]
                m = live["role"] == want["role"] and live["sv"] == want["sv"]
                strict[side] += m
                tol = (not m and STALE_RULE and want.get("stale") and want["role"] == "combobox"
                       and want["sv"] == "[collapsed]" and live["role"] == "combobox" and live["sv"] == "[expanded]")
                m = m or tol
                counts[side] += m
                detail.append({"task": t, "side": side, "step": i, "transcript": want["line"], "live": live["line"], "match": bool(m), "stale_ref": bool(tol)})
                print(("MATCH " if m else "DIFF  ") + f"{t} {side} #{i}\n   transcript: {want['line']}\n   live:       {live['line']}", flush=True)
            for i in idx:
                if i in gaps:
                    detail.append({"task": t, "side": side, "step": i, "transcript": ref_nodes[i]["line"], "live": None, "match": False})
                    print(f"UNRESOLVED {t} {side} #{i}  transcript: {ref_nodes[i]['line']}", flush=True)
    tot = {s: sum(1 for d in detail if d["side"] == s) for s in counts}
    assert all(v == n_ord for v in tot.values()), (tot, n_ord)
    summary = f"manuk {counts['manuk']}/{n_ord} match, chrome {counts['chrome']}/{n_ord} match (of {n_ord} ordinal steps each)"
    print(summary, f"[strict (rule off): manuk {strict['manuk']}, chrome {strict['chrome']}]")
    if a.mutate is None:
        json.dump({"chrome_ver": pin, "counts": counts, "strict_counts": strict}, open(BASE, "w"), indent=1)
        md = ["## RG.3.0.b.a.a.a.a.a.a.a", "", f"chrome_ver={pin}. For each ordinal step of the RG.3.0.b.a.a.a.a.a.a replay "
              "(settle 2 s, wait 2 s): the live line of the resolved handle beside the transcript line of the node it was "
              "recorded against. Match = same role and same state/value (name excluded).", "",
              "| task | backend | step | transcript line | live line | match |", "|---|---|---|---|---|---|"]
        for d in detail:
            md.append(f"| {d['task']} | {d['side']} | #{d['step']} | `{d['transcript']}` | `{d['live'] or '-'}` | {'yes' if d['match'] else 'no'} |")
        md += ["", "Rows scored by the stale-reference tolerance (strictly a miss): " + (", ".join(f"{d['task']} {d['side']} #{d['step']}" for d in detail if d.get("stale_ref")) or "none"), ""]
        md += ["", f"**{summary}**", "", f"Strict count (stale-reference tolerance off): manuk {strict['manuk']}/{n_ord}, chrome {strict['chrome']}/{n_ord}.", ""]
        open(DOC, "w").write("\n".join(md) + "\n")
        sys.exit(0)
    base = json.load(open(BASE))["counts"]
    lower = sum(counts.values()) < sum(base.values())
    print(f"baseline manuk {base['manuk']} chrome {base['chrome']}; mutant total {sum(counts.values())} vs baseline {sum(base.values())}")
    print("GATE", "PASS (red detected: mutant lowers the match count)" if lower else "FAIL (mutant did not lower the match count)")
    sys.exit(0 if lower else 1)


if __name__ == "__main__":
    main()
