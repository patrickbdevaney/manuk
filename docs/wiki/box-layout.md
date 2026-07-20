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

## object-fit — a replaced image fits its box without distorting (tick 181)

`object-fit: cover` is the card-grid thumbnail idiom — `img { width:100%; height:100%;
object-fit:cover }` — so a photo fills its tile without distorting, cropping the overflow. It was
**completely unimplemented** (0 hits in the engine): the replaced-image blit stretched the decoded
bitmap to fill the box, so every non-square photo in a square tile came out squashed to the tile's
ratio. This is one of the most common rendering bugs a real page would show.

Three-crate mechanism, each layer minimal:
- **css** — `ObjectFit` enum (`Fill` default / `Contain` / `Cover` / `None` / `ScaleDown`), parsed
  from the `object-fit` property into `Style::object_fit`, and recovered from MinimalCascade on the
  shipping **Stylo** path (same recovery block as background-size — Stylo's servo build models it as a
  generic type we would otherwise have to consume).
- **layout** — `object_fit` carried on `LayoutBox` (populated at every construction site alongside
  `background_size`). No layout-math change: the used box is unchanged; only how the bitmap fills it.
- **paint** — `object_fit_geometry(fit, box, img_w, img_h) -> (dest_rect, content_clip)` at
  display-list build. `fill` returns the box (stretch, unchanged). The aspect-preserving modes scale
  the bitmap (contain = min fit-scale, cover = max, none = 1.0, scale-down = min(contain,1)) and center
  it (`object-position: 50% 50%`, the default). `cover`/`none` can exceed the box, so
  `DisplayItem::Image` gained a `content_clip`; the paint walk intersects it with any ancestor overflow
  clip before blitting, so the overflow is cropped to the tile.

**Gate.** `object_fit_preserves_aspect_ratio` (engine/paint): a 200×100 (2:1) photo in a 100×100 tile —
`fill` → dest 100×100 no clip; `cover` → dest 200×100 + a 100×100 crop box; `contain` → dest 100×50,
letterboxed, no clip. RED against the stretch baseline, which reports 100×100 for cover. css+layout+
paint suites green; HANG/CRASH 0. Residue: explicit `object-position` (only the 50% 50% default is
applied); `object-fit` on `<video>`/`<canvas>` follows the same path once those decode; `none` uses
raw bitmap pixels (approximate at devicePixelRatio ≠ 1). [[box-layout]]

## object-position — placing the fitted image within its box (tick 185)

`object-fit:cover`/`none` (tick 181) scales a replaced image to overflow its box and crops the excess,
but hardcoded the crop to the CENTRE (`object-position: 50% 50%`). Pages override that constantly to
keep a subject in frame — `object-position: top` on a portrait avatar so the face survives the crop,
`object-position: right` / `20% 50%` on a banner — and without it the wrong slice of every non-centered
cropped image shows.

Mechanism (css + layout + paint):
- **css** — `ObjectPosition { x: f32, y: f32 }` (0..1 free-space fractions, default `0.5/0.5`), parsed
  from `object-position`: 1–2 values, each a keyword (`left`/`center`/`right`, `top`/`center`/`bottom`)
  or a percentage → a fraction; `top`/`bottom` bind the vertical axis and `left`/`right` the horizontal,
  so `top left` resolves as well as `left top`. Non-inherited (a box property like `object-fit`),
  recovered from MinimalCascade on the shipping **Stylo** path.
- **layout** — carried on `LayoutBox::object_position` beside `object_fit` (no layout-math change).
- **paint** — `object_fit_geometry` distributes the free space `(box − dest)` — which is NEGATIVE (an
  overflow) for `cover`/`none` — by the per-axis fraction: `x = box.x + (bw − dw)·pos.x` (and y), so
  `0` pins the start edge, `1` the end, `0.5` centres. The crop clip is unchanged (still the box).

**Safety.** The default `0.5/0.5` reproduces tick 181's centering to the float, so every existing image
is byte-identical and the ratchet cannot regress; only an explicit `object-position` moves anything.

**Gate.** `object_position_places_cropped_image` (engine/paint): a 2:1 photo in a 100×100
`object-fit:cover` tile overflows 100px horizontally — `left` pins the dest at box.x, `50% 50%` sits 50px
left of that, `right` 100px left; `0%` == `left`. RED vs the hardcoded-center baseline (all three equal).
css+layout+paint green (paint 10→11), HANG/CRASH 0. Residue: `px`-length object-position (a length can't
become a fraction without the box size — falls back to centred), and the 3–4-value edge-offset form.

## box-shadow — a LIST of shadow layers, each with spread (tick 189)

`box-shadow` is a comma-separated **list** of shadow layers, and each layer has a `spread` radius —
but the engine modelled it as a single `Option<BoxShadow>` with no spread, taking only the first layer
and dropping the rest. That renders *every* modern elevation wrong: **Tailwind's `shadow`, `shadow-md`,
`shadow-lg`, `shadow-xl` are all two stacked layers**, the second tightened with a *negative* spread
(`shadow-md` = `0 4px 6px -1px …, 0 2px 4px -2px …`). One un-spread layer is a different, flatter shadow.

Mechanism (css + layout + paint, + the Stylo map):
- **css** — `BoxShadow` gains `spread: f32` and `inset: bool`; `ComputedStyle.box_shadow:
  Option<BoxShadow>` becomes `box_shadows: Vec<BoxShadow>`. `parse_box_shadows` splits on *top-level*
  commas (commas inside `rgba()` don't separate layers), and per layer reads `[inset] dx dy [blur
  [spread]] [color]` — a layer missing dx/dy is dropped, not the whole value.
- **Stylo map** (`stylo_map.rs`) — maps Stylo's own `clone_box_shadow().0` to the **full** layer list
  (was `.find(|sh| !sh.inset)` → one layer): `spread: sh.spread.px()`, `inset: sh.inset`, in source
  order. This is the shipping path, so real pages get every layer with correct selector matching.
- **Stylo engine** (`stylo_engine.rs`) — only falls back to MinimalCascade's parse **when Stylo left
  the list empty** (`if cs.box_shadows.is_empty()`), never overwriting a shadow Stylo resolved.
- **layout** — `LayoutBox::shadow` → `shadows: Vec<BoxShadow>` (clone, not Copy; ~12 construction sites).
- **paint** — iterate the list in **reverse** (source order = first layer on top, so it must paint
  last), skip `inset` layers (inner painting not built — an inset-only shadow honestly paints nothing,
  as before), and inflate each shadow rect by `spread` before offset/blur:
  `x = rect.x + dx − spread`, `width = (rect.width + 2·spread).max(0)`.

**Safety.** An empty list reproduces the old `None` (no shadow); a single outer layer with `spread: 0`
inflates by nothing and offsets identically, so every existing single-shadow render is byte-for-byte
unchanged. Behaviour changes only when a value actually has a second layer, a spread, or `inset`.

**Gate.** `box_shadow_is_a_list_with_spread` (engine/paint): a two-layer `box-shadow` emits **two**
Shadow items (old model: one); `spread:10px` inflates a 100×40 shadow rect to 120×60; an inset-only
shadow paints nothing. RED against the single-shadow/no-spread baseline. css+layout+paint green,
HANG/CRASH 0. Residue: `inset` painting (an inner shadow clipped inside the box), and per-layer
blur that differs from tiny-skia's single-pass gaussian at large radii. [[box-layout]]


## background-image — a LIST of layers, painted back-to-front (tick 190)

`background-image` is a comma-separated **list** of layers painted back-to-front — the **first** layer
sits on top — but the engine modelled it as a single `Option<BackgroundImage>`, and worse, the parser
scanned for `url(` **first**. So the single most common layered pattern on the modern web — a darkening
scrim over a hero photo, `background: linear-gradient(rgba(0,0,0,.5), rgba(0,0,0,.5)), url(hero.jpg)` —
returned **only** the url and silently dropped the overlay. Every hero/banner with text over a photo
rendered the photo at full brightness with the scrim gone, which is exactly the case where white text
becomes unreadable.

Mechanism (css + layout + paint + page):
- **css** — `ComputedStyle.background_image: Option<_>` becomes `background_images: Vec<_>` (source
  order, index 0 = topmost). `parse_background_images` splits the value on **top-level** commas (commas
  inside `linear-gradient(...)` don't separate layers) and parses each piece as one layer via the
  single-layer `parse_background_image`, dropping only unreadable layers rather than the whole value.
- **Stylo engine** (`stylo_engine.rs`) — recovers the **full** layer list from MinimalCascade exactly
  as it did the single image (Stylo's servo build models background-image as a generic type we don't
  consume), so the shipping path renders every layer.
- **layout** — `LayoutBox::background_image` becomes `background_images: Vec<_>` (~10 construction sites).
- **paint** — iterate the layers in **reverse** after `background-color` (last layer painted first =
  bottom; first layer painted last = on top). A gradient paints directly; a `url()` layer blits from
  the per-node bitmap.
- **page** (`fetch_and_apply_background_images`) — takes the **first** url() layer across the list.

**The one-url constraint.** The per-node bitmap map holds **one** decoded image per node, so at most
one `url()` image per element is fetchable — this is the architectural cap. Multiple **gradient**
layers over one photo (the common case) is fully supported; two url() layers on one element is not.

**Safety.** An empty list reproduces the old `None` (no image); a single-layer list — one gradient OR
one url — paints byte-for-byte identically (same item, same order, same node-bitmap path), so every
existing background render is unchanged. Behaviour changes only when a value has two or more layers. The
`bg_is_url` guard that suppresses the replaced-image blit now checks whether **any** layer is a url.

**Gate.** `background_image_is_a_layer_list` (engine/css): `linear-gradient(...), url(x)` parses **two**
layers with the gradient at index 0 (old single-`Option` model: one, and it was the url); a comma
inside a gradient doesn't split; `none` yields no layers. RED against the single-`Option` baseline.
css+layout+paint+page green, HANG/CRASH 0. Residue: one url() image per element (per-node bitmap
keying); per-layer `background-size`/`-repeat`/`-position` still apply to the url layer only. [[box-layout]]


## background-position — placing a background image in its box (tick 191)

`background-position` was unimplemented (0 hits): a `url()` background always painted from the box's
top-left corner. The standard icon/logo/sprite idiom — `background: url(sprite.png) no-repeat;
background-position: -16px -48px` (or `center` / `right bottom`) — showed the **wrong slice** of a
sprite sheet, and a `no-repeat` logo meant to sit centred/bottom-right sat jammed in the corner.

Model: a new `BackgroundPosition { x, y }` where each axis is a `BgPos`:
- `Pct(f32)` — a `<percentage>`/keyword, a fraction of the box's **free space** (`box − tile`):
  `left/top`=0.0, `center`=0.5, `right/bottom`=1.0. This is CSS's "align the p-point of the image with
  the p-point of the box" rule.
- `Px(f32)` — a `<length>`, an **absolute** offset from the top-left.

The two resolve differently, so they stay distinct until the box and tile sizes are known at paint
time. `parse_background_position` reads 1–2 keyword/percentage/length values (one value sets the
horizontal, vertical defaults to `center`; keywords bind to their own axis so `top right` resolves).
The default is `Pct(0.0), Pct(0.0)` = `0% 0%` = top-left.

Mechanism (css + layout + paint, + the Stylo recovery path):
- **css** — `parse_background_position` + a `background-position` property handler; the field lands on
  `ComputedStyle`, recovered from MinimalCascade in `stylo_engine.rs` (Stylo's servo build models it as
  a generic `Position`), so the shipping path places it too.
- **layout** — carried on `LayoutBox` beside `background_size` (~10 construction sites, `Copy`).
- **paint** — the `BackgroundImage` display item gains `position`, and `blit_background` shifts the
  tile origin by `offset = match axis { Pct(f) => f·(box − tile), Px(p) => p }`
  (`lx = fx − rect.x − off_x`), which places a `no-repeat` image and shifts a `repeat` one's tiling
  phase exactly as CSS specifies.

**Safety.** The default `Pct(0,0)` yields offset 0 on both axes — every existing background render (the
fixed top-left blit) is byte-identical, so the ratchet cannot regress. Behaviour changes only when a
value sets a non-default position. Applies to `url()` image layers only; gradients still fill the box.

**Gate.** `background_position_places_the_image` (engine/paint): a 20×20 image in a 100×100 no-repeat
box — default `0% 0%` paints the top-left (bottom-right empty); `right bottom` (`Pct(1,1)`) paints the
bottom-right (top-left empty); `50px 50px` (`Px`) places the slice at `[50,70)`. RED against the
fixed-origin blit. css+layout+paint green (paint 14→15), HANG/CRASH 0. Residue: gradient-layer
position, the 3–4-value edge-offset form (`right 10px bottom 20px`), and per-layer positions for
multi-layer backgrounds. [[box-layout]]


## border-style — dashed / dotted / double borders (tick 192)

`border-style` was **parsed then discarded**: `parse_border_shorthand` used the style keyword only to
default the width, and `ComputedStyle` had no `border_style` field. Every `dashed`/`dotted`/`double`
border rendered **solid** — a drop-zone outline, a ticket-card perforation, a `double` frame, a dashed
divider all came out as a plain line.

Model: a uniform `BorderStyle` (Solid/Dashed/Dotted/Double), stored uniform to match `border_color`
(also uniform — per-side styles are a follow-on). `groove`/`ridge`/`inset`/`outset` collapse to `Solid`
(their bevel is a paint refinement; a solid line is the honest approximation).

Mechanism (css + layout + paint, + the Stylo recovery path):
- **css** — `border_style_of` maps the keyword; `parse_border_shorthand` returns the style alongside
  width/color; `border`/`border-<side>` set it; the `border-style`/`border-<side>-style` longhands take
  the first style token (`none`/`hidden` still zero the width). Recovered from MinimalCascade in
  `stylo_engine.rs`, so the shipping Stylo path renders it.
- **layout** — `Border` gains `style`, set from `s.border_style`.
- **paint** — the per-edge closure dispatches on the style. **Solid** emits one Rect (byte-identical to
  before). **Dashed** breaks the edge into `3×thickness` dashes with equal gaps; **Dotted** into
  one-thickness square dots with one-thickness gaps; **Double** into two `⌊thickness/3⌋` lines at the
  outer edges with a middle gap (below 3px the thirds collapse → reads solid, the honest degradation).

**Safety.** The default `Solid` emits exactly the single Rect per edge the painter drew before — every
existing border is byte-for-byte unchanged, so the ratchet cannot regress. Only a declared
`dashed`/`dotted`/`double` changes.

**Gate.** `border_style_breaks_the_line` (engine/paint): a plain bordered `<div>` (no background) emits
one Rect per edge, so the Rect count separates the styles — `solid`=4, `double`=8, `dashed`/`dotted`≫8.
RED against the all-solid baseline. css+layout+paint green (paint 15→16), HANG/CRASH 0. Residue: per-side
border styles, groove/ridge/inset/outset bevels, exact dash-length fitting. [[box-layout]]


## text-shadow — a shadow behind the glyphs (tick 193)

`text-shadow` was unimplemented (0 hits): the painter drew each text run once, in the text colour. The
readability treatment on hero/heading text — a dark shadow under light text over a photo/gradient, the
raised look on buttons/logos — did nothing, and light-on-light or light-on-image headings lost all
contrast.

Model: a `TextShadow { dx, dy, blur, color }` (Copy; like `BoxShadow` without spread/inset), stored as
`ComputedStyle.text_shadow: Option<_>`, **inherited** (a shadow on a heading flows to its inline spans).

Mechanism (css + layout + paint, + the Stylo recovery path):
- **css** — `parse_text_shadow` reads the FIRST layer (`offset-x offset-y [blur] [color]`; a comma list
  takes the first — multi-shadow is residue); a missing colour defaults to semi-transparent black. The
  field is inherited in `inherit_from` and recovered from MinimalCascade in `stylo_engine.rs` (Stylo's
  servo build models it as a generic list), so the shipping path paints it.
- **layout** — the shadow rides `TextStyle` onto every text fragment (`text_style()` copies
  `cs.text_shadow`; the marker/spacer fragments carry `None`).
- **paint** — `draw_text` factors the glyph loop into a run-painter and calls it twice: once at
  `(dx, dy)` in the shadow colour (BEHIND), then at the origin in the text colour.

**Safety.** The default `None` skips the shadow pass — every existing text render is byte-for-byte the
single main pass it was before, so the ratchet cannot regress. Only authored `text-shadow` changes.

**Gate.** `text_shadow_paints_behind_the_glyphs` (engine/paint): white text on a white canvas paints
~no dark pixels (<10) without a shadow but >60 with `text-shadow: 4px 4px 0 black`. RED against the
no-shadow baseline. css+layout+paint green (paint 16→17), HANG/CRASH 0. Residue: gaussian blur, stacked
shadows, `currentColor` resolution. [[box-layout]]

## Scroll anchoring — the feed stops jumping (tick 203)

Phase-0 finish-line lever 4. A feed loads an image, an ad or the next page of posts **above** the
user's reading position; the document grows there, every following box shifts down, and the line they
were mid-sentence on jumps off the screen. On an infinite feed that fires on every lazy load, which
is why it is one of the most complained-about behaviours on the mobile web and why every engine
implements anchoring.

Two `Page` methods, used around any mutation that may reflow:

- `capture_scroll_anchor(scroll_y) -> Option<ScrollAnchor>` — remember the element at the top of the
  viewport and how far below the top edge it sat.
- `scroll_anchor_delta(&anchor, scroll_y) -> f32` — how far `scroll_y` must move so that element
  stays visually still. `0.0` when nothing moved (the common case) or when the anchor is gone.

**Choosing the anchor is the entire correctness of this, and the obvious choice is wrong.** The
anchor must be the first box that begins **at or below** the viewport's top edge. A box that
*straddles* the top edge — `<body>`, `<html>`, the article container, every ancestor — begins at
`y = 0` and **does not move when content is inserted inside it**, so anchoring to one yields a
correction of exactly zero and the page jumps precisely as if there were no anchoring at all. The
gate caught this: the first implementation preferred the box closest to the top edge by absolute
distance, picked `<body>`, and reported `delta=0` while the read line sat 300px lower.

Nor is the deepest box right: a text run is the thing a reflow is most likely to destroy, and an
anchor that no longer exists corrects nothing.

Gated by `g_scroll_anchor`: with the reader's line at the viewport top, a 300px ad is appended above
it via a real click handler; the gate first asserts the *uncorrected* jump is exactly the inserted
height (so the scenario is real), then that applying the delta restores the line to the same screen
position, then that a relayout changing nothing above the fold produces a correction of **zero** —
anchoring must be inert when nothing moved, or it becomes its own source of drift.

Residue, stated plainly: **`overflow-anchor: none` is not honoured yet** — the property is not parsed,
so anchoring applies unconditionally, and a site that deliberately opted out will still be anchored.
That is a real (if narrow) divergence and it needs a `ComputedStyle` field. Anchoring is also
document-scroll only (not per-`overflow:auto` container), and **the shell does not call it yet** —
wiring it around the relayout paths in `gui.rs` is what makes it live during browsing, and is the
completing step for lever 4.

## Scroll anchoring is live (tick 204) — `with_scroll_anchor`

Tick 203 built the mechanism; nothing called it. `gui.rs::with_scroll_anchor(f)` wraps any operation
that may reflow: capture the anchor, run `f`, then move `scroll_y` by however far the anchor moved.

It wraps the two delivery handlers that can grow the document under the reader —
`PageFetchStream` and `PageWebSocket`. Those are the paths a real feed uses: a lazy image, a late ad,
or the next page of posts arriving over the network and being appended above the reading position.

**The half-pixel threshold is not a fudge.** Anchoring that is not inert when nothing moved becomes
its own source of drift, so a correction under 0.5px is discarded rather than applied. The result is
clamped to `[0, max_scroll]`, because a correction must not scroll past the end of the document.

Gated by `g_scroll_anchor_live`, which does what `with_scroll_anchor` does — capture, deliver,
measure, apply — around the same `deliver_fetch_stream` call, with the ad's height arriving as the
fetch body. The shell has no UI harness (the standing limitation), so this gates the **composition**:
if the mechanism and the delivery path disagreed about when geometry is valid, it fails where the
unit gate passes.

Still open for lever 4: `overflow-anchor: none`. Honouring it means a `ComputedStyle` field fed by
Stylo, which is where the shipping cascade reads from — a bigger change than it looks, and it is the
one remaining honest divergence here: a site that opted out is still anchored.

## The `width`/`height` attributes are an aspect ratio, and a clamp transfers through it (tick 218)

Two gaps that only bite together, and together they broke the most common image markup on the web.

**Gap 1 — the ratio only ever came from a decoded bitmap.** `Page::apply_images` sets
`aspect_ratio` when the pixels arrive. So `<canvas>` and `<video>`, which never decode a bitmap,
had **no ratio at all, ever**, and an `<img>` had none **until it loaded** — which is precisely the
window `<img width="800" height="400">` exists to cover. Those attributes are an
`aspect-ratio: auto 800 / 400` presentational hint (HTML §"dimension attributes"), and reserving the
right-shaped box before the bytes arrive is the whole anti-layout-shift story that Next.js `<Image>`,
WordPress and GitHub all ship. Now set in both cascade paths (`apply_ua_defaults` and
`apply_presentational_hints`), and only into an empty slot — `auto` means a real intrinsic ratio
still wins, so the decode pipeline continues to overwrite it. `iframe`/`embed`/`object` are excluded:
they get the 300x150 default instead, not a ratio.

**Gap 2 — a min/max-width clamp did not transfer through the ratio.** CSS2.1 §10.4: for a *replaced*
element, clamping one axis is a constraint violation and the other axis is recomputed proportionally
— even when it was specified. `layout_block` only derived the height from the ratio when the height
was `auto` (`(None, Some(r))`), so with both axes specified the clamp narrowed the box and left the
height alone. `img { max-width: 100% }` is in every CSS reset on the web, so an 800x400 asset in a
400px column rendered **400x400**: the picture squashed to half its width, at every viewport narrower
than the image. The new arm fires only on an actual violation (`inline_constraint_violated`) and only
for replaced elements (`is_replaced_element` — an ordinary box's specified height stands; only a
replaced box's two axes are tied together by the thing being displayed).

**Measured:** `css/css-sizing` 343 → 395 subtests (20.5% → 23.6%); css-flexbox and css-grid flat;
Bar 0 clean. Gated by `g_replaced_ratio` (end-to-end, shipping stylo+spidermonkey config) and
`dimension_attributes_give_a_replaced_element_its_ratio_before_it_loads` (layout, in the wall's
`manuk-layout` suite). Both proven RED **two independent ways** — disabling the transfer and
disabling the attribute hint each yield `400x400`, which is exactly the squashed render.

**Residue:** only the width→height direction transfers. A `max-height` clamp does not yet push back
into the width (CSS2.1 §10.4's other half), and the full ten-case constraint table — where both axes
violate at once — is approximated by the single pass.

### The instrument note that outranks the above

`/home/patrickd/wpt` is a **sparse checkout with no `fonts/` directory**, so `@import
"/fonts/ahem.css"` 404s and every Ahem-based layout test measures in a fallback font. Ahem's whole
purpose is that each glyph is exactly 1em square, which is what makes `data-expected-width` legible;
without it those assertions cannot pass no matter how correct the layout is. This is not a small
tail: **838 of the css-grid files reference Ahem**, plus 93 in css-flexbox and 40 in css-sizing. Any
read of "css-grid is at 9.6%" has to be discounted by that. Corpus fixture, observer-owned — recorded
here and in the journal rather than fixed from inside a tick.

## `width: stretch` was thrown away, and it only mattered where `auto` does not fill (tick 219)

`stretch` / `-webkit-fill-available` / `-moz-available` reached layout as plain `Dim::Auto`. On an
ordinary block box that is the *right answer* — `auto` fills there too — and that equivalence is
precisely what hid the gap for so long, because it holds for the one box shape where it does not
matter. Every box that **shrink-to-fits on `auto`** diverged: a float, an inline-block, a form
control, a replaced element, and an abspos box without both insets. `height_stretch` had existed
since tick 154; this is its inline mirror.

**Four consumers, because there are four places a width is decided:** the block/inline-block path
(`layout_block`), the float path, the abspos path (`layout_abs`), and — the one that took the longest
to find — the replaced-element **aspect-ratio mirror**, which derived `height x ratio` straight over
the top of the stretched width and kept a `width:stretch` `<canvas width="40" height="20">` at 40px.

**The second half is a precedence rule, and it generalises past `stretch`.** A UA default and an HTML
presentational hint are the two lowest-priority sources of a width, so both may only fill a
*genuinely absent* one. Every such site tested `s.width == Dim::Auto` — and `stretch` and the
intrinsic keywords **compute to `Dim::Auto`**, so they read as absent. `<canvas width="40">`,
`<input size=20>` and `<textarea cols=20>` each beat the author's declaration. The flags
(`width_stretch`, `width_keyword`) are what tell "no width was specified" apart from "a width was
specified that resolves later", and the guard is now on all of them.

**Measured:** `css/css-sizing` 395 → 407 (23.6% → 24.3%); css-flexbox and css-position flat; Bar 0
clean. Gated by `g_width_stretch` — six boxes at `170px` (a 200px container less 30px of margin) plus
a `width:auto` control that must **still** hug, so a change that simply made everything fill fails.
RED two independent ways: dropping the cascade flag collapses all five (`50/18/50/10`), and dropping
only the block-path arm collapses exactly the two it owns while the float and abspos arms still fill
— which also demonstrates the four consumers are genuinely independent.

**Residue:** an abspos box with **no** inset at all produces no box whatsoever (found while building
this gate — pre-existing, unrelated to `stretch`, and the reason the gate uses `left:0`).
Logical `inset-inline-start`/`-end` are likewise unmapped, which is the rest of the stretch suite.

## The static position of an out-of-flow box (and how it goes missing)

`position: absolute` with all-`auto` insets does **not** go to its containing block's origin. It goes
to its **static position** — the spot it would have occupied had it stayed in normal flow. That spot
exists for exactly one instant: while flow layout walks past the box. Nothing later can reconstruct
it, so flow records it (`Ctx::static_pos`) and `position_absolutes` reads it back.

The consequence of a miss is severe and asymmetric. `position_absolutes` treats "no recorded static
position" as unplaceable and `continue`s, so the box **generates no box at all** — it does not render
in the wrong place, it renders nowhere. Any layout path that returns *before* the child walk must
therefore record the static position of its out-of-flow children on the way out.

Two paths return early:

- the **pure inline formatting context** branch (`!has_block` and no floats). Out-of-flow children
  are filtered out of `flow_kids`, so a parent whose only children are out-of-flow has none left,
  takes this branch, and used to lose them. It records `(cx, cy)` now.
- **flex** and **grid**, which place their abs children through their own machinery.

The failing shape was `position: relative` wrapping *only* an absolutely positioned child — the
overlay / dropdown / tooltip / portal-root idiom. It hid because every neighbouring case is fine: one
block-level sibling is enough to route the parent onto the block path, which always recorded
correctly. When debugging a vanished absolutely positioned element, the first question is therefore
**what formatting context does its parent establish**, not what the box's own style says.

## Where a replaced element's size comes from (three channels, and they must agree)

An image's used size can be decided in three places, and the bugs come from one of them not knowing
what the others know:

1. **The `width`/`height` attributes** — a presentational hint, and also an aspect-ratio hint. Lowest
   priority: it may only fill a genuinely absent width (see the `stretch` note above).
2. **The decoded bytes** — the natural size. `apply_natural_size` records the *ratio* and only pins an
   axis when both are `auto`. Pinning the natural height outright is wrong: a `max-width:100%` clamp
   then narrows the box and leaves the height alone, and the image renders stretched. That reset is
   on essentially every site on the web.
3. **The formatting context** — block, flex or grid decides the used value from whichever axis is
   definite.

The two failures worth remembering both come from a channel being starved:

- **Sizing that only exists on the async path.** Decoding used to happen exclusively in the
  subresource pass, so a `data:` image — which carries its own bytes and has nothing to wait for —
  laid out `0x0` on every path that does not run that pass. Inline images are decoded before the
  first layout now (`decode_inline_images`).
- **A ratio the layout engine cannot see.** The block path derives an `auto` axis through
  `ComputedStyle::aspect_ratio`, but flex and grid items are sized by taffy, and `to_taffy_style` did
  not pass the ratio along. An image with only a `height` came out **zero pixels wide** — present,
  laid out, invisible. Any value the block path uses to derive a size has to cross into
  `to_taffy_style`, or it silently does not exist inside flex and grid.

## `overflow` is two properties, and layout's copy is lossy

`ComputedStyle` keeps three overflow values and they answer different questions:

- `overflow_x` / `overflow_y` — the real per-axis computed values. CSS Overflow §3 applies: a
  `visible` paired with a non-`visible` **computes to `auto`**, so setting one axis silently changes
  what the other reads back.
- `overflow` — the *more-clipping* of the two, kept for layout's single clip rect.

The third is a lossy summary and must never be what a script reads: `overflow-x: hidden; overflow-y:
scroll` collapses to one keyword, and the axis that actually scrolls cannot be recovered from it.
`getComputedStyle` therefore serializes the axes, and the shorthand renders as one value when they
agree and two when they differ (the CSSOM shorthand-serialization rule).

This matters because of one specific walk: **finding the scroll container** by climbing ancestors and
testing `overflowY`/`overflowX` for `auto|scroll`. Dropdowns, modals, virtualised lists and
scroll-into-view all do it. If the property reads `undefined` the walk silently matches nothing and
falls through to the document — the popup anchors to the viewport instead of its container, and the
DOM looks perfectly fine.

## Bare text inside a flex/grid container is an ITEM, and filtering children to elements deletes it

`flex_items` collected only elements, so a text run sitting directly inside `display:flex` never
became a box. Not mispositioned — **absent**. Measured against Chrome:
`<div style="display:flex;width:max-content">Recent changes</div>` is **154×21 in Chrome and was
2×2 here**, and the icon+label form (`<i>*</i>Recent changes`) came back **8px wide against Chrome's
160** — an element item laid out, so a box existed and looked plausible while the label was gone.

Flexbox §4 / Grid §6: each contiguous run of child text is wrapped in an **anonymous block-level
item**. White-space-only runs are not (otherwise the newline between two children takes a slot).

**The visible symptom is not a missing label — it is a wrapped one.** A shrink-to-fit container
whose text is dropped collapses to the widest remaining thing, so every sibling label re-wraps to two
lines and each one silently doubles in height. The page below it drifts. That is why this reads as a
vertical-placement bug and gets investigated as font metrics.

### The item's style cannot be read off the text node — THE TWO CASCADES DISAGREE

The text node itself serves as the item (no synthetic node needed), but its *stored* style is not
usable, and which way it is wrong depends on which cascade ran:

| cascade | what a text node holds |
|---|---|
| `MinimalCascade` | `inherit_from(parent)` — non-inherited props already at initial values |
| Stylo (`cascade_via_stylo`) | a **full clone of the parent's computed style** |

Under Stylo the clone carries `display:flex`, so the anonymous item is taken for a flex *container*,
recurses into a text node's empty child list, and collapses to zero — the original bug wearing a
different hat — besides re-applying the parent's width, padding and background. It also makes
`max_content_width` route a text node into the taffy path, whose leaf measure lands back in
`max_content_width`: **unbounded recursion, not a wrong number**.

So the anonymous-box contract is *synthesised* at the three seams (taffy style, max-content, box
extraction) rather than read from either cascade. Only genuinely inherited properties —
`visibility`, folded `opacity`, font, `text-align` — are taken from the node, because those two
cascades do agree there. Cf. [[two-cascades-stale-source-of-truth]]: the fix that trusts one
cascade's representation is the fix that breaks when the other one runs.

**Honest scope.** This is a real, Chrome-exact fix (100% placement on the probe, all four shapes),
and it did **not** move Wikipedia — whose sidebar labels are wrapped in `<span>`s, so no anonymous
item is involved. The sidebar's 93px-vs-186px narrowing is a separate, still-open cause.

---

## `position:absolute` + intrinsic width keywords (tick 274)

`layout_abspos` resolved width through arms for `stretch`, both-insets and aspect-ratio transfer,
then fell through to shrink-to-fit — with **no arm for `s.width_keyword`**, the field carrying
`min-content` / `max-content` / `fit-content`. The in-flow block path had had one all along, so the
two paths disagreed about what an intrinsic keyword means.

Shrink-to-fit sizes against the **containing block**, and for an absolutely-positioned panel that is
the nearest positioned ancestor — the trigger it hangs off, which for a dropdown is an icon button
about 20px wide. So `width:max-content` on an anchored panel resolved to roughly half the content
width instead of the content width.

```
                   Chrome    before    after
abspos max-content   180       114       180
static max-content   180       180       180   ← control
```

**The diagnostic shape to remember:** the failure presents as *vertical* drift. The panel renders,
at about half width, so every row wraps to two lines, each wrap adds a line box, and the accumulated
height pushes everything below down. A fidelity sweep reports `mdx=0, mdy=45` and the next tick goes
looking for a margin or a line-height. A median offset cannot say the cause is a width; per-element
boxes plus a `position:static` control in the same file can.

**And on a right-anchored box, `dx = -dw`.** A dump reading `cx=778 cw=150 · mx=823 mw=105 · dx=45
dw=-45` looks like an x error *and* a width error. `778+150 = 823+105`: the right edges agree
exactly, there is one bug, and fixing the width fixes both columns.
