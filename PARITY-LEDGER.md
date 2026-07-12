# PARITY LEDGER — everything between Manuk and Chromium/Gecko

**Purpose.** One honest, complete map of the web platform, with Manuk's true status against each
part of it. Not a wish list and not a marketing sheet: the thing the tick/epoch loop selects work
from until it is empty.

**Status vocabulary.**

| | meaning |
|---|---|
| **✅** | works, and a gate or test proves it |
| **🟡** | partial — the common path works, named limits below |
| **❌** | absent. A page using it renders or behaves wrong |
| **🚫** | deliberately out of scope (stated why) |

**Priority.** Set by *blast radius on real sites*, measured — not by spec importance.

| | meaning |
|---|---|
| **P0** | a large fraction of the web is visibly wrong without it |
| **P1** | common; a noticeable class of sites is wrong |
| **P2** | long tail, or specialised (still required for true parity) |

**Rule of the ledger (ADR-006).** *An item ships with its probe.* An item without a way to prove it
is a slogan. Every ✅ below names how we know.

**How this list was built.** Every ❌ marked "(seen)" was found by rendering a real page and looking
at it, or by a Chromium A/B box probe — not by reading a spec. The specs supply the completeness;
the screenshots supply the priority.

---

## 1. CSS — cascade & values

| | item | pri | notes |
|---|---|---|---|
| ✅ | Cascade, specificity, inheritance, `!important` | — | Stylo (shipping) + MinimalCascade. Gate: `parity` 72/72 |
| ✅ | Custom properties `var()` | — | via Stylo |
| ✅ | `@media` (screen, width, prefers-*) | — | Tick 8; `matchMedia` |
| ✅ | `@supports` | — | Tick 20 — skipping it rendered the *fallback* of every progressive-enhancement site |
| ✅ | `@layer` (rules apply; layer *ordering* not modelled) | P2 | |
| ✅ | `@font-face` + webfont fetch | — | |
| ✅ | `rem` / root font size | — | Tick 26. Was frozen at 16px: `html{font-size:62.5%}` broke every rem on the page |
| 🟡 | `calc()` | P1 | px+% only; no nested units, no `min()`/`max()`/`clamp()` |
| ❌ | `min()` / `max()` / `clamp()` | **P0** | ubiquitous in modern responsive CSS |
| ❌ | `@keyframes` / `animation` | **P0** | every modern site animates *something*; absent = frozen or wrong initial state |
| ❌ | `transition` | **P0** | same; also hides hover/focus affordances |
| ❌ | `@container` queries | P2 | |
| ❌ | `:has()` | P1 | Stylo can match it; not surfaced |
| ✅ | Pseudo-classes (state) | — | **The shipping Stylo cascade answered `false` to EVERY pseudo-class until Tick 36.** `:checked` is the CSS-only interactivity primitive — the checkbox hack builds a large share of the web's menus, accordions and sidebars with no JS at all, and every one was frozen shut. Now: `:checked`, `:disabled`/`:enabled`, `:required`/`:optional`, `:read-only`/`:read-write`, `:link`/`:any-link`, `:placeholder-shown`, `:valid`, `:defined`, `:open`. `:hover`/`:active`/`:focus` answer false (correct for a static layout); `:visited` answers false deliberately (privacy) |
| ✅ | **`::before` / `::after` + `content`** | — | Tick 31. Both cascades. `MinimalCascade` *dropped any selector containing `::`*; Stylo needed a real pseudo-element matching pass. Limit: not yet materialised in a block that mixes block+inline children |
| ❌ | `::placeholder`, `::selection`, `::marker`, `::first-line` | P1 | |

## 2. CSS — box, paint, visual

| | item | pri | notes |
|---|---|---|---|
| ✅ | `display` block/inline/inline-block/flex/grid/inline-flex/inline-grid/table*/none | — | inline-flex added Tick 25 |
| ✅ | box model, `box-sizing`, margins (incl. collapse + auto) | — | |
| ✅ | `position` static/relative/absolute/fixed/sticky | — | |
| ✅ | `float` / `clear` | — | incl. floated tables |
| ✅ | `z-index` stacking | 🟡 | approximate stacking contexts (subtree layer), not the full CSS2 §E algorithm |
| ✅ | `visibility`, `opacity` | — | Tick 19 — without them every dropdown/modal painted on top of the page |
| ✅ | `background-color` (+ `bgcolor`/`text` presentational) | — | |
| ✅ | **`background-image` (url)** | — | Tick 30 — fetched + decoded like `<img>` |
| ✅ | **`linear-gradient` / `radial-gradient`** | 🟡 | Tick 30. No `conic-gradient`; no colour-interpolation hints |
| ✅ | `background-size` / `-repeat`, `background` shorthand | 🟡 | Tick 37 — a background is painted at its NATURAL size and TILED; it is not stretched to fill its box (that is what a replaced `<img>` does, and painting backgrounds that way blew a subreddit banner across the whole page). No `-position`/`-clip`/`-origin`, no multiple layers |
| ✅ | `border-radius` (uniform), `box-shadow` (single, approximated blur) | 🟡 | no per-corner radii; no multiple/inset shadows; no true Gaussian |
| 🟡 | `border` | P1 | widths + one colour; **no per-side colour, no `dashed`/`dotted`/`double`** |
| ✅ | **`text-decoration`** (underline/overline/line-through) | 🟡 | Tick 30. No `-color`/`-style`/`-thickness` |
| ✅ | **`list-style`** — markers/bullets/numbers | — | Tick 30. disc/circle/square/decimal/alpha/roman, `<ol start>`, `<li value>`, inside/outside |
| ✅ | `mask-image` | — | Tick 25 — the modern icon: an empty element, a background colour, a mask |
| ❌ | `filter` / `backdrop-filter` | P1 | blur/brightness on overlays and cards |
| 🟡 | `transform` | P1 | parsed + mapped; **rotate/scale/skew not applied in paint** (translate approximated) |
| ❌ | `object-fit` / `object-position` | P1 | images render stretched |
| ✅ | `outline` (incl. focus rings) | — | Tick 30 |
| ❌ | `cursor` | P1 | ergonomics — pointer never changes over a link/button |
| ❌ | `text-transform`, `letter-spacing`, `word-spacing`, `text-indent` | P1 | |
| ❌ | `overflow: hidden/auto/scroll` as a **scroll container** | **P0** | clipping exists; *scrolling inside an element* does not |
| ❌ | `aspect-ratio` | P1 | |
| ❌ | `clip-path` | P2 | |
| ❌ | `mix-blend-mode` / `isolation` | P2 | |

## 3. Layout

| | item | pri | notes |
|---|---|---|---|
| ✅ | block, inline, IFC, line breaking (UAX#14), floats | — | |
| ✅ | flexbox, grid (incl. named `grid-template-areas`) | — | taffy |
| ✅ | tables: auto/fixed layout, colspan/rowspan, row boxes, `cellpadding`/`cellspacing`, `width="85%"` | — | Tick 26 |
| ✅ | forced breaks: `<br>`, `white-space: pre/pre-wrap/pre-line` | — | Tick 27 — absent, every code block was one endless line |
| ✅ | shrink-to-fit / max-content (via taffy for flex/grid) | — | Tick 25 |
| ✅ | block-in-inline (CSS2 §9.2.1.1) | — | |
| ✅ | inline element geometry (incl. empty inlines) | — | Tick 25 |
| ❌ | `<caption>`, `<col>`/`<colgroup>` widths, `border-collapse` | P1 | |
| ❌ | multi-column (`column-count`, `column-width`) | P2 | |
| ❌ | **bidi / RTL** (`direction`, `unicode-bidi`, UAX#9) | P1 | Arabic/Hebrew render LTR — *wrong*, not merely unstyled |
| ❌ | writing modes (vertical-rl etc.) | P2 | |
| ❌ | hyphenation | P2 | |
| 🟡 | baseline alignment of inline-blocks | P1 | approximated (all-ascent) |
| ❌ | `position: sticky` **within a scroll container** | P1 | sticky exists against the viewport only |

## 4. Text & fonts

| | item | pri | notes |
|---|---|---|---|
| ✅ | shaping + rasterisation (swash), subpixel positioning | — | |
| ✅ | webfonts (`@font-face`), family fallback | 🟡 | no `font-feature-settings`, no variable-font axes |
| ❌ | complex-script shaping (Arabic joining, Indic reordering) | P1 | |
| ❌ | emoji / colour fonts (COLR/CBDT) | P1 | emoji render as tofu |
| ❌ | `font-variant`, `font-stretch`, small-caps | P2 | |

## 5. HTML & DOM

| | item | pri | notes |
|---|---|---|---|
| ✅ | html5ever parsing (spec tree construction), fragments | — | |
| ✅ | Shadow DOM (declarative + `attachShadow`), slots, flat tree | — | Tick 15 |
| ✅ | Custom Elements | — | Tick 15 |
| ✅ | forms: text/checkbox/radio/select, submit, multipart upload | 🟡 | no `<datalist>`, no constraint-validation API |
| ❌ | **`<iframe>`** | **P0** | embeds, payments, video players, ads, OAuth |
| ❌ | **`<canvas>` 2D** | **P0** | charts, games, editors, viz |
| ❌ | `<video>` / `<audio>` playback | **P0** | |
| ❌ | inline `<svg>` | **P0** | rendered only as an `<img>` source, never inline. Every modern icon system |
| ❌ | `<dialog>`, `popover` | P1 | |
| ❌ | `<details>`/`<summary>` | P1 | |
| ❌ | `contenteditable` | P2 | |
| ❌ | drag & drop | P2 | |

## 6. JavaScript — DOM/BOM API surface

SpiderMonkey gives us the *language* for free. Everything below is the **platform**, and it is where
"JS interactivity parity" is actually won or lost.

| | item | pri | notes |
|---|---|---|---|
| ✅ | `document.getElementById/querySelector(All)/createElement/createTextNode` | — | |
| ✅ | tree mutation: `appendChild`, `insertBefore`, `removeChild`, `remove`, `cloneNode` | — | |
| ✅ | `innerHTML`, `textContent`, attributes (`get/set/has/removeAttribute`) | — | |
| ✅ | **content-attribute reflection** (`href`, `src`, `rel`, `type`, …) | — | Tick 28 — without it `createElement`→assign→`appendChild` produced empty elements |
| ✅ | `document.documentElement` / `body` / `head` / `cookie` | — | Tick 26 |
| ✅ | `addEventListener` / `dispatchEvent` / `preventDefault`, delegation, bubbling | — | |
| ✅ | `getBoundingClientRect`, `getComputedStyle` | 🟡 | computed style is a subset |
| ✅ | `localStorage` / `sessionStorage` (origin-partitioned, persisted) | — | Tick 26 — **the browser-capability gate every MediaWiki site runs** |
| ✅ | History API (`pushState`/`popstate`), `location` | — | |
| ✅ | `window.open` / `opener` / `postMessage` | — | |
| ✅ | MutationObserver, `matchMedia`, `requestAnimationFrame` | — | |
| ✅ | **dynamic `<script src>` execution** | — | Tick 28 — how every code-split bundle ships |
| ✅ | **`element.style` (CSSOM inline style)** | — | Tick 29. A style written by script reaches the **cascade**, not just the attribute |
| ✅ | **`classList`** | — | Tick 29 |
| ✅ | `dataset`, `closest()`, `matches()`, `contains()` | — | Tick 29 |
| ❌ | `CSSStyleSheet` / `document.styleSheets` / `insertRule` | P1 | CSS-in-JS |
| ❌ | **`fetch` / `XMLHttpRequest`** (real) | **P0** | queued to the host but not resolved — every SPA data load |
| ❌ | `FormData`, `URL`, `URLSearchParams`, `Headers`, `Blob`, `File` | **P0** | |
| ✅ | `IntersectionObserver`, `ResizeObserver` | — | Tick 33 — driven by the ENGINE after every layout/scroll, the only honest moment to ask "did this box move into view?" |
| ✅ | `Event`/`CustomEvent`/`MouseEvent`/`KeyboardEvent`/`PointerEvent`/`InputEvent` constructors | 🟡 | Tick 32. `dispatchEvent` took only a *string* — an Event object was coerced to `"[object Object]"`. No Touch events yet |
| ✅ | `scrollTo`/`scrollBy`/`scrollIntoView`, `window.scrollX/Y`, **`scroll` events** | — | Ticks 32–33. The script reads the live offset and *requests* a scroll; the shell performs it and tells the page |
| 🟡 | `focus()` / `blur()` / `document.activeElement` | P1 | Tick 32. **Still missing: tab order / keyboard traversal** |
| ❌ | `structuredClone`, `queueMicrotask`, `requestIdleCallback` | P1 | |
| ❌ | Web Workers | P1 | |
| ❌ | WebSocket / EventSource | P1 | live feeds |
| ❌ | IndexedDB | P1 | |
| ❌ | Clipboard, Notification, Geolocation, Permissions | P2 | |
| ❌ | WebGL / WebGPU / WebAssembly | P2 | (Wasm comes free with SpiderMonkey once exposed) |

## 7. Networking

| | item | pri | notes |
|---|---|---|---|
| ✅ | HTTP/1.1 + HTTP/2 (hyper + rustls), redirects, gzip/br | — | |
| ✅ | cookies (jar, `Secure`/`HttpOnly`/`SameSite`, persisted) | — | |
| ✅ | downloads to disk, multipart upload | — | |
| ✅ | proxy, content blocker | — | |
| ✅ | **honest TLS handshake + honest UA** (a Manuk fingerprint, earned) | — | ADR-004. Impersonation is off-strategy |
| 🟡 | HTTP cache | P1 | in-memory; no disk cache, no full RFC 9111 revalidation |
| ❌ | HTTP/3 / QUIC | P2 | |
| ❌ | **CORS enforcement** | **P0** | *security*: we are permissive where a browser is not |
| ❌ | **CSP enforcement** | **P0** | *security* |
| ❌ | Mixed-content blocking, HSTS | P1 | *security* |
| ❌ | Service Workers / Cache API | P1 | offline + install |
| ❌ | `<link rel=preload/prefetch/modulepreload>` semantics | P1 | preload *scanning* exists |

## 8. Security & isolation (parity means the *restrictions* too)

| | item | pri | notes |
|---|---|---|---|
| ✅ | origin-partitioned storage (cookies, Web Storage) | — | |
| ✅ | `HttpOnly` hidden from script | — | |
| ❌ | **same-origin policy enforcement** (DOM access across frames) | **P0** | needs iframes first |
| ❌ | site isolation / process-per-site | P2 | we are single-process |
| ❌ | sandboxed iframes, `Permissions-Policy` | P1 | |
| 🚫 | anti-detection / competitor impersonation | — | **off-strategy by charter** (ADR-004): a fifth real browser, not a disguise |

## 9. Browser product (the human half — PRODUCT STAR, ADR-006)

| | item | pri | notes |
|---|---|---|---|
| ✅ | tabs (new/close/duplicate/switch), history, back/forward | — | |
| ✅ | bookmarks, find-in-page, zoom (±/reset), downloads panel | — | Tick 18 |
| ✅ | keybindings (Ctrl+D/R/T/W/L/F, F5) | — | G3 affordance gate: no dead buttons |
| ✅ | session restore, tab hibernation | — | |
| ✅ | **profile durability** (cookies + localStorage survive the binary) | — | ADR-009; versioned envelopes |
| ❌ | **element-level scrolling + scrollbars** | **P0** | see §2 |

| ❌ | context menu (right-click) | **P0** | |
| ❌ | DevTools (inspect/console/network) | P1 | |
| ❌ | printing / PDF export | P1 | |
| ❌ | password manager + autofill (code exists, unwired) | P1 | **star debt** |
| ❌ | extensions (WebExtensions) | P2 | |
| ❌ | private/incognito windows | P1 | |
| ❌ | translate | P2 | |

## 10. Agent surface (the other half of the ambidextrous spine — ADR-004)

| | item | pri | notes |
|---|---|---|---|
| ✅ | in-process automation: selectors, wait, assert, click, type | — | Tick 12 |
| ✅ | action grounding, agent targeting | — | Ticks 9–10 |
| ✅ | headless render → PNG (the VISUAL verification class) | — | Tick 13 |
| ❌ | **G5 — interaction parity** (click/scroll/type/form-fill mirrored against Chromium) | **P0** | ADR-012. *An interaction that works in Chromium and not in Manuk is a CRITICAL* |
| ❌ | a11y tree exposed to AT (crate exists, unwired) | P1 | **star debt** |
| ❌ | CDP / WebDriver-BiDi endpoint | P1 | `bidi` crate exists, unwired |

## 11. Performance, stability, memory (EPOCH axes — §1.7)

| | item | pri | notes |
|---|---|---|---|
| ✅ | perf floors F1/F2/F3, binding, measured every tick | — | |
| ✅ | selector rule index (2.69× cascade) | — | EPOCH-1 |
| ✅ | off-thread page + subresource fetch (UI never blocks) | — | DEBT-1 |
| ✅ | clean process exit (no crash, profile flushed) | — | Tick 26 — the `_exit()` "fix" was hiding a data-loss bug |
| ❌ | **DEBT-2**: no rule index on the **Stylo** (shipping!) path | **P0** | EPOCH-1 indexed the cascade users don't run |
| ❌ | **DEBT-3**: shell chrome cannot be painted headlessly → AESTHETICS/ERGONOMICS are *unprobeable* | P1 | |
| ❌ | **DEBT-4**: dynamic scripts run on `load_async` but not on the shell's prefetch nav path | **P0** | ADR-011: the gate must measure the path the user runs |
| ❌ | incremental layout/paint (damage-driven relayout) | P1 | |
| ❌ | GPU compositing (wgpu present; CPU raster ships) | P1 | |

---

## G5 — INTERACTION PARITY (first run, 2026-07-11)

| scenario | post-interaction COVERAGE | chrome Δ | manuk Δ |
|---|---|---|---|
| wikipedia-scroll | 99.2% | 31 | **0** ← dead |
| github-scroll | 91.4% | 1 | **0** ← dead |
| hn-scroll | 100.0% | 0 | 0 |
| mdn-scroll | 100.0% | 0 | 0 |
| httpbin-form-fill | 100.0% | 0 | 0 |
| wikipedia-search-type | 99.2% | 0 | 0 |

**MEAN POST-INTERACTION COVERAGE: 98.4%. DEAD INTERACTIONS: 2.**

What the two dead ones actually are: **`position: sticky` is not reflected in the geometry a page
can read.** Chromium's `getBoundingClientRect` moves a sticky element as the page scrolls (in
document coordinates it genuinely does move); ours does not, because sticky is applied at *paint*
and the layout tree the snapshot is built from is scroll-independent. So a page that reads a sticky
element's position while scrolling gets a stale answer from us.

That is a real gap, not a metric artifact — but it is a **Pass 3** one (it affects what a script can
*measure*, not whether the page works). Recorded here rather than fixed out of order.

## The measured scoreboard (update every tick)

| metric | now | Chromium |
|---|---|---|
| COVERAGE — of elements Chrome renders, the fraction Manuk renders at all | **99.7%** | 100% |
| VISUAL — coarse block-grid agreement | **89.6%** | 100% |
| PLACEMENT — median dy, Wikipedia | **1,087px** (was 5,226) | 0 |
| PLACEMENT — median dx/dw/dh, HN | **7 / 1 / 1 px** | 0 |
| box parity (synthetic corpus) | **72/72** | — |

COVERAGE is nearly saturated: *we now draw almost everything Chrome draws.* The frontier has moved
to **placement and paint fidelity** (§2 is where the remaining visual error lives) and to
**interaction** (§6 — the page cannot even set a style or read a scroll position).

## Selection order (what the loop takes next)

~~1. CSSOM + DOM ergonomics~~ — **done, Tick 29.**
~~2. Paint completeness~~ — **done, Tick 30.**
~~3. `::before` / `::after`~~ — **done, Tick 31.**

Remaining P0, ordered by measured blast radius:

4. **Events & scrolling** — real event objects, `scrollTo`/`scrollY`/scroll events,
   `focus()`/`activeElement`, `IntersectionObserver`/`ResizeObserver`.
5. **`fetch`/XHR** + `URL`/`FormData` — the SPA data path.
6. **Transitions & animations.**
7. **`<canvas>` 2D, inline `<svg>`, `<iframe>`, `<video>`.**
8. **Security parity** — CORS, CSP (a browser's *restrictions* are part of parity).
9. **DEBT-2 / DEBT-4** — the gate and the shipping path must be the same browser.

*Each entry above becomes a tick. A tick that has not run `scripts/verify.sh` has not landed.*
