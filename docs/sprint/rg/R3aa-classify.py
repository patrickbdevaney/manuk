#!/usr/bin/env python3
"""RG.3.0.a.a: classify why a REAL run did not finish, from its transcript alone.
Labels: engine (a tool result in the last third reports OCCLUDED / unresolved /
NO EFFECT and the agent's final text blames the page), model-stop (every action
in the last third succeeded and the final text is the agent declining/stopping
by its own judgement), pass (final text claims completion).
Usage: R3aa-classify.py [--mutate] TRANSCRIPT... ; exits 1 if a label differs
from EXPECT. --mutate swaps the engine/model-stop labels (gate must go red)."""
import json, re, sys, os
EXPECT = {"manuk_staynb-4_r3": "model-stop", "manuk_udriver-10_r2": "model-stop",
          "manuk_staynb-4_r1": "pass", "manuk_udriver-10_r3": "pass"}
ERR = re.compile(r"OCCLUDED|unresolved|NO EFFECT|FAILED|not found|stale", re.I)
STOP = re.compile(r"^(I stopped|I didn'?t book|I did not|I couldn'?t|I was unable)", re.I)
def events(p):
    out = []
    for l in open(p):
        try: o = json.loads(l)
        except ValueError: continue
        c = o.get("message", {}).get("content")
        if not isinstance(c, list): continue
        for b in c:
            t = b.get("type")
            if t == "tool_result":
                x = b.get("content"); x = x if isinstance(x, str) else " ".join(y.get("text", "") for y in x)
                out.append(("res", x))
            elif t == "text" and o.get("type") == "assistant": out.append(("say", b["text"]))
    return out
def classify(p, mutate=False):
    ev = events(p); final = [t for k, t in ev if k == "say"][-1]
    res = [t for k, t in ev if k == "res"]; tail = res[len(res) * 2 // 3:]
    errs = [t[:120] for t in tail if ERR.search(t[:400])]
    if not STOP.match(final.strip()): lab = "pass"
    elif errs: lab = "engine"
    else: lab = "model-stop"
    if mutate and lab != "pass": lab = {"engine": "model-stop", "model-stop": "engine"}[lab]
    return lab, errs, final[:160]
if __name__ == "__main__":
    mut = "--mutate" in sys.argv; bad = 0
    for p in [a for a in sys.argv[1:] if a != "--mutate"]:
        k = os.path.basename(p)[:-6]; lab, errs, fin = classify(p, mut)
        ok = EXPECT.get(k) == lab; bad += not ok
        print(f"{k}\t{lab}\texpect={EXPECT.get(k)}\ttail_errors={len(errs)}\t{'OK' if ok else 'MISMATCH'}\t{fin!r}")
    sys.exit(1 if bad else 0)
