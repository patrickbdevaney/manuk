# W6.J1.b — are the css-transitions 0/560 TIMEOUTs watchdog budget or engine?

chrome_ver=145.0.7632.116 (MANUK_CHROME_PIN). Engine binary: `target/release/manuk-wpt`, built from the tree at
HEAD 9f35bdcd (no engine/ or tests/wpt change in this node). Checkout `~/wpt`, unmodified.

Command, per run (file index from the sorted discovery of `css/css-transitions`, which is what `--start/--limit` slices):

    ./target/release/manuk-wpt wpt css/css-transitions --start <75|79> --limit 1 --timeout <10|120> --show-failures --json docs/sprint/w6/raw/J1b/t<T>_r<R>_<file>.json

`75` = `properties-value-001.html`, `79` = `properties-value-inherit-001.html`. 3 runs per (file, timeout) = 12 runs.
Raw JSON + full runner logs: `docs/sprint/w6/raw/J1b/`. Verdict script: `docs/sprint/w6/raw/J1b/verdict.py` (reads the raw JSON).

## Per-run results (pass/total, harness status, wall ms)

| file | --timeout | run 1 | run 2 | run 3 |
|---|---|---|---|---|
| properties-value-001 | 10 | TIMEOUT 0/560, 8490 ms | NO_REPORT 0/0 (instrument flake), 14130 ms | TIMEOUT 0/560, 8172 ms |
| properties-value-001 | 120 | OK 0/560, 9737 ms | TIMEOUT 0/560, 8416 ms | TIMEOUT 0/560, 8601 ms |
| properties-value-inherit-001 | 10 | TIMEOUT 0/560, 8498 ms | TIMEOUT 0/560, 8460 ms | TIMEOUT 0/560, 8380 ms |
| properties-value-inherit-001 | 120 | TIMEOUT 0/560, 6689 ms | TIMEOUT 0/560, 8882 ms | TIMEOUT 0/560, 8565 ms |

Pass counts across all 12 runs: **0 of 560 subtests in every scored run**, at both timeouts.

## Verdict

The pre-registered falsifier fired: pass counts are identical at 10 s and 120 s (`verdict.py` -> `ENGINE` for both files).
The `--timeout` is not the binding constraint. It bounds only the page-load phase (`run_one`, `tokio::time::timeout`
around fetch + `load_async` + `finish_loading`); these pages finish that phase in roughly 8 s, well inside 10 s, so
raising it to 120 s changes nothing. The TIMEOUT that stops the test is testharness's own status, and the raw logs
locate the stall inside the test script: `mozjs::rust Warning ... Script terminated by timeout` in `assert_wrapper`
(`properties-value-001`, `runLoop` / `Test.prototype.step`) and in `getStyle` / `_getStyleFor` (`properties-value-inherit-001`).
So the 0/560 is an engine-side stall, not a `--timeout` artefact.

Two caveats, stated rather than hidden:
- The 0/560 is a stall inside script execution, not a measured wrong answer. Where a test finished (`t120_r1`,
  `properties-value-001`, OK 0/560), its messages are real assertion failures (e.g. `background-color color(rgba) /
  values: got disallowed value "rgba(10, 10, 10, 0.4)"`). Those per-subtest failures are the engine gap once the stall is
  removed; this node does not score them.
- The stall is not clearly the same mechanism as the `--timeout` budget, but it is an engine-side *clock* bound: the
  mozjs warning comes from the engine's JS interrupt watchdog (`engine/js/src/watchdog.rs`), whose drain clock is
  `max_drain_ms()` (`engine/js/src/event_loop.rs`, default `DEFAULT_MAX_DRAIN_MS = 5000`). Whether that bound kills
  these scripts is the next question and is left to a follow-up row (W6.J1.b.a), not settled here.

## Mutation check (the verdict must be able to go red)

- Mutant A, swap the two timeout labels on the REAL data: `verdict.py --swap` -> still `ENGINE`. This mutant is vacuous
  here, because both arms are identical (0/560 everywhere), so no relabelling can flip the verdict. Stated plainly.
- Mutant B, a synthetic 120 s arm with 300/560 passes (a budget-shaped input, written under /tmp, not in the repo):
  verdict = `BUDGET` for both files. With the labels swapped the same input reads `INVERTED`, so the verdict flips and
  is not `BUDGET`. The verdict logic can therefore go red in both directions.

## Instrument notes (recorded, not scored)

- `t10_r2` `properties-value-001` returned `NO_REPORT` with 0/0 subtests (14130 ms). It is excluded from the verdict
  (total 0), and it is an instrument flake, not an engine result.
- The OK/TIMEOUT harness status flips between runs (`t120_r1` OK vs the other runs TIMEOUT) with pass counts unchanged.
  The status is noisy; the pass count is the stable quantity.

## Verdict for the node

Budget is ruled out as the `--timeout` cause: 0/560 at 10 s equals 0/560 at 120 s, in all runs. The css-transitions
0/560 pair is an engine-side stall in script execution. Next step: W6.J1.b.a (is the stall the engine's drain
budget?). No engine change in this node.
