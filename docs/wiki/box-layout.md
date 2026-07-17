# BOX LAYOUT — taffy mapping realities and quirks (flex / grid / sizing)

Manuk's flex and grid layout runs on a vendored **taffy 0.12** tree (`engine/layout/src/taffy_tree.rs`,
`flex.rs`). Block / inline / float / table nodes are Manuk-measured *leaves* of that tree; taffy only
lays out the flex/grid containers and their directly-nested flex/grid descendants. The mapping from
Manuk's `ComputedStyle` to `taffy::Style` (`to_taffy_style`) is where the realities below live.

## A mixed `calc()` must NOT collapse to one term — taffy has calc plumbing; wire it (tick 139)

`Dim::Calc { px, pct }` is Manuk's reduced linear form of a `calc()`: the used length is
`px + pct% · basis` (percentages stored 0–100). The **block** path resolves it correctly everywhere via
`Dim::resolve(reference, _)`. The **taffy** path did not: `dimension()`/`lp()`/`lp_auto()` mapped a
`Dim::Calc` to `length(px)` when `px != 0` **else** `percent(pct)` — i.e. it kept **one term and threw
the other away**. So `width: calc(100% − 250px)` (`px = −250, pct = 100`) became `length(−250)`, which a
flex item clamps to **0**. Every fixed-gutter sidebar split — `calc(100% − <rail>)` main, `calc(<fixed> +
100%)` panel — collapsed to nothing on a flex/grid parent while rendering fine on a block parent.

**The fix is not a hand-rolled resolver — taffy already has one.** taffy's `Dimension` /
`LengthPercentage` / `LengthPercentageAuto` each have a `::calc(ptr: *const ())` constructor (behind the
`calc` feature, which is in taffy's default set). `ptr` is an **opaque handle** taffy never dereferences;
it hands the handle back to `LayoutPartialTree::resolve_calc_value(&self, ptr, basis) -> f32` with the
definite basis at layout time, and expects the resolved px.

**The encoding (why it is an index, not an address).** `CompactLength::calc` asserts the handle is
**non-null and 8-byte aligned** (low 3 bits = 0) and round-trips it verbatim (the calc tag is `0b000`, so
no bits are stolen). We keep a `calc: Vec<(f32, f32)>` on the `TaffyDom` and encode the *index* as
`((idx + 1) << 3) as *const ()` — 8-aligned, non-null (the `+1` keeps index 0 off null), and an index
rather than a pointer into the `Vec`, so the `Vec` may reallocate freely without invalidating any live
handle. `resolve_calc_value` decodes `(ptr >> 3) − 1`, looks up `(px, pct)`, and returns `px + basis · pct
/ 100` — the *same* linear form the block path uses, so flex/grid items and block items now agree.

Only a **genuinely mixed** calc (both terms non-zero) needs the handle; a single-term calc still takes
taffy's `length`/`percent` fast path (no table entry). Gate: falsifiable unit test
`flex_item_calc_width_mixes_px_and_percent` + full-pipeline `flex_sidebar_calc_width_resolves_in_full_pipeline`
(a `calc(100% − 250px)` sidebar in a 1000px flex row must be 750px; the old collapse made it 0, and
reverting the wiring makes both tests go red).

**Bound.** Grid *track* sizing (`track_min`/`track_max`) still maps calc via the collapse — `calc()` in a
`grid-template-columns` track is rarer and is a follow-on. Sizes, min/max, inset, margin, padding and
flex-basis all resolve calc correctly.

## Why the WPT calc suites barely move on a layout-calc fix

`css/css-values`'s ~85 `calc(...)` tests are overwhelmingly **serialization / computed-value** tests
(`getComputedStyle` returns the calc expression) — a *cascade* axis, not a *layout* one — so a taffy-path
fix does not touch them. The `css/css-sizing` and `css/css-flexbox` calc tests that DO measure layout are
mostly **reftests** (Bar-2, skipped) or additionally depend on **intrinsic sizing** (min/max-content
propagation, still open). A layout-calc fix is therefore a *daily-driver render* win with a near-zero WPT
flip — gate it with a falsifiable layout assertion, not a subtest count.

## Absolute box with both insets set is a DEFINITE size — lay its children out with it (tick 144)

An `absolute`/`fixed` box with both block insets set (`top` and `bottom`, i.e. the `inset:0` fill pattern)
has a **definite** used height via the constraint equation — *containing-block height − top − bottom −
frame* (CSS2 §10.6.4) — even when `height:auto`. Manuk's abspos layout computed that height correctly for
the box itself, but did so **after** laying out the children (`layout_children(..., None, ...)`), so a
`height:100%` child was resolved against an *indefinite* base and **collapsed to 0**. On the real web that
is the overlay / modal / backdrop: `position:absolute; inset:0` to fill a positioned ancestor, with a
`height:100%` inner layer that then measures 0 and vanishes.

**Fix (`layout_abs`, `engine/layout/src/lib.rs`).** Compute the definite content height *before* the
children in the two cases where it is knowable without them — an explicit non-`auto` height, and
`height:auto` with both insets set (the constraint equation) — and thread it down as the percentage base
(`pch`). The content-sized case (auto height, not both insets) keeps `pch = None`, which is correct: a `%`
height there is `auto`. The post-children height computation is unchanged (a non-`auto` `Dim` ignores its
`auto_px` fallback, so this equals the old `other.resolve(cb.height, ch)`), so box heights do not move —
only percentage-height *children* of definite abspos boxes gain a real base.

**WPT / gate.** `css/css-sizing` +2 (`abspos-intrinsic-height-inset-percentage-child`'s `height:auto` and
`height:stretch` cases; the `fit/min/max-content` cases stay failing — those need real intrinsic-keyword
`Dim` variants, still `Dim::Auto` today). Gated by the falsifiable layout unit test
`abspos_inset_zero_gives_percentage_height_child_a_definite_base` — RED (child = 0) when the base is
withheld, GREEN (child = 200) with it. **Note:** the test cascade `MinimalCascade` parses the
`top/right/bottom/left` longhands but *not* the `inset` shorthand, so the unit test uses the longhands; the
full stylo pipeline (what the WPT run and real pages use) parses `inset:0` too.

## The CSS `aspect-ratio` property was never mapped from the cascade (tick 145)

`ComputedStyle.aspect_ratio` (a plain `width/height` f32) was set in exactly one place — the page layer,
from a **decoded image's** intrinsic pixels (`engine/page/src/lib.rs`). The **CSS `aspect-ratio`
property** had no arm in `stylo_map.rs`, so `aspect-ratio: 16/9` on a `<div>` reached layout as `None`.
The transfer machinery already existed and was correct — the in-flow block path derives an auto width
from a definite height (`layout/src/lib.rs` §1372) and an auto height from the width (§1459) — it just
**never had a value to transfer**. A first attempt that added an abspos transfer moved the WPT count by
zero and named the real bug: *the mechanism existed; the value never reached it* (the metric-won't-move →
suspect-the-metric lesson).

**Fix (three parts).**
1. `stylo_map.rs` maps stylo's computed `AspectRatio { auto, ratio: PreferredRatio<NonNegativeNumber> }`
   onto `s.aspect_ratio = w/h` whenever a `<ratio>` is present (the `auto` keyword is dropped — for a
   non-replaced box the specified ratio always applies).
2. The hand parser `MinimalCascade` (`engine/css/src/lib.rs`) learns `aspect-ratio` at parity: `w/h`, a
   bare number (`n/1`), and `auto <ratio>`. This keeps the two cascade paths in step and lets the layout
   tests drive real CSS instead of injecting the field.
3. `layout_abs` gains a **box-sizing-aware** aspect-ratio transfer for its auto width (scale the definite
   height in the box the ratio names — `ch + bs_extra_h` — then convert back to content, `- bs_extra_w`;
   both deltas 0 under content-box) and, a pre-existing gap, now honours `box-sizing:border-box` for its
   own explicit `width`/`height`.

**WPT / gate.** `css/css-sizing` 229→240 (+11), all from the mapping (the in-flow transfer, live at
last); css-flexbox/grid/position/overflow flat. Gated by `aspect_ratio_parses_to_a_width_over_height_ratio`
(css) and the end-to-end `abspos_aspect_ratio_transfers_definite_height_to_auto_width` (layout, RED when
the transfer arm is neutralised). **Residue:** `abspos-aspect-ratio-border.html` still fails — those boxes
set no insets, and a static-position abspos box records no geometry, so `offsetWidth` reads 0 regardless
of the ratio. That is a separate mechanism (static-position abspos placement), not an aspect-ratio bug.
