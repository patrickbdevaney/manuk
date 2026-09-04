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


## t1414 — twelve specs, and the number

Audit #83 ranked widening the checkout first. Ten more directories were added and every one measured:

```text
  css/css-transitions         638/2664    23.9%     2026 failing
  css/cssom-view              563/2109    26.7%     1546 failing
  css/css-images             2160/3582    60.3%     1422 failing
  css/mediaqueries            757/1766    42.9%     1009 failing
  css/css-multicol            732/1616    45.3%      884 failing
  css/css-animations          503/1305    38.5%      802 failing
  css/css-lists               448/959     46.7%      511 failing
  css/css-pseudo              134/615     21.8%      481 failing
  css/css-cascade             110/526     20.9%      416 failing
  css/css-tables              512/899     57.0%      387 failing
  css/css-writing-modes        96/337     28.5%      241 failing
  css/css-variables           346/542     63.8%      196 failing
  ───────────────────────────────────────────────────────────────
  TOTAL                      6999/16920   41.4%     9921 failing     cost: 31 MB
```

**6,999 already-passing subtests the primary metric had never counted.** HANG/CRASH 0 in all twelve.

⚠ **`cssom-view` at 26.7% is the one that should alarm**: `getBoundingClientRect`, `scroll*`,
`elementFromPoint`, `matchMedia` — the channel the AGENT reads geometry through and the channel the
fidelity oracle scores placement with. But it is not yet 1,546 defects: `diag` reports
`testsCreated: 0` with `errors: []` on three of its files, which needs a probe before it needs a fix.

⭐⭐ **And the control row caught a near-miss.** The release binary's mtime predated the last landed
tick, which made every number look stale. Checked rather than assumed — `cargo build` printed
`Finished` with no `Compiling`, so the mtime was a preserved hardlink and the artifact was current.
One command, against a table that would have been wrong in a way nobody could later detect.


## t1416 — and a count cannot tell you what KIND of work an area is

The twelve-area table above is ranked by failing count. `--show-failures` (already built — the third
"already built" of that session) says the counts hide three different work items:

```text
  css/cssom-view          1,480 failing
      600  scrollWidthHeight-negative-margin-002.html      ← ONE combinatorial matrix
      125  scrollWidthHeight-overflow-visible-margin-collapsing.html
       44  scrollWidthHeight-overflow-visible-negative-margins.html

  css/css-cascade           347 failing
      235  spread across ~20 `@scope` files                ← ONE unimplemented FEATURE
      112  everything else
```

> **A failing count cannot tell one combinatorial file from one unimplemented feature from a genuine
> spread, and they are three different work items.** Check an area's CONCENTRATION before ranking by
> its count — one `--show-failures` run and one `awk`.

So `cssom-view` at 26.7% is not 1,546 defects: it is ~769 subtests of one rule (how negative margins
and margin collapsing contribute to the scrollable overflow region — `scrollHeight expected 90 but got
80`, short by exactly the negative margin) plus a tail. And `css-cascade` at 20.9% is `@scope`: a
feature to decide on, not a bug to fix.
