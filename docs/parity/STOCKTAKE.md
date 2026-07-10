# Manuk — Browser-Parity Stock-Take (2026-07-10)

An honest, evidence-backed inventory of what the engine does today versus what
Chromium/Gecko do, produced before any parity work so the delta is real and not
guessed. Every claim is anchored to `file:line`. This is step 1 of the parity
initiative (stock-take → research prompt → research → delta/implementation doc →
execute).

**Method.** Seven parallel subsystem audits (CSS, layout, HTML/DOM, JS/web-APIs,
networking, shell/UI, paint/text/compositor) plus hands-on reproduction of the
user-reported failures against the real `manuk render`/`browse` binary and headless
Chrome.

---

## 0. The reported symptoms, diagnosed to root cause

| Symptom (user) | Root cause (evidence) |
|---|---|
| "The search bar can't ingest a search term / Google search doesn't load" | The omnibox **does** turn a term into a search URL (`chrome.rs:132`) and GET forms **do** submit (`gui.rs:322`). But the default search template is DuckDuckGo's **JS-heavy** page (`chrome.rs:86`) / Google's (`chrome.rs:186`), both of which need JS+cookies to render results. Google's no-JS results page is a `<noscript>` shell with a `<meta http-equiv=refresh>` we don't honor → renders **8px tall**. Proof it's a *choice*, not a *capability* gap: `lite.duckduckgo.com/lite/?q=rust` renders **1103px of real, clickable results** today. |
| "Text input cursor doesn't center on the text" | `draw_focus_caret` (`gui.rs:480-495`) centers the caret in the input **box** (fixed `+4px`/`-8px` inset), but the value text is **baseline-positioned** by layout — the two diverge. Omnibox caret is a separate, cruder appended `│` glyph (`gui.rs:530`). |
| "Not blazing fast / init + search latency" | (a) A **fresh Tokio runtime is built per navigation** (`gui.rs:402`), killing the global connection pool → cold DNS+TCP+TLS every load (`net lib.rs:103` vs `gui.rs:402`). (b) **Synchronous system-font enumeration on startup** before the window exists (`gui.rs:175` → `text/lib.rs:107`). (c) Navigation **blocks the UI thread** (`block_on`, `gui.rs:412`). (d) Every keystroke/scroll triggers **full-document relayout + full-canvas repaint + full GPU texture re-upload** (`gui.rs:357,442-471`; `page/lib.rs:364`). (e) **No glyph raster cache** — every glyph re-rasterized every frame (`text/lib.rs:245`). |
| "example.com / google.com don't look on par with Chrome" | No **images** (Google logo absent — no decoder anywhere), no **`font-family`** (everything renders as system sans; `layout/lib.rs:476`), no **border-radius / gradients / box-shadow / opacity**, **integer glyph snapping** (no subpixel positioning; `paint/lib.rs:290`), no **kerning/ligatures** (naïve advance stacking; `text/lib.rs:218`). See side-by-side renders in `docs/parity/repro/`. |

---

## 1. HTML parsing & DOM — **strongest layer**

Parsing is **html5ever 0.39** (Servo's spec tokenizer + tree builder) via a custom
arena `TreeSink` (`html/src/sink.rs:104`). Tokenizer, insertion modes, adoption
agency, foster-parenting, `<template>`, and even declarative Shadow DOM are correct
**for free**.

- **IMPLEMENTED:** full HTML5 tokenizer + tree construction; entities; malformed
  recovery; `<template>`; declarative shadow roots; arena DOM with append/insert/
  remove/detach, traversal, ordered attributes, shadow/slot flat-tree; WHATWG charset
  sniffing on the **buffered** path (`net/lib.rs:168-260`).
- **PARTIAL / MISSING:** SVG/MathML **namespaces flattened** to HTML local names
  (`sink.rs:125-152`) → SVG won't render; `innerHTML` uses **full-document parse, not
  fragment parsing** → `<tr>`/`<option>` contexts break (`html/lib.rs:176-193`);
  streaming parse is **UTF-8-only** (`html/lib.rs:79`); **scripting flag semantics**
  (`<noscript>` handling) not set; no `document.write`; arena slots for detached nodes
  **leak** (`dom/lib.rs:490`).

## 2. CSS — hand-rolled subset; **Stylo is scaffolding only**

Default engine is `MinimalCascade` (`css/lib.rs:944`), a byte-scanning parser + a
hand-rolled matcher. `StyloEngine` (`--features stylo`) **delegates to
MinimalCascade** — it links Stylo but does not run its cascade (`stylo_engine.rs:37`,
`TODO(stylo)`), and `stylo_dom.rs:302` stubs pseudo-classes to `false`.

- **IMPLEMENTED:** tag/class/id/`*` selectors, **descendant combinator only**;
  specificity; UA stylesheet; inheritance; `!important` (author only); `calc()` (real
  recursive evaluator); px/em/rem/pt/…; hex/`rgb()`/`rgba()`/~35 named colors; the box
  model, flex, grid, positioning, transform, table, and vertical-align property sets.
- **MISSING (load-bearing):** **`@media`, `@font-face`, `@keyframes`, `@supports`,
  `@import` — all at-rules dropped** (`css/lib.rs:759`); **`var()`/custom properties**;
  **child `>` / sibling `+ ~` / attribute `[a=b]` / all pseudo-classes/-elements**
  (any such token drops the selector, `css/lib.rs:824,877`); **`font-family`** (!);
  `font` shorthand; **`hsl()`**, `currentColor`; **`opacity`, `overflow`, `box-shadow`,
  `border-radius`, gradients, `background-image`, `visibility`, `filter`,
  `text-decoration`, `letter-spacing`, `text-transform`**; `inherit`/`initial`/`unset`
  keywords; viewport units `vw/vh/vmin/vmax`, `ch/ex`; logical properties.

## 3. Layout — solid primitives, **no incremental layout, no overflow/z-index**

Hand-rolled single-pass fragment tree; flex/grid via **taffy 0.12**.

- **IMPLEMENTED (near-parity shaped):** box model + box-sizing + min/max + auto-margin
  centering; **floats/clear** (genuine BFC-aware, a real strength); absolute/fixed vs
  the containing-block chain; **tables** incl. colspan/rowspan; transforms (as AABB);
  greedy line-breaking; taffy-backed flex/grid basics; vertical-align for atomics.
- **MISSING (load-bearing):** **`overflow` / clipping / element scroll containers**
  (property doesn't exist; `layout/lib.rs:653`); **`z-index` / stacking contexts**
  (parsed, ignored; paint = DOM order; `layout/lib.rs:1531`); **`position:sticky`**
  (parsed, treated static); **static-position abs boxes** (dropped); **bidi/RTL**;
  **writing-modes**; **real text shaping** (kerning/ligatures/complex scripts);
  **`font-family` selection** (always sans; `layout/lib.rs:476`); `white-space:pre`
  preservation; `text-align:justify`; parent/child margin collapsing.
- **PERFORMANCE (the snappiness complaint):** dirty-bits + restyle-damage taxonomy
  **exist but are a coarse gate** — **any `>= Repaint` change re-runs
  `layout_document` over the whole tree** (`page/lib.rs:364`); a fresh fragment tree is
  allocated every pass; shrink-to-fit re-lays subtrees at 1e6 width. No partial layout.

## 4. JavaScript & web-platform APIs — **live surface is tiny**

SpiderMonkey via mozjs (feature-gated; **off in the default build** — scripts parsed
but not run, `js/lib.rs:59`). Correct one-runtime-per-process handling; a real
macrotask+microtask event loop with a native Promise job queue (`job_queue.rs`).

- **Live DOM API (all of it):** `document.getElementById/querySelector[All]/
  createElement/getElementsByTagName/getElementsByClassName`; element `appendChild/
  set|get|remove|hasAttribute/remove/getBoundingClientRect/addEventListener/
  dispatchEvent`; props `textContent/innerHTML/tagName/id/className`; `setTimeout(cb)`,
  `queueMicrotask`, `fetch` (thenable), partial `XMLHttpRequest`.
- **MISSING (breaks real sites immediately):** **`console`** (a `console.log` throws
  and kills the script); **`window`/`self`**; **`document.body/head/documentElement/
  title/cookie/readyState`**; **DOM traversal** (`parentNode`, `children`,
  `firstChild`, `nextSibling`…); **`createTextNode`, `insertBefore`, `cloneNode`**;
  **`classList`, `dataset`, `element.style`, `getComputedStyle`**; control IDL
  (`value`/`checked`/`href`); **real `Event` object + propagation + `preventDefault`**;
  **engine-generated events** (no click/input/submit/DOMContentLoaded dispatch) and
  **inline `on*` handlers**; **`navigator`, `localStorage`, `location`, `history`**
  (the last two are **implemented in `history_bindings.rs` but never `install`ed** into
  `run_scripts`); `MutationObserver`; ES modules (run as classic → syntax error);
  `setInterval`/`requestAnimationFrame`. `getBoundingClientRect` returns a **stale
  pre-script snapshot** (zero for script-created nodes).

## 5. Networking — good transport, **unwired cache/cookies, pool defeated**

hyper + hyper-rustls, process-global pooled client.

- **IMPLEMENTED:** HTTP/1.1 + **HTTP/2** (ALPN); rustls + webpki roots; gzip/br/deflate
  + chunked; full charset sniff; GET redirects (cap 10); GET form submission; streaming
  first-paint (**render CLI only**).
- **MISSING / BROKEN:** **HTTP cache entirely absent** (`storage.rs:22`) — no ETag/
  If-Modified-Since/304, every nav cold; **cookie jar fully implemented but never wired
  into `send_raw`** (`net/lib.rs:509`) → no sessions/logins/consent; **connection pool
  defeated** by the per-nav throwaway runtime (`gui.rs:402`); **GUI path fetches no
  subresources or external CSS** (`gui.rs:412`); **no image loading** (enumerated, never
  fetched); **no web fonts**; POST refused; `request()` doesn't follow redirects; no
  HTTP/3; no zstd; preconnect is **dead code** (no callers).

## 6. Paint / text / compositor — **CPU tiny-skia, two draw primitives**

Layout → flat `DisplayList` of **only `Rect` + `Text`** (`paint/lib.rs:27`) → tiny-skia
CPU pixmap → uploaded whole as a wgpu fullscreen quad each frame. Vello/Parley/swash
are named in comments but **not wired**; text is **fontdb + fontdue**.

- **IMPLEMENTED:** solid backgrounds; solid square single-color borders; Latin text
  blit with grayscale AA; system-font discovery; measure/face caches; frame timer.
- **MISSING (visual fidelity):** **images (all formats)**; gradients; **box-shadow**;
  **opacity**; **border-radius**; dashed/dotted/double borders; per-side border color;
  **overflow clipping**; **stacking contexts/z-index**; **`font-family`**; **real
  shaping** (kerning/ligatures/bidi/complex); **subpixel glyph positioning** (integer
  snap, `paint/lib.rs:290`); LCD subpixel AA; hinting; **color/emoji glyphs**;
  **per-glyph font fallback** (CJK/emoji blank).
- **MISSING (performance):** **glyph raster cache** (re-rasterize every glyph every
  frame, `text/lib.rs:245`); **display-list caching** (rebuilt every frame); **partial
  invalidation** (`Damage` type exists but paint ignores it — full repaint + full
  texture upload on every caret blink / 1px scroll).

## 7. Shell / chrome / UX — **logic present, UI crude, thread blocks**

winit/wgpu single window; chrome is hand-drawn rects + glyphs on the CPU canvas.

- **IMPLEMENTED (logic):** omnibox URL-vs-search disambiguation (well-tested);
  ranked suggestions; history back/forward; session restore (hibernated tabs); find-in-
  page; zoom; checkbox/radio with group exclusivity; GET form submit; hover cursor.
- **MISSING / CRUDE (UX):** **back/forward/reload are bare Unicode glyphs**, not buttons
  (no shape/hover/press; `gui.rs:519`), and **click zones don't match glyph positions**
  (14/40/68 drawn vs 30/56/92 hit); **no tab strip** (full tab backend exists, nothing
  renders it); **no suggestions dropdown** (completions go to `tracing` logs only); no
  Home/menu/bookmarks/favicon/loading indicator; **caret is fake** (end-only, non-
  blinking, mis-centered); **text editing is append/backspace only** (no caret movement,
  selection, copy/paste, `<textarea>` multiline); `<select>`/date/range/color/file
  inputs unsupported; **UI freezes on every navigation and agent run** (synchronous
  `block_on`, new runtime each time).

---

## 8. What already works well (do not rebuild)

html5ever parsing; the arena DOM; floats/clear; absolute/fixed positioning; tables;
the flex/grid taffy integration; box model; the event-loop + Promise job queue; the
HTTP/2 + TLS + decompression transport; charset sniffing; the cookie **library**;
omnibox intent logic; find-in-page; the layout-parity harness itself (70/70 probes).

## 9. The shape of the delta

Three tiers, by how directly they block "usable core browsing":

1. **Correctness/UX bugs blocking the reported flows** (small, high-impact): default
   search endpoint; honor `<meta refresh>`; caret centering; **one long-lived Tokio
   runtime** (pool reuse); async/off-thread navigation; defer font enumeration; wire the
   GUI path to fetch external CSS; glyph raster + display-list caches; use `Damage` for
   partial repaint.
2. **Missing platform features common sites need** (medium): `font-family` + real
   shaping + subpixel positioning; **images**; `overflow`/clipping/element scroll;
   `z-index`/stacking; `@media`; `var()`; child/sibling/attribute/pseudo selectors;
   `hsl()`/`currentColor`; border-radius/opacity/box-shadow/gradients; wire the cookie
   jar; HTTP cache; the core JS BOM (`console`/`window`/`navigator`/traversal/`Event`+
   propagation/inline handlers/`getComputedStyle`); wire `history`/`location`.
3. **Deep architecture for full parity** (large): real Stylo cascade (or grow the
   matcher to spec); incremental/partial layout; a real compositor with layers/tiling;
   GPU rasterization (Vello); bidi/RTL + writing-modes; web fonts; HTTP/3; ES modules;
   `MutationObserver`; context-aware fragment parsing; SVG namespaces.

The research prompt (`RESEARCH-PROMPT.md`) enumerates the full Chromium/Gecko landscape
so tier 2–3 can be planned against a complete map rather than this triage.
