# `list-item` lays out as a block and must not report as one

*Tick 1488. A survey refused its own hypothesis and named the actual bug on the way past.*

## How it was found

The near-bar cohort — 28 of 200 corpus sites with shape between 55% and 75%, the cheapest sites to
push over the 0.75 bar — was surveyed rather than ground. `--shape-dump` across six of them, keyed on
the miss axis:

```text
  height 133 · y 91 · x 74 · width 18      -> 71% of near-bar shape misses are on the BLOCK axis
  the leaf tag of the height misses:  div 68 · img 7 · a 7 · section 2 · body 2
```

`div` names nothing — `div` is not a layout mode. The hypothesis was **wrong layout mode**: a flex
row falling back to a column stacks N items and is exactly N× too tall, which matches the worst
misses (a `<section>` 2630px tall against Chrome's 798; a child Chrome sizes at `7052px` wide against
our viewport-clamped 1200).

**The oracle already carried the answer and the dump discarded it.** Both `Seen` maps hold each
element's computed `display` — brick 4b put it there so the jarring invariants could read it — and
`--shape-dump` printed tag paths and rectangles only. One line of plumbing later:

```text
  1,352 shape misses across six near-bar sites · only 50 disagree about display at all
  and 40 of those 50 are ONE keyword:   list-item -> block
```

**The hypothesis is refused.** The near-bar shape gap is *inside agreed layout modes*: both engines
agree the box is a block and disagree about how tall it is. That is a much narrower place to look
next, and it cost one run.

## The bug the refusal named

`display: list-item` is block-level; the marker is generated elsewhere. So **both** cascades map the
keyword to `Display::Block` on purpose, and every layout path is correct as written. The collapse
then leaked into the *computed value*: `getComputedStyle(li).display` answered `block` where Chrome
answers `list-item` — on every `<li>` and every `<summary>` on the web.

⚠ **The rule this breaks was already written down, three arms away in the same `match`:**

> *"`getComputedStyle(el).display` must round-trip the specified keyword — a feature-detect that
> reads back `block` for `flow-root` concludes the value is unsupported and falls back to a clearfix
> or `overflow:hidden`, both of which have side effects the author avoided by choosing `flow-root`."*

Identical failure, one keyword over, live the whole time.

## Arbitrated against headless Chrome

```text
  li=list-item  over=block  dt=block  dd=block  sum=list-item  oli=list-item
  explicit=list-item  two=list-item
```

* **`dt`/`dd` are NOT list items.** The UA table had `"li" | "dd" | "dt"` on one arm, so the obvious
  fix makes all three `list-item` and is wrong for two of them.
* **`sum=list-item`** — `<summary>` is a list item in Chrome. Not obvious; measured, not assumed.
* **`over=block`** — an author's `display: block` on an `<li>` must stop reporting `list-item`. *A
  flag that is only ever set is a latch, and a latch on a cascaded property is wrong for every
  element that overrides it.*
* **`two=list-item`** — the two-value syntax `display: block flow list-item` normalises to the same
  keyword, so the flag is read from the NORMALISED value and not the raw declaration.

## A side flag, not a new `Display` variant

`legacy_webkit_box` beside it is the precedent. A new variant would force twenty layout sites to
handle a mode that behaves exactly like `Block`, and the first one that forgot would be a real
rendering regression bought for a serialization fix. The gate asserts the two `<li>` still stack, so
"this was a serialization fix and must not have moved a box" is checked and not asserted.

⚠ **Both cascades.** The MinimalCascade tracks the keyword beside the collapse; the Stylo path
recovers it exactly the way it already recovers `appearance`, `display_in_flow` and `counter_set` —
*"Stylo cannot answer it and the other cascade can."* A keyword one cascade knows and the other does
not is the two-cascades trap this file has been bitten by before.

## Result

```text
  WPT css/css-display   332 -> 353   (+21)
  WPT css/cssom        2868 -> 2894  (+26)   ⚠ and the ROW was spelled `cssom`; the directory is
                                              `css/cssom`, so the old figure was six audits stale
  HANG/CRASH 0 in both
```

Gated by `g_display_list_item_does_not_report_as_block` under five mutations, with **two** vacuity
arms — a positive row (`li=list-item`) because every `=block` row is satisfied by an engine that never
reports `list-item` at all, and a negative row (`dt=block`) because if everything reported
`list-item` the positive row would prove nothing.
