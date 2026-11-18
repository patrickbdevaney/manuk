# W4.E1.b.b — does lean mode (W4.E2) touch the late subres_fetch time? (chrome_ver=145.0.7632.116)

**Setup.** Paired same-binary A/B of `target/release/a11y-dump` (rebuilt at HEAD for this node, copied to /tmp):
arm A = default, arm B = `MANUK_AGENT_PROFILE=lean`. Driver: `scripts/perf1-profile.py --manuk-b ... --env-b MANUK_AGENT_PROFILE=lean
--urls docs/sprint/w4e1bb/urls.txt --runs 3 --timeout 90`; arm order flips each run. Raw rows: `paired.jsonl`, traces in `traces/<arm>/`,
log in `run.log`. Analysis: `scripts/w4e1bb-lean-ab.py` (output in `verdict.txt`).

**Two things were measured, not assumed.** (1) Does lean actually change what goes on the wire? Image body bytes per arm (full
bodies plus the 206 ranged-header prefixes lean reads instead). (2) Does subres_fetch move beyond the 3-run band?

| URL | subres_fetch A (default) min..max, median | subres_fetch B (lean) min..max, median | band verdict | image bytes A / B (median, MB) |
|---|---|---|---|---|
| comix.to | 1.42..19.02, 6.09 | 1.03..7.20, 1.72 | overlap (A spread 13x: 3 runs cannot place it) | 5.39 / 0.29 |
| mobile.bg | 2.43..2.63, 2.45 | 2.64..2.67, 2.65 | A faster, bands disjoint by 0.01 s | 0.56 / 0.07 |
| puentedemando | 2.61..3.27, 2.92 | 2.07..3.26, 2.15 | overlap | 5.58 / 0.59 |
| ebanking (HTTP/1.x) | 1 OK run of 3 (2 FAIL, exit 1) | 2 OK runs of 3 (1 FAIL) | not computed: fewer than 3 OK runs in an arm | 0.16 / 0.00 |

**Reading.**
- Lean does what it says on the wire: image bytes fall 95% (comix), 88% (mobile.bg), 89% (puentedemando) and ~100% (ebanking, only 2 OK runs).
  The bytes move to 206 ranged header reads (2 KB prefixes) on the same image URLs.
- It does **not** move subres_fetch beyond the 3-run band on the sites where a band exists. On mobile.bg lean is 0.2 s slower
  at the median, with bands separated by 0.01 s; that is not a claim either way. So for these pages, the image bodies lean defers
  are not the late time that W4.E1.b attributed (85-96% of subres_fetch in requests discovered >100 ms after the parse batch).
  Stated as an observation: the bytes lean removes are not the seconds.
- Lean has a defect: it requests the same image URL more than once. mobile.bg lean issues 43 image requests for 29 unique URLs;
  puentedemando 70 for 54 (default: 81/81 and 55/55). Follow-up W4.E1.b.b.a.
- ebanking: arm A failed 2 of 3 runs and arm B 1 of 3 (the known handshake resets, see W4.E1.b.a). Not enough OK runs for a band, and
  I did not re-run it to get three, because that would be tuning the sample to the outcome.

**Gate and its red proof.**
- `scripts/w4e1bb-lean-ab.py --self-test` OK: a synthetic lean-treated pair is SETTLED; a mutant where arm B carries the same
  image bytes (env not applied) is red (`TREATMENT NOT APPLIED`); a mutant with 2 OK runs gets no band verdict.
- Real data, same-arm mutant (`--arm-a manuk --arm-b manuk`, `mutant-same-arm.txt`): exit 2, `GATE VOID`, treatment not applied.
- Real-data verdict (`verdict.txt`): NOT_SETTLED, because of the ebanking sample only; the three other URLs carry the treatment.

**Caveats.** Image bytes are read from trace `body` lines and the 206 prefix length; the extension regex is the only image
classifier (an image served without an extension is counted as non-image on both arms). Traces are per run, not per second, so
"late" time is the W4.E1.b heuristic, not discovery timestamps.
