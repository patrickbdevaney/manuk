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

## An intrinsic-keyword `height` is INDEFINITE — not the same as `auto` (tick 146)

`size_to_dim` (`stylo_map.rs`) collapses **every** non-length `Size` to `Dim::Auto`: `auto`, `stretch`,
`fill-available`, *and* the intrinsic keywords `min-content`/`max-content`/`fit-content`. That is fine for
length *resolution* (they all lack a length), but it erases a distinction layout needs. Tick 144 taught
`layout_abs` that an `auto` height with **both** insets set is **definite** (CSS2 §10.6.4 constraint
equation: `CB − top − bottom`) so a `height:100%` child gets a real base. But an **intrinsic-keyword**
height is **indefinite** (CSS Sizing 3 §cyclic-percentage-contribution): the box sizes to content and the
`%`-height child sees an indefinite base → auto. Collapsed to `Dim::Auto`, `height:fit-content` looked
exactly like `auto`, so an `inset:0; height:fit-content` popover **stretched to the containing block
(200)** instead of hugging its content (80). The `top-only` case already did the right thing — only the
both-insets definite path over-reached.

**Fix.** A new `ComputedStyle::height_intrinsic: bool`, set true for `min`/`max`/`fit-content` (and
`fit-content(...)`) — NOT for `auto`/`stretch`/`fill-available`, which stay definite. Set in `stylo_map`
(`size_is_intrinsic`, matching the `GenericSize` keyword variants) and in the hand parser at parity.
`layout_abs`'s `definite_ch` gains one arm — `Dim::Auto if s.height_intrinsic => None` — so the box falls
to the existing content-sizing path. In-flow layout is deliberately untouched: a block's `auto` and
intrinsic-keyword heights both size to content there, so the collapse stays correct; only the abspos
both-insets path changes.

**WPT / gate.** `css/css-sizing` 240→243 (+3, the fit/max/min-content subtests of
`abspos-intrinsic-height-inset-percentage-child.html`); css-flexbox/grid flat. Gated by
`intrinsic_height_keywords_flag_the_box_as_indefinite` (css) and
`abspos_intrinsic_height_with_inset_zero_sizes_to_content_not_stretch` (layout) — the latter also asserts
`auto`/`stretch` **still** stretch to 200, locking tick 144's behaviour in as a regression guard. Proven
RED by neutralising the guard arm. **Note:** the unit cascade parses the inset *longhands* but not the
`inset` shorthand (a tick-144 note), so the layout test drives `top/right/bottom/left:0`; the WPT run uses
stylo, which parses `inset:0`.

## A `position:relative` percentage `top`/`bottom` resolves against the containing-block HEIGHT (tick 147)

`layout_block`'s `position:relative` offset resolved the horizontal delta against `cw` (the containing
block width — correct) but the **vertical** delta against a hardcoded `0.0`. The comment rationalised it
(*"height unknown here"*), but the height is **not** unknown: `pch: Option<f32>` — the definite content
height already threaded down for percentage *sizing* (`height:50%`, `min/max-height`; tick 144) — is
exactly the containing-block height a `%` inset resolves against. So `top:50%` on a relative box computed
`50% of 0 = 0` and the box **never moved vertically**; every percentage-nudge / vertical-centering relative
idiom silently sat at its flow position.

**Fix.** `let cb_h = pch.unwrap_or(0.0);` and resolve the vertical delta (`top`/`bottom`, and their calc/
percent forms) against it. `pch == None` (indefinite CB) still yields 0 — which is the spec's "computes to
auto" for `top`/`bottom` percentages against an auto-height containing block, so nothing regresses. The
containing block only threads a definite `pch` when it has one — an abspos box with a definite `height`
(the position-relative-016 cases), or any block with an explicit/resolved height.

**WPT / gate.** `css/css-position` 69→75 (+6, the definite-CB subtests of `position-relative-016.html`;
the inline / auto-height-ancestor cases t6–t9 still fail — they don't thread `pch`, a separate mechanism).
Bonus `css/css-flexbox` 949→953 (+4, relative flex items). css-sizing/grid/transforms flat. Gated by
`relative_percentage_top_resolves_against_containing_block_height` (layout), which measures the shift as a
*delta* vs `top:0` (isolating it from the box origin) and is proven RED by reverting `cb_h` to `0.0`.

## Percentage heights resolve against the initial containing block, and `max-height:%` on an indefinite parent is `none` (tick 150)

Two percentage-height bugs, one theme — a `%` height reference that was silently **0**.

**1. The full-height app-shell chain never filled the window.** `layout_document` seeds the root box
(`body`, or `html` if no body) with `pch: None`. But the initial containing block has the **viewport's**
dimensions (CSS2 §10.1), and its height is the reference a root-level `height: 100%` resolves against — the
`html,body{height:100%}` → `#app{height:100%}` chain every SPA uses to make a scroll pane fill the window.
With `None`, that root percentage was indefinite, so the whole chain fell back to *content* height: the
pane collapsed to its content and a `100vh` sibling (resolved at **parse** time against the same viewport)
filled the window while the `height:100%` box next to it did not — a visible inconsistency between two
spellings of the same intent. **Fix:** seed the root with `Some(manuk_css::values::viewport_size().1)` —
the *same* viewport `vh` resolves against, so the two can never disagree. Only elements with an explicit
percentage/definite height up the chain change; an `auto`-height body still yields `None` to its children,
so content-sized pages are untouched.

**2. `max-height:%` against an auto-height parent clamped the box to 0.** `max_h` resolved the percentage
against `pch.unwrap_or(0.0)`, so an indefinite containing block gave `max-height: 100%` → `0` and the box
vanished. Per CSS2 §10.7 a percentage `max-height` against an indefinite CB height is treated as **`none`**
(no cap). **Fix:** `Dim::Percent(_) if pch.is_none() => f32::INFINITY` (and the `Calc{pct != 0}` form).
This is the ubiquitous `img { max-width:100%; max-height:100% }` responsive reset — previously every such
image collapsed to nothing inside an auto-height parent. (`min-height:%` against an indefinite CB is `0`,
which `unwrap_or(0.0)` already produced — no change needed there.)

**WPT / gate.** `css/CSS2/normal-flow` 17→18 (the `height:30000px; max-height:100%` case). The app-shell
chain is mostly reftest-covered (Bar 2, deferred), so it is gated by unit tests instead:
`root_percentage_height_fills_the_viewport` (asserts `#app` height == the viewport height through the root)
and `percentage_max_height_indefinite_parent_is_none` (asserts a `height:500px; max-height:100%` box stays
500 inside an auto-height parent). Both proven RED by reverting the respective change. flex/position/
overflow/sizing flat, HANG/CRASH 0.

## Parent↔child margin collapsing (tick 151)

The last unmodeled piece of CSS2 §8.3.1: for ~150 ticks `layout_block` collapsed **adjacent-sibling**
margins (`collapse_margins`) but left a documented gap — a parent's margin did not collapse with its
first/last in-flow **block child's**. That left the child's margin sitting *inside* the parent as a
spurious gap: the classic `<div class=card><h2>…</h2></div>` where the h2's top margin opened a band of
card-background above the heading, and the symmetric bottom case where the parent's height double-counted
the last child's trailing margin.

**When it applies.** A block collapses its top (resp. bottom) margin with its first (resp. last) in-flow
block child when the box is a plain `display:block`, `overflow:visible`, establishes no BFC, and has **no
border and no padding on that edge** (`top_margin_collapses` / `bottom_margin_collapses`). Bottom
additionally requires **auto height** — a definite height fixes the content box, so the margin cannot
escape. Clearance on the first child, or a leading/trailing out-of-flow (float/abs) child, declines the
collapse (conservative: never wrong, occasionally incomplete). `overflow:hidden`/`auto`/`scroll` — the
card/clearfix margin-containing idiom — deliberately does **not** collapse.

**Top — hoist upward.** A cheap left-spine peek `collapse_through_top(node)` computes the first in-flow
block child's *collapse-through* top margin (its own top margin joined recursively with ITS first block
child's, down the spine — the h2-margin has to travel up through however many border/padding-less wrappers
sit between it and the card). `layout_block` folds that into the box's own top margin (`effective_mt`),
which both raises the box's border-top and is reported as `margin_top` so a grandparent collapses against
the already-collapsed value. `layout_children` recomputes the identical hoist and places the first block
`hoist_top` higher, landing it flush at the content top. Using the *same* peek on both sides makes the
child land exactly at `content_y` regardless of the peek's precision.

**Bottom — escape downward.** `collapse_through_bottom` walks the right spine symmetrically. When the box
is bottom-eligible and auto-height, that trailing margin is **subtracted from content height** (undoing the
old "the last in-flow block's trailing margin still occupies the container" line) and collapsed into the
box's own `margin_bottom` (`effective_mb`), so it escapes below the border-bottom instead of padding the
parent from the inside.

**Approximation (documented).** The spine walks resolve percentage *vertical* margins against an
approximate width (the top box's containing-block width, not each level's own content width). px/em
margins — width-independent and the overwhelming norm — are exact; only a percentage vertical margin deep
in the spine drifts, and only in where the collapsed margin lands, never in whether the collapse fires.

**Gate.** Four unit tests, the two collapse ones proven RED by disabling the eligibility helpers:
`parent_child_top_margin_collapses` and `parent_child_bottom_margin_collapses` (child flush / no internal
gap), plus the eligibility guards `overflow_hidden_contains_child_margin` and
`top_border_blocks_margin_collapse` (which correctly stay green when collapse is off — they assert
*non*-collapse). The visible wins are mostly Bar-2 reftests (deferred); the testharness sweep held or
nudged up — css-flexbox 26.5→26.6%, css-sizing 14.5→14.8%, css-position/overflow/normal-flow flat,
**HANG/CRASH 0**. Nothing regressed, which is the bar for a mechanism this broad.

## `overflow` establishes a block formatting context — float containment / the clearfix (tick 152)

`establishes_bfc` had listed float/abspos/flex/grid/inline-block but **not `overflow`** ("overflow is
not modeled yet"). So `overflow:hidden`/`auto`/`scroll` — the single most common float-containment idiom
on the web — did nothing structural: a container's floated children escaped it (the box stayed as tall
as its own non-float content) and the box's content still wrapped around *outer* floats. A probe made it
concrete: `<div style="overflow:hidden"><div style="float:left;height:60px"></div>text</div>` came out
**18px** tall (one text line) — the 60px float escaped.

**Fix.** One clause: `s.overflow != Overflow::Visible` establishes a BFC (CSS2 §9.4.1 / Display §2.1).
Any non-`visible` value (hidden/auto/scroll/clip — Chrome establishes a BFC for `clip` too) then routes
through the existing BFC branch in `layout_block`: the box gets its own `FloatContext`, its floats stay
inside, its content does not overlap outer floats, and it grows to contain its floats via
`own_bfc.lowest_bottom()` (CSS2 §10.6.7 auto-height). This is the modern clearfix and the reason
`overflow:hidden` on a card/row makes it wrap its floated media.

**Interaction with margin collapse (tick 151).** A BFC does not collapse margins with its children, and
`top_margin_collapses`/`bottom_margin_collapses` already gate on `overflow == Visible` *and*
`!establishes_bfc` — so `overflow:hidden` correctly both contains floats AND contains child margins, with
no double-handling. The `overflow_hidden_contains_child_margin` guard (t151) still holds.

**Gate.** `overflow_hidden_contains_floats` (parent height >= the 60px float), proven RED by the 18px
probe. Sweep: css-flexbox 26.6%, css-position 28.8%, css-overflow 27.8%, css-sizing 14.8%,
CSS2/normal-flow 15.4%, CSS2/floats 60% — all flat, **HANG/CRASH 0**. overflow:hidden is pervasive, so
flat-with-no-crash across the layout suites plus the full wall (parity + oracle fidelity) is the bar.

## An intrinsic-keyword `width` HUGS its content — not the same as `auto` (which fills) (tick 153)

**Symptom.** `width:fit-content` on a block filled the containing block (a probe: `<div width:fit-content>fit</div>`
in a 300px parent came out **300px** where Chrome hugs at ~14px). `width:max-content`/`min-content` likewise
filled. This is the companion of the tick-146 *height* case, on the axis where it actually shows: the
"hug the contents" idiom — a `fit-content` badge/tag/pill, a `max-content` single-line label, and the
`width:fit-content; margin-inline:auto` centered-block-that-hugs pattern — silently stretched edge-to-edge.

**Cause.** All three intrinsic keywords collapse to `Dim::Auto` in both style paths (`stylo_map::size_to_dim`,
the hand parser). Only `height_intrinsic: bool` was retained (t146, for the abspos indefinite-height case) —
nothing carried the keyword on *width*, so a keyword width was indistinguishable from `auto` and took the
block auto-width **fill** branch (`cw − extra`). `stretch`/`-webkit-fill-available` were already correct
because they ARE definite fills.

**Fix.** A new `IntrinsicSize { MinContent, MaxContent, FitContent }` enum, stored as
`ComputedStyle::width_keyword: Option<IntrinsicSize>`, set in `stylo_map` (`size_intrinsic_kw`, matching the
`GenericSize` variants; `fit-content(<len>)` → `FitContent`) and in the hand parser at parity. Block width
resolution gains one arm before the auto-fill: `Dim::Auto if width_keyword.is_some()` →
`MinContent → min_content_width(node)`, `MaxContent → max_content_width(node)`,
`FitContent → shrink_to_fit(node, cw − extra)` — the *same* measure functions inline-block already uses, so
identical Bar-0/recursion profile, and they return **content-box** widths so the box-sizing subtraction
(guarded on `width != Auto`) correctly stays skipped. min/max-width clamps still apply after (CSS Sizing L3).
The auto-margin centering guard also widens to `width != Auto || width_keyword.is_some()` so a keyword width
(definite for margins) centers under `margin:auto`. Flex/grid **items** are taffy-decided (`taffy_known`) and
untouched; width-only scope because block auto-height already resolves to content height.

**Gate.** `width_fit_content_hugs`, `width_max_content_hugs`, `width_min_content_is_longest_word`,
`width_fit_content_still_clamped_by_max_width` (layout), the first three proven RED by the 300/1000px fill.
Sweep: css-sizing 14.8%→**15.1% (+5)**, css-flexbox 26.8%, css-grid 9.2%, css-position 28.8%,
CSS2/normal-flow 15.4% — neighbors flat, **HANG/CRASH 0**.

## `height: stretch` / `-webkit-fill-available` FILLS the parent's definite height (tick 154)

**Symptom.** `height:stretch` on a block inside a 200px-tall parent came out **18px** (content height) —
a full-height panel collapsed to one line. `-webkit-fill-available` same. The vertical companion of t153:
on the WIDTH axis `auto` already fills so `stretch` "worked" incidentally, but a block's `height:auto` is
CONTENT height, so `stretch` vs `auto` is a real, visible distinction that was never modeled. Tick 146's
comment even declared stretch "definite" — but nothing gave it filling behavior; it collapsed to `Dim::Auto`.

**Cause.** `stretch`/`-webkit-fill-available`/`-moz-available` on `height` collapse to `Dim::Auto` and are
NOT flagged `height_intrinsic` (they are definite, not indefinite) — so they were indistinguishable from
plain `auto` = content height.

**Fix.** New `ComputedStyle::height_stretch: bool`, set in `stylo_map` (`size_is_stretch`: `GS::Stretch |
WebkitFillAvailable` — crates.io stylo 0.19 folds `-moz-available` into `WebkitFillAvailable`, no separate
variant) and the hand parser at parity (`stretch`/`-webkit-fill-available`/`-moz-available`). In
`layout_block`'s `own_definite_h`, a new arm: `Dim::Auto if height_stretch => pch.map(|h| (h − mt − mb −
pt − pb − bt − bb).max(0))` — the MARGIN box fills the containing block's definite content height `pch`, so
the content box is `pch` minus this box's own margins/border/padding (box-sizing-independent: stretch fills
available space, not a specified length). `pch` (threaded since t144) is the same reference `height:%`
children use, so a stretched box is correctly a definite-height CB for them. `pch = None` (auto-height
parent) leaves it content-sized, at parity with Chrome. min/max-height clamps still apply; the
bottom-margin-collapse (guarded on `own_definite_h.is_none()`) correctly skips a now-definite box.

**WPT / gate.** `css/css-sizing` 253→**341 (+88)** — the `css-sizing/stretch` `block-height-*` mass — with
css-flexbox +1, css-grid/position/normal-flow flat, **HANG/CRASH 0**. Gated by
`height_stretch_fills_definite_parent` (RED→18px), `height_fill_available_fills_definite_parent`,
`height_stretch_in_auto_parent_stays_content`, `height_stretch_is_a_definite_base_for_percentage_child`.
Residue: `width:stretch` in a shrink-to-fit context (float/inline-block/abspos, where `auto` shrinks) still
behaves as `auto` — a separate, smaller mechanism.

## Scrollbar-gutter reservation — `overflow-y:scroll` narrows the content box (tick 155)

**Symptom.** Every `scrollbar-gutter-001` `offsetWidth` case failed: a 200px `overflow-y:scroll`
container gave its `width:100%` child a **200px** used width, where Chrome gives ~185 — the child
should be narrower than the container by the vertical scrollbar's width. The daily-driver face: the
ubiquitous `html{overflow-y:scroll}` idiom (reserve a scrollbar on every page so navigating between a
short and a tall page doesn't shift the layout) rendered content ~15px too wide, so every centered
container sat off-centre by half a scrollbar.

**Cause.** A classic (space-taking) vertical scrollbar lives on the inline-end edge and eats inline
width, but layout laid children out across the box's *full* content width — no scrollbar was ever
reserved. Compounding it, `ComputedStyle` collapsed `overflow-x`/`overflow-y` into a single
more-clipping `overflow` field, so `overflow-x:auto; overflow-y:scroll` (the test's base) read back as
`auto` and lost the fact that the *vertical* axis force-shows a scrollbar.

**Fix.** Keep `overflow_x`/`overflow_y` per-axis on `ComputedStyle` alongside the collapsed `overflow`
(clip/BFC logic unchanged — no regression there); `stylo_map` and the hand parser both set them. In
`layout_block`, reserve an inline gutter of `SCROLLBAR_WIDTH` (15px) when `overflow_y == Scroll` — the
one deterministic case where a classic scrollbar is *always* present. The gutter narrows only the
content width handed to children and the BFC float band (`inner_width = width − gutter`); `width` and
`border_box_w` — the box's own `offsetWidth` — are untouched, so the container stays 200 while the
child becomes 185.

**Scope / residue.** Only `overflow-y:scroll` (deterministic). The `overflow:auto`-and-*actually*-
overflows case needs a second layout pass (reserve, re-flow, re-measure) and stays residue — matching
Chrome, an `auto` pane that fits reserves nothing. `scrollbar-gutter: stable`/`both-edges` is
**unreachable**: crates.io stylo 0.19 has no `scrollbar-gutter` support at all (it is dropped at parse),
so those keyword cases can't be modeled through the primary CSS path and were left out rather than
adding dead surface. The horizontal-scrollbar-reserves-height axis (`overflow-x:scroll`) and RTL/vertical
writing-mode gutter placement are also residue.

**WPT / gate.** `css/css-overflow` `scrollbar-gutter-001` `overflow scroll, scrollbar-gutter {auto,stable}`
flip green (the two `overflow-y:scroll` rows). Gated by `overflow_y_scroll_reserves_inline_gutter`
(child 200→185, RED on revert), with controls `overflow_visible_reserves_no_gutter` and
`overflow_y_auto_without_overflow_reserves_no_gutter` proving the reservation is scoped to scroll
containers. Full regression sweep across css-overflow/css-sizing/css-flexbox/css-grid/css-position with
**HANG/CRASH 0** and no suite regressing.

## Auto margins center an absolutely-positioned box — the `inset:0; margin:auto` modal idiom (tick 156)

**Symptom.** `position:absolute; inset:0; margin:auto` with a definite `width`/`height` — the canonical
way to center a dialog, modal or backdrop over its containing block — pinned the box to the **top-left
corner** instead of centering it. A 200×200 target in a 400×400 relative CB laid out at `(0,0)` where
Chrome puts it at `(100,100)`.

**Cause.** In `layout_abs`, margins were resolved with `Dim::resolve(cw, 0.0)`, so an `auto` margin fell
straight to **0**. CSS2 §10.3.7 (inline) / §10.6.4 (block) say that when an axis is *fully constrained* —
both insets set **and** a definite size — the leftover free space is distributed into the auto margins
instead. That distribution step was simply missing; the box therefore sat at `cb.origin + inset`.

**Fix.** After the border box is known, redistribute per axis. Inline: when `left` and `right` are both
set and `width != auto` (a definite size, not the stretch-to-fill case), `free = cw − left − right −
border_box_w`; **both** margins auto → `free/2` each (negative free in ltr → start margin 0, overflow
past the end edge); a **start** (`margin-left`) auto → `free − margin-right` (it repositions the box); an
**end** (`margin-right`) auto or neither auto → no-op, because the box is already pinned by
`left`+`margin-left` and an end margin only absorbs slack. The block axis is symmetric on
`top`/`bottom`/`height`/`margin-top`. The `!= auto` guard is load-bearing: it excludes both the
stretch-to-fill case (`width:auto` between two insets, where auto margins are correctly 0) **and** an
intrinsic keyword (`fit-content`/`min`/`max`, which collapses to `Dim::Auto`), so neither is mistaken for
a definite size.

**Scope / residue.** Static centering only. The sibling WPT subtest *"margin:0 auto on abspos resolves
correctly after **dynamic** inset change"* still fails — not a layout-math gap but a **dynamic-reflow**
one: it mutates `.style.inset` from JS and reads back `offsetTop`, which needs abspos re-layout on
inline-style mutation (a separate mechanism). The `margin:auto` (both-axes) sibling passes even without
reflow because its centered offset is inset-independent. Writing-mode-aware start-edge selection (the
negative-overflow branch assumes ltr/ttb) is also residue.

**WPT / gate.** `css/css-position` **76 → 79 (+3)**; *"margin:auto on abspos resolves correctly after
dynamic inset change"* flips green. Gated by `abspos_auto_margins_center_a_constrained_box` (a 200×200
`inset:0;margin:auto` box centers at `(100,100)`, RED at `(0,0)` on revert; a `margin:0 auto` control
proves the two axes resolve independently — inline centered, block pinned). Regression sweep across
css-position/css-flexbox/css-grid/css-sizing/css-values/css-overflow: **all flat, HANG/CRASH 0**.

## `min-width`/`max-width`/`min-height`/`max-height` clamp an absolutely-positioned box (tick 157)

**Symptom.** `layout_abs` computed a used width/height and never clamped it — a `max-width:200px` dialog
that specified `width:500px` came out 500 wide; a `min-width` tooltip, a `max-height` scroll panel, all
took their unconstrained size. The in-flow block path has always clamped (lib.rs §min-width/max-width);
the abspos path simply never grew the same three lines.

**Cause.** The abspos width/height arms (definite / stretch-between-insets / aspect-transfer /
shrink-to-fit) each produced a size, but there was **no `min_*`/`max_*` step at all** — the four
`ComputedStyle` fields were dead on this code path.

**Fix.** Mirror the block clamp on both axes. Width: after the `content_w` arm, `min_w =
min_width.resolve(cw) − bs_extra_w`, `max_w = max_width.resolve(cw)` (`auto` → ∞), then
`content_w.min(max_w).max(min_w)` — clamped **before** `layout_children` so children see the constrained
width. Height: after `content_height` is resolved, the same against `cb.height` (which is always definite
for an abspos CB, so a `%` bound resolves against it — no indefinite-parent `none` case). Max applied
first, then min wins, both converted to the content box via the existing `bs_extra_*` (box-sizing) deltas.

**Scope / residue.** Clamps only — it does not add **replaced-element intrinsic sizing**. The 30
remaining `position-absolute-replaced-minmax` iframe rows still fail: an empty abspos `<iframe>` needs its
300×150 default intrinsic size *before* the clamp table applies, and Manuk shrink-to-fits it to ~0 (a
separate mechanism). The over-constrained interaction (an `auto`-height box stretched between two insets
then clamped by `max-height`, where the freed space should re-open the bottom inset / auto margins) uses
the simple block-style clamp rather than CSS2 §10.6's full re-solve — matching the in-flow path.

**WPT / gate.** `css/css-position` **79 → 88 (+9)**; the explicit-size min/max rows of
`position-absolute-replaced-minmax`, `position-absolute-*-minmax` and the abspos min/max table cases flip.
Gated by `abspos_min_max_size_clamps_apply` (500→200 max-width, 50→150 min-width, 500→80 max-height; RED
unclamped on revert). Regression sweep across css-position/css-flexbox/css-grid/css-sizing/css-values/
css-overflow: **all flat, HANG/CRASH 0**.

## `overflow-x:scroll` reserves a horizontal-scrollbar gutter — block-axis mirror (tick 158)

**Symptom.** Tick 155 taught `layout_block` to reserve a classic vertical scrollbar's inline width for
`overflow-y:scroll` (narrowing `inner_width`), but the block axis was untouched. An `overflow-x:scroll`
pane's horizontal scrollbar lives on the block-end edge and eats block-axis space, yet children were laid
out across the box's FULL content height — so a `height:100%` child ran 15px into the scrollbar strip.

**Fix.** Mirror the inline gutter. `gutter_x = SCROLLBAR_WIDTH` when `overflow_x == Overflow::Scroll`,
subtracted from the definite content height passed to children:
`inner_definite_h = own_definite_h.map(|h| (h - gutter_x).max(0.0))`. Applied at BOTH `layout_children`
call sites (BFC root and shared-float). Crucially guarded by *definiteness*: `own_definite_h` is `Some`
only when the box has a resolved height, so an auto-height `overflow-x:scroll` box (the common case)
reserves nothing and grows to its content as before. `content_height` (and thus `border_box_h` /
`offsetHeight`) still uses the full `own_definite_h` — only the space *offered to children* shrinks, so
the reserved strip is exactly where the scrollbar renders. CSS Overflow 4 §3.2, block axis.

**Scope / residue.** Deterministic case only — `overflow-x:scroll` always shows a scrollbar. The
`overflow-x:auto`-and-actually-overflows case needs a second layout pass to know a scrollbar appeared and
stays unreserved (same as the inline `auto` case). RTL / vertical-writing-mode gutter-edge selection is
unchanged. Symmetric with the inline reservation, so a box with both `overflow-x:scroll` and
`overflow-y:scroll` reserves on both axes independently.

**WPT / gate.** `css/css-overflow` **132 → 136 (+4)**. Gated by
`overflow_x_scroll_reserves_block_gutter_only_when_height_definite` (a 200px-tall box gives its
`height:100%` child 185 while offsetHeight stays 200; an auto-height control's 40px child stays 40; RED
before at child 200). Regression sweep across css-position/css-sizing/css-flexbox/css-grid/css-values/
css-display: **all flat, HANG/CRASH 0**; full manuk-layout suite 72/72.
