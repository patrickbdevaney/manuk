# The flex cross axis is a LOGICAL question, and the predicate was asking a physical one

**Tick 1438.** `css/cssom-view` **+60** (1208 → 1268), `css/css-flexbox` **+3**, `css/css-grid` flat,
`css/css-sizing` flat.

## The defect

t1436 and t1437 both needed "is the y axis the flex CROSS axis?", and both wrote it as:

```rust
let main_is_y = row == s.writing_mode.is_vertical();
```

That is the correct expression for a **physical** question — it is what the scroll origin uses
(t1427), and the scroll origin really is physical. But every quantity this predicate is used with is
in the container's own **logical** space. An orthogonal subtree is laid out on swapped axes (t1347)
and mapped back to physical coordinates afterwards, so `Placed::slot` and the `cw`/`solved_h` a
container reports are logical.

Measured directly, a `vertical-lr` `row` flex box **80 wide and 120 tall** around a 300×50 item:

```text
  cw                  prints 120     ← the physical HEIGHT
  slot.x + slot.width prints  50     ← the physical height of the ITEM
```

> ⭐⭐⭐ **So `x` is the INLINE axis whatever the writing mode is, and a `row` flex's main axis is the
> inline axis by definition.** The predicate is `cross_is_y = (flex-direction is a row)`, full stop.
> Consulting the writing mode transposed the answer for *every* vertical writing mode.

That was **80 of the 90 rows** still failing in `cssom-view/scrollWidthHeight-negative-margin-002`
when t1437 landed.

## What made it hard to see, and it is the fixture again

`cssom-view/scrollWidthHeight-negative-margin-002`'s wrapper is **80×80**. On a square container the
two candidate cross sizes are equal, so a fix that picks the wrong axis still reads the right number —
and a fix that picks the right axis still reads the right number even where the *size* is wrong.

> ⚠⚠ **A SQUARE FIXTURE CANNOT TELL TWO AXES APART.** The gate's `b3` row is an 80×120 container for
> exactly that reason: the wrong axis reads −220 there instead of −180.

This is the fifth instrument of this shape the loop has paid for: `width:0` (t1424), a symmetric
`scale()` (t1426), a zero border (t1424), zero cross-axis margins (t1437), a square box (t1438).
**Every one of them is a fixture whose own symmetry hid the term under test.**

## The honest report that turned out to be right

`g_flex_stretch_can_shrink` (t1436) reported its mutation **M5 — replace `row == is_vertical()` with
`!row` — as GREEN**, saying plainly that its writing-mode row was *pinned* but not *discriminated*.
That report was correct, and it was correct in a stronger way than it knew: the expression was not
merely undiscriminated, it was **wrong**, and the discriminating fixture (an overflowing
`wrap-reverse` line) arrived one tick later.

*A gate that names what it cannot catch beats one that pretends — and this time the thing it could not
catch was a live bug.*

## Named residue — a NON-SQUARE orthogonal `row` container

With the axis right, `cross_size` is still read from `solved_h`, and for an orthogonal container that
is the CSS `height`: a physical length pinned as if it were the logical block size.

```text
                                             Chrome    ours
  80x120  vertical-lr, row, 300x50 item      -220,0    -180,0
  80x120  vertical-rl, row, 300x50 item       0,0      -40,0
```

`-180 = 120 - 300`, where the logical block size is the container's physical **width**, 80. That is
the orthogonal-root sizing seam (t1347), not this predicate, and it is invisible on a square
container.

## Gate

`engine/page/tests/g_flex_cross_axis_is_logical.rs` — 10 rows, 4 of them horizontal controls plus a
vertical row whose line FITS. Red under P1 (restore the physical expression → the vertical rows go
un-shifted, controls green) and P2 (invert to `!row` → the controls break instead). **The two failure
sets are disjoint, so no single-axis constant passes this fixture.**
