# Deep-Research Findings (executed 2026-07-10)

Five web-enabled research threads answering the build-vs-adopt and crate-selection
questions from `RESEARCH-PROMPT.md`. Condensed to recommendation + crates + rationale +
sources; the verdicts feed `PARITY-IMPLEMENTATION.md`.

---

## 1. CSS engine — **finish Stylo, don't grow the hand-rolled matcher**

**Premise correction from the repo audit:** Manuk's `engine/css/src/stylo_dom.rs` already
implements the full 30-method `selectors::Element` trait and runs Stylo's **real** matcher
(`matches_selector`) over the arena DOM, with passing tests (type/id/class/attribute/child/
`:empty`). Only `StyloEngine::cascade` still delegates to `MinimalCascade`.

- `selectors` (0.39) and `cssparser` (0.37) are cleanly usable standalone — proven by
  `scraper`, `kuchikiki` (Brave), `lightningcss`. But they give **matching/parsing only**.
- **`var()` and `@media` *evaluation* effectively only exist in full `style`/`stylo`.** No
  lightweight standalone crate evaluates them (the one dedicated media crate is dead since
  2017). Everyone needing a real cascade takes full Stylo (**Blitz** = Stylo+Taffy+Parley);
  everyone else punts the cascade (scraper, lightningcss store `var()` as raw tokens).
- **Recommendation:** implement the remaining `TElement` (~76) + `TNode` (~20) trait walls
  and wire `StyloEngine::cascade` to a real `Stylist` producing `ComputedValues`, mapping to
  `ComputedStyle`. Reference **Blitz `blitz-dom`** + `stylo_taffy`. Unlocks `@media`, `var()`,
  all combinators, pseudo-classes, `font-family` at once, spec-correct. Keep `MinimalCascade`
  as the fast default; Stylo behind `--features stylo`. **Risk:** `0.x` version churn (pin
  exact) + heavy compile (keep off default build).
- Sources: github.com/servo/stylo · github.com/DioxusLabs/blitz · crates.io/selectors ·
  crates.io/cssparser · github.com/parcel-bundler/lightningcss · doc.servo.org/style/media_queries

## 2. Text stack — **target Parley + swash + fontique; cosmic-text as on-ramp**

- fontdb+fontdue gives no shaping/bidi/fallback/`font-family`/subpixel/cache. **Parley**
  (+ HarfRust shaping, ICU4X bidi, Fontique fallback, swash/Glifo raster) is the browser-
  grade Rust stack and the exact text layer of **Blitz** — architecturally consistent with
  the Stylo direction. **cosmic-text** bundles shaping+bidi+fallback+raster in one crate
  (Zed/Bevy/iced) → lower-effort Phase-1 if correctness-fast is wanted.
- **Not a drop-in swap:** the char-based `GlyphPos{ch,x}` model can't express ligatures/
  complex scripts → must become glyph-ID runs `{glyph_id,font_id,x,y}`. Contained to
  `engine/text` + `engine/paint`'s glyph consumer.
- swash gives subpixel fractional positioning + a glyph raster cache (key
  `(glyph_id,font_id,size,subpx)`) + COLR/CBDT color emoji; pack into a GPU atlas via
  `etagere` when/if compositing moves to GPU.
- Phased: (1) redefine the `engine/text` glyph-ID boundary, (2) swap backend to Parley+
  Fontique+swash, (3) add raster cache + atlas + subpixel, (4) web fonts into Fontique.
- Sources: github.com/linebender/parley · github.com/pop-os/cosmic-text · github.com/dfrg/swash
  · github.com/DioxusLabs/blitz · docs.rs/swash · linebender.org/blog

## 3. Paint / compositor — **CPU invalidation now, Vello later (feature-gated)**

- **Central finding:** the caret-blink/1px-scroll full-frame repaint is a *missing
  invalidation layer*, not a slow rasterizer. Fixes on the existing `tiny-skia` path (days):
  wire the `Damage` type (display-list diff → damage rect → clipped repaint), scroll-blit
  (shift pixmap, repaint exposed band), reuse the wgpu texture with partial `write_texture`
  of the damage sub-rect, and a CPU glyph coverage cache (swash/`SwashCache`).
- `tiny-skia` already covers rounded rects, gradients, images, alpha-mask clips, and manual
  opacity groups → most "missing primitives" are achievable without switching. The only true
  gap is **blur** (`box-shadow`/`filter`) — unimplemented in *both* tiny-skia and Vello → hand-
  roll a separable Gaussian on the alpha mask (cacheable).
- **Vello:** classic path is alpha; **Vello Hybrid** (WebGL2, no WebGPU needed) is ~beta as
  of Q1 2026. Introduce an **`anyrender`-style swappable backend trait** now (Blitz's pattern),
  keep `tiny-skia` default, add feature-gated `vello_hybrid` later once blur/filters + glyph
  caching mature.
- Sources: github.com/linebender/vello · linebender.org/blog/tmil-24,25 · github.com/DioxusLabs/blitz
  (anyrender) · docs.rs/tiny-skia · browser.engineering/invalidation.html · gfx-rs/wgpu wiki

## 4. JS web-APIs + binding strategy — **Tier-0 globals first; hand-bind on mozjs**

- **Failure model:** bundles cache globals eagerly at load; one missing Tier-0 global is a
  `ReferenceError` on line 1 that aborts the whole `<script>` before Manuk's 30-method
  surface is reached. jQuery hard-requires `window.document`; React's scheduler probes
  `MessageChannel`/`setTimeout` at module-init.
- **Priority (abort-on-load → breadth):** T0 `window`/`self`/`globalThis` + `console` +
  `navigator`; T1 `document.body/head/documentElement/readyState` + engine-generated
  `DOMContentLoaded` + `createTextNode`/`insertBefore`/`cloneNode` + traversal + real `Event`
  objects & propagation; T2 engine-generated click/input/change/submit + control IDL
  (`value`/`checked`) + `classList` + `element.style` + inline `on*` + `form.submit()`; T3
  `requestAnimationFrame`/`MessageChannel` + `getComputedStyle` + `localStorage`/`cookie` +
  install `history`/`location`; T4 `dataset`/modules/`matchMedia`. **`console` is essentially
  first** — trivial and unconditionally referenced.
- **Binding strategy:** Servo generates bindings from WebIDL (Python codegen in
  `script_bindings`) — worth it only past ~100 interfaces / when spec-conformant type
  coercion across inheritance is needed. Manuk (~tens of interfaces) should **hand-write on
  the `mozjs` safe layer** (reserved-slot reflector + `JS_DefineFunctions`/`rooted!`), behind
  one `define_interface!` macro capturing the JSClass/spec-table boilerplate. No standalone
  reusable mozjs-DOM-binding crate exists; `mozjs`/`mozjs-sys` (already a dep) is the base.
  All of this is pure FFI/embedding — **never modifies SpiderMonkey**.
- Sources: chromestatus / HTTP Archive Web Almanac 2024-25 · jQuery #3426 · React scheduler
  (jser.dev) · book.servo.org (Implementing a DOM API) · servo/servo#43180 · docs.rs/mozjs

## 5. Networking — **one long-lived runtime is the master fix**

- **Root cause of latency:** a fresh Tokio runtime per navigation destroys the connection
  pool + DNS cache + TLS resumption every load. Fix: one app-lifetime runtime + one
  `hyper_util legacy::Client` (cheap to clone, *is* the pool); restores h2 multiplexing across
  subresources. Prerequisite for parallel fetch.
- **HTTP cache:** `http-cache-semantics` (correct RFC 9111 freshness/`Vary`/revalidation
  headers — decision engine only) + storage via `moka` (memory) + `cacache` (disk),
  hand-glued to hyper. Skip `http-cache-reqwest` (assumes reqwest middleware).
- **Images:** `image` (png/jpeg/gif/webp/bmp/ico/tiff; AVIF via C `dav1d` feature — no prod
  pure-Rust AVIF decoder) + `resvg`/`usvg` for SVG (renders to tiny-skia RGBA8, shares
  fontdb). Normalize all to one RGBA8 bitmap struct + a `PaintImage` display item, decode
  off-thread, key by URL to align with the HTTP + image caches.
- **Web fonts:** `woff2` (decompress WOFF2→sfnt; watch the `hmtx`-transform gap) →
  `fontdb::load_font_data` (shared with resvg) → `skrifa`/`read-fonts` for metrics/outlines.
- **Cookies:** wire the existing RFC 6265 jar at the send site **and inside the redirect loop**
  (store `Set-Cookie` before the next hop, recompute `Cookie` per new host); SameSite enforced
  in the attach decision (site-context), `publicsuffix` to reject public-suffix cookies.
- **DNS/parallel:** `hickory-resolver` via `hyper-hickory` (caching, on the shared runtime) +
  `tokio::sync::Semaphore` + `JoinSet`/`FuturesUnordered` with a browser priority ladder.
- Sources: docs.rs/http-cache-semantics · github.com/image-rs/image · github.com/linebender/resvg
  · docs.rs/woff2 · docs.rs/read-fonts · seanmonstar.com (hyper-util pools) · docs.rs/hickory-resolver
