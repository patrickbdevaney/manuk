# The scrolling area of every element — measured, implemented, and refused

> t1425. **No engine source changed** — three implementations were measured against two WPT areas and
> every one of them regressed one. This file is the arbitration, so the next attempt starts from
> Chrome's numbers instead of re-deriving them.

## The defect

`scroll_geometry_of` maps only `overflow: auto|scroll|hidden`. CSSOM-View defines `scrollWidth` and
`scrollHeight` as *"the width/height of the element's scrolling area"* with **no scrollability
precondition**, so every other element falls through to the JS binding's fallback — its own **border
box**. `scrollHeight - clientHeight` is the *"is this overflowing?"* test that every clamped-text
widget, tooltip placer and read-more toggle on the web runs, and on an ordinary `<div>` it reads a
constant **zero**.

## The Chrome table — one box, overflow the only variable

`100x50; padding:10px; border:3px solid`, headless Chrome 145, `--hide-scrollbars`:

```text
                                          chrome sh/sw   ours
  overflow:visible, a 20px child that FITS    70 / 120   76 / 126   ← the BORDER box
  overflow:visible, a 300x200 child          210 / 310   76 / 126   ← the overflow, unseen
  overflow:clip,    the same                 210 / 310   76 / 126
  overflow:hidden,  the same                 220 / 320  220 / 320   CONTROL — already right
  overflow:scroll,  the same                 220 / 320
  overflow:auto,    the same                 220 / 320
  a bordered <span> (non-replaced inline)      0 / 0      21 / 12
```

⭐⭐⭐ **`hidden` sits with `scroll`, not with `clip`.** The end-padding inflation of CSS Overflow 3
§3.1 belongs to SCROLL CONTAINERS, and `overflow: hidden` is one (programmatically scrollable) while
`clip` is not. Ten pixels, and it is the entire distinction between the two halves of *"not visibly
scrollable"*.

## The second mechanism, and it is the one that generalises

Widening the map made the 140-cell WPT matrix go from 125 failures to **140**, with `clientHeight`
identical in all twenty configurations.

> ⭐⭐⭐ **A MAP LOOKUP DOES NOT FORCE THE REFLOW THAT A RECT READ DOES.** `layout_rect` calls
> `force_reflow_if_stale()`; `SCROLL_GEOM` is a published snapshot. While only scroll containers were
> mapped this was a rare staleness — the moment every element is mapped it is the COMMON path, and a
> loop that writes a style then reads `scrollHeight` reads the pre-write layout every time.
>
> *A latent bug in a rare path becomes the headline the moment the path stops being rare.*

With the reflow forced: 140 → **94**.

## Why it was refused — three implementations, two areas, same binary each way

Against t1424's HEAD (`cssom-view` 792/2109, `css-overflow` 517/963):

```text
                                        cssom-view      css-overflow
  widen the map + FORCE the reflow      825  (+33)      505  (-12)   ← 31 fixed, 40 broken
  widen the map, do NOT force           727  (-65)      524   (+7)
  widen + force + skip vertical modes   769  (-23)      508   (-9)
```

**The 40 broken are one file and one axis, with no exceptions**: every one is a `writing-mode:
vertical-lr|vertical-rl` row of `css-overflow/scrollable-overflow-transform-unreachable-region` — a
`flow-root|flex|grid × direction × writing-mode × flex-direction × flex-wrap` matrix over an
`overflow:scroll` wrapper with a **transformed** child — and not one `horizontal-tb` row. They pass
without the forced reflow and fail with it, which says it plainly: **our re-layout is not idempotent
for a transformed child in a vertical writing mode.** That is a layout defect, and a binding must not
paper over it.

⚠ Two scoping attempts failed, each for a reason worth keeping:

* *Skip vertical writing modes when widening* — no effect, because the wrapper is `overflow: scroll`
  and was already mapped before this tick. **A clause about newly-mapped elements cannot rescue an
  old one.**
* *Force the reflow only for non-scroll-containers* — ⭐⭐ **the predicate itself forces the reflow**:
  the only way to ask "is this a scroll container?" from the binding is `with_style`, which calls
  `force_reflow_if_stale()` on the way in. **A guard that has to look at the thing it is guarding
  cannot guard it.**

## The gate, written and proven red before the revert

`g_scroll_area_of_every_element`, six claims over the fixture above plus a `restyle` row (write
`height: 400px`, then read: Chrome `70 -> 410`). Reproduce it verbatim; each mutation failed exactly
its own rows and no others:

```text
  N1  map only auto|scroll|hidden (the pre-tick state)  → v_fit 76/126, v_over 76/126, c_over 76/126
  N2  give non-scrollable boxes the end padding too     → v_over 220/320 against 210/310
  N3  answer the border box for a non-replaced inline   → sp 21/0/12/0
  N4  drop force_reflow_if_stale in the scroll getters  → restyle 70->70
```

## The order for the next attempt

1. **Fix the layout, not the binding.** A transformed child of an `overflow:scroll` wrapper in
   `writing-mode: vertical-*`, laid out twice, must give the same scrollable overflow both times. The
   40-row matrix file is the fixture and `force_reflow_if_stale()` is how to make it fail on demand.
2. **Then land the widening and the forced reflow together**, gated on the table above. They are one
   change: the widening without the reflow costs `cssom-view` 65 subtests, and the reflow without the
   widening buys nothing.
