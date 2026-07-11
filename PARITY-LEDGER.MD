# PARITY-LEDGER.md — The Full Chrome/Gecko Parity Checklist

Everything below is either missing entirely or a known-partial in Manuk today,
organized by subsystem. This is the fixed inventory epoch audits check
progress against — items get added if discovered, never silently dropped.
Tag convention: mark each item `[doc]` / `[app]` / `[both]` as it's worked, per
the document-web/app-web band split.

**Band definitions:** `[doc]` (document web) — content meaningfully complete
from server-rendered HTML alone; correct rendering doesn't depend on
post-load script execution. `[app]` (app web) — content or interactivity
that only exists after client-side script runs: SPAs, hydration,
dynamically-inserted scripts/stylesheets, XHR/fetch-injected content, and
anything gated behind the scripting surface (Sections 5-8). `[both]` —
correct on both axes independently (e.g. media embedding renders
server-side but often needs script for controls/interaction). Coverage,
visual, and placement numbers are reported per band, never blended into one
figure, and the corpus in Section 15 is what supplies real examples of each
band to measure against.

---

## 1. CSS — Cascade & Values

**Selectors**
- Full `:nth-child()` / `:nth-of-type()` / `:nth-last-*` with An+B and `of S`
- `:not()`, `:is()`, `:where()` with correct specificity rules
- `:has()` (relational — needs invalidation support)
- State pseudo-classes: `:hover`, `:active`, `:focus`, `:focus-visible`,
  `:focus-within`, `:target`, `:checked`, `:indeterminate`, `:disabled`,
  `:enabled`, `:required`, `:optional`, `:valid`, `:invalid`, `:in-range`,
  `:out-of-range`, `:read-only`, `:read-write`, `:placeholder-shown`,
  `:default`, `:link`, `:visited`, `:empty`, `:root`, `:defined`
- Pseudo-elements: `::before`, `::after` (with `content:` — including
  `counter()`, `attr()`, strings, images), `::first-line`, `::first-letter`,
  `::marker`, `::placeholder`, `::selection`, `::backdrop`,
  `::file-selector-button`
- Shadow-DOM selectors: `:host`, `:host()`, `:host-context()`, `::slotted()`,
  `::part()`
- Attribute selectors with `i`/`s` case flags
- Namespace selectors

**At-rules**
- `@layer` — real cascade-layer ordering (currently flattened and cascaded by
  specificity, which is wrong)
- `@container` (size and style queries) + container query units (`cqw`,
  `cqh`, `cqi`, `cqb`, `cqmin`, `cqmax`)
- `@scope`
- `@font-face` + real webfont loading (no webfont pipeline at all today)
- `@keyframes`
- `@property` (registered custom properties, with types and interpolation)
- `@page`, `@counter-style`, `@font-feature-values`, `@import` (including
  `layer()` and `supports()` conditions), `@namespace`, `@charset`
- `@media` beyond viewport: `prefers-color-scheme`, `prefers-reduced-motion`,
  `prefers-contrast`, `forced-colors`, `hover`, `any-hover`, `pointer`,
  `any-pointer`, `orientation`, `aspect-ratio`, `resolution`,
  `dynamic-range`, `scripting`, range syntax (`width >= 600px`)

**Values & units**
- `calc()` with mixed units and nesting; `min()`, `max()`, `clamp()`
- `round()`, `mod()`, `rem()`, trig (`sin`, `cos`, `tan`, `atan2`), `pow`,
  `sqrt`, `hypot`, `abs`, `sign`
- Viewport units in full: `vw`, `vh`, `vmin`, `vmax`, `svw`/`svh`,
  `lvw`/`lvh`, `dvw`/`dvh`
- Font-relative: `ex`, `ch`, `cap`, `ic`, `lh`, `rlh`, `rex`, `rch`
- `attr()`, `env()` (safe-area insets)
- Custom properties with fallbacks, cycles, and correct
  inheritance/invalidation
- Color: `color()`, `lab()`, `lch()`, `oklab()`, `oklch()`, `hwb()`,
  `color-mix()`, `light-dark()`, relative color syntax, `currentColor`
  everywhere, `system-color` keywords, wide-gamut / `display-p3`
- `system-ui` and generic font families resolved correctly
- CSS-wide keywords everywhere: `initial`, `inherit`, `unset`, `revert`,
  `revert-layer`

## 2. CSS — Layout

**Box model gaps**
- `min-content`, `max-content`, `fit-content`, `stretch` as width/height
  values
- `aspect-ratio`
- Logical properties (`inline-size`, `block-size`, `margin-inline`,
  `padding-block`, `inset-inline`, `border-start-start-radius`, ...) and
  `writing-mode` / `direction` / `text-orientation` — vertical writing modes
  and full RTL, which are entirely absent today
- `overflow: hidden|scroll|auto|clip` with real scroll containers,
  `overflow-anchor`, `scrollbar-gutter`, `overscroll-behavior`
- `resize`
- `contain` / `content-visibility` (and the perf wins they enable)

**Formatting contexts lacking or under-implemented**
- Multi-column (`columns`, `column-count`, `column-width`, `column-gap`,
  `column-rule`, `column-span`, `break-inside`)
- Flexbox: full `flex-basis: content`, `align-content` on single-line,
  `order`, `gap` in all cases, `min-size:auto` rules, nested-flex intrinsic
  sizing (routes to Taffy, but the measure seam is still approximate)
- Grid: `subgrid`, `masonry`, `repeat(auto-fill/auto-fit)`, `minmax()` in all
  forms, `fit-content()`, implicit tracks, `grid-auto-flow: dense`, named
  lines (not just named areas), alignment (`justify-items`, `align-items`,
  `justify-self`, `align-self`, `place-*`)
- Tables: `border-collapse` (only the separated model exists), `<caption>`,
  `<col>`/`<colgroup>` width hints, fixed layout edge cases, rowspan
  interaction with row heights, anonymous table fixup
- Floats: `shape-outside`, `clear` in all cases, float interaction with BFCs
- Positioning: `position: sticky` (detected but scroll-driven recomputation
  not fully implemented), `position: fixed` w.r.t. transforms/filters
  creating containing blocks, anchor positioning (`anchor()`,
  `position-anchor`), inset shorthand
- Stacking contexts done properly — currently approximated by propagating a
  layer to a subtree; real stacking contexts are created by `opacity < 1`,
  `transform`, `filter`, `will-change`, `isolation`, `mix-blend-mode`,
  `contain: paint`, and painting order must follow CSS 2.1 Appendix E
- `display: contents`, `display: flow-root`, `display: list-item`,
  two-value `display` syntax
- Fragmentation: `break-before`/`after`/`inside`, orphans/widows, printing

**Text & fonts**
- Line-height: real `normal` from font metrics (currently hardcoded 1.2× in
  the Stylo map). **This is not a cosmetic gap** — it's a live contributor
  to the placement-fidelity problem: on the real-site corpus, median
  vertical offset from Chrome runs ~5,200px on Wikipedia, driven
  substantially by the article rendering ~20% shorter than Chrome's. Fixing
  this is a direct, measurable lever on the placement-within-8px floor in
  Section 15, not just a text-rendering nicety.
- `text-align: justify` with real inter-word/inter-char distribution,
  `text-align-last`
- `text-indent`, `text-transform`, `letter-spacing`, `word-spacing`,
  `word-break`, `overflow-wrap`, `hyphens`, `hanging-punctuation`,
  `text-wrap: balance|pretty`
- `text-decoration` (underline/overline/line-through,
  `text-decoration-thickness`, `text-underline-offset`, `skip-ink`),
  `text-emphasis`, `text-shadow`
- `white-space` fully (`pre`/`pre-wrap`/`pre-line` just landed;
  `break-spaces` remains)
- Font features: `font-variant-*`, `font-feature-settings`,
  `font-variation-settings` (variable fonts), `font-stretch`,
  `font-synthesis`, `font-kerning`, `font-optical-sizing`
- `font-size-adjust`
- Bidi (UAX #9) — full bidirectional reordering
- Complex-script shaping: Arabic, Indic, Thai, CJK line-breaking rules
  (UAX #14 beyond current implementation), ruby annotations
- Webfont loading: `@font-face`, `font-display`, `unicode-range` subsetting,
  WOFF2 decode, the CSS Font Loading API
- Emoji / color fonts (COLR, CBDT, sbix)
- `list-style-type` (including `@counter-style` and CJK/alpha/roman),
  `list-style-image`, `list-style-position`
- CSS counters: `counter-reset`, `counter-increment`, `counter-set`,
  `counters()`

## 3. CSS — Paint & Compositing

- Backgrounds: `background-image` (not supported at all today — only
  `background-color`), multiple backgrounds, `background-size`
  (cover/contain/lengths), `background-position`, `background-repeat`,
  `background-attachment`, `background-origin`, `background-clip`
  (including `text`)
- Gradients: `linear-gradient`, `radial-gradient`, `conic-gradient`,
  repeating variants, gradient interpolation color spaces
- Borders: per-side widths/colors/styles (currently a uniform border only),
  all `border-style` values (dashed, dotted, double, groove, ridge, inset,
  outset), per-corner `border-radius` with two radii each, `border-image`
- Outline (`outline`, `outline-offset`) — needed for focus rings
- Real `box-shadow`: proper Gaussian blur (currently stacked concentric
  rects), spread, inset, multiple shadows
- Masking & clipping: `mask` shorthand, `mask-size/position/repeat/mode/
  composite`, `clip-path` (inset, circle, ellipse, polygon, path,
  `url(#clip)`), `mask-border`
- Filters: `filter` (blur, brightness, contrast, drop-shadow, grayscale,
  hue-rotate, invert, opacity, saturate, sepia, `url(#filter)`),
  `backdrop-filter`
- Blending: `mix-blend-mode`, `background-blend-mode`, `isolation`
- Transforms: full 2D + 3D (perspective, `transform-style: preserve-3d`,
  `backface-visibility`, `rotate3d`, `translateZ`, `matrix3d`),
  `transform-origin`, `transform-box`, and separate
  translate/rotate/scale properties
- Images: `object-fit`, `object-position`, `image-rendering`,
  `image-orientation`, `srcset`/`sizes` responsive selection, `<picture>`,
  lazy loading, AVIF/WebP/GIF(animated)/APNG decode, ICC color management
- Real opacity groups — offscreen compositing rather than per-item alpha
  scaling
- GPU-accelerated compositing: layerization, damage tracking, tiling,
  partial invalidation

## 4. CSS — Animation

- `transition` (all properties, `transition-behavior: allow-discrete`)
- `animation` / `@keyframes` (all timing functions incl. `steps()`,
  `linear()`, `cubic-bezier()`, `direction`, `fill-mode`,
  `iteration-count`, `play-state`, `delay`, composite ordering)
- Interpolation for every animatable type (lengths, colors in the right
  color space, transforms with matrix decomposition, filters, shadows,
  paths, custom properties via `@property`)
- Web Animations API (`element.animate()`, `getAnimations()`, `Animation`
  objects, playback control)
- Scroll-driven animations (`scroll-timeline`, `view-timeline`,
  `animation-timeline`)
- `view-transition` API
- Compositor-thread animation (so animation doesn't block layout/JS)
- `prefers-reduced-motion` honored
- `will-change` hints

## JS Scripting Surface — Current-State Diagnosis (read before Sections 5-9)

This is the direct answer to "what is the JS scripting/interactivity gap,"
preserved as its own unit rather than dissolved into the flat inventory
below — it's a *diagnosis* (what's broken and why it matters most), not an
*inventory* (everything that's missing). Sections 5-9 list every item;
this is which of them actually matter first and why.

**The four that break almost every script**, grouped together specifically
because they share near-universal blast radius — not four items among
forty, the four that make everything else moot:
1. **IDL attribute reflection** (full item: Section 5, Core DOM)
2. **`element.style` + `classList` + `dataset`** (full item: Section 5)
3. **Event handler attributes and properties** — `onclick="..."` in HTML
   and `el.onclick = fn` in JS; only `addEventListener` works today, and a
   large fraction of the real web, especially older and server-rendered
   pages, wires buttons the other two ways (full item: Section 6)
4. **Live layout reads** — `getBoundingClientRect` answers from a
   pre-script snapshot; real scripts read layout, mutate, and read again;
   this needs reflow-on-read, meaning the layout tree has to be queryable
   and dirty-able from the JS boundary (full item: Section 5)

**Current-state characterization, not just target-state:** `fetch` and
`XHR` exist today only as a host-queued stub — present enough to not
crash, not present enough to do real network work from script (full
target: Section 9).

**Why this diagnosis outranks the flat inventory below:** rendering
failures are visible in a screenshot diff; scripting failures are silent —
the page just doesn't respond, with no visual signal anything is wrong.
That's why G5 interaction parity (Section 15) is load-bearing specifically
for this surface, not a general verification nicety — it's the only check
that catches a page that *looks* right but doesn't *work* at all.


**Parser**
- Full HTML5 tree construction: every insertion mode, the adoption agency
  algorithm, foster parenting, implied end tags, template contents,
  foreign content (SVG/MathML) integration points, `<noscript>` handling,
  fragment parsing
- Streaming/incremental parse with `document.write` support
- Encoding sniffing and the full encoding table
- `<template>`, `<slot>` assignment algorithm
- Declarative Shadow DOM (exists, but flat-tree correctness needs
  hardening)

**Core DOM**
- Full `Node` / `Element` / `Document` / `DocumentFragment` /
  `CharacterData` interfaces
- `NodeList`/`HTMLCollection` live semantics
- `Range`, `Selection`, `TreeWalker`, `NodeIterator`, `DOMParser`,
  `XMLSerializer`
- `Element.closest`, `matches`, `insertAdjacentHTML`/`Element`/`Text`,
  `replaceWith`, `before`/`after`, `append`/`prepend`, `cloneNode(deep)`
  with correct shadow/template semantics
- attributes `NamedNodeMap`, `Attr` nodes
- `dataset`, `classList` (`DOMTokenList`), `style` (`CSSStyleDeclaration`)
  — **all three missing today**
- IDL attribute reflection for every content attribute (currently reflects
  only `id`/`className`/`value`/`checked`). **Confirmed, not assumed:**
  `img.src = 'x.png'` does not reach the DOM — `getAttribute('src')`
  returns null. Re-run this exact check as the acceptance test for this
  item; it's a fast, unambiguous pass/fail.
- Live layout reads: `offsetTop/Left/Width/Height`, `offsetParent`,
  `clientTop/Left/Width/Height`, `scrollTop/Left/Width/Height`,
  `getBoundingClientRect` recomputed on read (currently answers from a
  pre-script snapshot), `getClientRects`, `scrollIntoView`,
  `elementFromPoint`, `elementsFromPoint`
- `innerText` (layout-aware, distinct from `textContent`)
- Custom Elements: full lifecycle (`connectedCallback`,
  `disconnectedCallback`, `attributeChangedCallback`, `adoptedCallback`,
  `observedAttributes`, form-associated custom elements,
  `ElementInternals`)
- Shadow DOM: closed mode, `delegatesFocus`, slot change events, event
  retargeting
- `MutationObserver`: all option combinations, correct microtask timing

**CSSOM**
- `document.styleSheets`, `CSSStyleSheet`, `insertRule`/`deleteRule`,
  `cssRules`
- `getComputedStyle` returning used values for every property (currently
  partial)
- `CSS.supports()`, `CSS.registerProperty()`, `CSS.escape()`
- Constructable stylesheets + `adoptedStyleSheets`
- `matchMedia` with change listeners (basic `matchMedia` exists)

## 6. Events & Interaction

**Scope framing, apply across sections 5-8:** the scripting gap is bigger
than the rendering gap, and it has a nastier shape. Rendering failures are
*visible* — a missing background or wrong offset shows up immediately in a
screenshot diff. Scripting failures are *silent* — the page just doesn't
respond, with no visual signal that anything is wrong at all. This is why
sections 5-8 (DOM/CSSOM, events, forms, script loading) don't get evaluated
by the same visual-diff instinct that works for sections 1-4 — a page that
"looks right" in a screenshot can still be completely non-functional, and
that failure mode won't show up in COVERAGE or VISUAL metrics. G5
interaction parity (Section 15) is the only thing that catches this class
of gap; treat it as load-bearing for sections 5-8 specifically, not just a
general verification nicety.

**Event model**
- Capture phase, `stopPropagation`, `stopImmediatePropagation`,
  `composedPath()`, `once`, `passive`, `signal` option
- Event constructors: `new Event`, `CustomEvent`, `MouseEvent`,
  `KeyboardEvent`, `PointerEvent`, `WheelEvent`, `InputEvent`,
  `FocusEvent`, `TouchEvent`, `DragEvent`, `SubmitEvent`, `ClipboardEvent`
- Event properties: `target`, `currentTarget`, `eventPhase`, `bubbles`,
  `cancelable`, `composed`, `isTrusted`, `timeStamp`, `defaultPrevented`
- Inline handlers (`onclick="..."` attributes) and handler properties
  (`el.onclick = fn`) — currently only `addEventListener` is supported
- `window.onerror`, `unhandledrejection`, `error` events

**Event types (all currently missing except click)**
- Mouse: `mousedown`, `mouseup`, `mousemove`, `mouseenter`, `mouseleave`,
  `mouseover`, `mouseout`, `dblclick`, `contextmenu`, `auxclick`
- Pointer: the whole Pointer Events model, `setPointerCapture`
- Wheel/scroll: `wheel`, `scroll`, `scrollend`
- Keyboard: `keydown`, `keyup`, `keypress` with `key`/`code`/`keyCode`/
  modifiers/repeat
- Focus: `focus`, `blur`, `focusin`, `focusout`, `document.activeElement`,
  `el.focus()`/`blur()`, focus ring, tab order + `tabindex`
- Form: `input`, `change`, `submit`, `reset`, `invalid`, `beforeinput`
- Document lifecycle: `DOMContentLoaded`, `load`, `beforeunload`,
  `unload`, `pagehide`, `pageshow`, `visibilitychange`,
  `readystatechange` + `document.readyState`
- Window: `resize`, `hashchange`, `popstate` (have), `message` (have),
  `online`/`offline`
- Drag & drop: full `DataTransfer` model
- Touch events + `touch-action`
- Composition/IME events
- Clipboard: `copy`, `cut`, `paste` + `navigator.clipboard`

**Default actions** — every one: link navigation, form submit,
checkbox/radio toggle, text insertion, Enter-to-submit, Space-to-scroll,
arrow-key scroll, `<details>` toggle, `<select>` open.

**Shell-side interaction (the "buttons actually work" layer)**
- Hit-testing that respects transforms, clip, border-radius,
  `pointer-events`, and stacking order
- Hover state driving `:hover` + repaint on mouse move
- Real scrolling: wheel, scrollbars, keyboard scroll, smooth scroll,
  scroll containers, `scroll-behavior`, `scroll-snap-*`
- Text editing: caret, selection, drag-select, double/triple-click,
  shift-select, IME, undo/redo, clipboard, `contenteditable`
- Native form-control UI: `<select>` popup, `<option>`/`<optgroup>`, file
  picker, date/time/color pickers, `<datalist>`, `<details>`/`<summary>`,
  `<dialog>` (+ `showModal`, `::backdrop`, top layer), `<progress>`,
  `<meter>`, range slider
- Context menu, drag-and-drop, autoscroll, middle-click paste (X11)
- Cursor (`cursor` property) and `user-select`

## 7. Forms

- Submission: GET/POST, `enctype` (urlencoded, multipart — have,
  `text/plain`), implicit submit, `formaction`/`formmethod`/
  `formnovalidate`, `<button type=submit|reset|button>`
- `FormData` API, `URLSearchParams`
- Constraint validation: `required`, `pattern`, `min`/`max`/`step`,
  `minlength`/`maxlength`, `type=email|url|number`, `checkValidity()`,
  `reportValidity()`, `setCustomValidity()`, `ValidityState`,
  `:valid`/`:invalid`
- All input types: text, password, email, url, tel, number, range, date,
  time, datetime-local, month, week, color, file, checkbox, radio, hidden,
  search, submit, reset, button, image
- `<textarea>`, `<select multiple>`, `<fieldset disabled>`, `<label>`
  association (`for` + wrapping)
- Autofill / password manager integration (the `store` crate exists but is
  unwired)
- Form state restoration on back/forward

## 8. Script Loading & the Event Loop

- Dynamically inserted `<script>` must fetch and execute — **the single
  biggest blocker to the app web** (every webpack chunk, every
  ResourceLoader module). **Confirmed on a corpus site already in hand:**
  this is exactly why Wikipedia's icon stylesheet never arrives and its
  fundraising banner never appears — both are injected by MediaWiki's
  ResourceLoader after initial load. Re-check both on Wikipedia
  specifically as the acceptance test for this item, not a synthetic page.
- Dynamically inserted `<link rel=stylesheet>` must fetch and apply
- `defer`, `async`, `type=module` ordering; `nomodule`
- ES modules over the network (currently self-contained only), `import()`
  dynamic, import maps, top-level await
- `document.write` during parse
- `requestAnimationFrame` — nothing JS-driven can animate without it; plus
  a real frame loop
- `queueMicrotask`, `requestIdleCallback`, `setTimeout`/`setInterval` with
  correct clamping and ordering
- Correct microtask/macrotask/rendering-steps interleaving (HTML spec's
  "update the rendering" steps)
- Long-task yielding so a busy script doesn't hang the UI (§1.7: no
  "app unresponsive")
- Incremental re-cascade / re-layout / repaint on mutation — currently the
  entire document re-cascades; without invalidation, interactivity is
  O(page) per event
- Web Workers, SharedWorker, Service Workers (offline, push), Worklets

## 9. Networking & Storage

- `fetch` completeness: headers, bodies, credentials, redirect modes,
  `AbortController`/`AbortSignal`, streaming (`ReadableStream`),
  `Response`/`Request`/`Headers` objects, cache modes
- `XMLHttpRequest` completeness (events, `responseType`, upload progress,
  sync mode)
- CORS enforcement (preflight, credentials, exposed headers) —
  correctness, not just security; sites branch on it
- CSP, mixed-content blocking, `Referrer-Policy`, `SameSite`,
  COOP/COEP/CORP, `Permissions-Policy`, HSTS
- HTTP/2 (have) and HTTP/3/QUIC; Brotli/zstd; caching per RFC 9111 with
  revalidation; range requests
- WebSocket, `EventSource`, `navigator.sendBeacon`
- Cookie partitioning (still open) + third-party cookie policy
- IndexedDB (nothing today)
- Cache API, `localStorage`/`sessionStorage` (just landed),
  `StorageManager` quotas
- `URL`/`URLSearchParams` objects
- `Blob`, `File`, `FileList`, `FileReader`, Blob URLs, `structuredClone`,
  `TextEncoder`/`TextDecoder`
- `crypto.getRandomValues`, `randomUUID`, `crypto.subtle` (WebCrypto)
- `BroadcastChannel`, `MessageChannel`/`MessagePort`

## 10. Observers & Platform Objects Sites Feature-Detect

The `localStorage` lesson generalizes: **absence gets you graded down, not
just degraded.**

- `IntersectionObserver` — without it, every lazy-loaded image on the web
  never loads
- `ResizeObserver`, `PerformanceObserver`, `ReportingObserver`
- `performance.now/mark/measure/getEntries`, Navigation Timing, Resource
  Timing
- `navigator`: `userAgent`, `platform`, `languages`, `onLine`,
  `hardwareConcurrency`, `deviceMemory`, `clipboard`, `permissions`,
  `mediaDevices`, `geolocation`, `serviceWorker`, `sendBeacon`, `share`
- `screen`, `devicePixelRatio`, `visualViewport`, `window.matchMedia`
- `document.visibilityState` / `hidden`
- `history.scrollRestoration`, navigation API
- Fullscreen API, Page Visibility, Wake Lock, Notifications, Web Share

## 11. Embedded Content

- `<iframe>` — nested browsing contexts, sandbox, cross-origin isolation,
  `postMessage` across frames, sizing/scrolling. Nothing today.
- `<canvas>` 2D — full `CanvasRenderingContext2D` (paths, transforms,
  gradients, patterns, text, images, compositing, `ImageData`). Nothing
  today.
- WebGL / WebGL2 / WebGPU
- OffscreenCanvas
- SVG — the full SVG rendering model (currently only rasterize SVG
  *images* via `resvg`; inline SVG with CSS, `<use>`, filters, animation,
  SMIL is a different thing)
- MathML
- `<video>` / `<audio>` — demux, decode (H.264, VP9, AV1, Opus, AAC), MSE,
  EME/DRM, subtitles, controls UI, autoplay policy, PiP
- `<object>`, `<embed>`, `<portal>`

## 12. Accessibility

- Full accessibility tree (the `a11y` crate exists; it needs to be
  complete and wired)
- ARIA roles, states, properties; `aria-*` reflection
- AT-SPI / IAccessible2 platform bridges — a screen reader must actually
  work
- Keyboard navigation, focus management, focus ring
- High-contrast / `forced-colors` mode
- `prefers-reduced-motion`, `prefers-contrast`
- Zoom (text-only and full-page) and reflow at 400%
- Caret browsing

## 13. Browser Product Surface (the k-point star, not the engine)

- Tabs: pinning, groups, drag-reorder, detach, mute, recently-closed,
  session restore
- Navigation: back/forward with bfcache, reload variants (hard reload),
  stop, home
- Omnibox: search suggestions, history/bookmark ranking, in-line
  autocomplete, search-engine management, drop-down UI
- Bookmarks: manager UI, folders, import/export, bookmark bar
- History: full manager, search, clear-browsing-data with granularity
- Downloads: pause/resume/cancel, shelf/panel, open-on-complete,
  dangerous-file warnings
- Find in page: highlight all, match count, next/prev, case sensitivity
  (basic version exists)
- Zoom: per-site persistence
- Print: preview + print-to-PDF + `@page` + `@media print`
- DevTools: inspector, DOM tree, computed styles, console, network panel,
  sources/debugger, performance profiler — with a CDP-compatible protocol
  (this also gives the agent surface a standard shape)
- Extensions: WebExtensions API (a genuine parity item; refusing it is a
  strategy choice, not parity)
- Profiles: multi-profile, sync, guest, durable + migratable user data
  (ADR-009)
- Privacy: private/incognito mode, tracker/ad blocking, fingerprint
  resistance (honest, not impersonating)
- Settings UI: everything above must be configurable
- PDF viewer
- Password manager + autofill, wired end-to-end
- Translate
- Reader mode
- Crash recovery / tab crash isolation
- Multi-process architecture: site isolation, sandboxed renderers, IPC —
  this is a security parity item and a stability one; a single-process
  browser is not at parity no matter how correct its layout is
- Updater: seamless binary updates decoupled from profile (ADR-009)

## 14. Performance & Stability (the EPOCH axes)

- Incremental style invalidation (selector-keyed, not whole-document
  re-cascade)
- Incremental layout (dirty-bit propagation, layout roots)
- Damage-driven repaint, layer tiling, GPU raster
- **DEBT-2**: the Stylo path still has no rule index (EPOCH-1 only
  indexed MinimalCascade)
- **DEBT-3**: shell chrome is unprobeable headlessly — AESTHETICS/
  ERGONOMICS can't be measured
- Off-main-thread: parsing, decoding, compositing, animation
- Memory: arena compaction, image cache eviction, tab
  hibernation/discarding, per-tab budgets
- No hangs: long-task yielding, watchdogs, never blocking the UI thread
  on I/O
- Speedometer / JetStream / MotionMark / Web Vitals benchmarking

## 15. Verification — Without This, None of the Above Is Provable

- **G5 interaction parity** (ADR-012, specified but not built): script
  clicks/scroll/typing/form-fill/hover/focus through the automation
  surface, mirror in Chromium via CDP, diff the resulting DOM and pixels.
  **Standing rule: an interaction that works in Chromium and not in Manuk
  is a CRITICAL.**
- **The corpus is 3 sites.** ADR-012's class-grouped corpus (real-time
  feeds, session-heavy platforms, media-rich client apps, authenticated
  dashboards) must actually exist, with the tick tier (3 sites) vs epoch
  tier (full corpus) split.

  **Corpus composition, reconciling ADR-012's classes with concrete site
  types — both are required, ADR-012's classes are the higher-level
  grouping, this list is what actually populates them:**
  - Document web `[doc]`: news/blog/wiki (have — Wikipedia, HN), forums and
    user-generated-content sites, WordPress-templated sites (a large
    fraction of the web by frequency — do not skip for a framework that
    feels more technically interesting)
  - App web `[app]`: at minimum one SPA per major framework — React, Vue,
    Angular, Svelte — plus one SSR+hydration site (Next.js or SvelteKit),
    since hydration is its own failure mode distinct from a pure
    client-rendered SPA
  - E-commerce `[app]`: product page *and* cart/checkout interaction, not
    a static product listing alone — this is where forms, validation, and
    dynamic script-inserted content compound
  - Dashboards/admin panels/complex data UIs `[app]` — maps to ADR-012's
    "authenticated dashboards" class
  - Media-heavy pages `[both]`: canvas, video, image galleries — maps to
    ADR-012's "media-rich client apps" class, and depends on Section 11
    (Embedded Content)
  - Real-time feeds `[app]` — ADR-012's own class, the hardest scripting
    surface on this whole list (constant DOM mutation, virtualized
    rendering)

  Every corpus entry reports its own COVERAGE/VISUAL/PLACEMENT numbers,
  never folded into one blended figure — document web and app web are
  reported as separate bands throughout, per the `[doc]`/`[app]`/`[both]`
  tagging convention used across this whole ledger.
- **Ratchet COVERAGE and PLACEMENT as binding floors** — placement is
  currently 0% within 8px on both real sites, which is the real frontier.
- Run real WPT at scale and track the pass rate as a public number.
- Reftest infrastructure.
- Fuzzing (parser, CSS, layout) + ASAN/UBSAN + a crash-free-session
  metric.

---

## The Honest Ordering (unlock sequence, not a priority ranking of importance)

The list above is long, but the *unlock* order is short and highly
non-uniform — most of it is gated behind a handful of foundational items:

1. **`el.style` + `classList` + `dataset` + full IDL reflection** — days of
   work, enormous blast radius. Almost every script touches these.
2. **Inline (`onclick=`) and property (`el.onclick=`) handlers** — a large
   fraction of the real web wires buttons this way and neither is
   supported today.
3. **Live layout reads** (`getBoundingClientRect` recomputed, `offset*`,
   `client*`, `scroll*`) — requires reflow-on-read.
4. **The full event model + shell-side hover/focus/scroll/text-editing**
   — this is what "the buttons work" actually means.
5. **Dynamically inserted `<script>` execution** — this one changes the
   *class* of site that can run at all. Everything behind it (SPAs,
   hydration, code-splitting, even Wikipedia's own icon CSS) is currently
   unreachable.
6. **`requestAnimationFrame` + incremental invalidation** — without these,
   even correct interactivity is unusably slow and nothing animates.
7. **`IntersectionObserver`** — lazy loading is the default on the modern
   web.
8. **`background-image` + gradients** — currently unsupported, and they
   are most of the visual web's surface.
9. **`::before`/`::after` with `content`** — pervasive in every design
   system.
10. **G5 + a real corpus** — because until this exists, every claim about
    parity, including every one made so far, is an assertion rather than
    a measurement.

Items 1-3 and 9 are the kind of gap that, like `<br>` doing nothing, is
invisible in aggregate scores and catastrophic in practice. Item 5 is the
door to the modern web. Item 10 is the only thing that turns any of this
from a story into a fact.