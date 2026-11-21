#!/usr/bin/env python3
"""RG.3.0.b.a.a.a.a.a.a.a.b: why opendining-7 ordinal step #2 (phone textbox) holds "+1" live but "+55 501 001 23 [invalid]"
in the transcript. Hypothesis: rg3b_replay.flatten drops the transcript's `fill_form` call (not in its tool list), so the
field is never filled before step #2.
  rg3baaaaaaaa_trace.py            replay as-is, write the step list + the live/transcript lines for #2 (both backends)
  rg3baaaaaaaa_trace.py --expand   same with fill_form expanded to type+click steps; #2 should now match on both.
Run with `python3 -I`; needs MANUK_CHROME_PIN. Exit 0 = trace written (as-is) / all #2 match (--expand)."""
import json, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import rg3b_replay as r
import rg3baaaaaaa_match as m

r.resolve = m.rec_resolve
T = "opendining-7"
expand = "--expand" in sys.argv
pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
st = r.lg.steps(os.path.join(r.TR, f"manuk_{T}_r1.jsonl"))[0]
tr_calls = [(n, tool, json.loads(inp)) for n, tool, inp, _ in st]
if expand:
    st2 = []
    for n, tool, inp, res in st:
        a = json.loads(inp)
        if tool == "fill_form":
            steps = [{"tool": "type", "handle": f["handle"], "text": f["value"]} for f in a["fields"]]
            if a.get("submit"):
                steps.append({"tool": "click", "handle": a["submit"]})
            inp = json.dumps({"steps": steps})
        st2.append((n, tool, inp, res))
    st = st2
seq = m.flatten_ref(st, 0)
lines = ["## RG.3.0.b.a.a.a.a.a.a.a.b" + (" (fill_form expanded)" if expand else ""), "",
         f"chrome_ver={pin}. {T}: replayed step list (flatten of the transcript) beside the transcript calls.", "",
         "Transcript calls: " + "; ".join(f"call {n} {t}" for n, t, _ in tr_calls), "",
         "Replayed steps: " + "; ".join(f"#{i} {s[0]} {s[1].get('handle', '')}" for i, s in enumerate(seq)), "",
         "Replay tool list in rg3b_replay.flatten: act type key navigate scroll select check press commit back (no fill_form).", ""]
idx = [i for i, s in enumerate(seq) if s[2] and s[2][0] == m.o.ORD]
ok_all = True
for side in ("manuk", "chrome"):
    m.REC.clear(); raw = []
    r.replay(side, [x[:3] for x in seq], 2, settle=2.0, raw=raw)
    gaps = {x[0] for x in raw if x[3]}
    got = [i for i in idx if i not in gaps]
    for i, (h, obs) in zip(got, m.REC):
        live = m.node_lines(obs)[h]; want = seq[i][3]
        mt = live["role"] == want["role"] and live["sv"] == want["sv"]
        if i == idx[0]:
            ok_all &= mt
        lines.append(f"- {side} #{i}: transcript `{want['line']}` live `{live['line']}` match={'yes' if mt else 'no'}")
    print(lines[-len(got):] if got else "no ordinal resolved", flush=True)
open(os.path.join(r.ROOT, "docs/sprint/rg/RG3baaaaaaaab-trace" + ("-expand" if expand else "") + ".md"), "w").write("\n".join(lines) + "\n")
sys.exit(0 if (not expand or ok_all) else 1)
