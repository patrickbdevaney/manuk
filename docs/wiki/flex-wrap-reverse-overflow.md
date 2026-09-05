# `wrap-reverse` packs overflowing lines from the wrong edge — and only the default `align-content`

**Tick 1437.** `css/cssom-view` **+30**, `css/css-flexbox` **+1**, `css/css-grid` flat,
`css/css-sizing` flat.

## The defect

With `flex-wrap: wrap-reverse` the cross axis is reversed, so the lines' start edge is the
container's **physical end**. When the lines together are larger than the container, CSS Box
Alignment §5.3 makes `stretch` (and `space-between`) behave as `flex-start` — which, reversed,
anchors the lines at that physical end and lets them overflow **backwards**. Taffy packs them
forwards from the physical start.

## What made this a SHIFT and not a MIRROR

The first instinct was a cross-axis mirror, because Chrome's `wrap-reverse` layout is exactly the
mirror of its `wrap` layout (`y' = cross − y − size`, verified on six rows). **Running our engine on
those same rows first is what stopped it:**

```text
                                             Chrome        ours (before)
  two lines that FIT, wrap-reverse           0,60 / 0,15   0,60 / 0,15   ✓
  the same two lines, wrap                   0,0  / 0,35   0,0  / 0,35   ✓
  column wrap-reverse, two lines             60,0 / 15,0   60,0 / 15,0   ✓
  align-content:flex-start, overflowing      0,-220        0,-220        ✓
  align-content:center / flex-end / around   exact         exact         ✓
  ONE line that OVERFLOWS                    0,-220        0,0           ✗
```

> ⭐⭐ **A general mirror would have broken every row that was already right.** Taffy's `wrap-reverse`
> handles line ORDER, the fitting cases and every explicit `align-content` correctly. The defect is
> *only* the overflow fallback of the DEFAULT value — so the fix is a bounded translation of the
> cross axis by the negative free space, not a re-implementation of the alignment.

## The two narrowings, and both were found by something other than the fixture

**1. The free space is measured on MARGIN boxes.** Flex lines are packed by margin box. Every
hand-written row had zero cross-axis margins, so border box and margin box agreed in all of them —
the first version was exact on all ten and read **80 subtests WORSE** on
`cssom-view/scrollWidthHeight-negative-margin-002`, whose item carries `margin: -100px`. *A fixture
with a zero in a term cannot see that term* — the fourth time this loop has paid for that shape
(`width:0` t1424, a symmetric `scale()` t1426, a zero border t1424).

**2. An out-of-flow child is not in a flex line.** Flexbox §4.1 takes an abspos child out of the
formatting context and leaves taffy's slot as its **static position** only, so the line-packing
overflow must not move it. Chrome keeps it at 70 while the in-flow item goes to −220; shifting it too
cost three subtests across `flex-abspos-staticpos-align-self-*`.

## Where the compensation lives, and why

Taffy is a sanctioned dependency and is **never patched** (CONSTITUTION I2). So this lands on the
placed slots on the way **out**, in `Ctx::shift_wrap_reverse_overflow` — the same seam and the same
reasoning as `mirror_rtl_inline`, which exists because taffy has no `direction` property either.

```rust
let cross_is_y = self.container_stretches_y(container);   // t1436's predicate, reused exactly
let max_end = in_flow_children.map(|p| p.cross_end + p.margin_end).max();
let shift = content_cross - max_end;      // the negative free space
if shift < 0.0 { /* translate every in-flow slot on the cross axis */ }
```

A line that FITS yields a non-negative shift and is excluded by arithmetic rather than by a second
predicate.

## The number

```text
                     base (t1436)   after      read by NAME
  css/cssom-view     1178/2109      1208/2109    +30, all in scrollWidthHeight-negative-margin-002
  css/css-flexbox    3209/4693      3210/4693    +1
  css/css-grid       failing 4183   4183         flat
  css/css-sizing     failing 1360   1360         flat
```

## Named residue

The same overflow in a **vertical writing mode** (`writing-mode: vertical-lr`, a `row` flex whose
cross axis is physical x) reads Chrome **−220** and ours **0** — before this tick and after it. The
predicate resolves the axis correctly; the slots in an orthogonal subtree are not in the space this
correction assumes.

## Gate

`engine/page/tests/g_flex_wrap_reverse_overflow.rs` — 15 rows, 7 of them controls. Red under all five
named mutations: N1 (no shift), N2 (border boxes → `m1` only), N3 (shift out-of-flow too → `ap` only),
N4 (no align-content filter → the three taffy already gets right), N5 (wrong axis → `c8` only).
