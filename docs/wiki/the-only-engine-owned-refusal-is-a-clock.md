# The only engine-owned refusal is a clock

*Tick 1499. `css-starved` is the compass's sole ENGINE term, and it is not a CSS bug.*

## Why this term

Two independent 200-site sweeps rank the refusals `ORIGIN 58/60 · METHOD 23/17 · ENGINE 6/5`. The
ENGINE column is `css-starved` (5/3) and `render-failed` (1/2) — and `css-starved` is ours by the
instrument's own words: *"our own `load_deadline` cut those sheets, so this is our bug."*

**Four of the five are one origin.** `trivago.be`, `.de`, `.fr`, `.jp`, `.pl` across two sweeps, plus
`nautica.com`.

## It is a performance bug wearing a CSS refusal's label

```text
  www.trivago.fr    load: manuk 61377ms · chromium 4020ms
```

Sixty-one seconds against Chrome's four. The 12-second load budget is exhausted, the stylesheet phase
is what it is exhausted *in*, and the row is filed `css-starved`. **The sheets are not failing; we
never get to them.**

```text
  cascade+layout+blocking scripts    8202 ms
  deferred scripts                  19610 ms   <- the largest
  subframes (pre-load)               7705 ms
  initial images+masks               4085 ms
  load event                           90 ms
  ─────────────────────────────────────────
  load_async                        41115 ms
  external CSS (finish_loading)      7763 ms   <- and the budget dies here
  TOTAL                             61377 ms
```

## The mechanism, and it is named in the engine's own diagnostic

```text
  SLOW RESTYLE+LAYOUT  cascade_ms=791  layout_ms=3226  container_query_ms=3395  cq_relaid=true  n_sheets=8
  SLOW FORCED REFLOW — one geometry read laid out the whole document  total_ms=7427
  FORCED-REFLOW BUDGET EXHAUSTED
```

**`container_query_ms` ≈ `layout_ms`.** Every layout on this page is run roughly twice: once, then
again for container queries, with `cq_relaid=true`. At 3.2 seconds a layout that is 3.4 seconds of pure
re-run, repeated — and a single geometry read then lays out the whole document for **7.4 seconds**,
which is what t1408's forced-reflow budget exists to bound and which duly reports EXHAUSTED.

## ⚠ And the cohort is not one thing either

```text
  www.trivago.*     layout_ms 3098   container_query_ms 3427   cq_relaid=true
  www.nautica.com   layout_ms   65   container_query_ms    0   cq_relaid=false
```

`nautica.com` is not container-query bound at all — its layouts are 65–484 ms. **Four of the five rows
have this mechanism and the fifth has a different one**, which is exactly the shape t1497 found in
`tree-divergence` and t1491 found in the near-bar font misses: *a cohort named by its symptom is
usually two mechanisms, and the aggregate hides the smaller one.*

## What this decides

The compass's only engine-owned term is **a clock, not a capability**. Nothing about CSS parsing,
fetching or cascading is wrong on these sites; the page is ten times too slow and the budget — which
exists to stop a dead subresource freezing a tab, and which `G_LOAD` gates — does exactly what it is
for.

**The ranked next step is container-query re-layout cost**, because it is 4 of the 5 rows and because
`cq_relaid=true` says the engine already knows when it is paying: a container-query pass that re-runs a
whole document layout is a bounded, measurable subsystem with an existing diagnostic and an existing
budget to gate it.

⚠ Not attempted here. This tick is the attribution, and the attribution is what says the work is a
performance subsystem rather than a CSS one.
