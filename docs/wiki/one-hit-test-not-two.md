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

## Overflow clipping (RG.S1.a)

`A11yNode::hit_test`/`landing` take a per-node `clip` (the intersection of every `overflow != visible`
ancestor's padding box, `Page::clip_map_for(true)`, which skips `html`/`body` because their overflow
is the viewport's). Content a clip removes is not painted, so it cannot occlude: a carousel's
positioned `overflow:hidden` track whose slides overflow it no longer reads OCCLUDED over the header
buttons above it (Dashdish: 6 filter pills and every "See all"). Gate:
`agent/tests/g_overflow_clipped_content_does_not_occlude.rs`. A target WHOLLY outside its own clip
(an option scrolled out of its listbox) is not refused: a click scrolls the container to it first, as
Playwright does, so it is judged unclipped. The first cut refused it and opendining-4 went 0 -> 6-16
UNREACHABLE refusals per run. Known remaining gap: nested
`z-index:auto` positioning still inflates the layer by depth (`parent + 1`) where Chrome paints
those in tree order within one stacking context.

## RG.S2: `z-index: auto` is flat, and "positioned" is a fact the hit-test needs

`stacking_layer` used to give a nested `position: relative; z-index: auto` box `parent + 1`, one layer per wrapper. A carousel track two wrappers deep then outranked its own later, absolutely positioned arrow (Chrome's `elementFromPoint` returns the arrow), so the agent saw `OCCLUDED by list` and a modal overlay with no close control. `auto` now stays on its stacking context's base layer. A flat layer needs a tie-break, and ranking *every* later box on a layer made G6 clickability 84.2% (60 Wikipedia links) because in-flow content inherits its positioned ancestor's layer. So `A11yNode.positioned` marks boxes that paint in the positioned pass: a positioned candidate beats an in-flow one on the same layer, two positioned ones paint in tree order, two in-flow ones keep smaller-box-wins. A target wholly outside its own overflow clip is `Clear` without a hit-test, since scrolling it into view moves it.

## RG.E2.b: an act scrolls a target to the middle of the viewport, not under the sticky header

`Landing::OffScreen { dy }` asked for `dy = bbox.y - viewport.y`, the target's top edge at the viewport's top edge. staynb's stay page has a `position: sticky; z-index: 99` header, and its sticky booking panel's "Reserve stay" button sat at viewport y=56 after the scroll, under that header's guest picker: the click was refused as OCCLUDED, or landed on the picker and did nothing ("0 click listeners"). Chrome opens `/book` for the same click because Playwright's scrollIntoViewIfNeeded centres. `landing` now centres a target that fits the viewport and keeps the top edge for a taller one. Fixture: `agent/tests/g_scroll_centres_under_sticky_header.rs` (red when top-aligned).

Same node, second cause: the Reserve click then reached the button but `AgentBrowser::activate` dropped the handler's `location.href = "/book?..."` (Next's router hard-navigates). `Page::take_script_navigation` recorded it and only the GUI shell performed it. `activate` now navigates to it (`Activation::Navigated`), so manuk and Chrome both end on /book. Fixture: `agent/tests/g_agent_follows_script_navigation.rs`.

## RG.2.E1: an explicit `z-index` inside a stacking context is relative to it

`stacking_layer` gave every explicit `z-index: n` the absolute layer `n * 1024 + 1`. A MUI radio is an `opacity: 0; position: absolute; z-index: 1` input over its icon span, and the whole dialog is `position: fixed; z-index: 1300` (layer 1331201), so the icon, an auto-positioned child, sat at 1331202 and outranked the input at 1025. The agent read `OCCLUDED: h30 is covered by [h41] generic ""` on dashdish's size radios and zilloft's filters; Chrome's `elementFromPoint` returns the input. Now an explicit `n >= 0` whose parent layer is already above the root band is `parent + 1 + min(n, 1000)`. Gate: `engine/page/tests/g_rg_e1_hidden_input_hit_test.rs`, RED under `MANUK_NESTED_Z_MUTATE=1`. Limits: negative nested z-index and auto children of a nested explicit context still use the old scale.

## RG.2.E1.a: a nested stacking context's auto descendants must outrank its own background

RG.2.E1 put a nested explicit `z-index` at `parent + 1 + n`, off the 1024 grid `auto_layer` rounds to. A MUI Menu is `fixed z-index:1300` > `absolute z-index:500` Paper > `position:relative` buttons (auto). The Paper became 1331702 while its auto buttons rounded down to the popover's base + 1 (1331202), so `elementFromPoint` returned the Paper over every control in zilloft's Beds & Baths menu (all `obstructed`). Nested roots now sit on a 32-step sub-grid inside the parent's 1024 slot (`root = base + 32*(1+min(n,30))`), auto descendants are `root + 1`, and one level deeper (a radio's `z-index:1` input in the Paper) takes `root + 2 + min(n,29)`. Gate: `engine/page/tests/g_rg_e1a_popover_paper_buttons.rs`.

## RG.3.0.a: an auto descendant of a nested explicit z-index stays on its parent's layer, and `type` is not a pointer action

A staynb guest popup is `z-index:100` three stacking contexts deep (`fixed z:10` > `relative z:1` > `absolute z:100`), so `stacking_layer` placed it in a 32-step slice at `root + 2 + n` (102400). `auto_layer` then rounded its auto-positioned "Add Adults guest" button DOWN to the slice root + 1 (102370), under the popup's own in-flow rows (102400): `OCCLUDED ... covered by generic ""` where Chrome's elementFromPoint returns the button. A slice offset of 2 or more now returns the parent's layer; `positioned` already ranks a positioned box above in-flow content on an equal layer. Red under `MANUK_AUTO_SLICE_MUTATE=1`.

Separately, `type` focuses the node and moves no pointer, so a small element over the field (udriver's Dropoff under Pickup's open "No results" list) no longer refuses it; a dialog, anything inside one, or a cover of half the viewport or more still does. Red under `MANUK_TYPE_COVER_MUTATE=1`.

An act on a named wrapper must dispatch where the pointer would land. opendining's MUI `Select time`: the agent-visible combobox (`aria-label`) is the outer InputBase, and the `onMouseDown` that opens the list is on the inner select div (its own a11y node). `agent::pointer_hit` re-asks the same `landing` hit test and, when the hit is a DOM descendant of the target, dispatches the click sequence there; events then bubble through the target as before. A click now also fires `pointerdown`/`pointerup` (mouse, `ctrlKey:false` etc. as real booleans, since Radix guards `e.ctrlKey === false`). Red under `MANUK_RETARGET_MUTATE=1` (mcp/tests/inner_target.rs) and `MANUK_POINTER_MUTATE=off` (g_pointer_events_click).

A click on an option scrolled out of its own `overflow:auto|scroll` list scrolls THAT list first (`Page::reveal_in_scrollers`, called from `verify_reach`, `reach`): the window scroll cannot reach it, and aiming at the unscrolled box lands on another row. The JS `scrollTop` mirror is refreshed after the host scroll so the clicked handler reads the new position. gocalendar-1 (RG.S1.b.b): the start-time list is 96 rows in a 200px menu.
