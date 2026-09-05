# `self-start` / `self-end` resolve in the ITEM's writing mode — the axis is the container's, the side is the item's

**Tick 1444.** `css/css-grid` failing **3636 → 3566 (−70)**, `grid-self-alignment.html` **20/72 →
72/72**; `css/css-flexbox`, `css/css-sizing` and `css/cssom-view` flat. **70 fixed, 0 new.**

## The defect

CSS Box Alignment §4 draws a distinction our pipeline erased:

* `start` / `end` resolve in the **alignment container's** writing mode.
* `self-start` / `self-end` resolve in the **alignment subject's** own.

Stylo hands both spellings to our mapper as `FlexStart`/`FlexEnd` — right about the **edge**, silent
about **whose axes name it**. The hand-rolled cascade did worse: it did not parse the keywords at all,
so they fell through to `auto` and the item deferred to its container's `align-items`.

```text
  20x20 horizontal-tb; ltr grid, 10x10 child       Chrome    before    after
  child htb  ltr, self-start          CONTROL       0,0       0,0       0,0   ✓
  child htb  RTL, self-start                       10,0       0,0      10,0
  child v-lr RTL, self-start                        0,10      0,0       0,10
  child v-RL ltr, self-start                       10,0       0,0      10,0
  child htb  ltr, plain `start`       CONTROL       0,0       0,0       0,0   ✓
  RTL child, plain `start`            CONTROL       0,0       0,0       0,0   ✓
```

## The rule, in one sentence

> ⭐ **The AXIS is chosen by the container and the SIDE is chosen by the item.**

That is why neither box's style is enough alone, and why the resolution cannot happen in the cascade:
the cascade sees one box at a time. It happens in `taffy_tree`'s grid-item pass, where the parent is
known — the same seam the replaced-item alignment rule already uses.

The cascade's job is reduced to recording **which spelling the author wrote** (`align_self_logical` /
`justify_self_logical`), because the enum cannot carry it.

## Why it is not a `direction` rule

```text
  child vertical-lr + rtl   flips the BLOCK axis   — its INLINE axis runs DOWN the screen
  child vertical-rl + ltr   flips the INLINE axis  — its BLOCK axis runs right-to-left, no `direction`
```

A fix written as *"flip when the child is `rtl`"* passes the two horizontal rows and gets **both**
vertical rows wrong. The predicate has to read `writing-mode` and `direction` together, on both boxes.

## Gate

`engine/page/tests/g_self_alignment_is_the_items_own.rs` — 12 rows, 7 controls. Red under V1 (never
reverse → the four `self-*` rows), V2 (`direction` alone → both vertical rows), V3 (flip regardless of
the spelling → a plain `start` dragged to the item's edge) and V4 (choose the acting axis from the
item instead of the container → the two vertical rows swap axes).
