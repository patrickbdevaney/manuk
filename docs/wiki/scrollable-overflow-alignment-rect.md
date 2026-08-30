# The alignment rectangle — a relatively-positioned box contributes BOTH positions

> Landed t1382. Gate: `a_relative_box_contributes_its_in_flow_position_too`
> (`agent/tests/g_scroll_overflow_alignment_rect.rs`), twelve rows, headless-Chrome-measured.
> WPT `css/css-overflow` 508 → 513. Closes the residual t1381 named.

## Two rules that only look like one

1. A relatively-positioned box contributes its **alignment rectangle** — the position it occupies in
   the FLOW, before the offset — as well as the position it was painted at. That is why
   `top: -1000px` does not shrink the scroller the box lives in.
2. **Only the in-flow rectangle is inflated by the container's own END PADDING.** The offset
   rectangle is added raw.

Chrome-measured (`--hide-scrollbars`, a `width:100px; height:100px; padding:10px 5px;
overflow:scroll` container around a `10px × 200px` child):

```text
                                      chrome   before   after
  no offset                 CONTROL     220      220      220     10 + 200 + 10
  top:   50px                           260      270      260     10 +  50 + 200 + 0
  top: 1000px                          1210     1220     1210     10 +1000 + 200 + 0
  top: -1000px                          220      105      220     the IN-FLOW rect, padded
```

⭐⭐⭐ **The `+10` in row 1 and its absence in rows 2 and 3 are the same padding.** Before this tick
the container's end padding was added ONCE, to the finished extent — so it was applied to whichever
rectangle happened to win. Rows 2 and 3 say it belongs to a *contribution*, not to the total, and
they were wrong by exactly `padding-bottom` in a way that reads as a rounding error until the offset
is made large.

⭐ **Row 4 is the family t1381 priced at 88 WPT subtests.** The painted rectangle is 790px above the
scroll origin and contributes nothing at all, so without the alignment rectangle the container
reports its own padding box and the 200px child may as well not exist.

## Why the in-flow position had to be recorded

`layout_block` applies the offset with `boxx.translate(dx, dy)`, which **overwrites** the in-flow
position — nothing in the fragment tree remembers it. `manuk_layout::relative_offsets()` is a
thread-local written at the two `Position::Relative` sites (the block path and the float path) and
published wholesale at the end of `layout_document`, exactly as `grid_tracks()` is, so a box that
stops being relatively positioned stops having an offset. It is skipped inside an `intrinsic_probe`
— the t1120 rule its neighbours already carry.

The three facts a box contributes (`end_margin`, `end_padding`, `relative_offset`) became the
`OverflowContribution` struct rather than a positional triple: two of them are lengths in the same
units and the third is a position delta, which is exactly the shape a later reader gets wrong
silently. (Same call as t1365's `NameCtx` and t1379's `NameStyle`.)

## The battery

```text
                                                         chrome   before   after
  g1  WPT's own shape: top:-1000px + margin-bottom:50      270      105      270
  g2  …the inline axis: left:-1000px + margin-right:50     260       95      260   (scrollW)
  h1  top: 1000px                                         1210     1220     1210
  h2  top: -1000px                                         220      105      220
  h3  left: 1000px                                        1205     1210     1205   (scrollW)
  h4  top: 50px                                            260      270      260
  c1  no offset                                CONTROL     220      220      220
  c2  no offset, margin-bottom: 50px           CONTROL     270      270      270
  c3  no offset, margin-right: 50px            CONTROL     260      260      260   (scrollW)
  d7  no offset, margin-bottom: -30px          CONTROL     190      190      190
  e5  no offset, two margined children         CONTROL     340      340      340
  f1  no offset, nested margin                 CONTROL     270      270      270
```

⚠ `g1`/`g2` keep WPT's own spelling, down to the `width: 0`, so this gate and
`scrollable-overflow-padding.html`'s 30 subtests are testing the same thing. `h1`–`h4` use a `10px`
width because a zero-width box is a degenerate case in Blink's propagation (measured and named in
`docs/wiki/scrollable-overflow-end-margin.md`), and the positive-offset rows must not be measured
through it.

⚠ The six CONTROL rows are t1381's whole battery re-asserted: moving the end padding from the
finished extent into each contribution is exactly the kind of change that is right on the new rows
and off-by-a-padding on the old ones.

## The receipt, and the honest size of it

WPT `css/css-overflow` moved **508 → 513, +5** — not the 88 the family was priced at. The two files
are `checkLayout` batteries whose subtests each depend on several rules at once (writing modes,
transforms, collapsed margins contributing to the alignment rectangle), so the offset rule alone
flips only the rows that need nothing else. The Chrome battery above is the measurement that says
the rule is right; the suite number says how much of those two files it was sufficient for.

## How it was proven red

- **N1** — `relative_offsets()` returns an empty map (the pre-tick state): the VACUITY assert fires
  first, and honestly — the recorded offset and the rule share a source, so N1 proves the fixture
  reaches the recorder and N2 proves the rule.
- **N2** — drop the in-flow arm: `g1`, `g2`, `h2` read 105 / 95 / 105. The three negative-offset
  rows and nothing else.
- **N3** — pad BOTH rectangles (the pre-tick "add the padding once, at the end"): `h1`, `h3`, `h4`
  read one end-padding too large; every negative-offset row and every control stays green.

## Related

- `docs/wiki/scrollable-overflow-end-margin.md` — t1381, the other half of the same 117 subtests,
  and the survey that refused the rest of the area.
