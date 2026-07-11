# Manuk — Parity Implementation Plan V2 (the delta, ordered)

## Execution status (live) — research top-10 ↔ shipped

The frontier synthesis (`RESEARCH-FINDINGS-V2.md`) ranked 10 highest-leverage moves.
Progress against them + the phase plan below:

| # | Move | Status |
|---|------|--------|
| — | **Stylo cascade wired + default (72/72)** | ✅ shipped (A1) |
| 4 | `@media` viewport coupling | ✅ shipped (via A1 viewport threading) |
| 5 | Parallelize subresource fetch | ✅ shipped (B4) |
| 1 | Frame-scheduled dirty-bit render loop | ✅ shipped (B2 coalescing); wiring scroll→incremental paths remains |
| 9 | Synchronous `Readiness` + role/name agent API | ✅ shipped (E2 + E1 handles) |
| 10 | In-process semantic DOM diffing (moat) | ✅ shipped (`A11yNode::diff` / `observe_diff`); provenance Action-Guard exists (E6) |
| 3 | WOFF2 web fonts | ✅ shipped (A3 — pure-Rust decoder + glyf/hmtx transforms) |
| 8 | Stable + generational `NodeId` | ✅ foundation shipped (packed gen+index, free-list reclaim infra, `is_alive`/`discard_subtree`); auto-free wiring at safe discard sites is the follow-on |
| 2 | Persistent-texture partial damage upload | ☐ remaining (GPU; headless-unverifiable) |
| 6 | Minimal spatial/scroll tree | ☐ remaining |
| 7 | Blitz-model Taffy 0.12 integration + intrinsic-size cache | ✅ **LayoutPartialTree shipped** — measure seam + intrinsic cache, then a full unified taffy tree (7 trait impls) now drives flex/grid; block/inline/float/table are Manuk-measured leaves. Verified 72/72. Stage-4 full-geometry extraction (avoid re-placing nested flex/grid) is a remaining perf refinement |

Also shipped this program: DDG search-click fix, `vw/vh/vmin/vmax` units, 9 engine
repomaps, `RESEARCH-PROMPT-V2` + `RESEARCH-FINDINGS-V2`.

Remaining big rocks are sequenced in Phases B3 / C / G below; the GPU ones need live-window
verification and the Taffy integration is a focused multi-step effort.

---


**Derived from** the wave-1+2 repomap (`docs/parity/repomap/01–09`) and
`RESEARCH-PROMPT-V2.md`. Ordered by the user's three stated pain points —
**(1) real-site visual parity ("1990s HTML"), (2) latency/snappiness, (3) agent-native
ergonomics** — filtered through the leanness mandate.

**Status legend:** ☐ todo · ◐ in progress · ☑ done. Every item names its **files**,
the **repomap source**, and a **verification** hook (the WPT parity harness stays the
gate; render-to-PNG for visual items; unit tests for logic).

> Sections F (JS) and the parser decision are pending wave-2 docs 06/09; filled on landing.

---

## Phase A — Visual parity (kills the "1990s" look)

### A1 ☑ Wire Stylo into the runtime — DONE, flipped to default  *(CSS repomap #1)*
**Status:** complete. `engine/page` routes every cascade through `cascade_styles()`; under
`stylo` (now a **default** shell feature) it drives `cascade_via_stylo` with the real
viewport. The full `ComputedValues`→`ComputedStyle` mapping was completed — color/font/
display/sizing/margin/padding/border(+style-zeroing)/inset/position/float/clear/overflow/
z-index/box-sizing/white-space/table/transform(2D)/calc(two-basis)/flex(AlignFlags)/grid
(+`layout.grid.enabled` pref)/presentational-hints — reaching **Stylo parity 72/72** (was
26/72). `vertical-align` (no computed accessor in stylo 0.19) is patched from MinimalCascade.
Default-on with a **per-page `catch_unwind` fallback to MinimalCascade** so a trait-wall
`unimplemented!()` on an untested page degrades gracefully instead of crashing. Verified: no
panic on example.com/HN/wikipedia; both cascades hold 72/72. Tradeoff accepted: heavier
build + larger binary (drop via `--no-default-features` for the lean MinimalCascade path).

<details><summary>original plan</summary>

#### A1 (orig) Wire Stylo into the runtime  *(highest leverage; CSS repomap #1)*
The `StyloEngine`/`cascade_via_stylo` path is real and tested but **dead code** — nothing
enables the `stylo` feature and `engine/page` hardcodes `MinimalCascade` everywhere. Only
Step 5 of `STYLO-CASCADE-PLAN.md` (wiring + gate) remains.
- **Files:** `engine/page/Cargo.toml` (add `stylo` feature → `manuk-css/stylo`),
  `engine/page/src/lib.rs` (select `StyloEngine` under the feature at every `.cascade()`
  site; thread viewport w/h into the `Device`), `shell/Cargo.toml` (expose the feature).
- **Why it matters:** unlocks real selector matching (combinators/attrs), `@media`,
  `var()`, `@layer`, correct specificity/`!important` — the actual gap behind broken
  real-site styling. MinimalCascade's greedy no-backtrack combinator + `@media`-skip are
  the root causes it can't fix cheaply.
- **Risk:** heavy build; `unimplemented!()` trait-wall paths could panic on real pages —
  must fuzz against a page set before defaulting it on.
- **Verify:** parity harness ≥ 72/72 with `--features stylo`; render example.com + a
  handful of real sites to PNG and eyeball; new cascade unit tests (`var()`, `@media`,
  child/attr selectors) that MinimalCascade fails.
- **Default decision:** ship behind the feature first; flip to default only after the
  page-set fuzz is clean. Keep `MinimalCascade` as `--no-default-features` fallback.

</details>

### A2 ☐ MinimalCascade stopgaps *(only if A1 stays feature-gated, not default)*
- `@media (min/max-width)` evaluation against the real viewport (currently `skip_at_rule`
  drops all at-rules). Files: `engine/css/src/lib.rs`.
- Fix the greedy no-backtrack descendant/child combinator matcher (drops rules like
  `.a .b`, `div > p` on some structures). Files: `engine/css/src/lib.rs`.
- Verify: unit tests for responsive + complex-selector cases.

### A3 ☐ Web fonts: WOFF2 decompression  *(text repomap #1)*
Today only raw sfnt passes `fetch_font_bytes`; WOFF/WOFF2 are skipped, so **most real web
fonts silently drop**. Gating question: maintained pure-Rust WOFF2 vs. vetted Brotli/`woff2`
FFI (the `woff2` crate 0.2/0.3 failed to build here — evaluate `brotli` crate + a WOFF2
table reassembler, or `allsorts`/`ttf-parser` ecosystem).
- **Files:** `engine/page/src/lib.rs` (`fetch_font_bytes`), maybe `engine/text`.
- **Verify:** load a page whose `@font-face` is WOFF2; confirm the face registers + shapes.

---

## Phase B — Latency / snappiness (the "not blazing fast" fix)

### B1 ☐ Route the GUI through the existing damage path  *(scheduling repomap #2)*
The engine already has `relayout_incremental` + `RestyleDamage` + `apply_paint_only` +
a display list — **the GUI bypasses all of it**, calling `relayout_zoomed` + full
re-raster on every event. Route scroll and paint-only changes through the damage
classification so scroll/color never triggers layout.
- **Files:** `shell/src/gui.rs`, `engine/page/src/lib.rs` (surface the incremental API).
- **Verify:** logic-testable at the page layer (unit test: scroll → no relayout); frame
  latency itself needs the live window (documented headless limitation).

### B2 ☐ Frame-scheduled render loop with a dirty bit  *(scheduling repomap #1)*
Input events set a `needs_render`/`needs_relayout` flag; render **once** in
`RedrawRequested` at frame cadence instead of synchronously per event. Coalesces bursts
(scroll wheels, key repeats).
- **Files:** `shell/src/gui.rs`.
- **Verify:** live window; add a headless-testable coalescing helper where possible.

### B3 ☐ GPU: partial/persistent texture upload + uv-scroll  *(paint #3, scheduling #3)*
The shell allocates a new full-viewport texture and re-uploads the whole canvas every
frame (`shell/src/gui.rs`). Wire the existing `damage_since` into a persistent texture
with partial `write_texture`; composite scroll as a uv-offset of the cached surface (the
"off-main-thread scroll" insight with no extra thread).
- **Files:** `shell/src/gui.rs`, use `engine/paint` `damage_since`/`changed_since`.
- **Verify:** live window; unverifiable headlessly — land carefully, keep CPU path intact.

### B4 ☐ Parallelize serial subresource fetches  *(net repomap #1 — smallest change, big win)*
`engine/page` awaits images/fonts/stylesheets in serial `await` loops after parse. Replace
with `join_all`/`buffer_unordered` over the shared hyper client (already pooled, HTTP/2).
- **Files:** `engine/page/src/lib.rs` (`fetch_images`, `fetch_and_apply_stylesheets`, font fetch).
- **Verify:** unit/integration timing; assert N subresources fetch concurrently.

### B5 ☐ Preload scanner + in-memory HTTP cache  *(net #2, #3)*
Minimal preload scanner over the existing streaming bytes to start CSS/font fetches while
parsing; port Servo's RFC-9111 in-memory cache (same hyper stack, near-portable),
top-frame-partitioned key.
- **Files:** `engine/net`, `engine/page`, maybe a new `engine/net` cache module.
- **Verify:** cache-hit unit tests (freshness, `Vary`, revalidation); scanner finds
  subresources before the main parser reaches them.

---

## Phase C — Layout completeness  *(layout repomap)*

### C1 ☐ Taffy deep integration (Blitz model)
Replace throwaway per-container Taffy trees with `LayoutPartialTree`/`CacheTree`
implemented over the arena DOM, with a **measure seam** back into block/inline — gives
flex/grid correct intrinsic sizing + Taffy's 9-slot cache for free.
- **Files:** `engine/layout/src/lib.rs`.
- **Verify:** parity harness with flex/grid probes (add them); intrinsic-size cases.

### C2 ☐ Intrinsic-size memoization (min/max-content) — removes the O(n²) risk.

---

## Phase D — Text quality  *(text repomap)*
### D1 ☐ Shaped-run / word cache keyed on `{text, font, size, features}`.
### D2 ☐ UAX#14 line breaking with safe-to-break edge-only reshaping (no icu4x pull).
### D3 ☐ `{script, locale}`-aware fallback replacing the fixed `FALLBACK_FAMILIES`.

---

## Phase E — Agent-native differentiator  *(automation/a11y repomap)*
Our in-process controller lets us skip the delta-serializer + dual node-ID machinery every
other engine carries for its process boundary.
### E1 ☐ Expose arena `NodeId` as the agent's stable handle (kills per-call tree re-resolve).
### E2 ☐ Awaitable typed `Readiness` computed synchronously from the shared page.
### E3 ☐ Occlusion-aware hit-test (flat z-ordered list, Firefox model).
### E4 ☐ Typed in-process Rust action bindings + role/name-first skill set; keep `bidi/` as
external interop only.

---

## Phase F — JS engine & bindings  *(js repomap; verdict: keep SpiderMonkey/mozjs)*
Manuk's reflector model = Gecko's minus the cycle collector. The wins are ergonomic/perf,
not an engine swap.
### F1 ☐ Replace `eval`'d JS-string bindings with direct JSAPI calls  *(js #R1 — do first)*
Identity cache, event dispatch, `getComputedStyle`, job enqueue currently round-trip
through `eval`'d source strings — slow, page-shadowing-fragile, injection-prone.
- **Files:** `engine/js/src/dom_bindings.rs`. **Verify:** existing JS tests stay green;
  add a test that a page redefining `Object`/`Array` can't break our bindings.
### F2 ☐ Minimal WebIDL binding generator (~30–50 curated interfaces via `weedle`)  *(#R2)*
Target mozjs's existing conversion traits — NOT a 25k-line codegen. Replaces the
~1,650-line hand-written conflated `Node`.
### F3 ☐ Servo-style wrapper-cache + GC trace hook (not a cycle collector)  *(#R3)*
Fixes the raw `*mut Dom` deref + **arena `NodeId`-reuse hazard** — pairs with **G3**.
### F4 ☐ Per-interface prototypes (Ladybird `ensure_web_prototype` model)  *(#R5)*
### F5 ☐ Native `Promise`-returning `fetch`, real timers, structured clone, `MutationObserver`.

## Phase G — HTML parser & DOM  *(html/dom repomap; already on html5ever — decision settled)*
### G1 ☑ Keep html5ever — validated as correct (Servo's, spec-complete). No action.
### G2 ☐ Context-aware fragment parsing for `innerHTML`  *(html #2)*
`set_inner_html` parses as a full document, so `innerHTML="<tr>…"` breaks. Use html5ever's
fragment-parsing with the element's context. **Files:** `engine/html/src/lib.rs`.
### G3 ☐ Arena generational free list  *(html #3 — the arena's one true weakness)*
`alloc` only pushes; slots are never freed, so long-lived pages leak and `NodeId`s can be
reused unsafely (the hazard **F3** guards against). Add generational indices. **Files:**
`engine/dom/src/lib.rs`. **Verify:** churn test (create/remove N nodes) shows bounded memory.
### G4 ☐ Stop folding namespaces to local names  *(html #4)*
`sink.rs` folds namespaced names, so inline SVG/MathML don't render. Preserve namespace.
**Files:** `engine/html/src/sink.rs`.
### G5 ☐ `id`→`NodeId` index for O(1) `getElementById` + id-selector matching.

---

## Sequencing rationale
1. **A1 (Stylo)** first — it's the single biggest visual-parity lever and the CSS repomap's
   top rec; verifiable via render-to-PNG + the parity gate.
2. **B4** (parallel fetch) and **B1/B2** (damage-path scroll + frame scheduling) next —
   cheap, high-impact latency wins, minimal risk, no new deps.
3. **B3/B5, C, D, E** as depth passes; **Vello** GPU backend behind the `Painter` trait is
   a later bet, not a blocker.
Each landed item keeps the WPT parity gate green and is committed atomically.
