# Manuk — Frontier Research Prompt (post-repomap synthesis)

**Status:** DRAFT. Wave-1 capability areas (CSS, layout, text, paint/GPU, automation/a11y)
synthesized from `docs/parity/repomap/01–05`. Wave-2 areas (JS bindings, event-loop/
scheduling, networking/loading, HTML/DOM) are stubbed and will be filled from
`docs/parity/repomap/06–09` as those land.

## 0. Purpose & framing

We have repo-mapped Blink/Chromium, Gecko/Firefox, WebKit, Servo, Ladybird, V8,
and the Rust stacks (Stylo, Taffy, Parley, Vello/Blitz, mozjs) against Manuk. The
repomap answered *"what do they do and what should we fold in."* This prompt drives
the next layer: **the SOTA/academic frontier and the unresolved design questions**
that the repomap surfaced — the things where copying an existing engine is *not*
obviously right for a lean, fast, agent-native Rust browser.

North star (unchanged):
1. Own the whole browser surface to ship bleeding-edge **human** ergonomics.
2. Own the whole surface to make **agent** automation first-class and in-process —
   beating Playwright-on-Chromium on ergonomics and round-trip latency, not riding CDP.
3. Stay **lean and fast** (Rust, wgpu) without ideological language lock-in; fold in
   only what earns its weight.

For every question below: prefer answers grounded in the cloned source + published
papers/specs over speculation. Output a decision with rationale and a rough
implementation cost, feeding directly into `PARITY-IMPLEMENTATION-V2.md`.

---

## 1. CSS — cascade, selectors, style computation

**Repomap verdict:** MinimalCascade is the only runtime path; StyloEngine is dead
code (never called by `engine/page`). MinimalCascade skips `@media` entirely and
(until this session) dropped viewport units. Real sites break on responsive CSS.

**Frontier / decision questions:**
- **The pivotal fork:** finish wiring Stylo into the runtime (full CSS correctness,
  heavy build, `unimplemented!()` trait-wall risk on real pages) **vs.** grow
  MinimalCascade to "good enough for the real-web head" (`@media`, container queries,
  `:has()`, cascade layers, custom properties). Quantify: what % of top-1000-site CSS
  features does each path cover, and what is the build-time / binary-size / panic-risk
  cost of the Stylo path? Is a *hybrid* (Stylo for computed values, our own selector
  fast-path) coherent?
- Selector matching at scale: Bloom-filter ancestor rejection + right-to-left matching
  + invalidation sets (Blink/Servo). Which of these does Manuk actually need at its
  DOM sizes, and which is premature?
- Style sharing / rule-tree caching (Servo's `Arc<ComputedValues>` sharing): worth it
  for our arena DOM, or does eager per-node computation stay fast enough?

## 2. Layout

**Repomap verdict:** hand-rolled block/inline/float/table is solid; flex/grid go
through Taffy but as **throwaway per-container trees with no measure function**, so
auto/intrinsic sizing is wrong and nested contexts re-solve. No intrinsic-size cache
(O(n²) risk). Incremental relayout is coarse (any reflow damage → full document
relayout). `margin:auto`/`width` is *correct* — the visible bug was viewport units.

**Frontier / decision questions:**
- Adopt the **Blitz integration model**: implement Taffy's `LayoutPartialTree`/
  `CacheTree` over Manuk's arena DOM with a **measure seam** back into our block/inline
  code, giving flex/grid correct intrinsic sizing + Taffy's 9-slot cache for free.
  What's the API surface and the risk of the two layout models fighting?
- Split **stable box tree vs immutable geometry results** (every mature engine does
  this) to enable a constraint-space-keyed incremental cache (Blink `LayoutResult` +
  `ConstraintSpace` 3-state hit test). Is this worth it before we have real perf data?
- Intrinsic-size memoization: cheapest correct design for min/max-content caching.

## 3. Text

**Repomap verdict:** competent swash pipeline (shaping, bidi, per-char fallback, COLR/
CBDT color). Missing the caching + quality layers every production engine invests in.
**#1 gap: WOFF2 unsupported → most real web fonts silently drop.**

**Frontier / decision questions:**
- **WOFF2 decompression** — the gating question: is there a *maintained pure-Rust*
  WOFF2 path, or is a vetted C FFI (Google's `woff2`/Brotli) the pragmatic lean choice?
  (The `woff2` crate 0.2/0.3 failed to build here.) Decide and cost it.
- Shaped-run / word cache (Gecko word cache, Blink `ShapeResult` + `ShapeResultView`):
  design a lean equivalent keyed on `{text, font, size, features}`.
- UAX#14 line breaking with **safe-to-break edge-only reshaping** (Blink
  `ShapingLineBreaker`): what's the minimal correct implementation without pulling icu4x?
- `{script, locale}`-aware fallback vs the current fixed `FALLBACK_FAMILIES` list.

## 4. Paint / compositing / GPU

**Repomap verdict:** honest CPU tiny-skia rasterizer presented as a full-screen wgpu
quad that **re-uploads the entire canvas every frame** (`shell/src/gui.rs`). Damage
primitives exist and are tested but **not wired to present**. No property/spatial tree;
scroll is a full repaint. The `compositor` crate is frame-timing policy, not a compositor.

**Frontier / decision questions:**
- **The lean GPU bet:** `VelloPainter` (compute-shader sort-middle rasterizer, zero
  per-shape draw calls) behind the `Painter` trait, reusing the shell's wgpu device,
  with `vello_cpu` as the headless fallback — **plus a touch of WebRender's
  *architecture* (retained scene + spatial tree), not its renderer.** Validate: does
  Vello's quality/perf/binary-size fit the lean mandate vs. keeping tiny-skia + a
  smarter present path?
- Wire the existing `damage_since` into **partial / persistent-texture upload** so we
  stop re-uploading the whole canvas — likely the single biggest snappiness win that
  needs no new dep. Cost it first.
- A **minimal property/spatial tree** so scroll and transforms stop being full
  repaints. What's the smallest version that pays off?

## 5. Automation & accessibility (the differentiator)

**Repomap verdict:** strong role+name tree with geometry/hit-test/click-points,
injection-fenced observation, honest forms, and a correctly-chosen WebDriver-BiDi
remote end. **Core insight: every other engine's automation complexity (delta
serializer + dual node-ID spaces) exists to cross a process boundary Manuk doesn't
have — our controller is an in-process agent sharing `engine/page`.** That is the whole
latency/ergonomics opportunity.

**Frontier / decision questions:**
- Expose the arena `NodeId` as the agent's **stable handle** (Ladybird model) to kill
  the re-resolve-the-whole-tree-every-call fragility. API design?
- **Typed in-process Rust bindings** (microsecond calls) + a tiny role/name-first skill
  set vs. Playwright's per-call IPC. What does the ideal agent action API look like?
- Awaitable typed **`Readiness`** computed synchronously from the shared page (no
  network-idle heuristics). What signals?
- Occlusion-aware hit-testing (Firefox's flat z-ordered list). Keep `bidi/` as the
  external interop layer, not the primary API.
- **Frontier:** what does an "agent-native browser" enable that no CDP-driven stack can
  — e.g. deterministic observation snapshots, semantic diffing of DOM state between
  actions, capability-scoped action guards? This is the most novel research area.

---

## 6. JS engine & bindings — *(wave 2, to fill from repomap/06)*
## 7. Event loop & scheduling / latency — *(wave 2, from repomap/07)*
## 8. Networking & loading — *(wave 2, from repomap/08)*
## 9. HTML parsing & DOM — *(wave 2, from repomap/09)*

---

## 10. Cross-cutting synthesis questions (finalize after wave 2)
- **Sequencing:** which 3–5 changes most move the needle on the user's actual
  complaints — (a) real-site visual parity, (b) snappiness/latency, (c) agent
  ergonomics — and in what order?
- **Leanness budget:** every proposed dependency (Vello, html5ever, icu4x, WOFF2/Brotli
  FFI, Taffy-deep-integration) costs build time + binary size + surface. Rank by
  value-per-weight; name what we explicitly decline.
- **Verification:** extend the WPT parity harness (currently 72/72 border-box probes)
  to cover the new surfaces (responsive/@media, web fonts, flex/grid intrinsic sizing).
