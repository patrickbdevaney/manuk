# `align-items: stretch` sometimes SHRINKS a box, and the adoption could only grow one

**Tick 1436.** `css/css-grid` **−62 failing**, `css/css-flexbox` **+3**, `css/css-sizing` flat,
`css/cssom-view` flat.

## The defect

At the end of `Ctx::extract_placed` (`engine/layout/src/lib.rs`), taffy's slot height was taken for
an `auto`-height flex/grid item **only when it was larger** than the height the item measured for
itself:

```rust
if self.style_of(p.dom).height == Dim::Auto && p.slot.height > boxx.rect.height {
    boxx.rect.height = p.slot.height;
}
```

`align-items: stretch` is the initial value, so it is the case on nearly every page, and it sets an
`auto` **cross** size to the **line's** cross size — letting the content overflow. When the content
is taller than the line, taffy's slot is *smaller*, and the `>` declined it.

> ⭐ **The same asymmetry t1435 fixed one property earlier, in the same function.** The inline axis
> takes taffy's verdict unconditionally; the block axis took it only when it agreed. First it was
> *which lengths* count (t1435), then *which direction* counts (t1436).

## The half that must NOT be adopted, and it cost ten rows to learn

The first version dropped the comparison outright. `css/css-flexbox` went **−3**, and the diff of the
failing NAME lists put all of it in one file:

```text
  flex-basis-013.html   10 new failures    "height expected 50 but got 0"
```

That file is `flex-direction: column` — the height is the **MAIN** axis, where an `auto` item's size
is its content size floored by `min-height: auto`. Taffy's slot comes back **0** there.

> ⚠⚠⚠ **A slot smaller than the content is a VERDICT on the cross axis and a MISSING MEASUREMENT on
> the main one.** The same number means opposite things depending on which axis it is on, and only
> the axis test can tell them apart.

So the rule carries the axis:

```rust
fn container_stretches_y(&self, container: NodeId) -> bool {
    match display {
        Grid | InlineGrid => true,
        Flex | InlineFlex => {
            let row = matches!(flex_direction, Row | RowReverse);
            !(row == writing_mode.is_vertical())   // the y axis is the CROSS axis
        }
        _ => false,
    }
}
```

`row == is_vertical()` is the same expression the scroll origin uses (t1427): a `row` flex is
physically horizontal in a horizontal writing mode and physically **vertical** in a vertical one.

## And a replaced element is not stretched — the wall taught the fixture

`align-items: stretch` on an `<img>`/`<canvas>` with an intrinsic ratio does **not** hand it the
line's cross size; the ratio decides. Adopting taffy's slot published taffy's own ratio arithmetic
over ours, and `manuk_layout::tests::replaced_constraint_violation_table_per_formatting_context` went
red on two cells that had been green:

```text
  j  box-sizing:border-box; padding:10px; max-width:150px, on a 480x474 replaced box
       Chrome  150.0x148.4      ours (flex, grid)  150.0x148.1
```

⭐ **The 0.3px is not the point.** A replaced box's cross size is a **transfer**, not an alignment —
so an alignment verdict does not apply to it, whatever the number. The gate now carries the pair
`s11` (canvas in a flex row) and `s12` (the same canvas in a block): they must agree.

⚠ This is the second time in two ticks that the WALL found what the tick's own fixture could not.
The fixture was all `<div>`s.

## A term measured and removed

The first version also carved out `height: stretch` (which shares `Dim::Auto`'s representation and is
a definite fill, not an auto height). Once the axis test was in place that carve-out was **inert** —
identical on the gate fixture, `css/css-flexbox` 3209 either way, `css/css-grid` failing 4183 either
way — so it is not in the tree. *Same discipline as t1435's `parent_is_flex` guard, same outcome.*

## The number, and how it was read

```text
                     base (t1435)   after      by NAME
  css/css-grid       failing 4245   4183       63 fixed, 1 swapped within one file
  css/css-flexbox    3206/4693      3209/4693  3 fixed, 0 new
  css/css-sizing     1331 failing   1331       IDENTICAL name lists
  css/cssom-view     1178/2109      1178/2109  flat
```

⚠⚠ **`css/css-sizing` read `1360` then `1361` failing from the SUMMARY line and its failing NAME
LISTS are byte-identical (1331 both).** The summary's denominator churns between runs of the same
binary; the name list does not. **Diff the names.** This is t1435's denominator lesson one level
sharper: even the *failing count* off the summary moves.

## An instrument finding, and it is bigger than this tick

`css/css-grid`'s first post-fix run read failing **4246** (+1 vs base) and the name diff put **63 of
the 64 new failures in ONE file**: `grid-lanes/animation/grid-template-columns-interpolation.html`.
Re-running the same binary gave **4183** (−62) with that file back to its usual count, and the
`grid-lanes/animation` subdirectory alone read `1105` then `1108` on consecutive runs.

> ⚠⚠⚠ **ONE WPT FILE CAN FLAKE BY 63 SUBTESTS — more than this tick's entire real signal, and enough
> to false-red the ratchet on an area total.** It is an ANIMATION interpolation file, so it is
> timing-dependent by construction. Any area delta smaller than that file's swing is unreadable
> without a repeat run.

## Named residues

* `grid-item-minimum-size-single-axis-scroll-container.html` scores 2/4 before and after — `.item 3`
  and `.item 4` **trade places** (`expected 0 but got 100` becomes `expected 100 but got 0`). One
  file, no net change, and it is the minimum-size rule for a single-axis scroll container.
* The `s9` shape in a **vertical** writing mode (`writing-mode: vertical-lr`, a `<canvas>` at
  `height:100%`) reads Chrome **5** and ours **0**. Pre-existing — this tick's branch is inactive
  there — and it is the same un-measured main axis, one writing mode over.

## Gate

`engine/page/tests/g_flex_stretch_can_shrink.rs` — 8 rows (5 controls/regression arms), red under M1
(the pre-tick comparison), M2 (drop the axis test → `s9=50x0`, the ten-row regression in one row),
M4 (grid arm off) and M6 (drop `!replaced` → `s11` disagrees with `s12`). **M5 (replace `row == is_vertical()` with `!row`) is reported GREEN** — no row in
the fixture discriminates the writing-mode term; it is kept for correctness, not coverage.
