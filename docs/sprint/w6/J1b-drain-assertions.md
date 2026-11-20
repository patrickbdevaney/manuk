# W6.J1.b.a.a: categorising the drain-off assertion failures

chrome_ver=145.0.7632.116 (MANUK_CHROME_PIN). Counting only; no engine/ or tests/wpt change.
Source: the drain-off arm (`MANUK_MAX_DRAIN_MS=0`) runs from W6.J1.b.a, logs in `docs/sprint/w6/raw/J1b-drain/`.
Classifier: `docs/sprint/w6/raw/J1b-drain/categorise.py` (each `FAIL <subtest>` header is matched to its assertion
message line; `disallowed` = message contains `got disallowed value`, `transitionend` = contains
`Expected TransitionEnd events`, `other` = any FAIL matching neither).

## Gate result: off_r1 (the gated run)

| file | FAIL subtests | `disallowed value` (values) | `Expected TransitionEnd` (events) | other |
|---|---|---|---|---|
| properties-value-001 | 560 | 280 | 280 | **0** |
| properties-value-inherit-001 | 560 | 280 | 280 | **0** |

`other = 0` in both files, so the split is complete and the row does not stay open under its falsifier.
`fail_total` equals the runner's `total` (560) in both files, so no FAIL line was dropped.

## Consistency: off_r2 and off_r3 (same binary, same arm)

| log | FAIL | disallowed | TransitionEnd | other |
|---|---|---|---|---|
| off_r2 properties-value-001 | 560 | 280 | 280 | 0 |
| off_r2 properties-value-inherit-001 | 560 | 280 | 280 | 0 |
| off_r3 properties-value-001 | 560 | 280 | 280 | 0 |
| off_r3 properties-value-inherit-001 | 560 | 280 | 280 | 0 |

## Mutant (the classifier must be able to go red)

`categorise.py --selftest` (run in the raw directory): a synthetic log with two FAIL subtests whose message is
`assert_true: something else entirely` and one with no message line. Result: `other=2`, `disallowed=1`,
`transitionend=1`, `fail_total=4`. A third message type lands in `other`, and the check fails if it lands in either
named category.

## Reading

Each of the 280 test cases (grouped by name, e.g. `background-color color(rgba)`) contributes one `/ values` subtest
(fails with `got disallowed value ""`) and one `/ events` subtest (fails because no TransitionEnd is dispatched). Checked
in off_r1 properties-value-001: 280 distinct case names, each with exactly 2 subtests, 280 `/ values` and 280 `/ events`.
So the 560 failures are 280 value-parse failures plus 280 missing-TransitionEnd failures, one pair per case. The
pairing is structural to the test, so the 280/280 split is expected, not a coincidence.

What this does not show: which of the two gaps is upstream. A `disallowed value` on the values subtest may mean the
transition never starts; that is an inference from the test design, not something these logs establish. Diagnosing
the cause belongs to a follow-up engine row, not this counting node.
