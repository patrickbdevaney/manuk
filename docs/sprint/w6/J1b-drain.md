# W6.J1.b.a — is the css-transitions 0/560 script stall the engine's drain budget?

chrome_ver=145.0.7632.116 (MANUK_CHROME_PIN). Engine binary: `target/release/manuk-wpt`, rebuilt at HEAD `ad51ab7d`
(the binary from 14:10 predated the last engine commit, so the first set of runs was discarded and rerun; see below).
No engine/ or tests/wpt change in this node.

## Question and arms

The mozjs warning `Script terminated by timeout` comes from the engine's JS interrupt watchdog
(`engine/js/src/watchdog.rs`, `ScriptDeadline::arm`). Its clock is `max_drain_ms()` (`engine/js/src/event_loop.rs`,
default 5000 ms; `MANUK_MAX_DRAIN_MS=0` disables the bound, `arm` returns an unarmed guard). This node asks whether
removing that bound changes the score.

- **default**: `MANUK_MAX_DRAIN_MS` unset (5000 ms drain budget).
- **off**: `MANUK_MAX_DRAIN_MS=0` (no drain bound).

Both arms use `--timeout 120` (page-load phase, not the variable under test). Each run is wrapped in an outer
`timeout 300` child guard (a hang would be rc=124, no JSON). Command per run:

    ./target/release/manuk-wpt wpt css/css-transitions --start <75|79> --limit 1 --timeout 120 --show-failures --json <out>

`75` = `properties-value-001.html`, `79` = `properties-value-inherit-001.html`. 3 runs per (arm, file) = 12 runs.
Raw JSON + full runner logs: `docs/sprint/w6/raw/J1b-drain/`. Verdict script: `docs/sprint/w6/raw/J1b-drain/verdict.py`.

## Per-run results (pass/total, harness status, wall ms)

| file | arm | run 1 | run 2 | run 3 |
|---|---|---|---|---|
| properties-value-001 | default | TIMEOUT 0/560, 8148 ms | NO_REPORT 0/0 (excluded), 14982 ms | TIMEOUT 0/560, 8161 ms |
| properties-value-001 | off | OK 0/560, 770 ms | OK 0/560, 858 ms | OK 0/560, 867 ms |
| properties-value-inherit-001 | default | NO_REPORT 0/0 (excluded), 14339 ms | NO_REPORT 0/0 (excluded), 15016 ms | TIMEOUT 0/560, 8352 ms |
| properties-value-inherit-001 | off | OK 0/560, 856 ms | OK 0/560, 903 ms | OK 0/560, 885 ms |

Scored runs: pass count is **0/560 in every scored run, in both arms.**
`mozjs` `Script terminated by timeout` count in each log: default arm 1–2 per log (6 of 6 logs); off arm 0 (6 of 6 logs).

## Verdict

Pre-registered falsifier, applied to pass counts: **pass counts are identical with the drain bound removed**, so
`verdict.py` returns `NOT_DRAIN` for both files. Per the pre-registered rule, the drain budget is not the cause of the
0/560 score, and a new row is opened on the assertion-level failures (follow-up W6.J1.b.a.a).

The same runs also show something the falsifier does not capture, stated plainly: the drain bound **is** what kills
the script. With the bound removed, the `Script terminated by timeout` warning disappears and the harness status moves
from TIMEOUT (8 s) to OK (under 1 s), because the test runs to completion. The stall is therefore the 5 s drain budget
interrupting script execution; it is not what determines the score. The score is zero because every subtest fails on
an assertion once the script is allowed to finish.

What the drain-off arm reports (`off_r1_*.log`, 560 FAIL lines per file, all at assertion level):

| file | `got disallowed value` (values subtests) | `Expected TransitionEnd events` (events subtests) | other |
|---|---|---|---|
| properties-value-001 | 280 | 280 | 0 |
| properties-value-inherit-001 | 280 | 280 | 0 |

Example: `background-color color(rgba) / values` → `assert_not_equals: initial and target values may not match got
disallowed value ""`; `background-color color(rgba) / events` → `Expected TransitionEnd events triggered on .transition
expected "background-color:2s" but got ""`.

Also recorded: the drain-off log shows `FORCED-REFLOW BUDGET EXHAUSTED` (2 per log in `off_r1_properties-value-001.log`).
That is a second budget in the engine, which this node does not score.

## Mutation check (the verdict must be able to go red)

- **Mutant, synthetic input** (`/tmp`, not in the repo): drain-off arm 300/560 passes, default arm 0/560. `verdict.py`
  → `DRAIN` for both files. With `--swap` the same input → `INVERTED`. The verdict logic goes red on both sides.
- **Real data, `--swap`**: still `NOT_DRAIN`. This is vacuous (all arms are 0/560, so no relabelling can flip it),
  the same limit noted in W6.J1.b. Stated plainly, not counted as evidence.

## Instrument notes (recorded, not scored)

- Default arm: 3 of 6 runs returned `NO_REPORT` with 0/0 subtests after ~14–15 s (`properties-value-inherit-001`
  runs 1–2, `properties-value-001` run 2). These are excluded from the verdict (total 0), the same convention as W6.J1.b.
  They are the 5 s drain budget plus a harness report that did not parse; they are not an engine result.
- The OK/TIMEOUT harness status is noisy across runs; the pass count is the stable quantity.

## Stale-binary runs (discarded, kept for transparency)

A first set of 12 runs used `target/release/manuk-wpt` built at 14:10, before the last engine commit (`ad51ab7d`,
14:15). Results: `docs/sprint/w6/raw/J1b-drain-stale-binary/`. Same pattern: 0/560 in every scored run in both arms,
default arm stalls with the drain warning, drain-off arm completes at about 0.4–0.5 s. The rebuilt-binary numbers above
supersede it. Neither set changes the verdict.

## Node verdict

The `--timeout` artefact (W6.J1.b) is ruled out. The drain budget is ruled in as the **mechanism that stops the
script** (`Script terminated by timeout`), but ruled out as the **cause of the 0/560 score** (pass counts identical
with the bound removed). The score is an engine assertion gap: value-parsing for `color(rgba)` and similar, and
TransitionEnd events not firing. Follow-up: W6.J1.b.a.a (categorise the assertion failures per property).
No engine change in this node.
