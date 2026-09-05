# `min-width` never reached the intrinsic contribution — the clamp had only its upper half

**Tick 1441.** `css/cssom-view` **+16** (all of it in
`scrollWidthHeight-overflow-visible-margin-collapsing`, 95/140 → 111/140); `css/css-sizing`,
`css/css-flexbox`, `css/css-grid`, `css/css-text` and `css/CSS2` all flat.

## The defect

CSS Sizing §5.1: the min-content and max-content contributions of a box are its outer size **clamped
by its min and max sizes**. Ours had only the upper half — `max-width` reached the used width and
`min-width` reached nothing, so a block child declaring `min-width` and holding no content
contributed **zero** to every shrink-to-fit ancestor.

```text
                                                        Chrome    before    after
  inline-block > div{min-width:20px}                       20        0        20
  inline-block > div{min-width:20px; margin:0 10px}        40       10        40
  float        > div{min-width:20px}                       20        0        20
  abspos       > div{min-width:20px}                       20        0        20
  flex item    > div{min-width:20px}                       20        0        20
  table cell   > div{min-width:20px}                       26        6        26
  inline-block > span[inline-block]{min-width}   CONTROL   20       20        20  ✓
  inline-block > div{width:20px; max-width:5px}  CONTROL    5        5         5  ✓
```

> ⭐⭐ **The two controls are what say this is the missing HALF of a clamp rather than a missing
> clamp.** An INLINE-level child already carried its `min-width` into the line box, and `max-width`
> already reached the used width — so only the block child's lower bound was unrepresented, and it was
> unrepresented in every context that asks for an intrinsic width at once.

## Where it lived

`content_right_extent` lays a subtree out at a 1e6 available width and **discards a block box's own
`rect.width`** as an artifact of the measuring width (it is ~1e6 and meaningless), recursing to the
inline text that carries the real extent.

> ⭐ **A declared `min-width` is the one part of that width that was never a function of the measuring
> width**, and it was going out with the artifact.

So the FILLED branch gains a floor — and, deliberately, does **not** `return`: the box's content may
still exceed the floor, and the walk below is what finds it.

## Three details the fixture had to carry

* **It is a floor, not an answer.** `min-width:20px` around an 8-character word is **77.0625** in
  Chrome; applying the floor and returning reads 20.
* **`content_right_extent` measures BORDER boxes.** A content-box `min-width` gains the frame the used
  width would give it; a border-box one must not. Without both rows, either convention passes.
* **Only `Dim::Px`.** A percentage `min-width` resolves against a containing block this measurement
  does not have, and guessing a basis is worse than declining.

## Gate

`engine/page/tests/g_min_width_floors_intrinsic.rs` — 12 rows, 3 controls. Red under S1 (drop the
floor → nine rows collapse, the three controls hold), S2 (skip the box-sizing frame → the content-box
row alone) and S3 (`return` after the floor → the content-wider-than-floor row alone).
