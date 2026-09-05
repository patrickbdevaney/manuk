# `minmax(<content-based>, 0px)` is `minmax(min, min)` — 458 subtests on one sentence of the spec

**Tick 1440.** `css/css-grid` failing **4183 → 3725 (−458)**; `css/css-sizing`, `css/css-flexbox` and
`css/cssom-view` flat. **474 subtests fixed, 16 shifted, no file regressed.**

## The defect

CSS Grid §12.5: *"if the growth limit is less than the base size, increase the growth limit to match
the base size."* A **zero** maximum therefore never caps anything — the track ends up at whatever the
items contribute. Taffy applies that flooring when the base comes from a **fixed** minimum and not
when it comes from the items, so every `minmax(auto, 0px)` track collapsed to zero.

```text
                                                   Chrome     before      after
  minmax(60px, 0px)                    CONTROL     60px       60px        60px  ✓
  minmax(auto, 0px)   item width:60px              60px        0px        60px
  minmax(auto, 0px)   in a 40px grid               60px        0px        60px
  minmax(min-content, 0px)  8-char word            92.4375px   0px        92.4375px
  minmax(auto, 0px)   item min-width:60px          60px        0px        60px
  minmax(0px,  0px)   item width:60px  CONTROL     0px         0px        0px   ✓
```

> ⭐⭐ **`minmax(60px, 0px)` is the row that localises it.** The same violation with a *fixed* minimum
> was already Chrome-exact, so taffy's flooring rule exists and only the content-derived base misses
> it. And `minmax(auto, 0px)` in a 40px grid — no free space to confuse the picture — reads **60**,
> which says the base itself is computed perfectly well. **It is the flooring that is missing, not the
> measurement.**

## Two candidates measured and refused

The general rule needs the base size, which is not known until taffy has run. Both wider remappings
were built and measured before the narrow one was:

```text
  max -> auto()          `minmax(auto,0px)` in a 100px grid reads 100px — it absorbs the free space
  max -> fit_content(L)  `minmax(auto,100px)` with a 60px item reads 60; Chrome 100 — growth lost
```

A third candidate — flooring the ITEM's `min_size` by its definite `width`, which is literally what
Grid §12.5.1 says the minimum contribution is — was also built and is **inert**: an item that already
declares `min-width: 60px` still produced a 0px track, so taffy is not consulting the item minimum for
this track at all.

## The fix, and why the bound is the point

```rust
CssTrackSize::MinMax(lo, hi) if lo is content-based && hi == Px(0.0)
    => minmax(track_min(lo), fit_content(length(0.0)))
```

**A limit of zero can never exceed a base**, so flooring it is unconditional and there is no growth to
lose. That is the same arithmetic the spec describes, restricted to the one case where it needs no
unknown.

## Named residue

A **non-zero** too-small maximum: `minmax(auto, 20px)` with a 60px item is **60px** in Chrome and
**20px** here. It is the general form of the same rule and it wants taffy's base size, not another
remapping — the honest next step is a second solve, on the model of `solve_subtree`'s auto-margin
re-solve.

## Gate

`engine/page/tests/g_grid_zero_max_track_is_its_minimum.rs` — 9 rows, 3 controls. Red under R1 (drop
the remap → six rows collapse, the three controls hold), R2 (`auto()` as the maximum → 100px) and R3
(`fit_content` for any fixed maximum → the growth control reads 60).
