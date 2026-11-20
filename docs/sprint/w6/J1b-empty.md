# W6.J1.b.a.a.a.b: why getPropertyValue reads "" in the properties-value-001 harness path

chrome_ver=145.0.7632.116 (MANUK_CHROME_PIN). Measurement only; no engine/ or tests/wpt change.
Binary: `target/release/manuk-wpt`. Raw outputs and probe copies: `raw/J1b-empty/`.

## Answer

The empty string is not a harness-path effect and not a value-parse gap. It is caused by the drain-off arm itself:
`MANUK_MAX_DRAIN_MS=0` also zeroes the forced-reflow budget (`engine/page/src/lib.rs` `reflow_budget_us()` falls back to
`drain_budget_ms()`), so `forced_reflow` returns at once (`used >= budget` on the first read) and a computed-style read
is answered from the layout already published. For a freshly cloned fixture nothing has been published yet, so
`getPropertyValue` returns `""`. Under the default budget the same read lays out and returns the value.

Flip test: the same probe, the same binary, changing only the env var flips the result.

## Gate result (the gated copy, logged)

Copy: `raw/J1b-empty/logged-properties-value-001.html` (the WPT file with `DBG` records added in `values.start`
after `getStyle` and after `startTransition`; records capped at 12 because logging alone made the script time out).

| run | env | DBG A (after getStyle) for `background-color color(rgba)` | result |
|---|---|---|---|
| filtered to 1 case (`raw/J1b-empty/logged-n1-properties-value-001.html`) | default | from=`rgb(100, 100, 100)` to=`rgba(10, 10, 10, 0.4)` gpvDirect=`rgb(100, 100, 100)` | values subtest passes; events FAIL |
| same copy, 1 case | `MANUK_MAX_DRAIN_MS=0` | from=`""` to=`""` gpvDirect=`""` (x2 runs) | values FAIL `got disallowed value ""` |
| 2 cases | default | from=`rgb(100, 100, 100)` (`n2-default.out`) | 1/5 subtests |
| 2 cases | `MANUK_MAX_DRAIN_MS=0` | from=`""` (`n2-drain0.out`) | empty |
| full file (280 cases) | `MANUK_MAX_DRAIN_MS=0` | from=`""` to=`""` gpvDirect=`""` for the first 12 records, all `conn=true` (`full-file-drain0-dbg12.out`) | the 560 FAILs reproduce |
| full file (280 cases) | default | not observable: `Script terminated by timeout` inside `getComputedStyle` (`computedStyle` in `helper.js`) before any DBG summary; the 50-case slice also times out (`n50-default.out`) | NO_REPORT / TIMEOUT |

Full-file numbers are not re-counted here; the 0/560 counts are the drain-off and default counts already in
`J1b-drain.md` and `J1b-drain-assertions.md` (chrome_ver=145.0.7632.116).

## Control: plain-selector probe (mutant control)

`raw/J1b-empty/control-plain-selector-probe-002.html` is the W6.J1.b.a.a.a probe-002 (class-based rules in `<head>`,
no clone, no harness), with its `rt-*` and `control-animate` assertions reporting the read values.

- Default budget (`control-probe-002-default.out`): `rt-background-color` reads `from="rgb(255, 0, 0)"`,
  `t0="rgb(255, 0, 0)"`, `mid(1s)="rgb(252, 0, 3)"` (non-empty; transitionend=null). `rt-padding-left`, `rt-opacity` and
  `control-animate` (opacity 0.6) are non-empty too.
- `MANUK_MAX_DRAIN_MS=0` (`control-probe-002-last-run.out`): `rt-background-color from=""  t0=""  mid(1s)="rgb(0, 0, 255)"`,
  `rt-padding-left from=""`, `rt-opacity from=""`, `control-animate` `opacity=` (empty).

So the plain-selector control is non-empty under the default budget and empty under the drain-off arm, the same flip as the
harness copy. The gate's stated expectation (control non-empty) holds only under the default budget; that is the
discriminator, and the mutant (one env var) flips both paths together.

## Mechanism (by source inspection, consistent with the numbers)

- `engine/js/src/event_loop.rs` `max_drain_ms()`: `0` is "no clock bound".
- `engine/page/src/lib.rs` `reflow_budget_us()`: when `MANUK_REFLOW_BUDGET_MS` is unset it returns
  `drain_budget_ms() * 1000`, so the forced-reflow budget is also 0.
- `forced_reflow` (`lib.rs` ~2556): `if c.reflow_us_used >= c.reflow_us_budget` returns before laying out. The
  off-arm logs print `FORCED-REFLOW BUDGET EXHAUSTED` for this path. The read is then answered from the published layout,
  which for a node cloned in this script round has no entry, so `getPropertyValue` is `""`.

## Consequence for the earlier rows

- The 280 `got disallowed value ""` FAILs counted in `J1b-drain-assertions.md` (W6.J1.b.a.a) were measured in the
  drain-off arm. They are an artefact of `MANUK_MAX_DRAIN_MS=0` (zero forced-reflow budget), not evidence of a value-parse
  gap. The categorisation is still arithmetically right about the logs; its reading ("value-parse gap") is retracted.
- The W6.J1.b.a.a.a inference "the two gaps are independent" stands on its own probe-002 evidence (default budget): with
  the value read back non-empty, no transition starts and no transitionend fires.
- Under the default budget the real harness failure is the script stall (`Script terminated by timeout` in
  `getComputedStyle`, forced reflow of 700 to 10000 nodes per read). That is the W6.J1.b.a stall, not a value gap.

## Scope and caveats

- Single engine build, one chrome_ver, 1 to 12 cases examined in the debug copy. Not every property class was read under
  the default budget, because the default full run times out before the summary.
- The 12-record cap was needed because each extra `getComputedStyle` in the harness forces a reflow; the logged copy
  under the default budget timed out, so the default-budget value for the full file is still unmeasured.
- Probe copies were placed in `~/wpt/css/zzprobe*` and removed afterwards.
