# One hit-test, not two

Two implementations of one question — *what is on top at this point?* — and they disagreed on the
same page:

```text
                                     Chrome    a11y hit_test    elementFromPoint
  auto-z overlay over in-flow link     b1           b1 (t1465)        l1   ✗
  z-index:5 overlay                    b2           b2                l2   ✗
```

`A11yNode::hit_test` folded a per-node layer map. `doc_element_from_point` was a **flat scan over
the layout rects resolving by smallest area only** — it consulted `pointer-events` and SVG
paintedness but never a layer at all. So an overlay with an *explicit* `z-index: 5` took the click
in the agent's tree and did not in the web API.

This is the shape this repo keeps finding — t1402's activation behaviour, t1403's `<summary>`
toggle, t1356's CSSOM views: **the tests of each implementation are evidence about that one only.**

## The fix is a join, not a mirror

`manuk_css::stacking_layer` is now the single definition of the rule, folded down the ancestor chain
by both callers. `TOP_LAYER_Z` moved beside it, because `elementFromPoint` lives in `manuk_js` which
cannot see `manuk_page` — **a constant only one of two implementations can reach is how they drift
apart in the first place.**

```text
                                         Chrome     before      after
  l1  auto-z overlay over in-flow link    b1/b1      l1/l1      b1/b1
  l2  z-index:5 overlay                   b2/b2      l2/l2      b2/b2
  l3  z-index:-1 underlay                 l3/l3      l3/l3      l3/l3   ✓
  l4  no overlay             CONTROL      l4/l4      l4/l4      l4/l4   ✓
  l6  4-deep auto chain vs z-index:1      z1/z1      l6/l6      z1/z1
```

## The plural is half the point

`doc_elements_from_point`'s own doc comment states the contract: *"`elementsFromPoint(x,y)[0]` must
equal `elementFromPoint(x,y)`"*. Teaching only the singular about layers would have broken the
invariant the plural was written to hold. Every row above is measured as a **pair**, and both
functions now sort by layer, then area, then document order.

```text
  WPT css/cssom-view     729 failing -> 727   2 fixed / 0 new   (1314 -> 1316 of 2109)
      css/css-position   285 failing -> 285   0 fixed / 0 new
```

Both fixes are in `elementsFromPoint-simple.html` — *"elementsFromPoint for each corner of a simple
div"* and *"…of a div that has a margin"*.

## ⚠ The area name was wrong for two ticks

`manuk-wpt wpt cssom-view` reports **0 runnable testharness files**; the area is `css/cssom-view`.
t1457 and t1465 both recorded "not measurable" on that basis, and the agent's own geometry channel —
`getBoundingClientRect`, `scroll*`, `elementFromPoint` — went unmeasured because of a missing path
prefix. **A zero from an instrument is a claim about the instrument until the spelling is checked.**

See also [[a-banner-wins-the-click]], [[two-implementations-of-one-rule]].
