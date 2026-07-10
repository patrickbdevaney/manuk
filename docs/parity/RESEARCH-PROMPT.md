# Deep Research Prompt — Full Browser Capability Parity (Chromium/Gecko)

**Purpose.** Drive an exhaustive survey of what a modern browser engine
(Chromium/Blink, Gecko/Firefox, and where instructive WebKit and Servo) actually
implements to browse the real web *headfully and usably*, so Manuk can be brought to
qualitative capability parity for core browsing **before** any agentic/headless-agent
or advanced-ergonomics work. The output feeds `PARITY-IMPLEMENTATION.md`, which records
the delta against `STOCKTAKE.md` (what Manuk has today) and the ordered plan to close
it.

**Framing.** Manuk is a from-scratch Rust engine (workspace: net, dom, html, css,
layout, text, js, paint, compositor, page, shell, agent). It embeds SpiderMonkey via
mozjs for JS. It already has: html5ever parsing, an arena DOM, a hand-rolled CSS
subset + taffy flex/grid, floats/tables/abs positioning, a CPU tiny-skia painter with
fontdb+fontdue text, hyper+rustls networking with a cookie library, and a winit/wgpu
shell with omnibox/history/find. It is **missing** most of the CSSOM/BOM/DOM JS surface,
images, `font-family`/real shaping, overflow/z-index, incremental layout, an HTTP cache,
wired cookies, a real compositor, and polished chrome UI. See `STOCKTAKE.md` for the
line-cited baseline.

**Guardrails for the research.**
- Prioritize the **80/20 that makes the top ~1000 sites usable**, not spec
  completeness for its own sake. Rank everything by real-web frequency and by whether
  its absence *blocks* vs *degrades* a page.
- Respect the CLAUDE.md **JS-engine boundary**: SpiderMonkey is embedded via sanctioned
  FFI only — never patch its internals. Research the *embedding/binding* surface, not
  engine modifications.
- Honesty over completeness: where parity is architecturally out of reach for a
  from-scratch effort (full CSS animation compositor, GPU process isolation, the entire
  web-platform test surface), say so and define the *pragmatic* target instead.
- Every recommendation must name **specific, mature Rust crates** where they exist
  (e.g. `image`, `resvg`/`usvg`, `rustybuzz`/`swash`/`cosmic-text`/`parley`,
  `unicode-bidi`, `selectors`+`cssparser` from Servo, `style`/Stylo, `http-cache-semantics`
  ports) and note licensing/build-cost tradeoffs.

---

## Research questions, by subsystem

For **each** area below, the research must produce: (1) the concrete capability list a
modern browser ships; (2) the subset that is *load-bearing* for common sites, ranked;
(3) the recommended Rust implementation approach + crates; (4) the explicit delta vs
Manuk's `STOCKTAKE.md` baseline; (5) a rough effort/level tag (S/M/L) and dependencies.

### A. CSS: cascade, selectors, values, properties
- The full selector grammar browsers match (combinators, attribute selectors, the
  pseudo-class/pseudo-element catalogue, `:is()/:where()/:has()`, specificity rules)
  and which are load-bearing.
- The cascade origins/layers (UA/user/author, `!important` inversion, `@layer`),
  inheritance, and the CSS-wide keywords (`inherit/initial/unset/revert`).
- Custom properties + `var()` substitution; `env()`, `min()/max()/clamp()`.
- At-rules that matter: **`@media`** (and the media-query grammar/features to support
  first), `@font-face`, `@supports`, `@keyframes`, `@import`, `@page`.
- The property set ranked by frequency: which 150–200 properties cover the real web,
  and specifically the currently-missing high-frequency ones (`font-family`/`font`
  shorthand, `overflow`, `opacity`, `border-radius`, `box-shadow`, gradients/
  `background-image`, `visibility`, `text-decoration`, `letter-spacing`,
  `text-transform`, `object-fit`, `aspect-ratio`, logical properties, `hsl()/hwb()/
  oklch()`, `currentColor`, viewport/`ch`/`ex` units).
- **Build-vs-adopt decision:** should Manuk grow its hand-rolled matcher, adopt Servo's
  `selectors`+`cssparser`+`style`(Stylo) crates, or a hybrid? Quantify what wiring real
  Stylo (already scaffolded, `stylo_engine.rs`) buys and costs.

### B. Layout & fragmentation
- The box/fragment model browsers use (box tree vs fragment tree, intrinsic-sizing
  passes) and how incremental/partial layout + invalidation works (dirty bits, subtree
  relayout, layout roots) — Manuk does full-document relayout on every change; research
  the minimal architecture to make interactive pages snappy.
- **`overflow`** (visible/hidden/scroll/auto/clip), scroll containers, scrollbar
  geometry, and clipping — currently absent and load-bearing.
- **Stacking contexts & `z-index`** paint ordering; when contexts are established.
- `position: sticky`; static-position resolution for inset-less absolutes.
- Inline/text layout to parity: **real shaping** (kerning, ligatures, GPOS/GSUB),
  **bidi/RTL** (`unicode-bidi`), `white-space` variants with preservation,
  `letter/word-spacing`, `text-overflow`, `writing-mode`. Recommend the shaping stack
  (`rustybuzz` vs `swash` vs `cosmic-text`/`parley`) given Manuk's fontdb/fontdue base.
- Replaced elements: image intrinsic sizing/aspect-ratio, `object-fit`.
- Generated content / list markers (`::before/::after`, `list-style`).

### C. HTML/DOM completeness
- Context-aware **fragment parsing** for `innerHTML` (table/select/template contexts).
- **SVG/MathML namespaces** in the tree (currently flattened) — what's needed for SVG
  to render at all, and the `resvg`/`usvg` path.
- `document.write` during parse; the **scripting flag** and `<noscript>` semantics.
- Encoding on the streaming path; form-owner association.
- The DOM mutation/traversal API surface JS needs (see D).

### D. JavaScript integration & the web-platform API surface
- The **minimum BOM** for JS-driven sites to boot without immediately throwing:
  `window`/`self`/`globalThis` identity, **`console`**, `navigator` (userAgent/
  language/platform), `location`, `history` (already coded — just unwired),
  `setTimeout(delay,args)/setInterval/clearTimeout/requestAnimationFrame`,
  `localStorage`/`sessionStorage` (backend exists — bind it), `document.cookie`.
- The **DOM API** delta: traversal (`parentNode`/`children`/`firstChild`/siblings),
  mutation (`createTextNode`/`insertBefore`/`replaceChild`/`cloneNode`/`append`/
  `before`/`after`/`insertAdjacentHTML`), `classList`, `dataset`, `attributes`,
  `document.body/head/documentElement/title/forms/readyState`, live `HTMLCollection`/
  `NodeList`.
- **CSSOM:** `element.style` (`CSSStyleDeclaration`), `getComputedStyle`,
  `document.styleSheets`.
- **Events done right:** a real `Event`/`CustomEvent` object, capture/bubble
  propagation, `target`/`currentTarget`, `preventDefault`/`stopPropagation`, listener
  options; **engine-generated events** (click/input/change/submit/keydown/load/
  DOMContentLoaded) dispatched from the shell's real input; **inline `on*` handlers**
  and `on*` IDL properties.
- Control **IDL reflection** (`value`/`checked`/`disabled`/`href`/`src`), `form.submit()`.
- `fetch` to spec (real Promise, `Response.json()`, `Headers`, `AbortController`),
  `MutationObserver`, ES modules. Rank by how many real sites each unblocks.
- The **binding strategy**: hand-written bindings on the safe helper layer (current
  approach) vs Servo-style WebIDL codegen — at what surface size does codegen win?

### E. Networking & loading
- The **resource loading pipeline**: preload scanner, parallel subresource fetch,
  priorities, the interactive path fetching CSS/JS/img (Manuk's GUI path fetches none).
- **HTTP caching** (RFC 9111): memory+disk cache, conditional requests (ETag/
  If-Modified-Since/304), `Cache-Control`/`Vary`. Recommend the crate/approach.
- **Wiring the cookie jar** into the request/response path; SameSite/Secure on the wire.
- Connection reuse across navigations (Manuk defeats its own pool with a per-nav
  runtime); DNS caching/prefetch; preconnect wiring.
- Redirect correctness (method preservation, cross-origin auth/cookie stripping),
  HTTP/3/QUIC (worth it?), zstd, web-font fetching.
- Image loading + the decoder set (`png`/`jpeg`/`webp`/`gif`/`avif`/SVG) via `image`+.

### F. Text, fonts, paint, compositing
- The shaping + rasterization stack for crisp, correctly-spaced text: shaper choice,
  **subpixel glyph positioning**, hinting, grayscale vs LCD AA, **glyph atlas/raster
  cache**, **per-glyph font fallback** (CJK/emoji), color/emoji (COLR/CBDT), web fonts.
- The paint model: display-list caching, the primitive set needed beyond rect+text
  (images, rounded rects, gradients, shadows, clips, opacity/blend groups).
- The **compositor**: layers, tiling, partial invalidation/damage (Manuk repaints the
  whole viewport + re-uploads the whole texture every frame), scroll-blit,
  GPU rasterization (Vello) vs the current CPU tiny-skia + fullscreen-quad present —
  when is GPU raster worth the complexity?

### G. Browser chrome & UX (headful usability)
- The concrete UI a usable browser needs and Manuk lacks: real **toolbar buttons**
  (back/forward/reload/home with hover/press/disabled states + icons), a **tab strip**,
  a **suggestions dropdown**, a proper **address bar** (rounded field, focus ring,
  security/lock, favicon, bookmark star, clear), a **loading/progress indicator**, and
  **menus**. Survey what's table-stakes vs nice-to-have.
- **Text-field editing to parity:** a real positioned/blinking caret centered on the
  text, caret movement (Left/Right/Home/End/word), selection, copy/paste/cut,
  `<textarea>` multiline, `<select>` dropdowns, and Tab focus traversal — all currently
  append/backspace-only.
- **Responsiveness architecture:** off-main-thread / async navigation so the window
  never freezes on load; input→paint latency budget; how Chromium/Gecko keep the UI
  live during loads.
- Rendering-correctness assurance: extend the layout-parity harness toward visual/pixel
  and interaction parity; where WPT fits.

---

## Deliverable shape the research must return

A single structured document that, for every capability above, gives: **browser
behavior → load-bearing subset (ranked) → recommended Rust approach + crates → delta vs
Manuk baseline → effort/level + dependencies.** It must end with a **dependency-ordered
backlog** partitioned into the three tiers from `STOCKTAKE.md §9` (reported-flow bug
fixes → common-site features → deep architecture), each item sized and sequenced, so
`PARITY-IMPLEMENTATION.md` can be assembled directly from it. Explicitly flag anything
that is *not* worth pursuing for pragmatic core-browsing parity and say why.
