# `load` fires after the webfont arrives

*Tick 1490. Every `window.onload` handler on the web measured text in a font the page does not use.*

## The gap

`load_async`'s pre-`load` block waits for subframes, images and masks, and each of those waits carries
a comment saying why it had to exist:

* a handler that reaches into a not-yet-loaded frame **throws** — it cost the entire 767k-subtest
  `encoding` suite;
* one that measures an undecoded image gets `naturalWidth === 0` — it cost
  `grid-minimum-size-grid-items-021` exactly half its subtests, every WIDTH assertion passing and
  every HEIGHT one failing.

**The stylesheet phase was not in that list**, and it is where `@font-face` is fetched and where an
arriving face triggers the relayout. So `load` fired on a document laid out in the FALLBACK face, and
`__fireLoad`'s once-only guard — *"`load` fires exactly once, ever"*, which is correct — then made the
later, correct dispatch a no-op.

## Arbitrated against headless Chrome

Ahem defines every glyph as exactly one em, so five characters at 32px is **exactly 160px**. A
fallback cannot coincidentally produce that number.

```text
                   DOMContentLoaded    load     a later task
  Chrome                  96            160         160
  before                  96             96          96
  after                   96            160         160
```

⚠⚠ **The layout was right the whole time.** `root_box` measured 160 before this fix; only the geometry
JS could see was stale, and a host re-entry one call later read 160 correctly. **No rendering test
could catch this** — the same shape as t1479's `document.styleSheets`, where the effect was right and
the description of it was wrong.

⚠ `dcl:96` is part of the assertion, not noise. Chrome fires `DOMContentLoaded` *before* the font has
arrived and this engine must too. A fix that merely loaded fonts earlier would make DCL read 160 and
be differently wrong; **the ordering is the claim**.

## ⚠⚠ The unguarded fix was a Bar 0, and the control is what caught it

Waiting for the stylesheet phase unconditionally took `wpt html/semantics` from **HANG/CRASH 0 to 1**,
on `tabular-data/processing-model-1/span-limits.html`. A same-hour old-binary control confirmed the
attribution: clean without the change, hanging with it.

That file is bare markup with `colspan=1000` cells — **no `<style>`, no `<link>`, no `@font-face`**.
The pass could not change one pixel of it and charged it a second full relayout of the expensive kind.
*"Idempotent for FETCHING" is not "free to call twice"*: `fetch_and_apply_stylesheets`'s relayout
branch also fires on `has_dirty()`, which the harness's own scripts make true.

The guard is the **precondition**, not a heuristic: a document with no stylesheet source has no
`@font-face` to arrive and therefore nothing for `load` to wait on. After it: `html/semantics`
6297/11635, HANG/CRASH 0 — the banked mark exactly.

⚠ There is no third case. A page whose only `@font-face` lives in an external sheet still passes the
guard, because it has a `<link>`. A page with neither has no font.

## ⚠⚠ AND SO WAS THE UNBOUNDED ONE — a SECOND Bar 0, caught by a different gate

With the guard in place, `G_LOAD` — *"the page renders when its subresources never answer"* — failed
at **13.6s against a 2s budget**. A phase added to the pre-`load` path sits outside
`finish_loading`'s `timeout(budget, …)` wrapper and answers to nothing on its own.

⚠ The comment written with the first version asserted the opposite — *"it runs under the same
`load_budget()` the block already spends"* — and was simply **wrong**. *A comment is a checkable
claim that dies silently* (t1303); here the gate checked it within the hour.

Bounding it against `nav_started + budget` got 13.6s → **5.41s**, against a 4s ceiling: bounded, and
still two budgets, because `finish_loading` starts a fresh one afterwards. The fix is the bound the
early-CSS block ninety lines above already uses — **a quarter of the budget**, never all of it:

```text
  unbounded                    13.6s
  + nav-budget bound            5.41s     still over the 2x ceiling
  + quarter-budget cap          4.20s     green
```

**Two Bar 0s in one tick, from two different gates, on two different mechanisms — and both were the
cost of the wait rather than the wait itself.**

## How it was found

Three ticks of narrowing, each refusing the previous hypothesis:

```text
  t1488  71% of near-bar shape misses are on the block axis; leaf tag `div` names nothing
         -> printing the `display` the oracle already carried refuted "wrong layout mode"
            (50 of 1,352 misses disagree) and named `list-item -> block`
  t1489  48% of misses are same-family, same-size, different ADVANCE
         -> a controlled fixture refuted "our metrics are wrong" (155 vs Chrome 154)
         -> a corrected counter refuted "the webfonts do not arrive" (14/15, 20/20, 7/7)
  t1490  -> the layout was right and JS could not see it
```

## The gate

`g_load_fires_after_the_webfont_arrives` — the Chrome row exactly, with `m=96` (the monospace control)
as the vacuity arm, plus **the guard's own two arms**: this fixture must reach the pass, and bare
markup must not. Red under two mutations, one of which is the Bar 0 shape itself.
