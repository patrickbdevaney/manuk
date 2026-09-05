# `flex-direction: column` never shrank an item — one word, one axis

**Tick 1435.** `css/css-flexbox` +20, `css/cssom-view` +60, `css/css-sizing` +4, `css/css-grid` flat.

## The defect

`Ctx::extract_placed` (`engine/layout/src/lib.rs`) turns taffy's solved slots back into Manuk boxes.
For the inline axis it records taffy's verdict **unconditionally**:

```rust
self.taffy_item_width.borrow_mut().insert(p.dom, p.slot.width);
```

For the block axis it recorded it **only for a percentage**:

```rust
let pct_h = matches!(self.style_of(p.dom).height, Dim::Percent(_) | Dim::Calc { .. });
if pct_h { self.taffy_item_height.borrow_mut().insert(p.dom, p.slot.height); }
```

The reasoning behind that narrowness is right about `auto` — for an `auto`-height item taffy's slot
is a **stretch verdict**, not a resolution — and wrong about a **length**. A `height: 300px` item in
a `height: 80px` container has been *shrunk* by taffy (`flex-shrink` is `1` by default and negative
free space is exactly what it is for), and dropping that verdict re-resolved the item at its own
300px.

> ⭐ **The main axis is the WIDTH in a `row` container and the HEIGHT in a `column` one.** So one
> word in one condition discarded exactly one direction's `flex-shrink` and nothing else — and every
> `row` fixture in the suite passed before the fix and after it. That is what kept a defect this size
> invisible for as long as the field has existed.

## The measurement that localised it

Taffy was never wrong. Printing the slots for `.b { width:80px; height:80px; display:flex }` around
one child:

```text
  flex-direction:column, child height:300px         slot 50x80    box 50x300
  flex-direction:column, child height:300px m:-100  slot 50x280   box 50x300
  flex-direction:row,    child width:300px          slot 80x50    box 80x50   ✓
```

⭐ **The slot's width was used and the slot's height was thrown away, in the same function, three
lines apart.** A defect whose symptom is "flexbox is wrong" and whose cause is one `matches!` arm.

## The fix

```rust
let definite_h = matches!(
    self.style_of(p.dom).height,
    Dim::Px(_) | Dim::Percent(_) | Dim::Calc { .. }
);
```

`Dim::Auto` stays out, deliberately, and `height: stretch` shares its representation and is excluded
with it.

## The guard that was measured and NOT shipped

The obvious worry is a **grid** item: a definite-height grid item should overflow a shorter track,
not be clamped to it, so the first version carried a `parent_is_flex` scope. It was measured rather
than assumed:

```text
  grid-template-rows: 80px, item height:300px   taffy's slot height = 300   (NOT the 80px track)
  css/css-grid failing subtests   with the scope 4245   without it 4245   (643 files)
```

⭐⭐ **Taffy does not clamp a definite-height grid item to its track, so the scope was inert.** One
rule, both formatting contexts; the guard is not in the tree and `grd=50x300` in the gate is why.
*An inert guard is not free — it is a claim the next reader has to re-derive.*

## Named residue — the `auto` half, with its fixture

Widening the condition to include `Dim::Auto` changes none of the gate's rows, so the gate cannot
catch it and says so. The `auto` case is a **separate open defect**, measured here:

```text
  Chrome-measured, same 80x80 box                        Chrome    ours
  s1  row flex, item height:auto, 200px content            80       200     ← align-items:stretch
  s6  grid, 80px row track, item auto, 200px content       80       200
  s4  COLUMN flex, item auto, 200px content   CONTROL     200       200  ✓
  s5  row flex, align-items:flex-start, 200px CONTROL     200       200  ✓
```

`stretch` sets an auto cross size to the **line's** cross size and lets the content overflow. We keep
the content height, because the adoption at the end of `extract_placed` is written `slot > box` and
can therefore only ever *grow* a box. One axis, one comparison — and s1/s4/s5/s6 is the fixture.

## Gate

`engine/page/tests/g_flex_column_shrinks_its_item.rs` — 7 rows (4 of them controls/regression arms),
red under N1 (the pre-tick condition) and N3 (dropping the percentage arm, which reads `pct=50x20` =
0.5² × 80, the squaring this project fixed on the width axis at tick 14). N2 is reported GREEN.
