# Manuk — Browser-Parity Implementation Plan (delta + ordered backlog)

Synthesis of the 7-subsystem stock-take (`STOCKTAKE.md`) and the 5 executed research
threads (CSS engine, text stack, paint/compositor, JS APIs + binding strategy,
networking) into the concrete delta and a dependency-ordered plan to reach **qualitative
core-browsing parity** with Chromium/Gecko — before any agentic/headless-agent or
advanced-ergonomics work.

**Guiding principle (from the research):** in almost every subsystem the highest-leverage
move is *not* a rewrite — it is finishing an integration that is already 60–80% scaffolded
(Stylo's matcher, the cookie jar, the `Damage` type, the `history` bindings) or swapping a
default. Rewrites (text→Parley, paint→Vello) are real but deferred and feature-gated.

Effort tags: **S** ≈ hours–1 day, **M** ≈ days, **L** ≈ 1–3 weeks, **XL** ≈ month+.

---

## Progress log (landed, verified)

- **T1.1** search follow-through → `lite.duckduckgo.com` (`c2ae03f`)
- **T1.2** caret tracks the text run (`c2ae03f`)
- **T1.3** one long-lived Tokio runtime — connection-pool reuse (`5149ae3`)
- **T1.7** GUI fetches + applies external stylesheets (`81876ca`)
- **T1.8** glyph raster cache (`b8643e3`) — damage-rect repaint still pending
- **T2E.1** Tier-0 JS globals: window/self/console/navigator (`0938636`)
- **T2A/2B interim** font-family serif/sans/mono (`c27d29f`); extended CSS selectors —
  combinators, attributes, pseudo-classes (`a3edffa`); hsl()/hsla() (`e593ea9`)
- **T2D.1 images** Stage A replaced-element sizing (`7a11291`) + Stage B fetch/decode/paint
  (`d96658c`) — `<img>` now renders, natural or attr/CSS-sized

- **T2A Stylo cascade — ✅ COMPLETE** (`8246d0f` → `f622dcb`): the 107-method DOM trait
  wall (`stylo_traits.rs`), the full `ComputedValues → ComputedStyle` mapping
  (`stylo_map.rs`), and `cascade_via_stylo` (UA sheet + author sheets + inline `style=`,
  matched with Stylo's selector engine, computed via `compute_for_declarations`). Proven
  end-to-end: `var()` resolution, inheritance, UA defaults, inline-style override, and
  geometric properties all flow through real Stylo. `StyloEngine` is now a real
  `StyleEngine`. `MinimalCascade` stays the crate default (hand-tuned to the parity
  harness); Stylo is opt-in under `--features stylo`. Rollout follow-on: a page `stylo`
  feature + re-tuning the parity corpus to Stylo's exact metrics before defaulting to it.

**Remaining flagship architecture:** the Parley text migration (2B) is the last large
integration; the interim slices above improve the shipping default engine in the meantime.

## Key research verdicts (these shape the plan)

1. **CSS → finish Stylo, don't grow the hand-rolled matcher.** The premise shifted: the
   Stylo `selectors::Element` matcher (30 methods) is **already wired and tested** over the
   arena DOM (the recent D2 commits). Only `StyloEngine::cascade` still delegates to
   `MinimalCascade`. Finishing the cascade (implement `TElement`/`TNode` walls +
   `ComputedValues→ComputedStyle`, referencing **Blitz** = Stylo+Taffy+Parley) unlocks
   **`@media`, `var()`, all combinators, pseudo-classes, `font-family`** at once, spec-
   correct. No lightweight standalone crate exists for `var()`/`@media` evaluation — every
   serious project either takes full Stylo (Blitz) or punts the cascade (scraper,
   lightningcss). Keep `MinimalCascade` as the fast default; Stylo behind `--features stylo`.
2. **Text → target Parley + swash + fontique (Blitz-style); cosmic-text is the pragmatic
   on-ramp.** This is a *contained rewrite* of `engine/text` (the char-based `GlyphPos`
   model can't express ligatures/shaping and must become glyph-ID based), touching only
   `engine/text` + the glyph consumer in `engine/paint`. Delivers shaping, kerning,
   bidi (ICU4X), line-breaking, per-run CJK/emoji fallback, `font-family`, subpixel
   positioning, and (via swash) a glyph cache + color emoji.
3. **Paint → the bottleneck is missing invalidation, not the rasterizer.** Stay on CPU
   `tiny-skia`; add damage-rect painting + display-list caching + scroll-blit + partial
   `wgpu::write_texture` + a CPU glyph coverage cache. `tiny-skia` already covers rounded
   rects, gradients, images, clips, and (manual) opacity groups; the only real gap is
   blur (`box-shadow`/`filter`) which *neither* tiny-skia nor Vello implements yet →
   hand-roll a separable Gaussian. Vello is alpha/beta in 2026 → introduce an
   `anyrender`-style swappable backend trait now, add feature-gated `vello_hybrid` later.
4. **JS → ship Tier-0 globals first (they abort load on line 1).** A missing `window`/
   `console`/`navigator` is a `ReferenceError` that kills the whole script before Manuk's
   existing 30-method surface is ever reached. Order by abort-on-load, then breadth.
   **Hand-write bindings** on the `mozjs` safe layer behind one `define_interface!` macro;
   WebIDL codegen only pays off past ~100 interfaces. Never modify SpiderMonkey.
5. **Net → one long-lived runtime + client is the master fix.** The per-navigation Tokio
   runtime defeats the connection pool, DNS cache, and TLS resumption every load. Fixing it
   restores pooling in one move and is the prerequisite for parallel subresource fetch.
   HTTP cache = `http-cache-semantics` (engine) + `moka`/`cacache` (storage), hand-glued to
   hyper. Images = `image` + `resvg`/`usvg`. Web fonts = `woff2` + `fontdb` + `skrifa`.
   Cookies = wire the existing jar at the send site **and every redirect hop**.

---

## Tier 1 — Reported-flow bug fixes (make the core *work*)

Small, high-impact, mostly no new deps. **Two already landed** (commit `c2ae03f`).

| # | Item | Where | Effort | Status |
|---|---|---|---|---|
| 1.1 | Search follows through (default → `lite.duckduckgo.com`) | `chrome.rs:86` | S | ✅ done (`c2ae03f`) |
| 1.2 | Caret tracks the text run, not the box | `gui.rs:480`, `layout value_run` | S | ✅ done (`c2ae03f`) |
| 1.3 | **One long-lived Tokio runtime + hyper client** (kill per-nav runtime → pool/DNS/TLS reuse) | `gui.rs:402,749` | M | ✅ done (`5149ae3`) |
| 1.4 | Async / off-main-thread navigation (window never freezes on load) + loading indicator | `gui.rs:402-432` | M | |
| 1.5 | Honor `<meta http-equiv=refresh>` client redirect | `page` load path | S | |
| 1.6 | Defer/parallelize system-font enumeration off the startup critical path | `gui.rs:175`, `text/lib.rs:107` | S | |
| 1.7 | GUI path fetches + applies external CSS (and subresources) like the render CLI does | `gui.rs:412` | M | |
| 1.8 | Glyph raster cache + display-list cache + damage-rect repaint + partial texture upload | `text/lib.rs:245`, `paint`, `gui.rs:442` | M | |
| 1.9 | Real omnibox caret (positioned/blinking) + suggestions dropdown rendering; align nav-button hit-zones to glyphs | `gui.rs:519,530,559` | M | |
| 1.10 | Text-field editing: caret movement (←/→/Home/End), selection, copy/paste; `<textarea>` multiline | `gui.rs:339-360,850` | M | |

## Tier 2 — Common-site features (make the core *render like Chrome*)

Medium, unlock large classes of real pages. Grouped by subsystem; sequence respects deps.

### 2A. CSS (finish Stylo) — unlocks the most at once
| # | Item | Effort |
|---|---|---|
| 2A.1 | Implement `TElement` (~76) + `TNode` (~20) trait walls on the arena DOM (ref: Blitz `blitz-dom`) | L |
| 2A.2 | Wire `StyloEngine::cascade` to a real `Stylist` → `ComputedValues`; map `ComputedValues → ComputedStyle` | L |
| 2A.3 | Build a `style::media_queries::Device` from the viewport → **`@media`** evaluation | M |
| 2A.4 | Pin exact `stylo`/`selectors`/`cssparser`; CI job to flag breaking bumps | S |

*Result:* `@media`, `var()`/custom props, child/sibling/attribute selectors, all pseudo-
classes/-elements, `font-family` resolution, `hsl()`/`currentColor` — spec-correct, "free."
*Fast-default fallback:* until 2A lands, add the highest-frequency props to `MinimalCascade`
directly (`font-family`, `overflow`, `opacity`, `border-radius`, `box-shadow`, gradients,
`visibility`, `text-decoration`) as an interim so the default build improves too.

### 2B. Text (Parley migration)
| # | Item | Effort |
|---|---|---|
| 2B.1 | Redefine `engine/text` boundary: `GlyphPos{ch}` → glyph-ID runs `{glyph_id,font_id,x,y}`; update `engine/paint` consumer | M |
| 2B.2 | Swap backend to Parley + Fontique + swash (shaping, kerning, bidi, line-break, `font-family`, CJK/emoji fallback) | L |
| 2B.3 | swash `ScaleContext` + glyph cache keyed `(glyph_id,font_id,size,subpx)` + `etagere` atlas; subpixel positioning; color emoji | M |

### 2C. Layout
| # | Item | Effort |
|---|---|---|
| 2C.1 | **`overflow`** (visible/hidden/scroll/auto/clip) + clipping + element scroll containers + scrollbar geometry | L |
| 2C.2 | **Stacking contexts + `z-index`** paint ordering | M |
| 2C.3 | `position: sticky`; static-position resolution for inset-less absolutes | M |
| 2C.4 | Replaced-element intrinsic sizing/aspect-ratio for images; `object-fit` | M |
| 2C.5 | `white-space` pre/pre-wrap preservation; `text-align:justify`; parent/child margin collapsing | M |

### 2D. Images + web fonts + networking
| # | Item | Crates | Effort |
|---|---|---|---|
| 2D.1 | Image loading + decode + a `PaintImage` display item (off-thread, URL-keyed) | `image` (+`dav1d` AVIF), `resvg`/`usvg` | M |
| 2D.2 | Web fonts (`@font-face` + remote WOFF2) into the font DB | `woff2`, `fontdb`, `skrifa` | M |
| 2D.3 | Wire the cookie jar into `send_raw` + every redirect hop; SameSite in attach logic | existing jar, `publicsuffix` | M |
| 2D.4 | HTTP cache (memory+disk) + conditional requests (ETag/If-Modified-Since→304) | `http-cache-semantics`, `moka`, `cacache` | M |
| 2D.5 | Parallel prioritized subresource fetch (semaphore + JoinSet); caching DNS resolver | `hickory-resolver`/`hyper-hickory`, `tokio` | M |

### 2E. JavaScript / web APIs (ordered by abort-on-load)
| # | Item | Effort |
|---|---|---|
| 2E.1 | **Tier-0 globals:** `window`/`self`/`globalThis` identity + `console.*` + `navigator` (honest UA) | S | ✅ done (`0938636`) |
| 2E.2 | `document.body`/`head`/`documentElement`/`title`/`readyState`; **engine-generated `DOMContentLoaded`** | M |
| 2E.3 | DOM mutation+traversal: `createTextNode`, `insertBefore`, `cloneNode`, `parentNode`/`children`/`firstChild`/siblings | M |
| 2E.4 | Real `Event`/`CustomEvent` objects + capture/bubble + `target`/`preventDefault`/`stopPropagation`; listener options | M |
| 2E.5 | **Engine-generated events** (click/input/change/submit/keydown) from real shell input; inline `on*` handlers | M |
| 2E.6 | Control IDL (`value`/`checked`/`disabled`/`href`); `form.submit()`; `classList`; `element.style`; `getComputedStyle` | M |
| 2E.7 | Wire the already-coded `history`/`location`; bind `localStorage`/`sessionStorage` (backend exists); `document.cookie` | S–M |
| 2E.8 | `requestAnimationFrame`, `setInterval`/`clearTimeout`, `MessageChannel` (React scheduler); `setTimeout` delay/args | M |
| 2E.9 | Extract a `define_interface!` macro for the reflector/JSClass/spec-table boilerplate | M |

### 2F. Chrome / UX polish
Real toolbar buttons (hover/press/disabled + icons); a **tab strip**; a proper address bar
(rounded, focus ring, security/favicon/bookmark star, clear); loading/progress indicator;
menus; Tab focus traversal; `<select>` dropdowns. Effort: **L** across the set.

## Tier 3 — Deep architecture (full parity)

`MutationObserver`; ES modules (module loader); context-aware fragment parsing (`<tr>`/
`<option>` `innerHTML`); SVG/MathML namespaces in the arena; bidi/RTL end-to-end +
writing-modes (much comes with Parley); incremental/partial **layout** (subtree relayout,
layout roots — the deepest perf item); a real compositor (layers/tiling) and a feature-
gated **Vello Hybrid** GPU backend behind an `anyrender`-style trait; HTTP/3; `fetch` to
spec (real Promise/`Response.json`/`Headers`/`AbortController`); `document.write`. Effort:
**L–XL** each; schedule only after Tier 1–2 make core browsing solid.

---

## Sequencing (critical path)

```
Tier 1 (1.3 runtime → 1.4 async nav → 1.7 GUI subresources → 1.8 caches) ── snappy + loads CSS
        │
        ├─ 2A Stylo cascade ─────────► @media/var()/selectors/font-family (biggest render unlock)
        ├─ 2B Parley text ──────────► shaping/bidi/subpixel/fallback  (parallel to 2A)
        ├─ 2D images/fonts/cookies/cache (parallel; 2D.1 needs 2C.4 image sizing)
        ├─ 2C overflow/z-index/sticky
        └─ 2E JS Tier-0 → events → IDL  (2E.6 getComputedStyle needs 2A/2B computed values)
        │
        └─ 2F chrome UX polish (parallel, low coupling)
                │
                └─ Tier 3 (incremental layout, compositor/Vello, modules, SVG, HTTP/3)
```

**Milestones.** M1 *Snappy & complete-load* (Tier 1) — no freeze, pooled net, external CSS,
cached paint. M2 *Renders like Chrome* (2A+2B+2D.1) — real fonts+shaping, `@media`/`var()`,
images. M3 *Interactive* (2C.1 overflow + 2E.1–2E.6) — SPAs mount and respond. M4 *Polished*
(2F). Only after M1–M4: revisit agentic/headless-agent and advanced ergonomics.

**Verification.** Extend the existing layout-parity harness (70/70 box-geometry probes)
toward visual/pixel and interaction parity; add WPT once the P0.3 runner lands. Every item
lands with a test and an honest note where parity is approximate.
