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
