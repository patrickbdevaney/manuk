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
escape. Clearance on the first child declines the collapse. `overflow:hidden`/`auto`/`scroll` — the
card/clearfix margin-containing idiom — deliberately does **not** collapse.

> ⚠ **SUPERSEDED (tick 859).** This paragraph used to also read *"or a leading/trailing out-of-flow
> (float/abs) child, declines the collapse (**conservative: never wrong**, occasionally incomplete)"*.
> It was wrong, and the phrase "never wrong" is why it stood for 700 ticks — a rule that declines a
> collapse leaves a **visible gap**, so it has no safe direction to fail in. See the section below.

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

## A float is not the FIRST IN-FLOW CHILD — it is SKIPPED, and treating it as a terminator cancelled the collapse (tick 859)

CSS 2.1 §8.3.1 collapses a box's top margin with its first **in-flow** child's. A float and an
absolutely-positioned box are, by definition, **not in-flow children** — so the search steps over them
and the block *after* them is the first in-flow child. All four §8.3.1 search helpers
(`collapse_through_top`, `collapse_through_bottom`, `leading_block_collapse_top`,
`trailing_block_collapse_bottom`) instead **returned** on one, and the comment called that
"conservative".

**There is no conservative direction here.** Declining a collapse leaves the child's margin *inside*
the parent — a band of parent background above the first paragraph, and a parent that much too tall.
It is not a cautious no-op; it is the bug, spelled the other way.

**Chrome-measured** (`/tmp/mc.html`, 800px, `body{margin:0}`, `p{margin:15px 0}`), reading the parent's
`y` and the first `<p>`'s `y`:

| first child of the parent | Chrome | reading |
|---|---|---|
| `<div style="float:right">` | parent `15`, p `15` | collapsed **through** the float |
| `<div style="position:absolute">` | parent `68`, p `68` | collapsed **through** the abspos box |
| a text node | parent `159`, p `192` | **not** collapsed — real inline content does separate |

A trailing float behaves the same on the bottom edge: the last `<p>`'s bottom margin escapes past it.

**The two halves of the engine already disagreed with each other**, which is the tell worth keeping.
`layout_children`'s placement loop clears `first_block` only for a *block-level* child, so a float
never counted there — the placement was already Chrome-correct and only the *hoist computation*
bailed out. When one mechanism is implemented in two places, check whether they agree before deciding
which is wrong: here the two answers were different and the shorter one was right.

**Why it is worth a tick.** `<div class=illu style="float:right"><img></div>` followed by prose is the
pull-quote / article-figure / sidebar-thumbnail idiom, and on `kicktipp.com` it cost a **reading-order
inversion**, not just a gap: Chrome reads the prose first (both at `y=0`, prose at `x=0`), we read the
float first because we alone pushed the prose down 15px. Measured, same file, same hour, against the
tick-858 binary:

```text
                        OLD (t858)        NEW (t859)
  kicktipp.com          ro 1  shape 85.3%  →  ro 0  shape 87.4%
  possssno.sbs          ro 1  shape 89.7%  →  ro 1  shape 89.7%   (3 interleaved pairs, identical)
  marktplaats.nl        ro 1  shape 95.2%  →  ro 1  shape 95.2%
  ubys.bingol.edu.tr    ro 1  shape 92.8%  →  ro 1  shape 92.8%
  wikipedia / HN / a11yproject / blog.rust-lang / martinfowler   byte-identical on every term
```

**Gate.** `an_out_of_flow_first_child_does_not_cancel_the_parent_child_margin_collapse` (float case,
abspos case, and the *text*-first guard that keeps the fix from becoming "collapse through anything")
plus `a_trailing_float_does_not_cancel_the_bottom_margin_collapse`. RED-proven by restoring the
`return 0.0` arm in `leading_block_collapse_top`.

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

## The same keywords on the four min/max properties were UNREPRESENTABLE — and the sidecar is the fix (tick 930)

**Symptom.** `min-content`/`max-content`/`fit-content` resolved exactly on `width` and `height`
(tick 146/153 above) and were **silently dropped** on `min-width`, `max-width`, `min-height` and
`max-height` — twelve of a twenty-case Chrome differential, with every control exact:

```text
  "hello there world"   min-content 48.17 · max-content 163.77 · 400px CB unless noted
                                          Chrome   before   after
  width:min-content         (CONTROL)      48.17     48       48
  max-width:min-content                    48.17    400       48
  max-width:max-content                   163.77    400      164
  min-width:max-content     (20px CB)     163.77     20      164
  min-width:fit-content     (20px CB)      48.17     20       48   <- NOT max-content: the
                                                                      stretch-fit is 20
  width:400px; max-width:min-content       48.17    400       48
  height:200px; max-height:min-content     48       200       48
  height:1px;   min-height:min-content     48         1       48
```

**Cause — a missing representation, not a missing rule.** `Dim` has no intrinsic variant, and the four
min/max properties were plain `Dim` with no keyword sidecar (`width` has had `width_keyword` since
t153). So on **both** cascades the keyword fell through to `Dim::Auto`, which the clamp reads as **0
on a min** and as **no limit on a max**. The declaration parsed to a different, *valid* value and did
nothing. ⚠ The t153 section above ends *"min/max-width clamps still apply after (CSS Sizing L3)"* —
true of a *length* clamp, and it is exactly the sentence that made the gap look closed.

**Fix.** Four more sidecars — `min_width_keyword` / `max_width_keyword` / `min_height_keyword` /
`max_height_keyword`, all `Option<IntrinsicSize>` — parsed on both cascades and consumed by the
min/max clamp in **three** layout paths (`layout_block`, `layout_float`, abspos: each carries its own
copy of §10.4). The inline axis calls the *same* `min_content_width` / `max_content_width` /
`shrink_to_fit` the `width` arm uses, so the Bar-0 and recursion profile is unchanged and there is no
new sizing code. The block axis is **one value, not three**: a box's min-content and max-content
*block* sizes are the same quantity, so all three keywords name the natural content height. Both are
content-box already, so the `box-sizing` conversion must NOT run for them — the same reason it is
skipped for a keyword `width`.

**⚠ `fit-content(<length>)` IS INVALID ON ALL FOUR, and only a measurement says so.** The grammar
reads as though the functional form goes wherever the keyword goes; Chrome drops it —
`min-width:fit-content(50px)` reads back `0px`, `max-width:fit-content(50px)` reads back `none`. The
`width` parser deliberately accepts it, so the min/max arms need a **separate, narrower** parser
(`intrinsic_kw_bare`; `minsize_intrinsic_kw` / `maxsize_intrinsic_kw` on the Stylo side) rather than
the obvious reuse. Sharing one function would make us *more permissive than Chrome* — laying out a box
Chrome does not — and no fixture written from the fix would ever show it.

**The CSSOM half is not optional.** `getComputedStyle(el).maxWidth` returned **`"none"`** — the string
that means *there is no cap* — while the box was capped. Both the physical and the logical spellings
(`max-inline-size` / `max-block-size`) now route through one `max_dim_css`, because
`extra_computed_props` already records catching those two drifting apart once.

**Gate.** `G_INTRINSIC_MIN_MAX` (layout, 14 claims) + `G_INTRINSIC_MIN_MAX_CSSOM` (16 claims). RED-proven
three ways — deleting the `layout_block` arms, deleting **only** the Stylo sidecar (the fixture runs the
shipping cascade, so a `MinimalCascade`-aimed proof would have passed against a broken engine), and
making the serialisers ignore their keyword. Both carry CONTROL rows that a fix taking the new branch
unconditionally fails.

**Bound.** A **flex item's** intrinsic min/max is still dropped — closed the following tick, below.

## A table's SURPLUS height is distributed over its rows — and the spec refuses to say how (tick 933)

**Symptom.** t908 taught the table BOX to grow to its declared height, correctly and gated. Nothing
inside it moved: the rows kept their natural heights and the declared height became **empty space at
the bottom**. The box was right and every row was wrong — a `<td>` that should be 56 tall was 26.

**Why it took four gates to close.** This one algorithm was named from four different doors, each
time pinned at OUR number with the mechanism written beside it:

```text
  t814  g_orphan_table_cell#c3          [0 92 67x20]  vs Chrome [0 90 300x80]
  t908  g_table_height_is_a_minimum#t10 (unasserted)  vs Chrome 196x56
  t925  the border-spacing tick's residue             "a <td> does not STRETCH"
  t932  g_anonymous_table_row#mid       400x24        vs Chrome 400x100
```

All four now assert Chrome. **The practice is the lesson, not the instance**: a residue pinned at our
own number, with its mechanism named and an explicit instruction that a future fix must change the
line deliberately, is what let four independent sightings converge instead of being re-measured a
fifth time.

**CSS 2.1 §17.5.3 declines to specify the distribution** — *"the distribution of the remaining space
is implementation-dependent"* — so the spec cannot be the source and every number below is a
measurement of Chrome.

```text
   200px table, spacing 2 (194 usable)        Chrome       proportional?   equal-share?
     two rows, natural 26 + 26                97 · 97         97 · 97        97 · 97
     two rows, natural 26 + 74             50.4 · 143.6    50.4 · 143.6      73 · 121
     row1 height:100px, row2 natural 26      100 · 94         154 · 40          ✗
```

* **Proportional to natural height, not equal shares.** The two models agree on every *equal-natural*
  row and disagree only where the naturals differ — so a fixture without a lopsided pair passes
  against the wrong algorithm.
* **A row that SPECIFIES a height is EXCLUDED from the surplus**, not merely counted in it. Row1 keeps
  exactly its 100 and row2 absorbs all 70px.
* **A row's own `height` is a MINIMUM on its natural height** — the other half of the fix, and
  unobservable on its own, which is why it is one tick and not two.

**Gate.** `G_TABLE_ROW_HEIGHT_DISTRIBUTION` (13 claims, 2 CONTROLs), RED-proven three ways: delete the
distribution (`#one_c` 56→26); distribute in equal shares (`#uneq_a` 50.4→73); include
specified-height rows (`#spec_a` 100→153.97 — not a rounding argument).

**The CONTROLs are t908's rule and they are what the plausible mis-phrasing breaks.** A fix written as
*"make rows fill the table"* rather than *"distribute the SURPLUS"* shrinks a row whose content is
taller than the declared height and passes everything else.

**Two things deliberately left, with their reasons.** When EVERY row specifies a height the surplus is
left at the bottom — Chrome's behaviour there is **unmeasured**, so the code keeps the pre-existing
behaviour rather than inventing a rule the fixture cannot defend. And a **row BOX is not inset by the
horizontal border-spacing**: Chrome reports a `<tr>` as 196 wide in a 200px table, we report 200 —
a different quantity from row height, moving no cell, and folding it in would have made the three RED
proofs ambiguous.

⚠ **A constant is only measured on the fixture that asserts it.** The gate's first values were carried
over from the *exploratory* fixture, which reset `td { padding: 0 }`; the gate's own fixture uses UA
defaults, so the naturals are 26 and 74 and the split is 50.4/143.6, not 48.5/145.5. Carrying a number
across two fixtures is exactly as unsound as carrying it from memory.

## A bare `table-cell` needs an ANONYMOUS ROW, and without one it does not shrink — it VANISHES (tick 932)

**Symptom.** `width:50%` inside a `display:table` > `display:table-cell` came out **4px against
Chrome's 200** — a percentage resolving against a container that had collapsed from 400px to the 8px
width of the letter it contained. In a second arrangement the cell produced **no box at all**.

**Mechanism.** CSS 2.1 §17.2.1 generates an anonymous table-row around a `table-cell` whose parent is
a `table`. `collect_table_rows` recognised only `table-row` and `table-row-group`, so a bare cell
matched no arm and was dropped. The table then had no rows, took the rowless shrink-to-fit path, and
collapsed onto its own text. The discriminator is exact and was in the first fixture: **with an
explicit `display:table-row` we were already correct.**

**Chrome's semantics, measured — three separate clauses, each with its own fixture row:**

```text
                                            Chrome           before             after
   two bare cells                     200@0 · 200@200     8@0 · 8@8       200@0 · 200@200
   bare · real row · bare             400 · 400 · 400   GONE · 400 · GONE  400 · 400 · 400
   display:table;height:100px + cell     400 wide           8 wide            400 wide
   a bare cell's width:50% child           200                4                 200
   explicit table-row     (CONTROL)        400               400                400
   real <table><tr><td>   (CONTROL)        394               394                394
```

* **Consecutive cells share ONE anonymous row** — side by side at x=0 and x=200, not stacked.
* **A real `table-row` BREAKS the run** — `bare · row · bare` is three rows in document order, which
  is why the accumulator is flushed when a real row or row-group is seen.
* **The anonymous row carries `None` for its node.** It is an anonymous box: no style lookup, so no
  background of its own, and no node on the emitted `LayoutBox`. The consumer already took an
  `Option<NodeId>` (written for the `<tr>`-has-real-geometry fix), so this slots into the shape that
  was already there.

**Why it is not the corner it looks like.** `display:table` + `display:table-cell` with no row between
them is the pre-flexbox **vertical-centring** idiom and the **equal-height-columns** idiom — both
everywhere in the legacy/CMS markup that makes up the CrUX tail. And a 392px container-width error is
burndown family #1 (`PHASE0-RENDER-BURNDOWN.md` §3.1) in its grossest available form: every line
inside re-wraps, so the whole subtree's height is wrong beneath it.

**Gate.** `G_ANONYMOUS_TABLE_ROW` (10 claims, 2 CONTROLs), RED-proven three ways — stop accumulating
bare cells (two cells report 8 and 8); give each bare cell its own row (they report 400 and 400); drop
the flush before a real row (`#run_a` reports 200). The middle one is the plausible wrong fix: it
satisfies "a bare cell is no longer dropped" completely and gets the arrangement wrong.

**Bound — the same missing algorithm through a THIRD door.** A cell does not STRETCH to fill a taller
table: `display:table; height:100px` with one bare cell is 400×**24** against Chrome's 400×**100**.
Width exact, height not, so the `vertical-align:middle` half of the centring idiom still does not
centre. t908 and t925 reached this from real `<table>` markup and t814 from the orphan cell.

**`g_orphan_table_cell` had already named this fix as its missing piece, in advance.** That gate
(t814, the *inline* half of the same idiom) pinned its `#c3` row at OUR `[0 92 67x20]` on purpose,
writing that it needed *"anonymous-row generation inside a real table plus cell stretching"* and that
a future fix would have to change the line deliberately. This was that fix, and it moved **three of
four coordinates onto Chrome** — `[0 90 300x20]` against Chrome's `[0 90 300x80]` — so the residue is
now isolated to the single remaining quantity. The lesson costs nothing to state and would have saved
an hour: **grep the gate corpus for the property before writing the fixture**; a gate header is the
densest form of the wiki there is.

### The negative result from the same sweep, which is worth as much as the fix

The fixture that found this was 25 composed width cases with no hypothesis in it, and **twenty-four
were already Chrome-exact**: nested percentages (50% of 50% of 400 = 100; 33.3333% twice = 44.44), `%`
width and `%` padding on the same box in both box-sizings, border-box with px padding and an
asymmetric border, `width:auto` carrying a full margin/border/padding frame in both box-sizings,
`width:auto` inside a float and inside an inline-block, `%` inside a flex item and a grid item,
`min-width:50%` / `max-width:25%`, `calc(100% - 2em)` at two font sizes, the `max-width + margin:0
auto + padding` page-wrapper idiom in both box-sizings, and `width:100%` inside a padded parent.

**All three real-prose LINE-COUNT probes matched exactly too** (the same paragraph at 317px / 288px /
409px gives 96/96/72 in both engines), so family #3 — sub-pixel advance accumulation flipping a wrap
boundary — did not reproduce at any of the three widths, including the two chosen to sit near one.

So: **family #1's residual mass is NOT in composed block-level width arithmetic**, which the loop had
been assuming for many ticks. Look for it in the box types that opt out of ordinary block sizing —
tables, replaced content, and scroll containers — rather than in the arithmetic itself.

### The scrollbar finding, recorded because it is an INSTRUMENT question and must not be "fixed" here

`width:50%` inside `overflow-y:scroll` measures 193 here. Chrome measures **192.5 with real
scrollbars and 200 with `--hide-scrollbars`** — and `--hide-scrollbars` is what `chrome.rs:949`
passes for every reference render, deliberately, because a visible scrollbar would shrink the layout
viewport and shift every box.

```text
                                    Chrome            Chrome           ours
                                 --hide-scrollbars   real scrollbars
   overflow-y:scroll,  50% child        200             192.5           193
   overflow-y:scroll,  prose            400             385             385
   overflow-y:auto (overflows), 50%     200             192.5           200
```

**Our engine is right and the reference has no scrollbars.** On every `overflow-y:scroll` container we
are 15px narrower than what we are scored against, prose re-wraps, and the line-count error cascades —
family #1's exact mechanism, arising from the instrument rather than the engine. Making the engine
stop reserving gutters would trade real-browser correctness for a score, which is the trade the
ratchet refuses; the reconciliation belongs on the instrument side (a scrollbar width the fidelity
harness can set to 0 to match its own reference). Frequency is modest — 5 of 85 snapshots carry
`overflow(-y):scroll`, none on `html`/`body` — so this is handed on rather than taken.

Separately and genuinely ours: **`overflow-y:auto` that actually overflows should reserve a gutter and
does not** (Chrome-with-scrollbars 192.5, ours 200). The source already documents this as deliberate
residue needing a second layout pass; the measurement now sits beside the comment.

## An intrinsic keyword is UNREPRESENTABLE in taffy, so it must be RESOLVED before the style is built (tick 931)

**Symptom.** t930 taught `ComputedStyle` to hold an intrinsic keyword on all four min/max properties
and taught the block path to honour it, and named "a flex item's intrinsic min/max is still dropped"
as its bound. Measured against Chrome, the bound was **wider than the note recorded**: not only the
four min/max properties, but plain `width: min-content` — whose `width_keyword` sidecar has existed
since t153 — is dropped on a flex item, and on a **grid** item too. Three formatting contexts, one
sidecar, and only one of them ever read it.

```text
   "hello there world", 16px serif: min-content 37.33 · max-content 109.30 · 400px container
                                                    Chrome   before   after
     flex item  width:min-content                    37.33      109      37
     flex item  max-width:min-content                 37.33      109      37
     flex item  min-width:max-content   (20px CB)    109.30       37     109
     flex item  flex:1; max-width:min-content         37.33      400      37
     grid item  width:min-content                     37.33      400      37
     grid item  max-width:min-content                 37.33      400      37
     flex item  padding:0 10px; max-width:min-content 57.33      129      57
```

**Mechanism.** `to_taffy_style` maps `cs.width` through `dimension()`, and an intrinsic width is
stored as `Dim::Auto` **plus a sidecar**. The sidecar never crossed, so every keyword became
`Dimension::Auto` — *"size me from my flex basis"*. A different, valid answer: the wrong answer of the
right type, one formatting context over. The before-state is legible from the numbers alone — 109
(max-content, the flex-basis answer) in a wide container, 37 in a narrow one, 400 when the item also
grew.

**Why not hand taffy the keyword.** taffy 0.12 *can* build a `CompactLength::min_content()`, and
`Dimension::from_raw` will take one. But `Dimension` validates as `LENGTH | PERCENT | AUTO`, so the
flexbox algorithm reads a tag it does not answer — a dependency asked a question outside its grammar,
which is worse than the bug. **Option 3 of the borrowed-engine table** instead: resolve to px through
the measure callback that is already threaded through `TaffyDom` for exactly this purpose. It bottoms
out in the same `measure_intrinsic` the block path's `min_content_width`/`max_content_width` use, so
the two contexts cannot drift apart.

**`box-sizing` has NO effect on an intrinsic keyword** — measured, because the grammar invites the
opposite assumption. With `padding: 0 10px`, Chrome gives a **57.33 border box under `content-box`
AND under `border-box`**. taffy subtracts the frame from `size` under border-box, so the frame is
added back there to land on the same number either way.

**`fit-content` is deliberately left as `Dimension::Auto`, and that is a measurement.** It is
`min(max-content, max(min-content, stretch-fit))`, and the stretch-fit inside a flex line does not
exist when the style is built. taffy's `auto` + `flex-basis: auto` + `flex-shrink` **is** that clamp:
Chrome-exact in a wide container (109.30) and a narrow one (37.33), on `width`, `min-width` and
`max-width` alike. Resolving it would replace a correct answer with a guess.

**Gate.** `G_INTRINSIC_FLEX_GRID` (15 claims, six of them CONTROLs). RED-proven three ways — delete
the resolver call (every fixed row snaps back, all CONTROLs hold); drop the `frame` term (the two
`box-sizing` rows split, 57 vs 37); resolve `fit-content` eagerly to max-content (the
`min-width:fit-content` CONTROL goes 37 → 109).

**Bounds, with their numbers.** (1) The **block axis** on a flex item: `height:200px;
max-height:min-content` measures 200 against Chrome's 18. A block-axis intrinsic size is the content
height *at the item's resolved width*, which does not exist at style-build time — a different
mechanism from the inline axis, where both quantities are answerable with no context at all. (2) An
item that is **itself a flex/grid container**: `display:flex; width:min-content` nested in a flex row
measures 109.30 against Chrome's 37.33. Resolving it re-enters — the measure callback answers a
container's intrinsic width by building a *second* `TaffyDom` for that node, whose `add` reaches the
resolver again on the same node and recurses without bound. **A Bar-0 crash, not a wrong number**;
the `container` guard at the call site is what keeps the recursion profile identical to before.

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

## Horizontal rails — measured against Chrome, and one of them is not a bug (tick 277)

Both real carousel shapes report the right scroll range, and have for a while:

```
                                  Chrome     here
  flex, items may shrink         300/300   300/300
  flex, flex-shrink: 0          1000/300  1000/300
  inline-block + nowrap         1000/300  1000/300
```

The first row is the trap. `display:flex` with default shrink reports **no scrollable width in both
engines, and that is correct** — a flex item defaults to `flex-shrink: 1`, and `min-width: auto`
floors it only at its *min-content* size, so short cards genuinely fit. A carousel that works is one
whose author wrote `flex-shrink: 0`. Reading 300/300 as "rails don't scroll" is how this was filed
as an open defect while it worked.

Gated with **Chrome-measured constants** rather than our own output, because a gate built from your
own numbers is a screenshot of today, not a regression test.

## Replaced elements compute `inline` — atomicity is layout's job, not the cascade's (tick 384)

The computed display of `<img>`/`<canvas>`/`<video>`/`<svg>`/`<object>`/`<embed>`/`<iframe>` is
`inline` (spec + Chrome). Both cascades used to force it to `inline-block` so the box would take
the atomic-inline layout path — a convenience mutation that leaked into every observable surface:
getComputedStyle, the oracle (81 corpus sites diverged on `<img>` alone, 80 on `<svg>`), and any
author check like `getComputedStyle(img).display === 'inline'`.

The contract now: the cascade REPORTS `inline`; layout's `is_atomic_inline_replaced` routes a
replaced inline through the same atomic path as `inline-block` (sized as a block, flowed like a
word, never recursed into as text) at THREE seams — the inline collector (else an `<img>` produces
NO box: it has no text children), block-in-inline blockification (an `<svg>` HAS element children
and must never be split by them), and `inline_contains_block`'s recursion (a replaced inline child
cannot blockify its ancestor). §10.4 ratio adjustment keeps its own narrower replaced list.
Claimed in manuk-layout `an_inline_replaced_element_is_atomic_but_computes_inline` (RED-proven:
severing the collector seam produces a boxless img). E2E: apnews 541→495 divergences, the
inline-block family to zero, jarring counts unchanged.

## `<br>` has geometry — the break that ends a line still owns a box (tick 385)

Chrome reports a zero-width, line-height-tall rect for `<br>` at the end of the line it
terminates; the tick-380 oracle counted our missing one on 64 sites (461 hits). The inline
line-builder treated a Break as pure control flow — it CLOSED the line, and only the empty-line
case (`<br><br>`) left a fragment. Now the non-empty case pushes a zero-width, empty-text
fragment at the pen position (line-height tall, `report_h` set) before closing: no alignment or
justification moves, the element just earns the rect editors and caret libraries read via
`getBoundingClientRect`. `<br>` only — a preserved newline in `pre`/`pre-wrap` also arrives as a
Break but carries its TEXT's owner, which already has geometry from its words. Claimed in
manuk-layout `a_br_on_a_nonempty_line_has_a_zero_width_box` (RED: sever the arm → no rect).
E2E: usa.gov's br rows went missing→geometry-near-miss ([872 52 0×24] vs [912 55 0×17]).

## The default object size lives in USED-size layout (tick 389)

CSS-Images §4.4: a replaced element with no intrinsic dimensions and no ratio-derivable size is
**300×150** — it neither fills its container nor collapses. The icon idiom `<svg viewBox="0 0 24
24">` (no width/height anywhere, sizing left to CSS that may never target it) rendered **784×0**
here: the `Dim::Auto` width fell through to the block fill arm and the height was the (empty)
content height — full-width, invisible, and every icon-only `<button>` collapsed to a dead
target with it (the tick-380 oracle's missing-svg + dead-target families). The fix sits AFTER
author width resolution and the definite-height×ratio derivation (both must win — the tick-153
lesson: never in `apply_ua_defaults`, where it beat author CSS and regressed css-sizing) and
covers `svg | canvas | video | iframe | object | embed`; `<img>` is deliberately excluded — a
sourceless image has no default object size in any browser. The height half fires only when the
WIDTH half did (a `used_default_object_size` flag), so ratio-derived heights are untouched.
Claimed in manuk-layout `an_unsized_svg_gets_the_default_object_size` (was RED at 784×0).
RESIDUE (named): `viewBox` does not yet feed an intrinsic ratio (an `<svg width="48" viewBox>`
derives height only from dimension-attr hints today); SVG internals (path/g) still have no boxes
of their own — that is the svg-geometry organ, not this fix.

## viewBox is an intrinsic ratio — and the default-size model is MEASURED, not recalled (tick 391)

Tick 389's model held one wrong pin: it gave a `viewBox`-only svg the 300×150 default. Headless
Chrome, measured directly (`--dump-dom` + getBoundingClientRect → title), says otherwise:
`<svg viewBox="0 0 24 24">` in a 400px block is **400×400** — with an intrinsic RATIO present,
auto width takes the AVAILABLE width (CSS2 §10.3.2's last-resort rule, which is what the plain
fill arm already computes) and the height follows the ratio (§10.6.2). The full measured model:
no ratio + auto width → 300×150; no ratio + authored width → width×150 (the default object
HEIGHT stands alone); ratio → available-width × ratio. `viewBox` now feeds `aspect_ratio` in
BOTH cascades' hint passes (svg only, empty-slot only — dimension attributes and the decode
pipeline still outrank it). The t389 test was corrected to the measured truth and RED-proven
(viewBox hint severed → the ratio case falls back to 300×150 and fails).

## BUILD SPEC — inline SVG internals: borrow usvg, don't hand-write geometry (tick 393)

The ledger's top actionable family after the t392 re-crawl: MISSING BOX `<path>` (34 sites,
1,658 hits) + `<g>`/`<circle>`/`<rect>` — Chrome gives every SVG child its own box (path data →
fill bounds → viewBox/transform-mapped rect); we lay the `<svg>` out atomically and its subtree
has no geometry at all. Writing a path parser + bezier-extrema + transform stack by hand is the
wrong rung: **resvg/usvg is ALREADY in the tree** (engine/page uses it to decode `<img
src="*.svg">`), and `usvg::Tree` resolves viewBox, `transform` attributes and per-node absolute
bounding boxes as part of its normal parse.

The build (a fresh-context subsystem, per the t371 container-queries precedent):
1. **Serialize** an inline `<svg>` element's DOM subtree back to SVG text (the DOM already holds
   it verbatim; namespace quirks are the known trap — [[namespace-null-xhtml-conflation]]).
2. **Parse with usvg** once per svg element per layout; scale factor = used box / viewBox.
3. **Geometry:** map usvg nodes back to DOM children (usvg preserves `id`; fall back to
   document-order pairing of shape nodes) and emit each `abs_bounding_box()` scaled into the used
   box as that child's rect — path/g/circle/rect/line all covered by ONE mechanism, hittable and
   measurable.
4. **Paint:** hand the resvg-rendered pixmap to the display list at the used size — inline SVGs
   (icons, logos, charts) become VISIBLE, not just measurable. This is the visible half and
   likely the larger parity win.
5. **Gates:** child-rect fixtures measured over headless Chrome (the t391 method — never
   recalled), one RED per mechanism (unmapped id, transform, viewBox scale); paint gate asserts
   non-background pixels inside the svg box for a solid-fill fixture.
Residue to name at build time: `<foreignObject>` (HTML-in-SVG), `<use>` cross-references,
CSS-styled SVG presentation attributes (usvg reads attributes, not our cascade).

Also recorded this tick: **nih.gov segfaulted the crawl** (rc=139, core dumped, no unwind) but
three quiet single-site runs are clean — the load-only/release-only heisenbug profile of the
open [[calc-size-interpolate-size-segfault]] Bar-0. Evidence banked; the prescribed fix context
is a fresh ASAN session, not mid-loop.

### Paint half LANDED (tick 394)

`decode_inline_svgs` (engine/page): each inline `<svg>` subtree → `serialize_outer` (+ injected
`xmlns` — the HTML parser drops it, usvg requires it) → the SAME `decode_svg` usvg/resvg path as
`<img src="*.svg">` → per-node `inline_svg_cache`. Two merge points, both load-bearing:
`apply_images` REPLACES `self.images` every round (the cache re-merges after), and the sync
construction paths (`load`, `from_prefetched` — the shell's path) never reach `apply_images` at
all, so they rasterize explicitly. Natural sizing is deliberately NOT applied — the measured
replaced-sizing model (t389/391) owns the box; the raster paints into it. Pixel-asserted in
G_FIRST_PAINT (`an_inline_svg_paints_its_vectors`, RED-proven: decode severed → white center).
Remaining from the spec: child geometry mapping, stale-raster-on-mutation, render-at-used-size.

### Geometry half LANDED (tick 704) — and the two things the spec got wrong

`engine/page/src/svg_geometry.rs`. Same serialized markup as the raster (one document, two halves),
usvg parsed once per svg per decode, leaves paired to DOM shape elements in document order, applied
**after** layout by `Page::set_root_box` — the svg's *used* box exists only then, and running after
layout is what makes the pass unable to perturb any geometry outside an `<svg>`.

What it replaced, measured on `www.desitales2.com`: every icon `<path>` reported **`0×22`** against
Chrome's `12×12` — a zero-width inline box one line-height tall, because `<path>` computes
`display:inline`, has no text, and CSS gave it the only box that description allows. Not a near miss;
the wrong formatting model. **`www.desitales2.com` SHAPE 61.5% → 70.3%**, VISUAL and COVERAGE
unmoved, `en.wikipedia.org` and `blog.rust-lang.org` unmoved.

⚠ **`getBoundingClientRect` is the DECORATED bounding box — it includes the stroke.** SVG 2 gives an
element two boxes and the two DOM methods return different ones (`getBBox()` = fill/geometry,
`getBoundingClientRect()` = transformed decorated). The oracle probes with the latter, so
`abs_stroke_bounding_box()` is the correct usvg accessor and `abs_bounding_box()` is not. This cost a
measurement to learn because **the motivating site could not tell the two apart**: desitales2's icons
are unstroked, where the boxes are identical, so the fill box matched Chrome *exactly* there (7×12,
10×9, 12×12) and quietly cost **−0.3 SHAPE on `en.wikipedia.org`**, whose icons are stroked. A
fixture that cannot distinguish two candidate rules has not chosen between them.

⚠ **`abs_bounding_box()` is in `Tree::size()` CANVAS space, not `viewBox` units** — the viewBox
transform is already folded into the absolute transforms. `<svg width="12" viewBox="0 0 24 24">` with
a path spanning 6..18 reports `[3 3 6×6]`, not `[6 6 12×12]`. So the only scale left is
`used_box / Tree::size()`, which is exactly 1 whenever the author sized the svg in px.

**The spec's step 3 said "fall back to document-order pairing" — this REFUSES instead.** usvg's tree
is a rendering tree, not the DOM: it drops `<defs>`, expands `<use>`, synthesises groups. A wrong
pairing attributes one shape's bounds to another element, which is a plausible-looking false number —
strictly worse than the honest `0×22` it replaces. So the mapping is emitted only when leaf counts
match exactly, and is refused outright for `<foreignObject>` (it holds real HTML whose boxes CSS owns,
and this pass replaces an svg box's children wholesale) and for a used box whose aspect does not match
the document's (`preserveAspectRatio` letterboxing is unmodelled). A refused svg keeps its old boxes:
a site this cannot map is left exactly as good as it was. Four gates, one per refusal plus the
mechanism.

**Named residue:** `padding`/`border` on the `<svg>` itself (the border box is used as the viewport);
`<use>` cross-references; non-matching `preserveAspectRatio`; stale-geometry-on-mutation (shares the
raster's cache lifetime, so it inherits that residue exactly).

## List markers follow the HTML "ordinal value" algorithm — a running counter, not a sibling index (tick 411)

`list_marker` built each `<li>`'s number from `start + (count of preceding <li> siblings)`, with an
item's own `value` attribute as a local override. That is right only for a plain forward list. It is
silently wrong for the two cases the HTML "ordinal value" algorithm exists to handle:

- **`<ol reversed>`** — ignored entirely. A countdown, a ranking, a "top 10 in reverse" numbered
  `1. 2. 3.` **upward** instead of `N … 1` downward.
- **`<li value="7">` continuation** — a `value` reset the counter for *that item only*. The spec says
  it resets the running counter for **every item after it too**: `<li>x</li><li value=7>y</li><li>z</li>`
  is `1, 7, 8`, not `1, 7, 3`. Any resumed or manually-renumbered list mis-counted from the first
  `value` onward.

The fix replaces the per-item index with a single left-to-right pass over the list items maintaining a
`counter`: it starts at `start` (or, for `reversed` with no `start`, the item count), each `value`
resets it, the marked item takes the counter, and it steps by `±1` (`-1` when `reversed`) between
items. One pass, one source for "what number is this item". Gated by
`list_ordinals_follow_reversed_and_value_continuation`, RED-proven (revert to the index form →
reversed reads `1. 2. 3.` and the value list reads `1. 7. 3.`).

## `justify-content: normal` is NOT `flex-start` — it is what makes an `auto` grid track stretch (tick 569)

Our `JustifyContent` enum had six variants and no `Normal`, so the CSS **initial value** was stored as
`FlexStart` — by both cascades — and handed to taffy as a concrete `JustifyContent::FLEX_START`. That reads
like a harmless normalisation. It is not, because `normal` is the one value whose meaning is **decided by the
formatting context**:

- in a **flex** container it behaves as `flex-start`;
- in a **grid** container it behaves as **`stretch`**.

And the grid half is load-bearing. CSS Grid §11.8 *Stretch auto Tracks* — the step that gives an `auto`-max
track its share of the container's leftover space — runs **only when the inline axis is stretch-aligned**.
taffy encodes this exactly: `style.justify_content()` is an `Option`, and it resolves `None` per context
(`compute/flexbox.rs` → `unwrap_or(FLEX_START)`, `compute/grid/mod.rs` → `unwrap_or(STRETCH)`). By always
passing `Some(FLEX_START)` we took that decision away from it, and **every grid we ever laid out skipped
§11.8** — whether or not the author had written `justify-content` at all.

The visible symptom was a two-column layout huddled against the left edge with the container's right half
empty. `tests/wpt/probes/grid-implied-tracks.html`: a 600px container, `grid-template-areas:"l r"`, no
`grid-template-columns`. Chromium 289px / 291px; ours **88px / 133px**. Every item was in the right cell and
nothing was missing — the columns were simply content-sized, which is why it read as a placement bug for
four ticks.

The fix is one variant (`JustifyContent::Normal`, now the `Default`), the two cascades mapping the initial
value onto it (`stylo_map.rs` `AlignFlags` 0/1/11; the text parser's fallback), `map_justify` returning
`Option` so `Normal → None`, and `getComputedStyle` serialising it as `"normal"` — which is also what Chrome
reports. Gated by `G_GRID_IMPLIED_TRACK_STRETCH`, RED-proven by re-collapsing `Normal → FLEX_START`
(290px columns → 10px/11px). The gate carries two guards beside the feature, because the failure mode here
is replacing one hard-coded alignment with another: an explicit `justify-content:center` must still leave the
tracks content-sized and centre them, and a flex row must still pack at the start.

**The general shape, and it is not specific to grid:** a CSS keyword whose meaning is *context-dependent*
cannot be flattened onto one of its meanings at parse time. `normal`, `auto` and `stretch` are the family to
watch. Where a downstream library models the distinction with an `Option`, that `Option` is the contract —
filling it in is discarding information the library was about to use.

## `repeat(auto-fill, …)` is a shape the cascade must NOT resolve (tick 570)

`grid-template-columns: repeat(auto-fill, minmax(18em, 1fr))` is the responsive-card idiom — one
declaration, no media queries, and the grid holds as many columns as the container can fit. **We
rendered it as one full-width column, on every site that used it, from both cascades independently.**

The two failures are different and each is instructive:

- **Stylo path.** Stylo keeps the auto-repeat in `TrackList::values` at `auto_repeat_index`, with a
  `RepeatCount` of `AutoFill`/`AutoFit`. `template_to_tracks` matched `RepeatCount::Number(i)` and sent
  everything else through `_ => 1`. A catch-all that turns *"repeat this as many times as it fits"* into
  *"once"* is the shape to watch for: it is not a parse failure, it produces a valid track list, and the
  page renders — narrower.
- **Text-cascade path.** `expand_grid_repeat` was a **string** rewrite that scanned for the first `)`
  after `repeat(`. For `repeat(auto-fill, minmax(180px,1fr))` that `)` closes `minmax(`, so it parsed
  `"auto-fill"` as the count, failed, emitted nothing, and left a stray `)` in the track list for the
  track parser to discard. **Pattern-matching text where the grammar nests is a bug waiting for its
  input**; the replacement parses the nesting (`split_tracks_top_level` already tracks depth) and splits
  on the *first top-level* comma so `minmax(a, b)` survives.

**The general principle, and it is the same one t569 landed from the other side.** The repetition count
is defined by CSS Grid §7.2.3.1 as the largest N whose tracks plus gutters fit **the grid container's
resolved inline size** — a number the cascade cannot know, because the container has not been sized yet.
So the cascade's job is to carry the *shape* (`TrackComponent::AutoRepeat { fit, tracks }`), not to
resolve it. taffy models exactly this (`GridTemplateComponent::Repeat` + `RepetitionCount::{AutoFill,
AutoFit}`) and does the counting where the size is known. Twice in two ticks, the bug was **our cascade
answering a question that belonged to layout**, and both times the borrowed library had already modelled
the distinction we flattened.

`auto-fit` differs from `auto-fill` in one way that is easy to implement halfway: the repetitions that
end up **empty collapse to zero**, gutters included. Two items in a would-be three-track grid must
therefore span the container (290px each at x=0/310 in a 600px box), not sit in the first two of three
186.67px tracks. A fix that generates the tracks but never collapses them looks correct on `auto-fill`
and leaves a third of every `auto-fit` row permanently blank, so the gate asserts both keywords.

MEASURED against live Chromium on `tests/wpt/probes/grid-auto-repeat.html`: SHAPE **15.0% → 100.0%**,
absolute placement **0.0% → 100.0%** (dx=dy=dw=dh=0 across all 20 paths). Gated by
`G_GRID_AUTO_REPEAT`, RED-proven by collapsing the `AutoRepeat` arm back to `Single(tracks[0])`
(`fillx:0,207,413 w:187` → `fillx:0,0,0 w:600`). The integer-`repeat()` guard stayed **green under the
RED patch**, which is what makes it a guard: the rewrite had to fix the auto- forms without breaking the
count that already worked.

## CSS `filter` — an offscreen GROUP is the whole mechanism, and the blur's integer division is where it nearly died (tick 592)

`filter` is on **51.9% of page loads** and it had reached exactly nothing: Stylo's servo build parsed
and computed it correctly and always had, `ComputedStyle` had no field for it, and so
`CSS.supports('filter', 'blur(4px)')` answered **yes** about a capability that painted nothing (t591
made that answer honest; this tick removed the reason for it). The chain that closes it is four links
long and every one of them is ordinary:

```text
stylo_map.rs  clone_filter()  →  ComputedStyle.filter: Vec<FilterOp>   (OWN value, not folded)
layout        LayoutBox.filters                                        (12 literal sites, like `shadows`)
paint         PaintGroup.filters                                       (COMPOSED down the tree)
paint         manuk-paint::filters over an offscreen Pixmap            (the pixels)
```

**The one design decision is that a filter needs a GROUP, and our display list is flat.** CSS applies
`filter` to the element *and its subtree, composited as one group* — which is why `opacity`'s
established trick here (fold the effective value into every box and scale each item's alpha) does not
transfer: a blur is not a per-item operation, and `blur(4px)` applied twice is not `blur(8px)`. So the
filter is **not** folded into descendants the way opacity is. It is composed at *group-build* time,
where `visit` already carries `z` and `clip` down the tree, and it **concatenates** rather than
overrides — a blurred card inside a greyscale section is both.

Rasterization then splits in two at one `if`: an unfiltered group draws straight onto the canvas
exactly as before, and a filtered one draws into a scratch `Pixmap`, runs the pipeline over it, and
composites the result back. Two things make that affordable. The scratch is sized to the **group's own
ink box** (grown by the chain's blur bleed, clamped to the canvas), not to the viewport — fifty
drop-shadowed icons must not cost fifty full-screen buffers. And extracting the per-item match into
`draw_item(pixmap, item, clip, dx, dy)` is what lets the same code draw into a surface whose origin is
not the page's; the offset pair generalises the `scroll_y` shift that was already threaded through
every arm.

**THE APPROXIMATION, NAMED.** Each paint group in the subtree is filtered separately rather than the
subtree being composited and then filtered. For the colour filters the two are identical wherever a
group's pixels do not overlap each other (a colour transform is per-pixel); for `blur` they differ only
across an internal edge. The case it gets right is the one that decides whether a page is readable: the
element **and everything inside it** is blurred, instead of nothing being.

### The blur lost 84% of its ink, and the test that caught it was itself wrong first

Blur is three box passes — the SVG filter spec's own Gaussian recipe, `d = floor(σ·3·√(2π)/4 + 0.5)` —
run with a sliding window so the cost is O(pixels) regardless of radius, on **premultiplied** samples
(blur is linear; that is what premultiplication is for). The first cut divided the window sum with
plain integer division. Six passes each biased downward by up to 1/window, and a blurred region came out
**dimmer**, not softer: 255 → 41 on the first measurement. Rounding (`(sum + win/2) / win`) makes the
error unbiased instead of cumulative. **An integer filter kernel that truncates is a fade, not a blur**,
and it is invisible in a screenshot of a single element — you have to sum the alpha to see it.

The test that found it was written on a **single lit pixel**, and that premise was wrong: an 8-bit
surface cannot represent a delta spread over a few hundred pixels, so *no* correct implementation could
have conserved its energy, and the assertion was measuring the format's rounding. Rewritten on a filled
block it pins three separate failure modes at once — the interior stays opaque, ink escapes the edge,
the edge becomes a ramp — and it kept the real defect. **A test whose subject cannot exhibit the
property being asserted is a false RED, and it costs the same as a false green.**

### Two coordinate spaces meet at the scratch surface, and only one of them still owes the scroll

`draw_filtered_group` receives display items in **page** space and a clip the caller has **already**
converted to device space. Applying the same offset to both — the obvious thing, since one variable
looks like it should serve — double-subtracts the scroll and slides every `overflow` clip off the
filtered element, which at scroll 0 is a perfect no-op. So it survives every gate that renders an
unscrolled page, which was all of them. `G_FILTER_RENDER`'s second test scrolls 150px over a 40px
`overflow: hidden` window and asserts both halves: the filtered block paints inside it, and its
overflow below it is gone. **A filter must not launder an element out of its ancestor's clip.**

### Colour matrices are spec constants, so they are asserted EXACTLY

`grayscale(1)` of `#f00` is `(54, 54, 54)` — Filter Effects 1's **legacy** 0.213/0.715/0.072 luminance
row, not Rec.709's 0.2126/0.7152/0.0722. `grayscale(a)` is derived as `saturate(1 - a)` because the spec
defines it that way and two hand-written copies drift. An engine that lands "approximately grey" is a
shade off in every screenshot diff forever, so `G_FILTER_RENDER` asserts the integer and not a range —
and it opens with a **vacuity guard** (the unfiltered control must paint pure red), because every claim
after it would otherwise be satisfiable by a canvas on which nothing painted at all.

`drop-shadow` is the one function that is not a colour or a convolution: it casts the surface's **alpha
silhouette**, offset and blurred, behind the source. That silhouette is the entire difference from
`box-shadow` — a cut-out PNG or an icon glyph casts the shape of its ink, not the shape of its box.

**RESIDUE, and it is deliberately not carried:** `backdrop-filter` (34.3% of page loads) stays a
honest no and its constellation row was **split out** rather than promoted along with this one. It
filters what is painted *behind* the element, which a group rasterized in isolation does not have —
that is a compositor-order change, not another entry in the pipeline. Also open: `getComputedStyle(el)
.filter` is still `undefined` (the CSSOM half of the t576/t590 class), `url()` SVG filter references
need an SVG filter graph, and a filter does not yet establish a containing block for its fixed/abspos
descendants.

## `clip-path` — the second capability out of the same offscreen group, at a fraction of the price (tick 593)

`clip-path` is 43.8% of page loads and was in exactly the state `filter` had been in the tick before:
Stylo parsed and computed it, nothing read it, every clipped element rendered as its full rectangle.
It landed in one short tick because **t592's offscreen paint group is precisely the surface a clip
mask applies to** — the plumbing (`ComputedStyle` → `LayoutBox` → `PaintGroup`, composed down the
subtree with the declaring element's box) is the same four links, and the raster step is
`Mask::fill_path` + `Pixmap::apply_mask`. That is the argument for building the group properly the
first time rather than special-casing blur.

Supported: `inset()` (including `round`), `circle()`, `ellipse()`, `polygon()` (both fill rules).
`path()`, `shape()` and `url(#svgclip)` need an SVG path graph and map to **`None`** — honestly
unclipped, rather than a variant nothing draws.

**Two things about the shape are not obvious and both are load-bearing.**

**The reference box is the box that DECLARED the clip, not the box being painted.** A `clip-path` on
a section applies to everything inside it, but `circle(50%)` still means 50% of *the section*. Get
this wrong and the shape still draws, percentages still resolve, and the result merely looks like a
layout bug — which is why the gate nests a child inside a clipped ancestor and asserts the ancestor's
circle, not the child's.

**The clip runs AFTER the filter**, per CSS Masking's `filter → clip → mask → opacity` order.
Clipping first would let a blur smear colour back across the edge the clip just cut, which is the
whole visible difference between a hard-edged shape and a fuzzy one.

### `inset(50%)` must be allowed to clip everything, and it is the one place the module's default is wrong

`apply_clip_shape` fails **open**: a degenerate radius or a two-point polygon clips *nothing*. That is
deliberate — an unclipped element is visibly wrong and fixable, an erased one reads as content that
was never there. But overlapping insets are not a failure, they are the point: `clip-path: inset(50%)`
is Bootstrap 5's `.visually-hidden` and the modern replacement for `clip: rect(0,0,0,0)`. Clamping
that rect to non-negative — the obvious defensive move — renders the screen-reader-only text the rule
exists to remove. So the empty region is an explicit branch, and the gate asserts three points inside
the box are all still page-white.

This is the same lesson as the `opacity: 0` fade-in, with the sign flipped: **an author can ask for
nothing, and "nothing" is an answer a renderer has to be able to give.**

⚠ **RESIDUE, found by this tick's own gate.** A `position: absolute` descendant is hoisted out of its
ancestor's box subtree by `position_absolutes`, so a paint-time tree walk cannot see it and neither
`clip-path` nor `overflow: hidden` clips it. That is a box-tree limitation shared by both, not a
clip-path one. Also open: `getComputedStyle().clipPath` is `undefined`, and `@supports (clip-path:
path(…))` is now a yes about a form we do not draw — a narrower lie than the one retired, named here
so it is not mistaken for coverage.

## `mix-blend-mode` — the offscreen group's *composite-back* is the backdrop, and that is the third capability out of one mechanism (tick 594)

t593 closed with an open question: `mix-blend-mode` (12.9%) and `backdrop-filter` (34.3%) both need
the group's **backdrop** — an input the paint path did not have — so is there **one** mechanism that
buys both, and is it therefore a 47% row rather than two 13%/34% ones?

**Yes, and t592 already built it.** The realisation is small enough to state in one line: *the
backdrop a blend needs is exactly what is already on the canvas under the group's ink box, and the
group's own pixels are exactly what the offscreen surface holds.* The blend then costs one field on
the composite-back `draw_pixmap`. All 16 CSS modes — separable **and** non-separable — have a
`tiny-skia` counterpart, so nothing is approximated. (`plus-lighter` has none and maps honestly to
`normal`; a wrong blend is harder to spot than none.)

That is three capabilities out of one tick's infrastructure — `filter`, `clip-path`, now
`mix-blend-mode`. The generalisable claim is **not** "blend modes are easy". It is: *when a property
needs the element's pixels considered apart from the page's, the expensive part is the separation,
not the operation.* `backdrop-filter` is the fourth and needs one more piece — reading the canvas
region *before* the group is drawn over it — and that is now a small addition rather than a design.

`mix-blend-mode` propagates down the subtree in the same paint sense as `filter` and `clip-path`, but
it **overrides** rather than composing: blending is not a pipeline, there is one backdrop and one
formula, so a descendant declaring its own mode replaces the ancestor's rather than stacking with it.

### One number is recorded and NOT asserted, on purpose

`luminosity` measures **(207, 0, 0)** on a red backdrop with a blue source. Working Compositing-1's
`SetLum` + `ClipColor` by hand gives ≈ **(94, 0, 0)**, and 207/255 is exactly the *un-clipped*
intermediate — which suggests `tiny-skia` skips `ClipColor`. **That derivation is a reading of the
spec, not a measurement**, and this repo has twice shipped a gate whose expected value came from
memory and which therefore tested the memory. A headless-Chrome cross-check was attempted and did not
reproduce the layout, so no third-party number is claimed.

So the gate asserts the **separable** modes to the exact integer — their answers on `#f00 × #00f`
have no rounding slack, and a gate that only checks "the pixel changed" passes for a wrong formula —
and asserts `luminosity` only on what was observed: the mode is applied, the backdrop's hue survives,
the source's luma darkens it. **Pinning 207 as correct would bank a possible upstream divergence as
an intended value**, which is precisely how a wrong constant becomes permanent. The exact
non-separable answer is an open item for the parity harness, which is the tool built for it.

⚠ Also open: `isolation` (18.0%) is still unread, so a blend is not confined to a stacking context
that asked to contain it; and `getComputedStyle().mixBlendMode` is `undefined` — the CSSOM half of
this bundle is now three properties deep and worth one tick together.

## `backdrop-filter` — the bundle closes, and the property that was LAST is the one that justifies the split (tick 595)

Four ticks, four properties, one mechanism: `filter` (t592), `clip-path` (t593), `mix-blend-mode`
(t594), `backdrop-filter` (t595). Together they are **~143% of page loads** by the Blink counters, and
they were all in the same state — Stylo parsed and computed them, nothing read the result.

`backdrop-filter` came last **on purpose**, and the reason is the reusable part. Every other property
in the bundle operates on the **element's own pixels**, which t592's offscreen group already
separates out; a new property was a new *field* on the composite. This one operates on the pixels the
element is about to **cover** — a different *input*, not a different operation. That is precisely why
t592 split its constellation row out of `filter`'s instead of carrying it along, and three ticks later
that split reads as vindicated rather than merely cautious: **the taxonomy that predicted which
capability would be cheap was "what does it consume", not "what does it look like".**

The implementation is small and the two decisions in it are the ones worth keeping:

- **Read back, filter, write with `Source`.** `clone_rect` the canvas region under the box, run the
  same pipeline over the copy, and write it down as a **replace**. Compositing it source-over its own
  unfiltered original would leave the sharp version showing through wherever the filter reduced alpha
  — a bug that looks like "the blur is too weak" and is actually double-drawing.
- **Confined to the border box.** `PaintGroup` gained `bounds` for this. A backdrop filter that
  blurred the whole canvas passes any "did the seam soften?" test and is catastrophically wrong, so
  the gate has a third, separate claim that the seam *outside* the panel is still hard.

The `filter`/`backdrop-filter` list mapper is **one function used twice**, not two copies: they share
a grammar exactly, and two copies of one grammar is how a `drop-shadow` fix lands in one property and
not the other.

### The session's recurring lesson collected a fifth time

`backdrop-filter` was on **both** denylists — `PARSE_ONLY_LONGHANDS` (it is pref-gated) *and*
`UNRENDERED_LONGHANDS` (t591 added it there too). Removing it from one left `CSS.supports` still
answering no, and the unit test caught it immediately. This is t591's rule again, now at its fifth
width in one session: **a change scoped to the shape the problem presented in is one category too
narrow — grep for the class.** Note the direction, though: the *fix* was fine, the *bookkeeping* was
in two places. Both halves of a capability's honesty — "do we render it" and "why was it parseable" —
have to move together.

⚠ Residue: the backdrop region ignores `border-radius`, so a frosted panel with rounded corners
filters a square. `isolation` (18.0%) is still unread, so a blend or a filter is not confined to a
stacking context that asked to contain it. And `getComputedStyle()` still returns `undefined` for all
four of these — the CSSOM half of the bundle is one tick's work and is now the obvious next one.

## An exact path bbox is the EXTREMA, not the control-point hull (tick 630)

`<path>` had no arm in `svg_bbox`'s match at all, so `getBBox()` answered `0×0` for the single most
common SVG element — every icon set (Lucide, Feather, Material), every chart shape generator, every
logo. It was a **deliberate** zero: the gate's own doc said *"`<text>` and `<path>` report zero size on
purpose"*, on the same reasoning `<text>` uses — a plausible guess mis-places everything that trusts
it. That was right about guessing and wrong about the alternative.

### The curve is the whole difficulty

```text
  M0 0 C 0 20 20 20 20 0     control points at y=20     the curve reaches y=15
  M0 0 Q 10 20 20 0          control point  at y=20     the curve reaches y=10
```

**A control-point hull is the easy, obvious, wrong implementation.** It is strictly LARGER than the
curve, and it looks entirely plausible — which is what makes it dangerous rather than merely
imprecise: a too-large icon box mis-positions every tooltip anchored to it and mis-sizes every chart
hit-area, while reading as "close enough" to anyone eyeballing it.

So each segment is solved for its real extrema — the roots of the derivative in `(0,1)`:

- **cubic**: `a = 3(−p0+3p1−3p2+p3)`, `b = 6(p0−2p1+p2)`, `c = 3(p1−p0)`; roots of `at²+bt+c` in
  `(0,1)`, evaluated on the Bézier.
- **quadratic**: `t = (p0−p1) / (p0−2p1+p2)`, when the denominator is non-degenerate.
- endpoints always count; `S`/`T` reflect the previous control point, and only when the previous
  command was of the matching family.

The tokeniser is a scanner rather than a `split_whitespace`, because `-` and `.` start a new number
without a separator: `M0-5` is two numbers, and `1.5.5` is `1.5` then `.5`.

### Elliptical arcs REFUSE, and that is the design

`A` returns `None`. Bounding an arc exactly needs the endpoint→centre parameterisation and then the
extrema of a rotated ellipse over the swept angle range. That work is not done, and **the honest
answer to "what is this path's box" when part of it cannot be bounded is no answer** — not a guess in
either direction. Same choice `<text>` makes, and `clip-path` before it.

⚠ **A negative assertion needs a probe that produces a DIFFERENT wrong answer.** The first RED probe
for the arc refusal made the "guess" the current point — `(0,0)`, giving `0×0`, indistinguishable from
the refusal — and the gate stayed green. A realistic guess consumes the seven arc parameters and takes
the endpoint, giving `0,0,10,10`, which the gate does catch.

### What this does NOT fix

`getBoundingClientRect()` on an SVG child still returns `0×19` — an empty inline box at the default
line height — because `svg_bbox` lives in the JS binding layer and the CSS box comes from layout.
That is a separate subsystem (t629); this makes sure it will have correct geometry to consume for the
commonest element when it lands.

## An image's size is in no stylesheet — so a re-cascade erases it (tick 656)

**Every other geometry input to layout comes from the cascade.** A replaced element's *intrinsic*
size does not: it arrives from the network, long after the cascade that will be asked to lay it out.
So the natural size was written straight into the cascade's **output** — the style map — by
`apply_images`:

```rust
for (&node, img) in &images {
    if let Some(style) = self.styles.get_mut(&node) { apply_natural_size(style, img) }
}
self.relayout(fonts, viewport_width);
```

That is correct exactly until the next cascade, and `self.styles = cascade_styles(...)` is
**wholesale**, at more than a dozen call sites. Measured on a page with no CSS at all and one 41×23
image, either side of the same load:

```text
  after load_async     (image applied)     width=Px(41)  ar=Some(1.78)   rect  41×23
  after finish_loading (re-cascaded)       width=Auto    ar=None         rect 784×0
```

**784 is the full content width; 0 is no height at all.** The picture occupies no space and every
element below it slides up into the space it should have taken.

### Why it was permanent rather than transient

The image phase dedups per `(node, url)`, so the *second* budgeted pass finds nothing to fetch, calls
`apply_images` with an empty map and returns early. The natural size is applied **exactly once** —
whichever cascade runs after it is final. There is no self-healing pass.

### Why no instrument saw it

Coverage asks *"is the node there?"* Every one of those images is there: parsed, styled, probed,
counted. They are simply all zero pixels tall. This is the same shape as tick 654's naked page from
the other direction — **a 98%-covered page can be one whose every picture is a hairline.** Only a
structural/placement score can see it, which is why it surfaced from a *control run* rather than from
a gate.

### The fix: a standing input belongs between the cascade and the layout

```rust
fn restyle_and_layout(dom, sheets, fonts, vw, images) -> (StyleMap, LayoutBox) {
    let mut styles = cascade_styles(dom, sheets, vw);
    apply_natural_sizes(&mut styles, images);   // <- restated on EVERY route to a new box tree
    let mut root_box = layout_document(dom, &styles, fonts, vw);
    ...
}
```

`restyle_and_layout` is documented in the tree as *"the one join every restyle path shares"*, which
makes it the right home; the four sites that call `cascade_styles` directly restate it themselves,
immediately before their own `layout_document`.

**`forced_reflow` is included, and it is the site tick 654 had to name and leave.** The synchronous
layout that a JS `offsetWidth`/`getBoundingClientRect` read forces mid-script is the ninth re-cascade
site, and t654 could not reach it: it runs off a `*mut ReflowCtx` installed at 17 call sites with no
route to the page, and threading a raw pointer to a `self` field while `&mut self.dom` is live is an
aliasing question that does not belong inside a layout tick. It never had to be answered — **the
context OWNS the data instead of pointing at it.** `ReflowCtx` carries the image map, cloned in at
`ReflowScope::install`: an `Rc` clone per decoded image, no pixels copied. *When a raw pointer is the
only route to a value, check whether the value is cheap to own.*

### The gate asserts three things, and two of them are not decoration

`G_IMAGE_NATURAL_SIZE_SURVIVES_RESTYLE`:

1. the image's box is its own size;
2. **the paragraph after it starts below it** — a width the layout ignores is not a fixed bug, and an
   assertion on the image's own box cannot tell "sized" from "sized and consumed" apart;
3. both still hold after a **second, independent** re-cascade trigger (a click), because the map is
   rebuilt at more than a dozen sites and *one rule with N implementations is not proven by the one a
   gate happens to touch* — the lesson tick 654 paid for with eight of them.

Its own preconditions are asserted as well (the external sheet DID cascade; the `fetch` DID resolve),
so it cannot pass by never reaching a re-cascade — which is precisely how this bug hid: at
`load_async` the image is correctly sized, and a test that stopped there would have been green.

## `www.naukri.com`: our `<body>` is 89,905px wide and Chrome's is 1,200 (tick 684, OPEN)

The site is the marginal scored row of the certificate, and — unlike `agoda` — it is a *good* lever:
the ORACLE's population is stable at **57 paths in every draw**, so the gap is ours alone. The §3b
root-cause cluster:

```text
  MISSING by tag: div×36  input×3  span×3  button×2  section×2  footer×1  ul×1
  geometry/mis-sized: width ~65536px  (<body>)  [median 88705px]
      body:nth-child(2)                    [0 0 89905×352]  vs  [0 0 1200×1513]
  geometry/mis-sized: width ~65536px  (<div>)   ×5
      body/div:nth-child(2)/div:nth-child(1)   [0 0 89905×0]    vs  [0 0 1200×0]
  geometry/displaced: x (horizontal) ~32768px
      body/div:2/div:4/div:2                   [44392 0 1120×72] vs [40 0 1120×72]
```

**The displaced element proves the mechanism.** Its width is 1120 in BOTH engines, and its x is 44392
here against 40 in Chrome. `(89905 − 1120) / 2 = 44392.5`. It is being **centred inside an 89905px
parent** — so the child is fine and the parent's width is the whole bug. Everything downstream of that
(36 missing `<div>`s, the 0.0% shape) is a consequence.

### Two mechanisms ELIMINATED, so the next attempt does not re-derive them

1. **"A block box takes its content's max-content width instead of its containing block's."** A hermetic
   fixture — a 5000px child, a 4000px flex row, a 160-character unbreakable string, and an
   `margin: 0 auto` box, all in an 800px viewport — gives **body 784**, the wide child overflowing
   correctly at w=5000, and the centred box at x=200 = (784−400)/2 + 8. Exactly right. Not this.
2. **"Something in the document does it."** The same document fetched to a local file with a
   `<base href>` and rendered here gives **body 1008** in a 1024 viewport — correct, and no oversized
   descendant at all (`WIDEST <DIV> w=1008`).

So the condition is **specific to the LIVE load** and not to the markup: something that only happens
when the document comes from the origin — a stylesheet that only resolves there, a subresource-driven
relayout, or a script path that only runs with real cookies/redirects. That is the next experiment, and
it is a bisection over the live load rather than a hypothesis about layout.

### The PHASE is now named: the external stylesheet (tick 685)

The document is identical under our User-Agent and under Chrome's (byte-for-byte, 15,222 bytes), so the
origin is not serving us something else. What differs between the local render and the live one is
**whether `//static.naukimg.com/s/7/103/c/main.8c85256c.min.css` arrives**:

```text
  stylesheet FAILED   ->  body ~0        (STYLESHEET FAILED — the page will render unstyled)
  stylesheet applied  ->  body 89905     (the sweep, where `Satoshi` is the computed font)
```

So the 89,905px comes **out of the cascade**, from that sheet, and the whole question is now *which
declaration*. Two candidates are visible in it, and both are the shape that produces a runaway width:

```css
@media only screen and (max-width:1270px) { html { width:-webkit-fit-content; width:-moz-fit-content;
                                                   width:fit-content; … } }
.gap-patch .circle { width:1000%; left:-450% }
```

⚠ Note the media query is **`max-width`**, so at the sweep's 1200px viewport it **matches** — the root
really is `width: fit-content` on this page.

**Third mechanism eliminated:** `html { width: fit-content }` alone does NOT do it. A hermetic fixture —
that declaration plus a 5000px child in an 800px viewport — gives `html` and `body` at **784**, correctly
clamped to the available width, with the wide child overflowing. So it is not a missing `fit-content`
clamp on its own; it needs something else in that sheet.

### The bisection ran, and it narrowed the question again (tick 686)

**Chrome's own built DOM plus that same stylesheet, rendered here offline, gives `html=1200 body=1200`** —
and a content height of **352px, which is exactly the height the sweep measured for our 89905×352 body.**
Same content, same height, correct width. So the sheet and the DOM together are not sufficient to produce
the bug.

**Fourth mechanism eliminated:** it is not the ASYNC arrival of the sheet either. A loopback-served
external stylesheet carrying `html { width: fit-content }` and a 5000px child re-cascades and re-lays-out
to `html=784` in an 800px viewport, with the child overflowing at 5000 — correctly clamped
(`G_EXTERNAL_CSS_SURVIVES_RESTYLE` now asserts this, with `#wide == 5000` as its vacuity guard).

So the surviving difference is the one thing not yet substituted: **the DOM OUR scripts build.** Chrome's
DOM lays out correctly here; ours does not. The next step is therefore a DOM diff, not a CSS bisection —
dump our own built tree and compare it against Chrome's for the elements on the 89,905px chain.

| substituted | result |
|---|---|
| the document (our UA vs Chrome's) | byte-identical, 15,222 bytes — **not it** |
| `html{width:fit-content}` alone, hermetic | html 784, child overflows — **not it** |
| the sheet arriving ASYNC + re-cascade | html 784, child overflows — **not it** |
| Chrome's built DOM + the real sheet, offline | html 1200, body 1200×352 — **not it** |
| **our own built DOM** | ← the only thing left |

⚠ Recorded as OPEN, with the eliminations, deliberately. Three of this session's ticks were saved by a
five-minute probe killing a good-sounding mechanism; the cost of *publishing* the eliminations is one
paragraph and it is what makes the next attempt cheap.

[[subpixel-error-compounds]] [[symptom-names-wrong-organ]]

## SHAPE is not a per-box metric error — it is one wrong HEIGHT above the content (tick 688)

The rendering-gap mandate hypothesises that *"ONE shared constant (font-metrics / line-height / margin /
border-box rounding) likely snaps MANY boxes into 8px tolerance at once."* The sweep already computes the
per-site median delta, so the hypothesis is testable without building anything:

```text
  site           dx    dy    dw    dh    absolute PLACEMENT
  comix.to        0     0     0     7    100.0%
  desitales2      0    91     0     3      1.2%
  www.welt.de     0  3077     0     0      2.8%
  www.agoda.com   1    14     1     1      3.1%
  keirin.jp       2   206     0     1      0.3%
  www.ikea.com    0   145     0     0      9.9%
  playhop.com     0     0    10     7     14.3%
```

⚠⚠ **`dx` is 0–2 everywhere, and `dw`/`dh` are 0 on the worst sites. The dominant term is `dy`** — 91, 145,
206, 3077 — a pure **vertical displacement of correctly-sized boxes.**

A box of the right size at the wrong `y` is not a per-box metric error. **Something ABOVE it has the wrong
height**, and every box below inherits the shift. The two sites that *do* carry a text-metric term
(`comix.to` dh=7, `playhop` dw=10 dh=7) are the two with the **highest** placement scores — so the metric
constant is the residual, not the lever. This does not say font metrics are perfect; it says they are not
what is holding SHAPE at ~6%.

### FIRST DIVERGENCE already points at the cause

```text
  keirin.jp    after …/nav:1/…/a:1/img:1   → …/nav:1/div:2      off by dy=70
  desitales2   after …/nav:3/div:1/div:2   → …/div:5/div:2      off by dy=-73
  welt.de      after …/p:2                 → …/article:1/div:2  off by dy=285
  agoda        after body/div:23/div:5     → body/div:23/div:9  off by dy=631
```

`keirin`'s divergence begins **immediately after an `<img>`**, and `desitales2`'s is **negative** — ours is
73px *higher* than Chrome's, i.e. we are missing content above rather than adding it. `Cc4e6 geometry:
<img>` is a **67-site** cluster, and this project has already recorded an `<img>` laying out at **784×0**.
An image whose height is wrong shifts every sibling after it.

**The lever, therefore, is the height of a box above the content — starting with `<img>` — and not a
tolerance constant.** Measured before building, which is the only reason it is one tick instead of several.

[[subpixel-error-compounds]] [[symptom-names-wrong-organ]] [[default-object-size-not-ua-width]]

## A broken `<img>` is 16×16 in Chrome and was 784×0 here (tick 689)

The tick-688 measurement said the SHAPE gap is a `dy` term — correctly-sized boxes in the wrong place, so
something above them has the wrong height — and `keirin.jp`'s first divergence begins **immediately after
an `<img>`**, off by `dy=70`. `Cc4e6 geometry: <img>` is a **67-site** cluster.

Measured on headless Chrome and on this engine, same fixture, 800px viewport:

```text
                                       Chrome        ours (before)
  <img src="…/never.png">              16×16         784×0
  <img width=120 height=70 src=…>      120×70        120×70      ✓
  <img> with CSS width+height          120×70        120×70      ✓
  the div AFTER the bare img           y=196         y=168
```

**784×0 is wrong twice:** an inline replaced element must not take the whole line, and a box whose source
broke is not zero-height. 16×16 is the placeholder Chrome reserves, and reserving it is what stops the rest
of the page sliding up.

The layout source even carried the reason it was excluded — *"a sourceless image has no default object size
in any browser"* — which is true of `<img>` with **no `src`** and not of an `<img src>` whose bytes never
arrive. That is the case the web is full of: dead CDNs, blocked trackers, lazy-load placeholders, icons
behind a 403.

Conditioned on `taffy_known.is_none()`, exactly like the `300×150` default-object-size arm next to it, so a
decoded image, author dimensions, or a derivable aspect ratio are all untouched — all three are resolved
before this line.

⚠ **The gate asserts the FOLLOWING sibling's `y`, not just the image's box.** A height that is right in
isolation and does not push its siblings down would satisfy a box-only assertion and fix nothing about the
`dy` term, which is the entire reason for the change. RED-proven by two independent mutations.

⚠ **NOT covered, named rather than left looking handled:** an `<img alt="text">` whose source failed —
Chrome sizes that box to the **alt text**, which needs the text measurer here and is its own change.

### And a hypothesis Chrome itself killed, before it cost a tick

I expected `<img width=200 height=100 style="width:100px">` to be **50px** tall here — the attributes map
to a presentational `aspect-ratio`, so overriding the width should carry the height. **Chrome measures
100×100.** A broken image has no intrinsic size, so the attribute height wins outright. Measuring the
oracle first is a standing rule in this project precisely because the spec-reasoned answer and the shipped
answer differ, and the shipped one is the target.

[[default-object-size-not-ua-width]] [[gate-measured-against-a-standard-chrome-fails]]

## An out-of-flow child neither splits its inline nor escapes it (tick 697)

Two defects, one behaviour, both about the same boundary: what an **out-of-flow** box does to the
**inline** it lives in.

**1. It must not split it.** CSS 2.1 §9.2.1.1 splits an inline box around a block-level box *in the
flow* — the box tree becomes anonymous-block / block / anonymous-block. A float or an absolutely
positioned box is removed from the inline formatting context and splits nothing. But `position:
absolute` **blockifies `display`** (CSS Display §2.7), so `<span style="position:absolute">` computes to
`display: block`, and a blockification check that reads `display` alone walks it straight into the
split. `inline_contains_block` did exactly that, and **blockified the inline ancestor into a full-width
block**.

**2. It must be able to use it as a containing block.** For an absolutely positioned box the containing
block is the nearest ancestor whose `position` is not `static`, and CSS 2.1 §10.1 is explicit that an
*inline-level* ancestor qualifies: *"the containing block is the bounding box around the padding boxes
of the first and last inline boxes generated for that element."* `LayoutBox::walk` descends
`BoxContent::Block` only and never enters `BoxContent::Inline`, so a **boxless inline has no entry** in
the rect map `position_absolutes` builds — and the walk went past it to the nearest BLOCK-level
positioned ancestor.

⚠⚠ **Each fix alone reads as a near no-op**, which is why they are one change. Un-blockify without the
rect map and the abspos child still escapes; fix the rect map without un-blockifying and the inline it
finds is a full-width block, so the child resolves against the wrong box anyway.

```text
   Chrome --headless=new, 1200x800, margin:0, 16px/normal sans-serif
   .outer{position:relative}  a.rel{position:relative}  .corner{position:absolute;top:0;left:0;10x10}

                        Chrome           before            after
     #aRel          [36 50 76x17]   [0 68 1200x18]   [36 50 76x19]
     #cRel          [36 50 10x10]   [0 68 10x10]     [36 50 10x10]
     #cStat (guard) [ 0 16 10x10]   [0 16 10x10]     [ 0 16 10x10]
```

`<a style="position:relative">text<span style="position:absolute">…</span></a>` is the stretched click
target, the badge on an icon link, the tooltip anchor and the dropdown under a nav item. Every one of
them took a whole line, forced a break, changed its parent's height and displaced everything below —
which makes this a **`dy` generator**, the term the fidelity sweep reports as dominant.

⚠ **`position: static` must still establish nothing.** The fix widens which ancestors *can* be a
containing block, not which ones *are* — a version that made every inline one passes both positive
assertions and breaks every real overlay on the web. The gate's `static` control is what separates them.

⚠ Still open: `node_rects` lifts out-of-flow descendants into boxless inline ancestors, so a
`position:static` inline holding an abspos child reports [0 16 113×88] where Chrome says [36 84 76×17].
That is `getBoundingClientRect` on a link — hit-testing, not just fidelity — and it needs an
out-of-flow marker on `LayoutBox`, which does not exist yet.

[[text-layout]] [[conformance-and-oracles]]

## The clearfix had no box to clear with (tick 698)

`.cf::after { content: ""; display: block; clear: both }` is **the** float-containment idiom of the last
fifteen years — every Bootstrap-era grid, every WordPress theme, every hand-rolled `.clearfix`. It did
nothing here, and the reason is that generated content had no block-level form at all.

`collect_inline_group` materialises `::before`/`::after` as inline **words**, and its own comment states
that is *"the only place [generated content] can enter the flow"*. So a pseudo with `display: block`
produced **no box**. That path also dropped `content: ""` — `(!text.is_empty()).then(…)` — because an
empty string looked like nothing to render. It is not: an empty string is a box with no text, and only
`content: none` suppresses a pseudo-element.

With no box, nothing cleared, and the parent **collapsed to zero**, dumping its floated children outside
itself and pulling every following sibling up.

**Bisect the idiom before fixing it.** *"The clearfix does not work"* has three candidate causes, and
two of them were already correct here:

```text
   a real sibling <div style="clear:both">     h75, matching Chrome   -> `clear` WORKS
   ::after{content:"XY"}                       renders                -> `::after` WORKS
   ::after{content:"";display:block}           no box                 -> THIS is the gap
```

One fixture answered it in a single run. Fixing either of the other two would have been invisible.

```text
   Chrome --headless=new, 1200x800, margin:0, one float:left 100x70 child
                                                     Chrome   before   after
     a plain block (must NOT contain its float)        h0       h0      h0
     overflow:hidden                                   h70      h70     h70
     ::after{content:"";display:block;clear:both}      h70      h0      h70
     ::after{content:"";display:table;clear:both}      h70      h0      h70
     ::after{content:"";clear:both}  (inline!)         h0       h0      h0
```

Measured on `keirin.jp`, whose nav is exactly this shape: `#nav_menus` and `#navbar` were **h=0 against
Chrome's h=70**, and 70 was precisely the `dy` the first-divergence probe reported for that page. After:
misplaced **1041 → 954** at an identical path count, median `dy` **124 → 38**, first divergence off the
nav entirely.

⚠ **`clear` does not apply to an inline box** (CSS 2.1 §9.5.2), so the `display` check is load-bearing,
not a shortcut: an `::after` that omits `display:block` must clear nothing. A version without that check
contains floats nothing asked it to, and it passes every positive assertion.

⚠ Scope, stated so the next extension knows which half exists: this places the generated box, honours
`clear`, and gives it its own height and margins — the whole of the idiom's observable effect. It paints
nothing. A pseudo carrying a background or a border still belongs to the inline path; giving it a
painted block box here would be a second implementation of the same rule.

⚠ Adjacent and still broken: **`display: flow-root`** comes out [0 70 0×19] where Chrome says
[0 0 1200×70]. Width zero says it is not parsed as a block-level display at all, and `establishes_bfc`
does not list it.

[[text-layout]] [[conformance-and-oracles]]

## The page was measuring a document with no CSS in it (tick 714)

An instrument that reads the **finished** answer cannot see a bug in the **order** the answer was
assembled in. Every fidelity number this project computes diffs our final layout against Chrome's
final layout; both are post-CSS, so a defect that exists only *during* the load is invisible to all
of them, permanently, no matter how good they get.

On `load_async`, external stylesheets were applied in `finish_loading` — after `DOMContentLoaded`
and after `load`:

```text
  cascade+layout+blocking scripts
  DOMContentLoaded
  load event                 <- every script the page has has now run
  initial images+masks
  external CSS               <- the site's stylesheets are applied HERE
```

Reduced to three lines and one `<link>`:

```text
  boxes (the engine's final layout)   ca [8 8 829 18]  wa [837 8 355 18]   flex, correct
  the page's own script, any phase    display=block  width=1184           UA default
```

So `getComputedStyle` and `getBoundingClientRect` answered from an unstyled document at `sync`, at
`DOMContentLoaded`, at `load`, and from every timer after them — while the painted result was right
the whole time.

**Why that matters more than it sounds.** A page that measures itself writes the answer back:
carousels size slides, sticky headers cache offsets, virtualised lists compute row heights, masonry
grids place columns, chart libraries pick tick counts. Fed UA-default geometry they write a wrong
answer that **no later cascade can undo** — the stylesheet arrives afterwards and restyles a tree the
page has already mis-built. The spec agrees in advance: a `<link rel=stylesheet>` is render-blocking
*and* script-blocking, and `load` does not fire until the sheets have loaded.

### The third instrument class

- the **oracle** diffs the OUTPUT,
- the **log** reports EVENTS,
- and only a **probe that runs inside the page** can observe the SCHEDULE.

The probe here is trivial and reusable: one function `obs()` reading a computed style and a rect,
called at four different moments and written into four different elements — one element each, because
a single accumulating string hides *which* phase produced what, and the phase is the whole finding.

### And a correct fix that was refused

Moving the apply into `load_async` gained `ikea` SHAPE 53.58% → 55.30% and collapsed `welt.de`
coverage 95.61% → 0.03%. welt's own anti-adblock guard fires — a false positive, since this build
has no `adblock` feature — **only once the page can measure a styled document.** The blindness was
masking a second divergence. Full detail, the exact patch and the gate's fixture: `WEB-PATTERNS.md`.

⚠ The reusable half: **a fix can be correct and still be refused, because correctness moved a page
from "blind" to "seeing something wrong."** welt was not working before; it was unable to notice that
it was not working. That is a real improvement to trade away only once the thing it now sees is fixed.

### The retraction (tick 715), and the control that was not enough

The fix above was **refused at t714** because `www.welt.de` collapsed from 95.6% coverage to 0.03%
with it in, controlled twice by reverting the hunk and rebuilding. That control was sound and its
conclusion was wrong.

```text
  control binary, UNMODIFIED, MANUK_LOAD_BUDGET_MS=40000
    ERROR  Failed to load website due to adblock: Error: Failed to execute packing script
    structural: 0.0% (3360 paths, 3359 missing, 1 misplaced)
```

The unmodified engine blanks welt too. welt's anti-adblock guard blanks the page whenever our engine
lets it reach a verdict, and the 95.6% was **our own 12-second timeout cutting the site off before it
could reject us** — coverage achieved by not running the page's script.

> **A control that varies YOUR CHANGE answers *"did my change do this?"*. It does not answer *"is my
> change the only thing that does this?"*** The second question needs a knob the change does not
> touch. Here it was the load budget.

⚠ And the corollary that is easy to mis-read: **a metric can go DOWN because the engine got more
honest.** Refusing a correct fix to hold that number would have been preserving the lie the north
star names by hand — *"fast because we never ran the script"*.

### And the arithmetic that forbids the obvious fix (tick 716)

Landing "apply the CSS earlier" cost `keirin.jp` **7.2 SHAPE points**, twice isolated on each tree,
with identical coverage and identical box counts — the same boxes, moved. `MANUK_LOAD_BUDGET_MS=40000`
restored it to the control's value exactly, so the cause is **budget starvation**, not ordering.

`G_LOAD`'s ceiling is **2x the load budget for the whole page**, and a navigation already spends one
budget in `load_async` (enhancements) and one in `finish_loading`. A third budgeted phase has two
outcomes and no third:

```text
  its own budget   ->  load_async spends 2x by itself, page 3x  ->  G_LOAD red (5.4s against 2s)
  a shared slice   ->  the phase it shares with is starved      ->  keirin -7.2
```

For any slice `s > 0` the worst case is `1 + s` in `load_async` plus `1` in `finish_loading`, against
a ceiling of `2`. **There is no slice that fits**, so the tuning loop has no solution in it.

⚠ **The fix is the other direction, and the spec was already pointing at it: move the LIFECYCLE EVENTS
later, not the CSS earlier.** `load` does not fire until the subresources are in, so `load_async`
should not be firing `DOMContentLoaded`/`load` at all — `finish_loading` should, after its CSS phase.
That spends no new budget and needs no slice.

> **When two gates disagree, the arithmetic between them is the design.** Computing `1 + s + 1 <= 2`
> took a minute and showed a tuning loop — one rebuild and one live-site run per iteration, against
> two opposed measurements — had no answer in it.

### ⚠ And the instrument is calibrated by the bug until it is fixed (tick 718)

The page-side probe above exists *because* external CSS arrives after `load`. So every measurement it
takes is taken in a document where external CSS has arrived after `load` — and using it to
investigate anything cascade-shaped returns the founding bug wearing the new subject's clothes.

Measured on `keirin.jp` while chasing an unrelated geometry divergence:

```text
              Chrome                        Manuk (page-side probe, at `load`)
  .fl-r x85   float=right  display=block    float=NONE  display=inline-block
  .searchbox  float=right  width=252px      float=NONE  width=auto
  styleSheets 9                             styleSheets 0
```

That reads as a general float-blockification bug on the top geometry cluster — specific, plausible,
spec-citable (CSS 2.1 §9.7). It is not: a four-line control fixture blockifies floats **exactly** like
Chrome in all four cases. The 85 elements read `float:none` because the rule had not arrived yet.

> **Before trusting a new instrument on a new subject, run it on a case where you already know the
> answer.** The control took two minutes and cost the finding its life, which is what a control is for.

⚠ Consequence for the parked fix: it is not only worth *"pages measure themselves correctly"*. It is
also **the loop's own newest instrument becoming usable at all** — the same shape as *"raising what
the instrument can SEE outranks fixing what it already sees."*

### Fetched at parse, waited for nowhere (tick 719 — the design that landed)

Two things had to be true at once: the sheets must be applied before the lifecycle events, and the
navigation must not spend a third budget. `G_LOAD` bounds the whole page at **2x the load budget**,
and `load_async` already spends one (enhancements) with `finish_loading` spending another.

```text
  own budget            page spends 3x                        ->  G_LOAD red, 5.4s against 2s
  a shared slice        the phase it shares with starves      ->  keirin.jp SHAPE 60.2 -> 53.0
  concurrent w/ scripts the fixture has dead sheets and NO     ->  G_LOAD red, that phase 0s -> 2s
                        scripts, so there was no wait to hide behind
  fetched at PARSE,     nothing waits, so nothing can starve  ->  G_LOAD 3.51s · keirin 60.2%
  taken if FINISHED                                               ikea 97.1% -> 100.0% coverage
```

Spawn the sheet fetches right after the parse (off the real tree, via `collect_style_sources`, so
`media`/shadow/inline are handled), and at the apply point take only the handles reporting
`is_finished()` — **never await one**. The head start is the external-script fetch, the module-graph
prefetch, the cascade, layout and every blocking script. Anything late falls through to
`finish_loading` exactly as before: strictly more capability, zero added latency.

> **The bound is not the budget, it is the FIXTURE that measures the budget.** The third design died
> on something the arithmetic could not see — `G_LOAD`'s page has dead sheets and *no scripts*, so the
> phase it planned to hide inside did not exist there.

> **And the path that does not have the bug is the design document.** `from_prefetched` had been
> applying CSS before the lifecycle events for its whole life. The answer was not to invent a
> schedule but to ask why it did not need one — its CSS is already in hand — which is the whole fix.

⚠ Blast radius, corrected: `load_async` has **no shell caller**. This bug was the AGENT's and every
fidelity measurement's, not the shipping browser's page scripts.

⚠ And `ikea`'s 21 missing boxes (open since t713) were never a layout bug: **a COVERAGE loss whose
cause was a MEASUREMENT the page took.** No box-diff could attribute it, because the missing boxes
are the ones the page decided not to create.

## The outermost `<svg>` is sized by a RATIO, never by its viewBox numbers (tick 742)

`viewBox="0 0 100 25"` on an `<svg>` with no `width`/`height` attributes is an intrinsic **ratio**
(4:1) and *no intrinsic size* — SVG2 §8.2 plus the CSS-Images §5.3.2 default sizing algorithm. So
`width:auto` fills the containing block and the height follows the ratio. Chrome, measured:

```text
  <div style="width:400px"><svg viewBox="0 0 100 25">      400×100
  <div style="width:250px"><svg viewBox="0 0 100 25">      250×63
  <svg>                              (no viewBox)          300×150   ← default object size
  <svg viewBox="0 0 100 25" height="10">                    40×10    ← ratio runs backwards too
```

We laid all of those out at **100×25 / 100×100 / 100×10** — the viewBox's own coordinate numbers,
read as pixels — and every `<path>`, `<g>`, `<rect>` and `<circle>` in the tree was measured against
a canvas four times too small.

**Where it came from.** `Page::images` is the map the painter reads, and the inline-svg raster cache
is merged into it. `apply_natural_sizes` reads the same map and applies each decoded image's size as
an intrinsic size — and usvg's `Tree::size()` falls back to the viewBox when the dimension
attributes are absent. The merge site carried the intent in a comment (*"inline svgs are
deliberately NOT natural-sized: the measured replaced-sizing model owns their geometry"*), which was
true of the function beside it and false of the map. The exclusion now lives in
`apply_natural_sizes` itself, where every restyle path shares it.

⚠ **The block path already implemented this correctly, and a unit test proved it — under
`MinimalCascade`,** which never runs the natural-size pass. The shipping Stylo path did. See
`live-cascade-is-stylo-not-minimal`: a green unit test is evidence about the cascade it ran under.

### A replaced element has no children, so measuring them reports ZERO

`measure_intrinsic` is the flex/grid measure seam: it sizes an item by laying its subtree out and
reading how far the content reached. For `<canvas>`/`<video>`/`<svg>` there is no subtree, so the
honest content extent is 0 — an unsized `<canvas>` flex item came out **0×150**, and the same for
`<video>`. `replaced_default_size` answers from the replaced model instead: author width wins; a
definite height plus a ratio derives the width; a ratio with `auto` width takes the available width;
otherwise the default object size 300×150.

⚠⚠ **The max-content probe passes `avail_width: None`, and the answer there is the UNBOUNDED one.**
Returning the default object width (300) instead reads to the flex algorithm as *a preference for
300px*, and it is honoured: measured, that put a nav-bar icon at 300×300 beside a 56px label where
Chrome gives it the 544px the label leaves. Taffy shrinks the unbounded answer down to the free
space, which is what makes it the Chrome answer rather than a runaway one.

⚠ **These two halves are one behaviour.** The flex hole was hidden *behind* the sizing bug: reading
a 16-unit viewBox as pixels gave an icon a 16×16 box, which looks like an icon, so fixing the sizing
alone moved the nav icon 16×16 → 300×300 and displaced its label. Landing either half by itself
reads as a regression.

**Residue, measured and open:** ~~`<use href="#sym">` reports no geometry~~ (LANDED t743, below),
`<symbol>`/`<defs>` content still gets a box it should not, an SVG `<text>` box sits at the svg's
top rather than following its baseline, and `www.ikea.com` holds 4 extra `<span>` ⇄ `<svg>`
reading-order pairs that the flex half made Chrome-exact on the fixture without clearing live.

## The icon-sprite `<use href="#icon">` resolved to NOTHING — the reference model (tick 743)

`Page::decode_inline_svgs` serialises **one `<svg>` element** with `serialize_outer` and hands that
string to usvg. The sprite idiom — the way most icons on the open web ship — puts the `<symbol>`
sheet in one `<svg>` and the `<use href="#check">` in another:

```html
<svg style="display:none"><symbol id="check" viewBox="0 0 24 24"><path d="…"/></symbol></svg>
…
<a class="nav"><svg width="20" height="20" viewBox="0 0 24 24"><use href="#check"/></svg> Basket</a>
```

So the id the `<use>` names was **not in the document usvg was given**, every time. usvg drops an
unresolvable `<use>`, which means this was not a wrong box — **the icon did not rasterise at all**,
and the geometry pass underneath was measuring a blank. Population, measured on 67 corpus sites
fetched fresh: **8 (12%) ship `<use href="#…">` in their initial HTML, 454 references** — apnews.com
alone 314, samsung 55, zdnet 39. A floor, not a ceiling: it counts static markup only, and
`www.welt.de` reaches 102 the same way.

**Two halves, and each alone is a regression** (RED-proven both ways — the outputs are identical):

1. **Inject what the subtree reaches outside itself.** `svg_geometry::external_use_defs` follows
   every `<use>`'s `href`/`xlink:href` to its target *anywhere in the document*, transitively, and
   emits the targets inside a `<defs>` appended before `</svg>` — `<defs>` because the definitions
   must be reachable by id and must render nothing where they are pasted.
2. **Count what usvg expands them into.** The leaf↔DOM pairing is positional and refuses the whole
   `<svg>` on a count mismatch. usvg *expands* `<use>`: a one-path symbol is one leaf, a two-shape
   `<g>` is two, a dangling reference is none — while the DOM walk saw an element with no element
   children and counted zero for all three. `use_leaf_count` resolves the reference in the DOM and
   counts the shape leaves the target contributes (starting **at** the target, since a referenced
   `<symbol>` renders its children even though a written one does not). The `<use>` is pushed once
   per leaf, and its box is that run's **union**, which is what `getBoundingClientRect` returns.

⚠ **Fixing either half alone makes a sprite page WORSE.** With the injection but not the counting,
usvg emits leaves the walk does not count; with the counting but not the injection, the walk counts
leaves usvg never emitted. Either way the pairing guard refuses the whole `<svg>` — so an ordinary
`<rect>` sharing the element goes from correct to `0×19`, a zero-width inline box one line-height
tall, because of the `<use>` beside it.

⚠⚠ **"One leaf per `<use>`" is the obvious implementation and it silently mis-attributes boxes.**
Measured as a mutation: a `<use>` of a two-shape `<g>` reported only the first shape, and the
*dangling* `<use>` next to it was handed the **circle's** bounds — a real box, plausibly sized,
belonging to a different element. That is precisely the failure the count-pairing guard exists to
prevent, and it passes four of the gate's six assertions.

**A dangling `<use>` still has a box.** Chrome gives it zero area at the `<svg>`'s origin. Dropping
it reads as a tidy result and is a MISSING_BOX where the page used to have a wrong one — the ledger
ranks missing as the worse of the two — so it is carried deliberately.

Chrome-measured, `g_svg_use_reference`:

```text
                                             CHROME        BEFORE       AFTER
  <use> of a <symbol> (24-unit vb at 20px)   3,25 13x10    0,20 0x19    3,25 13x10
  <use> of a <defs> path, x/y offset         3,43 17x17    0,40 0x19    3,43 17x17
  <use> of a multi-shape <g>                 1,105 15x6    0,104 0x19   1,105 15x6
  <use> beside an ordinary <rect>            24,84 10x10   NO-BOX       24,84 10x10
  a DANGLING <use href="#nope">              0,104 0x0     0,104 0x19   0,104 0x0
```

**Residue, unchanged:** `<symbol>`/`<defs>` content is `0x0` in Chrome and absent here (the
non-rendered-container skip, which predates this and is its own mechanism); an external-file
reference (`icons.svg#check`) is a fetch this pass does not do and reads as dangling; the SVG
`<text>` baseline still sits at the svg's top.

## A blockified inline is the spec's ANONYMOUS BLOCK — and the margin-collapse predicates could not see it (tick 746)

`is_block_level` has blockified a **block-in-inline** since tick ~384: an inline that contains an in-flow
block cannot stay in an inline formatting context, so CSS2 §9.2.1.1 splits it and wraps the runs in
anonymous block boxes, and we approximate that by making the inline itself block-level. Every other layout
decision in the crate reads that helper.

The two margin-collapse predicates did not. They tested the **raw** `display`:

```rust
fn top_margin_collapses(s: &ComputedStyle, cw: f32) -> bool {
    s.display == Display::Block && …
```

so a blockified `<a>` answered *"inline"* and became **opaque to the collapse**. The child's vertical
margins had nowhere to go and stayed inside it.

**Why that is the wrong answer and not merely a conservative one.** The blockified box stands in for the
spec's anonymous block boxes, and *an anonymous block box has no margin, border or padding of its own* —
there is nothing on it for the child's margin to be trapped by. Passing the margin through is not a
liberty; it is the only behaviour the substitution can have.

Chrome-measured on the exact shape (`manuk-wpt oracle --urls file://… --tol 0`, `margin:3px 2px 6px`,
`width:10px; height:10px`):

```text
                                              CHROME            BEFORE            AFTER
  <div><a><div m/></a></div>                  [0 3 1200x10]     [0 0 1200x19]     match
  <center><a><div m/></a></center>            [0 42 1200x10]    [0 39 1200x19]    match
  <div><a>text<div m/></a></div>              [0 78 1200x33]    [0 78 1200x39]    match
  <div><span><div m 7/9/></span></div>        [0 144 1200x10]   [0 137 1200x26]   match
```

**17 divergences over 17 probed nodes → 3**, and none of the 3 is a height (one is `<center>`'s
`-webkit-center` centring of a block descendant, two are 1px text-baseline residue). The third row is the
eligibility half: real inline text *before* the block is the first in-flow content, so the **top** margin
correctly stays in — while the **bottom** one still escapes, because the block is the last in-flow child.
Chrome does exactly that (33 = 20 + 3 + 10), and it is why the fix has to route both edges through the
shared predicate rather than only the one that produced the symptom.

**The fix is one helper, and its point is that there is now one implementation:**

```rust
fn collapses_as_block(dom, styles, node, s) -> bool {
    s.display == Display::Block
        || (s.display == Display::Inline && is_block_level(dom, styles, node))
}
```

Both predicates call it. Grepped first: those two were the **only** raw `display == Display::Block` tests
in the crate, so the class is closed rather than merely one instance fixed.

**Gate.** `a_block_inside_an_inline_collapses_its_margins_out` asserts both halves, RED-proven twice —
**M1** revert `collapses_as_block` to the raw display test (height 19, not 10); **M2** revert only
`bottom_margin_collapses` (height 16, not 10), which is what makes the bottom edge a real assertion
instead of a passenger on the top one.

**Corpus mass.** `C990e geometry: <a>` is the **#2** cluster in `docs/loop/CLUSTERS.md` — 97 sites, 15
classes, 4,964 hits (only `C01ca geometry: <div>` outranks it). `<a>` wrapping a block is the card link,
the nav item, the vote arrow; a wrong height there is a `dy` term that charges every sibling below it,
which is exactly what SHAPE measures.

---

## An indefinite MAIN size is INFINITE available main space, not zero (tick 762)

`solve_subtree` hands taffy an `AvailableSpace` for each axis. The width is always `Definite` (Manuk has
already resolved it in the block path); the height was:

```rust
height: match container_height {
    Some(h) => AvailableSpace::Definite(h),
    None    => AvailableSpace::MinContent,   // ← wrong when the block axis is the MAIN axis
}
```

For a **row** container the block axis is the cross axis, it does not decide line breaking, and the
container is content-sized either way — `MinContent` is harmless there. For a **column** container the
block axis is the **main** axis, and available main space is precisely what `flex-wrap: wrap` breaks
lines against (CSS Flexbox §9.3.5). `MinContent` means *"be as short as you can"*, so **every item taller
than nothing started its own flex line**: a vertical stack rendered as N side-by-side columns, each
`1/N` of the cross size.

Chrome, measured on `#c{display:flex;flex-direction:column;flex-wrap:wrap;min-height:100vh;width:1200px}`
with 200 / 900 / 150-tall children:

| box | Chrome | was | now |
|---|---|---|---|
| `#c` | `1200×1250` | `1200×900` | `1200×1250` |
| `#a` | `[0 0 1200×200]` | `[0 0 400×200]` | `[0 0 1200×200]` |
| `#b` | `[0 200 1200×900]` | `[400 0 400×900]` | `[0 200 1200×900]` |
| `#d` | `[0 1100 1200×150]` | `[800 0 400×150]` | `[0 1100 1200×150]` |

An auto-height column container has an **indefinite** main size, so all items share one line;
`min-height` only floors the resulting height. The fix passes `MaxContent` (taffy's "unbounded") for the
height when — and only when — the root is a flex container with `column`/`column-reverse` direction.

**What must NOT change, and was controlled for.** A column container with a *definite* height **does**
wrap (Chrome: `height:300px` → two columns), and that path already passes `Definite(h)`. A control run
with the change stashed moved exactly one of six cases:

| case | Chrome | before | after |
|---|---|---|---|
| column, nowrap | stacked | stacked ✓ | stacked ✓ |
| column, wrap, **auto** height | stacked | 3 columns ✗ | stacked ✓ |
| column, wrap, definite `height:300px` | 2 columns | 2 columns ✓ | 2 columns ✓ |
| row, wrap (the 12-column grid idiom) | 2 lines | 2 lines ✓ | 2 lines ✓ |
| grid, 2 columns | 2 columns | 2 columns ✓ | 2 columns ✓ |

**Why it survived this long: SHAPE cannot see it.** The fidelity score is *parent-relative* — each box is
compared against its nearest common ancestor frame — so a document displaced 2,201px **with** its
container scores about one point worse while being completely unusable. The `h_overflow` jarring
invariant is the channel that sees it: marktplaats.nl **742 → 0**, repubblica.it **139 → 0**.

**Gate.** `auto_height_column_flex_does_not_wrap` (`engine/layout/src/taffy_tree.rs`), RED-proven by
restoring `AvailableSpace::MinContent` — the two items land at x=0 / x=600 in one row of two lines.

**Residue, un-fixed and named:** a percentage height on a child of an indefinite-height column container
(`height:50%`) measures 9px here against Chrome's 18px (content height). The control proves it predates
this tick; it is a percentage-resolution question, not a wrapping one.

---

## `row` is a LOGICAL direction, and taffy only speaks physical (tick 764)

Taffy has **no `direction` property**. Its `FlexDirection::Row` means *left-to-right*, full stop. CSS's
`flex-direction: row` means *along the inline axis*, which under `direction: rtl` runs **right-to-left**
(CSS Flexbox §5.1). So the mapping is where the logical→physical resolution has to happen:

```rust
fn map_direction(d: CssDir, rtl: bool) -> FlexDirection {
    match (d, rtl) {
        (CssDir::Row, false) | (CssDir::RowReverse, true) => FlexDirection::Row,
        (CssDir::RowReverse, false) | (CssDir::Row, true) => FlexDirection::RowReverse,
        (CssDir::Column, _) => FlexDirection::Column,          // main axis is the BLOCK axis
        (CssDir::ColumnReverse, _) => FlexDirection::ColumnReverse,
    }
}
```

`row-reverse` under RTL swaps back to `Row`, which is the case that makes this a *mapping* rather than a
conditional. `column` is untouched: `direction` does not flip the block axis.

Measured against live Chromium (`<html dir=rtl>`, a 600px flex row of three 100px items, x within the
row): Chrome **500 / 400 / 300**; ours was 0 / 100 / 200.

**Real-site effect.** `mobile.ir` — the worst `reading_order` in the 200-site CrUX sample — went shape
**0.174 → 0.320**, `h_overflow` **268 → 1**, `reading_order` 874 → 820, with `coverage` and `shape_n`
identical across both runs. The LTR control (`marktplaats.nl`) was byte-identical.

**Gate.** `an_rtl_flex_row_runs_right_to_left`, RED-proven by dropping the `rtl` argument.

**What RTL still gets wrong, measured on the same fixture and recorded in `CONSTELLATION.tsv` rather than
left to be re-discovered:**

| | Chrome | ours |
|---|---|---|
| `body{width:600px}` in a 1200px viewport — the block's own x | **600** | 0 |
| `<li>` in a default `<ul>` (600px RTL body) | **x=0 w=560** | x=40 w=560 |
| RTL grid column order | reversed | not reversed (taffy has no grid equivalent) |

The first is the over-constrained block rule: in RTL the margin that gives is `margin-left`, so a block
narrower than its containing block sits flush **right**. The second is a logical-property question in the
UA sheet (`padding-inline-start`), and it is the same two-cascades surface as every other UA default.

**And the half that was already correct** — bidi shaping, intra-run reordering, mixed Arabic+Latin, two
spans on one line, `text-align: start` resolving to `right` — is Chrome-exact. That asymmetry is what let
`G_BIDI_BASE` stand in for the whole RTL web on the capability map for 549 ticks (surface audit #48).

---

## An RTL table's COLUMN AXIS runs right-to-left (tick 765)

`direction` on a table box orders the **columns**, not just the text inside them — the column axis follows
the inline direction (CSS 2.1 §17.5.3). So in `<html dir=rtl>` the first `<td>` in source order is the
**rightmost** cell. Measured against live Chromium (600px table, four 150px cells, x relative to the
table):

| cell | Chrome | was |
|---|---|---|
| 1st | **450** | 0 |
| 2nd | **300** | 150 |
| 3rd | **150** | 300 |
| 4th | **0** | 450 |

The implementation mirrors each cell's *span* inside the table's content box rather than reversing the
column list, which is what keeps `colspan` landing on the right cells:

```rust
let cx = if rtl_cols { content_x + content_w - (cx0 - content_x) - cw_span } else { cx0 };
```

**The direction is the TABLE's own**, not the document's: `<table style="direction:ltr">` inside an RTL
page keeps LTR column order, and Chrome agrees (fixture `#t2`: 0 / 300 in both engines, before and after).
That assertion is what makes this a *direction* fix rather than a *reverse the cells* fix.

**Real-site effect.** `mobile.ir`, the worst `reading_order` site in the 200-site CrUX sample: shape
**0.320 → 0.493**, `reading_order` **820 → 87**, `coverage` and `shape_n` unchanged; LTR control
byte-identical. Across ticks 764–765 the same site went shape **0.174 → 0.493** and `reading_order`
**874 → 87**.

**Gate.** `an_rtl_table_orders_its_columns_right_to_left`, RED-proven by forcing `rtl_cols = false`
(`0 150 300 450`).

### ⚠ The fix that was reverted to get here

The other RTL primitive on the same worklist — CSS 2.1 §10.3.3, *in RTL the over-constrained margin that
gives is `margin-left`, so a narrow block sits flush right* — was built first, matched Chromium on 7 of 8
fixture rows, and was **reverted**: `mobile.ir`'s `h_overflow` went 1 → 16, deterministic over two control
runs. Flush-right on a block whose containing block is already the wrong width points its content off the
right edge of the viewport, where flush-left had quietly hidden the same error.

It is re-scoped, not abandoned (`CONSTELLATION.tsv` carries the note): **land it after the containing-block
errors beneath it, not before.** The general rule — *when a spec-correct change makes a real page worse,
it is nearly always ORDER* — is the one worth keeping, together with the move that found it: ask the
mechanism oracle *why*, rather than defending the fixture.

---

## An RTL grid's COLUMN AXIS runs right-to-left — and taffy cannot be told (tick 766)

`direction` reverses a grid's inline-axis track order (CSS Grid §3: the column axis *is* the inline axis),
so under `dir=rtl` the first item lands in the **rightmost** column. Two of the three RTL axis fixes had a
natural home in the style mapping; this one does not:

- **flex (t764)** — swap `row` ⇄ `row-reverse` in `map_direction`. Taffy expresses it natively.
- **table (t765)** — mirror each cell's span inside the table's content box. Our own table code.
- **grid (t766)** — taffy has no `direction`, and `grid-auto-flow` is *not* a direction. There is nothing
  to swap.

So the mirror is applied to the **placed slots on the way out** of `solve_subtree`, recursively:

```rust
fn mirror_rtl_grid(&self, p: &mut Placed, content_w: f32) {
    p.slot.x = content_w - p.slot.x - p.slot.width;
}
```

Mirroring the slot is enough because `extract_placed` positions each subtree *relative to* its slot — the
whole child moves with it. The recursion re-mirrors any RTL grid nested inside the placed tree against
**its own** content width (padding and border subtracted), so a grid inside a flex row inside a grid is
each flipped in its own frame rather than all against the outermost one.

Measured against live Chromium (`<html dir=rtl>`, a 600px `1fr 1fr` grid, x relative to the grid):

| item | Chrome | was |
|---|---|---|
| 1 | **300** | 0 |
| 2 | **0** | 300 |
| 3 (wraps to row 2) | **300** | 0 |
| a `direction:ltr` grid in the same page | 0 / 100 | 0 / 100 (unchanged) |

**Real-site effect, and the arc across three ticks.** `mobile.ir` — the worst `reading_order` site in the
200-site CrUX sample:

| | t758 | after t764 (flex) | after t765 (table) | after t766 (grid) |
|---|---|---|---|---|
| shape | 0.174 | 0.320 | 0.493 | **0.523** |
| `reading_order` | 874 | 820 | 87 | **75** |
| `h_overflow` | 268 | 1 | 1 | **1** |

`coverage` (0.997) and `shape_n` (1186) are unchanged throughout, and the LTR control `marktplaats.nl`
is byte-identical at 0.708642 in every run — so the movement is the fixes, not the population.

**Gate.** `an_rtl_grid_orders_its_columns_right_to_left`, RED-proven by forcing `grid_is_rtl → false`.

### What is still wrong, pinned rather than guessed

An `li` with `display:inline-block` on `mobile.ir` sits at **x=1208 while its parent `ul` spans 363…918**
— 290px outside its own parent, and it is what makes 16 elements escape the viewport when the RTL
block-margin rule is applied. Two candidates were eliminated by fixture rather than by argument:
`float:right` is Chrome-exact, and table columns are correct as of t765. The cause is inline-level and
unknown; it is a `missing` row in `CONSTELLATION.tsv` and it is where the next RTL tick starts.

---

## `box-sizing` applies to a FLOAT too — and the float path is a SECOND width resolution (tick 770)

`layout_float` resolves its own width:

```rust
let width = match s.width {
    Dim::Auto if s.width_stretch => avail,
    Dim::Auto => self.shrink_to_fit(node, avail),
    other => other.resolve(cw, avail).max(0.0),   // ← the specified width, used as CONTENT width
};
```

That last arm is the bug. Under `box-sizing: border-box` a specified width is the **border box**, so the
content width is that minus padding and border — which `layout_block` has done for many ticks via
`bs_extra_w`, and which this function never learned.

Measured against live Chromium on the shape the corpus actually ships
(`*{box-sizing:border-box}` + `.card{width:50%;float:left;padding:0 5px}`, 704px container):

| box | Chrome | was | now |
|---|---|---|---|
| 1st float (border box) | **352** | 362 | 352 |
| its content | **342** | 352 | 342 |
| 2nd float's x | **352** | 362 | 352 |
| **the same box without `float`** | 352 / 342 | 352 / 342 | unchanged |

The last row is the control, in the same fixture: the non-float path was already exact. That is what
makes this a *float* defect rather than a *box-sizing* defect, and it is the reason the bug survived —
every direct test of `box-sizing` passed.

**Real-site effect.** `possssno.sbs` — coverage 1.000, shape 0.123, the sharpest single target on the
t767 ledger — went shape **0.123 → 0.430**. `marktplaats.nl` (no floats of this shape) was
byte-identical.

**Gate.** `box_sizing_border_box_applies_to_a_float`, RED-proven by dropping the arm (`float 0/362,
inner 5/352`).

### The audit this opens

**Any function that resolves `s.width` itself owes every width-modifying property** — `box-sizing`,
`min-width`, `max-width`, the intrinsic keywords. `layout_block` carries the full list; the variants
(float, flex/grid item, table cell, abspos) each resolve width separately. Diff their lists against
`layout_block`'s: it is a bounded audit with a known yield, because the forgotten copy is never the main
path — it is the variant, written once for its special case and never revisited as ordinary properties
land in the main path.

## An out-of-flow pseudo takes no advance — the custom-bullet idiom

`collect_inline_group` materialises `::before`/`::after` as ordinary inline **words** — the only place
generated content can enter the flow, since it is not in the DOM. It never consulted `position`.

So `.item::before { content: "–"; position: absolute; left: 0 }` over `padding-left: 20px` — **the**
custom-bullet idiom, and the shape that carries every pseudo icon, chevron and decorative bar on the web
— produced a marker that **took advance width**: it pushed the item's own text right by its own width
and drew itself where the text should have started. Against Chrome on `255md.com`, the dash was glued to
`ad delivery` instead of sitting 20px to its left.

`InlineItem::AbsPseudo` contributes **zero advance, zero inter-word space and zero line metrics**, and
paints at `dx` from the pen:

| inset | `dx` |
|---|---|
| `left: L` | `L − padding-left` |
| `right: R` | `−(R + padding-right)` |
| both `auto` | `0` — the static position; the box still takes no space, which is the part that matters |

Insets resolve against the containing block's **padding** box while the inline pen starts at the
**content** box, which is where the padding term comes from. It is exact whenever the owner is itself
the containing block — `position: relative` on the owner, which is what this idiom always writes.

### ⚠ Deliberately partial, and named so the next person knows which half exists

The **vertical** inset is not honoured: the fragment keeps the line's baseline, which is right for the
one-line markers and inline icons this idiom is made of and wrong for a tall block with
`::before { top: 0 }`. Nor does it walk to a positioned ancestor when the owner is `static`. Both need
the pseudo to become a real out-of-flow box with its own containing block. Same posture, and the same
reasoning, as the block path's clearfix `::after`.

### The two mutations the test demands

1. **Restore the in-flow behaviour** — the marker lands on the text and the text moves right.
2. **Apply the over-broad fix**: drop the pseudo instead of repositioning it. Every *positional* claim
   still passes. **Out of FLOW is not out of the PAGE** — the "both markers must still render" claim is
   the only thing standing between a placement fix and a missing-content bug.

A third guard, the in-flow control, stops the whole test passing by having removed all generated content
from the flow: a pseudo with no `position` must still push the text.

### And the burndown could not see any of it

Fixing this moved **zero** shape points on all six near-bar sites — as did tick 774's mojibake fix the
tick before. Shape scores **element** geometry; both defects live *inside* an element's box. When a
metric is used to RANK work, its blind spot silently deprioritises an entire class of visible defect —
and the near-bar pages it ranks are exactly where those defects sit.

## A form control does not inherit the page's font (t787)

A control's size is the browser's arithmetic, not the page's. Nothing in a document says how wide a
search box should be — so when that arithmetic is wrong, it is wrong on every form that does not
override it, and the error does not stay in the control: a text field's width is a container's width
one level up, and a textarea one line short pulls the whole page below it upward.

Read out of headless Chrome, body font `16px sans-serif` (so the control font is the UA's):

```
                      Chrome        ours (before)     ours (after)
<input size=1>         53×21          27×22            53×19
<input>  (size=20)    205×21         179×22           205×19
<input size=40>       365×21         339×22           365×19
<textarea>            182×36         179×22           182×36
<textarea rows=3>     182×51         179×22           182×51
```

**Three separate facts, each of which was a defect.**

1. **Chrome gives every control `font: -webkit-small-control`** — the ~13.3px system face, *not* the
   document's 16px. We inherited. Every control was ~20% too big in both axes. Authors who want
   inheritance ask for it (`input { font: inherit }` is in most resets) and the rule is UA-origin, so
   they still win.
2. **The `<input>` intercept is 45px border box; ours was 19.** The slope is exactly 8.0px/char in
   both engines — it is the *constant* that was 26px short, and on a short field the constant is most
   of the box.
3. **`rows` was never read.** An empty `<textarea>` sized to its empty content.

⚠ **One shared constant was wrong for one of them.** `<input>`'s intercept is 45px border box and
`<textarea>`'s is 22 — a text field reserves caret-scroll room a textarea does not. The old code used
one number for both, which is how it managed to be 3px off for textareas and 26px off for inputs at
the same time. Both terms are `font-size`-relative now, so an authored control font gets a
proportional box rather than one calibrated for a font it is not using.

**Measured residuals, named so they are not rediscovered.** `<input>` heights read 19 against Chrome's
21 (Chrome's inner editor clears 1px top and bottom; the textarea path carries that explicitly, the
auto-height input path does not). A `<select>`'s intrinsic width is short by **exactly 17px** — 142 vs
159 with a long option, 13 vs 30 with a one-character one, the same 17 either way. That is the
dropdown arrow, it is the whole of `chat.google.com`'s form cluster, and it belongs in layout where
the width is content-derived rather than in the UA-hint pass.

⚠⚠ **The transferable part is about the comment, not the code.** The old constant shipped with a note
saying it was *"the same approximation Chrome's own default ends up at (`size=20` → ~173px)"*. Chrome
ends up at 205. A calibration claim that reads as evidence, never throws, and is wrong by an amount
invisible without running the reference is the easiest kind of unmeasured claim to keep. **Any number
here that claims Chrome parity should carry the command that produced it.**

## The 17px a `<select>` reserves — and the property that says not to (t789)

A `<select>` sizes to its selected option, and then every engine adds room for the arrow it draws
beside it. Measured against headless Chrome:

```
  <select><option>English (United States)</option></select>    Chrome 159   ours 142
  <select><option>a</option></select>                          Chrome  30   ours  13
  <select id=z style="appearance:none">…long option…</select>  Chrome 139   ours 142
```

**The same 17px on a 24-character option and on a one-character one is the whole diagnosis.** A font
metric difference scales with the text; a constant across two very different inputs is a reserved slot.
That single observation turned *"our select text measures narrow"* into *"our select reserves no
arrow"* before any code was read.

⚠ **It cannot be reserved unconditionally.** `appearance: none` takes the native widget off the control
— Chrome's third row — so an unconditional 17px corrects the classic select and newly breaks every
restyled one, which is most of the modern web's design systems. That is a trade, and this project
refuses trades. So the property had to be read, and `clone_appearance()` is `engine="gecko"` in stylo
0.19: `appearance` is recovered from `MinimalCascade` and merged in `stylo_engine`, the same fence as
`scrollbar-width` and `-webkit-line-clamp`.

⚠ **`G_APPEARANCE_NONE` had concluded that reading this property would be theatre, and it was right at
the time.** Its measurement still stands: this engine draws no native widget, our controls are ordinary
UA CSS at lowest specificity, and an author rule already beats them — so `appearance: none` had nothing
to switch off *visually*. It has a reader now, and the reader is geometric rather than visual. **A
capability correctly measured as worth zero can acquire a value when a different subsystem starts
asking the question.**

The arrow is **reserved, not painted**: the strip is blank, deliberately. The box is what every sibling
and every ancestor is laid out against, and a right box with a missing glyph is a smaller error than a
wrong box.

⚠ **The site that motivated the lead did not move, and the reason is worth more than the fix.**
`chat.google.com`'s footer `<select>` reads ours 236 against Chrome's 162 — but its `<form>` and the
`<div>` above it are ALSO exactly 236 vs 162, so the control is filling an ancestor we size wrong and
its own intrinsic width never runs. The oracle filed the row under `<select>` because **the tag on a
cluster row is the tag of the element, not of the cause: a cluster keyed that way names the victim, not
the culprit.**

## A float belongs to its own block, not to the viewport (t792)

A float participates in its nearest **block formatting context** — that is why exclusion bands are
shared across nested plain blocks, and it must stay that way. But CSS 2.1 §9.5.1 rules 1 and 2 pin the
float to *its own containing block*. We conflated the two:

```html
<div style="width:300px"><div style="float:right;width:50px"></div></div>
      Chrome x = 250          ours x = 1150       (a 1200px viewport)
```

900px, on the most common legacy layout primitive there is — and a miss that size is never one wrong
box: it spawns overlap and reading-order violations across everything the float was meant to sit
beside. `en.wikipedia.org`, whose articles are built from floated infoboxes and thumbnails, went shape
**53.8% → 58.8%** on this one change.

The fix is to pass the containing block's content edges into float placement and clamp the hugged edge
against them, leaving the shared exclusion bands alone.

⚠ **The first draft clamped BOTH edges, and would have traded a 900px error for a 100px one.** *"A box
should not start outside its own block"* sounds right and is wrong: Chrome puts a 400px right float in
a 300px block at **x = -100** — the right edge stays pinned and the box overflows to the left. The gate
asserts −100 so the plausible version cannot come back.

⚠ **Residue, measured in the same fixture and not fixed here: a BFC root must not overlap preceding
floats.** Chrome moves an `overflow:hidden` block down to clear the floats above it; we leave it in
place. The gate therefore asserts **x only** — the y column carries a second, independent defect, and a
gate that is red for a reason it does not name is worse than one that states its scope.

## `order` lays items out in order-modified document order (t793)

Flexbox §5.4 and Grid §6.3: items are laid out in **order-modified document order**. `order: -1` to
pull the image above the copy, `order: 2` to send the sidebar after the article — it is how a
responsive layout rearranges blocks without touching the markup, and it is in essentially every design
system's breakpoint CSS. taffy has no `order` field, so the sort belongs where the items are collected.

```
  x positions, 400px rows of 100px items            Chrome        ours (before)
  second item has order:-1                       100   0  200     0  100  200
  order 3 / 1 / 2                                200   0  100     0  100  200
  the same, in a GRID                            100   0  200     0  100  200
```

**This is a reading-order defect, not a quiet missing property.** `reading-order` is scored over
sibling PAIRS, so one `order` flips every comparison across the reordered group at once — and it is the
jarring dimension this corpus is worst at (14.5% of in-scope sites clean).

⚠ **The tie is the whole specification of the sort.** Equal `order` — every item on most pages, since
the initial value is 0 — must keep DOCUMENT order, so the sort must be STABLE. An unstable one would
shuffle ordinary flex rows for no reason, on every page: a far worse bug than the one being fixed. The
sort is skipped entirely unless some item carries a non-zero `order`.

⚠ **And the DOM must not move.** `order` is visual only by design: the accessibility tree and
sequential focus read source order. An engine that reordered the tree would pass every box assertion
and silently rewrite what a screen reader announces.

### …and the containing block is the float's ORIGIN, not just its limit (t797)

t792 taught float placement that the containing block is a LIMIT — clamp into it. That is only half
the rule, and the missing half has a famous idiom:

```css
.row { margin: 0 -15px }        /* Bootstrap's gutter row */
.col { float: left }
```

A negative horizontal margin puts the row's content edges OUTSIDE the formatting context that owns the
exclusion bands. `place()` took its origin from `available()`, which folds the context's own
`left_edge` in as a floor — right for line content, wrong for a float whose containing block starts
further out. Chrome puts the column at **-15**; we put it at **0**, and a clamp cannot fix that
because `l.max(cb_left)` is a no-op whenever `cb_left` is the smaller number.

**The containing block is the origin; the floats are the obstacle.** Overlapping floats now push
inward from `cb_left`/`cb_right` rather than from the context's edges.

⚠ **A measured number is only measured for the fixture it was measured in.** The right-float half of
this case reads **265** in isolation and **150** in the gate's own fixture, where five earlier right
floats share the band — both are Chrome. Porting the isolated number into the gate is inventing it,
and it is the second time in seven ticks the gate caught an asserted-not-measured value. Extract the
gate's own `const HTML`, run THAT through Chrome.

### A percentage height on a flex item was resolved twice (t798)

`layout_flex` hands each item its taffy slot as the parent's definite height; `own_definite_h` then
re-resolved the item's own `height: 50%` against that slot. The percentage was applied twice and the
used height came out squared:

```
                                              Chrome   ours (before)
  height:50%  in a height:200px flex row        100        50          0.5² × 200
  height:25%  in a height:200px flex row         50        13          0.25² × 200
  height:50%  child of a 200px BLOCK            100       100          always right
```

Instrumenting the bridge showed taffy's own answers were already correct, so the squaring was entirely
on our side of the seam. The fix is `taffy_item_height` — record taffy's verdict before laying the
item out, exactly as `taffy_item_width` has done since **tick 14**, when this project fixed *the same
bug on the width axis* and left the mirror standing.

⚠ **The `pct_h` guard is a conservatism, not a proven necessity**: recording the slot for `auto` items
too passes every case in the gate, including two written to break it. The gate's RED list says so
rather than claiming a red it cannot produce.

⚠ Residue, measured alongside: a `height:auto` flex item in a ROW whose content is taller than the
container should stretch to the line and overflow (Chrome 30 in a 30px row); we keep the content
height (58).

## An anonymous block box INHERITS from the container that made it (t799)

When a block container holds *both* inline and block-level children, CSS 2.1 §9.2.1.1 wraps each run
of inline content in an **anonymous block box**. That box has no element, so it has no cascade of its
own — and the spec is explicit that it therefore **inherits every inheritable property from the block
container that generated it**.

`flush_inline_run` built those boxes with none of it. It called `layout_inline` with the literals

```rust
self.layout_inline(items, cx, start, cw, TextAlign::Left, 0.0, floats, None)
//                                       ^^^^^^^^^^^^^^^  ^^^         ^^^^
//                                       align            indent      strut
```

where the pure-IFC branch of the same file — the path taken when a container's children are *all*
inline — passes `bcs.text_align`, the resolved `text_indent`, and `Some(&bcs)`. **Two paths, one
formatting context, and only one of them knew what it inherited.**

### The trigger is "…and one block child", which is why it hid

The same markup renders correctly right up until a block-level element joins it:

```html
<div style="text-align:center"><span class="chip"></span></div>              <!-- centred ✓ -->
<div style="text-align:center"><span class="chip"></span><textarea></textarea></div>  <!-- x=0 ✗ -->
```

Nothing about the alignment changed; the *presence of a sibling* moved the inline run onto a code
path that dropped it. Measured against headless Chrome on a fixed-width `inline-block` (font-
independent, so the numbers are exact):

```
                                                     Chrome   ours (before)
  inline-block in text-align:center, INLINE-ONLY        350       350   ✓ always right
  …the same, with one block-level sibling               350         0
  …the run AFTER the block child                        350         0
  …between two block children                           350         0
  text-align:right, mixed                               700         0
  <center> with a block child                           350         0
  align inherited from a GRANDPARENT                    350         0
  a plain TEXT run, centred, mixed                      344         0
  default (left) with a block child                       0         0   ✓ must not move

  the anonymous line box's HEIGHT (20px inline-block)    24        20    ← the STRUT
```

### The strut is the second symptom of the same omission

`strut_style: None` gives the line box a zero strut. A **text** run survives that, because each
fragment carries its own inherited `line-height`; an **atomic** inline-block does not — it sits on the
baseline and Chrome adds the containing block's font *descent* below it. So a line whose only content
is a 20px inline-block was 20px here and is 24px in Chrome, and every mixed container was 4px short.

⚠ **A test had frozen that missing descent as ground truth.** `inline_block_boxes_flow_horizontally_
then_a_block_drops_below` asserted the following block lands at `y=30`, and its comment claimed the
number was *"verified numerically against Chrome by the parity harness"*. Chrome, run on that exact
markup, says **34**. A number asserted from an unverified claim of verification is the most expensive
kind: it defends the defect.

### `text-indent` is the third literal and is deliberately NOT fixed here

Chrome indents only the **first** anonymous run of a container: with `text-indent:40px` on a mixed
container, run 1 starts at x=40 and the run after the block child starts at x=0. Passing the indent
through unconditionally would over-indent every run but the first, so the literal `0.0` stays, with
the measurement written down, until a tick can pin the edge rule properly.

### Residue, measured in the same fixture

A `float:left` sibling *after* an inline run: we flush the pending run before registering the float,
so the float drops to the next line instead of sharing it, and the run centres in the full width
(350) rather than the float-narrowed band (Chrome 380). Pre-existing and untouched by this fix; the
gate asserts the property this tick delivers (the run is centred, not at x=0) rather than a number it
knows is wrong.

## A `max-width` clamp RE-RUNS the auto-margin split (t801)

```css
.container { max-width: 1200px; margin: 0 auto; }
```

That rule centres the content column of most of the modern web — every Bootstrap `.container`, every
Tailwind `mx-auto max-w-*`, every blog theme's article body. **It rendered flush left.**

CSS 2.1 §10.4 is one sentence: when the used width violates `max-width`, the §10.3.3 rules are
**applied again** with the constraint as the computed width. §10.3.3 is where a pair of `auto`
margins splits the leftover space. We did the first half and skipped the second — the auto-margin
block was guarded on

```rust
if s.width != Dim::Auto || s.width_keyword.is_some() {
```

which asks *did the author write a `width`*. For `max-width: 1200px; margin: 0 auto` the answer is no,
so the box was clamped to 1200 (correctly) and then placed at x=0 (not correctly), because the margins
were never told the width had become definite.

```
                                                   Chrome   ours (before)
  max-width:400px; margin:auto        in 800px       200          0
  max-width:400px; margin:0 auto      in 800px       200          0
  max-width:400px; margin-left:auto   in 800px       400          0
  …inside a 48px-padded parent                       200         48
  width:400px;     margin:0 auto      in 800px       200        200   ✓ always right
  max-width:400px; NO auto margin                      0          0   ✓ must not move
  max-width:1000px (NOT binding)                       0          0   ✓ must not move
  min-width:600px; width:100px; margin:0 auto        100        100   ✓ always right
```

The fix is the third term, `inline_constraint_violated` — a value the function already computed for
the replaced-element ratio case, one line above, and never used here.

### ⚠ Why the `min-width` half looked fine

Both constraints live in the same §10.4 sentence and only one of them was broken *observably*. A
clamp **upward** needs an explicit `width` to bind at all — a `width:auto` block already fills its
containing block, so `min-width` has nothing to raise — and an explicit width always satisfied the
guard's first term. So every `min-width` case in existence took the working path, and nothing could
distinguish *"we implement the §10.4 re-run"* from *"we don't"*. **One rule, two constraints, and the
one that needed no help was the one that worked.**

### `margin-left:auto` alone is what proves it is the SPLIT and not a special case

A plausible wrong fix is *"if a max-width clamped the box, centre it"*. `#m6` refutes it: with only
`margin-left:auto`, Chrome pushes the box fully right (x=400 in an 800px parent), because §10.3.3
gives the whole remainder to the single auto margin. The gate asserts that, so the plausible version
cannot come back.

## A form control does not inherit the page's `line-height` either (t802)

t787 gave form controls the UA's `font-family` and `font-size`, because Chrome's `html.css` says
`font: -webkit-small-control` and a control is a widget the browser sizes. **`line-height` is the
third property of that shorthand**, and a shorthand resets what it does not mention — so Chrome's
controls carry `line-height: normal` as a UA *declared* value, which beats inheritance.

We set two of the three. The page's own value walked back in through the door the shorthand closes:

```
   body { line-height: 1.7 }                     Chrome   ours (before)
   <textarea rows=5>   (UA font)                 182x81    182x119
   <textarea>          (rows=2 default)          182x36    182x51
   <select>                                       30x19     30x27
   <textarea line-height:2>  (AUTHOR)            182x86    182x86    ✓ author still wins
   <div> plain block                            1200x27   1200x27    ✓ still inherits 1.7
```

A `<textarea>`'s height is **rows × line-height**, so the error is proportional to the control: a
one-line field was 6px too tall and a five-row box was 38px too tall. And because
`body { line-height: 1.5 }`-ish is one of the most common typographic rules on the web, this landed
on essentially every styled form.

### The two constraints are what make it a fix rather than a trade

* An author's own `line-height` on the control **must still win** — the rule is UA-origin, and
  `line-height:2` on a textarea reads 86 in both engines before and after.
* A plain block **must still inherit** the body's value. A fix that reset `line-height` globally
  would correct every control and silently re-typeset every page.

### Residual, named

At an *author* font-size of 16px the textarea is 96 here against Chrome's 101 — `line-height: normal`
resolving to 18/row where Chrome uses 19. That is a font-metric question, independent of this rule
and unchanged by it, and the gate deliberately does not assert Chrome's number there.

## A TEXT NODE IS NEVER OUT OF FLOW (t803)

```html
<div style="position:absolute">Menu</div>
```

That box measured **0×0** — not misplaced, *sized to nothing*. Every dropdown item, tooltip, badge,
absolutely-positioned caption and `.sr-only` label whose content is bare text collapsed to a point.

`layout_children` filters a container's out-of-flow children out of the in-flow list:

```rust
.filter(|&k| { let s = self.style_of(k); !is_float(s) && !is_out_of_flow_positioned(s) })
```

**Under the Stylo cascade a bare text node carries a CLONE of its parent's style.** So inside a
`position:absolute` box, the box's own text answers *yes, I am out of flow* — and filters itself out
of the content it IS. No children left to measure, `shrink_to_fit` returns 0, content height 0.

An **element** child hides it completely, because a `<span>` carries its own `position: static`:

```
                                                       Chrome   ours (before)
  <div abspos>bare text</div>                           62x20     0x0
  …with padding:10px                                    82x40    20x20    ← the padding alone
  …with height:40px (width still auto)                  62x40     0x40
  <span abspos>bare text</span>                        130x20     0x0
  <div abspos>text<div/>text</div>  (MIXED)             70x52    70x12    ← both runs dropped
  <div position:fixed>bare text</div>                  101x20     0x0
  <div abspos left:0 right:0>bare text</div>           600x20   600x0
  <div abspos><span>elem child</span></div>             72x20    72x20    ✓ always right
  <div float:left>floated bare text</div>              115x20   115x20    ✓ always right
```

So the bug fires on exactly the shape people write and not on the shape a test-writer reaches for.

### The guard already existed, one function away

`max_content_width_uncached` documents this precise trap for `display:flex` — *"a bare run inside
`display:flex` reads back as `flex` here … routing it into the taffy path would build a tree whose
root measures via `measure_intrinsic`, which lands back in this function: unbounded recursion"* — and
guards it with `self.dom.is_element(node)`. Same cascade quirk, same guard, **four more call sites**
that never got it: the in-flow filter, the has-a-float check, the static-position loop, and the
block-children dispatch.

The fix is two node-aware predicates that every child filter must now use:

```rust
fn kid_is_float(&self, k: NodeId) -> bool {
    self.dom.is_element(k) && is_float(self.style_of(k))
}
```

The element check is not an optimisation — a text node has no box of its own, cannot be positioned and
cannot float, so it is the **predicate's precondition**.

### ⚠ What this fix EXPOSED, and it is a real defect that was being hidden

`en.wikipedia.org`'s `header > div:nth-of-type(1)` is Chrome 180px wide and is now **248px** here; it
matched before. That header is a flex container with an absolutely-positioned dropdown inside it, and
`taffy_tree::flex_items` pushes **every** element child, including out-of-flow ones. Flexbox §4.1 is
explicit that an absolutely-positioned child of a flex container **is not a flex item** and does not
contribute to the container's size — so this was always wrong, and it was invisible only because
those boxes measured zero.

Excluding them is not a one-liner: an all-auto-inset abspos box is placed from `static_pos`, which the
flex path never records, so removing it from `flex_items` without also recording that position makes
the box vanish (`position_absolutes` has a `continue` for exactly that case). Both halves together are
the next tick, and they are named here rather than folded in.

## `text-align: justify` — the slack goes into the WORD GAPS, not into one offset (t805)

`justify` was **parsed and then ignored**. `TextAlign::Justify` reached `close_line`, fell through the
`_ => 0.0` arm of the offset match, and rendered identically to `left` — for the engine's whole life.

Every other alignment is a single translation of the line, which is exactly why this one fell through:
it is the only value that is not an offset.

```
                                                     Chrome   ours (before)
  2nd word of a justified line                         49        45
  6th word of the same line                           237       220
  …the same words with NO justify (control)         45/220    45/220   ✓
  last line of a justified block                       43        43     ✓
  line ended by <br>, and the line after it          45/59     45/59    ✓
  one unbreakable word (no gaps)                        0         0     ✓
  an inline-block inside justified text                49        45
```

It does not degrade gently: on a justified paragraph **every word after the first is misplaced** and
the error grows along the line, so one paragraph yields dozens of divergences.

### The three call sites ARE the specification

CSS Text §7.3: `justify` justifies every line except the last, and except any line ended by a
**forced break** (those take `text-align-last`, `start` by default). `close_line` already had exactly
three callers — the `<br>` site, the wrap site, and the final flush — so eligibility is one boolean
per caller rather than a heuristic. Justifying a three-word last line across the whole column is the
most recognisable rendering bug the property has, and it is one wrong argument away.

### ⚠ Snapshot the gaps BEFORE shifting

A gap is where the next fragment starts after this one ends. Reading `line[i-1].x` inside the loop
that has *already moved* it compares a shifted fragment against an unshifted one, so every gap after
the first measures as closed and the expansion stops accumulating. Written that way first, the 2nd
word landed exactly right and the 6th was 10px short — **from the outside, a shift that stops
accumulating is indistinguishable from a slightly-wrong per-gap constant.** Two words on the same
line is what separates them, and both are in the gate.

## The space is a character — `letter-spacing` and the inter-word gap (t806)

`letter-spacing` adds a fixed advance after every character. We added it once per character of each
**word** and stopped there, so an inter-word space was the one character on the line that did not get
it.

That is the hardest shape a layout defect takes: **every word's own box stays exactly right while its
POSITION falls one `letter-spacing` behind per preceding space**, cumulatively along the line. The
quantity you would think to measure — the word's width — is correct.

```
   letter-spacing:2px, 16px sans-serif        Chrome   ours (before)
     2nd word   (one preceding space)           39       37
     4th word   (three preceding spaces)       115      109
   word-spacing:5px  (the sibling property)
     2nd word / 4th word                    36 / 106  36 / 106   ✓ always right
   no spacing, three faces                                       ✓ unchanged
```

The arithmetic is what identifies it rather than a fudge: at the 4th word Chrome has advanced
**12 characters × 2px** and we had advanced **9 × 2px** — exactly the three spaces missing, and
nothing else.

`word-spacing` — the sibling property, one line away in the same expression — was **always** applied
to the space. So a probe of "spacing" that happened to use `word-spacing` reports everything working.

`letter-spacing: .05em` on nav bars, buttons, headings and uppercase labels is design-system standard,
so this rides on a large share of the chrome of the modern web, and on every one of those runs
everything after the first word was in the wrong place.

## The padded inline BOX grows; the LINE does not (t808)

```html
<a class="btn" href="/login">Login</a>          .btn { padding: 10px 20px }
```

An inline `<a>` with padding is how every tag, badge, nav pill, chip and button-styled link on the web
is written. CSS 2.1 §10.6.1: on a non-replaced inline, vertical padding and border **do not affect
line height** — but the box still has them, so the pill *overflows* its line. That overflow is the
entire visual point of the idiom.

We grew neither. The box was its text's content area, so a 37px pill reported **18** — and painted
its background at 18, half the height the author drew.

```
                                             Chrome     ours (before)
  <a padding:10px 20px>Login</a>          [0 -9 79x37]  [0 0 79x18]
  <span padding:10px 0>  (VERTICAL only)  [0 31 61x37]  [… 61x17]
  <span border:5px solid>                 [0 76 76x27]  [… 76x18]
  <span display:inline-block padding:…>  [0 100 117x40] [… 117x40]  ✓ always right
  THE CONTAINING DIV                     [0 140 600x20] [… 600x20]  ✓ must not move
```

An **atomic** box (`inline-block`) was always correct, because it owns its own border box — which is
exactly the shape a test-writer reaches for, and why this survived.

### The containing div is what makes it a fix and not a trade

`close_line` folds a synthetic reporter's `line_height` in as a **floor on the line box**. That is
right for an empty inline (Chrome gives `<span id="anchor"></span>` a line-height-tall rect and a real
line) and wrong here. The first working version of this change reported 37 correctly on every anchor
**and made the containing div 37 too**, pushing every following line down the page. So a padded edge
now reports a tall RECT and a **zero** line-height, and the gate asserts the div at Chrome's 20
alongside every 37.

### Two arms, because the edges are not symmetric

The horizontal padding edges already existed as `InlineItem::Spacer`s (they occupy inline flow width);
they now carry `report_ascent` — how far above the baseline the rect starts, since the box begins
*above* its own text and a line-top-anchored rect cannot express that. And `padding: 10px 0` emits no
horizontal edge at all, so it needs a zero-width one — which does **not** hold a line box open,
because the measurement already recorded in `collect_inline_node` says only an edge occupying inline
flow width does.

## Not rendered is not `display: none` (t809)

Eight elements were hidden with `display: none` in the UA sheet. That produced the right BOX and the
wrong ANSWER for half of them. Measured out of headless Chrome with `getComputedStyle(el).display`:

```
   <source>    inline   ← we said none        <param>     none  ✓
   <track>     inline   ← we said none        <datalist>  none  ✓
   <area>      inline   ← we said none        <template>  none  ✓
   <noscript>  inline   ← we said none        <rp>        none  ✓
```

Those four generate no box because their **parent consumes them** — `<picture>`/`<video>` render
their `<img>`/media, `<map>` is not a container, `<noscript>` with scripting enabled holds raw text —
and *not* because a stylesheet hides them. The difference is invisible until a page asks, and
`getComputedStyle(source).display` is exactly what a responsive-image shim reads. `<picture><source>`
is how the entire modern web serves responsive images.

Half the list was already right, which is what makes the measurement worth taking rather than
reasoning by analogy: `<param>` and `<datalist>` really are `display: none` in Chrome, and a fix
applied to "the metadata elements" as a class would have broken them.

### The structural guard turned out not to be needed

The first version added a `never_rendered(tag)` check to `is_rendered`, to keep the four from drawing
once their `display` stopped being `none`. Disabling that check entirely changed **nothing** — on the
fixture or on the corpus (`mobcup.fm` reads 0.909091 either way) — because these elements' parents
never lay them out as content in the first place. It also *improved* `en.wikipedia.org`'s coverage
from 0.998141 to **1.000000**.

**A guard that cannot be shown to do anything is not a safety margin, it is unexplained machinery.**
The shipped change is the UA sheet edit alone, in both cascades.

### Both cascades, same tick

The `display: none` list exists twice — the Stylo UA sheet and `apply_ua_defaults`'s
`MinimalCascade` — and the second one's own comment warns: *"Keep in lockstep … The two cascades
disagreeing about which elements render at all is how a `<source>` ends up with 19px of height in one
configuration and none in the other."* Both moved together.

## A BFC root sits BESIDE a float, or below it (t811 — ⛔ REVERTED at t812, kept for the retry)

> ⛔ **This section describes a change that was landed at t811 and REVERTED at t812.** It costs
> `www.ta3lemkonline.com` — a float-heavy page with `reading_order` 816, which was not in the control
> set — **26 elements of 457** (0.540481 → 0.483589, bisected exactly against the t809 tree). Nine
> controls were byte-identical and that was true and not enough. The Chrome table below is measured
> and correct; it is kept here because the next attempt needs it, and because the missing piece is not
> the specification but an account of *why* a float-band narrowing costs a float-heavy page 26
> elements. Do not re-land it without answering that and without `ta3lemkonline` in the controls.

CSS 2.1 §9.5: *"the border box of a table, a block-level replaced element, or an element in the normal
flow that establishes a new block formatting context must not overlap the margin box of any floats in
the same block formatting context."*

We did neither half. A BFC root sat straight on top of the float.

```
   float:left 80x40, then a BFC root in a 300px column
                                       Chrome         ours (before)
     overflow:hidden               [80   0 220x20]   [0 … 300x20]
     display:flow-root             [80  50 220x20]   [0 … 300x20]
     display:flex                  [80 100 220x20]   [0 … 300x20]
     display:grid                  [80 150 220x20]   [0 … 300x20]
     float:RIGHT + overflow:hidden [0  320 220x20]   [0 … 300x20]
     overflow:hidden, width:280px  [0  290 280x20]   [0 … 280x20]  ← Chrome DROPS it to clear
     a PLAIN block (not a BFC)     [0  200 300x20]   [0 … 300x20]  ✓ correct in both
```

**The reach is the media object** — a floated avatar or thumbnail with an `overflow:hidden` /
`flow-root` / flex content block beside it: every comment thread, every card list, every article with
a pull-quote, and the standard pre-flexbox two-column idiom.

### Two halves

An `auto` width shrinks to the band and always fits. An **explicit** width that will not fit is moved
DOWN past the floats rather than squeezed — squeezing would also satisfy *"must not overlap"* and is
the wrong answer.

### The plain block is the rule's boundary, not an oversight

A non-BFC block's border box legitimately **does** overlap floats; only its *line boxes* avoid them,
which `open_band` already handles. Keying this on anything broader than `establishes_bfc` passes every
shifted row and is badly wrong on the commonest layout on the web.

### `left_float_edge`, not `left_offset`

The `Option` form reports the float-derived edge **alone** and is `None` when nothing overlaps;
`left_offset` falls back to the CONTEXT's edges, which are not this block's containing block when the
two are nested, and would shift blocks with no float near them. That is t797's distinction, reused
rather than rediscovered.

## A `display:table` with no rows is a shrink-to-fit BLOCK (t815)

`collect_table_rows` keeps only `table-row` / `table-row-group` **elements**. A `display:table` box
whose content is bare text — or any non-table content — therefore yields zero rows, and `layout_table`
produced an **empty box**. Not narrow: absent.

```
                                            Chrome        ours (before)
  display:table, bare text "short"       [0   0  36x20]     0x0
  display:table, a longer run of text    [0  20 213x20]     0x0
  display:table, width:200px, bare text  [0  86 200x20]     0x0     ← even EXPLICIT
  display:inline-table, bare text        [0 106  72x20]     0x0
  display:table + table-row + table-cell [0  40 109x20]   109x20    ✓ always right
```

An explicit width did not save it either, which is what rules out sizing and names the cause: the box
was never built.

**The fix is not a patch on the table formatter.** CSS 2.1 §17.2.1 wraps non-table content in an
anonymous table-cell inside an anonymous table-row, and a table with ONE anonymous cell is — in both
axes — exactly a shrink-to-fit block over the same content. `collect_table_rows` returns real DOM
ids, so there is no anonymous node it *could* return; instead the style clone in `layout_block` gets
`width: fit-content` and the generic block path runs. Anything that really has rows still goes to the
table formatter, which was never the defect.

**The reach is the pre-flexbox layout vocabulary** — `display:table; margin:0 auto` to shrink-wrap and
centre, `display:inline-table`, and the `display:table-cell; vertical-align:middle` centring trick.
Still everywhere in the CrUX tail.

⚠ **Third time in one session a bare text node fell through a structural filter**: t799 (an anonymous
block inherited nothing), t803 (a text node cloned its parent's `position:absolute` and filtered
itself out of the box it *was*), and this. **The recurring shape is a filter written for elements,
applied to a child list that contains text** — and the guard is always the same one word, `is_element`
or its equivalent, which `max_content_width_uncached` has had all along.

## An orphaned `table-cell` is ATOMIC, not a run of inline text (t816)

`display: table-cell` written **without** a `table`/`table-row` wrapper is the legacy
vertical-centring and equal-height-column idiom, and it is still everywhere in the CrUX tail. CSS 2.1
§17.2.1 wraps such a box in **anonymous table objects** — an anonymous row inside an anonymous table —
and the box that results is *atomic*: laid out as a block, then flowed like a word.

We had it in **neither of the two places that make a box atomic**, and the two omissions were the
same omission:

- the inline collector's atomic list read `InlineBlock | Flex | Grid | InlineFlex | InlineGrid`, so
  an orphan cell **fell through to the text recursion and was laid out as a plain non-replaced
  inline**;
- the `width: auto` shrink-to-fit arm in `own_definite_w` had the identical list, so once the box
  *was* atomic it filled its container instead of hugging its content.

Chrome-measured, `16px/1.25 sans-serif`, both halves landed together:

```text
                                          Chrome         before        after
  #ib  display:inline-block             [0   0  85x20]  [0  0 85x20]  unchanged ✓
  #c1  table-cell, no table             [0  30  85x20]  [0 31 85x17]  [0  30  85x20] ✗→✓
  #c2a two sibling cells share one row  [0  60  21x20]  [0 61 21x17]  [0  60  21x20] ✗→✓
  #c2b   …so they sit SIDE BY SIDE      [21 60  31x20]  [21 61 31x17] [21 60  31x20] ✗→✓
  #c4c cell inside an orphan table-row  [0 180  87x20]  [0 181 87x17] [0 180  87x20] ✗→✓
  #c5a a cell, then a real block, then  [0 210  45x20]  [0 211 45x17] [0 210  45x20] ✗→✓
  #c5b   …a cell — the block SPLITS     [0 230 600x20]  [0 230 600x20] unchanged ✓
  #c5c   the anonymous table run        [0 250  32x20]  [0 251 32x17] [0 250  32x20] ✗→✓
  #c6  a proper table/row/cell          [0 280  46x20]  [0 280 46x20] unchanged ✓
```

**An inline box is sized to its GLYPH BOX (17); an atomic one to its LINE BOX (20)** — and the
leftover half-leading is also what pushed `y` down by 1. So every orphan cell on the page was ~3px
short at the default metrics *and* one pixel low, with the error accumulating downward.

### The control is what makes this a diagnosis rather than a symptom

⚠ `#ib` is an `inline-block` with **byte-identical content** and it was already Chrome-exact at
`85x20`. Without that row the whole table is equally consistent with a general line-height or
half-leading error — the kind this project has chased before — and the fix would have been aimed at
the strut. **One control turned "our boxes are 3px short" into a statement about one code path.**
`#c6` (a properly structured table) and `#c5b` (a real in-flow block) are the same argument from the
other side: both were always right, and both had to stay right.

### Two edits, one behaviour — and the mutation proves which

Reverting the shrink-to-fit arm alone goes red (`600x20`: the right height at the full container
width). But **adding that arm alone, to the original code, would have been a no-op** — and the
mutation demonstrates it instead of claiming it: reverting the *atomic* half while leaving the
shrink-to-fit arm in place reproduces the untouched baseline to the pixel, `85x17` at `y=31`, because
`layout_block` — the only caller of that arm — is never reached for a box that is still a
text-recursion inline. The second edit is measurable only once the first has landed.

### The residue, named rather than blamed

⚠ A `table-cell` inside a table with an **explicit height** must stretch to fill it: Chrome gives
`[0 90 300x80]` for the classic `display:table{height:80px}` + `vertical-align:middle` pair, and we
give `[0 92 67x20]`. This fix takes that box from `67x17` to `67x20` — it is a cell, so it now gets a
line box — but it does **not** stretch it. That needs anonymous-row generation *inside* a real table
plus cell stretching: a different mechanism in a different function. `G_ORPHAN_TABLE_CELL` asserts it
at **our** number on purpose, so a future fix has to come and change that line deliberately. Per t814,
a residue's stated cause is a guess until it has been measured on its own.

**Gate:** `G_ORPHAN_TABLE_CELL` (`engine/page/tests/g_orphan_table_cell.rs`).

## A sub-pixel float excess breaks a flex line — and Bootstrap is written in exactly those percentages (t817)

taffy collects flex items into lines with a bare `>` and no tolerance
(`taffy-0.12.1/src/compute/flexbox.rs:930`):

```rust
line_length += child.hypothetical_outer_size.main(constants.dir) + gap_contribution;
line_length > main_axis_available_space && idx != 0
```

`width: 66.66666667%` is not representable in binary. As `f32` it resolves against a 1200px row to
`800.00004`, its `33.33333333%` sibling to `400.00002`, and the pair sums to a **hair over 1200** —
which is enough. The second column starts a new flex line and the two columns stack.

**Chrome never sees it.** Blink quantises every resolved length to `LayoutUnit` — **1/64 CSS px** —
before anything compares them, so the same pair is exactly `800 + 400 = 1200` and fits.

Those are Bootstrap's literal column widths. `.col-8` ships as `width: 66.66666667%` and `.col-4` as
`33.33333333%`, so **a two-column Bootstrap 5 row stacked instead of sitting side by side, on every
page that uses one** — and because such cards' overlays are usually absolutely positioned, they then
landed on top of each other rather than merely flowing wrong.

Chrome-measured on a 1200px `flex-wrap: wrap` row, x of the SECOND item:

```text
  width pair                      Chrome x    before        after
  50% + 50%                          600     600           unchanged ✓  (exact in binary)
  75% + 25%                          900     900           unchanged ✓  (exact in binary)
  66.6667% + 33.3333%                800     800           unchanged ✓  (sums UNDER 100)
  66.66% + 33.33%                    800     800           unchanged ✓  (sums under 100)
  66.66666667% + 33.33333333%        800     0, WRAPPED    800  ✗→✓   ← Bootstrap 5
  66.666667% + 33.333333%            800     0, WRAPPED    800  ✗→✓
  33.33333333% × 3  (3rd item)       800     0, WRAPPED    800  ✗→✓
  70% + 40%  (genuinely too wide)      0, WRAPPED in BOTH states — asserted
```

⚠ The three thirds are the sharpest row: `33.33333333% × 3` sums to **under** 100% in decimal and
still overflowed, because each one rounds *up* in `f32`. The defect is binary representability, not
the digit count and not the decimal sum.

### The fix, and why it is where it is

taffy is a crates.io dependency and its resolver is not ours to patch, so the quantisation happens on
our side of the boundary: `solve_subtree` already knows the container's resolved content width, and
`snap_row_item_percent_widths` converts each direct child's percentage main-axis width into a length
snapped to the 1/64 px grid before taffy runs. Anything that is not a percentage, not a width, or not
a direct child of a `row` flex container is untouched.

### Bounds, stated rather than glossed

⚠ **Direct children only.** A flex container nested *inside* a flex item has a content width that
taffy itself decides, so its children keep raw `f32` resolution and can still lose a line-break by a
sub-pixel. That needs the quantisation inside taffy's resolver.

⚠ **`row` containers only.** Line breaking is a main-axis question; a `column` container breaks on
height, where the main size is usually indefinite and there is no definite base to snap against.

⚠ **The gate cannot prove the 1/64 constant.** Snapping to whole pixels (`LAYOUT_UNIT = 1.0`) passes
the fixture identically, because its percentages of 1200px all land on integers. 1/64 is chosen
because it is *Chrome's actual quantum*, and `G_FLEX_PERCENT_LINEBREAK` says so rather than implying
a red it cannot produce.

⚠ **CORRECTED AT t819 — this paragraph originally called Bootstrap 4's defect "a flex-basis
defect". It is not.** `flex: 0 0 66.666667%; max-width: 66.666667%` comes out `533` / `133` against
Chrome's `800` / `400`, but measured on its own **the flex-basis percentage is CORRECT**: drop the
`max-width` and the same row is exactly `800` / `400`. t819 extended the snap to `flex_basis` (the
line-break half) and that row is now Chrome-exact. What remains is `max-width: <pct>` on a flex item
resolving against the item's **own** taffy-assigned width instead of its containing block —
`800 × 0.666667 = 533` — which is the height axis's documented `taffy_item_height` shape appearing on
the width axis. ✅ **CLOSED AT t823** — see *"A flex/grid item's SLOT is a finished answer"* below. The
label was right and understated: measured on its own it also caught the item's MARGINS being applied
twice, which is not a percentage bug and needs no framework.

**t819 addendum — `flex-basis` is a main size too, and leaving it out left the fix half-done.**
`flex: 0 0 <pct>` never touches `width`; the hypothetical main size comes from the BASIS. Those rows
came out the right *widths* (800/400) on the *wrong lines* until `flex_basis` was snapped as well.
Both properties are handled in `snap_row_item_percent_widths`, and `G_FLEX_PERCENT_LINEBREAK` asserts
the shorthand and the `flex-basis` longhand separately so a shorthand-parsing change cannot silently
take both.

**Gate:** `G_FLEX_PERCENT_LINEBREAK` (`engine/page/tests/g_flex_percent_linebreak.rs`).

## A flex/grid item's SLOT is a finished answer, not an input (t823) — and two things kept recomputing on top of it

**This closes the `max-width` residue the section above ends on, and it turned out not to be alone.**

Taffy does three things to a flex/grid item: it resolves the item's `width`, it applies the item's
`min-width`/`max-width` clamp **against the real containing block**, and it positions the slot with
the item's **margins already taken out of the line**. `layout_block` then ran over that answer and did
two of those three a second time, using the SLOT as the containing block.

Tick ~700 had already found the third case and fixed it — that is the `taffy_item_width` map and the
comment beginning *"A flex/grid item's width was already decided by taffy — do not resolve it a second
time"*. **The sentence was right and its coverage was one property wide.** Ten lines further down, the
min/max clamp was still resolving percentages against `cw`; twenty lines up, `border_x = x + ml` was
still spending a margin taffy had already spent.

Chrome-measured, 1200px `display:flex; flex-wrap:wrap` row and an `800px 400px` grid:

```text
                                                 Chrome           before      after
  flex:0 0 90%; max-width:50%                  [  0 600]        300 wide     600  ✗→✓
  width:90%;    max-width:50%                  [  0 600]        300 wide     600  ✗→✓
  flex:0 0 66.666667%; max-width:66.666667%    [  0 800]        533 wide     800  ✗→✓  ← Bootstrap 4
  flex:0 0 33.333333%; max-width:33.333333%    [800 400]        133 wide     400  ✗→✓  ← Bootstrap 4
  flex:0 0 50%; margin-left:100px              [100 600]        x = 200      100  ✗→✓
  flex:0 0 50%; margin-left:10%                [120 600]        x = 180      120  ✗→✓
  grid item, 800px track, max-width:50%        [  0 400]        200 wide     400  ✗→✓
  grid item, 400px track, margin-left:10%      [840 360]        x = 876      840  ✗→✓
  flex:0 0 10%; min-width:300px                [  0 300]        300          300   ✓  guard
  flex:0 0 90%; max-width:300px                [  0 300]        300          300   ✓  guard
  flex:0 0 50%; padding-left:100px / 10%       [0 700]/[0 720]    same        same  ✓  guard
  plain block, max-width:50% / margin-left:10% [0 600]/[120 600]  same        same  ✓  control
```

### ⚠ The asymmetry that hid it: a percentage clamp always binds again, a pixel one never does

Of the four min/max × px/pct combinations, **exactly one is observable**:

| | `px` | `<pct>` |
|---|---|---|
| **`max-width`** | no-op — 300px against an already-300px slot | ✗ **the percentage SQUARED** — 50% of the 50% answer is 25% of the container |
| **`min-width`** | no-op | latently wrong, unobservable — a percentage of the slot can never exceed the slot |

So the two rows anyone would reach for first to sanity-check a clamp (`max-width: 300px`,
`min-width: 300px`) **cannot fail**, and were green through the whole defect. They are asserted in the
gate as guards for exactly that reason: they are also what now holds the fix honest, because the clamp
is skipped *wholesale* for a taffy item rather than re-resolved. If taffy ever stopped applying
`max-width`, those rows go red here instead of being silently covered by our own second clamp.

### The margin half needs no percentage and no framework

`margin-left: 100px` on a flex item put the box at **x = 200**. `extract_placed` passes
`base + slot.x` straight into `layout_block`, and `slot.x` is taffy's placement — which already has
the margin in it. Every margined flex item in the corpus moved by twice its margin, and the same
double-count was in `extract_placed`'s content-height accumulator (`slot.y + margin_top + …`).

### The fix

One boolean, `taffy_item = taffy_known.is_some()`, gating four sites in `layout_block`: the min/max
clamp, the auto-margin re-centring (taffy is what distributes `ml-auto` free space — against the LINE,
not against this item's slot), and the `ml`/`mt` terms of `border_x`/`border_y`; plus the accumulator
in `extract_placed`. Padding is deliberately **not** included: taffy's slot is a BORDER box, so
subtracting the item's own padding from it here is correct and stays (the two padding rows above are
the guard that says so).

**Gate:** `G_FLEX_ITEM_SLOT_IS_FINAL` (`engine/page/tests/g_flex_item_slot_is_final.rs`) — proven RED
twice, one mutation per half.

⚠ **The lesson for the next seam like this: a guard is written for the property that was failing, not
for the rule it enforces.** When a value is guarded, grep every other *consumer* of that value. This
is the "one rule, N implementations" shape arriving as one rule with N **readers**.

### t827 addendum — the same rule on the BLOCK axis, and a defect hidden by a later write

`pch` for a taffy item is `Some(p.slot.height)`, so the `min-height`/`max-height` clamp squared its
percentage exactly as the inline one did. Chrome-measured on a 400px `display:flex` row:

```text
                                                   Chrome    before   after
  flex:0 0 50%; height:100%; max-height:50%       600x200     100      200   ✗→✓
  …the same with max-height:200px                 600x200     200      200    ✓  guard
  …with height:10%; min-height:50%                600x200     200      200    ✓  guard
  column-flex item, max-height:50% (no height)    600x200     200      200    ✓  guard
  grid item in a 300px track, max-height:50%      600x150     150      150    ✓  guard
  plain block, height:100%; max-height:50%        600x200     200      200    ✓  control
```

⚠⚠⚠ **ONE ROW OF SIX WAS OBSERVABLE, AND THE SECOND MASK IS THE INTERESTING ONE.** Beyond the px/pct
asymmetry, a percentage `max-height` *still* hides unless the item also carries a percentage `height`
— because with `height: auto` the item's box is overwritten by `extract_placed`'s slot adoption
(`if height == Auto && slot.height > rect.height { rect.height = slot.height }`) **after** the clamp
runs. The wrong arithmetic produced the right box.

**A defect masked by a later assignment is invisible to every test that checks the final box.** Input
variation cannot reach it — only asking why a row that *should* be wrong is right. The clamp is now
skipped for taffy items on both axes, and `#v1` stays in the gate as the row that documents the mask.

⚠ HONEST SCOPE: all seven anchors byte-identical across this change. The idiom needs `height: <pct>`
AND `max-height: <pct>` on a flex/grid item; this completes a mechanism rather than moving a corpus.

## A shrink-to-fit box hugged its text ONE PADDING too tightly — then still re-wrapped it (tick 830)

**Symptom.** `www.kicktipp.com`'s footer link: Chrome `[742 790 103x30]`, ours `[749 804 96x48]`. One
line in Chrome, two here, and the height error cascades down the subtree. This is the burndown's #1
named mechanism (*"container-WIDTH errors LAUNDER into dy"*) caught in the act — but it took **two
independent defects** to produce, and fixing either alone leaves the site failing.

### ⓵ The FILL_SENTINEL discard is asymmetric

`content_right_extent` reads max-content by laying the subtree out at a 1e6 available width and
measuring how far its content reached. A block-level child *fills* that width, so its own
`rect.width` (≈1e6) is meaningless as a contribution — the walk discards the box and recurses to the
inline text that carries the real extent. Correct, and **asymmetric**:

- the discarded box's **left** padding / border / margin survive the discard *for free* — they are
  baked into where its descendants were laid out, so they arrive in the fragment's `x`;
- its **right** ones have no content after them to carry them, so discarding the box discards them.

Measured against headless Chrome, a `13.2px/17.16px Arial` run inside a `box-sizing: border-box;
padding: 6.6px` block, itself inside a shrink-to-fit box:

```text
  outer box                       Chrome    before    after
  flex item                        86.5      80.0      86.5   ✗→✓
  inline-block                     86.5      80.0      86.5   ✗→✓
  float: left                      86.5      80.0      86.5   ✗→✓
  position: absolute               86.5      80.0      86.5   ✗→✓
  display: table                   86.5      80.0      86.5   ✗→✓
  grid item                        86.5      80.0      86.5   ✗→✓
  padding on the BOX ITSELF        86.5      86.5      86.5   ← guard: says which half was broken
  margin: 0 10px on the child      93.3      83.3      93.3   ✗→✓  same loss, other property
  border: 3px on the box itself    83.3      83.3      83.3   ← guard
```

**Fix.** Carry the skipped box's right insets (`margin-right + padding-right + border-right`) down
the walk as a `pending` term, and add it to whatever extent its descendants report.

### ⓶ A box sized to its own max-content re-wrapped the run it was measured from

With ⓵ landed the `<a>` matched Chrome's `103` **exactly** — and was still 48px tall.

max-content is read by laying the run out unbounded and measuring the reach; the box is then given
exactly that number and the run is laid out **again** against it. The second pass accumulates the
same fragment advances in a different order and can land a few thousandths of a pixel *over* — and
the line breaker has no tolerance, so it takes the break. Bisected by giving the box explicit widths:

```text
  width           lines
  89.520px          2      ← what max-content reported
  89.525px          1      ← what the box's own re-layout needs
```

⚠⚠⚠ **A BOX SIZED TO ITS OWN MAX-CONTENT MUST FIT ITS OWN CONTENT, AND ON A BARE `f32` IT DOES NOT.**

Blink cannot reach this state: a preferred width is a `LayoutUnit` built with `FromFloatCeil`, so the
quantisation is **outward** — the box is never smaller than the content it was measured from. This is
the tick 813-818 quantum (`snap_to_layout_unit`, 1/64 px) with the rounding direction reversed, and
the direction is the whole point. `taffy_tree::ceil_to_layout_unit` now snaps every intrinsic width
out: `max_content_width`, `min_content_width`, and the table-cell `(min, max)` pair.

### What it bought, against an OLD-BINARY CONTROL

Same 14 sites, same hour, one binary apart (a live site moves on its own — a per-site delta measured
against a banked row from six hours ago is not a result):

```text
  site                            shape old   shape new    delta
  www.kicktipp.com                 0.7349      0.8313     +0.0964   ★ crossed 0.75
  celeb.gate.cc                    0.7284      0.7832     +0.0547   ★ crossed 0.75
  www.library.chiyoda.tokyo.jp     0.7472      0.7528     +0.0056   ★ crossed 0.75
  en.wikipedia.org                 0.6066      0.6242     +0.0177
  momon-ga.com                     0.5682      0.5699     +0.0017
  blog.rust-lang.org               0.9934      0.9946     +0.0012
  (8 others)                          —           —       +0.0000
  mean                                                    +0.0127
```

**Zero regressions; `coverage` byte-identical on all 14** — so this is layout math, not a denominator
effect (t825's rule: a shape move at unchanged coverage cannot be composition).

### Two lessons, and the second is about the gate

⚠⚠⚠ **THE SECOND DEFECT WAS ONLY VISIBLE BECAUSE THE FIRST ONE LANDED.** A residue narrowed to *"the
width is wrong"* would have been closed by ⓵, and kicktipp would still have failed. The aim survived
because the check was **the height against Chrome**, not the width against my own hypothesis.

⚠⚠ **AND THE SECOND GATE WAS VACUOUS ON ITS FIRST WRITING.** The exact fixture that reproduces on the
shipping path (`Page::load`/Stylo) **passed under `MinimalCascade` with the fix mutated out** — t826's
two-cascade trap arriving from the other direction: not a false positive, a false GREEN. Finding a
case the unit harness can actually see took a brute-force scan of 8 strings × 8 sizes *under the
mutated build*. **A gate written from a live-site reduction must be re-falsified under the harness it
will live in**, not merely under the engine it was found in.

Gates: `shrink_to_fit_counts_the_right_padding_of_a_filled_block_child` and
`a_flex_item_at_its_own_max_content_does_not_rewrap_its_own_text` (`manuk-layout`), each RED-proven by
mutating out its own half.

## A float is a SECOND width resolution, and it never learned three of the block path's rules (t831)

`layout_float` resolves width and height itself rather than routing through `layout_block`. That is
defensible — a float shrink-to-fits where a block fills — but it means every rule the block path
holds has to be **re-landed here by hand**, and three of them never were. They were found together
because they are all one grep away from each other in one function.

### The aim, and it came from the scorer rather than from a hand-rolled probe

Six sites in the sweep share a signature that is close to a diagnosis on its own — `coverage
1.000000`, `h_overflow 0`, `overlap 0`, and shape stuck between 0.55 and 0.66. Every box drawn,
nothing overflowing, nothing overlapping, and a third of the boxes in the wrong place: that is
layout math, not coverage and not keying.

`fidelity --shape-dump N` (new this tick) prints the per-element misses **the scorer already
computes and threw away**, worst-first, in the frame the score is computed in. On the smallest
member — `app.ordertime.com`, 29 elements — it named the mechanism in one command:

```text
  height +32  c[0 0 113x40]   m[0 0 113x8]    div
  height +32  c[12 2 101x36]  m[12 2 101x4]   a
  height +32  c[0 2 101x32]   m[0 2 101x0]    img   ← ZERO HEIGHT
  width  +14  c[16 13 14x14]  m[16 13 0x16]   img   ← ZERO WIDTH
```

`boxes --images` then showed both images decoded with `natural 101x32` and `14x14` — **exactly
Chrome's boxes**. So the intrinsic sizes were in hand and layout produced zero anyway. The site's
CSS said why: `.logo a img { float:left }` and `.help a img { float:left }`.

### ⓵ A floated replaced element has no content, so without its ratio it has no size

An `<img>` has no children. `layout_float`'s height was its content's height and its `auto` width
was `shrink_to_fit`, and neither consulted `aspect_ratio` — which `layout_block` has done for as
long as it has had the field. Measured against Chrome on the two PNGs above, **with the identical
unfloated image in the same document as the control**:

```text
                                       Chrome   before   after
  float:left, no width/height          101x32   101x0    101x32   ✗→✓
  float:left, height=16 attr only       16x16     0x16     16x16   ✗→✓
  the SAME image, NOT floated          101x32   101x32   101x32    ✓  ← control
```

⚠ **The control is what turned a symptom into a diagnosis** (t813-818's rule, again): the two
resolutions disagreed *inside one document*, which says the bug is the second resolution and not
the ratio machinery.

### ⓶ `min-width` / `max-width` / `min-height` / `max-height` did not exist on this path

Not mis-applied — **the words did not appear in the function**. Measured on plain floated `<div>`s
so that no replaced-element machinery could explain the numbers away:

```text
                                             Chrome   before   after
  float, width:200px; max-width:50px         50x10    200x10   50x10   ✗→✓
  float, width:20px;  min-width:80px         80x10     20x10   80x10   ✗→✓
  float, width:10px; height:200px; max-h:50  10x50    10x200   10x50   ✗→✓
  float, width:10px; height:20px;  min-h:80  10x80     10x20   10x80   ✗→✓
```

`.col { float:left; width:50%; max-width:600px }` is the entire pre-flexbox responsive column and
`img { max-width:100% }` is in every CSS reset written since 2011, so this is not an edge of the
float path — it is most of what floats are used for.

With the clamp comes **CSS 2.1 §10.4 in both directions**: a clamp that moves a replaced element's
used size in one axis recomputes the other through the ratio, or the picture renders stretched.

```text
                                                 Chrome   before   after
  float, 101x32 image, max-width:50px            50x16    101x0    50x16   ✗→✓
  float, 14x14 image, height=16, max-height:14   14x14      0x16    14x14   ✗→✓
```

The second row **is** `app.ordertime.com`: `.help img { max-height:14px; max-width:14px }` over an
`<img height="16">` is where its `0x16` against Chrome's `14x14` came from.

### ⓷ And `box-sizing: border-box` on the block axis

The width arm got `box-sizing` in an earlier tick; the height arm, four lines below it, did not — so
a border-box float came out padding+border too tall (Chrome 100×100, ours 100×120). **One rule, two
axes, one of them landed.** The existing `box_sizing_border_box_applies_to_a_float` test passed the
whole time because it only ever asked about width.

### Priced against an OLD-BINARY control, 16 sites, same hour

```text
  app.ordertime.com                  0.6552 → 0.8621   +0.2069   ★ CROSSED 0.75
  littlecaesarsbcs.libellum.com.mx   0.6154 → 0.9487   +0.3333   ★ CROSSED 0.75
  14 others                                            +0.0000
  mean +0.0338 · M1 crossings 2 · ZERO regressions · coverage byte-identical on all 16
```

⚠ **`littlecaesarsbcs` was never looked at.** It was in the cohort, it shared the mechanism, and it
moved +0.33 on its own — which is what a cohort is *for*. Equally: the four other cov=1.000 members
did **not** move, so the signature was a lead, not a single cause. Both halves of that are the
result.

Gates (`manuk-layout`): `a_floated_replaced_element_derives_its_missing_axis_from_its_ratio`,
`a_float_clamps_both_axes_by_min_and_max`,
`a_max_constraint_on_a_floated_image_transfers_through_its_ratio`, and a block-axis assertion added
to `box_sizing_border_box_applies_to_a_float` — each RED-proven by mutating out its own half, and
each proven to leave the *other* halves green, so the four are separable rather than one gate wearing
four names.

## §10.4 runs BLOCK → INLINE too, and the block path only ever ran it one way (t833)

CSS 2.1 §10.4 is symmetric: when a used size violates a `min-`/`max-` constraint, the rules are
applied **again** with the constraint as the computed value — and for a replaced element with an
intrinsic ratio that means the *other* axis is recomputed so the ratio survives. `layout_block` has
had the inline→block half for a long time (`inline_constraint_violated`: a `max-width` that moves
the width recomputes the height). The block→inline half was never written, so a `max-height` clamp
left the box with the width it had **before** the clamp, and the picture rendered stretched.

Aimed by t832's banked residue: `admin.zoomph.com`, one `<img>` at `320x30` against Chrome's
`113x30`. `boxes --images` gave the input — `natural 1000x266` — and 113/30 = 3.767 against
1000/266 = 3.759, so Chrome was sizing from the height and deriving the width. The site is the AWS
Cognito hosted login page: `.logo-customizable { max-width:100%; max-height:30px }`.

```text
                                             Chrome     before      after
  max-width:100% + max-height:30px           113x30     320x30     113x30    ✗→✓
  max-width:100% alone                       320x85     320x85     320x85     ✓  ← control
  max-height:30px alone                      113x30    1000x30     113x30    ✗→✓
  …+ display:block; margin:0 auto        113x30 @104  113x30 @0  113x30 @104 ✗→✓
```

The `max-width`-alone control is what names **which half** was missing — it was already correct.
The centred row is why the fix re-runs the auto-margin split rather than only assigning a width:
§10.3.3 is where two `auto` margins share the remainder, so a width assigned without re-splitting
leaves a correctly-sized image flush left — a new bug wearing the old one's fix.

Moving the width this late is safe **only** under the `is_replaced_element` guard: a replaced box
has no children, so nothing was laid out against the old width.

⚠⚠⚠ **THIS IS t831'S PATTERN NOTE ARRIVING FROM THE OTHER DIRECTION.** t831 concluded that
`layout_float` accumulates `layout_block`'s backlog. One tick later the debt ran the other way — the
float path had both §10.4 directions and the block path had one. **Two implementations of one rule
drift in whichever direction the last fix landed, so the grep is symmetric or it is not a grep.**

⚠⚠ **HONEST SCOPE: this bought ZERO M1 crossings.** Priced against the t831 binary over 16 sites in
the same hour: `admin.zoomph.com` +0.0294 (to 0.5882, still far under the bar), `crazyshop.pl`
+0.0007, fourteen unchanged, zero regressions, coverage byte-identical. A completed spec rule, not a
corpus lever — the same shape as t827, and labelled as such.

Gate: `a_max_height_on_a_replaced_element_pulls_its_width_back_through_the_ratio` (`manuk-layout`),
RED-proven twice — once by removing the transfer, once by removing ONLY the auto-margin re-run while
keeping the width fix, so the two halves are separately falsifiable.

## An absolutely positioned image was ZERO PIXELS TALL, always (t834)

`layout_abs` took its height from `definite_ch` (an explicit `height`, or both `top` and `bottom`)
or else from the CONTENT height. **A replaced element has no children.** So an absolutely
positioned `<img>` with neither measured `<w>x0` and painted nothing.

This is the third implementation of the rule t831 landed in `layout_float` and t833 completed in
`layout_block`. It was found by taking t833's own conclusion literally — *the grep is symmetric or
it is not a grep* — and enumerating every remaining size resolution rather than waiting for a site
to name the next one. It was the worst of the three: the other two produced a wrong size, this
produced no box.

```text
                                 Chrome    before     after
  max-width:100%                 320x85    320x0     320x85    ✗→✓
  max-height:30px                113x30   1000x0     113x30    ✗→✓
  max-width:100% + max-height    113x30    320x0     113x30    ✗→✓
  min-width:1500px              1500x399  1500x0    1500x399   ✗→✓
```

Every `before` height is 0 and every `before` width but one is already right: the min/max clamps
reached this path in an earlier tick and the ratio never did.

⚠⚠⚠ **THE `inset:0` VARIANT HAPPENED TO WORK, WHICH IS WHY THIS SURVIVED.** Both insets make
`definite_ch`, so the most-cited spelling of the idiom was fine while `position:absolute; top:0;
left:0` — the same pattern, written the other common way — was invisible. **A defect whose canonical
form works is not discoverable from the canonical form.**

### The falsification pass deleted a third of the fix

All three rules were written onto this path for symmetry: the auto-height derivation, §10.4
inline→block, and §10.4 block→inline. Mutated out one at a time, the **inline→block half left the
gate GREEN** — the auto-height arm already derives from `content_w` *after* the width clamp, so that
transfer could only recompute the number it had just computed. `layout_block` and `layout_float`
genuinely need their copies (both resolve the height from a source that is not the clamped width);
this path does not.

It was removed, with the reason left where the code would have gone, because the next person to run
the symmetric grep will otherwise notice the asymmetry and re-add it. **A fourth copy of a rule
added for symmetry is unreachable code guarded by a test that cannot fail** — this project's own
definition of a vacuous gate, nearly committed while quoting the lesson about them.

⚠⚠ **HONEST SCOPE: zero movement on the 16-site control**, priced against the t833 binary in the
same hour, all sites byte-identical. None of them absolutely-positions an image without both insets
— which is exactly the limitation t832 named one tick earlier (*a per-fix control is evidence about
the sites it contains and nothing else*). The witness is the Chrome table above and the next full
sweep, not a control padded until something moves.

Gate: `an_abspos_replaced_element_takes_its_height_from_its_ratio` (`manuk-layout`), RED-proven by
mutating out the derivation.

## A flex item `<img>` told taffy its content wanted ZERO (t835)

CSS Flexbox §4.5: a flex item's `min-width:auto` — the default — is its **automatic minimum size**,
which for a replaced element is its intrinsic width. Chrome therefore refuses to shrink a row of
logos below their own size and lets the container overflow, which is the entire point of a
`display:flex; overflow-x:scroll` carousel. We shrank them to fit.

Aimed by `fidelity --shape-dump` on `promo.golesliga1max.pe` (shape 0.5873, coverage 1.000, n=63):
**15 of its 26 misses were one row of team badges**, each `74x82` in Chrome and `18x82` for us.

The cause was one omission doing two jobs. `replaced_default_size` — the seam that tells taffy how
big a replaced item is — listed `svg|canvas|video|object|embed` and **not `<img>`**. That list was
written for the DEFAULT OBJECT SIZE (300×150), which `<img>` correctly does not have; excluding it
there silently also excluded it from reporting its **intrinsic** size.

```text
  four 1000×266 images in a 320px flex row   Chrome        before      after
                                             1000x266 ea   68x266 ea   1000x266 ea   ✗→✓
```

### The first version of the fix shipped a regression, and only the control caught it

Admitting `<img>` to the seam also handed it the `300×150` fallback. The guard written for that —
return `None` when *neither* axis nor a ratio is known — was **the wrong guard**: the case that broke
is an image with a definite **width** and no ratio, which sailed past it and took the `150.0` height.
`777juegos.com`'s footer is a row of exactly those (unloaded payment icons, which Chrome measures at
height **0**) and it cost **-8.75 shape points**.

The fix is `_ if is_img => return None` on **both** arms: an `<img>` with an underivable axis says
nothing rather than inventing a number, and falls back to the broken-image path (t689).

Priced against the t834 binary, 16 sites, same hour: `promo.golesliga1max.pe` **0.5873 → 0.8254 ★
crossed 0.75**, the adversarial control `ta3lemkonline` +0.0284, `777juegos` -0.0058 (inside its
measured ±1.2pt drift, coverage 0.941→0.965), 13 unchanged, mean +0.0163, zero engine regressions.

### Residue, bounded and with its refutations attached

A `min-width:0` flex item image in a 320px row is `160x43` in Chrome and `160x266` for us — the width
already correct, so only the picture's proportions give it away. **Not** the measure seam (a
known-width→ratio derivation written into the closure moved no number and was removed under t834's
rule); **not** cross-axis `stretch` (`align-items:flex-start` measures identically). What remains:
taffy applies `aspect_ratio` to the item's **specified** width rather than its **flexed** width, so
the cross size comes from 1000 (→266) instead of 160 (→43). The fix is on the way OUT of taffy, in
the slot adoption, and it is a tick of its own.

Gate: `a_replaced_flex_item_is_floored_at_its_intrinsic_width` (`manuk-layout`), both halves
RED-proven separately — mutating `is_img` off in the seam fails the intrinsic-floor assertion;
restoring the `150.0` leak fails the default-object assertion.

## The out-of-flow pass's `viewport` held the DOCUMENT height (t837)

CSS 2.1 §10.1: the initial containing block has the dimensions of the **viewport**, and a
`position:fixed` box's containing block IS the viewport. The out-of-flow pass built its containing
block as `Rect { width: viewport_w, height: root.content_bottom() }` — **the variable was named
`viewport` and held the whole scrolled document** — so every percentage height on an out-of-flow box
resolved against the page instead of the window.

That is every full-height drawer, modal backdrop, off-canvas menu and overlay on any page long
enough to scroll, i.e. exactly the pages where it shows. Chrome-measured on a 3000px page in an
800px window (`innerHeight` 713):

```text
                                       Chrome   before    after
  position:fixed;    height:100%       300x713  300x3000  300x713   ✗→✓
  position:fixed;    height:50%        100x357  100x1500  100x357   ✗→✓
  position:absolute; height:100%       100x713  100x3000  100x713   ✗→✓
  position:fixed;    height:auto        100x50   100x50    100x50    ✓  ← control
```

⚠⚠ **The IN-FLOW initial containing block already had this right** — `layout_document` reads
`icb_height` from `manuk_css::values::viewport_size()` sixty lines above, with a comment explaining
why a root `height:100%` must fill the window. Only the out-of-flow pass still used the document
height. One rule, two implementations, and only one had ever been corrected.

⚠⚠⚠ **THE NAME IS WHY IT SURVIVED.** Every reader downstream took `viewport` as the specification.
The project's rule — *a wrong fix is caught by the next gate; a wrong label is caught by nothing* —
was recorded about wiki prose (t817) and about a string in a data column (t824). This is the same
failure in an **identifier**, which is the one place a wrong label also compiles.

### What it was priced at, honestly

Priced against the t835 binary over 19 sites in the same hour: `app.ordertime.com`
**0.8621 → 1.0000** (a perfect score), `golesliga` +0.0159, `ta3lemkonline` +0.0044, `neutypechic`
+0.0017, mean +0.0065, **zero crossings, zero regressions**, coverage byte-identical.

⚠ **The site that AIMED the tick moved 0.0035.** `possssno.sbs` has 172 misses; `#aside` at
`300x4462` against Chrome's `300x713` was the single **largest** one, and fixing it moved almost
nothing, because a score counts elements within tolerance and one subtree is a handful of them.
**Rank by frequency, aim by magnitude, and do not confuse them** — `--shape-dump` is worst-first
because that is how a mechanism is found, but the metric pays per element.

Two rows in the single-draw control read as regressions (`ta3lemkonline` -0.0241, `777juegos`
-0.0122) and **neither was one**: repeated twice per binary, `ta3lemkonline` is deterministic
0.5448 → 0.5492 (an improvement, the old 0.5733 being the outlier) and `777juegos` is 0.7439 on
both. **A single row is a draw; the repeat is the measurement.**

Gate: `an_out_of_flow_percentage_height_resolves_against_the_viewport_not_the_document`
(`manuk-layout`), RED-proven by restoring `root.content_bottom()`. It asserts against the viewport
the engine was actually given rather than a literal, because the defect is *which reference* is used
and a hard-coded number would make the test a statement about the harness's window size.

## A `position:relative` ancestor inside an out-of-flow subtree is still a containing block

`position_absolutes` builds its rect map from the **in-flow** fragment tree. `abs_containing_block`
tests `position != Static` and *then* requires a rect for that ancestor — and when it cannot find
one it **walks straight past**. So nothing inside an out-of-flow subtree could ever be a containing
block: a `position:relative` row inside a `position:absolute` drawer was invisible, and every abspos
box under it escaped to the outer positioned ancestor.

AdminLTE 2.4.5's sidebar is the canonical instance —
`.main-sidebar{position:absolute}` > `section` > `ul` > `li` > `a{position:relative}` >
`span.pull-right-container{position:absolute;right:10px;top:50%;margin-top:-7px}` — and it is the
shape of every off-canvas menu, drawer, dropdown panel and fixed toolbar whose rows carry their own
badges, carets or absolutely-placed icons.

Chrome-measured on `ubys.bingol.edu.tr` (14 sidebar rows), and reduced to 12 lines of CSS:

```text
                                                    Chrome        before      after
  abspos ancestor > section>ul>li > relative <a>     17 / 77       353 / 353   17 / 77   ✗→✓
  …the same, min-height:100% on the drawer           17            353         17        ✗→✓
  real AdminLTE stylesheet, 3 sidebar rows           65/109/153    353 ×3      65/109/153 ✗→✓
  a relative <a> with NO positioned ancestor above   15            15          15        ✓ control
```

`353` is `viewport/2 − 7`: `top:50%` resolving against the sidebar, whose `min-height:100%` makes it
viewport-tall.

⚠ **Only one axis was visibly wrong.** `right:10px` is a length, and the drawer and the row share a
right edge, so `x` came out **correct from the wrong containing block** — 210 in both engines, on
every row. The defect therefore presented as a bug in `top:50%`, a percentage bug, in the exact
family t837 had just worked. **A wrong containing block is only as visible as the insets that
distinguish it.**

⚠ **And the diagnosis was in the VARIANCE, not the magnitude.** Fourteen elements at fourteen
different `y` in Chrome and *one* `y` in ours: no per-element arithmetic produces that. A constant is
not a layout error, it is a containing-block error.

### The one-line version is wrong, and two existing gates say so

`rects.extend(b.node_rects(dom))` is the obvious fix and it breaks two tests. `node_rects` **lifts** a
boxless element's geometry up the DOM until it reaches an ancestor that has a box *in the tree it was
called on* — right for the whole-document call, inverted from inside an out-of-flow subtree, where
**every ancestor is boxless**. `#modal`'s rect propagated onto its own `position:relative` containing
block, so the next abspos sibling resolved against `[100 100 200x200]` instead of `[0 0 400x400]`
(`abspos_auto_margins_center_a_constrained_box`), and a `position:static` inline acquired geometry it
must never have (`an_out_of_flow_child_neither_splits_its_inline_nor_escapes_it`).

The lift cannot simply be dropped — a `position:relative` **inline** inside a drawer has no box of
its own and is a legal containing block (CSS 2.1 §10.1). So the union is kept and everything it
pushed *above* the box is filtered out.

**A helper that walks UP the DOM has an implicit precondition about which tree it is walking, and
that precondition is not in its signature.**

## CSS 2.1 §10.3.3: the over-constrained equation ignores `margin-left` under `rtl`

With a definite `width` and neither margin `auto`, the block-width equation cannot hold, and the
spec says which term gives:

> *"If the `direction` property of the containing block has the value `ltr`, the specified value of
> `margin-right` is ignored and the value is calculated so as to make the equation true. If the value
> of `direction` is `rtl`, `margin-left` is ignored."*

So a narrower-than-container block is flush LEFT in an LTR page and flush RIGHT in an RTL one — every
sidebar, card, fixed-width panel and `width`-without-`margin:auto` wrapper on the Arabic / Hebrew /
Persian / Urdu web. Chrome-measured, `<html dir=rtl>`, 400px blocks in a 1200px viewport:

```text
                                         Chrome   before   after
  plain 400px block                        800       0      800    ✗→✓
  dir=ltr ON THE BLOCK ITSELF              800       0      800    ✗→✓
  margin-right:auto                          0       0        0     ✓ control
  margin-left:auto                         800     800      800     ✓ control
  margin-left:auto + margin-right:auto     400     400      400     ✓ control
  inside a dir=ltr WRAPPER                   0       0        0     ✓ control
```

⚠ **Row 2 is what makes this a containing-block rule rather than "RTL elements go right".**
`direction` is inherited, so reading the element's own style agrees with the spec everywhere *except*
there: a `dir=ltr` block inside an RTL page is still **placed** by its RTL parent and stays flush
right, while its own contents lay out LTR. Row 6 is the same point inverted — an LTR wrapper puts its
child back on the left even in an RTL document.

⚠ **Non-replaced only, and the corpus taught that the hard way.** §10.3.3 is written for a
block-level *non-replaced* box. The first draft omitted the guard and moved every `<svg>` on
`www.ta3lemkonline.com` — an atomic inline whose position belongs to its line box, not to this
equation.

### The diagnostic that settled it: `delta × n`

`www.ta3lemkonline.com` is **bimodal** — identical coverage, element count and `reading_order` on
every run, with shape landing on one of two values. The first draft shifted **both** modes by the
same amount:

```text
  0.601751 − 0.595186 = 0.006565      0.573304 − 0.566740 = 0.006564
  0.006565 × 457 elements = 3.0
```

Exactly three elements, deterministically — on a site whose own spread (0.028) is four times the
delta. **A per-site delta smaller than the site's spread can still be exactly attributable**: the
spread bounds what a single reading proves, not what the arithmetic does. Then `comm` over two
`--shape-dump` runs said **zero fixed, sixteen broken, every one an `svg` or `svg/path`** — and *zero
fixed / N broken is a class error, not a tuning error*. No threshold would have found it, because the
number was never about magnitude.

## The static position of an insetless `position:absolute` box includes the INLINE ADVANCE (t848)

CSS 2.1 §10.3.7 / §10.6.4: when `left`/`right` (or `top`/`bottom`) are both `auto`, the box sits at
its **static position** — where its hypothetical box would have started. In an inline formatting
context that is *after* everything already on the line, not at the line's start edge.

The engine recorded the container's content-box origin as it stepped over the out-of-flow child, and
the comment beside it named the gap rather than closing it: *"Text preceding it on the line should
push the static position along that line; that refinement is not modelled here, and the box lands at
the line start instead."*

Chrome-measured, `body{margin:0;font:16px Arial}`, a 400px `position:relative` wrapper,
`a{display:block}`, x of the absolutely positioned span:

```text
                                                   Chrome   before   after
  <span>Hello</span><span class=sr-only>             35       -1      35    ✗→✓
  <span>Hello</span><span position:absolute>         36        0      36    ✗→✓
  <span position:absolute>FIRST</span><span>Hello     0        0       0     ✓ control
  …a WRAPPED first span, then an abspos span         61        0      61    ✗→✓
  the in-flow spans themselves                        0        0       0     ✓ control
  dir=rtl wrapper                                   334        0       0     ✓ INERT by the guard
```

Row 1 carries `margin:-1px`, so 35 is 36 less the margin: **the margin is applied after the static
position and the two must not be conflated.** Row 4 is why the search is `(line_top, then x)` and not
`max(x)` — a fragment that wrapped onto a later line is genuinely later even though its right edge is
further left.

**Attribution is by SUBTREE.** `refine_inline_static_positions` collects every node under the in-flow
siblings that *precede* the out-of-flow child (a prefix, never the whole set — content after it must
not push it along), then takes the furthest-along fragment or atomic inline belonging to that set.

### What it deliberately does not cover, with numbers

* **Bare text directly in the block.** `<a>Bare text<span position:absolute>` belongs at **x=64** and
  stays at 0. A `TextFragment`'s `node` is the deepest *element* ancestor, so such a fragment reports
  the block itself and there is no way to tell WHICH bare-text sibling produced it. Guessing would
  move boxes on pages this rule should not touch.
* **`left:200px; top:auto`** belongs at **y=294** and lands at **234**. `position_absolutes` anchors
  to the static position only when **all four** insets are `auto`; §10.3.7 is written PER AXIS. A
  separate defect in the same section — bigger, because it moves full-size boxes rather than a 1×1
  `.sr-only`.
* **RTL**, excluded at the call site: under an RTL base direction the inline start is the right edge,
  and the fragments have already been through UAX #9 rule L2, so "the trailing edge of the last
  preceding fragment" is the wrong end of the wrong box.

### The lesson worth carrying

**A build spec whose second half is unbuilt is an untriaged tick with good prose.** This one sat in
the source, correctly described, invisible to every instrument — because the code was *honest* about
it and honest text does not fail a gate. `"not modelled here"` is worth grepping for as a defect
class, not read as documentation.

## The static position is resolved PER AXIS (t849)

CSS 2.1 §10.3.7 solves the horizontal equation for an absolutely positioned box and §10.6.4 solves
the vertical one — **separately**. `left` and `right` both `auto` makes the box's *inline* position
its static position; independently, `top` and `bottom` both `auto` makes its *block* position static.

`position_absolutes` tested all four insets at once:

```rust
let all_auto = s.inset.left.is_auto() && s.inset.right.is_auto()
            && s.inset.top.is_auto()  && s.inset.bottom.is_auto();
```

so naming ONE inset threw the static position away on **both** axes, and the box fell back to the
containing block's origin on the axis that was still `auto`. That is every
`position:absolute; right:8px` badge and close button, every `left:0` full-bleed underline, every
`top:100%` dropdown.

Chrome-measured, `body{margin:0;font:16px Arial}`, 400px `position:relative` wrappers with 60px of
spacer above their line, `a{display:block}`, the abspos span following a 36px `<span>Hello</span>`
(`y` relative to the wrapper's top):

```text
                                          Chrome        before          after
  left:200px  (top auto)               [200, +60]    [200,   0]      [200, +60]   ✗→✓
  top:0       (left auto)              [ 36,   0]    [  0,   0]      [ 36,   0]   ✗→✓
  right:10px  (top auto)               [309, +60]    [309,   0]      [309, +60]   ✗→✓
  all four auto                        [ 36, +60]    [ 36, +60]      [ 36, +60]    ✓ control
  top:0; left:0                        [  0,   0]    [  0,   0]      [  0,   0]    ✓ control
```

**Rows 2 and 3 are what make this per-axis rather than "use the static position more often."** Row 2
takes `x` from flow and `y` from the containing block; row 3 does exactly the opposite. A single
boolean, however it is tuned, cannot produce both — so a fixture carrying only one direction would
have admitted a wrong fix.

An axis with a real inset keeps the containing block, because that inset must resolve against the
containing block's edge and not against the flow cursor.

### The drop-guard had to narrow at the same time

The `continue` that discards a box flow never recorded a cursor for is now conditioned on **both**
axes wanting the static position. A box with a real inset on one axis is placeable, and dropping it
because no cursor exists would turn a placement bug into a **missing box** — strictly worse, and the
exact failure the guard was written to avoid.

### What it bought, measured

+2 attributable elements across 28 sites (12-site M1 cohort byte-identical; 10 of the 16 largest
scored sites byte-identical). Banked on the spec and the fixture, not on a corpus delta: a scored
divergence needs the abspos box to be both mis-placed **and** large enough to fail the tolerance.

The one clean-looking negative — `www.taphouse23.com`, `delta × n` of exactly **−18.00** elements at
identical coverage and identical `shape_n` — was refuted by two runs of the OLD binary alone
(`0.408292 / 0.407590 / 0.395643`, `overlap` wandering 10–13), whose range contains every new
reading. Second consecutive tick where the integer test alone would have condemned a correct fix.

## A button centres its content vertically, and no stylesheet can say so (t850)

The UA sheet already gives buttons `text-align: center`, which is why the **horizontal** half has
always matched Chrome. The **vertical** half is not expressible in CSS at all: Blink lays a button's
children out inside an anonymous flex-like box with `align-items: center`, and the HTML rendering
spec describes the same thing. A button taller than its content centres that content in its **content
box**, after padding, **as a single group**.

Every design system fixes a button height, so before this the label sat 5–20px too high on
essentially every button on the web — and, being a label inside a fixed-size box, it is the kind of
divergence the fidelity instrument reports as `overlap` rather than as `shape`.

Chrome-measured, `button{display:block;width:300px;padding:0;border:0;font:16px Arial}`, y of the
label relative to the button's border box:

```text
                                                   Chrome   before   after
  height:50px, one 18px line                          16       0      16    ✗→✓
  height:80px, TWO block spans (36px together)         22       0      22    ✗→✓
  height:20px, an 18px line (nearly full)               1       0       1    ✗→✓
  height:auto                                           0       0       0     ✓ control
  a plain <div> at height:50px                          0       0       0     ✓ control
  display:inline-block button, height:50px             16       0      16    ✗→✓
```

**The content moves, not the box** — the border box is already `height`, and shifting it would turn a
centring bug into a placement bug one level up. **And the whole content moves as one group**: row 2's
two block children keep their own 18px separation and travel 22 together. That is what makes this
*centring* rather than per-line alignment, and a fixture with a single child cannot tell the two
apart.

The gate derives its expectations from the **auto-height button's own height** rather than from `18`,
so the UA font's metrics cannot make it lie: the rule is `(box − content) / 2`, and the auto button
*is* the content.

### The `box-sizing` residue this measured on the way past

Chrome's UA sheet computes `border-box` for `button`, `input[type=submit|reset|button]` and `select`,
and `content-box` for `input[type=text]`, `textarea` and every ordinary element. At
`height:50px; padding-top:20px`:

```text
              button  submit  text  select  textarea  div
  Chrome        50      50     70     50       70      70
  ours          70      70     70     70       70      70
```

Three controls are 20px too tall whenever they carry padding **and** a height. A padded button's
centring cannot be right until its content box is, which is why the padded row is absent from the
table above.

`input[type=submit]` takes the same centring path and its vertical offset already matches (16 of 50);
its *horizontal* centring does not — the synthetic-text path draws the label at x=0 where Chrome
centres it.

### What it bought, and the finding that outranks it

Zero corpus movement: 11 of 14 M1-cohort sites byte-identical old vs new, and
`littlecaesarsbcs.libellum.com.mx` — the site whose `overlap` hit motivated the whole reduction —
reads its two spans **identically on both binaries**, so the centring did not fire on that button and
why its content box has no slack is not established.

**Three consecutive ticks (t848, t849, t850) landed spec-correct, Chrome-exact, RED-proven primitives
and moved the M1 cohort by nothing.** The cohort's remaining jarring hits are not being explained by
the mechanism families these reductions keep landing on. The next render tick should diagnose ONE
cohort site end to end with `--why` until its specific failing pair is understood, rather than reduce
to a family and hope the family is the cause.

## `colspan`/`rowspan` are CLAMPED unsigned longs, and an unclamped one is a HANG (t854)

`<td colspan="2147483648">` parses cleanly as a `usize` on a 64-bit target. The table builder is then
asked for **two billion columns** and the page never finishes. Chrome-measured
(`--headless=new --dump-dom`):

```text
  <td colspan="2147483648">   colSpan 1000     (2-column table: the cell is 2 cells wide)
  <td colspan="1000">         colSpan 1000
  <td rowspan="2147483648">   rowSpan 65534    <- a DIFFERENT bound; one shared constant is wrong
  <td colspan="3px">          colSpan 3        <- residual: HTML integer parsing stops at the first
                                                  non-digit; `parse::<usize>()` rejects it and we
                                                  read 1. A wrong answer, not a hang. Not fixed here.
```

⚠⚠⚠ **ONE RULE, TWO IMPLEMENTATIONS, AND ONLY ONE OF THEM HAD IT.** `engine/js/src/reflect_js.rs`
implements `clamped unsigned long` correctly and its own comment says so — *"a colspan of a billion
is 1000, not the default"* — so `td.colSpan` answered **1000** while `LayoutBox::cell_span`, which
actually builds the table, read **2,147,483,648**. The IDL was right, the geometry hung, and nothing
compared the two. `G_SPAN_CLAMP` now asserts them together.

⚠⚠⚠ **A HANG IS NOT A RED, WHICH IS HOW IT SURVIVED.** `g_reflect_numeric` has carried this exact
attribute value since it was written and **did not fail — it spun**: `user 2m57s` of a 3m00s cap on a
four-element fixture, which reads as *a slow gate*. The wall runs 19 of 104 gates, so nothing else was
looking. It surfaced only because t853 ran the whole `manuk-page` suite during an unrelated
regression sweep, and it became a *defect* rather than a symptom of that tick only because the **old
binary reproduced it identically** (3m00.2s, `user 2m56.7s`) from a stashed tree in the same hour.
After the fix that gate runs in **0.40s**.

⚠⚠ **SO THE GATE FOR A HANG MUST NOT HANG.** A Bar-0 gate that stalls the wall instead of failing it
recreates the exact condition that hid the bug. `G_SPAN_CLAMP` runs the load on its own thread behind
a 20s `recv_timeout`, so the unclamped engine produces `test result: FAILED ... finished in 20.00s`
with a message, which is a red the wall can read. **A gate whose failure mode is silence is not a
gate.**

⚠ The widths are asserted against a **control cell in the same document**, not Chrome's absolute 46:
our `border-collapse` cell is 24px where Chrome's is 23, a 1px-per-cell residual that predates this
and belongs to the collapsing-border model. `2 × unit` vs `1 × unit` asks *"did the span apply and get
bounded?"* in either engine; pinning 46 would fail for a reason the gate does not test, and pinning
our own 48 would freeze the residual as if it were correct.

## An empty inline reports its CONTENT AREA only when it shares a line — alone it has no line box to report against (tick 868)

The code carried this justification for reporting an empty inline's rect as `line-height` tall,
anchored to the line's top:

> An EMPTY inline keeps the old line-top anchoring: Chrome reports a line-height-tall rect for
> `<span id="anchor"></span>`, and that is measured behaviour this must not disturb.

Re-measured across five contexts with `chromium --headless --dump-dom` (`16px/1.5 sans-serif`):

```text
                                          Chrome        before
  <div><span></span></div>               [0, 0,0, 0]   [0, 0,0, 0]   agree
  <div><span></span><span></span></div>  [0,48,0, 0]   [0,48,0, 0]   agree
  <div><span></span>text</div>           [0, 3,0,17]   [0, 0,0,24]   <-
  <div>text<span></span></div>          [26,27,0,17]  [26,24,0,24]   <-
  <div style="line-height:3">…</div>     [0,63,0,17]   [0,48,0,48]   <- the error SCALES
```

The comment was **half-measured, and its half is the one that does not matter.** Both `0x0` rows are
the case it was looking at, and they are right for a reason with nothing to do with the reported
height: an empty inline **alone** brings no line box into existence (CSS2 §9.4.2, `holds_line:
false`), so there is no line for a height to be reported *against* — the fields are never consulted.
The moment it shares a line with content, Chrome reports the element's **own content area on the
line's baseline**, which is the identical rule the no-fragment branch forty lines below already
implements. Two branches, one rule.

**The error scales with `line-height`** — 48 against 17 at `line-height:3` — so it is worst exactly
where authors are most generous with leading.

### Why this construct is worth a gate

`<a><i class="icon"></i><span>Label</span></a>` is on every navigation bar on the web. An empty `<i>`
reported 3px too high and 7px too tall, sitting beside its label, is a candidate for flipping a
reading-order comparison.

**And it was NOT the cause of the reading-order inversions it was reduced from.** Three sites in the
t867 sweep are over the M1 shape bar and fail only on `reading-order 1`; all three still read `1`
after this fix. The honest sentence is *"the instrument cannot price this"*, not *"this bought
nothing"* — the fix is Chrome-exact on a ubiquitous construct — but the next shape tick must find the
real mechanism rather than inherit the guess. See [[conformance-and-oracles]] on why a Chrome-exact,
RED-proven, high-usage fix can move the corpus metric by zero.

### Both halves must be asserted

Report the content area, **and** keep the line boxes byte-identical. A fix that grew a `holds_line`
would move the containing block's height (0/24/48 in the fixture) and turn a geometry correction into
a layout regression; a fix that reported the content area unconditionally would give the alone-case a
phantom 17px box that never existed.

[[text-layout]] [[conformance-and-oracles]]

## An intrinsic width is what the content IS — `text-align` cannot change it, and the space before an atomic inline is a CHARACTER (t871)

Two defects in one mechanism family — *"what width does an atomic inline contribute to its line?"* —
found from the single `reading-order` inversion that kept `possssno.sbs` off the M1 bar. The
construct is `<a class="float"><i class="icon">…</i> <span>Label</span></a>`: **icon plus label**,
which is how every nav bar, chip, button and tab strip on the web is written.

### 1. The space before an atomic was a constant

`layout_inline`'s `Atomic` and `Spacer` arms measured the preceding collapsed white space as

```rust
let key = FontKey { family: FontFamily::SansSerif, .. };   // "the default text space width"
let space_w = if space_before { self.fonts.measure(" ", key, 16.0) } else { 0.0 };
```

— **~5px on every page, in every font, at every font size.** The `Word` arm three match-arms away
had always measured its own space correctly, which is exactly why this survived so long: the same
document rendered the same space right and wrong depending only on what followed it.

The space is not decoration, it is a **character of the inline formatting context**, and it belongs
to the element that contains it. `space_before` is now the white space's own `TextStyle` (`None`
where there is none), so the width is `measure(" ", its font, its size) + word-spacing +
letter-spacing` — the identical expression the word arm uses. Chrome-measured, `float:left` with
`padding:10px 15px`:

```text
                                                     Chrome    before    after
  <i>LABEL</i> <span style=inline-block>MM</span>    107x39    102x39    107x39   16px mono
  …the same at 32px                                  154       139       154      <- does NOT scale
  …the same, plain inline <span>                     107       107       107      ✓ word path
  <i>LABEL</i> <img width=20>                         78        73        78
```

**The error not scaling with the font is the tell.** At 32px monospace the space owes 19px and we
paid 5.

### 2. `text-align` was changing an intrinsic measurement

An intrinsic width is read by laying the subtree out at an absurd available width (1e6 for
max-content, 1.0 for min-content) and measuring how far the content reaches. `text-align:center`
distributes the **leftover** space — and at 1e6 the leftover *is* the measurement, so every fragment
lands at x≈500,000.

`content_right_extent` already knew this and handled it **for text**: each line is spanned from its
own leftmost fragment to its rightmost, so the centring offset is discarded as slack while the span
survives. But an **atomic inline leaves the line as its own `LayoutBox`** (`close_line` pushes it to
`atomic_boxes`, not to `frags`), so it was spanned *alone* — contributing its own width and not its
place on the line.

So a centred float came out sized to its **widest single item**, its last token wrapped, and a
one-line control became two:

```text
  .navicon a { float:left; text-align:center; padding:10px 15px }
  <a><i>فیلم بکن بکن</i> <span>منو</span></a>       Chrome 152x38     ours 123x56
```

123 = the `<i>`'s 93 plus 30 of padding: the `<span>` contributed **nothing**. And a two-line box
puts the label above the icon instead of beside it — which is the `reading-order` inversion the M1
certificate was failing on, arriving as a *width* bug three steps upstream. (This is the burndown's
own §3 thesis — *container-WIDTH errors LAUNDER into dy* — with a named mechanism.)

CSS Sizing §5.1 settles it: a max-content size is what the box wants given unlimited space, and
alignment cannot change what the content *is*. `Ctx::intrinsic_probe` is set for the duration of
both probes (through a `Drop` guard, because the probes nest), and `layout_inline` lays out at the
start edge while it is. The slack machinery stays as the belt to this braces.

### What it bought, OLD binary vs NEW in the same hour, identical denominators

```text
                       shape OLD   shape NEW   jarring OLD → NEW      n
  possssno.sbs            0.897       0.991     reading-order 1 → clean   575   M1 CROSSING
  www.marktplaats.nl      0.952       0.967     reading-order 1 → clean   810   M1 CROSSING
  en.wikipedia.org        0.4792      0.4845    overlap 97→95, ro 80→79  2673
  www.apple.com           0.4137      0.4260    unchanged                 730
  www.a11yproject.com     0.3578      0.3761    unchanged                 218
  blog.rust-lang.org      0.99639     0.99639   unchanged                1664   control, byte-identical
  littlecaesarsbcs…       0.94872     0.94872   unchanged                  78   control, byte-identical
```

`possssno.sbs` went from **503 misplaced elements to 4**. Every measured site rose or was
byte-identical; two controls did not move at all.

Gates: `the_space_before_an_atomic_inline_is_measured_in_the_font_that_owns_it` and
`text_align_does_not_change_a_floats_intrinsic_width`, both **self-comparisons** (the same content
with/without `inline-block`; the same box with/without `text-align:center`) so no font metric is
hard-coded, and each RED-proven by restoring only its own half.

## A box laid out at a PROVISIONAL origin left its out-of-flow descendants behind (t872)

A `float` and an `inline-block` are **sized before they are placed** — their size is what decides
where they go — so both lay their content out at `(0,0)` and translate it into position once the
box lands. `layout_float` says so in its own comment: *"content was laid out at (0,0); shift it to
the float's content origin"*, and it shifts the child boxes and the text fragments.

`static_pos` — where an insetless `position:absolute` box **would have been** in flow — is a
**third output of that same inner layout**, and it was not shifted. So every out-of-flow descendant
of a float or an inline-block was placed against the provisional origin. Chrome-measured, one
`position:absolute` 1×1 span after `MENU` inside a container with `padding:15px`:

```text
                                  Chrome      before      after
  inside float:left              [56, 15]    [41,  0]    [56, 15]
  inside display:block           [56, 62]    [56, 62]    [56, 62]   ✓ never moved, so never wrong
  inside display:inline-block    [56,109]    [56, 15]    [56,109]
  inside display:flex            [15,155]    [15,155]    [15,155]   ✓
```

**The two that were wrong are exactly the two that lay out at a provisional origin.** That is the
whole rule, and it is why `display:block` and `display:flex` — which lay out in place — were already
Chrome-exact and hid the defect for as long as they did.

### The guard that looked right and was not

The obvious test for *"did the inner layout record a static position?"* is whether
`static_pos.len()` grew. It is **wrong, and silently**: a float calls `shrink_to_fit` *before* it
lays its content out, that probe lays the same subtree out and records the same key, and the real
pass then **overwrites** it. Same length, a write that happened, and the float's `.sr-only` stayed
put. The fix-up landed for `inline-block` and not for `float` on the first attempt for exactly this
reason. `Ctx::static_pos_writes` is a monotone write counter; a counter cannot be fooled by an
overwrite.

`close_line` — which places an atomic inline on its line — is a free function with no `&self`, so
the atomic's provisional origin is banked in `Ctx::atomic_static_origin` and the shift is applied in
`layout_inline` where the final box comes back. The map is empty for every box with no out-of-flow
descendant, which is nearly all of them.

### Reach, and what it bought

`.sr-only` (Bootstrap's, and every copy of it) is `position:absolute` with no insets, and it lives
inside `.sidebar-toggle{float:left;padding:15px}` on every AdminLTE/Bootstrap admin header — plus
every React portal root, dropdown and tooltip anchored inside a floated or inline-block card.

```text
                       shape before   after    jarring
  ubys.bingol.edu.tr       0.9518     0.9578   reading-order 1 → CLEAN   ← M1 CROSSING
  littlecaesarsbcs…        0.9487     0.9615   clean → clean
  www.library.chiyoda…     0.8341     0.8596   overlap 1 (unchanged)
  possssno.sbs / marktplaats / blog.rust-lang / en.wikipedia / a11yproject — byte-identical
```

Gated by `an_out_of_flow_childs_static_position_survives_its_containers_translate`, a
**self-comparison against `display:block`** (the offset of the absolute box from its container must
not depend on how the container was placed), with each half RED-proven on its own.

## CSS 2.1 §9.5's other half — a BFC root is placed BESIDE a float, not under it (t873)

*"The border box of a table, a block-level replaced element, or an element in the normal flow that
establishes a new block formatting context must not overlap the margin box of any floats in the same
block formatting context as the element itself."*

A **plain** block does overlap a float — only its line boxes shorten around it — and that half was
built and correct. The other half was not built at all, so `float:left` image + `overflow:hidden`
text block put the text **under** the float instead of beside it. That is the **media object**, and
it is the whole pre-flexbox two-column web: sidebar-plus-content, avatar-plus-comment,
icon-plus-description, `<p class="header-fontsize__title">` beside `<ul class="header-fontsize__list">`.

Chrome-measured, a 100px `float:left` in a 400px container:

```text
                                     Chrome         before        after
  plain block                      [  0, 400]     [  0, 400]    [  0, 400]   ✓ correct to overlap
  overflow:hidden                  [100, 300]     [  0, 400]    [100, 300]
  display:flow-root                [100, 300]     [  0, 400]    [100, 300]
  overflow:auto                    [100, 300]     [  0, 400]    [100, 300]
  display:table                    [100,  48]     [  0,  48]    [100,  48]
  …right float instead             [  0, 300]     [  0, 400]    [  0, 300]
  …both sides                      [100, 200]     [  0, 400]    [100, 200]
  …float 10px tall, box 60px       [100, 300]     [  0, 400]    [100, 300]
  …margin-left:20px                [100, 300]     [ 20, 380]    [100, 300]
  …margin-right:20px               [100, 280]     [  0, 380]    [100, 280]
  …margin-left:200px               [200, 200]     [200, 200]    [200, 200]   already clears
```

Two details the fixture pinned rather than assumed:

* **The band is read at the box's TOP edge only.** A float 10px tall next to a 60px box does not
  widen the box lower down — the box stays a rectangle in the band its top sits in.
* **`margin-left` is ABSORBED, not added.** With a 20px left margin the box lands at 100 and is 300
  wide, not 120/280. So `bfc_float_band` returns a *containing block* `(left, width)` chosen so that
  `left + margin-left` lands on the band edge, rather than narrowing the containing block directly.

### What is deliberately not built

A **specified** width keeps today's behaviour. Chrome shifts such a box beside the float only while
it still fits — `width:300px` shifts to 100, `width:301px` stays at 0 and overlaps, which is §9.5's
*"if necessary, implementations should clear the said element"*. We never shift, which is Chrome-exact
for the does-not-fit half and wrong for the fits half. Narrowing the containing block for it would
also change every percentage the child resolves, so it is measured, named, and left for its own tick.

### Measured, OLD binary vs NEW in the same hour

```text
  www.library.chiyoda.tokyo.jp   0.8596 → 0.8624   overlap 1 → 0 CLEAN   ← M1 CROSSING
  en.wikipedia.org               0.48447 → 0.48522  jarring identical
  www.tz.de                      0.80902 → 0.80952
  desiviral · freesupertips · ubys · possssno · marktplaats · littlecaesars ·
  blog.rust-lang · a11yproject                       byte-identical
  sestra.cc                      0.9322 → 0.9225    NOT A RESULT — the OLD reading sits inside the
                                                    NEW binary's own three-run spread (0.9225 /
                                                    0.9394 / 0.9274, reading-order 5 / 2 / 4)
```

Gated by `a_bfc_root_is_placed_beside_a_float_and_a_plain_block_is_not`, which asserts **both**
halves — the plain block must still span the full width, so a fix that simply shortened every block
beside a float fails the gate rather than passing it.

## `transform` was silently discarded on a flex item that is itself a flex container (t874)

The rule "bake a box's `transform` into its subtree's coordinates" is written in **`layout_block`**
(every ordinary box) and again in the **out-of-flow pass** (every absolutely-positioned box).
`extract_placed` — the third emitter, the one that returns a flex/grid item **that is itself a flex
or grid container** — had neither, so the transform did not apply at all.

A **leaf** flex item is laid out through `layout_block` and was therefore always right. That is what
hid this: the same page transforms correctly or not at all depending only on whether the transformed
box happens to carry `display:flex`.

Chrome-measured, a 120×40 flex item inside a `display:flex` row:

```text
                                            Chrome              before              after
  display:block  translateX(50px)         [170,  0]           [170,  0]           [170,  0]   ✓ leaf
  display:flex   translateX(50px)         [170,  0]           [120,  0]           [170,  0]
  display:grid   translateY(10px)         [120,142]           [120,132]           [120,142]
  display:flex   scale(2)                 [ 60,178 240x80]    [120,198 120x40]    [ 60,178 240x80]
```

### Reach

Every **slide-in drawer**, hover-lift card, carousel track and centred modal that is a flex container
inside a flex container — which is how the framework CSS of the moment writes all four. On
`desiviral.net` it is an off-canvas `aside.fixed.flex.-translate-x-full`: the sidebar **stayed on
screen**, overlapping the header and the footer, because the translate that hides it never applied.
`overlap 5 → 0`, and the site crossed the M1 bar.

### A second finding, measured and NOT fixed

An out-of-flow child of a flex container is emitted **twice** — once by the flex path and once by the
out-of-flow pass — and `node_rects` unions the two. Before this fix the two copies disagreed and the
union was visible as a **doubled width** (an `aside` reported 512px wide with a 256px `w-64`); now
they coincide and it is invisible again. Per Flexbox §4 an absolutely-positioned child *"does not
participate in flex layout"* and should not be emitted by that path at all, but deleting a box is
how elements vanish, and the out-of-flow pass's ownership of every such case needs its own tick to
establish. Named here so the next reader does not have to rediscover it from a doubled width.

## The two highest-ranked causes are not the two primitives you reach for first (t905)

The t904 sweep's mechanism oracle ranks corpus-wide causes by DISTINCT SITES:

```text
  36 sites · 2386 hits   missing box: <div>
  29 sites ·  280 hits   geometry/mis-sized: height ~256px   [median 364px]
  23 sites · 1154 hits   geometry/mis-sized: width  ~32px    [median 32px]   <- median EXACTLY the label
```

**A bucket whose median lands exactly on its own label is a spike, not a band.** Every other band
medians well above its label (`height ~64px` at 89, `width ~8px` at 13) because a power-of-2 bucket
has a spread inside it. The 32px row does not — and its ebay instance reads like a constant: Chrome
`[48 1158 1104×132]`, ours `[32 1156 1136×116]`, sixteen pixels further left and thirty-two wider.
That is the signature of a 16px-per-side horizontal inset being dropped.

It is not one. Thirty Chrome-captured claims, ten minutes of fixture: `padding: 0 16px` ·
`padding-inline` · `padding-inline-start/end` · `margin: 0 16px` · `margin-inline` ·
`box-sizing:border-box` with padding · left/right borders · flex container · grid container · `1rem`
· `1em` · `4%` · `direction:rtl` · physical longhands — parent box and child box, x and width, all
exact.

The second cause died the same way. Its examples are boxes with the right x, the right width and
**zero height** (ebay `1200×360` against ours `1200×0`; ikea `739×456` against `739×0`) — the
signature of unimplemented `aspect-ratio` or of the `padding-top:56.25%` hack it replaced. Twelve
more claims: bare ratio, positioned child, flex, grid, `min-height`, `max-height`, the percentage
hack, and float containment by `overflow:hidden`/`flow-root`. All exact.

> **Rank a cause by sites and you learn where to look. You do not learn what to build.** The two
> highest-ranked causes on this corpus were both disproved by a four-line fixture before a line of
> engine code was written — which is the cheapest possible outcome and the one to seek first.

### `aspect-ratio` had no row in the capability map at all

Not `gated`, not `missing`, not `unknown` — absent. So nothing in the project could say the
capability was already built and already correct, and the burndown was free to keep it as a live
suspect forever. Fourth occurrence of the same law, after `localStorage`, `FormData`,
`position:sticky` and `IntersectionObserver`: **an absent measurement is not a negative measurement.**

### Two of the three defects the probe found were the probe

1. `padding-top:56.25%` read Chrome **667** against our **675** — a clean 15px, which is a scrollbar.
   The manual Chrome invocation omitted **`--hide-scrollbars`**, a flag the fidelity harness always
   passes, so the percentage resolved against 1185 rather than 1200. With the flag: 675, to the pixel.
2. A `display:flow-root` box appeared 120px left of Chrome's. The fixture put each case in a plain
   `<div>`, so each row's float **escaped into the next row** and the x values were cumulative.
   Isolating each case in its own BFC removed it.

Both are the standing laws firing inside one tick — *every number has a harness*, and *the probe's own
sentinel widens its subject*. A differential fixture is only a control if each case is a control.

### ⚠ CORRECTED AT t906 — the third defect was the fixture too

t905 concluded from the isolated fixture that a BFC box fails to avoid a float that **escaped a
previous sibling**. It does not. That fixture's boxes carried `width:400px` as well as the wrapper —
two variables, one reading. With `width:auto` restored, every escaped-float case is Chrome-exact and
always was:

```text
  one 60px left float in a plain <div id=host>, boxes at width:AUTO
    display:flow-root / overflow:hidden / display:flex   Chrome x=60 w=1140   ours x=60 w=1140
    a plain block                                        Chrome x=0  w=1200   ours x=0  w=1200
    clear:left                                           Chrome x=0  w=1200   ours x=0  w=1200
```

**Three defects across two ticks, and all three were the fixture**: a missing `--hide-scrollbars`,
floats leaking between un-isolated rows, and a confounded width.

> **A differential probe is only a control if each case varies ONE thing.** Isolation is not a tidiness
> preference; it is the entire difference between a measurement and a story.

### What the confound led to instead, which was better

The real defect was in `bfc_float_band`'s own comment, named and declined since t859: *"A SPECIFIED
width is deliberately NOT handled here … Chrome shifts such a box beside the float only while it
still fits … Measured, named, and left as its own tick."* `width:400px` was the variable, and it led
straight to the follow-up the file had been asking for. A 100px `float:left` in a 400px `flow-root`:

```text
                                         Chrome    before    after
  width:300px  (fits the 300px band)      x=100     x=0       x=100    <- boundary, INCLUSIVE
  width:301px  (one px too wide)          x=0       x=0       x=0
  width:200px  margin-left:20px           x=100     x=20      x=100    the margin is ABSORBED
  width:50%                               x=100     x=0       x=100
  float 10px tall, box 60px tall          x=100     x=0       x=100    band read at the TOP
  box-sizing:border-box with padding      x=100     x=0       x=100
```

Eight of fourteen were wrong — **and the six that were right are half the deliverable.** `301px` and
`400px` are the spec's *"if necessary, implementations should clear"* half; a fix that shifted
unconditionally would satisfy the other eight and break these two.

**`cw` is returned UNNARROWED, and that is the whole difference from the `auto` arm.** The stated
reason for declining this work was that narrowing `cw` would re-resolve every percentage inside the
box against the band. That objection is answered by not narrowing it, not by declining the shift: an
auto box takes the band as its containing block because the band is what sizes it, while a specified
box keeps its width and only its ORIGIN moves. `width:50%` proves it from outside — Chrome resolves
50% against the 400px container, gets 200, and still shifts the result to 100.

### The original (superseded) reading, kept because the error is the lesson

### And isolating the second artefact is what found the real defect

A float that escapes a non-BFC previous sibling belongs to the ancestor's float context, and every
following BFC sibling must shift past it:

```text
  one 60px left float inside a plain <div id=host>     Chrome    ours
    display:flow-root  after the escape                 x=60      x=0     <- WRONG
    overflow:hidden    after the escape                 x=60      x=0     <- WRONG
    display:flex       after the escape                 x=60      x=0     <- WRONG
    a plain block      after the escape                 x=0       x=0     correct to overlap
    clear:left         after the escape                 x=0       x=0     correct
```

`bfc_float_band` implements CSS 2.1 §9.5 correctly and is Chrome-exact **whenever the float and the
BFC box share a container**. The gap is *which float context the band is read from*. This is the
pre-flexbox web meeting the modern one — a float wrapped in a plain `<div>`, followed by an
`overflow:hidden` or `flex` section — and it produces a wrong `x` **together with** the `overlap` and
`h_overflow` jarring dims, which makes it the shape-and-jarring-together mechanism t904 identified as
the only lever with real M1 crossings.

## A table box's `height` is a MINIMUM (t907)

CSS 2.1 §17.5.3: *"the table's height is the maximum of the value of [the] 'height' property … and
the sum of the row heights."* A table whose content is taller than its declared height **grows**,
where a block clamps and lets the content overflow — and `max-height` on a table has no effect for
the same reason. This engine treated a table box's height as a used value like any other block's.

```text
                                                      Chrome   before   after
  display:table; height:20px       (content 24)         24       20      24
  display:table; height:20px       (three lines, 72)    72       20      72
  display:inline-table; height:20px                     24       20      24
  display:table-cell; height:20px                       24       20      24
  display:table; height:20px; border-box; padding:5px   34       20      34
  display:table; max-height:10px                        24       10      24
  display:table; height:60px       (content 24)         60       60      60   already right
  display:BLOCK; height:20px                            20       20      20   MUST still clamp
```

**The last row is half the deliverable.** A plain block that overflows its declared height is
correct, and a fix phrased as *"let boxes grow"* rather than *"this is the table box's own rule"*
would satisfy every other row and silently break every fixed-height block on the web.

### Two unrelated probes, two ticks apart, the same number

`display:table` turned up as an open row in t905's float battery (Chrome 24, ours 20) and was left
unasserted as a curiosity. `display:table-cell` turned up in t907's missing-box battery with the
identical reading. **A second sighting under a different subject is what turns a one-off into a
family worth a rule** — the same shape as t720-724's *"three sightings under three subjects were ONE
bug"*.

## A probe that cannot distinguish ABSENCE from ZERO is measuring its own encoding (t907)

The missing-box probe asked *"what does Chrome give a box that we give none?"* and reported `hidden`,
`<template>` and `display:none` as defects. They are correct in both engines:
`getBoundingClientRect()` on a boxless element returns `0,0,0`, which is byte-identical to a real box
of zero size. The probe's own encoding erased the distinction it was built to find.

That is the fourth fixture defect in three ticks, after a missing `--hide-scrollbars` (15px), floats
leaking between un-isolated rows (120px), and a confounded `width:400px`. Each was caught by reading
the NUMBERS rather than the verdict, and each was cheaper than the fix it would have bought.

> Running tally worth keeping, because the ratio is the point: across t905-t907 these probes produced
> **seven** apparent defects. **Four were the probe.** Three were real, and all three are now gated.

## A capability correct whenever anyone asks for it, and wrong when nobody does (t908)

`table { border-spacing: 2px }` is in Chrome's UA stylesheet and was not in ours. The separated-
borders model insets every cell from the table edge and from its neighbours by that much, so a plain
`<table>` with no author CSS — most of the data tables on the web — had every cell 4px too wide,
flush against the table edge, and the table 4px too short per row.

```text
  a 200px table, one `padding:0` cell            Chrome        before    after
    <td>                                         x=2 w=196     x=0 w=200  ✓
    <table>                                      h=28          h=24       ✓
    two cells side by side                       100 / 94      103 / 97   ✓
    two rows                                     h=54          h=48       ✓
```

**The property itself was already perfect.** `border-spacing: 10px` matched Chrome to the pixel and
always had; `border-spacing: 0` matched; `border-collapse: collapse` matched. Parser, cascade and
layout consumer were all correct. The defect was in the one place nobody writes a test for.

> **A capability that is correct whenever anyone asks for it, and wrong when nobody does, is invisible
> to every test that sets the property.** Every fixture this engine had for `border-spacing` declared
> it. Probe the DEFAULT, not just the declaration.

One line; 19 of 23 measured rows went from wrong to exact.

### Naming something out of scope is a hypothesis about its size

t907 measured these same rows, called them *"the table ALGORITHM rather than the box's own height
rule"*, and deferred them. Two of the three were a one-line UA gap — wrong by two orders of
magnitude. An out-of-scope note is a useful thing to write and a dangerous thing to trust: it records
where the evidence stopped, not how big the work is.

### The guards belong beside the fix

A UA declaration is the easiest kind of change to over-apply, so the gate asserts the three ways it
must NOT fire — `border-spacing: 0` still collapses to zero, an author's `10px` still wins, and
`border-collapse: collapse` still ignores spacing entirely — as INSET relationships
(`cell.x - table.x`) rather than absolute coordinates, so ten stacked tables cannot make one
regression print as twenty-three.


## A control's own box and the line that holds it are ONE change (t917)

Chrome's UA defaults for form controls, read with `getComputedStyle` rather than guessed:

```text
              border   padding      box-sizing        ours (ONE shared rule)
  input        2px     1px 2px      content-box       border 1px, padding 1px 2px
  button       2px     1px 6px      border-box        border 1px, padding 1px 6px
  select       1px     0            border-box        border 1px, padding 1px 2px
  textarea     1px     2px          content-box       border 1px, padding 1px 2px
  checkbox     0       0            border-box        border 1px (inherited)
```

Four controls, four different answers, one shared rule. Every text input and every button on the web
was **exactly 2px short in both axes**. Correcting it took all ten measured heights exact.

### `getComputedStyle` did not predict the used box for two of the five

Chrome reports `textarea { padding: 2px }` and `select { padding: 0 }`. Adopting either made a control
that was **already byte-exact** wrong — the textarea 36 → 38, the select 30×19 → 26×17 — because both
have an internal shadow subtree the reported longhand does not account for.

> **The used metric is the ground truth; the declaration is not.** Take the UA value only where the
> RENDERED box confirms it.

### And then the composite case regressed, so the whole thing was reverted

`<div><input></div>` is 24 in Chrome. It was 26 here, and with the *correct* 21px input it became
**28** — the control's own box got right and the box containing it got further wrong, because our
form controls take CSS 2.1 §10.8.1's **fallback baseline** (the bottom margin edge) rather than their
internal text baseline:

```text
  Chrome   baseline ~17 from the top     ->  above 17, below 4  ->  max(17.5,17) + max(6.5,4) = 24
  ours     baseline = h = 21 (fallback)  ->  above 21, below 0  ->  21 + 6.5                 = 27.5 -> 28
```

The ratchet is absolute: a universal improvement to five controls' own boxes does not buy a composite
row moving away from Chrome. Reverted. Same shape as t913 → t914, where growing the line box without
moving the glyphs was refused for the same reason and the pair shipped together one tick later.

**Both halves, for whoever takes it:** the five UA rows above, plus a real internal baseline for form
controls — and `<div><input></div>` must read 24 when they land.

### The baseline half landed at t918, and it stands alone

`last_line_baseline` returns `None` for an `<input>` because its value lives on the element and not
in the tree, so §10.8.1's fallback applied. Synthesising the control's own first-line baseline
(border + padding + the ascent of ITS font — Chrome's UA gives these 13.333px Arial, not the page's
16px) takes `<div><input></div>` to Chrome's 24 **with the UA boxes untouched**, which is what makes
the UA correction landable beside it rather than instead of it.

The narrowness is the point. The synthesis fires only where the real rule cannot, and the guards say
so: a text-bearing `inline-block` still uses its own last line (24), an `overflow:hidden` one still
takes the fallback (**31**, not 24), an empty one is unchanged, and `textarea` — already byte-exact —
is excluded entirely. **A row that is already right is not a row to route through a new mechanism.**

Open, 1px: an input with an explicit `height:40px` reads 47 against Chrome's 46, because Chrome
centres the internal editor in a taller control and we place the baseline at border+padding+ascent
regardless.


## `None` from a baseline lookup has two meanings (t924)

`last_line_baseline` returning `None` can mean either:

* **"this control's value lives on the ELEMENT"** — an `<input>`, whose text is not in the tree, where
  CSS 2.1 §10.8.1's bottom-margin-edge fallback is simply wrong; or
* **"this box genuinely has no in-flow line box"** — a `<button>` wrapping a block, where the fallback
  is CORRECT and Chrome uses it too.

t918 synthesised a baseline for `input | button | select` on the strength of the first meaning and was
reverted by the corpus. Measured, a `<div>` around each, with no synthesis at all:

```text
  <input>                                   Chrome 24   ours 26   <- the ONLY one that is wrong
  <button><span>Sign In</span></button>            24         24
  <button>Sign In</button>                         24         24
  <button></button>                                24         24
  <select><option>a</option></select>              24         24
  <button><div class=icon></div></button>          28         29   <- 1px, a different mechanism
```

`secure5.entertimeonline.com` — the site that caught t918 — contains
`<button><div class='icon-Eye_18'></div></button>`: content a single empty block, so no line box, so
the synthesis fired where Chrome takes the fallback.

### And the element set was two thirds of it, not all of it

Narrowed to `<input>` alone, the site still falls 0.872 → 0.692 (two solo runs, byte-identical). So
the `<input>` formula itself is wrong on a **styled** control — the site's real fields are
script-created and styled, while every isolated fixture used a default-height input where the formula
happens to be exact.

The model t918 named as open — Chrome centres the editor vertically in a taller control — was tried
and **overshot**: `<input style="height:40px">` is 46 in Chrome, 47 without centring, **44 with it**.

> **Narrowing a defect is a result; guessing the remainder is not.** The next attempt starts with a
> settled element set (a measured claim now, not an assumption), a refuted centring model, a
> two-line discriminating fixture, and a corpus reproducer with a known-good number to return to.


## A RED proof aimed at the wrong cascade is a green light (t925)

`border-spacing` takes two lengths and `ComputedStyle` carried one `f32`, so
`border-spacing: 10px 20px` inset rows by the COLUMN value: Chrome 64, ours 44. The parser's own
comment said so — *"Only the first (horizontal) length is used in this slice"* — which is the useful
half of a comment that documents a gap and the dangerous half of one nobody re-reads.

**The first RED proof passed.** Mutating MinimalCascade's parser left the gate green, because
`stylo_map.rs` reads the pair from Stylo's `clone_border_spacing()` and a `manuk-page` gate runs the
**shipping** cascade. The proof only bites when the Stylo mapping is reverted
(`.vertical()` → `.horizontal()`).

> **Falsification has to hit the path the gate actually runs.** The standing note
> `live-cascade-is-stylo-not-minimal` has been about fixes; it applies identically to RED proofs, and
> a proof aimed at the other cascade is indistinguishable from a gate that cannot fail.

Both cascades were updated — Stylo's `.vertical()` and MinimalCascade's two-length parse — because
t923 landed one tick earlier on exactly the drift between them.

```text
                                 Chrome   before   after
  border-spacing: 10px 20px        64       44       64
  border-spacing: 10px             44       44       44   <- one value still sets BOTH
  border-spacing: 0                24       24       24
  the UA default (2px)             28       28       28
```

The single-value rows are what make the new one assertable: a fix that read the second value and
forgot the shorthand would satisfy the new claim and break four old ones.


## Chrome's `<input>` baseline, measured — and when the CORRECT model scores worse (t927)

Nine control heights determine it. The containing `<div>` in a 16px/1.5 line, against the input's own
border-box height:

```text
  input border-box h     6    16    26    36    46    66   106   21(default)
  Chrome div height     24    24    26    36    46    66   106   24
```

For every `h` past the strut the div is **exactly `h`**, which pins the baseline at
`h − (border-bottom + padding-bottom + descent)`. **The editor's text sits on the control's bottom
padding edge** — bottom-anchored, not centred, and not the bottom margin edge.

The same table refutes both earlier attempts: CSS 2.1 §10.8.1's fallback (bottom margin edge) gives
26 where Chrome says 24 (t918's starting point), and a centred editor gives 44 where Chrome says 46
(t924). Implemented, the bottom-anchored model is exact on all nine heights, on the whole ten-claim
baseline fixture (previously 7/10), and on `<div><input></div>`.

### And it still scores worse on the corpus

`secure5.entertimeonline.com`: three solo runs byte-identical at **0.692308** against the clean tree's
**0.871795**, with `cov 1.000000` and `n=39` in **both** — so not a composition effect, not the
element set, and now not the formula.

> **Three different baselines produce three different wrong scores on this site, and only one of them
> is right about Chrome. When the CORRECT model scores worse than the incorrect one on the same 39
> elements, the thing being scored AGAINST is the next place to look.**

The standing hypothesis: the oracle scores us against Chrome rendering a `curl`'d snapshot from
`file://`, and this is a login page whose fields are script-built. If that snapshot's JS does not run,
moving our inputs to where Chrome LIVE puts them moves them away from where the REFERENCE has them.
`fidelity.rs` carries three variants for this family of reference defect and none fires here, because
the page is not a shell — it renders 39 comparable elements, just possibly not in the same states.

**The cheap kill:** dump the oracle's reference for the site and count its `<input>`s against the live
page's. One `--dump-dom` each.


## The form-control baseline: four attempts, four reverts, and a handoff (t928)

```text
  t917  the UA box alone            all ten control heights exact   <div><input></div> 26 -> 28
  t918  baseline, input|button|select   nine fixtures, four guards   secure5 0.872 -> 0.692
  t927  baseline, <input> only, bottom-anchored   nine heights + 10/10 fixture   secure5 unchanged at 0.692
  t928  both halves together        six isolated heights 1px WORSE   secure5 unchanged at 0.692
```

**Every attempt was Chrome-exact on its fixtures and cost a corpus site or a composite case, and
every one was reverted rather than traded.** The tree is byte-identical to where t917 found it.

### What is settled, so nobody re-measures it

* **The model.** `baseline = h − (border-bottom + padding-bottom + descent)`, from nine control
  heights — the containing div is exactly `h` for every `h` past the strut. Rivals refuted with
  numbers: §10.8.1's bottom-margin-edge fallback gives 26 where Chrome says 24; a centred editor
  gives 44 where Chrome says 46.
* **The element set.** `<input>` only. `<button>` — with text, with a block child, empty — and
  `<select>` are already exact with no synthesis.
* **The UA box.** Chrome's `getComputedStyle` defaults: input 2px / 1px 2px · button 2px / 1px 6px ·
  select 1px / 0 · textarea 1px / 2px · checkbox 0 / 0. **`textarea` and `select` must keep OUR
  padding** — adopting Chrome's declared value breaks controls that are byte-exact, because both have
  an internal shadow subtree the longhand does not describe.
* **The third part.** The intrinsic content width is 2px wide (`<input size=1>` is 53 in Chrome and
  becomes 55 once the border is right); `g_form_control_metrics` already asserts it.
* **The reproducer.** `secure5.entertimeonline.com`, known-good **0.871795 / cov 1.000000 / n=39**.
* **A dead hypothesis.** The oracle's reference for that site is *structurally identical* to the live
  page — 99 tags, 19 inputs, 3 visible, 2 buttons in both — so "the snapshot has a different form
  state" is refuted.

### What is not known

Why a Chrome-exact input baseline moves seven of that site's thirty-nine elements out of tolerance
when the reference has the same structure. **The next attempt should start by diffing the two renders
element-by-element, not by proposing a fifth model.**

## The four Box-Alignment longhands, and the two that never existed (t980, t981)

CSS Box Alignment is a 2×2: an **inline** axis and a **block/cross** axis, each with a
**distribution** property (how the container spreads its lines/tracks) and an **item-default**
property (what a child gets when it says nothing). Manuk had built one diagonal of that square and
neither of the other two corners, at any layer:

```text
                     INLINE axis (justify-*)     BLOCK / CROSS axis (align-*)
   container: distribute   justify-content ✓          align-content   ✗  (t981)
   container: item default justify-items   ✗ (t981)   align-items     ✓
   item: override          justify-self    ✗ (t980)   align-self      ✓
```

Three properties, three ticks, and the same failure each time. Each missing property sat **directly
beside** its axis-twin in `taffy_tree.rs`'s `ComputedStyle → taffy::Style` mapping, so every reader
of that block — including the ones who added a property to it — saw a self-alignment property being
mapped and moved on.

> **A gap survives when the neighbouring line looks like coverage.** The strongest predictor of an
> unimplemented CSS property is not obscurity; it is having a well-implemented twin in the same
> struct literal.

### Why the initial value was right the whole time, and only declared values were wrong

`align-content`'s initial `normal` behaves as **stretch** on the block axis, in both flex and grid —
which is also taffy's default when the field is left at `None`. So an undeclared `align-content`
produced Chrome-exact geometry, on every page, forever. The property is invisible to any instrument
that measures pages which do not use it, and wrong on every page that does. The same is true of
`justify-items`, whose initial `legacy` computes to `normal` → stretch → start for a definite-width
item.

This is the difference between *"absent"* and *"broken"*, and it decides how you find it: a
divergence sweep cannot rank a property that is only wrong when declared, because the corpus rows
that declare it are a minority of a minority. **What finds it is a battery in which every row
declares one value of one property and a control row declares none.**

### The `place-*` shorthands are the sharpest diagnostic in the family

`place-content: center` and `place-items: end end` each set both axes in one declaration. Before
t981 each of them landed **exactly half**: Stylo expanded the shorthand correctly, one longhand was
consumed and the other was discarded because there was no field to put it in.

```text
   place-content: center      Chrome [120, 60]   ours [120,  0]   <- justify landed, align dropped
   place-items:   end end     Chrome [140, 60]   ours [  0, 60]   <- align landed, justify dropped
```

A property that **arrives and is discarded** is indistinguishable from one that never parsed — from
the outside. A shorthand that sets two axes is the cheapest instrument that can tell them apart,
because it puts both halves on one declaration and asks which one survived.

### The shape of the fix, and the shape that prevents the next one

Four longhands now share **two** parsers (`values::parse_content_distribution` and
`parse_item_alignment`, one per value set) rather than four hand-written match blocks, and the Stylo
map shares one `map_cd` closure between `justify-content` and `align-content`. That turns a missing
property from a *missing arm* — which nothing can grep for — into a **missing call site**, which
`grep -c parse_content_distribution` finds in one second.

The one thing the axes genuinely do not share is `left`/`right`: legal on `justify-*`, invalid on
`align-*` (where the declaration is dropped and the initial value stands). That is threaded as an
explicit `AlignAxis` argument rather than papered over, because a shared parser that silently accepts
`align-content: right` would be a *new* divergence introduced by the cleanup.

### Measured

Headless Chrome against a 300×200 container of 60×40 items, every row an offset from its own
container. Eleven declared rows moved, four control rows did not, and `grid-auto-flow: column`
remained the family's one open defect (a placement algorithm, not an alignment property).

```text
   flex-wrap  align-content:flex-start     last line   Chrome [0,  40]   before [0, 100]
   flex-wrap  align-content:center        first line   Chrome [0,  60]   before [0,   0]
   flex-wrap  align-content:flex-end      first line   Chrome [0, 120]   before [0,   0]
   flex-wrap  align-content:space-between  last line   Chrome [0, 160]   before [0, 100]
   grid       align-content:center          first row  Chrome [0,  60]   before [0,   0]
   grid       align-content:end             first row  Chrome [0, 120]   before [0,   0]
   grid       align-content:space-between    last row  Chrome [0, 160]   before [0,  40]
   grid       justify-items:end                        Chrome [140, 0]   before [0,   0]
   grid       justify-items:center                     Chrome [ 70, 0]   before [0,   0]
   ── controls, unmoved ──
   flex-wrap  (undeclared: stretch)        last line   Chrome [0, 100]   ours   [0, 100]
   flex-wrap  justify-content:center        last item  Chrome [120,100]  ours   [120,100]
   grid       justify-content:center                   Chrome [120,  0]  ours   [120,  0]
   grid       align-items:end                          Chrome [0,   60]  ours   [0,  60]
```

Gated by `G_CONTAINER_ALIGNMENT` (Stylo path) and
`the_four_container_alignment_longhands_all_parse` (minimal cascade). RED-proven three ways: drop the
`align_content` line, drop the `justify_items` line, or fold `align-content: normal` into
`flex-start` — the third fails **only** the undeclared control row, which is the row that exists for
it.

## The grid tracks the author did not write down (t982)

`grid-template-rows` / `grid-template-columns` size the **explicit** tracks. When a grid holds more
items than those tracks have room for, the auto-placement algorithm invents **implicit** tracks, and
three properties govern them:

| property | what it decides |
|---|---|
| `grid-auto-flow` | which axis placement advances along (`row` \| `column`), and whether it may go **backwards** to back-fill a hole (`dense`) |
| `grid-auto-rows` | the sizes of the implicit **rows**, as a list that is **cycled** |
| `grid-auto-columns` | the same for implicit **columns** |

All three are fields on taffy's `Style`. **Nothing ever wrote any of them**, at any layer — no
`ComputedStyle` field, no parse arm, no `stylo_map` line, no `to_taffy_style` line — so every grid
whose items outran its template put the overflow in a new *row* of *content* height, whatever the
author declared. Measured against headless Chrome (a 300px grid of 60×40 items, offsets from each
item's own container):

```text
                                                     Chrome       before        after
  grid-auto-flow:column           3rd item          [150,  0]   [  0, 80]   [150,  0]
  grid-auto-flow:column
    + grid-auto-columns:90px      3rd item          [ 90,  0]   [  0, 80]   [ 90,  0]
  grid-auto-rows:80px             5th item          [  0,120]   [  0, 80]   [  0,120]
  grid-auto-rows:80px 20px        5th item          [  0,120]   [  0, 80]   [  0,120]
  grid-auto-rows:80px 20px        7th item          [  0,140]   [  0,120]   [  0,140]
  grid-auto-flow:row dense        back-filled item  [  0,  0]   [  0, 40]   [  0,  0]
 ── controls, none of which moved ──
  (nothing declared)              3rd item          [  0, 40]      same     unchanged
  same grid, no `dense`           2nd item          [  0, 40]      same     unchanged
  fixed-height container, implicit rows STRETCH     [  0,120]      same     unchanged
  explicit tracks only                              [  0, 40]      same     unchanged
```

### Why a divergence sweep can never rank this family

`row` is the initial `grid-auto-flow`; an empty auto track list means `auto`. Both are *also* taffy's
defaults. An undeclared grid was therefore Chrome-exact forever, and the properties were wrong **only
where they were declared** — the identical shape as `align-content` one section up. Three ticks
(t980, t981, t982) found three such properties, each sitting directly beside a complete twin in the
same struct literal, and none of them was rankable from corpus divergence. What finds them is a
battery where **every row declares one value of one property and a control row declares none**.

### Two grammars that look like one

`repeat()` is legal in `grid-template-*` and **forbidden** in `grid-auto-*`. An auto track list has no
length of its own — it is *cycled* over however many implicit tracks placement creates, so
`grid-auto-rows: 80px 20px` makes them 80, 20, 80, 20… Sharing the richer template parser would
silently accept `grid-auto-rows: repeat(auto-fill, 80px)` and then have nowhere to put it, so the two
properties get two parsers and the difference is asserted in both directions.

### Building the fixture so it can actually fail

The first `grid-auto-rows` fixture attempt **could not discriminate**: in a fixed-height container the
undeclared `auto` implicit rows *stretch* into the free space and happened to land exactly where the
declared 80px rows would. A fixture where the wrong model and the right model agree is not a test. The
gate uses an **auto-height** container — zero free space, nothing to stretch into — and keeps the
fixed-height version as a *control*, so a fix that hard-coded implicit rows to content height fails
there.

The `dense` bit needs the same care in the other direction: `row dense` differs from `row` only in
back-filling, so the gate carries the **same markup twice**, with and without the keyword. A gate that
only tested `column` would call the flow property covered while half its value space was thrown away
— and folding `RowDense → Row` fails exactly one row of the eleven.

### Named, measured, not built — a DIFFERENT mechanism

**A grid container's block size comes from its items' content, not from its resolved tracks.** A grid
with `grid-template-rows: 100px` holding one 40px item is 100 tall in Chrome and **40** here — with no
implicit track and no `grid-auto-*` declaration anywhere, so it predates this tick and is untouched by
it (the three new lines map to taffy's own defaults when undeclared, and `k`/`l`/`m` read identically
on every RED variant). It hides because the *tracks* are laid out correctly and only the container's
own height is short: every item offset in `G_GRID_IMPLICIT_TRACKS` is exact while two of its
containers are 40px shy.

Gated by `G_GRID_IMPLICIT_TRACKS` (Stylo path) and
`grid_implicit_track_properties_parse_on_the_minimal_cascade` (minimal cascade). RED-proven four ways,
each **confined**: dropping `grid_auto_flow` fails the three flow rows and no auto-rows row; dropping
`grid_auto_rows` fails the three auto-rows rows and no flow row; dropping `grid_auto_columns` fails
only the row that declares one; folding `RowDense → Row` fails only the `dense` row.

## A grid container's height is its TRACKS, not its children's bottom edge (t983)

`layout_flex_or_grid` returned the container's content height as `max_h` — how far down the lowest
child reached — and **threw away the height taffy had already resolved for the container itself**.
For **flex** those two are the same number, which is why it survived a long time: a flex line's cross
size *is* its tallest item. For a **grid** they are different questions. A grid container's block
size is the sum of its resolved ROW TRACKS plus the row gaps, and **a track has a size whether or not
anything fills it**.

```text
                                                Chrome     before      after
  grid-template-rows:100px, one 40px item         100         40        100
  grid-template-rows:20px,  one 40px item          20         40         20
  grid-template-rows:40px 100px, two items        140         80        140
  grid-template-rows:40px 70px, ONE item          110         40        110
  grid-template-rows:100px + padding:10px         120         60        120
  grid-template-rows:40px 40px; row-gap:30px      110        110      unchanged
 ── FLEX controls, the half that was always right ──
  flex row, tallest item 70px                      70         70      unchanged
  flex column, two 40px items                      80         80      unchanged
  flex, height:30px around a 40px item             30         30      unchanged
```

### The row that decides the SHAPE of the fix

`grid-template-rows: 20px` around a 40px item is **20** in Chrome — the container is *shorter than
its own content* and the item overflows. So the fix can never be `max(child_extent, tracks)`: a
combination that only ever grows keeps that case wrong in the direction that looks safe. The answer
has to be the formatting context's own, and **taffy had computed it and the call site discarded it**.

### Why it hid for so long

The `row-gap` row was already correct, because a gap sits *between* the children and the lowest
child's bottom edge therefore includes it. Every grid whose tracks are exactly as tall as their
content — which is every grid that sizes its rows `auto`, the common case — agreed with Chrome **for
the wrong reason**. Only a track bigger or smaller than what fills it can tell the two models apart,
and a trailing *empty* track is the sharpest version: there is no child down there at all, so the
child-extent model cannot even see it.

### Where it was found

Not by aiming at it. `G_GRID_IMPLICIT_TRACKS` (t982) got all eleven of its ITEM offsets exact and two
of its CONTAINERS stayed 40px short. **Measure the container after the items agree** — an item-only
readout declares a family finished one layer too early, and this is the second consecutive tick where
the residue came from that habit.

### Blast radius

Every flex and grid container on every page now takes its height from taffy rather than from its
children. An old-binary A/B on four anchor sites in the same hour moved mean shape 87.3% → 87.4% with
all four jarring invariants byte-identical: **no regression, and no measurable gain either.** The fix
is Chrome-exact on the fixture and invisible on those four pages, which is the honest report.

Gated by `G_GRID_CONTAINER_HEIGHT`. RED-proven two ways: returning `max_h` (the original code) fails
the five track rows while every flex control passes — exactly the confinement that let it survive —
and returning `max_h.max(solved_h)` fails only the shrink row. ⚠ A third recipe, swapping
`content_box_height()` for `size.height`, does **not** go red: `TaffyDom::build` zeroes the root's
frame, so the two are equal by construction. Recorded rather than dropped, because a RED recipe that
cannot fire is worse than no recipe.

## `width: fit-content` reached the block path and was given up on inside flex and grid (t984)

`fit-content` parses, maps out of Stylo, lives on `ComputedStyle.width_keyword`, and the **block**
path consumes it in six places via `shrink_to_fit`. The taffy path had one line for it:

```rust
IntrinsicSize::FitContent => return None,   // taffy_tree.rs
```

so inside a flex or grid container the keyword was dropped and the box kept `width: auto` — which for
a grid item means **stretch to the track**, the opposite of what the declaration asks for. This is the
*half-installed* shape, not the absent one: two of the three intrinsic keywords resolve to a length by
asking the measure closure, the third cannot, and the arm that could not return a number did nothing.

### Why it cannot be resolved to a length there

```text
  fit-content = min(max-content, max(min-content, stretch))
```

The `stretch` term is *the space the formatting context is about to hand this box* — not known inside
the style-conversion pass, and not askable without re-entering the measure it sits in. So the keyword
is not resolved; it is expressed as the **bounds it is defined by**, with `size.width` left at `auto`
so taffy's own offer supplies the middle term. Clamping that offer between the two content bounds is
`clamp(min-content, available, max-content)` — `fit-content`, evaluated by the one participant that
knows the available width.

### The ordering, which is the whole subtlety

The `min-content` term lives **inside** `fit-content`; `max-width` clamps the **result**. Taffy
resolves min-over-max, so a first implementation that pushed `min-content` in as a floor and the
author's `max-width` in as a ceiling let the floor outrank the ceiling. The synthetic floor is
therefore clamped by the ceiling first, and only the author's own `min-width` wins over it afterwards
(CSS 2.1 §10.4).

```text
                                                    Chrome     before      after
  fit-content, 200px track, "abc"                      29        200         29
  fit-content, 20px track (narrower than the word)     29         20         29
  fit-content, 40px track, wrappable "aa bbbbbb c"     58         40         58
  fit-content + max-width:20px                         20         20 †       20
  fit-content + min-width:120px                       120        200        120
  fit-content on a FLEX item                           29         29         29
 ── controls ──
  width:max-content · width:min-content · no keyword · block-path fit-content   all unchanged
```

**† that row was right before the fix, by accident, and the first version of the fix broke it.** The
box stretched to its 200px track and `max-width: 20px` clamped it to 20 — the correct answer by a
route that has nothing to do with `fit-content`. Pushing the floor in made it 29.

> **A row that already passes is not a row you can leave out.** This one is the only thing in the gate
> that catches the ordering, and it looked like the least interesting row in the fixture.

Gated by `G_FIT_CONTENT_WIDTH`. RED-proven four ways, each read off the whole fixture rather than the
gate's first failing assertion: deleting the block fails four rows and leaves `#a4`/`#a6` and every
control passing (the partial agreement that let it survive); resolving to `max-content` fails only the
wrappable row; not clamping the floor fails only the `max-width` row; dropping the `min-width`
composition fails only the `min-width` row.

⚠ **Still open, measured in the same battery and deliberately out of scope:** a **percentage
`column-gap`/`row-gap`** is dropped — `column-gap: 10%` of a 300px grid is 30px in Chrome and 0 here.
`ComputedStyle.row_gap`/`column_gap` are bare `f32` px, so a percentage has nowhere to be stored; the
fix is a type widening to `Dim` across the cascade and the taffy mapping, not a missing arm.

## `column-gap: 10%` had nowhere to be stored, so it became zero (t985)

Not a missing arm — a **field that could not represent the value**. `ComputedStyle.row_gap` and
`.column_gap` were bare `f32` px, and every producer funnelled into them accordingly:
`parse_length_px` in the minimal cascade, and in `stylo_map` an `lp_to_dim` whose result was
immediately narrowed by `Dim::Px(p) => p, _ => 0.0`. So a percentage **arrived from Stylo intact and
was thrown away one line later** — the same *arrived and dropped* shape t981 found in the `place-*`
shorthands, one type down.

The fix is the widening. `row_gap`/`column_gap` become `Dim`; taffy's `gap` is a `LengthPercentage`
too, so the percentage crosses intact and is resolved by the participant that knows the basis.

> **A missing ARM is a hole you can grep for. A field that cannot hold the value is a hole with a
> plausible number sitting in it.** Nothing in the pipeline was ever "unhandled" — every stage did
> something, and the something was `0.0`.

### Which basis, measured rather than assumed

```text
                                                       Chrome     before      after
  column-gap:10%, 300px grid                   2nd x      90         60         90    (gap 30)
  column-gap:10%, 300px grid + padding:0 50px  2nd x     130        110        130    (gap 20)
  row-gap:10%, grid with height:200px          2nd y      60         40         60    (gap 20)
  row-gap:10%, AUTO-height grid                2nd y      48         40         48    (gap  8)
  gap:10% 20% shorthand (row THEN column)      3rd y      48         40         48
  column-gap:10% on a FLEX container           2nd x      90         60         90
 ── controls ──
  column-gap:30px · no gap declared                    both unchanged
```

A gap percentage resolves against the container's **content box on that axis**. The padded row is the
only one that separates *content box* from *border box*: 10% of 300 is 30, 10% of the 200px content
box is 20, and only one of those is Chrome's.

⚠ **The auto-height rows were not predicted.** A `row-gap` percentage on an indefinite block size has
a circular basis — the height depends on the gap which depends on the height. Chrome resolves it
against the height computed **with the percentage treated as zero** (40+40=80, gap 8). Taffy does the
same, so those rows came out exact with no extra work; they are asserted because that agreement is a
fact about taffy that could change under us, not a consequence of this fix.

### The CSSOM half, which a geometry-only gate would have missed

`getComputedStyle(el).columnGap` on `column-gap: 10%` returns **`"10%"`** in Chrome — the percentage,
not a used pixel length. Both readers of these fields formatted `{}px` around a float, so widening the
type without touching them prints `10px` for a 10% gap: a *plausible wrong answer of the right type*,
which is a class this project has been caught by before. Both now route through `dim_css`.

⚠ **Still open, named rather than built:** an undeclared gap reads back as `0px` where Chrome says
`normal`. Pre-existing, nothing to do with percentages (`Dim` has no `normal` and the initial value is
`Px(0.0)`), and the `normal` gap *behaves* as zero in both engines — only the serialisation differs.

Gated by `G_PERCENTAGE_GAP` (Stylo path + CSSOM) and
`gap_carries_percentages_and_the_shorthand_sets_row_first` (minimal cascade). RED-proven three ways;
a fourth — swapping the `gap` shorthand's halves — **cannot fire in the page gate** because Stylo
expands the shorthand before we see it, so it lives in the `manuk-css` test instead. That test also
pins the axis order, which matters: `gap: <row> <column>` puts the **block** axis first, the opposite
of the `margin`-style analogy.

## A transformed ancestor is a containing block, and nothing knew it (t986)

CSS Transforms §3: an element with a `transform` becomes the containing block for its
`position: absolute` **and** `position: fixed` descendants — **whatever its own `position` is**.
`filter` and `backdrop-filter` carry the same rule. `abs_containing_block` tested only
`position != Static`, and `position: fixed` was handed the viewport unconditionally, so an
out-of-flow box inside a transformed wrapper escaped past it entirely.

```text
                                                       Chrome      before       after
  fixed    inside transform:translateX(10px)         [ 20, 20]  [ 10,-1328]  [ 20, 20]
  absolute inside transform, ancestor NOT positioned [ 20, 20]  [ 10,-1200]  [ 20, 20]
  fixed    inside filter:blur(0px)                   [ 20, 20]  [ 20,-1072]  [ 20, 20]
  fixed    inside a transformed GRANDparent          [ 20, 20]  [ 10, -816]  [ 20, 20]
 ── controls ──
  absolute inside a plain position:relative ancestor [ 20, 20]  unchanged
  a transformed box with an IN-FLOW child            [  0,  0]  unchanged
```

**Not a rounding error — a box on a different part of the page**, which makes it an I3/jarring-class
defect rather than a shape one. And not rare: `transform` is on **34.5% of the corpus** — every
animated card, carousel slide, `translateZ(0)` compositing hint and CSS-transitioned panel — and the
out-of-flow children inside them are the badges, close buttons, dropdowns and tooltips a user clicks.

### The `absolute` row shows the old test was the *wrong test*

Its wrapper is `position: static`. Under `position != Static` alone the wrapper was invisible as a
containing block, so the box escaped to the viewport — **the ancestor was right there and failed a
test that had nothing to do with the rule being applied.** That is a different failure from "the rule
is unimplemented": the code asked a question, got a truthful answer, and the question was wrong.

### One predicate, not a `transform` special case

`filter: blur(0px)` — a *no-op* blur — is enough, measured. Three properties, one rule; writing it as
`transform`-only leaves two silent holes of identical shape, and narrowing the predicate to
`transform` fails exactly the filter row.

### Named, measured, not built — the t985 shape one level up

`will-change`, `contain` and `perspective` obey this rule too, and they are **not unhandled values**:
they have no `ComputedStyle` field at all, so there is nowhere for the information to live and the fix
is a cascade addition rather than a layout one. `will-change: transform` — much the commonest of the
three, since it is *the* standard compositing hint — is Chrome-exact at `[20, 20]` and reads
`[20, -364]` here.

Gated by `G_TRANSFORM_CONTAINING_BLOCK`. RED-proven three ways, each hitting exactly its own row:
dropping the predicate from the `absolute` walk fails only the absolute row (the `fixed` rows go
through the other walk, so the two halves are separately provable); restoring `fixed => viewport`
fails only the fixed rows; narrowing the predicate to `transform` fails only the filter row. An
old-binary A/B on four anchor sites in the same hour was **byte-identical** — 87.4% mean shape and all
four jarring invariants unchanged on both binaries.

### `will-change`, `contain` and `perspective` — the negative half is the whole difficulty (t987)

t986 left these three out because they had **no `ComputedStyle` field at all**. They now reach layout
as one `bool` — one bit is all layout needs, and carrying a `will-change` string list on every
`ComputedStyle` is the per-node allocation the custom-property field already documents as a measured
mistake. The interesting part is not which values set it but **which do not**:

```text
  will-change: transform / filter / perspective     containing block      [ 20,  20]
  will-change: top, transform  (one qualifying)     containing block      [ 20,  20]
  contain: layout / paint / strict / content        containing block      [ 20,  20]
  perspective: 100px                                containing block      [ 20,  20]
 ── NEGATIVE, and each one is a trap ──
  will-change: opacity                              NOT                   [ 20,-364]
  contain: style                                    NOT                   [ 20,-1132]
  contain: size                                     NOT                   [ 20,-1260]
```

**A predicate written as `!will_change.is_empty() || !contain.is_empty()` — the obvious version —
passes every positive row and is wrong about all three negatives.** `will-change: opacity` creates a
*stacking context*, which is a different thing the same property also does; `contain: style` and
`contain: size` are containment of other kinds. All four negative rows are Chrome-measured, not
reasoned from the grammar, and that naive predicate is the most useful RED recipe in the gate.

On the Stylo path the keyword list is not re-derived: `WillChangeBits::FIXPOS_CB_NON_SVG` is literally
*"a property that creates a containing block for fixed-position descendants will change"*, so the
engine that already computed the answer is asked for it. **Re-deriving it by hand is exactly how the
`opacity` case gets shipped wrong.**

⚠ The cost of the boolean, stated so it is not rediscovered: `getComputedStyle().willChange` cannot be
served from it. We do not publish that property today, and the day we do it needs the list, not the
flag.

## A cell is STRETCHED to its row, and stretching a box does not move what is in it (t989)

A table cell is laid out at its own content height and then stretched to the row's height. That
stretch is a single assignment to `rect.height`, and it does not move the cell's children — so
**every cell was top-aligned**, whatever `vertical-align` said. On a table cell that property is the
pre-flexbox vertical-centring idiom and is still everywhere: toolbars, data grids, icon+label rows,
and the whole `display: table` / `display: table-cell` centring pattern.

```text
                                                       Chrome     before      after
  vertical-align:middle, 19px word in a 60px cell        [20]       [ 2]       [20]
  vertical-align:bottom, same cell                       [38]       [ 2]       [38]
 ── controls, none of which moved ──
  vertical-align not declared (top / baseline)           [ 2]       [ 2]   unchanged
  the CELL BOX of a row sized by its tallest cell        [ 0]       [ 0]   unchanged
```

### The trap, and it cost a build to find

The obvious implementation reads the free space as `row_height − cell_box_height`. That is **zero for
exactly the cells that have the most free space**: a cell with `height: 60px` around a 24px line
reports a *border-box* height of 60, because the explicit height was already applied when the cell was
laid out. The free space has to be measured against the cell's **natural content height**, which
`layout_cell` now returns alongside the border-box height.

> A first version compiled, ran, and moved nothing, because it asked the box how tall it was instead
> of asking the content. **`height` on a box you have already sized tells you what you asked for, not
> what is inside it.**

### The shift goes to the CONTENT, and the gate could not tell until a control was added

The subtree is translated and the box's own origin restored, so the cell's background, borders and hit
rect keep the row's geometry. The RED recipe for that — *shift the cell instead* — **came back green**:
every row in the first draft measured a `<span>` inside the cell, which moves under either rule. The
control that separates them (the cell box of a cell that actually shifts) had to be added *after* the
RED refused to fire. **A gate whose rows all measure the same thing cannot tell two rules apart, and
the RED proof is what reveals that — not the passing run.**

### Named, measured, not built — three more table defects from the same battery

The sixteen-row table battery found **five** divergences in **four** mechanisms. The other three
(fixture `/tmp/tbl.html`):

```text
  ROWSPAN row-height distribution   a 60px rowspan=2 cell must give 30/30 to its two rows; we
                                    give 24/36 — the overflow all lands on the LAST row
  CAPTION                           `<caption>` reserves no space and does not widen the table:
                                    the first cell belongs at y=20 and 29 wide, reads y=0 and 10
  THEAD ORDERING                    a `<thead>` written AFTER a `<tbody>` must still render FIRST;
                                    we render in source order
```

The rowspan one is what `CONSTITUTION.MD` VI.2 has carried as *"t933 row-height distribution"* since
check #82 — and this is the first fixture to put a number on it.

Gated by `G_TABLE_CELL_VALIGN`.

## A rowspan cell's excess is SHARED by the rows it spans (t990)

A `rowspan` cell taller than the rows it covers has excess height to place. That excess was added
entirely to the **last** spanned row — `row_h[last] += *bh - spanned`. Chrome shares it across every
spanned row, **proportionally to their natural heights**. This is the defect `CONSTITUTION.MD` VI.2
has carried as *"t933 row-height distribution"* since check #82, and t989's table battery is the first
fixture to put numbers on it.

```text
                                                        Chrome     before     after
  rowspan=2 60px over two 24px rows          row 1       [30]       [24]      [30]
                                             row 2       [30]       [36]      [30]
  rowspan=2 100px over rows of 40 and 24     row 1       [63]       [40]      [63]
                                             row 2       [38]       [84]      [38]
  rowspan=3 90px over three 24px rows        each        [30]    [24/24/42]   [30]
 ── controls ──
  rowspan=2 30px over rows of 40 and 24 (cell SHORTER)  [40]/[24]      unchanged
  the same two rows with NO rowspan at all              [40]/[24]      unchanged
```

### Proportional, not even — and only one row in the fixture can tell

36px of excess over rows of 40 and 24 splits **22.5 / 13.5** → 63 / 38. Even distribution gives
58 / 42. Every other row in the fixture has *equal* natural heights, where the two rules **degenerate
to the same answer** — so the unequal-rows case is the entire discriminator, and a fixture built from
equal rows would have shipped either rule. Even distribution survives only as the fallback for rows
that are all zero-height, where "proportional" has no meaning and the divisor would be zero.

> The same lesson as t990's sibling ticks, in a third costume: **the rows that discriminate are rarely
> the rows that made you look.** The battery that found this used equal rows.

### Why it is not a one-cell error

Every spanned row grows, so **everything inside the other cells of those rows moves too**. A rowspan
in a real table — an invoice's line-item block, a schedule's merged slot, a spec table's grouped
column — displaced its whole neighbourhood, and the further down the table it sat the more it moved.

Gated by `G_ROWSPAN_DISTRIBUTION`. RED-proven three ways, each hitting exactly its own rows: the
original dumps 24/36 on the equal case; an even split fails **only** the unequal case at 58/42;
dropping the `deficit > 0` guard shrinks the rows a shorter rowspan cell has no right to shrink
(18.75/11.25).

⚠ Still open from the same battery: `<caption>` reserves no space and does not widen the table, and a
`<thead>` written after a `<tbody>` renders in source order instead of first.

## `<tfoot>` written first rendered first, and the UA sheet is where we lost the distinction (t991)

CSS Tables lays row groups out **header → body → footer, regardless of source order**. Our UA sheet
said:

```css
thead, tbody, tfoot { display: table-row-group; }
```

— one value for three groups, which **discards the only thing that distinguishes them**. So
`<tfoot>` written before `<tbody>` rendered at the *top* of the table.

That idiom is not exotic: putting `<tfoot>` before `<tbody>` is the classic HTML4 pattern — it exists
so a long table's footer reaches the parser before its thousand body rows — and it is still
everywhere in legacy markup, invoice templates and report generators. **A totals row at the top of an
invoice is not a geometry error, it is a reading-order one**: every number present, correctly sized,
and meaning something else. That is the I3 class, not the shape class.

```text
                                                    Chrome     before      after
  <tfoot> before <tbody>           foot / body       [24/0]     [0/24]     [24/0]
  <thead> after  <tbody>           head / body       [0/24]     [24/0]     [0/24]
  all three scrambled              head/body/foot  [0/24/48] [48/24/0]  [0/24/48]
 ── controls ──
  the usual thead/tbody/tfoot order                 [0/24/48]         unchanged
  TWO <tbody>s keep their source order                 [0/24]         unchanged
```

### The fix is not where the symptom is

The layout code walked the DOM in order, and adding a rank there is the obvious fix — but it **could
not work**, because every group arrived carrying the same `display` value. The distinction is made in
the UA sheet, and Chrome makes it there too. Two new `Display` variants (`TableHeaderGroup`,
`TableFooterGroup`) exist so the value can *survive* the cascade; the layout rank reads them.

> The symptom was in layout. The lost information was three layers up, in a stylesheet, in a rule
> that looked like a tidy abbreviation of three identical declarations. **A fold that discards a
> distinction reads as a simplification right up until something needs the distinction.**

The two RED recipes prove the pipe from both ends: restoring the folded UA rule fails the three
scrambled rows *with the layout rank still in place* (the layout fix alone is inert), and dropping the
sort fails the same rows *with the UA distinction still in place*.

### Why the sort must be stable

Groups of the same kind keep their document order — two `<tbody>`s are ordered by the source. A
stable sort on the rank preserves that, and the two-`<tbody>` control is the only row that can see it.
⚠ Swapping in `sort_unstable_by_key` does **not** go red today (two elements cannot differ); the
control exists for a future three-bucket rewrite, and that is recorded rather than claimed as a proof.

Gated by `G_ROW_GROUP_ORDER`.

## `<caption>` — the box we rendered as NOTHING, and the width interaction that is not obvious (t992)

`<caption>` was skipped alongside the column groups in `collect_table_rows` and never laid out.
**Three defects in one dropped child**, and they are not the same kind: the text did not appear (a
MISSING_BOX, not a geometry error), the rows did not move down for it, and the table did not widen
for it.

```text
                                                    Chrome     before      after
  caption 20px tall, one-cell table  caption box    [0, h20]   ABSENT    [0, h20]
                                     the cell       [y=20]     [y= 0]    [y=20]
  caption `a very long caption`      table width    [   67]    [   10]   [   67]
                                     the cell       [y=72]     [y= 0]    [y=72]
  caption written AFTER the rows     caption box    [y= 0]     ABSENT    [y= 0]
 ── control: no caption                              unchanged
```

### A caption WIDENS its table — to its MIN-content width

The table's used width is at least the caption's **min-content** width, and the surplus is
distributed over the columns exactly as a rowspan's surplus is distributed over its rows (t990), on
the other axis. Chrome-measured: a one-cell table whose cell holds `x` (10px) with the caption *"a
very long caption"* comes out **67** wide — the longest word — and the column takes the extra.

**Min-content and not max-content, because the caption may wrap.** At 67 that caption is three lines
tall and Chrome keeps it three lines rather than widening the table to fit it on one. A max-content
floor gives 183 and a one-line caption: a wrong answer of the right shape, and **the row that
distinguishes them is the caption's HEIGHT, not its width.**

### The caption is FIRST among the table's children

It paints above the rows and, more importantly, **reads first in the semantic order the agent surface
walks**. A caption is a table's accessible name; emitting it after the rows puts the label after the
data for every consumer of the a11y tree — the I3 surface, not the paint one. ⚠ Reordering it does
**not** fail this gate, whose assertions are geometric; recorded as a NON-red because the gate that
could see it is an a11y-order gate.

⚠ **`caption-side: bottom` is measured and NOT built** — a 20px caption under a 30px row belongs at
y=30 — because there is no `caption_side` field on `ComputedStyle`. The t985/t987 "nowhere to live"
shape again, and a cascade addition rather than a layout one.

Gated by `G_TABLE_CAPTION`. **With this, the whole sixteen-row table battery of t989 is Chrome-exact:
four mechanisms found, four closed** (cell `vertical-align` t989 · rowspan distribution t990 ·
row-group order t991 · caption t992).

## The borders/backgrounds battery — 14 of 16 exact, and two defects with their rules derived (t993)

The last family on audit #39's steer list. Sixteen rows, every one chosen so that a *geometry* reading
can see it — a paint-only difference is invisible to `boxes` and is deliberately out of scope (we have
no raster diff, which audit #38 already named as the reason `clip-path` was untestable).

```text
  EXACT (14): border-width 4-value shorthand · `border:10px` with no style (NO border) ·
              `border:10px none` · border-radius (no geometry) · `border:10%` INVALID ·
              outline takes no space · a cell border in the SEPARATE model ·
              background-clip/origin are paint-only · border-inline logical shorthand ·
              no-border control · border-box (the border eats the CONTENT, inner child at 10,10) ·
              content-box (the border grows the box) · border-image (no geometry) ·
              an inline border ADVANCES THE PEN correctly
  DIVERGING (2)
```

### 1. `border-collapse: collapse` — the rule, derived

Every CSS reset and nearly every real data table sets this. **Each edge's collapsed width is the
`max` of the borders that meet at it, and each cell's border box extends HALF of that on each side.**
Chrome-measured, and the unequal case is what proves it is `max`-then-halve rather than anything else:

```text
   two cells, uniform 10px      cell 1 [5,5,20,34]    cell 2 [25,5,20,34]
     ours                              [0,0,30,44]           [30,0,30,44]
   4px cell beside a 20px cell  cell 1 [2,10,22,44]   cell 2 [24,10,30,44]
     -> shared edge = max(4,20) = 20, half = 10; cell 1 = 2 + 10 + 10 = 22 wide
   table border 10 + cell border 2     cell   [5,5,20,34]   outer edge = max(10,2) = 10
   CONTROL, the separate model         cell 1 [2,2,30,44]   cell 2 [34,2,30,44]   ours EXACT
```

We give every cell its full border on all four sides and share nothing, so a collapsed table is
`(n+1) × border` too wide and every column after the first is displaced cumulatively. ⚠ The full
algorithm also has *conflict resolution* (style priority, cell vs row vs table) that these rows do not
exercise; the geometry half above is what they pin. ⚠ Chrome's own collapsed-table *outer* width has a
1px quirk (49 where 50 is the arithmetic, 63 where 64 is) — a gate should assert the CELLS, not the
table box.

### 2. An inline with a frame on the INLINE AXIS ONLY takes the line box vertically

t851 established that a non-replaced inline's box is its own content area, resolved **per axis**, and
that holds — until the inline carries a horizontal-only frame:

```text
                                       Chrome              ours
  <span>y</span>                       [10, 2, 10, 19]    [10, 2, 10, 19]   ✓ (t851)
  <span style=background>              [10, 2, 10, 19]    [10, 2, 10, 19]   ✓
  <span style=border-left:12px>        [10, 2, 22, 19]    [10, 0, 22, 21]   ✗
  <span style=padding-left:12px>       [10, 2, 22, 19]    [10, 0, 22, 21]   ✗
  <span style=border:12px>             [10,-10, 34, 43]   [10,-10, 34, 43]  ✓
```

**The inline axis is right in every row** — the frame advances the pen correctly. Only the block axis
goes wrong, and only when the *vertical* frame is zero: the box top becomes the line box's top rather
than the content area's, 2px of half-leading high, and the height 2px over. An all-sides border is
correct, which is what makes this a narrow path rather than the general case — and `<code>`, `<kbd>`,
padded `<a>` chips and syntax-highlighted spans are all exactly the horizontal-only shape.

Both are recorded with numbers and fixtures (`/tmp/bc.html`, `/tmp/ib.html`) rather than built:
collapse is a multi-tick algorithm, and the inline one sits in the hot path of every page's line
layout. **Naming them precisely is the deliverable of a measurement tick; guessing at either is what
the ratchet forbids.**

## The form-controls / replaced-elements battery — the highest-weight area in the corpus (t995)

`<button>` is on **55.6%** of the corpus, `<input>` **51.5%**, `<svg>` **34.5%** — by frequency this is
the most valuable area there is, and no battery had covered it. Nineteen rows.

```text
  EXACT (10): <button> unstyled · <button> with w/h · <button> label centring · <button> as a
              FLEX ITEM · inline <svg> with w/h attrs · <svg> with only a viewBox (a RATIO, never
              a size — it fills, 400×200) · <img> with w/h attrs and no src · <label> as an
              ordinary inline · an inline-block control · the <button>'s inner label
  DIVERGING (9, in five mechanisms)
```

**`<button>` is exact on all four of its rows** — t972's 2px-outset UA border landed and holds,
including as a flex item and for its label's centring. The divergences are everything else.

```text
                                    Chrome              ours
  <input> unstyled              [0, 3, 201, 19]    [0, 0, 201, 17]
  <input size=5>                [0, 3,  81, 19]    [0, 0,  81, 17]
  <input> border-box width:100  [0, 3, 100, 19]    [0, 0, 100, 17]
  checkbox                      [0, 4,  13, 13]    [0, 2,  15, 15]
  radio                         [0, 4,  13, 13]    [0, 2,  15, 15]
  <select> unstyled             [0, 3,  52, 19]    [0, 4,  49, 17]
  <textarea> unstyled           [0, 0, 178, 32]    [0, 0, 178, 34]
  <textarea rows=2 cols=10>     [0, 0,  98, 32]    [0, 0,  98, 34]
  <fieldset>                    [0, 0, 400, 50]    [0, 0, 400, 24]
```

⚠ **`<input>`'s WIDTH is exact on all three rows and only its HEIGHT is short** (17 against 19), which
rules out a symmetric border error — the shape t972 fixed on `<button>`. Whatever this is, it is not
that, and assuming it was would have been the obvious wrong move.

### Chrome's UA numbers, read rather than inferred

Expensive to obtain and worth keeping, from `getComputedStyle` on each unstyled control:

```text
            font                 border          padding      box-sizing  computed h/w
  input     13.333px Arial       2px inset       1px 2px      content     15 / 197
  select    13.333px Arial       1px solid       0            border      19 / 37
  textarea  13.333px monospace   1px solid       2px 2px      content     30 / 176
  checkbox  13.333px Arial       0 none          0            border      13 / 13
  button    13.333px Arial       2px outset      1px 6px      border      21 / 33.78
  fieldset  16px monospace       2px groove      5.6px 12px   content     46 / (fills)
  legend    16px monospace       0 none          0 2px        content     24 / 9.64
```

⚠⚠ **These computed values do NOT reconcile arithmetically with the measured boxes**, and that is
itself the finding: `<input>` computes `content 15 + padding 2 + border 4 = 21` and *measures* 19;
its width computes `197 + 4 + 4 = 205` and measures 201. **A native control's USED border is the
platform theme's, not the UA sheet's** — so a fix driven from `getComputedStyle` alone would be built
on numbers Chrome itself does not lay out with. The measured boxes above are the ground truth; the UA
table is context, not a specification.

### The five mechanisms, separated

1. **`<input>` height and baseline** — 2px short, 3px high, on all three rows including `border-box`.
2. **checkbox / radio** — we draw 15×15, Chrome 13×13, and ours sits 2px higher.
3. **`<select>`** — 3px narrow *and* 2px short, and the only row where our y is *lower* than Chrome's.
4. **`<textarea>`** — 2px too tall, on both the default and the `rows`/`cols` row, with width exact.
5. **`<fieldset>`** — 24 against 50: we render it as a plain block with **no UA border or padding at
   all**, and its `<legend>` (which sits *on* the border) is a second mechanism inside the first.

Of these, only `<fieldset>` is plainly a UA-sheet gap of the kind t991 fixed; the other four are
native-control metrics where the used values are theme-derived. **Recorded with numbers rather than
guessed at** — see the tick-995 journal entry for why that is the deliverable.

### `<fieldset>`'s UA border cannot land without the `<legend>` rule (t996, refused)

The fieldset row of t995's battery looked like the one clean UA-sheet gap in it, and it is — but
building it in isolation is a **trade**, and the ratchet refuses trades. Recorded so it is not
re-attempted from scratch.

`<fieldset>` carried `display: block` and nothing else: no border, no padding, no margin. Adding
Chrome's UA rules —

```css
fieldset { margin: 0 2px; border: 2px groove; padding: 0.35em 0.75em 0.625em; min-width: min-content }
legend  { display: block; padding-inline: 2px }
```

— measured as follows (author `*{margin:0;padding:0}` applied, so only the border is in play):

```text
                                     Chrome        before      with the UA rules
  fieldset, NO legend                [400, 14]    [400, 10]      [400, 14]   FIXED
    its child                        [2, 2]       [0, 0]         [2, 2]      FIXED
  authored border:5px                [400, 20]    [400, 20]      [400, 20]   unchanged
  fieldset WITH a legend             [400, 36]    [400, 34]      [400, 38]
    the <legend> box                 [2, 0, 10, 24] [_, 2, _, 19] [_, 2, _, 24]  better
    the content below it             [2, 24]      [2, 24]        [2, 26]     WORSE
```

**The last row is why it was reverted.** Chrome lets a `<legend>` *replace* the top border it sits on:
the legend is at y=0 and the content starts immediately below the legend, not below border+legend. We
have no such rule, so adding the border pushes the content down by exactly the border width — and the
content position had been **right by accident**, because *no border* and *no legend rule* were two
errors that cancelled.

> A fix that corrects one row by 4px and breaks another by 2px is not a partial win; it is a trade.
> **The fieldset border and the legend-replaces-the-top-border rule are one tick, not two.**

⚠ A second finding from the attempt, worth more than the attempt: **`margin-inline: 2px` in the UA
sheet was NOT reset by the author's `* { margin: 0 }`**, while Chrome's is. The logical and physical
shorthands did not resolve as one property group in the cascade, so the UA's logical margin survived
an author reset that should have removed it. Written physically (`margin: 0 2px`) it behaves. That is
a cascade defect with a far wider blast radius than fieldsets, and it is worth its own probe.

## The collapsing border model, and why its conflict resolution is geometrically inert

`border-collapse: collapse` is in the CSS of **57.3% of the CrUX corpus** (204 of 356 sites, fetched
with their linked stylesheets) — every reset and every framework sets it. It is *inert* without a
table, though, and only **5.6%** (20 of 356) have both, which is the honest population. Quoting the
57.3% would be the same error as pricing a property by how often it is *declared* rather than how
often it *applies*.

**The model. Every clause was measured against
`google-chrome-stable --headless=new --hide-scrollbars --window-size=1200,800` over 15 tables, and
every clause has a row that fails without it.**

1. Each **grid line** — `ncols + 1` vertical and `nrows + 1` horizontal — carries ONE width: the
   `max` of every border that meets it. The cells on either side contribute, and so does the table's
   own border at the two outer lines.
2. Each side of a line takes **half**, kept fractional. A 1px line puts the cell at `x = 0.5` with
   width 11; rounding the half per side gives 10 or 12 and Chrome gives 11.
3. The table's **`padding` is ignored** (CSS 2.1 §17.6.2 — and measured: `padding: 30px` on a
   collapsed table is byte-identical to none). A **cell's** padding is kept.

```text
                                                       Chrome         ours before
   two cells, uniform 10px                a1          [ 5,  5,20,30]  [ 0,  0,30,40]
                                          the table   [ 0,  0,50,40]  [ 0,  0,60,40]
   a 4px cell beside a 20px one           a2          [ 2, 50,22,40]  [ 0, 40,18,60]
   table border 10 + cells 2              a3          [ 5,105,16,30]  [10,110,14,24]
   1px borders (odd halves)               a15         [ 1,299,11,21]  [ 0,362,12,22]
```

The **unequal** row is what proves it is max-then-halve and not anything else: `max(4, 20) = 20`,
half 10, so cell 1 is `2 + 10 + 10 = 22`. The **table-border** row is what proves the table is just
another participant in the outer line: `max(10, 2) = 10`, the table takes the outer half and the
*cell* takes the inner one, so the cell starts 5 in and not 10.

### A grid line is PER-LINE, not per-segment — and one row of fifteen can tell

Give the middle vertical line 20px in row 1 and 2px in row 2. Chrome gives **both** rows the 20px
line: row 2's cell is inset 10 though its own border is 2, because a column has to be rectangular.

> A per-segment reading — collapse each cell against its immediate neighbour — passes every uniform
> table and gets wrong every real table with a heavier header row. **A fixture of uniform tables
> cannot contain the row that decides the rule**, so it has to be written on purpose.

### Conflict resolution cannot move a box, and that is the whole reason this is small

CSS 2.1 §17.6.2.1 resolves a collapsing conflict in this order:

```text
   hidden  ->  WIDER  ->  style priority (double > solid > dashed > dotted > ...)  ->  origin
```

**Width is consulted before style.** So style priority can only ever break a *width tie* — and two
borders of the same width occupy the same space whichever one wins. Measured: 2px `solid` against
6px `double` lays out with the 6px geometry, with no style-priority rule implemented at all.

> When a spec orders its tie-breakers, ask which term the OUTPUT you care about is a function of.
> The "other half" of this algorithm is a function of *style*; geometry is a function of *width*.

**The one exception is `hidden`**, which must force its line to zero even against a wider neighbour:
a 10px solid cell beside a 10px `hidden` one is **15 wide, not 20**. `manuk_css::BorderStyle` has no
`Hidden` variant and stores one style for all four sides, so honouring it is a cascade change rather
than a layout one. It is 0.8% of the corpus by a deliberately generous grep. `none` needs no rule at
all — its computed width is already 0, so it loses the `max` on its own.

## A float's fit test is against its containing block, and the mirror is what names the branch

`FloatContext`'s `left_edge`/`right_edge` belong to the nearest **block formatting context**. Floats
correctly share one across nested plain blocks — that is what makes a float escape its parent. But
CSS 2.1 §9.5.1 rules 1 and 2 pin a float to **its own containing block**, and those are different
boxes for every nested plain `<div>`, which is to say almost every element on the web.

`place()`'s *position* expressions already used `cb_left`/`cb_right`. The test for whether the float
**fits** still called `available()`, which folds the context's edges in. Result: four 50px left
floats fill a 200px block, and the fifth was asked *"do you fit in 1200?"* and placed at **x = 200**,
outside its own container — then the sixth at 250, the seventh at 300.

```text
   five 50px float:left in a 200px block          Chrome      before      after
     overflow: hidden   — a BFC                  [  0, 30]   [  0, 30]   [  0, 30]
     overflow: visible  — NOT a BFC              [  0, 30]   [200,  0]   [  0, 30]
     ...one plain block deeper, still not        [  0, 30]   [200,  0]   [  0, 30]
     the same non-BFC block, RIGHT floats        [150, 30]   [150, 30]   [150, 30]
```

### The mirror row is the finding

**Right floats wrapped correctly in the very same non-BFC container the whole time.** A right float
is placed at `cb_right - w`, which lands it *inside* the container, so `right_offset` picks it up and
collapses the available width to zero on its own — the containing block's edge reached that test *by
accident*, and only on one side. A left float has nothing on its right to bound it, so the context's
far edge stood in.

> Two diagnoses fit the left-float rows perfectly and are both wrong: *"we do not wrap floats"* and
> *"a non-BFC block loses its width"*. The right-float row kills both in one line.
>
> **When a defect has an obvious mirror — left/right, top/bottom, row/column — measure the mirror
> before writing the fix. If the mirror passes, what you have is a branch, not the algorithm.**

### A float fixture whose rows are not isolated is measuring the wrapper

The first 20-row battery used plain `<div>` wrappers, so every float escaped into the body's context
and polluted every row after it: 13 of 62 rows "diverged" and the diff was unreadable. Making each
wrapper a BFC gave **74/74 exact** — which is the result that proves the other 22 float behaviours
are already Chrome-correct. The defect then had to be found by going *back* to the leaky version and
asking why the two engines leaked differently.

**Both versions were needed.** The isolated one is the battery; the leaky one is the discovery.

## A self-collapsing box, and the two shortcuts that fail in opposite directions

CSS 2.1 §8.3.1 has three margin collapses. Parent↔first-child and parent↔last-child were built here
long ago. The third — a box collapsing **through itself** — was not, so every empty block contributed
two margins where Chrome contributes one, and pushed everything after it down.

```text
   <div>x</div>   <div style="margin:10px 0 30px"></div>   <div>R</div>

                                     Chrome   before   after
     the empty box's own edge          30       30       30     <- never moved
     the block after it                50       60       50
```

Only what comes *after* an empty box changes. The box's own border edge is still placed after its
(collapsed) top margin — so the fix rewrites the two values the parent stacks with, and leaves the
box alone: `flow_bottom` goes back to the position the caller handed in, and the whole run of margins
becomes one.

### One ratio cannot distinguish three rules

| rule | `10 / 30` (Chrome 30) | `40 / 5` (Chrome 40) |
|---|---|---|
| collapse (`max`) | 30 ✓ | 40 ✓ |
| "only the bottom margin" | 30 ✓ | 5 ✗ |
| "only the top margin" | 10 ✗ | 40 ✓ |

**Both ratios have to be in the fixture**, or two of the three rules survive it. And it is
`collapse_margins`, not `max`: two negatives take the **min** (`-10 / -30` measures **-30**, where
`max` gives -10 and the sum gives -40).

### The recursive clause, and why the flat approximations come in mirror pairs

> *"...and it does not contain a line box, and **all of its in-flow children's margins (if any)
> collapse**."*

```text
   shortcut 1  "no in-flow children"   -> an empty block wrapping an empty block
                                          Chrome 50,  shortcut 60        WRONG
   shortcut 2  "contains no line box"  -> `height: 0` wrapping a block WITH TEXT
                                          Chrome 60,  shortcut 30        WRONG
```

Shortcut 1 was implemented first and the fixture caught it at 14/15. Shortcut 2 is what came to mind
next, and it would have been **a different bug of the same size** — the row that kills it exists only
because shortcut 1 had already been wrong once.

> **When a spec clause is recursive, its flat approximations come in mirror-image pairs, and each one
> passes the rows that made you look.** Reaching for the second approximation after the first fails
> feels like converging and is sampling. Two rows that fail opposite shortcuts are a proof: any rule
> passing both is doing the recursion.

### The clauses, each with a measured row

```text
   collapses through:  plain empty · height:0 · only child is a FLOAT · only child is abspos ·
                       only child is another EMPTY block · three deep · height:0 around an empty
                       block · two empty SIBLINGS · whitespace-only text · clear:both with no float
   does NOT:           border 1px · padding-top 1px · min-height 1px · overflow:hidden ·
                       display:flow-root · height:20px · has text · height:0 around real text
```

`overflow: hidden` and `display: flow-root` are what stop a height check from standing in for the
concept — both produce a **zero-height** box that must not collapse. The whitespace row decides
whether the fix works on real markup at all: pretty-printed HTML puts a newline inside every "empty"
element, and counting that as content makes the whole thing inert.

## Rule 6 is a bound, not a placement

CSS 2.1 §9.5, float placement rule 6:

> *"The outer top of a floating box may not be higher than the top of any line-box containing a box
> generated by an element earlier in the source document."*

A float written after text on a line therefore belongs at that line's **top**. We flushed the pending
inline run — which advances past the whole run — and then placed the float below it, so every such
float came out **one line-height too low** and dragged its entire exclusion band with it.
`<img class="alignright">` inside a paragraph is exactly this shape.

```text
                                                       Chrome      before      after
   the float FIRST, then text                         [  0,  0]   [  0,  0]   [  0,  0]
   text, then the float (one line)                    [  0,  0]   [  0, 20]   [  0,  0]
   a <p> BLOCK, then the float                        [  0, 52]   [  0, 52]   [  0, 52]
   text wrapping to TWO lines, then the float         [  0, 20]   [  0, 40]   [  0, 20]
   line-height: 40px, text, then the float            [  0,  0]   [  0, 40]   [  0,  0]
   text, then a float TOO WIDE for the line           [  0, 40]   [  0, 40]   [  0, 40]
```

### The last row is the finding, and the spec sentence does not mention it

Implementing the sentence alone — a top-bound and nothing else — goes **6/10 → 9/10** and moves the
last row from *right* to **wrong**. A 280px float after text in a 300px block goes on the **next**
line in Chrome.

`FloatContext::place` cannot decide this: it scans bands of **floats**, and what is in the way here
is the line's own already-placed **inline content**, which is not a float and is not in the context.
So the line's remaining width is measured from the fragments, and a float too wide for it starts
below the run exactly as it used to.

> **A rule quoted from the spec is half a rule until the row where it must NOT apply is written.**
> Rule 6 is a *bound*; implementing it as a *placement* is invisible in every row where the float
> fits — nine of ten, and approximately all of the real web — until the one where it does not.

### Where the decision lives is a real choice

The fit test needs the float's resolved **margin-box** width, and that resolution (shrink-to-fit,
`box-sizing`, aspect-ratio, `min`/`max`-width) is already a *second* hand-rolled copy of
`layout_block`'s. A third copy at the call site would drift from both the first time either gained a
rule — so `layout_float` receives the line as a **candidate** and decides once `width` is final.
`clear_to` is re-applied to the candidate: joining a line must never lift a float above something it
was told to clear.

### The two rows that stop it degenerating

A *block* before the float means there is no pending line, so the float stays where it was. And when
a real float already occupies `x = 0..40` on that line, the second float goes **beside** it rather
than below. Without those, *"place every float at the top of its container"* passes everything else:
the new top is a **starting point for the band search, not a replacement for it**.

## The matrix reaches the subtree it was applied to, and an abspos child is not in it

`layout_block` bakes a `transform` into the box's own fragment tree — `boxx.transform_affine(&m)`
walks the box and everything under it, so an in-flow or `position: relative` child of a
`transform: scale(2)` container comes out exactly right, at double the offset and double the size.
An **absolutely positioned** child is not under it. It is laid out later, in the global positioned
pass, against a containing-block rect read out of the *already transformed* fragment tree — and
`left: 20px` still means twenty **untransformed** pixels.

So the ancestor's transform reached an abspos descendant as a **displacement of the containing
block's origin, and nothing else**:

```text
   a 40x20 box at left:20px top:10px, in a 200x60 container         Chrome         before
     container transform: translateX(25px)                        [ 45, 280]     [ 45, 280]   ✓
     container transform: scale(2), origin 0 0                    [ 40, 560]     [ 20, 550]
                                                                   80x40          40x20
     container transform: rotate(90deg), origin 0 0               [-30, 830]     [-40, 820]
     an IN-FLOW child of that same scaled container               [ 40, 630]     [ 40, 630]   ✓
     a RELATIVE child of that same scaled container               [ 40, 740]     [ 40, 740]   ✓
```

**A pure translation came out right by accident** — displacing the containing block's origin *is*
what a translation does to its children — which is why the defect survived every fixture that
reached for `translateX` first. And the in-flow / relative rows are what make this a
**containing-block** defect rather than a transform defect: the same container, the same matrix,
correct for every child that happens to live inside the subtree the matrix was applied to.

### The fix, and why it needs a second map rather than an inverse

Two things are recorded the moment a transform is applied, before it is baked in:

- **the matrix**, keyed by the transformed element. Composed outermost-first up the DOM it is the
  map from the innermost untransformed space to the page. Outermost-first because layout unwinds
  bottom-up: an inner element's matrix is expressed in its *parent's* untransformed space.
- **the pre-transform border box** of every box in the subtree that could be a containing block
  (`position != static`, or a grouping property). First write wins, and inner transforms are applied
  first, so the entry is always innermost-local — the space the chain above is expressed in.

The positioned pass then resolves the containing block in that pre-transform space, lays the box out
there, and applies the chain to the finished box. The static position needs no adjustment and that
is not a coincidence: it is recorded during flow, *before* the transform is baked in, so it was
already in pre-transform space and was previously being combined with a post-transform containing
block.

**Why not invert the stored rect instead of recording a second map.** The containing block is often a
`position: relative` element *inside* the transformed box rather than the transformed box itself, so
its rect must be recovered too — and a stored rect is an **axis-aligned bounding box**. Inverting it
is exact for translate and scale and silently inflates for a rotation, which is the failure that
would only show up on the sites that rotate.

**The reach.** `position: absolute` is on 76% of the burndown's corpus and `transform:` on 65.5%, and
the intersection is the standard idiom, not an edge case: a hover-zoom card with an absolutely-placed
badge, a carousel track with positioned arrows, a modal centred with `translate(-50%,-50%)` carrying
a positioned close button. The error is unbounded — the child keeps its untransformed size, so it
both moves and mis-sizes — which is an `overlap` and `reading_order` divergence rather than a small
`dy`.

## `translate` / `rotate` / `scale` are PROPERTIES, and the order is the spec's not the author's

CSS Transforms 2 splits the three commonest transform functions into properties of their own. The
point is composability: `el.style.translate = '30px 15px'` leaves a `rotate` the stylesheet set
alone, where `el.style.transform = 'translate(30px,15px)'` destroys it. That is why every animation
library now writes them, and why they show up in **19.3%** of the burndown corpus (171 sites fetched
with their linked stylesheets: `rotate:` 12.9%, `scale:` 8.8%, `translate:` 3.5%).

Both of this engine's cascades matched only `"transform"`. The three names appeared *only* as
**values** inside `will-change`, which is what made the gap read as handled. So the properties were
**absent** rather than wrong, and the element sat at its untransformed position and its
untransformed size — the largest error a transform property can produce.

### The two shorthand rules are opposite, and one fixture cannot tell them apart

```text
   translate: 30px     ->  translate(30px, 0)    y stays 0
   scale: 2            ->  scale(2, 2)           UNIFORM
```

Either rule, applied to both, passes half the rows. A percentage `translate` resolves against the
element's **own border box**, exactly as the function does: `translate: 50% 100%` on a 40×20 box is
`(20px, 20px)`.

### The composition order is fixed, whatever order the declarations came in

§3: the matrix is **`translate`, then `rotate`, then `scale`, then the `transform` list**. So
`translate:30px 0; rotate:90deg` and `rotate:90deg; translate:30px 0` are the *same* transform, and
Chrome gives both `(30,10) 20x40`. That single rule is the reason these are four fields composed at
use (`ComputedStyle::effective_transform`) rather than one `Vec` appended to at parse time: a Vec
built during the cascade can only ever record declaration order.

`effective_transform` returns `Cow::Borrowed(&self.transform)` when none of the three is set, so the
overwhelming majority of boxes allocate nothing.

### ⚠ A rotation about x or y is NOT beyond a 2D pipeline — measured

`g_transform_3d.rs` carries the claim that a rotation about the x or y axis *"foreshortens, which a
2D pipeline cannot express, and inventing one would be a wrong answer of the right type."* With no
`perspective` in force that is measurably false: the projection is exactly a scale on the other
axis, and Chrome reports it in `getBoundingClientRect`.

```text
   a 40x20 box                        Chrome            ours
     rotate: x 45deg                40   x 14.14      40 x 20      (14.14 = 20 * cos45)
     rotate: y 45deg                28.28 x 20        40 x 20      (28.28 = 40 * cos45)
```

Banked, not built: it is a different rule from the properties above — it belongs equally to
`rotateX()`, `rotateY()` and `rotate3d()` — and two RED proofs in one tick make neither of them a
proof.

## A rotation about x or y is a SCALE on the other axis, and a gate asserted otherwise for 150 ticks

`stylo_map.rs` and `parse_transform` both carried this note, and both dropped every such rotation:

> *"`rotate3d` is taken **only about the z axis** — a rotation about x or y foreshortens, which a 2D
> pipeline cannot express, and inventing one would be a wrong answer of the right type."*

With no `perspective` in force that is measurably false. The orthographic projection of a rotation
about x or y **is exactly** a scale by `cos θ` on the perpendicular axis, and Chrome reports it in
`getBoundingClientRect`:

```text
   a 100x40 box                     Chrome            before          cos θ x the axis
     rotateX(45deg)               100 x 28.28       100 x 40        40 x cos45  = 28.28
     rotateY(45deg)                70.71 x 40       100 x 40       100 x cos45  = 70.71
     rotate3d(0,1,0,60deg)          50 x 40         100 x 40       100 x cos60  = 50
     rotateX(90deg)               100 x 0           100 x 40        40 x cos90  = 0
     rotateX(120deg)              100 x 20 (y=-20)  100 x 40        40 x cos120 = -20
   ── still excluded, and now the exclusion is MEASURED ──
     rotate3d(1,1,0,45deg)         91.21 x 48.79    100 x 40        not a scale on either axis
```

**`rotateX(120deg)` is the row that makes the rule precise.** Past 90° `cos` is negative, the box
flips through its origin, and Chrome reports the flipped position — which `Scale(1, cos θ)` gives for
free, because a box's rect comes from mapping its corners. A rule written as `abs(cos θ)` passes
every row under 90° and is wrong above it.

### ⚠⚠⚠ The gate asserted the reasoned number, so fixing the bug looked like a regression

`g_transform_3d.rs` — the gate written to kill exactly this class of `_ => {}` omission — asserted
`#y08` (`rotate3d(1,0,0,45deg)`) at **100 x 40** and said in its own message *"Chrome leaves the box
100 x 40 in this 2D projection."* Nobody had asked Chrome. The number came from the same reasoning as
the arm the gate was written to kill.

> **A gate whose reference value is reasoned rather than measured does not merely fail to catch the
> bug. It pins the engine to it, and turns the fix into a red wall.**

That is the generalisable half of this tick, and it is why every row added here carries the headless
Chrome measurement that produced it — including the exclusion row, which asserts that a genuinely
mixed axis is left alone *because 91.21 × 48.79 is not a scale on either axis*, not because a comment
says 3D is hard.

## A non-visible overflow zeroes the automatic minimum size — the flex web's escape hatch

CSS Box Sizing §5.1 and Flexbox §4.5: `min-width: auto` on a flex or grid item resolves to the
item's **min-content size** — but only while that item's overflow **in that axis** is `visible`. A
non-visible overflow (`hidden`, `scroll`, `auto`, `clip`) resolves it to **zero**.

That exception is not a corner: it is *the* reason `.item { overflow: hidden }` is the canonical fix
for "my flex row will not shrink", and it appears in every truncating sidebar, breadcrumb trail, chat
list and table-shaped flex row on the web. Measured against Chrome, a 200px flex row whose only item
holds a 337px `white-space: nowrap` string:

```text
                                          Chrome    before    after
   flex item, nowrap                      337.16     337       337     <- CONTROL: must NOT shrink
   flex item, nowrap, min-width: 0        200        200       200     <- CONTROL: already right
   flex item, nowrap, overflow: hidden    200        337       200
```

⚠ **`min-width: 0` was already correct, and that mirror is what makes this a branch rather than the
algorithm.** The author who writes the explicit zero has always been served; the author who writes
`overflow: hidden` was not — and that is the more common of the two. Priced on the burndown corpus
with linked stylesheets: **69.0%** of sites declare both `display:flex` and an `overflow:hidden`,
against **46.2%** for `min-width:0`.

The failure it produces is `h_overflow`, one of the four jarring dimensions M1 is a conjunction of
and non-clean on 40 of 123 scored sites — an item that refuses to shrink pushes its row past the
container and everything downstream of it sideways.

**Applied per axis and to every box, not only to flex/grid items.** `overflow-x` governs the inline
minimum and `overflow-y` the block one, because the property is per-axis. Applying it everywhere is
safe rather than sloppy: a block box's automatic minimum is already zero, so the rule can only ever
agree with it — and scoping it to "items" would need the parent's display, which is not available
where the style is built.

### ⚠ Banked, measured and NOT built: `overflow-wrap: break-word` must not reduce min-content

The same battery found a second rule, in the same function's neighbourhood, and it is a real
distinction the spec draws:

```text
   a 30-character unbreakable token in a 200px flex row      Chrome     ours
     (no property)                                          288.98     289      ✓
     word-break: break-all                                  200        200      ✓
     overflow-wrap: anywhere                                200        200      ✓
     overflow-wrap: break-word                              288.98     200      ✗
```

`anywhere` **does** affect the intrinsic min-content size; `break-word` **does not** — it only
permits the break when the line is actually being laid out and would otherwise overflow. Priced at
**48.5%** of the corpus (`overflow-wrap|word-wrap: break-word`) against 11.7% for `anywhere`.

**BUILT at t1015.** The distinction is one condition, and the place to put it is the one the engine
already had: `Ctx::intrinsic_probe` is set for the duration of a min-content/max-content measurement,
so `break-word` grants a break opportunity **only while that flag is false**. `anywhere` and
`word-break: break-all` grant it unconditionally.

⚠ **We were wrong in the SHRINKING direction, which is the quiet one.** The item came out *narrower*
than Chrome's, so an overflow the author deliberately kept was hidden rather than shown — a
divergence that makes a page look tidier than the reference and is therefore the kind an eyeball
review never flags.

## The dimension attribute is a presentational HINT, and ours sat above the author's cascade (t1025)

`<img width="100" height="40" style="width:100%;height:auto">` in a 400px block is **400×160** in
Chrome and **400×40** here. That is not an edge case — it is *the* responsive-image idiom, the one
every CLS guide, CMS and framework emits: dimension attributes to reserve the aspect ratio,
`height:auto` in CSS to let it scale. We render it four times too short and every box below it slides
up by 120px.

### The one line

`engine/css/src/stylo_engine.rs:1216-1226`, a pass that runs *after* the cascade:

```rust
   if s.height == Dim::Auto && !s.height_stretch && !s.height_intrinsic {
       if let Some(h) = el.attr("height")... { s.height = h; }
   }
```

> **A post-cascade pass cannot tell `auto` that nobody set from `auto` the author asked for.** The
> HTML dimension attributes are presentational hints — a cascade ORIGIN *below* author CSS. Applying
> them afterwards puts them *above* it, and the only declarations that can lose are the ones that
> compute to the same value as the default.

That last clause is why it hid so long: the hint is only wrong when the author explicitly writes the
initial value, and `height: auto` is the one case where a whole ecosystem does exactly that.

### The battery, with a control per rule

Rewriting a 43-row SVG battery as ten rows with one variable each is what made it attributable — the
first cut had 23 divergences, 22 of which were one 40px `y` cascade from the first bad row.

```text
                                                                  chrome     ours
   a  attrs only                                    CONTROL       100x40    100x40   ok
   b  attrs + CSS width, height SPECIFIED           CONTROL       200x40    200x40   ok
   c  attrs + CSS width + height:auto                             200x80    200x40   DIVERGES
   d  viewBox only + CSS width + height:auto        CONTROL       200x100   200x100  ok
   e  attrs AND viewBox + height:auto                             200x80    200x40   DIVERGES
   f  attrs + max-width clamp, height SPECIFIED                   100x80    100x40   DIVERGES
   g  attrs + max-width clamp + height:auto         CONTROL       100x40    100x40   ok
   h  no ratio at all + height:auto                 CONTROL       200x150   200x150  ok
   i  attrs + CSS height, width SPECIFIED           CONTROL       100x200   100x200  ok
   j  attrs + CSS height + width:auto                             500x200   100x200  DIVERGES
```

Row **d** is the load-bearing control: with no attributes at all, the `viewBox` ratio path produces
Chrome's exact 200×100. **The ratio machinery downstream is correct; only its input is being
clobbered.** Rows **c**, **e** and **j** are the single line above (j is its `width` twin), and
`e` additionally pins the precedence — the ATTRIBUTE ratio (100:40) wins over the `viewBox` ratio
(100:50), which is what the cascade already implements.

⚠ **And it is not an SVG bug.** The same one-variable row against three tags:

```text
   img     width:100% + height:auto     chrome 400x160    ours 400x40
   canvas  width:200px + height:auto    chrome 200x80     ours 200x40
   svg     width:200px + height:auto    chrome 200x80     ours 200x40
   img     CSS width, height SPECIFIED  chrome 200x40     ours 200x40   CONTROL ok
```

### Price

Stylesheet-inclusive over the 170 corpus sites with a real body (551 stylesheets):

```text
   max-width:100% AND height:auto in the SAME RULE     72/170   42.4%
   height:auto anywhere + a dimension-attributed
      replaced element on the same site                84/170   49.4%
```

⚠ **42.4% is the honest number.** Same-rule means both declarations reach the same element by
construction; the co-occurrence figure is the weaker form whose real bound is `0 ≤ n ≤ 84`.

### The second defect, from the same battery

Row **f** — `<svg width="200" height="80" style="max-width:100px">` — is Chrome **100×80**, ours
**100×40**: when `max-width` clamps the used width we rescale the other axis from the intrinsic ratio
*even though the height is specified*. CSS 2.1 §10.4 recomputes the other axis only when it is `auto`,
and row **g** is the control (same markup plus `height:auto` → both engines 100×40).

> **The two defects hold exactly the knowledge the other lacks.** The max-width path owns an attribute
> ratio the `height:auto` path never builds, and applies it under a condition the spec forbids. Two
> code paths, one rule, and each implemented the half the other was missing.

### The fix — and t1025 named the wrong place, which is worth more than the fix

**⚠⚠⚠ CORRECTED at t1026.** Tick 1025 published the fix as *"implement the stub at
`engine/css/src/stylo_traits.rs:449`, because Stylo's own `rule_collector.rs:209` calls it"*.
`rule_collector.rs:209` does call it — **and our cascade never goes through `RuleCollector`.**
`cascade_one_element` matches candidates with `matches_selector` itself and hands one merged block to
`compute_for_declarations`. The hook is dead on our path, exactly like the `unimplemented!()` methods
beside it: required so a concrete `E: TElement` can be named, never reached at runtime.
**Implementing it would have changed nothing, and the tick would have "landed" with a green wall.**

> **Reading the dependency's source told me the caller exists; it could not tell me WE call it.** The
> producer to read was not Stylo's — it was ours. This is `READ THE PRODUCER, NOT ONLY THE CONSUMER`
> arriving one level up, and the stub's own comment (*"handled by our own UA pass"*) is what made it
> plausible: it described the outcome and was silent about the hook being unreachable.

The real fix is in the cascade we actually run — `engine/css/src/stylo_engine.rs`:

- `presentational_hint_block` turns `width`/`height` attributes into a real
  `PropertyDeclarationBlock`, re-serialised through `parse_dimension_attr_dim` so HTML's attribute
  grammar is parsed exactly once, in the function that already knew how.
- Its declarations are pushed into `ascending` **before** every matched rule, so first-seen-wins
  hands the win to any author declaration. `ORIGIN_PRES_HINT` (between user and author) names the
  intended rank.
- The `if s.width == Dim::Auto { s.width = attr }` pair is **deleted**, not patched. It had already
  been patched twice — `width_stretch`, then the intrinsic keywords — and a fourth flag would have
  been the third patch to the same wrong shape.

Result, and the controls are the point:

```text
   the 10-row isolation battery      6/10  ->  9/10 exact
   img / canvas / svg generalisation  3/7  ->   7/7 exact
   row f (the SECOND defect)                still open, by design
```

⚠ **Gated by `tests/wpt/corpus/dimension-attr-hint.html`, and its load-bearing row is `#p-sheet`** —
which writes `img { max-width:100%; height:auto }` as a **stylesheet rule**, not an inline style.
Every battery above used `style="..."`; a fix verified only that way would pass while the 42.4% of
the corpus that ships the reset as a rule stayed broken. `#p-attr` and `#p-spec` are the controls
that stop the gate being satisfied by simply ignoring the attributes.

**RED-proven twice, because the two mutations mean different things:** deleting the hint entirely
gives `1/6`, and putting it back *above* author CSS — the old origin — also gives `1/6`. The first
says the hint is load-bearing; only the second says the ORIGIN is.

## The default object size is a USED value, and writing it as a COMPUTED one switches off every rule that asks whether the size is auto (t1027)

The section above is tick 1026. This one is tick 1027, and its first finding is that **tick 1026 did
not do what tick 1026 says it did** — for one of the seven tags it names.

### There were TWO post-cascade dimension passes

```text
   engine/css/src/stylo_engine.rs:1206   img canvas video svg object embed iframe   <- t1026 fixed this
   engine/css/src/stylo_engine.rs:1046   table td th col colgroup iframe hr pre     <- this one survived
```

Both were `if s.width == Dim::Auto { s.width = attr }` after the cascade had finished; `iframe` was
in **both** lists, so `<iframe width=200 height=100 style="height:auto">` measured **200×100**
against Chrome's **200×150** *after* the origin was supposedly fixed. Both tag sets now feed the one
`presentational_hint_block`, so there is a single producer of dimension-attribute declarations at a
single origin.

> **`one rule, N implementations` is how this project loses a fix.** The way to find the second
> implementation was not to re-read the diff — the diff is correct and complete for what it touches.
> It was to **measure the tag the previous tick claimed to cover.**

### And the second deletion is the same error one step worse

```rust
   if tag == "iframe" {
       if s.width  == Dim::Auto { s.width  = Dim::Px(300.0); }
       if s.height == Dim::Auto { s.height = Dim::Px(150.0); }
   }
```

**The 300×150 is right; its PLACE was wrong.** It is the default object size (CSS-Images §4.4) — a
**used** value, resolved when the box is laid out — and `layout::default_object_tag` already lists
`iframe` and already writes it for `<svg>`/`<canvas>`/`<video>`. Written into the *computed* style it
turned `auto` into a definite length, which is the one fact four independent layout rules read:

```text
                                              chrome     before     after
   flex item, align-items:stretch            300x360    300x150    300x360
   flex COLUMN item (cross axis is width)    400x150    300x150    400x150
   width:200px + aspect-ratio:2/1 + h:auto   200x100    200x150    200x100
   attrs + height:auto                       200x150    200x100    200x150
   align-items:flex-start   (the CONTROL)    300x150    300x150    300x150
```

The generalisation battery is what identifies the cause: `<div>`, `<svg>`, `<canvas>`, `<img>`,
`<input>`, `<select>` and `<button>` **all already stretched**. `<iframe>` was the only tag with this
defect, which is what rules out the flex algorithm and names the cascade.

### The lesson worth carrying, in two halves

**A control is only a control next to a positive row that fails.** `align-items:flex-start` on an
iframe passed before the fix and after it — a box that can never stretch also never stretches when
told not to. Same for `frameborder="0"`: we match Chrome there because we have **no border to
remove** (see the open item below), not because we implement it.

**A wrong value upstream can be the only thing keeping a missing case downstream from ever being
reached.** With the computed lie deleted, a flex-item `<iframe>` measured **0×360**:
`layout::replaced_default_size` — the seam that reports a replaced element's intrinsic size to taffy
— did not list `iframe` either, and had never needed to. Deleting the lie is what exposes the gap,
and the gap then arrives as a red row in the same tick rather than as a bug found later.

### Gated, red-proven, and free

`tests/wpt/corpus/dimension-attr-hint.html` goes from 6 probes to 15 and stays **page 32 of 32** — a
parity fixture is free until the page count crosses a multiple of eight (audit #38), so the way to
add wall-enforced coverage without cost is to add probes to a page that already exists.

Two mutations, two disjoint failing pairs, five rows green under both:

```text
   restore the post-cascade dimension pass   p-tbl-auto  340x22 vs 11x22 · p-if-hauto  200x100 vs 200x150
   restore the computed 300x150 override     p-if-flex   300x150 vs 300x360 · p-if-ratio 200x150 vs 200x100
```

⚠ **`p-tbl-auto` exists because the first table row was not a test.** `<table width="85%">` under an
author `width:120px` stayed **green** under the mutation — the old pass only fired on `Dim::Auto`, and
`120px` is not auto. The discriminating row is an author who writes `width:auto`: Chrome shrink-to-fits
to 11px and the old code gave 340px. **A row that passes under the mutation is a control, whatever it
was meant to be.**

⚠ Every new row sits in a fixed-height container. In the first cut, mutation A failed seven probes,
five of them purely on `y`, because one 50px error cascaded down the page — the same trap as t1025's
43-row SVG battery.

### Still open on `<iframe>`, measured here

`iframe { border: 2px inset }` is Chrome's UA rule (asked via `getComputedStyle`, not recalled) and
it is the **only** replaced element with one — so every unstyled iframe is 4px small in both axes.
⚠ It cannot land alone: `frameborder="0"` must become a presentational hint in the same tick, or the
10 of 50 corpus iframe sites that use it regress.

## `overflow: clip` is the one non-visible overflow that is NOT a formatting context, and the comment said the opposite (t1029)

`engine/layout/src/lib.rs` carried this, as a deliberate claim:

> *"Chrome establishes a BFC for `overflow:clip` too, so any non-`visible` value counts."*

It is wrong. `clip` is the **one** overflow value defined to clip *without* becoming a scroll
container — which is precisely why the web uses it: `overflow-x: hidden` kills `position: sticky` in
an ancestor and `overflow-x: clip` does not. Measured against Chrome:

```text
                                                    chrome     before
   clip box containing a 60px float                200x0      200x60   <- must NOT contain it
   overflow-X:clip box containing a float          200x0      200x60
   clip box, child with margin-top:30px            200x10     200x40   <- margin must escape
   hidden box containing a float        CONTROL    100x60     contained in both   ok
```

> **A claim in a comment is a claim.** This one reads as diligence — it names Chrome and it gives a
> reason — and nothing in the file distinguishes *measured* from *reasoned, plausibly, once*.

### The fix is one predicate, because the old shape was `one rule, N implementations`

`establishes_bfc` asked `s.overflow != Overflow::Visible`, and the two margin-collapse predicates
*each* carried their own `s.overflow == Overflow::Visible` term **beside** a `!establishes_bfc(s)`
term that already implied it. `clip` therefore had to be got right in three places or in none — and
fixing only the BFC predicate would have left the margin half broken while looking complete.

```rust
fn overflow_establishes_bfc(o: Overflow) -> bool {
    !matches!(o, Overflow::Visible | Overflow::Clip)
}
```

Three callers, one definition; the two redundant terms are deleted rather than updated. `s.overflow`
is `overflow-x` when that is non-`visible`, else `overflow-y`, so `overflow-x: clip` alone — the form
the web actually writes — reaches the predicate as `Clip`.

### The controls are the half that needed proving

Gated by `tests/wpt/corpus/block-flow.html` (5 probes → 10, still page 32 of 32, so free).
RED-proven twice with **disjoint** failing pairs:

```text
   put `clip` BACK in the BFC set     p-clip-float · p-clipx-float · p-clip-margin
   take `hidden` OUT of the BFC set   p-hid-float · p-hid-margin
```

Mutation A says the `clip` rows are load-bearing. **Only mutation B says the controls are** — without
them the fixture is satisfied by an engine that has stopped establishing BFCs at all, a far larger
regression that would read as a pass.

⚠ Each row sits in its own `display:flow-root` wrapper at a fixed height. In the first cut the floats
the fix correctly stopped containing escaped into the *next* row and contaminated both controls, so
mutation A failed five probes instead of three. **Row isolation is not fixture polish — it is what
makes a RED proof attributable**, and this is the third consecutive tick where a battery needed it.

## An atomic inline is placed by its LINE BOX, and §10.3.3 was adding `leftover` on top of it (t1035)

Under `direction: rtl`, an `inline-block` was displaced by **exactly the containing block's width
minus its own**:

```text
   <html dir="rtl">, a 20x20 inline-block in a 400px row
                    chrome        ours
     first  box   x = 780      x = 1160     delta 380
     second box   x = 760      x = 1140     delta 380        380 == 400 − 20
```

**The 380 identified the arm, not a search**: it is literally `leftover` in `layout_block`'s CSS 2.1
§10.3.3 branch — *"if the `direction` of the containing block is `rtl`, the specified value of
`margin-left` is ignored"* — which sets `ml = leftover - mr`. §10.3.3 is for a **block-level** box.
An `inline-block` is an **atomic inline**: §10.3.9 sizes it and its **line box** places it. It was
taking `ml = leftover` on top of the line box's already-correct RTL placement, and the two stack.

⚠ **The guard was one class short and the comment explaining why was already there.** The arm already
excluded replaced elements, with a note saying an atomic inline *"belongs to its LINE BOX, not to
this equation"* — added because the corpus punished an earlier draft. The right reason sat next to a
guard implementing half of it.

### The variable is `inline-block`, not inheritance

RTL had already been refuted at 13/13 (t1032), so the first hypothesis was `dir` on `<html>` vs on
the row. The isolation battery says otherwise:

```text
   dir on <html>,     inline-block   FAILS  1160 vs 780
   dir on the ROW,    inline-block   FAILS  1160 vs 780   <- identical
   direction:rtl CSS, inline-block   FAILS  1160 vs 780   <- identical
   dir on <html>,     plain inline   ok
   dir=ltr,           inline-block   ok     CONTROL
   dir on <html>,     block child    ok     CONTROL (§10.3.3 itself still works)
```

t1032's 13/13 was not wrong — its rows used *plain inline* anchors, which still pass. **The row that
discriminates was not the row that made you look.**

### Gated, and the control is the load-bearing half

`tests/wpt/corpus/inline-flow.html` (4 probes → 10, still page 32 of 32, so free). RED-proven twice
with disjoint sets: deleting the atomic-inline guard kills `p-rtl-ib1/2`; deleting `parent_is_rtl`
kills the LTR mirrors **and** `p-rtl-blk`. ⚠ `p-rtl-blk` is why the fixture cannot be satisfied by
deleting §10.3.3 outright — which would put every Arabic sidebar and fixed-width panel back on the
wrong side, a much larger regression than the one being fixed.

⚠⚠ **It did not move the site that motivated it.** `m.youm7.com` — the Arabic page whose footer row
of `<a>` siblings gave 24 on-screen inversions — is unchanged at 24. **A fix can be real, spec-cited,
Chrome-exact, gated and twice RED-proven and still not be the cause of the thing you were chasing.**
What it buys is elimination: the RTL inline-block family is no longer a candidate explanation for
that lead.

## The iframe border, and the ten sites that were passing without it (t1037)

`<iframe>` is the **only** replaced element Chrome gives a UA border, and we had no rule at all.
Asked of Chrome directly (`getComputedStyle` over every HTML element — the enumeration half of
surface audit #43), not recalled:

```text
   <iframe>                    border = 2px inset    304x154      ours 300x150
   <iframe frameborder="0">    border = 0px INSET    300x150      ours 300x150   <- passed BY ACCIDENT
   <iframe frameborder="1">    border = 2px inset    304x154      ours 300x150
   <iframe frameborder="no">   border = 0px inset    300x150
```

### The second rule is the tick

**10 of the 50 corpus sites carrying an iframe write `frameborder="0"`**, and they matched Chrome
*because we had no border to remove*. Landing `iframe { border: 2px inset }` alone would have traded
ten working sites for the rest — the trade THE RATCHET refuses. Both rules land together or neither.

### `border-width: 0`, not `border: none`

Under `frameborder="0"` Chrome reports `border = 0px **inset**`: the width goes to zero and the style
survives. `border: none` would compute the style away too and disagree on a property a page can read
back. **The distinction is invisible in the geometry and visible in `getComputedStyle`** — only ever
found by asking the reference rather than matching a rect.

### Why a UA attribute selector and not a presentational hint

Chrome maps `frameborder` as a hint on `border-width`. Ours cannot: t1026 recorded that the hint block
is **prepended below the UA sheet**, noting *"if one ever [declares one] the UA value is the one a page
cannot have asked for."* Adding a UA `border` to `iframe` **is** that day. A hint would now lose to the
rule above it, so the rule is written as `iframe[frameborder="0"], iframe[frameborder="no"] {
border-width: 0 }` — same origin, higher specificity, Chrome's four computed values reproduced exactly,
no cascade surgery.

> **The latent tie t1026 named became real the moment this tick added the rule it was waiting for.**
> The note cost nothing to write and saved this tick from shipping a `frameborder` that silently did
> nothing.

Gated by `box-model.html` (2 probes → 5, still page 32 of 32). RED-proven twice on disjoint own
geometry: deleting the border rule kills `p-ifb-bare` and `p-ifb-one`; deleting the `frameborder` rule
kills `p-ifb-zero`. ⚠ `p-ifb-one` (`frameborder="1"` must KEEP the border) stops the gate being
satisfied by honouring *any* `frameborder` attribute; `p-ifb-zero` is the ten-site regression guard.

## An enumerated computed value is a fact about the reference, not always a rule you can copy (t1038)

Surface audit #43 established a method: **ask the reference to recite its own UA sheet** —
`getComputedStyle` over every HTML element, diffed against a `<span>`. Its second outing found the
method's own failure mode.

**Six declarations copied cleanly**: `small`/`big` `font-size`, `audio { display: none }`,
`hgroup, search { display: block }`, `nobr { white-space: nowrap }`. `<small>` alone is **8.8% of the
corpus** — every legal line, caption, byline and footnote was rendering at the parent's size.

⚠ **`smaller` is a RATIO, not a size**, which the nesting row is what proves:

```text
   parent 16px   <small> 13.3333px          <big> 19.2px
   parent 16px   <small><small> 11.1111px   <- 13.3333 / 1.2, it COMPOUNDS
   parent 10px   <small>  8.33333px         parent 32px  <small> 26.6667px
```

### `<legend>`: two attempts, nothing landed

The enumeration reported `display=block paddingLeft=2px paddingRight=2px`:

```text
   attempt 1  `display: block`         ours 400x20 vs chrome 29x20   <- 371px REGRESSION
   attempt 2  the 2px padding alone    ours  29x19 — INERT, x still 0 vs chrome's 2
```

A legend inside a `<fieldset>` shrinks to fit **whatever its computed `display` says**, so the
declaration does not reproduce the value; and Chrome's legend sits at `x=2` because of the
**fieldset's border**, not its own padding.

> **The value may be produced by machinery the declaration does not carry.** Enumerating the
> reference tells you *what is true there*; it does not tell you *which rule makes it true here*.

⚠ The second attempt is the more instructive half: **an inert rule is worse than no rule**, because it
reads as coverage in the sheet and in every audit that greps it. Deleted rather than kept "because it
is not wrong". `<ruby>` is absent by the same discipline — Chrome computes `display: ruby`, a layout
mode this engine does not implement, and the declaration would claim what the layout cannot honour.

### The gate proves the ratio, not a number

`inline-flow.html` (10 probes → 17, still page 32 of 32). RED-proven twice: replacing
`font-size: smaller` with a **constant** `13.3333px` passes `p-ua-small` and fails `p-ua-small2`;
deleting the `hgroup, search` rule fails those two. **A gate that asserted one `<small>` would have
been satisfied by a constant** — the nested row is what makes it a test of the mechanism.

## `reading_order` is a long tail, and one site's outlier was a quadratic artefact (t1041)

`jarring_reading_order` counts **pairs**, so one mis-laid row of `n` siblings contributes `n(n-1)/2` —
a 7-anchor footer row is 21. A site at `reading_order 24` might be **one** broken container. Since
`jarring-clean` is TOL 2, a single broken 3-sibling row already fails the conjunct. Measured:

```text
   site                   inversions   distinct containers   biggest contributes
   rockstaractu.com           13              13             1  (a 2-sibling group)
   www.otomoto.pl             11              11             1  (a 2-sibling group)
   www.kuechenmomente.de       5               5             1  (a 2-sibling group)
   m.youm7.com                24               5            17  (a 25-SIBLING group)
```

**For four sites in five, every inversion is its own container** — `reading_order 13` really is
thirteen independent two-element pairs. No single fix collapses that, because there is no shared
container to fix.

`m.youm7.com` is the exception and now explains itself: 17 of 24 from one 25-sibling footer row. Its
outlier count — what made it *"the sharpest lead the loop has"* — is a **quadratic artefact of one
container**, so ranking sites by raw `reading_order` put the least representative site at the top.

> **The reframe:** `reading_order` is not a missing mechanism waiting to be found. It is **distributed
> geometric inaccuracy crossing a binary threshold** — dozens of independent two-box pairs each landing
> on the wrong side of a 2px tolerance. A pair can be inside `shape` tolerance and still be ordered
> wrongly. **It is not a different problem from shape; it is the same problem measured with a step
> function** — which is why four Chrome-exact fixes moved it by exactly zero (t1040) and seven refuted
> mechanisms (t1032–t1036) never found "the" cause. There isn't one.

⚠ This does not mean stop: t1034 measured 85% of inversions between real on-screen boxes, so they are
defects a user could see. It means **hunting a shared mechanism across a long tail cannot work.**

## The intrinsic ratio fills an axis nobody specified, and CSS2.1 §10.4's table is not what Chrome does (t1042)

A replaced element with an intrinsic aspect ratio has two sources for each axis: what the page said,
and what the ratio implies. CSS2.1 §10.4 gives a **constraint-violation table** that, when a clamp
moves one axis, recomputes the *other* from the tentative used size — both axes, unconditionally.
Three places in this engine implemented that table, and one of them carried a code comment asserting
a Chrome measurement to back it up.

**Headless Chrome does not do that, and the comment's number was reasoned rather than read.**

```text
   <img width="800" height="400" style="max-width:100%">   in a 400px column
        §10.4's table  →  400 x 200        the comment claimed this was Chrome
        Chrome         →  400 x 400
   …the same with height:auto              →  400 x 200    both models agree
```

§10.4's table was superseded for replaced elements by CSS-Sizing. The rule Chrome implements is:

> **The intrinsic ratio fills an axis that is `auto`. A clamp on one axis never overwrites a value
> the page specified on the other.**

This is why every responsive-image reset on the web is written `max-width:100%; height:auto` and not
`max-width:100%` alone — **the `height:auto` is not decoration, it is the thing that makes the
transfer legal.** An author who omits it gets the full declared height in Chrome, and got a
ratio-scaled one here.

### The third state: `natural` is not `specified`, and a `Dim::Px` cannot tell you which

The guard is not "is this axis `auto`", because by layout time it may not be. `apply_natural_size`
writes a decoded bitmap's own pixel size into an `auto` axis, and what it leaves behind is a
`Dim::Px` indistinguishable from one the page asked for. **The difference is observable on one
bitmap and one clamp, with the dimension attributes as the only variable:**

```text
   <img              style="max-height:30px">    1000x266 bitmap    112.78 x 30    transfers
   <img w=1000 h=266 style="max-height:30px">    same bitmap        1000   x 30    does not
```

So `ComputedStyle` carries `width_is_natural` / `height_is_natural`, set by the one producer
(`manuk_css::fill_natural_size`), and the transfer fires into an axis that is `auto` **or** natural.

### `<canvas>` is the one tag whose dimension attributes are not the dimension properties

Every other member of the presentational-hint set maps `width`/`height` to the CSS properties.
`<canvas>` maps them to the **output bitmap** — the element's natural size — and it was in that set.

```text
   <canvas w=40 h=20 style="width:100px">    100 x 50    the auto height follows the natural ratio
   <svg    w=40 h=20 style="width:100px">    100 x 20    the attribute IS the height property
```

⚠⚠⚠ **AND IT READ AS CORRECT, BECAUSE THE TWO DEFECTS CANCELLED EXACTLY.** With canvas wrongly
pinned to a specified height *and* the transfer wrongly overwriting specified axes, `<canvas w=40
h=20 style="max-width:20px">` came out 20x10 — Chrome's answer, by two errors. Fixing either one
alone regresses it. This is the third time this project has found a cancelling pair, and the tell is
the same every time: **the row that passes under both the right model and the wrong one is not
evidence, and only a row where the two models predict different numbers is.**

### How it was found, and the part of the method that did the work

A property-family battery on inline `<svg>` — the corpus's fifth-ranked construct at 34.5%
(`CORPUS-CONSTRUCTS.md`), and one with no differential reading. 29 rows, **one** diverged. Chasing
that single row through four more batteries (103 rows total) turned it into a rule, and the rows that
settled it were never the row that made me look: the `<svg>` that started it is not in the corpus's
top three, and the decisive pair was two `<img>`s differing only by their attributes.

⚠ Every number in the gate was read off headless Chrome, **including the one written down wrong**:
the control row was asserted at 40x20 from the shape of the markup and Chrome said 60x30, because a
`viewBox` is a ratio and never a size. It is a better control for it — both of its axes are derived,
so it goes red if the transfer is *deleted* rather than guarded, which is what makes
`the_ratio_transfer_never_overwrites_a_specified_axis` falsifiable in both directions.

## A blockified inline is an ANONYMOUS BLOCK, and the engine said so in one function and contradicted it in every other (t1048)

CSS 2.1 §9.2.1.1 splits an inline box around a block-level child into anonymous block boxes. This
engine approximates that by **blockifying** the inline (`is_block_level` → `inline_contains_block`),
which reproduces the right box *structure* — and, until this tick, also handed the block child a box
model the spec says it never sees.

**What CSS actually says.** A non-replaced inline **ignores `width` and `height` outright** (§10.2,
§10.5). Its padding, border and margin apply at the **split edges of its own fragments**, never to
the block-level child — that child is laid out in the containing block the inline was in. Blockifying
made every one of those properties real.

Measured against headless Chrome, 1200px, `<div style="width:400px">` container, 30px block child.
The row is the **child's** parent-relative `[dx dy w h]`, because that is what cascades down a page:

```text
                                Chrome            before             after
  <a width:100px>   <div>   [0  0 400x30]   [0 0 100x30]  ✗ 4x    [0  0 400x30]  ✓
  <a height:100px>  <div>   [0  0 400x30]   h(a) = 100    ✗ 70    [0  0 400x30]  ✓
  <a padding:10px>  <div>   [0 20 400x30]   [10 10 380x30] ✗      [0  0 400x30]  ~
  <a padding:10px 0><div>   [0  0 400x30]   [0 10 400x30] ✗ 10    [0  0 400x30]  ✓
  <a margin:10px>   <div>   [0 20 400x30]   [10 0 380x30] ✗       [0  0 400x30]  ~
  <a border:5px>    <div>   [0 20 400x30]   [5 5 390x30]  ✗       [0  0 400x30]  ~
  <a background>    <div>   [0  0 400x30]   [0 0 400x30]  ✓       [0  0 400x30]  ✓
```

### It was already written down as a rule, in one place, and obeyed there only

`collapses_as_block`'s own doc comment states the model exactly — *"the blockified inline stands in
for the spec's ANONYMOUS BLOCK BOXES, and an anonymous block has no margin, border or padding of its
own"* — and that sentence was written to fix the **margin-collapse** predicates. Width, height and
the four min/max clamps were never told. The same box was an anonymous block for margin collapse and
the author's own styled block for everything else: **one rule, two implementations**, the class this
project keeps finding (t720-724).

### ⚠⚠⚠ Neutralising it in `layout_block` alone made a row WORSE, and that is the reusable part

The first version zeroed the box model on `layout_block`'s style clone. The `margin:10px` row went
from `dy 0` to **`dy -10`** — a *new* error on the row the fix was aimed at. The parent independently
re-derives the same child's top margin through `collapse_through_top` to compute `hoist_top`, and it
reads `style_of` directly, so it hoisted the child by a margin the child had just deleted. **The
margin was spent in one place and refunded in another** — the identical two-implementations shape,
one level up, reproduced by the fix for it.

> **A neutralised style must be reached through ONE accessor, because a box's margin is read by its
> parent as well as by itself.** `block_box_style` is now the single reader; `layout_block`,
> `collapse_through_top` and `collapse_through_bottom` all go through it, and it borrows on every
> path but the blockified one.

### The paint moves towards Chrome too, which is why this is not a trade

Chrome paints a split inline's background and border on its **fragments**. With a block-only child
those fragments are empty, so Chrome draws no box around the card. We were drawing a fully padded,
fully bordered rectangle around it. Dropping it is not losing a border Chrome has — it is stopping
one Chrome does not.

### What is NOT built, with its numbers

The three `~` rows keep a `dy 20`: Chrome generates a **leading anonymous block** for the inline's
start fragment when that fragment has horizontal padding/border/margin (it then has real inline
extent, so it opens a line box). We generate none. Note the discriminator — `padding:10px 0` is
exact, `padding:10px` is not, so it is the *horizontal* edge that decides. Also open, from the same
battery: an inline sitting mid-line before its own split does not join the preceding text run
(`foo <a>bar<div/></a>` puts the block at `dy 40` against Chrome's 20), a **float inside an inline
loses its box entirely**, and an inline's own rect excludes an inline child's padding (`27x27` in
Chrome, `27x19` here).

### The frequency claim that licensed it had never been measured

The comment authorising the approximation said it was *"invisible unless a block-containing inline is
itself styled, which is vanishingly rare."* Measured on the 170-page burndown corpus (t1047): a
block-in-inline appears on **71 pages (41.8%)**, and the inline is **itself styled on 51 (30.0%)** —
1,925 elements, led by meet.google.com 288, bbs.ruliweb.com 268, id.vk.ru 247, fragrantica 154,
sports.yahoo 121. It is `<a class="card"><div>…</div></a>`, the whole-tile-is-a-link behind every
card grid, product tile and article teaser on the web. **Grep the corpus against a comment's PREMISE,
not only against a construct.**
