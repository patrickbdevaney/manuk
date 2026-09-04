# The unreachable scrollable overflow region follows the scroll origin

> Landed t1427. Gate: `the_unreachable_side_of_the_scrolling_area_follows_the_scroll_origin`
> (`engine/page/tests/g_unreachable_scrollable_overflow.rs`), six Chrome rows, red under four
> mutations.

## The rule

A scroll container can only be scrolled **away** from its scroll origin, so overflow on the origin
side cannot be reached and is not in `scrollWidth`/`scrollHeight` (CSS Overflow 3
§unreachable-scrollable-overflow-region). Our extent expressed that as `.max(0.0)` on the maxima —
which is the origin at the **top-left**, and nothing else.

```text
  x origin at the END edge   when  (horizontal mode AND rtl)  OR  writing-mode: vertical-rl
  y origin at the END edge   when  (vertical mode AND rtl)

  origin at the START edge:  scroll extent = max(padding box, END overflow)
  origin at the END edge:    scroll extent = padding box + |START overflow|
```

## The measurement, and why it is a scrolling-area rule and not a layout one

Headless Chrome 145, `100x200; overflow:scroll; scrollbar-width:none` around a `100x200` child at
`transform: translate(-3px,-6px) scale(1.10)`. ⭐ **The child's physical rect is identical in all six
rows** — `[-8,-16,102,204]` — so nothing about layout varies:

```text
                         chrome sw / sh   before      origin
  ltr  horizontal-tb        102 / 204    102 / 204    top-left      CONTROL
  ltr  vertical-lr          102 / 204    102 / 204    top-left      CONTROL
  ltr  vertical-rl          108 / 204    102 / 204    top-RIGHT
  rtl  horizontal-tb        108 / 204    102 / 204    top-RIGHT
  rtl  vertical-lr          102 / 216    102 / 204    BOTTOM-left
  rtl  vertical-rl          108 / 216    102 / 204    BOTTOM-RIGHT
```

`108 = 100 + |−8|`, `216 = 200 + |−16|`. Note the two routes to the same 108: `vertical-rl` moves the
origin by the **block** axis and `rtl` moves it by the **inline** axis, which is why the rule is
keyed on the two directions and not on `writing-mode` alone. The `ltr vertical-lr` control is what
costs a *"vertical means mirrored"* shortcut its pass.

⚠ `scrollbar-width: none` on the fixture on purpose: this engine reserves a gutter and Chrome was
measured with `--hide-scrollbars`, so a fixture with a gutter compares two scrollbar policies rather
than two scrolling areas.

⚠ **Named residue:** the scroll-offset add-back uses `.abs()` on a flipped axis. The sign convention
for `scrollLeft` in `rtl` is its own question and every row above is measured unscrolled, where the
term is zero either way.

## What it unblocked, with the number

t1425 refused to widen `scrollWidth`/`scrollHeight` to every element because the forced reflow that
the widening requires turned **40 `css/css-overflow` subtests red** — every one a `writing-mode:
vertical-*` row of `scrollable-overflow-transform-unreachable-region`. Those 40 were passing on
**stale reads**; forcing the reflow exposed two real defects, and t1426 and t1427 are those two.

Probed with the forced reflow re-applied on top of this tick and then reverted:

```text
  css/css-overflow, forced reflow, at t1425   505 / 963   (−12)
  css/css-overflow, forced reflow, at t1427   558 / 963   (+41)
```

A **53-subtest swing**, and it is the proof that the ratchet's refusal at t1425 was reading a real
signal rather than a flaky one. The widening is now unblocked and priced — its gate, fixture and four
mutations are written verbatim in
[the-scrolling-area-of-every-element](the-scrolling-area-of-every-element.md).
