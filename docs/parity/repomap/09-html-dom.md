# REPOMAP 09 — HTML parsing & the DOM tree

Comparative map of how Blink, Gecko, WebKit, Servo, and Ladybird implement the
HTML5 tokenizer + tree construction and the DOM tree they build — to guide
Manuk's arena DOM and its html5ever-driven parser.

> **Framing correction (read first).** The Wave-2 brief calls Manuk's parser
> "from-scratch." It is **not**: `engine/html/src/lib.rs:3` reuses **html5ever**
> (Servo's spec-compliant tokenizer/tree-builder) and drives it into the arena
> DOM through a custom `TreeSink` (`engine/html/src/sink.rs`). What *is*
> from-scratch is the **arena DOM** (`engine/dom/src/lib.rs`) and the `TreeSink`
> adapter. This distinction changes the recommendations in §4 substantially:
> the "should Manuk adopt html5ever?" question is already answered *yes* — the
> live questions are about the sink's fidelity and the arena DOM's tradeoffs.

---

## 1. Scope & sources

| Engine | Paths (all under `/home/patrickd/manuk/`) |
|---|---|
| **Blink** | `chromium/third_party/blink/renderer/core/html/parser/` — `html_document_parser.{cc,h}`, `html_tokenizer.{cc,h}`, `html_tree_builder.{cc,h}`, `html_document_parser_fastpath.{cc,h}`, `background_html_scanner.{cc,h}`, `html_preload_scanner.*`, `html_tree_builder_adoption_agency_test.cc`. DOM: `core/dom/` — `node.{cc,h}`, `element.{cc,h}`, `container_node.{cc,h}`, `element_data{,_cache}.*`, `node_rare_data.*` |
| **Gecko** | `firefox/parser/html/` — `nsHtml5TreeBuilder.{cpp,h}` (+`CppSupplement`/`HSupplement`), `nsHtml5Tokenizer.cpp`, **`nsHtml5TokenizerSIMD.cpp`** + `nsHtml5TokenizerALU.cpp`, `nsHtml5StreamParser.*`, `nsHtml5Speculation.*`, `nsHtml5SpeculativeLoad.*`, `nsHtml5AtomTable.*`. DOM: `firefox/dom/base/` — `nsINode.{h,cpp}`, `Element.*`, `FragmentOrElement.*` |
| **WebKit** | `WebKit/Source/WebCore/html/parser/` — `HTMLDocumentParser.*`, `HTMLTokenizer.*`, `HTMLTreeBuilder.*`, `HTMLConstructionSite.*`, `HTMLElementStack.*`, `HTMLFormattingElementList.*`, `HTMLStackItem.h`, `HTMLDocumentParserFastPath.*`. DOM: `WebKit/Source/WebCore/dom/` — `Node.*`, `Element.*`, `ContainerNode.*` |
| **Servo** | `servo/components/script/dom/servoparser/` — `mod.rs`, `html.rs` (html5ever driver), `async_html.rs` (off-main-thread parser), `prefetch.rs`, `xml.rs`. DOM: `servo/components/script/dom/node/node.rs`, `dom/element/`, `dom/document/`. Parser lib: **html5ever** (crates.io) |
| **Ladybird** | `ladybird/Libraries/LibWeb/HTML/Parser/` — `HTMLParser.{cpp,h}`, `HTMLTokenizer.*`, `HTMLToken.*`, `SpeculativeHTMLParser.*`, `IncrementalDocumentParser.*`, **`Rust/`** (a new pure-Rust tokenizer+parser: `src/parser.rs` 5369 L, `src/tokenizer.rs` 3295 L). DOM: `ladybird/Libraries/LibWeb/DOM/` — `Node.*`, `Element.*`, `Document.*`, `ContainerNode` |
| **Manuk (subject)** | `engine/html/src/lib.rs` (html5ever driver + `StreamParser` + serialization + `set_inner_html`), `engine/html/src/sink.rs` (`ArenaSink: TreeSink`), `engine/dom/src/lib.rs` (arena `Dom`, `NodeId`) |

---

## 2. Per-engine approach

All five engines implement the **same WHATWG HTML5 spec**: a state-machine
tokenizer feeding a tree-construction stage with ~23 insertion modes, a stack of
open elements, a list of active formatting elements, the adoption-agency
algorithm (misnested `<b><i></b></i>`), and foster parenting (mis-nested table
content). The interesting differences are in *threading*, *node representation*,
and *how parsing interleaves with script*.

### Blink (Chromium) — the speculative/background-parsing reference

- **Tokenizer.** `HTMLTokenizer` (`html_tokenizer.cc`) is the flat WHATWG state
  machine; tag/attribute names come from a JSON5 table (`html_tokenizer_names.json5`).
- **Tree builder.** `HTMLTreeBuilder` holds `enum InsertionMode` with all modes
  (`html_tree_builder.h:138`: `kInitialMode`, `kBeforeHTMLMode`, `kInTableMode`,
  …). `ResetInsertionModeAppropriately()` (`.cc:416`), foster/format recovery
  via `ReconstructTheActiveFormattingElements()` (`.cc:921` and many call sites),
  and the adoption agency at `CallTheAdoptionAgency()` (`.cc:1751`, dispatched at
  `:2264`). There is a **dedicated adoption-agency test suite**
  (`html_tree_builder_adoption_agency_test.cc`) — a signal of how load-bearing
  and bug-prone this algorithm is.
- **Background parsing (Blink's signature innovation).** `BackgroundHTMLScanner`
  (`background_html_scanner.h:42`) runs on a **worker thread**, scanning *all*
  body bytes (unlike the main-thread `HTMLPreloadScanner`, which scans only the
  first chunk). Its documented job (`background_html_scanner.h:25-41`) is to find
  inline `<script>` early and **kick off streaming V8 compile tasks** so the
  compiled script is ready by the time the main-thread parser reaches it. Preload
  scanning (`html_preload_scanner.*`) speculatively fetches subresources ahead of
  the tree builder.
- **Fast path.** `html_document_parser_fastpath.cc` is a **non-spec optimized
  parser** for simple `innerHTML` fragments (no scripts, no weird nesting) that
  skips the full state machine — a big real-world win because `innerHTML` is hot.
- **Cooperative yielding.** `PumpTokenizer()` (`html_document_parser.cc:626`)
  processes tokens under a **time budget** (`GetTimedBudget`, `:217`;
  `kNumYieldsWithDefaultBudgetValue`) then yields to the scheduler
  (`SchedulePumpTokenizer`, `DeferredPumpTokenizerIfPossible` `:579`) so parsing a
  huge document never blocks the main thread's event loop.
- **document.write / blocking scripts.** A synchronous inline `<script>` blocks
  the tokenizer; the tree builder runs the script, whose `document.write()`
  re-enters the input stream at the current insertion point. Blink tracks nesting
  and re-pumps.

### Blink DOM tree — pointer tree + Oilpan GC + rare-data

- **Node model.** `Node` is **garbage-collected (Oilpan)**, not refcounted.
  Sibling/parent links are `Member<>` GC pointers. A space optimization: the
  parent pointer is **tagged** — `TaggedParentOrShadowHostNode
  parent_or_shadow_host_node_` (`node.h:1438`) packs "parent" vs "shadow host"
  into one slot. `previousSibling()` uses a **circular** `previous` link on the
  first child (`node.h:1443`, `PreviousSiblingCircular`) so `lastChild` is O(1)
  without a separate `last_sibling` field.
- **Rare data.** Rarely-used per-node state (id, dataset, event listeners, shadow
  data) lives in a lazily-allocated `NodeRareData` (`node.h:1368`,
  `EnsureRareData`) so the common `Node` stays small.
- **ContainerNode fast paths.** `first_child_`/`last_child_` on `ContainerNode`
  (`container_node.h:477`); `HasOneChild()`, `HasOneTextChild()` special-cases
  (`:119,126`); bulk mutations stage into a `NodeVector`
  (`HeapVector<Member<Node>, 11>`, `:90`).
- **Element attribute sharing.** `ElementData` is **immutable + shareable**
  across elements with identical attribute sets via `ElementDataCache`
  (`element_data_cache.h`) — cuts memory for pages with thousands of
  `class="..."`-identical elements.

### Gecko (Firefox) — the Java-translated tree builder, now SIMD-tokenized

- **Provenance.** `nsHtml5*` is **machine-translated from the Java `htmlparser`
  reference implementation** (validator.nu). `nsHtml5TreeBuilder.cpp` therefore
  reads like generated code, with all logic in `resetTheInsertionMode()`
  (`:180`), `reconstructTheActiveFormattingElements()` (`:287`),
  `adoptionAgencyEndTag()` (`:1244`) — spec algorithm names preserved verbatim.
- **SIMD tokenizer (recent innovation).** The tokenizer now has three back-ends:
  `nsHtml5Tokenizer.cpp`, `nsHtml5TokenizerALU.cpp`, and **`nsHtml5TokenizerSIMD.cpp`**
  with policy headers (`nsHtml5TokenizerLoopPoliciesSIMD.h`) — vectorized fast
  scanning of runs of plain text/attribute chars.
- **Speculative parsing on a stream thread.** `nsHtml5StreamParser` runs the
  tokenizer + tree builder **off the main thread** and *speculatively* continues
  past `<script>` boundaries. `nsHtml5Speculation` records a checkpoint; if a
  script's `document.write` invalidates the speculation, Gecko **rolls it back**
  and re-runs. `nsHtml5SpeculativeLoad` drives speculative subresource loads. Of
  all five engines this is the most aggressive off-main-thread design (it runs
  *tree construction* speculatively, not just preload scanning).
- **DOM tree.** `nsINode` (`nsINode.h:416`) — intrusive `mFirstChild`,
  `mChildCount` (`:826`), `mParent` (`:1411`) refcounted pointers, with
  cycle-collected refcounting (not tracing GC). Rare state hangs off lazily
  allocated `nsSlots` (`:1760`) — same "keep the common node small" idea as
  Blink's rare-data.

### WebKit — the clean spec-mirroring C++ implementation

- **Construction split.** `HTMLTreeBuilder` (insertion-mode logic) delegates
  actual DOM building to `HTMLConstructionSite`, which owns the **stack of open
  elements** (`HTMLElementStack`) and the **list of active formatting elements**
  (`HTMLFormattingElementList`). Insertion modes are a scoped enum
  (`InsertionMode::Initial`, `::InTableText`, …, `HTMLTreeBuilder.cpp:405,410`).
- **Foster parenting + formatting reconstruction, well-commented.**
  `HTMLConstructionSite::reconstructTheActiveFormattingElements()`
  (`HTMLConstructionSite.cpp:919`) and `fosterParent()` (`:987`) with a precise
  comment about the "adjusted insertion location / last template element" rule
  (`:955-959`) — the clearest prose model of foster parenting across the engines.
- **Fast path.** `HTMLDocumentParserFastPath.cpp` mirrors Blink's optimized
  `innerHTML` path (the two share heritage).
- **DOM.** `Node`/`ContainerNode`/`Element` — intrusive refcounted
  (`Ref<>`/`RefPtr<>`) sibling/parent pointer tree, structurally very close to
  Gecko's but with WebKit's `Ref` smart pointers instead of cycle collection.

### Servo — html5ever, and Manuk's closest peer

- **Same parser as Manuk.** `servoparser/html.rs:13-14` builds
  `HtmlTokenizer<TreeBuilder<Dom<Node>, Sink>>` from **html5ever** — the exact
  crate Manuk uses. Servo's `Sink` implements `TreeSink` over the *script* DOM
  (GC'd `Dom<Node>` handles) rather than an arena. `feed()`
  (`html.rs:101`) returns `TokenizerResult::{Done, Script, EncodingIndicator}` —
  the tokenizer **hands a `<script>` back to the driver** to run, then resumes.
- **Off-main-thread parser.** `async_html.rs` runs html5ever on a **separate
  thread** (`thread::spawn`, `crossbeam_channel`), sending `ParseOperation`
  messages (`CreateElement`, `GetTemplateContents`, …, `:70,467`) back to the
  script thread to mutate the real DOM. This is Servo's answer to Gecko's stream
  parser — parse off-thread, apply on-thread.
- **Prefetch.** `prefetch.rs` is a lightweight speculative-load tokenizer pass
  (Servo's preload scanner), fed via a separate `prefetch_tokenizer`
  (`mod.rs:158`).
- **document.write.** `mod.rs:131,142` — a dedicated script-input buffer and
  `script_nesting_level`/`suspended` flags support re-entrant `document.write`.
- **DOM tree.** `node/node.rs:130` — `Node` with `MutNullableDom<Node>`
  `parent_node`/`first_child`/`last_child`/`next_sibling`/`prev_sibling`
  (`:135-147`): a **GC'd pointer tree** (SpiderMonkey-traced `Dom<T>` handles),
  the closest production analog to a "classic" DOM but in Rust.

### Ladybird — hand-written C++ spec parser, now migrating to Rust

- **C++ parser.** `HTMLParser.cpp` is a direct, readable transcription of the
  spec (insertion modes, adoption agency, foster parenting) — historically the
  most approachable reference implementation.
- **A new pure-Rust parser (highly relevant to Manuk).** `Parser/Rust/`
  (`libweb_html_tokenizer`, edition 2024) contains a **from-scratch Rust
  tokenizer + tree builder**: `parser.rs:178` `enum InsertionMode`,
  `:303` `list_of_active_formatting_elements`, `:304`
  `stack_of_template_insertion_modes`, `:322` `foster_parenting_enabled`,
  `:4073` `reconstruct_the_active_formatting_elements()`, `:4105`
  `run_the_adoption_agency_algorithm()`. It exposes a C ABI via cbindgen. So
  Ladybird is independently building the *exact* thing the brief imagined Manuk
  building — and chose **not** to reuse html5ever, presumably for tighter
  integration with LibWeb's DOM and encoding stack.
- **Speculative parsing.** `SpeculativeHTMLParser.*` + `IncrementalDocumentParser.*`
  — incremental/streamed parse with a speculative preload pass.
- **DOM.** `DOM/Node.*` / `Element.*` — intrusive GC'd (LibJS `GC::Ptr`) pointer
  tree.

### Cross-engine summary

| Aspect | Blink | Gecko | WebKit | Servo | Ladybird |
|---|---|---|---|---|---|
| Parser origin | hand C++ | Java-xlated | hand C++ | **html5ever** | hand C++ **+ new Rust** |
| Off-thread | preload+bg-scan (worker) | **full tree-build speculation** | preload | **full async parser** | speculative preload |
| Tokenizer accel | — | **SIMD** | — | — | — |
| `innerHTML` fast path | ✓ | — | ✓ | — | — |
| DOM memory model | **Oilpan GC** + rare-data + tagged parent | refcount+cycle-collect, `nsSlots` | `Ref<>` refcount | **traced GC** (`Dom<T>`) | LibJS GC |
| Node links | intrusive, circular-prev | intrusive | intrusive | intrusive | intrusive |

Every production DOM is an **intrusive pointer tree with lifetime management
(GC or refcount)**. None uses an index/arena model — that is Manuk's deliberate
divergence (§3).

---

## 3. Manuk today

### The parser: html5ever driven into the arena

`engine/html/src/lib.rs` + `engine/html/src/sink.rs` reuse html5ever exactly as
Servo does, so Manuk inherits a **fully spec-complete tokenizer + tree builder**
for free: all insertion modes, adoption agency, active formatting elements,
foster parenting, quirks-mode detection, and character-reference decoding are
html5ever's, battle-tested against html5lib and the web. Manuk writes only the
`TreeSink`.

- **Streaming.** `StreamParser` (`lib.rs:59`) feeds byte chunks through a
  `Utf8LossyDecoder` and shares the arena via `Rc<RefCell<Dom>>`, so a
  parsed-so-far `snapshot()` (`:88`) can drive a first paint of `<head>` +
  above-the-fold before the tail arrives (`body_started()`, `:99`). This is a
  genuinely good streaming design — cheaper than Servo's message-passing because
  the sink writes the shared arena directly.
- **Declarative Shadow DOM works** (`sink.rs:261`, `attach_declarative_shadow`)
  — html5ever already implements the DSD parsing rules; the sink overrides the
  hook that `markup5ever_rcdom` leaves defaulting to `false`. Named slots, slot
  fallback, and text slottables are covered by tests (`lib.rs:407,444`).
- **Text-run merging** (`sink.rs:92`, `append_text_to`) — adjacent text is merged
  into one node, avoiding split inline runs in layout (tested `lib.rs:457`).
- **`<template>` contents** parsed into a real `Fragment` (`sink.rs:148,191`).

**Honest sink gaps (documented, not faked — `sink.rs:18`):**
- **Namespaces are folded to the local name** — SVG/MathML foreign content is not
  modelled. This is the biggest correctness gap: `<svg>` inline content, foreign
  attribute case-fixups, and CDATA in foreign content will be wrong. The adoption
  agency itself is fine (html5ever's), but foreign-content insertion is degraded
  at the sink boundary.
- `associate_with_form` is a no-op (`sink.rs:234`) — no form-owner pointer; forms
  are found by walking up the tree. Breaks form controls reparented away from
  their `<form>`.
- `mark_script_already_started` / script execution: **no scripts run during
  parse** — Manuk does not implement the blocking-script / `document.write`
  re-entry path at all. Fine for a read-mostly renderer; wrong for script-driven
  document construction.
- The `<template>` element itself is still exposed in the node tree.

### Fragment parsing / `set_inner_html`

`set_inner_html` (`lib.rs:179`) parses the fragment as a **full document** and
deep-clones `<body>`'s children into the target. This is *not* the spec's
**context-aware fragment parsing** — `dom.innerHTML = "<tr>...</tr>"` on a
`<table>` will be dropped/misparsed because the fragment is parsed in the "in
body" context rather than "in table". html5ever exposes `parse_fragment` with a
context element; Manuk does not use it. Flagged in the code comment (`:177`).

### The arena DOM (`engine/dom/src/lib.rs`)

`Dom` is a `Vec<Node>` (`:130`); `NodeId(usize)` (`:23`) indexes it. Links are
`Option<NodeId>` (`:90`). This is the **deliberate lean-Rust divergence** from
every production engine's pointer tree.

**Strengths (real, and worth keeping):**
- **Trivially `Send` + cheap to share across passes.** No `Rc`/GC/refcount; the
  whole tree is one `Vec`, so parse → style → layout → paint can hand it around
  (or clone it: `snapshot()` is a `Vec` clone) without lifetime gymnastics. The
  crate is deliberately **JS-engine-free** (`dom/Cargo.toml:9` deviation note),
  keeping the JS feature gate off the parse/layout path.
- **Cache-friendly traversal.** Nodes are contiguous; DFS (`descendants`, `:600`)
  walks mostly-sequential memory. Production engines chase pointers across the
  heap.
- **Incremental-layout dirty bits built in.** The **double dirty-bit** model
  (`dirty` + `dirty_descendants`, `:105`, `mark_dirty` `:395`) matches exactly how
  Blink/Gecko propagate style/layout invalidation — good architectural instinct.
- **Shadow DOM as a first-class separate tree** (`shadow_root` link that is *not*
  a child, `:99`; `flat_children` `:345`) — models the node-tree/flat-tree split
  correctly, which is the classic shadow-DOM bug others get wrong.

**Tradeoffs / gaps vs the pointer-GC model:**
- **No node deletion / reclamation.** `alloc` only pushes (`:182`); `detach`
  unlinks but the `Node` stays in the `Vec` forever. A long-lived page that
  churns the DOM (SPA, animations) **leaks arena slots** — a generational-index
  free list is the standard fix and is absent. This is the arena model's single
  biggest liability.
- **`NodeId` is not generational** — a stale id after (future) reuse would
  silently alias. Fine today because slots never free; a landmine if a free list
  is added without generation tags.
- **`NodeId`s are arena-local**, so cross-`Dom` moves must **deep-clone**
  (`clone_into`, `lib.rs:197`; `set_inner_html`). Production engines just move a
  pointer. Adoption across documents is O(subtree) copies.
- **Thin Web-API surface** (acknowledged, `dom/lib.rs:9`): no `ChildNode`,
  `NodeList`, ranges, mutation observers, `ElementData` sharing, or rare-data
  split. Attributes are a linear `Vec<Attr>` scanned per lookup (`:42`) — fine
  at current scale, O(n) per attribute on huge elements.
- **No id/class hash index** — `find_first` is a full DFS (`:594`);
  `getElementById` will be O(n).

---

## 4. Fold-in recommendations (ranked by leverage)

1. **Keep html5ever. Do not build a from-scratch parser.** The brief's premise is
   already the wrong direction: Servo ships html5ever in production; Ladybird's
   own 8600-line Rust rewrite (`parser.rs`+`tokenizer.rs`) shows what "from
   scratch" actually costs, and it exists mainly for DOM/encoding integration
   Manuk doesn't need. html5ever gives Manuk the adoption agency, foster
   parenting, active-formatting reconstruction, and character references —
   thousands of html5lib-tested edge cases — for the price of one `TreeSink`.
   **This is the highest-leverage decision, and it's already made correctly.**

2. **Fix context-aware fragment parsing** (correctness, real sites). Replace the
   parse-as-document hack in `set_inner_html` (`lib.rs:179`) with html5ever's
   `parse_fragment` + a context element. `el.innerHTML = "<tr>…"`,
   `"<td>…"`, `"<li>…"`, `"<option>…"` are extremely common in real JS-driven
   pages and are silently broken today.

3. **Add a generational free list to the arena** (the arena's one true weakness).
   Give `NodeId` a `{index, generation}`, keep a free list of detached slots, and
   reuse them. Without this, any long-running page leaks. This is the change that
   makes the arena model *actually* production-viable rather than demo-viable —
   and it's a well-trodden Rust pattern (slotmap/generational-arena). **Validate
   the arena, then harden it here.**

4. **Model foreign content (SVG/MathML) at the sink** (correctness). Stop folding
   namespaces to local names (`sink.rs:33-48`): carry html5ever's `QualName`
   namespace into `Attr`/`ElementData` (the `namespace` slot is already reserved,
   `dom/lib.rs:27`). Inline `<svg>` is common enough (icons, logos) that dropping
   the namespace mis-renders real pages. html5ever already does the foreign-content
   *tree construction* correctly; Manuk only discards the result.

5. **Add an id → NodeId index** on the `Dom` (perf) so `getElementById` /
   `find_first("body")` aren't O(n) DFS. A `HashMap<String, NodeId>` maintained in
   `set_attr`/`detach`. Low effort, matches Blink's `TreeScope` id map.

6. **Later, if/when scripts mutate the DOM at parse time:** implement the
   blocking-script + `document.write` re-entry path (currently a no-op,
   `sink.rs`). Not needed for a read-mostly renderer; required for full SPA
   compatibility. Servo's `feed()`→`TokenizerResult::Script` loop
   (`servoparser/html.rs:101`) is the model.

### BLOAT to avoid

- **Do not port Blink's `BackgroundHTMLScanner` / Gecko's speculative stream
  parser.** They exist to hide multi-hundred-ms parse+compile latency on
  desktop-class pages with a JIT. Manuk's `StreamParser` already gives streaming
  first-paint; off-main-thread *tree construction* with speculation/rollback is
  enormous complexity for a lean engine. Preload-scan-only (like a trimmed
  `prefetch.rs`) is the most Manuk should consider, and only if subresource
  latency proves to be the bottleneck.
- **Do not build the `innerHTML` fast path** (Blink/WebKit `*FastPath`). It's a
  micro-optimization for a hot JS API on a mature engine; premature for Manuk.
- **Do not adopt Oilpan/tracing-GC or refcounting for the DOM.** The arena is the
  *right* lean-Rust call — the whole point is to avoid that machinery. Harden the
  arena (rec. 3) instead of abandoning it.
- **Resist a full spec DOM API surface up front.** Add `ChildNode`/`NodeList`/
  ranges/mutation-observers only as specific sites demand them.

---

## 5. Open questions for frontier research

1. **Arena vs generational-arena vs GC for a real SPA workload.** Does a
   generational free list keep the arena bounded under heavy DOM churn
   (animations, virtualized lists), or does fragmentation/clone cost eventually
   favor a pointer tree? No production engine has validated the pure-arena model
   at scale — Manuk (and Ladybird's Rust experiment) are the live probes.
2. **Can html5ever's `TreeSink` express everything Manuk needs long-term** —
   foreign content, template semantics, form association, custom-element
   reactions — or does the sink boundary eventually force forking html5ever?
   Servo runs custom-element reaction stacks *through* the sink
   (`async_html.rs`), suggesting it can.
3. **Streaming-into-a-shared-arena vs Servo's message-passing async parser.**
   Manuk's `Rc<RefCell<Dom>>` snapshot model is simpler and cheaper single-threaded;
   is there a first-paint-latency win from ever moving tree construction off the
   main thread, given the arena is already `Send`?
4. **Is SIMD tokenization (Gecko's `nsHtml5TokenizerSIMD`) worth pushing upstream
   into html5ever?** Tokenization is a measurable fraction of parse time; a
   vectorized fast-scan for text/attribute runs could benefit the whole Rust
   ecosystem, not just Manuk.
5. **Where should the node-tree / flat-tree boundary live** as shadow DOM,
   slotting, and (future) CSS `display:contents` interact — recomputed on demand
   (`flat_children`, today) or cached with invalidation like Blink's flat-tree
   traversal? The dirty-bit infrastructure is already present to support caching.
