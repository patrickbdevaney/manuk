# W6.J1.b.a.a.a.b.a: what each getComputedStyle read costs in the properties-value-001 slice

chrome_ver=145.0.7632.116 (MANUK_CHROME_PIN). Default drain budget (no MANUK_MAX_DRAIN_MS, no MANUK_REFLOW_BUDGET_MS).
Raw: `raw/J1b-reads/`. Slice = `getPropertyTests().slice(0,50)` of css/css-transitions/properties-value-001.html (`n50-properties-value-001.html`).

Instrument: the existing `dom_bindings::REFLOW_COST` hook point plus the real-work point in `forced_reflow`, with a
temporary env-gated `eprintln` (`MANUK_REFLOW_TRACE`, patch in `raw/J1b-reads/trace-instrument.patch`; NOT committed, engine
reverted). `RFREAD` = one read reaching the hook (any read, incl. no-ops); `RFWORK` = a read that actually re-ran
cascade+layout. Run with `--show-failures` so the child's stderr is kept (otherwise it is nulled and the trace is empty).

## Result (n50-trace.out)

| quantity | value |
|---|---|
| reads reaching the forced-reflow hook | 3042 |
| real relayouts (FORCED-REFLOW-spanned) | 657 |
| reads answered idempotently (<=5 us) | 2383 (78%) |
| total time in forced reflow | 5.00 s (relayouts) |
| ms per relayout | mean 7.6, median 7.0, max 22.1 |
| document size at those relayouts | 642 -> 1412 nodes (cost grows as the cloned fixtures accumulate) |

`FORCED-REFLOW BUDGET EXHAUSTED` fires once (5 s default budget = drain budget), after which reads are answered from the
published layout, and one `Script terminated by timeout` follows. The slice scores 20/100 subtests.

## Answer

The stall is a per-mutation forced reflow, not a per-read cost: only the first read after a DOM/style mutation relays out
the whole document (~7.6 ms and growing with node count); repeat reads are ~0. 657 relayouts x ~7.6 ms exhausts the 5 s
budget inside the slice, which is why the 50-case slice (and the full 280-case file) cannot finish under the default budget.
The falsifier (zero relayouts during the stall) did not hold.

## Mutant / idempotence control (double-read.out)

`raw/J1b-reads/double-read-probe.html` appends a node, then reads `getPropertyValue('background-color')` twice, x3:
each mutation gives one `RFWORK` (1.0-1.4 ms at 21-23 nodes) and the second read `RFREAD us=0`. The instrument therefore
measures the read: it shows cost on the first read after a mutation and ~0 on the repeat; had the trace been inert
(no cost on first reads) the control would read all zeros.

## Caveat
Per-read timing is wall-clock under the full test run; one slice run, not a 3-run band. Ms/relayout is a single sample set (657 points).

## W6.J1.b.a.a.a.b.a.a: does the slice finish when the reflow budget is raised?

chrome_ver=145.0.7632.116 (MANUK_CHROME_PIN). Same n50 copy (`n50-properties-value-001.html`, run as `css/zzprobeN50/properties-value-001.html`
with its `support/` copied from the WPT tree, `manuk-wpt wpt css/zzprobeN50 --show-failures`). Raw: `raw/J1b-reads-budget/`.
Binary: `target/release/manuk-wpt` built from HEAD 2f1915ed, no engine change. ⚠ Host load average was **27 on 24 cores** during
these runs (another process tree was busy), so all wall-clock budgets were contended. See the caveat.

| arm | env | run 1 | run 2 | run 3 | script "terminated by timeout" | budget-exhausted warn |
|---|---|---|---|---|---|---|
| gate | `MANUK_REFLOW_BUDGET_MS=60000`, drain default | 40/100 | 40/100 | 40/100 | yes, each run | no |
| mutant | `MANUK_REFLOW_BUDGET_MS=1`, drain default | 0/100 | 0/100 | - | (budget warn fires, used_ms=33-35) | yes |
| default baseline | none | 9/100 | 10/100 | 10/100 | yes | yes |
| default baseline (repeat) | none | 0/100 | - | - | yes | yes |
| follow-up, not the gate | `MANUK_REFLOW_BUDGET_MS=60000` + `MANUK_MAX_DRAIN_MS=60000` | 41/100 | - | - | no | no |

Files: `budget60s-run{1,2,3}.out`, `mutant-budget1ms-run{1,2}.out`, `default-baseline-run{1..4}.out`, `followup-drain60s-run1.out`.
Timeouts: the gate arm has 1 `Script terminated by timeout` per run (in `__csAlias`, reached from `root.computedStyle` in the
test's `start`), so the slice does NOT finish under the gate arm. The gate arm lists 60 FAIL subtests in run 1 (of 100).

## Answer to the node question

- Raising only the reflow budget to 60 s gives **40/100 in 3 of 3 runs**, not a full finish: the 5 s drain budget (`MANUK_MAX_DRAIN_MS`
  default) still terminates the script. So the budget is one cut-off, not the only one.
- Raising both budgets to 60 s gives 41/100 with no timeout (1 run, follow-up only). The slice then finishes; the other 59 subtests fail
  on their own (events/values), not on time.
- The deeper failure is therefore a second cut-off, the drain/script deadline, which the reflow budget sits under.

## Gate verdict: NOT MET as written

- The mutant (budget 1 ms) does go red: the budget fires ("FORCED-REFLOW BUDGET EXHAUSTED") and the score falls to 0/100 versus 40/100.
  But it gives **0/100, not the 20/100 the gate names**, and the default-budget baseline reproduces **0-10/100 today, not 20/100**.
- Cause (measured): load average 27 on 24 cores while these ran. The 20/100 in the parent node was recorded under lower load. The
  per-relayout cost (`used_ms`=33-35 for one relayout at 644 nodes vs 7.6 ms mean in the parent trace) and the wall-clock budgets
  both move with load, so the absolute score is not comparable across these two sessions.
- Thresholds were not edited. The node is BLOCKED until a quiet-machine re-run (follow-up row) reproduces the 20/100 baseline and the mutant.

## Caveat
Scores are single-machine wall-clock under load; not a clean band. Do not diff these numbers against the parent's 20/100 (different load).
