# `grid-column-start` was published and `grid-row-start` was not — the fifth axis asymmetry

**Tick 1446.** `css/css-grid` failing **3429 → 3405 (−24)**, 24 fixed and 0 new; `css/cssom-view`
flat.

## The defect

`getComputedStyle` published `grid-column-start` and `grid-column-end` — they have been in the table
since t901 — and served `undefined` for `grid-row-start`, `grid-row-end`, and all four shorthands
(`grid-row`, `grid-column`, `grid-area`, `grid-auto-flow`).

> ⭐ **A grid library reading back an item's placement got a real answer on one axis and `undefined`
> on the other.** That is worse than serving neither: `undefined.split("/")` is a TypeError, so the
> script does not fall back — it dies.

This is the **fifth** instance this session of *one of the pair was mapped and the other was not*
(t1435 which lengths count, t1436 which direction, t1438 which coordinate space, t1445 which extent).

## The serialisation rules, measured rather than assumed

```text
  grid-row-start:2; grid-row-end:span 2   grid-row  = "2 / span 2"
  nothing declared                        grid-row  = "auto"            NOT "auto / auto"
  grid-row-end:3                          grid-area = "auto / auto / 3" leading autos KEPT
  grid-area:1/2/3/4                       grid-area = "1 / 2 / 3 / 4"
  grid-row-start:2; grid-column:1/3        grid-area = "2 / 1 / span 2 / 3"
  grid-auto-flow: column dense            "column dense"
```

Two rules fall out, and each has a row that isolates it:

* **Only TRAILING `auto`s are dropped.** `auto / auto / 3` keeps both leading ones because a real line
  follows. A serializer that filters every `auto` reads `3` — a *different placement*, silently.
* **`grid-area` interleaves the axes**: row-start / column-start / row-end / column-end. It is the one
  ordering in this family that is not "start then end", and only a row whose four values are all
  distinct can catch getting it wrong.

## Named residue

`grid-template` still reads `undefined`: it serialises the **explicit** template (`"50px 50px /
100px"`) where `grid-template-rows` reports the **used** tracks, so it is a different value and not a
join of the two. And a custom-ident line name has no representation at all — `GridLine` is
`Auto | Line(n) | Span(n)`, so `grid-row-start: foo` cannot round-trip. Both are cascade-side gaps
rather than serialisation ones.

## Gate

`engine/page/tests/g_grid_placement_is_readable.rs` — 8 assertions over 6 elements. Red under Y1 (drop
the row longhands and shorthands → `undefined`, while `gridColumnStart` stays green: the asymmetry
itself), Y2 (never drop trailing `auto`s → `auto / auto`), Y3 (drop every `auto` → `3` where Chrome
says `auto / 3`) and Y4 (order `grid-area` start/end/start/end → the all-distinct row alone).
