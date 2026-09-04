# The scrollable overflow region has TWO terms, and `width:0` hides one of them

> Landed t1424. Gate: `a_negative_end_margin_does_not_shrink_a_box_that_has_area`
> (`engine/page/tests/g_scroll_overflow_empty_box.rs`), nine rows, headless-Chrome-145-measured,
> proven red under three mutations. Closes the arc t1420–t1423 opened.

## What five ticks were fitting a rule to

t1420, t1421, t1422 and t1423 each produced a candidate rule for the scrollable-overflow end-margin
term, each fitted every fixture in the tree except one, and each was **reverted**. Three reverts, one
line of arithmetic, five rules. The fixture that refuted every border-box floor was `d7` in
`agent/tests/g_scroll_overflow_end_margin.rs`:

```html
<div style="width:100px;height:100px;padding:10px 5px;overflow:scroll">
  <div style="width:0;height:200px;margin-bottom:-30px"></div>
</div>
<!-- Chrome: scrollHeight 190. A border-box floor gives 210. -->
```

⭐⭐⭐ **The child is `width:0`, and that is the entire difference.** Re-measured with the width as
the only variable and nothing else changed:

```text
                                                     chrome   ours(before)
  width:0    height:200  margin-bottom:-30px           190        190   ← d7. A CORNER.
  width:1px  same                                      210        190
  width:50px same                                      210        190
  width:50px same, container padding:0                 200        170
```

**Every row of the battery that arbitrated this rule uses `width:0`** — `c1`, `c2`, `d7`, `f1`, all of
them — because a zero-width box is the tidy way to write *"a 200px-tall thing"* in a fixture. The
battery selected a code path Chrome treats specially, and then five ticks of rules were fitted to it.

> ⭐⭐⭐ **A fixture that zeroes a dimension to keep itself simple is not a simpler case of the general
> one. It is a different case.** And a refuting fixture is an *instrument*: when one fixture refuses
> what every other fixture accepts, re-measure THAT fixture before believing it.

## The rule, and it is two terms

For each box in the scroll container's subtree, measured from the container's border-box origin:

```text
  IF the box is CONTAINED   →  its MARGIN box, inflated by the container's end padding
  IF the box is NON-EMPTY   →  its BORDER box, with NO padding
  scrollHeight = max(clientHeight, the union of all of those)
```

*Contained* is t1418's rule: the box's border box does not spill past its parent's **margin** box, all
the way up. It is what makes the first term *"the flow"* — a box that has escaped its ancestors is not
part of the container's in-flow content, so it contributes its painted box and nothing else: no
margin, no padding. *Non-empty* is Blink unioning rectangles, where a rect with a zero dimension is a
no-op.

Validated against **72 Chrome cells** — `{no wrapper, wrapper == content, wrapper smaller than
content} × margin {-5,0,+5} × padding {0,10} × height {auto,fixed} × content {inside, past}`, the
matrix t1423 asked for with the DOM-shape axis it had held fixed — plus every row of all four banked
scroll gates. **72/72, one expression.**

## Two rules it retires

**t1417's negative-margin subtree clamp is superseded.** Containment already does its job, and does it
without bounding a subtree by a box it is not inside:

```text
  a `width:60px;padding:10px;overflow:hidden` container       chrome   with the clamp
  > `height:5px;margin-bottom:-5px` wrapper > 100px box         110         20
  the same wrapper AUTO-height, margin -5px                     115        115   CONTROL
  the same with margin +5px                                     125        125   CONTROL
```

**And the nested-propagation gap named as residue in
[scrollable-overflow-end-margin](scrollable-overflow-end-margin.md) closes for free** — it was the
second term leaking. A `height:0` wrapper holding a 200px box:

```text
                                     chrome   before   after
  the inner box is width:0             120      220     120   ← contributes NOTHING
  the inner box is width:5px           210      220     210   ← its border box, unpadded
```

## How it goes red

- **N1** — drop the border-box term (the pre-tick state): `z1` 190, `zw` 190, `zp0` 170. Every
  control stays green, which is what identifies the mechanism as the border box and not the margin.
- **N2** — drop the emptiness test: `z0` 210 and `zp0w0` 200 against Chrome's 190/170 — and this is
  the mutation that re-breaks `d7`, which is why that gate is the second half of this one.
- **N3** — restore t1417's clamp: `esc` 20 against Chrome's 110.
