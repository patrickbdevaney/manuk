# Manuk — Technical Specification

*A from-scratch, memory-safe browser engine in Rust, built to reach qualitative
core-browsing parity with Chromium/Gecko while being lean, fast, and **agent-native**.
This spec describes the stack as it exists now.* ~37k LOC across 16 crates.

---

## 1. North star & principles

- **Own the whole browser surface** to ship bleeding-edge human ergonomics *and* to make
  AI-agent browser automation first-class and **in-process** (not Playwright/CDP over a
  socket).
- **Reuse the best production engines as libraries**, wrapped behind our own traits, rather
  than reimplement them: Servo's `stylo` (CSS), `taffy` (flex/grid), `html5ever` (parsing),
  `swash` (text), Mozilla's SpiderMonkey via `mozjs` (JS). Hand-roll only where a lean Rust
  design beats the alternative (block/inline/float/table layout, the arena DOM, the paint
  pipeline, the agent surface).
- **Hard boundary:** never patch a JS-engine's internals — only sanctioned FFI/embedding.
- **Verifiable:** a WPT-style parity harness compares element geometry to headless Chrome at
  ±3px and gates every change (**72/72 probes across 30 pages** today).
- **Platform:** Linux/X11 primary (wgpu present path verified headed at ~1288 fps);
  headless `render`/`eval` need no display.

---

## 2. Workspace / crate map

| Crate | LOC | Role |
|---|---:|---|
| `engine/net` | 2300 | HTTP(S) via hyper 1 + rustls (ring); redirects, streaming, charset sniff, content-decode, **in-memory HTTP cache**, optional adblock |
| `engine/html` | 790 | HTML parse via `html5ever` → arena DOM (custom `TreeSink`); streaming + fragment parsing |
| `engine/dom` | 1140 | Arena DOM: `Vec<Node>` + **generational `NodeId`**; flat tree (shadow DOM/slots); session history |
| `engine/css` | 5340 | Cascade behind a `StyleEngine` trait: **Stylo** (default) or **MinimalCascade** (fallback); value parsing |
| `engine/layout` | 3490 | From-scratch block/inline/float/table + **unified `taffy` `LayoutPartialTree`** for flex/grid |
| `engine/text` | 1630 | `swash` shaping + rasterization; `fontdb` discovery; bidi; per-char fallback; **WOFF2/WOFF1** decode; measure + shaped-run + glyph caches |
| `engine/paint` | 765 | Display-list build + `tiny-skia` CPU raster; stacking (z), overflow clip, glyph blitting, damage detection |
| `engine/compositor` | 525 | Frame-timing/telemetry policy + damage-rect model (not a layer compositor) |
| `engine/js` | 3830 | SpiderMonkey via `mozjs 0.18` (feature-gated): reflectors, event loop, DOM bindings, ES modules |
| `engine/a11y` | 1220 | DOM → role+name accessibility tree with geometry; **occlusion-aware hit-test**; semantic diff |
| `engine/page` | 1590 | The shared pipeline: `bytes → DOM → style → layout → paint`; subresource fetch; scripts |
| `shell` | 4390 | `winit`+`wgpu` GPU window, browser chrome, event loop, tabs, find-in-page; headless `render`/`eval` CLI |
| `agent` | 6620 | In-process AI-agent browser: observation, typed actions, forms, LLM backends, capability scoping |
| `bidi` | — | WebDriver-BiDi remote end (JSON-RPC/WebSocket) over the shared pipeline (standards-track, not CDP) |
| `store` | — | Local encrypted password store + origin-scoped autofill (audited crates, zero hand-rolled crypto) |
| `tests/wpt` | 1140 | Parity harness (geometry vs headless Chrome) + reftests |

**Key external deps (pinned):** `html5ever 0.39`, `cssparser 0.34`, `stylo 0.19`,
`taffy 0.12`, `swash 0.2`, `fontdb 0.23`, `unicode-bidi 0.3`, `mozjs 0.18`, `wgpu 27`,
`winit 0.30`, `tiny-skia 0.12`, `resvg 0.44`, `hyper 1` + `hyper-rustls 0.27`/`rustls 0.23`
(ring), `lru 0.12`, `brotli-decompressor 5`, `selectors 0.39`.

---

## 3. Build variants & feature flags

The shell's **default is `["gui", "stylo"]`** — the full GPU window with the real Stylo
cascade. Cascade selection is a compile-time feature that swaps the `StyleEngine`
implementation behind a runtime `catch_unwind` fallback.

| Feature (crate) | Effect |
|---|---|
| `gui` (shell) | winit + wgpu window. Drop with `--no-default-features` for headless `render`/`eval`. |
| `stylo` (shell→page→css) | Real Stylo cascade (heavy build). Off → `MinimalCascade`. |
| `spidermonkey` (shell→page→js) | Link SpiderMonkey (`mozjs` jit + intl/ICU) → real JS. Off → scripts are a no-op. |
| `spidermonkey-noicu` | SpiderMonkey without ICU (smaller binary). |
| `adblock` (net) | Brave's adblock crate at the request layer. |

**Runtime commands** (`manuk <sub>`): `browse [url|terms]` (GUI), `render <url> -o png`,
`eval <expr>`, `browse … --frames N` (GPU frame stats).

---

## 4. The rendering pipeline (`engine/page`)

```
URL ─▶ net::fetch_streaming ─▶ bytes
        │  (in-memory HTTP cache; preload scanner warms subresources concurrently)
        ▼
   html5ever streaming parse ──▶ arena Dom  (first-paint snapshot when <body> opens)
        ▼
   cascade_styles()  ──▶ StyleMap (Stylo or MinimalCascade), viewport-aware (@media, vw/vh)
        ▼
   layout_document() ──▶ LayoutBox fragment tree  (block/inline/float/table + taffy flex/grid)
        ▼
   [spidermonkey] run inline + external <script> against the layout snapshot; if the DOM
        mutated → re-cascade + re-layout
        ▼
   fetch_and_apply_stylesheets / images / @font-face (concurrent) → re-layout
        ▼
   paint: DisplayList ──▶ tiny-skia Canvas ──▶ (shell) wgpu texture ──▶ present
```

Subresource fetches (images, external CSS, fonts) run **concurrently** (`join_all`) on the
shared pooled HTTP/2 client. Incremental relayout classifies damage: `>= Reflow` → full
relayout; `== Repaint` → paint-only fast path (`apply_paint_only`).

---

## 5. Subsystems

### 5.1 Networking (`engine/net`)
- `hyper 1` + `hyper-util` legacy client over `hyper-rustls`/`rustls` (ring, webpki-roots),
  HTTP/1.1 + HTTP/2, connection pooling/reuse, a process-global shared client.
- `fetch` (GET + redirects), `fetch_streaming` (chunked, drives first paint), `request`
  (arbitrary method/headers, no redirects) for API clients.
- Body content-decoding (gzip/deflate/br) via `async-compression`; WHATWG charset sniffing.
- **In-memory HTTP cache** (RFC-9111 subset): fresh `GET 200` with `Cache-Control`
  `max-age`/`s-maxage` served without a round-trip; `no-store`/`private`/`no-cache`/`max-age=0`
  bypass. *(No Vary, conditional revalidation, heuristic freshness, or disk yet.)*
- Optional request-layer content blocking (`adblock` feature).

### 5.2 HTML parsing (`engine/html`)
- Reuses `html5ever` (spec-complete tokenizer + tree builder, incl. adoption agency, foster
  parenting) driven by a custom `ArenaSink` (`TreeSink`, `Handle = NodeId`) that builds
  directly into the arena DOM — no intermediate `RcDom`.
- Streaming parser (`StreamParser`) yields a first-paint snapshot before the tail arrives.
- **Context-aware fragment parsing** (`parse_fragment_in`) for `innerHTML`, so table-scoped
  content (`<tr>`/`<td>`/`<option>`/`<li>`) survives.
- Declarative shadow DOM (`<template shadowrootmode>`) → real shadow roots.

### 5.3 DOM (`engine/dom`)
- Arena: `nodes: Vec<Node>` indexed by `NodeId`, with parent/child/sibling links as
  `Option<NodeId>`. No `Rc`/GC → cheap to share across passes, trivially `Send`.
- **Generational `NodeId`**: packs `generation<<32 | index` into a `usize`. A freed slot bumps
  its generation, so a stale handle to a reused slot fails `is_alive` (returns `None` from
  `element()`) instead of aliasing a new node. Generation-0 (never-reused) nodes are
  byte-identical to a bare index — JS reflectors' `i32` encoding stays valid. Free list +
  `discard_subtree` reclaim slots; **no auto-free** (parser reparenting + JS `removeChild`
  re-insert), so reclamation is opt-in at proven-discard sites.
- Flat tree (shadow host → shadow content; `<slot>` → assigned light-DOM nodes).
- Double dirty-bit (`dirty` + `dirty_descendants`) for incremental restyle/relayout.
- Shared `SessionHistory` model (used by shell, agent, BiDi).

### 5.4 CSS / cascade (`engine/css`)
Pluggable `StyleEngine` trait (`cascade(&Dom, &[Stylesheet]) -> StyleMap`). Two impls:

- **Stylo (default).** Servo's production style system (`stylo 0.19`). Manuk implements the
  full `selectors::Element` matcher + `TElement`/`TNode`/`TDocument`/`TShadowRoot` DOM trait
  wall over the arena, builds a `Stylist` + `Device` (real viewport → correct `@media`/`vw`/
  `vh`), matches author + UA + inline `style=` declarations, cascades to `ComputedValues`, and
  maps them onto our flat `ComputedStyle`. Gets real selector matching, `@media`, `var()`,
  `@layer`, correct specificity/`!important`. Grid is enabled via the `layout.grid.enabled`
  pref. `vertical-align` is patched from MinimalCascade (no computed accessor in 0.19).
  **72/72 parity.** Wrapped in `catch_unwind` (per-page fallback to MinimalCascade).
- **MinimalCascade (fallback, `--no-default-features`).** From-scratch cascade over a
  documented subset: type/class/id/attr/pseudo selectors + combinators, specificity,
  `!important`, inheritance, UA defaults, `calc()`, `hsl()`, viewport units. Also 72/72 on the
  harness; its real-world weakness is selector/@media/var completeness (why Stylo is default).
- `ComputedStyle` is a flat struct of resolved values: color/bg, font (family list, size,
  weight, style), text-align, white-space, line-height, `Dim` sizing (`Auto/Px/Percent/Calc`,
  % stored 0–100), box model, `position`+insets, `z-index`, `overflow`, `box-sizing`, float/
  clear, flex (direction/wrap/grow/shrink/basis/justify/align/gap), grid (tracks + placement),
  `transform` (2D op list), table props.

### 5.5 Layout (`engine/layout`)
Produces a `LayoutBox` fragment tree (`rect` border-box in absolute coords, `background`,
`border`, `node`, `content: Block(children) | Inline(runs)`).

- **From-scratch** block formatting (margin collapsing, BFC), inline/line-breaking, floats
  (`FloatContext`), and tables (CSS2 §17 separated model) — the parts Taffy can't do and that
  carry the parity gate.
- **Flex & grid via a unified `taffy` tree** (Blitz model): `engine/layout/taffy_tree.rs`
  implements taffy 0.12's low-level traits (`TraversePartialTree`, `LayoutPartialTree`,
  `CacheTree`, `RoundTree`, `LayoutFlexboxContainer`, `LayoutGridContainer`) over the arena
  DOM. A flex/grid container **and its directly-nested flex/grid descendants are solved and
  placed in one tree** (shared cache); block/inline/float/table children are Manuk-measured
  **leaves** (`compute_leaf_layout` → `measure_intrinsic` = shrink-to-fit width + content
  height). `ComputedStyle → taffy::Style` mapping is a shared function; geometry is extracted
  back into the `LayoutBox` tree without re-solving.
- Intrinsic-size **memoization** per layout (keyed by `(node, avail-width)`) — kills taffy's
  repeated-probe O(n²).
- Positioned (`relative`/`absolute`), `z-index` effective-layer map, overflow clip rects.

### 5.6 Text (`engine/text`)
- Font discovery via `fontdb`; system + a `FALLBACK_FAMILIES` list (Noto CJK/Emoji/Symbols/
  Arabic/…). Faces registered by an internal `FaceId`; `@font-face` families aliased.
- Shaping/rasterization via **swash**: `ShapeContext` (kerning, ligatures, RTL), `ScaleContext`
  (subpixel raster in 4 quarter-pixel buckets, color glyphs COLR/CBDT).
- **Bidi** reorder (`unicode-bidi`) → visual runs → per-face segmentation → shape.
- **Web fonts:** pure-Rust **WOFF2** (`brotli-decompressor` + glyf/loca transform-v0 + hmtx
  transform-v1 reconstruction → sfnt) and WOFF1 (zlib) decode; raw sfnt passthrough.
- Caches (LRU): measured widths, **fully shaped runs** (glyph ids + positions), rasterized
  glyph bitmaps — all keyed on `(font, size, …)`, so paint/scroll re-use instead of re-shaping.

### 5.7 Paint (`engine/paint`)
- Builds a `DisplayList` of `Rect`/`Text`/`Image` items grouped by stacking order
  (effective z), each carrying an optional overflow clip.
- CPU rasterizer over **tiny-skia** (`CpuPainter`): fills, borders, image blits (straight-
  alpha RGBA), glyph coverage blit + color-glyph source-over, rect masks for clipping.
- `DisplayList` derives `PartialEq` + `changed_since`/`damage_since(prev) -> Rect` — the
  invalidation primitives.
- SVG images rasterized via `resvg`/`usvg` (`engine/page`).

### 5.8 Compositor / GPU (`engine/compositor` + `shell/gui.rs`)
- `compositor`: frame-timing telemetry (`FrameTimer`, jank vs 60fps budget) and the
  damage-rect *policy* — not a layer compositor.
- GPU present (shell): winit surface + a wgpu full-screen-triangle pipeline sampling one
  **persistent page texture**. `upload` reuses the texture (re-created only on resize) and a
  **row-level canvas diff** uploads only changed rows (`upload_damage`) — a small change
  writes a small band, an unchanged frame nothing. Paint is **coalesced** to one per frame
  (input sets a `needs_paint` flag; the paint happens in `RedrawRequested`).
- *Not yet:* GPU spatial/scroll tree (uv-offset scroll) and a Vello backend behind a `Painter`
  trait — both need live-window verification / a large dep.

### 5.9 JavaScript (`engine/js`, feature `spidermonkey`)
- Embeds SpiderMonkey via `mozjs 0.18` (one `Runtime` per process, thread-local; `rooted!`;
  reserved-slot reflectors keyed by `NodeId`). **Never patches SpiderMonkey internals.**
- HTML **event loop**: macrotask FIFO (`setTimeout`) + **microtask checkpoints** draining host
  `queueMicrotask` *and* SpiderMonkey's native promise-job queue to quiescence.
- DOM bindings: Tier-0 globals (window/console/navigator), traversal getters + mutation,
  `getElementById`/`querySelector`, `value`/`checked`, reflector identity cache, a real
  `Event` model (capture/bubble/preventDefault), `getComputedStyle`, **ES modules**
  (`CompileModule`/`ModuleLink`/`ModuleEvaluate`).
- *Honest gaps:* some bindings still round-trip through `eval`'d JS strings; no WebIDL
  codegen, `MutationObserver`, or native Promise `fetch` yet; `setTimeout` ignores the delay.

### 5.10 Accessibility (`engine/a11y`)
- Hand-rolled DOM → `A11yNode` tree (role + accessible name) per HTML-AAM implicit roles +
  WAI-ARIA overrides + a pragmatic accname subset; attaches absolute geometry.
- **Occlusion-aware `hit_test`**: prefers the highest effective-`z` box containing a point (a
  `position:fixed`/high-z overlay wins a click), deepest-wins within a layer.
- Agent-facing: `find`/`find_containing` by role+name, flat `role "name" @(x,y)` rendering, and
  a **race-free semantic `diff`** (which role+name nodes appeared/disappeared between snapshots).

---

## 6. Agent-native layer (`agent`, `bidi`, `store`)

The differentiator: the controller is an **in-process** agent sharing `engine/page`, so it
skips the delta-serializer + dual node-ID machinery every CDP/WebDriver stack needs for a
process boundary.

- **`AgentBrowser`** owns a live `Page` + scroll + history. Consent-gated `Handoff` moves a
  live session between the human shell and the agent (keeps logged-in state, half-filled forms).
- **Stable handles:** arena `NodeId` is the agent's handle — `resolve_handle` once, then
  `activate`/`type_into_handle` repeatedly (no per-call tree rebuild).
- **Typed actions:** `BrowserAction` (Click/ClickHandle/Type/TypeHandle/Submit/Navigate/
  ScrollBy) + `perform()` — an allocation-light in-process API.
- **Readiness:** synchronous snapshot (loaded/title/interactive-affordance count) read from the
  shared page — no network-idle heuristics.
- **Semantic diffing:** `observe_diff()` returns what changed after an action.
- **Provenance / Action-Guard (E6):** page-derived observations are provenance-tagged
  (prompt-injection fence); forms model + capability scoping.
- LLM backends (e.g. Groq), replay, translation, triage, concurrency helpers.
- **`bidi`:** a standards-track WebDriver-BiDi remote end over the same pipeline.
- **`store`:** local encrypted password store + origin-scoped autofill.

---

## 7. Shell (`shell`)

- winit event loop; a chrome band (back/forward/reload, omnibox with search-template + bare-
  term → search, suggestions), tabs, find-in-page (highlight + active-match), text-field focus
  + caret, hand-cursor over links, `resolve_href` (unwraps DDG `/l/?uddg=` redirects,
  protocol-relative).
- Shared Tokio runtime (warm connection pool). Page work routes through `engine/page`; the
  shell only presents. Headless `render`/`eval` subcommands need no display.

---

## 8. Core data models

| Type | Shape | Notes |
|---|---|---|
| `NodeId(usize)` | `generation<<32 \| index` | generational; `.index()`/`.generation()`/`is_alive` |
| `ComputedStyle` | flat resolved-value struct | one per node; `Dim` = `Auto/Px/Percent/Calc` |
| `LayoutBox` | `rect, background, border, node, content` | fragment tree; border-box absolute coords |
| `DisplayList` | stacking-ordered `Rect/Text/Image` + clips | `changed_since`/`damage_since` |
| `ShapedRun` | `glyphs: Vec<GlyphPos>, width, metrics` | cached per `(font,size,text)` |
| `A11yNode` | `node, role, name, bbox, z, children` | occlusion-aware hit-test; `diff` |

---

## 9. Verification

- **Parity harness** (`manuk-wpt parity`): lays out 30 pages and compares each probed
  element's border-box geometry to headless Chrome at **±3px** (font-agnostic). Gate: currently
  **72/72** under both the Stylo and MinimalCascade cascades. Reftests + per-crate unit tests
  round it out; the whole workspace test suite is green.
- Headed GPU present is measured with `browse … --frames N` (≈1288 fps present here). GUI
  visual behavior (scroll rendering) can't be verified headlessly — those items are gated on a
  live window.

---

## 10. Invariants & honest limitations

**Invariants:** never patch SpiderMonkey/mozjs internals; every change keeps the parity gate
green; the shell/agent share one `engine/page`; commits go to `main`.

**Known gaps (documented, not faked):** JS bindings partly via `eval`-strings; no
`MutationObserver`/native-`fetch`/WebIDL-codegen; `setTimeout` ignores delay; HTTP cache lacks
Vary/revalidation/disk; line-breaking is whitespace-based (no UAX#14); no inline SVG/MathML
layout (namespaces folded); no GPU layer compositor / uv-scroll or Vello backend; DOM arena
reclamation is opt-in (no GC); WOFF2 skips TrueType-collection and unknown table transforms.
These are the tracked frontier (`docs/parity/PARITY-IMPLEMENTATION-V2.md`,
`docs/parity/RESEARCH-FINDINGS-V2.md`), most needing heavy builds or live verification.
