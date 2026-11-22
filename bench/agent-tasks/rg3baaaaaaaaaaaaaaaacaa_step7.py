#!/usr/bin/env python3
"""RG.3.0.b.a.a.a.a.a.a.a.b.a.a.a.a.a.a.a.c.a.a: two fresh re-runs of the 13-step ordinal replay (rg3baaaaaaa_match.py)
at $MANUK_CHROME_PIN, recording opendining-4 step #7 (the click on 'Select time') per backend, into
docs/sprint/rg/RG3baaaaaaaaaaaaaaaacaa/ as step7-run{1,2}.json + RG3baaaaaaa-match-run{1,2}.{md,json}.
Reuses rg3baaaaaaaaaaaaaa_remeasure.one_run (same wrapper as the 3x re-measure, so the step-#7 records are comparable).
  rg3baaaaaaaaaaaaaaaacaa_step7.py      ~10 min per run; run with `python3 -I`; needs MANUK_CHROME_PIN.
Writes nothing else: the shared docs/sprint/rg/RG3baaaaaaa-match.{md,json} is overwritten by the match script and
copied out after each run (restore it from git afterwards; it is not a result of this node)."""
import os, sys
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import rg3baaaaaaaaaaaaaa_remeasure as rm

pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
d = os.path.join(rm.RG, "RG3baaaaaaaaaaaaaaaacaa")
os.makedirs(d, exist_ok=True)
for n in (1, 2):
    print(f"== run{n} chrome_ver={pin}", flush=True)
    rm.one_run(n, d)
    print(f"== run{n} done", flush=True)
