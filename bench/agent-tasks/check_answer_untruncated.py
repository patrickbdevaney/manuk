#!/usr/bin/env python3
"""W4.P0.h gate: a stored answer equals the transcript's final text (flattened), with no cap.
Feeds a 1000-char answer through the real row builders' shared helper and through a fake claude
for run_arm.run_one-style flow is too heavy; the cap lived only in flat_answer's call sites, so we
check the helper and that no call site slices the answer."""
import re, sys, os
sys.path.insert(0, os.path.dirname(__file__))
import run_arm
long = ("word\t" + "x\n" * 600)
want = long.replace("\t", " ").replace("\n", " ")
assert len(want) > 1000
assert run_arm.flat_answer(long) == want, "stored answer differs from final text"
here = os.path.dirname(__file__)
for f in ("run_arm.py", "real_run.py"):
    src = open(os.path.join(here, f)).read()
    bad = re.findall(r'"answer":[^\n]*\[:\d+\]', src)
    assert not bad, f"{f}: answer still sliced: {bad}"
    assert "flat_answer(" in src, f"{f}: does not use flat_answer"
print("ok: full answer stored", len(want))
