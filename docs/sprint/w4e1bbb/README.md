# W4.E1.b.b.b — lean vs default subres_fetch on ebanking.bdm-sa.com (chrome_ver=145.0.7632.116)

**Setup.** Paired same-binary A/B: arm A = default, arm B = `MANUK_AGENT_PROFILE=lean`, binary `target/release/a11y-dump` at HEAD
c8360ed5 (copied to /tmp/a11y-dump.w4e1bbb). Driver: `scripts/perf1-profile.py --runs 3 --urls docs/sprint/w4e1bbb/urls.txt
--manuk-b /tmp/a11y-dump.w4e1bbb --env-b MANUK_AGENT_PROFILE=lean --engines manuk --timeout 90`. Arm order flips each run.
Raw rows: `paired.jsonl` (all 6 attempts kept), traces `traces/<arm>/`, log `run.log`. Analysis: `scripts/w4e1bb-lean-ab.py` -> `verdict.txt`.

**Result (all 3 attempts per arm, nothing dropped, no extra runs).**

| arm | run 1 | run 2 | run 3 | OK | FAIL |
|---|---|---|---|---|---|
| A default | OK subres 4.78 s | FAIL (rc 1, 16.3 s) | FAIL (rc 1, 16.6 s) | 1 | 2 |
| B lean    | OK subres 3.38 s | OK subres 2.34 s | FAIL (rc 1, 16.6 s) | 2 | 1 |

**Verdict: NOT_SETTLED.** Fewer than 3 OK runs per arm, so no band is computed and no speed claim is made either way.
The raw OK values (A 4.78 s; B 3.38 s, 2.34 s) are one-sample observations, not a band. This matches W4.E1.b.b: the site resets
about 60% of handshakes (W4.E1.b.a), so the FAIL count is the honest result. The run was not repeated to get three OK runs,
because that would tune the sample to the outcome.

**Gate.** `scripts/w4e1bb-lean-ab.py --self-test` OK: a treated synthetic pair is SETTLED; a no-treatment mutant is red
(`TREATMENT NOT APPLIED`); a 2-OK-run mutant gets no band (`INSUFFICIENT`). The real-data gate accepts the per-arm FAIL count
over all 3 attempts, which is reported above.

**Caveat.** The FAIL rows carry no failure reason in `paired.jsonl`; the cause (handshake reset) is inferred from W4.E1.b.a, not
captured here.
