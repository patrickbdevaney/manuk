# REPOMAP 02 — Layout

How production browser engines implement **layout** — block/inline flow, flexbox,
grid, fragmentation, and incremental/dirty-bit relayout — read against Manuk's
current engine to guide a lean, from-scratch Rust implementation.

All paths are absolute under `/home/patrickd/manuk/`. Citations are `file:line`
against the local clones (snapshots; line numbers drift with upstream).

---

## 1. Scope & sources

Engines and the directories actually inspected:

| Engine | Root inspected |
|---|---|
| **Blink / LayoutNG** | `chromium/third_party/blink/renderer/core/layout` (flattened — no `ng/` subdir; per-FC subdirs `inline/`, `flex/`, `grid/`, `table/`) |
| **Gecko** | `firefox/layout` (`base/`, `generic/`, `tables/`, `forms/`) |
| **Servo** | `servo/components/layout` (modern engine; `layout_2020` was promoted, legacy `layout_thread` removed) |
| **WebKit** | `WebKit/Source/WebCore/layout` (LFC) + `WebKit/Source/WebCore/rendering` (shipping tree) |
| **Ladybird** | `ladybird/Libraries/LibWeb/Layout` |
| **Rust refs** | `taffy/` (flex+grid+block algorithm library), `blitz/` (stylo + layout glue) |
| **Manuk (subject)** | `engine/layout/src/lib.rs` (2945 lines) + `engine/layout/src/flex.rs`; driven from `engine/page/src/lib.rs`; CSS via Stylo mapped in `engine/css/src/stylo_map.rs` |

---

## 2. Per-engine approach

### 2.1 Blink — LayoutNG

**Two parallel trees.** A persistent, mutable **layout tree** (`layout_object.h`
`LayoutObject`, `layout_box.h` `LayoutBox`, `layout_block_flow.h`) is the input;
an immutable **fragment tree** (`physical_fragment.h:64` `PhysicalFragment :
GarbageCollected`, `physical_box_fragment.h:37` `PhysicalBoxFragment`) is the
per-layout output consumed by paint/hit-test. Lightweight value wrappers
(`block_node.h:40` `BlockNode::Layout(const ConstraintSpace&)`,
`inline/inline_node.h` `InlineNode`) drive the algorithms over the layout tree.

**Constraint spaces.** `constraint_space.h` is the immutable per-layout input:
`AvailableSize()` (:177), `PercentageResolution*Size()` (:181), fragmentainer
geometry (:314/341), and crucially `CacheSlot()` (:428) with
`enum class LayoutResultCacheSlot { kLayout, kMeasure }` (:89). Built once via
`constraint_space_builder.h` (`ToConstraintSpace()` :647). Each algorithm
(`layout_algorithm.h`, CRTP base) consumes a space and emits a `LayoutResult`
(`layout_result.h:46`) — which **embeds the ConstraintSpace it was produced with**
(`:847 const ConstraintSpace space_`), so the cache key travels with the value.
Algorithms: `BlockLayoutAlgorithm`, `flex/flex_layout_algorithm.cc`,
`grid/grid_layout_algorithm.cc` (+ `grid_track_sizing_algorithm.cc`),
`table/table_layout_algorithm.cc`.

**Inline & line breaking.** `InlineItemsBuilderTemplate`
(`inline/inline_items_builder.h:45`) flattens the DOM subtree into `InlineItem`s;
`SegmentText` (`inline/inline_node.cc:1213`) does bidi (`BidiParagraph`) + script
+ font-orientation segmentation via `RunSegmenter`. `ShapeResult`s are cached per
item and **reused across edits** (`CollectReusableShapeResults`/`Reshape`,
`inline_node.cc:149`). `LineBreaker` (`inline/line_breaker.h:37`) runs in
`kContent`/`kMinContent`/`kMaxContent` modes; an optional Knuth-Plass optimal
breaker exists (`inline/score_line_breaker.h`). Inline output is a **flat list**
`FragmentItems` (`inline/fragment_items.h:22`) walked by `InlineCursor` — no nested
inline-box object tree.

**Fragmentation.** Multicol `ColumnLayoutAlgorithm` (column balancing,
`column_layout_algorithm.cc:213`), pagination
(`paginated_root_layout_algorithm.cc`), and **break tokens**
(`block_break_token.h`) as resumable continuations — one uniform model across FCs.

**Incremental relayout & caching (the crown jewel).**
- `LayoutBox` stores `layout_results_` (fragment/result cache), `measure_cache_`
  (`MeasureCache`, cap 8 entries, `measure_cache.h:33`), `min_max_sizes_cache_`,
  `intrinsic_logical_widths_` (`layout_box.h:1298`).
- A cache hit is a **size-based comparison of the incoming ConstraintSpace against
  the stored one**: `CalculateSizeBasedLayoutCacheStatus` →
  `enum LayoutCacheStatus { kHit, kNeedsSimplifiedLayout, kNeedsLayout }`
  (`layout_utils.h:25`, impl `layout_utils.cc:176`).
- **Separate measure vs layout cache slots** — flex/grid re-measure children many
  times; caching measures separately avoids O(n²) blowup.
- **Block-size-keyed MinMaxSizes cache** (`min_max_sizes_cache.h:44
  Find(initial_block_size)`) correctly caches intrinsic widths even when they
  depend on the block axis (aspect-ratio, orthogonal flows).
- **Three-tier relayout**: full → *simplified* (`simplified_layout_algorithm.cc`,
  reposition/resize only, `NeedsSimplifiedLayout`) → cache hit. Dirty bits on
  `LayoutObject`: `NeedsLayout()`, `SelfNeedsFullLayout`, `SetNeedsLayout`,
  `MarkContainerChainForLayout` (`layout_object.h:1427/1900`).

### 2.2 Gecko

**Frame tree.** One tree of `nsIFrame` (`generic/nsIFrame.h:695`; `nsFrame` was
merged in). Each frame links to `nsIContent` (`GetContent()` :884) and is born
dirty (`mState(NS_FRAME_FIRST_REFLOW | NS_FRAME_IS_DIRTY)` :750). Concrete:
`nsBlockFrame`, `nsInlineFrame` (:24), `nsFlexContainerFrame`,
`nsGridContainerFrame`. Fragmentation is realized via **continuation chains** on
`nsSplittableFrame` (:16): *fluid* continuations ("in-flows") from
line-breaking/pagination vs *non-fluid* from bidi splits / `::first-letter`.
`nsCSSFrameConstructor` builds the tree.

**Reflow.** `ReflowInput` (`generic/ReflowInput.h`, formerly `nsHTMLReflowState`)
carries logical `AvailableISize()`/`ComputedISize()` and resize flags
`mIsIResize`/`mIsBResize` (replacing the old reason enum); output is `ReflowOutput`
(`ReflowOutput.h:187`). Block reflow: `nsBlockFrame::ReflowDirtyLines`
(`nsBlockFrame.cpp:3217`) → `ReflowLine` → `ReflowBlockFrame` / `ReflowInlineFrames`;
`BlockReflowState` threads the float manager + running block position;
`nsBlockReflowContext` does margin collapsing.

**Inline.** `nsLineLayout` (`generic/nsLineLayout.h:21`) positions inline frames on
a line; `nsLineBox` (`nsLineBox.h:5`) is one CSS line box. `nsTextFrame` integrates
`gfxTextRun` (lazy `EnsureTextRun`); bidi resolved *before* line layout by
`nsBidiPresUtils::Resolve(nsBlockFrame*)`, splitting frames into non-fluid
continuations per bidi level.

**Flex/grid.** Flex (`nsFlexContainerFrame.cpp`): `FlexItem`/`FlexLine`/
`FlexboxAxisTracker`; §9.7 `FlexLine::ResolveFlexibleLengths` (:3141);
`CachedFlexItemData` (:40) caches item measurements across reflows. Grid
(`nsGridContainerFrame.cpp`) is heavily **spec-step-annotated** with `#algo-*` URLs:
§12.3 track sizing `Tracks::ResolveIntrinsicSize` (:7045), auto-placement
`Grid::PlaceGridItems` (:5173), fragmentation `ReflowInFragmentainer` (:8257).

**Incremental reflow & dirty bits (mature).**
- **Two-level dirty bits**: `NS_FRAME_IS_DIRTY` (reflow self + all descendants) vs
  `NS_FRAME_HAS_DIRTY_CHILDREN` (only dirty children); combined `IsSubtreeDirty()`
  (`nsIFrame.h:4926`). `ChildIsDirty` bubbles the has-dirty-children bit up.
- **Reflow-root queue**: `PresShell::FrameNeedsReflow` marks + enqueues; dirty roots
  live in a **depth-ordered** `mDirtyRoots` (`DepthOrderedFrameList`), processed
  shallowest-first by `PresShell::DoReflow`. `IntrinsicDirty` enum controls how far
  intrinsic-size dirtiness propagates. **Reflow roots** decouple independently-sized
  subtrees (e.g. scroll frames) so a dirty child need not reflow ancestors.
- **Intrinsic inline-size cache**: `GetMinISize`/`GetPrefISize` (`nsIFrame.h:2750`) →
  `IntrinsicISizesCache` (`generic/IntrinsicISizesCache.h:18`) — a *pointer-sized*
  cache in the common case that spills out-of-line only under percentage-basis
  dependencies (`NS_FRAME_DESCENDANT_INTRINSIC_ISIZE_DEPENDS_ON_BSIZE`).
- **Logical (writing-mode) coordinates everywhere** (`ISize`/`BSize`, `LogicalSize`)
  avoid physical-axis branching; flex/grid express main/cross via `LogicalAxis`.
- Floats factored into a reusable `nsFloatManager` band/exclusion model.

### 2.3 Servo (modern layout, shares Stylo with Manuk)

**Box tree → fragment tree.** `BoxTree::construct` (`flow/root.rs:40`) builds a
mutable **box tree** from the styled DOM; `BoxTree::layout` (`flow/root.rs:173`)
produces an immutable **`FragmentTree`** (`fragment_tree/fragment_tree.rs:19`) of
`Fragment` (`fragment_tree/fragment.rs:32`: `Box`/`Float`/`Positioning`/`Text`/
`Image`/`IFrame`). The FC abstraction is `IndependentFormattingContext`
(`formatting_contexts.rs`) whose payload enum
`IndependentFormattingContextContents` dispatches
`Flow(BlockFormattingContext)` / `Flex(FlexContainer)` / `Grid(TaffyContainer)` /
`Table(Table)` / `Replaced` (:52). `BlockFormattingContext` wraps `BlockContainer`
(`flow/mod.rs:57/63`), which is either block-level boxes or an
`InlineFormattingContext`.

**Constraints / containing block.** `ContainingBlock` (`lib.rs:130`),
`DefiniteContainingBlock` / `IndefiniteContainingBlock` (`lib.rs:86`),
`ConstraintSpace` for intrinsic queries, and `ContentSizes { min_content,
max_content }` (`sizing.rs:41`) with algebraic combinators implementing
min/max-content. `ContainingBlockManager` (`fragment_tree/containing_block.rs:13`)
threads in-flow/abs/fixed CBs down.

**Inline.** `InlineFormattingContext` (`flow/inline/mod.rs`); line breaking via the
**`icu_segmenter` crate** (`LineBreaker`, `flow/inline/line_breaker.rs:9`), bidi via
**`unicode-bidi`** (`mod.rs:116`), shaping via Servo's own `fonts` crate
(`text_run.rs:266 TextRun::shape_text`). Not hand-rolled segmentation.

**Flex & grid.** Flex is **hand-rolled** (`flexbox/layout.rs`); **grid is delegated
to `taffy`** (`taffy/layout.rs:84`, `TaffyContainer` implements
`taffy::TraversePartialTree`; Stylo→Taffy adapter in `taffy/stylo_taffy/`). Tables
hand-rolled (`table/`).

**Floats.** `flow/float.rs` — `FloatContext` (:311), `FloatBand`/`FloatBandTree`
(:536/633), `PlacementInfo`, `FloatSide`, `Clear`. **This is the file Manuk's
`FloatContext` mirrors**; Servo's band-tree is the more scalable data structure.

**Incremental layout.** Servo **does** incremental/damage-driven layout.
`LayoutDamage` from `RestyleDamage` (`traversal.rs:191`) sets flags
`Relayout`/`RecalculateOverflow`/`RecomputeInlineContentSizes` (`traversal.rs:148`).
`LayoutBoxBase` caches `cached_layout_result` + `cached_layout_result_dirty`
(`layout_box_base.rs:52`), with `invalidate_caches` (:164) and `repair_style` for
style-only changes. Damage is **isolated at independent FC boundaries and body/root**
(`dom.rs:368`) to bound relayout scope. `Arc`/`ArcRefCell` sharing exists precisely
to make box/fragment reuse possible.

### 2.4 WebKit — LFC (modern) vs `rendering/` (shipping)

WebKit runs **two** engines. The legacy `RenderObject` tree still drives the page;
the modern **LFC** (Layout Formatting Context, `Source/WebCore/layout/`) is
production **only for covered content via "integration"**, not a whole-page
replacement.

**LFC model — box tree vs geometry.** The input is a tree of `Layout::Box`
(`layouttree/LayoutBox.h:48`) / `ElementBox` / `InlineTextBox` — stable structural
boxes. Results live in **`BoxGeometry`** (`LayoutBoxGeometry.h:35`) stored in a
side-map inside **`LayoutState`** (`HashMap<const Box*, BoxGeometry>`,
`LayoutState.h:126`), never mutating the box. This box-vs-geometry split is LFC's
defining idea. `LayoutState` also caches inline items across relayouts
(`inlineContentCache`) and holds per-FC state maps.

**Formatting contexts as the unit of algorithm + encapsulation.** Base
`FormattingContext` (`formattingContexts/FormattingContext.h:48`); `Block` and
`Table` still derive from it, while the newer `Inline`, `Flex`, `Grid` FCs are
standalone with their own `layout(...)` entries. An FC communicates only via typed
constraints in / `BoxGeometry` out; the **`EscapeReason` enum** (:62) explicitly
enumerates the *only* sanctioned cases where an FC may read geometry outside its own
subtree — a clean encapsulation boundary (and a great fit for a Rust borrow model:
each FC touches only its own geometry slice).

**Constraints.** `HorizontalConstraints{logicalLeft,logicalWidth}` /
`VerticalConstraints` (`FormattingConstraints.h:34`); `ConstraintsForInFlowContent`
is **tagged by `BaseTypeFlag {GenericContent, InlineContent, TableContent,
FlexContent}`** (:52) so a child FC statically knows its context flavor.
`IntrinsicWidthConstraints{minimum,maximum}` (:97) is the min/max-content currency.

**Inline (the mature, shipping integration — "modern line layout").**
`InlineFormattingContext` (`formattingContexts/inline/InlineFormattingContext.h:59`).
Pipeline: DOM/style → `InlineItem` list (`InlineItemsBuilder`, cached in
`InlineContentCache`) → line building → `InlineDisplay::Content`. Line building sits
behind an abstract trait `AbstractLineBuilder` (:43) with **multiple strategies**:
`InlineLineBuilder` (full floats/bidi/hyphenation), `TextOnlySimpleLineBuilder` and
`RangeBasedLineBuilder` (fast paths). `InlineContentBreaker` decides break/overflow;
shaping bridges to `FontCascade`/`TextRun` via `text/TextUtil`. **Damage-based
incremental inline relayout**: `InlineLayoutResult::Range {Full, FullFromDamage,
PartialFromDamage}` + `InlineDamage` (:44) let relayout resume from a damaged line.

**Legacy shipping tree.** `RenderBlockFlow` is the block+inline workhorse; it
selects a `LineLayoutPath` and holds the modern `LayoutIntegration::LineLayout`
when coverage allows (`integration/LayoutIntegrationCoverage.h` gatekeepers
`canUseForLineLayout`/`canUseForFlexLayout`/`canUseForGridLayout`). Production
flex/grid are `RenderFlexibleBox`/`RenderGrid`.

**Incremental & dirty bits (legacy path drives relayout).** `RenderObject` state
bits: `selfNeedsLayout`, `normalChildNeedsLayout`, `needsSimplifiedNormalFlowLayout`,
`needsOutOfFlowMovementLayout` (`RenderObject.h:704`). `setNeedsLayout(MarkContaining
BlockChain)` → `markContainingBlocksForLayout(layoutRoot)` walks to a **layout root**.
Intrinsic-width caching: `m_minContentLogicalWidthContribution` /
`m_maxContentLogicalWidthContribution` on `RenderBox` (:736), invalidated via
`StateFlag::ContentLogicalWidthsInvalidated`. A pragmatic **coverage-gated migration**
(ship inline first via integration, fall back to legacy otherwise).

### 2.5 Ladybird (from-scratch C++, closest in spirit to Manuk)

**One `FormattingContext` base + `run(LayoutInput)` polymorphism**
(`Layout/FormattingContext.h:64`). A `Type` enum (`Block/Inline/Flex/Grid/Table/
SVG/...`, :72) is chosen by `formatting_context_type_created_by_box(Box const&)`
(`FormattingContext.cpp:339`) mapping CSS `display` → FC; the factory
`create_independent_formatting_context_if_needed` (:439) switch-constructs the
concrete FC. IFC is special-cased (a BFC creates its own IFC directly). Layout tree:
`Layout::Node → NodeWithStyle → NodeWithStyleAndBoxModelMetrics → Box`.

**Constraints as arithmetic.** `AvailableSize` (`AvailableSpace.h:16`) is a tagged
`Definite | Indefinite | MinContent | MaxContent` whose comparison operators treat
max-content/indefinite as +∞ and min-content as −∞ (:55) — folding intrinsic
constraints into ordinary arithmetic, no special-case branches. **Geometry is
separated from the tree**: all used values live in a swappable `LayoutState` /
`UsedValues` store (`LayoutState.h:154`), published to paintables via
`commit(Box&)` (:395) — enabling throwaway "measuring" layouts.

**Block/inline/floats.** Floats are hand-rolled *inside* the BFC (no separate class):
`FloatingBox`/`FloatSide`/`FloatBand` + `SpaceUsedByFloats` from
`available_inline_space(...)` (`BlockFormattingContext.h:50`). Inline: `IFC`
drives `InlineLevelIterator` → `LineBuilder` (`LineBuilder.h:13`,
`break_line`/`append_text_chunk`, float-aware `recalculate_available_space`).

**Flex & grid — full CSS algorithms, hand-rolled, spec-named methods.** Flex
(`FlexFormattingContext.h`) mirrors CSS §9 step by step:
`collect_flex_items_into_flex_lines` → `resolve_flexible_lengths` (§9.7) →
`determine_used_cross_size...` → `distribute_any_remaining_free_space` →
`align_all_flex_items_along_the_cross_axis`. Grid (`GridFormattingContext.h`)
implements §11/§12 track sizing: `run_track_sizing` (:383) with
`initialize_track_sizes`/`resolve_intrinsic_track_sizes`/`expand_flexible_tracks`
(fr) /`stretch_auto_tracks`, plus auto-placement `place_grid_items` (:344). No
third-party library.

**Incremental & fragmentation.** *Full relayout* per pass
(`Document::update_layout`, gated by a dirty flag); coarse partial-relayout reuses
cached paintable geometry via `discard_used_values_for_descendants`. Layout-level
memoization = per-`Box` `struct IntrinsicSizes` (`Box.h:22`, width-keyed heights).
**No CSS fragmentation/pagination** as a first-class abstraction — a notable gap.
`InternalReplaced`/`InternalDummy` "hack" FCs keep the engine crash-free on
unimplemented display types.

### 2.6 Rust references — Taffy & Blitz

**Taffy** is a *trait-driven layout kernel* — you implement its traits over your own
host tree; it never owns your nodes (`src/tree/traits.rs`).
- `TraversePartialTree` (:148) exposes only a container's *immediate children* — this
  is exactly what lets it lay out one container in isolation. `LayoutPartialTree`
  (:174) is central: `get_core_container_style`, `resolve_calc_value`,
  `set_unrounded_layout`, and the recursion hook `compute_child_layout(node,
  LayoutInput) -> LayoutOutput`. Per-algorithm getter traits
  `LayoutFlexboxContainer` (:240), `LayoutGridContainer` (:259),
  `LayoutBlockContainer` (:288, threads a `BlockContext` for margin collapsing).
- Algorithms (`src/compute/`): `compute_flexbox_layout`, `compute_grid_layout`,
  `compute_block_layout` (margin-collapsing block!), `compute_leaf_layout`. Entry
  `compute_root_layout(tree, root, Size<AvailableSpace>)`.
- **Measure seam**: `compute_leaf_layout` takes a
  `MeasureFunction: FnOnce(Size<Option<f32>>, Size<AvailableSpace>) -> Size<f32>` —
  the host's text/replaced hook. `RunMode` (`PerformLayout`/`ComputeSize`/
  `PerformHiddenLayout`) + `SizingMode` (`ContentSize`/`InherentSize`) implement the
  cheap size-vs-layout two-pass.
- **Caching (`src/tree/cache.rs`)** — the model for Manuk's incremental goal.
  `CACHE_SIZE = 9`: one `final_layout_entry` + **eight measure slots**.
  `compute_cache_slot(known_dimensions, available_space)` (:187) routes each
  (known-dims × min/max-content) probe combination to a distinct slot so repeated
  intrinsic probes in one pass don't clobber each other. `CacheKey` bit-packs
  known dims + available space + parent size. `compute_cached_layout`
  (`compute/mod.rs:174`) wraps *every* recursion, so caching is transparent to the
  algorithms. `Cache::clear` → `ClearState::{Cleared, AlreadyEmpty}` for cheap
  dirty invalidation.
- Rounding rounds *cumulative* coords and derives sizes as edge differences
  (never rounds width directly) to avoid gaps (`compute/mod.rs:219`).

**Blitz** proves the "drive taffy directly over your own DOM" model that Manuk
should adopt.
- `BaseDocument` (a flat node arena) implements **all** taffy traits
  (`packages/blitz-dom/src/layout/mod.rs`): `TraversePartialTree` (:284),
  `LayoutPartialTree` (:315), `CacheTree` (:347, delegates to a per-node taffy
  `Cache` — free 9-slot cache per DOM node), plus the flex/grid/block getter traits.
  Root driver: `resolve_layout` calls `taffy::compute_root_layout(self, root,
  avail)` then `taffy::round_layout` (`resolve.rs:375`).
- **Stylo→`taffy::Style` converted once and cached on the node**
  (`node.style: taffy::Style<Atom>`; `stylo_taffy/convert.rs`); getters return
  borrows — zero per-query conversion. (`Display::Table → taffy Grid`,
  `Flow/FlowRoot → Block`.)
- **Text/inline is NOT taffy's job** — the inline root is a taffy leaf whose measure
  closure runs **Parley** (`layout/inline.rs`, `compute_inline_layout`;
  `stylo_to_parley.rs`). Replaced elements (img/input/textarea) are leaves with
  custom measure closures; tables wrap in `TableTreeWrapper` computed as grid.

---

## 3. Manuk today

**Model: fragment tree only, single pass, absolute px.** `layout_document`
(`engine/layout/src/lib.rs:356`) produces one `LayoutBox` tree
(`lib.rs:157` — absolute border-box `Rect` + `BoxContent::Block|Inline`). There is
**no persistent layout/box tree and no constraint-space object** — the whole
document is re-laid-out from styles each call. Everything is physical px,
**Latin/LTR only** (no writing-mode / logical axes, no RTL/vertical).

**Block formatting (hand-rolled, solid).** `layout_block` (`lib.rs:769`) →
`layout_children` (`lib.rs:952`). Implements box-sizing, min/max clamps, and
adjacent-sibling **margin collapsing** (`collapse_margins`, `lib.rs:749`). Floats
via `FloatContext` (`lib.rs:558`) — a linear scan mirroring Servo's
`flow/float.rs` (but a flat `Vec<PlacedFloat>`, not Servo's `FloatBandTree`), with
BFC establishment (`establishes_bfc`, `lib.rs:701`), clearance, and shrink-to-fit.

**On the reported `margin:auto` / `width` gap — the layout code is present and
correct.** `layout_block` resolves a definite `width` and performs horizontal
auto-margin centering (`lib.rs:833-840`: `(true,true) => ml = leftover/2`), and
Stylo maps `width:600px → Dim::Px(600)` and `margin:auto → Dim::Auto`
(`engine/css/src/stylo_map.rs:37,53`). So the "600px card renders full-width
flush-left" symptom is **not** a block-width-algorithm bug. Most likely causes to
verify against the harness: (a) a **stale report** already fixed by the committed
centering code; or (b) a **viewport-width mismatch** — the `@media` evaluation
`Device` is hardcoded to **1024×768** in the primary cascade path
(`engine/css/src/stylo_engine.rs:79 cascade_via_stylo(dom, sheets, 1024.0, 768.0)`)
and is decoupled from the layout `viewport_width`. For any render width ≠ 1024,
example.com's `@media (max-width:700px){div{width:auto;margin:0 auto}}` can fire (or
fail to fire) against the wrong width, yielding a full-width flush-left card even
though the block algorithm is correct. **This viewport coupling is the real thing to
fix / confirm**, not `layout_block`.

**Inline (hand-rolled, weakest area).** Greedy line breaker `layout_inline`
(`lib.rs:2080`) over `InlineItem`s; float-aware line bands. **No shaping, no bidi,
no ICU segmentation** — widths from `fonts.measure`. Documented defects: inserts an
inter-word space between adjacent tokens so `a<b>b</b>` gains a spurious space
(`lib.rs:29`); float band uses the first word's height as the line-height estimate.

**Flex & grid via Taffy — but as isolated per-container solves.** `flex.rs`
(`solve_flex`/`solve_grid`) builds a **fresh `TaffyTree` per container**, computes,
extracts `Slot`s, then Manuk lays out each child as a block within its slot
(`place_taffy_slots`, `lib.rs:1946`). Item sizes handed to taffy are only fixed
`Px` or `None` (`lib.rs:1868`) — there is **no measure function**, so taffy cannot
intrinsically size auto items from their content, and nested flex/grid inside a flex
item is re-solved in a separate throwaway tree. This is the opposite of the
Blitz/Servo model (implement taffy's traits over your own tree, one
`compute_root_layout`, shared per-node cache).

**Tables (hand-rolled, CSS2 separated model).** `layout_table` (`lib.rs:1211`):
colspan/rowspan placement grid, auto (`auto_col_widths`, min/max distribution) and
fixed column algorithms. No `border-collapse`, captions, or `<col>` hints.

**Positioned / transforms.** relative/absolute/fixed in a final pass
(`position_absolutes`, `lib.rs:1580`); **static position for inset-less abs boxes is
unimplemented** (left unplaced, `lib.rs:1604`); no `sticky`; z-index = DOM order.
`transform` baked into fragment coords as an affine (`transform_affine`, `lib.rs:295`).

**Intrinsic sizing — recomputed, not cached.** `shrink_to_fit` (`lib.rs:1193`) and
`cell_intrinsic` (`lib.rs:1430`) lay children out at width `1e6` (max-content) / `0`
(min-content) each call, with **no memoization** — an O(n²) risk for nested
shrink-to-fit and auto tables.

**Incremental relayout — coarse.** `Page::relayout_incremental`
(`engine/page/src/lib.rs:487`): `subtree_clean` short-circuits a fully-clean tree;
otherwise re-cascade, compute a tree-wide `RestyleDamage` (`None`/`Repaint`/
`Reflow`/`Rebuild`) via `diff_style`, and: `>= Reflow` → **full document
relayout**; `Repaint` → in-place paint-attribute update via `walk_mut`
(`apply_paint_only`, `lib.rs:531`) with no geometry recompute. There is **no
subtree-partial relayout, no fragment/layout-result cache, and no dirty-bit-driven
partial layout** — the paint-only fast path is the only sub-full-relayout tier.

**Fragmentation.** None — no pagination, multicol, or `break-inside`.

---

## 4. Fold-in recommendations (ranked by leverage)

1. **Confirm the `margin:auto`/`width` symptom against the WPT harness first — the
   block algorithm is already correct.** Then fix the real latent bug: **couple the
   `@media` `Device` viewport to the actual layout `viewport_width`**
   (`stylo_engine.rs:79`). This is a few-line change and is the most probable cause
   of the flush-left card at non-1024 widths. Low effort, high correctness payoff.

2. **Replace the isolated per-container Taffy calls with the Blitz/Servo trait
   model.** Implement `taffy::LayoutPartialTree` + `CacheTree` (+ `RoundTree`, flex/
   grid getter traits) over Manuk's own node tree, converting Stylo→`taffy::Style`
   once and caching it per node, and route flex/grid *item content* back through
   Manuk's block/inline via `compute_child_layout` + a **`MeasureFunction`**. This
   (a) fixes intrinsic sizing of auto flex/grid items, (b) makes nested flex/grid
   correct, and (c) gives Manuk taffy's **9-slot per-node cache for free** — the
   single biggest architectural leverage point. Keep hand-rolled block/inline
   (taffy's block layout does **not** model floats/BFC, which Manuk needs).

3. **Add intrinsic-size memoization.** A per-node min/max-content cache (à la
   Gecko's pointer-sized `IntrinsicISizesCache`, Ladybird's `Box::IntrinsicSizes`,
   or taffy's measure slots) keyed on available-space, invalidated on restyle. Kills
   the O(n²) `shrink_to_fit`/`cell_intrinsic` blowup. Medium effort, immediate perf.

4. **Split box tree from geometry to unlock incremental layout.** Adopt the WebKit
   LFC / Blink `LayoutResult` idea in lean form: an immutable box arena + a parallel
   geometry `Vec` indexed by node id, plus dirty bits and a **constraint-keyed cache**
   (Blink's `ConstraintSpace`-as-cache-key, `kHit/kNeedsSimplifiedLayout/kNeedsLayout`
   3-state test). This is the path from "full document relayout on any reflow" to
   subtree reuse. Higher effort; do after (2)/(3).

5. **Upgrade inline to Parley behind the measure seam** (as Blitz does): real
   shaping + bidi + Unicode line-break segmentation. Fixes the inter-word-space bug
   and non-Latin text. Higher effort — sequence after the taffy integration so the
   inline root is already a measurable leaf.

6. **Parent↔first/last-child margin collapsing**, then defer fragmentation.

**Is Taffy the right core?** Yes for **flex + grid** (both Servo and Blitz delegate
grid to taffy; taffy also does block-with-margin-collapsing). The mistake is *how*
Manuk uses it (throwaway per-container trees). Fix the integration, not the choice.
**Hand-rolling flex/grid (Ladybird-style) is BLOAT to avoid** — thousands of lines
of spec machinery duplicating a crate already in the dependency graph.

**Other BLOAT to avoid now:** full CSS fragmentation/pagination (break tokens,
continuations — Ladybird ships without it); a GC'd immutable fragment tree with dual
measure/layout cache slots (adopt the *constraint-keyed cache idea*, not Blink's full
apparatus); optimal/Knuth-Plass line breaking (greedy is fine); writing-modes /
vertical text until a real need exists.

---

## 5. Open questions

- **Incremental granularity vs. cost.** Is a constraint-keyed per-node layout cache
  (item 4) worth its complexity before Manuk carries interactive reflow load? Measure
  full-relayout latency on real pages first; item 3's intrinsic cache may capture
  most of the win at a fraction of the risk.
- **Unify on taffy's tree (Blitz) or keep Manuk's own block/inline?** Blitz puts
  *everything* behind taffy + Parley; but taffy's block layout omits floats/BFC,
  which Manuk models. Is the right long-term shape "taffy drives, Manuk's block/inline
  is a measured leaf FC" (max code reuse) or "Manuk drives, taffy is a per-FC solver
  via traits" (max control of floats)? Recommendation leans to the latter — but the
  boundary deserves a spike.
- **Reconciling the absolute-px single-pass model with a logical-coordinate future.**
  Gecko/WebKit/Servo all use writing-mode (ISize/BSize) coordinates. Retrofitting RTL
  and vertical text into Manuk's physical-px core is invasive; when is the right time
  to pay it, and can it be localized to inline layout?
- **Damage isolation boundaries.** Servo bounds relayout at independent-FC and
  body/root boundaries (`dom.rs:368`). What is the analogous minimal set of "relayout
  roots" for Manuk once box/geometry are split?
- **`@media`/viewport coupling correctness** (item 1): should cascade always take the
  live viewport, and how does that interact with the WPT harness's fixed probe width?

