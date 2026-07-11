# Manuk — Frontier Research Findings (V2)

**Status:** Answers to the open questions in `RESEARCH-PROMPT-V2.md`, grounded in the
cloned engine source (`file:line`), the 9 repomaps (`docs/parity/repomap/01–09`), and
current (2025–2026) industry/academic SOTA. Each area gives **(1) the decision**,
**(2) evidence**, **(3) cost/leanness tradeoff**, **(4) concrete next step**. The
ranked top-10 is at the end (§11).

North star unchanged: (a) real-site **visual parity** — largely DONE, Stylo is the
default cascade at 72/72; (b) **latency/snappiness**; (c) **agent-native ergonomics**.
Frame every decision against those three, in that priority.

---

## TL;DR of the decisions

1. **CSS:** Stylo is already the wired default and correct call. Do **not** grow
   MinimalCascade toward conformance; keep it only as the `--no-default-features`
   fallback. Next investment is *incremental* restyle (invalidation), not more coverage.
2. **Layout:** Replace throwaway per-container Taffy trees with the **Blitz model** —
   implement `LayoutPartialTree`/`CacheTree` over the arena, with a **measure seam**
   back into hand-rolled block/inline. This is the single biggest layout-correctness +
   perf lever. Add an intrinsic-size cache alongside.
3. **Text:** Ship **WOFF2** (pure-Rust: `brotli-decompressor` + a small glyf/loca
   reconstructor, ~600 LOC) — it is the #1 real-web font gap. Then a **shaped-word
   cache**. Do **not** adopt parley wholesale.
4. **Paint/GPU:** Two independent wins. (a) Wire the existing `damage_since` into a
   **persistent-texture partial upload** — biggest snappiness-per-effort, zero new deps.
   (b) Then a **Vello GPU tier** behind the `Painter` trait with `vello_cpu` as the
   headless fallback. Add a **minimal spatial tree** so scroll stops being a full repaint.
5. **Automation/a11y (the differentiator):** Expose the arena `NodeId` as a **stable
   agent handle**; add synchronous typed **`Readiness`**; keep a plain in-process a11y
   tree with dirty-tracking (no serializer). This is where Manuk beats Playwright — build
   the novel in-process capabilities (deterministic snapshots, semantic DOM diffing,
   per-node capability guards) no CDP stack can offer.
6. **JS bindings:** Kill the string-`eval` bindings (direct JSAPI), add a **small WebIDL
   generator** over a curated ~40-interface subset, give reflectors a **trace-based
   lifetime** (no cycle collector), per-interface prototypes. Keep SpiderMonkey.
7. **Event loop / latency:** Add a **frame-scheduled render loop with a dirty bit** and
   wire the GUI to the *existing* `relayout_incremental`/`apply_paint_only` fast paths.
   This is the direct fix for the latency complaint and needs almost no new code.
8. **Networking:** **Parallelize subresource fetches** (join_all), add a **preload
   scanner**, then an **in-memory HTTP cache** (Servo `http_cache.rs` design, top-frame
   partitioned).
9. **HTML/DOM:** Keep html5ever. Fix **context-aware fragment parsing**, add a
   **generational free list** to the arena, model **SVG/MathML namespaces** at the sink,
   add an **id→NodeId index**.

---

## 1. CSS — cascade, selectors, style computation

### 1.1 The pivotal fork: Stylo vs grow-MinimalCascade — **DECIDED: Stylo, already wired**

**Decision.** Keep Stylo as the runtime cascade (it is now the default at 72/72 per the
brief). Do **not** invest further in MinimalCascade as a conformance path — every hour
spent there is re-implementing Stylo by hand. Retain MinimalCascade *only* as the
`--no-default-features` fallback for fast unit tests and heavy-build-averse environments,
behind the existing `StyleEngine` trait boundary.

**Rationale / evidence.**
- The expensive 90% of the Stylo integration is already built and paid for: the
  `selectors::Element` wall (`engine/css/src/stylo_dom.rs:218`), the 107-method
  `TElement`/`TNode`/`TDocument` wall (`engine/css/src/stylo_traits.rs:141–426`), the
  matcher bridge, and the `ComputedValues → ComputedStyle` mapping
  (`engine/css/src/stylo_map.rs`) with a passing `var()`/inheritance/UA test
  (`stylo_engine.rs:305`). See repomap `01-css-cascade.md §3`.
- MinimalCascade's deficits are architectural, not incidental: selector matching is
  O(rules×elements) with **no bucketing and no ancestor Bloom** (`lib.rs:1544–1559`); no
  `@media`/`@supports`/`@layer`; `var()` unsupported; `background` shorthand collapsed to
  `background-color` (`lib.rs:1760`); approximate specificity (`lib.rs:618`). Reaching
  conformance means rebuilding `SelectorMap` + rule tree + invalidation — i.e. rebuilding
  Stylo (repomap `01 §3`).
- CLAUDE.md already names Stylo as the reuse target; the four production engines converge
  on the same bucket→right-to-left→Bloom→cascade-and-cache architecture Stylo embodies.

**Cost / leanness (measured locally + web-verified).** Enabling the `stylo` feature
(`engine/css/src/../Cargo.toml`: `stylo = ["dep:stylo","dep:url","dep:euclid",
"dep:selectors","dep:stylo_dom","dep:app_units","dep:stylo_static_prefs"]`) adds
**~70–90 crates** to the graph (`Cargo.lock` has 665 packages total). The weight is
**concentrated and specific**:
- **44 `icu_*` crates** — the single largest cost, pulled transitively by
  `icu_segmenter` (stylo depends on it with `features=["auto","compiled_data"]`). `compiled_data`
  bakes locale data as `const` blobs: per Mozilla's ICU4X docs this can add *tens of MB*
  to the binary and consume ~1 GB build-time RAM, though dead-code elimination reclaims
  unused locales, so a segmentation-only browser realizes well below the worst case.
  (https://firefox-source-docs.mozilla.org/intl/icu4x.html, unicode-org/icu4x#140)
- **~15–25 style-family crates** — `stylo`(=`style`), `selectors`, `servo_arc`,
  `cssparser`, `app_units`, `euclid`, `to_shmem`, `string_cache`, `web_atoms`, etc. Small,
  hand-written, negligible next to icu4x.
- **A mandatory Python 3 + Mako build step** — `stylo/style/build.rs` invokes
  `properties/build.py` and `panic!`s if `python3` is absent. This is a real toolchain
  requirement (not just Cargo). No nightly, **no C compiler** on the `servo` feature path
  (bindgen/C is `gecko`-only). Builds on stable Rust 1.80+.

Two notable findings: (1) the css crate depends on **stylo 0.19 from crates.io**, *not*
the vendored `/home/patrickd/manuk/stylo` checkout — there is no `[patch]` redirect, so
the local tree is a reading reference only. Confirm that's intended. (2) The icu4x
subtree is where the fat lives; if binary size becomes a concern, trim `icu_segmenter`
features or ship a custom `icu4x-datagen` slice.

Verdict: **worth it** — this is a one-time, already-accepted cost since Stylo is the
default parity engine, and no hand-written cascade reaches Firefox/Servo correctness.
Protect the leanness budget by declining *other* heavy deps (a second icu4x for text —
see §3 — WebRender, harfrust), not by second-guessing the cascade.

**Is a hybrid (Stylo compute + own selector fast-path) coherent?** *No — not worth it.*
Stylo's `SelectorMap` + `AncestorHashes` fast-reject is already the industry's best
selector fast-path; bolting a second matcher in front duplicates it and risks divergence
(a rule that matches in one but not the other). The only defensible hybrid is the
opposite: use Stylo for everything and treat MinimalCascade as a *fallback engine*, never
a front-end. Reject the hybrid.

**Next step (small).** Two things: (1) upgrade the Stylo entry point from
`compute_for_declarations` to the rule-tree path (`cascade_style_and_visited` /
`rule_tree().compute_rule_node`) so cross-element struct sharing and correct
origin/layer/`!important` come for free (repomap `01 §4.2`); (2) **couple the `@media`
`Device` viewport to the live layout `viewport_width`** — today it is hardcoded 1024×768
(`engine/css/src/stylo_engine.rs:79`), which is the most probable cause of responsive
mis-rendering at non-1024 widths (repomap `02 §4.1`). The viewport coupling is a few-line
change with high correctness payoff.

### 1.2 Selector matching at scale — which optimizations does Manuk actually need?

**Decision.** With Stylo as the runtime, Manuk **inherits** bucketing + `AncestorHashes`
Bloom + right-to-left matching for free (`stylo/style/selector_map.rs:122`,
`selectors/bloom.rs`). So the question is only about the *fallback* MinimalCascade and
about *invalidation*. For the fallback: leave it O(n) — it is a test/degraded path and
optimizing it is wasted effort. For the real path: the one thing Stylo does *not* give
Manuk automatically today is **incremental invalidation**, because the `TElement` wall's
`state`/snapshot/`borrow_data` methods are only partially real.

**Rationale / evidence.** Repomap `01 §5 Q4`: most `TElement` methods are `unimplemented!()`
on the `None`-cascade path (`stylo_traits.rs`); Stylo's own `InvalidationMap`/`RestyleHint`
cannot drive until `state`, snapshot, `has_animations`, and `borrow_data` are filled in.
Manuk currently re-cascades the **whole document** on any mutation
(`engine/page/src/lib.rs:497,636`). Style sharing (`StyleSharingCache`) and the rule cache
matter only once restyle is incremental and DOMs are repetitive.

**Cost / leanness.** Filling the invalidation-driving `TElement` methods is medium effort
but strictly additive (the wall exists). A hand-rolled invalidation layer over
MinimalCascade would be *more* work and would be thrown away. Prefer wiring Stylo's own
invalidation.

**Next step (medium, deferred behind interactivity).** Only once interactive mutation is
common: implement the ~4 `TElement` methods Stylo's `InvalidationMap` needs, then let a
class/attr/state change restyle only dependent subtrees. Until then, whole-document
re-cascade is acceptable for a read-mostly renderer. **Bloom filter / style sharing in the
fallback engine = premature; decline.**

### 1.3 Style sharing / rule-tree caching — worth it for the arena DOM?

**Decision.** Adopt Stylo's rule-tree path (§1.1 next step) which gives `Arc`-shared style
structs and the rule cache. Do **not** port the parallel rayon driver or
`insert_parents_recovering` — single-threaded sequential DFS keeps the Bloom trivial and is
plenty for a lean single-tab engine (repomap `01 §4 BLOAT`).

**Cost/leanness.** The rule tree is data-structure reuse already inside the `style` crate —
zero marginal dependency. The parallel driver is the part to decline.

---

## 2. Layout

### 2.1 The Blitz integration model — **DECIDED: adopt it. Highest layout leverage.**

**Decision.** Replace the current throwaway per-container Taffy trees with the
**Blitz/Servo model**: implement Taffy's `LayoutPartialTree` + `CacheTree` (+ `RoundTree`
and the flex/grid/block getter traits) **over Manuk's own arena/box tree**, convert
Stylo→`taffy::Style` **once and cache it per node**, drive one `taffy::compute_root_layout`,
and route flex/grid *item content* back into Manuk's hand-rolled block/inline layout through
a **`MeasureFunction`**. Keep the hand-rolled block/inline/float/table code — Taffy's block
layout does **not** model floats/BFC, which Manuk needs.

**Rationale / evidence.**
- Today `flex.rs` builds a **fresh `TaffyTree` per container**, extracts slots, then
  re-lays-out each child as a block; item sizes given to Taffy are only fixed `Px` or `None`
  (`engine/layout/src/lib.rs:1868`), so there is **no measure function** — auto/intrinsic
  sizing of flex/grid items is wrong and nested contexts re-solve in separate throwaway
  trees (repomap `02 §3`, `02 §5`).
- Blitz proves the correct wiring: `BaseDocument` (a flat node arena, exactly Manuk's shape)
  implements all Taffy traits — `TraversePartialTree`, `LayoutPartialTree`, `CacheTree`
  (delegating to a per-node Taffy `Cache` — a **free 9-slot cache per DOM node**) — and the
  inline root is a Taffy *leaf* whose measure closure runs the text layout
  (repomap `02 §2.6`; `blitz/packages/blitz-dom/src/layout/mod.rs`). Text/inline is **not**
  Taffy's job — it is a measured leaf.
- Taffy's measure seam is `compute_leaf_layout` taking
  `MeasureFunction: FnOnce(Size<Option<f32>>, Size<AvailableSpace>) -> Size<f32>`
  (repomap `02 §2.6`), with `RunMode`/`SizingMode` implementing the cheap size-vs-layout
  two-pass. This is exactly the hook Manuk needs to feed block/inline intrinsic sizes back
  to flex/grid.
- The 9-slot cache (`taffy/src/tree/cache.rs`, `CACHE_SIZE=9`: 1 final layout + 8 measure
  slots keyed by `compute_cache_slot(known_dims, available_space)`) is transparent to the
  algorithms because `compute_cached_layout` wraps every recursion (repomap `02 §2.6`). Manuk
  gets it for free by implementing `CacheTree`.

**Risk (the two-layout-models-fighting concern).** Blitz avoids it by making the boundary
crisp: **Taffy owns flex/grid/block-container geometry; the host owns leaf measurement.**
They never both size the same box. Manuk should adopt the same contract — Taffy drives
flex/grid containers, and every inline/replaced/float-containing subtree is a *measured
leaf FC* that Manuk's own code sizes. The one genuinely delicate seam is floats/BFC:
because Taffy has no float model, any container that establishes a BFC with floats should be
a Manuk-owned leaf, not a Taffy container. Keep that rule explicit.

**Cost / leanness (research-agent sized).** Taffy is **already a dependency** — this is a
*re-integration*, not a new dep. **Pin to Taffy 0.12.x** (current is 0.12.1, 2026-07-03;
the older "0.5–0.7" API in the brief is stale — style accessors are now GATs returning
*borrowed style views*, not `&Style`). Realistic total for a correct integration is
**~2,000–3,500 LOC**, dominated not by Taffy plumbing (small/boilerplate) but by two things:
(a) the **`manuk_taffy` style adapter** (~600–1000 LOC, the `stylo_taffy` analog — borrowed
views implementing `CoreStyle`/`FlexboxContainerStyle`/`GridContainerStyle` over Manuk's
computed style), and (b) **refactoring Manuk's block+inline engines to run in size-only
mode** (~800–1500 LOC — the actual fix for wrong intrinsic sizing). Trait impls on the
document (~300–500) + dispatch/measure seam (~200–400) + per-node `Cache` field and damage
pass (~150 trivial / ~500 with damage flags) round it out. Net: substantial, but it is the
single biggest layout-correctness + perf lever and much of it *is* the correctness fix, not
overhead.

**Next step.** Spike the trait impl over the arena with `taffy::compute_flexbox_layout`/
`compute_grid_layout`, wire inline/replaced leaves to Manuk's block/inline in size-only
mode as the measure closure, and verify flex/grid intrinsic-size cases against the WPT
harness. Exact 0.12 API in §2.4.

### 2.2 Split stable box tree vs immutable geometry — worth it before real perf data?

**Decision.** **Defer, but design toward it.** Do the Blitz integration (§2.1) and the
intrinsic-size cache (§2.3) *first* — they capture most of the win at a fraction of the
risk. The full box-tree/geometry split with a constraint-space-keyed 3-state cache
(Blink `LayoutResult` + `ConstraintSpace`, `kHit/kNeedsSimplifiedLayout/kNeedsLayout`) is
the right *eventual* architecture but is premature until Manuk carries interactive reflow
load and has `FrameTimer` p95 numbers on real pages.

**Rationale / evidence.** Every mature engine separates a stable structural tree from
immutable geometry results (Blink fragment tree, WebKit LFC `BoxGeometry` side-map, Servo
box-tree→fragment-tree, Ladybird `LayoutState`/`UsedValues`) — repomap `02 §2`. But the
value of a constraint-keyed cache only materializes with incremental reflow; Manuk's current
`relayout_incremental` does a **full document relayout** on any `>= Reflow` damage
(`engine/page/src/lib.rs:487`), so there is nothing yet to cache against. Measure first.

**Cost / leanness.** High effort (invasive to the whole layout core). The lean interim is
Taffy's per-node cache (§2.1), which is an incremental cache Manuk gets *for free*.

### 2.3 Intrinsic-size memoization — cheapest correct design

**Decision.** Add a **per-node min/max-content cache keyed on available-space**, invalidated
on restyle — the Gecko `IntrinsicISizesCache` / Ladybird `Box::IntrinsicSizes` model.

**Rationale / evidence.** Today `shrink_to_fit` (`engine/layout/src/lib.rs:1193`) and
`cell_intrinsic` (`:1430`) re-lay-children-out at width `1e6`/`0` **every call with no
memoization** — an O(n²) risk for nested shrink-to-fit and auto tables (repomap `02 §3`).
Gecko keeps this cache *pointer-sized* in the common case, spilling out-of-line only under
percentage-basis dependencies (`firefox/layout/generic/IntrinsicISizesCache.h:18`,
repomap `02 §2.2`).

**Cost / leanness.** Low–medium effort, immediate perf, no new dep. If §2.1 lands first, a
chunk of this comes free via Taffy's measure-slot cache; a small explicit cache still helps
the hand-rolled table/shrink-to-fit paths. **Do it either way.**

### 2.4 Taffy 0.12 API surface (research-agent verified)

Implement on Manuk's document struct (`NodeId` = arena index):
- **`TraversePartialTree`** — `child_ids`, `child_count`, `get_child_id` (3 trivial methods
  over the arena's child arrays); `TraverseTree` is an empty marker super-trait.
- **`LayoutPartialTree`** — associated `CoreContainerStyle<'a>: CoreStyle` view;
  `get_core_container_style`, `resolve_calc_value`, `set_unrounded_layout` (writes geometry),
  and `compute_child_layout(node, LayoutInput) -> LayoutOutput` (the dispatch hub).
- **`CacheTree`** (split out in 0.7) — `cache_get`/`cache_store`/`cache_clear` over a per-node
  `taffy::Cache` field.
- **`LayoutFlexboxContainer` + `LayoutGridContainer`** — 2 style-view accessors each. Do
  **not** implement `LayoutBlockContainer`'s compute path — Manuk's block engine owns block
  layout. `RoundTree` optional (pixel rounding).

**Measure seam:** `compute_child_layout` wraps `compute_cached_layout(...)` then matches node
kind: block/inline/float/table → Manuk's engines; flex → `compute_flexbox_layout`; grid →
`compute_grid_layout`; leaf/replaced → `compute_leaf_layout(inputs, &style, resolve_calc,
|known_dims, avail_space| -> Size<f32>)`. The closure is where Manuk's intrinsic sizing runs:
`MaxContent` → lay out at infinite width, return widest content; `MinContent` → break
everywhere, return longest unbreakable; `Definite(w)` → normal layout at `w`, return height.
Blitz's inline path calls `layout.calculate_content_widths() -> {min,max}` for exactly this
(`blitz-dom/src/layout/inline.rs`). **Critical rule:** the measure pass and the layout pass
must be the *same engine in two `run_mode`s* (`ComputeSize` vs `PerformLayout`), never two
implementations — that is how the two models avoid silently desyncing.

**The 9-slot cache:** `CACHE_SIZE=9` = 1 final-layout slot + 9 measure slots (a node is
probed under a bounded set of known-dims×available-space combos per intrinsic pass; 9 covers
the fan-out). Measure entries use a relaxed 64-bit `mixed_cache_key`; `PerformLayout` needs
an exact match. Turns flex/grid's multi-pass sizing from O(passes×subtree) to ~one measure
per distinct constraint. **Invalidation is the safety half** (Blitz `damage.rs`): a
style/DOM change that can affect a box's size must `cache_clear` that box and propagate
upward, else a stale measure slot desyncs the two engines. Start by clearing all caches each
layout (correct), add damage flags for incrementality later.

**Box-tree/geometry discipline (Servo layout_2020):** keep a *stable box tree* (survives
frames, owns style + structure + the `Cache`) separate from *immutable per-frame geometry*
(what `set_unrounded_layout` writes). Layout is a pure function box-tree→geometry; never
mutate the box tree during layout. This is what keeps the cache sound and makes incremental
relayout reachable (Servo is now landing incremental box-tree construction: PRs #37751/#37957/#38084).

---

## 3. Text

### 3.1 WOFF2 — **DECIDED: pure-Rust. `brotli-decompressor` + a glyf/loca reconstructor.**

**Decision.** Ship WOFF2 via a **pure-Rust** path — no C FFI needed. Two viable options,
in preference order: (A leanest) `woff2-patched` + `brotli-decompressor` as a
"WOFF2 bytes → sfnt bytes" shim feeding the existing swash pipeline; (B most robust)
**vendor allsorts' `src/woff2.rs`** (Apache-2.0, battle-tested in Prince/PDF) +
`brotli-decompressor`. Start with A; if real-font corpus turns up correctness gaps, switch
to B. Keep Google's C++ `woff2` (via the `woofwoof` FFI hybrid) only as a last-resort
compat fallback. Plain WOFF (zlib) is trivial and should land first.

**Rationale / evidence (research-agent verified, July 2026).**
- The hard part — the **glyf/loca transform reconstruction** — exists in maintained pure
  Rust today: `allsorts/src/woff2.rs` (allsorts **v0.17.0, May 2026**) decodes the
  `UIntBase128`/`255UInt16` varints, the point/flag/contour/bbox/instruction streams, and
  rebuilds head/maxp/hhea/loca — the exact thing that was previously "C++-only."
  `woff2-patched` (v0.4.0) does the same. (https://github.com/yeslogic/allsorts/blob/master/src/woff2.rs)
- **Brotli** in pure Rust is solid: Dropbox's `brotli-decompressor` (decode-only, safe by
  default, `no_std`-capable) is what allsorts uses. Depend on `brotli-decompressor`, **not**
  the heavier full `brotli` crate. (https://github.com/dropbox/rust-brotli-decompressor)
- **swash does not decompress WOFF2** — it operates on already-decompressed sfnt, so this
  shim is exactly the missing seam. **skrifa/fontations do not decode WOFF2 either** (open
  request googlefonts/fontations#1703) — but that's moot since Manuk is swash-based.
- The stock `woff2` 0.2/0.3 crate (the one that failed to build here) is abandoned;
  `woff2-patched` is its fix-fork. Avoid the original.

**Cost / leanness.** Pure-Rust path adds `brotli-decompressor` (small, safe, decode-only)
plus a few-hundred-LOC transform — **no C/C++ toolchain, no `cc` step, cross-compiles
cleanly, minimal binary-size impact**. This is the correct lean call. The C++ FFI route
(`woofwoof`) adds a C++ compiler requirement and mixed-language supply chain for only a
robustness edge on pathological fonts — decline unless pure-Rust visibly fails.

**Next step.** Add the shim at the web-font entry point (`engine/page/src/lib.rs:181/184`,
which today only accepts raw sfnt magic `\x00\x01\x00\x00`/`true`/`OTTO`/`ttcf`): detect
`wOF2`/`wOFF` magic, decompress to sfnt, hand to the existing `register_named_font` path.
~400–600 LOC vendored + one small dep. **This is the highest-value, smallest-scope text fix
— most real web fonts silently drop today** (repomap `03 §3 gap 1`).

### 3.2 Shaped-run / word cache — **DECIDED: add a Gecko-style word cache.**

**Decision.** Add an LRU keyed like cosmic-text's proven `ShapeRunKey` —
`{text, font family+weight+style+features, script, direction, language}` → `ShapedRun`,
**excluding pixel size** (shaping is size-independent up to hinting; scale advances
afterward — a real win) — plus a separate **codepoint-coverage cache** (memoizes "does this
font cover char X", which dominates fallback cost). Generational-LRU eviction (a global
`age` counter, refresh on hit) as cosmic-text does.

**Rationale / evidence.** Manuk caches advance widths (`measure_cache`) and glyph bitmaps
(`glyph_cache`) but has **no shaped-glyph-run cache** — `shape()` re-runs the shaper every
call (`engine/text/src/lib.rs:148`, repomap `03 §3 gap 2`). Layout measures the same words
many times per reflow; this is the single biggest text CPU win. Gecko's `mWordCache`
(`gfxFont.h:2299`) and Blink's three-tier `NGShapeCache`/`FrameShapeCache`/`ShapeResultView`
are the references, but for a single-tab engine a **plain LRU suffices** — do **not** port
Blink's GC'd `ShapeResult` graph (repomap `03 §4 BLOAT`).

**Cost / leanness.** Low effort, no new dep, reuses existing cache infrastructure.

### 3.3 UAX#14 line breaking + safe-to-break reshape — **DECIDED, with a shaper caveat.**

**Decision.** (a) Line breaking: adopt **`unicode-linebreak`** (~15 KiB, **zero deps**, full
UAX#14 pair-table algorithm — the Servo/cosmic-text choice). Flag `icu_segmenter` behind a
feature for dictionary-segmented scripts (Thai/Lao/Khmer/Burmese/CJK-no-space) *later* — and
note Stylo already pulls `icu_segmenter` transitively (§1), so if those scripts matter, reuse
that icu4x rather than paying twice. (b) **Safe-to-break edge reshaping requires a shaper
change** — see caveat.

**The load-bearing caveat (research-agent verified).** **Swash's shaper does NOT expose
HarfBuzz `unsafe_to_break` flags** — its `GlyphCluster` carries source range / ligature /
complex classification but no safe-to-break flag. So with a swash-only shaper Manuk **cannot**
do Blink's correctness-preserving edge-only reshaping; it can only approximate (reshape from
the previous word/grapheme boundary, usually-but-not-always safe). The whole Rust text
ecosystem (cosmic-text, parley) has moved shaping onto **`harfrust`/`rustybuzz`** (pure-Rust
HarfBuzz ports whose `GlyphInfo` *does* expose `unsafe_to_break()`) and kept swash **only for
rasterization**. The minimal correct algorithm: shape the paragraph once → find the UAX#14
break that fits → if the break cluster is safe, just *slice* the shaped run (zero reshape);
if unsafe, reshape *only* the boundary sub-range and splice.

**Decision on the shaper.** This is the single most consequential text architecture call.
**Recommendation: stay swash-only for now** (approximate reshape from word boundaries is fine
for a read-mostly Latin/space-delimited renderer and keeps the pipeline lean), and **switch
the shaper to `harfrust` if/when correct complex-script line layout or measured reshape cost
becomes a real bottleneck** — keeping swash for raster (the cosmic-text split). Do *not* pay
for harfrust speculatively. Swash's boundary pass-through (feed UAX#14 boundaries in via
`CharInfo`, read back `ClusterInfo.boundary()`) covers the approximate path today.

**Cost / leanness.** `unicode-linebreak` = 15 KiB, zero deps — trivially worth it. Harfrust =
a real shaper swap (deferred). icu_segmenter = only if dictionary scripts are in scope.

### 3.4 {script, locale}-aware fallback

**Decision.** Replace the fixed `FALLBACK_FAMILIES` const (`engine/text/src/lib.rs:120`)
with **coverage-driven fallback ordered by `(script, locale)`**, modeled on cosmic-text's
`Fallback` trait (`common_fallback` / `forbidden_fallback` / `script_fallback(script,
locale) -> &[&str]`). The walk: author family → script-specific → common → remaining
installed, testing codepoint coverage at each step (memoized by the coverage cache from
§3.2). **Lift cosmic-text's static tables** (MIT/Apache; they already track Chromium/Firefox
fallback lists) rather than authoring from scratch. Keep `locale` in the key specifically for
CJK Han disambiguation (zh-Hans/zh-Hant/ja/ko share codepoints, differ in preferred glyph).

**Cost / leanness.** Small borrowed static table + the coverage cache, no new dep. Fixes CJK
JP-vs-SC-vs-KR disambiguation (repomap `03 §3 gap 5`) and mixed-script runs.

### 3.5 Verdict on parley — **DO NOT adopt wholesale.**

The swash stack is sufficient; borrow parley's *ideas* (three-tier shape cache,
`{script,locale}` fallback, UAX#14 breaker) selectively. Adopting parley pulls
icu4x + fontique + harfrust and re-plumbs layout — large surface against the lean mandate
(repomap `03 §4`). Reserve as a drop-in only if complex-script line layout becomes a sink.

---

## 4. Paint / compositing / GPU

### 4.1 The sequencing decision — **architecture + damage FIRST, Vello LATER.**

**Decision (research-agent confirmed).** Do these in order:
1. **Build the retained-scene + minimal spatial/property tree + damage tracking** — steal
   WebRender's *three-tree architecture* (spatial/scroll/reference-frame nodes), not its
   renderer. This is the highest snappiness-per-effort item and a **prerequisite for both**
   rasterizer paths.
2. **Wire the existing `damage_since` into a persistent-texture partial upload** on the
   current tiny-skia CPU tier — kills the two current wastes (full re-raster + full
   re-upload) with **zero new dependency**. Best near-term ratio.
3. **Adopt a Vello GPU tier later**, behind the `Painter` trait, selecting
   `vello`(compute) / `vello_hybrid`(WebGL2) / `vello_cpu`(headless) per platform from one
   scene encoder — once Vello's quality gaps close.

**Rationale / evidence.**
- Manuk today CPU-rasterizes and **creates a brand-new texture + `write_texture`s the entire
  canvas every frame** (`shell/src/gui.rs:1443–1473`); `damage_since`/`Damage` exist and are
  unit-tested but are **not wired to present** (repomap `04 §3 gaps 2,3`). The partial-upload
  fix needs no new subsystem.
- Vello is **not production-ready as of mid-2026**: v0.9.0 (May 2026), still self-described
  alpha-maturing-to-beta, with **web-relevant quality gaps** — conflation artifacts
  (seams/hairline gaps at shared AA edges, issue #49, directly hurts box/border rendering),
  blur/`box-shadow`/filter effects still being implemented, and glyph caching/fine text AA
  "initial." The unified API was deferred; `vello_hybrid` gives **no API-stability
  guarantee**. Plan for churn if adopting now.
- `vello_cpu` is genuinely good — a SIMD sparse-strip CPU rasterizer, **likely the fastest
  Rust CPU renderer** (second only to Blend2D in Linebender's benchmarks, beating
  Skia/Cairo as geometry grows), a viable *superior* headless/deterministic fallback that
  could replace tiny-skia's software path later.
- WebRender's **spatial tree** is the prize to steal: `SpatialNode`s of {reference frame,
  scroll frame, sticky frame}; scroll = mutate one scroll-node offset, transform/opacity
  animation = mutate one bound property — **no re-tessellation, no content repaint**. This
  is orthogonal to the rasterizer and is the single biggest interactive-snappiness lever
  (research agent; `firefox/gfx/wr/webrender/src/spatial_tree.rs:147`, repomap `04 §2.2`).
- Adoption signal: Servo uses Vello for **canvas** (not yet whole-page); Blitz/Xilem use it;
  API still moving in 2026. Right *destination*, not yet the critical path.

**Cost / leanness.** Step 2 (partial upload) = **zero new deps**, modest localized effort,
10–100× idle/scroll frame cost reduction. Step 1 (spatial tree) = a small arena of
transform/clip/scroll node structs (Blink `cc/trees/*_node.h` are the simplest reference),
medium effort. Step 3 (Vello) = marginal weight over the existing wgpu stack for Vello's own
crates (+skrifa/peniko if you take its text), but adds an alpha dependency and a
compute-capable-GPU requirement — defer.

**Next step.** (a) Wire `DisplayList::damage_since`/`Damage::bounding` into
`shell/src/gui.rs:1443` to (i) skip upload when `!changed_since` and (ii) partial-upload only
the dirty sub-rect into a **persistent** texture. (b) Introduce a minimal spatial/scroll node
so `Viewport::scroll_by` becomes an offset update, not `Damage::full`
(`engine/compositor/src/lib.rs:374`). Both are pure wins on the existing CPU tier.

### 4.2 One IR for two rasterizers

**Decision.** Generalize `DisplayList` into an `anyrender::PaintScene`-shaped command trait
(`fill`/`stroke`/`draw_glyphs`/`draw_image`/`push_clip_layer`/`push_layer`/`pop_layer`) —
Ladybird's "record once, one pluggable player" seam (repomap `04 §2.5`). `CpuPainter`
(tiny-skia / later vello_cpu) stays the headless/CI tier; `VelloPainter` becomes the focused
GPU tier. Mind Vello's clip-depth cap (`blitz-paint/src/layers.rs:60`). This refactors
`engine/paint/src/lib.rs:317` with no new subsystem and future-proofs the split.

---

## 5. Automation & accessibility — the differentiator

**Framing (research-agent validated across every source).** CDP and WebDriver-BiDi are both
large, ongoing engineering efforts to make a **process boundary** tolerable; the frontier
agent tools (browser-use) are actively *fleeing the abstraction back toward the metal* (they
dropped Playwright for raw CDP, citing a Node relay hop that "incurs meaningful latency
across thousands of CDP calls," three-process state drift, and deadlocks). Manuk **deletes
the boundary** — its controller is an in-process Rust agent sharing `engine/page`. That turns
automation's hardest problems (readiness, node identity, observation cost, injection
provenance) from cross-process approximations into **local, synchronous,
correct-by-construction engine features**. This is the whole opportunity.

### 5.1 Stable node handle — **DECIDED: expose arena `NodeId` (Ladybird model).**

**Decision.** Expose the arena `NodeId` as the agent's stable handle, addressable as
`{ "node": 4213 }` alongside role+name and coordinates, with a cheap staleness check
(connected + document current). Do **not** invent a second opaque id space — CDP's
nodeId-vs-backendNodeId split is boundary-driven accidental complexity Manuk has no reason
to reproduce.

**Evidence.** Ladybird's WebDriver element reference *is* the DOM node's `UniqueNodeID`
stringified (`WebDriver/ElementReference.cpp:76`), with a staleness check (`:247`) — a
complete, lean addressing model. Manuk's arena already has `NodeId` and `A11yNode.node`
carries it (`engine/a11y/src/lib.rs:216`); it is simply not surfaced as a handle
(repomap `05 §3, §4.1`). This kills the "re-resolve the whole tree every call" fragility
(`AgentBrowser::resolve`, `agent/src/lib.rs:458`) and lets the agent reference *the node it
saw last turn* — the single biggest ergonomic gap vs CDP. Pair with a **generational
`NodeId`** (see §9) so a stale handle after slot reuse fails loudly rather than silently
aliasing.

### 5.2 Typed in-process bindings + readiness — **DECIDED.**

**Decision.** Formalize the in-process Rust API as the *primary* surface (microsecond
synchronous calls over the shared arena), with a tiny **role/name-first** skill set as the
ergonomic layer. Add an awaitable typed **`Readiness`** computed **synchronously from the
shared page** — the single highest-value new feature.

**Evidence / rationale.**
- Latency: `page.click('button')` in Playwright is JSON→WebSocket→Node relay→browser→
  Runtime.evaluate→box→Input.dispatch→ack (~ms of IPC per call, ×thousands per action);
  Manuk's is a function call over a shared arena — **microseconds, synchronous, no
  serialization** (repomap `05 §4.3`; research agent's browser-use latency data).
- **Readiness is the #1 flakiness source in all web automation.** Playwright's own docs
  concede network-idle "does not work for state updates triggered by client-side events,
  focus changes, or mount-time effects" and never settles in SPAs with long-lived
  websockets. BiDi *still lacks* an event distinguishing navigation *commit* vs *complete*
  (Playwright issue #32577). An in-process agent can block on the **real lifecycle signal**
  (style clean, layout clean, microtask queue drained, a *specific* fetch resolved) — a
  synchronous readiness primitive **inexpressible over any websocket**. Expose as awaitable
  Rust futures, strictly better than subscribing to your own events. Signals:
  `NodePresent(handle|role+name)`, `NavigationSettled` (real once the incremental parser +
  event loop land), `NetworkIdle` (once per-request hooks exist) — repomap `05 §4.4`.
- Role+accessible-name is the right primary locator vocabulary: semantic not structural
  (survives CSS/DOM refactors), matches how an LLM reasons ("the *Sign in* button"), and is
  injection-resistant in a way CSS selectors are not. Manuk **computes** the a11y tree
  internally, so it can offer role/name locators **correct-by-construction and synchronous**,
  with geometry/visibility fallback when a11y metadata is missing — curing the "legacy UIs
  render invisibly to the a11y tree" problem that sinks external a11y-first agents
  (research agent; Playwright `getByRole`). Keep an `nth-of-type`/structural CSS escape hatch
  backed by the Stylo matcher, but not as the primary mode.

### 5.3 The novel frontier — what no CDP/BiDi stack can do

**These are Manuk's differentiation surface** (research-agent synthesis, all grounded in
documented boundary-caused pain points):

1. **★ Synchronous truthful readiness** (§5.2) — attacks the #1 automation flakiness source;
   literally inexpressible over CDP/BiDi. Highest ROI.
2. **★ Race-free between-action semantic DOM diff** — because Manuk owns the arena, it can
   atomically snapshot the DOM *between* JS turns and compute "here is exactly what your last
   action changed" **in memory**, delivered as a compact causal observation. No CDP stack has
   a consistent global snapshot to diff — it has a mutation *stream* it must reassemble. This
   is what the web-agent literature (WebArena/WorkArena) lacks. Manuk's existing D2
   "turnkey cluster map" is the seed of this.
3. **★ Provenance/taint as a first-class DOM-node property + in-process Action-Guard.**
   Tag every node/text-run with its **origin at parse time** (which URL/frame, first- vs
   third-party, injected-by-script) — a real taint label baked into the node, exactly the
   CaMeL (DeepMind, arXiv 2503.18813) capability/IFC model that "practically solved" AgentDojo
   *without modifying the model*. The Action-Guard becomes an in-process reference monitor
   checking taint against policy synchronously before any side effect. A CDP client can only
   *approximate* provenance from a flattened serialized tree with origin structure already
   lost. This turns Manuk's E6 work (provenance-tagged observations + Action-Guard) into a
   defensible **security moat**. Encode Meta's "Rule of Two" (untrusted input / sensitive
   access / state change — at most two) as a mechanical action-layer check.
4. **★ Purpose-built, server-compressed agent observation** — prune/cluster/rank-by-
   interactability *before* it costs a token, solving the observation-bloat failure mode that
   CDP clients pay for only *after* serializing the whole tree. Manuk holds live DOM +
   computed style + layout geometry + a11y in one process, so it emits a purpose-built agent
   observation (not a repurposed screen-reader tree) that is simultaneously token-efficient,
   runtime-truthful, and geometry-aware — collapsing the vision/a11y-tree/runtime-DOM
   trilemma the field is stuck in.
5. **Occlusion-aware hit-test** — maintain a **flat z-ordered hit-test list** (Firefox
   `RemoteAccessible.cpp:690` insight) so `click_at` respects paint order, fixing the
   documented occlusion gap (`engine/a11y/src/lib.rs:25`). Mark each snapshot node
   `occluded`/`disabled`/`offscreen` so the agent never attempts an impossible click.

### 5.4 Cache the a11y tree, keep BiDi as interop only

**Decision.** Keep a plain in-process a11y tree with **dirty-tracking** (rebuild only when
DOM-dirty and layout is clean — Chromium's layout-gating discipline, `ax_object_cache_impl.h:992`);
**skip the serializer** (`AXTreeSerializer`, `AXTreeUpdate`, shadow client tree, mojo) —
all boundary-driven bloat Manuk has no boundary for (repomap `05 §4.2, §4.7`). Keep `bidi/`
as the **external interop** layer (drivable by existing Puppeteer/Selenium without CDP), not
the primary API — do not let BiDi conformance dictate the in-process API shape.

**Cost / leanness.** Removing the serializer is the single largest "bloat to skip." Dirty
tracking reuses the arena's existing mutation funnel + double dirty-bit.

---

## 6. JS engine & bindings

### 6.1 Keep SpiderMonkey; kill string-`eval` bindings first — **DECIDED.**

**Decision.** Keep SpiderMonkey via `mozjs` (correct for a Rust-native embedding; V8's Rust
binding is lower-level and Chromium-versioned — repomap `06 §4`). The **highest-leverage
cleanup** is replacing the string-`eval` bindings with direct JSAPI calls.

**Rationale / evidence.** Large parts of Manuk's "bindings" build JS source strings and
`eval` them: the identity cache (`new_reflector` → `eval_in_current_global(cx,
"…__nodes[{id}]=…")`, `engine/js/src/dom_bindings.rs:352`), the listener registry, event
dispatch, `getComputedStyle`, `getBoundingClientRect`, and the promise job enqueue
(`job_queue.rs:123`) all format-and-evaluate (repomap `06 §3 gap 2`). This is slow
(parse+compile per op), **fragile to any page that shadows `__`-prefixed globals or
`Array.prototype.push`**, and a latent injection surface. It is also a prerequisite for a
real lifetime story (you cannot trace edges that live inside eval-string state).

**Cost / leanness.** No new architecture — move existing behavior onto the JSAPI surface
Manuk already uses for `JS_SetElement`. Build the identity cache as a native
`HashMap<NodeId, Heap<*mut JSObject>>` (traced) or a JS `Map` via `JS::MapSet`; invoke
listeners with `JS::Call`; build `DOMRect`/computed-style with `JS_NewObject` +
`JS_DefineProperty`.

### 6.2 Small WebIDL generator, per-interface prototypes, trace-based lifetime

**Decisions.**
- **Build a *small* WebIDL generator** over a curated **~30–50 interface** subset (reuse the
  `weedle` WebIDL parser; target Manuk's reserved-slot reflectors + `mozjs`'s
  `ToJSValConvertible`/`FromJSValConvertible` traits + per-interface prototypes). Do **not**
  port Gecko's 25k-line `Codegen.py` or Blink's Mako pipeline. Hand-written bindings do not
  reach platform coverage — every shipping engine generates (repomap `06 §2, §4 R2`).
- **Per-interface prototype objects** (Ladybird `ensure_web_prototype`, `Intrinsics.h:66`):
  today methods are defined per-instance (N nodes × M methods, `dom_bindings.rs:350`); hang
  them off one shared prototype per interface. Falls out of the generator; fixes
  `instanceof`/`constructor` semantics pages test for (repomap `06 §4 R5`).
- **Trace-based reflector lifetime, NOT a cycle collector.** Manuk's arena DOM is
  `NodeId`-indexed, not refcounted, so the classic C++↔JS *cycle* largely dissolves — a JS
  wrapper holding a `NodeId` cannot form a native refcount cycle. The remaining job: (a) don't
  reuse a `NodeId` slot while a wrapper references it (→ **generational `NodeId`**, §9,
  shared with the agent-handle decision §5.1), and (b) trace wrapper→node reachability for
  detached subtrees via `mozjs`'s `CustomTrace`. This is dramatically simpler than Gecko's
  `nsCycleCollector` — **explicitly decline the cycle collector** (repomap `06 §4 R3`, `§5 Q1`).

**Cost / leanness.** Generator: a few hundred LOC + declarative IDL replaces ~10k lines of
hand-rolled `unsafe extern "C"`. `weedle` is a small dep. Reject: 25k-line codegen, cycle
collector, reimplementing hidden classes / JIT (SpiderMonkey provides these).

### 6.3 Minimal high-value Web API surface

**Decision (ranked).** `Event`/`CustomEvent` constructors + real `dispatchEvent(Event)`;
timers honoring delay ordering + `clearTimeout` (today delay is ignored,
`event_loop.rs:37`); `fetch` returning a **native `Promise`** (now unblocked by the working
job queue) + `Response`/`Headers`; `URL`/`URLSearchParams`, `TextEncoder`/`Decoder`,
**structured clone via SpiderMonkey's own `JS_StructuredClone`** (fixes History state +
`postMessage` in one move); **`MutationObserver`** — *disproportionately valuable for the
agent* as the native "DOM changed" primitive, reusing the microtask checkpoint. Agent-native
angle: because the controller is in-process it reads the arena directly — prioritize APIs
*pages* need to run, not a bindings-heavy CDP-style surface (repomap `06 §4 R4`).

---

## 7. Event loop & scheduling / latency

### 7.1 Frame-scheduled render loop with a dirty bit — **DECIDED. The direct latency fix.**

**Decision.** Replace "relayout+repaint inline on every input event" with: events set dirty
flags (`needs_layout`/`needs_paint`/`scroll_dirty`) and call `request_redraw()`; run the
actual pipeline **once** per frame in `RedrawRequested`, paced off winit `AboutToWait` / a
vsync-bound wgpu Fifo present. This is Ladybird's `needs_repaint()` gate + Blink's
`ScheduleAnimation` model, minus threads.

**Rationale / evidence.** Manuk's "event loop" is winit's OS loop; every interaction
(resize/scroll/keystroke/zoom) calls straight into `relayout_zoomed` + full-viewport repaint
synchronously (`shell/src/gui.rs:1130,1148,1043`), so **N input events in one frame = N full
pipelines** with no coalescing — the direct source of the latency complaint (repomap
`07 §3 gaps 1,2`). The fix needs almost no new code: the dirty/damage infrastructure already
exists (`relayout_incremental`, `RestyleDamage`, `apply_paint_only`, `changed_since`).

### 7.2 Wire the GUI to the existing incremental fast paths — **DECIDED, near-zero code.**

**Decision.** Route the GUI through the *already-existing* `relayout_incremental` +
`RestyleDamage` + `apply_paint_only` path instead of `relayout_zoomed`, so a keystroke that
only repaints doesn't relayout, and a color change is paint-only. Pure win using code that
already exists in `engine/page/src/lib.rs:489,533` but which the GUI currently bypasses
(repomap `07 §3 gap 5, §4②`).

### 7.3 GPU-composited scroll + minimal priority — **DECIDED.**

**Decision.** (a) Cache the painted surface in a persistent GPU texture; on scroll, change
the sampled offset in the present shader (shared with §4.1) — the WebRender/cc "off-main-thread
scroll" insight in a single-threaded budget, no compositor thread needed. Re-raster only the
newly-exposed band. (b) A **3-lane** priority scheme (input/scroll > rendering/rAF >
background) — do **not** build Blink's 11 levels or Gecko's full `TaskController` (repomap
`07 §4③⑤`). (c) A real single-threaded HTML event loop (one task → drain microtasks → once-per-
frame "update the rendering" in spec order) is needed only once JS/rAF/timers matter — model
on Ladybird `EventLoop::process()`.

**BLOAT to decline.** Full multiprocess site isolation (IPC hops *hurt* latency), per-frame/
per-page scheduler objects, an 11-priority sequence manager, a separate compositor *process*
(a worker *thread* — §4/§7④ — captures the win). Move raster off the UI thread only *after*
the dirty-bit + incremental + composited-scroll wins land (repomap `07 §4⑥`).

---

## 8. Networking & loading

### 8.1 Parallelize subresource fetches — **DECIDED. Highest-leverage, smallest change.**

**Decision.** Replace the serial `await` loops in `fetch_and_apply_stylesheets`
(`engine/page/src/lib.rs:667`) and `fetch_images` (`:99`) with concurrent issue —
`futures::future::join_all` or a bounded `buffer_unordered(6)`. The pooled H2 client already
multiplexes onto one connection; this turns N serial RTTs into ~1 (repomap `08 §3 gap 2,
§4.1`).

### 8.2 Preload scanner + in-memory HTTP cache — **DECIDED.**

**Decisions.**
- **Preload scanner** (highest *structural* win): as chunks arrive in `StreamParser`, extract
  `<link rel=stylesheet>`, `<script src>`, `<img src/srcset>`, CSS `@import`/`url()`,
  `<link rel=preload/preconnect>` and immediately kick off (parallel) fetches into a pending
  map keyed by URL; adopt the in-flight fetch when the DOM node is reached (WebKit's
  `m_preloads` model). Keep it minimal — a synchronous scan of each arriving chunk, no
  background thread initially. *Open question worth a spike:* Manuk's `StreamParser` is
  already incremental and cheap, so a **single pass emitting fetch-intents inline** may beat
  maintaining a separate scanner — modulo the `document.write`/`<base>`/CSP-nonce hazards
  that forced Blink to keep them separate (repomap `08 §5`).
- **In-memory HTTP cache** (skip disk first): port Servo's `http_cache.rs` nearly verbatim —
  RFC 9111 essentials (`max-age`/`Expires` freshness, heuristic freshness from
  `Last-Modified`, `ETag`/`If-None-Match` + `If-Modified-Since` → 304, `Vary` matching,
  `no-cache`/`no-store`). **Partition the key by top-frame origin from day one** (Chromium's
  double-key, `http_cache.cc:897`) — retrofitting partitioning is painful and it is a real
  privacy property. Makes repeat visits + back/forward nearly free (repomap `08 §4.3`).
- **Two-bucket prioritization** (blocking CSS/fonts before images) captures ~80% of Blink's
  `TypeToPriority` benefit for near-zero complexity; **honor page-declared
  `preconnect`/`preload`** hints. Defer HTTP/3 (repomap `08 §4.4–4.6`).

**BLOAT to decline.** A custom HTTP/1+2 stack, a bespoke socket pool, and a full disk-cache
(mmap chunk files, on-disk index, frecency eviction) — hyper/rustls already give pooling,
keep-alive, H2 multiplexing, Happy-Eyeballs, TLS resumption for free (repomap `08 §4`).

---

## 9. HTML parsing & DOM

### 9.1 Keep html5ever; harden the arena — **DECIDED.**

**Decision (ranked).**
1. **Keep html5ever.** The brief's "from-scratch parser" premise is the wrong direction —
   Servo ships html5ever in production; Ladybird's 8600-line Rust rewrite exists mainly for
   DOM/encoding integration Manuk doesn't need. html5ever gives adoption agency, foster
   parenting, active-formatting reconstruction, character references — thousands of
   html5lib-tested edge cases — for one `TreeSink`. **Already decided correctly** (repomap
   `09 §4.1`).
2. **Fix context-aware fragment parsing** (correctness, real sites). Replace the
   parse-as-document hack in `set_inner_html` (`engine/html/src/lib.rs:179`) with html5ever's
   `parse_fragment` + a context element. `el.innerHTML = "<tr>…"` / `"<td>…"` / `"<li>…"` /
   `"<option>…"` are extremely common in JS-driven pages and silently broken today.
3. **Generational free list on the arena** (the arena's one true weakness). Give `NodeId` a
   `{index, generation}`, keep a free list of detached slots, reuse them. Without this any
   long-running page **leaks arena slots** (`detach` unlinks but never reclaims,
   `engine/dom/src/lib.rs:182`). This is the change that makes the arena production-viable —
   and it is the *same* generational `NodeId` the agent stable-handle (§5.1) and reflector
   lifetime (§6.2) both need. **One change, three payoffs.**
4. **Model SVG/MathML namespaces at the sink** (correctness). Stop folding namespaces to
   local names (`engine/html/src/sink.rs:33`); carry html5ever's `QualName` namespace into
   `Attr`/`ElementData` (the `namespace` slot is already reserved, `dom/lib.rs:27`). Inline
   `<svg>` icons/logos mis-render today.
5. **id→NodeId index** so `getElementById`/`find_first` aren't O(n) DFS
   (`engine/dom/src/lib.rs:594`). A `HashMap<String, NodeId>` maintained in `set_attr`/`detach`.

**BLOAT to decline.** Blink's `BackgroundHTMLScanner` / Gecko's speculative stream parser
(off-main-thread *tree construction* with rollback — enormous complexity; Manuk's
`StreamParser` already gives streaming first paint); the `innerHTML` fast path; Oilpan/
refcounting for the DOM (the arena is the right lean call — harden it, don't abandon it)
(repomap `09 §4 BLOAT`).

---

## 10. Cross-cutting synthesis

### 10.1 Sequencing against the three complaints

Ordered by leverage against **(a) visual parity — mostly done**, **(b) snappiness**, **(c)
agent ergonomics**:

1. **(b, immediate, near-zero code)** Frame-scheduled dirty-bit render loop + wire the GUI to
   the existing `relayout_incremental`/`apply_paint_only` fast paths (§7.1–7.2). The single
   most direct fix for the latency complaint; uses code that already exists.
2. **(b, zero new deps)** Persistent-texture partial damage upload + minimal spatial/scroll
   node so scroll stops being a full repaint (§4.1). Biggest snappiness-per-effort after #1.
3. **(a, correctness)** Ship **WOFF2** (pure-Rust shim, §3.1) + fix the `@media` viewport
   coupling (§1.1). Web fonts are the most visible remaining parity gap; the viewport fix is
   a few lines.
4. **(b, network)** Parallelize subresource fetches (§8.1) — N serial RTTs → ~1.
5. **(a+b, structural)** Blitz-model Taffy integration + intrinsic-size cache (§2.1–2.4).
   Fixes flex/grid auto sizing and kills the O(n²) shrink-to-fit risk.
6. **(c, differentiator)** Stable `NodeId` agent handle + synchronous `Readiness` +
   role/name-first API (§5.1–5.2). Where Manuk beats Playwright.
7. **(c, novel)** In-process semantic DOM diffing + provenance-tagged Action-Guard (§5.3) —
   the defensible agent-native moat.

### 10.2 Leanness budget — value-per-weight ranking

**Accept (earns its weight):**
- **Stylo** — already the default; correctness unreachable by hand. The one big dep worth it.
- **`unicode-linebreak`** (15 KiB, 0 deps), **`brotli-decompressor` + vendored WOFF2 shim**
  (small, pure-Rust), **`weedle`** (small) — all cheap, high-value.
- **Taffy 0.12** — already present; re-integration, not a new dep.

**Decline (explicitly):**
- **A second icu4x for text** (parley/fontique/harfrust wholesale) — Stylo already pays for
  one icu4x; don't pay again. Borrow parley's *ideas*, not its stack.
- **harfrust shaper swap** — defer until correct complex-script line layout is a proven
  bottleneck; swash-only is fine for the Latin read-mostly case now.
- **WebRender** (steal the spatial-tree *architecture*, not the renderer), **Skia/Ganesh**,
  **Vello *right now*** (alpha, web-relevant quality gaps — adopt later behind the trait).
- **C++ WOFF2 FFI**, **cycle collector**, **25k-line WebIDL codegen**, **from-scratch HTML
  parser**, **multiprocess site isolation**, **disk HTTP cache**, **Blink's 11-priority
  scheduler**, **parallel Stylo rayon driver**, **Oilpan/refcount DOM**.

### 10.3 Verification — extend the WPT parity harness

Extend beyond the current 72/72 border-box probes to cover the new surfaces:
- **Responsive/`@media`** — probe at multiple viewport widths *after* the viewport-coupling
  fix (§1.1), asserting `@media`-gated rules fire correctly.
- **Web fonts** — a WOFF2 `@font-face` page asserting the web font actually loads and metrics
  match (guards §3.1).
- **Flex/grid intrinsic sizing** — auto/min-content/max-content item cases that are wrong
  today (guards the Taffy measure-seam integration §2.1–2.4).
- **Incremental correctness** — assert `apply_paint_only`/`relayout_incremental` produce
  pixel-identical output to a full relayout (guards §7.2).
- **Scroll** — assert composited-scroll output matches full-repaint (guards §4.1).

---

## 11. Top 10 highest-leverage moves for the best-possible Manuk

Ranked by value against the north star (visual parity mostly done → weight snappiness +
agent ergonomics), and by value-per-unit-effort.

1. **Frame-scheduled render loop with a dirty bit** + wire the GUI to the existing
   `relayout_incremental`/`apply_paint_only` fast paths. *(§7.1–7.2 · latency · near-zero new
   code · the most direct fix for the #1 complaint.)*
2. **Persistent-texture partial damage upload** — stop re-uploading the whole canvas each
   frame; wire the already-tested `damage_since`/`Damage` into present. *(§4.1 · latency ·
   zero new deps · 10–100× idle/scroll frame cost.)*
3. **WOFF2 web-font support** via a pure-Rust `brotli-decompressor` + vendored glyf/loca
   reconstructor (allsorts' `woff2.rs` or `woff2-patched`). *(§3.1 · visual parity · ~400–600
   LOC + one small dep · most real web fonts silently drop today.)*
4. **Couple the `@media` `Device` viewport to the live layout width** (drop the hardcoded
   1024×768). *(§1.1 · visual parity · a few lines · most probable cause of responsive
   mis-render.)*
5. **Parallelize subresource fetches** (`join_all`/`buffer_unordered(6)`). *(§8.1 · latency ·
   small change · N serial RTTs → ~1.)*
6. **Minimal spatial/scroll tree** so scroll and transforms are offset/property updates, not
   full repaints (steal WebRender's *architecture*, not its renderer). *(§4.1 · latency ·
   medium effort · biggest interactive-smoothness win after #1–2.)*
7. **Blitz-model Taffy 0.12 integration** — `LayoutPartialTree`/`CacheTree` over the arena
   with a measure seam into block/inline, converting Stylo→`taffy::Style` once per node.
   *(§2.1–2.4 · visual parity + perf · ~2–3.5k LOC · fixes flex/grid intrinsic sizing +
   free 9-slot cache; add the intrinsic-size cache alongside.)*
8. **Stable `NodeId` agent handle + generational `NodeId`** — one change that simultaneously
   gives the agent a durable handle (§5.1), the reflector lifetime story (§6.2), and arena
   slot reclamation (§9). *(agent ergonomics + correctness · one change, three payoffs.)*
9. **Synchronous typed `Readiness` + role/name-first in-process agent API** — replace
   network-idle heuristics with real lifecycle signals; the feature that beats Playwright on
   reliability and latency. *(§5.2 · agent ergonomics · inexpressible over CDP/BiDi.)*
10. **In-process semantic DOM diffing + provenance-tagged Action-Guard** — race-free
    "what did my last action change" observations and CaMeL-style parse-time taint labels as a
    capability-gated reference monitor. *(§5.3 · the novel, defensible agent-native moat no
    CDP-driven stack can match.)*

**Runners-up (do opportunistically):** kill string-`eval` JS bindings (§6.1); in-memory
top-frame-partitioned HTTP cache (§8.2); context-aware fragment parsing + SVG/MathML
namespaces at the sink (§9); shaped-word cache (§3.2).
