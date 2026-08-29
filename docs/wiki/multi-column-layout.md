# CSS multi-column layout — and the pref that made it look unimplemented

> Landed t1358. Gates: `multicol_balances_columns_the_way_chrome_does` (`engine/layout`),
> `multicol_longhands_survive_the_stylo_cascade` (`engine/css`). Every number below is
> headless-Chrome-measured on a 600px box at `16px/20px Arial`.

## The finding: it was switched off, not missing

`column-count` and `column-width` both carry `servo_pref = "layout.columns.enabled"` in Stylo's
`properties/longhands.toml`, and that pref defaults to **false** under the `servo` feature. So the
shipping cascade did not *ignore* `column-count: 3` — it **refused it at parse time**, and the
computed value came back `auto`, which is byte-identical to the answer a page that never asked for
columns gets. There is no way to tell those two apart by reading a computed style, which is why this
read as *"multicol is unimplemented"* for as long as it did.

The `columns` shorthand made it worse rather than better. It has no pref of its own, so it parses
happily — and then expands into the two longhands that were being refused. Both spellings therefore
produced the same nothing, from two different places.

⚠ **This is the third property found this way** (`layout.grid.enabled`, `layout.writing-mode.enabled`,
now `layout.columns.enabled`). The signature is always the same: *the computed value reads the
initial value on a page that plainly set it.* When a CSS feature looks absent, check
`longhands.toml` for a `servo_pref` **before** concluding anything about the layout engine.

## Priced first, on the corpus

Sampled 39 sites of `docs/bench/corpus-crux-trend.txt` (page + up to six of its stylesheets) and
grepped for real multicol declarations, excluding `auto`/`none` and excluding the `grid-template-`
prefixed spelling that a lazy regex catches:

    multicol (column-count / column-width / columns), count >= 2 ... 10 of 39 sites

Two of the ten use `columns: 1` as a reset. A three-column footer rendered as one tall column is a
whole-page `dy` error — every box below it moves — so this is a subsystem, not a decimal.

⚠ The candidate that was **refused** on the same evidence, in the same session: an inline box whose
fragments are all zero-width (`<span><br><br></span>`, where Chrome reports `0x16` and we reported
`605x62` and manufactured 28 false sibling overlaps). Real divergence, cleanly reproduced — and it
priced at **0 of 59** corpus pages. Price before building.

## The rules, as Chrome answers them

**Used count is a function of BOTH longhands and the available width** (Multicol §7.1) — the count is
a *maximum*, not a count:

| declared (in a 600px box) | used columns | column width | x positions |
|---|---|---|---|
| `column-count: 3` | 3 | 189.33 | 0, 205.33, 410.67 |
| `column-width: 180px` | 3 fit, **2 used** | 189.33 | 0, 205.33 |
| `columns: 2; column-gap: 40px` | 2 | 280 | 0, 320 |
| `column-count: 1` | — | ordinary flow, 600 wide | — |
| `display:flex; column-count:3` | — | flex items | — |

**`column-gap: normal` is `1em` here and `0` for flex/grid.** One property name, two readings, and
they disagree on the initial value. The keyword has to survive the cascade — resolving it to a
number at parse time loses the disagreement — so `ComputedStyle` carries `column_gap_normal`
alongside `column_gap`.

**Balancing is a search, not a division.** Four 20px children over three columns want 26.67px each;
no child fits twice; the naive fill leaves the fourth child nowhere to go. Chrome puts two in each of
two columns and leaves the third empty (height 40). The rule that reproduces every measured row:
*the candidate heights are the unit bottom edges, and the answer is the smallest one at which greedy
packing needs no more than `n` columns.*

**A wrapper's padding is not repeated per column.** `<ul style="padding:10px 0">` with four items in
two columns puts its `padding-top` above the first item of column one **only**, and its
`padding-bottom` below the last item of the last column:

    li y = 10, 30 | 0, 20        <- column two starts flush at the container top
    ul  600 x 50                 <- not 60; the padding counts once

## How it is implemented

The column pass is a **re-origining of already-laid-out content**, not a second layout. The content
is laid out once at the *column* width — so every line break, percentage and shrink-to-fit was
resolved against the containing block the spec names — and then the single stack is cut into `n`
pieces, each slid right by one column step and up to the container's top. Nothing is measured twice
and no box changes size.

**The lone-wrapper descent is the whole reason it moves anything.** `column-count` is almost never
put on the box that holds the items; it is put on a `<div>` or `<nav>` that holds a single `<ul>`,
and the items are the `<li>`s one level down. Fragmenting only the direct children finds ONE unit,
leaves it alone, and renders the commonest idiom on the web as one column. So a sole in-flow child is
descended through and then stretched back across every column, which is exactly the box Chrome
reports for it (`<ul>` at `600x60`, not `189x180`).

## What is NOT implemented, said out loud

**A block is never SPLIT across a column break.** Chrome fragments a box: `column-count: 3` over one
200px child gives three 67px fragments and a `600x67` bounding box. We cannot, and the honest
consequence is wired in rather than hidden — when the pass finds nothing it can cut, the caller
**re-lays the content out at the full width**, because leaving it at the column width would report
that box a third of its real width, which is *worse* than the single-column answer the engine gave
before columns existed. That row is therefore exactly the pre-tick behaviour: `600x200` against
Chrome's `600x67`, a height error and nothing else.

Also absent: `column-fill: auto`, `column-rule`, `column-span`, and `break-before`/`-after`/`-inside`.

## Receipts

Fidelity, same-hour old-binary control, live sites:

    www.crazyshop.pl        66.4% -> 75.4%   (+9.0 — CROSSES the 0.75 bar)
    ru.restaurantguru.com   69.7% -> 73.6%   (+3.9)
    www.repubblica.it       75.2% -> 76.6%   (+1.4, mean of two interleaved runs each)
    developers.google.com   61.0% -> 61.0%   CONTROL, flat
    serennu / patrickmorin / ikea / razaoautomovel      flat

⚠ **A live news homepage is not a frozen page.** A single before/after pair on `repubblica.it` read
**−5.0**, and the element dump showed *Chrome itself* had rendered a different footer between the two
runs (`996x667` at x=102 against `1200x1253` at x=0). Interleaving two runs of each binary in the
same minutes turned the −5.0 into a +1.4. One pair is not a measurement on a page that changes.

⚠ **No WPT number is available**: `css/css-multicol` is not in the sparse WPT checkout (18 `css/*`
directories present). Same shape as the missing `css/support/` at t1176 — recorded, not worked around.
