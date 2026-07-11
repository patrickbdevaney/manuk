# REPOMAP 01 — CSS: parsing, selector matching, the cascade, style computation

Comparative map of how production engines implement CSS style resolution, to steer
Manuk's lean-Rust engine. All paths absolute; line numbers verified against the on-disk
checkouts under `/home/patrickd/manuk/`.

---

## 1. Scope & sources

Directories/files inspected per engine:

- **Blink (Chromium)** — `chromium/third_party/blink/renderer/core/css/**` and
  `.../core/style/**`. Read: `css/selector_filter.h`, `css/rule_set.h`,
  `css/invalidation/invalidation_set.h`, `css/rule_feature_set.h`,
  `css/resolver/style_cascade.h`, `css/resolver/matched_properties_cache.h`,
  `css/resolver/style_resolver.h`, `style/computed_style.h`.
- **Stylo / Servo (Rust)** — `stylo/style/**` (this is Servo's `style` crate ~0.19; note the
  crate root is `stylo/style/`, not `stylo/`). Read: `bloom.rs`, `selector_map.rs`,
  `stylist.rs`, `rule_tree/{mod,core,level,source}.rs`, `properties/cascade.rs`,
  `properties/properties.mako.rs`, `sharing/mod.rs`, `rule_cache.rs`,
  `invalidation/element/{invalidation_map,restyle_hints}.rs`, `servo/restyle_damage.rs`,
  `driver.rs`, `parallel.rs`, plus `stylo/selectors/{matching,bloom}.rs`. Driven-in-practice
  reference: `blitz/packages/blitz-dom/src/{document,stylo}.rs`.
- **WebKit** — `WebKit/Source/WebCore/css/**` and `.../style/**`. Read:
  `css/SelectorChecker.h`, `css/SelectorFilter.h`, `style/RuleSet.h`,
  `style/ElementRuleCollector.h`, `style/RuleFeature.h`, `style/StyleResolver.h`,
  `style/MatchResult.h`, `style/PropertyCascade.h`, `style/StyleBuilder.h`,
  `style/StyleInvalidator.h`, `style/MatchedDeclarationsCache.h`, `style/MatchResultCache.h`,
  `style/computed/StyleComputedStyle*.h`.
- **Ladybird** — `ladybird/Libraries/LibWeb/CSS/**`. Read: `StyleComputer.{h,cpp}`,
  `SelectorEngine.cpp`, `Selector.h`, `ComputedProperties.h`, `CascadedProperties.{h,cpp}`,
  `InvalidationSet.h`, `StyleInvalidationData.h`, `StyleInvalidation.h`, `Invalidation/` dir.
- **Manuk** — `engine/css/src/{lib.rs,values.rs,stylo_engine.rs,stylo_map.rs,stylo_traits.rs,stylo_dom.rs}`,
  `engine/css/Cargo.toml`, and all cascade call-sites (`engine/page/src/lib.rs`, `engine/layout`,
  `engine/paint`, `shell/src/find.rs`, `tests/wpt`).

The four production engines converge on the **same three-part architecture**: (a) bucket
rules by rightmost simple selector into hash maps keyed on id/class/tag/attribute; (b)
match **right-to-left** with an **ancestor Bloom filter** fast-reject; (c) resolve the
cascade origin-by-origin and cache the result. They differ mainly in *what* they cache
(matched-declaration lists vs whole computed styles vs a shared rule tree) and in how
fine-grained their invalidation is.

---

## 2. Per-engine approach

### 2.1 Blink (Chromium)

**Selector matching.** Rules are bucketed in `RuleSet` (`css/rule_set.h:52`) into
`RuleMap id_rules_` (`:726`) plus `class_rules_`, `tag_rules_`
(`Find` accessors at `:420/:423/:433`), attribute, and pseudo-class buckets
(`link_/focus_/focus_visible_pseudo_class_rules_` `:454-463`). Each `RuleSet` carries a
`RuleFeatureSet features_` (`:417`). Matching is right-to-left with an **ancestor Bloom
filter**: `SelectorFilter::FastRejectSelector` (`css/selector_filter.h:129,167`) tests a
selector's precomputed identifier hashes (`CollectIdentifierHashes` `:132`) against the
filter of ancestor id/class/tag hashes before running the full checker. Blink additionally
maintains a per-element *subject* `TinyBloomFilter` (`:135`).

**Style invalidation — the Blink innovation.** `RuleFeatureSet` (`css/rule_feature_set.h`)
compiles each selector into **invalidation sets**: `InvalidationSet`
(`css/invalidation/invalidation_set.h:96`) with concrete subclasses
`DescendantInvalidationSet` (`:462`), `SiblingInvalidationSet` (`:473`),
`NthSiblingInvalidationSet` (`:546`). When a class/id/attribute/state changes on an element,
the engine looks up only the descendants/siblings that *could* be affected (or falls back to
`kInvalidateWholeSubtree`) instead of re-matching the document. This is the model Ladybird
and (partially) WebKit later adopted.

**Cascade & specificity.** `StyleCascade` (`css/resolver/style_cascade.h`) takes a
`MatchResult` (`:45`), then `Analyze` + `Apply` (`:110`, callable up to 15× for
interpolations/`revert`) over `cascade_origin.h` origins (UA/user/author) and `@layer`
priorities. `StyleResolver` (`css/resolver/style_resolver.h`) orchestrates matching →
cascade → build.

**Computed style storage & sharing.** `ComputedStyle` (`style/computed_style.h:223`) extends
generated `ComputedStyleBase` (`:195`); fields are grouped into ref-counted `DataRef` groups
(inherited / rare-inherited / rare-non-inherited) that are **shared copy-on-write** across
elements — the header explicitly says the split exists "to optimise" memory (`:195`). The
cross-element reuse cache is `MatchedPropertiesCache` (`css/resolver/matched_properties_cache.h`;
`Find` `:151`, `Add` `:156`, `CachedMatchedProperties::Entry`): elements whose matched
declaration set + parent style hash agree reuse a computed style. Note modern Blink has
**removed** whole-element "style sharing" (no `SharedStyleFinder` in this tree) in favour of
the MatchedPropertiesCache.

### 2.2 Stylo / Servo (the Rust reuse target)

**Selector matching.** `SelectorMap<T>` (`stylo/style/selector_map.rs:122`) buckets rules
into `id_hash`, `class_hash`, `local_name_hash`, `attribute_hash`, `namespace_hash`, and a
catch-all `other` (`:126-138`); `get_all_matching_rules` (`:204`) probes only the relevant
buckets. The matcher is the shared **`selectors` crate**: `matches_selector`
(`stylo/selectors/matching.rs:260`) → `matches_complex_selector` (`:445`), right-to-left over
`Combinator`. Fast-reject uses **`AncestorHashes`** packed into the selector
(`matching.rs:150-161`, `BLOOM_HASH_MASK = 0x00ffffff` at `selectors/bloom.rs:12`) checked
against `StyleBloom` (`stylo/style/bloom.rs:63`). Because Servo styles **in parallel**
(breadth-first work-stealing, not sequential DFS), the bloom needs
`insert_parents_recovering` (`bloom.rs:290`) to rebuild the ancestor stack when a worker
steals a subtree — a genuine subtlety absent in single-threaded engines.

**The rule tree — the core innovation.** `RuleTree { root: StrongRuleNode }`
(`stylo/style/rule_tree/core.rs:40`). Each `RuleNode` (`core.rs:207`) holds
`parent`, `source: Option<StyleSource>`, `cascade_priority`, `refcount: AtomicUsize`, and
`children: RwLock<Map<ChildKey, WeakRuleNode>>`. Matched rules are inserted least-specific
first (`insert_ordered_rules_with_important` `mod.rs:59`, splitting normal vs `!important`
into their correct cascade positions), and **common rule prefixes are shared across
elements** as a single path. Freed nodes go on a lock-free free list (`next_free`
`core.rs:265`) and are GC'd lazily (`maybe_gc`, `RULE_TREE_GC_INTERVAL = 300`
`core.rs:97,204`) so churny nodes like `:hover` survive to be re-used. `ComputedValues`
carries a back-pointer `rules: Option<StrongRuleNode>`.

**Cascade & computed values.** `properties::cascade` (`properties/cascade.rs:79`) →
`apply_declarations` (`:282`) walks the rule-node chain into a `StyleBuilder`
(`properties.mako.rs:2239`, `build() -> Arc<ComputedValues>` `:2621`). `ComputedValuesInner`
(`properties.mako.rs:1592`) holds **each style struct as its own `Arc<style_structs::X>`**, so
an unchanged struct (e.g. `Font`) is shared by pointer across many elements — copy-on-write at
struct granularity.

**Style sharing & rule caching.** `StyleSharingCache` (`sharing/mod.rs:574`) is a
32-entry LRU (`SHARING_CACHE_SIZE`) of candidate siblings/cousins; a candidate can donate its
whole style after cheap up-front checks (`sharing/checks.rs`) plus a **revalidation** step
(`revalidation_match_results` `mod.rs:268`) that re-runs only the stylist's *revalidation
selectors*. `RuleCache` (`rule_cache.rs:124`) memoizes computed values keyed on rule node +
conditions.

**Invalidation.** `InvalidationMap` (`invalidation/element/invalidation_map.rs:318`) maps
changed class/id/attr/state → dependent selectors (Blink-style sets); `RestyleHint` bitflags
(`restyle_hints.rs:13`: `RESTYLE_SELF/DESCENDANTS`, `RECASCADE_*`) and `ServoRestyleDamage`
(`servo/restyle_damage.rs:30`, `RELAYOUT`/`REPAINT`) drive the minimum re-work.

**Entry points & trait boundary.** `Stylist` (`stylist.rs:599`): `push_applicable_declarations`
(`:1711`, matching), `cascade_style_and_visited` (`:1480`, full per-element cascade →
`Arc<ComputedValues>`), and `compute_for_declarations` (`:1989`, cascade a standalone block,
**bypassing the rule tree**). Everything is generic over `E: TElement` (`stylo/style/dom.rs`),
which extends `selectors::Element`. **Blitz** drives exactly this: builds `Device` + `Stylist`
(`blitz-dom/src/document.rs:332,392`), feeds sheets via `append_stylesheet`, and reuses Stylo's
**parallel** driver `style::driver::traverse_dom` with its own rayon pool
(`blitz-dom/src/stylo.rs:150`).

### 2.3 WebKit

**Selector matching.** `SelectorChecker` (`css/SelectorChecker.h:50`) matches right-to-left
via `matchRecursively`/`checkOne` (`:139-140`); a four-state `Match` enum
(`SelectorMatches / FailsLocally / FailsAllSiblings / FailsCompletely` `:52`) gives precise
early-out along combinator chains. `SelectorFilter` (`css/SelectorFilter.h:40`) is the ancestor
Bloom: `CountingBloomFilter m_ancestorIdentifierFilter` (`:83`), `fastRejectSelector` (`:51`).
Rules bucket in `RuleSet` (`style/RuleSet.h:79`) as `AtomRuleMap` hash maps —
`m_idRules`, `m_classRules`, `m_attributeLocalNameRules` (+ lowercased variants for quirks),
`m_tagLocalNameRules`, `m_universalRules` (`:216-235`); `ElementRuleCollector`
(`style/ElementRuleCollector.h:50`) drives bucket probing and skips empty buckets inline
(`:144-155`).

**Invalidation.** `StyleInvalidator` (`style/StyleInvalidator.h`) plus per-change invalidators
(`ClassChangeInvalidation`, `IdChangeInvalidation`, `AttributeChangeInvalidation`,
`PseudoClassChangeInvalidation`, `ChildChangeInvalidation`) backed by `RuleFeatureSet`
(`style/RuleFeature.h:126`).

**Cascade & specificity.** `Style::Resolver` (`style/StyleResolver.h:91`) →
`styleForElement` (`:102`). `MatchResult` (`style/MatchResult.h:50`) keeps origin-separated
lists `userAgentDeclarations / userDeclarations / authorDeclarations` (`:56-58`), each
`MatchedProperties` tagged with `CascadeLayerPriority` (`:45`) and `IsCacheable`
(`Yes/No/Partially` `:47`). `PropertyCascade` (`style/PropertyCascade.h:42`) resolves winners,
with dedicated **rollback cascades** for `revert`/`revert-layer` (`:82,:98`) and logical-group
tracking (`:187`). `Style::Builder` (`style/StyleBuilder.h:42`) applies them.

**Computed style storage & sharing — two-tier caching (innovation).** Computed style lives in
`style/computed/StyleComputedStyle*` (the old monolithic `RenderStyle` split into DataRef-backed
COW sub-structs). Above it sit two caches: `MatchResultCache`
(`style/MatchResultCache.h:45`) reuses a *matched-declaration list* across inline-style tweaks
(`isUsableAfterInlineStyleChange` `:58`, exploiting `IsCacheable::Partially`), and below it
`MatchedDeclarationsCache` (`style/MatchedDeclarationsCache.h:41`) reuses a **whole computed
style** across elements with identical matched declarations (`find` `:63`, `add` `:64`,
`isCacheable` `:47`).

### 2.4 Ladybird

**Key finding: Ladybird is *not* a coarse full-recalc engine.** It has ported Blink/WebKit
machinery and sits architecturally close to WebKit, only leaner in storage.

**Selector matching.** `SelectorEngine::matches` (`SelectorEngine.cpp:1923`) is right-to-left,
starting at the last compound and recursing toward index 0 (`:1937`, combinator walk
`:1813-1872`) — directly analogous to WebKit's `matchRecursively`. Each `Selector` precomputes
`Array<u32,8> m_ancestor_hashes` (`Selector.h:283`). The ancestor Bloom lives in the style
engine: `CountingBloomFilter<u8,14> m_ancestor_filter` (`StyleComputer.h:32,254`),
push/pop during traversal (`StyleComputer.cpp:4469-4479`), queried by
`may_contain_ancestor_hash` (`:686`). Rules bucket in `RuleBuckets` (`rules_by_id/class/
tag_name/attribute_name/other` `StyleComputer.cpp:4487-4629`, probed by
`for_each_matching_rule_bucket` `:4637`). **Innovation:** a dedicated `:has()` fast-reject
filter (`should_reject_with_has_fast_reject_filter` `SelectorEngine.cpp:219`,
`collect_has_fast_reject_hashes` `:143-177`).

**Invalidation.** A near-complete Blink port: `InvalidationSet`
(`InvalidationSet.h:20`, `Type::{InvalidateSelf, InvalidateWholeSubtree, …}` `:23`),
`StyleInvalidationData` (`property_sets` / `match_set` / `InvalidationPlan`
`StyleInvalidationData.h:46-164`), and a whole `Invalidation/` directory of specialized
invalidators (Attribute, PseudoClass, ElementState, HasMutation, StructuralMutation,
MediaQuery, Slot, Part, FormControl, …). `RequiredInvalidationAfterStyleChange::full()`
(`StyleInvalidation.h:67`) is only a coarse *fallback*.

**Cascade.** `StyleComputer::compute_style` (`StyleComputer.cpp:2848`) →
`compute_cascaded_values` (`:2273`) calls `cascade_declarations` (`:952`) once per
origin/importance in spec order (UA→User→Author normal, then important reversed,
`:2282-2335`) with a cascade-layer loop, into a `CascadedProperties` winner map.
`sort_matching_rules` (`:812`) is a stable sort by specificity then source order.

**Computed style storage — the deliberate simplicity (good lean model).**
`ComputedProperties` (`ComputedProperties.h`) is a **flat, non-shared**
`Array<RefPtr<StyleValue const>, number_of_longhand_properties>` (`:352`) per element, with
parallel inheritance/importance bitsets — no COW DataRef grouping, only a cached font list.
There is **no whole-computed-style sharing cache** (no MatchedDeclarationsCache analog): rule
*matching* is cached (RuleCache + ancestor filter), but each element resolves and stores its
own style array. `revert` is handled inline (no rollback-cascade objects).

### 2.5 Cross-engine synthesis

| Concern | Blink | Stylo | WebKit | Ladybird |
|---|---|---|---|---|
| Rule bucketing | `RuleMap` id/class/tag | `SelectorMap` hashes | `AtomRuleMap` | `RuleBuckets` |
| Ancestor fast-reject | `SelectorFilter` TinyBloom | `StyleBloom` + `AncestorHashes` | `CountingBloomFilter` | `CountingBloomFilter<u8,14>` |
| Right-to-left match | yes | `selectors` crate | `SelectorChecker` | `SelectorEngine` |
| Fine-grained invalidation | InvalidationSets (origin) | InvalidationMap + RestyleHint | per-change invalidators | full Blink port |
| Computed-style reuse | MatchedPropertiesCache + DataRef COW | **rule tree** + Arc structs + sharing cache | MatchedDeclarationsCache + MatchResultCache | **none** (flat array/elem) |
| Parallel styling | worklets/threaded | **rayon driver** | limited | no |

---

## 3. Manuk today (honest assessment)

**Two engines exist; the fast one is dead code.**

- **`MinimalCascade`** (`engine/css/src/lib.rs:1467`) is the *only* engine the product uses.
  Every pipeline call-site hardcodes it: `engine/page/src/lib.rs:369,381,436,498,636,947`,
  plus `engine/layout`, `engine/paint`, `shell/src/find.rs`, `tests/wpt`.
- **`StyloEngine`** (`engine/css/src/stylo_engine.rs:73`) is a **real** cascade —
  `cascade_via_stylo` (`:151`) builds a `Device`+`Stylist`, parses each sheet with Stylo's own
  parser, matches with Stylo's `matches_selector` over the arena DOM (via the fully-implemented
  `selectors::Element` wall in `stylo_dom.rs:218` and the 107-method `TElement`/`TNode`/
  `TDocument`/`TShadowRoot` wall in `stylo_traits.rs:141-426`), merges winners by
  (specificity, order), inline `style=` last, computes with `compute_for_declarations`
  (`:294`), and maps `ComputedValues → ComputedStyle` in `stylo_map.rs`. It has a passing
  end-to-end test that exercises `var()`, inheritance, and UA defaults
  (`stylo_engine.rs:305`). **The module-level doc comment claiming it "delegates to
  MinimalCascade" is stale** — the code no longer delegates.
- **The gap is Step 5 of `docs/parity/STYLO-CASCADE-PLAN.md`: wiring.** Steps 1-4 (the trait
  wall, the matcher, the compute call, the value mapping) are all landed. What is missing:
  (a) no crate enables the `stylo` cargo feature — `engine/css/Cargo.toml` gates all Stylo code
  behind `default = []` / `stylo = [...]`, and `engine/page/Cargo.toml` has **no** `stylo`
  feature forwarding to `manuk-css/stylo`; (b) `StyloEngine` is referenced **nowhere** outside
  the css crate (grep confirms zero hits). So it is doubly dead: not compiled in the default
  build, and never selected even when it is. It also runs **unconditionally single-threaded and
  rule-tree-free** (`compute_for_declarations`, not `cascade_style_and_visited`), so it forgoes
  Stylo's headline optimizations.

**MinimalCascade's design and concrete deficits vs. production engines:**

- **Selector matching is O(rules × elements) with no indexing.** `cascade_node`
  (`lib.rs:1544-1559`) loops every rule in every sheet against every element. There is **no
  rule bucketing** (no id/class/tag hash maps) and **no ancestor Bloom filter** — the single
  biggest structural gap vs. all four engines. On real pages (hundreds of rules × thousands of
  elements) this is quadratic.
- **Combinator matching is greedy with no backtracking** (`lib.rs:842-897`, acknowledged in
  the comment): a pathological descendant/sibling selector can false-negative.
- **This is why author box rules drop.** The reported `div { width; margin:auto; … }` failure
  on example.com is a *matching/robustness* bug in this subset engine, not an architectural
  inevitability — but the subset is small enough that such misses are expected.
- **No custom properties / `var()`** (values.rs supports hex/rgb/hsl/named/`calc()` linear/
  em-rem, but not `var()`, `env()`, or `currentColor` resolution).
- **No `@media`, `@supports`, `@import`, `@keyframes`** — `@font-face` is captured; all other
  at-rules are skipped (`lib.rs:997`, `skip_at_rule`).
- **`background` shorthand is treated as `background-color` only** (`lib.rs:1760`) — images,
  gradients, position, repeat all dropped.
- **Cascade origin handling is thin.** UA is folded in as imperative `apply_ua_defaults`
  (`lib.rs:1593`) rather than a low-priority origin; there are **no cascade layers (`@layer`)**,
  **no user origin**, and `!important` is a naive "apply important decls last" pass
  (`lib.rs:1572`) with no UA-important-wins-over-author-important reversal and no `revert`.
- **Specificity is approximate** (`lib.rs:618`): pseudo-elements not counted; `:not()` inner
  specificity not contributed; caps each component at 255.
- **No computed-style sharing or caching of any kind** — no rule tree, no
  MatchedPropertiesCache analog, no sibling sharing. Every restyle re-cascades from scratch:
  `engine/page/src/lib.rs` rebuilds sheets and calls `MinimalCascade.cascade` over the **whole
  document** on mutation (`:497,:636`), and even re-parses selector strings per
  `matches_selector` call.
- **Invalidation is absent at the style layer.** The `RestyleDamage` taxonomy + `diff_style`
  (`lib.rs:481-530`) exist for *layout/paint* damage, but there are **no invalidation sets** —
  a class toggle triggers a full re-cascade, not a targeted subtree.

What MinimalCascade does well for a lean browser: it is ~2800 lines, has zero heavy
dependencies, handles the flat tree / shadow DOM scoping correctly (`cascade_scoped`
`lib.rs:1516`, `ScopedSheet`, `::slotted`), and covers a broad, pragmatic property set
(flex, grid, transforms, tables, box model).

---

## 4. Fold-in recommendations (ranked by leverage)

**Headline decision: FINISH WIRING STYLO — do not keep growing MinimalCascade as the
conformance path.** The expensive 90% (the `TElement` wall, the selector bridge, the value
mapping, a passing test) is already built and paid for. The remaining work is Step 5:
one cargo-feature forward in `engine/page/Cargo.toml` (`stylo = ["manuk-css/stylo"]`) and a
`StyleEngine` selection at the pipeline call-sites. This immediately buys `var()`, `@media`,
`@supports`, cascade layers, spec-correct specificity/`!important`/`revert`, the full selector
grammar, and `background`/shorthand coverage — none of which MinimalCascade will ever reach
without re-implementing Stylo. Growing MinimalCascade toward conformance is rebuilding Stylo by
hand; CLAUDE.md already names Stylo as the reuse target.

**Keep MinimalCascade as the fallback**, not the default: it stays valuable for
`--no-default-features` builds, fast unit tests, and environments where the heavy Stylo build
is undesirable. The `StyleEngine` trait boundary already makes this a config choice.

Ranked actions:

1. **(Highest) Wire `StyloEngine` into the page pipeline behind a `stylo` feature and make it
   the default for real rendering.** Fixes the dropped-`div`-box class of bugs at the root
   (correct matching + full property set), not case-by-case. Verify against the layout-parity
   harness (must stay ≥ current pass count) and add the cascade tests the plan lists
   (`var()`, `@media`, child/attribute selectors).
2. **Upgrade the Stylo entry point from `compute_for_declarations` to the rule-tree path**
   (`cascade_style_and_visited` / `rule_tree().compute_rule_node`, Plan Step 3 alt). Unlocks
   cross-element structural sharing and correct origin/layer/`!important` resolution for free.
3. **If (1) is deferred, the single highest-leverage MinimalCascade fix is rule bucketing +
   an ancestor Bloom filter.** Port the universal design: hash rules by rightmost
   id/class/tag (mirror `SelectorMap`/`RuleSet`) and precompute per-selector ancestor hashes
   with a counting Bloom filter. Turns the O(rules×elements) scan into near-linear and is the
   one optimization all four engines share.
4. **Add invalidation sets** (Blink/Ladybird model) so a class/attr/state change restyles only
   dependent subtrees instead of the whole document — pairs with the existing `RestyleDamage`
   plumbing. Medium leverage; only matters once interactivity/mutation is common.
5. **Add a MatchedProperties-style computed-style cache** keyed on (matched declarations,
   parent style) — cheap once matching is bucketed; big win on repetitive DOMs (lists, tables).

**Explicit BLOAT to avoid** for a lean browser:
- **Quirks mode / limited-quirks** and the lowercased-vs-exact attribute/tag rule-map
  duplication WebKit carries (`RuleSet.h:216-222`). Ship `NoQuirks` only.
- **Parallel styling (rayon driver, `StyleBloom::insert_parents_recovering`, per-thread TLS).**
  Stylo's parallelism is real but is a large complexity/coordination cost; a single-threaded
  sequential DFS keeps the Bloom filter trivial (plain ancestor stack) and is plenty for a lean
  engine. Adopt the *data structures* (rule tree, sharing cache) without the *parallel driver*.
- **The two-tier WebKit cache (`MatchResultCache` + partial cacheability)** — the second tier
  is a micro-optimization for inline-style animation; one MatchedProperties cache suffices.
- **Legacy pseudo/`-webkit-`/XBL/`-moz-` surface, `revert-layer` rollback cascades, presentational
  hint mapping beyond the few HTML attributes already handled.** Match the modern subset.
- **Visited-link styling machinery** (`CascadeMode::Visited`, `visited_style`) — a privacy
  feature with a dedicated cascade; skip until needed.

---

## 5. Open questions for the frontier-research phase

1. **Build cost & binary size of enabling Stylo by default.** Does the `stylo` feature's
   compile time / dependency weight (cssparser, selectors, servo_arc, app_units, euclid, style)
   violate Manuk's "lean" budget? Measure before defaulting it on.
2. **Rule tree vs. Manuk's damage model.** Stylo's rule-tree GC (`RULE_TREE_GC_INTERVAL`) and
   Arc-shared structs assume long-lived documents with incremental restyle. How do they interact
   with Manuk's current "re-cascade whole document on mutation" pipeline — is incremental restyle
   a prerequisite to getting value from the rule tree, or can it help a one-shot render too?
3. **Single-threaded Stylo throughput.** With the rayon driver deliberately omitted, is Stylo's
   sequential cascade fast enough on large pages, or does the abstraction overhead make a
   bucketed MinimalCascade competitive for the common case?
4. **How much of the `TElement` wall must become *real* for incremental restyle/invalidation?**
   Today most methods are `unimplemented!()` on the `None`-cascade path (`stylo_traits.rs`).
   Which specific methods (`state`, snapshot, `has_animations`, `borrow_data`) must be filled in
   to drive Stylo's own `InvalidationMap`/`RestyleHint`, and is that cheaper than a hand-rolled
   invalidation layer over MinimalCascade?
5. **`:has()` cost.** Ladybird invested in a dedicated `:has()` fast-reject filter. Does Stylo's
   relative-selector invalidation (`invalidation/element/relative_selector.rs`) come "for free"
   once wired, or is `:has()` a separate performance cliff Manuk must budget for?
6. **Value-mapping fidelity.** `stylo_map.rs` maps ~30 properties and approximates `line-height:
   normal` as `1.2×` (needs font metrics we stub). Which computed values silently lose precision
   through the `ComputedValues → ComputedStyle` reduction, and does layout/paint parity depend on
   any of them (e.g. anchor-positioning `_ =>` fallbacks, calc mixed length%)?
