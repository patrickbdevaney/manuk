# The used end margin — the input three refusals were pointing at

> Landed t1432. Gate: `a_box_contributes_the_margin_it_actually_has_after_collapsing`
> (`engine/page/tests/g_used_end_margin.rs`), two rows, red under three mutations.
> `css/cssom-view` **1029 → 1074 (+45)**, `css/css-overflow` **587 → 588**.

## What was missing

```html
<div><div style="margin-bottom:50px">…</div></div>
```

The inner child's margin collapses through the auto-height wrapper, so the **wrapper's margin box ends
50px below its border box** while `getComputedStyle(wrapper).marginBottom` is `0px`. Anything that
compares a child against its parent's margin box from the STYLE map therefore reads the parent as
smaller than it is, and concludes the child escapes a box it is inside.

`manuk_layout::used_end_margins()` publishes the used value. **Layout knew it all along** — it is
`BlockResult::margin_bottom`, computed on every block box and thrown away.

## Why it matters: the refusal it unblocked

t1431 measured *"judge containment on the child's MARGIN box, not its border box"* at **90 failing
configurations → 45** on `cssom-view/scrollWidthHeight-overflow-visible-margin-collapsing`, and the
ratchet refused it because three banked gates went red — all on the wrapper shape above. With the used
margin published, the same comparison is correct and all three stay green.

```text
                                                            chrome sh/ch   before
  a 20px box at `margin:20px` inside a 20px-tall FLEX ITEM,
  in an `overflow: auto` flex container                        60 / 60     80 / 60
  an auto-height wrapper whose child carries margin 50           270         270   CONTROL
```

⚠ `overflow: auto` on the first row, not `visible`: under `visible` the t1431 collapsed-margin rule
already produces 60, so the row would pass with or without the fix. The discriminating configuration
was found by **diffing the 140-cell matrix under the mutation**, not by assuming the first fixture
written would fail. *A control that cannot fail is not a control* — the second time in two ticks.

## Two scopings, each with a number

* **Only for a box with height.** A zero-height box's `effective_mb` is its collapse-THROUGH value and
  contains its own TOP margin, which is already in its position — publishing it makes
  `g_scroll_overflow_end_margin`'s `c5` read 320 against Chrome's 270. *A start margin is already in
  the position; an end margin is not; a collapse-through box's used margin is both.*
* **Only in a horizontal writing mode.** `USED_END_MARGINS` is recorded in the TRANSPOSED space an
  orthogonal run is laid out in, so for a box inside a `writing-mode: vertical-*` run the engine's
  "block end" is physically HORIZONTAL. Feeding it into the physical bottom margin is the same class
  of mistake t1426 fixed in `transform`. Priced: `css/css-overflow` reads **586 without the scoping
  and 588 with it**, same binary both ways — a net −1 that the failing-NAME diff (6 new, 5 fixed)
  separated from the containment change measured in the same tick.

> ⭐⭐ **A value computed in transposed space is not physical**, and this is the second mechanism in
> one session to be caught by that rule. It is now worth checking of any layout value a consumer
> reads as physical.
