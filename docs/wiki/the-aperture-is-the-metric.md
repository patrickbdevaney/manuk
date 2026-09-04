# The metric is a percentage over what happens to be checked out

> Surface audit #83, t1413. No engine source changed — the aperture was the defect.

Audit #82 asked *"does the map know what the platform shipped?"* and found nothing to add. This one
asked **what does the instrument look at?**

```text
  WPT's own css/ tree      93 directories (≈78 of them css-* test dirs)   [asked GitHub, not memory]
  our sparse checkout      14 css-* test dirs + CSS2 · cssom · selectors · support
```

Absent entirely: `css-animations` · `css-transitions` · `css-writing-modes` · `css-multicol` ·
`css-tables` · `css-cascade` · `css-variables` · `css-nesting` · `css-shapes` · `css-masking` ·
`filter-effects` · `css-scroll-snap` · `css-contain` · `css-lists` · `css-pseudo` · `css-images` ·
`mediaqueries` · `cssom-view` · `geometry` · `css-break` · `css-inline` · `css-align` · `css-logical`
and forty more. **`WPT TOTAL` is a number over under a fifth of the CSS suite.**

## Two directories, 14 MB, 828 subtests

```text
  css/css-writing-modes    96/337  = 28.5%   (80 testharness files of 479)
  css/css-multicol        732/1616 = 45.3%   (94 testharness files of 532)
```

Both first measurements, ever.

## ⭐⭐⭐ A capability that cannot be measured drifts to "missing" in every document that mentions it

t1347-1349 built the whole writing-mode subsystem — `engine/layout/src/writing_mode.rs`, orthogonal
roots, `transpose_in_place`. It has never been scored, because the directory that scores it is not in
the checkout. So:

* the **lever board** still calls writing-mode *"UNIMPLEMENTED — the biggest single unlock"*,
* the **map** had no row for how far it gets,
* the **engine** has had it for sixty ticks.

Three records, one blind spot — and the blind spot is in none of the three. It is the checkout.

## The map, corrected

`multicol`'s row was a considered **refusal** with no number: its receipt argues (correctly, and
Chrome-measured at t1325) that multicol is gated on **box fragmentation**, not on the column
algorithm. Status stays `missing` — the capability as Chrome implements it is not built — but it now
carries **45.3%**, which is how far the parts that ARE built get. *The refusal was right and the row
was blind for 1,188 ticks.*

## Where these numbers live, and why not in WPT-AREAS.tsv

`scripts/wpt-sweep.sh`'s AREAS list is observer-owned. This tick widened the **checkout** — additive,
changing no existing area's number — and deliberately did **not** add rows to `WPT-AREAS.tsv`, because
the sweep regenerates that file from its own list and would delete them. **A number that disappears on
the next sweep is worse than no number.**
