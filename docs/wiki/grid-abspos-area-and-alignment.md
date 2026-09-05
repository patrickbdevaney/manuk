# The grid area and the item's alignment are two answers, and neither alone is the containing block

**Tick 1447 — REFUSED.** `css/css-grid` failing 3405 → **3471 (+66)**: 39 fixed, 105 new. Reverted.
The product is the measurement.

## What CSS Grid §9.1 asks for

> *"If the grid container is the containing block of an absolutely-positioned child with a definite
> grid position, the containing block is that child's grid area."*

And the grid area is the **static position** too, so a child with all-`auto` insets sits at the area's
start corner.

## What is actually there — three layers, and each hid the next

```text
  400x200 grid, tracks 100px 50px / 70px 30px, abspos child at grid-column:1/2; grid-row:1/2
                                        Chrome            ours
  justify/align-content: start          0,0    100x70     0,0    100x70   ✓
  the second track                      100,70  50x30     100,70  50x30   ✓
  direction: rtl                        300,0  100x70     300,0  100x70   ✓
  justify/align-content: center         125,50 100x70     0,0    225x120  ✗
```

**1. Taffy's abspos grid area ignores content alignment.** It returns `loc 0,0` with the size
stretched to the *aligned far edge* — `225 = 125 + 100`, `120 = 50 + 70`. ⭐ **The far edge is right in
every row and only the near edge is wrong**, so the missing quantity is recoverable without patching
taffy (I2): solve the same probe tree a second time with the two content-alignment properties removed,
which yields the area's true SPAN, and take `near = far − span`. **Measured working** — the probe then
returns `125,50 100x70`.

**2. The probe tree never gets the RTL mirror the placed slots get.** taffy has no `direction`, so an
RTL grid's inline axis is mirrored on the way out of `layout_flex_or_grid`; `grid_area_containing_block`
is a *separate solve* that never passes through it. It answered x=0 for a first column that Chrome puts
at x=300.

**3. Neither was visible, because the element gets TWO boxes.** One from taffy's slot, one from the
abspos pass, and `node_rects` reports their **union** — so `0,0 225x120` is the near edge of the wrong
copy and the far edge of the right one. Fixing the containing block alone changes nothing.

## Why suppressing the duplicate is not the fix — and this is the refusal

Removing the taffy-slot box (and letting the corrected area serve as the static position) makes all
six probe rows Chrome-exact and reads **+66 failing** on `css/css-grid`:

```text
  FIXED  22  abspos/grid-positioned-items-content-alignment-rtl-001
         14  abspos/grid-positioned-items-content-alignment-001
  NEW    12  abspos/grid-positioned-items-content-alignment-001
          4  alignment/grid-{row,column}-axis-alignment-positioned-items-005/009/011  (and others)
```

> ⭐⭐⭐ **THE SLOT CARRIES THE ITEM'S ALIGNMENT AND THE ABSPOS PASS CARRIES THE AREA. Neither alone is
> the answer, and the code already said so** — the comment guarding that box reads *"the slot is not
> worthless, and deleting it outright was the WRONG fix — it is where the alignment lives"*, with a
> measurement behind it. This tick re-derived that the hard way and the ratchet refused it, which is
> the ratchet working: a trade of `justify-self`/`align-self` inside the area for the area's own
> origin is still a trade.

## The next tick, named

Correct the **slot** instead of suppressing it: apply the same `near = far − span` recovery and the RTL
mirror to the out-of-flow child's slot in `layout_flex_or_grid`, so the two boxes agree rather than one
being deleted. Then the union is harmless and the alignment survives. The `far − span` technique is
measured and working; what it needs is a second container solve at that seam, which
`solve_subtree` does not currently do — the same shape as its existing auto-margin re-solve.
