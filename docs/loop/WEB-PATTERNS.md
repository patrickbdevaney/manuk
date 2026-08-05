# WEB PATTERNS — what the web is actually made of, and how much of it we render

**This is the coverage ledger.** Not a bug list: a list of the *recurring HTML/CSS/JS patterns* that
real sites are built out of, what each one unlocks, and whether we support it. It is updated **every
tick** — a tick that closes a pattern class edits this file, and one that discovers a new pattern adds a
row.

**Why this file and not the bug tracker.** Chromium's team doesn't write bespoke code per website; they
cover *patterns*, and the vast majority of the web is a recombination of a comparatively small number of
them. So the honest question is never "how many bugs are left" (unbounded, unknowable) but **"which
pattern classes do we cover, weighted by how much of the web actually uses them."** That number is
estimable. This file is where it gets estimated.

**How to read the estimates.** The "% of web" column is a deliberate, stated-as-such *judgement* — it is
not measured, and it is not pretending to be. What IS measured is the status column, and the oracle's
265-site crawl is what corrects the judgement when it is wrong. When the two disagree, the crawl wins and
the estimate gets edited. A number in this file that has never been contradicted by the crawl is a number
that has never been tested.

---

## Bar 0 — the stability floor. Nothing else counts until these hold.

| Pattern | What breaks without it | Status |
|---|---|---|
| A page that panics the renderer | **Browser dies**, every tab lost | ✅ contained (`G_CONTAIN`) — panic kills the page, not the process |
| A dead/blackholed subresource | **Tab frozen** until the kernel gives up | ✅ `G_LOAD` — per-request + per-page deadline |
| `setInterval(fn, 0)` / self-reposting `setTimeout` | **Tab frozen forever** — the event loop never quiesces | ✅ `G_RUNAWAY` — task ceiling, page still renders |
| A node the cascade never saw (script-injected `<svg>`) | **SIGSEGV** — a panic through SpiderMonkey's C++ frames aborts | ✅ layout degrades to initial style and *logs* |
| **Quitting the browser** after any page ran JavaScript | **The process SIGSEGVs on the way out** — and a crash in the exit handlers aborts the handlers that follow it, which is exactly where the cookie jar and `localStorage` are flushed to the profile. The user closes the window and **silently loses their session** (ADR-009). | ✅ `G_CLEAN_EXIT` — the engine tears SpiderMonkey down **itself**, on the thread that owns it, whether or not the caller remembers to ask |
| A fault *inside* SpiderMonkey's C++ frames | Browser dies | ⚠️ **not containable in-process.** Needs a per-tab process. Deferred, and stated rather than hidden. |
| Sites that **hang** (CPU + duplicate work) | Unusable | ⚠️ **4 of 265 (1.5%)**, measured. This row said *"~1 site in 4 — nothing else matters at this ratio"* and **steered the roadmap on a number 16× wrong**. Real, and no longer the top item. |

---

## The document web — text, layout, links. The majority of the internet by page count.

| Pattern | Unlocks | Status |
|---|---|---|
| Block/inline flow, the box model | Everything | ✅ |
| **Flex** — cards, navbars, sidebars, split layouts | Nearly every page built since ~2016 | ✅ (incl. the shrink + percentage-squaring fixes) |
| **Grid** | Modern editorial + dashboard layouts | ✅ |
| Float / clear | The pre-2015 web, still enormous | ✅ |
| Tables (incl. as layout) | Docs, wikis, gov/enterprise, email-derived pages | ✅ |
| `position: absolute/relative/fixed/sticky` | Dropdowns, modals, headers, tooltips | ✅ (sticky geometry not reflected in `getBoundingClientRect` — Bar 2) |
| **`position:absolute` with no insets** (static position) | **React portal roots, JS-positioned dropdowns, every `.sr-only` node** | ✅ — these were being *deleted from the page* |
| `@media` (incl. `em`/`rem` breakpoints) | Every responsive site | ✅ |
| `@supports` | Progressive enhancement — without it we rendered the *fallback* of every such site | ✅ |
| `@layer`, `var()`, `calc()` | Modern design systems | ✅ (incl. **mixed `calc(100% − 250px)` in flex/grid layout** — tick 139; the taffy path used to collapse a mixed calc to one term, so `calc`-sized flex/grid sidebars fell to 0) |
| **`font-family`** | **Literally every page** — was never mapped from the cascade at all | ✅ |
| `line-height: normal` from real font metrics | Every line box on every page | ✅ |
| **Intrinsic aspect ratio** (`img { max-width:100% }`) | **Every responsive image on the web** | ✅ |
| Background tiling / `background-size` / `-repeat` | Every sprite, texture, pattern, icon | ✅ |
| `font-size: 0` (inline-block gap killer, image replacement) | Painted **glyph-shaped continents** across the page | ✅ |
| `<source>`, `<track>`, `<picture>` | Responsive images — got phantom boxes | ✅ |
| SVG (inline, `<img src=*.svg>`) | Icons everywhere | ✅ renders; namespaces not modelled |
| `<canvas>` 2D | Charts, games, visualisations | ✅ **it rasterizes** (`G_CANVAS`). Fills, strokes, paths (incl. `arc`), the transform stack, `clearRect`, real `getImageData`, real `toDataURL`, `fillText`/`strokeText` (`G_CANVAS_TEXT`), `drawImage` (`G_CANVAS_IMAGE`) — on tiny-skia, the same rasterizer that paints the page. **And the pixels reach the screen**: a canvas is composited as an image the page drew into, through the very map an `<img>` lands in. Not done: `clip`, real gradients (each an honest no-op, not a lie). |
| `Path2D` (reusable + SVG path-data paths) | Icon systems (Lucide/Feather/Material), Chart.js/D3 shape generators, glyph-on-canvas helpers | ✅ **it's a real path** (`G_PATH2D`, tick 320). `new Path2D()` builds imperatively, `new Path2D(other)` copies, and `new Path2D("M… A… Z")` parses SVG path-data (M/L/H/V/C/S/Q/T/A/Z, abs+rel, S/T reflection, endpoint→center arc flattening). `ctx.fill(path)`/`ctx.stroke(path)` rasterize its command stream through the existing single native call; `addPath` supports a `DOMMatrix` transform. Was absent → `new Path2D(...)` threw `Path2D is not defined` and the whole draw routine died. |
| Canvas patterns (`createPattern`) | Hatch fills, textured/repeating backgrounds, tiled sprites | ✅ **it tiles** (`G_CANVAS_PATTERN`, tick 323). `createPattern(img/canvas, 'repeat'\|'repeat-x'\|'repeat-y'\|'no-repeat')` returns a real `CanvasPattern` that tiles the source through a tiny-skia `Pattern` shader via `__cvPathPattern` — `fill()`, `fillRect()`, `stroke()`, `strokeRect()` route to it, source pixels reuse the node-keyed `drawImage` registry (no new decode), `globalAlpha` folds in, built at identity so the context transform tiles it in user space. Was `null` forever → the fill became `null`→black. Honest limits: tiny-skia's `SpreadMode` is not per-axis, so `repeat-x`/`repeat-y` both tile and `no-repeat` pads its edges (not transparent); `pattern.setTransform` not wired. |
| Canvas conic gradients (`createConicGradient`) | Conic pie/donut fills, colour wheels, loading spinners, angular progress rings | ✅ **it sweeps by angle** (`G_CANVAS_CONIC`, tick 324). `createConicGradient(startAngle, cx, cy)` rasterizes a real tiny-skia `SweepGradient` (kind 2 in the existing gradient plumbing) — colour varies with the ANGLE around the centre, start angle radians→degrees (same +x-origin clockwise convention). Completes the gradient story: linear / radial / conic all real. Was a flat last-stop block. |
| Canvas gradients (`createLinearGradient`/`createRadialGradient`) | Chart.js area/bar fills, button glosses, progress bars, sparklines | ✅ **real gradient shaders** (`G_CANVAS_GRADIENT`, tick 322). `fillStyle`/`strokeStyle = grad` now rasterizes a genuine tiny-skia `LinearGradient`/`RadialGradient` (two-point conical) through `__cvPathGradient` — `fill()`, `fillRect()`, `stroke()`, `strokeRect()` all route to it, stops carry offsets, `globalAlpha` folds in, the shader is built at identity so the context transform locks it to the geometry. Was a flat approximation that painted the whole shape in the last stop's colour. Honest limit: conic gradients keep the flat last-stop fallback (no `SweepGradient` wired); `createPattern` still `null`. |
| `createImageBitmap` (drawable `ImageBitmap`) | Texture uploaders (Pixi/Three), image editors, tile renderers | ✅ **it round-trips** (`G_CREATE_IMAGE_BITMAP`, tick 321). Returns a `Promise<ImageBitmap>` for `<img>`/`<canvas>`/`ImageBitmap` sources — reusing the node-keyed image-source registry, so **zero new decode path** — including the crop overload (`sx,sy,sw,sh`, which composes on an already-cropped bitmap) and `close()`. `ctx.drawImage(bmp, …)` honours the bitmap's crop rect. Was absent → the call threw `createImageBitmap is not a function`. Honest limit: `Blob`/`ImageData` sources reject LOUDLY (no decode-to-pixels path yet) rather than blit blank. |
| `<video>` / `<audio>` playback | Media sites | ❌ **no codecs.** Element boxes lay out; nothing plays. Graceful, not crashing. |
| Web fonts (`@font-face`) | Typography-heavy sites | ✅ |
| `display: contents` | Layout-transparent wrappers — every component framework emits them | ✅ **the wrapper dissolves** (`G_DISPLAY_CONTENTS`). Its children become the *grandparent's* grid/flex items. Unparsed it fell through to `inline`, which is the worst answer available: the wrapper stayed a real box, the grid saw **one** anonymous item instead of three, and the layout collapsed into a single cell — every element present, every element styled, all in the wrong place. |
| CSS transforms / transitions / animations | Motion, and *layout* when transforms shift boxes | ✅ **applied AND readable** (`G_TRANSFORM`). The box moves, `getBoundingClientRect()` agrees, and `getComputedStyle().transform` returns the spec's resolved `matrix(a,b,c,d,e,f)` — which is what every animation library reads before composing its own. `undefined + ' scale(2)'` is the string `"undefined scale(2)"`: not an error, just an element that quietly stops moving. Transitions still snap to the end state (no tween). |

---

## The app web — SPA frameworks. Fewer pages, but the ones people spend hours in.

**The finding that decided the schedule: this is ADDITIVE SUBSTRATE, not a missing subsystem.** Eight
real framework bundles went from **0/8 rendering to 4/8** on ~10 additive IDL fixes and no new
architecture. Each one below was *named by a framework*, not guessed at.

| Pattern | Unlocks | Status |
|---|---|---|
| **`import.meta`** (module metadata hook) | **Every Vite/Rollup/esbuild bundle on the internet** — they emit `import.meta.url` unconditionally | ✅ |
| ES modules (`<script type=module>`) | All of the above | ✅ |
| **`nodeType`** | React's `isValidContainer` — without it, **React error #299** and the whole ecosystem | ✅ |
| **`ownerDocument`** | React indexes it immediately after; `undefined["_reactListening…"]` | ✅ |
| **DOM interface constructors** (`x instanceof HTMLIFrameElement`) | `instanceof undefined` **throws** | ✅ (via `Symbol.hasInstance`) |
| **`<template>.content` as a real fragment** | Svelte/Solid/Lit clone a parsed template per instance | ✅ |
| **Real comment nodes** (`nodeType 8`) | **lit-html finds template holes by walking to COMMENT markers.** Vue/Svelte anchor every `v-if` and `{#each}` on them | ✅ |
| **DocumentFragment: inserting moves its CHILDREN** | Every framework commits a built subtree in one insertion | ✅ |
| `createTreeWalker` + `NodeFilter` | How lit-html locates dynamic bindings | ✅ |
| `createElementNS` / `createComment` / `createDocumentFragment` | Vue/Svelte/SVG | ✅ |
| `MessageChannel`, `performance.now`, `queueMicrotask` | Every framework scheduler | ✅ |
| Custom elements + shadow DOM | **Every design system** — Material, Fluent, Shoelace, Spectrum, every `<x-y>` on a bank or gov site | ✅ (shadow trees are laid out; prototype-chain upgrade fixed) |
| `adoptedStyleSheets` / constructable stylesheets | How web components ship styles | ✅ **fed to the cascade** (tick 25) — the sheet text is materialized into a real `<style>` in the adopting root, so one cascade serves both paths. |
| **Unhandled promise rejections surfaced** | Every framework renders inside an `async` fn — a throw there is a *rejected promise*, and ours went into a void | ✅ |
| `Error.captureStackTrace` (V8-only, TC39 proposal) | Libraries with custom error classes | ✅ |
| Patching a DOM prototype (`Element.prototype.setAttribute = wrapper`) | **The patch silently does nothing.** The element's own property shadows the prototype, so the wrapper is never called — and nothing throws. This is how error trackers (Sentry), ad-blockers, polyfills, framework internals and React DevTools all hook the DOM: **the library believes it is installed and it is not.** | ✅ `G_PROTOTYPE` — the members live on a real `EventTarget`→`Node`→`Element`→`HTMLElement` chain; a patch lands *between* the element and the method, and is actually called |
| `Element.prototype.setAttribute`, `Node.prototype.appendChild`, `EventTarget` | `undefined`, `undefined`, and a bare `ReferenceError`. Feature detection (`'matches' in Element.prototype`) and borrowed methods (`.call()`) both fail | ✅ `G_PROTOTYPE` |
| React committing its render | React | ✅ **it renders.** A probe ran the real Vite/React bundle: `#root` gets its 6 children, the app's own text (*"Count is 0"*), 59 elements, **zero errors**. The ❌ here was **wrong for an unknown number of ticks** — nobody had run React and looked. *An absent measurement is not a negative measurement* (PROCESS #35, fifth recurrence). |
| Lit committing its template | Lit | ✅ (tick 26) — it needed `CharacterData.data` on its comment markers |
| Svelte's runtime | Svelte | ❌ opaque error in minified code |
| Hydration (SSR → interactive) | Next.js, Nuxt, SvelteKit | ❓ **unmeasured** |

---

## Interactivity — what makes a page usable rather than a picture.

| Pattern | Unlocks | Status |
|---|---|---|
| Click → navigate, focus, type, submit | The basic web | ✅ |
| **`element.click()`** (programmatic) | **Menus, dropdowns, hidden file inputs, every Copy button, every custom control forwarding to a real one** | ✅ |
| **`setInterval` / `clearInterval` / `clearTimeout`** | **Carousels, clocks, pollers, countdowns, progress bars, live scores.** *A page could not even STOP a timer it started.* | ✅ |
| **`document.readyState`** | **Half the scripts on the web open by comparing it against a string** | ✅ |
| `document.defaultView` | Frameworks get `window` from a *node*, so they work in iframes | ✅ |
| `visibilityState` / `hidden` | Video players and animation loops refuse to start if the tab looks backgrounded | ✅ |
| `isConnected` | React/Vue check it before every commit; `undefined` is falsy → they silently skip work | ✅ |
| `AbortController` | **Every modern `fetch` passes a signal** — a library constructing one unconditionally throws before the request | ✅ |
| `btoa`/`atob`, `TextEncoder` | Data URLs, JWTs, request ids | ✅ |
| `crypto.getRandomValues` / `crypto.randomUUID` — **cryptographically secure** | Session tokens, CSRF nonces, OAuth `state`, password-reset ids, React keys — **anything that must be unguessable** | ✅ **real OS CSPRNG (tick 160, `G_CRYPTO`).** The ✅ here was a *lie* until tick 160: both were filled from `Math.random()`, a non-cryptographic PRNG, so every token a page minted was predictable — and `getRandomValues` gave a `Uint32Array` only `0..255`, and `randomUUID` omitted the RFC 4122 variant nibble. Now: entropy from `getrandom` (`/dev/urandom`), byte-view fill (full element width), version+variant stamped. |
| **`crypto.subtle.digest`** (SHA-1/256/384/512) | **Subresource Integrity, content-addressed caches, auth/signing libraries** — an absent `crypto.subtle` makes `crypto.subtle.digest(...)` a TypeError that takes the caller with it | ✅ **(tick 162, `G_SUBTLE_DIGEST`)** — RustCrypto hashes in the host, wrapped in a resolved Promise; unknown algo rejects (`NotSupportedError`). Only `digest`; `sign`/`encrypt`/`deriveKey` stay honestly **undefined** so feature-checks take their fallback. |
| Event bubbling / capture / `stopPropagation` | All delegation-based UIs | ✅ |
| **`text-align: start`/`end` resolve against `direction`** (RTL alignment) | the entire Arabic / Hebrew / Persian web — a `dir="rtl"` paragraph with no explicit alignment must RIGHT-align; also `text-align:end` menus/toolbars | ✅ **(tick 414, `text_align_start_and_end_resolve_against_direction`)** — `start` is the INITIAL value and logical; the map hard-wired `end`→right and `start`→left, so RTL body text left-aligned (backwards). Now the map keeps `start`/`end` logical and the cascade resolves them to physical per node right after `direction` is recovered, so layout/getComputedStyle still see only physical values. LTR unchanged. Residue: `justify` last-line direction, and full bidi character reordering, are separate. |
| **`text-indent`** — first-line indent + image replacement | first-line paragraph indentation (article/book typography); and the ubiquitous **image-replacement idiom** (`text-indent:-9999px` / `text-indent:100%` on logos and icon buttons — hide the text, show the background image) | ✅ **(tick 416, `text_indent_offsets_the_first_line_only` + `text_indent_maps_through_the_stylo_cascade`)** — was **unimplemented** (the string appeared only in a code comment): no field, no map, no application, so both idioms silently no-op'd — and "unhandled" image-replacement meant duplicate text rendered at x≈0 ON TOP of the logo. New inherited `text_indent: Dim` (zoom-scaled, %-of-container resolved at layout), mapped through both cascades (Stylo `clone_text_indent().length` + MinimalCascade parse); `layout_inline` offsets ONLY the first line box (a `first_line` flag flips false after the first `close_line`), and a negative indent widens the line so it never wraps and sits off-screen — the image-replacement recipe. **Safety: default indent 0 is the IEEE arithmetic identity, so every existing line box is byte-identical.** Residue: `hanging`/`each_line` keywords ignored; anonymous mixed block+inline runs and form-control text pass indent 0. |
| **`-webkit-line-clamp: N`** — multi-line card/excerpt truncation | the truncation idiom on nearly every content site (`display:-webkit-box;-webkit-box-orient:vertical;-webkit-line-clamp:N;overflow:hidden`) — card/product/article-excerpt text capped at N lines with a trailing `…` | ✅ **(tick 417, `line_clamp_caps_lines_and_appends_ellipsis` + `line_clamp_recovers_through_the_stylo_cascade`)** — was **unimplemented**: `-webkit-line-clamp` is `engine="gecko"` in stylo 0.19 so the servo build never parsed it, and every clamped block showed ALL its wrapped lines (a 2-line teaser becoming a wall of text). Lucky break: `display:-webkit-box` is likewise gecko-only so the container just stays a **block** (UA default) and flows text normally — no `-webkit-box` formatting context needed for the real single-text-run case. New non-inherited `line_clamp: Option<u16>`, parsed in MinimalCascade and recovered into the shipping cascade via the same merge as `object-fit`/`text-overflow`; `apply_line_clamp` (block-inline path) keeps the first N line boxes, drops the rest, **unconditionally** ellipsizes line N (content DID continue, unlike single-line `text-overflow`), and shrinks the box height so siblings reflow up. **Safety: `line_clamp` unset → the branch never runs → byte-identical.** Residue: the `line-clamp` shorthand's `<block-ellipsis>`/`continue` ignored (bare integer only); clamped blocks with block (not inline) children unhandled; true old-flexbox `-webkit-box` child layout out of scope. |
| **`white-space: pre-wrap`** space preservation | `<textarea>` content (pre-wrap by UA default), aligned/indented text that must still wrap, chat and log/code panes that wrap | ✅ **(tick 413, `pre_wrap_preserves_spaces_while_pre_line_collapses`)** — pre-wrap and pre-line shared one path that COLLAPSED runs of spaces (right for pre-line, wrong for pre-wrap), so indentation and column alignment silently vanished into a single-spaced blob. Now pre-wrap emits each whitespace run as its own measured token (N spaces stay N), while pre-line keeps collapsing; pre/normal/nowrap untouched. Residue: trailing-space hanging at a wrap boundary not specially modelled. |
| **`text-transform: capitalize`** on words with leading punctuation/quotes/digits | headings/titles that open with a quote or bracket (`"Twas the night"`, `(Draft)`), tag/version labels (`3d printing`) | ✅ **(tick 412, `capitalize_skips_leading_punctuation_and_digits`)** — the pass cleared its word-start flag on every non-whitespace char, so a leading `"`/`(`/digit swallowed the capital (`(hello)` stayed `(hello)`). Now leading symbols pass through and the first typographic LETTER is titlecased, matching Chrome (`(Hello)`, `'Twas`, `3D`). The uppercase/lowercase/plain-capitalize paths were already correct; this is the word-boundary edge. |
| **`<ol reversed>` and `<li value>` numbering** (HTML "ordinal value") | ranked / countdown lists (`reversed`), resumed or manually-renumbered lists (`value` continues the count), any legal/spec doc that renumbers | ✅ **(tick 411, `list_ordinals_follow_reversed_and_value_continuation`)** — markers were built from `start + preceding-<li>-count`, which ignored `reversed` (a countdown numbered `1,2,3` upward) and treated `value` as a one-item override (`<li>x<li value=7>y<li>z` gave `1,7,3` not `1,7,8`). Now a single running counter: starts at `start` (or the item count when `reversed` has no `start`), each `value` resets it for every following item, steps by ±1. The bullet/decimal/alpha/roman rendering was already complete; this is the counting. |
| **`content: attr(name)` in `::before`/`::after`** | CSS-only tooltips (`[data-tip]::after{content:attr(data-tip)}`), print stylesheets expanding links (`a::after{content:" ("attr(href)")"}`), breadcrumb separators, data-table cell labels | ✅ **(tick 409, `content_attr_resolves_the_elements_attribute`)** — the generated-content extraction kept only `String` items and **dropped `Attr`**, so every such pseudo drew an **empty box**: in the tree, invisible on the page (the worst class — content nobody can see). Now resolves `attr(name)` against the live element via the same accessor the attribute-selector matcher uses; a missing attribute → the empty string (CSS2.1), never a dropped pseudo. CSS2.1 string form; Level-5 typed/fallback `attr(x number, 0)` stays an honest gap (not in this Stylo's `Attr` shape). |
| **HTML constraint validation** — `input.validity`, `checkValidity()`, `willValidate`, `setCustomValidity`, the `invalid` event | **Every signup/login/checkout form.** The browser's native validation AND every library (React Hook Form, Formik, VeeValidate) reads `validity.valueMissing` / calls `form.checkValidity()`; on the absent API `if(!input.checkValidity())` is a TypeError that kills the submit handler and the form silently won't submit | ✅ **(tick 161, `G_CONSTRAINT_VALIDATION`)** — JS API on the shared HTMLElement prototype, computing the `ValidityState` flags from reflected attributes + value; `invalid` event fired; form aggregates its controls. **Not** the `:valid`/`:invalid` CSS pseudo-classes (a Stylo cascade tick) — script validation works; CSS-driven red-border styling is the honest gap. |
| `fetch` / XHR | Every dynamic page | ✅ |
| `fetch`/XHR **request headers** (`Authorization`, `Content-Type`, `X-*`) | **Every authenticated API read / token exchange / form-POST** — without them the request is anonymous and 401s, looking like a network fault | ✅ (tick 148) — headers travel to the wire; `Content-Type` defaulted only when unset. Response headers still a stub. |
| `MutationObserver` / `IntersectionObserver` / `ResizeObserver` | Lazy-loading, infinite scroll, sticky headers | ✅ |
| `localStorage` / `sessionStorage` / cookies | Sessions, preferences | ✅ (partitioned; RFC 6265) |
| `history.pushState` (client-side routing) | Every SPA's navigation | ✅ |
| `append`/`prepend`/`before`/`after`/`replaceWith` | Modern DOM mutation — very common | ✅ all five, plus `insertAdjacentHTML`/`remove` — **measured**, `G_CAPABILITY`. The ❌ was never measured. |
| `insertAdjacentHTML` / `insertAdjacentElement` | Extremely common — every hand-rolled "load more", all of htmx | ✅ (tick 25) |
| `append` `prepend` `before` `after` `replaceWith` `replaceChildren` | The ChildNode/ParentNode mixins — what any script reaches for to place a node *next to* another | ✅ (tick 25) — all eleven were missing |
| `outerHTML` (get + set) · `innerText` · `getAttributeNames` | Ubiquitous | ✅ (tick 25) — `innerText` is honestly approximated as `textContent`; the true definition needs layout |
| `outerHTML`, `innerText` | Common | ✅ both — **measured**, `G_CAPABILITY` |
| `scrollTop`/`scrollLeft` + `scrollHeight`/`clientHeight` | Scroll containers, virtualised lists, chat panes, infinite feeds | ✅ **real** (`G_SCROLL`) — truthful geometry, clamped writes, survives re-layout, **moves the actual pixels**, and fires `scroll`. Was worse than missing: `scrollHeight` was aliased to the element's own box, so **`scrollHeight - clientHeight` was always ZERO** — the one number every virtualised list divides by. |
| `scroll-snap-type` + `scroll-snap-align` — **the horizontal carousel** | Paged image galleries, story trays, mobile card rows, product carousels | ✅ **both axes** (`G_SCROLL_SNAP` vertical, `G_SCROLL_SNAP_HORIZONTAL` x). The engine snaps the layout tree at its scroll chokepoint AND the **JS mirror snaps at assignment time** — `el.scrollLeft = 130; el.scrollLeft` reads the snapped `100` on the same line, as in Chrome. Both carousel shapes (`white-space:nowrap` + inline-blocks, `display:flex` + `overflow-x:auto`) report truthful horizontal geometry. Was stale-pessimistic since t266 ("no horizontal scroll range, max_x=0"): layout work closed the geometry gap as a side effect and nothing re-pinned it. One candidate collector (`snap_candidates_for`) feeds both consumers — recomputing snap points in the bindings would be the two-sources-of-truth trap. |
| **HTML attribute reflection** — `a.href`, `input.disabled`, `img.width`, `td.colSpan`, `form.action`, `option.selected` … | **How ordinary page code touches the DOM.** `if (input.disabled)` reading `undefined` does not throw — it silently takes the wrong branch | ✅ **generic** (`G_REFLECT`). **They were ALL `undefined`.** ~38,000 WPT subtests — 80% of `html/dom`'s failures — behind one mechanism. `html/dom` **21.0% → 37.7% (+9,940 subtests)**. Boolean is *presence* (`el.disabled = false` **removes** the attribute); URLs resolve against the base. **Numeric coercion made spec-correct (tick 117, `G_REFLECT_NUMERIC`, +437):** `-0`→`+0` (JS `parseInt("-0")` is `-0` and `Object.is` fails on it), overflow *falls back* to the default rather than ToInt32-wrapping, `maxLength`/`minLength` default `-1`, and `clamped unsigned long` (`colSpan`) *clamps to max* instead of falling back. **Not done:** `tokenlist` (`relList`, `sandbox`) — skipped rather than stubbed, because a string where a `DOMTokenList` belongs is worse than `undefined`. |
| `setAttributeNS` / `getAttributeNS` / `hasAttributeNS` / `removeAttributeNS` | SVG's `xlink:href`, MathML, and every XML-ish document — it is how they set an attribute **at all** | ✅ (`G_CAPABILITY`). `setAttributeNS is not a function` was **160 failing subtests**, found by *reading the failure messages the harness had been printing all along*. **+170 subtests in one fix.** Honest limit: the namespace is validated then ignored for storage (attributes are keyed by qualified name), which no real page can tell apart. |
| `DocumentType` / `createDocumentType` / `document.doctype` | quirks-mode branching, XML/XHTML tooling, DOM serializers that must re-emit the doctype | ✅ (`G_CAPABILITY`). `createDocumentType()` returned a **plain object literal** — prototype `Object`, so `instanceof DocumentType` was false — and validated nothing. `document.doctype` was `null` on every page, including one that plainly declares `<!doctype html>`. **Validation re-pinned to the CURRENT spec (tick 239):** the rule is "valid doctype name" — reject ONLY ASCII whitespace, U+0000 and `>` — not the pre-2020 QName production. WPT expects a doctype back for `''`, `1foo`, `@foo`, `:foo`, `foo:` and `a.b:c`, and `InvalidCharacterError` for exactly two names (`edi:>`, `edi:a `). This row previously claimed the old rule, so `G_CAPABILITY` sat RED for ~100 ticks unseen — see the gate-coverage finding in JOURNAL.md tick 239. |
| `MutationObserver` | Vue, Alpine, lit reacting to DOM they did not change; every analytics/consent script noticing injected content | ✅ **real** (`G_MUTATION`). It was an **inert stub** — `observe()` returned, `takeRecords()` returned `[]`, the callback never fired, and `typeof MutationObserver === 'function'` was true the whole time. **A stub is worse than an absence**: the library feature-detects, finds it, registers, and silently never reacts. Records batch on a **microtask** (100 appends → *one* callback with 100 records, not 100 callbacks). |
| `element.attributes` / `Attr` / `NamedNodeMap`, `getAttributeNode`, `createAttribute`, `toggleAttribute` | **DOMPurify walks `attributes` to strip `on*` handlers**; every DOM serializer, differ, and "copy these attributes across" helper | ✅ (`G_ATTRS`). `element.attributes` was **`undefined`** — `.length` was a `TypeError`. **A sanitizer that cannot enumerate attributes cannot sanitize them.** The map is **live** (a frozen `length` makes `while (el.attributes.length) el.removeAttribute(…)` spin forever — the same dead-collection hang as tick 73), and an `Attr` is a **handle**: `attr.value = 'x'` writes through. |
| `classList` as a real `DOMTokenList`; `createElement`/`createElementNS` **name validation**; real **namespaces** | SVG and MathML (case-sensitive names), custom elements, and every `classList.add()` typo | ✅ (`G_NAMES`). All three **accepted things that are not names** and produced elements/classes that could never match anything. `classList.add('btn primary')` silently wrote ONE class matching neither selector. `createElement('<div>')` produced a phantom. `createElementNS` threw the namespace away, so SVG's `linearGradient` came back uppercased and unmatched. `dom/nodes` **1522/5401 → 1645/5401**. |
| `addEventListener(…, {once, capture, passive, signal})`, `e.returnValue`, `e.cancelBubble`, `document.createEvent`/`initEvent` | jQuery's event normalisation, Google Analytics, every component that tears down handlers with an `AbortSignal`, and every `{once: true}` in modern code | ✅ (`G_EVENT_SURFACE`). **All of these failed SILENTLY.** `{once:true}` fired **forever** (the options object was read as a bare boolean); `returnValue`/`cancelBubble` were `undefined`, so `if (e.returnValue === false)` was dead code and `e.cancelBubble = true` stopped nothing; `createEvent` did not exist. And a **passive** listener's `preventDefault()` was honoured — which is the exact scroll jank the flag exists to prevent, and is why `touchstart`/`wheel` are passive by default on the root targets. `dom/events` **102/401 → 145/412**, plus **+44** from passive alone. **Dispatch validity added (tick 118, `G_EVENT_DISPATCH_STATE`, +15):** `dispatchEvent` throws `InvalidStateError` for an uninitialized `createEvent()` event (initialized flag) or a re-entrant dispatch of an in-flight event (dispatch flag). The real bug was that the native `el.dispatchEvent` **swallowed the thrown exception into `false`** — it now propagates the pending exception. |
| `element.children` / `getElementsByTagName()` — **live** collections | `while (el.children.length) el.removeChild(el.firstChild)` — the universal "empty this element" idiom | ✅ **live** (`G_COLLECTIONS`). They were **snapshots**, which is not a conformance gap but a **Bar 0 hang**: with a frozen `length` that loop never terminates and the tab locks up. A dead collection does not fail loudly — it *spins*. `dom/collections` **3/48 → live**. |
| `NodeIterator` / `TreeWalker` | **DOMPurify** (the sanitizer half the web runs untrusted HTML through), Lit's template holes, every editor and DOM-diffing library | ✅ **both, with the real filter protocol** (`G_TRAVERSAL`). `FILTER_REJECT` prunes the **subtree**, `FILTER_SKIP` skips only the node — swap them and a sanitizer that rejects `<script>` walks *into* it and keeps the contents. `NodeIterator` treats `REJECT` as `SKIP` (it has no subtree), and aliasing the two is the bug nobody notices until something leaks. `dom/traversal` **11/53 → 34/53**. |
| `getSelection` / `Range` | Rich-text editors, selection, copy/paste, `contenteditable` | ✅ **a real `Range`** (`G_RANGE`): boundary-point comparison, `extractContents`/`cloneContents`/`deleteContents` **across structure** (partially-contained ends are split, not moved whole), `insertNode`, `surroundContents`, `toString`. `dom/ranges` **2/200 → 16/200**. `Selection` is still a stub. |
| `Blob` / `File` / `FileReader` | Uploads, downloads, image preview | ✅ all three — **measured**, `G_CAPABILITY`. (`URL.createObjectURL` is still missing.) |
| **Downloading a large file** (`Content-Disposition: attachment` / binary payload — model weights, installers, datasets, archives) | Saving anything bigger than RAM, or slower than 30s — the agentic-browser staple of "fetch this checkpoint / installer" | ✅ (tick 149) — was **un-saveable at scale**: the body was buffered whole in a `Vec<u8>` under the 30s document deadline, so a multi-GB file OOM'd or was killed mid-transfer and reported as a network fault. Now `fetch_document_or_download` decides from the **headers**, streams a download decoded chunk-by-chunk into a `.part` file (renamed on completion) with **no body deadline** — never held whole in RAM. Documents keep the whole-fetch deadline; cookie carry / `Set-Cookie` store / HTTP-cache / wire-dedup preserved. Gated by `attachment_streams_to_disk_without_buffering`. |
| **`WebSocket` / `Worker`** | Live feeds, chat, heavy compute | ❌ **deliberately absent** — a page that feature-detects and falls back is better served by honest absence than a stub that lies |

---

## Site classes — what we can actually open today

Ranked by how much of the real web each represents. Status is from the 265-site oracle crawl.

| Class | Examples | Renders | Notes |
|---|---|---|---|
| **Reference / wiki** | Wikipedia, MDN, docs sites | ✅ good | ~99% structural coverage |
| **Docs / technical** | rustdoc, Python docs, mdbook | ✅ good | |
| **News / article** | Guardian, NPR, CNN-lite | ✅ good | heavy ad/tracker JS is the load cost |
| **Forums / social (server-rendered)** | old.reddit, HN, lobste.rs, Discourse | ✅ good | |
| **Code hosting** | GitHub, GitLab | ✅ mostly | 97.8% coverage; React portals were the gap |
| **Marketing / landing** | rust-lang.org, most SaaS front pages | ✅ good | |
| **Academic / paper** | arXiv, PubMed | ✅ expected good (static HTML) | |
| **Design-system-based** (web components) | Banks, gov, enterprise portals | ✅ **renders styled** (tick 25) — a `<style>` inside a shadow root was invisible to the stylesheet walk, which used the light tree. |
| **SPA app shells** | Linear, Notion, Figma, HuggingFace | ⚠️ **partial** — Vue/Solid/Preact class works; React class does not yet |
| **Feed / infinite scroll** | X, Mastodon, Bluesky | ❓ needs `scrollTop`, virtualised lists, WebSocket |
| **Media** | YouTube, Twitch, Spotify | ❌ layout only, no playback |
| **Canvas/WebGL** | Games, maps, editors | ⚠️ **canvas 2D rasterizes** (`G_CANVAS`); WebGL returns `null` from `getContext`, which is the spec's "this machine cannot" and every library already branches on it |

---

## The roadmap, in order of web-coverage bought per unit of work

**Rebuilt from measurement at tick 65, because the previous version was fiction.** Its #1 was *"the hangs
(~1 site in 4) — nothing else matters at this ratio"*: the measured figure is **4 sites in 265**. Its #2
was *"React's commit"*: React renders, and probably had for many ticks. Its #3 was `append` /
`insertAdjacentHTML` / `outerHTML`: **all three already worked.** Three of the top three were phantoms,
and the loop was being steered by them.

Every row below has a receipt in `G_CAPABILITY`, which now runs the ledger's claims as assertions.

1. ~~**`<canvas>` 2D**~~ — **done, tick 66.** It rasterizes on tiny-skia and the pixels reach the screen
   (`G_CANVAS`). `fillText`/`drawImage`/`clip`/gradients remain honest no-ops.
2. ~~**`scrollTop`/`scrollLeft`**~~ — **done, tick 67** (`G_SCROLL`).
3. ~~**`getComputedStyle().transform`**~~ — **done, tick 68** (`G_TRANSFORM`).
4. ~~**`display: contents`**~~ — **done, tick 69** (`G_DISPLAY_CONTENTS`).
5. **`document.createRange` / `createEvent` / `URL.createObjectURL`** — small, named, and each one a
   `TypeError` in code that expects them.
6. **The hangs** (4/265). Real, Bar 0, and worth doing — but it is not the emergency the old ledger said.
7. **Media.** Codecs are a large, separate problem. The first step is *graceful*, and already taken.

---

## How this file stays honest

- A tick that closes a pattern class **edits this file** (`scripts/self-audit.sh` checks it was touched).
- The "% of web" judgements are corrected by the **oracle crawl**, not defended.
- A row that says ✅ but whose class still fails in the crawl is a **lie**, and the crawl is what finds it.

## Tick 25 — the shape that keeps recurring, now named

Five times now the bug has had one shape, and it is not "a feature is missing":

| The mechanism | Existed and was correct | Reached by the renderer |
|---|---|---|
| `flat_children` | ✅ | ❌ → nothing that drew pixels called it |
| `NodeData::Comment` / `NodeData::Fragment` | ✅ | ❌ → `cloneNode` fell through to `<div>` |
| The flat tree in the cascade | ✅ | ❌ → shadow trees went unstyled |
| `serialize_node` (i.e. `outerHTML`) | ✅ since the parser was written | ❌ → unreachable from JS |
| `<style>` inside a shadow root | ✅ parsed, stored | ❌ → `collect_style_sources` walked the **light** tree |

**The feature being present in the codebase is not the same as the feature being reachable from the
pixels, and no gate was asking whether a line existed between the two.** That is a gate-shaped hole,
not five bug-shaped ones.

And the sixth, which is worse, because it made a *diagnosis* wrong rather than a feature absent:

**React was never broken. Its bundle was never fetched.** `format!("file://{relative/path}")` parses
`tests` as a *hostname*; every subresource of every local fixture failed to resolve. React mounted
nothing because not one line of React ever ran. For several ticks "React renders nothing" sat in this
ledger as a framework problem. It was a string-formatting bug in the test harness, and the harness's
failure was indistinguishable from the framework's.

*Test your own primitives before blaming the framework.* Third time this prior has paid.

## Tick 26 — the app web is open: 8 of 8 frameworks mount

React · React (JS) · Vue · Svelte · Solid · Preact · Lit · Vanilla. Every one of them was blocked by a
**primitive**, not by anything framework-shaped, and not one of the five would have been found by
reading the DOM standard:

| Framework | What it actually needed | What its failure looked like |
|---|---|---|
| **React** | `ownerDocument` surviving a **GC** | `o.createElement is not a function` — true, and pointing at nothing wrong with React |
| **Svelte 5** | `get_descriptor(Node.prototype,'firstChild').get` | `can't access property "get", a(...) is undefined` |
| **Lit** | `CharacterData.data` on its comment markers | `i.hasAttributes is not a function`, then `i.data is undefined` |
| **Lit** | a shadow root being `nodeType` **11**, not 8 | (silent) |
| **all of them** | `file://` being a scheme the net layer supports | (silent — the bundle never loaded) |

**Every one of these is now asserted in G2 scenario 14**, each labelled with the framework that found
it. The `ownerDocument` case **allocates 60,000 objects to force a collection**, because a test that
does not allocate cannot see that bug at all — which is precisely why it survived several ticks.

**The rule this produces, and it is the tick's real output:** *when a framework fails silently, the
bug is below the framework.* Four of the five above were in our own primitives — one of them a
use-after-GC, one an unsupported URL scheme, one a missing character-data accessor. The framework was
never once the thing that was broken. Stop reading the framework's source and go test the primitive it
sits on.

## Tick 28 — media: degrade honestly

| Pattern | Where it appears | Status |
|---|---|---|
| `<video>` / `<audio>` **layout** | Everywhere | ✅ the element reserves its box and the page flows around it (was already true) |
| `<video poster>` | Every video on the web | ✅ (tick 28) — a poster is a still image, and we decode still images. The user sees the frame the author chose. |
| `HTMLMediaElement` **API** | Every player library, every feature-detect | ✅ (tick 28) — **an honest NO.** `canPlayType()` → `''` · `play()` → **rejected** `NotSupportedError` · `error.code` → 4 · `readyState` 0 · `networkState` 3 |
| Actual video **decode/playback** | — | ❌ not this tick, and openly so |

**The point, which took a measurement to see:** the layout was already right and the API was entirely
absent, and *that is the worst combination*. A site calling `video.play()` got a `TypeError` and lost
the whole page. A site that politely feature-detected read `undefined` and could not even be told no.

**Graceful degradation is not doing nothing — it is answering the question honestly.** The spec already
has the vocabulary for a browser that cannot play a thing, and `play()` returning a rejected promise is
the *best-tested failure path on the web*, because autoplay policies make rejection routine in real
browsers. Every player library is already written to handle it.

Asserted in **G2 scenario 15**. A missing codec is an acceptable limit; a thrown exception is not.

## Tick 30 — first paint does not wait for images

| Pattern | Status |
|---|---|
| Document painted before subresources land | ✅ (tick 30) — `prefetch_document` no longer fetches images; the shell streams them in after (`NavEvent::ImagesReady`) and repaints once |
| `<img>` reflow on late arrival | ✅ — which is what an `<img>` without intrinsic dimensions does in a real browser anyway |

**Measured, time to a paintable document (the shell's real path):**

```
nytimes.com      14,000ms → 5,773ms     then 42 images in 452ms, after the page is up
theguardian.com            → 6,488ms    then 135 images in 8,006ms — the user is reading, not waiting
wikipedia.org              → 2,044ms
```

The load path used to fetch and decode **every image** before the shell was handed anything, so the
window stayed blank until the last tracking pixel had arrived or timed out. On nytimes the document was
parsed, cascaded and laid out — everything needed to paint — **in 1.7s**, and the user saw it at **14s**.

Gated by **G_FIRST_PAINT**, which drives the shell's actual path and additionally asserts the images are
still *pending* — because "fast" achieved by never loading them is a different bug wearing this one's
success as a disguise.

## Tick 32 — `defer` / `async` / `type=module` mean what they say

| Pattern | % of the web that uses it | Status |
|---|---|---|
| `<script defer>` | very common | ✅ (tick 32) — runs after paint |
| `<script async>` | ubiquitous (every analytics/ad tag) | ✅ (tick 32) |
| `<script type="module">` | **every Vite/Rollup/esbuild bundle** — deferred by DEFAULT | ✅ (tick 32) |
| Classic blocking `<script>` | ubiquitous | ✅ still blocks, as the spec requires |
| Incremental paint *during* parse (paint what is above a blocking script) | how Chromium hides blocking-script cost | ❌ **not done** — we parse the whole document, run every blocking script, then paint |

`defer` and `is_async` had been parsed into a struct and used for **nothing**. nytimes: 5,773ms → 5,083ms
to first paint, with 10 deferred scripts (997ms) moved off the paint path.

**The honest read of that number:** most of nytimes' JavaScript is *classic blocking* script, which a
real browser must also run before painting — it just paints **incrementally as it parses**, so the parts
above a blocking script are already on screen. That is the next thing, and it is a bigger change than
this one.

## Tick 33 — the capability ledger, and canvas stops throwing

**`docs/loop/CAPABILITIES.md` is now the answer to "what unlocks the most of the web"**, and it is
measured (237 real site snapshots × a feature probe run through the real pipeline), not imagined. That
file supersedes guesswork for prioritisation; this one continues to record what each tick actually
unlocked.

| Pattern | % of the web | Status |
|---|---|---|
| `<canvas>` + `getContext('2d')` | 3% use canvas | ✅ (tick 33) — **it THREW.** A real context; drawing ops are no-ops; `measureText` returns a real shape. A blank chart on a **working page**. `getContext('webgl')` → `null`, the spec's "cannot". |
| `Notification` | 14% | ✅ (tick 33) — honest: `permission === 'denied'`. The site asked and was told no. |
| `localStorage` / `sessionStorage` | 27% / 12% | ✅ (already worked — persisted, per-origin. My probe said otherwise because it ran from `file://`, an opaque origin, which gets no storage in *any* browser.) |

**The rule this tick added, and it is about scoring not coding:** *3% of sites USING a feature is 3% of
sites BROKEN when that feature throws.* The usage number and the damage number are not the same number,
and a capability that throws outranks capabilities used by ten times as many sites.

## Tick 34 — the browser becomes writable

| Pattern | % of the web | Status |
|---|---|---|
| `submit` event + `preventDefault()` | **~every modern form** | ✅ (tick 34) — **this was the bug.** No `submit` event was ever dispatched, so a React/Vue form's handler never ran, and we performed the **full GET navigation the author had explicitly cancelled**. The user watched the site "reload itself" and lose what they typed. |
| `form.submit()` / `requestSubmit()` | common | ✅ (tick 34) — and they differ, as the spec requires: `requestSubmit()` fires `submit` (the page may cancel); `submit()` does not (the script has decided) |
| `form.reset()` | common | ✅ (tick 34) |
| `FormData` from a `<form>` | every AJAX form | ✅ **already existed** — fixed: a checked checkbox with no `value` submits `"on"`, not `""` |
| `URLSearchParams` / form-urlencoded | ubiquitous | ✅ **already existed** — fixed: a space is `+`, not `%20`, which is what a server's form parser expects |
| `<form method=POST>` | logins, checkouts | ❌ **still not implemented** — and now it says so out loud instead of being silently ignored |

**Forms are 50% of the corpus, and they are the difference between a reader and a browser.** You cannot
search, log in, or buy anything without them.

## Tick 91 — innerText is the RENDERED text, not textContent

`textContent` wearing `innerText`'s name works until a page hides a node — then a script reads the hidden
text and does the wrong thing. Every framework reads `innerText`; it must mean what the spec says.

| Pattern | Unlocks | Status |
|---|---|---|
| `element.innerText` (rendered text) | reading visible text — `display:none` excluded, `<br>`→newline, block boundaries→newline, whitespace collapsed (respecting `white-space: pre*`) | ✅ (tick 91) — a structural approximation over the pre-script computed styles the binding already holds. NOT layout-exact (line-break counts, `::first-letter`, multicol still fail); the innerText suite went 2 → 35 / 455 |
| `element.outerText` (get + set) | the sibling property, asserted alongside innerText everywhere; the setter replaces the element with text (`\n`→`<br>`) | ✅ (tick 91) — was `undefined`, which failed every innerText subtest regardless of innerText's correctness |
| layout-exact innerText | pixel-faithful required-line-break-count rendering | ❌ needs the layout tree; the structural form is what scripts actually read innerText for |

## Tick 84 — the nested browsing context becomes readable (+~721k WPT)

Tick 35 gave the iframe a box and a bitmap. This makes the document *inside* it a real, scriptable
document — the difference between a picture of an embed and an embed.

| Pattern | Unlocks | Status |
|---|---|---|
| `iframe.contentDocument` / `contentWindow` | **the platform web** — embeds, OAuth frames, payment fields, ads, comment widgets, video players all read into their own frame | ✅ (tick 84) — reflectors resolve against their **own arena** (`SLOT_DOM` + a live-arena registry); a per-arena identity cache, so `===` cannot lie across documents; child `Page`s kept alive, arenas unregistered in `Drop`. `G_IFRAME` gates it, incl. cross-document node identity |
| legacy CJK encodings (Shift_JIS / Big5 / GBK / EUC-KR) | **the pre-2010 CJK web** — MOJIBAKE without it | ✅ (tick 84) — `encoding` **128 → ~721k subtests**. The decoder (`encoding_rs`) was correct all along; the tests read their expectations *from an iframe*, which is why it scored zero |
| inline event handlers (`onclick`, `onload`, `onsubmit`) | **every server-rendered form, every legacy page** — the oldest way to attach behaviour to markup | ✅ (tick 84) — compiled + wired at parse time; `<body>`/`<frameset>` `on*` map to the **Window**. `G_CAPABILITY` |
| `element.dataset` (`data-*`) | `data-testid`, Stimulus, Bootstrap, Hotwire — the standard HTML↔JS channel | ✅ (tick 84) — live `Proxy` over `data-*`, works across the iframe boundary. `G_CAPABILITY` |
| a `display:none` iframe still loads | analytics beacons, OAuth relays, `postMessage` shims | ✅ (tick 84) — loading is a DOM decision; the box is only a painting decision |
| iframe **live re-render on mutation** | an embed the parent mutates and expects to see repaint | ❌ the pixels are still a snapshot; the DOM is live and readable, the bitmap is not. Next. |

**The one durable lesson:** a node id is unique only *within* an arena. Resolve a reflector against
the one global `CURRENT_DOM` and a child document's node #7 returns the **parent's** node #7 — a
different element, in a different document, with total confidence. That is the whole reason
`contentDocument` could not exist, and it is a trap any second-document feature will hit.

## Tick 35 — `<iframe>`, and the white void

| Pattern | % of the web | Status |
|---|---|---|
| `<iframe>` **box** | 23% | ✅ (tick 35) — **`iframe` was in NO replaced-element list**, so it laid out at **zero width**. The box was gone before we ever got as far as failing to fetch its document. Unsized is now **300×150**, the spec's default. |
| `<iframe>` **content** | 23% | ✅ (tick 35) — the child document is fetched **after first paint**, rendered as a whole `Page` (its own DOM, cascade, layout **and JS context**) and blitted through the replaced-element path |
| iframe **isolation** | — | ✅ **by construction** — a `PageContext` is per-`Page`, so a child's script has no path to the parent's DOM. It cannot reach it because it does not have it. Gated, so a refactor cannot turn a guarantee back into a coincidence. |
| `<body>` background → **canvas** | **every dark site on the web** | ✅ (tick 35) — see below |
| iframe **scrolling / live updates** | — | ❌ the embed renders as a bitmap; it does not scroll and does not update. A live nested browsing context is where this goes next. |

### The bug that was not an iframe bug

The child document painted **white**. Chasing it found this:

> **`<body>`'s background never propagated to the canvas.** CSS says the root element's background paints
> the whole canvas, and if the root has none, `<body>`'s is propagated up to it. We hard-coded `WHITE`.

So **every dark-themed page whose content is shorter than the viewport** was painting its content on a
correct dark box **floating in a white void**. It was found through an iframe only because a child
document is, by definition, "a page shorter than its viewport" — and it was never an iframe bug at all.

*The symptom names the wrong organ*, for the fourth time in this project.

## Tick 36 — a fifth of the web had invisible content

| Pattern | % of the web | Status |
|---|---|---|
| CSS animation **reveal** (`opacity:0` → keyframes) | **21% of sites** | ✅ (tick 36) — an animated element renders its **END state**. Rendering the first frame literally meant a fifth of the web had content **nobody could see**. |
| `opacity: 0` with **no** animation | — | ✅ **stays hidden** — a closed dropdown, an off-screen menu, an un-fired cookie banner. Revealing those would be a louder bug than the one being fixed. |
| `position: sticky` | 14% | ✅ **already worked** — the ledger claimed otherwise and had never tested it |
| CSS transition tweening | 13% | ⚠️ end state renders, no tween. Low damage: the end state *is* the content. |

**The rule, and it is the spec's own** (`prefers-reduced-motion: reduce` says the same thing):
**show the destination, skip the journey.**

## Tick 39 — the cascade was silently dropping 41% of the web's CSS

| Pattern | % of the web | Status |
|---|---|---|
| **CSS nesting** (`.card { & .x { … } }` and the implicit `.card { .x { … } }`) | **≥41%** of sites | ✅ (tick 39) — **every nested rule was being THROWN AWAY** |
| `:is()` / `:where()` / `:not()` | common | ✅ already worked |
| attribute selectors (`=`, `^=`, `*=`, `$=`, presence) | common | ✅ already worked |
| `+` / `~` combinators | common | ✅ already worked |
| **`:has()`** | 13% of sites | ❌ **rules are DROPPED** — Stylo's *servo* build hardcodes `parse_has() -> false`. Enabling it means editing Stylo, which a **settled decision forbids**. See STATUS.md. |

### What happened

`RuleIndex` — added in tick 14 as a **cascade optimisation** (339ms → 199ms) — walked each stylesheet's
rules, read every `StyleRule`'s `selectors` and `block`, and **never looked at its `rules` field.** That
field holds the rule's **nested** rules. Stylo parses them correctly and always has. We threw every one
of them away before it could match anything.

Measured: **41% of the corpus uses CSS nesting** in its inline `<style>` blocks *alone* — external
stylesheets are not even scanned, so that is a **floor**. It is the single largest cause of the two real
rendering divergences the oracle found:

- *"we lose flex/grid on this node"* (**11,324**) — a nested `display: flex` never applied.
- *"we show what Chrome hides"* (**2,433**) — a nested `display: none` never applied either, so we
  render menus, modals and off-screen panels that Chrome correctly hides.

> **An optimisation that makes a data structure smaller must be asked what it DROPPED.** This one was
> measured for speed and never once asked whether the rules it indexed were all the rules there were.
> No gate could see it: every gate compared *boxes*, and the boxes were internally consistent — they
> were just consistently wrong.

## Tick 41 — a missing constructor is a thrown exception

| Pattern | Status |
|---|---|
| `WebSocket` | ✅ (tick 41) — constructs, then **honestly reports it cannot connect** (`error` + `close`, on a macrotask so a reconnecting client cannot spin the queue). Never throws at construction. |
| `Blob` / `File` / `FileReader` / `FileList` | ✅ (tick 41) — real enough to be used, honest about what they hold |
| `Image` / `Audio` / `Option` | ✅ (tick 41) — element factories. `new Image().src = …` is the commonest preload on the web. |
| `DOMParser` / `XMLSerializer` | ✅ (tick 41) — every sanitiser and markdown renderer parses an HTML string |
| `PerformanceObserver` | ✅ (tick 41) — every RUM bundle constructs one on its first line |
| `EventSource` / `BroadcastChannel` / `Worker` | ✅ (tick 41) — construct, then report they cannot do the thing |
| `DOMRect` / `getSelection` | ✅ (tick 41) |
| **`window.dispatchEvent`** | ✅ (tick 41) — **it did not exist**, with a whole window-listener registry sitting behind it. `window.dispatchEvent(new Event('resize'))` is how a router tells the app it navigated. |
| `document.title` (get **and set**) / `.referrer` / `.characterSet` / `.currentScript` | ✅ (tick 41) — all were `undefined`, and `undefined.split(…)` is a `TypeError` |
| `navigator.vendor` | ✅ (tick 41) — read on the first line of every UA-sniffing bundle |
| **`navigator.deviceMemory` + canonical `navigator.platform`** — the completeness-identity surface every logged-in app cross-checks | **logged-in apps behind UA/CH-sniffing gates** (LinkedIn, banks, Cloudflare-protected enterprise consoles) that branch on `navigator.platform === 'Linux x86_64'` / `/^Linux/` and adaptive-loading bundles that read `navigator.deviceMemory` | ✅ (tick 458, `G_DEVICE_IDENTITY`) — `deviceMemory` was **absent** (`if (deviceMemory<4)` silently took the wrong branch; `deviceMemory.toFixed()` threw; its absence beside the present `userAgentData` was itself a headless inconsistency), and `platform` was the raw lowercase `"linux x86_64"` where every real browser reports the canonical `"Linux x86_64"`/`"MacIntel"`/`"Win32"` — the exact LinkedIn degraded-path tell. Now `deviceMemory: 8` (spec-quantised {0.25,0.5,1,2,4,8}-capped-at-8, honest at the spec's granularity, consistent with `hardwareConcurrency:4`) and a canonical OS→token `platform` (Axis F: what we ARE, in the browser-canonical casing). RED-proven by reverting both. Residue: whether to advertise a *recognized Chrome* UA string is the only remaining identity policy call. |
| ~40 interface names (`ProgressEvent`, `HTMLFormElement`, `NodeList`, …) | ✅ (tick 41) — inert but **present**: a referenced name that does not exist is a `ReferenceError`, not a `false` |
| **The page's own `fetch()`/XHR — actually PERFORMED during load** | ✅ (tick 41) — see below. This one is bigger than it looks. |

### A missing constructor is a thrown exception, and its blast radius is whatever was rendering

`canvas.getContext` was used by **3%** of sites and **broke 100% of them**. `WebSocket` was missing and
took an entire **news front page** with it: aljazeera.com's **2,591 server-rendered elements became 141**,
because a live-blog client constructed one at boot, React's render threw, and its error boundary showed a
skeleton where the article had been.

Fixing that revealed `Blob`. Fixing `Blob` revealed `FileList`. **Each was a different library's first
line.** A page does not get to run its fallback path if the *check* for the fallback throws.

**Construct successfully, and answer honestly.** A blank canvas, an unopened socket, an empty `Blob` are
all survivable — every library on the web is written to survive them, because real browsers produce
exactly those behind captive portals and in private windows. **A `ReferenceError` is survivable by
nothing.** Gated by `G_GLOBALS`.

### The page's own fetches were never performed outside the shell

`take_fetches()` handed a page's `fetch()`/XHR calls to the **shell**, and the shell alone performed them.
So the **oracle**, `boxes`, the agent — every consumer that is not the shell — queued a data-driven SPA's
API calls and **never made them**. The app sat in its loading state and rendered a skeleton.

**This is why the oracle reported 13,741 "missing" nodes.** A measurement harness that cannot load a
modern site's content is not measuring the browser; it is measuring itself. `finish_loading` now performs
them, in rounds, inside the load budget.

## Tick 42 — `:has()`, hand-rolled rather than forked

| Pattern | % of the web | Status |
|---|---|---|
| **`:has()`** — subject, descendant, `>`, `+`, `~`, forgiving list | **13% of sites** | ✅ (tick 42) — **Stylo DISCARDS these rules at parse.** Matched by our own selector engine in a supplement pass. |

**Cost re-measured and removed (tick 580).** The supplement was correct from tick 42 but priced per
element: it re-walked every rule of every `:has()`-carrying sheet, re-evaluating each rule's `@media` and
re-asking each selector whether it was relative — for every element on the page. Since **13% of sites use
`:has()`**, and a site that uses it usually uses it across many sheets, the class carried a standing tax.
Measured on 60 sheets × 18,125 elements, adding one `:has()` rule per sheet cost **~+14% of the whole
cascade**; after hoisting the collection out (`collect_relative_rules`, once per cascade) the delta's sign
flips and it is no longer distinguishable from zero. The `n` that drives it is the **sheet count**, not
rules-per-sheet — quadrupling rules within a sheet moved almost nothing, which is what made the first
measurement look like a refutation.
| CSS nesting | ≥41% | ✅ (tick 39) |

**Stylo's *servo* build hardcodes `parse_has() -> false`** (Gecko's returns `true`), so a selector
containing `:has()` **fails to parse and CSS error-recovery discards the whole rule** — its declarations
never reach the cascade at all.

Enabling it upstream costs **vendoring Stylo** (`./stylo` is a *reference checkout that nothing builds*;
the dependency is `stylo = "0.19"` from crates.io). So instead we extended **the selector engine we already
own** — the one behind `querySelectorAll` — and apply the discarded rules in a second cascade pass, ordered
by `(specificity, source order)`.

**The known, bounded inaccuracy, stated rather than discovered:** a low-specificity `:has()` rule cannot
currently *lose* to a higher-specificity normal rule, because Stylo does not tell us which rule won each
property. That is strictly better than the rule not existing — and it is written down.

**The ladder this establishes** (STATUS.md): *pref → minimal flag delta → **hand-rolled supplement** →
hand-rolled module.* **Never: give up the capability.**

## Tick 43 — the document lifecycle: the class of site that never initialised

| Pattern | % of the web | Status |
|---|---|---|
| **`window.addEventListener('load', init)`** — the classic init idiom | **most of the web** | ✅ (tick 43) — **`load` was NEVER dispatched. Ever.** Every site initialising this way ran nothing. |
| **`document.addEventListener('DOMContentLoaded', init)`** | **most of the web** | ✅ (tick 43) — never dispatched either |
| **`document.readyState`** guards (`if (readyState !== 'loading') init()`) | jQuery = ~74% of pages | ✅ (tick 43) — **this is why the gap survived 40 ticks**: libraries that *check* readyState fell through to running immediately, so it *worked often enough to look fine*. Libraries that only *listen* got nothing. |
| **Delay-ordered timers** — debounce, throttle, retry-backoff, staged animation, carousels | **effectively all interactive sites** | ✅ (tick 43) — `setTimeout` **discarded its delay**; timers ran in *insertion* order. Nothing errored; it simply happened in the wrong order. |
| **A page whose first timer callback throws** | long tail | ✅ (tick 43) — one throwing callback used to **stop the page's clock forever** (Bar 0) |
| **Self-referential DOM mutation** (`node.after(node)`) | adversarial / library-internal | ✅ (tick 43) — was an **infinite loop** (Bar 0). No real site does this — **which is why only WPT could find it.** |
| **`Range`** (`dom/ranges` = 3.9%) | rich-text editors, selection APIs | ❌ **inert stub** — exists, does nothing. Now *visible* rather than assumed. |

**The class this unlocks is not a *kind of site* — it is a *stage of every site*.** A page whose
scripts parse, whose DOM builds, and whose init handler is never called renders its **skeleton**: the
server-rendered HTML, with nothing wired up. That is indistinguishable, from the outside, from a
hydration failure — and it is a large part of what the oracle has been reporting as "missing nodes" for
forty ticks.

## Tick 44 — activation, and text mutation

| Pattern | Reach | Status |
|---|---|---|
| **`element.click()`** — menus, modals, carousels, "click the hidden file input", every framework's programmatic activation | **effectively every interactive site** | ✅ (tick 44) — **did not exist.** A `TypeError` on the call, taking down whatever was running. |
| **Delegated click handling** (`document.addEventListener('click', …)`) | ubiquitous | ✅ (tick 44) — a programmatic click now **bubbles** |
| **`CharacterData`**: `length`, `substringData`, `appendData`, `insertData`, `deleteData`, `replaceData` | every text-editing surface; the DOM's own range/normalize machinery | ✅ (tick 44) — **only `data` existed.** WPT scored `replaceData` 0/34. |
| **Text indexing in non-Latin scripts** (emoji, CJK, combining marks) | **the majority of the world's users** | ✅ (tick 44) — offsets are **UTF-16 code units**; counting Rust `char`s corrupts every surrogate pair, silently, *only for the people who write in those scripts* |
| **`DOMException` thrown by DOM methods** | every `try/catch` around a DOM call | ⚠️ **partial** — CharacterData throws `IndexSizeError`; the rest of the DOM still does not throw. That is the WPT work list. |
| **Full activation behaviour** (click toggles a checkbox / follows a link / submits a form) | forms, nav | ❌ **follow-on** — `click()` fires the event; it does not yet run activation behaviour |

## Tick 46 — the multi-document process (i.e. every browser)

| Pattern | Reach | Status |
|---|---|---|
| **A page holding a handle from a previous document** | **every multi-page process — which is every browser** | ✅ (tick 46) — was a **SIGSEGV that killed every tab**. A reflector's bare `NodeId` indexed past the end of a *different, smaller* arena, inside an `extern "C"` native where a Rust panic **cannot unwind**. |
| **A panic anywhere inside a JSNative** | Bar 0 | ❌ **OPEN** — still aborts the process. `catch_unwind` at the native boundary is the real containment. |

**The class this closes is not a kind of site — it is a property of the browser itself.** Any engine that
reuses one process for many documents has this bug until it proves otherwise, and **it is invisible to
single-page testing**: the failing file passes alone, and passes in a 120-file batch; it dies only when it
runs *after other documents*.

## Tick 47 — Bar 0 containment at the JS boundary

| Pattern | Reach | Status |
|---|---|---|
| **A page that makes any DOM call hit an internal bug** | **every page, every DOM method** | ✅ (tick 47) — a panic in any of the **112 page-callable natives** used to be a **SIGSEGV that killed every tab**. It is now caught at the FFI edge, **logged loudly**, and the page carries on. |
| **SpiderMonkey engine callbacks** (module hooks, rejection tracker) | not page-callable | ⚠️ residual, named |

**This is not a class of *site* — it is Bar 0's founding promise finally being true of the JS boundary:
a bad page kills the PAGE, not the browser.** Every DOM method written from here on is born contained.

## Tick 48 — the detached document

| Pattern | Reach | Status |
|---|---|---|
| **`createHTMLDocument()`** — the sanitizer's safe detached tree | **DOMPurify and every sanitizer** | ✅ (tick 48) — a real second document in the arena |
| **A cycle-forming insertion** (`node.appendChild(itsAncestor)`) | Bar 0 / adversarial | ✅ (tick 48) — throws `HierarchyRequestError`; was **unguarded and would hang** |
| **`createEvent`/`initEvent`** | jQuery, GA, legacy code | ❌ deferred — exposes an infinite dispatch loop (Bar 0) |

## Tick 49 — the parity-scope map (infrastructure, no new capability)

No web capability changed this tick (CI lane + WPT horizon map + fmt). Recorded here only so the
capability ledger stays complete: **`docs/wiki/wpt-horizon.md`** is now the spec-shaped anchor of *which*
classes of the web to target next, feeding the same `usage × divergence` ranking this ledger uses.

## Tick 50 — the engine in a browser (infrastructure)

No web capability changed (a portability fix + wasm feasibility). Recorded so the ledger stays complete:
the render pipeline (dom · css+stylo · layout · paint · html · text) now compiles to `wasm32-unknown-unknown`,
which unblocks the **in-browser demo** — a visitor running our *actual* Stylo/Taffy/tiny-skia pipeline live
(`docs/loop/DEMO.md`). The enabling fix (`NodeId` → `u64`) also hardens the ARM/cross-platform target.

## Tick 51 — CI + OOM guard (infrastructure)

No web capability changed. Recorded for ledger completeness: the **headless configuration** (no GUI, no JS
— the substrate the wasm demo, the agent and mac/windows CI all build on) **had silently stopped
compiling** and now builds again, which is a prerequisite for the in-browser demo class of work.

## Tick 52 — CI observability (infrastructure)

No web capability changed. Recorded for completeness: CI failures are now **readable** (public check-run
annotations carry the real compiler error), which is a prerequisite for getting the cross-platform and
wasm lanes green without guessing.

## Tick 53 — the repo is buildable by anyone who clones it (infrastructure)

No web capability changed. But: a committed `.cargo/config.toml` hard-coded `rustc-wrapper = "sccache"`,
which made **the repository unbuildable for every contributor and every CI runner without that tool
installed**. Now opt-in. This is a prerequisite for anyone else ever running this engine.

## Tick 54 — Windows buildability (infrastructure)

No web capability changed. But the engine now links on **Windows**: an unpinned `tokio-rustls` was pulling
the `aws-lc-rs` crypto backend (NASM/CMake) into a graph that had otherwise been pinned to pure-Rust
`ring`, and it failed the Windows link. Cross-platform reach is a prerequisite for "a daily driver", not a
footnote.

## Tick 55 — Windows links (infrastructure)

No web capability changed. The engine now links on **Windows**: a committed `+crt-static` was being forced
on *every* Windows build (not just the static-release one), which cannot link. Cross-platform reach is a
prerequisite for a daily driver.

## Tick 57 — the engine, in the visitor's browser

| Pattern | Status |
|---|---|
| **The engine compiled to wasm, rendering real pages on a canvas** | ✅ (tick 57) — Stylo + Taffy + tiny-skia executing in the browser; scroll re-renders, hover hit-tests |
| **Fonts in a no-filesystem target** | ✅ (tick 57) — Liberation faces compiled into the binary (the same ones Chrome's Arial/Times resolve to) |
| **JS in the demo** | ❌ **permanent** — SpiderMonkey is C++ and does not target wasm. Stated in-product. |

## Tick 58 — cycle wall time (infrastructure)

No web capability changed. The verify wall went 92.6s → 40.3s with every gate intact, and the fidelity gate
became **deterministic** (one snapshot, both engines) in the process — it had been fetching live sites on
every tick, which is a rigor bug wearing a performance bug's clothes.

## Tick 59 — the platform web's live viewport

| Pattern | Reach | Status |
|---|---|---|
| **Lazy-loaded image feeds** (IntersectionObserver + `data-src`) | **the dominant content-loading pattern on the modern web** | ✅ **verified end-to-end + gated** (tick 59) — was already built; **nothing proved it, so the ledger called it missing** |
| **Infinite scroll** (scroll/IO → fetch more) | social, news, commerce | ✅ the primitive is live (IO fires, scroll fires); tick 140 fixed `rootMargin` — it was single-token, so the `'0px 0px 300px 0px'` **prefetch-early** idiom was silently dropped and the feed loaded late. Now a 4-side shorthand (px/%), bottom margin applied asymmetrically |
| **Sticky headers, scroll-linked animation, virtualization** | ubiquitous | ✅ same primitive — *one gap seen five times, and it was closed* |
| **Horizontal carousels / galleries** (IO-driven lazy slides) | product galleries, media rails | ✅ tick 141 made IO intersection **2-D** — off-screen-to-the-side slides no longer report intersecting (were eager-loading every slide); left/right `rootMargin` now live |
| **Native `loading="lazy"`** | perf hint | ❌ not honoured (renders correctly; fetches eagerly) |

## Tick 60 — DOM code that catches errors

| Pattern | Reach | Status |
|---|---|---|
| **`text.appendChild(x)` throws** | **tree integrity** | ✅ (tick 60) — it used to **succeed**, leaving a subtree on a text node that nothing can render |
| **`insertBefore` with a non-child reference throws `NotFoundError`** | every framework's insert path | ✅ (tick 60) — used to silently **append somewhere else** |
| **`removeChild` of a non-child throws `NotFoundError`** | **every framework's unmount path** | ✅ (tick 60) — used to silently do nothing, turning a loud bug into a leak |
| The rest of the DOM's `assert_throws_dom` surface | — | ⚠️ ~500 more in the WPT work list |

## Tick 96 — the `<body onload>` bootstrap fires exactly once

| Pattern | Reach | Status |
|---|---|---|
| **`<body onload>` handler fires ONCE** | **every geometry-assertion suite + legacy page bootstrap** | ✅ (tick 96) — it used to fire **twice** (dispatch AND an explicit call), corrupting any non-idempotent handler |
| **`check-layout-th.js` (`checkLayout` → `done()`) reports honestly** | css-flexbox / css-grid / css-sizing / any `data-expected-*` suite | ✅ (tick 96) — the second fire created duplicate `test()`s after `done()` → whole file reported a harness error |
| **`window.onload = fn` and `addEventListener('load', fn)` each fire once** | ubiquitous page init | ✅ (tick 96) — same single-dispatch guarantee |
| Non-idempotent onload work generally (form submit, single-run counters) | broad | ✅ (tick 96) — no longer doubled |

## Tick 97 — layout geometry reads as spec integers

| Pattern | Reach | Status |
|---|---|---|
| **`el.offsetWidth === 133`** (exact integer geometry reads) | sizing/measuring scripts, `data-expected-*` test suites | ✅ (tick 97) — offset*/client*/scroll{W,H} return rounded integers per CSSOM, not raw floats |
| **`scrollTop`/`scrollLeft` stay fractional** | smooth-scroll / high-DPI scroll math | ✅ (tick 97) — correctly NOT rounded (they are `double`) |
| **`getBoundingClientRect()` stays fractional** | sub-pixel layout math | ✅ (tick 97) — untouched; only the integer metrics round |

## Tick 98 — shrink-to-fit wraps a child's full margin box

| Pattern | Reach | Status |
|---|---|---|
| **Shrink-to-fit around a child with margins** (flex/grid item, inline-block, float, table cell sizing to content) | ubiquitous in layout | ✅ (tick 98) — content extent now includes the child's right margin (margin box), was short by one margin |

## Tick 99 — attribute-selector case flag `[attr=val i]` and namespace prefix

| Pattern | Reach | Status |
|---|---|---|
| **`[type='text' i]`, `[data-state='OPEN' i]`** — ASCII case-insensitive attribute matching | forms, data-attribute state selectors, any CSS/JS targeting HTML values case-insensitively | ✅ (tick 99) — the `i` flag used to be **stripped and ignored**, so the selector matched case-sensitively and `querySelector` returned `null` |
| **`[attr=val s]` and unflagged matching stay case-SENSITIVE** | correctness guard | ✅ (tick 99) — `s` forces case-sensitive; the flag can't leak case-insensitivity into plain matching (gated must-not-match) |
| **`[*|foo]`, `[|foo]`** — namespaced attribute selectors | XHTML-origin selectors, libraries emitting explicit-namespace attribute rules | ✅ (tick 99) — the `*|`/`|` prefix now resolves to the local name (HTML: null namespace), was carried into the name and matched nothing |
| The case flag on every operator (`~= ^= $= *=`) | broad | ✅ (tick 99) — applies uniformly, not just to `=` |

## Tick 100 — `classList` is an ordered set, and no-ops preserve the raw attribute

| Pattern | Reach | Status |
|---|---|---|
| **`el.classList.remove('x')` strips EVERY occurrence** | every framework's class toggling | ✅ (tick 100) — was deduped-blind, spliced only the first index (`"a b a"` → `"b a"`) |
| **A modifying op serializes the DEDUPED set** | broad | ✅ (tick 100) — `class="a a b"` + `add('c')` → `"a b c"`, was `"a a b c"` |
| **A no-op `toggle`/`replace` leaves the raw attribute untouched** | class-state churn | ✅ (tick 100) — `toggle('z', false)` when absent preserves `"a  b"` whitespace; was re-serialized every call |
| **`classList.value` / `String(classList)` return the RAW attribute; `length`/indexing use the deduped set** | iteration + measurement | ✅ (tick 100) — the raw-vs-set split, was conflated |

## Tick 101 — `Range.createContextualFragment` (HTML string → DocumentFragment)

| Pattern | Reach | Status |
|---|---|---|
| **`range.createContextualFragment(html)`** | sanitizers, `jQuery.parseHTML`, "string → nodes → insert" idioms | ✅ (tick 101) — was entirely absent (0 refs); failures surfaced as unhandled promise rejections downstream |
| **Result is a `DocumentFragment` (nodeType 11), parsed in the start node's context** | correctness | ✅ (tick 101) — reuses the `insertAdjacentHTML` fragment parser, `<html>`→`<body>` context fallback |
| **Zero-argument call is a `TypeError`** | WebIDL required-arg semantics | ✅ (tick 101) — not a parse of `"undefined"` |

## Tick 102 — getComputedStyle exposes visibility / white-space / opacity (ratchet-neutral, correct)

| Pattern | Reach | Status |
|---|---|---|
| **`getComputedStyle(el).visibility` / `.whiteSpace` / `.opacity`** | ubiquitous in real scripts + tests | ✅ (tick 102) — were `undefined` though the cascade already computed them; now surfaced (camelCase + kebab `getPropertyValue`) |
| **Initial values resolve too** (`visibility:"visible"`, `opacity:"1"`) | correctness | ✅ (tick 102) — unset ≠ undefined |
| appearance / caret-color computed values | form-control styling | ⚠️ deferred — need new ComputedStyle fields + Stylo extraction (the scored css-ui lever) |

## Tick 103 — document.elementFromPoint hit-testing

| Pattern | Reach | Status |
|---|---|---|
| **`document.elementFromPoint(x, y)`** | drag-and-drop, tooltips, custom controls, hit-test suites | ✅ (tick 103) — was entirely missing; returns the deepest element box containing the point, else null |
| **Miss / non-finite coord → null** | correctness | ✅ (tick 103) — CSSOM-View semantics |
| **Transformed hit areas** | transformed UI | ⚠️ follow-on — rects are pre-transform; flipped the untransformed-box cases (+25 css-transforms), transform-aware quad test is next |

## Tick 107 — element.getClientRects()

| Pattern | Reach | Status |
|---|---|---|
| **`el.getClientRects()`** | geometry measurement (layout libs, scroll math, drag) | ✅ (tick 107) — was missing; returns a DOMRectList (`.item(i)` + indexed) from the layout snapshot |
| **`display:none` → empty list, not a zero rect** | correctness | ✅ (tick 107) — the getBoundingClientRect distinction |
| Inline boxes wrapping across lines (multi-rect) | inline measurement | ⚠️ follow-on — returns the single bounding box (block/replaced majority) |

## Tick 108 — high-usage DOM ergonomics: isConnected / toggleAttribute / webkitMatchesSelector

| Pattern | Reach | Status |
|---|---|---|
| **`node.isConnected`** | every framework's detach/reattach guard | ✅ (tick 108) — was absent; true iff the node reaches the document root |
| **`el.toggleAttribute(name[, force])`** | boolean-attribute ergonomics (hidden, disabled, aria-*) | ✅ (tick 108) — add/remove/force, returns presence |
| **`el.webkitMatchesSelector(sel)`** | legacy code still shipping the prefixed alias | ✅ (tick 108) — aliased to matches |

## Tick 109 — Node interface constants + compareDocumentPosition

| Pattern | Reach | Status |
|---|---|---|
| **`Node.ELEMENT_NODE` / `TEXT_NODE` / … constants** | `n.nodeType === Node.ELEMENT_NODE` — everywhere | ✅ (tick 109) — were undefined → the comparison ran silently false; +128 html/dom |
| **`node.compareDocumentPosition(other)` + `DOCUMENT_POSITION_*`** | DOM ordering, focus/selection managers | ✅ (tick 109) — CONTAINS/CONTAINED_BY/PRECEDING/FOLLOWING/DISCONNECTED |

## Tick 110 — DOMException legacy codes + Event phase constants

| Pattern | Reach | Status |
|---|---|---|
| **`DOMException.NOT_FOUND_ERR` / `INDEX_SIZE_ERR` / … (legacy codes)** | `e.code === DOMException.X_ERR` checks | ✅ (tick 110) — were undefined; 25 codes on ctor+prototype, matching instance `.code` |
| **`Event.AT_TARGET` / `CAPTURING_PHASE` / `BUBBLING_PHASE`** | `e.eventPhase === Event.AT_TARGET` dispatch checks | ✅ (tick 110) |

## Tick 111 — global HTMLElement attribute reflection (+18,245)

| Pattern | Reach | Status |
|---|---|---|
| **`el.dir` / `el.hidden` / `el.tabIndex` / `el.accessKey` / …** on every element | ubiquitous — frameworks read/write these constantly | ✅ (tick 111) — were undefined on non-specific elements; a `"*"` global row reflects them everywhere (+18k html/dom) |
| Tag-specific attributes stay inert on other elements | correctness | ✅ (tick 111) — the global fallback does not clobber (`div.disabled` still undefined) |
| ARIA + whole-tree idlharness reflection | the rest of the ~35k mass | ⚠️ crash-gated on the effective-stack-quota fix (re-scoped tick 106/110) |

## Tick 112 — lang reflection (getter-only fallback gains a setter)

| Pattern | Reach | Status |
|---|---|---|
| **`el.lang = x` reflects to the content attribute** | every i18n/framework path | ✅ (tick 112) — lang had a getter fallback but no setter; setting was silently dropped. +4560 html/dom |
| `title` stays native (not double-defined) | correctness | ✅ (tick 112) — a reflected `title` over the native accessor CRASHED (css-grid 35); reverted, kept only lang |

## Tick 113 — HTML attribute qualified names are ASCII-lowercased (+10,249)

| Pattern | Reach | Status |
|---|---|---|
| **`el.setAttribute('accessKey'/'tabIndex'/'noValidate'/…, v)`** stores the lowercase content name | every mixed-case IDL-attribute write — frameworks, forms, editors | ✅ (tick 113) — DOM §Element lowercases the qualified name for HTML-namespaced elements; we stored it **verbatim**, so `getAttribute('accesskey')` and the reflected getter `el.accessKey` both missed it. A shared `attr_qname` folds at store+lookup in all five natives. **html/dom 45,495 → 55,744 (+10,249)** |
| SVG/MathML attributes keep their case (`viewBox`, `preserveAspectRatio`) | foreign-content correctness | ✅ (tick 113) — the fold applies iff `namespace` is `None` (HTML); `Some(ns)` preserves case |
| `setAttributeNS`/`getAttributeNS`/… stay **case-preserving** | XML/SVG namespaced attributes | ✅ (tick 113) — routed through new `__*AttrExact` natives that skip the fold, so `setAttributeNS(ns,'Abc',v)` still stores `Abc` |

## Tick 114 — the HTMLDocument named collections (+39)

| Pattern | Reach | Status |
|---|---|---|
| **`document.forms` / `images` / `links` / `scripts` / `embeds` / `plugins` / `anchors`** | every form library/serializer + analytics/ad/prerender scanners; `document.forms.length` was a TypeError | ✅ (tick 114) — were all `undefined`; each now a static Array over a shared `doc_collection` selector helper, tree-ordered. **html/dom 55,744 → 55,783 (+39)** |
| **`document.getElementsByName(n)`** matches ANY element by its `name` content attribute | legacy control resolution | ✅ (tick 114) — enumerates `"*"`, filters stored `name` (exact string); resolves because tick 113 lowercases the `name` key |
| `document.links` = `a`/`area` **with href**; `anchors` = `a` **with name**; `plugins`≡`embeds` | spec-correctness (a bare `<a name>` is not a link) | ✅ (tick 114) — encoded in the selectors, gated by `g_doc_collections` |

## Tick 115 — the locate-a-namespace algorithm (`lookupNamespaceURI` / `isDefaultNamespace`)

| Pattern | Reach | Status |
|---|---|---|
| **`node.lookupNamespaceURI(prefix)` / `node.isDefaultNamespace(ns)`** | XML/SVG-aware code, serializers, sanitizers, any script touching namespaces | ✅ (tick 115) — were `undefined` (a `TypeError`); now the full DOM §Node "locate a namespace" algorithm on `Node.prototype`, inherited by every node type |
| `xml`/`xmlns` prefixes are always bound and un-overridable; HTML element's own xhtml namespace wins over its `xmlns` attr | spec-correctness | ✅ (tick 115) — pinned by `g_namespace_lookup` (27 branch cases from WPT) |
| A comment/text resolves on its parent **element**, not by climbing to the document | spec-correctness | ✅ (tick 115) |

## Tick 116 — `nodeName` per node type + namespace casing

| Pattern | Reach | Status |
|---|---|---|
| **`element.nodeName`** case-preserved outside the HTML namespace | every DOM-diffing lib and serializer keys on nodeName; SVG/XML content | ✅ (tick 116) — was uppercased unconditionally; now mirrors `tagName` (HTML→upper, else preserved) via `Dom::node_name` |
| **`nodeName` of comment/document/fragment/doctype** | correctness | ✅ (tick 116) — every non-element returned `"#text"`; now `#comment`/`#document`/`#document-fragment`/doctype-name |

## Tick 125 — `getElementsByTagNameNS` — the namespace-aware query (+44)

| Pattern | Reach | Status |
|---|---|---|
| **`el.getElementsByTagNameNS(ns, local)`** / **`document.getElementsByTagNameNS`** enumerate by (namespace, localName) with `"*"` wildcards | every SVG/MathML/XML-touching tool, sanitizers and serializers that walk foreign content by namespace, and — the real yield — every `dom/nodes` test that queries by namespace to check something else | ✅ (tick 125) — was `undefined` (`TypeError: not a function`). Native `el_get_by_tag_ns` on both prototypes; local name derived exactly as `localName` (post-prefix, case-sensitive for foreign / lowercased for HTML); result is a **live `HTMLCollection`** via `collections_js`. An HTML element (`namespace: None`) is matched as XHTML, so `(XHTML, "div")` finds page divs. **dom 3052 → 3096 (+44)**, gate `g_get_by_tag_ns` |
| the genuinely-empty-string namespace edge is the *one* unserved query | spec-conformance only (`createElementNS("", x)` is ~never on the real web) | ⚠️ known-limit (tick 125) — `None` storage conflates null-ns with XHTML; serving `getElementsByTagNameNS("", "*")` needs the null-vs-XHTML storage split (a subsystem: `namespaceURI`/`tagName`-casing/parser). Stated, not hidden — 2 subtests left RED |

## Tick 123 — `Text.splitText()` + `wholeText` (+8)

| Pattern | Reach | Status |
|---|---|---|
| **`text.splitText(offset)`** splits a Text node in two, returning the tail | rich-text editors, text-diffing, template engines that carve text runs; the DOM Range/Selection machinery builds on it | ✅ (tick 123) — was `TypeError` (not a function); now a native (new node as next sibling, `IndexSizeError` on overflow). Live-Range boundary adjustment deferred. Gate `g_split_text` |
| **`text.wholeText`** reads a contiguous Text run back as one string | normalization-aware reading of split text | ✅ (tick 123) — was `undefined`; walks contiguous Text siblings |

## Tick 133 — the `CharacterData` abstract base interface (+9)

| Pattern | Reach | Status |
|---|---|---|
| **`node instanceof CharacterData`** (and code that branches on the CharacterData base) | DOM-walking libs, sanitizers and serializers that test `instanceof CharacterData` to treat Text/Comment/PI uniformly; every WPT file that asserts it before its real checks | ✅ (tick 133) — `CharacterData` was never installed as a global, so the check threw a ReferenceError; now `iface('CharacterData', nodeType ∈ {3,8,7,4})`. Gate `g_characterdata_iface`. **whole dom 3603 → 3612 (+9)**; `Document-createTextNode` 0/6 → 6/6 |

## Tick 132 — `getElementsByClassName` splits on ASCII whitespace, not Unicode (+30)

| Pattern | Reach | Status |
|---|---|---|
| **`getElementsByClassName`** with class names containing non-ASCII "space" characters or selector metacharacters (`.`, `#`, `:`, `[`) | any page whose class names include U+00A0/em-space/etc. (CMS output, i18n, obfuscated builds) or dotted BEM-ish names; jQuery `.getElementsByClassName` fast paths | ✅ (tick 132) — split used Rust `split_whitespace()` (Unicode White_Space), which split those class names into empty tokens; now ASCII-whitespace-only (TAB/LF/FF/CR/SPACE) and filters elements directly instead of building a `.{class}` CSS-selector string. Gate `g_class_ascii_whitespace`. **whole dom 3573 → 3603 (+30)**; the `getElementsByClassName-whitespace-class-names` file 0/26 → 26/26 |

## Tick 131 — `HTMLCollection` iterable surface + numeric `namedItem` (+7)

| Pattern | Reach | Status |
|---|---|---|
| **`for..of` / spread over an `HTMLCollection`**, and correct feature-detection (`"forEach" in coll`) | code that iterates `getElementsByTagName`/`.children` results, and libs that branch on whether a collection has `forEach`/`entries` (HTMLCollection vs NodeList) | ✅ (tick 131) — HTMLCollection wrongly exposed `values`/`entries`/`keys`/`forEach` (NodeList-only) and `Symbol.iterator in coll` read false; now the iterable members are per-type and `@@iterator` is reported consistently |
| **`coll.namedItem(-2)` / numeric named access** resolves `id="-2"` | legacy DOM code reaching elements by numeric-looking id/name through the method form | ✅ (tick 131) — `namedItem` compared a number against string ids and never matched; now string-coerced. Gate `g_collection_iterator_indices`. **whole dom 3566 → 3573 (+7)** |

## Tick 130 — `dataset`/`attributes` enumerate their supported names (+9)

| Pattern | Reach | Status |
|---|---|---|
| **`Object.keys(el.dataset)` / `for..in` / `{...el.dataset}`** yields the camelCased `data-*` names | every framework/lib that snapshots or spreads a `data-*` set (state hydration, analytics dataLayer, component prop mirroring) | ✅ (tick 130) — `dataset` was a `Proxy` with no `ownKeys`; `getOwnPropertyNames` saw the empty target. Now enumerates `data-*` → camelCase (`data-date-of-birth` → `dateOfBirth`, `data-` → `""`) |
| **`Object.getOwnPropertyNames(el.attributes)`** = indices ++ attribute names (no `length`) | DOM serializers/sanitizers (DOMPurify) and diffing libs that enumerate an element's attribute map | ✅ (tick 130) — `NamedNodeMap.ownKeys` pushed indices + `'length'` and no names; now indices ++ qualified names, `length` off the own set, named descriptors `[LegacyUnenumerableNamedProperties]`. Gate `g_dataset_attrs_enum`. **whole dom 3557 → 3566 (+9)**; closes the `dom/collections` supported-property-names cluster |

## Tick 129 — `HTMLCollection` is a WebIDL legacy platform object (+21)

| Pattern | Reach | Status |
|---|---|---|
| **Named access on collections** — `document.forms.login`, `getElementsByTagName('span').someId`, `collection.namedItem('x')` resolve by `id` / HTML `name` | every legacy DOM idiom that reaches an element by name off a live collection (forms, image maps, plugin-era markup), and every framework/test that enumerates a collection | ✅ (tick 129) — `namedItem` matched `.id === ''` so every element answered the empty string, and named properties were not exposed as own properties at all. Now supported names = every `id` + every HTML-namespace `name` (tree order, deduped, non-empty), exposed as `[LegacyUnenumerableNamedProperties]` |
| **`Object.getOwnPropertyNames`/`Object.keys`/spread over a collection** returns `[...indices, ...names, ...expandos]` — never `length` | collection introspection in polyfills, serializers, `Array.from` shims, dev tools | ✅ (tick 129) — `ownKeys` pushed `'length'` (a prototype accessor) and no names; now spec-correct, and named descriptors are `writable:false, enumerable:false, configurable:true` |
| **Read-only index/named + expando shadowing** — `coll["some-id"] = 5` is a no-op (TypeError in strict); an expando set before a name exists shadows the later named element | correctness for code that assigns onto collections or does `Object.create(coll)` | ✅ (tick 129) — new `set`/`defineProperty`/`deleteProperty` traps reject shadowing; `length` is branded (`Object.create(coll).length` throws) and `[[Set]]` through a non-collection receiver lands as an own property. Gate `g_collection_named_props`. **whole dom 3536 → 3557 (+21)**; `NodeList` kept byte-for-byte to avoid perturbing the tracked cross-file UAF |

## Tick 128 — `Node.lookupPrefix` + the DocumentType namespace-lookup surface (+20)

| Pattern | Reach | Status |
|---|---|---|
| **`node.lookupPrefix(namespace)`** returns the in-scope prefix for a namespace URI | namespace-aware SVG/MathML/XML code, XML serializers choosing a prefix, any DOM code doing the `lookupNamespaceURI` round-trip | ✅ (tick 128) — was a `TypeError` on *every* node (registered nowhere, unlike its sibling `lookupNamespaceURI`); now `Dom::lookup_prefix` (own-namespace prefix → `xmlns:<p>` declaration → recurse to parent element), native `el_lookup_prefix` on the shared prototype. **+11** |
| **`DocumentType.lookupNamespaceURI`/`lookupPrefix`/`isDefaultNamespace`** | `dom/nodes` calls them directly on a doctype; namespace code that walks mixed trees | ✅ (tick 128) — a doctype is a JS shim with none of the Node namespace surface; the spec answers are constant (a doctype has no parent element to climb): both lookups `null`, `isDefaultNamespace` true only for null/empty. **+9**, gate `g_lookup_prefix` |

## Tick 127 — DOM validation throws are real `DOMException`s, not decorated `Error`s (+420)

| Pattern | Reach | Status |
|---|---|---|
| **A DOM validation error is a real `DOMException`** — `e.code` set, `e instanceof DOMException` true, `e.constructor === DOMException` | every `catch` block that branches on `e.code === DOMException.SYNTAX_ERR` or `e instanceof DOMException` (sanitizers, editors, framework unmount paths, classList/attribute helpers), and the entire `assert_throws_dom` conformance surface which checks `.code` before the name | ✅ (tick 127) — `classList.add`/`createAttribute('')`/`setAttributeNS`/`removeNamedItem`/`Range.setStart`-OOB/`compareBoundaryPoints` threw `new Error(); e.name = 'X'` — `.code` undefined, `.constructor` Error. Now `throw new DOMException(msg, name)` via the existing global polyfill (maps `.code`, chains `Error.prototype`). **whole dom 3096 → 3516 (+420)**, gate `g_dom_exception`, pure-JS (zero Bar-0 risk) |
| **WebIDL `TypeError` where the spec says `TypeError`** — `new MutationObserver(nonfn)`, `observe()` with no fields, `classList.supports()` | correctness for feature-detection and error-branching code | ✅ (tick 127) — were decorated `Error`s named `'TypeError'` (not `instanceof TypeError`); now real `new TypeError(...)` |

## Tick 122 — constructable node interfaces: `new Text`/`new Comment`/`new DocumentFragment` (+29)

| Pattern | Reach | Status |
|---|---|---|
| **`new Text(d)` / `new Comment(d)` / `new DocumentFragment()`** mint real detached nodes | every library/test that builds nodes with the constructors instead of `document.create*` (common in test harnesses and vDOM code) | ✅ (tick 122) — were the generic `iface()` **inert** constructor returning `{data: undefined, nodeType: undefined}`; now real constructors delegating to `document.create*` with the `instanceof` predicate preserved. **whole dom 3016 → 3045 (+29)**, gate `g_node_constructors`, pure-JS-prelude (zero Bar-0 risk) |

## Tick 121 — the typed Event subclass hierarchy (instanceof + inherited members) (+41)

| Pattern | Reach | Status |
|---|---|---|
| **`new MouseEvent`/`WheelEvent`/`KeyboardEvent`/`UIEvent`/`CompositionEvent`** carry their inherited members and satisfy the `instanceof` chain | every library that constructs synthetic events (test frameworks, drag/gesture libs, `dispatchEvent` polyfills) and every handler that reads `e.view`/`e.detail`/`e.relatedTarget`/`e.deltaX`/`e.location` or branches on `e instanceof UIEvent` | ✅ (tick 121) — events were flat parent-less objects: `new MouseEvent() instanceof UIEvent` was false and `.view`/`.detail` `undefined`; `UIEvent`/`WheelEvent`/`CompositionEvent` did not exist. Now `defEvent(name, defaults, parent)` merges inherited defaults + chains prototypes; hierarchy `Event → UIEvent → MouseEvent → WheelEvent`. **whole dom 2975 → 3016 (+41)**, gate `g_event_constructors` |
| **`new UIEvent('x', {view: 7})` throws TypeError** | WebIDL `Window?` coercion correctness | ✅ (tick 121) — a supplied non-null non-object `view` is rejected |

## Tick 139 — mixed `calc()` resolves in the flex/grid layout path (sidebar-splits stop collapsing to 0)

| Pattern | Reach | Status |
|---|---|---|
| **`width: calc(100% − 250px)` on a flex/grid item/container** | **every dashboard, docs site and app shell** — the fixed-gutter sidebar split (`calc(100% − <fixed>)` main beside a fixed rail, or `calc(<fixed> + 100%)`) is one of the most common layout idioms on the modern web | ✅ (tick 139) — the block path already resolved calc via `Dim::resolve`, but the taffy flex/grid mapping **collapsed a mixed calc to a single term** (`Dim::Calc{px,pct}` → `length(px)` OR `percent(pct)`), so `calc(100% − 250px)` became `−250px` → clamped to **0** and the sidebar vanished. Now the two terms are packed into taffy's `calc()` handle and resolved as `px + pct% · basis` against the definite basis at layout time — the same linear form the block path uses, so flex/grid items agree with block ones. Falsifiable unit + full-pipeline layout tests (750px sidebar in a 1000px row); WPT-neutral (the css-sizing/flexbox calc tests are reftests or also need intrinsic sizing), a daily-driver render fix rather than a flip-count move |

## Tick 138 — `offsetLeft`/`offsetTop` are offsetParent-relative, and `offsetParent` exists (CSS layout: +665 flexbox, +107 grid)

| Pattern | Reach | Status |
|---|---|---|
| **`el.offsetLeft`/`offsetTop` measured from the offsetParent's padding edge**, not the viewport | **the whole CSS layout web** — every `check-layout-th.js` WPT suite (flexbox/grid/sizing/position) asserts these against container-relative offsets; and every popup/tooltip/dropdown/drag library positions at `el.offsetLeft` and landed in the wrong place when it was absolute | ✅ (tick 138) — the values returned the absolute page X/Y (`LAYOUT_RECTS[node]`), correct only when the offsetParent is at the origin. Now `self.borderEdge − (offsetParent.borderBoxEdge + offsetParent.borderWidth)`. **css-flexbox 6.2%→24.7% (+665), css-grid 5.3%→9.0% (+107), css-sizing 12.0%→13.6%, css-position +5**; Bar 0 clean; one coordinate-space fix flips all four shared-harness suites |
| **`el.offsetParent`** returns the nearest positioned ancestor / body / table cell, else `null` | every layout-measuring library (positioning engines, virtualisation, `getComputedStyle`-free measurement) that walks `offsetParent` to sum offsets to the page | ✅ (tick 138) — the property did not exist (`undefined`); now CSSOM-View `offsetParent`: `null` for root/body/`fixed`/boxless, else nearest positioned ancestor, body, or (element-static only) `td`/`th`/`table`. Gate `g_offset_parent` |
| `offsetParent` reflector identity (`el.offsetParent === container`) and transform-aware offset geometry | frameworks that compare the returned node by identity; transformed containers | ✅ identity via the shared `return_node_or_null` reflector path; ⚠️ offsets are pre-transform (same honest bound as `getBoundingClientRect`/`elementFromPoint`) — a follow-on |

## Tick 137 — selector identifiers decode CSS escapes (+40)

| Pattern | Reach | Status |
|---|---|---|
| **`querySelector`/`matches` decode escapes in id/class/pseudo idents** (`#has\.dot`, `#\30 start`, `#a\:b`) | `CSS.escape` output, jQuery, any framework querying by an id/class that contains CSS syntax chars (`.`, `:`, digits-leading), and the cascade matching such selectors in stylesheets | ✅ (tick 137) — `take_ident` stopped at `\`, so every escaped selector matched nothing; now css-syntax §4.3.7 "consume an escaped code point" (1–6 hex + optional trailing space → code point; else literal), plus an escape-aware pre-tokenizer so `#\30 x` is one compound, plus raw non-ASCII (U+0080+) accepted as ident chars. **dom/nodes 3245 → 3285 (+40)**, css/selectors held at 784, gate `selector_ident_escapes_decode_per_css_syntax` |
| a **surrogate-half escape** (`\d83d`) resolves to U+FFFD and round-trips through an attribute | non-BMP/surrogate ids | ❌ **named limitation** — the escape is DROPPED (not U+FFFD'd) because attribute values are stored UTF-8; emitting U+FFFD would false-match a lossily-stored lone-surrogate id. Gated on WTF-8/UTF-16 attribute storage (same subsystem as CharacterData surrogate splitting) |

## Tick 136 — CharacterData offsets are `unsigned long` = ToUint32, not clamp-to-0 (+33)

| Pattern | Reach | Status |
|---|---|---|
| **`substringData`/`insertData`/`deleteData`/`replaceData`/`substringData` coerce offset & count as WebIDL `unsigned long` (ToUint32)** | every `contenteditable`/rich-text surface, every incremental-text framework, and the DOM's own `normalize`/Range machinery — all specified in terms of these ordinal edits | ✅ (tick 136) — `arg_u32` did `to_int32().max(0)` (clamp negatives to 0), silently turning every out-of-range/negative call into an in-bounds no-op. Now ToUint32: `-1` → 4294967295 (so `deleteData(-1,10)` is `IndexSizeError`), a large negative wraps in bounds (`insertData(-0x100000000+2,"X")` → `"teXst"`), a giant count clamps to remaining length. **dom/nodes 3212 → 3245 (+33)**, gate `g_chardata` (extended) |
| **required CharacterData arguments are a `TypeError` before any DOM step** | WebIDL "not enough arguments" — `node.appendData()` / `node.substringData()` throw, not silently default | ✅ (tick 136) — `argc < N` guard |
| **`node.data = null` is `""`** (`[LegacyNullToEmptyString] DOMString`) | frameworks that clear a text node with `data = null` | ✅ (tick 136) — was the literal `"null"`; `= undefined` still stringifies to `"undefined"`, only *null* is special |
| CharacterData ops across a **surrogate pair** preserve the lone surrogate | non-BMP text (emoji, some CJK) edited at a mid-pair offset | ❌ **follow-on** — the DOM stores `data` as UTF-8 Rust `String` (cannot hold a lone surrogate; `from_utf16_lossy` → U+FFFD); needs WTF-8/UTF-16 storage + `JS_NewUCStringCopyN` return — a subsystem |

## Tick 135 — `createDocumentType` DOCTYPE-name validity + per-document `.implementation` (+190)

| Pattern | Reach | Status |
|---|---|---|
| **`document.implementation.createDocumentType(name, publicId, systemId)`** validates a *doctype name* (not a QName) and returns a real `DocumentType` | XML/XSLT tooling, DOCTYPE-emitting serializers, sanitizers that rebuild a doctype, and every `dom/nodes` test that mints a doctype to test something else | ✅ (tick 135) — the only rule is now the spec's `#valid-doctype-name` (reject only ASCII whitespace / U+0000 / `>`); the old QName check wrongly threw for `1foo`/`@foo`/`prefix::local`/`:foo`/`foo:`/``. `publicId`/`systemId` carried through; `nodeType` 10; `instanceof DocumentType` |
| **every document exposes its OWN `.implementation`**, bound to itself | any code that creates a second document and calls `createdDoc.implementation.*` — DOMPurify chains, DOMParser round-trips, off-DOM builders; WPT asserts the created doctype's `ownerDocument === createdDoc` | ✅ (tick 135) — `.implementation` moved from a global singleton (closed over the top-level `document`) to a `Document.prototype` getter + `__makeImpl(ownerDoc)` factory, cached per-document. Created docs had **no `.implementation`** before (a `TypeError` aborting the whole test file). **whole dom 3632 → 3822 (+190)**, gate `g_dom_impl` (extended) |
| `createDocument(namespace, qualifiedName, doctype)` returns a proper **XMLDocument** (namespaced root, lowercase tags, `application/xhtml+xml`) | XML/XHTML tooling | ❌ **follow-on** — still returns an HTML document ignoring its args; a separate bounded tick |

## Tick 134 — a document from `DOMImplementation` is a REAL Document (+20)

| Pattern | Reach | Status |
|---|---|---|
| **`document.implementation.createHTMLDocument()` returns a usable Document** — the factory surface (`createElement`/`createTextNode`/`createComment`/`getElementById`/…) resolves on it | **DOMPurify and every sanitizer** parse hostile markup into a detached document; template engines and off-DOM builders; every `dom/nodes` test that mints a second document to test something else | ✅ (tick 134) — the reflector now carries `Document.prototype` (mirroring the iframe path) instead of `HTMLElement.prototype`; was `TypeError: doc.createElement is not a function` |
| **a second document in the same arena resolves its OWN structure** — `documentElement`/`body`/`head`/`title` are subtree-scoped, not arena-root-wide | correctness AND safety: without it a created `doc.body` aliased the MAIN page's body, so a write corrupted the real document (and the WPT harness that lives in it) | ✅ (tick 134) — new `Dom::find_first_in(root, name)`; the getters scope to the `this` document node. This was the exact blocker the prior "stated limit" comment described |
| **`createHTMLDocument()` structure + metadata** — `[doctype, html]` children, `instanceof Document/HTMLHtmlElement/HTMLHeadElement/HTMLBodyElement`, `compatMode`/`contentType` | the shape sanitizers and serializers assume of a fresh document | ✅ (tick 134) — doctype child added; `instanceof Document` matches nodeType 9 (was singleton-only); structural element ifaces + `CSS1Compat`/`text/html` constants. Gate `g_created_document_is_real` |
| documents from `new DOMParser().parseFromString(...)` and XML `createDocument`; `createAttribute`/`createCDATASection`/`adoptNode` on any document | XML/XSLT tooling, DOMParser round-trips | ❌ **follow-on** — same "Document.prototype at the mint site" mechanism, different creation paths; and three factory methods absent on ALL documents. Each a separate bounded flip |

## Tick 120 — `document.createProcessingInstruction` (a whole missing node type) (+43)

| Pattern | Reach | Status |
|---|---|---|
| **`document.createProcessingInstruction(target, data)`** returns a real `ProcessingInstruction` node | XML/XSLT tooling, `<?xml-stylesheet?>` handling, sanitizers/serializers that must round-trip PIs, any DOM code that walks mixed-content trees; and — the real yield — every `dom/nodes` test that *creates a PI to test something else* | ✅ (tick 120) — was `undefined` (a `TypeError` that threw before the test's first assertion). Now a `NodeData::ProcessingInstruction { target, data }` node: `nodeType` 7, `nodeName`/`.target` = target, `.data`/`nodeValue`/`textContent` = data (CharacterData), HTML-serializes to `<?target data>`. **whole dom 2932 → 2975 (+43)** |
| **pre-mint validity** — `InvalidCharacterError` on a non-`Name` target or `?>`-containing data; a colon is a valid `Name` | spec-correctness the WHATWG "create a PI" steps require | ✅ (tick 120) — gated by `g_processing_instruction` |
| **`nodeValue` is the data for a Comment AND a PI**, not just Text | correctness — every DOM-diffing lib reading `nodeValue` on non-text CharacterData | ✅ (tick 120) — latent bug: the getter knew only Text; now routed through `character_data` (Text/Comment/PI) |

## Tick 119 — `Node.prototype.moveBefore` (the atomic move) (+18)

| Pattern | Reach | Status |
|---|---|---|
| **`parent.moveBefore(node, child)`** relocates a connected node without the remove+insert side effects | framework reconcilers (React/Preact/lit) preserving state — iframe not reloaded, animation/transition not restarted, focus/selection kept — during DOM re-order; feature-detected and called | ✅ (tick 119) — was `undefined` (a `TypeError`); now a native on the flat `Node.prototype` beside `insertBefore`, so Element + Document + DocumentFragment get it. Relocation reuses `insert_before`/`append_child` (both detach first). **dom/nodes/moveBefore 3/106 → 21/106; whole dom +18** |
| **pre-move validity throws** — TypeError (non-Node/missing arg), HierarchyRequestError (disconnected either side, cross-document, ancestor/cycle, wrong kind), NotFoundError (bad reference child) | the branches real move-code takes on failure | ✅ (tick 119) — the stricter "both connected + same root" rule that distinguishes an atomic move from `insertBefore`; gated by `g_move_before` |
| a plain `{a:1}` is no longer mistaken for a Node | correctness/safety of every native that coerces a Node arg | ✅ (tick 119) — `node_and_dom`'s blind `SLOT_NODE` read (slot 0 of `{a:1}` holds `1`, aliasing the node slot) is now gated by `is_node_reflector` (a `NODE_CLASS` class check) |
| **`getComputedStyle(el).<flexProp>`** resolves the flexbox longhands (`alignItems`/`justifyContent`/`flexDirection`/`flexWrap`/`flexGrow`/`flexShrink`/`flexBasis`/`alignSelf`/`rowGap`/`columnGap`) | every framework/CSS-in-JS/animation lib that reads back a flex container's resolved style to measure or interpolate it | ✅ (tick 142) — these read `undefined` before (garbage concatenated into layout logic); `ComputedStyle` already stored them, so this is pure serialization wiring. Chrome's exact resolved keyword; `getPropertyValue` kebab lookup too. **css-flexbox 888→945 (+57), css-grid 150→257 (+107) — one fix, both suites; gated by `js_conformance` scenario 23** |
| **`getComputedStyle(el).boxSizing` / `.minWidth` / `.maxWidth` / `.minHeight` / `.maxHeight`** read back the box-model longhands | framework layout-measurement code that branches on border-box vs content-box and on min/max fit constraints | ✅ (tick 143) — `undefined` before; stored+computed on `ComputedStyle`, pure serialization. `max-*` unset resolves to `none` not `auto` (the `Dim::Auto`→"none" rule). +4 css-flexbox measured; bulk is `css/cssom` (absent locally), so gated by `js_conformance` scenario 24 |
| **`position:absolute; inset:0` with a `height:100%` child** — the overlay / modal / backdrop *fill* pattern | every full-bleed overlay, lightbox, dialog backdrop, sticky media layer that fills a positioned ancestor and stacks a percentage-height inner layer on top | ✅ (tick 144) — an abspos box with both insets set is a **definite** height (constraint equation: CB-height − insets), but manuk laid its children out *before* computing it, so the `height:100%` child saw an indefinite base and **collapsed to 0** (the overlay's contents vanished). `layout_abs` now threads the definite content height down as the percentage base. `css/css-sizing` +2 (`abspos-intrinsic-height-inset-percentage-child` auto/stretch cases); gated by `abspos_inset_zero_gives_percentage_height_child_a_definite_base` (RED→child 0, GREEN→child 200) |
| **CSS `aspect-ratio` property** (`aspect-ratio: 16/9` on a non-replaced box) | every media container, card, image/video placeholder, hero and embed wrapper that reserves a ratio-shaped box before content arrives (prevents layout shift) | ✅ (tick 145) — the property was **never mapped from the cascade** (`s.aspect_ratio` was set only from a decoded image's pixels), so `aspect-ratio: N/M` reached layout as `None` and the ratio transfer (in-flow *and* abspos) never fired. `stylo_map.rs` now maps stylo's `AspectRatio.ratio`; the hand parser learns it too (parity); `layout_abs` gains a box-sizing-aware transfer + border-box own-size handling. `css/css-sizing` 229→**240 (+11)**; gated by `aspect_ratio_parses_to_a_width_over_height_ratio` (css) + `abspos_aspect_ratio_transfers_definite_height_to_auto_width` (layout). Residue: static-position (inset-less) abspos still records no geometry — a separate mechanism |
| **`position:relative` with a percentage `top`/`bottom`** (`position:relative; top:50%`) | vertical nudging / centering idioms, tooltips and badges offset by a percentage of their box, any relative box positioned as a fraction of its container's height | ✅ (tick 147) — `layout_block` resolved the *horizontal* relative offset against the containing-block width (correct) but the **vertical** one against a hardcoded `0.0`, so `top:50%` computed `50% of 0 = 0` and the box **never moved vertically**. The definite containing-block height was already threaded in as `pch` (tick 144's percentage-sizing base); resolving the vertical delta against `pch.unwrap_or(0.0)` fixes it (and `None` → 0 matches the spec's "computes to auto" for an auto-height CB). `css/css-position` 69→**75 (+6)**, `css/css-flexbox` +4 (relative flex items); gated by `relative_percentage_top_resolves_against_containing_block_height` (layout) |
| **Intrinsic-keyword `height`** (`height: fit-content` / `min-content` / `max-content`) on an `inset:0` absolutely-positioned box | dropdowns, popovers, tooltips, auto-sized panels and cards pinned with `inset:0` that must *hug their content* rather than fill the positioned ancestor | ✅ (tick 146) — stylo parses these keywords into distinct `Size` variants but `size_to_dim` collapsed them (and `auto`/`stretch`) all to `Dim::Auto`, so an intrinsic-keyword height was indistinguishable from `auto` and tick 144's "auto + both insets ⇒ definite" rule wrongly **stretched the box to the containing block** (200) instead of sizing to content (80). New `ComputedStyle::height_intrinsic` (set by stylo map + hand parser) marks the height as **indefinite**; `layout_abs` skips the constraint-equation for it, so the box hugs content and its `%`-height child sees an indefinite base → auto. `css/css-sizing` 240→**243 (+3)**; gated by `intrinsic_height_keywords_flag_the_box_as_indefinite` (css) + `abspos_intrinsic_height_with_inset_zero_sizes_to_content_not_stretch` (layout, which also guards tick 144's `auto`/`stretch`-stretches-to-200 behaviour) |
| **root `height:100%` chain** (`html,body{height:100%}` → `#app{height:100%}`) and **`max-height:%` against an auto-height parent** | every SPA app-shell whose scroll pane must fill the window; every `img{max-width:100%;max-height:100%}` responsive-image reset | ✅ (tick 150) — `layout_document` seeded the root box with `pch:None`, so a root `height:100%` was indefinite and collapsed to content height while a `100vh` sibling filled the window; now seeded with the viewport height (the ICB, CSS2 §10.1) read from the same source `vh` uses. And `max-height:%` resolved against `pch.unwrap_or(0.0)` → `0` on an indefinite parent, clamping the box to nothing; now treated as `none` (CSS2 §10.7). `css/CSS2/normal-flow` 17→**18**; gated by `root_percentage_height_fills_the_viewport` + `percentage_max_height_indefinite_parent_is_none` (layout) |
| **parent↔child margin collapsing** (`<div class=card><h2>…</h2></div>` — a heading/first-block margin, and the symmetric last-block bottom margin) | every content page's vertical rhythm: a first/last child's margin must **escape** a border/padding-less, `overflow:visible`, auto-height parent, not sit inside it as a gap — the single most common vertical-layout difference from Chrome on ordinary document pages | ✅ (tick 151) — CSS2 §8.3.1's last unmodeled case (`layout_block` did adjacent-sibling collapse only). A left/right-spine peek (`collapse_through_top`/`collapse_through_bottom`) folds the first/last in-flow block child's collapse-through margin into the box's own edge margin (top: box raised + child placed flush; bottom: trailing margin removed from content height + collapsed into `margin_bottom`). Eligibility (`display:block`, `overflow:visible`, no BFC, no border/padding on that edge; bottom also auto-height; clearance on the first child declines). ⚠ **the "out-of-flow first-child declines" clause was WRONG and is superseded at tick 859** — an out-of-flow child is skipped, not a terminator. `overflow:hidden` correctly still contains. css-flexbox 26.5→26.6%, css-sizing 14.5→14.8%, position/overflow/normal-flow flat, HANG/CRASH 0; gated by `parent_child_top_margin_collapses` + `parent_child_bottom_margin_collapses` (RED on disabling eligibility) and the guards `overflow_hidden_contains_child_margin` + `top_border_blocks_margin_collapse` |
| **`overflow:hidden`/`auto`/`scroll` contains floats** (the modern clearfix; and a BFC does not wrap an outer float) | every card/row/media-object that uses `overflow:hidden` to enclose floated children, and every sidebar layout that must not let body text wrap under a floated column it shouldn't — one of the most common float idioms on the document web | ✅ (tick 152) — `establishes_bfc` ignored `overflow` ("not modeled yet"), so a floated child escaped its `overflow:hidden` parent (probe: 60px float in an 18px-tall box) and content overlapped outer floats. Now `overflow != visible` establishes a BFC (CSS2 §9.4.1/§10.6.7): own float context + grows to contain its floats via the existing `own_bfc.lowest_bottom()` path. Composes cleanly with t151 margin-collapse (both gate on overflow:visible). Gated by `overflow_hidden_contains_floats` (RED→18px); sweep flat, HANG/CRASH 0 |
| **`width: fit-content` / `max-content` / `min-content`** on a block-level box | the "hug the contents" idiom everywhere — a `fit-content` badge/tag/pill/chip, a `max-content` single-line label or nav item, and the `width:fit-content; margin-inline:auto` centered-block-that-hugs pattern used for centered headings, buttons and callouts | ✅ (tick 153) — all three keywords collapsed to `Dim::Auto` and took the block auto-width **fill** branch, so a `fit-content` badge stretched edge-to-edge (probe: 300px where Chrome hugs at 14px). New `IntrinsicSize` enum on `ComputedStyle::width_keyword` (set by stylo map + hand parser at parity) routes the auto branch to `min_content_width`/`max_content_width`/`shrink_to_fit` — the same measures inline-block already uses (content-box result, so the box-sizing subtraction stays skipped); min/max-width clamps and `margin:auto` centering both still apply. `css/css-sizing` 14.8%→**15.1% (+5)**; css-flexbox/grid/position + CSS2 normal-flow flat, HANG/CRASH 0; gated by `width_fit_content_hugs`, `width_max_content_hugs`, `width_min_content_is_longest_word`, `width_fit_content_still_clamped_by_max_width` (first three RED at the 300/1000px fill) |
| **`height: stretch` / `-webkit-fill-available`** on a block | full-height panels, columns and app-shell regions that must fill a definite-height parent — the `-webkit-fill-available` mobile-viewport idiom, and any "this pane fills the card" layout not built on flexbox | ✅ (tick 154) — these collapsed to `Dim::Auto` and (unlike width, where auto already fills) took the block **content-height** path, so a full-height panel came out one line tall (probe: 18px in a 200px parent). New `ComputedStyle::height_stretch` (stylo map + hand parser) makes `own_definite_h` fill the parent's definite content height `pch` minus this box's own margins/border/padding; a stretched box is then a definite-height CB for its `%`-height children. **`css/css-sizing` 253→341 (+88)** — the `stretch/block-height-*` mass; css-flexbox +1, grid/position/normal-flow flat, HANG/CRASH 0; gated by `height_stretch_fills_definite_parent` (RED→18px) + 3 more. Residue: `width:stretch` in a shrink-to-fit context (float/inline-block/abspos) still behaves as `auto` |
| **`overflow-y: scroll` reserves a scrollbar gutter** (the `html{overflow-y:scroll}` layout-shift-prevention idiom; any always-scrollable pane) | every site that forces a persistent scrollbar so navigating between a short and a tall page causes no horizontal shift, and every `overflow-y:scroll` scroll pane whose inner content must sit inside the reserved scrollbar space | ✅ (tick 155) — a classic vertical scrollbar eats inline-end width, but layout laid children across the box's FULL content width, so content was ~15px too wide and centered layouts sat off-centre by half a scrollbar. `ComputedStyle` collapsed `overflow-x/y` into one field, losing that the vertical axis force-shows a scrollbar; new per-axis `overflow_x`/`overflow_y` (stylo map + hand parser, incl. the `overflow: <x> <y>` shorthand) restore it. `layout_block` reserves `SCROLLBAR_WIDTH` (15px) of content width when `overflow_y == Scroll` — narrowing the children's content box and BFC float band while leaving `offsetWidth`/`border_box_w` untouched (200px container, 185px `width:100%` child). Applies to block + taffy flex/grid leaf items alike. `css/css-overflow` 131→**132 (+1)**; css-sizing/flexbox/grid/position all flat (stash-rebuild-measured), HANG/CRASH 0; gated by `overflow_y_scroll_reserves_inline_gutter` (RED→child 200) + controls. Residue: `scrollbar-gutter:stable` (stylo 0.19 has no support), the auto-and-overflows two-pass case, RTL/vertical-writing-mode placement |
| **`position:absolute; inset:0; margin:auto`** with a definite `width`/`height` centers over its containing block | the canonical centered-modal / dialog / lightbox / backdrop idiom — every overlay that pins to all four edges and uses auto margins to sit dead-centre of a positioned ancestor | ✅ (tick 156) — `layout_abs` resolved auto margins to **0** (`Dim::resolve(cw, 0.0)`), so the box pinned to the top-left corner (probe: [0 0] where Chrome centers at [100 100]). CSS2 §10.3.7/§10.6.4: on a **fully-constrained** axis (both insets + a definite size) the free space distributes into the auto margins; that step was missing. Now redistributed per axis after the border box is known — both auto → `free/2` each, a start margin auto → `free − end`, an end-only/neither auto → no-op (the box is already pinned by `inset`+start-margin). The `!= Auto` guard excludes both the stretch-to-fill case and an intrinsic keyword. `css/css-position` 76→**79 (+3)** ("margin:auto on abspos after dynamic inset change" flips); css-flexbox/grid/sizing/values/overflow flat, HANG/CRASH 0; gated by `abspos_auto_margins_center_a_constrained_box` (RED at [0 0]). Residue: the `margin:0 auto` sibling needs **dynamic reflow** on `.style.inset` mutation (a separate mechanism), and writing-mode-aware start-edge selection |
| **`min-width`/`max-width`/`min-height`/`max-height` on an abspos box** actually clamp its used size | every `max-width` dialog/modal, `min-width` tooltip/menu, `max-height` scroll panel or dropdown pinned with `position:absolute`/`fixed` — the size caps that keep an overlay from overflowing the viewport | ✅ (tick 157) — `layout_abs` computed a used width/height and **never clamped** it (the four `min/max-*` fields were dead on the abspos path), so a `width:500px; max-width:200px` box stayed 500. Now mirrors the in-flow block clamp on both axes (max first then min wins, content-box via box-sizing deltas; width clamped before children lay out; height against the always-definite CB height). `css/css-position` **79→88 (+9)**; css-flexbox/grid/sizing/values/overflow flat, HANG/CRASH 0; gated by `abspos_min_max_size_clamps_apply` (RED unclamped). Residue: the 30 `position-absolute-replaced-minmax` **iframe** rows still need replaced-element intrinsic sizing (300×150 default before the clamp table) — a separate mechanism |
| **`overflow-x: scroll` reserves a horizontal-scrollbar gutter** (block-axis mirror of the tick-155 gutter; any always-horizontally-scrollable pane with a fixed height) | code viewers, wide-table wrappers, timeline/carousel strips and any `overflow-x:scroll` pane of definite height whose inner content must sit above the reserved scrollbar strip | ✅ (tick 158) — tick 155 reserved the vertical scrollbar's inline width but left the block axis, so an `overflow-x:scroll` pane's horizontal scrollbar (block-end edge) ate no space and a `height:100%` child overran into it (15px too tall). New `gutter_x = SCROLLBAR_WIDTH` when `overflow_x == Scroll`, subtracted from the definite content height offered to children (`inner_definite_h`) — ONLY when height is definite (an auto-height box grows instead, so reserving would wrongly shrink a `height:100%` track). `border_box_h`/`offsetHeight` left untouched, exactly as the inline case leaves `border_box_w`. `css/css-overflow` 132→**136 (+4)**; css-position/sizing/flexbox/grid/values/display all flat (stash-rebuild-measured), HANG/CRASH 0; gated by `overflow_x_scroll_reserves_block_gutter_only_when_height_definite` (RED→child 200) + auto-height control. Residue: the `overflow-x:auto`-and-actually-overflows two-pass case, RTL/vertical-writing-mode placement |
| **`fetch(...).then(r => r.headers.get('content-type'))` and `xhr.getResponseHeader(...)`** read the server's real response headers | every SPA/data-layer that branches on `Content-Type` before parsing, follows `Link`-header pagination, reads `X-RateLimit-*` to pace requests, or uses `ETag`/`Last-Modified` for conditional re-fetch — the read-side of the HTTP contract, which was entirely invisible to page JS | ✅ (tick 171) — the JS `Response` was built with `headers: { get: () => null, has: () => false }` and XHR's `getResponseHeader`/`getAllResponseHeaders` were `null`/`""`, so the server's headers **never reached the page** (read-side twin of tick 148's dropped *request* headers). Now the real `Vec<(String,String)>` from `manuk_net::request` threads through both fetch pumps → `Page::resolve_fetch(id,status,body,headers,…)` → `event_loop::deliver` → `__makeResponse`, which builds a Fetch-standard `Headers`: `get`/`has` match names **case-insensitively** and `get` comma-joins repeats, `getAllResponseHeaders()` emits lower-cased `name: value\r\n` lines, an absent header is `null` not `""`. Additive (an empty slice → `get` returns null, so the mock-fetcher loop and all prior callers are unchanged). Gated by `js_conformance` scenarios (5) fetch + (6) XHR. Residue: `Access-Control-Expose-Headers` per-header safelist (same-origin exposes the full list, correct; cross-origin bodies are already blocked wholesale by the tick-170 CORS barrier), and `response.body`/ReadableStream still `null` |
| **`fetch(url, {signal: controller.signal})` + `controller.abort()`** actually cancels the request | every React `useEffect` data-fetch cleanup (`return () => c.abort()`), React-18 StrictMode double-mount cancellation, search-as-you-type debounce that aborts the stale request, and any request library (axios/ky/SWR/react-query) that wires an AbortSignal — the universal modern cancellation idiom | ✅ (tick 172) — `AbortController`/`AbortSignal` existed but `fetch` **ignored `opts.signal`**, so `abort()` was a no-op: the request still ran, and on unmount the resolved `.then` set state on a dead component (the classic StrictMode race). Now `fetch` honours the signal — a **pre-aborted** signal rejects synchronously and queues no request; an **in-flight** abort rejects with `signal.reason` and drops `__fetchCb[id]` so a late host delivery can't resolve it; unchanged when no signal. Reject reason is a `DOMException` named **`AbortError`** (`err.name === 'AbortError'`, which libs check to tell a cancel from a failure) — the abort default was `new Error('AbortError')` (`.name === 'Error'`) and is now a real DOMException. Gated by `js_conformance` scenario (25): pre-abort queues nothing, in-flight late-delivery yields `AbortError` not the body. Residue: `XMLHttpRequest.abort()` still a no-op; `AbortSignal.timeout()` doesn't yet reject an in-flight fetch |
| **`fetch(url, {body: formData})` / `xhr.send(formData)` with a File uploads the file** as `multipart/form-data` | every file upload on the web — avatar/profile-photo pickers, attachment fields, document/CSV import, drag-drop uploaders, and any `new FormData(form)` submit where the form has an `<input type=file>` | ✅ (tick 174) — a FormData body was `String(fd)` = **urlencoded**, turning a File part into the literal `"[object File]"`, so the upload silently sent a placeholder and no file. Now `fetch`/`XHR.send` encode a FormData body as `multipart/form-data`: `FormData.prototype.__multipart(boundary)` emits each field as a part and each Blob/File (detected by `__blobText`) with `Content-Disposition: …; filename="…"` + its `Content-Type` + content; the browser generates the boundary and sets/overrides `Content-Type: multipart/form-data; boundary=…` (only the browser knows the boundary). `toString()` stays urlencoded for `new URLSearchParams(fd)`. Gated by `js_conformance` scenario (26): the request body carries the field, the filename, and the file's real content between boundaries. Residue: File content is a JS string (no byte-accurate binary body path yet); native `<form enctype=multipart>` submit is a separate path |
| **Typing into a controlled `<input>`/`<textarea>`** fires `input`, so React `onChange` / Vue `v-model` / Svelte `bind:value` update state | every SPA form field — search boxes, login/signup, checkout, comment composers, settings, filters — i.e. essentially all text entry on the modern web | ✅ (tick 175) — the shell's `edit_focused_input` mutated the `value` attribute directly and fired NOTHING, so a controlled component never saw the keystroke: it re-rendered from stale state and **reverted the character**, making every framework text field unusable. New `Page::dispatch_input(node, value)` sets the value and fires `input` (only — `change` is a commit/blur event, wrong per keystroke), and the shell calls it per keystroke. The existing `dispatch_type` (input+change) had zero callers — a mechanism wired to nothing. Gated by `js_conformance` scenario 27: an `input` listener reads `event.target.value` (`hi`→`hip`), and the `change` counter stays 0. Residue: `change`-on-blur, `keydown`/`keyup`/`beforeinput` still unfired |
| **Leaving a form field fires `change`+`blur`** so on-blur/on-change validation runs (email/username/password checks, the red-border-on-blur pattern) | every signup/login/checkout/settings form with per-field validation — i.e. essentially all forms that give feedback before submit | ✅ (tick 176) — the shell cleared focus and fired nothing, so field-level validation never ran and the field never committed. New `Page::dispatch_blur(node, value_changed)` fires `change` (only if the value changed since focus — a `focus_value` snapshot guards it, so tabbing through fires no spurious change) then `blur`. `blur_focused_input()` is the chokepoint for every user focus-loss (click-away, focusing another field, Escape, Enter-before-submit). Commit half of tick 175's per-keystroke `input`. Gated by `js_conformance` scenario 28 (no-change blur → blur only; changed → change then blur). Residue: programmatic `.focus()` doesn't blur the old field; `focus`/`focusin`/`focusout`, `keydown`/`keyup` separate |
| **`xhr.abort()`** actually cancels — a late response does not fire `onload` | search-as-you-type / autocomplete that aborts the stale request per keystroke, any jQuery.ajax / request-library cancel path, upload cancel buttons on the XHR path | ✅ (tick 177) — `abort()` was a no-op, so a cancelled XHR still applied its response when it arrived (the stale-result race: old result clobbers new). Now abort drops the pending callback (a late `__deliverXhr` for that id no-ops — the XHR twin of tick 172's fetch drop) and fires `readystatechange`→`abort`→`loadend` (XHR standard order), leaving readyState UNSENT. Gated by `js_conformance` scenario 29 (aborted XHR, then late delivery → onload NEVER fires, abort+loadend do). Residue: AbortSignal-on-XHR not wired |
| **`onKeyDown` + `preventDefault()`** — a page intercepts a key before the browser's default | chat/comment composers (Enter sends, Shift+Enter newlines), command palettes, comboboxes/listboxes (arrow-key highlight), any "press Enter to…" that isn't a form submit, hotkey libraries | ✅ (tick 178) — the shell went straight from keypress to its default (submit/edit/blur) firing NO keydown, so a page could never pre-empt a key. New `Page::dispatch_key(node, "keydown", key)` fires a real KeyboardEvent carrying `key` (modern) + `keyCode`/`which` (legacy) and returns whether the default proceeds; the shell fires it on the focused field first and stops if `preventDefault()` was called (Enter no longer submits, the char isn't inserted). `__dispatchEvent` already accepted an event object, so the KeyboardEvent shape was free. Gated by `js_conformance` scenario 30 (`event.key`/`keyCode` correct; Enter preventDefault → dispatch returns false). Residue: `keyup` not yet fired; `event.code` approximate for characters |
| **A "copy" button** — `navigator.clipboard.writeText(text)` puts text on the OS clipboard | code-block copy icons, "copy link"/"copy API key"/"copy coupon" buttons, share widgets — one of the most common single-purpose buttons on the web | ✅ (tick 179) — `navigator.clipboard` was absent, so `writeText` threw on undefined inside the click handler and the button silently did nothing. Now `navigator.clipboard.writeText` queues the text via a native `__clipboardWrite` (the window.open/postMessage host-queue pattern) and returns a resolved Promise; the shell drains it after a click (`pump_clipboard`) and writes to the real OS clipboard (arboard). `readText` resolves with the last text the page wrote (within-page round-trip; OS-clipboard read is a permission-gated follow-on). Gated by `js_conformance` scenario 31 (copy button click → `take_clipboard_writes()` == the text). Residue: OS readText, execCommand('copy'), off-click-path writes |
| **`keyup`** fires on key release — a field sees the key come up | search-as-you-type / autocomplete that debounces on `keyup` (the jQuery-era idiom), character/word counters, keyboard-shortcut *release* logic, any `keyup` handler on a text field | ✅ (tick 180) — the shell fired `keydown`+`input` on key PRESS but processed only `ElementState::Pressed`, dropping every `Released`, so a `keyup` listener never ran and those boxes stayed dead. `Page::dispatch_key` was already generic over the event type, so the fix is pure shell wiring: on release, `dispatch_keyup` fires `keyup` on the focused field via the same `key_name_for_dispatch` mapping (no default action is bound to keyup, so its `preventDefault` return is irrelevant). Completes the trio keydown→input→keyup. Modifier-only releases surface no key name → no spurious keyup. Gated by `js_conformance` scenario 32 (`keyup` reads `event.key`/`keyCode`, `x:88`). Residue: keyup only for a focused field (not document-global); `event.code` inherits keydown's approximation |
| **`object-fit: cover`** — a replaced image fits its box without distorting | the near-universal card-grid thumbnail idiom (`img{width:100%;height:100%;object-fit:cover}`), avatar/profile photos, hero/banner crops, product-tile images, `<video>` posters — essentially every non-icon image on a modern styled page | ✅ (tick 181) — object-fit was **completely unimplemented** (0 hits): the replaced-image blit stretched the decoded bitmap to fill the box, so every non-square photo in a square tile came out squashed to the tile's ratio. Three-crate mechanism: `ObjectFit` enum parsed into `Style::object_fit` (css) and recovered from MinimalCascade on the shipping Stylo path; carried on `LayoutBox` (layout, no layout-math change); `object_fit_geometry(fit,box,iw,ih)` at display-list build (paint) returns the aspect-ratio-preserved destination rect + a crop box — `cover`/`none` scale to cover/natural and clip the overflow (new `DisplayItem::Image.content_clip`, intersected with any ancestor overflow clip), `contain`/`scale-down` fit inside (no clip), `fill` stretches (unchanged); all centered (`object-position:50% 50%`). Gated by `object_fit_preserves_aspect_ratio` (engine/paint): a 200×100 photo in a 100×100 tile → cover dest 200×100 + 100×100 crop, contain dest 100×50, fill 100×100; RED vs the stretch baseline. css+layout+paint suites green, HANG/CRASH 0. Residue: explicit `object-position`; `<video>`/`<canvas>` once they decode; `none` uses raw bitmap px (approximate at DPR≠1) |
| **`text-transform: uppercase`/`capitalize`** renders text in the design's casing without mutating the DOM | nav bars, buttons (`SUBMIT`), section headings, table column headers, tab labels, breadcrumb caps, title-case headings — a large fraction of styled UI text | ✅ (tick 182) — text-transform was **unimplemented** (0 hits): a `text-transform:uppercase` button whose textContent is "Submit" rendered lowercase "Submit", diverging from the design everywhere it's used. New inherited `TextTransform` (None/Uppercase/Lowercase/Capitalize) parsed into `Style::text_transform` (copied in the MinimalCascade inheritance step beside `white_space`, recovered on the shipping Stylo path); `apply_text_transform(raw, cs.text_transform)` in layout's `collect_inline_node` re-cases the RENDERED run (measured at its new width) while leaving the **DOM text untouched** (JS still reads the author's string). Unicode casing honoured (ß→SS); capitalize upper-cases each word's first cased letter. Gated by `text_transform_recases_rendered_text_only` (unit Submit→SUBMIT/HELLO→hello/"hello world"→"Hello World"/straße→STRASSE + E2E inherited-uppercase nav renders HOME, a child `text-transform:none` stays "Keep", `text_content` still "home"); RED vs no-transform baseline. css+layout green (layout 72→73), HANG/CRASH 0. Residue: full-width/full-size-kana, exact capitalize grapheme boundary, letter/word-spacing separate |
| **`overflow-wrap: break-word` / `word-break: break-all`** — a long unbreakable token wraps inside its column instead of overflowing | any place a URL, a commit/tx hash, an API key, an unspaced foreign string or a long email lands in a narrow column: chat/comment threads, code and log viewers, table cells, cards, sidebars, mobile-width layouts — the ubiquitous "don't let one long link blow out the layout" reset (`word-break:break-word` / `overflow-wrap:anywhere` on body copy) | ✅ (tick 183) — char-level intra-word breaking was **unimplemented** (0 hits): `break_segments` only splits at whitespace/UAX-14 opportunities, so a token with none stayed one word and the line-filler let it overflow its column, pushing the layout sideways. New inherited `OverflowWrap` (Normal/BreakWord/Anywhere, parsed from `overflow-wrap` **and** legacy `word-wrap`) + `WordBreak` (Normal/BreakAll/KeepAll) on `ComputedStyle` (copied in the MinimalCascade inheritance step, recovered on the shipping Stylo path); a derived `break_word` flag rides `InlineItem::Word`; `break_overwide_words` (a pre-pass at the head of `layout_inline`, where content width `cw` + font metrics are known) splits any `break_word` word wider than `cw` at char boundaries into chunks each fitting `cw`, emitted as ordinary breakable words so the existing filler wraps them across lines — losslessly, only over-wide words touched, every other word byte-identical so parity/UAX-14 are unmoved. Gated by `overflow_wrap_break_word_wraps_long_token` (60-char token in 100px: normal → one fragment >100px overflows; break-word → >1 fragment each ≤100px + joins back to the token; word-break:break-all same); RED vs the no-char-break baseline. css+layout green (layout 73→74), HANG/CRASH 0. Residue: `break-all` breaking a word that would still fit later in a line, `anywhere`'s smaller min-content contribution, `line-break`/`hyphens` |
| **`letter-spacing` / `word-spacing`** — a tracked run measures and paints at its intended (wider) width | tracked uppercase nav bars, buttons, small-caps labels, kickers/eyebrows, hero headings, table-header caps — the tracking that goes hand-in-hand with `text-transform:uppercase` on a large fraction of styled UI | ✅ (tick 184) — both **unimplemented** (0 hits): a tracked heading measured and painted at its *untracked* width (box too narrow, glyphs too tight). New inherited `ComputedStyle::{letter_spacing,word_spacing}` (px, parsed via `values::parse_length_px`, `normal`→0, recovered on the shipping Stylo path) carried on `TextStyle`; layout adds `letter_spacing × char_count` to each word's width (trailing tracking included, matching Chrome) and `word_spacing` to each inter-word space; paint offsets glyph *i* by `i × letter_spacing` so measure and paint stay in step; `close_line`/`inline_extent` use the stored `f.width` (carries the tracking) instead of re-measuring. **Safety: the default 0 is byte-identical to before — shaping/measure/align/paint all unchanged, so parity/WPT are unmoved and the ratchet cannot regress; only an explicitly-tracked run changes.** Gated by `letter_and_word_spacing_widen_runs` (letter-spacing:4px → +20px on "hello"; word-spacing:10px → second word +10px); RED vs the no-tracking baseline. css+layout+paint green (layout 74→75), HANG/CRASH 0. Residue: word-spacing inside `pre` internal spaces, per-grapheme-cluster tracking for ligatures |
| **`object-position`** — a cropped image shows the intended slice, not always the centre | portrait avatars cropped square (`object-position:top` so the face survives), hero/banner crops that keep their subject in frame (`object-position:right` / `20% 50%`), product tiles — the positioning half of the tick-181 `object-fit:cover` idiom | ✅ (tick 185) — `object-fit:cover`/`none` cropped to the CENTRE only (`object-position:50% 50%` hardcoded), so a subject at the top/side of a cropped image was cut off. New `ObjectPosition {x,y}` (0..1 free-space fractions, default 0.5/0.5) parsed from `object-position` (1–2 keyword/percentage values; `top`/`bottom` bind vertical, `left`/`right` horizontal so `top left` resolves; recovered on the shipping Stylo path), carried on `LayoutBox` beside `object_fit`; paint's `object_fit_geometry` distributes the (negative-for-cover) free space by the fraction — `x = box.x + (bw−dw)·pos.x` — instead of `/2`. **Safety: default 0.5/0.5 reproduces tick 181's centering to the float — every existing image byte-identical, ratchet cannot regress.** Gated by `object_position_places_cropped_image` (2:1 photo in a cover tile: left pins box.x, 50% is −50px, right −100px, `0%`==`left`); RED vs the hardcoded-center baseline. css+layout+paint green (paint 10→11), HANG/CRASH 0. Residue: px-length object-position, 3–4-value edge-offset form |
| **`text-overflow: ellipsis`** — a clipped single-line title/label truncates with `…`, not a hard mid-glyph cut | the ubiquitous one-line-truncation idiom (`white-space:nowrap; overflow:hidden; text-overflow:ellipsis`): card/list titles, nav & tab labels, table cells, file names, chat/message previews, breadcrumbs — nearly every dense UI | ✅ (tick 186) — **unimplemented** (0 hits): a `nowrap; overflow:hidden` title was clipped at the box edge with no ellipsis, cutting a word in half. New `TextOverflow{Clip,Ellipsis}` (css, non-inherited, recovered on the shipping Stylo path); after inline layout of a pure inline-formatting-context block, if it `text-overflow:ellipsis` + clips (`overflow`≠visible) + doesn't wrap (`nowrap`/`pre`) + the single line overflows `cx+cw`, `apply_text_overflow_ellipsis` keeps the fragments before `cutoff = cx+cw−width('…')`, truncates the straddling one (`truncate_to_width`, char boundary), drops the rest, and appends an `…` fragment. **Safety: a fitting line is untouched and `clip` is a no-op — the default path is byte-identical, ratchet cannot regress; only a genuinely-overflowing ellipsis box renders differently.** Gated by `text_overflow_ellipsis_truncates_clipped_line` (80px nowrap title → truncated + `…`, kept text a proper prefix; clip control keeps full text, no `…`); RED vs the no-truncation baseline. css+layout green (layout 75→76), HANG/CRASH 0. Residue: pure-inline path only (mixed block/float lines), `-webkit-line-clamp`, leading-ellipsis value |
| **`text-decoration-color`** — a colored decoration line paints in its own hue, not always the text color | brand/hover underlines, colored link underlines, a strikethrough price in a distinct hue, overline accents — anywhere the underline is meant to contrast with the text | ✅ (tick 187) — paint hardcoded the decoration line to `fade(f.style.color)` and the parser discarded any color token, so `text-decoration-color:red` on blue text drew a *blue* underline. `TextDecoration` gains `color: Option<Rgba>` (`None` == currentColor): the `text-decoration` shorthand takes the first token `parse_color` accepts (skipping line/style keywords), the `text-decoration-color` longhand sets it directly (`currentColor`→None), `text-decoration-line` leaves it intact; recovered wholesale from MinimalCascade on the shipping Stylo path. Paint's line color is now `fade(d.color.unwrap_or(f.style.color))` — default `None` is byte-identical to before. Gated by `text_decoration_color_overrides_text_color` (paint). Residue: `text-decoration-style` (dotted/dashed/wavy/double paint solid), `-thickness`, `text-underline-offset` |
| **`text-decoration-thickness` / `text-underline-offset`** — a decoration line at the design's own weight and position | Tailwind `decoration-2` / `underline-offset-4`, thick brand underlines, links with breathing room under them — pervasive in modern design | ✅ (tick 188) — paint drew the line at a hardcoded thickness (`font_size/14`, so a 14px font always got a 1px hairline) at a fixed underline y, so `decoration-2` drew a hairline and `underline-offset-*` did nothing. `TextDecoration` gains `thickness: Option<f32>` (`None`==auto, font-derived) and `underline_offset: f32` (px below default, default 0); the `text-decoration-thickness`/`text-underline-offset` longhands parse a length (`values::parse_length_px`), the `text-decoration` shorthand resets `thickness` but leaves `underline_offset` (not a shorthand longhand), recovered wholesale on the shipping Stylo path; paint uses `d.thickness.unwrap_or((fs/14).max(1))` and adds `underline_offset` to the underline y. Dropped the struct's `Eq` derive (f32 can't be Eq) — safe, no map keys on it; **no new DisplayItem field so the manuk-wpt TextLine match is untouched.** **Safety: defaults (None/0) are byte-identical to before — ratchet cannot regress; only an explicitly-set thickness/offset changes.** Gated by `text_decoration_thickness_and_offset_shape_the_underline` (paint): 6px thickness paints a 6px line, 8px offset drops the underline exactly 8px; RED vs the hardcoded baseline. css+paint green, HANG/CRASH 0. Residue: text-decoration-style (dotted/dashed/wavy/double paint solid), skip-ink, from-font metrics |
| **`box-shadow` — layered elevation** (`shadow-md`/`shadow-lg`) renders every stacked layer at its own spread | cards, dropdowns, popovers/menus, modals, buttons, toasts, floating action buttons — the whole Material/Tailwind "elevation" vocabulary, on essentially every modern styled surface | ✅ (tick 189) — `box-shadow` was a single `Option<BoxShadow>` with no spread, taking only the first layer, so **every Tailwind elevation** (`shadow`/`shadow-md`/`shadow-lg` are TWO layers, the second with a negative spread) rendered as one flat, wrong-sized shadow. `BoxShadow` gains `spread: f32` + `inset: bool`; `ComputedStyle.box_shadow: Option<_>` → `box_shadows: Vec<_>`; `parse_box_shadows` splits on top-level commas and reads `[inset] dx dy [blur [spread]] [color]` per layer. `stylo_map.rs` maps Stylo's own `clone_box_shadow().0` to the **full** layer list (was `.find(!inset)` → one) with spread+inset in source order — the shipping path, so real pages get every layer; `stylo_engine.rs` falls back to MinimalCascade only when Stylo left the list empty (never overwrites a resolved shadow). `LayoutBox::shadow` → `shadows: Vec` (~12 sites); paint iterates the list in reverse (first layer on top), skips `inset` (inner painting not built — inset-only paints nothing, as before), and inflates each rect by `spread` before offset/blur. **Safety: an empty list == old `None`; a single outer layer with spread 0 is byte-identical (inflate by 0, same offset) — ratchet cannot regress; only a value with a 2nd layer / spread / inset changes.** Gated by `box_shadow_is_a_list_with_spread` (paint): a two-layer shadow emits TWO Shadow items (old: one), `spread:10px` inflates a 100×40 rect to 120×60, inset-only paints nothing; RED vs the single-shadow/no-spread baseline. css+layout+paint green, HANG/CRASH 0. Residue: inset (inner) shadow painting, per-layer blur vs tiny-skia's single-pass gaussian at large radii |
| **`background-image` — layered backgrounds** (a scrim/gradient over a photo) render every layer, not just the image | hero/banner sections with a darkening overlay so white text stays readable, gradient-tinted cards, texture-over-gradient panels — the ubiquitous `linear-gradient(rgba(0,0,0,.5),…), url(hero.jpg)` idiom on essentially every marketing/landing surface | ✅ (tick 190) — `background-image` was a single `Option<BackgroundImage>` and the parser scanned for `url(` **first**, so `background: linear-gradient(...), url(hero.jpg)` returned only the photo and dropped the scrim, leaving white hero text unreadable over a full-brightness image. `ComputedStyle.background_image: Option<_>` → `background_images: Vec<_>` (source order, index 0 = top); `parse_background_images` splits on top-level commas (commas inside `linear-gradient` don't separate layers) and parses each layer via the single-layer `parse_background_image`, dropping only unreadable layers. `stylo_engine.rs` recovers the full list from MinimalCascade (shipping path). `LayoutBox::background_image` → `background_images: Vec` (~10 sites); paint iterates layers in **reverse** after background-color (first layer on top). `page::fetch_and_apply_background_images` takes the first url() layer (one bitmap per node caps url images at one/element; multiple gradient layers over one photo — the common case — fully supported). **Safety: empty list == old `None`; a single-layer list paints byte-identically (same item/order/node-bitmap path) — ratchet cannot regress; only a 2+-layer value changes.** Gated by `background_image_is_a_layer_list` (css): gradient+url parses TWO layers with the gradient at index 0 (old: one, the url), internal commas don't split, `none`→empty; RED vs the single-`Option` baseline. css+layout+paint+page green, HANG/CRASH 0. Residue: one url() image per element (per-node bitmap keying), per-layer background-size/-repeat/-position |
| **`background-position`** — a sprite/logo/positioned hero image lands where the design placed it | CSS sprite sheets (icons/logos rendered by shifting one image: GitHub-style toolbars, older sites), `no-repeat` logos meant to sit centred or bottom-right, positioned hero/texture backgrounds — the positioning half of the `background-size`/`-repeat` idiom | ✅ (tick 191) — `background-position` was **unimplemented** (0 hits): a `url()` background always painted from the box's top-left, so `background-position:-16px -48px` showed the wrong sprite slice and a `no-repeat` logo sat jammed in the corner. New `BackgroundPosition {x,y}` where each axis is a `BgPos` — `Pct(f32)` (fraction of the box's FREE space: `left/top`=0, `center`=0.5, `right/bottom`=1, per CSS percentage/keyword) or `Px(f32)` (absolute length offset), kept distinct until box+tile sizes are known at paint. `parse_background_position` reads 1–2 keyword/percentage/length values (one sets horizontal, vertical→center; keyword axis binding so `top right` resolves); on `ComputedStyle`, recovered from MinimalCascade on the shipping Stylo path beside `object_position`; threaded through `LayoutBox` (~10 sites, `Copy`); `blit_background` shifts the tile origin by `off = Pct(f)·(box−tile) | Px(p)` (`lx = fx−rect.x−off_x`), placing a `no-repeat` image and shifting a `repeat` one's phase. **Safety: default `Pct(0,0)` gives offset 0 — the historic top-left blit is byte-identical, ratchet cannot regress; only a non-default position changes.** Applies to `url()` layers (gradients fill the box). Gated by `background_position_places_the_image` (paint): default `0% 0%`→top-left, `right bottom`→bottom-right, `50px`→slice at [50,70); RED vs the fixed-origin blit. css+layout+paint green (paint 14→15), HANG/CRASH 0. Residue: gradient-layer position, 3–4-value edge-offset form, per-layer positions |
| **`border-style: dashed / dotted / double`** — a broken/paired border renders as intended, not solid | drag-and-drop upload zones ("drop files here"), coupon/ticket cards (perforation), dashed dividers and section separators, empty-state placeholder boxes, `double` frames/blockquotes, some table/input styles | ✅ (tick 192) — `border-style` was **parsed then discarded** (the keyword only defaulted the width; `ComputedStyle` had no `border_style` field), so every dashed/dotted/double border rendered SOLID. New uniform `BorderStyle` (Solid/Dashed/Dotted/Double; groove/ridge/inset/outset→Solid) on `ComputedStyle`, stored uniform like `border_color`. `border_style_of` maps the keyword; `parse_border_shorthand` returns it alongside width/color; `border`/`border-<side>` + `border-style`/`border-<side>-style` longhands set it (`none`/`hidden` still zero width); recovered from MinimalCascade on the shipping Stylo path. `layout::Border` gains `style`; paint's per-edge closure dispatches — Solid=one Rect (byte-identical), Dashed=`3×thickness` dashes+gaps, Dotted=one-thickness dots+gaps, Double=two `⌊thickness/3⌋` lines with a middle gap (<3px reads solid). **Safety: default Solid emits the exact single Rect/edge as before — ratchet cannot regress; only a declared dashed/dotted/double changes.** Gated by `border_style_breaks_the_line` (paint): a plain bordered div emits one Rect/edge, so Rect count separates styles — solid=4, double=8, dashed/dotted≫8; RED vs all-solid. css+layout+paint green (paint 15→16), HANG/CRASH 0. Residue: per-side styles, groove/ridge bevels, exact dash-fit |
| **`text-shadow`** — hero/heading text stays readable over a busy background | white/light headings over a photo or gradient hero (a dark shadow for contrast), raised/engraved button & logo text, drop-shadowed captions, subtle depth on cards/nav — a pervasive readability + polish treatment | ✅ (tick 193) — `text-shadow` was **unimplemented** (0 hits): the painter drew each run once in the text colour, so light-on-image headings lost all contrast and raised/engraved effects did nothing. New `TextShadow {dx,dy,blur,color}` (Copy; like BoxShadow sans spread/inset) on `ComputedStyle.text_shadow: Option<_>`, **inherited** and recovered from MinimalCascade on the shipping Stylo path. `parse_text_shadow` reads the first layer (`offset-x offset-y [blur] [color]`; comma list→first; missing colour→translucent black). Rides `TextStyle` onto every fragment; paint's `draw_text` factors the glyph loop into a run-painter called twice — once at (dx,dy) in the shadow colour BEHIND, once at the origin in the text colour. **Safety: default None skips the shadow pass — every existing text render is the exact single main pass as before, ratchet cannot regress; only authored text-shadow changes.** Gated by `text_shadow_paints_behind_the_glyphs` (paint): white-on-white paints <10 dark px without a shadow, >60 with `text-shadow:4px 4px 0 black`; RED vs no-shadow. css+layout+paint green (paint 16→17), HANG/CRASH 0. Residue: gaussian blur, stacked shadows, currentColor resolution |
| **`<dialog>` + `showModal()`** (the modal: cookie banner, confirm-delete, command palette, the Radix/Headless-UI/shadcn `<Dialog>` primitive; Interop 2026 focus area) | every app-class page that asks the user anything — and the failure was **double**: the modal could not open, AND its contents were already on the page | ✅ (tick 194) — the whole surface was **absent** (0 hits for showModal/popover/::backdrop/top_layer; `dialog` existed only as `{"open":boolean}` in reflect_table.rs). Two independent failures: (1) `dlg.showModal()` was a **TypeError** thrown inside the click handler, taking the rest of the handler with it, so the button did nothing at all; (2) with no UA `display:none` rule a `<dialog>` is an unknown element, so a **CLOSED** dialog's contents were laid out and painted into the page in tree order ("DELETE EVERYTHING?" as a paragraph mid-article) — the same shape as the `<source>`/script-paints-its-own-source bugs. Fixing only (1) yields a browser where the modal opens *and was already there*. Four places: **js prelude** — show/showModal/close(v)/returnValue, the `close` event, InvalidStateError on re-showModal(), `<form method="dialog">` (capture-phase click on the document: closes with the button's value, never reaches the native GET path; `formmethod` overrides), Escape→cancelable `cancel`→dismiss topmost modal, HTMLDialogElement branding; **both cascades in lockstep** (stylo_engine UA_CSS + apply_ua_defaults) — `dialog` hidden, `dialog[open]` a bordered auto-margin block; **page/lib.rs** — `TOP_LAYER_Z` + a modal branch in `z_index_map`, the single choke point paint/hit-test/a11y all read, so a modal outranks every author z-index and its subtree inherits the promotion. Modality crosses the JS↔Rust boundary as `data-manuk-modal` (a JS property is invisible to z_index_map; same device as `data-manuk-adopted`); non-modal `show()` deliberately does not set it. **Safety: additive — the UA rule touches one tag that had no rule at all, the z_index_map branch fires only on `dialog[data-manuk-modal]`, the prelude block is guarded on `typeof __HP.showModal === 'undefined'`.** Gated by `g_dialog` (13 claims, JS surface) + `g_dialog_render` (a closed dialog yields no box and no display item; an open modal paints AFTER a z-index:50 overlay); both proven RED by reverting each half independently (`display:none`→`block` gave the closed dialog a real 18.4px box; `TOP_LAYER_Z`→`z` put the modal behind the overlay). All gates green, wall 59s. Residue: `::backdrop`, inertness + focus trap (the page behind a modal is still clickable), the `popover` attribute API, auto-centering (the modal is a `margin:auto` block IN FLOW — stacking is right, geometry still occupies layout space) |
| **the `popover` API** (`<div popover>` + `showPopover()`/`popovertarget` — menus, tooltips, dropdowns, toasts, select-listboxes; Interop 2026 with `<dialog>`) | every app-class page's navigation and disclosure UI — and, like `<dialog>`, the failure was **double**: the menu could not open, AND its items were already on the page | ✅ (tick 195) — same two-part failure as `<dialog>` (tick 194) and built on the same machinery, which is why it fit one tick. (1) `showPopover()` was a **TypeError** inside the click handler; (2) with no `[popover]` UA rule the dropdown's items rendered inline mid-page before anyone opened them. **js prelude**: showPopover/hidePopover/togglePopover(force); `el.popover` reflecting auto/manual/null (`auto` = the enumerated attribute's invalid-value default); `beforetoggle` (**cancelable** — the veto hook) + `toggle`, both carrying oldState/newState; `<button popovertarget popovertargetaction=show\|hide\|toggle>` **declaratively, no script**; light dismiss (outside click or Escape closes `auto`, `manual` ignores both); `auto` popovers mutually exclusive. **Both cascades in lockstep**: `[popover]` hidden, `[popover][data-manuk-popover-open]` a bordered block — attribute-keyed, not tag-keyed, since `popover` is a global attribute. **page/lib.rs**: the tick-194 modal branch in `z_index_map` widened, so an open popover gets the same `TOP_LAYER_Z` promotion — a menu that renders under the sticky header it hangs off is not a menu. `data-manuk-popover-open` IS the `:popover-open` state (same JS↔Rust boundary problem as `data-manuk-modal`). **The gate caught a real bug beyond the feature:** `'popover' in HTMLElement.prototype` — the canonical detection — was FALSE while every element had the members, because the custom-elements shim gives the `HTMLElement` constructor a fresh `{}` prototype on purpose (upgrade grafts onto the host object); mirrored the descriptors so both reads agree, and logged that EVERY `'x' in HTMLElement.prototype` detection shares the blind spot. **Safety: additive — UA rule keys on an attribute nothing else mentions, z_index_map branch fires only on `[data-manuk-popover-open]`, prelude guarded on `typeof __HP.showPopover === 'undefined'`.** Gated by `g_popover` (14 claims) + `g_popover_render`; both halves proven RED independently (`display:none`→`block` gave the closed menu an 18.4px box; disabling the top-layer branch put it behind the header). Residue: nested popovers (flat exclusivity), anchor positioning (a popover is a block in flow, not floating next to its invoker), `::backdrop`, inertness/focus trap |
| **`response.body` / `ReadableStream`** — a streamed answer renders at all (the fetch-streaming read) | **every AI chat** (claude.ai, ChatGPT, Gemini, Grok), cloud-console live-log tails, inference token streams, progress-reporting uploads/downloads — anything whose answer arrives incrementally; named the **#1 unlock** by the Phase-0 edge audit | ✅ (tick 196) — `__makeResponse` hardcoded **`body: null`** and `ReadableStream` was an `__inertNames` stub (a *named, EMPTY* constructor with no `getReader`), so the canonical `const reader = (await fetch(url)).body.getReader()` threw a **TypeError inside the response handler**, taking the handler with it. The symptom is not "the answer streams in slowly" — **the answer never appears**, so the whole class rendered blank. **`typeof` lied twice:** `typeof ReadableStream === 'function'` was already true against the stub and `'body' in res` already true against the `null` — the gate therefore asserts a reader that actually READS (the `g_globals` lesson). Built a real `ReadableStream` — a chunk queue plus a list of `read()` calls parked on an empty queue, `enqueue`/`close`/`error` settling the parked readers, which is the entire mechanism — with `getReader()` (locking) + `ReadableStreamDefaultReader` (`read`/`releaseLock`/`cancel`/`closed`), `locked`, `cancel()`, `tee()` (AI SDKs fork the token stream) and `Symbol.asyncIterator` for `for await`. `Response` gained a **lazy** `body` (eager construction would copy bytes for every response a page only `.json()`s), an accessor-backed honest `bodyUsed` flipping on any consumption route, and `arrayBuffer()`/`bytes()`/`blob()`. Defined ahead of the inert sweep that runs LAST, which is what suppresses the stub (the `AbortSignal` ordering mechanism). **HONEST BOUNDARY, not smuggled:** the body reaches JS **fully buffered** (`manuk_net::request`→`NavEvent::PageFetch`→`deliver` carries one `String` as a JS string literal), so the stream yields from memory, not off the wire — the *page's* path is entirely real (pump loop, `done`, `TextDecoder`, SSE framing all execute as written and the answer renders) but incremental wire-level delivery needs a per-chunk channel through shell→page→js that does not exist below `manuk_net::fetch_streaming` (document loader only). That is a **subsystem, not a tick** — residue, NOT claimed; a long answer appears in one go rather than token by token. **Safety: additive, guarded on `typeof ReadableStream === 'undefined'`; `text()`/`json()`/`clone()` keep exact previous semantics.** Gated by `g_fetch_stream` (12 claims over a real SSE body through `Page::load`→`take_fetches`→`resolve_fetch`: non-null body with getReader, locked before/after, Uint8Array chunks, `{done:true,value:undefined}`, bodyUsed flip, SSE `data:` framing reassembling to "Hello world" **and reaching the DOM**, clone freshness, tee mirroring, arrayBuffer byte length); proven RED with `THREW:TypeError: res.body is null`. Residue: wire-level chunking, `EventSource` still an honest stub, permissive double-`text()`, no BYOB/backpressure/`WritableStream` |
| **incremental fetch delivery** (`FetchStreamEvent` Head/Chunk/End) — a streamed answer **types itself out** instead of appearing in one lump | AI chat token streams (claude.ai/ChatGPT/Gemini/Grok), cloud-console live-log tails, inference output, progressive upload/download reporting — the half of streaming that makes it *feel* like streaming | ✅ (tick 197) — tick 196 gave the page a real `response.body` to READ; it could still only be FED the whole body at once (`Page::resolve_fetch` settles with one complete `String`), so a streamed answer appeared in a single lump when the server finished. New `manuk_js::FetchStreamEvent { Head{status,headers}, Chunk(Vec<u8>), End }` with ONE entry point per layer: `Page::deliver_fetch_stream` → `manuk_js::deliver_fetch_stream` → `PageContext::deliver_fetch_stream` → `event_loop::{deliver_head,deliver_chunk,deliver_end}`. **Resolving at the HEADERS is load-bearing** — a real `fetch()` settles when headers arrive, not when the body ends, which is what lets a page take a reader and pump while the rest is in flight; resolving at the end is buffered behaviour in a stream's costume. Each step runs the page's reactions before returning and `Page::deliver_fetch_stream` re-cascades+re-lays-out after, **guarded on the dirty bit** — that guard is what renders the answer BETWEEN chunks at no cost for a chunk the page ignores. **Bytes stay bytes** across the boundary (`js_bytes_literal` one `\u00NN` per byte ↔ `__bytesFromLatin1`), explicitly NOT `from_utf8_lossy`: a chunk boundary routinely splits a multi-byte sequence and lossy decoding substitutes U+FFFD. **`TextDecoder` gained `{stream:true}`** (hold the incomplete trailing sequence, prepend to the next call) — mandatory for the same reason; every streaming client passes it. A streaming Response keeps a buffered mirror for `text()`/`json()` but **drops it once a reader is taken** (an endless SSE stream must not accumulate every token forever); `clone()` on a streaming body throws, `body.tee()` is the honest fork. Gated by `g_fetch_stream_incremental` — Head→Chunk→Chunk→End asserting the DOM **between** chunks, so each claim is checked at a moment when the rest of the body **does not exist yet** and no buffered implementation can pass; includes a chunk boundary splitting "café"'s é. **Proven RED by disabling the per-step reaction drain** (`head:200` never reached the DOM). Residue: the host still calls buffered `resolve_fetch` (`pump_fetches` uses `manuk_net::request`; `fetch_streaming` is GET-only, no request headers) — wiring them + a `NavEvent` per step is the next tick; `EventSource` and XHR `readyState 3` should ride this same spine |
| **live streaming over the wire** (`request_streaming` + `NavEvent::PageFetchStream`) — a page's `fetch()` streams during REAL navigation, not just in the engine | AI chat token streams, cloud-console live-log tails, inference output — the half that makes the capability reachable from the browser rather than from a test | ✅ (tick 198) — tick 197 built the engine spine but `pump_fetches` still called buffered `resolve_fetch`, so nothing streamed in the actual browser. New `manuk_net::request_streaming(method, url, headers, body, on_head, on_chunk)`: what `fetch_streaming` is to the document, plus the three things it cannot do (arbitrary **method**, request **headers** — an API call without its `Authorization` is a 401 — and a request **body**) and one it does not (**`on_head` fires BEFORE the body starts arriving**; returning `ResponseMeta` at the end cannot express "headers now, body later", and late headers hand the page a stream that is already complete). Redirects follow the browser rule (301/302/303 → bodiless GET, 307/308 replay method+body). `NavEvent::PageFetch` → `NavEvent::PageFetchStream {gen,id,event}`, one event per step, `gen` still dropping responses for a navigated-away page. **The CORS read barrier moved to the HEADERS and is strictly stronger there** — the buffered path read the whole cross-origin body then decided it was unreadable; now it is refused before a single byte is forwarded and the chunk callback drops the rest, still surfaced as Chromium does (`status 0` → TypeError). **Failure has two shapes:** before the headers must REJECT (`Head{status:0}`), after them can only TRUNCATE (`End`) — a reader that never sees `done` spins forever. On the UI thread the follow-on work (re-pump, history, messages, cookie/storage persist) runs only on `End` (per-chunk would re-drain the queue and re-save cookies every token) while `rerender()` runs on EVERY step — the visible half. Gated by a **timing** claim, the only kind buffering cannot fake: a raw-TCP server sends headers, half the body, then holds the rest 250ms; the first chunk must land ≥200ms before the last, and the request's POST/Authorization/body must reach the wire. **Proven RED by making the impl collect the body and hand it over at the end — `chunks=1, first=last=253ms`.** net 59 + shell 58 green. Residue: `EventSource`/SSE and XHR `readyState 3` still stubs (should ride this spine); no per-header Expose-Headers safelist; the shell path itself is not wall-gated (no UI harness) — the net half is |
| **a11y node STATES** (`A11yState`: checked/expanded/selected/disabled/required/readonly/focused/value) — the agent can CONFIRM ITS OWN ACTION | every agentic task on the web: tick a consent box, open a menu, fill a field, skip a disabled button — and the verification step after each one | ✅ (tick 199, Phase-0 finish-line lever 2) — `A11yNode` carried role/name/bbox/z and **nothing about state**, so an agent's observation was byte-identical before and after its own click (`checkbox "Remember me"` → `checkbox "Remember me"`). An agent that cannot observe the result of its action cannot verify it: it proceeds on faith, or re-clicks and toggles the setting back off. **The agentic moat, not an a11y nicety — so the gate asserts the DIFFERENCE between two snapshots, not the presence of a field.** New `A11yState` on every node, computed by `state_of` from the DOM. **`Option` = NOT APPLICABLE, not false** (a link is not "unchecked"; reporting `checked:false` on it is a lie an agent could act on). **Checkedness is TRI-STATE** — `mixed` is what a "select all" parent checkbox really shows, and flattening it to false says the opposite of what the page means. **ARIA wins over the native attribute** (the cascade AT uses; the native attribute cannot express `mixed`). Script-driven state is visible because `el.checked = true` writes the `checked` ATTRIBUTE through the reflector — that is what makes click-then-read-back work. `render()` returns "" when there is no state, so static documents' observation lines are byte-unchanged; a control appends ` [checked disabled value="ada"]`. **Focus is HOST-owned** (the shell publishes it via `set_view_state`, unreadable from the DOM) → `build_tree_with_focus` / `Page::a11y_tree_with_focus` take it from a caller that knows; plain builders leave it false rather than guessing. Gated by `g_a11y_state` (click a button whose handler flips checked/aria-expanded/value/details-open, then assert `before != after` plus each specific read-back, that exactly the disabled button reports disabled, that `mixed` survives, and that a plain button gets NO suffix); **proven RED by stubbing `state_of` to default — before==after**. a11y 14, agent 125, workspace check green. **Workspace-wide edit:** `A11yNode` literals also live in `agent/src/{targeting,grounding,automation}.rs`. Residue: `disabled` doesn't inherit from `<fieldset disabled>`; no valuemin/max/text, aria-invalid/busy/pressed/current/level; `A11yDiff` still diffs `(role,name)` only so a pure state change shows in observation lines but not in `diff()`; **and the larger gap it exposes — `element.click()` fires the event but does NOT run activation behaviour**, so read-back confirms script-driven state today, native activation is its own tick |
| **WebSocket transport** (`manuk_net::websocket::WebSocketConn`, borrowed from tokio-tungstenite) | live chat + DMs, presence indicators, collaborative editing, trading/sports tickers, cloud-console live logs — every page whose content arrives without being asked for | ✅ (tick 200, Phase-0 finish-line lever 3, transport half) — the page-facing `WebSocket` was an **honest stub** (constructs, then reports failure), so a live-news site's live-blog silently never updated. **BORROWED, not hand-rolled**: RFC 6455 framing, client masking, the close handshake, continuation frames and ping/pong are the wheel not to reinvent — subtly wrong masking works against one server and hangs against another. **But the TLS is OURS, and that is load-bearing**: tokio-tungstenite's TLS features pull an unpinned tokio-rustls and cargo's feature UNION would re-enable the `aws-lc` backend graph-wide — the exact failure documented in engine/net/Cargo.toml that once broke the Windows build (`link.exe: 1104`). Taken with `default-features=false, features=["handshake"]`; we connect the socket, run TLS with the ring-pinned `proxy::tls_connect` (now `pub(crate)` for this), and hand tungstenite a ready stream via `client_async`. **Subprotocols negotiated, not assumed** — handshake built by hand so `Sec-WebSocket-Protocol` carries the offered list, and `protocol()` reports what the SERVER chose (offered two, got "" back → speak neither). Ping/pong consumed, not surfaced (keepalive, not page data; the JS API doesn't expose them either). **The close handshake is a real trap the gate caught:** the gate's first server `return`ed on the first Close frame, but tungstenite replies to a close from inside `next()`, so bailing early drops the socket before the reply flushes — client correctly reported `Connection reset without closing handshake`, which is NOT a client bug (a server that drops the socket is indistinguishable from a crashed one). Gated against a REAL server (tungstenite's accept side, not a mock of our own client): handshake, subprotocol negotiation, text+binary round-trip, **an unprompted server push** (the capability polling cannot express), and a clean close observed as end-of-stream so `onclose` fires instead of hanging. net 60 green, workspace check green. Residue: the JS `WebSocket` is STILL the stub — wiring transport→JS (shell pump, per-connection id, onopen/onmessage/onclose/onerror, bufferedAmount, binaryType) is the next tick and finishes lever 3; no permessage-deflate, no auto-reconnect (correctly the page's job), no Blob binaryType |
| **page-facing `WebSocket` connects** (`WsOp` out / `WsEvent` in) — a live chat receives a message the page never asked for, and it appears | live chat + DMs, presence, collaborative editing, tickers, cloud log-tails — the page-facing half of the same class | ✅ (tick 201, Phase-0 finish-line lever 3, page half) — tick 200 built the transport but the JS `WebSocket` was still the honest stub (constructs, sits in CONNECTING, fires error+close), so every live-blog connected, failed and rendered nothing, and `send()` threw unconditionally because the socket was never open. Now queues ops for the host and receives events back, the same shape `fetch` uses: **`WsOp`** `Connect{url,protocols}` / `Send{data,binary}` / `Close{code,reason}` via `Page::take_ws_ops()`; **`WsEvent`** `Open{protocol,extensions}` / `Message{data,binary}` / `Sent{bytes}` / `Error` / `Close{code,reason,clean}` via `Page::deliver_ws_event()`, which runs handlers and re-renders if they dirtied the DOM. **What the stub got wrong BEYOND not connecting:** it pre-filled `socket.protocol` with the client's first OFFERED subprotocol — but `protocol` is what the SERVER selects and is empty until it does, so the stub told pages a negotiation had happened when none had. `send()` before OPEN still throws InvalidStateError (spec; clients are written for it), after CLOSING drops the frame. `close()` moves to CLOSING(2), not straight to CLOSED(3) — the handshake is not instant and a page watching readyState sees the real intermediate. **Bytes stay bytes**: frames cross one char per byte and Rust decodes `c as u32 & 0xff`, NOT `as_bytes()` (which would UTF-8-encode 0x80..0xFF into two bytes each and corrupt every binary frame); `binaryType` then picks the page-visible shape (arraybuffer→ArrayBuffer else Blob). The `error` event carries NO detail to the page, deliberately — the spec withholds it as a cross-origin info leak. Gated by `g_websocket` (connect op carries URL+offered protocols; early send throws; onopen reports the SERVER's protocol + readyState 1; a frame sent from onopen reaches the host queue; **an unprompted server push lands in onmessage and mutates the DOM**, twice, appending; a binary frame preserves 0xFF; onclose reports code/wasClean/readyState 3); **proven RED by making deliver_ws_event not reach the page — onopen never fires**. Residue: **the shell is NOT wired** — nothing calls take_ws_ops/deliver_ws_event from gui.rs, so this is engine-reachable but not live during browsing; that is the next tick and the true end of lever 3 (needs a per-connection task holding WebSocketConn plus an mpsc from the UI thread for sends — bidirectional, unlike fetch). `bufferedAmount` decrements via `Sent` but nothing emits it yet; no Blob binaryType read path; no permessage-deflate |
| **WebSocket LIVE in the browser** (`gui.rs::pump_websockets`) — a live chat works during ordinary browsing, not just in a gate | live chat + DMs, presence, collaborative editing, tickers, cloud log-tails — the class is now actually reachable by a user | ✅ (tick 202, Phase-0 finish-line lever 3 COMPLETE) — t200 built the transport, t201 the page surface; nothing called them, so the capability was engine-reachable but not live. **Not shaped like `pump_fetches`:** a fetch is one request/one response so its worker is fire-and-forget, but a socket **stays open and is written to long after it opened** — so each connection gets a task owning the `WebSocketConn` plus an `mpsc::UnboundedSender` the UI thread queues onto (`App::ws_send` by socket id), and the task `select!`s between "the page wants to send" and "the server said something" (the only way to service both without starving one; a polling loop cannot). **Dropping the sender IS the close signal** — `WsOp::Close` removes the entry, `rx.recv()` returns None, the task completes the closing handshake and reports the REAL close, so `onclose` reflects what happened rather than an optimistic local guess. **Navigation closes every socket** (`ws_send.clear()` beside the `nav_gen` bump) — a live-chat socket must not keep streaming into a document the user has left; the gen guard drops frames already in flight. `WsEvent::Sent{bytes}` emitted once a frame is on the wire is what makes `bufferedAmount` fall for a client polling it against a slow socket; a failed connect sends error then close(1006, wasClean:false), which is what a reconnect loop backs off on. **Gated by COMPOSITION, because the shell cannot be** (no UI harness — same honest limit as T6.1 and the t198 fetch wiring): `g_websocket_live` does exactly what `pump_websockets` does, in the same order, with a REAL server in the middle — drain ops, connect a real `WebSocketConn`, resolve the page's relative `'/live'` against the doc URL, put the page's own frame on the wire, pump replies back, assert the DOM reads `offline[pong:ping][push](closed 1000)`. If the halves disagreed about op encoding, the one-char-per-byte convention, the subprotocol or close semantics, that gate fails where both unit gates pass. shell 58+2, net 60, page gates green. Residue: no Blob binaryType read path; no permessage-deflate; no auto-reconnect (the page's job); the server's close CODE is not threaded through `recv()` yet (a clean close reports 1000 regardless) |
| **scroll anchoring** (`capture_scroll_anchor` / `scroll_anchor_delta`) — the feed stops jumping when something loads above what you are reading | every infinite feed, comment thread and article with lazy images or late ads — the single most complained-about behaviour on the mobile web | ✅ (tick 203, Phase-0 finish-line lever 4, mechanism) — 0 hits for scroll anchoring: content loading ABOVE the reading position pushed every following box down, so the line being read jumped off screen on each lazy load. Two `Page` methods used around any reflow-causing mutation: `capture_scroll_anchor(scroll_y)` remembers the element at the viewport top + its offset from the top edge; `scroll_anchor_delta(&anchor, scroll_y)` returns how far `scroll_y` must move so it stays visually still (`0.0` when nothing moved — the common case — or when the anchor is gone, because correcting for a vanished element moves the page for no reason). **Choosing the anchor IS the correctness, and the obvious choice is wrong:** it must be the first box beginning AT OR BELOW the top edge, because a box that STRADDLES it (`<body>`, `<html>`, every ancestor container) begins at y=0 and **does not move when content is inserted inside it** — anchoring to one yields a correction of exactly zero and the page jumps as if anchoring did not exist. **The gate caught precisely this**: the first implementation preferred the box closest to the top edge by absolute distance, picked `<body>`, and reported `delta=0` with the read line 300px lower. The deepest box is wrong too (a text run is what a reflow most likely destroys). Gated by `g_scroll_anchor`: reader's line at the viewport top, a 300px ad appended above it by a real click handler; asserts the UNCORRECTED jump equals the inserted height (proving the scenario is real), then that the delta restores the exact screen position, then that a relayout changing nothing above the fold yields a correction of **zero** (anchoring must be inert when nothing moved or it becomes its own drift). Residue: **`overflow-anchor: none` is NOT honoured** — not parsed, so anchoring applies unconditionally and a site that opted out is still anchored (a real if narrow divergence; needs a ComputedStyle field); document-scroll only, not per-`overflow:auto` container; **the shell does not call it yet** — wiring it around gui.rs's relayout paths is the completing step for lever 4 |
| **scroll anchoring is LIVE** (`gui.rs::with_scroll_anchor`) — a feed stops jumping during real browsing, not just in a gate | every infinite feed / comment thread / article with lazy images or late ads, as actually experienced by a user | ✅ (tick 204, Phase-0 finish-line lever 4) — t203 built the mechanism and nothing called it. `with_scroll_anchor(f)` wraps any reflow-causing operation: capture the anchor, run `f`, move `scroll_y` by however far the anchor moved. Wraps the two delivery handlers that can grow the document under the reader — `PageFetchStream` and `PageWebSocket` — which are the paths a real feed uses (lazy image, late ad, next page of posts arriving over the network and appended above the reading position). **The half-pixel threshold is not a fudge:** anchoring that is not inert when nothing moved becomes its own source of drift, so a sub-0.5px correction is discarded; the result is clamped to `[0, max_scroll]` so a correction cannot scroll past the document end. Gated by `g_scroll_anchor_live` — does what `with_scroll_anchor` does (capture → deliver → measure → apply) around the same `deliver_fetch_stream` call, with the ad's height arriving AS the fetch body; the shell has no UI harness (standing limit) so this gates the COMPOSITION: if the mechanism and the delivery path disagreed about when geometry is valid it fails where the unit gate passes. shell 58+2 green. Residue: `overflow-anchor: none` still not honoured — needs a ComputedStyle field fed by Stylo (where the shipping cascade reads from), and it is the one remaining honest divergence: a site that opted out is still anchored |
| **`EventSource` / SSE connects** — a live-updates page receives its stream, frame by frame | score tickers, CI/deploy log tails, notification streams, dashboard metrics, and the many AI chats that use SSE rather than fetch-streaming | ✅ (tick 205, completes finish-line lever 1's stated scope) — `EventSource` constructed and then reported it could not connect (honest, better than throwing, but every live-updates page was dead). **Implemented on top of our own `fetch`, which is why it is small:** t196-198 made `response.body` a real ReadableStream fed incrementally off the wire, and SSE is precisely "a text stream cut into frames on blank lines" — so this needed **NO new Rust plumbing**, the same route a polyfill takes except our fetch is real. Also the first proof the streaming spine carries a second consumer. The frame parser is where correctness lives: **a frame ends at a BLANK LINE, not a chunk boundary** (the trailing partial stays buffered — dispatching per chunk delivers half a message, which is exactly what the gate falsifies to: `[first\npar/1]`); CRLF/CR normalised first (a server sending `\r\n` would otherwise never appear to terminate a frame); multiple `data:` lines join with `\n` as ONE message; exactly one leading space stripped after the colon; a comment line (`: keepalive`) dispatches nothing; a named `event:` goes to its own listener and NOT to onmessage; `id:` persists as lastEventId; `{stream:true}` decoding for split multi-byte chars. Gated by `g_eventsource` — asserts the SSE `Accept` header reached the request, onopen at the headers with readyState 1, a complete frame dispatching while a partial one does NOT, the split frame reassembling across chunks, a named multi-line event reaching its own listener, and the keepalive dispatching nothing. Residue: **no automatic reconnection** — a real EventSource reconnects when the stream ends, honouring the server's `retry:` and resending `Last-Event-ID`; we parse `retry:` but ignore it and a finished stream fires `error` and stays closed. That is what makes SSE resilient in practice and is worth closing |
| **XHR `readyState 3`** — a transfer reports progress instead of jumping from "sent" to "done" | download/upload progress bars, pre-`fetch`-era streaming clients, long-poll comet endpoints, any library still built on XMLHttpRequest | ✅ (tick 206, completes finish-line lever 1 IN FULL) — the streaming path from t197-198 only knew about `fetch`; `__deliverHead` bailed out on an XHR id (a documented residue), so an XHR still got its whole body in one delivery: `readyState` went 1→4, `onprogress` never fired, `responseText` was empty until complete. **A progress bar showed nothing and then 100%** — the transfer appeared to take zero time. The three delivery entry points now branch by request kind: `__deliverHead` → readyState 2 (HEADERS_RECEIVED, status+headers readable, body empty); `__deliverChunk` → append to responseText, readyState 3 (LOADING), fire readystatechange + onprogress with `loaded`; `__deliverEnd` → readyState 4 (DONE), parse `responseType:"json"` **at this point, not before** (partial JSON does not parse), then onload/onerror + onloadend. `{stream:true}` decoding for split multi-byte chars. The buffered `__deliverXhr` remains for the non-streaming path (headless loader, mock-fetcher loop) where going straight to DONE is correct because the whole body really is there. Gated by `g_xhr_progress`: lifecycle is `2→3→3→4` not `1→4`; at readyState 3 the page reads a PARTIAL responseText and onprogress reports `loaded`; the body GROWS across chunks; onload has not fired while unfinished and fires once with the complete body. **Proven RED by never reporting LOADING — the state string collapses from `23` to `22`.** Lever 1 now complete in full: ReadableStream+response.body (196), incremental spine (197), the wire (198), real SSE (205), XHR readyState 3 (206) |
| **SSE reconnects and RESUMES** (`Last-Event-ID` + server-set `retry:`) — a live stream survives the connection dropping | score tickers, CI/deploy log tails, notification streams, dashboards — every SSE page, across the idle-timeouts and sleeps that happen constantly in practice | ✅ (tick 207) — t205 shipped EventSource with NO reconnection and flagged it as the one substantial gap. **Reconnection is the defining feature of SSE, not a nicety:** the contract a page is written against is "this stream stays alive", and servers close idle connections, proxies time out, laptops sleep — one blip otherwise ended the live updates permanently, ticker frozen, and the page had no way to know it should care. The stream ending now triggers a reconnect on a **macrotask** (`setTimeout`), so a stream that fails instantly cannot spin the microtask queue without yielding (the same reasoning the old honest-failure stub used). **`Last-Event-ID` is what makes it a RESUME rather than a restart** — the reconnect sends the last `id:` seen so the server replays what was missed; without it the reconnect LOOKS like it works and quietly drops every event in the gap. **The server sets the delay**: `retry:` parsed and honoured (default 3000ms) — not politeness but how a server sheds load after an incident instead of being hammered by every client at its own fixed interval. **A 204 or any 4xx means STOP** and is not retried (reconnecting into a 404 forever is a self-inflicted DoS). Gated by `g_eventsource_reconnect`: first request carries no Last-Event-ID; after a frame with `id: 42` the stream drops and the client reconnects to the same URL **carrying `Last-Event-ID: 42`**; the resumed stream appends to existing page state; a 204 is not reconnected into. **Proven RED by never scheduling the reconnect — no second request is issued at all.** Residue: no exponential backoff beyond what the server asks; a network failure and a clean stream end are treated identically |
| **click ACTIVATION behaviour** (checkbox ticks, radio group selects, `input`+`change` fire) | every consent box, settings toggle, plan picker and filter checkbox — and the agentic loop, where a click that changes nothing is indistinguishable from a click that failed | ✅ (tick 208) — `dispatch_click` fired the EVENT and stopped: no activation behaviour, so clicking a checkbox left it unchecked, a radio selected nothing, and `input`/`change` never fired. t199 gave the agent state read-back and flagged this as what made it half-useful: an agent could see a box unchecked, click it, and see it still unchecked. **The ordering is the subtle half, and getting it backwards still passes a naive test:** the toggle happens BEFORE the click event is dispatched, which is why a real handler reading `this.checked` sees the NEW state — toggling after would end in the same final state while handing every handler on the web a stale value, so the gate asserts what the handler SAW. **checkbox** toggles, and `preventDefault()` undoes it (the canceled-activation steps — a page validating before allowing a toggle depends on this). **radio is a GROUP, not a toggle**: clicking one deselects its peers grouped by `name` (how the form serialises), a radio never unchecks itself, a different name group is untouched — two checked radios in one group means the form submits the wrong value. **`input` then `change`**, in that order, both after the state is committed (every controlled-component binding is written for it). Gated by `g_click_activation`: box ticks/unticks; handler log reads `click:true input:true change:true click:false input:false change:false`; preventDefault leaves it unticked; radio deselects its peer and not the other group; an already-selected radio stays selected. **Proven RED by returning no activation — the box never ticks and `click:true` collapses.** Residue: only checkbox and radio activate — a link does not navigate and a submit button does not submit FROM `element.click()` (the native GUI paths handle those separately, so this is a gap in the scripted/agent path specifically); `<select>`/`<option>` selection and `<label>`→control forwarding are not done |
| **`<label>` forwards its click to the control** | every checkbox/radio on the web that is clicked by its text rather than its 12px box — consent, settings, filters, plan pickers — and every agent told to "click the X checkbox", since the label is what carries the accessible name | ✅ (tick 209) — clicking a `<label>` did nothing, and the label is how most checkboxes are ACTUALLY clicked. **Worse for an agent than for a person:** the label carries the accessible name, so "click the Remember me checkbox" resolves to the label, clicks it, and nothing happens — and a click that does nothing is indistinguishable from a control that does nothing. Both association forms handled: `for="id"` resolved to a LABELABLE element (input/select/textarea/button/meter) and a label WRAPPING its control (first labelable descendant); a `for` naming nothing labelable labels nothing and does NOT fall back to a descendant, because the author said which control they meant. **The recursion trap:** a control nested inside its own label is the common markup, and forwarding naively means the control's own click forwards back through the label forever — or double-toggles and so appears to do nothing at all; forwarding only happens when the clicked node IS the label. **The label's own click still fires and can still be cancelled** — `preventDefault()` on the label stops the control being activated, exactly as on the control itself. Gated by `g_label_click` (a `for=` label ticks and unticks; a wrapping label forwards to its descendant; clicking the control INSIDE its own label toggles exactly once; a cancelled label click does not reach the control; a label pointing at nothing does not panic). **Proven RED by not forwarding — the box never ticks.** |
| **a disabled control is inert** (incl. `<fieldset disabled>` inheritance) — and a script-free form still works | every form with conditionally-disabled controls or a disabled step; and every agentic run, where ticking an inert box and reading it back as ticked reports success on a form the server rejects | ✅ (tick 210) — ticks 208/209 ran activation without checking disabledness, so clicking a disabled checkbox ticked it, and so did clicking its label. A disabled control is not "styled grey", it is **inert**. **Worse than cosmetic for an agent:** it ticks a disabled consent box, reads the state back (t199), sees it ticked, and reports success on a form the server will reject — **a wrong observation is more expensive than a failed action, because nothing downstream questions it**. So the a11y tree was fixed in the same tick (`disabled` now inherits from an ancestor `<fieldset disabled>` there too) and the gate asserts the tree and the activation path AGREE. **`<fieldset disabled>` inheritance is not an edge case** — disabling a whole step of a multi-step form with one fieldset is idiomatic; checking only the control's own attribute leaves every control in that step live. Only a `<fieldset>` propagates it; a disabled `<div>` means nothing. **Second finding, exposed by the gate's positive case:** activation was gated on having a JS context — `dispatch_click` returned early when `self.js` was None, so **a static form with no `<script>` had inert checkboxes**, which tick in every real browser. Event dispatch needs JS; the toggle does not, and the two are now separate (with no JS nothing can call preventDefault, so activation always proceeds). This surfaced ONLY because the gate deliberately included a control that must still work (`#live`) alongside the ones that must not — without that positive case, an implementation that made everything inert would have passed every other assertion. Gated by `g_disabled_inert`; **proven RED by skipping the disabled check — the disabled box ticks.** a11y 14, workspace check green |
| **clicking a submit button submits the form** | every login, search, checkout and settings form — and the single most common agentic instruction there is ("click Sign in") | ✅ (tick 211) — `element.click()` on a submit button fired a click event and stopped; nothing was queued, so the form never submitted and the agent could not distinguish "the button is broken" from "we never submitted". A submit-button click now pushes its form onto `Page::pending_submits`, drained by `take_form_submits()` into the **`requested`** list the shell already services. **`requested`, not `direct`, is the load-bearing choice:** `requested` fires the `submit` event first so the page's validation handler runs and can cancel — and click-to-submit is exactly the case pages validate; `direct` would skip every client-side validator on the web. Details that decide whether real pages work: **a bare `<button>` inside a form defaults to `type=submit`** (the classic source of "why did my page reload" — not honouring it means Sign in does nothing); **`type=button`/`type=reset` do not submit** (else every toggle and menu built from a `<button>` reloads the page); **`form="id"` associates a button with a form it is not inside** and wins over the ancestor; a **disabled** submit button submits nothing (t210's rule applied here); and the queue is a **drain**, so the host cannot submit the same form twice. Gated by `g_submit_click` covering each; **proven RED by not queueing — the form never submits.** Residue: `formaction`/`formmethod`/`formnovalidate` on the button are not carried to the submission, and the submitter is not recorded (a form with two submit buttons cannot tell which was used — `<button name=action value=delete>` is a real pattern); link navigation from `element.click()` still not wired |
| **the submitter reaches the server** (`<button name=action value=delete>`) — Save and Delete stop being indistinguishable | multi-action forms everywhere: save/delete, approve/reject, add/remove, publish/discard — and any agent driving one, since it cannot detect the failure | ✅ (tick 212) — a submit button contributes `name=value` ONLY when it is the activating control, which is why the field walk skips every button; `agent/src/forms.rs` said so in a comment ("we do not model that, so they are skipped"). **The failure closed is a silent WRONG-ACTION bug, not a missing field:** without the submitter, `<button name="action" value="delete">` and `value="save"` post a **byte-identical body**, so the server cannot tell the destructive action from the safe one and an agent has no way to detect it. Threaded end to end: `Page::pending_submits` records `(form, submitter)` on click → `take_form_submits()` yields `Vec<(NodeId, Option<NodeId>)>` → `gui.rs::navigate_form_with` → `forms::urlencoded_submission_with_submitter` → `fields_with_submitter`. **`None` is the honest answer for a script's `requestSubmit()`** (no submitter unless passed, which is not modelled — nothing is guessed); the submitter goes **LAST**, matching the order a browser builds the entry list; **a button with no `name` is not a successful control** and contributes nothing (its value must not be smuggled in under another key); a button that was not clicked still never appears. Gated in `agent/src/forms.rs` — Save and Delete must produce DIFFERENT bodies, the nameless button contributes nothing, and the submitter reaches the **POST body** not just the field list (the wire is what the server reads). **Proven RED by ignoring the submitter — Save and Delete collapse to the same body.** agent 126 green. Residue: `formaction`/`formmethod`/`formnovalidate` on the button do not override the form's; `requestSubmit(submitter)` does not carry its argument |
| **a geometry read after a mid-script mutation sees the NEW layout** (forced synchronous reflow) | the `measure -> mutate -> measure` round every **virtualized list** is built out of — react-window, react-virtuoso, TanStack Virtual, every data grid, every autosizing textarea, every "measure my own height then position the tooltip/dropdown" helper. All of them write to the DOM and immediately read it back inside one task | ✅ (tick 213) — the engine lays out in a **batch** (script runs against a snapshot taken before it started, one relayout after), which is right for measure-only and for mutate-only and **wrong** for measure→mutate→measure: the second read returned pre-mutation geometry, `0` for a node that did not exist yet, so rows collapse, overlap or render blank. The relayout machinery already existed (`relayout_incremental`, `RestyleDamage`); the missing piece was the **read path**. New monotonic `Dom::mutation_seq` — deliberately *not* the dirty bits, which the batch pass **consumes** and so cannot answer a mid-script question without disturbing it; a counter answers by comparison, so repeated reads on an unchanged tree cost one integer compare and the post-script relayout still sees exactly the bits it always saw. A `ReflowFn` hook installed per script round calls **upward** from `manuk-js` into `manuk-page` (layout lives there; `manuk-js` must not grow a layout dependency), armed at every round-entry: load, `dispatch_click` (incl. the nested `<label>` forward), input/change, focus/blur, WS delivery, fetch-stream delivery, popstate. A **stack** of hooks not a slot (nested rounds — an inner teardown would disarm the outer and every later read would quietly revert to the stale snapshot); the reflow builds its **own** maps and re-points the bindings, `ReflowScope::Drop` restoring the previous pointers (absence reads not as a crash but as *the next document measuring freed memory*); an `IN_REFLOW` re-entrancy guard. **Both** `layout_rect` and `with_style` force it — `getComputedStyle` is a forced-reflow trigger as much as `getBoundingClientRect`, and gating only the geometry read leaves the two APIs disagreeing about the same element one line apart. Gated by `G_FORCED_REFLOW`, **proven RED** by removing `force_reflow_if_stale()`: `after:0 row:0 grown:10 offset:0` — the blank-list bug exactly. Residue: `scrollIntoView`/`getClientRects` share the path but are not separately asserted; the reflow drops its box tree rather than committing it (the post-script pass still produces what gets painted) |
| **complex-script shaping** — Arabic letters JOIN, Devanagari conjuncts FORM (the shaper is told which script it is shaping) | the entire non-Latin web: Arabic (~"400M speakers", every `.sa`/`.ae`/`.eg` site, Al Jazeera), Persian/Urdu, Hindi/Marathi/Nepali Devanagari, and by the same mechanism Thai, Bengali, Tamil, Khmer. Any page whose text is not Latin/CJK | ✅ (tick 214) — swash's `ShaperBuilder` defaults `script` to `Script::Latin` and `shape_run` never called `.script()`. **The script selects the OpenType feature set**, so every run on the web was shaped with Latin's: no `init`/`medi`/`fina` (Arabic joining), no `akhn`/`half`/`pres` (conjuncts), no matra reordering. `مرحبا` rendered as five disconnected isolated letterforms; `नमस्ते` as a flat 1:1 codepoint→glyph mapping with the virama a visible dangling mark. **It survived because nothing was MISSING** — no `.notdef`, no tofu, no error, a plausible width, and the per-glyph fallback picking exactly the right face: real letters, right font, wrong text, and it looks fine to anyone who does not read the script. Every instrument here measured *coverage*, and this bug has perfect coverage. Fixed by script-aware segmentation: `segment()` returns `(FaceId, Script, String)` and breaks a run when **either** face or script changes, with `Common`/`Inherited` (spaces, punctuation, marks) **extending** the run rather than opening one — otherwise an Arabic word split at its own comma stops joining across the cut, the same bug hiding in running text. Gated by `G_COMPLEX_SCRIPT`, both claims **proven RED** independently (Devanagari 6 glyphs for 6 codepoints; the Arabic interior letter keeping its isolated glyph id inside the word), with Latin/CJK glyph counts pinned against over-splitting. Residue: `line-break`/`hyphens` for CJK, Thai word segmentation (needs a dictionary), vertical writing modes, and `lang`-driven script disambiguation (Han is shaped the same for JP/SC/KR today) |
| **per-glyph font fallback across scripts** (CJK / emoji / Arabic / Hebrew / Devanagari) | Japanese/Chinese/Korean pages, emoji anywhere in UI text, mixed-script lines | ✅ **already worked — measured, not assumed (tick 214)**. `FALLBACK_FAMILIES` resolves real faces for every script probed, zero `.notdef`. The board listed "CJK/emoji renders as TOFU" as an unknown; the answer is **no**. Fifth time a feature assumed missing here was already built (after `localStorage`, `FormData`, `position: sticky`, `IntersectionObserver`) — **an absent measurement is not a negative measurement**. What was actually broken was the *shaping* of those correctly-resolved faces (row above) |
| **bidi base direction** — `direction: rtl` / `dir="rtl"` orders an RTL line from the right | the entire RTL web: Arabic (Al Jazeera, every `.sa`/`.ae`/`.eg` site), Hebrew, Persian, Urdu — and any LTR page embedding an RTL quote, name or address | ✅ (tick 215) — shaping picks the glyph, the **base level** decides where it goes, and `shape()` hard-coded the base to LTR, so `direction: rtl` and `dir="rtl"` changed nothing. After tick 214 every character was present and correctly shaped and still in the **wrong order**: trailing punctuation on the wrong end, embedded Latin/numbers on the wrong side of their neighbours, short lines hugging the wrong margin. Six touch points in `manuk-css` on the tick-183 `OverflowWrap` template + `stylo_engine` recovery (the shipping path is Stylo, which does not surface `direction` — without it the property works in tests and does nothing in the browser) + `dir="rtl"` as a presentational hint in `apply_ua_defaults` (nearly every RTL site uses the ATTRIBUTE, not CSS — a stylesheet-only implementation reads as "RTL unsupported" on exactly the sites that need it) + `TextStyle.rtl` layout→paint + the base direction added to `RunKey` (without it the second paragraph is a cache hit returning the first one's ordering). **HTML's initial value is `ltr`, not content detection** — inferring RTL from an unmarked Arabic paragraph would look more correct and would be a structural divergence from Chromium. Gated by `G_BIDI_BASE`, proven RED by pinning the base back to LTR; also pins pure-LTR text byte-identical under both bases. Measured residual: ~0.89px width difference on a 70px mixed line (the bases split into different bidi runs, so the inter-script space picks up a different advance) — inherent to per-run shaping, bounded at 3% by the gate. Residue: `dir="auto"`, `unicode-bidi` isolate/embed/override, RTL `text-align` default, and RTL block layout (markers, scrollbar side, float reversal) — RTL text now READS correctly; an RTL page does not yet lay out mirrored |
| **`<details>`/`<summary>` disclosure** — a closed section hides its body, and clicking the summary toggles it | GitHub (every folded diff, every collapsed review thread, every "Show more" in an issue), MDN's collapsible sections, every docs-site FAQ and changelog. Pure UA behaviour — none of them ship a line of script for it | ✅ (tick 216) — `details`/`summary` appeared **nowhere** in the engine, so every collapsible on the web rendered **permanently expanded** (a page of folded sections becomes a wall of everything at once) and clicking the summary did nothing, leaving the section unopenable and "click Show more" unactionable for an agent. Rendering follows the `<dialog>` precedent — a UA rule pair mirrored in both cascades (`UA_CSS`: `details > *:not(summary){display:none}` + `details[open] > *{display:block}`; `MinimalCascade`: `summary`→Block in `apply_ua_defaults` plus the collapse in **`cascade_node`**, which is where the PARENT's `open` is visible). Toggling is **activation behaviour** in `dispatch_click` — after the event, cancellable by `preventDefault()` — then a `toggle` event on the `<details>` fired after the attribute changes so a handler reading `.open` sees the new state. `summary_details_target` **walks up** from the clicked node, which is load-bearing: a click lands on a `<span>`/`<svg>`/text element inside the summary, essentially never on the summary box, so exact-hit matching works in a test and fails on every real page. **Uncovered a general bug underneath:** `set_attr` marked the tree dirty and `remove_attr` did NOT, so *unsetting* any boolean attribute (`open`, `checked`, `hidden`, `disabled`) never triggered a restyle — things could be turned ON and never back OFF, presenting as an intermittent "the UI didn't update" rather than a reproducible bug. Gated by `G_DETAILS`, whose assertions falsify **three independent mechanisms** (UA collapse rule, summary toggle, `remove_attr` dirty marking), each proven RED separately. Residue: the disclosure triangle marker (`::marker`/`list-item`) is not drawn; `name=` accordion grouping and `<details>` in `find-in-page` auto-expansion are unimplemented; the gate exercises the Stylo cascade only, so the MinimalCascade mirror is lockstep-by-convention |
| **The responsive image** — `<img width="800" height="400">` under the universal `img { max-width: 100% }` reset | the single most common image markup on the web: the dimension attributes reserve the box before the bytes arrive (the anti-layout-shift contract Next.js `<Image>`, WordPress and GitHub all emit), the reset makes it fit its column. Also every `<canvas>`/`<video>` given its size in markup | ✅ (tick 218) — **broken in two places at once, and only visible together.** (1) `aspect_ratio` came ONLY from a decoded bitmap, so `<canvas>`/`<video>` had none *ever* and an `<img>` had none *until it loaded* — precisely the window the attributes exist to cover. The attributes are an `aspect-ratio: auto w / h` hint (HTML §dimension attributes); now applied in both cascade paths into an EMPTY SLOT only, so a real decode still wins. (2) A min/max-width clamp did not transfer through the ratio: `layout_block` derived the height from the ratio only when it was `auto`, so with both axes specified the clamp narrowed the box and left the height alone — an 800x400 asset in a 400px column rendered **400x400, the picture squashed to half its width**, at every viewport narrower than the image. New arm applies CSS2.1 §10.4's proportional adjustment on an actual constraint violation, replaced elements only (an ordinary box's specified height stands). **Safety: fires only when a min/max clamp MOVED the width AND the element is replaced AND a ratio exists — an unclamped box keeps its declared size exactly, so nothing that laid out before lays out differently.** css-sizing 343→395 (20.5%→23.6%), css-flexbox/css-grid flat, Bar 0 clean. Gated by `g_replaced_ratio` (the `<canvas>` arm passes only if the ratio came from the ATTRIBUTES — a canvas has no bitmap) + a mirrored `manuk-layout` test; RED two independent ways, each yielding the squashed 400x400. Residue: only width→height transfers; `max-height` does not yet push back into the width |
| **`width: stretch` / `-webkit-fill-available`** — "fill the column" on a box that would otherwise hug | full-bleed floated cards and banners, a form field that fills its row, a `<canvas>`/`<img>` sized to its container rather than its intrinsic pixels, an overlay that fills without writing both insets. The mobile-Safari-era `fill-available` idiom, and `stretch` as shipped in Chrome | ✅ (tick 219) — the value reached layout as plain `Dim::Auto`. **On a plain block that is correct** (`auto` fills), which is why it hid; every box that SHRINK-TO-FITS on `auto` was wrong — float, inline-block, form control, replaced element, abspos. New `ComputedStyle.width_stretch` (inline mirror of the tick-154 `height_stretch`) set on both cascade paths and consumed at all four places a width is decided: `layout_block`, the float path, `layout_abs`, and the replaced-element aspect-ratio mirror (which was deriving `height x ratio` over the top of the stretched width). **Second half — a precedence rule that generalises: a UA default and an HTML presentational hint may only fill an ABSENT width, but every such site tested `width == Dim::Auto` and `stretch`/`min-content`/`max-content`/`fit-content` all COMPUTE to `Dim::Auto`** — so `<canvas width="40">`, `<input size=20>` and `<textarea cols=20>` each beat the author's declaration. Guarded on the flags now. **Safety: every arm is `Dim::Auto if s.width_stretch`, false for all existing content, so no box that laid out before lays out differently.** css-sizing 395→407, css-flexbox/css-position flat, Bar 0 clean. Gated by `g_width_stretch` (six boxes at 170px + a `width:auto` control that must still hug); RED two independent ways. Residue: an abspos with NO inset produces no box at all (pre-existing, own tick); logical `inset-inline-*` unmapped |

## The `position:relative` wrapper holding only an absolutely positioned child

The overlay / dropdown / tooltip / portal-root idiom — a relatively positioned wrapper whose *only*
child is `position:absolute` with no insets — generated **no box at all** until tick 220. The parent
establishes an inline formatting context (its sole child is out of flow, so it has no flow children),
and that branch never recorded the child's static position, which is the one thing that keeps an
all-`auto`-inset abs box placeable. Unlocks React portal roots, JS-positioned dropdowns and tooltips,
badge overlays, and `.sr-only` accessibility nodes written this way. Neighbouring shapes (a
block-level sibling, or a flex/grid parent) always worked, which is why the gap survived this long.

## Inline `data:` images, and images sized from one axis

An inline `data:` image had **no size at all** until tick 221 on any path that skips the async
subresource pass — it carries its own bytes, so nothing needed fetching, but nothing decoded it
either. Inline icons, base64 logos and sprite data-URIs are ubiquitous in component libraries and
bundled SPAs. Separately, any image that was given only a `width` **or** only a `height` inside a flex
or grid container came out zero-sized on the other axis, because the intrinsic ratio never reached
taffy: the avatar/logo/thumbnail case in every card and nav bar. Both render correctly now.

## Scroll-container detection (`getComputedStyle(el).overflowY`)

Until tick 222 the per-axis overflow values were absent from `getComputedStyle` entirely, so the
scroll-parent walk that every dropdown, modal, virtualised list and scroll-into-view helper performs
matched nothing and fell through to the document. Popups anchored to the viewport instead of their
scroll container, with nothing visibly wrong in the DOM. Unlocks the positioning layer of essentially
every component library (Floating UI / Popper and everything built on them).

## Adaptive streaming (MSE) — the player script itself, not just the video

Until tick 223 `MediaSource`, `SourceBuffer`, `SourceBufferList` and `TimeRanges` did not exist. The
class of the web this blocked is larger than it sounds: adaptive players — hls.js, dash.js, shaka,
video.js, and YouTube's own — construct a `MediaSource` inside a **capability probe at
module-evaluation time**, so the missing name was a `ReferenceError` that killed the player script
before it rendered a single control, and took the rest of the surrounding bundle's evaluation with
it. Not "the video does not play" — *the player is absent, and the page around it is damaged.* Nor
could such a player fall back to progressive download, since it died before reaching its own
fallback.

What works now is the **byte pipe**: `new MediaSource()`, `video.src = URL.createObjectURL(ms)`
flipping the source to `open` and firing `sourceopen`, `addSourceBuffer(mime)`, and an
`appendBuffer()` loop clocked by the real `updatestart`→`update`→`updateend` task sequence with a
correct `updating` flag. That is byte-for-byte the control flow every player executes, and it must
survive unchanged when a demuxer takes over the middle of it.

What deliberately does **not** work is decoding, and the way that is reported is the point.
`isTypeSupported()` answers from an empty decode registry, so every player is told **no** and takes
its documented fallback path. A stubbed `true` would be strictly worse than the honest `false`: it
steers the player onto the adaptive path, where it appends segments and polls `buffered` for a range
that can never grow — a silent hang, surfacing far from its cause, instead of a clean fallback. That
registry is the seam M3/M4/M5 (demux, AAC, VP9) fill in, at which point `isTypeSupported` begins
saying yes for exactly what can genuinely be played and nothing else changes.

## Canvas text — the labels on every chart, and the whole of a canvas-rendered app

`ctx.fillText` was `function(){}` and `measureText` returned `text.length * 7` until tick 224. This is
the **silent** shape of failure: a page feature-detects canvas, is told yes, draws its axis labels,
legend, tooltips and cell text, and gets back a picture with every label missing and nothing thrown
anywhere. It reads as a rendering bug, not a missing API, which is why it survives so long.

The class this unlocks is wider than "charts", though charts are the visible half — Chart.js, ECharts,
Plotly, D3-on-canvas, every sparkline and dashboard tile. Canvas is also how a growing set of apps
render **all** of their text: Google Docs/Sheets draw their document surface as glyphs on a canvas,
and terminal emulators (xterm.js and everything built on it) draw every cell that way. For those,
`fillText` is not a label — it is the entire application.

The `length * 7` half mattered independently, and is worse than an imprecise width: it has **no
relationship to the glyphs**, so everything derived from it compounds — centring, wrapping, column
fitting, "does this label collide with its neighbour", and hit-testing a terminal cell. Under it,
`IIIIIIIIII` and `WWWWWWWWWW` measure identically.

Both halves now run through `engine/text` — the same swash shaper, bidi reordering, per-glyph
fallback and raster cache as DOM text — so a canvas draws joined Arabic, Devanagari conjuncts, CJK
and emoji with no additional work, and cannot drift from the paragraph beside it. Gated by
`g_canvas_text`, which reads the canvas back with `getImageData` and counts **pixels**: a stub that
recorded the call, or a plausible `measureText` over a `fillText` that drew nothing, passes any
API-shaped assertion and fails this one. Proven RED both ways independently.

Residue, bounded and recorded: rotation/skew are not applied to the glyph raster (text lands at the
transformed origin at the correctly scaled size, but upright — rotated axis labels are the loss);
`maxWidth` re-shapes smaller rather than condensing horizontally; `strokeText` renders filled in the
stroke colour; `drawImage`, `putImageData`, real gradients and `clip()` remain unimplemented, so a
canvas app that composites images is still short.

## WebAssembly — measured, not assumed (tick 225)

Carried as an `unknown` in the capability constellation with "Figma, games, ffmpeg.wasm" as the cost
of not having it. It works: a real module compiles, instantiates, resolves its export and returns the
right value. Nothing had to be built — the cell was simply never measured, and it had been steering
the lever board's priorities for the whole time it sat there.

That is now the sixth capability assumed missing here that was already present (after `localStorage`,
`FormData`, `position: sticky`, `IntersectionObserver`, and CJK/emoji font fallback). The lesson keeps
being the same one and is worth stating flatly: **an absent measurement is not a negative
measurement.** A cheap behavioural probe is a better first move than an implementation plan.

The same run measured CJK line breaking and print/media queries as already working, and pinned eleven
capabilities as *measured* missing (multicol, container queries, scroll snap, `text-wrap: balance`,
View Transitions, Navigation API, WebCodecs, Sanitizer, custom highlights, scoped custom element
registries, drag and drop) — evidence in place of assumption, with a gate that flips them to failures
the day they get built.

## "Sign in with…" — the OAuth redirect login (tick 226)

Carried as `unknown` in the constellation with the bluntest cost line in the file: *you cannot log in
to the modern web*. GitHub, Google, every SaaS dashboard, every "continue with" button. It **already
worked** — a full authorization-code flow completes across two real origins, first run.

The flow is six features agreeing, each in a different layer, and they fail into one
indistinguishable symptom (the callback screen hangs forever): the cross-origin 302 is followed; the
query survives it (the authorization code *is* the query); the post-redirect `final_url` reaches the
page, so `location.search` is the callback's and not the authorize URL's; a cross-origin `fetch` POST
carries both its body and the page's chosen `Content-Type`; and `Authorization: Bearer` survives onto
the wire.

Gated by `g_oauth_redirect` against two real `TcpListener`s, asserting the **wire** as well as the
DOM — the code in the POST body, the form content type, the bearer token on the userinfo call. The
reason for that: the RED probe that drops page headers still renders a logged-in page, reading
`signedin:ANONYMOUS` — a complete logged-in shell with nobody in it. A DOM-only assertion passes it.

Still unbuilt on this track (O2-O5): interactive cross-origin iframe re-render, popup +
`postMessage`, third-party/cross-site cookie policy, and FedCM `navigator.credentials`. The redirect
flow — which is what the overwhelming majority of "sign in with" buttons actually use — is done.

## Binary response bodies (media segments, and everything after them)

Measured in tick 227 and **broken**: a 260-byte media segment fetched with `.arrayBuffer()` comes
back as 407 bytes. The response body crosses the JS boundary as a Rust `&str`, so every byte above
`0x7F` is carried as a codepoint and re-encoded as two on the way out (`0xDF` → `0xC3 0x9F`).

Bytes below `0x80` survive perfectly, which is why nothing noticed until now — JSON, HTML, SSE and
form bodies all round-trip exactly. The classes this blocks are the ones that fetch *bytes*: MSE
media segments (and therefore the entire watch-the-web track — a demuxer cannot parse a corrupted
stream, and the failure surfaces inside the demuxer as if it were a codec bug), plus WASM modules
fetched over the network, binary file uploads read back, and any `arrayBuffer()` consumer.

Byte-range requests, the other half of segmented delivery, **do** work: `Range` reaches the wire and
`206` surfaces intact. Gated by `g_media_segment_fetch`, which pins the working half today and has
the binary claims written and waiting.

## Binary response bodies — FIXED (tick 228)

The 260-byte media segment that came back as 407 now round-trips byte-exact. The body crosses the JS
boundary on two channels: charset-decoded text for `.text()`/`.json()`, raw bytes for
`.arrayBuffer()`/`.bytes()`/`.body` and an `arraybuffer` XHR. This unblocks MSE segment fetching (and
therefore the demuxer step), WASM modules fetched over the network, and every other `arrayBuffer()`
consumer. Byte ranges already worked and are gated alongside it in `g_media_segment_fetch`.

## Hydration (SSR markup + client attach) — measured working (tick 229)

The dominant delivery pattern of the modern web — Next.js, Nuxt, Remix, SvelteKit, Astro — and it
works: server markup is in the DOM before any script runs, node identity survives the client's
attach, listeners bound to that server markup fire on a real dispatched click, and a server/client
mismatch is both detectable and patchable.

It is the canonical **silent** failure, which is why it needed driving rather than looking at: every
step is ordinary DOM work, so a broken hydration throws nothing and the page looks perfect while
being dead — inert buttons, menus that never open, forms that never validate. Gated by
`g_hydration`, whose decisive assertion is a `dispatch_click` on the server-sent button, not anything
the page script can report about itself. The identity claim is the load-bearing one: a framework that
re-created the node instead of adopting it would produce a byte-identical DOM while discarding the
server's work and every listener on it.

## Modern CSS surfaces — measured absent (tick 230)

subgrid, `@scope`, CSS anchor positioning, `attr()` as a typed length, and scroll-driven animations
are all **measured** missing, along with JSPI and the media pseudo-classes (`:muted`/`:playing`).
These are evidence now rather than assumptions: each was checked by the geometry or computed value it
should produce, not by whether the property parsed.

The practical read for daily-driver work: none of these blanks a page. They are progressive
enhancements — a site using anchor positioning falls back to its own positioning, `@scope` degrades
to ordinary specificity. That is a different class from the boot-critical holes, and they should be
weighed accordingly rather than by count.

## The popup login (`window.open` + `postMessage`) — and a forgeable origin

Gated in tick 231. This is the half of OAuth that never navigates the page: Google Identity Services,
Stripe Checkout, Auth0 `loginWithPopup`, GitHub's popup. Sites prefer it precisely because the opener
keeps its state — no reload, no lost form.

Two real bugs, both silent. `window.opener` was `null` during the popup's load-time scripts (identity
was seeded after they had run), so the popup posted nothing and the opener spun forever. And
`e.origin` carried the sender's own `targetOrigin` **argument** — so the origin check every one of
those SDKs performs could be defeated by passing the expected value, since the receiver has no other
way to learn who sent a message. Both fixed, in the shell's real load paths as well as the gate.

Residue: `targetOrigin` is still not enforced as a delivery restriction, and `window.close()` from the
popup is not modelled.

## Interactive frames — 3-D Secure, embedded OAuth, payment forms (tick 232)

An `<iframe>`'s pixels were a one-time snapshot: the child document stayed live and mutable, but
nothing repainted it, so the DOM changed and the screen did not. Every read from script came back
correct, which is why it survived — and it lands on exactly the content the web puts in frames
*because* it is interactive. A 3-D Secure challenge, an embedded OAuth consent screen, a payment form
or a CAPTCHA showed its first state forever, so the payment or login could never be completed and the
frame read to the user as frozen.

Frames now re-render when a script round mutates them, gated by `g_iframe_rerender` — which asserts
the frame's actual pixels change, since the DOM half already worked. Residue: a frame's own
timers/fetches do not yet trigger a repaint, and clicks are not routed into a frame (script can drive
the embedded form; a user cannot yet click it).

## Operating an embedded form — 3-D Secure "approve", OAuth "allow" (tick 233)

t232 made frames re-render; this makes them **usable**. Clicks are routed by document point into the
frame's own document and hit-tested there, so a user can press the bank's button rather than only a
script being able to change it. Nested frames recurse.

Gated by `g_iframe_click`, which asserts the child's document reaches `approved`, that the frame's
pixels changed, and — the negative — that a click *outside* the frame's box does not reach it.

Residue: keyboard/typing is still not routed into frames, a frame's own timers/fetches do not drive a
repaint, and a child's `body { background }` does not propagate to the frame's canvas.

## The adaptive-streaming append loop — every player that is not `<video src>` (tick 234)

Every site that matters for watching — YouTube, Twitch, Vimeo, and every player library (hls.js,
dash.js, Shaka, video.js) — never puts a media file in `src`. It constructs a `MediaSource`, waits
for `sourceopen`, `addSourceBuffer(mime)`, and then runs a loop: fetch a segment, `appendBuffer` it,
and on `updateend` **read `buffered` to decide what to fetch next**.

That last clause is the pattern, and it is why a complete byte pipe was still worth nothing. Ticks
223/227/228 built everything up to the append — the object graph, the attachment handshake, and a
byte-exact `arrayBuffer()` fetch with `Range` — and `sb.buffered.length` stayed `0` because nothing
demuxed the bytes. A player reading a `buffered` that never advances either re-fetches the same
segment forever or stalls, so no adaptive site progressed past its first segment regardless of how
correct the transport underneath was.

Since M3 the appended stream is demuxed (`engine/media`): `buffered` reports real presentation-time
ranges, and `videoTracks`/`audioTracks` carry the container's own codec strings and dimensions. The
loop can steer.

**What this does NOT unlock, deliberately: playback.** There is no decoder and no frame is produced.
`isTypeSupported` still answers `false`, which is what keeps YouTube serving its progressive fallback
rather than a stream we would accept and never render — `g_media_buffered` asserts that `false` so
the demuxer landing cannot quietly start over-promising. WebM/EBML is recognised and refused by name.

Residue: M4 (AAC via symphonia + cpal) and M5 (video decode); WebM demux; incremental rather than
whole-buffer parsing.

## Audio that is real numbers — AAC decode (tick 235)

The web's audio is AAC-in-MP4: every `<video>` with sound, every podcast player, every adaptive
stream's audio track. M3 could find it and name it and could not produce a sample of it.

AAC now decodes to PCM (`symphonia`, borrowed narrowly). What this unlocks is *not* audible playback
— there is no audio device yet, and `isTypeSupported` still answers `false` because a stream needs
video too. What it unlocks is the next step being a *device* step rather than another decode step,
and it is proven by length rather than by listening: decoded frames must equal the container's
declared duration to the sample.

Residue: M5 video decode; `cpal` output + A/V sync; MP3/Opus/Vorbis/FLAC/AC-3 (refused by name, not
silently accepted).

## Tick 240 — a decoded video frame reaches the SCREEN (MEDIA.md tick 1)

Ticks 234/235/236 built the media pipeline to within one step of the display — demux, AAC→PCM,
H.264→RGBA — and every one of them stopped at a value in memory. `decode_first_frame` returned a
correct picture that nothing could show. **A decoded frame that cannot be displayed is not video.**

**What this unlocks is a CLASS, and it is not "YouTube".** It is the hero video, the background
loop, the product demo, the GIF-replacement clip — `<video>` used as *moving decoration*, which is a
large fraction of all `<video>` on the open web and none of which needs MSE, ABR, a codec
negotiation or a clock. MEDIA.md ranks this the highest (web unlocked)/(effort) item in the whole
media plan, and the reason is that the browser already had every piece but the connection.

**The mechanism is deliberately three lines, and that is the finding.** A `<video>` was already a
replaced element, and `<video poster>` already decoded and painted through the identical route as
`<img>` — `Page::images` keyed by the video's own `NodeId`, blitted into the content box. So a frame
needs **no video path in the painter, no new display item, and no relayout**: it overwrites one map
entry. `Page::set_video_frame` takes raw RGBA rather than a `manuk_media::video::Frame`, which keeps
`manuk-page` decoder-agnostic and keeps `openh264`'s C toolchain out of the ~25 gate binaries that
link it.

**No relayout is a correctness property, not an optimisation.** A `<video>`'s box comes from its
attributes or CSS, never from the frame currently on screen — otherwise the page reflows on the first
frame and again whenever an adaptive stream changes resolution mid-playback, which is what adaptive
streaming does by design.

**The gate's baseline assertion found a real bug before the feature was written.** `decode_inline_images`
matched `<img>` only, while the async subresource pass matched `<img src>` *and* `<video poster>` — so
a **network** poster rendered and an **inline `data:`** poster silently did not, on `Page::load`, every
gate and the WPT runner. Two passes decoding the same elements for the same reason had drifted. Fixed
by making the inline pass choose its source attribute exactly as the async pass does.

Residue, stated rather than implied: nothing yet *drives* the frames — no decode thread, no clock, no
`play()`. This is one frame on demand, which is MEDIA.md tick 1, not tick 2. `isTypeSupported` is
unchanged and still answers `false`.

## Tick 242 — quirks mode is wired end-to-end (the long tail stops being mis-rendered)

**The class this unlocks is the pre-standards web**, which is not a nostalgia category: it is the
intranet app, the government form, the university department page, the vendor manual, the CMS template
nobody has touched since 2008 — documents with no doctype, which browsers render in quirks mode and we
were rendering in standards. Every unitless `width=`/`height=` in their inline styles was being
**dropped as invalid**, so their layouts collapsed to auto widths.

**It was a dead-end wire, not a missing feature, and that is the more dangerous shape.** html5ever
detected quirks correctly and stored the verdict in a field that was **written and never read**; every
Stylo call site hard-coded `NoQuirks`; and `document.compatMode` returned a constant `"CSS1Compat"`
behind a comment asserting *"our documents are never quirks-mode"*. The engine had the answer and threw
it away, which no capability probe would ever surface — the feature *appears* present at every layer
you inspect.

**The fix is a field on `Dom`, not a parameter.** Every consumer already receives a `Dom`, so the
verdict reaches Stylo, layout and JS with **no signature change anywhere** — including all 18
`cascade_styles` call sites. Threading it explicitly would have made the change too large to land in
one tick, which is the trade worth remembering: *a value every consumer already has a handle to should
ride on that handle.*

**Stylo already implements the quirks** — unitless lengths, case-insensitive id/class matching, the
`<font size>` table. `QuirksMode` is an input we were failing to supply, so this is plumbing rather
than layout math.

**Reporting and rendering shipped together, deliberately.** Flipping `compatMode` alone would have been
a worse lie than the constant: a site branching on it would take a quirks path we do not honour.

Residue: the ~9 `MatchingContext`/media-query `NoQuirks` sites in `stylo_engine.rs` still say standards,
so **case-insensitive id/class matching is not yet enabled** — a real quirk, deliberately left for a
follow-up rather than claimed. `LimitedQuirks` folds to `false` (it does not enable the unitless quirk).

## Tick 243 — quirks mode, completed: case-insensitive id/class, and the index that would have eaten it

Tick 242 wired the unitless-length quirk and named its own residue honestly: the `MatchingContext`
sites still said standards, so **case-insensitive id/class matching was not enabled**. This closes it,
and the closing turned out to contain the interesting part.

**`#FOO { }` must match `id="foo"` in quirks mode.** That is not a curiosity — it is how a large share
of hand-authored legacy markup was written, back when the id in the stylesheet and the id in the
markup being spelled differently was simply not a bug. The same documents that lack a doctype.

**THE HALF-FIX TRAP.** Flipping the matcher's constants is *not* the fix. This engine buckets rules in
its own `RuleIndex` by id/class before matching, as a cascade optimisation. With the index keyed by
exact case, `#FOO` files under `FOO`, the element queries `foo`, the bucket misses, and the rule is
**discarded before matching ever runs** — the change compiles, reads as complete, and does nothing.
Proven rather than reasoned: reverting only `index_key` makes the gate report 800px instead of 250px
while every `MatchingContext` already says `Quirks`.

**This is the second time this exact index has silently eaten rules** — the CSS-nesting bug was the
same structure dropping rules it never looked at. The rule worth keeping: **an index is a lossy copy of
the rule set, and every predicate added to the matcher must be reflected in the key**, or the index
pre-filters the very thing the matcher was just taught to accept.

Residue: `LimitedQuirks` still folds to `false` (it does not enable either quirk implemented here), and
the `<font size>` mapping table quirk is available from Stylo but unexercised by any gate.

## The page asks what state the browser is in — and gets answers that agree (tick 244)

**Pattern:** `if (document.hidden) return;` at the top of an animation loop, poll or heartbeat, and
`navigator.permissions.query({name:…})` cross-checked against the permission value the platform
already published elsewhere. Both are first-page-load code, both were absent, and both failed in the
direction that is hardest to notice.

**`document.hidden` was `undefined`, and `undefined` is falsy** — so the guard did not abstain, it
voted *"the tab is in front"*, permanently. Backgrounded tabs kept animating, polling and decoding:
the precise cost the Page Visibility API exists to prevent, produced by the API's own absence. Had
the spec spelled the property `visible` rather than `hidden`, the same absence would have frozen
every foreground tab and been fixed in a day. **The quiet direction is the one that survives.**

**`permissions.query` is a consistency surface, not a coverage one.** A caller usually already knows
the answer; it is asking whether our two answers match. The state for `notifications` is therefore
*read off* `Notification.permission` rather than written as a second literal — two constants in two
files agree only until someone edits one. Everything unimplemented answers `'denied'` and never
`'prompt'`, because `'prompt'` makes the page raise permission UI and wait for a decision nothing
here can deliver: a hang dressed as a feature.

**The class this unlocks** is every page that budgets its own work — which is now most of them.
Backgrounded-tab throttling, autoplay gating, poll suspension and reconnect-on-return all key off
these two surfaces, and a browser that cannot answer them is a browser that never rests.


## The hover-reveal navigation menu — a whole category of navigation, missing in silence (tick 245)

**Pattern:** `nav li:hover > ul { display: block }`. Top navigation built with **no JavaScript at
all** — structurally the same trick as the checkbox hack, and just as common. `:hover` was
hard-coded `false` in the cascade, so every one of those menus was permanently closed. The links
inside were unreachable to a user and invisible to an agent, and **nothing anywhere reported a
problem**: the page rendered exactly what it was told to render.

**`:hover` matches the hovered element AND ALL ITS ANCESTORS, and that half is the mechanism.**
Match only the exact hit target and the menu fails in a way that looks like it works: the pointer
enters the `<li>` and the submenu opens; the pointer moves one pixel into that submenu and is now
over an `<a>` inside the `<ul>`, so the `<li>` stops matching and the menu closes underneath the
cursor. The element whose style actually changes is the one the pointer is never over.

**The class this unlocks** is the desktop navigation bar — plus hover cards, tooltips, image-swap
affordances, and `:hover`-gated "reveal on hover" controls, which together are on a large fraction
of desktop pages. It is also an **agentic** unlock: a menu that never opens is a set of links no
agent can see, let alone click.

**The trap worth carrying forward:** a cascade *input* can change while the *tree* does not. Every
incremental path in this engine was built around tree mutation and asks "did the DOM change?" rather
than "did anything the cascade reads change?". State pseudo-classes are the first inputs that move
without the tree moving and will not be the last. Details in `docs/wiki/css-cascade.md`.



## The focus ring, and the search box that expands when you click into it (tick 246)

**Pattern:** `.searchbox:focus-within { height: 300px }` and `a:focus-visible { outline: 2px solid }`.
The first is how a large share of sites build an expanding search field or an open combobox panel —
the `<input>` takes focus, the wrapping `<div>` is what changes size. The second is, on many sites,
**the only focus cue that exists**, because authors spent twenty years writing
`:focus { outline: none }` to remove the ring mouse users did not want.

Focus never reached the cascade. The shell tracked it and published it to
`document.activeElement`, so it looked present at every layer anyone would inspect — but `:focus`
answered a hard-coded `false`, so those search boxes never expanded and tabbing through those pages
moved an invisible cursor.

**The class this unlocks is keyboard navigability**, which is both an accessibility floor and an
agentic one: an agent driving a page by keyboard has the same problem a human does if nothing
visibly or structurally marks where focus is.

**The distinction that must not collapse:** `:focus` is the exact element, `:focus-within` is it or
any ancestor, and `:focus-visible` adds *"and the ring is warranted"* — which is false for a
mouse-clicked button. Only the caller knows how focus arrived, so the engine takes that as an input
rather than guessing. Details in `docs/wiki/css-cascade.md`.

## The upload form, and the file that arrives as the string `C:\fakepath\a.txt` (tick 247)

**Pattern:** `<input type="file">` + `new FormData(form)` + `fetch(url, {method:'POST', body: fd})`.
This is how every avatar picker, attachment control, photo uploader and document dropzone on the web
submits — and it is first-interaction code on most sites that have accounts.

**It had no door.** Choosing a file is the one common interaction with no scriptable analogue: a
click is an event and typing is an event, but the bytes arrive through a native OS picker with no
scriptable surface. So the whole class was not broken — it was **unreachable**, and nothing reports a
missing door. `Page::set_input_files` is the entry point.

**And the encoder was already right.** `manuk-net::multipart` is real, tested and correct, and had
never once been handed a file: `new FormData(form)` harvested `e.value` for every control, and a file
input's `value` is the spec's deliberately-useless `C:\fakepath\a.txt`. **The bytes were dropped one
layer above the code that knew how to send them.**

**The failure direction is the point.** The page could see the file perfectly — `files.length`,
`name`, `size`, `type` all correct — while the server received the literal string where a JPEG should
be. **An upload that succeeds and delivers garbage is worse than one that fails**, and it is invisible
to any assertion of the form "the page can see the file". The gate therefore asserts the multipart
body carries the file's actual *bytes*, and the RED probe flips that claim **alone**.

**The class this unlocks** is every account-holding site an agent has to get a file into: profile
photos, support-ticket attachments, résumé uploads, CSV imports, image posts. Drag-and-drop upload is
still closed — `DataTransfer` remains inert — and that is the next door.

## The dashed rectangle: "drag files here", and the handler that threw (tick 248)

**Pattern:** a `<div>` with `ondragover="event.preventDefault()"` and an `ondrop` that reads
`e.dataTransfer.files`. Gmail attachments, GitHub issue images, Slack, Drive, and essentially every
uploader built in the last decade. On the modern web this is the *more* common upload path — the
`<input type=file>` is often only the fallback behind a "browse" link.

**The absence did not read as absence.** With `DataTransfer` inert, `e.dataTransfer` was `undefined`
and `e.dataTransfer.files` was a **TypeError inside the drop handler**. The page did not ignore the
drop and fall back — it threw, the dashed rectangle stayed lit, and the upload never started. **A
handler that throws leaves the UI actively lying**, which is worse than a feature that plainly does
nothing.

**The opt-in is the part that looks like ceremony and is not.** A dropzone that does not
`preventDefault()` its `dragover` **never receives a `drop`** — the page has to say it accepts drops.
So the interaction is a *pair* of handlers, and any host that fires `drop` alone is exercising a path
no real browser can reach.

**And the default action matters.** If the host performs its default after the page accepted the
drop, the browser **navigates to the dropped file** and replaces the app the user was uploading to —
the classic "my app vanished when I missed the drop target" bug.

**Still closed:** pointer-driven drag between elements (no `dragstart` from a draggable source, no
drag image), so drag-to-REORDER — sortable lists, Trello columns, editor blocks — does not work yet.

## Double-click to select, and the right-click menu the page draws itself (tick 251)

**Pattern:** two interactions an agent could not perform at all. **Double-click** — rename in a file
manager, select-a-word in an editor, open-a-row in a table, expand a Kanban card. **Right-click** —
every application-grade web app replaces the browser's native menu with its own: Google Drive, Figma,
VS Code for the Web, Notion, GitHub's file tree, any data grid.

**A double-click is a SEQUENCE, and dispatching the notification alone is the trap.** A real
double-click fires `click`, `click`, `dblclick` — and `event.detail` carries the click count. The
idiomatic handler on the web is `if (e.detail === 2)` on an *ordinary click listener*, used precisely
because it needs no second listener at all. So a host that fires `dblclick` by itself leaves that
branch permanently unreachable, and skips the two `click` handlers a real double-click always runs: a
page that selects on the first click and opens on the second **opens something it never selected**.
The two clicks are the interaction; `dblclick` is only the notification that it happened.

**This failed in the intermediate state and looked fine.** With the sequence correct but `detail`
absent, the gate read `clicks=2 dbl=1` — every handler firing, order perfect — while `e.detail` was
`undefined` and no page could tell the second click from the first.

**For right-click, the RETURN VALUE is the capability.** `contextmenu` is cancelable, and cancelling
it is *how* a custom menu works: the handler calls `preventDefault()` and draws its own. A browser
that ignored that verdict would render its native menu **on top of the page's own**, which is the
visible symptom users report as "the right-click menu is doubled". `button` is 2 and `buttons` is 4 —
one is an index, the other a bitmask, and they coincide often enough to hide a bug.

**Still closed:** `mousedown`/`mouseup` are not part of the click sequence, so a page tracking
press-then-release (drag handles, press-and-hold, custom sliders) sees neither. Native `<select>`
option-choice (`selectedIndex`) has zero implementation and is genuinely missing, not stale.

## The dropdown that opens on mousedown, not click (tick 252)

**Pattern:** a menu, combobox, drag handle, slider or press-and-hold control whose handler is
`mousedown` — used deliberately so the menu is up *before* the button comes back up. This is most
custom menus, every `<select>`-like widget built in JS, and the opening move of every drag library.

**The absence was total and silent.** `mousedown` and `mouseup` were dispatched nowhere in the
engine, so a page with a `mousedown` menu and no `click` listener simply never opened its menu.
Nothing threw. A host that fires only `click` looks like it is driving the page and is skipping the
event half the interactive web actually listens for.

**The truthful `buttons` mask is the subtle half.** `buttons` is a bitmask of the buttons *currently
held*, so it is 1 during `mousedown` and **0 during `mouseup`** — the press is over by then. It is
not derivable from `button` (an index) across the whole sequence, though the derived form is
accidentally right for `click`.

**And `preventDefault()` on `mousedown` does not cancel the click** — it suppresses focus and text
selection. Every rich-editor toolbar button depends on that pairing: prevent the press so the
document selection survives, then act on the click.

**Still closed:** Pointer Events (`pointerdown`/`pointermove`/`pointerup`) and `mousemove`, so drag
*gestures* — as opposed to the press that starts them — remain unreachable.

## The country picker that branched on an empty string (tick 253)

**Pattern:** `<select>` — country, currency, quantity, sort order, shipping method, language, every
settings page and every checkout. Pages read `select.value` and branch on it, and agents must be able
to choose an option through what is otherwise an OS-drawn popup with no scriptable surface.

**The failure was a DIVERGENCE, not an absence.** Form submission reads the DOM directly and was
correct, so the select submitted the right value — while `select.value` in script returned `""`,
`selectedIndex` returned `undefined`, and `options` did not exist. A page whose script branches on
the selection took the empty-string path every time, on a form that would have submitted fine. Two
paths to one question, and pages read the one that lied.

**A select with nothing marked is NOT a select with nothing selected.** A single-select showing no
`selected` attribute still shows and submits its **first** option; an explicit assignment of an
unmatched value must land on **-1**. The two states look identical in the markup, which is why the
spec carries a selectedness bit separate from the attribute.

**An option's value falls back to its text** (`<option>Blue</option>` is `"Blue"`), and options
inside `<optgroup>` still belong to the select — a children-only reading makes every grouped select
look empty.

**`input` before `change`, and both.** React's `onChange` is the `input` event, so firing only
`change` leaves React selects unchanged while vanilla pages work.

**Still closed:** `select.options`/`selectedOptions` (a live collection, so `s.options[i]` throws),
multi-select actuation, and `select.add`/`remove`.

## The script that enumerated its own dropdown and died (tick 254)

**Pattern:** `for (var i = 0; i < s.options.length; i++)` — relabelling, filtering, counting or
syncing a `<select>`'s options. Dependent dropdowns (country → state), search-filtered pickers, any
form that rebuilds one select from another.

**It did not read as empty — it THREW.** `select.options` did not exist, so `s.options.length` was a
TypeError and the whole script stopped at that line, taking with it everything the page had not yet
initialised. And the empty answer would have thrown too, one line later, at `s.options[0]`: **for a
collection, "reports nothing" and "throws" are usually the same bug a line apart**, because the
caller's next move is to index it.

**An untouched single-select still has a selected option**, so `selectedOptions` is not "the options
carrying a `selected` attribute" — that reports an empty collection for a perfectly ordinary
dropdown, and pages index straight into it.

**`option.index` counts across `<optgroup>`s.** Grouped dropdowns are the common case for long lists
(countries by continent, fonts by category), and a per-parent index makes every group after the
first address the wrong option.

**Still closed:** these are snapshot arrays, not live `HTMLOptionsCollection`s — `options.item()`,
`namedItem()`, `select.add()`/`remove()` are absent, and multi-select actuation has no entry point.

## The captions two people are speaking over each other in (tick 255)

**Pattern:** `<track kind="captions" src="...vtt">` — an accessibility requirement, a legal
requirement in many contexts, and how a large fraction of viewers watch video at all.

**Cues overlap, so "what is on screen now" is a LIST.** Two speakers captioned simultaneously, a
speaker label held across lines, a translation over an on-screen sign. Answering in the singular
drops the second speaker for the entire span where both are live — a wrong answer that looks like a
valid one.

**A strict parser fails SILENTLY, not loudly.** Hours are optional in a WebVTT timestamp
(`00:01.500` is the common form), and a parser demanding `HH:MM:SS` does not reject the file — it
skips every cue and returns an empty track. The video plays with no captions and nothing is logged.

**`NOTE` blocks are comments shaped exactly like cues**, and rendering one puts a translator's
private remark on screen. **Cue settings share the timestamp line** (`align:start position:50%`) and
are not caption text.

**Still closed:** nothing fetches `<track src>` yet and no cue is painted — this is the parser, not
the pipeline. Positioning settings are discarded, and inline cue markup (`<v Alice>`, `<i>`) is kept
as literal text.

## The player that added 900 cues to an object that held none (tick 256)

**Pattern:** `var t = video.addTextTrack('captions','English','en'); t.addCue(new VTTCue(...))` —
what hls.js, dash.js and every custom HLS player do, because segmented streams carry captions inside
the media segments rather than as a separate `.vtt` file.

**The stub reported success.** `addTextTrack` returned a plain object with empty `cues` and
`activeCues` arrays, so the player's caption pipeline ran to completion, added every cue, and
rendered nothing — with no error to notice. And `VTTCue` did not exist at all, so on the players that
construct cues first, the caption path died on a `ReferenceError` mid-initialisation.

**`mode` is the on/off switch, and its default is OFF.** A `TextTrack` starts `disabled` and a
disabled track has no active cues; players set `mode = 'showing'` as a separate deliberate step. An
implementation that ignores mode shows subtitles to a user who turned them off.

**`activeCues` is a LIST** — cues overlap whenever two people speak at once.

**Still closed:** no `cuechange` event, `<track src>` is not fetched, and the caption *parser* and
the caption *API* are both built but not yet connected to each other.

## The caption renderer that was never called (tick 257)

**Pattern:** `track.addEventListener('cuechange', function () { render(this.activeCues); })` — the
entire caption *display* loop, in hls.js, dash.js, video.js, Plyr and every hand-rolled player. None
of them poll `activeCues`; they all wait to be told.

**The class this unlocks:** any `<video>` whose captions are drawn by the page's own JavaScript —
which is every adaptive-streaming site, because segmented streams carry captions inside the media
segments and the player owns the overlay. Before this tick the cues were parsed correctly, held
correctly, and reported correctly to a question nobody asked. The renderer was never invoked, so the
caption area stayed empty for the whole video with nothing in the DOM or console to see.

**`currentTime` is the clock, not a number.** A media element that stores the time and tells nobody
cannot fire this event at all — the only thing that knows a cue boundary was crossed is the write
that moved past it. Same for `mode`: turning captions on is a state change, and with a long cue
already under the playhead there is no later moment for the renderer to learn about it.

**Fire on CHANGE, never on every write.** Players write `currentTime` every frame. And compare the
active sets by cue IDENTITY, not by length: seeking from one single-cue line straight to another (a
transcript click — the common case) leaves both sets at length 1, and a length comparison reports
no-change while the viewer sits on the previous caption.

**Still closed:** `<track src>` is not fetched, so the caption *parser* and the caption *API* remain
unconnected; cue positioning settings are inert; the UA paints no cue of its own (a page that relies
on native caption rendering rather than its own overlay still shows nothing); no per-cue
`enter`/`exit` events.

## The caption file nobody fetched (tick 259)

**Pattern:** `<video><track kind="subtitles" srclang="en" src="/captions.vtt" default></video>` — the
plain-HTML way captions ship, with no player library and often no JavaScript touching the video at
all. News clips, course videos, conference talks, documentation screencasts, `<video>` in a wiki
article.

**The class this unlocks:** any video whose captions are a *file* rather than something a player
parses out of the media segments. Ticks 255–257 built the parser, the `TextTrack` and the timeline —
all three reachable only through `new VTTCue`, i.e. only by hls.js and dash.js. The file half was
never requested by anything, so `video.textTracks` was empty on every one of these pages.

**`default` is the whole on-switch.** There is no script to set `mode` and no captions button in our
chrome, so a `<track default>` whose attribute is ignored parses correctly, holds every cue, reports
`mode=disabled` and renders nothing — passing every other check while delivering zero.

**Load from the DOCUMENT, not from element reflection.** The tempting hook fires when the page's JS
touches the video; these pages never touch it.

**Two things still closed, and they bound the claim.** A page with *no* `<script>` at all gets no JS
context, so its `<track>` never loads — measured, not assumed. And nothing paints a cue: the track is
loaded, `showing`, holding the right cues at the right times, and a viewer of a plain `<video>` still
sees no text, because the UA has no caption overlay of its own.

## The caption placed exactly where the author said not to (tick 260)

**Pattern:** `00:00:06.000 --> 00:00:08.000 line:0 align:start position:10%` — the settings run on a
cue's timestamp line, which every real caption file uses and tick 255 discarded.

**The class this unlocks:** any video whose frame is already busy at the bottom — sports with a
scoreboard, news with a lower third, foreign films with burned-in subtitles, interviews where the
speaker's mouth is where the text would go. Authors write `line:0` precisely to move the caption
away, so dropping the setting puts every cue in the one position they were avoiding.

**`auto` is not `0`.** `line:0` is the top of the frame; `auto` is the bottom. A parser that collapses
auto to 0 moves every default caption in every file to the top.

**A bare `line` number is a line COUNT, not a percentage.** `line:0` reads correctly either way — which
is what lets the bug through — but `line:-1` means the last line, i.e. the bottom.

**Still closed:** nothing paints a cue. The placement is now correct and complete for a page's own
overlay to consume; the UA still has no caption renderer of its own.

## The caption the browser draws itself (tick 261)

**Pattern:** `<video src="..."><track kind="subtitles" src="cc.vtt" default></video>` — and no player
library anywhere on the page.

**The class this unlocks:** every video that is just a `<video>` tag. Course and lecture pages, museum
and archive collections, government and public-health videos, conference talk recordings, product
demos and support clips, news embeds, university department pages — the whole non-YouTube long tail
that ships a file and a caption track and lets the browser handle it. Also every accessibility and
compliance context where captions are the *point*: a deaf or hard-of-hearing viewer, a muted autoplay
feed, a noisy room, a second-language viewer. For those users a video with no visible captions is not
degraded, it is unusable.

**The failure this closes is the one every layer passes.** Six ticks each parsed, held, timed, fetched
and placed cues correctly, and each handed them to *a page's own renderer*. Every gate was green. The
last handoff was to nobody, because a page with no player library has no renderer — and the browser
is the one that is supposed to draw it. Correct data delivered to an absent consumer looks identical,
from every layer above, to a working feature.

**`hidden` is not `disabled`, and it is what stops double captions.** A player that draws its own
overlay sets `mode='hidden'`: the cues stay live and `cuechange` keeps firing so its renderer works,
but the UA must not paint. A UA that paints hidden tracks double-captions every site that has a
player — the sites most likely to be tested.

**Still open, and it is fidelity rather than absence:** vertical (`rl`/`lr`) cues paint horizontally;
text width is estimated rather than shaped, so `align:end` and `size:` clipping are approximate; two
simultaneous cues at the same explicit `line` overlap.

## The video actually plays — and the browser stops denying it (ticks 262-264)

**Pattern:** `<video src="clip.mp4">`, and the feature-detect that guards it:
`if (v.canPlayType('video/mp4; codecs="avc1.42E01E"')) { showPlayer(); } else { showFallback(); }`

**The class this unlocks:** the same non-YouTube long tail the caption work served, but the *picture*
rather than the text — course and lecture recordings, conference talks, archive and museum clips,
product demos, support videos, government and public-health media, news embeds. Anywhere a page ships
a file and expects the browser to play it. Three ticks in sequence: the browser had to **ask** for the
movie (262 — `<video src>` was never fetched at all, only `poster` was), **drive** it (263 — the
shell had no media handling, so decoded frames reached nothing), and then **admit** it (264).

**The failure this closes, and it is a distinct one from the previous three.** 261/262/263 each closed
a variant of *built, correct, connected to nothing*. This one is different and worth naming
separately: **the capability worked and the browser was still announcing that it did not.**
`canPlayType` answered `''` for everything and `play()` returned a rejected promise — both scrupulously
honest while nothing could decode, and both lies the instant playback landed. A site that politely
feature-detects was told no about something that works, hid its player, and showed
"your browser cannot play this" over a video that would have run.

**An honest answer is not a fixed answer.** A capability stub hard-coding "no" is correct exactly as
long as the capability is absent, and it is the only place in the tree that knows when that stops
being true. Nothing fails when it goes stale — no test reddens, the browser simply under-reports
itself forever. The gate that asserted `canPlayType === ''` was *pinning the limitation in place*.

**Two questions that look alike and have different answers.** `canPlayType` asks about `<video src>`,
which now works. `MediaSource.isTypeSupported` asks about MSE, where `appendBuffer` accepts segments
that nothing drives into a decoder — so it still answers `false`, correctly, because an adaptive
player told "yes" would append forever against a stall. That is strictly worse than being told no.

**Still open:** whole-file buffering (no `Range` requests, so a feature-length file is an OOM);
autoplay is unconditional until controls land; no audio device (`cpal` unbound); High-profile H.264
and VP9/AV1 do not decode, and are refused up front rather than failed mid-stream; and `el.error`
still reports `MEDIA_ERR_SRC_NOT_SUPPORTED` eagerly, which the next tick fixes with a shell→JS bridge
carrying the real decode outcome.

## The video that cannot play says so — and the one that can stops saying it (tick 265)

**Pattern:** `<video src="clip.mp4" onerror="showFallback()">`, and its polling twin
`if (v.error) { showFallback(); } else { showPlayer(); }` — the cheapest capability test on the web,
and the one nearly every player runs before anything else.

**The class this unlocks:** both halves of the same long tail. Sites serving a codec we genuinely
cannot decode (WebM/VP9, High-profile H.264, a 404 on the media file) get their fallback back — the
"download the file" link, the "your browser cannot play this" notice, the alternate `<source>`. Sites
serving Constrained-Baseline MP4 stop being told to show one over a video that plays.

**The failure this closes, and why it took its own tick.** `el.error` was eagerly
`MEDIA_ERR_SRC_NOT_SUPPORTED` on every media element. That was the right signal while nothing could
decode and became a contradiction the moment tick 264 had `canPlayType` answer `'probably'`. But the
obvious fix — default it to the spec-initial `null` — is *also* wrong, just in the other direction: a
`<video src="x.webm">` we truly cannot play would report no error at all, and every player reads that
as **still loading**, so the fallback never fires and the user gets a dead rectangle forever.

**Neither fixed value is honest, and that is the general shape.** When a capability becomes *partial*
— some inputs work, some do not — no constant is a truthful answer any more. The default has to stop
being a guess and start being a **report**, which means finding the layer that actually knows. Here
that is the shell: it fetched the bytes and it knows whether they decoded. It was already recording
exactly that and simply never told the page.

**A 404 is a failure, not a silence.** A media fetch that fails now reports as a failed decode rather
than being dropped. Dropping it leaves the element at `error === null`, which is indistinguishable
from "still loading" — so a missing video file hangs the very fallback a missing file should trigger.

**Still open:** `MediaSource.isTypeSupported` remains `false` for everything and remains correct — it
answers for MSE, where appended segments feed no decoder, and a "yes" would wedge every adaptive
player. `readyState` jumps to HAVE_ENOUGH_DATA rather than climbing through HAVE_METADATA, which is
honest for a whole-file fetch and becomes wrong when ranged fetching lands.

## The carousel stops on a slide (tick 266)

**Pattern:** `<div style="overflow-y:scroll; scroll-snap-type: y mandatory">` with children carrying
`scroll-snap-align: start` — plus the feature-detect `if (getComputedStyle(el).scrollSnapType)` that
a carousel library runs before deciding to load its own JS fallback.

**The class this unlocks:** paged feeds, story trays, image galleries, onboarding walkthroughs,
full-page scroll-jacking marketing sites, and the card rows every mobile-first layout is built from.
Without snapping a flick lands wherever momentum stopped — two half-slides on screen and neither
readable — and the page looks broken in a way no capability count can see, because the scroll
container itself "works" perfectly.

**Where the bug would have been.** Snapping is one transformation at one chokepoint, and the ordering
against the clamp is the whole correctness question: snap first and a point past the scrollable range
gets chosen and then clamped back to an unaligned offset, so **the container can never reach its own
last slide.** That is the classic carousel bug, it is invisible unless a test scrolls all the way to
the end, and it looks like a content problem rather than a scrolling one.

**A declared property must not break the undeclared case.** A container with `scroll-snap-type` but
no aligned children has an empty candidate set, and "snap to the nearest of nothing" degrades to
pinning it at zero — turning an unused declaration into a scroller that cannot scroll at all.

**Still open, and it is the bigger half:** only the **vertical** axis actually works. An inline-block
row yields no horizontal scroll range in layout today, so `overflow-x: scroll` does not scroll —
which means **horizontal carousels, the commonest kind, do not scroll at all, let alone snap.** That
is a scroll-geometry gap rather than a snap gap, and it is the next lever here.

## The nav bar that is one line, not three (tick 267)

**Pattern:** `<nav style="white-space:nowrap; overflow-x:auto">` wrapping a row of
`display:inline-block` children — tabs, chips, breadcrumbs, toolbar buttons, carousel slides. The
declaration says "this row is ONE line; let the container scroll it".

**The class this unlocks:** nav bars, tab strips, filter/chip rows, breadcrumb trails, toolbars and
horizontal image carousels — the entire pre-flexbox idiom for a horizontal row, still load-bearing on
a large slice of the current web (and the fallback markup most frameworks still emit).

**The bug was NOT the one tick 266 wrote down.** That entry above blames *scroll geometry* ("an
inline-block row yields no horizontal scroll range"). Measuring four container shapes instead of
theorising from one: `display:flex` rows and wide block children **already** reported `scrollWidth`
correctly, and `nowrap` **already** worked for plain text. Horizontal scroll geometry was fine.
`white-space: nowrap` was broken for **exactly one token type** — the atomic inline box.

**Where the bug was.** An inline formatting context is a run of tokens, and an `inline-block` is a
token in it exactly like a word. The line breaker suppresses a break only when both sides are nowrap
(the break opportunity belongs to both). The word path read `white-space` off the inherited style;
the atomic path passed a hardcoded `false`, so an inline-block permanently advertised itself as a
legal break point.

**What it looked like on the page — and why no metric saw it.** Not "the carousel doesn't scroll".
The row **silently wrapped into a stack**: five 100px tabs in a 200px bar became three rows, the bar
grew to 3× its declared height and shoved the rest of the page down, and only *then* did
`scrollWidth == clientWidth` — so nothing scrolled, correctly, given the wrapped layout. Every number
the engine reported was self-consistent with a layout that was wrong, which is why the symptom
pointed at the scroll container and why capability coverage could not see it.

**The control is the real assertion.** The same row *without* `nowrap` must still wrap. That is what
separates "honours `white-space`" from "never breaks inline-blocks" — a blanket disable makes the
headline assertion greener while turning every ordinary inline-block gallery into one infinite line.

## The article that sits 45px too high (tick 268)

**Pattern:** any document that uses the block elements HTML has had since 1995 — `<ul>`/`<ol>` menus
and bullet lists, `<dl>` definition lists, `<pre>` code blocks, `<hr>` rules, `<figure>` images,
`<blockquote>` quotes — and relies on the browser's own margins to space them.

**The class this unlocks:** every content page on the web, and the placement half of the Phase-0 exit
gate specifically. Wikipedia, usa.gov, old.reddit, airbnb — the FID-SWEEP's whole NEAR-MISS
population, where `mdx=0` and `mdy` is 12–82px.

**Why no capability count could see it.** Coverage was 85.9%: we were rendering these elements, all
of them, with correct `display`, correct text and correct horizontal placement. Only their *vertical
spacing* was absent, so every element existed and every one of them was in the wrong place — and each
missing 32px pushed everything below it up by 32px, so the error accumulated with content density.
A feature checklist marks this page as fully working.

**Where the bug was:** two cascades. `apply_ua_defaults` had `ul { margin: 1em 0 }`; the Stylo
`UA_CSS` sheet, which is the live path for every real page, did not. The stale source of truth was
the one that runs.

**The trap in fixing it:** a nested list has NO vertical margin in Chrome. Adding `1em` to every list
fixes the top-level case and newly over-spaces every nested menu and sidebar — including Wikipedia's,
which is where the divergence was measured. The fix and the trade are one selector apart.

## Every page with more than a screenful of text (tick 269)

**Pattern:** prose. Any element positioned below a block of wrapped text — which is every article,
every docs page, every feed, every sidebar that follows a paragraph.

**The class this unlocks:** the placement half of the Phase-0 exit gate, on the sites where the
error grows with content: wikipedia (`mdy=45`), usa.gov (82), airbnb (20), old.reddit (12).

**Why no capability count could see it.** There is no feature here. Font selection was right (both
engines resolve `sans-serif` to Liberation Sans), shaping was right, advance widths were right. The
line box was 18.398px where Chrome's is 18 — **0.4px, on every line box on the page.** One line looks
perfect; a hundred lines is 45px of accumulated drift landing on every element below the text.

**The general shape, worth naming:** a per-instance sub-pixel error is invisible in every local test
and unbounded in the aggregate. It cannot be found by looking at one element, only by measuring a
STACK of them against the reference — which is why the 6-line paragraph in the gate is 6 lines and
not 1.

**The trap:** the rule is `round(ascent + descent + gap)`. Rounding each term separately is equally
plausible, agrees with Chrome on DejaVu and Noto, and is wrong on Liberation — the face we ship. A
gate built on the wrong face passes the broken implementation.

## Icon-plus-label buttons and text-only flex boxes (tick 270)

**Pattern:** `display:flex` on an element whose content is (or ends in) a bare text run —
`<a class="nav-link" style="display:flex">Recent changes</a>`, `<button><svg/>Save</button>`,
`<div class="chip">Draft</div>`. This is the default shape of every nav item, toolbar button, badge,
menu row and tab in every component library shipped in the last decade; a flex box with an icon
element and an unwrapped label beside it is the single most common small layout on the modern web.

**The class this unlocks:** those labels existing at all. The text was not misplaced — it generated
**no box**, so it never sized its container and never took part in layout.

**Why it reads as a vertical bug.** The container shrink-to-fits to whatever content survives, which
is the icon or the longest remaining word, so the visible symptom is not a missing label but a
**wrapped** one: every row silently doubles in height and everything below it drifts down. The
fidelity sweep reports that as `mdx=0 mdy=N` — "vertical drift" — and a median offset cannot say that
the cause is a width. Per-element boxes could, and did.

**The trap, and it cost half the tick:** the obvious fix (use the text node as the item, read its
style) PASSED the unit gate while the live page was unchanged, because the two cascades store
different things on a text node — `inherit_from(parent)` vs a full clone of the parent's style. A
gate that exercises one cascade cannot see a divergence between two.

## Inline text elements — links, emphasis, code, badges (tick 271)

**Pattern:** any non-replaced inline element inside flowing text — `<a>`, `<span>`, `<em>`,
`<strong>`, `<code>`, `<label>` — on a page that sets `line-height`. Which is essentially every page:
`line-height: 1.5`/`1.6` is the default in Tailwind, Bootstrap, every CSS reset and every design
system shipped this decade.

**The class this unlocks:** the *geometry* of inline content — everything that asks an inline element
where it is. `getBoundingClientRect()` on a link, hit-testing a click near a link's edge, the a11y
tree's bounding boxes, tooltip and popover anchoring, a sticky highlight, an underline overlay, and
the fidelity probe itself. We reported the **line box** for all of them: on a 16px/1.6 paragraph an
`<a>` came back 25.6px tall starting at the line top, where Chrome says 17px tall starting 4px lower.
Wrong in both coordinates, on every inline element on the page.

**Why it hid for 270 ticks.** Nothing *looks* wrong: the glyphs paint from the baseline, which was
right, so the page renders correctly and only the reported box is off. It is invisible to any test
that renders one element and looks at it, and invisible to a median — it shows up only as a small
constant `dh` repeated across hundreds of elements, which is exactly what the sweep had been printing
(`dw=0 dh=+7` on dozens of wikipedia rows) and what three consecutive ticks read as "vertical drift".

**The trap:** the content area's rounding rule is `round(ascent) + round(descent)` — the *opposite*
of tick 269's line-box rule, which rounds the sum. Inheriting 269's conclusion here is the natural
mistake and it is wrong by 1px at most sizes. Only a sweep across sizes forecloses it: Liberation Sans
is 16px tall at font-size 14 and 17px at font-size 16, and no ratio can do that.

**The second trap, which broke a different gate:** inline padding/border spacers are textless,
fontless synthetic fragments that carried their height in `style.line_height` *because that is what
`rect()` read*. Changing what `rect()` means deleted them from the layout's element-geometry map
entirely — a **coverage** regression caused by a **placement** fix, caught only because the wall runs
a gate the tick was not aiming at.

## Closed menus, popovers and tooltips — `visibility:hidden` overlays (tick 272)

**Pattern:** `position:absolute; visibility:hidden` on a panel that is laid out at full size and
revealed by toggling `visibility` — the standard way every dropdown, popover, menu, tooltip and
autocomplete list is hidden, because unlike `display:none` it keeps the box and reveals without a
reflow.

**The class this unlocks:** clicking anything a closed menu happens to sit over. We hit-tested
invisible panels, so a link the user can see and aim at resolved to the menu on top of it — and
because those panels are permanently laid out, this was not a transient state but the page's normal
condition.

**Why it hid:** the panel is invisible, so nothing looks wrong, and the failure only manifests as a
click landing on the wrong element. The engine had two separate notions of "hidden" — the `hidden` /
`aria-hidden` *attributes* (which the a11y builder checked) and CSS `visibility` (which it never
saw, because it was given the DOM and not the cascade).

**The trap:** `visibility` is the one hiding mechanism a descendant can undo — `visibility:visible`
inside a hidden ancestor is shown, and is in Chrome's accessibility tree. Pruning the subtree is the
obvious implementation and silently deletes those nodes. Drop the node, keep walking.

## Responsive blocks — anything inside `@media` (tick 273)

**Pattern:** a declaration inside `@media`. Not a niche one: the breakpoint block is how the entire
web ships layout, and `@media (prefers-color-scheme: dark)` / `@media print` are how it ships themes
and print styles. The page pipeline itself wraps every conditional `<link media="…">` sheet in
`@media … { }` so the cascade decides whether it applies.

**The class this unlocks:** every responsive site, correctly, for a dozen properties that were
silently exempt. `visibility` inside a breakpoint block is how the web hides closed dropdowns,
popovers, tooltips and autocomplete panels — so tick 272's fix for those had nothing to act on,
because nothing was ever marked hidden. Alongside it: responsive `background-image` swaps, gradient
heroes, icon masks, `border-style` dividers, `object-fit` thumbnails and dark-theme sheets.

**Why it hid:** the shipping cascade is Stylo, and Stylo re-parses the sheet source with its own
parser and evaluates `@media` correctly. `display`, `width`, `color` — everything a `@media` test
naturally reaches for — worked. Only the twelve properties `cascade_via_stylo` recovers from a
second `MinimalCascade` pass (because Stylo's servo build does not expose them) inherited that
parser's at-rule skip. **A property recovered from a second engine inherits that engine's bugs,
silently and only for that property**, so a green `@media` test and a total `@media` failure sat in
the same repository, both honest.

**The trap:** "descend into `@media` and apply what's inside" is not less wrong than skipping it. It
renders `@media print` on screen and a dark-scheme sheet on a light display. Descent is only half of
the fix; the query still has to be evaluated, and an unknown media feature has to evaluate FALSE.
Gate both directions or the fix is a different bug.

**Measured on landing:** the live Wikipedia Terrier page, `.vector-dropdown-content`, 0/8 panels
hidden before and 8/8 after — matching Chrome and matching the un-`@media`-wrapped control exactly.
23 links on that page are inside a `visibility:hidden` subtree once the rule applies; Chrome, asked
directly over CDP, counts 25 in the same state and cannot hit-test any of them either.

## Anchored panels — dropdowns, popovers, menus, tooltips (tick 274)

**Pattern:** `position:absolute; width:max-content` on a panel anchored to a small
`position:relative` trigger. Every dropdown menu, every popover, every tooltip, every autocomplete
list, every context menu — the panel must be as wide as its own longest row, and must not be
constrained by the 20px icon button it hangs off.

**The class this unlocks:** anchored panels being the right width. We sized them to the *anchor*,
because the absolutely-positioned width path had no arm for intrinsic sizing keywords and fell
through to shrink-to-fit against the containing block. A panel came out at roughly half width with
every row wrapped to two lines.

**Why it hid:** the panel is present, styled and full of the right content — it is just narrow. No
coverage gate can see it, nothing is missing, no crash, no error. And because wrapped rows are
taller, the visible consequence is *vertical*: a fidelity sweep reports `mdx=0, mdy=45`, which reads
as vertical drift and sends the next tick after the wrong organ.

**The trap:** a repro that reproduces the sizing CSS faithfully but omits `position:absolute` scores
100% Chrome-exact and proves the engine is fine. Keep a `position:static` sibling in the same file —
the control is what localises the bug to what `position:absolute` does to `max-content`, rather than
to `max-content` itself.

## `matchMedia` + the stylesheet, deciding the same thing (tick 275)

**Pattern:** a component reads `matchMedia('(max-width: 700px)').matches` to decide whether to mount
the mobile tree, while the stylesheet decides the layout with the identical breakpoint. Every
responsive framework ships this shape — it is how a drawer, a nav, a data table and a chart pick
between two renderings.

**The class this unlocks:** responsive JS branches that agree with the rendered layout. We had two
media-query evaluators with opposite unknown-feature defaults (`true` in the JS prelude, `false` in
the cascade), so every feature the prelude's table omitted was a guaranteed disagreement, and
`not` / `only` / range syntax were unparseable on the JS side.

**Why it hid:** nothing throws. The page renders a combination no designer specified — the desktop
grid holding the mobile component, a drawer open in JS and off-screen in CSS — and both halves look
individually reasonable.

**The trap:** testing `matchMedia` for the *right answer* tests `min-width`/`max-width`, which is
exactly the half a hand-written second evaluator gets right. Test that the two sides give the *same*
answer, over features nobody thought to put in the second table.

## Feature-detected CSS — `@supports` and `@layer` (tick 276)

**Pattern:** `@supports (display: grid) { … }` shipping an enhancement beside a fallback, and
`@layer base, components, utilities` ordering a design system's cascade. Between them they wrap a
large share of every modern stylesheet — and a theme's gradients, icon masks, dividers and shadows
usually live inside one of them.

**The class this unlocks:** feature-detected and layered CSS, for the twelve properties this cascade
owns on the shipping path. Both at-rules were deleted wholesale at parse time.

**Why it hid:** the same reason `@media` hid — Stylo evaluates both correctly for every property it
exposes, so `display`, `width` and `color` were fine inside them, and only the twelve recovered
properties were exempt.

**The trap:** answering `@supports` from a hand-maintained list of property names. That list is a
second source of truth and goes stale the moment a property is implemented. Answer it by *trying*
the declaration — apply it to an initial style and see whether anything moved. And `@supports` must
be able to say **no**: the author wrote the fallback for exactly that case, so a "descend into
everything" implementation applies both branches, which is worse than applying neither.

## Horizontal rails — carousels, poster rows, tab strips (tick 277)

**Pattern:** a fixed-width container holding a row that is wider than it is —
`white-space: nowrap` over `inline-block`s, or `display: flex` with `flex-shrink: 0` on the items.
Product carousels, poster rows, chip bars and scrollable tab strips are all this shape.

**The class this unlocks:** nothing — it already worked. This entry exists because the *board* said
otherwise, and the measurement is the deliverable: verified against headless Chrome across five
shapes, we agree exactly, and the behaviour is now gated with Chrome's numbers.

**The trap:** `display: flex` with default shrink reports no scroll range, and that is the correct
answer, not the bug. Flex items shrink by default and `min-width: auto` floors them only at
min-content. Filing that as "rails don't scroll" is what kept a working capability on the hole list.

## Client-side structured storage — IndexedDB (tick 278)

**Pattern:** `indexedDB.open(name, version)` → `onupgradeneeded` → `createObjectStore` →
`transaction(…, 'readwrite')` → `put`/`get`/`getAll`/cursor. Offline-capable apps, big client-side
caches and every wrapper library (idb, Dexie, localForage) are this shape. The AWS and GCP consoles
hard-fail without it.

**The class this unlocks:** the offline/cached app web — apps that keep structured state across
reloads rather than refetching it, and the wrapper libraries that sit on top. It was absent
entirely.

**Why it hid:** absence of `indexedDB` is a *boot* condition, not a runtime error. Apps
feature-detect it and take a degraded or dead path silently — the same shape as the MediaWiki
`localStorage` grading. Nothing throws, so nothing points at the cause.

**The trap:** two of them, and both were caught by probing rather than by reasoning. **(1)** Keys
must sort, so numeric keys need padding — unpadded, `2 < 9 < 10` comes back as `10, 2, 9` and every
ordered read is wrong. **(2)** A rollback claim asserted against a *failed* `add()` measures
nothing: the rejected write never wrote, so there is nothing to roll back and the assertion passes
with the undo log deleted. Assert rollback against a write that **succeeds** and is aborted from
inside its own success handler. The unit that needs a proven RED is the CLAIM, not the gate.

## Query by a value property — IndexedDB indexes (tick 329)

**Pattern:** `store.createIndex('by_email', 'email', {unique:true})` in `onupgradeneeded`, then
`store.index('by_email').get(addr)` / `.getAll(IDBKeyRange.bound(lo,hi))` / `.openCursor()` on every
read after. This is how you look a record up by a field that is NOT its primary key, and it is the
foundation the auth SDKs stand on: the Firebase and Cognito session layers, Dexie, and the `idb`
wrapper all `createIndex` at boot and query by it. Compound and `multiEntry` indexes back tag/facet
lookups.

**The class this unlocks:** the *logged-in* app web on top of the base store (tick 278). An app that
can only fetch by primary key cannot answer "the user whose email is X" or "everything tagged Y"
without scanning — so the SDKs simply require the index and do not degrade gracefully without it.

**Why it hid:** `store.index` was `undefined` and `store.indexNames` permanently empty, so the SDK's
own call `undefined.get(...)` throws **inside its promise chain**, the app "just doesn't load", and
nothing the page surfaces names the cause — the same boot-grading shape as `indexedDB` itself before
tick 278. `if (!store.index)` is not a check any app writes; it assumes the index exists.

**The trap, and it decides where the code lives:** an index must survive a **reopen at the same
version**, where no `versionchange` fires and `createIndex` therefore never re-runs. So the index
metadata cannot live only in the JS shim — it is persisted with the store in `manuk_net::idb`
(`ObjectStore.indexes`), serialized out on `open` and re-applied on every `upgrade`. A shim that held
indexes in a JS map would pass a single-session gate and break every returning visit. The gate proves
this the hard way: it opens, indexes, closes the connection, reopens **without an upgrade**, and
requires `store.index(...)` to still resolve records. Sort order is the store's own encoded-key order,
so an index's "between" and the store's "in order" can never disagree.

## The offline asset store — the Cache API (tick 279)

**Pattern:** `caches.open('shell-v1')` → `cache.put(url, response)` at install, then
`caches.match(request).then(r => r || fetch(request))` on every navigation after. Every PWA, every
"works on a plane" app, and every Service Worker's `fetch` handler is this shape.

**The class this unlocks:** the offline asset web — pages that keep their own shell, fonts and
scripts rather than refetching them. It was absent entirely, and `Response` was not constructible,
so nothing could have been put into a cache even if it had existed.

**Why it hid:** the same grading shape as `localStorage` and IndexedDB before it. `if ('caches' in
window)` does not report a bug; it silently selects the network-only path. Nothing throws.

**The traps, three of them.** **(1)** Bodies are not text. A cache holds fonts and wasm, and storing
them through a UTF-8 `text()` inflates every byte above `0x7F` into two — six bytes came back as
nine. Store one char per byte. **(2)** A miss must resolve `undefined`, never reject: the universal
handler is `r => r || fetch(...)`, so a rejecting `match` converts the whole offline path into an
unhandled rejection. **(3)** `put` replaces on (url, method, `Vary`) rather than appending — with
append semantics a PWA's re-install grows the cache without bound *and serves the stale first copy
forever*, which looks like a caching bug in the site rather than in the browser.

**And the one that generalises:** `typeof Response === 'function'` was **true** while `new
Response('x')` produced an object with no `status` and no `clone()`. An inert name on an
interface-surface list satisfies feature detection and fails at first use, somewhere else entirely.

## Work that happens off the main thread — Web Workers (tick 280)

**Pattern:** `const w = new Worker(URL.createObjectURL(new Blob([src])))` — or a bundler's
`new Worker(new URL('./w.js', import.meta.url))` — then `w.postMessage(job)` and
`w.onmessage = e => render(e.data)`. Markdown renderers, syntax highlighters, diff views, search
indexers, spreadsheet recalc, image decode, PDF rendering and every "parse this 40MB file" widget
are this shape.

**The class this unlocks:** the app web that does its real work in a worker. Not "the UI stays
smooth" — **the result arrives at all**. Previously the constructor fired `error` on the next turn,
which is the honest shape of a 404'd worker script, and a page's `onerror` path almost always
surfaces the failure rather than redoing the job inline. So the observable symptom was never an
error message: it was a **spinner that never resolves**.

**Why it hid:** `typeof Worker === 'function'` was true, and the constructor did not throw. Feature
detection passed cleanly and the failure arrived one turn later, on a path the page treats as fatal.

**The traps, three of them.** **(1)** The worker scope must **deny** the DOM explicitly, not merely
fail to provide it. `typeof document === 'undefined'` is how nearly every isomorphic module picks
which half of itself to run, so a scope that lets `document` fall through does not fail loudly — it
makes that choice *wrong*, then lets the main-thread branch touch a DOM that must not be there. This
is measurable: with the deny-list removed the compute still returns the right answer while all three
scope claims flip. **(2)** The structured clone is taken at **post** time, not at delivery. Cloning
late passes every round-trip assertion and still lets a page's next-line mutation reach the worker,
so the two sides share state the spec says they do not. **(3)** Messages posted between
`new Worker(...)` and the end of script evaluation must be **queued**, not dropped — posting the job
on the very next line is the normal shape, not an author error.

**And the one that generalises:** an honest failure is still a failure. The old stub was *correct* —
it reported exactly what a browser reports when a worker script cannot load — and it left an entire
class of the web unusable. "We report this accurately" is not a resting place; it is a description
of a hole, and the hole is what the checklist should count.

## The offline shell — Service Workers (tick 281)

**Pattern:** `navigator.serviceWorker.register('/sw.js')`, then in the worker
`self.addEventListener('install', e => e.waitUntil(caches.open(V).then(c => c.addAll(SHELL))))` and
`self.addEventListener('fetch', e => e.respondWith(caches.match(e.request).then(r => r || fetch(e.request))))`.
Every PWA, every docs site with an offline mode, every "instant on repeat visit" app, and most
production React/Vue/Next deployments ship exactly this file.

**The class this unlocks:** the installable web — and, more often than "offline", **first render**.
A growing number of sites await `navigator.serviceWorker.ready` before painting, so the absence of
the API did not degrade them to an online-only experience; it stopped them at a blank page.

**Why it hid:** `'serviceWorker' in navigator` is a feature test that does not report a bug when it
fails. It silently selects a different path — and when the page's chosen path is "wait for ready",
the different path is *no path at all*. Nothing throws.

**The traps, three of them.** **(1)** `activate` must not run until every promise passed to install's
`waitUntil` has settled. Skip the await and registration still resolves, both events still fire in
the right order, and the worker serves from a cache it has not finished writing — a failure that
surfaces as a miss on the **first offline load**, long afterwards, and reads as a bug in the site.
**(2)** The network `fetch` must be captured **before** the interception wrapper is installed. The
cache-first handler calls `fetch` on every miss, and a worker that re-enters its own wrapper recurses
without bound; the symptom is a hang, not an error. **(3)** `respondWith` must be recorded
synchronously during dispatch — a handler calling it after an `await` has already lost the race in
every real browser, so accepting it here would greenlight code that is broken everywhere else.

**And the one that generalises:** this capability took three ticks — the store (279), the scope
(280), and the lifecycle (281) — and **none of the first two did anything observable on its own**. A
capability whose pieces are individually inert is not a reason to land them separately without
saying so; it is a reason for the board to carry a row that says which third is missing. Tick 279
split its row rather than flipping it for exactly this reason, and that split is what made this tick
obviously next instead of obviously done.

## Progressive enhancement — `CSS.supports()` (tick 282)

**Pattern:** `if (CSS.supports('container-type: inline-size')) { root.classList.add('cq'); }`, or the
CSS twin `@supports (display: grid) { … }` guarding a modern layout while the legacy one is hidden.
Every design system, every CSS framework's feature-detect bundle, and most sites that shipped a
layout change in the last five years contain this shape.

**The class this unlocks:** correct fallback selection — which is to say, *the layout the author
actually tested*. Not a new capability so much as the removal of a wrong answer.

**Why it hid:** `CSS.supports` returned `true` for everything, including `notaproperty: 1` and the
bare string `": "`. Feature detection cannot fail loudly; its entire job is to return a boolean and
be believed. So the browser was not reported as broken — the *site* was, one layout at a time, on
exactly the properties it had been careful enough to check for first.

**The traps, three of them.** **(1)** `return true` is not a permissive default. This API is only
ever consulted by code that is about to act on the answer, and acting on a false yes means discarding
a working fallback. Where a stub must guess, it should guess **no**. **(2)** A lookup table of
supported properties is a second source of truth — right when written, wrong the first time the
engine changes, silent when it drifts. Ask the real parser instead; there is one, and it was already
answering this question for `@supports`. **(3)** Test both directions. A gate that only asserts
"unsupported things are false" is satisfied by a flat `false`, which breaks every enhancement on the
web just as thoroughly as `true` broke every fallback.

**And the one that generalises:** the fix was not implementing anything. The engine **already knew**
the right answer and was giving it correctly on the CSS side while the JS side made one up. Before
building a capability, check whether some other surface of the same engine already has it — this tick
was found by probing a board row that turned out to be mostly done, and the probe cost one test run
where building would have cost a tick.

## The site's own restriction on what may run — Content-Security-Policy (tick 283)

**Pattern:** `Content-Security-Policy: script-src 'self' 'nonce-r4nd0m'` on the document response,
and inline `<script nonce="r4nd0m">` paired to it. Every security-conscious site — GitHub, Google,
every bank, every framework's production build — ships one, and the whole point of it is that the
browser, not the site, is the enforcing party: the header is the site saying *"even if an injection
lands in my HTML, do not run it."*

**The class this unlocks:** honest security posture on the sites that depend on it. Not a rendering
change a user sees, but the difference between a browser that *honours* a page's `script-src` and one
that merely receives it — indistinguishable from the page's side until the day an XSS lands and runs
anyway.

**Why it hid:** a browser that parses the policy and runs the script regardless behaves *identically*
to one that ignores the header entirely. There is no visible symptom, no thrown error, no failed
load — the un-nonced injected script runs exactly as the site's own scripts do. The only observable
difference is an attack that should have been blocked, succeeding. So "we have a CSP module" proves
nothing; the capability is only real if a script that would have run does not.

**The traps.** **(1)** Four layers must all consult the evaluator — net must not drop the header at
the document boundary, the page must seed the policy *before the first script runs*, the external
fetch must check *before issuing the request*, and inline collection must read each element's nonce.
Any one silently failing gives a browser that "supports CSP" and enforces nothing. **(2)** A directive
that parses but never blocks is the exact lie this project keeps catching — `style-src`/`img-src`/etc.
are left honestly absent rather than stubbed, and `restricts_scripts()` reports which of the two a
caller is in. **(3)** Fail *closed* on what a present policy forbids, but fail *open* on a policy you
cannot parse — an absent `script-src` allows, an unrecognised source expression matches nothing.
**(4)** A repeated directive keeps the *first*, not the last: last-wins would let an injected trailing
directive loosen the very policy it was meant to be constrained by.

## Bytes the page made itself — Blob object-URLs (tick 284)

**Pattern:** `canvas.toBlob(b => { const u = URL.createObjectURL(b); img.src = u; /* or */ fd.append('file', b); })`,
and its reader `fetch(URL.createObjectURL(blob)).then(r => r.arrayBuffer())`. Image editors' "save",
every chart library's PNG-download button, upload previews, and PDF/worker bundlers that ship code as
a Blob all run this shape.

**The class this unlocks:** exporting and re-reading content the page generated locally — canvas to a
downloadable/uploadable file, and the `createObjectURL → fetch → arrayBuffer` roundtrip libraries use
to move a Blob's bytes without a server.

**Why it hid:** `canvas.toBlob` returned `cb(null)`, which is *exactly* what a real browser hands a
tainted cross-origin canvas — so a page feature-testing for taint took the "cannot export" branch and
silently produced nothing, no error thrown. And `blob:` URLs resolved only for MediaSource attachment
and Worker source, so `fetch('blob:…')` for real content went to the network and rejected. Both are
invisible from the page's side until an export button does nothing.

**The traps.** **(1)** `toBlob` is async by spec — fire the callback on a microtask, never inline, or
a page reading a variable the callback sets finds it undefined. **(2)** Label the Blob with the format
you *actually* encoded (PNG), never the requested `type` you did not honour — PNG bytes labelled
`image/jpeg` is a lie that surfaces the moment something decodes them. **(3)** One object-URL registry,
not two: `createObjectURL`, MSE attachment, Worker `sourceOf`, and `blob:` fetch must all read the same
store, or a URL registered in one and looked up in another silently misses. **(4)** A revoked or
unknown `blob:` URL is a network error, not an empty 200 — freed bytes must stop being readable.

## The last request on the way out — `navigator.sendBeacon` (tick 285)

**Pattern:** `addEventListener('visibilitychange', () => { if (document.visibilityState === 'hidden')
navigator.sendBeacon('/collect', JSON.stringify(session)); })` — Google Analytics, every RUM agent,
Sentry-style error reporters, and A/B frameworks all flush their final payload this way.

**The class this unlocks:** unload-time telemetry without a thrown handler. Not a visible rendering
change, but an unguarded `navigator.sendBeacon(...)` on `undefined` threw and took the rest of the
`pagehide` handler with it — which is where SPAs flush state, so the failure surfaced as lost data on
the *next* visit, not the current one.

**The traps.** **(1)** It returns a boolean, so `return true` is the cheapest wrong answer — it passes
every shape check while sending nothing. It must ACTUALLY enqueue a POST; gate on the outgoing request,
not the return value. **(2)** Fire-and-forget: no response callback, because a beacon fires when
nothing is left to await it. **(3)** The content-type follows the payload (string→text/plain,
Blob→its type, FormData→multipart), never a fixed guess. **(4)** An oversized payload is refused with
`false` and NOT queued — a silent drop that returns `true` loses the data while claiming success.

## The browser that answers when asked what it is — `navigator.userAgentData` (tick 286)

**Pattern:** `const ua = await navigator.userAgentData.getHighEntropyValues(['platform',
'architecture', 'uaFullVersion']); if (ua.platform === 'Windows') showWindowsDownload();` — modern
sites stopped parsing the UA string and read the structured UA Client Hints surface instead: download
pages pick the right binary, analytics segment by platform, and login flows gate on it.

**The class this unlocks:** structured client-hints feature-detection. Its absence was a double
failure — `navigator.userAgentData.getHighEntropyValues(...)` threw on `undefined` and took the
surrounding detection block with it, and the missing object is itself the loudest "not a real browser"
tell a headless detector has. We report the SAME honest facts the UA string carries (a Manuk brand,
our real version/arch/OS), never a competitor's number: completeness, not evasion.

**The traps.** **(1)** `getHighEntropyValues` returns ONLY the hints the page asked for, folded onto
the always-present low-entropy set — a shim that dumps every field is detectable and wrong. **(2)** The
CH `uaFullVersion` and the UA string are the SAME fact: derive both from one source or they drift and
an inconsistency check flags it. **(3)** `toJSON()` is the low-entropy dict, not the method surface —
don't leak `getHighEntropyValues` into it. **(4)** Include the GREASE `Not.A/Brand` entry so sites
can't brittle-match an exact brand list — that's UA-CH's own guidance, not mimicry.

## Paste reads what the user actually copied — `navigator.clipboard.read`/`readText` (tick 287)

**Pattern:** `pasteBtn.onclick = async () => { const text = await navigator.clipboard.readText();
editor.insert(text); }` — and the richer `for (const item of await navigator.clipboard.read()) { if
(item.types.includes('image/png')) { const blob = await item.getType('image/png'); ... } }` — every
rich-text editor, "paste from clipboard" button, and AI-chat screenshot drop zone reads the clipboard
this way.

**The class this unlocks:** PASTE. The copy half (`writeText`) already worked; the read half returned
only the text THIS page had written, so pasting anything copied in *another* application came back
empty — which is the whole point of paste. The read now pulls the real OS-clipboard contents through
the host bridge.

**The traps.** **(1)** `readText()` must return what was copied ELSEWHERE, not an echo of the page's
own last `writeText` — a self-echo passes a naive test and fails every real paste. **(2)** `read()`
returns `ClipboardItem`s keyed by MIME type: `getType(present)` resolves a Blob, `getType(absent)`
REJECTS — a shim that resolves every type lies to code that feature-checks `image/png`. **(3)** One
clipboard cell: a same-page copy→paste must round-trip, so `writeText` seeds the same store `readText`
reads. **(4)** Be honest about binary: a text-only bridge carries `text/plain`; don't fabricate an
`image/png` Blob you can't actually produce — mark the row `partial`, not `works`.

## Injecting untrusted markup without an XSS hole — `Element.setHTML` (tick 288)

**Pattern:** `commentBody.setHTML(userMarkdownRenderedToHtml)` — a comment system, a CMS field, a
"paste as rich text" editor, any place that takes markup from an untrusted source and puts it in the
page. `setHTML` is the platform's own DOMPurify: it parses like `innerHTML` and strips the scriptable
parts. The escape hatch is `setHTMLUnsafe(trustedHtml)`, which is `innerHTML` with a name that says so.

**The class this unlocks:** XSS-safe HTML injection. Absent, `el.setHTML` was `undefined` and the
injection path either threw `is not a function` or fell back to the raw `innerHTML` hole.

**The traps.** **(1)** `setHTML` is NOT an alias for `innerHTML` — a stub that forwards to it passes a
"does it render markup" test and ships the exact vulnerability the API exists to close. Gate on the
`<script>` being GONE, the `onerror=` attribute being GONE, the `javascript:` href being GONE. **(2)**
Sanitizing is not deleting — `<b>`, text, and a normal `href` must survive, or the feature is useless.
**(3)** `setHTMLUnsafe` must genuinely keep the script (it is the opt-out); if both strip, you have one
method wearing two names. **(4)** Only ever REMOVE, never rewrite — a sanitizer that "fixes" a URL can
introduce a value the page never authored. **(5)** Be honest about scope: the safe baseline (script /
handlers / `javascript:`) is real; the configurable allow/block lists are a follow-on — mark the row
`partial`, not `works`.

**Update (tick 545) — the config's first brick, `removeElements`.** The baseline answers *"is this
markup safe?"*; every real caller also asks *"and drop the things I don't want"*. A comment renderer
permits `<b>` and `<a>` but never an `<img>` or an `<iframe>` — a baseline-safe `<img src=…>` is still a
tracking pixel and a layout bomb, and the baseline has no reason to remove it.
`setHTML(html, { sanitizer: { removeElements: ['img','iframe'] } })` now removes those elements
entirely. Before this the options argument was **read and ignored**, which is the worst shape a failure
takes: the page is told YES and gets an unfiltered tree.

**Trap (6), and it is the one that decides the whole config's shape: the baseline must not be
configurable OFF.** `sanitize_subtree` takes the block-list as a *parameter* rather than the config
replacing the sanitizer, so `<script>` is stripped whether or not a config was passed — and every
malformed config (no options object, no `sanitizer` key, a non-array `removeElements`) degrades to an
EMPTY block-list, i.e. to the safe baseline, never to nothing. *The safe answer is the floor and
configuration only ever raises it.* The row stays `partial`: this is a block-list, and the allow-list
(`elements`), `replaceWithChildrenElements`, the attribute lists and a reusable `Sanitizer` object are
the named follow-ons.

## Validating a URL without a try/catch — `URL.canParse` / `URL.parse` (tick 289)

**Pattern:** `if (!URL.canParse(userInput)) return showError('bad url')` and
`const u = URL.parse(href, base); if (u) route(u.pathname)` — form validation, router libraries and
input sanitizers reach for the static URL validators instead of wrapping `new URL(x)` in a try/catch.

**The class this unlocks:** URL validation on the hot path. Absent, `URL.canParse` was `undefined` and
the call threw `is not a function`, taking the surrounding validation branch with it.

**The traps.** **(1)** `canParse` must return a real BOOLEAN that AGREES with the constructor — `true`
where `new URL` succeeds, `false` where it throws — a stub that returns `true` unconditionally is a
validator that validates nothing. **(2)** `parse` returns `null` on failure, never a throw — that is
the whole reason it exists over the constructor. **(3)** Relative-URL semantics must match: `/path`
with no base is NOT parseable, but is once a base is passed — get this wrong and a router mis-resolves
every relative link. **(4)** Keep them delegating to the one native constructor, so the validator and
the thing it validates can never disagree.

## Compound request cancellation — `AbortSignal.any` (tick 290)

**Pattern:** `fetch(url, { signal: AbortSignal.any([userController.signal, AbortSignal.timeout(5000)]) })`
— one request that cancels on EITHER a user action OR a timeout. Request libraries and data-fetching
hooks compose cancellation this way.

**The class this unlocks:** compound cancellation. `AbortSignal.timeout` existed but `any` was missing,
so the compose threw `AbortSignal.any is not a function`. Wiring it also fixed a latent bug: the timeout
flipped `aborted` without firing its `abort` event, so a fetch given a timeout signal was never actually
cancelled.

**The traps.** **(1)** The result must be a REAL `AbortSignal` — its `abort` event fires, `aborted`/
`reason` are live — not an inert object that only looks like one, or a fetch keyed off the event never
cancels. **(2)** An already-aborted input aborts the result IMMEDIATELY (synchronously), not on the next
turn. **(3)** Forward the SOURCE reason, so a caller can tell a `TimeoutError` from a user `AbortError`.
**(4)** If you add a combinator over signals, check the signals it combines actually DISPATCH — a
"timeout" that sets a flag without an event is a cancel that never happens.

## Is this element actually on screen? — `Element.checkVisibility` (tick 291)

**Pattern:** `if (el.checkVisibility()) el.scrollIntoView()` and `if
(!row.checkVisibility({ visibilityProperty: true })) skipAnimation(row)` — UI libraries guard
scroll-into-view, lazy mounting, and a11y "is it on screen" with it instead of hand-rolling a
`getComputedStyle` + `offsetParent` + ancestor-walk check.

**The class this unlocks:** rendered-visibility testing. Absent, `el.checkVisibility` was `undefined`
and the call threw `is not a function`, taking the guard's branch with it.

**The traps.** **(1)** `display:none` must be checked up the WHOLE ancestor chain — a descendant of a
hidden element keeps its own computed `display`, so reading self returns a false positive. **(2)** The
default checks only rendering: `visibility:hidden` and `opacity:0` are STILL visible unless the caller
passes the option — fold them in unconditionally and you disagree with every other browser. **(3)** A
disconnected element is not visible. **(4)** Back it with the REAL computed cascade, not `offsetParent`
alone (which is also null for `position:fixed` and the body) — a guess here silently mis-guards.

## Serialising access to a shared resource — `navigator.locks` (tick 292)

**Pattern:** `navigator.locks.request('token-refresh', async () => { await refresh() })` — auth SDKs
and any code with a critical section wrap it so two concurrent callers can't both run it. The second
`request` for the same name waits for the first to finish.

**The class this unlocks:** in-page mutual exclusion. Absent, `navigator.locks.request` threw on
`undefined`, and the SDK either crashed or raced (two token refreshes clobbering each other).

**The traps.** **(1)** The whole point is SERIALISATION — a stub that runs both callbacks at once passes
a "does it call my function" test and ships the exact race the API exists to prevent; gate on ordering
(`b` starts only after `a` ends). **(2)** The lock is held until the callback's returned promise
SETTLES, not until it returns — an async callback holds across its awaits. **(3)** `ifAvailable` must
NOT queue: on a held lock it invokes with a `null` grant so the caller can take its "busy" path. **(4)**
`request` resolves with the callback's value, so `await navigator.locks.request(...)` returns it.

## Keeping the main thread responsive — `scheduler.postTask` (tick 293)

**Pattern:** `scheduler.postTask(() => renderExpensiveList(), { priority: 'background' })` alongside
`scheduler.postTask(handleClick, { priority: 'user-blocking' })` — frameworks split work by priority so
interaction pre-empts background rendering.

**The class this unlocks:** priority-aware main-thread scheduling. Absent, `scheduler.postTask` threw on
`undefined`, and the framework fell back (or crashed on a hard reference).

**The traps.** **(1)** It is NOT `setTimeout` — priority must actually ORDER execution (user-blocking
before user-visible before background), or the whole point is lost; gate on run order, not just that the
callback runs. **(2)** Same-turn posts must collect before any runs (a macrotask turn), or the first
post always wins regardless of priority. **(3)** An `AbortSignal` that fires before the task runs must
REMOVE it and reject — a scheduler that runs an aborted task wasted the cancel. **(4)** `postTask`
returns a Promise of the callback's value, so `await scheduler.postTask(cb)` yields it.

## Transform math off the main geometry path — `DOMMatrix` (tick 294)

**Pattern:** `const m = ctx.getTransform().inverse(); const p = m.transformPoint({x, y})` — canvas apps
map screen coordinates back into world space, and charting/graphics libraries compose
`new DOMMatrix().translate(x, y).scale(k)` to place things.

**The class this unlocks:** client-side 2D transform math. Absent, `new DOMMatrix(...)` threw
`DOMMatrix is not defined` and took the graphics path with it.

**The traps.** **(1)** It is MATH, not a bag of setters — the gate must assert computed coordinates
(`rotate(90)` maps `(1,0)→(0,1)`, `inverse()` actually inverts), or a wrong-convention stub ships silent
mis-transforms. **(2)** `multiply`/`translate`/`scale`/`rotate` return a NEW matrix (the non-`*Self`
forms don't mutate) — mutating in place breaks `a.multiply(b)` used as a pure expression. **(3)** Watch
the composition order: `m.translate(t).scale(s)` applies scale to the point FIRST, then translate. **(4)**
Be honest about 2D vs 3D — a 2D-only matrix must say `is2D:true` and not pretend to carry `m13..m44`.

## The point half of the transform pair — `DOMPoint` (tick 295)

**Pattern:** `const world = new DOMPoint(sx, sy).matrixTransform(ctx.getTransform().inverse())` — map a
screen coordinate into world space, or read back `matrix.transformPoint(p)` from a transform.

**The class this unlocks:** coordinate transforms alongside `DOMMatrix`. Absent, `new DOMPoint(...)`
threw `is not defined`.

**The traps.** **(1)** `w` defaults to `1` (a position), not `0` — get this wrong and perspective/
homogeneous math silently breaks. **(2)** `matrixTransform` must apply the SAME affine convention as the
matrix, or a point and its matrix disagree. **(3)** `DOMMatrix.transformPoint` should return a real
`DOMPoint` (chainable, carrying `w`), not a bare `{x,y}` — a caller that chains `.matrixTransform` on the
result breaks otherwise.

## The transformed rectangle — `DOMQuad` (tick 296)

**Pattern:** `const box = el.getBoxQuads()[0].getBounds()` — after CSS transforms have rotated/skewed an
element, its screen footprint is a quadrilateral, and code reduces it to an axis-aligned box.

**The class this unlocks:** the general (non-axis-aligned) rectangle. Absent, `DOMQuad.fromRect(...)` /
`new DOMQuad(...)` threw.

**The traps.** **(1)** `getBounds` is min/max over ALL four points — a skewed quad's box is larger than
any one edge; compute it, don't assume the corners are ordered. **(2)** `fromRect` corners go clockwise
from the top-left, and each is a real `DOMPoint` (so `.matrixTransform` chains). **(3)** It completes a
family — a `DOMQuad` whose `getBounds` returns something other than a `DOMRect` breaks callers that read
`.width`/`.height`.

## Routing by URL shape — `URLPattern` (tick 297)

**Pattern:** `new URLPattern({ pathname: '/api/:resource/:id' }).exec(request.url)` — SPA routers and
Service Worker `fetch` handlers dispatch by matching the URL against a pattern and reading the named
groups.

**The class this unlocks:** declarative URL routing. Absent, `new URLPattern(...)` threw `is not
defined` and took the router's registration with it.

**The traps.** **(1)** Anchor the match — `/users/:id` must NOT match `/users/42/extra`; a pattern
without a trailing `$` silently over-matches every deeper path. **(2)** `:name` captures one segment
(`[^/]+`), `*` captures across segments (`.*`) — mixing them up mis-routes. **(3)** `exec` returns
`null` on a miss, not an empty match — routers branch on it. **(4)** Accept a full URL string, a bare
pathname, and an object — a router passes whichever it has.

## Stream pipelines that actually move data — `WritableStream` / `TransformStream` (tick 298)

**Pattern:** `await response.body.pipeThrough(new TextDecoderStream()).pipeTo(sink)` — streaming fetch
pipelines transform bytes→text and drain them into a consumer, chunk by chunk, without buffering the
whole body.

**The class this unlocks:** composable stream pipelines. `ReadableStream` was real but `WritableStream`
and `TransformStream` were INERT NAMES (`typeof` said function, `getWriter`/`readable` were undefined),
so any pipeline threw.

**The traps.** **(1)** `typeof X === 'function'` proves NOTHING — an inert constructor passes it and
fails the moment you call a method; gate on data actually flowing (a chunk reaching the sink). **(2)** A
`TransformStream` must reshape via `controller.enqueue`, not pass through — test that the output differs
from the input. **(3)** `pipeThrough` returns the transform's READABLE (not the source); `pipeTo`
returns a promise that resolves when the source closes. **(4)** Be honest about backpressure — if
`ready` is always resolved, say so; don't imply a slow sink throttles the source when it doesn't.

## Decoding a streaming response as text — `TextDecoderStream` (tick 299)

**Pattern:** `for await (const chunk of res.body.pipeThrough(new TextDecoderStream())) append(chunk)` —
an LLM token stream, an SSE-ish feed, or any large text download is read incrementally as decoded
strings without buffering the whole body.

**The class this unlocks:** streaming text decode. Absent, `new TextDecoderStream()` threw and the pipe
fell apart.

**The traps.** **(1)** Decode with the STREAMING flag — a multi-byte character (`é` = `0xC3 0xA9`) lands
split across a chunk boundary constantly, and decoding each chunk independently turns it into two U+FFFD
halves; hold the partial sequence back and prepend it to the next chunk. **(2)** Flush on close — the
last held bytes must still emit. **(3)** It is a real `TransformStream` — `pipeThrough` returns its
readable, so the decoded stream composes with the rest of the pipeline.

## Reading a File/Blob as a stream — `Blob.stream()` (tick 300)

**Pattern:** `for await (const chunk of file.stream()) hash.update(chunk)` and
`blob.stream().pipeThrough(new TextDecoderStream())` — a file upload or a downloaded blob is processed
incrementally without loading it all into memory at once.

**The class this unlocks:** blob/file streaming. `blob.stream()` returned `null`, so any code that read
or piped it threw `can't access property 'getReader' of null`.

**The traps.** **(1)** Return a real `ReadableStream`, not `null` and not an inert look-alike — the chunk
must be the blob's actual `Uint8Array` bytes. **(2)** It must compose: `pipeThrough`/`pipeTo` on the
returned stream have to work, since streaming a blob into a decoder or a hash is the whole use. **(3)**
Bytes, not code units — a binary blob's stream carries `0..255` byte values, not UTF-16 units.

## Returning JSON in one call — `Response.json()` (tick 301)

**Pattern:** `return Response.json({ user, token })` — a Service Worker `fetch` handler or an
edge/app route replies with JSON without hand-building the body and `Content-Type`.

**The class this unlocks:** one-call JSON responses. `Response` and `res.json()` were real, but the
static `Response.json` was missing, so the idiom threw `is not a function`.

**The traps.** **(1)** Default the `Content-Type` to `application/json` — but only if the caller did not
set one, or you clobber an explicit override. **(2)** It is read-symmetric — a value built with
`Response.json(x)` must parse back via `res.json()`. **(3)** Honour `init.status`/`statusText` — a `201`
or `404` JSON response is common. **(4)** Non-serialisable data (a value whose `JSON.stringify` is
`undefined`) is a `TypeError`, not an empty body.

## Cursor and selection in text fields — `setSelectionRange` / `select` (tick 302)

**Pattern:** `input.addEventListener('focus', () => input.select())` (select-all on focus),
`input.setSelectionRange(pos, pos)` (place the caret after formatting), and reading
`input.selectionStart` in an input mask to know where the user is typing.

**The class this unlocks:** programmatic text selection. The whole surface was `undefined`, so a copy
button, an input mask, or an editor got `setSelectionRange is not a function` or `undefined` offsets.

**The traps.** **(1)** Clamp to the value length — `setSelectionRange(50, 99)` on an 11-char value must
land at `11/11`, not `50/99`. **(2)** `selectionStart`/`End` are readable AND writable, and setting one
must keep the other consistent (`start ≤ end`). **(3)** Count in UTF-16 code units, the unit the value's
`length` uses. **(4)** `select()` is the whole value (`0..length`), not just a cursor move.

## Insert/replace at the cursor — `setRangeText` (tick 303)

**Pattern:** `input.setRangeText(completion, input.selectionStart, input.selectionEnd, 'end')` —
autocomplete drops the chosen text in at the caret; an editor toolbar wraps the selection; a formatter
rewrites a span. It edits the value THROUGH the selection.

**The class this unlocks:** programmatic text editing of a field. Absent, `setRangeText` threw `is not a
function` and the insert/replace fell back to clobbering the whole `value` (losing the caret).

**The traps.** **(1)** Splice in UTF-16 units (the unit `value.length` and the selection use), or a
multi-byte character mis-offsets the cut. **(2)** No range means the CURRENT selection, not the whole
value. **(3)** `selectMode` matters: `'end'` puts the caret after the insert (so typing continues),
`'select'` highlights it — defaulting everything to one behaviour breaks the next keystroke. **(4)** An
empty range is an INSERT (delete nothing), not a no-op.

## Normalising a query string — `URLSearchParams.sort()` + value-aware `has`/`delete` (tick 304)

**Pattern:** `p.sort(); const canonical = p.toString()` (a stable cache key / canonical URL), and
`if (params.has('mode', 'edit'))` / `params.delete('filter', staleValue)` — routers and query handlers
match and prune specific name=value pairs.

**The class this unlocks:** precise query-param manipulation. `sort()` was missing; `has`/`delete`
silently ignored the value, so they matched/removed by name alone.

**The traps.** **(1)** `sort()` is STABLE — two entries with the same name must keep their order, or
round-tripping a query string reorders duplicates. **(2)** Compare keys by code units, not locale.
**(3)** The 2-arg `has`/`delete` must actually check the value — matching by name alone is the exact bug
that makes a router accept the wrong tab. **(4)** Keep the 1-arg forms working (value `undefined` =
name-only).

## Walking a form's fields — `FormData.keys()` / `values()` (tick 305)

**Pattern:** `for (const name of formData.keys()) validate(name)` and `[...formData.values()]` — a page
iterates the fields it is about to submit.

**The class this unlocks:** the FormData field iterators. `entries()`/`forEach()` worked but
`keys()`/`values()` threw `is not a function`, breaking the name-only / value-only loops.

**The traps.** **(1)** Preserve insertion order AND duplicates — a form with two `a` fields yields `a`
twice from `keys()`. **(2)** Return real iterators (`[Symbol.iterator]`), so `for...of` and spread both
work. **(3)** Keep them consistent with `entries()`/`forEach()` — three views of the same ordered list.

## Verifying a webhook signature / HS256 JWT — `crypto.subtle` HMAC (tick 306)

**Pattern:** `const mac = await crypto.subtle.sign('HMAC', key, payload); if (!timingSafeEqual(mac,
headerSig)) reject()` — a webhook handler (Stripe/GitHub/Slack) authenticates the request, and an HS256
JWT verifier checks the token signature.

**The class this unlocks:** HMAC signing/verification. `digest` worked but `importKey`/`sign`/`verify`
threw `is not a function`, so signature validation couldn't run in the page/worker.

**The traps.** **(1)** HMAC is `H((k⊕opad)||H((k⊕ipad)||m))` with the key hashed-if-long / zero-padded to
the BLOCK size (64 for SHA-256, not the digest size) — get the padding wrong and it silently produces a
plausible-but-wrong MAC. **(2)** Gate against a KNOWN-ANSWER vector (RFC 4231), not self-consistency — a
sign/verify pair can agree with each other while both being wrong. **(3)** `verify` must compare in
constant time; a short-circuit `===` leaks the signature byte by byte. **(4)** Be honest about scope —
HMAC is a composition of an existing hash; asymmetric crypto is a different, absent capability.

## Deriving keys from a secret — `crypto.subtle.deriveBits` (HKDF) (tick 307)

**Pattern:** `const bits = await crypto.subtle.deriveBits({name:'HKDF', hash:'SHA-256', salt, info},
ikmKey, 256)` — expand a shared secret / master key into per-purpose keying material for a token scheme
or an encrypted channel.

**The class this unlocks:** HKDF key derivation. `deriveBits` threw `is not a function`, so any
derivation step failed.

**The traps.** **(1)** Extract-then-Expand — `PRK = HMAC(salt, IKM)` first, then expand; skipping Extract
(using IKM directly as the PRK) is a common wrong shortcut. **(2)** Empty salt defaults to a zero block
of hash length, not the empty string. **(3)** The expand counter is a single byte appended AFTER `info`,
starting at 1. **(4)** Gate against RFC 5869 known-answers — a self-consistent but wrong derivation
produces stable garbage.

## Animated route/state change — `document.startViewTransition` (tick 308)

**Pattern:** `document.startViewTransition(() => { this.render(nextRoute); })` — an SPA (or an MPA via
the CSS half) wraps a route/state DOM mutation in a transition so the browser can snapshot before/after
and cross-fade. Interoperable now, so Next.js/SvelteKit/Astro and hand-rolled routers all reach for it.

**The class this unlocks:** View-Transition-driven SPAs. The method was absent, so the call threw `is
not a function`, the TypeError took down the click handler, and **the wrapped DOM update never ran** —
the page froze on the previous view with no visible error. That silent-freeze is the app-class failure
this closes.

**The traps.** **(1)** The load-bearing behaviour is that the update callback RUNS and its mutations
land — not the animation. **(2)** This engine composites no snapshot pseudo-elements, so there is no
cross-fade; that is the spec's own SKIP path (reduced-motion / not-visible documents still run the
callback and settle the promises), so running the callback and resolving is honest, not a stub.
**(3)** A throwing callback must reject `ready`/`finished`/`updateCallbackDone` — do not swallow it into
a false success — while each branch absorbs its own rejection so a site awaiting only one does not trip
an unhandled-rejection. **(4)** `typeof document.startViewTransition === 'function'` is exactly what an
inert stub passes; the gate drives a real click and reads the resulting DOM.

## Client-side routing via the Navigation API — `window.navigation` (tick 309)

**Pattern:** `navigation.addEventListener('navigate', e => { if (e.canIntercept) e.intercept({ handler:
() => renderRoute(e.destination.url) }); })` — a single hook that takes over every same-document
navigation, replacing the pushState + popstate + link-click-interception dance routers used to hand-roll.

**The class this unlocks:** Navigation-API SPA routers. `window.navigation` was absent, so a router that
feature-detected it and bound a `navigate` listener silently bound nothing — every in-app link did a
full document load or nothing, with no visible error (the "dead router").

**The traps.** **(1)** Do not create a second URL source of truth — commit through the existing
`history.pushState`/`replaceState` so `location`, the omnibox and the back-stack stay consistent.
**(2)** The `navigate` event fires for same-document navigations and must expose `destination.url` +
`canIntercept`; the router reads those to decide whether to take over. **(3)** `intercept({handler})`
handlers are async per spec — they run in a microtask, and their DOM writes are what actually change the
view, so the capability is the handler RUNNING, not the event firing. **(4)** `preventDefault()` must
truly veto (route guards, unsaved-changes) — a veto that still commits is worse than no API. **(5)**
`typeof navigation === 'object'` is what an inert stub passes; gate by driving `navigate()` and reading
the resulting DOM + URL.

## Imperative animation — `element.animate` (Web Animations API) (tick 310)

**Pattern:** `await el.animate([{opacity:0},{opacity:1}], {duration:300, fill:'forwards'}).finished;
next();` — a fade/slide/scale run imperatively, often awaited to sequence the next step. Also the object
form `el.animate({transform:['none','scale(1.1)']}, 200)` and `el.getAnimations().forEach(a=>a.cancel())`.

**The class this unlocks:** imperative animations. `element.animate` was absent, so the call threw
`is not a function` out of the interaction handler (dead interaction), and `await …​.finished` hung on a
promise that never existed.

**The traps.** **(1)** With no compositor timeline the honest move is to FAST-FORWARD to the end state,
not to fake a tween — run the keyframes to completion, apply the final frame when `fill` is
`forwards`/`both`, settle `finished`. State the "no intermediate frames" limit. **(2)** Normalize BOTH
keyframe forms — the array of frames and the object-of-arrays. **(3)** `cancel()` must reject `finished`
with an `AbortError`; animation-racing code unwinds on it. **(4)** Install element-prototype methods on
`Object.getPrototypeOf(document.createElement(...))` (the live chain link), never on `g.Element.prototype`
(absent early in the prelude) or `g.HTMLElement.prototype` (a disconnected fresh constructor) — both miss
every instance. **(5)** `typeof el.animate === 'function'` is what a stub passes; gate by driving it and
reading the resulting computed style.

## Location — `navigator.geolocation`, and the honest denial (tick 311)

**Pattern:** `navigator.geolocation.getCurrentPosition(pos => useIt(pos.coords), err => fallback())` —
called straight from a load or click handler by weather sites, store locators, delivery/ride apps and
"near me" search. Also `id = navigator.geolocation.watchPosition(...)` / `clearWatch(id)`.

**The class this unlocks:** location-aware sites. Real code does NOT feature-detect the object (in a
real browser it is always present), so a missing `navigator.geolocation` is `undefined` and
`undefined.getCurrentPosition` throws a TypeError out of the handler — the whole interaction (and often
boot) dies.

**The traps.** **(1)** There is no location provider, so DO NOT invent coordinates — that is the
dishonest path. Fail instead, and fail with the answer the permission layer already gives: we model the
geolocation permission as `'denied'`, so the error `code` is `PERMISSION_DENIED` (1), self-consistent
with `navigator.permissions.query({name:'geolocation'})`. A browser is allowed to be unusual; it is not
allowed to contradict itself. **(2)** Delivery is ASYNCHRONOUS — invoke the error callback on a later
turn (microtask), never synchronously inside `getCurrentPosition()`, or code relying on the ordering
breaks. **(3)** Put the interface constants (`PERMISSION_DENIED`/`POSITION_UNAVAILABLE`/`TIMEOUT`) on
BOTH the error instance and the constructor — real code branches on `err.code === err.PERMISSION_DENIED`.
**(4)** `watchPosition` still returns a numeric id so `clearWatch(id)` and the store-the-id pattern work.
**(5)** `typeof navigator.geolocation === 'object'` is what an inert stub passes; gate by DRIVING
`getCurrentPosition` and asserting the async error branch runs with the right code.

## Media control — `navigator.mediaSession` + `MediaMetadata` (tick 312)

**Pattern:** `navigator.mediaSession.metadata = new MediaMetadata({title, artist, artwork:[{src}]});
navigator.mediaSession.setActionHandler('play', onPlay); ...setActionHandler('nexttrack', onNext)` —
every media player wires this at startup so OS media keys, the lock screen and headset buttons control
playback, and the lock screen shows the track.

**The class this unlocks:** media playback UX. Real player code assumes `navigator.mediaSession` is
present (does NOT guard it), so its absence throws `undefined.setActionHandler` out of the init and the
player dies.

**The traps.** **(1)** RETAIN state, do not no-op it — `metadata`, `playbackState`, position and the
action handlers must round-trip, because the site (and a host/agent) read them back to render and
actuate. An inert stub that accepts and drops them passes `typeof` and fails the moment anything reads.
**(2)** Normalize `MediaMetadata.artwork` to an array of `{src,sizes,type}` — sites read `.artwork[0].src`.
**(3)** `setActionHandler` must THROW a TypeError on an out-of-enum action; silently accepting a typo
hides the bug. `null` unsets. **(4)** There is no OS media-key surface to invoke handlers from — state
the limit — but expose a non-standard seam so a host/agent CAN invoke a stored handler (read "now
playing", trigger play/pause). That turns an honest-limit shim into an agentic-actuation win.
**(5)** Gate by DRIVING it (metadata round-trip + invoking a stored handler), never by `typeof`.

## Environment — `window.visualViewport` mirrors the layout viewport (tick 313)

**Pattern:** `visualViewport.addEventListener('resize', () => fixKeyboardInset());
el.style.height = visualViewport.height + 'px'` — keyboard-aware and pinch-zoom layouts size off the
VISUAL viewport (what is actually visible) rather than the layout viewport.

**The class this unlocks:** responsive/keyboard-aware layout. The API is used UNGUARDED, so its absence
throws `undefined.addEventListener` (or `undefined.width`) out of the layout setup and the responsive
code dies.

**The traps.** **(1)** With nothing zoomed, the visual viewport EQUALS the layout viewport — so read
`width`/`height` from the SAME real `innerWidth`/`innerHeight` the cascade lays out against (a getter,
so it tracks a later resize), `scale` 1, offsets 0. A hardcoded size is the same bug as `innerWidth`
disagreeing with `@media`. **(2)** Retain the `resize`/`scroll` listeners even though nothing fires them
yet (no live pinch-zoom / OSK) — the unguarded `addEventListener` must not throw, and a future host can
drive them; state the limit. **(3)** Gate by asserting the metrics MIRROR `innerWidth`/`innerHeight`,
not just that they are numbers — a stub returning a constant passes `typeof` and lies about the layout.

## Adaptive loading — `navigator.connection` (Network Information API) (tick 314)

**Pattern:** `if (navigator.connection.saveData) loadLowRes(); else loadHiRes();` and
`navigator.connection.addEventListener('change', reevaluate)` — adaptive-loading code tunes image
quality, autoplay and prefetch to the link.

**The class this unlocks:** adaptive/data-aware loading. Some of this code reaches for
`navigator.connection.*` unguarded, so its absence throws `undefined.effectiveType` /
`undefined.addEventListener` out of the loader.

**The traps.** **(1)** We do not measure the link continuously, so report the HONEST default a real
browser gives on a fast desktop connection — `effectiveType:'4g'`, plausible downlink/rtt — and
`saveData:false`, which is not a guess but the true state (no data-saver). **(2)** Do NOT fabricate a
SLOW link — that would needlessly degrade every page; the un-metered default is both honest and
non-harmful, whereas a slow fabrication costs the user. **(3)** Provide the `change` EventTarget so the
unguarded `addEventListener` does not throw (it never fires — state the limit). **(4)** Gate on the
VALUES (saveData false, a valid ECT token), not just `typeof` — a stub returning a slow/metered guess
passes `typeof` and silently downgrades the page.

## Storage headroom — `navigator.storage` (StorageManager) (tick 315)

**Pattern:** `const {quota, usage} = await navigator.storage.estimate(); if (quota - usage < needed)
warnUser(); await navigator.storage.persist();` — offline apps check headroom and request durable
storage before caching data into IndexedDB/Cache.

**The class this unlocks:** offline-first / PWA storage. The methods are AWAITED in boot, so an absent
`navigator.storage` throws `undefined.estimate()` out of startup.

**The traps.** **(1)** This is a capability you HAVE (a real IndexedDB/Cache backend) — so answer
TRUTHFULLY, not with a denial: `persist()`/`persisted()` are genuinely true on a durable single-user
desktop that does not evict. **(2)** `estimate()` returns `{quota, usage}` — report a generous real
quota; `usage` may be a floor if you cannot cheaply sum live bytes, but `quota` is the number apps check
against, so it must be honest and large. **(3)** Do NOT stub OPFS `getDirectory()` unless you back it —
a present-but-broken `FileSystemDirectoryHandle` is worse than an honest absence a feature check sees.
**(4)** Gate on the VALUES (quota>0, usage<=quota, persistence true), not `typeof`.

## Read-aloud — `speechSynthesis` present but honestly mute (tick 316)

**Pattern:** `const u = new SpeechSynthesisUtterance(text); u.onend = next; speechSynthesis.speak(u)` —
screen readers, "read aloud" buttons and language-learning apps voice text.

**The class this unlocks:** accessibility read-aloud / TTS. The constructor and `speechSynthesis` are
used UNGUARDED, so absence throws `SpeechSynthesisUtterance is not defined` out of the a11y handler.

**The traps.** **(1)** With no TTS engine, do NOT fire `end` — that claims it spoke when the user heard
nothing, a lie the code cannot see. Report the honest failure via `error` ('synthesis-unavailable'), the
geolocation pattern; code that handles `onerror` degrades correctly. **(2)** `getVoices()` returns `[]`
— true, no voices installed — not a fabricated voice list. **(3)** Deliver the error ASYNCHRONOUSLY (a
microtask), never inside `speak()`. **(4)** Gate on the honest result (error fired, `end` NOT fired),
not on `typeof` — a stub that fires `end` passes `typeof` and silently swallows every read-aloud.

## Keep-awake — `navigator.wakeLock` (Screen Wake Lock) (tick 317)

**Pattern:** `const sentinel = await navigator.wakeLock.request('screen'); …; await sentinel.release()`
— video players, presentations, kiosks and reading UIs keep the display awake while active.

**The class this unlocks:** display keep-awake for media/presentation. The request is awaited in the
play/present handler, so an absent `navigator.wakeLock` throws `undefined.request` out of it.

**The traps.** **(1)** The OS sleep timer is host-owned, so — like mediaSession — GRANT and retain a
real sentinel (a handle the player holds and can `release()`, a seam a host can later enforce) rather
than rejecting; state the limit. Rejecting sends every video into its "could not keep awake" branch.
**(2)** `release()` must resolve a Promise, flip `released` to true and fire the `release` event — the
player's cleanup path depends on it. **(3)** Gate by driving request → sentinel shape → release
round-trip, not `typeof`.

## Custom form controls — `ElementInternals` / `attachInternals` (tick 318)

**Pattern:** `class MyInput extends HTMLElement { static formAssociated = true; constructor(){ super();
this._internals = this.attachInternals(); } set value(v){ this._internals.setFormValue(v); } }` — a
form-associated web component wires its value/validity/ARIA through internals.

**The class this unlocks:** web-component design systems (form controls). `attachInternals()` is called
UNGUARDED in the constructor, so its absence throws `attachInternals is not a function` and the whole
component fails to upgrade — it renders as an empty dead tag.

**The traps.** **(1)** Return a REAL internals that RETAINS state (form value, validity flags+message,
custom states), not an inert stub — `checkValidity()` must reflect the flags the component set, and
`states.has()` must drive `:state()`. **(2)** `states` is a CustomStateSet — back it with a real Set.
**(3)** Enforce once-per-element (a second `attachInternals()` throws NotSupportedError) via a WeakSet —
components rely on that being an error. **(4)** Install on the live element-prototype chain link
(`Object.getPrototypeOf(createElement(...))`), so custom elements (which extend HTMLElement) inherit it.
**(5)** Gate by driving setValidity → checkValidity and the once-throw, not `typeof`.

## Drag tracking — pointer capture (tick 319)

**Pattern:** `el.addEventListener('pointerdown', e => { el.setPointerCapture(e.pointerId); }); ...
el.releasePointerCapture(e.pointerId)` — a slider/drag keeps receiving moves after the pointer leaves
the element.

**The class this unlocks:** drag interactions (sliders, drag-reorder, canvas draw, croppers). The call
is UNGUARDED in pointerdown, so its absence throws `setPointerCapture is not a function` and the drag
dies on the first press.

**The traps.** **(1)** Retain the captured pointer id per element so `hasPointerCapture(id)` reflects
the truth — a drag reads it back. **(2)** Fire `got`/`lostpointercapture` — capture-based drags wire
those hooks. **(3)** The host owns the live pointer pipeline, so a prelude shim cannot yet re-route
stray moves outside the element — state that limit; retaining state + not throwing is the load-bearing
part. **(4)** Gate by driving the false→true→false capture cycle and the got event, not `typeof`.

## Selection API — programmatic getSelection (tick 328)

**Pattern:** `var s = window.getSelection(); s.selectAllChildren(pre);
navigator.clipboard.writeText(s.toString())` — copy-a-code-block / share-selection widgets; and
editors that read `s.anchorNode`/`s.getRangeAt(0)` or drive `s.collapse`/`s.extend`/`s.setBaseAndExtent`.

**The class this unlocks:** any script that reads or sets the document selection — "copy code" buttons,
"copy link to highlight", rich-text editors tracking the caret/selection, `Notion`/docs-lite selection
state. The calls are UNGUARDED, so the old stub's `toString()===''` made them fail SILENTLY (button
copies nothing, nothing thrown).

**The traps.** **(1)** ONE persistent object per window (`getSelection()===getSelection()`), not a fresh
inert object per call — state must survive between two lines of a caller. **(2)** Back it with the real
`Range` (`document.createRange`), don't build a second boundary-point model. **(3)** A Selection is
DIRECTIONAL where a Range is normalised: track `_dir` so `extend()` before the anchor keeps the anchor
fixed (`anchorOffset > focusOffset`) instead of silently swapping ends. **(4)** Real `Selection`
constructor + remove it from the inert-names list so `instanceof` works and the stub doesn't shadow it
(the AbortSignal lesson). **(5)** One-range model (a second `addRange` is ignored); `getRangeAt(0)` on
empty THROWS `IndexSizeError`. **(6)** Honest limit: user mouse-drag selection GEOMETRY is layout/hit-
test, not modelled — this is the scripting surface. **(7)** Gate by driving selectAllChildren→toString,
the forward/backward extend, and addRange, not `typeof`.

## Fullscreen toggle — `element.requestFullscreen()` (tick 330)

**Pattern:** a fullscreen button calls `videoEl.requestFullscreen()` (or the container's) from a click,
listens for `document.onfullscreenchange` to swap its controls, and calls `document.exitFullscreen()`
to leave. Every video player, slide deck, browser game and image lightbox is this shape, and many
feature-detect the `webkit`-prefixed names first.

**The class this unlocks:** the fullscreen-video/media-viewer web — the single most-used player
affordance after play/pause. Aligned with the media marquee: a YouTube-class player's fullscreen
button now functions instead of throwing.

**Why it hid:** `requestFullscreen` was `undefined`, and pages do not guard a method they assume
exists. `undefined()` throws out of the click handler, so the button does nothing AND the throw can
abort the rest of that handler — a compound silent failure.

**The trap, and where the honesty line falls:** the reflex is to call a state-only fullscreen shim the
"told yes, renders blank" anti-pattern. It is not. The OS window resize is the *shell's* job and is the
one thing this API does not expose to script — `fullscreenElement`, the `fullscreenchange` event and
the promise are the whole page-observable contract, and all are truthful. The player's own content
enters its fullscreen view off this state; only the window is unchanged, which no page can observe here.
Model the DOM state machine completely and honestly, document the window/`:fullscreen`-CSS limits, and
dispatch to a shell hook when one exists.

## Cookie attribute enforcement — prove flags ACROSS layers, not in the jar (tick 331)

`SameSite`/`Secure`/`HttpOnly` enforcement lives in `engine/net/src/cookies.rs`+`storage.rs` and had
full unit coverage — but a unit test on the jar cannot prove the property that protects a login: that
the flag holds across the JS `document.cookie` shim, the network `Cookie:` header, and the jar all at
once. A wiring bug leaks an `HttpOnly` session cookie to script while every jar unit test stays green.

**(1)** The daily-driver-critical cookie facts are cross-layer: `HttpOnly` must be **hidden from
`document.cookie`** (XSS session-theft mitigation) yet **still ride the wire** (hidden from script, not
from the origin — dropping it logs the user out). **(2)** Gate it as an INTEGRATION test against a real
`TcpListener` (the `g_oauth_redirect` shape): serve `Set-Cookie` headers, load over the net so they
cross the boundary, run the page's script to read `document.cookie`, then pump a `fetch` so the server
observes the real `Cookie:` header. **(3)** RED-prove through the boundary, not the jar: flip the
`document.cookie` read predicate (`|c| !c.http_only`), not a `cookies.rs` internal — that is the layer
the property actually crosses. **(4)** Re-probe before building: "flags unmeasured" / "dead code, 0
callers" were both stale by ~170 ticks; the enforcement was built and wired.

## IME composition — CJK/accented text enters as a committed burst, not a keystroke (tick 332)

**The class of the web this unlocks:** every rich text field for a CJK/hanja/kana/accented-Latin user,
plus mobile autocorrect. These users type phonetic/romanised input into an IME buffer and **commit** a
character — there is no per-glyph `keydown` for the committed text. A browser that only synthesised
`keydown`/`input` for ASCII left a third of the planet unable to type into Gmail compose, a search box,
a comment field.

**(1)** The commit is a fixed ordered BURST, not one event: `compositionstart` → `compositionupdate` →
`beforeinput` → `input` → `compositionend`. A rich editor keys on all of it — it suppresses its
per-keystroke autocomplete/submit while `isComposing` is true and acts on `compositionend`. Firing a
bare `input` makes it treat half-composed phonetic text as a finished word; skipping `compositionend`
leaves it believing a composition is open forever. **(2)** `isComposing` is the guard: `true` on the two
`InputEvent`s, `false` on `compositionend` (the composition has ended). The `if (e.isComposing) return;`
idiom depends on it. **(3)** The value commits through the `.value` **setter, between `beforeinput` and
`input`**, so a controlled component reading `e.target.value` in its `input` handler sees the composed
text — the same contract ASCII keystrokes honour. **(4)** `beforeinput` is the ONLY cancelable step and
carries `inputType: 'insertCompositionText'`: it is the veto point (read-only-while-composing, a
maxlength guard) and the tag an undo stack uses to tell a composition commit from a paste. **The trap:**
modelling only the `input` event reads as "text entry works" from the outside while every IME editor
mis-fires; the burst and the `isComposing`/`inputType` fields are the capability, not decoration.

## `:active` press feedback — the held pointer state that lights on press and releases on lift (tick 333)

**The class of the web this unlocks:** press-state visual feedback on essentially every interactive
control — `button:active { transform: translateY(1px) }`, `a:active { color: … }`, the tab/nav item that
darkens while tapped, the "pressed" affordance a touch UI relies on. It was the last dynamic pseudo-class
left unfed (the Stylo matcher answered a hard `false`), so all of it was dead, silently.

**(1)** `:active` is a HELD state — true from `mousedown` to `mouseup`, not an attribute — so it needs a
live input path, the same shape `:hover` (pointer motion) and `:focus` (focus tracking) already have. Wire
it as a cascade input on the DOM the shell writes on pointer down/up, not as a one-shot on click. **(2)**
It matches the pressed element **and every ancestor** (the press-anywhere-in-this-panel idiom); match only
the exact target and a whole class of container-feedback rules silently fails. **(3)** The
press→restyle→release cycle must recascade with the full stylesheet set both times and must CLEAR on
release — a state only ever added leaves every control the pointer ever touched stuck lit. **The trap:**
feeding `:active` only at the engine level (matcher + state) with no shell input is a "dead-end wire" — it
reads as present at the cascade layer and never lights on a real press. The capability is the full path:
matcher ↔ DOM state ↔ shell pointer feed.

## `:muted` querySelector matching — selecting media elements by mute state (tick 344)

**The class of the web this unlocks:** player-UI scripts that enumerate media by state — `document.querySelectorAll('video:muted')` to find the muted players, style a mute badge, or drive a
"mute all"/"unmute all" control. It joins the state-derived structural pseudo-classes (`:checked`,
`:disabled`, `:required`) the hand-rolled querySelector engine already matches on content attributes.

**(1)** `:muted` matches a `<video>`/`<audio>` carrying the `muted` content attribute — the INITIAL mute
state, exactly as `:checked` matches the `checked` attribute and not the live `.checked` IDL property. It
is the honest, attribute-derived half; the runtime `.muted` property is not tracked here. **(2)** It is a
new `Pseudo::Muted` in the engine we OWN (the querySelector selector engine), not Stylo — one enum variant,
one match arm, one parse arm, mirroring `:checked`. **The fence (not a shortcut):** the *servo* Stylo build
has no `Muted`/`Playing`/`Paused`/`Seeking` variant in `NonTSPseudoClass` (they are gecko-only), so
`video:muted { … }` cannot CASCADE without vendoring Stylo — the identical constraint `:has()` carries.
CSS-cascade styling of player state and the dynamic media pseudo-classes are a Stylo-vendoring tick; the
constellation row stays `partial`, not `works`, to keep that honest.

## HTTP conditional revalidation — the 304 that reuses the body (tick 345)

**The class of the web this unlocks:** every repeat visit and every warm subresource. A browser that
only caches *fresh* responses re-downloads the whole body the instant `max-age` elapses — the CSS, the
JS bundle, the font, the sprite, all pulled again in full on the second page view even though not a byte
changed. Conditional revalidation is how that cost collapses to a header exchange: the server answers
`304 Not Modified` with no body and the browser reuses the copy it already has.

**(1)** The unit is a **stale-but-revalidatable** cache entry. A response that is immediately stale
(`no-cache`, `max-age=0`, or no freshness at all) but carries a validator (`ETag` or `Last-Modified`) must
be **kept, not dropped** — not to serve blind (the fresh-only read still declines it) but so the next
request can *ask*. Dropping it is the bug: it turns every conditional-cacheable resource into a full
re-download. **(2)** The next GET rides `If-None-Match: <etag>` (preferred) and/or
`If-Modified-Since: <date>`; `ETag` wins but both are sent so a server keyed on either can answer. **(3)**
A `304` refreshes the entry's freshness from the *304's own* `Cache-Control` and hands back the **stored**
body — the whole point being that no body crossed the wire; the conditional round-trip still counts as a
wire request (it is one), but the bandwidth saving is the body that didn't move. **The trap:** treating
`no-cache` as "do not store." It means the opposite — *store, but always revalidate before serving* — so a
`no-cache` response with a validator is exactly the case revalidation exists for, and dropping it is
indistinguishable, on a cold second view, from having no cache at all.

## Drag-and-drop editor half — the source→target reorder handoff (tick 346)

**The class of the web this unlocks:** everything reorderable a page drives itself — a sortable list, a
kanban board's card between columns, a reorderable table row, a drag-to-rank UI. These are the *source*
side of drag-and-drop, the half a file drop never touches: the page originates the drag from one of its
own elements rather than receiving an OS file.

**(1)** The capability is the **setData→getData handoff through ONE DataTransfer**. A reorder works only
because the id the source writes on `dragstart` (`e.dataTransfer.setData('text/plain', id)`) is the id the
target reads on `drop` (`getData`) — the *same object* threaded through the whole gesture. Fire `drop`
alone and there is no `dragstart` to populate the transfer, so `getData` returns `''` and the card moves
nowhere. **(2)** The full protocol matters at both ends: `dragstart` on the source, then
`dragenter`/`dragover`/`drop` on the target (which opts in by cancelling `dragover`, exactly as a file
dropzone does), then `dragend` on the source — the notification every drag library uses to clear its
"dragging" class and commit the move. **(3)** `dragend` fires *last*, so a record written during `drop`
misses it; the final state is captured in the `dragend` handler. **The trap:** the synthetic transfer is
built from a files-array shape (`__makeFileList`), so an empty transfer must be `'[]'`, not `'{}'` —
the latter makes `items.length = undefined` throw and silently aborts the entire gesture with no event
delivered at all.

## HTTP `Expires` freshness — the older date-based cache signal (tick 347)

**The class of the web this unlocks:** static assets and CDN responses that predate (or simply prefer)
`Cache-Control` — a huge amount of the images, CSS, JS and fonts on the long-tail web carry an `Expires:
<date>` and no `max-age`. Without honouring it, every one of those is treated as stale on arrival and
re-fetched (or, once revalidation exists, needlessly revalidated) on the next view.

**(1)** `Expires` is an *absolute* deadline; the cache's freshness model is a *relative* lifetime
(`stored + fresh_for`). Convert at store time — the entry was just stored, so `expires - now` is its
lifetime — and it slots in with no second clock. **(2)** Precedence is fixed (RFC 7234 §5.3): `no-cache`
forces revalidation, then `Cache-Control` `max-age`/`s-maxage`, then `Expires`. A response with both a
past `Expires` and a positive `max-age` is fresh; `max-age` wins. **(3)** Reuse the ONE date parser the
cookie jar already ships — a second one is a second thing that can disagree about the same date string.
**The trap:** a past or unparseable `Expires` is not an error to surface, it is simply a zero lifetime —
stale — which then composes with revalidation: kept and conditionally re-checked iff it carried a validator.

## HTTP `Age` header — a CDN response is not as fresh as its max-age says (tick 348)

**The class of the web this unlocks:** anything served through a CDN or shared proxy — which is most of
the modern web's static assets. The origin says `max-age=300`, but the CDN edge has already been holding
the object for 290 seconds and says so with `Age: 290`. Honour only the `max-age` and you serve it as
fresh for a full 5 more minutes when the origin considers it good for 10 — content the origin already
treats as stale. **(1)** Remaining freshness is `lifetime - Age` (RFC 7234 §4.2.3), a plain subtraction
at store time on the lifetime already derived from `max-age`/`Expires`. **(2)** An `Age` at or past the
lifetime is stale on arrival — which then composes with revalidation: kept and conditionally re-checked
iff it carried a validator, dropped otherwise. **The trap:** treating the cache as a private cache that
starts every object's clock at zero — behind a CDN the clock started upstream, and `Age` is the only
thing that tells you by how much.

## MSE playback join — the bytes a player APPENDS are the movie (tick 349)

**The class of the web this unlocks:** adaptive streaming — YouTube's player and every player library
(hls.js, dash.js, shaka, video.js: Twitch, Vimeo, news-site video, course platforms). None of them set
`<video src>` to a media file; they construct a `MediaSource`, set `src` to a `blob:` object URL, and
push segments through `appendBuffer` in an `updateend` loop. For that entire class the network path a
browser normally decodes from **does not exist** — the only copy of the media is the byte-stream the
page accumulated in JS, so a browser that cannot lift it back OUT of the page shows a dead player with
every individual piece working. **(1)** Publish the SourceBuffer's FULL stream on each settled append
that demuxed a video track — an fMP4 decoder needs the init segment plus every fragment as one buffer,
and coalescing to the newest stream per element on the host side makes a burst of appends cost one
decode. **(2)** A re-decode must RESUME (carry transport position + play/pause into the longer
timeline): players append every few seconds, and restart-on-append is a video that never gets past its
own opening. **(3)** An init-only buffer that cannot decode yet is the NORMAL first state of every MSE
session — retry it when the stream grows; the progressive path's "failed once, never retry" discipline
here kills every session at its first append. **The trap:** `isTypeSupported` steering. Advertise only
what genuinely plays end-to-end (here: MP4 + Baseline H.264 + AAC; VP9/webm stay false) — a `true` not
backed by a decoder steers the player OFF its working fallback and onto a `buffered` range whose media
never decodes, turning a degraded-but-working player into a hung one.

## Audio output — the gate must never need the sound card (tick 350)

**The class of the web this unlocks:** everything with sound — the video the tick-349 join made
visible was still MUTE, and to a user a silent video is a broken site, not a degraded one.
**(1)** Split pump from device: the pump (decoded PCM + cursor, chunk-size-agnostic `fill`) is
pure arithmetic a headless test drives sample-exact against the real decode; the device (`cpal`)
is a best-effort wrapper whose absence is the *normal* headless case. A gate that opens hardware
false-REDs on every CI box — gate on decoded-PCM delivery, never audible playback. **(2)** Silence
is a WRITTEN contract: every non-delivering path must zero the whole buffer, because the device
plays whatever is in it and an untouched buffer replays the last callback as a stutter-loop.
**(3)** The device holds an Arc clone from open time, so an MSE re-decode must mutate the feed in
place — a fresh Arc kills the audio on the first append and only `Arc::ptr_eq` can see it.
**The trap:** a RED probe that cannot fire. The obvious cursor bug (advance by chunk size, not
copied count) does NOT corrupt the sample stream — full chunks are equal, it only overshoots at
the tail — so the byte-exact assertion alone was a green that could not go red for that bug; the
exact-landing assertion (`cursor == len` after drain) is what makes it falsifiable. Run the RED
edit and WATCH it fail before trusting any green.

## AV1 playback — organ and registries land together (ticks 353-354)

**The class of the web this unlocks:** AV1-in-MP4 `<video>` — the codec the open web is migrating
to (YouTube serves it first where supported; AVIF stills ride the same decoder next). Decoded in
memory-safe Rust: `re_rav1d` through its safe `dav1d` module, no C, no nasm, behind the
`VideoDecoder` trait M5 defined for exactly this second backend.
**(1)** The organ-then-registry order is a RULE: t353 lands the decoder gated in isolation; t354
ships it in the shell lane and flips ALL THREE honesty registries (isTypeSupported, canPlayType,
`<source type>` certain-no list) in the same tick. A registry ahead of the organ steers players
into a hang; one behind it hides a working capability.
**(2)** dav1d is a QUEUE, not a call: pictures arrive after their sample, pts must ride THROUGH
the decoder as timestamps, and `flush()` is a seek-reset that DISCARDS pending pictures — a
`flush` in the end-of-stream drain silently truncates every stream while looking fully decoded.
**The traps:** (a) a claim label that is a SUBSTRING of another record entry is vacuous —
`contains("av1:true")` was satisfied by `cpt-av1:true`, so the deleted MSE arm kept a green gate;
tripwire-print the record and rename the label. (b) two mozjs contexts in one test binary abort
on thread-local teardown — one JS test per binary, fold claims into the existing JS page.

## AVIF hero images — decode in the lane that owns the decoder (tick 355)

**The class of the web this unlocks:** AVIF stills — modern CDNs (and every image-heavy site
behind them) serve AVIF FIRST, so a browser without the decoder shows a hole where the page's
largest picture belongs. **(1)** The container is not the codec: `avif-parse` walks HEIF to the
primary item's OBUs, the same rav1d that plays `<video>` turns them into pixels — an image format
landed for the cost of a JOIN. **(2)** The isolation rule decides the architecture: the decode
CANNOT live beside `image::load_from_memory` in manuk-page (every gate binary links it), so the
page returns undecodable bytes RAW and the shell decodes and merges into the same
`apply_images_by_url` map. "Honestly undecodable to this crate, decodable to the browser" — the
raw channel keeps both true. **(3)** Refusals are graceful by construction: 10-bit on the 8-bit
build, malformed containers, truncated OBUs are all an `Err` that leaves the image un-rendered
like any broken JPEG — never a panic on network bytes.
**The trap:** asserting "an image decoded" without asserting the COLOR — the solid-red fixture
turns a U/V swap (blue) or a range error (grey wash) into a hard failure instead of a plausible
picture.

## Live media-IDL properties — the write the player's buttons actually perform (tick 360)

**The class of the web this unlocks:** every player UI's mute button and volume slider — they
execute `v.muted = true` / `v.volume = 0.3` (the IDL properties), never `setAttribute`. A browser
that honors only the attribute path renders the controls dead while looking media-complete.
**(1)** Properties that must REACH the host become publishing accessors over a drained host queue
(the clipboard/msePublish shape); the host coalesces to the last write per (node, prop) so a
dragged slider is one gain change. **(2)** Precedence is the spec's: the attribute is the DEFAULT,
the IDL property once set is the LIVE state — implementing attribute-always-wins makes unmuting a
`<video muted>` impossible from script, which is every autoplay-then-unmute player. **(3)** Writes
precede bytes: players set `.muted` at construction, so overrides key by node independently of the
loaded-media entry. **(4)** Gain applies to DELIVERED samples only — the silence contract
(mute/pause/exhaustion writes zeros) is upstream of gain, or a "quiet leak" ships.
**The trap:** a stored-but-silent property reads back correctly forever (`v.muted === true`) while
doing nothing — only asserting the host-side drain catches it ("got []").

## playbackRate — scaled time without the chipmunk (tick 361)

**The class of the web this unlocks:** the speed control on every video/podcast player (1.25-2x is
how a large share of lecture/podcast content is actually consumed). **(1)** Rate lives on the
transport clock and scales the WALL path only; a device consuming at 1x must NOT govern a scaled
transport (mastery refusal), and the snap-back on returning to 1x is correct — the audio position
is where the sound is. **(2)** Without time-stretch, rate≠1 MUTES regardless of what else asked:
pitch-shifted audio is the defect users hear instantly; silent scaled video is degraded-and-honest.
The audible rung (WSOLA-class stretch) is named residue, not smuggled.
**The trap:** applying rate by scaling the AUDIO clock's position — the device consumes real
seconds; only the transport's wall path may scale, or sync arithmetic silently corrupts.

## Raw-stream audio — the podcast class end-to-end (ticks 362-363)

**The class of the web this unlocks:** `<audio src="episode.mp3">` — podcasts, previews, legacy
audio everywhere. A raw MPEG stream is NOT an MP4 track: it needs a format PROBE (symphonia's, one
seam that will serve FLAC/Ogg), not a box parser. **(1)** Gate the CLOCK, not activity: a 10s file
must decode to ~10s of frames — a decoder dropping packets passes every produced-samples check.
**(2)** An audio-only playback entry has no transport: the FEED is the playhead (device consumes,
position reports, exhausted is ended), no frame is ever published, and policy that consulted the
transport (the chipmunk rule) must derive from the requested value instead. **(3)** Organ, then
join+registry, never registry first: canPlayType said '' until the shell could actually route the
stream, then flipped in the same tick as the join.
**The trap:** metadata tags (ID3v2 with an embedded PNG) sit BEFORE the sync word — a prober that
treats tag bytes as sync kills the stream; assert the tagged fixture decodes.

## FLAC + Ogg/Vorbis — free rungs of an existing seam; Opus as a named wall (tick 364)

**The class of the web this unlocks:** lossless audio (`audio/flac` — music archives, audiophile
players) and legacy Ogg/Vorbis (`audio/ogg` — game wikis, older podcast archives, Wikipedia media).
**(1)** A well-placed seam makes codecs near-free: three symphonia features and a two-brand sniff
widen, zero new decode code. **(2)** The sniff ROUTES, the probe DECIDES: Opus-in-Ogg sniffs yes
and then refuses downstream as a named error — a sniff that tried to be the authority would need
codec knowledge it cannot have. **(3)** The bare-container 'maybe': `audio/ogg` without codecs may
be Vorbis (plays) or Opus (refused), so the honest canPlayType answer is exactly 'maybe' — 
'probably' only when vorbis is NAMED. **(4)** Only a CERTAIN no belongs on a source-selection
reject list; Ogg left it the tick the seam could read it.
**The trap:** the silent-vanish class again — narrowing a sniff makes a whole format's loads die
while every suite stays green; only a load-this-exact-fixture gate claim catches it.

## AVIF alpha — the mask is a picture, and the fixture can lie (tick 368)

**The class of the web this unlocks:** transparent heroes, logos and product shots — AVIF's alpha
rides a separate auxiliary AV1 image, and ignoring it paints the encoded background over the page.
**(1)** The aux image is monochrome and its Y plane IS the mask — decode it with the same decoder
but NEVER through the color matrix, which would rescale the mask's numbers. **(2)** Deliver
STRAIGHT alpha (un-premultiply per the container flag): src-over compositing double-darkens edges
otherwise. **(3)** The negative claim matters as much: an alphaless file must stay A=255 — an alpha
path that fires on everything fades the web.
**The trap:** fixtures lie by NAME — Blink's `alpha-mask-*` files ARE masks (alpha_item=None), so a
gate on one can never observe compositing. Probe the fixture's actual structure (10 lines of
avif-parse) before debugging the code it "proves" broken.

## WAV — the RIFF form-type routing nuance (tick 369)

**The class of the web this unlocks:** audio/wav — notification sounds, previews, TTS output.
**The one insight:** RIFF alone is NOT an audio signature (AVI and WebP are RIFF too); routing on
4 bytes sends video containers into an audio probe. Check the form type at offset 8 and assert the
NEGATIVE (an AVI-shaped RIFF must not route) alongside the positive.

## The audio mixer — N elements, one device, honest degradation (tick 370)

**The class of the web this unlocks:** any page with more than one sound — a video plus a
notification, two players, a game with effects. One-stream-wins renders every later element mute.
**(1)** Keep the mix PURE (a function over the feed-set) so the gate drives it headlessly — the
same pump/device split that made t350 gateable. **(2)** Hard-clamp the sum: two loud streams
overflow ±1 into device distortion. **(3)** Config mismatch = SILENT SKIP, never reinterpretation:
pulling 48k frames at a 44.1k device is a pitch shift that sounds like playback. **(4)** Mastery
follows consumption: with a mixer every contained feed is consumed, so sync mastery is
membership, not identity-with-the-one.
**The trap:** a clamp claim the fixture cannot trigger — the real stream was too quiet to clip and
the RED edit passed; synthesize the loud case or the claim measures nothing.

## Cross-rate mixing — resample on the source's clock (tick 375)

**The class of the web this unlocks:** pages mixing sample rates — a 48k notification beside 44.1k
video, TTS beside music. **(1)** Linear interpolation is policy arithmetic and speech-grade; name
the quality rung (windowed-sinc) instead of silently shipping it. **(2)** THE contract: the source
cursor advances at the SOURCE's rate — it is also the sync master's clock, and consuming at the
device rate is a pitch shift that corrupts mastery arithmetic too. **(3)** Constant-in →
constant-out is the wobble gate: interpolation reading wrong neighbours invents modulation a
spectrum would show but a length check never will.

## Scroll promises + the synchronous read-back contract (tick 378)

**The class of the web this unlocks:** post-scroll code — `await scrollTo(...)` (the
Baseline-crossing 2026 idiom replacing settle-timers) AND the far older `scrollTo(0,40);
if (scrollY === 40)` next-line read that a request-model viewport silently broke.
**(1)** A promise that resolves before the effect is applied is a LIE with a .then on it — the
gate's awaited continuation caught the tick's own premise (scrollp:false) before it shipped.
**(2)** Request-model state needs an optimistic local echo: update the page-visible position at
request time; the owner's application overwrites with the clamped truth, so out-of-range requests
over-report only transiently — the trade real browsers make invisible by clamping synchronously.
**The trap:** "immediate resolve is truthful because the operation is instant" — instant for the
OWNER is not instant for the OBSERVER when the operation crosses a request boundary.

## Container queries — the rung-3 source supplement + sized re-pass (tick 379)

**The class of the web this unlocks:** component-responsive layout — design-system components
(cards, navs, sidebars) that restyle by their CONTAINER's inline size, not the viewport. The
dominant post-2023 CSS architecture; every major design system ships @container rules, and a
browser that drops the block wholesale renders their narrow-container variants wrong everywhere.
**(1)** When a vendor engine cfg-drops a feature (compile-time, not a pref), rung 3 is: lift the
blocks from raw sheet source and hand the pieces to the vendor's own PUBLIC parsers — never
hand-parse the grammar (`ContainerCondition::parse` + `Stylesheet::from_str`, the :has()
precedent upgraded).
**(2)** Size-dependent style needs a re-pass: cascade → layout → re-cascade with pass-1 sizes →
re-layout, with container-gated rules held OFF on the unsized pass — unknown must never style, so
feature-detect fallbacks stay honest.
**The trap:** re-wrapping lifted blocks without their ENCLOSING @media/@supports/@layer preludes
silently un-gates them.

## Multi-byte at-rule names — hostile bytes vs length-guarded slices (tick 381)

**The class of the web this unlocks:** any site whose CSS carries non-ASCII at-rule-shaped
tokens — i18n custom at-rules, minifier artifacts, or plain hostile bytes (netlify.com shipped
one and the whole engine died). Crash-robustness IS a rendering-parity feature: Chrome renders
the page, we rendered a corpse.
**(1)** A byte-length guard (`rest.len() >= 6 && rest[..6]`) is NOT a boundary guard: UTF-8
slicing panics mid-character. `str::get(..n)` folds the boundary check into the keyword match —
None means "not this keyword", which is exactly CSS's skip-unknown recovery.
**The trap:** the pattern passes every ASCII test you'll ever write; only real-web bytes find it.
The tick-380 oracle crawl is what surfaced it — measurement finds what unit tests cannot.

## A differential instrument needs a health check on BOTH sides (tick 383)

**The class of the web this unlocks:** none directly — it stops the exit instrument LYING about
the classes we already handle. The tick-380 ledger's "author-style-not-applied" trio (49/43/39
sites) largely evaporated on quiet re-runs: crawl-load fetch starvation had rendered pages
UA-default and charged the difference to the engine.
**(1)** The one-snapshot rule must extend to everything that styles the page — pinning the HTML
while the CSS rides live network weather pins nothing.
**(2)** `oracle_is_healthy` guarded the reference engine only; the measured engine can degrade
the same way. `Page::failed_stylesheet_fetches()` + discard-on-starved is the symmetric guard.
**The trap:** a divergence count that MOVES between identical runs is not a measurement yet —
before acting on any ledger family, re-run one affected site on a quiet box and see if the
family survives.

## Computed values are an observable surface — don't encode layout policy in them (tick 384)

**The class of the web this unlocks:** every page an author or framework feature-detects with
`getComputedStyle(el).display` on replaced elements, and the whole corpus-diff signal for them
(81/80 sites on img/svg). A cascade that mutates computed values to steer its own layout is
lying to every OTHER consumer of those values.
**(1)** The spec's computed value and the layout treatment are separate contracts: `<img>` is
`inline` AND atomic. Encode atomicity where it is consumed (the layout routing), not where it is
reported (the style map).
**The trap:** the mutation is invisible until something diffs you against a real browser — this
one lived through ~380 ticks and two cascades.

## Control-flow items can still be elements — a Break owns a box (tick 385)

**The class of the web this unlocks:** editors, caret/selection libraries, and any script doing
`getBoundingClientRect()` on `<br>` to find line ends (64 corpus sites carry measurable brs).
**(1)** An inline item that ENDS a line is still an element IN the line: closing the band and
recording the element's geometry are two responsibilities, not one.
**The trap:** the empty-line case worked (`<br><br>` opens a band attributed to the br), so
spot-checks passed; only the corpus diff showed the common case — br after text — had no box.

## Recovered properties have ORDER, not just presence (tick 388)

**The class of the web this unlocks:** auto-growing comment boxes / chat inputs — every modern
form that writes `field-sizing: content` (Baseline June 2026) instead of a JS autosize library.
**(1)** A property recovered from a second parser is only correct if it lands before its
CONSUMERS run: `field-sizing` vetoes a presentational hint applied mid-walk, so the generic
after-the-walk recovery merge silently loses the race.
**The trap:** the probe passes with the property parsed and the width still wrong — measure the
BEHAVIOR (the box hugged), never the parse.

## Auto on a replaced element is never "fill" and never "zero" (tick 389)

**The class of the web this unlocks:** unsized inline SVG — the icon/logo idiom — and unsized
canvas/video/iframe embeds; plus every icon-only button those were collapsing into dead targets.
**(1)** The replaced-sizing fallback chain is: author size → ratio-derived size → DEFAULT OBJECT
SIZE (300×150) — and the last rung must live in used-size layout, after the first two resolved,
never in UA defaults where it outranks author CSS.
**The trap:** the failure is invisible twice over — a 0-height box paints nothing to screenshot,
and a full-width box looks "laid out" in a box dump. Only the reference diff named it.

## Measure the reference before pinning a model (tick 391, reaffirming t264's rule)

**The class of the web this unlocks:** every viewBox-only inline svg — logos, illustrations,
icon sprites — now shaped by their ratio instead of a fixed 300×150 guess.
**(1)** t389 pinned "default object size" from the spec's headline number; one headless-Chrome
measurement showed the ratio case takes available-width×ratio instead. A gate pinned to a
recalled model is a gate that locks the wrong behavior in — measure first, pin second.
**The trap:** the wrong pin PASSED its own test; only re-deriving the truth from the reference
engine exposed it. Same lesson as [[gate-measured-against-a-standard-chrome-fails]].

## Inline vectors ride the raster image path (tick 394)

**The class of the web this unlocks:** the vector half of every modern page — inline SVG icons,
logos, illustrations, chart glyphs — visibly painted instead of blank squares.
**(1)** When an engine already vendors a renderer for one entry point (`<img src="*.svg">` via
usvg/resvg), the inline case is a SERIALIZATION problem, not a rendering problem: subtree →
markup (+ the xmlns the HTML parser dropped) → the same decode path.
**(2)** Assert on PIXELS, not on the decode returning Some — a decoded image that never reaches
the display list is the actual failure mode (self.images gets REPLACED every apply_images round;
the cache-and-merge is the load-bearing half).
**The trap:** the sync construction paths (`load`, `from_prefetched` — the SHELL's path) never
pass through `apply_images`; hook only the fetch path and every offline/gate/shell page stays
blank while the fetch path works.

## document.location is the login-flow's URL read (tick 402)

**The class of the web this unlocks:** identity/SSO components and legacy redirect flows —
every SPA whose auth widget reads `document.location.search` for its callback params (okta's
Identity components die in their async mount without it), plus every page using the legacy
`document.location = url` redirect idiom and `document.URL`/`documentURI` reads.
**(1)** `document.location` IS `window.location` per spec — and when the location shim is
REPLACED wholesale on SPA navigation (`__applyUrl`), the alias must be an ACCESSOR; a copied
reference is a first-pushState time bomb.
**(2)** The t401 selector-path-keyed oracle converts silent phantom diffs into NAMED console
errors — this fix is the first harvest of that pipeline: error names organ, organ gets a gate.
**The trap:** `history_bindings::install` carries a native Location and LOOKS like the live
surface — it is dead code nothing calls; the prelude shim is the one BOM surface. Fixing the
dead one would have changed nothing (the two-sources-of-truth class again).

## .getPropertyValue(x).trim() is written as one expression (tick 403)

**The class of the web this unlocks:** every style-reading utility that chains directly off
getPropertyValue — theme detectors, CSS-variable readers, feature probes ("is backdrop-filter
set?"), animation libraries reading a property they never set. They all write
`getComputedStyle(el).getPropertyValue(p).trim()` in ONE expression, so a partial accessor
kills the caller's whole async frame (okta.com, verbatim).
**(1)** CSSOM accessors are TOTAL functions: the unknown answer is `''`, never undefined —
a partial map plus "return the lookup" is a contract violation wearing a working demo's face.
**The trap:** the FALLBACK object is part of the surface too — a no-style `({})` fallback
throws "not a function" on the same line the main path merely returns undefined from; both
spellings of the same missing contract.

## Chunk loaders find themselves via document.currentScript (tick 404)

**The class of the web this unlocks:** every webpack/Rollup/parcel chunk loader that resolves
its own <script> tag for nonce, data-config and base URL (publicPath:"auto" is literally a
currentScript read) — the load-time bootstrap of most code-split SPAs (okta's stubScriptElement
stash, verbatim).
**(1)** currentScript is a LIFETIME property, not a lookup: the executing element during a
classic evaluation, null outside it and inside modules — a thread-local set/cleared around
evaluation is the honest shape.
**The trap:** a hardcoded null LOOKS spec-shaped (it is the right answer for modules and
callbacks) and passes every after-the-fact probe — the lie is only visible DURING execution,
which is exactly when chunk loaders read it.

## getAllRecords() returns keyed records in one request (tick 420)

**The class of the web this unlocks:** every offline-first app that pages a keyed range and needs
BOTH the record and its key back — Dexie 4's `getAllRecords`-backed bulk reads, the `idb` helper's
range queries, the Firebase/Cognito offline persistence layers, and any Interop-2026-targeting app
built against the new IDB surface. Before this, an index range query that wanted key+value cost two
requests (`getAll` + `getAllKeys`) zipped by hand; a library that reached for the one-call form found
`store.getAllRecords === undefined` and threw inside its own promise — the app "just doesn't load".
**(1)** A record is `{ key, primaryKey, value }`: on a STORE `key === primaryKey`, on an INDEX they
DIFFER (`key` = index key, `primaryKey` = store key). Returning the pair already zipped is the whole
point — the caller must not have to re-join `getAll` against `getAllKeys`.
**The trap:** a `getAll` stand-in wearing the `getAllRecords` name passes every store-side probe
(where `key === primaryKey` anyway) and only lies on an INDEX, where `key !== primaryKey` — so the
gate proves the split on an index, not the easy store case.

## structuredClone preserves binary types, or the copy is silent corruption (tick 421)

**The class of the web this unlocks:** anything that deep-copies or messages BINARY data — a Web Worker
receiving a Uint8Array of decoded audio/image bytes over postMessage (Manuk routes messaging through the
same shim), a state library structured-cloning a store that holds an ArrayBuffer, a WASM host copying a
typed-array view, a crypto.subtle caller. The old shim cloned arrays/Date/Map/Set/cycles but degraded a
typed array to a plain `{0:.., 1:.., length:..}` object — the bytes were present but the TYPE was gone,
so `clone instanceof Uint8Array` was false and every byte read was garbage.
**(1)** A structured clone is TYPE-preserving: a Uint8Array clones to a Uint8Array, an ArrayBuffer to an
INDEPENDENT ArrayBuffer, a DataView to a DataView, a RegExp to a RegExp — and two views SHARING one
buffer clone to two views over ONE cloned buffer (buffer identity survives).
**The trap:** a typed array is `typeof x === 'object'` and not an Array/Date/Map/Set, so the generic
object-copy branch silently swallows it — the copy has the right keys and the wrong type, which is the
one failure mode worse than a throw because it looks like it worked.

## A Blob holds bytes, not String(part) (tick 422)

**The class of the web this unlocks:** anything that makes a Blob out of BINARY data — a decoded image
or audio buffer wrapped for an object URL, a file-upload body assembled from a Uint8Array, `canvas.toBlob`,
a Blob posted as a `fetch` body, a drag-and-drop file read through `FileReader`. The shim stored parts
as a UTF-16 string via `String(p)`, so `new Blob([new Uint8Array([1,2,3])])` held the text `"1,2,3"` —
size 5, the wrong bytes — and every binary consumer downstream read garbage.
**(1)** A Blob is a byte sequence. A binary part (ArrayBuffer / typed-array view / DataView) contributes
its RAW BYTES; a typed-array view contributes only its own window of the buffer (`byteOffset`/
`byteLength`), not the whole backing store.
**The trap:** `String(typedArray)` is `"1,2,3"` and `String(arrayBuffer)` is `"[object ArrayBuffer]"` —
both look like "we handled it", both are silent corruption. And the neighbouring stub was worse:
`FileReader.readAsArrayBuffer` returned `new ArrayBuffer(0)`, an empty buffer that throws no error and
loses every byte.

## Canvas pixels must be writable, not just readable (tick 423)

**The class of the web this unlocks:** every canvas image-processing routine — grayscale/blur/threshold
filters, histograms, barcode and QR readers, in-browser image editors, and the CPU fallback path of
WebGL/Three/Pixi demos. They all build a `Uint8ClampedArray`, wrap it in `new ImageData(...)`, and blit
it with `putImageData`. The canvas could READ pixels (`getImageData`) but `putImageData` was a no-op and
`ImageData` did not exist, so a filter ran, wrote nothing, and left the image untouched — no error.
**(1)** `putImageData` REPLACES pixels: it ignores the transform, `globalAlpha` and compositing (unlike
every draw op) — a raw blit of the source rectangle. Implement it as a direct pixel write, not a
`fillRect`, or the alpha and transform silently corrupt the result.
**The trap:** a canvas with a working `getImageData` LOOKS like it has pixel access — the read half
passes every probe — while the write half is a `function(){}` stub that discards silently. A round-trip
gate (put then get and compare) is the only thing that catches it.

## TextDecoder must honour its label, not decode everything as UTF-8 (tick 424)

**The class of the web this unlocks:** every page that reads NON-UTF-8 bytes through the JS TextDecoder
API — a Windows-authored windows-1252 CSV or HTML file dropped into an editor, a `fetch(...).arrayBuffer()`
that a script decodes with the response's declared charset, a binary protocol (some WebSocket framings,
older APIs) that carries text as UTF-16. The shim ignored the `label` and always decoded UTF-8, so a
single byte over 0x7F came back as `Ã©`-shaped mojibake, silently.
**(1)** `new TextDecoder(label)` MUST honour the label. windows-1252 (the `latin1`/`iso-8859-1` family)
is a single-byte encoding whose 0x80-0x9F block is punctuation (€, curly quotes, — …), not the C1
controls raw Latin-1 puts there; utf-16le/be are two bytes per unit and endianness matters.
**The trap:** UTF-8 is a superset of ASCII, so a label-ignoring decoder LOOKS correct on every English
test string and only corrupts once a byte exceeds 0x7F — which is exactly the accented/CJK/symbol content
the non-UTF-8 encoding existed to carry. Test a byte over 0x7F, or the bug hides.

## A parsed <template>'s .content holds its children, not an empty fragment (tick 425)

**The class of the web this unlocks:** every compiler-based framework's DOM instantiation — lit-html,
Svelte, Solid, and Vue's compiled render functions parse a `<template>` once and `template.content
.cloneNode(true)` (or `.content.firstChild.cloneNode(true)`) per instance. The element and its `.content`
fragment both existed, but for a PARSED template the fragment was EMPTY, so the clone brought nothing and
the component rendered blank with no error.
**(1)** The HTML parser puts a `<template>`'s children in its content fragment, NOT as direct children of
the element — so `.content` must read the parser's fragment, and `template.childNodes` is empty by design.
**The trap:** two storages for "the template's contents" (the parser's fragment vs a lazily-moved copy of
the element's direct children) look interchangeable until you notice the parser fills one and the accessor
reads the other. The imperative `createElement('template')+innerHTML` path hid the bug — its children ARE
direct children, so it limped while the parser path was silently empty.

## url.searchParams is live; building a URL with query params depends on it (tick 426)

**The class of the web this unlocks:** every paginator, filter bar, sort control and API client that
assembles a request as `const u = new URL(page); u.searchParams.set('page', String(n)); fetch(u)`. The
`searchParams` object existed but was a dead SNAPSHOT — the mutation never touched `u.href`/`u.search`,
so the ORIGINAL url was fetched and the page "didn't paginate" with no error. And `new
URLSearchParams(new FormData(form))` — the standard form-to-query-string idiom — read the FormData's
methods as keys.
**(1)** `searchParams` is LIVE: a `set`/`append`/`delete`/`sort` rewrites `search` and `href` (preserving
the `#hash`, dropping the `?` when the query empties). Its constructor accepts any ITERABLE of pairs — a
FormData, a Map, another URLSearchParams — not only a literal array.
**The trap:** a snapshot `searchParams` passes every read-only test (`u.searchParams.get('x')` is right)
and only fails once something MUTATES it and then reads the URL back — which is exactly what "build a URL"
means. And `Array.isArray` looks like "handle the sequence form" but silently excludes every other
iterable of pairs.

## Computed CSS custom properties reach getComputedStyle (tick 427)

**The class of the web this unlocks:** every runtime that reads a design token — a chart library pulling
`getComputedStyle(document.documentElement).getPropertyValue('--color-primary')`, a component that reads
`--gap` to size a canvas, and the CSS-in-JS / design-system runtimes (many read computed `--vars` to
bridge CSS and JS). The computed-style object exposed only the fixed longhand map, so every `--x` read
came back `''`, the token was "missing", and the component fell to a hardcoded default or drew nothing.
**(1)** `getComputedStyle(el).getPropertyValue('--x')` returns the CASCADED, inherited, `var()`-expanded
value — custom properties inherit, so a `--brand` set on `:root` is readable on any descendant.
**The trap:** custom properties are not longhands, so a getComputedStyle built from a fixed property map
silently omits them — and the omission is invisible until a page actually reads a token, which is exactly
what a themed site does on every component. The engine (Stylo) already computed them; only the plumbing
to the CSSOM object was missing.

## :open must match in the querySelector engine, not only the style cascade (tick 429)

**The class of the web this unlocks:** disclosure-widget and accessibility code that ENUMERATES open
panels — `document.querySelectorAll('details:open')` to close the others (accordion "only one open"),
an a11y audit that finds open dialogs, a component that reacts to `el.matches(':open')`. The style
cascade already styled `details:open`, so the visual side looked done — but the JS selector engine is
separate, so `querySelectorAll('details:open')` returned nothing and the accordion logic silently
no-op'd.
**(1)** A browser has TWO selector matchers: the CSS cascade (Stylo here) and the querySelector/matches/
closest engine. A pseudo-class must be taught to BOTH or they disagree — the cascade paints it, the
script can't find it.
**The trap:** testing `:open` only by whether `details:open {color}` renders passes while
`matches(':open')` is still false. Test the JS engine (querySelectorAll/matches) explicitly, or the
half that scripts depend on stays broken.

## event.getModifierState reads modifiers; shortcut libs call it, not e.ctrlKey (tick 430)

**The class of the web this unlocks:** every keyboard-shortcut library and rich-text editor —
Mousetrap, CodeMirror/ProseMirror keymaps, the ubiquitous Cmd+K command palette — reads a modifier via
`e.getModifierState('Control')` rather than `e.ctrlKey`, because it also answers `AltGraph`/`CapsLock`/
`NumLock`. Absent, the call threw `not a function` inside the keydown handler and the whole shortcut
dispatch died.
**(1)** `getModifierState(name)` lives on the MODIFIER-BEARING events (Mouse/Keyboard/Wheel/Pointer) and
maps the standard key names onto the boolean flags; a plain `Event`/`CustomEvent` does not have it.
**The trap:** the flags (`e.ctrlKey`) worked, so direct reads passed while the spec accessor every
library actually uses was missing — and a throw inside a keydown handler takes the entire keymap down,
not just one shortcut.

## element.scrollTo/scrollBy scroll the container, not just scrollTop= (tick 431)

**The class of the web this unlocks:** every programmatic scroll — a "scroll to top" button, a chat pane
pinning to the bottom (`el.scrollTo(0, el.scrollHeight)`), a virtualised list jumping to an index, a
carousel's prev/next, `el.scrollTo({ top, behavior: 'smooth' })`. Assigning `el.scrollTop` worked, but
the methods every framework and helper actually call were missing, so `el.scrollTo is not a function`
threw and the control silently did nothing.
**(1)** `scrollTo(x, y)` / `scrollTo({ left, top, behavior })` (and its `scroll()` alias) and the relative
`scrollBy(...)` are the ergonomic scroll API. They should REUSE the `scrollTop`/`scrollLeft` setters so
they inherit clamping and scroll-snap — reimplementing the scroll math in the method would drift.
**The trap:** `scrollTop = n` passing makes scrolling look done, while the method form the ecosystem uses
is absent — and a throw inside a click handler kills the whole interaction. And `behavior: 'smooth'` must
be ACCEPTED (ignored is fine — jump to the correct position) not rejected, or the option throws.

## CSSStyleDeclaration is array-like and separates value from !important (tick 432)

**The class of the web this unlocks:** every library that ENUMERATES a style declaration — copying a
computed style onto another element, a CSS-in-JS serializer walking `for (i=0;i<s.length;i++) s.item(i)`,
an animation lib reading every property — and everything that reads or sets an `!important` from JS.
`getComputedStyle(el)` had no `.length` at all and neither declaration had `.item`, so the enumeration
loop threw `s.item is not a function`; `setProperty(k, v, 'important')` silently dropped its third arg.
**(1)** A `CSSStyleDeclaration` is BOTH a map (`getPropertyValue`/`setProperty`) and an array (`.length`,
`.item(i)` → the property NAME, indexed `s[i]`). `item(i)` past the end returns `''`, not `null` (the
CSSOM contract differs from FileList/DOMRectList here).
**(2)** A value and its PRIORITY are separate: `getPropertyValue` and a camelCase read (`s.color`) return
the value ALONE, `getPropertyPriority` returns `'important'`, `cssText` keeps the raw `!important` text.
Computed style's `getPropertyPriority` always returns `''` — a computed value never carries a priority.
**The trap:** `getPropertyValue`/`setProperty` (the map half) passing makes the declaration look done,
while the array half every serializer uses is absent — and reusing the parse/write helpers for priority
(strip on read, append on write) keeps `cssText` as the single source of truth instead of a shadow flag.

## form.elements is a live HTMLFormControlsCollection with named access (tick 433)

**The class of the web this unlocks:** every form-serialization and validation library, and every page
that reads a control by name off its form. `form.elements` was `undefined` ENTIRELY, so the canonical
`for (i=0;i<form.elements.length;i++)` loop and `form.elements['field']` / `.namedItem('field')` all threw
`can't access property … form.elements is undefined` — the first line most form code runs.
**(1)** Its members are the LISTED controls in tree order — button/fieldset/input/object/output/select/
textarea — MINUS `input[type=image]` (a submit button the collection omits). Indexed access, `.length`,
`.item(i)`, `.namedItem(name)`, and named access by `name` (HTML ns) then `id` all resolve against them.
**(2)** The named getter returns a **`RadioNodeList`** when >1 control shares a name — a radio group — and
that list's `.value` READS the checked radio's value and WRITING it selects the matching radio. Return a
single element there and `form.elements.plan.value` silently yields the FIRST radio, not the selected one.
**The trap:** a plain HTMLCollection reused for `form.elements` looks done — until a radio group is read
through it, or an image button appears as a phantom control. It gets a self-contained builder rather than
routing through the hot `live()` childNodes proxy, whose own note records that enriching its traps once
surfaced a cross-file UAF. KNOWN LIMIT (honest): association is by SUBTREE, not the `form=` attribute
reassociating a control elsewhere in the document (the ~99% case; `form=` is a follow-on).

## control.labels and label.control link a form field to its <label> (tick 434)

**The class of the web this unlocks:** every accessibility helper and form library that reads the text
NAMING a control, and every "click the label to focus the field" behaviour. Both `input.labels` and
`label.control` were `undefined`, so `input.labels[0].textContent` (the accessible-name walk) threw and a
`<label for=x>` had no live link to its control.
**(1)** `label.control` resolves the `for=` attribute to its target IF that target is labelable, else the
FIRST labelable DESCENDANT (`<label><input></label>` with no `for=`). Labelable = button / input (NOT
hidden) / meter / output / progress / select / textarea.
**(2)** `control.labels` is a NodeList (recomputed per read, static within a read) of every `<label>` whose
`.control` resolves back to this element, in tree order — a control can carry more than one label. A hidden
input is non-labelable, so its `.labels` is `null` (not an empty list), and a `<label for=hidden>` claims
no control.
**The trap:** returning the first `<label for=id>` alone misses both the second label and the containment
form; and treating a hidden input as labelable makes `<label for=hidden>` falsely claim it. It uses a
STATIC NodeList, not the hot `live()` childNodes proxy, whose heap sensitivity is documented for tick 129.

## The <table> DOM: table.rows/tr.cells and row/cell indices (tick 435)

**The class of the web this unlocks:** every data-grid, sortable-table and spreadsheet-lite widget that
reads or renumbers a table through the DOM, and every accessibility walk that reports "row R, column C".
The whole read surface (`table.rows`, `table.tBodies`, `table.tHead`/`tFoot`, `tr.cells`, `tr.rowIndex`,
`tr.sectionRowIndex`, `td.cellIndex`) was `undefined`.
**(1)** `table.rows` is a LIVE HTMLCollection in **logical** order — thead rows, then tbody + direct
`<tr>` rows in tree order, then tfoot rows — NOT document order. A sort widget that reads document order
mis-numbers any table whose `<tfoot>` is authored before its `<tbody>` (a common pattern, since the spec
lets the footer precede the body in source).
**(2)** `rowIndex` is the index in `table.rows`; `sectionRowIndex` is the index within the row's own
section; `cellIndex` is the index in `tr.cells`. Each is -1 when the element is unparented.
**The trap:** returning rows in document order looks right on the common `<thead><tbody><tfoot>` source
and is silently wrong on the reordered one — which is exactly why the gate authors the fixture with the
footer first. Collections reuse the existing HTMLCollection live() path, never the childNodes NodeList one.

## The <table> write API: insertRow/deleteRow/insertCell + section methods (tick 436)

**The class of the web this unlocks:** every grid/spreadsheet/data-table widget that BUILDS or edits a
table through the DOM instead of innerHTML — the classic non-framework pattern, and what a lot of shipped
table code still emits. The whole write side (`table.insertRow`, `tr.insertCell`, `createTHead`/`TFoot`/
`TBody`/`Caption`, and the `delete*` inverses) was `undefined`.
**(1)** The index rules are exact and code branches on them: `-1` means "at the end", an out-of-range
index is an **IndexSizeError** (a THROW, never a clamp), and inserting a row into an EMPTY table
**materialises a `<tbody>`** rather than dropping a bare `<tr>` into the table.
**(2)** `createTHead`/`createTFoot` REUSE an existing section (idempotent — libraries call them defensively);
`createTBody` always makes a new one; `createCaption` inserts the `<caption>` as the first child.
**The trap:** clamping an out-of-range index looks friendlier but silently corrupts a widget that inserts
at a computed position and catches the throw; and appending a bare `<tr>` to an empty `<table>` produces a
row with no section, which then does not appear in `table.rows` (t435's logical-order reader).

## element.form resolves the form owner (tick 437)

**The class of the web this unlocks:** every form library that groups controls by their owning form
(`input.form === thisForm`), every framework that reads `el.form` to decide where a control submits, and
`ElementInternals.form` for form-associated custom elements. `input.form` was `undefined` — including the
`form=` reassociation case.
**(1)** The owner is: if the element carries a `form=` attribute, the element with that id **iff it is a
`<form>`** — an id pointing at a non-form yields NULL (per spec), NOT the nearest ancestor; otherwise the
nearest ancestor `<form>`. This is what lets a control live OUTSIDE its form (a common layout escape) and
still belong to it.
**(2)** An `<option>` reports its `<select>`'s owner; a `<label>` reports its labeled control's owner; a
non-form-associated element (`<div>`) has no such property at all.
**The trap:** falling back to the ancestor form when `form=` names a non-form is the intuitive-but-wrong
behaviour — the spec makes a dangling `form=` yield null, because the author explicitly opted the control
OUT of its ancestor and into a form that does not exist.

## the <select> write API — add / remove(index) + options collection (tick 438)

**The class of the web this unlocks:** every JS-driven `<select>` that populates or edits its own options
at runtime — country/region pickers, dependent/cascading dropdowns, "add another row" forms, and any
widget that builds a `<select>` from fetched data. The READ side already worked; the WRITE side was
silently wrong two ways.
**(1)** `select.add(element[, before])` was `undefined` — the primary insertion method threw. `before` is
null/omitted → append, a number → insert before `options[n]` (append if out of range), or an element →
insert before it in its own parent (which may be an `<optgroup>`).
**(2)** `select.remove(0)` DETACHED THE WHOLE SELECT: with no own `remove(index)` the call fell through to
the inherited `ChildNode.remove()`, which ignores the argument and tore the control out of its `<form>`.
`remove(index)` now removes `options[index]`; `remove()` with no argument keeps the legacy detach-self
overload, and `div.remove()` is untouched. `select.options.namedItem`/`add`/`remove` are also present now.
**The trap:** a naive `EP.remove` override to fix `select.remove(index)` would break `div.remove()` for
EVERY element — the fix must delegate to the native `ChildNode.remove` on every path except
`select.remove(<index>)`, preserving the spec's overload.

## option.text + Option() defaultSelected (tick 439)

**The class of the web this unlocks:** every page that reads the LABEL of a chosen `<select>` option
(`select.options[select.selectedIndex].text` — the canonical way to get the human-readable choice) and
every `new Option(label, value, true)` that builds a pre-selected option in JS.
**(1)** `option.text` was `undefined` (a plain expando). It now returns the option's text content with
ASCII-whitespace runs collapsed and trimmed (spec), and is settable (replaces the content).
**(2)** `new Option(text, value, defaultSelected)` ignored its 3rd argument; it now sets the `selected`
attribute when `defaultSelected` is truthy, so the constructed option is selected as authored.
**The trap:** putting `text` on the element prototype must not eat the ordinary `div.text = x` expando —
the getter is `undefined` for non-options and the setter materialises a normal own property there, so no
non-option `.text` assignment regresses.

## textarea.value reads the text content (tick 440)

**The class of the web this unlocks:** the entire "edit existing content" web — every server-rendered form
with a pre-filled `<textarea>` (edit a comment, a bio, a post, a description, a config blob). Reading
`textarea.value` returned `""`, so the field looked empty to JS, dirty-checks fired wrong, and re-submitting
wiped the content. It also corrupted insert-at-cursor editors: `setRangeText` shared the same broken value
source and replaced the whole field instead of the selection.
**(1)** A `<textarea>`'s raw value is its child TEXT CONTENT (until dirtied), NOT a `value` attribute — the
three value/selection paths (`el_get_value`, `text_value_len`, `el_set_range_text`) now read it through one
`text_control_value` helper.
**(2)** `<input>` is unchanged (its value really is the `value` attribute); a dirtied textarea stores its
current value in the `value` attribute, which the helper prefers over the text content.
**The trap:** reading `attr("value")` for both control types looks uniform but silently returns empty for
every textarea — a whole control type reads blank, and the corruption compounds through setRangeText and
form submission.

## select.length counts and resizes the options (tick 441)

**The class of the web this unlocks:** the `select.length = 0` "clear the dropdown then repopulate" idiom —
a dependent/cascading select (country → state → city) rebuilds its option list this way, as does any widget
that reloads options from fetched data.
**(1)** `select.length` returned `0` (it was reading the CharacterData text length of a non-text node). It
now reports the option count; `select.length = n` truncates (removing trailing options) or grows (appending
`<option>` elements).
**(2)** CharacterData `.length` (text node length) is unchanged — the `length` property is overloaded and
dispatches on the tag.
**The trap:** the same idiom via the collection (`select.options.length = n`) still no-ops because the
native `options` getter returns a fresh Array — settable-length needs a persistent HTMLOptionsCollection
(pinned unknown). `select.length = n` is the one that now works.

## input.valueAsNumber + stepUp/stepDown (tick 442)

**The class of the web this unlocks:** numeric-form widgets — quantity steppers ("+"/"−" buttons on a
cart), range sliders that read `valueAsNumber`, price/measurement inputs, and form-validation libraries
that compare the numeric value against min/max.
**(1)** `input.valueAsNumber` (get/set) was `undefined`; it now parses/writes the number behind a
`type=number`/`type=range` control (NaN for empty/invalid; NaN for unsupported types; undefined on a
non-input).
**(2)** `stepUp(n)`/`stepDown(n)` threw; they now add/subtract `n × step` (default step 1) and clamp to
`min`/`max`.
**The trap:** the value behind a numeric input is a STRING in the `value` attribute — reading it as a
number and stepping it correctly (default step, min/max clamp, float-fuzz trim) is what every stepper
needs, and all of it was absent.

## input.valueAsDate + valueAsNumber for date inputs (tick 443)

**The class of the web this unlocks:** date/time pickers and range widgets — a `<input type="date">` whose
JS reads `valueAsDate` to get a `Date`, compares two dates via `valueAsNumber`, or sets the control from a
`Date` object (calendar widgets, booking/scheduling forms, date-range filters).
**(1)** `type=date` → `valueAsDate` is the UTC-midnight `Date`, `valueAsNumber` the epoch ms; `type=time` →
ms-since-midnight + a 1970 `Date`; `type=month` → a month index. Setters write the control's string back.
**(2)** All arithmetic is UTC so a `type=date` round-trips regardless of the host timezone; `valueAsDate` is
`null` where it does not apply (number/range).
**The trap:** doing this in local time drifts a date by a day across a timezone; the spec mandates UTC for
exactly that reason.

## progress.position + output.value (tick 444)

**The class of the web this unlocks:** progress bars whose JS reads the completion fraction
(`progress.position` for an upload/download/step indicator) and `<output>`-based calculators / live form
results that read and set `output.value`.
**(1)** `progress.position` was `undefined`; it now returns `value/max` in `[0,1]` (or `-1` when
indeterminate — no `value` attribute).
**(2)** `output.value` returned `""` (a dead expando); it now IS the element's displayed text content, read
and settable (`output.value = total` updates what the user sees).
**The trap:** `output.value` is the same bug class as `textarea.value` (t440) — a control whose value is its
text content, not a `value` attribute; reading the attribute returns blank and assignment silently fails to
render.

## the .text property for a/script/title (tick 445)

**The class of the web this unlocks:** code that reads `<a>.text` (link label without markup),
`<script>.text` (inline JSON-LD / config / template source, e.g. `JSON.parse(script.text)`), or
`<title>.text` (page title) — and code that sets them.
**(1)** These were dead expandos (`undefined`); `.text` now returns/sets the RAW text content for
`<a>`/`<script>`/`<title>`.
**(2)** `<option>.text` stays whitespace-collapsed (tick 439) and a plain element keeps its `.text` expando
— one accessor, tag-dispatched.
**The trap:** `<script>.text` is how a page reads an inline JSON-LD or config block; returning `undefined`
means `JSON.parse(script.text)` throws and the whole feature (structured data, config-driven UI) dies.

## datetime-local + week typed values (tick 446)

**The class of the web this unlocks:** scheduling / booking / admin forms that drive a
`<input type="datetime-local">` or `<input type="week">` numerically — reading `valueAsNumber` to compute a
duration or a min/max window, or writing `valueAsNumber = ms` / `valueAsDate = d` to seed the control from a
Date. Both types returned `null` and their setters were no-ops, so the picker stayed empty and any duration
math produced `NaN`.
**(1)** `datetime-local` → `valueAsNumber` is the UTC ms of the local datetime (no timezone; read AS-IF UTC);
`valueAsDate` stays `null` (does not apply), matching Chrome.
**(2)** `week` → `valueAsNumber`/`valueAsDate` run ISO-8601 week arithmetic (weeks start Monday; week 1 holds
Jan 4), so `2020-W03` ↔ Monday 2020-01-13 round-trips regardless of host timezone.
**The trap:** these were the two epoch-arithmetic follow-ons ticks 442/443 explicitly left unbuilt — a typed
surface that looks complete (number/range/date/month/time all work) but silently drops the two calendar
types most forms use for appointments.

## <a>/<area> URL-decomposition setters (tick 447)

**The class of the web this unlocks:** analytics/consent tags and SPA navigation code that mutate a link's
target in place — `link.search = '?utm_source=x'`, `a.hash = '#' + sectionId`, `a.pathname = newPath`,
`a.hostname = cdnHost`. Reading these components already worked; writing them was a silent no-op, so the
UTM param never attached and the in-page anchor never moved.
**(1)** Each setter re-serialises only its own component of `href` via the real `url` crate (the parser the
net stack uses), so the getter and any subsequent navigation see the change.
**(2)** Works on `<area>` too; `origin` stays read-only; the write is tag-guarded so a plain element never
grows a spurious `href`.
**The trap:** the read-side working made this look done — a getter that returns the right `search` while the
matching setter throws the value away is the dead-setter class (cf. textarea.value t440, output.value t444).

## pointer-events: none is transparent to hit-testing (tick 448)

**The class of the web this unlocks:** every page that lays a full-bleed `pointer-events: none` element over
its content — gradient hero scrims, toast/notification layers, drag-ghosts, `::before` sheens, chart tooltips,
loading shimmers. Before, `document.elementFromPoint` returned that overlay, so a click resolved to it and was
swallowed; the button, link or menu-item *underneath* never received the event. This is also the agentic
actuation surface (component #2): an agent (or a test harness) that resolves a click target by coordinate hit
the transparent overlay and actuated the wrong element.
**(1)** `pointer-events` is now an inherited computed value bridged from Stylo; `elementFromPoint` (and the
click-dispatch path it feeds) drop any `none` candidate via the published styles snapshot, so the point passes
through to the element behind. Because the property inherits, a descendant that re-enables with
`pointer-events: auto` is hit again — no extra tree-walk needed.
**(2)** `getComputedStyle(el).pointerEvents` and `getPropertyValue('pointer-events')` now resolve `"none"`/
`"auto"` instead of `undefined`, so feature-detection and overlay-management code read a real value.
**The trap:** the getComputedStyle gap looked like the whole bug, but the load-bearing defect was behavioral —
hit-testing ignored the property. And the property-list growth exposed a latent CSSOM enumeration-count bug
(a hardcoded `.length` one short of the final custom property).

## pointer-events: none is transparent to the agent's hit-test (tick 449)

**The class of the web this unlocks (agentic):** the same `pointer-events:none` overlays tick 448 fixed for
scripted clicks — gradient scrims, toasts, drag-ghosts, shimmers — but now for the AGENT'S OWN click
grounding. An agent (or the shell's click-by-coordinate) resolves a target through the accessibility tree's
`hit_test`, a different path from JS `elementFromPoint`. That path was occlusion-only, so a decorative
overlay on a high stacking layer intercepted the agent's click and it actuated the wrong element — exactly
the failure component #2 exists to prevent.
**(1)** `A11yNode.hittable` is now `false` for a `pointer-events:none` element (fed from the live computed
styles via `Page::non_hittable_nodes()`, mirroring `invisible_nodes()`); the node stays in the tree for a
screen reader but `hit_test` passes through it to the control behind.
**(2)** Default `hittable = true` and the fix is scoped to the two live builders, so no other a11y consumer
changes; a normal node stays hittable (no over-marking).
**The trap:** it is a SEPARATE code path from the JS fix — the same property, two hit-test implementations,
and fixing one leaves the other lying. `pointer-events:none` must NOT be treated like `visibility:hidden`
(which omits the node entirely): the element is still perceivable, so it is announced but not a target.

## The HTML `inert` attribute neutralises a subtree for the agent (tick 450)

**The class of the web this unlocks (agentic + reflection):** the modal-dialog backdrop. When a site opens
a modal — `<dialog>.showModal()`, or a library that sets `document.body.inert = true` around an overlay —
the rest of the page must become non-interactive: a user (and an AGENT) must not be able to click a button
behind the modal. `inert` was entirely unhandled, so an agent grounding a coordinate click through the
accessibility tree's `hit_test` actuated neutralised UI behind an open dialog — the exact component-#2
failure a modal exists to prevent — and `el.inert` read `undefined`, misleading any feature-detect.
**(1)** `el.inert` now reflects as a boolean (a one-row addition to the global `"*"` reflection table; the
generic mechanism from tick 111 supplies the getter/setter): `false` when unset (never `undefined`), `true`
when present, and assigning it adds/removes the content attribute.
**(2)** `Page::non_hittable_nodes()` now walks the DOM subtree and unions in every node under an `inert`
element, feeding the same `build_tree_full` path tick 449 wired, so each is marked `hittable = false` — in
the tree (announced) but skipped in `hit_test`.
**The trap:** `inert` looks like a sibling of `pointer-events:none` but the mechanism is different in two
ways that decide the implementation — it is an HTML ATTRIBUTE (reflection, not cascade) and it inherits
down the DOM SUBTREE (a tree walk, not a per-node computed-style read). And unlike `pointer-events`, `inert`
must NOT be fed to the JS `elementFromPoint` path: per spec it changes interaction targeting, not the
geometric CSSOM-View hit-test.

## The HTML `inert` attribute blocks focus — the modal focus-trap (tick 451)

**The class of the web this unlocks (agentic + keyboard a11y):** the modal focus-trap. Tick 450 stopped
an agent *clicking* through to the page behind an open modal; this stops focus *tabbing* through. Every
`<dialog>.showModal()` and every modal library depends on the backdrop being untabbable — otherwise a
keyboard user (or an agent driving via Tab / `el.focus()`) walks straight out of the dialog into the
neutralised page. `inert` did not affect focus at all, so the trap leaked.
**(1)** `Page::set_focus` — the single sink the shell, the agent, and the JS `el.focus()` queue all
funnel through — now refuses a focus request whose target is inside an `inert` subtree
(`is_inert(node)`, an ancestor walk), before any DOM state changes.
**(2)** Moving focus AWAY (`None`) is always allowed, so closing the modal is never blocked.
**The trap:** the fix belongs at the shared `set_focus` sink, not at each caller — putting it in the
shell's Tab handler would leave the agent's focus grounding and the JS `el.focus()` path still leaking.
One chokepoint, one guard.

## A disabled form control is not a tab stop (tick 452)

**The class of the web this unlocks (forms + keyboard a11y):** every form with a disabled submit button
or a `<fieldset disabled>` section. A disabled control is not focusable — Tab must skip it and
`el.focus()` must be a no-op — or a keyboard user (and an agent driving via Tab) snags on greyed-out
controls and `:focus` visibly styles them. `set_focus` checked `inert` (tick 451) but not `disabled`.
**(1)** The focusability guard at the shared `set_focus` sink is now `is_inert(n) || is_disabled(n)`.
**(2)** `is_disabled` already covered inherited disabledness (`<fieldset disabled>`), so the bulk-disable
idiom is handled for free.
**The trap:** the click/activation path already refused disabled controls, which made it *look* handled —
but focus is a separate sink, and a control you cannot click yet can Tab-focus is still broken. Each sink
that grants interaction needs the focusability check.

## Bulk-disabled form sections style correctly (tick 453)

**The class of the web this unlocks (forms):** any form that disables a whole section with
`<fieldset disabled>` — a checkout step, a settings panel gated behind a toggle, a wizard's future steps.
The `input:disabled { opacity:.5 }` / greyed styling every design system ships depends on `:disabled`
matching controls disabled *via their fieldset*, not just ones with their own attribute. Both matchers
(the live Stylo cascade AND the querySelector engine) were own-attribute-only, so a bulk-disabled section
rendered as if fully enabled and `querySelector(':disabled')` missed its controls.
**(1)** One shared `is_disabled_control` (own attr or ancestor `<fieldset disabled>`) backs `:disabled`/
`:enabled` in both engines.
**(2)** Same rule as the focus path (`is_disabled`), so styling, querying, and focusability agree.
**The trap:** it is the two-engines-disagree shape again — the cascade matcher and the querySelector
engine are separate code, and fixing a pseudo-class in one leaves the other lying. Both had to change.

## Editable vs. locked fields are queryable, not just styleable (tick 454)

**The class of the web this unlocks (forms):** any form that reads mutability with a selector — a
validation library calling `querySelectorAll('input:read-write')` to find the fields it should validate,
a form-serializer skipping `:read-only` inputs, a design system's `input:read-only { background:#eee }`
paired with JS that enumerates the same set. An `<input>`/`<textarea>` without a `readonly` attribute is
`:read-write`; a readonly control — and every non-editable element — is `:read-only`. The live Stylo
cascade already styled both, but the querySelector engine dropped them: `:read-only` was an unknown pseudo
(the whole selector discarded) and `:read-write` never matched. So the CSS greyed the locked field while
`querySelectorAll('input:read-write')` returned nothing — the script and the stylesheet disagreed about
which fields are editable.
**(1)** Two new `Pseudo` variants match in the querySelector engine, mirroring the cascade's rule exactly
(own `readonly` attr + input/textarea tag).
**(2)** `contenteditable` making an arbitrary element `:read-write` is unmodelled on BOTH sides, so the
engines never diverge.
**The trap:** the two-engines-disagree shape a third time (`:open` t429, `:disabled` t453, `:read-only`
t454) — the cascade matcher and the querySelector engine are separate code; a pseudo-class working in one
is not working in the browser.

## Rich-text editors can detect their editable host (tick 456)

**The class of the web this unlocks (messaging + docs + forums — editor DETECTION):** any page that mounts a
rich-text editor or reads editability — ProseMirror, Slate, Draft, TinyMCE, CKEditor, and every
`el.isContentEditable`-gated init path or contenteditable-detection library. `el.isContentEditable` was
`undefined` (falsy) on a `<div contenteditable>` because the whole surface was absent, so an editor read its
own mount point as plain and either bailed or initialised in the wrong mode.
**(1)** `el.contentEditable` reflects the enumerated attribute; `el.isContentEditable` is computed up the
ancestor chain (explicit `contenteditable=false` islands block an editable ancestor); `document.designMode`
makes the whole document editable.
**(2)** Honest scope — this is the QUERY surface (detection), not the editing path (`execCommand`/keystroke
mutation), which is a separate later brick and still absent. Detection is correct without claiming typing works.
**The trap:** a reflection-only brick of a subsystem is only honest if it does NOT imply the rest works — the
gate asserts reflection + inheritance, never "editing works," and the journal names the editing path as the
next brick. Brick 1 of the Tier-1 rich-editing subsystem, the pivot off the mined-out selector/interaction vein.

## Editor hosts match :read-write and get styled (tick 457)

**The class of the web this unlocks (messaging + docs + forums):** any rich-editor UI that styles or queries
its editable region by mutability — `[contenteditable]` comment boxes, Gmail-compose / Notion-class hosts,
and design systems whose `:read-write`/`:read-only` rules or `querySelectorAll(':read-write')` calls target
editable content. A `<div contenteditable>` was styled by `:read-only` rules and missed by `:read-write`
ones (both engines checked input/textarea only), disagreeing with the `el.isContentEditable` t456 landed.
**(1)** A shared `is_contenteditable` (walk ancestors for the attribute, nearest explicit state wins) now
backs `:read-write`/`:read-only` in both the querySelector engine and the live Stylo cascade.
**(2)** `:read-only` is the exact complement of `:read-write`, so the two pseudos + `isContentEditable` all
agree; `contenteditable=false` islands correctly revert to `:read-only`.
**The trap:** the fourth two-engines-disagree pseudo (`:open`/`:disabled`/`:read-only`/contenteditable) — a
pseudo-class fixed in the JS/query engine still lies in the cascade until both change together.

## The dropdown that clears itself before repopulating (tick 459)

**The class of the web this unlocks (JS-driven dependent/cascading dropdowns):** every non-framework page that
rebuilds a `<select>` from data — country/state/city pickers, "choose a plan then choose a tier" cascades,
timezone lists, "add another row" forms — using the canonical clear-then-fill idiom
`sel.options.length = 0; for (…) sel.add(new Option(…))`. The `.length = 0` was a dead expando: it truncated
the throwaway snapshot Array `select.options` returns and left the real `<option>`s in the DOM, so the
"cleared" dropdown showed every stale row *under* the freshly-added ones — a visibly duplicated, wrong list.
**(1)** `select.options` is now a Proxy over the decorated array whose `length` get is the LIVE option count
and whose `length =` routes to the proven native `select.length` setter (truncate trailing options from their
own parents / grow with bare `<option>`s). **(2)** Indexed access, iteration, `namedItem`/`add`/`remove` and
Array methods pass through unchanged, so `Array.isArray(sel.options)` / `instanceof Array` / spread still hold.
**The trap:** the same dead-expando bug class as `option.text`/`select.length` — a getter that hands back a
fresh snapshot makes every write to that snapshot a silent no-op; the fix is to make the collection LIVE, not
to make the write louder. Closes the collection form of the clear-idiom (the element form `select.length = 0`
landed t441).

## Reactive web components respond to script-driven attribute changes (tick 460)

**The class of the web this unlocks (design-system web components + Lit-class libraries):** any custom element
that reacts to its own attributes changing from script — `<my-toggle checked>` / `<x-tab selected>` flipping on
`setAttribute`, `aria-expanded`/`aria-pressed`-driven state, disclosure widgets, and Lit's attribute→property
reflection. `attributeChangedCallback` fired only for attributes present at UPGRADE; a later
`el.setAttribute('checked','')` / `removeAttribute` / `toggleAttribute` wrote the DOM and never told the element,
so the component's rendered state froze at boot and every script-driven state flip was a silent no-op.
**(1)** `setAttribute`/`removeAttribute`/`toggleAttribute` now fire `attributeChangedCallback(name, old, new)`
SYNCHRONOUSLY (the component has re-rendered by the next line of script) when the element is an upgraded custom
element observing that attribute. **(2)** setAttribute reacts on every call (spec); remove/toggle only on an
actual change; unobserved attributes never fire. **The trap:** the MutationObserver feed cannot substitute — it
runs BEFORE the attribute is written (no new value) and is delivered async on a microtask, but a custom-element
reaction is synchronous; wiring ACC onto that feed would be observably wrong. The fix wraps the JS setAttribute
family directly.

## Paste a screenshot into the page (tick 461)

**The class of the web this unlocks (AI chat + issue trackers + rich editors — image paste):** every page
whose paste handler reads `navigator.clipboard.read()` and branches on `it.types.includes('image/png')` to
accept a copied image — ChatGPT/Claude-style "paste a screenshot", GitHub/Linear issue image paste, rich
editors and image drop zones. The read bridge carried `text/plain` only, so a copied image came back as an
empty text item and the picture never arrived.
**(1)** `read()` now returns a `ClipboardItem` keyed by the image MIME whose `getType(mime)` resolves a real
image Blob (exact bytes, correct size/type), seeded by the host via `set_host_clipboard_image`. **(2)** The
binary transport is base64 (`"<mime>;base64,<data>"`) decoded with `atob` — a JS string is UTF-16 and raw
bytes are not valid text, so the same `b64`/`data:`-URL transport is reused. **The trap:** an image-only
clipboard must NOT invent a `text/plain` item — a ClipboardItem is keyed only by the types it holds, and a
paste handler that finds a phantom text type takes the wrong branch. Follow-on: the WRITE direction + the
shell round-trip to the real OS clipboard.

## Copy an image to the clipboard (tick 462)

**The class of the web this unlocks (copy-image / copy-chart buttons):** every page that copies a
generated image to the OS clipboard — a "copy chart" button (`canvas.toBlob` → `ClipboardItem` →
`clipboard.write`), "copy image" in an editor/gallery, a QR/diagram export. `clipboard.write()` honoured
`text/plain` only, so an image `ClipboardItem` resolved successfully while the picture was silently
dropped — the write "worked" and copied nothing.
**(1)** `write()` now reads an image Blob's bytes, base64s them (`btoa`) and queues `(mime, bytes)` for the
host via `__clipboardWriteImage`/`take_pending_clipboard_image_writes()`; text and image parts of one item
both go through (`Promise.all`). **(2)** Symmetric to the READ side (t461): base64 is the transport because
a JS string is UTF-16 and raw bytes are not valid text. **The trap:** a write path that resolves its
Promise is indistinguishable from one that did the work — the silent-success failure mode. The gate asserts
the exact bytes reached the host queue, not merely that write() resolved. Follow-on: the shell round-trip to
a real OS clipboard.

## The legacy copy button (tick 463)

**The class of the web this unlocks (copy-to-clipboard buttons everywhere):** the dominant copy-button
implementation on the web — select a node (often a hidden `<textarea>` or a code block), then
`document.execCommand('copy')`. clipboard.js and its clones, and countless hand-rolled buttons, use it —
usually as the FALLBACK when `navigator.clipboard` is unavailable. `document.execCommand` was absent, so the
call was a `TypeError` that took the copy handler down.
**(1)** `execCommand('copy')` copies `getSelection().toString()` synchronously through the same host bridge
as `navigator.clipboard.writeText`; `selectAll` selects the document; `queryCommandSupported` reports them.
**(2)** Honest scope: `cut` and formatting commands (bold/italic/insertText) mutate editable content — the
contenteditable EDITING subsystem — so they return `false`, and a page feature-detects the truth. **The
trap:** execCommand returns a synchronous boolean, NOT a Promise — routing `copy` through the async
Clipboard API's Promise would make the return value meaningless; it must queue the host write synchronously
and return `true`.

## Toolbars and editors detect their own selectability (tick 464)

**The class of the web this unlocks (UI chrome that suppresses text selection):** `user-select: none` is on
nearly every toolbar, button, tab strip, drag-handle and code-copy widget so a stray double-click-drag on
the chrome does not select label text; rich editors set `user-select: all` on atomic tokens. Feature-
detection and selection-management libraries read the value back through the CSSOM to decide whether to run
their own selection-suppression fallback — and that read returned `undefined`, sending them down the
polyfill path (or throwing on `getComputedStyle(el).userSelect.indexOf(...)`).
**(1)** `user-select` now cascades: Stylo's servo build gates it behind the shared `layout.unimplemented`
pref (off by default → dropped at parse → every element computed `auto`); flipping that pref on lets it
parse, and the computed keyword maps onto `ComputedStyle.user_select` beside `pointer_events`.
**(2)** `getComputedStyle(el).userSelect` (plus the `webkitUserSelect` alias Chrome exposes) and
`getPropertyValue('user-select')`/`'-webkit-user-select'` now resolve `none`/`text`/`all`/`auto` from the
stylesheet, inline style and `-webkit-`/`-moz-` prefixes. **The trap:** the pref is SHARED by ~35
properties, so there is no per-property flip — flipping it is safe only because it gates PARSING and we
consume a fixed set of computed values via explicit `clone_*` calls. **Scope boundary:** this resolves the
COMPUTED VALUE; the geometry of a user mouse-drag selection honouring `user-select` is a layout/hit-test
concern not modelled — the same boundary the `Selection` shim documents.

## The dark-mode page is dark all the way down (tick 465)

**The class of the web this unlocks (dark-themed sites and dark-mode UIs):** a page opts into a dark UA
appearance with `color-scheme: dark` (the property or `<meta name="color-scheme" content="dark">`). Its
most visible effect is the canvas: CSS propagates the root's background to the whole viewport, so a
dark-only page with no explicit background must paint the void below its content dark. Before this,
`color-scheme` did not exist in the engine and the canvas fell through to a hard-coded WHITE — dark content
floating in a white void below the fold, and `getComputedStyle(el).colorScheme` was `undefined`.
**(1)** `color-scheme` now cascades (via the `layout.unimplemented` pref flipped in t464; it is inherited),
maps to `ComputedStyle.color_scheme`, and reflects through `getComputedStyle` as `normal`/`light`/`dark`/
`light dark`. **(2)** `Page::canvas_background()` returns the dark UA canvas (`rgb(18,18,18)`) when the
root's used scheme is dark and no explicit background wins. **The trap:** the used scheme is dark only for
a dark-ONLY page (`dark` listed, `light` not) — Chrome renders that dark regardless of the OS setting,
while `light dark` defers to `prefers-color-scheme`. **Scope boundary:** only the canvas default is
modelled (the void has no text, so darkening it cannot make content unreadable); UA control/scrollbar
appearance and the default dark text color are deeper system-color adjustments not modelled, and dark pages
set their own content colors in practice.

## Accessible theming picks legible text without JS (tick 466)

**The class of the web this unlocks (dynamic-theme / brand-color UIs):** `contrast-color(<color>)` (CSS
Color 5, Baseline 2026) returns whichever of black/white contrasts more with the given color — so a page
can write `color: contrast-color(var(--brand))` and get legible text over any dynamic background without a
JS luminance calculation. Before this the function was dropped at parse (Stylo gates it behind
`layout.css.contrast-color.enabled`) and the declaration fell back to the inherited/initial color —
unreadable text on a saturated background.
**(1)** Flipping the pref lets Stylo parse it and compute a `ComputedColor::ContrastColor`, which the
engine's color mapping already resolves to the absolute companion via `resolve_to_absolute`. **(2)** It
resolves for both `color` and `background-color` (any color property), and `getComputedStyle` reports the
resolved rgb. **The trap:** the function computes to a deferred `ContrastColor` variant, NOT an absolute
color at computed time — it works only because the used-value resolution path was already wired; a naive
`clone_*` that skipped resolution would have serialized `contrast-color(...)` back as a string.

## Exclusive accordions open one panel at a time (tick 467)

**The class of the web this unlocks (FAQ / docs-sidebar / settings-panel UIs):** `<details name="group">`
is HTML's exclusive accordion (Baseline 2024) — several disclosures sharing a `name` are guaranteed at
most one open at a time, with no page script. `<details>` already toggled on a summary click, but the
grouping was absent: a named FAQ opened every section a user clicked and never collapsed the previous one,
so a multi-item accordion sat fully expanded — the same "wall of everything at once" the plain-details fix
prevents, one level up. Now, when a `<details>` goes closed→open, if it carries a non-empty `name` the
engine closes every other open same-name `<details>` and fires `toggle` on each. **The trap:** exclusivity
is scoped strictly BY NAME — an empty name is not a group and a different name is a different group; closing
"all other open details" regardless of name would break any page with two independent accordions.

## State-driven disclosures reveal correctly (tick 468)

**The class of the web this unlocks (React/Vue/state-controlled accordions & disclosures):** a framework
that renders `<details open={isExpanded}>` — or any code that writes `el.open = true` — drives the widget
through the IDL setter, never a summary click. The attribute already reflected, but the change was silent:
no `toggle` event and no `<details name>` exclusivity, so a lazy-load listener wired to `toggle` never
fired (the panel revealed empty) and a script-controlled named group could show every section at once. Now
a contained hook in the shared reflected-boolean setter fires `toggle` and enforces same-name exclusivity
on the script path too, matching the click path (tick 467). **The trap:** the hook is scoped strictly to
`<details>.open` — `<dialog>.open` (driven by show()/close(), not the IDL boolean) and every other
reflected attribute must stay side-effect-free, or the change becomes a regression in the spec-critical
reflection machinery it lives in.

## Dark-mode scrollbar theming reports its computed value (tick 469)

**The class of the web this unlocks (dark-UI dashboards, editors, docs sidebars):** a dark-themed page sets
`scrollbar-color: #888 #222` / `scrollbar-width: thin` on its scroll containers so the OS scrollbar does
not sit bright-on-dark, and a custom-scrollbar library reads `getComputedStyle(el).scrollbarColor` back to
decide whether to skip its own overlay. Both were `undefined`, so the feature-detect always fell through.
Now the resolved value is reported (thumb/track as `rgb()` pair, `getPropertyValue` too). **The trap:** these
two are `engine="gecko"` in the crates.io Stylo the browser actually compiles — no `layout.unimplemented`
pref flip can surface them (a vendored `stylo/` tree that marks them pref-gated is a decoy), so they must be
MinimalCascade-recovered like `-webkit-line-clamp`, not mapped from Stylo's computed values. Scope is the
computed value only; painting a themed scrollbar is out of scope.

## Lazy-load a disclosure just before it renders (tick 470)

**The class of the web this unlocks (docs sites, GitHub folded diffs, FAQ accordions with deferred content):**
a page defers a `<details>` section's heavy DOM until it is about to open, wiring
`el.addEventListener('beforetoggle', () => hydrateSection())` so the content is built the instant before the
panel becomes visible — not on page load (wasteful) and not on `toggle` (one frame late, the empty panel
flashes first). `<details>` had `toggle` on both actuation paths but no `beforetoggle`, so this hook never
fired and the section revealed its skeleton. Now `beforetoggle` fires immediately before `toggle` on both a
summary click and a scripted `el.open = true` (including accordion auto-close). **The trap:** details'
`beforetoggle` is NON-cancelable (unlike popover's) — a page that tries to `preventDefault()` it to block the
open is relying on behavior no browser gives; the toggle proceeds regardless.

## Insert text into a rich-text editor at the caret (tick 471)

**The class of the web this unlocks (comment boxes, Notion/Google-Docs-style editors, any
`contenteditable` surface):** a rich editor mounts on a `<div contenteditable>` (it already detected the
host via `isContentEditable`, t456) and then puts text into it — an "insert emoji", "insert snippet", or
paste-as-plaintext button calls `document.execCommand('insertText', false, str)`, and the framework
(ProseMirror/Slate/Lexical/Draft) listens for the `beforeinput`/`input` (`inputType:'insertText'`) pair to
sync its own document model and undo stack. Before this, `insertText` returned `false` and nothing happened:
the editable box merely *looked* editable. Now the text is inserted at the caret (merged into the existing
text run), and the cancelable `beforeinput` → mutate → `input` sequence fires. **The trap:** a cancelled
`beforeinput` is not a no-op you can ignore — it is how a framework editor VETOES the browser's default
insertion so it can perform its own; on veto the DOM must be left untouched and `input` must NOT fire.
Formatting commands (`bold`/`italic`) and multi-node deletion are still honestly `false` +
`queryCommandSupported`-false — a page feature-detects the truth rather than getting a silent lie.

## Type into a rich-text editor / comment box with the keyboard (tick 472)

**The class of the web this unlocks (comment boxes, chat composers, Notion/Docs-style editors — any
`contenteditable` a user types into):** the editor mounted (`isContentEditable`, t456) and could take a
programmatic insert (`execCommand('insertText')`, t471), but a person pressing a key saw nothing happen —
`dispatch_key` fired the `keydown` and stopped, so the character never entered the DOM. Now a printable
keydown that no handler cancelled inserts the pressed character at the caret and fires `beforeinput`→`input`
(`inputType:'insertText'`), through the SAME primitive `execCommand('insertText')` uses. **The trap:** a
framework editor (ProseMirror/Lexical) suppresses this by `preventDefault()`-ing the keydown and running its
own model — so the default action must honor that cancel (no insert when `proceed===false`), and it must NOT
fire for non-character keys (`Enter`/arrows/`Backspace` — those are separate editing intents, later bricks).
Known honest gap: with no modifier state on `dispatch_key` yet, an UNHANDLED Ctrl/Meta+letter still inserts
the letter; a real editor's shortcut handler prevents the keydown, which is what suppresses it in practice.

## Edit text in a rich editor — delete a character with Backspace (tick 473)

**The class of the web this unlocks (every comment box / chat composer / contenteditable editor a user
corrects a typo in):** typing worked (t472) but Backspace did nothing — you could write into the box but
never fix a mistake, so it was still not a real editor. Now an uncancelled Backspace deletes the grapheme
before the caret (or the current selection) and fires `beforeinput`→`input`
(`inputType:'deleteContentBackward'`), the DELETE sibling of the shared insert primitive. **The trap:** a
no-op Backspace (caret already at the start, nothing to remove) must NOT fire `input` — an editor's model
listens for `input` and a spurious empty one desyncs it; and a framework editor that `preventDefault()`s the
`beforeinput` must have its delete vetoed so it can run its own removal. Cross-block merge (Backspace at a
block's start pulling it into the previous block) is a later brick — the common in-a-line case is what lands.

## Edit text in a rich editor — forward-delete with the Delete key (tick 474)

**The class of the web this unlocks (the same editors, now with the Delete key):** t473 gave a
contenteditable Backspace; this gives it the Delete key — remove the character AFTER the caret. Together
they are the complete caret-delete pair every text field needs. Fires `beforeinput`→`input`
(`inputType:'deleteContentForward'`) through the same shared primitive, honoring the veto and the no-op
(a Delete at the end of the text removes nothing and fires no `input`, so an editor's model is not desynced
by a phantom event). Cross-block forward merge is a later brick; the in-line case is what lands.

## Insert a hard line break in a rich editor (tick 475)

**The class of the web this unlocks (code-snippet editors, plaintext composers, "insert line break"
toolbar buttons):** `execCommand('insertLineBreak')` drops a `<br>` at the caret — the soft-newline
primitive. Before it, the command returned `false` and the editor's line-break button did nothing. Now a
`<br>` is inserted (splitting the current text run) and `beforeinput`→`input` fire with
`inputType:'insertLineBreak'`. **The trap:** the `<br>` is structural — it must NOT change the editable's
textContent (a break is not a character), and a framework editor that `preventDefault()`s the `beforeinput`
must get no `<br>` so it can insert its own. Full Enter-key paragraph behavior (`insertParagraph`, which
splits the containing block and is browser-divergent — Chrome `<div>`, Firefox `<br>`) is a later brick that
also needs keyboard modifier state; this lands only the programmatic line break.

## Cut selected text in a rich editor (Ctrl+X) (tick 476)

**The class of the web this unlocks (every editor/comment box where a user cuts text):**
`execCommand('cut')` now copies the selection to the clipboard AND removes it from a contenteditable,
firing `beforeinput`→`input` (`inputType:'deleteByCut'`) — the "cut" toolbar button and the clipboard.js
cut fallback were dead before (returned `false`). **The trap:** cut is not just a delete — it must put the
text on the clipboard FIRST (a cut that deletes without copying loses the user's data), and it must refuse
(`false`) when the selection is not in an editable region, because you cannot remove text you cannot edit.
The fuller clipboard-`cut`-event model (a page cancelling the whole cut, custom clipboard payloads) is a
later brick.

## Trigger a keyboard shortcut with a modifier chord (Cmd/Ctrl+K) (tick 477)

**The class of the web this unlocks (every app with a keyboard shortcut or command palette):** a dispatched
`KeyboardEvent` now carries `ctrlKey`/`shiftKey`/`altKey`/`metaKey`, so a page handler that gates on a
modifier — `if (e.metaKey && e.key==='k')` (the Cmd/Ctrl+K command palette in Slack/Notion/Linear/GitHub), a
composer inserting a newline only on `Shift+Enter` — actually fires. Before this the flags were all
`undefined`, so every modifier-gated shortcut was silently dead. **The trap:** the modifier state is not just
metadata for the handler — the DEFAULT editing action must read it too, or a `Ctrl+B`/`Cmd+K` chord typed at a
contenteditable inserts its letter as text (a chord is a shortcut, not typing). Ctrl/Meta held → no text;
Shift/Alt alone → still text (capitals, AltGr). The full Ctrl+X/C/V → cut/copy/paste routing (the execCommand
halves all exist) and Shift+Enter/Enter paragraph split are follow-on bricks this substrate enables.

## Cut and copy with the keyboard (Ctrl+X / Ctrl+C) (tick 478)

**The class of the web this unlocks (keyboard cut/copy in every editor, comment box and text field):** the
default browser action for the clipboard chords now runs — `Ctrl/Cmd+X` cuts the selection, `Ctrl/Cmd+C`
copies it — routed through the existing execCommand cut/copy machinery. Before this the chord was suppressed
from typing (t477) but never routed anywhere, so a keyboard cut/copy silently did nothing even though the
toolbar-button equivalents worked. **The trap:** this is a DEFAULT action, so it must yield to a page that
claims the chord — an editor with its own clipboard handler calls `preventDefault()` on the keydown, and the
default cut/copy must then NOT run (or it double-acts). And an empty Ctrl+C must be a no-op, not a
clipboard-clobbering write of "". `Ctrl+V`→paste waits on `insertFromPaste` (a paste-event trigger) — the
next brick.

## Insert a newline in a chat composer with Shift+Enter (tick 479)

**The class of the web this unlocks (every chat composer, comment box and rich editor):** Shift+Enter now
inserts a hard line break (`<br>`) at the caret in a contenteditable — the universal "newline without
submitting" gesture (Slack, Discord, WhatsApp Web, GitHub comments, every AI-chat box). It reuses the t475
`execCommand('insertLineBreak')` machinery, now driven from the keyboard because the dispatched KeyboardEvent
carries the shift flag (t477). **The trap:** Enter handling is TWO different actions and only ONE is safe to
land blindly — Shift+Enter → `<br>` is cross-browser identical, but PLAIN Enter → insertParagraph is a block
split that Chrome and Firefox implement DIFFERENTLY (`<div>` vs `<br>`), so plain Enter must stay a no-op (the
page's own composer handler owns it) rather than guess wrong. The block-split insertParagraph is a later brick.

## Paste text into an editor with Ctrl+V (tick 480)

**The class of the web this unlocks (pasting into every editor, comment box and chat composer):** Ctrl/Cmd+V
now reads the clipboard text, fires a cancelable `paste` event, and inserts the text at the caret
(`inputType:insertFromPaste`) — completing the keyboard clipboard trio (cut/copy landed t478). Before this,
Ctrl+V did nothing. **The trap:** paste is not a blind insert — a real editor listens for the `paste` event to
SANITIZE or transform the incoming content (strip formatting, block scripts, convert to its own model), so the
event must fire FIRST and be cancelable, and a `preventDefault()` must suppress the default insert entirely
(or the editor gets the text twice — once its way, once ours). Rich (HTML) paste and the full DataTransfer
(`getData('text/html')`, files) are later bricks; this lands plain-text paste with a vetoable event.

## Bold the selected text with a toolbar button (tick 481)

**The class of the web this unlocks (every rich-text toolbar):** `execCommand('bold')` / `('italic')` now wraps
the current selection in `<b>` / `<i>`, firing a vetoable `beforeinput`→`input` pair
(`inputType:formatBold`/`formatItalic`) and reporting `queryCommandSupported` true — the write half of the
Bold/Italic button in Gmail compose, Slack, GitHub/Reddit comment editors, and countless CMS WYSIWYG boxes.
Before this every command past insert/cut/copy returned `false`. **The trap:** formatting is TWO shapes and only
one is safe to land as a bounded brick — a NON-COLLAPSED selection has an unambiguous result (wrap it), but a
COLLAPSED caret arms a stateful "typing style" for the next keystroke, and re-running bold on already-bold text
is a TOGGLE (unwrap), not a second wrap. Landing only the wrap and returning `false` on a collapsed caret is the
honest brick; the typing-style/toggle/`queryCommandState` state machine is the declared follow-on. It reuses the
Selection/Range substrate (`extractContents`/`insertNode`) rather than a bespoke formatter, so it inherits the
same node-splitting the cut/delete path uses.

## Toolbar button shows active bold/italic state (tick 482)

**The class of the web this unlocks (every rich-text toolbar's button highlighting):**
`document.queryCommandState('bold')` / `('italic')` now reports whether the current selection/caret is inside
a `<b>`/`<strong>` or `<i>`/`<em>`, so a WYSIWYG editor's Bold/Italic button renders pressed when the caret is
in bold text — the read-back complement of the `execCommand('bold')` write (t481). **The trap:** a rich editor
wires this to `selectionchange` and calls it CONSTANTLY, so a missing method is not a silent false — it is a
`TypeError` thrown on every caret move that takes the toolbar's whole render path down (the aljazeera
"referenced-name-that-doesn't-exist is a crash" class). It must exist and answer honestly; and unlike the
write it must work with a COLLAPSED caret (buttons light up as you arrow through styled text), so it cannot
borrow the write path's non-collapsed guard.

## Insert a rich HTML snippet at the caret (tick 483)

**The class of the web this unlocks (rich editors + the rich-paste foundation):**
`document.execCommand('insertHTML', false, html)` now parses an HTML fragment and inserts it at the caret,
firing a vetoable `beforeinput`→`input` pair (`inputType:insertHTML`) — the path an editor's "insert
merge-tag / emoji-as-image / paste-with-formatting" button funnels through, and the substrate the eventual
rich (text/html) paste builds on (Ctrl+V today is plain-text only, t480). **The trap:** insertHTML is the
UNAMBIGUOUS sibling of the two editing bricks that are NOT — Enter→insertParagraph (Chrome `<div>` vs FF
`<br>`) and formatting toggle-off both need a real Chrome oracle to land correctly, but insertHTML's result is
exactly the parsed fragment at the caret, so it can land atomically now. It reuses `createContextualFragment`
+ `insertNode` rather than a bespoke parser, inheriting the same fragment-parsing the DOM already ships; the
one sharp edge is caret placement — capture the fragment's `lastChild` BEFORE `insertNode` empties the
fragment, or the caret has nothing to anchor after.

## Turn the selected text into a link with a toolbar button (tick 484)

**The class of the web this unlocks (every rich-text editor's link button):**
`document.execCommand('createLink', false, url)` now wraps the current selection in `<a href="url">`, firing a
vetoable `beforeinput`→`input` pair (`inputType:insertLink`, `data:url`) — the "add link" button in Gmail
compose, Slack, GitHub/Reddit comments, and CMS editors. **The trap:** createLink is the LAST of the
UNAMBIGUOUS wrap commands (bold/italic/createLink all wrap the selection with a definite DOM result); the
remaining editing commands (insertParagraph, formatting toggle-off) are browser-DIVERGENT and cannot be landed
honestly without measuring Chrome. It shares one helper with bold/italic (`__wrapSelectionFormat`,
generalised to set attributes + carry event data), so the anchor inherits the exact selection-wrap +
node-splitting the bold path already proved — one code path, three toolbar buttons.

## Sign in with a passkey, and fall back to a password when there is none (tick 485)

**The class of the web this unlocks (every passkey-first login: banks, GitHub, Microsoft, Apple, Okta):**
`navigator.credentials` + `window.PublicKeyCredential` now exist, so a login page's
`navigator.credentials.get({publicKey}).then(useAssertion).catch(showPasswordForm)` reaches its
`.catch` and reveals the password/TOTP fallback instead of dying on a synchronous `TypeError` the promise
`.catch` never saw. **The trap:** the failure was NOT a thrown error the page could handle — it was that a
MISSING `navigator.credentials` throws *before the promise exists*, so the page's own error handling is
bypassed and the user is stranded on a dead login screen. The honest surface (no authenticator →
`isUserVerifyingPlatformAuthenticatorAvailable()` false, `get`/`create` reject `NotAllowedError`) is
DETECTION + graceful degradation, not WebAuthn; a real authenticator is the vault/passkey subsystem ahead.

## Check `navigator.userActivation.isActive` before a gesture-gated action (tick 486)

**The class of the web this unlocks (any site gating autoplay-with-sound, requestFullscreen, popups,
clipboard.write, Web Share, PaymentRequest behind a user gesture):** `navigator.userActivation` now exists and
tracks REAL gesture state, so a click handler's `if (navigator.userActivation.isActive) video.play(); else
showPlayButton();` takes the right branch instead of dying. Inside a real host-dispatched click it reads
`isActive:true` (transient) and `hasBeenActive:true` (sticky); at load and after the handler returns,
`isActive` is false. **The trap:** the object was ABSENT, so the read was a synchronous `TypeError` before any
branch — the `else` fallback never ran either, leaving a dead button; AND a hardcoded `false` would be worse
than the crash, taking the "no gesture" branch during a real click. The discriminator is a private
`__actgesture` marker the engine stamps on the mouse/key events it synthesises — NOT `isTrusted`, because
engine gestures carry a supplied object and read `isTrusted===false` exactly like a page's own `el.click()`,
which must grant nothing. Agentic bonus: an agent's `dispatch_click` now trips the same activation a real user
would, so gesture-gated actions the agent initiates are honoured.

## The global `hidden` attribute collapses the element (tick 489)

**The class of the web this unlocks (any site that ships markup pre-hidden and reveals it with script —
tab panels, initial-collapsed accordions/FAQs, `<template>`-free "hidden until needed" fragments,
feature-detect fallbacks, and the ubiquitous `el.hidden = false` / `toggleAttribute('hidden')` toggle):**
`<div hidden>` now computes `display: none` instead of `block`. Before, only `input[type=hidden]` (the
*value* on one control) was in the UA sheet; the **global boolean `hidden` attribute** — valid on every
element and one of the most common visibility toggles on the web — had no rule, so every panel authored
hidden-until-shown painted its contents permanently into the page (the same failure shape as a closed
`<dialog>`/`<details>`/`[popover]` rendering inline). **The trap:** it is a *live* toggle, not a static
rule — `el.hidden = false` removes the attribute, the cascade re-runs on the mutation, `[hidden]` stops
matching and the element returns; a collapse that could not be undone would break every toggle it exists
to serve. `hidden="until-found"` is the spec exception and is deliberately LEFT VISIBLE: it renders with
`content-visibility: hidden` (collapsed-but-findable), unsupported here yet, so collapsing it would hide
content a user could never reveal on find. Rule kept in two-cascade lockstep (stylo_engine.rs +
apply_ua_defaults).

## `inputMode` / `enterKeyHint` reflect as global HTMLElement attributes (tick 490)

**The class of the web this unlocks (every mobile form and custom `contenteditable` field that steers the
on-screen keyboard):** `<input inputmode="numeric">` brings up a digit pad, `enterkeyhint="search"`
relabels the Enter key — and scripts read/write these through the IDL properties `el.inputMode` /
`el.enterKeyHint` to switch keyboard modes dynamically. Both read `undefined` before: the rows existed in
the reflection table but were keyed under a tag name `"undefinedelement"` that matches no element, instead
of the `"*"` global bucket that applies to every element. **The trap:** the row was *present*, so a
presence check would have passed — but it reached nothing, so `input.inputMode` was `undefined` and
`el.inputMode = 'tel'` no-opped. Fix is data-only (move both rows into `"*"`); the generic enum mechanism
then gives spec behaviour for free — absent/invalid → `""`, valid keyword round-trips through the lowercase
content attribute. Global, so it works on a `<div>` too, as the spec requires.

## `dialog.requestClose()` — a dismiss that a guard can veto (tick 491)

**The class of the web this unlocks (any dialog/modal component library whose Close button and ✕ call
`requestClose()` — the pattern Chrome shipped as the recommended dismiss):** unlike `close()`, it fires a
cancelable `cancel` event first, so a "you have unsaved changes — discard?" confirmation can
`preventDefault()` and keep the dialog open. `<dialog>` show/showModal/close/returnValue and the Escape veto
already existed; this exposes that same veto path as the method libraries call. **The trap:** absent, it was
not a graceful missing-feature — `dlg.requestClose()` threw a synchronous TypeError that took the click
handler with it, so the button did nothing at all (the whole-dialog-surface failure mode again). Guards
mirror `close()`: no-op without `open`, returnValue threads through, no throw on a closed dialog.

## `<img>.currentSrc` — which image resource actually loaded (tick 493)

**The class of the web this unlocks (lazy-load, lightbox, gallery and analytics libraries on essentially
every image-heavy site):** scripts read `img.currentSrc` to learn which file an `<img>` is displaying — to
avoid re-fetching one already shown, to build a full-size link from a thumbnail, or to log what loaded. It
returned `undefined` (the property did not exist). This engine loads an `<img>`'s `src` directly (no
srcset/`<picture>` bitmap selection yet), so currentSrc now honestly returns the resolved absolute `src` URL
— the resource we actually load — and `''` before any source is selected. **The trap:** a naive
`currentSrc = src` inside `<picture>` would be a lie in a browser that picks a `<source>`; here it is truthful
precisely because we load `src`, and it will track the chosen candidate for free once srcset selection lands.
Read-only and IMG-scoped, so a non-image element reads `undefined`.

## `document.activeElement` defaults to `<body>`, never null (tick 494)

**The class of the web this unlocks (focus-trap libraries, modals, keyboard handlers, editors — anything
that reads the focused element, which is nearly every interactive widget):** `document.activeElement` is read
constantly, and callers assume it is a real element — `document.activeElement.blur()` to dismiss focus,
`document.activeElement.tagName` to branch, `document.activeElement === el` to test. It was returning `null`
when nothing was focused, so all three crashed or misfired. It now defaults to the `<body>` element (the spec
default for a loaded document, what Chrome returns), and still moves to a real element on `.focus()`. **The
trap:** null is not a graceful "nothing focused" signal — it is a `TypeError` the moment any of those idioms
runs, so a page that dismisses focus on Escape threw instead.

## `document.hasFocus()` — is the user looking at this tab (tick 496)

**The class of the web this unlocks (idle-detection, analytics heartbeats, presence indicators, and "pause
the video/carousel/animation when the tab is not in front" logic — on a large fraction of media and app
sites):** scripts call `document.hasFocus()` to decide whether to keep doing work. It was absent — a
synchronous `TypeError` that killed the handler (the same failure the missing `document.hidden` once caused
for animation loops). It now returns the tab-in-front state the shell already owns (the fact behind
`visibilityState`/`document.hidden`), so it is honest for the dominant foreground-vs-backgrounded case and can
never contradict `document.hidden`. **The trap:** a hardcoded `true` would keep a backgrounded tab "focused"
and defeat the very battery/CPU savings the check exists for — tying it to real visibility avoids that.

## `<textarea>.textLength` — the character-counter number (tick 497)

**The class of the web this unlocks (every textarea with a live character counter — comment boxes, tweet
composers, bio/description fields with a `maxlength`):** a counter reads `textarea.textLength` on each
keystroke to render "120 / 280". It was `undefined`, so the counter showed "undefined / 280" or NaN-ed its
maths. It now returns `value.length` (the control's live text), read-only and textarea-only. Small, but it is
the exact number the UI puts on screen.

## `ch` unit = the font's real `0`-advance (tick 499)

**The class of the web this unlocks (every readable-text column and every monospace layout — `max-width:65ch`
articles/blogs/docs, code blocks and terminals sized in `ch`, form fields with a `size`-in-`ch` width, ASCII
tables):** the `ch` unit was resolving to the spec's `0.5em` *"cannot determine"* fallback while the text laid
into the box used the font's true advance (monospace `0` ≈ `0.6em`), so every `Nch` box was ~17% too narrow
and its content overflowed — the classic "the article column is squeezed and the last word wraps" look.
`ch` now measures the `0` glyph through the SAME shaper layout places text with, so `Nch` is exactly `N`
monospace chars and a `65ch` column matches Chrome. `ex`/`cap`/`ic` still use their spec fallbacks (bounded
follow-up); webfont-exact `ch` (page-context threading) is next. **The trap:** a constant `0.6em` looks right
but can't guarantee `N chars fit in N ch` to the pixel — only measuring through the real shaper does.

## `ex` unit = the face's real x-height (tick 500)

**The class of the web this unlocks (anything sized in `ex` — icon/glyph columns, vertical rhythm and
drop-cap sizing, form-control heights, and CSS that aligns to the x-height):** `ex` was the spec `0.5em`
fallback while real faces have an x-height slightly over half an em, so `ex`-sized boxes came out a few
percent short — invisible on one element, cumulative down a column. `ex` now reads the face's OS/2
`sxHeight` (the value Chrome uses) off the same face the text is drawn with. Completes the `ch`+`ex`
font-relative pair from tick 499. `cap`/`ic` remain their spec fallbacks (bounded follow-up).

## `cap` unit = the face's real cap-height (was 0px) (tick 502)

**The class of the web this unlocks (anything sized in `cap` — cap-height-aligned headings, drop caps,
and icon/badge sizing that matches capital letters):** the `cap` unit resolved to **0px** (its fallback
is `ascent`, which the provider left at 0), so a `cap`-sized box vanished entirely — a harder failure
than the `ch`/`ex` under-sizing. `cap` now reads the face's OS/2 `sCapHeight` (Chrome's source) off the
same face the text is drawn with. Completes the real font-relative units `ch`+`ex`+`cap`; `ic` stays its
spec `1em` fallback (correct, and not cleanly gate-able). **The trap:** a `None` metric here didn't fall
back to something reasonable — it fell back to an unset `ascent` of 0, turning a missing measurement into
a collapsed box.

## ES-module import GRAPHS render on the real page paths (ticks 512-517)

**The class of the web this unlocks (every native-ESM / no-bundler app and every Vite/Rollup DEV server):**
an app whose entry `<script type=module>` does `import { App } from './app.js'` — and whose `./app.js`
imports its own siblings, transitively — used to die at `ModuleLink` with *"module not found"*, so the app
never mounted. A single self-contained module already worked (tick 32); a multi-FILE import graph did not.
It now does, on BOTH real page paths: the streaming/headless/AGENT render path (`fetch_streaming_page` →
`load_async`) and the INTERACTIVE SHELL path (`prefetch_document` → `from_prefetched` → deferred pass).

The subsystem was built as rooting-safe bricks: B1 a GC-rooted module registry + per-module `import.meta.url`
private (t512); B2 the synchronous `module_resolve_hook` that resolves a specifier against the importer's
own URL and returns the registered module (t513); B3 the cycle-safe population walk `esm_load_graph`
(insert-before-recurse, a miss is loud-but-safe) over an injected fetch seam (t514); B3b-i the page runner
`run_module` driving that walk over a pre-fetched source map + per-root registry clear (t515); B3b-ii the
async PRODUCER on `load_async` — a textual static-import scanner + a BFS graph pre-fetch (`manuk_net::fetch`,
`Url::join` matching the resolve hook) that fills the map before scripts run (t516); B3b-iii the same
producer on the shell's off-thread `prepare_prefetched` path, carried on `Prefetched` → `Page` →
`run_deferred_scripts` so the graph survives the shell's blocking→paint→deferred gap (t517). Gates:
`g_esm_import_graph` (loader core, in-memory), `g_esm_page_graph` (load_async, real localhost 2-level
graph), `g_esm_prefetched_graph` (shell path, same graph across paint). **The trap:** `ModuleLink` is
synchronous on the JS thread and there is no blocking network here, so the whole reachable graph must be
pre-fetched BEFORE any module runs — the scanner is a superset-or-miss heuristic that only decides what to
fetch, while `esm_load_graph` (reading SpiderMonkey's real `GetRequestedModuleSpecifier`) stays the
authoritative walk, so an over-fetch is harmless and a miss fails one import loud-but-safe, never a crash.
Residue: dynamic `import()` uses a separate lazy hook (still unresolved).

**Bare specifiers resolve through a `<script type=importmap>` (tick 520)** — so a CDN-pinned no-bundler app
(`import {h} from 'preact'` with `{"imports":{"preact":"https://esm.sh/preact"}}`) boots. The page parses
the import map's flat `imports` object (`extract_import_map`, serde_json), carries it on `Page`/`Prefetched`
beside the graph sources, and seeds it into the JS layer (`IMPORT_MAP`) for the module pass. One
`resolve_module_specifier` now governs BOTH the resolve hook and the graph walk: a relative specifier
resolves against its importer, a BARE specifier (not `./ ../ /`, not a URL) is looked up in the map — exact
key first, then the longest trailing-slash PREFIX key (`"utils/"` maps `utils/num.js`) — and its target
resolved against the DOCUMENT url; an unmapped bare specifier returns null (loud-but-safe, `ModuleLink`
fails there, exactly as before). The page pre-fetch mirrors the same resolution so mapped urls are fetched
too. Gate `g_esm_import_map` drives both forms end-to-end over localhost; RED = empty map → bare specifier
unresolved → the app does not render. Residue: import-map `scopes` (per-path overrides) not yet honoured;
dynamic `import()` still separate.

**A `<video>` has a running clock — `timeupdate`/`ended` fire, `currentTime` advances (tick 521)** — so a
progress bar tracks, a `% watched` analytics beacon sends, a synchronized transcript scrolls, an
ad-cue/chapter marker triggers, and a playlist advances to the next track on `ended`. `play()` used to flip
`paused` and stop; `currentTime` sat at 0 and the two most-bound media events never fired. Now `play()`
fires `play`→`playing`, `el.__advance(delta)` (in `__manukMedia`) moves `currentTime` by `delta ×
playbackRate` and fires `timeupdate` per step, reaching a finite `duration` clamps and fires a final
`timeupdate` then `ended` (not `pause` — the spec routes end-of-media to `ended`), a `loop` clip wraps to 0,
and `play()` after `ended` replays from 0. `volume`/`muted`/`playbackRate` setters fire
`volumechange`/`ratechange`. The clock is HOST-DRIVEN through one entry point,
`__mediaAdvance(nodeId, elapsedSeconds)` (the shell's frame loop, holding the audio/wall clock, calls it) —
a self-pumping `setTimeout` would spin forever on the muted `autoplay loop` background clip. Gate
`g_media_playback_clock` drives that exact seam; RED = neuter `__advance` → clock frozen at 0. Residue: the
shell frame loop does not yet call the seam (GUI driver = next integration); `seeking`/`seeked` on a
`currentTime` write are a separate scrub brick.

**Writing `<video>.currentTime` is a real seek — scrub bars, chapter jumps, resume-position work (tick
522)** — so dragging a scrub bar, clicking a chapter marker, or a "resume where you left off" that sets
`currentTime` fires `seeking`→`seeked` (a player hides its buffering spinner on `seeked`), moves the clock,
and repositions the host decoder. The `currentTime` setter is now the seek algorithm: a write to a NEW
position raises `seeking`, clamps into `[0, duration]` (a scrub past the end lands on the end, not on empty
media), publishes the position to the host on the same live-write channel volume/rate use
(`__mediaProp(nodeId,"currentTime",n)`), then fires `seeked`+`timeupdate`; a same-position write is not a
seek (no per-frame event storm); a backward seek after `ended` clears `ended` (rewatch from the end).
`seekable` reports the live `[0, duration]` span, and `fastSeek(t)` shares the path. Gate `g_media_seek`
asserts the JS events + clamp AND the host seam (`take_media_props` must contain the final seek). RED: drop
the seeking/seeked dispatch, the clamp, or `"currentTime"` from the `media_prop` allow-list.

**`<video>.played` is the union of actually-watched spans (tick 523)** — so watch-progress analytics
("you've watched 80%"), the "continue watching" resume marker, and per-segment engagement heatmaps read a
real TimeRanges instead of a frozen empty one. As the playback clock advances, `__addPlayed(from, new)`
inserts the just-played span into a sorted, non-overlapping list, MERGING adjacent/overlapping ranges
(playing 0→5 is ONE span, not five); a seek does not play, so skipping the middle leaves a genuine hole
(`played` is a union, not an envelope), and seeking back into a gap and playing merges the spans down. This
completes the JS-visible playback model — forward clock (521) + seek (522) + played (523). Gate
`g_media_played`; RED: drop the `__addPlayed` calls and every range collapses to empty.

**`<video>` fires `durationchange` when the MSE timeline length becomes known (tick 524)** — so a player
(hls.js/dash.js/shaka and hand-rolled ones) sizes its scrub bar, computes "% watched", and enables seeking
the moment the length arrives. `mediaSource.duration = N` (live/DVR) or a demuxed moov (VOD) set the
length, and the element's `duration` getter reflected it — but silently, so a `durationchange` listener
never woke. Now `MediaSource.__fireDurationChange` dispatches it on the attached element from both the
setter (on real change only — NaN→N fires, N→N does not) and the demux path. Gate `g_media_durationchange`;
RED: drop the dispatch and dc stays 0.

**A popover ToggleEvent names its invoker via `ToggleEvent.source` (tick 526)** — so a menu/tooltip
framework that opens one popover from several `<button popovertarget>` controls knows WHICH button fired
and anchors/focuses the popover on it. `beforetoggle`/`toggle` now carry `source`: the button for a
declarative open, the `{source}` option for `showPopover({source})`/`hidePopover({source})`, `null` for a
bare call. Threaded through `__popToggleEvent` + `__popClick` (`{source: t}`) + `togglePopover({force,
source})`. Gate `g_toggle_event_source`; RED: drop `ev.source`. Residue: the `<dialog>` toggle path
doesn't carry source yet (a command-invoker follow-up).

## Wasm glue that registers a finalizer — `new FinalizationRegistry(fn)` (tick 546)

**Pattern:** `const registry = new FinalizationRegistry(ptr => wasm.__free(ptr))`. Nobody writes this
by hand; **`wasm-bindgen` and Emscripten emit it in their standard JS glue**, and so do libraries that
hand out handles to native resources. It is the standard way JS-side code releases something a
non-JS heap owns.

**The class this unlocks:** any embedder-driven eval of code that touches finalizers. `typeof
FinalizationRegistry` was `"function"` — so every feature detector said yes — and constructing one
**segfaulted the process**. The constructor asks the host for the *incumbent global* through
`JS::JobQueue`; the bare `SpiderMonkeyRuntime` seam installed no queue and dereferenced a null one.

**Scope, stated precisely rather than dramatically:** the page path was already safe, because
`event_loop::install` installs the queue when it builds a document's global. The crash was on the
[`JsRuntime`] seam — `manuk eval` and any other embedder of `manuk-js`. **The real defect is that two
constructors of the same engine set the host up differently and nothing said so**, which is the kind of
bug a gate cannot catch because neither path is wrong on its own.

**The traps.** **(1)** A crash reports *nothing* — no exception, no log, no failing assertion — which is
why this lived for 500+ ticks with a live constructor. **(2)** `typeof X === 'function'` is satisfied by
a constructor that kills you; presence is not capability, and this is the fifth time that has been
written down here. **(3)** The fix is the *job queue*, not the cleanup callback — installing
`SetHostCleanupFinalizationRegistryCallback` alone does not stop the crash (measured). **(4)** Never
firing the cleanup callback is **spec-legal** (ECMAScript does not require an implementation to ever
call one), so the honest state is "the registry works, callbacks do not fire" — a legal answer, not a
lie, and draining `doCleanup` through the real job queue is the named follow-on. **(5)** Found by
running **test262**, not by the corpus crawl: no corpus site happens to construct one, which is the
whole argument for an instrument that carries its own verdict.

## A page that names its fonts — `font-family: "Inter", "Helvetica Neue", Arial, sans-serif` (tick 557)

**Pattern:** every branded site on the web names its typefaces. A design system ships
`font-family: "Inter", system-ui, sans-serif`; a docs site names its mono face; a news site names its
display serif. The generic keyword at the end of the stack is the *fallback*, not the intent.

**The class this unlocks:** correct text measurement on any page that names a font it has, or that the
system has. `fontdb::Family::Name` matching is **case-sensitive** and we lowercased the family before
querying it, so the lookup returned `None` for every mixed-case family name — i.e. **all of them** — and
every named family silently became `sans-serif` or a `contains("serif")` guess. Two real installed
families and a deliberately non-existent one all rendered the same width.

**The traps.** **(1)** This is invisible per-element and enormous in aggregate: the page renders, the text
is there, every box is *slightly* the wrong size, and no single screenshot looks broken — the
`subpixel-error-compounds` failure mode with a font on top. **(2)** It produces **two** unlike symptoms
from one cause — text widths wrong in *both directions* (per-glyph advances) and a *constant* line-box
height error (ascent+descent) — so a debugger chasing either one alone concludes "box model" or
"line-height" and is wrong twice. **(3)** The generic stacks measure FINE, which is what makes it survive:
every synthetic test page that says `sans-serif` passes. **(4)** Case-insensitivity is a CSS property, not
a font-database property — CSS matches families case-insensitively, `fontdb` does not, and the boundary
between those two facts is exactly where the bug lived. **(5)** The `@font-face` map is keyed lowercase for
the CSS reason, so preserving case for the system query means lowering it again for the webfont lookup;
miss that and you trade a system-font bug for a webfont bug.

**Honest limit:** resolution is fixed; the *advance* does not yet follow the resolved face (measured — five
families still render one width), so the pattern is not closed. `tests/wpt/probes/font-family-resolution.html`
is the standing proof: five declarations must produce five widths.

**Update (tick 558) — the pattern is now CLOSED.** t557 fixed the resolution and the widths did not move:
`intern_family` stored the lowercased key, so `face_id` re-queried the case-sensitive
`fontdb::Family::Name` with lowercase, missed again, and every family still fell back. **A fix upstream of a
lossy step is not a fix**, and the t557 assertion lived at the resolution layer where everything already
looked right — so the t558 assertion measures the **WIDTH** instead (417 installed mixed-case families; more
than one distinct face AND more than one distinct width required). Measured against live Chromium on the
committed probe: **SHAPE 36.4% → 90.9%**, misplaced spans **5 of 5 → 1 of 11**. **Trap (6), the one this
pair is really about: assert on the OBSERVABLE, not on the intermediate.** The intermediate was correct after
t557 and the rendered page was not. Residual, named: an *unknown* family falls back to sans here and to serif
in Chromium — a default-family divergence, not a resolution one.


## A page whose webfont fails to download — `@font-face` shadowing (tick 561)

**Pattern:** every site that self-hosts or CDN-hosts a typeface. `@font-face { font-family: "Inter"; src:
url(/fonts/inter.woff2) }` plus `font-family: Inter, system-ui, sans-serif`. The interesting case is not the
happy path — it is the **download failing**: a CDN blip, an ad-blocker, a CSP rule, a format we cannot decode.

**The class this unlocks:** honest degradation when a webfont does not arrive. Per CSS Fonts the declared
family **shadows** any same-named local font for that document, so a failed `src` means the family has no
usable face and matching moves to the **next entry in the stack**. We instead fell back to a same-named local
face — so a *failed download* rendered as *a different font*, page-wide, with different advances and a
different line box.

**The traps.** **(1)** It is only reachable once named families resolve at all — before that everything fell
to a generic and the bug was invisible, which is how a fix creates the conditions for the next bug to matter.
**(2)** The rule keys on the **declaration**, not the load: register the family name *before* attempting the
fetch, or a failure is indistinguishable from "we don't have that font". **(3)** The symptom is not a missing
glyph — the page renders, in the wrong metrics, everywhere — so it reads as a layout bug and gets chased in
the wrong subsystem. **(4)** Do not over-fit: a *loaded* webfont must still win, so shadowing is precedence,
not suppression. **(5)** ⚠ This was diagnosed from a site (`martinfowler.com`) that turned out **not** to
have the pattern at all — it names `Open Sans` with no `@font-face` anywhere. The rule is right, the site was
the wrong witness, and the lesson is the recurring one: **read the page before believing a mechanism that
fits the numbers.**

## A stylesheet that `@import`s another — Google Fonts and CSS architecture (tick 564)

**Pattern:** `@import url(https://fonts.googleapis.com/css?family=Lora:400,700);` at the top of a site's
one stylesheet — the classic Google Fonts delivery — and its architectural twin, an entry-point sheet that
`@import`s `tokens.css`, `components.css`, `layout.css` so the page needs one `<link>`.

**The class this unlocks:** every rule and every `@font-face` inside an imported sheet. We never fetched
imports at all, so an `@import` chain was **silently deleted** — not degraded, deleted. The symptom appears
far from the cause: `martinfowler.com` imports Open Sans, Inconsolata and Lora, so Chromium resolved
`{Lora/13}` where we fell back to `{serif/13}`, and the diff only became readable once the instrument carried
the computed font (t563).

**The traps.** **(1)** `@import` is relative to the **importing sheet's** URL, not the document's — resolve
against the wrong base and every import 404s. **(2)** A **media list** may follow the URL
(`@import url(print.css) print;`) and it must not swallow the URL, and it must not prevent the FETCH: the
enclosing `@media` decides *application*, the network decides *delivery*, and conflating them drops a sheet
the page may still need. **(3)** Imports **chain**, so a single pass finds only the first level (tokens →
components → page is ordinary), and an unbounded walk is a **cycle waiting to hang a tab** — depth-bounded,
because Bar 0 outranks the last sheet in a chain. **(4)** Dedupe through the same map the `<link>` sheets use,
or a re-entry after dynamic scripts re-fetches the whole chain. **(5)** The imported sheet must reach the
**cascade**, not only the `@font-face` scan — an import that carries fonts almost always carries rules too,
and wiring only the font path is the kind of half-fix that looks like it worked.

## A page whose search box is wrapped in `<search>` (tick 568)

**Pattern:** `<search><input type="search" name="q"><button>Go</button></search>` — the modern wrapper
(Baseline Apr 2026) replacing `<div role="search">`. Sites adopt it precisely because the role is implicit.

**The class this unlocks:** finding the search affordance **by role**. `Role::Search` existed for the explicit
`role="search"` attribute; the ELEMENT fell through to `Role::Generic`, so the landmark vanished on any site
that adopted the wrapper.

**And the reason it belongs in this ledger rather than only in an a11y one:** `manuk-a11y` already feeds
`manuk-agent`'s observation channel (CONSTITUTION VI.1), so an unmapped landmark is a missing **affordance** —
the agent has to fall back to guessing from `input[type=search]` or placeholder text, which is the
coordinate-and-heuristic brittleness the semantic surface exists to remove.

**The traps.** **(1)** The ARIA half being present makes the HTML half look done — `grep Role::Search` finds a
hit and the map arm is still missing. **(2)** The fix must not shadow the explicit attribute path; assert both.
**(3)** When testing the tree, locate nodes **by tag, not child index** — `<head>` is `<html>`'s first element
child, so an index walk silently inspects head and reports nothing, which fails (or passes) for the wrong
reason. **(4)** This class of gap is systematically under-found: every audit that reconciles against CSS-shaped
sources walks past it, and it took an audit that read what actually SHIPPED to surface it.

## A CSS Grid whose tracks are `auto` — i.e. almost every grid on the web (tick 569)

**Pattern:** `display:grid` with tracks the author never sized explicitly — `grid-template-areas:"sidebar main"`
with no `grid-template-columns`, an implied column from auto-placement, `grid-auto-columns` left at its initial
`auto`. The author's mental model is "the grid fills its container and the tracks divide it up", and that model
is correct *because* CSS Grid §11.8 **Stretch auto Tracks** gives every `auto`-max track a share of the
container's leftover space.

**The class this unlocks: every grid layout that does not hard-code its track widths.** §11.8 runs **only when
the inline axis is stretch-aligned**, and the inline axis alignment is `justify-content`, whose initial value is
`normal`. Our CSS enum had no `Normal` variant, so both cascades stored the initial value as `FlexStart` and
handed taffy a concrete `FLEX_START` — which meant **no grid this browser has ever laid out ran §11.8**, whether
or not the page mentioned `justify-content`. The visible result is a two-column layout huddled against the left
edge with the container's right half empty: nothing missing, nothing misplaced, every item in the right cell,
the columns simply content-sized. Measured against live Chromium on a 600px container: **88px / 133px where
Chromium gives 289px / 291px** → now 267px / 313px.

**The traps.** **(1)** It reads as a *placement* bug and it is an *alignment* bug — four ticks hunted it in the
grid code, and the defect was one enum in the cascade. **(2)** `normal` is a **context-dependent keyword**:
flex-start in flex, stretch in grid. Flattening it onto one meaning at parse time is exactly the mistake, and
`auto` and `stretch` are the rest of that family. **(3)** Where a borrowed layout library models the distinction
as an `Option`, **that `Option` is the contract** — filling it in discards the information the library was about
to use. **(4)** The fix's own failure mode is replacing one hard-coded alignment with another, so an explicit
`justify-content:center` must still leave the tracks content-sized, and flex must still pack at the start;
both are asserted as guards beside the feature in `G_GRID_IMPLIED_TRACK_STRETCH`. **(5)** The `repeat(auto-fill,
minmax(…,1fr))` responsive-card idiom is a **separate** unfixed bug in the same area — both cascades collapse it
to one column — and conflating the two is how the martinfowler.com hunt acquired four wrong answers.

## The responsive card grid — `repeat(auto-fill|auto-fit, minmax(…, 1fr))` (tick 570)

**Pattern:** one declaration that replaces a stack of media queries — a card list, a product grid, a
docs-site topic list, a dashboard tile wall. The author states a *minimum* card width and the browser
decides how many columns fit. It is the default way responsive grids have been written since Grid
shipped, and `auto-fit` is the variant that lets a short list still span the container.

**The class this unlocks: every card/tile/product grid on the web.** Both of our cascades dropped the
auto-repeat independently and produced **one full-width column** — Stylo's `RepeatCount::AutoFill` fell
through a `_ => 1` catch-all, and the text cascade's string rewrite matched the first `)` after
`repeat(`, which belongs to the nested `minmax(`. Measured against live Chromium: `auto-fill`
`minmax(180px,1fr)` in 600px → three 187px tracks (we gave one 600px), `auto-fit` with two items → two
290px tracks (we gave one 600px), martinfowler.com's `minmax(18em,1fr)` at 619px → two 300px columns
(we gave one). Probe SHAPE **15.0% → 100.0%**, absolute placement **0.0% → 100.0%**.

**The traps.** **(1)** The count is **not the cascade's to compute** — CSS Grid §7.2.3.1 defines it
against the container's *resolved* inline size, so the cascade must carry the auto-repeat as a shape and
let layout count. Same lesson as tick 569's `justify-content: normal`, one tick apart, and both times
the borrowed layout library had already modelled the distinction we flattened. **(2)** `auto-fit` is
`auto-fill` **plus collapsing the empty repetitions** (gutters included). Implementing only the
generation looks right on `auto-fill` and leaves a third of every `auto-fit` row blank. **(3)** Parsing
`repeat()` by scanning for the next `)` breaks on the *one* argument shape that matters, because
`minmax()` nests — pattern-matching text where the grammar nests is a bug waiting for its input.
**(4)** `repeat(100000, 1fr)` is legal CSS; expanding an integer count needs a bound, or one declaration
allocates until the tab dies. Bar 0 outranks fidelity to a track list no page can see.

## The site-builder mega-CSS page — megabytes of inline `<style>` in dozens of blocks (tick 571)

**Pattern:** a marketing/landing page generated by a site builder or CMS component system, which inlines
every component's CSS into the document rather than linking a sheet — wix.com ships **1.8 MB of CSS
across 68 separate `<style>` blocks** in a 3 MB document, and hubspot, asana and replit are the same
shape. It is not authored CSS; it is a build tool concatenating every component's styles because inlining
removes a render-blocking round-trip. The trade the tool is making is *network latency for parse cost*,
and it assumes parse cost is free.

**The class this covers: builder-generated marketing pages, which are a large fraction of the commercial
web** — and the first thing a new user is likely to open.

**What it costs us, measured (tick 571, `manuk-wpt memtabs`):** wix.com alone made the process resident
set **1.31 GB**. Bisected against the input rather than guessed: strip `<script>` → 655 MB; strip
`<style>` → **143 MB**; keep all 68 `<style>` blocks but give the document a one-element body → 65 MB.
CSS-only 65 MB plus DOM-only 143 MB is 208 MB against **1308 MB** together, so **the cost lives in
neither input but in the cascade's cross product of them**. It is also *transient* — the retained
`StyleMap` is ~11 MB — which is why it was invisible to every per-tab number we had: the spike is freed
immediately and then held by the allocator forever.

**It is a TIME defect too — and tick 572 found that the time and the memory are DIFFERENT causes, not one.**
The same wix.com page took **164.7 seconds to load**. `STATUS.md` has carried `ORACLE_HANGS: 31` — pages over 30 s
on our own clock — as a separate top-of-file concern; this pattern is very likely a chunk of it. One root
cause, two dashboards, no one had joined them.

Tick 572 fixed the time half: `cascade_pseudo` re-walked every rule in every sheet **twice per element**
to find the few carrying a `::before`/`::after`, which was 46% of every cascade; indexing those rules once
per document took the load to **101.8 s (-38%)**. **Memory did not move at all — 1308 MB before and after.**
So this pattern is two defects that happened to share a page: an `elements x rules` TIME cost in the pseudo
matcher, and a ~1.3 GB of transient allocation in the per-element tail of the cascade.

**Tick 573 closed the second one, and it was ONE loop.** These pages ship design-token sheets — wix
declares **575 custom properties** — and custom properties INHERIT, so every element's computed style
carries a copy of the whole vocabulary (1.44M entries per cascade). The copy was written as
`while let Some(..) = cp.property_at(i) { i += 1 }`, and `property_at` is `iter().nth(index)` — so the
loop was **quadratic while reading linear**. One `iter()` instead: wix **101.8 s -> 26.5 s** and
**1308 MB -> 471 MB**, and across the whole 100-site corpus **4390 MB -> 2457 MB (-44%)**. The pattern's
cost is now ordinary rather than pathological.

**The token sheet is the thing to watch for in this pattern, not the CSS bytes.** A page-weight or
node-count budget sees nothing unusual; what makes these pages expensive is `elements x tokens` and
`elements x rules`, both invisible in any single-axis measurement.

**The traps.** **(1)** A page-size or node-count budget does not catch this: 3 MB of HTML and ~10k nodes
are unremarkable, and the blow-up is 400x the input. **(2)** It is invisible to retained-heap accounting
(`Page::estimated_bytes` reported 11.1 MB against 1305 MB of real growth), so the honest instrument is
process RSS around the load, not a heap walk after it. **(3)** Four sites of this shape in a 100-site
corpus produced **76% of the whole 100-tab footprint** — so a mean, or any "typical site" figure, hides
the entire problem. Report the p90 and name the outliers.

---

## The close request — Escape dismisses the topmost overlay, and exactly one of them (tick 574)

| pattern | where it shows up | status |
| --- | --- | --- |
| **A close request (`Escape`, Android back) dismisses the TOPMOST dismissable and only it** — modal `<dialog>`, `auto` `[popover]`, and script-owned overlays via **`CloseWatcher`** | every app-class page that stacks UI: a command palette that opens a menu, a confirm-dialog over a dropdown, a settings sheet with a select popup — plus every hand-rolled drawer/lightbox/palette that is neither a `<dialog>` nor a `[popover]`, which is still most of what ships | ✅ (tick 574) — `CloseWatcher` was **absent** (measured and pinned t568), so a script-owned overlay had no way to answer Escape at all and a feature-detecting page took its fallback. Building it surfaced a **live bug it was designed to prevent**: `__dialogEscape` (t194) and `__popEscape` (t195) were two independent, unconditional `keydown` capture listeners on `document`, so one Escape over a modal that had opened a menu dismissed **both**, and `__popEscape` looped every open `[data-manuk-popover-open]` closing all of them — one keypress could clear the entire top layer. **Neither feature's own gate could see it**: `g_dialog` and `g_popover` each proved *their* construct closes on Escape, and both assertions stayed true; the defect lived only in the seam between two individually-correct features. Now one `__closeStack` of `{active, request}` entries and one listener: `showModal()`/`showPopover()` (auto only — `manual` is not light-dismissable) / the `CloseWatcher` constructor enrol; the scan walks from the top, reaps entries whose `active()` went false (closed meanwhile by a click, by script, or by popover exclusivity), and stops at the first **active** entry — *not* the first that actually closed, which is what makes a `cancel` veto keep its place instead of falling through to the overlay underneath. `CloseWatcher` hand-rolls its listener surface because this engine's `EventTarget.prototype` is the DOM chain's root (`iface` gates on `isNode`), with the one required difference from `MediaSource`/`WebSocket`: `dispatchEvent` returns `!ev.defaultPrevented`, since that boolean **is** the veto `requestClose()` reads. Gated by `G_CLOSE_WATCHER`, RED-proven twice — restore the private dialog listener → `dlgkept:false` (one Escape, two dismissals, reproduced); let a vetoed request fall through → `w2untouched:false`. Residue: no back-gesture/`navigation` integration (Escape is the only close-request source here), and the spec's user-activation grouping (one "free" watcher without a gesture) is not modelled.

**The lesson, and it generalises past this feature.** Two ticks each added a global listener for the same
input, each gated alone, each correct alone. A per-feature gate is *structurally* unable to reach that
seam, because neither gate ever opens the other's construct. **Look for this wherever two features
independently register a handler for one shared input** — Escape, the back gesture, `beforeunload`,
outside-click, focus-trap boundaries. The tell is not a failing assertion; it is that no assertion exists
which opens both at once.

---

## The CSS reset — an author `*` rule must beat a UA type rule (tick 575)

| pattern | where it shows up | status |
| --- | --- | --- |
| **`* { margin: 0; padding: 0 }`** and its descendants — Tailwind's preflight (`*,::before,::after{...margin:0;padding:0}`), Normalize/`sanitize.css`, Bootstrap's reboot, and every hand-rolled reset since 2004 | the **first stylesheet rule on a very large fraction of the open web**, and the one that decides a page's entire horizontal and vertical rhythm before any of its own layout runs | ✅ (tick 575) — it did not apply. Our matcher merged winning declaration blocks by `(specificity, source order)` with **no origin term**, so `UA_CSS`'s `body { margin: 8px }` (0,0,1) beat the author's `*` (0,0,0). A reset is written with the **weakest possible selector on purpose**, which is precisely the shape that loses a specificity tie-break — so being one origin too high made our UA sheet beat the rules that exist to override it. Every rule in `UA_CSS` carries a type or descendant selector, so this was never about 8px: `ul, ol { padding-left: 40px }` and `blockquote { margin: 1em 40px }` survived the same reset, and any author rule deliberately written weak lost. Fixed by parsing the sheet as `Origin::UserAgent` **and** leading the merge sort with an `origin_rank` — the parse change alone is inert, because the Stylist's own origin machinery is bypassed by our `RuleIndex`. Gated by `G_CASCADE_ORIGIN`. |

**The shape to look for.** The old comment said *"the UA sheet is matched first (lowest priority); author
rules override it"* — true of the append order, false of the outcome, because **document order is the
cascade's last tie-break, not a way to express priority.** Any invariant of the form *"X always loses to
Y"* has to be a **sort term**, never a position in a list; a position is only ever consulted after
everything else has tied. The same question is worth asking of `@layer` ordering and of the `:has()`
supplement's second pass, both of which are currently expressed positionally here.

**Blast radius, and why it was found by measurement rather than by a gate.** It was filed at tick 556 as
a side-observation of a *font* probe — Chromium put `body` at `[0 0 1200×92]` where we put it at
`[8 8 1184×91]` — because no gate on this side of the tree ever wrote an author rule that *ought* to lose
on specificity and *ought* to win on origin. Every cascade gate we had tested author-vs-author.

---

## Progressive enhancement — `@supports` is a bet the page places on our answer (tick 576)

| pattern | where it shows up | status |
| --- | --- | --- |
| **`@supports (<modern-property>: <v>) { … }`** and `CSS.supports()` — the feature-detect that decides whether a page keeps its fallback or commits to the modern path | universal on the design-forward web: frosted-glass headers (`backdrop-filter`), view transitions, `offset-path` motion, `contain`, the `mask-*` family, container-query and anchor-positioning branches. The whole point of the construct is that the page **deletes the working fallback** when told yes | ✅ (tick 576) — we said **yes to 31 properties we do not render**. Stylo's servo build hides 35 longhands behind one shared `layout.unimplemented` pref; the cascade flips it because **four** of them are real here (`user-select`, `color-scheme`, `mask-image`, `text-overflow`), and the other 31 became *parseable* as a side effect — which is exactly what `@supports` and `CSS.supports()` answer. The flip's comment claimed it "changes nothing we read"; `@supports` reads it. Fixed by a measured denylist plus a condition-tree rewrite handed back to Stylo (so `not (unsupported)` still composes correctly), applied at all three sites the cascade descends into an `@supports` block. Gated by `G_SUPPORTS_HONESTY`, RED-proven on both the list and the composition. |

**The failure mode is asymmetric, and that decides the design.** A false **no** costs a page its
enhancement and leaves it looking like an older browser — annoying, and *working*. A false **yes**
costs it the fallback it wrote and tested, and there is nothing underneath. So the list is a
**denylist**: a property a future dependency bump puts behind that pref defaults to unsupported, and
only an explicit deletion promotes it. Everywhere this trade appears — capability strings, codec
`isTypeSupported`, `queryCommandSupported`, permission queries — the same asymmetry holds, and the
default should fall the same way.

**And the measurement that produces the list is not the obvious one.** Deriving "what do we render?"
from `clone_*` accessors gives **two** of the four; `mask-image` and `text-overflow` arrive through
the MinimalCascade recovery block instead, so the obvious grep would have made two properties every
page uses answer "no". The question that works is *"does it reach a `ComputedStyle` field?"* — the
consumer, not the accessor.

---

## The honest "no" rots in BOTH directions — and the fixture can hide it (tick 576)

| pattern | where it shows up | status |
| --- | --- | --- |
| **An assertion that a capability is absent**, written truthfully, and never re-measured after the capability landed | every `honestly returns false` / `not built yet` claim in the tree — the currency this project pays for honest failure | ✅ (tick 576) — `g_exec_command_copy` carried **two** stale claims. `queryCommandSupported('bold') === false` was written at tick 463 and `execCommand('bold')` **landed at tick 481**: red on disk for ninety-four ticks, unnoticed because the wall launches ~19 of ~280 page-test binaries. `execCommand('cut') === false` was worse — it was **passing**, because the only selection the fixture ever made was inside a `<pre>` and `cut` correctly declines outside an editing host. Both replaced by claims with teeth: bold reports supported *and* wraps a real selection in `<b>`; cut declines outside an editable, succeeds inside one, and the text is really gone. |

**Two distinct rots, and only one of them is loud.** The `bold` line went red the day the capability
landed and simply was not watched. The `cut` line stayed **green for 113 ticks while being wrong** —
the t573 fixture lesson again: *an assertion whose fixture cannot reach the mechanism is green for a
reason unrelated to the claim.* The second kind cannot be found by running the tests, only by asking
of each capability-denial: **what would this fixture have to do to make the "no" turn into a "yes",
and does it do it?**

The standing rule (`honest-answer-is-not-a-fixed-answer`) is usually read as *the engine's* "no"
becoming a lie. It runs the other way too: here the engine told the truth and the **test** was the
lie. Same failure — a claim about capability that nobody re-measured after the capability moved — and
the same correction: **the gate follows the capability, never the reverse.**

---

## The page as READ — hyphenated words, URLs and long tokens in the agent's observation (tick 577)

| pattern | where it shows up | status |
| --- | --- | --- |
| **Extracting "the text of this page"** for an agent to reason over, for full-text history search, for a summarizer or a find-in-page | `Observation.text` (what `manuk-agent` hands a model) and `store::history_index`'s embedded body — the two places the browser's *semantic* output is consumed rather than looked at | ✅ (tick 577) — every **break opportunity** was rendered as a space. The line breaker emits one fragment per opportunity, not per line, and CSS puts one after a hyphen, after `//`, and after `?` in a query string, so `visible_text`'s `join(" ")` produced `non- mainstream`, `https:// walled.example/? a=1&b=2`, `bot- challenge`, `standards- based`. Fixed with the geometry already on the fragment: same baseline + touching boxes ⇒ one word. Gated by `G_VISIBLE_TEXT_RUNS`, RED-proven on both halves of that condition. |

**The class, and it is bigger than this function.** A rendering engine has two outputs: pixels and
**text-for-machines**, and only the first has instruments. Every gate, cluster and fidelity score in
this repo compares *boxes* — so a defect that produces correct boxes and a wrong string is invisible to
the entire apparatus by construction. This one was found by a `contains()` assertion in a test about
honest error pages, which the wall does not even launch.

**What that implies for the agentic surface.** For an agent-native browser the extracted text is not a
convenience API, it is the product; a model that cannot find `non-mainstream` on a page that plainly
says it will conclude the page does not say it, and will act on that. The same question is worth asking
of every other machine-facing string we emit — accessible names, `innerText`, link text, the a11y
tree's labels: **is it assembled from the DOM (safe), or from laid-out fragments (needs this rule)?**

**And the tell for the next one.** The bug produced a *plausible* string. It read like English, it
passed casual inspection, and it broke exactly the queries a user or a model would issue. Corrupted
output that still looks like output is the hardest kind to notice, and the only defence is asserting on
a value the defect must change — here, `!contains("non- mainstream")` alongside `contains("non-mainstream")`.

### Three consumers, not one — and the fix is a predicate, not three patches (tick 578)

Asking the follow-up question — *which other code assembles a string from laid-out fragments?* — took one
grep for `BoxContent::Inline` and found the identical bug twice more, both **user-facing shell features**:

| consumer | what a user saw |
|---|---|
| `Page::visible_text` | the agent's `Observation.text` and the history search index (t577) |
| `shell::find` | **Ctrl+F** for `non-mainstream` or for a URL found **nothing** on a page containing it |
| `shell::gui` selection | **Ctrl+C** on `non-mainstream` pasted `non- mainstream` |

So the class this unlocks is not only the agentic one: **find-in-page and copy now work on any page
containing a hyphenated compound, a URL, or a long token** — which is nearly every page. `find.rs`'s own
comment had stated the wrong premise outright (*"inline layout drops the original whitespace"*), which is
how three authors reached the same wrong answer independently.

The rule now lives on the data as `TextFragment::continues(&prev)`, so a fourth consumer is handed the
question where the geometry is still in scope. **Each consumer keeps its own assembly loop** — they
genuinely differ (whole-document concatenation vs per-run byte spans for hit-mapping vs per-line grouping)
— so *the shared thing is the predicate, not the loop*. A `join_runs()` helper would have been abandoned by
the first consumer that needed spans.

> **The generalisation.** When a defect is found in one consumer of a data structure, the question is not
> *"are there other bugs like this"* but **"what else reads this structure, and does it ask the same
> question?"** — a grep for the *type*, not for the *symptom*. `BoxContent::Inline` had seven readers;
> three assembled text and all three were wrong.

---

## The Tailwind v4 palette — `oklch()` and `color-mix()` as the DEFAULT colour syntax (tick 579)

| pattern | where it shows up | status |
| --- | --- | --- |
| **`oklch()` colour literals and `color-mix(in oklab, … N%, transparent)`** | Tailwind v4 emits both **by default**: every `text-slate-700`/`bg-blue-500` is an `oklch()` literal and every `/50` opacity utility compiles to a `color-mix()`. Plus `lab()`, `color(display-p3 …)` and the rest of CSS Color 4 on design-led sites | ✅ (tick 579, **measured — it already worked**) — inherited free through Stylo and never once asked for. Five declarations probed, four reproduce a from-scratch derivation off the CSS Color 4 matrices **to the integer**; the mix honours its percentage. Gated by `G_OKLCH_COLOR_MIX`, RED-proven by moving the declaration and by moving the mix ratio. |

**The failure this would have been is the silent kind.** Had `oklch()` not resolved, every Tailwind v4
site would have rendered in a fallback colour with **no error and no structural divergence** — same boxes,
same text, wrong colours. Nothing in this project's instrument set compares colours across a corpus, so it
would have gone unnoticed indefinitely while the affected population grew every month.

**And the reason it was measured rather than built: the map distinguished `unknown` from `missing`.** The
audit that surfaced it refused to write `missing` on the strength of a grep, because *a grep is not a
measurement when the capability lives below you* — `oklch` appears nowhere in `engine/`, and Stylo is a
dependency. Collapsing those two statuses would have filed a working capability as weeks of work. This is
the fifth already-built phantom this project has caught, and the first found by asking the question about
a **dependency** rather than about our own code.

---

## Responsive images — `srcset`, `sizes` and `<picture>` (tick 582)

| pattern | where it shows up | status |
| --- | --- | --- |
| **`<img srcset="… 400w, … 800w" sizes="…">`** and **`<picture><source type media srcset>`** | WordPress emits `srcset`+`sizes` on essentially every content image; so does every modern CMS, Next.js `<Image>`, and every image CDN. `<picture>` is how a site ships AVIF/WebP with a JPEG fallback | ✅ (tick 582) — **every candidate list was ignored and the `src` fetched every time.** Now x- and w-descriptor selection, `sizes`, `<picture>` `<source>` with `media` and `type`, and the `srcset`-with-no-`src` case. Gated by `G_SRCSET_SELECTION`, RED-proven on the `type` skip and on the candidate choice. |

**The row that described this said *"2× displays get 1× images"*, and that framing is why it sat
unmeasured for hundreds of ticks.** The real failure is worse in three separate ways:

- On a **`w`-descriptor list the `src` is frequently the *smallest* candidate** — so we were not
  serving a 1× asset to a 2× screen, we were scaling a thumbnail across a hero.
- **`<img srcset>` with no `src` is legal** and common. There we requested *nothing at all* and rendered
  an empty box.
- **`type` is load-bearing in the opposite direction from the obvious one.** AVIF is deliberately off in
  this build (`image` is compiled with png/jpeg/gif/webp/bmp/ico; AVIF's decoder is C dav1d). So the
  valuable half of `<picture>` is not *choosing* the modern format — it is **skipping** the one we cannot
  decode and taking the author's fallback. Choosing it would render nothing, which is strictly worse than
  ignoring `srcset` altogether. An engine with a narrow decoder set needs `<picture>` support *more* than
  one with a wide set, which is the reverse of the intuition.

**Residue, named rather than discovered later:** DPR is fixed at 1, and `sizes` resolves its
*unconditional last entry* rather than evaluating the media-condition list. A page whose first matching
condition would choose differently gets one candidate step off — against a baseline of fetching the
thumbnail every time.

---

## Patching `localStorage` — the private-mode / SSR / analytics wrapper (tick 587)

| pattern | where it shows up | status |
| --- | --- | --- |
| **`localStorage.setItem = wrapper`** (and `spyOn(localStorage, 'setItem')`) | private-mode and quota fallbacks that catch `QuotaExceededError` and swap in an in-memory shim; SSR/hydration guards that no-op storage during pre-hydration render; session and analytics libraries that mirror, namespace or expire writes; a page's own test bundle | ✅ (tick 587) — the assignment was **accepted and discarded**. `localStorage` is a Proxy whose `set` trap stored non-method keys and fell through to a bare `return true` for method names, so every wrapper reported a successful install and never ran. Fixed: a method name shadows (as `Storage.prototype` does in a browser), anything else still writes to storage. Gated by `G_STORAGE_PATCHABLE`, RED-proven in **both** directions. |

**The failure shape is why it survived so long: silent success.** No error, no warning, and the original
behaviour continues — so the page works right up until the moment its fallback was supposed to engage,
which is the moment storage was already failing. A wrapper that cannot install is worse than no wrapper,
because the author has stopped checking.

**The generalisable half is about host objects.** `indexedDB.open`, `fetch` and `IntersectionObserver` all
wrapped fine; only storage did not. **Two host objects in one engine disagreeing about the same idiom** is
a class worth sweeping for — anything exposed through a Proxy or a native accessor should be asked whether
the web's standard monkey-patch works on it, because "can a page wrap this?" is a capability the platform
implicitly promises everywhere.

**And it was found by an instrument that needed the capability**, not by a conformance test: tick 586's
certificate probe tried to wrap storage to record touches and recorded nothing. Building the measuring
tool out of the same primitives the web uses is what made the engine's divergence visible.

---

## The line-one UA sniff — `navigator.plugins` / `navigator.mimeTypes` (tick 589)

| pattern | where it shows up | status |
| --- | --- | --- |
| **`navigator.plugins.length`**, `plugins.namedItem('PDF Viewer')`, `navigator.mimeTypes['application/pdf']`, and a `for` loop over `plugins` | UA-sniffing and capability-detection bundles — analytics, ad tags, anti-fraud, video players — which by nature run **before anything else on the page** | ✅ (tick 589) — both were `undefined`, so `navigator.plugins.length` threw a **TypeError in the first line of the first bundle**, taking the rest of that bundle down with it. Read on **32.5% / 12.5% of page loads**. Now the five PDF-viewer entries the HTML standard mandates, as a real legacy platform collection. Gated by `G_NAVIGATOR_PLUGINS`, RED-proven twice. |

**The argument for this was already written in this repo, next to a different property.** The comment beside
`navigator.vendor` says: *"it is one of the handful of things a UA-sniffing bundle reads on its first line…
a TypeError that takes the rest of the bundle with it — and sniffing code is, by nature, the code that runs
before anything else."* That reasoning was correct, and it was applied to **one** property. **A correct
argument in a comment does not generalise itself** — the next property with the same shape needs someone to
notice it has the same shape, which is what a usage-ranked map is for.

**Honesty, precisely bounded.** Since Chrome 93 the spec **hard-codes** this list to five fixed PDF-viewer
entries on every desktop browser, *specifically to stop it being a fingerprinting surface*. So the list is
not a report of what is installed — it is a **constant the standard requires**, and returning `undefined`
is the divergence. This is not "pretend to have Flash", and it is not the bot-evasion the project's scope
rules out: **whether we render a PDF is a separate question, and its answer stays no.** Keeping those two
apart is what makes the row honest.

**The gate's sharpest claim is `walk`.** A collection that supports `[0]` and `namedItem` but whose
`length` is wrong satisfies every other assertion — and enumeration is how the older sniffs actually read
it. RED-proven by making `length` lie.

## An HTTP error status is a DOCUMENT — the 403 bot-wall, the 404, the 429, the 500 (tick 607)

| pattern | where it shows up | status |
| --- | --- | --- |
| A top-level navigation whose response carries **`status >= 400` and a real HTML body** — the Cloudflare/Akamai `403` interstitial, a site's own branded `404`, a `429` rate-limit notice, a `500` stack trace | **A quarter of the head of the representative corpus.** Measured on the 20 HEAD sites of `corpus-v2.tsv` (tick 606's pilot): **5 of 20 answer `403` with a ~5.5KB challenge page** (tamildhool · mangago · supjav · fdown · quora). Also every framed consent/challenge screen: an OAuth `403` or a 3DS `404` inside an `<iframe>` | ✅ (tick 607) — both top-level navigation paths `bail!`d on `status >= 400`, so *the server answered* was reported to the user as *the network broke* and the tab went blank. The body now renders, as it does in every other browser; the status still rides on `Response::status` and is logged. Gated by **`G_ERROR_DOCUMENT`** (7 claims), RED-proven **per-path** — restoring either `bail!` alone fails a different claim. |

**The correct rule was already written in this repo, eight hundred lines from the wrong one.**
`page::prefetch_document_post` carries it verbatim: *"A 4xx/5xx still has a body worth showing (the
server's 'invalid password' page), so it is rendered rather than turned into an error — matching a real
browser, which shows the page."* So the **POST** navigation rendered error pages while the **GET**
navigation refused them: one question, two implementations, and the wrong one sitting on the path
virtually every navigation takes. That is this project's most-repeated defect shape — *two implementations
of one rule, and the live one goes stale* — booked here for the fourth time, and it is why §VI.3 of the
constitution check now asks, on every defect, **whether the rule is implemented more than once and whether
the copies agree.**

**Why this is a measurement bug as much as a rendering one.** Those five sites were not scoring badly in
the Phase-0 certificate; they were **unscoreable**, and not because we render them poorly but because we
*declined to look*. An instrument finds bugs in everything it must traverse to measure, not only in what
it measures.

**Honesty floor, and the gate asserts it.** *"An error status is a document"* must not decay into
*"nothing ever fails"*: a **refused connection** (bind a port, learn its number, drop the listener) must
still `Err`. Without that claim, an engine that never reported a network failure at all would satisfy
every "did it render?" assertion above. A dead origin, a DNS failure and a timeout are a different fact
and they keep their own answer.

## Pattern — the custom bullet: `::before { content: "–"; position: absolute; left: 0 }` (tick 775)

| pattern | where it shows up | status |
| --- | --- | --- |
| A list item, card or nav link sets `padding-left` and hangs a marker in an absolutely-positioned `::before` — a dash, a chevron, an icon glyph, a decorative bar. The generated content was materialised as an ordinary inline WORD with `position` never consulted, so the marker **took advance width**: it pushed the item's own text right by its own width and drew itself where the text should have started | **`255md.com`**, whose bullets are exactly this, and every site using the idiom — it is how the web has drawn custom list markers since `list-style: none` became normal, and the same shape carries pseudo icons, chevrons and decorative rules | ✅ **tick 775** — `InlineItem::AbsPseudo`: zero advance, zero inter-word space, zero line metrics, painted at `left − padding-left` from the pen. Gated by `an_absolutely_positioned_pseudo_leaves_the_flow_but_still_paints`, RED-proven two ways (restore the in-flow behaviour; and the OVER-BROAD fix of dropping the pseudo, which every positional claim still passes). ⚠ Partial and named: vertical insets are not honoured, and a `static` owner's positioned ancestor is not walked |

**Out of FLOW is not out of the PAGE** — the failure mode of the obvious fix is to delete the marker,
which trades a placement bug for a missing-content bug and passes every position assertion.

⚠ **The burndown could not see this, or tick 774.** Shape scores ELEMENT geometry, and both defects live
*inside* an element's box — the `<li>`'s rect is identical whether the marker is glued to the text or
20px to its left. Two consecutive real, Chrome-verified fixes on the cohort the ranking named, zero
metric movement: **when a metric RANKS work, its blind spot silently deprioritises a whole class of
visible defect.**

## Pattern — a stylesheet that contains ANY non-ASCII character: `content:`, `font-family` in its own script (tick 774)

| pattern | where it shows up | status |
| --- | --- | --- |
| A stylesheet carries a literal non-ASCII character — `content: "\u2013"` for a list bullet, `content: "\u2192"` for a chevron, a checkmark in a `::before`, or a **`font-family` written in its own script** (`"\u5fae\u8f6f\u96c5\u9ed1"`, `"\u30d2\u30e9\u30ae\u30ce\u89d2\u30b4"`, `"\ub9d1\uc740 \uace0\ub515"`). Every one of them reached the cascade as its raw UTF-8 bytes widened to Latin-1 code points | **Everywhere, silently.** `255md.com` drew `\u00e2` glued to each list bullet where Chrome draws an en dash. The expensive half is invisible: a mangled family name matches no font, so an entire CJK font stack falls through to a default with nothing logged — and the CrUX tail this corpus is stratified to reach is heavily CJK | ✅ **tick 774** — `strip_comments` walked the source as bytes and emitted `out.push(b[i] as char)`; it now copies the whole character. Gated by **`G_CSS_UTF8`** (11 claims in four scripts plus an astral emoji, RED-proven by restoring the original line) |

**This was one character of Rust, and it corrupted every stylesheet in the engine** — `Stylesheet::parse`
stores the stripped text as `source`, and `source` is what is handed to Stylo, so the cascade never saw a
correctly-decoded sheet. The DOM was correct throughout, which is why nothing pointed at it.

⚠ **It survived because the escape form was never affected.** `content: "\2013"` is pure ASCII and always
worked, and every CSS test in this repo was written in ASCII. **A test suite written in one alphabet
cannot see an encoding bug** — the fix is to leave the alphabet, not to add assertions.

⚠ **And the headline metric could not see it either**: fixing it moved zero shape points, because a
`::before` is not an element and the `<li>`'s rect is identical whichever glyph is drawn inside it.

## Pattern — a page that narrows `sheet.cssRules` by `instanceof`, and an inert stub that answers `false` about a rule that IS one (tick 773)

| pattern | where it shows up | status |
| --- | --- | --- |
| A CSS-in-JS runtime walks `document.styleSheets[i].cssRules` and narrows each entry by `instanceof CSSStyleRule` / `CSSMediaRule` / `CSSKeyframesRule` (or by `rule.type === CSSRule.MEDIA_RULE`). An **inert named stub** satisfies the `typeof` check and then answers **`false` about an object that IS a media rule** — so the runtime takes its "this browser has no media rules" branch with the rules sitting right there | **styled-components, Emotion, Lit, JSS** — the injection/rehydration path of all four. `CSSStyleRule` was an inert stub here, so `rule instanceof CSSStyleRule` was `false` for every real style rule; `CSSMediaRule` and the rest of the family did not exist at all (a `ReferenceError`) | ✅ **tick 773** — real `Symbol.hasInstance` predicates over what `__ruleOf`/`__makeSheet` actually build, plus the `CSSRule.*_RULE` numeric constants libraries read instead of hard-coding `4`. Gated by **`G_IFACE_SURFACE_2`** (47 claims, RED-proven three ways; the mutation that demotes `CSSMediaRule` back to an inert stub leaves `absent:none` passing and is caught only by `media:0`) |

**The inert-stub doctrine has a boundary, and this is it.** `x instanceof FileList` answering `false` is
*correct* — this engine never builds a `FileList`. The justification is the non-existence, not the
naming, so it **does not transfer to interfaces we do build**. A stub for something real is not a
placeholder; it is a wrong answer that no feature-detect can see.

The same re-measure found **59 of 262** platform globals absent (`MessageEvent`, the SVG shape elements,
the IndexedDB interface names every wrapper references at module scope, `TextMetrics`,
`HTMLOptionsCollection`, `FontFace`/`FontFaceSet`, the `navigator` sub-object interfaces) — five ticks
after the surface was last extended to "174 of 183". **A surface goes stale from the WEB's side, so its
denominator is a measurement, not a constant.**

⚠ **And 17 of the 59 were REFUSED.** `OffscreenCanvas`, `TrustedTypePolicyFactory`,
`XMLHttpRequestUpload`, `DeviceMotionEvent`, `ToggleEvent`, `FormDataEvent` and the rest each name a
capability this engine does not have, and `'DeviceMotionEvent' in window` is exactly how a page decides
whether to run a motion-permission flow. Naming them would defeat the feature detection that is a page's
only route around us — tick 772's half-installed-API trap with the sign flipped. The gate asserts those
absences (`overclaimed:none`).

## Pattern — an API family that is only HALF implemented: the feature-detect passes, the next call throws (tick 772)

| pattern | where it shows up | status |
| --- | --- | --- |
| A bundle feature-detects **one** method of an API family (`typeof performance.mark === 'function'`), commits to its instrumented path, and then calls a **sibling** the engine never implemented. The detect answered yes, so the author's fallback path is unreachable — and the throw happens where nobody wrapped it | **`www.trivago.de` (+ `.be`/`.fr`/`.jp`/`.pl` — one bundle, five corpus origins) and `coinmarketcap.com`.** `performance.mark`/`measure` existed as **no-ops**; `clearMarks` did not exist at all. `uncaught: performance.clearMarks is not a function` → trivago rendered **0 of 1410** elements, coinmarketcap **2 of 2116** | ✅ **tick 772** — full User Timing L3 (live entry buffer, `mark`/`measure`/`clearMarks`/`clearMeasures`/`getEntries*`/`toJSON`, real `PerformanceMark`/`PerformanceMeasure`, spec `SyntaxError`/`TypeError`/`InvalidAccessError`, legacy `PerformanceTiming` names resolving ahead of the buffer), gated by **`G_USER_TIMING`** (33 claims, RED-proven two ways). **`coinmarketcap.com` crossed `render-failed` → scored (shape 0.374), reversed by the control.** trivago's next rung is a failed dynamic `import()`; pogoda.by's is Zone.js `Promise` patching |

**An absent API is survivable; a half-present one is not** — and that inverts the usual intuition about
stubs. Absence *fails* the detect and routes the caller into the fallback its author wrote and tested.
Half-presence *passes* the detect, the caller commits, and it walks into a wall it had no way to see.
This is the same mechanism as the interface-object row below with the halves being sibling methods
instead of a bare identifier, and the same mechanism as `innerText` (tick 612) with the halves being
getter and setter. **The feature-detect surface and the call surface are different sets**, so the rule is:
implement the family, not the method a page happens to sniff.

The second half is subtler and applies to every inert stub in the prelude: `mark('a'); measure('m','a')`
could not have worked even with `clearMarks` present, because `mark()` discarded and `getEntriesByName`
returned `[]`. **The buffer is the feature; the function existing is not.** A no-op passes every `typeof`
check ever written — a wrong answer of the right type.

## Pattern — a page that PROBES the platform's interface objects and aborts when one is absent (tick 608)

| pattern | where it shows up | status |
| --- | --- | --- |
| A boot script reads a bare interface-object identifier (`HTMLMetaElement`, `Navigator`, `HTMLTableCellElement`, `CanvasRenderingContext2D`). If it is absent the read is a **`ReferenceError`**, which kills the frame — and a page whose loader is wrapped in a `try/catch` reads that throw as a **hostile environment** rather than a missing feature | **A top-1k HEAD site of `corpus-v2.tsv`.** `www.welt.de`'s loader probed `HTMLMetaElement`, took the throw, **concluded it was being ad-blocked, and aborted its own boot** — `0.0% coverage`, **3,242 of 3,243 elements never rendered**. A probe of the 183 interface objects a browser exposes found **63 absent** here | ⚠ partial (tick 608) — 54 added, **120 → 174 of 183**, gated by **`G_IFACE_SURFACE`** (37 claims, 9 of them negatives, RED-proven three ways). **welt.de still does not render**: removing this abort revealed the next one (`TypeError: setting getter-only property "innerText"`), caught by the same adblock handler |

**This failure class is not "a missing feature", and the difference is mechanical.** `el.foo?.()`
survives a missing *method*; **nothing survives a missing interface object**, because the throw happens
at the identifier read — before any operator the author could have guarded with. That is why the
absence does not degrade the page by a little: it removes the page.

**The instrument could see the size and never the cause.** `0.0% coverage` and *"we are slow"* are
indistinguishable in a box-diff, and they want opposite fixes — t606's pilot had in fact filed this
site under **timing** (*"the 12s load budget is exhausted, so pages paint incomplete"*). The page was
not painted incomplete; it was **never booted**, and the 31s was the cost of an aborted load rather
than a slow one. **When a coverage number is ~0 rather than merely low, read the console before
theorising: 0% is a different failure mode from 40%, not a worse one.**

**The rule, and its negative half is what keeps it honest.** An interface object is defined **iff the
thing it names exists in this engine** — each name added was probed present first. `OffscreenCanvas`
is **deliberately absent** (no offscreen tier), and the gate asserts that absence, so the list cannot
quietly become a claim instead of a fact. A stub that names a capability we lack defeats
feature-detection and is worse than the gap.

**Boot-path failures on real sites STACK.** One fix peels one layer — the same shape the aljazeera
investigation took, one named error per fix. **Do not book "site X works now" from "site X's first
error is gone."**

## The page asks for its subresources all at once, and the origin throttles per client (tick 609)

| pattern | where it shows up | status |
| --- | --- | --- |
| A page references N subresources on a handful of hosts. Every phase issues them in ONE unbounded `join_all`, so N images = N simultaneous sockets. The origin serves a few and **stalls** the rest; the stalled ones burn the per-request deadline, fail, and are entered in the per-navigation negative cache — so they are **never retried within that navigation** | **`mangago.me`, a HEAD site of `corpus-v2.tsv`.** 173 images issued at once, **26 arrived**. The page rendered **85% imageless**, and the same origin answers a single one of those images `200` in **0.52s** | ✅ fixed (tick 609) — a per-origin permit at the single choke point (`manuk_net::fetch_scoped`), default **6**, gated by **`G_CONN_CAP`** (RED-proven twice: removing the permit drops the capped arm 40 → 6; un-throttling the test origin fires the counterfactual assertion) |

**A stalled request is worse than a refused one, and that asymmetry is the mechanism.** A refused
connection fails fast and cheaply. A stalled one holds the socket, consumes the client's entire
deadline, and then presents to every layer above as *"this origin is down"* — indistinguishable, at
the call site, from a genuinely dead host. Our own concurrency is what produced the stall, so the
engine was manufacturing the evidence that the web was broken.

**The deadline was measuring the wrong interval.** A per-request clock exists to bound *the server's*
slowness; it was being spent on *our own backlog*. Any queue placed under a deadline must start that
deadline when the work reaches the wire, never when it joins the queue — otherwise the deadline
silently becomes a cap on throughput. Bounding the total belongs a level up, where the question
*"how long may this page take"* is actually asked.

**"Slow" and "missing content" were ONE bug, which is why this is not a trade.** The board carried
`mangago.me` as an OURS-IS-SLOW row and the certification pilot carried it as a low-coverage row;
both were the same stampede. The North Star's standing trap is *"fast because we never loaded the
images"* — here the engine was **both** slow **and** not loading the images, and the fix moves both
the same way (bbc.com's image phase: 22/22 in 2825ms → 22/22 in **1282ms**). **Doing less
concurrently got more content, sooner.** When a perf finding and a fidelity finding name the same
site, suspect they are one defect before pricing them separately.

**Pick the constant by measuring the knee, not by citing another browser.** 6 → 171 landed; 12 → 59;
24 → 26; 48 → 26 — identical to no cap, because 173 images over ~8 hosts is only ~22 per host to
start with. The cliff is real and steep, and "Chromium uses 6" arrived afterwards as a *check* on the
number rather than as its justification.

**One choke point, not one per phase.** Seven phases each had their own unbounded `join_all`; the
permit went into the single function all of them route through. §VI.3's fourth clause in reverse — do
not implement a rule seven times and hope the copies agree.

**⚠ AND THE HONEST HALF: this did NOT fix the site.** `mangago.me`'s end-to-end fidelity did not
improve, because its load is dominated by a different defect this tick did not touch — **~30s spent
draining the JS event loop to its 20,000-task ceiling, which has no wall-clock bound at all.** The
images now arrive; the page is still slow for another reason. Same discipline as t608: **do not book
"site X works now" from "one of site X's defects is gone."**

## A self-rescheduling timer is bounded by a task COUNT, and the count is not the harm (tick 610)

| pattern | where it shows up | status |
| --- | --- | --- |
| A page schedules work that reschedules itself (`setInterval(fn, 0)`, a self-reposting `requestAnimationFrame`, a poller that re-fetches on every delivery). The drain is capped by a TASK COUNT, so the cost of "not converging" is however long that page's 20,000 tasks happen to take — milliseconds for one page, half a minute for another | **`mangago.me`**: 20,000 tasks / **30,216ms per drain, FIVE drains in one load** — the largest single segment of an 85s load, bigger than every network phase combined. **`theguardian.com`**: did not finish inside **480s**. A converging page (`en.wikipedia.org`) drains in **1 task / 4ms** | ✅ fixed (tick 610) — a wall-clock budget alongside the count in BOTH drains, gated by **`G_DRAIN_BUDGET`** |

**The bound existed; it was measured in the wrong unit.** The ceiling's own comment says what it is
for — *"the alternative is a frozen tab"* — which is a wall-clock harm. It was enforced with a task
count, and the two are not related by any fixed factor: the same declared policy gave one page 4ms of
grace and another 30s, five times over. **When a limit's stated purpose and its unit disagree, the
unit wins silently, and the disagreement is invisible until a page arrives that separates them.**

**WHY THIS IS NOT "fast because we never ran the script", argued from what was already true.** The
North Star's standing trap says a speed win from *skipping work* is indistinguishable, on the clock,
from an optimisation. The answer here is not a fidelity number — it is that **these pages were
already being cut**, by the 20,000-task ceiling, just 30 seconds per drain later. Nothing that would
have completed is lost; only *when* the non-convergence is admitted changed. `mangago.me`, two runs
per arm on an idle box: **latency 115s (or never) → 36s, visual UNCHANGED at 0.2-0.9%.** The
unchanged score is not a disappointment, it is the evidence — same outcome, a third of the clock.

**⚠ AND A RETRACTION, kept because the tempting sentence has to be written down to stay caught.** A
first A/B showed `theguardian.com` at **22.9% → 52.4% visual** when bounded — *bounding script
execution RAISES fidelity*, a far better story, and the wiki page and journal entry were drafted
around it. **It does not reproduce.** Guardian does not reliably finish inside 480s in *either* arm,
so nothing can be differenced across them, and the original reading was taken beside a running
release build. t609 established that an A/B on a live origin must be run in **both orders**; this
tick adds that it must also be run **more than once, on a box doing nothing else**.

**⚠ THE GATE'S FIRST DRAFT WAS GREEN FOR THE WRONG REASON, AND ONLY THE RED PATCH FOUND IT.** It
asserted *"the runaway load finishes in under 20s"* — a reasonable-looking claim that **passed with
the clock bound deleted**, because a cheap self-rescheduling task trips the 20,000-task ceiling
quickly and the COUNT was doing the stopping. The gate had never touched the thing it was written to
defend. The fix was to stop asserting against a constant and compare ARMS: load the same runaway with
the budget off and then on, and compare how far each got. With the bound removed the two arms read
**80001 and 80001**. **A gate whose subject has a second mechanism that produces the same observable
must vary the mechanism, not the threshold.**

**One rule, two implementations, one enforced — again.** `run_deferred` was capped; `run_with_fetcher`
(which `run()` delegates to and `dom_bindings` drives) had **no bound of any kind**, so the exact
runaway Bar 0 forbids ran forever there. Its `did_io` arm was actively hostile: *"a delivered result
may have scheduled more work"* `continue`d past the task check unconditionally. Third consecutive
session in which the defect was a rule implemented more than once with the copies disagreeing.

## A site that detects a broken DOM write and BLANKS ITSELF (tick 612)

**The pattern:** a page probes the DOM for tampering, and on a *thrown* result concludes it is under
attack and refuses to render. `www.welt.de` writes through `innerText`, catches the exception, and
blanks the document:

```text
  ERROR page.console: Failed to load website due to adblock:
                      TypeError: setting getter-only property "innerText"
  structural: 0.0% (3182 paths, 3181 missing)
```

**Why this deserves a pattern entry rather than a bug entry.** The usual failure shape here is *we
render less than Chrome*. This one inverts: the page renders nothing **on purpose**, and does so
because our DOM genuinely was wrong. Debugging it from the outside — the screenshot is white, the
box diff says 3,181 elements missing — points at layout, cascade, or the fetch. All three are fine.
The evidence was one console line, and the engine had emitted it the first time it was asked.

**The generalisation, which is what to carry forward:**

> **An anti-tamper check is a CONSUMER of our error behaviour, not just of our features.** A missing
> setter, a getter that returns `undefined`, a method that throws the wrong exception type — these
> are not silent gaps to such a page. They are *positive evidence of an adversary*, and the site's
> response to that evidence can be worse than the missing feature ever was.

So the blast radius of a wrong-shaped failure is not bounded by what uses the API. Sites in this
class include ad-block detectors, bot detectors, paywall enforcers and fraud/BOT SDKs — and they are
disproportionately on exactly the news and commerce sites a daily driver has to open.

**Corollary for triage:** when a page renders BLANK while the fetch, parse and cascade all look
healthy, read the console before measuring anything. A blank page that Chromium fills from the same
bytes is more likely a page that *decided* not to render than a pipeline that failed to.

**And the layers peel one per fix.** Fixing the setter did not make welt.de render — its check simply
advanced to the next rung (`Error: Failed to execute packing script`), with `HTMLScriptElement.supports`
and an `addEventListener` on some object still missing behind it. Expect a chain, and do not claim the
site on the strength of clearing one link of it.

## AJAX set up the modern way: `xhr.addEventListener`, not `xhr.onload` (tick 613)

**The class:** every page whose data layer is `XMLHttpRequest` driven through **EventTarget** rather
than the `on*` handler properties. That is analytics beacons, ad SDKs, consent managers, older
jQuery/axios-era app code, upload widgets, and anything that needs more than one listener per event —
which is the entire reason the EventTarget form exists.

We had the **legacy half only**. `onload`, `onerror`, `onreadystatechange`, `onabort`, `onloadend`
worked; `addEventListener`, `removeEventListener` and `dispatchEvent` were `undefined` and
`xhr instanceof EventTarget` was false. Calling an undefined method is a **`TypeError` that kills the
calling frame**, so such a request was not merely unobserved — **it was never sent**, along with
whatever else that frame was doing.

**Measured across the 20 HEAD sites of `corpus-v2.tsv`** (HTML + up to 12 bundles each):

```text
  use `new XMLHttpRequest`               8 of 16 sites   (50%)
  addEventListener within 500ch of one   4 of 16 sites   (25%)
  XHR-specific listener event names      readystatechange 9 · progress 4 · loadend 3 · timeout 2
```

**Half the stratum uses XHR; a quarter attaches listeners to one.**

**The generalisation worth keeping:** a web API with a legacy form and a modern form is **two**
surfaces, and shipping only the legacy one is not "partial support" — it is a **TypeError** on the
form the ecosystem actually converged on. The legacy form working is what makes it invisible: the
capability probe says `XMLHttpRequest: function`, the constructor works, `open`/`send` work, and the
one method real code reaches for is missing. Check both forms of any dual-form API — `on*` vs
`addEventListener`, `callback` vs `Promise`, `attribute` vs `property`.

**And the corollary that bit here:** when the same event is dispatched from several hand-written call
sites, they drift. `loadend` was fired by the streaming delivery path and not by the buffered one, so
a spec event's delivery depended on **whether the response arrived in chunks**. One dispatch function,
or the copies will disagree — the fifth instance of that shape in five ticks.

## A subresource that 404s to an HTML error page (tick 616)

**The class:** every page with a stale, moved or mis-deployed asset URL — which is most of the web at
any moment. A CDN or SPA host answers a missing `/bundle.abc123.js` with its **HTML error page**, at
404 or sometimes at 200, and the browser must treat that body as *nothing* rather than as JavaScript.

We were injecting it into the `<script>` node as inline code. The resulting `SyntaxError` then kills
the frame that ran it — which is not a lost script, it is a lost *page*, since script execution order
means whatever ran next did not run. The same body reached the module compiler and the CSS parser.

**Why the class is bigger than "a broken asset":** it is the normal steady state of a deploy. A cache
holding an old HTML shell that references a hashed bundle the new deploy no longer has is the single
most common transient failure on a modern site, and every real browser rides through it by treating a
non-OK subresource as absent. A browser that instead *executes the error page* turns a missing file
into a broken document.

**The generalisation:** one HTTP response has as many correct interpretations as it has consumers.
The same 403 challenge page is a document to render (navigation), not evidence (the certificate), and
not code (a subresource). **Fixing the interpretation at one consumer says nothing about the others**,
and a comment promising that "the status rides along for every caller that cares" is not a mechanism —
it is a hope about callers that had not been audited.

## The bundled-SPA entry module under a hashed asset directory (tick 617)

**The class:** every site built by Vite, Rollup or esbuild for production. The output shape is
invariant across all of them — one entry module in a hashed asset directory, importing its code-split
chunks **relatively**:

```html
<script type="module" src="/assets/app/entry.a1b2c3.js"></script>
```
```js
import { x } from './chunks/vendor.d4e5f6.js';
```

We resolved that `./chunks/...` against the **document** rather than against the module, so every
chunk was requested one directory tree too high. And the wrong URL does not 404 cleanly on a real
site — an SPA host answers it with the **index HTML**, which then compiles as JavaScript and throws.

`www.welt.de` went **COVERAGE 0.0% → 94.9%** on this one fix, from a blank white page to a rendered
front page.

**Why this belongs in the ledger rather than the bug list:** the failure is invisible on the sites
most likely to be in a test corpus. A page whose modules sit next to the document — which is how
almost every hand-written example, tutorial and local dev server lays them out — resolves identically
either way. The bug only appears once a **bundler** has moved the entry into `/assets/`, which is to
say: only on real production sites, and on essentially *all* of them.

**The generalisation:** when a rule's two cases coincide in the common configuration, the code will be
written for whichever case the author had in front of them, and the comment will state the general
rule correctly while the code implements the special one. Look for the configuration that separates
the cases — here, "is the module in the same directory as the document?" — and test *that*.

## The code-split bundle — `import()` for a route, a component, a polyfill (tick 624)

**The class:** every application built with code splitting, which is the default in every modern
bundler. A lazy route (`const Page = lazy(() => import('./routes/settings.js'))`), an on-demand
component, a polyfill loaded only where needed, an analytics module deferred past first paint.

`import()` threw *"Dynamic module import is disabled or not supported in this context"* at **every**
call, because no `HostImportModuleDynamically` hook was installed. That is not a degraded experience —
the promise rejects, so the route never mounts, the component never appears, and the page's error
boundary (if it has one) shows a failure instead of content.

**The map claimed this `works` for as long as the row existed**, bundled into `ES modules + dynamic
import()` — one row asserting two capabilities and reporting the stronger one's verdict for both.
Surface audit #34 caught it; t624 built it.

**What makes it tractable here without an async loader:** the page already pre-fetches its whole
reachable module graph before any script runs, because there is no synchronous network on the JS
thread. Extending the pre-scan to literal `import("…")` specifiers puts the code-split chunks in that
same map, so the hook resolves from memory and finishes immediately. **A computed specifier
(`import(url)`) still rejects**, and that is the honest boundary: a page that builds specifiers at
runtime already has a `.catch()`, and telling it "no" is far better than a promise that never settles.

`www.welt.de`: VISUAL 82.8% → 91.1%, COVERAGE 94.9% → 95.7%.

## The WebM `<video>` — every non-MP4 video on the open web (tick 633)

**The class:** WebM is the container YouTube ships, and it is what most `<video>` that is not MP4
actually is — VP9 or AV1 video, Opus or Vorbis audio, in an EBML/Matroska wrapper. Every adaptive
player that streams it (dash.js, shaka, YouTube's own) drives its fetch loop by `SourceBuffer.buffered`.

`manuk_media::demux` answered `Unsupported(WebM)` for every EBML stream, and `isTypeSupported`
refused every WebM MIME type, so `addSourceBuffer` threw `NotSupportedError` and there was no door
into the demuxer at all. **The MP4 ladder went demux → AAC → H.264 → playback, one rung per tick;
WebM had no rung 1**, so a VP9 decoder would have had nothing to feed it — no tracks, no timestamps,
no byte ranges.

**What is now true, and the boundary is the point.** A page can open a WebM: two tracks with their
real codec strings (`vp9`, `opus`, and `av01.0.01M.08` derived from an `av1C`), real dimensions and
sample rate, one contiguous `buffered` span over the whole stream, and a `MediaSource.duration` from
the file's own `Info`. **Nothing decodes.** `isTypeSupported('video/webm; codecs="vp9"')` is still
`false` and `canPlayType('video/webm')` is still `''` — the second deliberately, because a `<video>`
listing a `.webm` `<source>` before its `.mp4` one must keep selecting the MP4 we can actually play.
Only the *bare* container form moved, and only because `addSourceBuffer` is the sole door to the
demuxer.

**The generalisation, and it is about gates rather than media:** the wrong answer this work could
produce was a sample offset **shifted by six bytes** — inside the buffer, disjoint from its
neighbours, and invisible to both structural checks a sample table admits. Containment and
disjointness are properties of the TABLE, not of the BYTES. It took a check against the *codec's*
framing to see it, and the RED probe — not review — is what revealed that the gate's first draft
could not fail on the bug its own doc claimed it caught.

## The AV1 `<video>` in a WebM — the half of that class that already worked (tick 634)

**The class:** the entry above says WebM carries "VP9 or AV1 video". The tick that wrote it then
concluded that no `codecs=` answer could move until a VP9 decoder existed — and read straight past
its own "or AV1". `re_rav1d` has decoded AV1 since t354, and a decoder does not care which container
the samples arrived in. AV1-in-WebM is what a modern Chrome is served on YouTube and on every site
that ships the newer rung of the same ladder.

**No decode code was written for this.** A probe fed t633's EBML sample table straight into the
existing `Av1Decoder`: **82 frames, 480×360, non-uniform, correctly timestamped.** The capability was
complete, reachable, and reporting absent — `isTypeSupported('video/webm; codecs="av01.…")` said
`false` and `canPlayType` said `''`. Both now answer truthfully; VP9, Opus, mixed lists and the
dotless `av01` form all stay `false`, and **bare** `video/webm` stays `''` because bare webm on the
open web is overwhelmingly VP9+Opus and `canPlayType` is what picks a `<source>`.

**The generalisation — a negative scoped one level too wide rots invisibly.** t633's refusal was
honest and it was stated about *the container* when the real constraint was about *the codec set*.
That sentence stays literally defensible ("there is no VP9 decoder") long after the conclusion it
supports ("so no WebM codec answer can move") has stopped following, which is precisely why nobody
re-reads it. **When writing an honest `no`, name the narrowest thing that is actually missing** — a
`no` about a missing decoder gets re-checked when a decoder lands; a `no` about a whole container
never does.

**The second-order finding: one rule, one implementation, made falsifiable.** Two files must answer
"does this WebM's codec list name something we decode". A shared helper is the fix — but a shared
helper that only one caller actually reaches looks identical to a shared helper. The RED probe is
what distinguishes them: mutating the single function moved **both** answers red together. That is
the check to run whenever this pattern is applied, because the failure mode of the fix is silent.

## The adaptive player's boot-time rendition scan — `mediaCapabilities.decodingInfo()` (tick 635)

**The class:** every modern adaptive player — shaka, dash.js, hls.js, YouTube's own — calls
`navigator.mediaCapabilities.decodingInfo()` on boot, **once per candidate rendition**, and filters
its variant list on the `supported` field. It is the modern replacement for `canPlayType`, and it is
how a player decides which quality ladder rung it is allowed to fetch.

`navigator.mediaCapabilities` was `undefined`, so the call **threw a TypeError** — and it threw
*inside the loop that enumerates renditions*, so the player never reached any of them. A missing
API here does not degrade quality selection; it removes the video. The RED probe shows the shape
exactly: deleting the install makes the gate's own record stop dead at the claim before it.

**The generalisation — the third asker is where a consolidated rule quietly un-consolidates.** Three
surfaces now answer "can this tree decode this contentType". The tick before this one had just
merged two of them after their answers drifted; adding the third with its own regex would have
restored the defect at full size, one tick after paying to remove it. **A rule that has been
consolidated is not safe — it is safe *until the next consumer*, and the next consumer always
arrives wearing a different API's name**, which is why it does not look like the thing you just
fixed.

**And the check that catches it is AGREEMENT, not answers.** Assert the two surfaces against each
other at runtime over inputs that are not all the same answer — never each against a constant you
wrote down. Giving the new surface its own plausible codec test turned the agreement claim red and
left every per-answer assertion green. Per-answer assertions cannot see a second implementation;
that is what a second implementation *is*.

## The bundler-free module graph — top-level await and cycles (tick 636)

**The class:** every `<script type="module">` graph shipped without a bundler — which is now most
modern app code in development, all of Vite's dev server, and a growing share of production. Two
things in it are load-bearing and easy to get subtly wrong: **multiple top-level awaits** (a module
that awaits before exporting, and the dependents that read those exports) and **cyclic module
records** (two modules importing each other, legal because bindings are live rather than snapshots).
Both are named Interop 2026 web-compat items precisely because real sites break when they resolve in
the wrong order.

Probed, and **both already worked**. The deliverable was the gate, not a fix — unmeasured-and-working
is one regression away from unmeasured-and-broken, with nothing to say so.

**The generalisation, and it is about probes rather than modules: RUN THE CONTROL.** The first probe
printed nothing and looked exactly like "top-level await is unsupported". The same graph with every
`await` removed printed nothing too — so the harness was wrong (an external module graph is
pre-fetched by `load_async`, never by the page fetch queue). **The control is the cheap form of
"name the code path that would deliver this absence": re-run the measurement with the feature under
test removed, and if it still fails, you were measuring the harness.** A negative result feels like
it needs no confirmation, which is exactly why absences get published at a price positives never
would.

**And the second-order rule: a capability assertion must be able to fail the FAKE version.** "The
module ran" is satisfied by an engine that ignores `await` at module scope entirely. Give two async
modules **different await counts** and the shorter one must finish first — the reverse of
declaration order. That inversion is what distinguishes real async-module semantics from modules run
in the order they were written, and it is the only claim in the gate that can tell them apart.

## The library that refuses to boot on a proxy check (tick 640)

**The class:** a third-party library's `isSupported()` / `isBrowserSupported()` predicate. Every
major player, framework and SDK has one, it runs before anything else, and a `false` from it means
the site shows its fallback — not a degraded experience, *no* experience.

**These predicates are rarely a list of what the library needs.** They are **proxies**, chosen years
ago, for a class of browser. shaka-player 4.11 refuses to run unless `window.MediaKeys`,
`navigator.requestMediaKeySystemAccess` and `MediaKeySystemAccess.prototype.getConfiguration` all
exist — **EME, which it does not need to play unencrypted content.** It uses EME's presence to mean
*"modern desktop browser"*. Measured, not inferred: hls.js and dash.js both boot here; shaka does
not, and every MSE predicate it checks is green.

> **The cost of omitting an interface is not bounded by the feature that interface names.** It cannot
> be reasoned about from the spec, because the spec does not know that a popular library uses your
> absence as a signal about something else entirely. It has to be measured against the code that
> reads it.

**The method that produced this, and it is cheap enough to be routine:** fetch the real minified
library, run its real boot path, and when it says no, **grep its own source for the predicate**. The
answer is right there in the bundle. Three libraries, 1.8MB, one run — after three ticks of
fixture-only evidence had said nothing about any of them.

**And the corollary for gates: assert deliberate absences too.** The EME triple is now asserted
*absent* in a gate, so the day it appears, the gate goes red and forces the question to be answered
rather than discovered. An absence that is a decision should be as load-bearing as a presence that is
a capability.

## The absent interface used as a proxy for browserhood (tick 641)

**The class:** an interface object a library reads not because it needs the feature, but as a
**shibboleth** for a class of browser. shaka-player demands the EME triple (`MediaKeys`,
`navigator.requestMediaKeySystemAccess`, `MediaKeySystemAccess.prototype.getConfiguration`) before it
will run **anything**, including unencrypted playback it does not need EME for. Omit them and a
library that would work perfectly refuses to start.

**The fix is the interfaces without the capability.** They exist, they are not constructible from
script, and `requestMediaKeySystemAccess` **never resolves** — `NotSupportedError` for Widevine,
PlayReady and Clear Key alike. That is exactly what Chrome without a CDM does. Measured on the real
660KB library: `isBrowserSupported()` false → **true**, `probeSupport()` completes, every key system
refused.

> **The honesty line is the RESOLVE, not the interface.** A missing interface is a lie about what
> kind of browser this is; a *resolved* access object would be a lie about what it can decrypt, and
> the second is far worse — it sends the site down a path that fails later and less legibly. Define
> the shape; refuse the grant.

**Beware the small concession.** Clear Key is the one that looks safe — no licence, "just AES", every
argument for granting it sounds reasonable. It still needs a decryptor. **A concession that requires
a capability you do not have is the same lie in a smaller font**, and it deserves its own RED probe
precisely because it is the one you will talk yourself into.

**And gate the deliberate absence.** Before this landed, a gate asserted those three interfaces
**absent on purpose**, so that the day any appeared it would go red and force the decision into the
open. It fired one tick later. An absence that is a decision should be as load-bearing as a presence
that is a capability — otherwise a permanent non-goal erodes not because anyone decided to, but
because nobody was told a decision was being made.

## The library that dies silently on a node-type guard (tick 642)

**The class:** a library that guards its own initialisation on a DOM identity value, and when the
value is wrong **fails inside its own evaluation** — so it never defines its global, and nothing is
reported. Not a degraded feature: no library at all, with a clean console.

jQuery 3.7.1's `setDocument` requires `9 === n.nodeType && n.documentElement` before it will
initialise its selector engine. `document.nodeType` was **8** (COMMENT_NODE), so `T` was never
assigned, the first selector call threw `can't access property "createElement", T is undefined`, and
`window.jQuery` was never defined. jQuery is still on a very large fraction of the web.

**The generalisation about how such a bug survives.** The `nodeType` getter was written for React
(`isValidContainer` checks `nodeType === ELEMENT_NODE`) and extended one arm at a time by whichever
framework complained next. **Its own comment already said that answering 8 for a fragment "is not a
near-miss, because every framework's node dispatch branches on this number"** — and the document had
that identical defect one `else if` away.

> **A property fixed by chasing the framework that noticed keeps exactly the holes no framework has
> noticed yet.** When the value comes from a small closed set — node types, ready states, event
> phases, visibility states, key-system names — **assert the whole set**, not the member that
> produced the bug report. The gate should be deliberately larger than the bug.

**The debugging chain, which is cheap and reusable when a bundle "does nothing":**

1. Rule out a module-shim leak — `typeof module/exports/define`. A stray CommonJS global sends every
   UMD bundle down the wrong branch and it defines no global at all.
2. `window.onerror` + an `error` listener. Silence here means *it aborted quietly*, not *it is fine*.
3. **Append a marker to the served bytes** (`;window.__tail=true`). A working library reaches it; one
   that aborts mid-evaluation does not. This is the step that turns "nothing happened" into "it died
   at some point inside".
4. **Wrap the served bundle in `try{…}catch`** and record the message. Page-level handlers may not
   see it; this always does.
5. **Grep the bundle for the variable the message names.** Minified or not, the guard is right there.

## The second document that was not a node (tick 643)

**The class:** every sanitizer, template engine, markdown renderer and HTML-diffing library parses
untrusted markup into a **detached document** so nothing in it can run, fetch, or touch the page.
`DOMParser.parseFromString` and `document.implementation.createHTMLDocument` are the two doors, and
code reaches the result through node APIs — `ownerDocument`, `createNodeIterator`, `importNode`,
`adoptNode`.

A parsed document that is an **object literal wearing `nodeType: 9`** answers the handful of
questions someone happened to need (`documentElement`, `body`, `querySelector`) and fails everything
that treats it as a node. Combined with an `ownerDocument` that returns the *page's* document for
every node, DOMPurify's walk — `createNodeIterator.call(root.ownerDocument || root, root, …)` —
iterated a tree its root was not in and returned **the empty string** for any input containing a
tag.

> **A duck-typed object passes exactly the checks its author thought of.** The failure is not that
> it is incomplete — it is that the incompleteness is invisible from the call site, because
> `nodeType === 9` is what a caller checks and it is precisely what the duck gets right.

**The diagnostic that breaks this open: walk the structure that would make the claim true.** The
object said `nodeType: 9`; printing the **parent chain** from a parsed child gave `BODY > HTML` with
an *element* at the root, and no document anywhere. One line of probe, and the whole class is
visible.

**And the design rule.** When a real mechanism for something already exists in the tree, a second
"lightweight" one beside it is not a shortcut — it is a divergence that will be found by a library,
not by a test. Route the shim through the real thing and delete it.

## The top-level ReferenceError that kills a library (tick 644)

**The class:** a library that touches a platform global **at module top level**, outside any
feature-detect. htmx 2.0.4 builds an `XPathEvaluator` expression as a module constant. One missing
global, and the `ReferenceError` propagates out of the bundle's own evaluation — so it never defines
`window.htmx`, and nothing on the page reports why. Not a degraded feature: no library.

This is the same silent-total-failure shape as jQuery's node-type guard, reached by a different
route, and it is why *"does the bundle define its global?"* is the cheapest single question to ask of
any library.

**The design rule this produced, which generalises past XPath:**

> **"Define the interface, refuse the capability" is honest only where refusal is a valid answer.**
> It works for EME — *"no key system is supported"* is something a player can act on. It does **not**
> work for an API whose contract is to *return the right data*: a stub XPath evaluator returning an
> empty node-set makes the library boot and then wire up nothing, which is strictly worse than the
> ReferenceError it replaced, because the failure moves from loud to silent.
>
> For those APIs the honest partial implementation is **correct over a documented subset, and an
> explicit error outside it.**

**And gate the refusals as hard as the results.** A partial implementation's refusals are what make
its answers trustworthy — without them, "it returned some nodes" is not evidence. Assert **counts and
identities**, never non-emptiness: the two failure modes are *returns everything* and *returns
nothing*, and only an exact expectation catches both.

## An injected `<script src>` must report that it landed — the loader waits on the event, not the code (tick 652)

| pattern | where it shows up | status |
| --- | --- | --- |
| A page creates a `<script>`, sets `src`, attaches a completion handler **as a property** (`script.onerror = script.onload = fn`), appends it, and **waits to be told the script arrived** before doing anything else | **Every chunked bundler build**, which is most of the modern web: webpack's `__webpack_require__.l` is exactly `script.onerror = script.onload = onScriptComplete` plus a timer, and rejects with `ChunkLoadError` if neither event comes. Far wider than bundlers — every `loadScript()` helper resolves its promise on this event: analytics and tag managers, ad tags, reCAPTCHA, payment and map SDKs. Measured on `www.agoda.com` (top-100k HEAD) | ⚠ partial (tick 652) — the script was fetched and executed **correctly**, and **no `load` or `error` was ever fired**, so no loader could learn its script had arrived. There was no symptom: no error, no log, no failing gate, because the script itself ran flawlessly. Two defects: the dynamic-script runner dispatched nothing, **and** `__dispatchEvent` walked only `__listeners` and never invoked the `on<type>` **property** — so `addEventListener` worked and the property form did not, exactly backwards from what real loaders use. Both fixed; `load` fires even when the script threw. Gated by **`G_SCRIPT_LOAD_EVENT`**, RED-proven **once per defect**. **`agoda` still paints blank**: injected scripts are fetched in a later `finish_loading` phase, *after* the page's own event loop has spun to its 20,000-task ceiling and webpack's timeout has fired — a real browser fetches them **concurrently with** the event loop. That scheduling defect is named, not fixed |

## The site whose CSS is in a `<link>` and whose fonts are slow (tick 654)

| pattern | where it shows up | status |
| --- | --- | --- |
| A page delivers its design in external `<link rel="stylesheet">` sheets, and one of those sheets declares `@font-face` with a **ladder of `src` alternatives** (`woff2`, `woff`, `ttf`, `eot`, `svg` — what every icon-font build ships). The sheets arrive fast; the font ladder is fetched **sequentially, per source, per face**, and can take longer than the whole load budget | **`keirin.jp`** (HEAD site of `corpus-v2.tsv`): nine sheets / 375KB all logged `stylesheet applied` at **+0.2s**, then **~11.5s** inside font-awesome's per-face `src` ladder, then `load budget of 12.0s exhausted mid-phase`. The same shape belongs to every site pairing external CSS with an icon font on a slow or blocked CDN | ✅ fixed (tick 654) — the stylesheet phase's apply sat **below** the `@import` walk and the `@font-face` fetches, so the hard deadline dropped the future two stages above the cascade and **discarded nine fully-downloaded sheets**. The page rendered NAKED: every box a full-width UA block in `serif/16`, the document 3× too tall, at **coverage 98% / SHAPE 2.1%** — we rendered every element Chromium did and placed almost none of them. Now the top-level sheets are committed where they are complete, before the phase returns to the network; imports and fonts are the enhancements the budget may drop. **SHAPE 2.1% → 40.7%.** Gated by **`G_CSS_SURVIVES_BUDGET`** (a local socket that serves the sheet instantly and stalls the font forever, so the deadline fires exactly where it used to lose the sheet; RED 275px → 784px) |

## The interactive page whose CSS is in a `<link>` (tick 654)

| pattern | where it shows up | status |
| --- | --- | --- |
| A page whose stylesheets are external does **anything** after load: a `fetch`/XHR resolves, the user clicks, a WebSocket frame arrives, a streamed body delivers a chunk, `postMessage` fires, the back button pops state | **Essentially every page on the web.** External CSS is the default delivery mechanism, and post-load activity is what an application *is* | ✅ fixed (tick 654) — eight re-cascade sites rebuilt their sheet list from `MinimalCascade::collect_style_elements`, which sees inline `<style>` and **not `<link>`ed sheets**, so each one silently deleted every external stylesheet and re-styled the document against UA defaults. Two *other* sites of the same rule were already correct, each with its own hand-rolled copy of the body — which is exactly why the wrong eight survived. All nine now call `Page::all_sheets()`. Gated by **`G_EXTERNAL_CSS_SURVIVES_RESTYLE`**, which asserts **two independent triggers** (a resolved `fetch` and a click) because a rule with eight implementations is not proven by the one a gate happens to touch. ⚠ **The ninth site is named, not fixed:** `forced_reflow` (the synchronous layout a JS `offsetWidth` read forces mid-script) has the same defect and cannot use the helper — it runs off a `*mut ReflowCtx` installed at 19 call sites with no route to `external_css` |

## The page whose subresources come from more than one host (tick 655)

| pattern | where it shows up | status |
| --- | --- | --- |
| A page's images, stylesheets or icon masks come from **several hosts**, and one of them accepts the connection and then does not answer — a third-party asset host, a tracking pixel, a CDN edge that is having a bad minute. The other hosts answered in milliseconds | **Essentially every page on the open web.** Mixing a first-party origin with a CDN and a third-party asset host is the default architecture, not an edge case; a news front page routinely names four. `keirin.jp` is the measured case (coverage 98.0% → 74.4% once its stylesheets landed — the drop is picture-shaped holes), and observer t602 reports the general symptom from the other side: 10 of 14 HEAD sites trip OURS-IS-SLOW and *"the 12s load budget is exhausted so pages paint incomplete and shape is PARTLY a TIMING result"* | ✅ fixed (tick 655) — every subresource phase fanned out with `futures_util::future::join_all`, which is **ONE future** yielding **ONE vector** when the **last** member settles. Under the load budget's hard `timeout` that made each phase all-or-nothing: the single stalled host discarded every response that had already arrived, decoded, in memory. Measured on a hermetic 3-image page whose third image stalls: **0 of 2 images on the page, and 4 responses on the wire for 2 images** — the cancellation lost the *"we already asked"* record along with the bytes, so the next budgeted pass re-fetched everything (a `G_DEDUP`-class storm caused by cancellation, not by dedup logic). Now `collect_before_deadline` drives a `FuturesUnordered` item by item, banking each answer **as it lands**, so the deadline ends the *waiting* and not the *results*: **2 of 2 images, 2 responses.** Applied to images, external `<link>` sheets, `@import` rounds, background images and masks; **`pump_page_fetches` and the script fan-out are deliberately excluded** (a partial set has ordering semantics this primitive does not answer). The budget itself is unchanged — the 400ms `COMMIT_RESERVE` that lets the phase apply what it has is taken **out** of the budget, never added to it. Gated by **`G_IMAGES_SURVIVE_BUDGET`**, RED-proven at `(None, None, None)` with 4 wire responses |

## The page whose pictures are the content (tick 656)

| pattern | where it shows up | status |
| --- | --- | --- |
| A page shows images at their **own** size — no `width`/`height` attribute, no CSS box: a photo essay, a product grid, an article's inline figures, a logo, an avatar, an icon `<img>`. Then *anything* re-cascades the document: an external stylesheet lands, a `fetch` resolves, a script runs, the user clicks, a JS geometry read forces a reflow | **Every page on the web that shows a picture and then does something.** The unsized `<img>` is the default form — sizing an image in CSS is the exception, not the rule — and a re-cascade after first paint is what an application *is* | ✅ fixed (tick 656) — a replaced element's **intrinsic size is the one geometry input that is in no stylesheet**: it arrives from the network long after the cascade that must lay it out, so it was written into the cascade's *output* and every later cascade rebuilt that map from the stylesheets and erased it. Measured on a page with **no CSS at all** and one 41×23 image: `width=Px(41) ar=Some(1.78)` → **`width=Auto ar=None`**, box `41×23` → **`784×0`**. Full content width, **zero height** — the picture occupies no space and everything below slides up into it. Permanent, not transient: the image phase dedups per `(node, url)`, so the second pass has nothing to re-apply and whichever cascade ran last was final. **Invisible to coverage** — every one of those elements is present, probed and counted; they are all hairlines. Now restated between cascade and layout at the one shared join (`restyle_and_layout`) plus the four direct-cascade sites, **and in `forced_reflow`** — the ninth re-cascade site t654 had to name and leave, reached by giving `ReflowCtx` the image map to *own* rather than a pointer to chase. Gated by **`G_IMAGE_NATURAL_SIZE_SURVIVES_RESTYLE`** (RED at `784×0`; asserts the image's size, that the following paragraph is pushed **below** it, and that both survive a *second* trigger) |

## The origin that answers with a large header block (tick 658)

| pattern | where it shows up | status |
| --- | --- | --- |
| An origin answers over HTTP/2 with a **response header block larger than 16 KiB** — a long `Set-Cookie` ladder, a fat `Content-Security-Policy`, a `Link:` preload list, the ordinary output of a session-heavy portal or a games/social platform | **`playhop.com`** (HEAD site of `corpus-v2.tsv`), and the whole class of cookie-heavy and CSP-heavy origins behind it. Not exotic: 16 KiB is small for a modern response, which is why Chrome announces sixteen times it | ✅ fixed (tick 658) — `h2`'s default `SETTINGS_MAX_HEADER_LIST_SIZE` is 16 KiB and an oversize response is **not** truncated or downgraded: the client sends `RST_STREAM(PROTOCOL_ERROR)` and the navigation fails outright. The page does not load slowly; it does not load, and it is **indistinguishable from a dead host** — the certificate sweep booked it `unreachable` and the instrument said out loud that meant *"a corpus or network problem, not a rendering one"*, while `curl` fetched the same URL in 2.5s and 978 KB. The trace named it: `REQUEST_HEADER_FIELDS_TOO_LARGE … send_reset(PROTOCOL_ERROR, **initiator=Library**)`. Now announces Chrome's **256 KiB** (`manuk_net::HTTP2_MAX_HEADER_LIST_SIZE`) — on capability Chromium is the ceiling to MATCH, so the number is not ours to tune. `playhop.com` went `unreachable` → **scored**. Gated by **`G_H2_LARGE_RESPONSE_HEADERS`** (a real h2 exchange with a ~40 KiB header block; its 16 KiB control runs FIRST and must fail, and a third assertion pins the constant so the gate cannot shrink with the bug) |

## The minified production bundle that throws (tick 662)

| pattern | where it shows up | status |
| --- | --- | --- |
| A page's own bundle throws an uncaught error, and the bundle is **minified** — so the message names a one-letter variable and identifies nothing. The page is blank or half-built and the report gives nothing to pull on | **Every production site on the web**, since every production site ships minified JavaScript. `www.agoda.com` (top-100k HEAD) had thrown `TypeError: can't access property "length", t is undefined` on every run for ten ticks and it was untriageable | ✅ fixed (tick 662) — `pending_exception` stringified the thrown value and stopped, discarding the `fileName` / `lineNumber` / `columnNumber` / `stack` **SpiderMonkey had already attached to the object it was handed**. `G_SILENT_FAIL` forbids swallowing an error; this is the step after it, and the gap is where several ticks went: **an error that is REPORTED but not ATTRIBUTABLE is a status, not a finding**. The report now carries the location and the stack, and a thrown non-object (`throw "x"`, `throw 42`) degrades to exactly the old string rather than to a lie about its origin. **It paid out on the first real site in one run**: agoda's anonymous throw resolved to `insertRules` → `getTag` → `this.sheet` — the CSS-in-JS runtime injection path styled-components and emotion share, reading `.sheet` off its own `<style>` element and getting `undefined`. The blank page is the **CSSOM `.sheet` bridge**, an already-scoped lever, now with a real site and a real stack behind it instead of a WPT count. Gated by **`G_SCRIPT_ERROR_HAS_A_LOCATION`** (asserts on the rendered `tracing` line a developer actually reads, and on a NAMED STACK FRAME — which only `Error.stack` can produce, so the gate cannot be satisfied by formatting a guess) |

## The app styled by its own JavaScript (tick 665)

| pattern | where it shows up | status |
| --- | --- | --- |
| An app generates its CSS **at runtime**: it creates a `<style>`, takes `.sheet`, and calls `insertRule` once per generated class — then `deleteRule` on unmount. Nothing about its styling is in the document the server sent | **styled-components, emotion, JSS, goober and every `<style>`-injecting runtime** — a large fraction of the React/Vue app web gets *all* of its styling this way. Measured on `www.agoda.com` (top-100k HEAD) | ✅ fixed (tick 665) — `styleEl.sheet` was **`undefined`**, and `undefined` is not the spec's absent value: `HTMLStyleElement.sheet` is `CSSStyleSheet?`, so the guard every consumer writes (`if (el.sheet === null)`) is FALSE against it and the code proceeds into the thing it just checked for. Meanwhile **`typeof CSSStyleSheet === "function"` was already true** — false presence, so every feature detect passed. The deferral was priced wrong: t283's shim was reverted for wanting a *native accessor to reach the cascade*, and a probe settled it in one line — `el.textContent = '#a{width:222px}'` moves the box 111→222, because a `<style>`'s own TEXT **is** the cascade's source of truth. So the CSSOM is a **view over the element's text**, not the parallel data model that made it a subsystem. `cssRules` (brace-DEPTH split, so `@media` is ONE rule), `insertRule`/`deleteRule` that reach the cascade, `addRule`/`removeRule` legacy aliases, live `document.styleSheets`, `el.sheet === el.sheet`, `IndexSizeError` past the end. **Scope stated:** `<style>` only; `<link>.sheet` stays `undefined` rather than a `null` that would be a lie about an applied sheet. Gated by **`G_CSSOM_SHEET_BRIDGE`**, whose central assertion is a **box width** (456px from a runtime-injected rule), plus `deleteRule` un-cascading and the authored sheet untouched. ⚠ **agoda is NOT fixed**: the throw is gone and the row moved `render-failed` → `thin-overlap-5`, which the instrument still labels ours |

## The page that spins and injects (tick 667)

| pattern | where it shows up | status |
| --- | --- | --- |
| A page runs a self-rescheduling timer (a carousel, a clock, a poller, a chunk loader retrying) **and** injects further `<script src>` at runtime — so each dynamic-script round finds new work, and each round's event-loop drain runs to its own ceiling | **`www.agoda.com`** and the chunked-bundler class generally: a loader that has not been told its chunk arrived polls, and its retry injects more script. Measured at the page level, three runs within a second: `finish_loading` **39.9s against its own 12s budget**, 17 give-ups at ~2.3s each | ✅ fixed (tick 667) — the hang guard's two bounds (`MAX_TASKS_PER_DRAIN` and its clock twin) both bound **a drain**, while the promise beside them is about a **page**. The outer `tokio::time::timeout` could never enforce the budget here: **a timeout fires at an await point and these drains are synchronous JavaScript.** The bound is now a decision made BETWEEN rounds — the round loop stops once a round reports the page did not converge, since a page that burned its ceiling without converging will not converge in the next one. Hermetic baseline vs fixed: **9 give-ups / 4 chained scripts → 3 / 1.** ⚠ Residual named, not hidden: three give-ups remain (document scripts, deferred pass, one dynamic round) — the navigation's FIXED drain sites, which are legitimate first executions; the gated property is that the cost **does not scale with the page**. Gated by **`G_DRAIN_BOUNDS_THE_PAGE`**, whose fixture must BOTH spin and inject — a spin-only fixture never enters the round loop, which is why tick 660's version of this claim could not fail and was retracted at t661 |

## The data-driven page whose fetch continuations never settle down (tick 671)

| pattern | where it shows up | status |
| --- | --- | --- |
| A page issues many `fetch`/XHR requests, and each one's `.then` starts work that does not converge — a poller, a retry timer, an animation kicked off by data arriving. The browser settles them one after another, running the page's JavaScript each time | **Every data-driven SPA**, which is what the app web is. `www.agoda.com` (top-100k HEAD) is the measured case | ✅ fixed (tick 671) — `pump_page_fetches` checked the load budget before and after each round and **had no clock at all in the loop that settles the round's results** — the part that actually spends it, because settling one result runs the page's promise continuation and drains the event loop. A non-converging page paid a full drain ceiling **per settled request**, up to 40 per round, invisible to the per-round check. Measured against a 12s budget: `ms=36061 gave_up=15` → `ms=13190 gave_up=5`, and the whole load **43.6s → 19.0s — 2.3× faster on a real HEAD site**. It costs nothing the budget was not already discarding: past the deadline the image, mask and background phases are skipped outright. Gated by **`G_SETTLE_RESPECTS_THE_BUDGET`** (thirty settles that each spin; asserts WALL CLOCK, RED-proven 1.90s → 7.99s). ⚠ The gate's first fixture could not fail — twelve settles gave 1.9s vs 3.4s, which no non-flaky ceiling straddles; the FIXTURE was hardened, not the threshold |

| pattern | where it shows up | status |
| --- | --- | --- |
| Server state is shipped as an inert **data island** — `<script id="__appData__" type="mime/invalid">{…}</script>` — and read back through **named access on the Window object**: `window.__appData__.innerHTML` → `JSON.parse`. Also bare `<id>` references, and the `name=` of a form/img/embed/object/iframe | Older and server-rendered pages of every stack; needs no framework and predates the word *hydration*. `playhop.com` (top-100k HEAD) is the measured case: its whole application subtree — **102 of 107 elements Chrome builds** — hung off two `window.__appData__ is undefined` TypeErrors | ✅ fixed (tick 677) — HTML §7.3.3 was absent entirely: `window.<id>` was `undefined` for every element and `'x' in window` was `false`. `__publishNamed()` defines a live accessor per `id`/`name` at each script entry seam, incrementally. **A real `Window` property WINS** (`<div id="location">` must not shadow `window.location` — the spec puts named properties in Window's PROTOTYPE chain), the getter **re-resolves by id at access time** (a cached node is a use-after-remove), and the properties are **non-enumerable** (a real browser lists no page ids in `Object.keys(window)`). Gated by **`G_GLOBALS`** (9 claims incl. the island's `innerHTML`→`JSON.parse` round-trip); RED-proven twice — dropping the publish, and dropping the real-property-wins guard. ⚠ It bought a **better failure, not a scored row**: playhop's two throws are gone and its coverage is unmoved at 4.7%, with the blocker moved to *the app does not converge inside the task ceiling* (23–32s loads against a 12s budget). ⚠ RESIDUE: a name is reachable at the next script ENTRY, not the instant the element is inserted |

| pattern | where it shows up | status |
| --- | --- | --- |
| A page arms a **far-future timer** — `setTimeout(fn, 86400000)` for a midnight rollover, a daily reset, a cache expiry — and otherwise converges immediately | Utterly ordinary; needs no framework. `playhop.com` (top-100k HEAD) is the measured case: ONE such timer, and the browser tripped its own Bar 0 hang guard **six times** on a page that had already finished | ✅ fixed (tick 680) — `__fireLoad` set `__timeBudget = Infinity`, so the virtual clock could jump forward without bound. The loop advanced **24 virtual hours per iteration** and ran that one timer 20,000 times — 54 virtual YEARS per drain, ~438 across the navigation, ~1.5–2s of real CPU each — and the guard's message **blamed the page** (*"a self-rescheduling timer, most likely"*). The page was innocent. Now a **horizon**: `__timeBudget = __now + 60000`, which is what Chrome's `--virtual-time-budget` already is. The number is decided by `testharness.js`'s 10s harness timeout (a clock that cannot reach it makes every async WPT file report TIMEOUT — the catastrophe `Infinity` was introduced to fix); 60s clears that by 6× and is 1,440× short of a day. Measured: **give-ups 6 → 0**, load 31.6s → 27.3s, `load event` 1330ms → 154ms, coverage 4.7% → 6.5%. Gated by **`G_LIFECYCLE`** (a 24h timer must not fire; its own report armed at 5000ms asserts the other side) and **`G_RUNAWAY`** (the give-up must NAME the spinner, from the page's callback and not our wrapper). ⚠ The first gate version was VACUOUS — it read a flag from inside the 5000ms report, and `(due, seq)` ordering means the report always precedes a 24h timer, so it passed with `Infinity` restored; the observation has to happen after the drain |

| pattern | where it shows up | status |
| --- | --- | --- |
| The user **reloads** a page that half-loaded — a CDN was down, a tracker timed out, the connection dropped mid-load | Every browser session. It is the single most common recovery action a person takes, and the whole reason a reload button exists | ✅ fixed (tick 683) — `manuk_net::FAILED`, the per-navigation negative cache, was **never cleared on the navigation path**: `reset_fetch_stats()` had two callers, one gate and one unrelated `manuk-wpt` subcommand. So it was per-PROCESS, and **every load after the first inherited the previous load's failures** — a reload of a half-loaded page returned the same half-load for the life of the process, silently, because from the fetch layer's side that is dedup working. `begin_navigation()` clears `FAILED`/`NETWORKED`/`SEEN`/`INFLIGHT` at the top of `Page::load_async` — **not** `Page::load`, because `render_iframe` calls that for a SUBFRAME and resetting there would clear the parent's state mid-navigation; and separate from `reset_fetch_stats` because the COUNTERS are what `G_DEDUP` reads. The POSITIVE HTTP cache is untouched: only the record of failure is per-navigation. Gated by **`G_DEDUP`**, which now asserts BOTH halves — a second navigation must reach the network again (**0 requests without the fix**) and must still make zero duplicates within itself, since a retry bought by turning dedup off trades the nytimes bug back in. ⚠ It did NOT fix `www.agoda.com`'s bimodal render (`external scripts` 1072ms → 9ms persists), so that has another cause |

| pattern | where it shows up | status |
| --- | --- | --- |
| An `<img src>` whose bytes never arrive — a dead CDN, a blocked tracker, a lazy-load placeholder, an icon behind a 403 | Every real page has several. `Cc4e6 geometry: <img>` is a **67-site** cluster, and `keirin.jp`'s FIRST DIVERGENCE begins immediately after an `<img>`, off by `dy=70` | ✅ fixed (tick 689) — measured over headless Chrome on the same fixture rather than recalled: a bare broken `<img>` is **16×16** in Chrome (the placeholder it reserves) and was **784×0** here. Wrong twice: an INLINE replaced element must not take the whole line, and a box whose source broke is not zero-height — so every sibling below it slid up. The layout comment even claimed `<img>` was excluded because *"a sourceless image has no default object size in any browser"*, true of `<img>` with NO `src` and not of the case the web is full of. Conditioned on `taffy_known.is_none()`, so a decoded image, author dimensions, or a derivable ratio are all untouched. Gated by **`a_broken_img_reserves_chromes_16x16_placeholder_and_pushes_its_sibling_down`**, which asserts the FOLLOWING sibling's `y` and not just the image's box — a height that is right in isolation and does not displace its siblings would satisfy a box-only assertion and fix nothing about the `dy` term. RED-proven by two independent mutations (drop the width, drop the height). ⚠ NOT covered and named: an `<img alt="text">` whose source failed — Chrome sizes that box to the ALT TEXT, which needs the text measurer and is its own change |

| pattern | where it shows up | status |
| --- | --- | --- |
| A **baseline-aligned inline `<img>`** — an icon in a button, a logo in a nav, an avatar beside a name, a spacer gif. The default `vertical-align` makes its bottom sit ON the text baseline, so the line box must also reserve the strut's DESCENT below it | Every page. `C01ca geometry: <div>` (111 sites) and `C7eb9 geometry: <body>` (93 sites) are the clusters it feeds, and tick 688 measured that the SHAPE gap is a pure `dy` term — correctly-sized boxes displaced downward because something above them is too short | ❌ **OPEN, measured and localised (tick 690) — recorded here rather than left unmapped.** Chrome vs ours on `margin:0; font:16px/normal sans-serif` with a 40×40 `<img>`: `div>img` **h=44 vs h=40** (the 4px strut descent), while `vertical-align:top` and `display:block` **already agree at 40** — so the atomic is PLACED correctly and the LINE is not opened far enough. Root cause: CSS 2.1 §10.8's **strut is absent** — `close_line` folds ascent/descent/line-height over the FRAGMENTS PRESENT, and an atomic or synthetic `LineFrag` carries `ascent == descent == 0` by construction, so a line whose only content is an image has ZERO descent. ⚠ The obvious fix (`atomic_h + descent`) was TRIED and changes nothing, because `descent` is 0 on exactly the lines that need it. The real fix seeds the strut from the CONTAINING BLOCK's font, which `close_line` does not receive — a signature change to the function that computes every line box, so it is its own tick with `w1: 40 → 44` as the bar and `top`/`block` staying at 40 as the over-correction guards. **What it will unlock when it lands: every page whose vertical rhythm is set by inline icons — each one currently shifts everything below it by the descent (32px over four images on one fixture).** |

| pattern | where it shows up | status |
| --- | --- | --- |
| A **baseline-aligned inline `<img>`** — an icon in a button, a logo in a nav, an avatar beside a name — whose line box must reserve the strut's DESCENT below the baseline the image rests on | Every page. It feeds `C01ca geometry: <div>` (111 sites) and `C7eb9 geometry: <body>` (93 sites), and tick 688 measured the SHAPE gap as a pure `dy` term: correctly-sized boxes displaced downward because something above them is too short | ✅ fixed (tick 691) — CSS 2.1 §10.8's **strut was absent**: `close_line` folded ascent/descent/line-height over the FRAGMENTS PRESENT, and an atomic or synthetic `LineFrag` carries `ascent == descent == 0` by construction, so a line whose only content was an `<img>` had ZERO descent. Now every line box starts with the CONTAINING BLOCK's font metrics (resolved through `text_style`, the same resolution the fragments use), and a baseline-aligned atomic demands `height + descent`. Measured `margin:0; font:16px/normal sans-serif`, 40×40 `<img>`: **h=40 → 43** against Chrome's 44 — the 1px residual is a font-descent difference between our `sans-serif` and the reference Chrome's, and it is inside the 8px SHAPE tolerance where 4px per image was not. ⚠⚠ **TWO CHANGES, ONE BEHAVIOUR:** the strut supplies a non-zero descent and `tallest_atomic + descent` spends it — tick 690 tried the second half alone, measured no change, and reverted it, correctly on the evidence then. Gated by **`a_line_box_starts_with_a_strut_so_a_baseline_atomic_reserves_its_descent`**, RED-proven by zeroing either half, with THREE over-correction guards that already agreed with Chrome (`vertical-align:top` 40, `display:block` 40, a plain text line 18) so a fix that opened every line box would fail. **`parity` 72/72 across 30 pages holds** — the wider net on a change to the function that computes every line box in the engine |

| pattern | where it shows up | status |
| --- | --- | --- |
| A line that mixes text with an **inline image taller than the text**, under an author `line-height` or a `vertical-align` other than the default — `img { vertical-align: middle }` and `vertical-align: bottom` are CSS-reset material, and `line-height` + an inline icon is the ordinary shape of a nav bar, a card, a byline, a chip row and an avatar list | Every page that styles its inline media at all. It feeds `C01ca geometry: <div>` (111 sites) and `C7eb9 geometry: <body>` (93 sites) — the same `dy` term tick 688 isolated, but the half of it the strut did not reach | ✅ fixed (tick 695) — CSS 2.1 §10.8 builds a line box from **two maxima taken about the baseline**, `max(distance above)` and `max(distance below)`, each inline box having added **its own** half-leading. We folded `max(ascent)`/`max(descent)`/`max(line-height)` over the line and then **centred the content area in the result**. Those agree *exactly* when the tallest box on the line is the one carrying the leading — a plain paragraph, i.e. most of the web — which is why it survived 690 ticks; they diverge the moment the tallest box is an **atomic**, and then the whole line is displaced, text included. Chrome-measured, `16px/normal sans-serif` + a 40×40 `<img>` + a `<span>`: `line-height:60px` div **h=60→65** with its img top **8→0** and its span top **34→26**; `vertical-align:top` span top **24→0**; `vertical-align:bottom` span top **0→22**; `vertical-align:middle` span top **16→10**. **22 of 22 probed boxes now match Chrome exactly** — and the 1px on the plain baseline row was NOT the font difference tick 691 recorded it as, it was the half-leading's rounding remainder. ⚠⚠ **`top` and `bottom` are OPPOSITES**: both align to the LINE BOX's own edges so both are applied last, but `top` grows the line DOWNWARD (the baseline stays) and `bottom` grows it UPWARD (the image pins the bottom edge; the strut's descent must still fit under the baseline above it). Both give a **40px line box** and differ only in where the text sits, so a height assertion cannot gate it — the first version treated them alike and left `bottom` **22px** out. Gated by **`the_half_leading_belongs_to_each_inline_box_not_to_the_line`**, which asserts POSITIONS, RED-proven against three mutations (whole-line centring; `bottom`-as-`top`; atomics back in the text metrics — that third one SURVIVED the first gate, which is why the `middle` row exists). `parity` 72/72 across 30 pages holds. ⚠ Still approximate and named: `middle` resolves x-height as `ascent/2` where the face says ~`0.52 × em` (2px), `sub`/`super` use 0.15/0.35 constants (1–2px) — all inside the 8px tolerance, all the same missing font-metrics plumbing |

| pattern | where it shows up | status |
| --- | --- | --- |
| An **app-shell page whose client render is its own error boundary** — the SSR markup is served, the bundle boots on `load`, clears its mount point and re-renders. Every reporter it ships (`window.onunhandledrejection = report`, `addEventListener('unhandledrejection', …)`) is how the app and its telemetry learn that the boot failed | Every SPA/SSR-hydrated site, which is most of the commercial web. It is the whole of `C3833 MISSING BOX: <div>` (32 sites, 7544 hits — the top cluster BY HITS) on its worst site: `wix.com` loses **3394 of 3775 probed boxes**, 90% of the page | ⚠ **root cause RE-CLASSIFIED (tick 696), fix NOT landed.** The cluster is not a layout gap and not a parse gap: on the byte-reproducible snapshot our DOM holds **5478 elements at DOMContentLoaded** and `manuk_html::parse` alone finds `#main_MF` with all its children — then, inside the `load` event, page script sets `#SITE_CONTAINER.innerHTML = ""`, deleting **4917 elements**, and the client re-render never performs **a single** `appendChild`/`insertBefore`. Final: 562 elements. So `MISSING BOX` on this class means *script deleted it*, not *we laid it out wrong* — and a coverage/geometry tick aimed here would have moved nothing. What landed is the capability whose absence made it unnameable: `unhandledrejection` reaches the page and the host report carries the STACK, turning `Error: couldn't get user details` into `isLoggedInUser@https://wix.com/ inline#102:94:15`. Gated by **`a_page_can_hear_its_own_rejected_promises`**, RED-proven against three mutations (fire nothing; ignore `preventDefault()`; drop the stack), each failing a DIFFERENT assertion. ⚠ The re-render's own failure is still OPEN and is the next tick on this cluster: `MessageChannel` delivers, no listener throws, and no console error is emitted — so the render is abandoned somewhere that reports nothing yet |

| pattern | where it shows up | status |
| --- | --- | --- |
| An **inline element holding an absolutely-positioned child** — `<a style="position:relative">text<span style="position:absolute">…</span></a>`: the stretched click target, the badge on an icon link, the tooltip anchor, the dropdown under a nav item, the `.sr-only` label | Every navigation bar and every card grid on the commercial web. It feeds `C01ca geometry: <div>` (111 sites / 14002 hits — the top geometry cluster), and the first-divergence probe put it at the head of the chain on **two of three** sampled sites (`keirin.jp` dy=+70 right after `nav/a/img`; `www.ikea.com` dy=-70 right after `li/astro-island/a/svg/path`) | ✅ fixed (tick 697) — TWO defects at one boundary, each alone a near no-op. (1) `position:absolute` **blockifies `display`** (CSS Display §2.7), so the child computed `display:block` and `inline_contains_block` fed it to the CSS 2.1 §9.2.1.1 block-in-inline split — which applies to **in-flow** blocks only. The inline was blockified into a **full-width block**: the link took the whole line, forced a break, changed its parent's height and displaced everything below it. (2) `LayoutBox::walk` descends `BoxContent::Block` only, so a boxless inline has no entry in `position_absolutes`' rect map and `abs_containing_block` walked past it — though CSS 2.1 §10.1 says an inline-level ancestor IS the containing block. Chrome-measured: `#aRel` **[0 68 1200×18] → [36 50 76×19]** against Chrome's [36 50 76×17], and the abspos child `#cRel` **[0 68 10×10] → [36 50 10×10]**, exact. Control `desitales2`: SHAPE **60.6% → 61.5%**, median `dy` **127 → 80**, with 597 paths / 8 missing / 582 misplaced byte-identical. Gated by **`an_out_of_flow_child_neither_splits_its_inline_nor_escapes_it`**, RED-proven against three mutations including the over-correction (every ancestor a containing block → the `position:static` control moves). ⚠⚠ Its FIRST version passed with the fix reverted: the unit harness runs `MinimalCascade`, which does not blockify, so the fixture computed `display:inline` and never reached the bug — the fixture now states `display:block` outright. ⚠ Still open: `node_rects` lifts out-of-flow descendants into boxless inline ancestors, so a static inline holding an abspos child reports [0 16 113×88] vs Chrome's [36 84 76×17] — `getBoundingClientRect` on a link, so hit-testing too |

| pattern | where it shows up | status |
| --- | --- | --- |
| **The clearfix** — `.cf::after{content:"";display:block;clear:both}`, the float-containment idiom of the last fifteen years: every Bootstrap-era grid, every WordPress theme, every hand-rolled `.clearfix`, and the ordinary way a legacy nav bar contains its floated `<ul>` | Enormous. It feeds `C01ca geometry: <div>` (111 sites / 14002 hits). On `keirin.jp` it IS the first divergence: `#nav_menus` and `#navbar` were **h=0 against Chrome's h=70**, and 70 was exactly the `dy` the probe reported | ✅ fixed (tick 698) — generated content had **no block-level form at all**: `collect_inline_group` materialises `::before`/`::after` as inline WORDS ("the only place it can enter the flow") and additionally dropped `content: ""`, because an empty string looked like nothing to render. It is not — an empty string is a box with no text; only `content: none` suppresses a pseudo. With no box nothing cleared, so the parent **collapsed to zero**, dumped its floated children outside itself and pulled every following sibling up. ⚠ BISECTED before fixing: a real `clear:both` sibling already worked (h75, matching Chrome) and `::after{content:"XY"}` already rendered — two of the three candidate causes were correct, and fixing either would have been invisible. Chrome-measured: `display:block` and `display:table` clearfixes both **h0 → h70**, with a plain block still h0 and `overflow:hidden` still h70. `keirin.jp`: misplaced **1041 → 954** at an identical path count and missing set, median `dy` **124 → 38**, SHAPE 56.8% → 59.2%, first divergence off the nav — verified against a same-session control run TWICE on the stashed tree (byte-identical both times, because keirin's 3.7-point spread would otherwise swallow the SHAPE delta). Gated by **`a_block_level_after_pseudo_clears_the_floats_its_parent_would_otherwise_drop`**, RED-proven against three mutations; ⚠⚠ the over-correction one (any `::after` clears, ignoring `display`) SURVIVED the first version — `clear` does not apply to an inline box (§9.5.2), and the fixture had no inline `::after` to notice. ⚠ Adjacent, measured, NOT fixed: `display:flow-root` comes out [0 70 0×19] vs Chrome's [0 0 1200×70] and is absent from `establishes_bfc` |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A `display` keyword the Stylo bridge never mapped** — `flow-root` (the modern "contain my floats"), `list-item` (every author-redeclared `<li>`), `table-column` / `table-column-group` (`<col>`/`<colgroup>`, how table column widths are declared) | The `display:` mechanism band — 423 sites / 5945 hits, and cluster `C7460 display: none → block (<div>)` (17 sites). `flow-root` and `list-item` are ordinary modern CSS; the two table values are how any `<table>` with `<colgroup>` sizes its columns | ✅ fixed (tick 699) — one shared cause: `map_display`'s catch-all is `_ => Display::Inline`, and the post-mortem comment for the LAST bug it caused (`display: contents`) sits directly above it. ⚠ **A catch-all that returns a PLAUSIBLE value is a bug factory**: it never errors, never logs, and an inline box still participates in layout — so every value it eats presents as a subtle geometry bug somewhere else entirely, not as an unsupported keyword. Fixed by SWEEP, not by the one value that surfaced: 23 keywords against Chrome went **6 of 23 divergent → 2 of 23** (the remainder is `ruby` and MathML `math`, both named post-Phase-0). `table-column`/`table-column-group` had variants in our own enum the whole time; `list-item` is a MODIFIER BIT in Stylo (`LIST_ITEM_MASK`), not a distinct value, so it matched no const. `flow-root` measured **[0 0 0×19] → [0 0 1200×70]** against Chrome. ⚠ `StyloDisplay::FlowRoot` is `#[cfg(feature="gecko")]` and we build servo — but only the CONSTANT is gated, not the parser, so the public `outside()`/`inside()` accessors close it with **no fork and no patch** (a Stylo bump would fail to compile rather than silently revert). Gated by **`flow_root_is_a_block_that_contains_its_floats`**, RED-proven against two mutations — block-level and BFC-establishing are separately load-bearing, and a plain-block control proves floats are not being contained unconditionally |

| pattern | where it shows up | status |
| --- | --- | --- |
| **An inline `<svg>` icon** — the nav chevron, the search magnifier, the social row in a post footer, the star rating, the "copy" button on a code block. The `<svg>` is one atomic box to CSS, but Chrome gives every element INSIDE it — `<path>`, `<g>`, `<circle>`, `<rect>` — its own box, mapped from path data through the `viewBox` transform | Enormous, and it is how the icon web is built since the sprite-sheet era ended. It IS cluster `Ccd7f <path>` — **34 sites / 1,658 hits**, third on the board's ranked mechanism list, and it also feeds the `<g>`/`<circle>`/`<rect>` rows | ✅ fixed (tick 704) — we laid the `<svg>` out atomically **and then let CSS layout loose on its subtree**. `<path>` computes `display:inline`, has no text, and an inline box with no text is exactly `0 × line-height`: on `www.desitales2.com` every icon path reported **`[40 2595 0×22]` against Chrome's `[40 2514 12×12]`**. Not a near miss — a number from the wrong formatting model. ⚠ **The fix is NOT to drop the boxes**: deleting them turns a *misplaced* element into a *missing* one, which trades SHAPE for COVERAGE and the board ranks MISSING_BOX as the worse of the two. `engine/page/src/svg_geometry.rs` borrows usvg (already in the tree decoding `<img src="*.svg">`) off the SAME serialized markup as the raster, pairs its rendered leaves to DOM shape elements in document order, and applies the result **after** layout — the svg's used box exists only then, and running post-layout is what makes the pass unable to perturb anything outside an `<svg>`. **`desitales2` SHAPE 61.5% → 70.3%** with VISUAL, COVERAGE and the 597/8/582 path counts byte-identical; `en.wikipedia.org` and `blog.rust-lang.org` unmoved; layout 94/94. ⚠⚠ **`getBoundingClientRect` is the DECORATED bounding box — it INCLUDES THE STROKE**; `getBBox()` is the fill box. The oracle probes with the former, so `abs_stroke_bounding_box()` is correct and `abs_bounding_box()` is not — and **the motivating site could not tell them apart**, because desitales2's icons are unstroked (the two boxes coincide) so the fill box matched Chrome *exactly* there while quietly costing **−0.3 SHAPE on `en.wikipedia.org`**, whose icons are stroked. A fixture that cannot distinguish two candidate rules has not chosen between them. ⚠ Also measured: `abs_bounding_box()` is in `Tree::size()` CANVAS space, not `viewBox` units — the viewBox transform is already folded in, so the only remaining scale is `used_box / Tree::size()`. **The build spec for this landed at tick 393 and named this exact gap** (`docs/wiki/box-layout.md`, *"geometry mapping is the other half"*); it sat unbuilt for 310 ticks while its symptom sat at #3 in the ledger, because no ranking instrument reads the wiki. ⚠ The spec said *fall back to document-order pairing*; this **REFUSES** instead — leaf counts must match exactly, `<foreignObject>` refuses the whole svg (it holds real HTML whose boxes CSS owns), and a mismatched aspect refuses too, since a wrong pairing attributes one shape's bounds to another element and a plausible false number is worse than the honest `0×22`. A refused svg keeps its old boxes. Gated by four tests, one per refusal plus the mechanism. ⚠ Named residue: `padding`/`border` on the `<svg>` itself, `<use>` cross-references, non-matching `preserveAspectRatio`, and stale-geometry-on-mutation (shares the raster's cache lifetime) |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A render-blocking external stylesheet that never arrives** — a slow CSS host, a cancelled fetch, or an origin that serves our engine a different document than it serves everyone else. The page still renders: as the **UA stylesheet's** idea of it, `body{margin:8px}` and all | 21 of the 265 corpus sites (8%) come out blank enough that the fidelity sweep books them `render-failed` — the one reason on its taxonomy it calls *"our own bug rather than a property of the origin, and the one that most deserves to count against the score"* | ⚠ **PARTIALLY fixed (tick 708) — the COUNTING hole is closed; the dominant CAUSE is localised, not repaired.** `collect_before_deadline` returns the futures that FINISHED, so a sheet the load deadline cut off never reached the loop that owns both the logging and the `failed_css` bookkeeping: the page rendered unstyled with **no** `STYLESHEET FAILED` line, no `failed_css` entry, and `failed_stylesheet_fetches()` — which exists precisely so a measurement can refuse to score a page we never styled — answering **0** with **zero callers**. Its blind spot was the common case. Now the requested set is diffed against the settled set and every cut URL is recorded, with ONE warning naming the count and the URLs (the failure is the DEADLINE, a single event; N lines would read as N faults). Gated by **`a_stylesheet_the_deadline_cut_off_is_counted_as_failed`**, deadline set in the PAST so the cut is deterministic and needs no network, RED-proven by deleting the bookkeeping. ⚠⚠ **It does NOT fire on the site that motivated it.** `serverfault.com` logs `load phase done phase="external CSS" ms=0 gave_up=0` — nothing was cut because nothing was requested. The measured cause is one layer lower: **our engine's fetch receives a different document than any other client does** — `<head>` element children **9 from our live fetch against 49 via `curl`, same URL, same User-Agent, same moment**, and not one `<link>` among our 9. So `collect_style_sources` finds no sheets, the page lays out at 7,328px under `body{margin:8px}` where Chrome has a 720px `display:Flex` shell, and the sweep calls it a paint failure. ⚠ t707 had ruled this cause OUT by fetching with `curl -A "<our UA>"` and getting the full document — **`curl` is not our net stack**, and standing one in for the other is exactly the error; that elimination is retracted. The remaining question is which request property (Accept, Accept-Encoding, HTTP version, TLS fingerprint, cookies) flips the origin, and it is now bounded because the two responses can be diffed byte for byte |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A `<script src>` written in the MARKUP whose completion drives the next step** — `<script id="x" src="…widget.js"></script>` plus `document.getElementById('x').onload = function () { Widget.render(…) }`, or a capture-phase `load` listener a tag manager installs over a document it did not author. The whole widget/loader web is shaped like this: analytics, tag managers, ad tags, reCAPTCHA, payment and map SDKs, and the "boot the app once the vendor bundle is here" line at the bottom of every SSR page | Every commercial page ships several. It is the mechanism half of `C3833 MISSING BOX: <div>` — **32 sites / 7,544 hits, the top cluster BY HITS**, and since t711 also the honest remainder of `render-failed`: an app shell that clears its mount point and then re-renders *from a script-load callback* performs zero DOM operations if that callback never runs | ✅ fixed (tick 712) — **one rule, two implementations, and only one of them was built.** A classic external script fires `load` at its element (HTML §4.12.1). The *script-inserted* route (`createElement` → `src` → `appendChild`) fires `load` and `error` and has since the agoda `ChunkLoadError` tick, gated by `g_script_load_event`. The *parser-inserted* route fired `error` on a 404 **by accident** — a failed fetch leaves `src` in place, so the injected-script drain adopts the node, re-fetches, fails and reports — and fired **nothing** on success, because `fetch_external_scripts` inlines the source and **removes `src`**, destroying the only evidence the element was ever external. ⚠ **Three loud outcomes and one silent one reads, from outside, exactly like a working feature** — which is why it survived 700 ticks. Chrome-measured on one fixture, four cases: Chrome `parserOK:load \| parser404:error \| dynOK:load \| dyn404:error \| window:load`, ours missing only the first. Because the DOM cannot answer *"was this external?"* at execution time, the fact is **carried** (`PENDING_EXTERNAL_SCRIPTS` → `PageContext::external_scripts`), the same shape as the CSP authorization decision seeded beside it, and owned by the `PageContext` so it is document-scoped by construction. On the byte-reproducible `wix.com` snapshot: DOM inserts after the wipe **6 → 44**, elements **560 → 598** — the re-render that performed *zero* operations now performs some. ⚠ **The site still does not render**: its `window.onload` dies at `google is not defined` because `accounts.google.com/gsi/client` answers our honest User-Agent with **403** — proven a UA wall and not a `Referer` one (three curl requests differing only in that header all return 200; the Manuk UA returns 403, a Chrome UA 200), and the bot-wall track is out of engine scope. The cluster moved; the site did not. Gated by **`g_markup_script_load_event`**, RED-proven against FOUR mutations each failing a DIFFERENT assertion — *"fires a `load` event"* has four implementations that satisfy the sentence and break the contract: fire it before the script runs (the handler sees nothing the script defined), fire it at every script (an inline `<script>` owes none), batch them after the pass (the next script is stranded), or call the element's `onload` property directly (invisible unless someone listens in the **capture** phase, which is exactly where a tag manager listens). ⚠ Named residual, proven pre-existing on a page with **zero** external scripts: the window `load` leaks into document-level capturing `load` listeners with `event.target === null` — Chrome `start;WINLOAD;` vs ours `start;WINLOAD;capture[t=null];`. ⚠ Population stated as the floor it is: **8 of 245 corpus snapshots** carry the handler in the *served* document; the commonest form assigns it from a separate external script, which a grep of served bytes cannot see |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A page that MEASURES ITSELF** — `getComputedStyle(el).display`, `el.getBoundingClientRect()`, `offsetHeight` — and then writes the answer back into the DOM. Every carousel, sticky header, virtualised list, masonry grid, chart library, responsive menu, "is this on screen" check and anti-adblock bait test is this shape | Every scripted page on the web, which is nearly all of them. It feeds `C01ca geometry: <div>` (111 sites / 14,002 hits) and it is the first divergence on `www.desitales2.com`, the byte-reproducible control: `.site-content` reads `display:block` to the page while the engine's own final layout is `flex`, and the sidebar ends up 28,000px below the article instead of beside it | ✅ **fixed (tick 719, on the THIRD design — the two reverts below are kept because the arithmetic that killed them is why the landed one looks as it does).**  `load_async` — the path the agent, the oracle and every fidelity measurement navigate with — applies external stylesheets inside `finish_loading`, which runs **after** it has already fired `DOMContentLoaded` **and `load`**. The navigation ledger printed the order for eight ticks: `cascade+layout+blocking scripts · DOMContentLoaded · load event · initial images+masks · external CSS`. So for the whole scripted lifetime of the document a script measures a page with **no author CSS**: on a three-line fixture with one `<link>`, `display=block width=1184` at sync/DCL/load/timer, where the engine's own final layout is `flex`/`829`. ⚠⚠ **The final painted layout was always correct**, which is why no screenshot, box dump or fidelity score has ever seen this — they all read the finished answer, and this defect exists only *during* the load. The spec: a `<link rel=stylesheet>` is render-blocking AND script-blocking, and `load` does not fire until the sheets have loaded. **THE FIX** is one `await` — `page.fetch_and_apply_stylesheets(fonts, viewport_width).await;` plus its `nav_phase("external CSS", …)`, inserted in `load_async` immediately before the `page.module_graph_sources = …` block (i.e. after `Page::from_dom`, before the deferred pass and both lifecycle events). **THE GATE** (`engine/page/tests/g_css_before_lifecycle.rs`, removed with the revert, restore verbatim): one local socket serving `.shell{display:flex} .main{width:70%} .side{width:30%}`; a document with `#at-blocking/#at-dcl/#at-load/#at-timer` each written by `obs() = getComputedStyle('#shell').display + '/' + round(rect('#main').width)`; drive with `Page::load_async` ONLY (calling `finish_loading` lets the bug's own late apply satisfy the gate); assert `dcl == load == timer == "flex/829"` and pin `blocking == "block/1184"` as the named remaining half (a parser-blocking script still runs before the sheets — same order `from_prefetched` has always had). RED-proven by two mutations: delete the call → `block/1184` at every phase (the exact pre-fix state); move it below `fire_lifecycle("DOMContentLoaded")` → DCL red while `load`/timer stay green. **THE MEASUREMENT THAT LOOKED LIKE A REGRESSION, AND THE CONTROL THAT RETRACTED IT:** measured `www.ikea.com` SHAPE **53.58% → 55.30%** (698 scored, identical) but `www.welt.de` COVERAGE **95.61% → 0.03%** (3,260 of 3,261 paths missing), controlled by reverting the hunk and rebuilding — control 95.6%, fixed 0.03%, at an unchanged load time, and 95.7% again after the revert. Cause named: with the fix in, welt emits `Failed to load website due to adblock: Error: Failed to execute packing script`, its own anti-adblock guard, a **false positive** (this build has no `adblock` feature) that fires only once the page can measure a styled document — **our blindness was masking a second divergence.** ⚠ Applying after the deferred/module pass but before `DOMContentLoaded` collapses identically, so the guard runs later than DCL and there is no narrow variant. ⚠⚠ **AND THEN THE CONTROL SAID THE FIX WAS NOT GUILTY (t715).** The **unmodified** engine reproduces the identical blank page and the identical `Failed to load website due to adblock` line with nothing changed but `MANUK_LOAD_BUDGET_MS=12000 -> 40000` — `structural: 0.0% (3360 paths, 3359 missing)`. **welt blanks itself whenever our engine lets its anti-adblock guard reach a verdict**, and the 95.6% was our own 12s timeout cutting the site off before it could reject us. That is coverage achieved by NOT RUNNING THE PAGE'S SCRIPT — the exact shape of lie `G_FIRST_PAINT` and `G_DEFER` exist to strip off, and refusing a correct fix to preserve it would have been preserving the lie. Fix and gate restored. The REAL open item, now named and separated: `html-load.com/app.js` (302 -> `stg.html-load.com/app.js`, 113KB, obfuscated, referencing `cache_adblock_circumvent_score` / `banner_ad` / `contentDocument`) **fails to execute** in this engine, which is what welt's guard reports. That is a capability question about dynamically-created iframes and `contentDocument`, not an ordering one, and it is what makes welt render rather than blank. ⚠⚠ **AND THE POPULATION READ KILLED IT (t716).** HEAD-20 on the landed tree: `keirin.jp` SHAPE **60.2% → 53.0%**, twice isolated on each tree, coverage and box counts (1377/339/954, 1038 scored) IDENTICAL — the same boxes, moved. `MANUK_LOAD_BUDGET_MS=40000` restores 60.2% exactly, so it is **budget starvation**, not ordering. And the arithmetic forbids fixing that by tuning: `G_LOAD`'s ceiling is **2× the load budget for the whole page**, and the navigation already spends one budget in `load_async` (enhancements) and one in `finish_loading`. A third phase either takes its own budget (`load_async` alone spends 2×, page 3× → G_LOAD red at 5.4s/2s) or shares one (the phase it shares with starves → keirin −7.2). For any slice `s > 0` the worst case is `1 + s + 1` against a ceiling of `2`; **no slice fits.** ⚠ **THE CORRECT FORM OF THIS FIX IS THE OTHER DIRECTION: move the LIFECYCLE EVENTS later, not the CSS earlier** — `load` is not supposed to fire until the subresources are in, so `load_async` should not be firing `DOMContentLoaded`/`load` at all and `finish_loading` should, after its CSS phase. That spends no new budget and needs no slice. It changes what every caller of `load_async` gets back, so audit them first. The gate above is restored verbatim to prove it. ⚠⚠ **AND THE THIRD DESIGN LANDED IT (t719): fetched at PARSE, waited for NOWHERE.** The audit that unlocked it: `load_async` has **no shell caller** — the shell navigates via `from_prefetched`, which has *always* applied CSS between `from_dom` and the deferred pass. So the blast radius is the **agent** and **every fidelity measurement**, not the shipping browser, and *the path without the bug is the design document*: `from_prefetched` does not need a schedule because its CSS is already in hand. A fourth attempt died first and is worth recording — *run it concurrently with the script fetch, a wait that already happens* is sound and still wrong, because **`G_LOAD`'s fixture has dead sheets and NO scripts**, so there was no existing wait to hide behind and that phase went 0s → 2s. **The bound is not the budget, it is the fixture that measures the budget.** Landed: spawn the sheet fetches immediately after the parse (off the real tree via `collect_style_sources`, so `media`/shadow/inline are handled) and at the apply point take only the handles reporting `is_finished()` — **never awaiting one**. Head start = the external-script fetch + module-graph prefetch + cascade + layout + every blocking script; anything late falls through to `finish_loading` as before. Measured: ordering `block/1184 → flex/829` at DCL/load/timer (Chrome `flex/829`) · `G_LOAD` **3.51s** against a 2s budget, *faster than the 3.4s before the tick* · `keirin.jp` SHAPE **60.2% unmoved** · `www.welt.de` **95.7% unmoved and NOT collapsing** (confirming t715: the earlier designs blanked it by freeing budget for its guard) · `desitales2` unmoved · and **`www.ikea.com` COVERAGE 97.08% → 100.0%, 21 missing boxes → ZERO**, SHAPE 53.6 → 55.4. ⚠⚠ Those 21 boxes were the t713 open item and were never a layout bug: **a COVERAGE loss whose cause was a MEASUREMENT the page took** — no box-diff could attribute it, because the missing boxes are the ones the page decided not to create |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A frame the page makes for ITSELF** — `<iframe srcdoc=…>`, `<iframe src="about:blank">`, or `document.createElement('iframe')` with no `src` at all — created in order to be *read and written*, not displayed | Enormous and mostly invisible: obtaining a **pristine `window`** to lift unpatched natives out of (every library that defends against a patched prototype), sandboxing untrusted markup, `postMessage` relays, OAuth and payment bridges, sandboxed previews and documentation embeds and mail clients (`srcdoc`), and **ad-bait tests** — create a frame, write ad-shaped markup into its `contentDocument`, measure whether it survives. Cluster `C500e MISSING BOX: <iframe>` (8 sites), and the mechanism behind `www.welt.de`'s `render-failed`: welt runs exactly that bait test and blanks itself when it fails | ✅ fixed (tick 717) — `pending_iframes` is a **fetch** work-list and skipped all three, *correctly*, because there is nothing to fetch; **nothing then loaded them either.** HTML §4.8.5 navigates a src-less `<iframe>` to `about:blank` and gives it a fully-formed same-origin document — ours got none, so `contentDocument` was `null`. ⚠⚠ **No feature detect could see it: `typeof null === 'object'`.** Chrome-measured before: `dyn.contentDocument=object` in BOTH engines, then `THROW: can't access property "body", f1.contentDocument is null` on our next line, and `late.getById` **found** vs **no-doc**. `load_inline_frames` adds a second, synchronous work-list for the frames whose document does not come off the network — `srcdoc` parses its markup, `about:blank` and a bare `<iframe>` get a real empty document with a writable `<body>` — run before the fetch round, since a page that creates a frame to *read* it should not wait on a network round with nothing to do. `render_iframe` already took its HTML as a string, so it reuses the whole child-page machinery and adds no subsystem. After: `late.getById=found`, matching Chrome. Gated by **`g_inline_frame_document`**, hermetic (not one frame touches the network — the point of them), RED-proven against three mutations each failing a DIFFERENT assertion: drop the `srcdoc` arm → `no-element`; require an explicit `src` → the bare `<iframe>` reads `no-doc`; drop the `about:blank` arm → the write-and-query reads `no-doc`. ⚠ The `about:blank` assertion **writes into the document and queries the result** rather than checking `contentDocument != null`, because a documentless stub satisfies a null check and still fails the page. ⚠ Named residual, pinned by assertion (4): these load on the host's next round, so a script that appends a frame and reads `contentDocument` on the **very next line** still sees `null` (Chrome has it immediately); at `DOMContentLoaded`, `load` or any later task it is a real document. Closing that means building a child document from inside a JS binding |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A page sized in `lh` / `rlh`** — `height:1lh` on a single-line clamp, `margin-block:1rlh` for vertical rhythm, `min-height:3lh` on a card excerpt. The units that let an author say *"exactly one line tall"* without hard-coding a pixel value | Growing fast and unguarded: `lh`/`rlh` reached **Baseline Widely Available in May 2026**, which is precisely the point at which authors stop wrapping a feature in `@supports`. `rlh` is the one used for page-wide vertical rhythm, so a wrong value is wrong *everywhere at once* rather than in one component | ✅ fixed (tick 722) — the cascade calls `Device::set_root_font_size` the instant `<html>` is cascaded (with a comment about `html{font-size:62.5%}` breaking every `rem` without it) and **never set its twin `set_root_line_height`**. Stylo's own `matching.rs` sets the two together, four lines apart, under *"Update root font size for rem units"* and *"Update root line height for rlh units"*; we had the first. Chrome-measured on root `line-height:2`/`16px` with an element `line-height:20px`: `width:5rlh` **160 vs 96** and `height:5rlh` **160 vs 96** — and `96 = 5 × 19.2 = 5 × (16 × 1.2)` is the **initial `normal` line-height**, so `rlh` was neither root- nor element-relative but *initial*-relative, the one answer no author can predict. Now Chrome-EXACT on all four cells (`width`/`height` × `lh`/`rlh` = 100/100/160/160). ⚠⚠ **The map read `works` from tick 509 to tick 721** because the probe behind it tests `width:5lh` and nothing else — and its own receipt said `rlh` was *"not separately geometry-tested"*. **A probe that tests one property has measured one property**; the row's NAME did the over-claiming, not the probe. Gated by **`G_RLH_UNIT`**, RED-proven against two mutations that fail the same assertion with **different wrong values** — delete the call → 96 (initial-relative), set the root values from every element → 100 (element-relative) — so the assertion discriminates all three candidates rather than merely rejecting one. ⚠ Residue pinned by the gate: `Device::calc_line_height` returns **0** for `line-height: normal` in this servo build (*"TODO: compute `normal` from the font metrics"*), so a root stating no line-height leaves `rlh` at zero — honest, and not a regression, since the value it replaces was wrong for every root. ⚠ Separate and still open: `CSS.supports('width','5lh')` answers **false** here and **true** in Chrome for a unit that demonstrably works — a false NEGATIVE, the mirror of this project's usual false-presence hazard, so a page that guards `lh` takes its fallback for no reason |

| pattern | where it shows up | status |
| --- | --- | --- |
| **Root-relative font-metric sizing** — `max-width: 65rch` for a page-wide readable column, `rex`/`rcap` for vertical rhythm that must NOT drift when a component sets its own `font-size`. The whole reason the `r*` twins exist is that `ch`/`ex`/`cap` follow the element and a design system needs one number for the document | Every design system that keys sizing off the root instead of the component — and because the unit is root-relative, a wrong value is wrong **everywhere at once** rather than in one place. Same Baseline-driven exposure as `lh`/`rlh` (t722): the guards come off as these reach Widely Available | ✅ fixed (tick 723) — **the third member of a set of three, named in writing by t722's pattern line before it was measured.** Stylo updates three things when the root element is cascaded: `set_root_font_size` (`rem` — we had it), `set_root_line_height` (`rlh` — landed t722), and `set_root_style` + `update_root_font_metrics` (`rcap`/`rch`/`rex`/`ric`). `update_root_font_metrics` reads `device.root_style`, and **nothing in this engine ever wrote that field**, so all four resolved against the device's *default* style. Chrome-measured, root `32px` / element `16px` sans-serif: `10rch` **178 vs 80**, `10rex` **169 vs 73**, `10rcap` **220 vs 105** — while the element-relative twins `10ch`/`10ex`/`10cap` were **already exact** at 89/85/110. ⚠⚠ **Every element-relative unit right and every root-relative one wrong is the signature of a root that was never published, not of a broken metric** — that shape is what identified it, and the twins are asserted in the gate as the over-correction guard. Now Chrome-EXACT on all six. `update_root_font_metrics` queries the font stack, so it runs only when the document actually used one of these units (`used_root_font_metrics()`), exactly as Stylo's own `matching.rs` gates it — a page with no `r*` unit pays a bool read. Gated by **`G_ROOT_FONT_METRIC_UNITS`**, RED-proven against two mutations giving *different* wrong values — drop `set_root_style` → 80/73/105 (device default); publish every element as the root → 89/85/110 (element-relative) — plus a **ratio** assertion (each `r*` is 2× its twin, since the root's font is 2× the element's) that survives a font-stack change the absolute numbers would not. ⚠ Still open, shared with `lh`/`rlh`: `CSS.supports('width','10rch')` answers **false** here and **true** in Chrome for units that demonstrably work |

| pattern | where it shows up | status |
| --- | --- | --- |
| **Progressive enhancement guarded by `CSS.supports()`** — `if (CSS.supports('display','grid')) mountGrid(); else mountFloats();` and every variant: scroll-snap vs a hand-rolled carousel, `position:sticky` vs a scroll listener, `aspect-ratio` vs a padding hack, a modern unit vs a pixel fallback | Every carefully-built site on the web, which is the point: the guard is what a *careful* author writes. And the failure is invisible from outside — a page that takes its fallback renders fine, just old | ✅ fixed (tick 724) — the hook letting `CSS.supports()` ask the real CSS engine was installed in **`Page::load` only**, the synchronous path. `load_async` (the agent + every fidelity measurement) and `from_prefetched` (**the SHELL — the shipping browser**) never installed it, so the binding answered **`false` to everything**: `width:5px` false, `display:flex` false, `color:red` false. ⚠⚠ **`display: flex` is the tell** — the Rust-level `supports_condition` has asserted that exact string true since it was written and its unit test passes, so **the engine knew the answer and no page could reach it.** A false NEGATIVE on feature detection is not a missing feature; it is every guarded site silently selecting its 2015 codepath on a browser that can run the 2026 one — and such a page **looks like a page that preferred the old layout**, including to our own fidelity diff, which then compares its float layout against Chrome's flex one and books the difference as a geometry bug (a plausible, unclaimed contributor to the `display:` mechanism band — 423 sites / 5,945 hits). The fix is a **move, not an addition**: `install_supports_hook()` runs in `Page::from_dom`, the one function every construction path goes through — *three callers is what produced one.* Now Chrome-matching on 7 of 8 probed cases including both call shapes, `not`/`and` compounds, and three negative controls. Gated by **`g_css_supports_hook`**, driven through `load_async` deliberately (`Page::load` is the one path that always worked, so a gate written against it would have passed throughout the entire bug), RED-proven against two mutations each failing a DIFFERENT assertion: remove the install → all-false (the shipped behaviour); make the hook `\|_\| true` → the three negative controls go true. ⚠ Assertion (2) exists because **a stub that answers yes is a worse bug than the one being fixed** — it makes a page ship a codepath this engine cannot run. ⚠ Named residual pinned by the gate: `CSS.supports('width','5cqw')` is `true` in Chrome and honestly `false` here — container-query length units are a real, separate gap. ⚠⚠ **Three consecutive ticks (721/722/723) recorded this and moved on**, each filing it under a different subject (*"a false negative on `lh`"* / *"on `lh`/`rlh`"* / *"on `rch`/`rex`"*): three tickets, three units, one line of plumbing with nothing to do with units. **When a residual keeps reappearing beside different subjects, test it with the most boring input you have** — `color: red` found it in one command |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A page that TOGGLES a stylesheet by its media** — `[...document.styleSheets].find(s => s.media.mediaText === 'print')`, then `sheet.disabled = true`. The print-preview switcher, the dark-mode sheet swapper, the "disable the mobile stylesheet" control, and every CSS debugging panel that groups sheets by media | Ordinary and long-lived: it is the standard way to reach one sheet among many without an id, and it is the only handle a page has on a sheet it did not author (a CMS block, an embedded widget). Also every style-inspector build tool | ✅ fixed (tick 727) — `sheet.media` was the **constant** `{ length: 0, mediaText: '' }`, so *every* sheet reported no media and the `find` above matched nothing: the toggle ran, found no sheet, and did nothing, silently. Chrome-measured on `<style media="print">`: `print` vs `''`. Now a **live getter** over the element's `media` attribute, with `length` and `item()`. ⚠ **A live getter, not a snapshot** — `MediaList` is live in the spec, and a snapshot taken at sheet-construction time passes every shape assertion (`mediaText`, `length`, `item`) and fails only a write-then-read. That is the implementation you write if you are not thinking about it, so the gate mutation-tests for it specifically. Gated by an extension to **`g_cssom_sheet_bridge`** (extended, not a new file — the third CSSOM finding, and a third gate would be the drift the one-gate-per-topic rule exists to stop), RED-proven against two mutations on two different assertions, plus a control that an **unmedia'd** sheet still reports the EMPTY list, because a getter that invents a value for every sheet is the same bug pointing the other way. ⚠ **Scope pinned, not fixed:** a `<link>`ed sheet is still absent from `document.styleSheets` and its `.sheet` is `undefined`. That is a t663 **decision**, not a defect — for an applied linked sheet, `null` is a lie that reads as honest and a half-built object is worse — and t718's *"styleSheets reports 0 where Chrome reports 9"* was nine `<link>`s, correctly absent. A new assertion pins it so the day linked sheets land, the gate says so rather than quietly widening |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A page that measures its own viewport** — `scrollHeight - clientHeight` (the divisor of every virtualised list), `scrollWidth > clientWidth` (every "is this overflowing?" test), `clientHeight` for sticky-header offsets and carousel page sizes, and `if (!el.clientHeight)` as the standard *"is this laid out yet?"* guard | Every bordered element on the web, which is most of them — and the consequence is not cosmetic: an overflow test that overstates the viewport answers **no** on a box that IS overflowing, so the scrollbar never appears and the virtualised list renders the wrong slice | ✅ fixed (tick 728) — `clientWidth`/`clientHeight` reported the **BORDER** box for every element that is not a scroll container. `scroll_geometry_of` has computed the padding box correctly since it was written (`rect.width - bw.left - bw.right`) and it only maps `overflow: auto\|scroll\|hidden` containers; everything else took the getter's *fallback*, which handed back `rect.width`. ⚠⚠ **The right answer was computed for the minority and the fallback answered for everyone else** — and the fallback's own comment was half right, which is why it survived: *"a plain `<div>` still has a `clientHeight`, and it is its own box"* — true, and **its own box is the BORDER box while `client*` is the PADDING box.** Chrome-measured on `width:200px; padding:10px; border:2px`: `clientWidth` **220 vs 224**, with `offsetWidth` **224 vs 224** (always right). Now Chrome-exact on 5 of 6 cases including the inline one. ⚠ A non-replaced **inline** box reports **0** per CSSOM (Chrome `0/0`, we returned `4x16`) — *a plausible number where the spec says zero*, which is exactly what defeats `if (!el.clientHeight)`. Gated by **`g_client_box_is_the_padding_box`**, RED-proven against two mutations on two different assertions (revert the fallback → `204x104`, the shipped value; drop the inline arm → `4x16`), with a no-border/no-padding **control** row so a fix that subtracted something from every element breaks first, and `offset*` asserted beside every `client*` so the two boxes cannot be conflated in either direction. ⚠ Named residual pinned by the gate: a scroll container reports `220` where Chrome reports `205`, because Chrome reserves a 15px classic scrollbar gutter inside the padding box and this engine reserves none — a scrollbar-model difference, not a box-model one |

| pattern | where it shows up | status |
| --- | --- | --- |
| **Asking what is UNDER the cursor, not just on top of it** — `document.elementsFromPoint(x, y)`. Drag-and-drop finding the drop target *beneath its own drag ghost*; an overlay or tooltip deciding whether it is occluding its own anchor; a click-through affordance forwarding an event to the layer below | Every drag-and-drop library, every floating-UI/popper-style positioner, every canvas-overlay editor — and the agentic surface, where occlusion is precisely what must be reasoned about before a click | ✅ fixed (tick 729) — the singular `elementFromPoint` has worked for hundreds of ticks and the plural threw `TypeError`. **The plural is not a convenience, it is a different question**: the singular collapses the stack to its first entry, so *"what is in the way?"* could not be asked at all. Chrome-exact on a fixture with an absolutely-positioned target over a full-bleed underlay and a `pointer-events:none` drag ghost on top: `target>under>wrap>BODY>HTML`, `[0] === elementFromPoint`, `[]` outside every box, a real `Array` (WebIDL `sequence<Element>`), and a `TypeError` on a non-finite coordinate. ⚠ The ghost is **absent from both engines' stacks** — the plural inherits the singular's `pointer-events` filter rather than reimplementing it, and a plural that reported the ghost would break the exact case it exists for. Gated by **`g_elements_from_point`**, RED-proven against two mutations (drop the `pointer-events` filter → the ghost appears; reverse the ordering → the stack inverts *and* `[0] === singular` goes false, so that invariant is independently load-bearing). ⚠⚠ **The fixture also caught an invented citation.** The first draft returned the empty list on `NaN` *"per CSSOM-View"* — and the same sentence was already in the engine (`elementFromPoint`'s comment: *"returns `null`, per CSSOM-View"*) **and enforced by `g_element_from_point`**. CSSOM-View types both parameters `double`, not `unrestricted double`, so WebIDL rejects NaN before the method runs and Chrome throws. Both corrected; the gate's assertion is now **stricter** (`instanceof TypeError` is satisfied by one thing, `=== null` by three). **When a comment cites a spec for a BEHAVIOUR, the fixture must measure that behaviour in the reference engine** |

| pattern | where it shows up | status |
| --- | --- | --- |
| **`await document.fonts.ready` before measuring text** — the standard prologue of every chart library, canvas text renderer, text-fitting/clamping widget, marquee, and any layout that depends on a webfont's metrics. Also `document.fonts.load('16px Brand')` to force a face before painting | **27.72% of page loads** (Blink use-counter, surface audit #32) — and it is not a soft dependency: the property was `undefined`, so the `await` threw. Measured killing a real boot at t719 on `www.welt.de`: `can't access property "ready", document.fonts is undefined` | ✅ fixed (tick 730, `partial` — the three gaps are stated below rather than papered over) — `document.fonts` now exists with `status`, `check`, `load`, `forEach`, iteration, the event-handler slots, and a **`ready` that is a RESOLVED promise resolving to the SET itself** (the real idiom is `fonts.ready.then(s => s.check(...))`, so resolving to `undefined` breaks the line after the wait succeeds). ⚠⚠ **The dangerous direction is a promise that never settles, not one that settles early** — the map's own row has said so since audit #32 (*"a never-resolving promise HANGS the app"*), so `ready` is deliberately not wired to a loading signal this engine does not expose; faces load during the load phase, before page script runs. Chrome-measured on one fixture and matched on the non-obvious rows: `check('16px NoSuchFamily')` is **true** (an unknown family needs no loading) and `check('notafont')` **throws SyntaxError** (a typo'd call that returns a confident `true` is a page that never finds out). Verified on the motivating site: welt's rejection is gone, fidelity unchanged at 95.6%/65.1%. ⚠ **Stated non-claims:** `size`/iteration do not model `FontFace` objects (Chrome reports 1 for a declared `@font-face`, we report 0) — a visibly empty set is honest where a fabricated entry would be believed; `check` answers `true` even for a declared-but-unloaded face (Chrome `false`) because **`false` is the answer that makes a page wait**; and the `FontFace` constructor is still absent. Gated by **`g_font_loading_api`**, whose first assertion **awaits** `ready` rather than inspecting it — RED-proven by a `ready` that never settles, **a mutation that passes every `typeof` assertion in the file** |

| pattern | where it shows up | status |
| --- | --- | --- |
| **`@font-face` declared in INLINE CSS** — `<style>@font-face{font-family:Brand;src:url(...)}</style>`, which is exactly what every *"inline your critical CSS"* build step produces, and what a font-loading snippet injects | Every performance-tuned site, which is every site with a build step. The font downloads, decodes and resolves correctly and the page is laid out with the FALLBACK — so the text is the wrong width, every line box and wrap point below it is stale, and nothing anywhere reports a problem | ✅ fixed (tick 732) — the relayout guard had already learned that an arriving face is a third reason to redo the work (beside *"external CSS arrived"* and *"the tree is dirty"*), and then handed that reason to `apply_stylesheets`, which gates its relayout behind a fingerprint of **the cascade's inputs**. A newly-arrived face changes none of them: the same inline `<style>` was there before the bytes came back. So the fingerprint matched, `RestyleDamage::None` came back, and **the layout was skipped along with the cascade** — correct for every input it knew about, wrong for the one it did not. ⚠⚠ **A trigger is only as good as what it triggers, and the two were one call apart.** Measured with Ahem, whose every glyph is exactly 1em so 5 characters at `font-size:20px` is exactly `100px` and no fallback can land there by accident: **66.7px → 100px**. ⚠ The gate `g_webfont_relayout` had been **RED since before t718** — twelve-plus ticks — and nobody knew, because `verify.sh` runs 19 of ~104 gates and this is one of the ~85 it does not (found at t730, sized at t731: a 13-gate deterministic sample came back 13/13 green, so this was an isolated red rather than mass rot). RED-proven against two mutations that are **each necessary and neither sufficient**: drop the `!registered_webfont` term from the early return → 66.7px; drop the explicit relayout → 66.7px. ⚠ A third change — clearing the font-resolution/measure/shape caches on registration — was written, reads well, and was **removed for lack of evidence**: with the real fix in, taking it out leaves both gates green |

| pattern | where it shows up | status |
| --- | --- | --- |
| **`performance.getEntriesByType('navigation')[0]`** — the modern, non-deprecated replacement for `performance.timing`, read to report page-load timing: `loadEventEnd - startTime`, `domContentLoadedEventEnd`, TTFB | **web-vitals, Google Analytics, Sentry, Datadog** — and usually the *first thing* the snippet does, so a throw there takes the rest of the snippet with it | ✅ fixed (tick 733) — `getEntriesByType` returned `[]` for every argument, so `entries[0]` was `undefined` and the next property read **threw**. ⚠⚠ `typeof performance.getEntriesByType === 'function'` was true the whole time and the returned `[]` is a *correct shape* and a *plausible answer* — every feature detect passed and the failure was one index away. Same false-presence family as `typeof null === 'object'` (t717) and `CSS.supports` answering false (t724); the discriminator each time is **a probe that USES the value rather than inspecting it**. The instants are recorded in `__fireDOMContentLoaded`/`__fireLoad`, which the HOST calls — the only part of the system that knows when *"the document finished parsing"* and *"the subresources finished"* are true — and **after dispatch, not before**, because the span a library wants is *"how long did my handlers take"* and recording before dispatch reports zero for every page. Chrome-matching on 9 of 10 probed rows including monotonic ordering and `duration > 0`. ⚠ **The network-phase fields are ABSENT, not zero**: `responseStart`/`domainLookupEnd`/`connectEnd` are not observable at this layer, and a `0` is indistinguishable from a real 0ms — a library would report a confident, wrong TTFB and nobody could tell, whereas `undefined` propagates to `NaN`, which is loud. Gated by **`g_navigation_timing`**, RED-proven against two mutations (return `[]` → `len=0` and `TypeError` on every read, exactly what shipped; add `responseStart: 0` → the non-claim assertion fires). ⚠ The **ordering** assertion (`domInteractive ≤ dclEnd ≤ loadEventEnd`) is what separates recorded instants from plausible constants, and the gate reads from a task *after* `load` because `loadEventEnd` does not exist yet inside a load handler |

| pattern | where it shows up | status |
| --- | --- | --- |
| **Writing to the accessibility tree from JS** — `el.ariaExpanded = true` on a disclosure, `el.ariaSelected` on a tab, `el.ariaLabel` on an icon button, `el.role = 'dialog'` — the IDL form, not `setAttribute` | React, Vue, Radix, Headless UI and every design system, because the property is the typed, minifier-friendly, framework-bound form. And it is the project's **stated moat**: invariant I3 makes the a11y tree load-bearing, and this is how the modern web *writes* to it | ✅ fixed (tick 734) — Chrome has **42 of 42** ARIA IDL properties reflecting both ways; this engine had **0 of 42**, so every such write landed on a plain JS own-property and **the agent and the screen reader saw the state from before the interaction**. Added as 42 rows in the existing reflect table plus a new `nullable string` type. ⚠⚠ **The name mapping is the trap: camelCase→kebab is the obvious derivation and it is wrong for every multi-word name** — `ariaValueNow` → `aria-valuenow` (not `aria-value-now`), `ariaPosInSet` → `aria-posinset`, `ariaRoleDescription` → `aria-roledescription`, `ariaMultiSelectable` → `aria-multiselectable`, and `role` carries no prefix at all. The mutation proves why that matters: deriving the names leaves `present=42/42` — the "is the API there?" assertion still passes — while every write goes to an attribute no accessibility tree reads. **A property that exists and writes to the wrong place is false presence that no feature detect can see.** ⚠ `DOMString?` semantics, Chrome-measured and gated: absent is **`null`** not `""` (`el.ariaChecked ?? computeDefault()` and `if (el.role === null)` are how a library asks *"did the author set this?"*, and `""` answers yes to both), `= null` **removes** the attribute, `= ''` leaves it present-and-empty — and stringifying `null` would write the literal `"null"` into an attribute a screen reader then announces. Gated by **`g_aria_reflection`**, RED-proven against two mutations each failing a different assertion while the earlier ones still pass |

| pattern | where it shows up | status |
| --- | --- | --- |
| **Moving a node between documents while KEEPING it** — `document.adoptNode(templateContent.firstChild)`, `document.adoptNode(iframeDoc.getElementById('x'))`. The sibling of `importNode` and the opposite trade: a clone breaks every caller holding a reference to the original | Template-based frameworks and compilers (Lit and friends move `template.content` into the live tree), editors and embed hosts pulling nodes out of an iframe, and anything that has stashed the node in a `Map`/`WeakMap` before moving it | ✅ fixed (tick 735) — `document.adoptNode` did not exist, so the call **threw `TypeError`** and took its caller with it. Now Chrome-exact on all five probed rows: returns the **same node** (identity, not a clone), `ownerDocument` becomes this document, the node is **detached** from its old parent, and `adoptNode(null)` throws `TypeError`. ⚠ **Named non-claim, pinned by the gate: a node from ANOTHER document's arena is REFUSED, loudly.** Each document owns its own `Dom` arena and a `NodeId` is only meaningful inside one — `node_and_dom` exists because reading an iframe's node #7 in the parent's arena returned *the parent's* node #7 "with total confidence". Moving between arenas is a transplant (subtree copy + reflector re-binding), not a re-parent. ⚠⚠ **The mutation that removes the refusal does not give a wrong answer — it destroys the document**: the gate fails on `#out must exist`, because detaching by an id that means something else in this arena corrupts the tree. *"Silently returning a node the other document still owns"* sounds like a small inaccuracy and is in fact tree corruption, and the code makes that argument rather than the author. Gated by **`g_adopt_node`**, RED-proven against two mutations (no detach → the node lives in two places at once; allow cross-arena → no page). ⚠ The cross-document case is testable at all only because t717 gave `srcdoc` frames a real document |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A web component reaching its own host, and an outsider asking whether a root is closed** — `this.shadowRoot.host`, `root.mode`, and `el.shadowRoot === null` as the standard *"is this root closed / not mine?"* test | Every web-component library and every design system built on custom elements; and the agentic surface, where reasoning about a component's internals needs `mode` and `host` to be true | ✅ fixed (tick 736) — three of fifteen Chrome-measured shadow rows were wrong: `root.mode` was `undefined`, `root.host === el` was **false**, and a `closed` root was **returned** from `element.shadowRoot`. ⚠⚠ **`host` was wrong because it is TWO properties sharing one name**: URL decomposition (`hostname:port`) on an `<a>`, and the hosting ELEMENT on a ShadowRoot. They share a reflector surface, so the URL getter answered for both — **a wrong answer of the right type**, which is invisible to every check except one that knows what the answer should be. Resolved by node kind, shadow root first; the gate asserts `link.host === 'example.com:8080'` as the collision control and `mode === undefined` on non-roots as the over-reach control. ⚠⚠ Hiding `closed` roots **supersedes a deliberate earlier position** — *"hiding it is a follow-on and would only obscure the page from itself"* — which is right about secrecy (`closed` is an encapsulation contract, not a security boundary) and wrong about the **contract**: the property is observable and libraries branch on it, so answering with the root sends that branch down a path that works **here and nowhere else**. Gated by **`g_shadow_root_identity`**, RED-proven against two mutations. ⚠ Measured in the same run and NOT fixed, each its own change: `root.getElementById` is `undefined`, `activeElement` is absent from the root, a second `attachShadow` returns the existing root where Chrome throws `NotSupportedError`, and — the largest — **a composed event is not retargeted**: `event.target` on a `document` listener reads the inner node where Chrome reads the HOST, leaking shadow internals to every outside listener |

| pattern | where it shows up | status |
| --- | --- | --- |
| **`event.composedPath()` walked to decide where an event went** — `path[path.length - 1] === window` as the *"did this escape my root / is this node connected?"* test, and `path.includes(myRoot)` for delegation inside a component | Every event-delegation library and every component that listens above its own subtree — the pattern React's synthetic events, Floating UI's outside-click detection and most modal/dropdown "click away" handlers are built on | ✅ fixed (tick 737) — the path stopped at the `document`, one entry short, so `path[path.length-1] === window` answered **no** for every connected node. ⚠ **A condition, not an append**: Chrome-measured on three shapes — connected `t>BODY>HTML>document>window`, **detached `I`** (just itself), and in a fragment `U>#document-fragment`. Appending the window unconditionally would answer **yes** for a node in no document at all, which is worse than the bug. Gated by **`g_composed_path`**, RED-proven both directions. ⚠⚠ **And the gate nearly shipped vacuous**: the unconditional-append mutation PASSED, because the assertion was `has("det=I")` and the mutated output `det=I>window` **contains** it. A prefix is not a value; the assertion now matches through to the next field's name. *A `contains` check is only as strong as what cannot follow it* — and **a mutation that passes is more informative than one that fails**, because it tells you the assertion is not measuring what its message claims |

| pattern | where it shows up | status |
| --- | --- | --- |
| **An event crossing a shadow boundary** — a click inside a web component bubbling to a `document` listener that runs `event.target.closest('.item')`, or an outside-click handler asking `event.composedPath().includes(myRoot)` | Every page that hosts a web component and every component that emits events: the outside listener belongs to a different tree, so what `event.target` *means* is the whole question. Also the agentic surface — an agent reasoning about what was clicked must not be handed a node it cannot address | ✅ fixed (tick 739) — a listener outside the shadow tree read the node **inside** it (`in`) where Chrome reads the **host** (`h`), leaking the component's internals to every outside listener and breaking `event.target.closest(...)`, which searches a tree it is not in. Now Chrome-exact: outside sees the host, a listener **on the root** still sees the inner node, and the light-DOM control is unchanged. ⚠⚠ **Landed as ONE change with two halves, and the second is why t738 refused the "twelve-line" version**: this engine's `composedPath()` is *derived from `this.target`*, so retargeting alone silently hands an outside listener a **shorter** path (`h>BODY>HTML>document>window`) where Chrome gives the full `in>#document-fragment>h>BODY>HTML>document>window`. The path is now **captured at dispatch** and preferred by the accessor. The mutation that removes only the capture demonstrates the trade exactly — retargeting correct, path wrong — *one bug swapped for another, visible only to a listener that asks for the path*. Gated by two new assertions in **`g_shadow_root_identity`**, each RED-proven by its own mutation, with ten dispatch-dependent gates re-run green |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A component addressing its own subtree by id** — `this.shadowRoot.getElementById('label')` in every hand-written web component, and `template.content.getElementById(...)` in the compilers that address a template before cloning it | Every custom element that is not built on a framework's render function, and every template-based compiler. Also `document.createDocumentFragment()` builders, which address the fragment before inserting it | ✅ fixed (tick 740) — the call **threw**, and the reason was not the method: `doc_get_by_id` is already generic (it roots at `this_node` and walks descendants) and needed **no change**. It was **homeless**. `getElementById` is `NonElementParentNode` — Document and DocumentFragment (which every ShadowRoot is) and **not Element** — while this engine's surfaces are prototypes (`EventTarget → Node → Element → HTMLElement`, with `Document` off `Node`), so a shadow root got `Node.prototype` and putting the method there would have defined it **on every element in the document**. Fixed with a link, not a line: `DocumentFragment.prototype → Node.prototype` and `ShadowRoot.prototype → DocumentFragment.prototype`, the real spec hierarchy, carrying one member. Chrome-exact on all six probed rows. ⚠⚠ **The mutation that makes the case for the control**: putting it on `Node.prototype` instead makes **every positive assertion pass** (`tplWorks`, `fragWorks`, `shadowWorks`, `miss=null`) and is caught **only** by `element.getElementById === undefined`. A gate written to check *"does the feature work?"* would have shipped it. Gated by **`g_fragment_get_element_by_id`**, RED-proven against both, and asserting that the new link did not COST anything (`querySelector`/`addEventListener` still inherit — a prototype inserted in the wrong place shadows rather than extends) |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A component that attaches its shadow root twice** — a lifecycle callback firing more than once, an upgrade racing a manual init, a hot-reload re-running a constructor. And `attachShadow` on an element the spec forbids | Every custom-element codebase, because double-initialisation is one of the two classic component bugs — and it is normally *caught by the browser throwing*, which is how the author learns it happened at all | ✅ fixed (tick 741) — both spec throws were **silent successes**: a second `attachShadow` returned the existing root (documented as *"idempotent"*) and `<br>.attachShadow()` attached one. ⚠⚠ **"Idempotent" sounds harmless and is not**: handing back the first root lets the second initialiser overwrite the first one's content, and the symptom — *"my component renders empty sometimes"* — surfaces at the other end of the codebase from the cause. Now `NotSupportedError` in both cases, Chrome-exact. ⚠ **The exception NAME is part of the fix**: libraries `catch` and check `e.name === 'NotSupportedError'` before deciding whether to retry or re-use, so a `TypeError` is a different branch — the first version threw one and was measured against Chrome before landing. ⚠ The **over-strict** direction is guarded too: the spec's valid-host list is short and admits **any custom element** (the hyphen rule), so a check that allows only the HTML list rejects every `<my-widget>` on the web — the gate asserts a custom element succeeds and the mutation dropping the hyphen clause fails on exactly that. Gated in **`g_shadow_root_identity`**, RED-proven against two mutations. ⚠⚠ Same shape as t736's `closed` root: **when this engine is more permissive than the spec, the cost is not a wrong value — it is a bug the page can no longer detect** |

| pattern | where it shows up | status |
| --- | --- | --- |
| **The inline-SVG icon/illustration**: `<svg viewBox="0 0 24 24">` with no `width`/`height` attributes, sized by its container — plus every `<path>`, `<g>`, `<rect>` and `<circle>` inside it, and the nav-bar shape `<a style="display:flex"><span>Label</span><svg …/></a>` | Every icon set on the web (Heroicons, Feather, Material, Font Awesome's SVG build), every charting library's output, every logo shipped inline — and the `CLUSTERS.md` ledger ranks the symptoms at `geometry:<path>` **61 sites**, `geometry:<svg>` 49, `geometry:<g>` 37, `MISSING BOX:<path>` 34 | ✅ fixed (tick 742) — laid out at **100×25 for a `viewBox="0 0 100 25"`**: its own coordinate numbers read as pixels. ⚠⚠ **`viewBox` is an intrinsic RATIO, never an intrinsic SIZE** (SVG2 §8.2 + CSS-Images §5.3.2): `width:auto` fills the containing block and the height follows the ratio — Chrome-measured **400×100 in a 400px block and 250×63 in a 250px one**, and two container widths is what separates *"fills its container"* from *"a constant that matched"*. ⚠⚠ **The size arrived through a second channel behind a comment saying it would not**: the inline-svg raster cache is merged into `Page::images` for the painter, `apply_natural_sizes` reads that same map, and usvg's `Tree::size()` falls back to the viewBox — so the merge site's *"inline svgs are deliberately NOT natural-sized"* was true of the function beside it and false of the map. ⚠⚠ **A unit test asserted the correct 400×100 and passed — under `MinimalCascade`**, which never runs that pass; the shipping Stylo path did (the two-cascades trap). ⚠⚠ **The flex half could not be a separate tick**: a replaced element has no children, so the flex measure seam reported ZERO and an unsized `<canvas>`/`<video>` flex item was 0px wide *already* — a hole hidden behind the bug, because reading a 16-unit viewBox as pixels gave an icon a 16×16 box that looks right. Gated in **`g_svg_auto_sizing`**, RED-proven against three mutations. ⚠ Measured cost, not cleared: `www.ikea.com` reading-order 19 → 23 (`<span>` ⇄ `<svg>` in a flex `<a>`) against shape 51.43 → 51.72 and coverage 97.08 → 100.0 |

| pattern | where it shows up | status |
| --- | --- | --- |
| **The CSS reset applied to a table** — `* { padding: 0 }` (Tailwind's preflight, Normalize, every hand-rolled reset since 2004) on a page that also uses `<table>` for layout or for an infobox | Every Tailwind/Normalize site with a table, and table-based layout is still how Hacker News, Wikipedia's infoboxes and every documentation reference page are built. The `CLUSTERS.md` ledger ranked the symptom as the `dh=+2` band on `<tr>`/`<td>`/`<table>` — blog.rust-lang.org 165 hits, martinfowler.com 77, news.ycombinator.com 54 | ✅ fixed (tick 745) — the author's `padding: 0` **cascaded correctly to 0 and was then overwritten with 1px**, on all four sides, by a presentational hint running *after* the cascade. ⚠⚠⚠ **`0` IS `padding`'s initial value, so `padding == 0` cannot distinguish "the author reset it" from "nobody set it"** — it answered "nobody" for both. ⚠⚠⚠ **The hint had silently un-done a fix that already landed**: t556 restamped `UA_CSS` to `Origin::UserAgent` for exactly this class of rule (a reset is deliberately written with the weakest selector, so as an *author* sheet our `td{padding:1px}` (0,0,1) beat `*{padding:0}` (0,0,0) on specificity) — the cascade then produced 0 and a hint two hundred lines downstream wrote 1px over the answer. **A correct cascade is not the last word if something runs after it.** The `MinimalCascade` twin was never wrong, and not because it was written more carefully: it runs BEFORE author declarations, so the same code is a *default* there and an *override* here. Fixed by **deleting** the hint — the default already exists in the one place that can express it without guessing what the author did (`UA_CSS`'s `td, th { padding: 1px }`, where the origin sort decides). Net −3 lines of behaviour. ⚠ **Why 2px is not a rounding note**: a cell 2px too tall makes its ROW 2px too tall, and a row height is a `dy` term — every row below moves down 2px, the table by 2×rows, and every block after the table with it. Re-measured on the same fixture: **22 divergences → 6**, all six the pre-existing 1px text-advance width. Gated by **`an_author_padding_reset_on_a_table_cell_is_not_undone_by_the_ua_default`**, which asserts BOTH halves (three reset shapes compute 0 **and** an unstyled `<td>` still computes 1px), RED-proven against two mutations |


| pattern | where it shows up | status |
| --- | --- | --- |
| **A block inside an anchor** — `<a><div>…</div></a>`: the card link, the nav item, the vote arrow, every "make the whole tile clickable" wrapper | The dominant modern link idiom — it is the shape `<a>` was allowed block children *for*. Any page whose links wrap a styled block with vertical margins, which is every card grid, every list feed and every documentation index | ✅ fixed (tick 747) — the wrapper was too tall by the sum of its child's vertical margins (`margin:3px 0 6px` → **19px instead of 10**; Chrome `[0 3 1200×10]`, ours `[0 0 1200×19]`). CSS2 §9.2.1.1 splits an inline around a block child and puts the block in an **anonymous block box**, and the load-bearing property of an anonymous box is what it *has not* got — no margin, border or padding — so it is transparent to margin collapsing (§8.3.1) and the child's margins pass straight through. ⚠⚠ **One rule, two implementations, and the fourth sighting of that shape in thirty ticks**: `is_block_level` already blockified the `<a>` and every other layout decision asked it, while the two collapse predicates tested the RAW cascaded `display` and still answered "inline" — so the box became **opaque** to a collapse it should have been invisible to. Fixed by extracting the question once (`collapses_as_block`) and routing both predicates and all five call sites through it. ⚠ A wrapper's height is a `dy` term: **every sibling below it and every block after its container moves down**, so one wrapper charges all N boxes below it. Gated by **`a_block_inside_an_inline_collapses_its_margins_out`**, which asserts the escape **and** the eligibility half (real inline text before the block still keeps the top margin in — the text is the first in-flow content, and that is Chrome's answer too, so a fix that made every split inline transparent unconditionally would pass three of five assertions and be wrong), RED-proven against the restored `s.display == Display::Block` |


| pattern | where it shows up | status |
| --- | --- | --- |
| **A self-hosted webfont family with more than one face** — four `@font-face` blocks under one `font-family` name (regular / italic / 700 / 700-italic), with `src: url(../fonts/…)` relative to a stylesheet in a subdirectory | The default output of the "self-host your Google font" download (`google-webfonts-helper`, `@fontsource`) — verbatim on a11yproject.com — and the `assets/css` + `assets/fonts` layout every Jekyll, Hugo and webpack build emits | ✅ fixed (tick 748) — **only the FIRST face was ever fetched.** Idempotence was keyed on the **family**, so the first block registered `regular` and the other three hit `continue`: **every bold and every italic run on the page was measured and painted in the regular face**, and with no synthetic bold anywhere in `engine/text`, bold text had *byte-identical advances* to regular text. ⚠⚠ **The consumer was already built and could never fire**: `FontContext::face_id` searches the family's face list for the matching weight/style — *"picking the bold/italic variant when present"*, in its own comment — over a `Vec<ID>` that `register_named_font` `.extend`s, so the search was dead code and `ids.first()` was the only reachable path (the **orphaned-reader** shape: when a search over a collection is written, check that anything ever puts a second element in it). The guard's *purpose* was real — this function re-runs after every round of dynamic scripts and a new face forces a full-document relayout — so the fix keeps it and changes only the grain: a `(family, first-src)` claim, which is stable across re-runs (identical idempotence) and distinct per face. ⚠ **Second defect, same block**: a relative `src` resolved against the **document** instead of the **stylesheet** (CSS Values §4.2) while the loop was *holding* the sheet's URL. `/css/screen.min.css` + `url(../fonts/x.woff2)` agrees by coincidence; `/assets/css/main.css` gives `/fonts/x.woff2` → **404**, and a webfont that 404s does not announce itself, it looks like a page in a different font. Gated by **`g_webfont_family_weights`**, which serves the sheet from a subdirectory and **404s `/fonts/*`** so defect B cannot pass by accident, and compares the bold span against a control reaching the same bytes through a single-face family (expected value built from the fixture, not a magic constant); RED-proven against both mutations |

| pattern | where it shows up | status |
| --- | --- | --- |
| **The modern system-font stack** — `system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif` and its variants, including Tailwind's `ui-sans-serif, system-ui, sans-serif` | The default body font of essentially every site built since ~2016: Bootstrap 4 and 5, Tailwind, GitHub, and every design system that copied one of them. It is the font the *body text* of the page is set in, so it decides every line box on the page | ✅ fixed (tick 749) — `system-ui`, `ui-sans-serif`, `-apple-system` and `BlinkMacSystemFont` all shared one match arm with `sans-serif` and returned the sans generic. **Two bugs in one line.** (a) `system-ui` is NOT `sans-serif`: our `sans-serif` deliberately resolves to Arial→Liberation Sans because that is the family *Chrome itself* asks for, while `system-ui` is the platform UI font — `fc-match system-ui` answers **Noto Sans** and Chromium measures exactly that. `LineMetrics::height`'s own verification table already had both numbers side by side (Liberation **18**, Noto **22**), so **every line on a `system-ui` page was 4px short**, and a line height is a `dy` term that charges every line below it too. (b) ⚠⚠⚠ **THE SHORT-CIRCUIT, which is why all four real stacks were wrong**: every one of those names is written FIRST in a stack, so returning a generic there **ended the search** and the family Chrome actually picks was never reached — measured 16px `"source"`, all four stacks gave us `48x18` where Chrome gave Bootstrap 5 / GitHub / Tailwind `50.23x22` (Noto Sans, reached at the **4th** entry on GitHub) and Bootstrap 4 `48.34x19` (**Roboto**). ⚠ `-apple-system`/`BlinkMacSystemFont` are Blink's **macOS-only** aliases for San Francisco; they name nothing on Linux and Chrome treats them as unknown and moves on, which is exactly why Bootstrap 4 lands on Roboto — so they now get no arm and fall through. All five stacks Chrome-exact after the fix. ⚠ Two hypotheses were REFUTED on the way (the low-shape pages' CSS never arriving — 0/24 under control; and a systematic advance error — ours is 317 vs Chrome's 316.61 over 43 characters), and the bug was in the one fixture row that had not been predicted. Gated by **`system_ui_is_not_the_sans_generic_and_the_macos_aliases_do_not_short_circuit`**, which asserts the measured `line-height: normal` difference and not merely the identity, RED-proven against two mutations. RESIDUE, named: an unmatched stack still falls back to sans where Chrome uses its **standard font** (Times→Liberation Serif) — a different primitive, untouched |

| pattern | where it shows up | status |
| --- | --- | --- |
| **CSS nesting** — `&:hover`, `&.active`, `&:not(...)`, `& > child`, and the bare `&` reset idiom, plus implicit nesting (`.child { }` written directly inside another rule) | Baseline 2023 and now the default authoring style: hand-written CSS, design systems, and the output of every modern preprocessor pipeline. **41% of the corpus uses it in inline `<style>` blocks alone** (measured t659; external sheets were not even scanned, so that is a FLOOR) | ✅ fixed (tick 757) — every `&` in every stylesheet was matching **`<html>`**. The rules were indexed (t659 taught the walk to recurse) but their selectors were indexed VERBATIM, and a verbatim `&` is `Component::ParentSelector`, which the matcher resolves as `scope_element` if set and **`element.is_root()`** if not — and we never set one. ⚠⚠ **It did not fail as "nested rules are dropped", which is why it survived**: the DESCENDANT form matched *by accident* (`<html>` is an ancestor of everything), so `& .child` applied **document-wide** while contributing **no specificity** from `&` — so `#other { & .leak {width:500px} }` with a later `.leak {width:100px}` matched and then LOST the tie, measuring 100 where Chrome says 500. Over-matching, under-specified, and right often enough to look fine. Measured vs live Chromium: bare `&` **300 vs 50**, `&:not(.x)` **260 vs 40**, `& > span` **240 vs 73**, the leak case **500 vs 100**. Fixed the way Stylo's own `stylist` does it — `replace_parent_selector` **before** indexing, which corrects matching, specificity and the index KEY together — with the RESOLVED list threaded into the recursion so nesting composes. All seven measurements Chrome-exact. Gated by **`a_nested_rules_ampersand_resolves_to_the_enclosing_selector_not_the_root`**, whose load-bearing assertion is that `& .leak` does NOT leak outside `#other` (a fix that merely made `&` match anything would pass every other assertion), RED-proven against the substitution being removed. ⚠ Named residue, recorded in `CONSTELLATION.tsv` rather than as a comment: `PseudoIndex` never recurses into nested style rules at all, so a `::before` declared inside a nested block is never collected — a different defect with a different fix |

| pattern | where it shows up | status |
| --- | --- | --- |
| **`&nbsp;` as content** — the spacer cell `<td>&nbsp;</td>`, the `&nbsp;&#124;&nbsp;` separator, `10&nbsp;km`, French punctuation, and the "stop this from collapsing" idiom that predates flexbox | One of the most common constructs in hand-written HTML, and universal in CMS output, legacy templates and email-derived markup. It survives every redesign because it is written into the *content*, not the stylesheet | ✅ fixed (tick 759) — `&nbsp;` was collapsed and trimmed **like an ordinary space**, so an element whose only content was one had no text left, hence **no line box, hence height 0** (Chrome 18). ⚠ The cause is that `char::is_whitespace` implements the **Unicode `White_Space` property**, while CSS Text collapses exactly SPACE/TAB/LF/CR/FF — and the extra Unicode members are precisely the characters an author chooses *because they must not collapse* (U+00A0, U+2007 FIGURE SPACE, U+202F NARROW NO-BREAK, U+2000–U+200A). All three collapse sites in `engine/layout` used it. Measured vs live Chromium: `<div>&nbsp;</div>` **18 vs 0**, `a&nbsp;&nbsp;&nbsp;b` **48 vs 29** (a run collapsing to one), while `a   b` correctly stays 29 — both directions gated, because a fix that merely stopped collapsing would be a worse bug. A zero-height spacer is a `dy` term, so each one charges everything below it. Gated by **`a_non_breaking_space_is_content_not_collapsible_white_space`** with deliberately font-INDEPENDENT thresholds, RED-proven by restoring `ch.is_whitespace()` (the div reads 0, the exact corpus symptom). ⚠ Two residues found by the same fixture and recorded in `CONSTELLATION.tsv`: an **empty inline** wrongly generates a line box (Chrome 0, ours 19 — the opposite direction), and `<br>` line boxes are 1px tall too many |

| pattern | where it shows up | status |
| --- | --- | --- |
| **The empty wrapper element** — `<div><span></span></div>`: a component that rendered nothing this pass, a conditional slot with a falsy value, a template's `{{#if}}` branch, an icon placeholder, a clearfix leftover, an analytics/ad container before its script fills it | Ordinary output from every templating engine and component framework — React/Vue/Svelte all emit the wrapper and skip the children, and server-rendered templates leave the element behind rather than the branch. It is one of the most common shapes in real HTML *because* nobody writes it deliberately | ✅ fixed (tick 761) — the wrapper was **19px tall against Chrome's 0**. The strut (t690) is folded into EVERY line box unconditionally, so a line whose only member was an empty inline still got a full line height: a phantom line under every empty wrapper, and a `dy` term that charges every sibling below it. CSS 2.1 §9.4.2 treats such a line box as **not existing**. ⚠⚠ **The rule is about the LINE, not about the empty inline, and the blunt fix regresses the case the code was built for**: Chrome reports the span in `<div>text<span></span>text</div>` as **17px tall**, and fragment anchors, scroll-spy targets and `getBoundingClientRect` on a marker span read that box — which is precisely why `InlineItem::Spacer` carries it, with a Chrome citation in its comment. So the predicate is `any(content_bearing)` over the line's fragments, and the reporter fragments are still **EMITTED at zero height** rather than dropped, because dropping them takes the element out of `node_rects` and trades a placement error for a coverage one (the regression `LineFrag::report_h` already documents). ⚠⚠⚠ **The spec sentence is WIDER than Chrome, and only the measurement says so**: §9.4.2 exempts an inline with *"non-zero margins, padding or borders"*, but live Chromium gives **0** for `padding:4px 0`, **0** for `border-top:3px` and **0** for `margin-left:10px` — three of four rows have exactly what the clause exempts. Only an edge that occupies **inline flow width** (our `pad_l`/`pad_r` spacers, `padding:4px` → **18**) holds a line open. Writing the predicate from the sentence would have shipped three of four rows at 18 against Chrome's 0 — *more* spec-compliant and *less* correct, under a citation that reads as authority. Gated by **`a_line_box_with_only_empty_inlines_does_not_exist`**, which asserts the suppression, the anchor case that must survive, and all four narrowing rows; RED-proven in BOTH directions (`holds_line: true` on the empty-inline spacer reads **19.2**, the exact corpus symptom; `holds_line: false` on the padding edges reads **0** where Chrome says 18). ⚠ Residue, still `missing` in `CONSTELLATION.tsv`: the empty inline's OWN rect does not carry its padding (Chrome 25, ours 0), and t759's 1px `<br>` line-box difference |

| pattern | where it shows up | status |
| --- | --- | --- |
| **`display:flex; flex-direction:column; flex-wrap:wrap` on the page shell** — the sticky-footer idiom, written once in a design system's root class (`.hz-Page-body`, `.page`, `.layout-root`) and inherited by every page built on it | Stock design-system CSS. The `column` + `min-height:100vh` pair is the standard "push the footer to the bottom" recipe, and `flex-wrap:wrap` rides along in the same declaration block because it is harmless in Chrome. Anything using that shell — marktplaats.nl, repubblica.it in the CrUX sample — puts its **header, nav, entire page body and footer** in that one container | ✅ fixed (tick 762) — the four children were laid out **side by side**, each 1200px wide: `#page-wrapper` at **x=2201** against Chrome's x=0, and `h_overflow` **742**. `solve_subtree` passed `AvailableSpace::MinContent` for the height of any container whose own height was indefinite — but for a **column** container the block axis is the **MAIN** axis, and available main space is what `flex-wrap: wrap` breaks lines against (Flexbox §9.3.5). `MinContent` says *"be as short as you can"*, so **every item taller than nothing started a new flex line** and a vertical stack became N side-by-side columns of `1/N` the width. ⚠ **An indefinite main size is INFINITE available main space, not zero** — Chrome does not wrap here at all, and `min-height:100vh` only floors the result. Measured on the reduced fixture: `#c` Chrome `1200×1250` vs ours `1200×900`; the 900-tall child at Chrome `[0 200 1200×900]` vs ours `[400 0 400×900]`. ⚠⚠ **The narrowing is what makes it a fix rather than a blanket "never wrap"**: a column container with a DEFINITE height *must* still wrap (Chrome-verified: `height:300px` → two columns), and that path already passes `Definite(h)`; the CROSS axis keeps `MinContent` because for a `row` container the height does not decide line breaking. A control run with the change stashed moved exactly one of six cases — nowrap-column, definite-height-column, row-wrap and grid are all byte-identical. ⚠⚠⚠ **SHAPE is nearly BLIND to this class**: the score is parent-relative, so a whole document displaced by 2201px inside its container scores ~1pt worse while being unusable — the `h_overflow` jarring invariant (742 → **0** on marktplaats, 139 → **0** on repubblica) is the only channel in the instrument that sees it. Gated by **`auto_height_column_flex_does_not_wrap`**, RED-proven by restoring `MinContent` |

| pattern | where it shows up | status |
| --- | --- | --- |
| **The line-clamp card excerpt** — `display:-webkit-box; -webkit-box-orient:vertical; -webkit-line-clamp:N; overflow:hidden` on a title, teaser or product name | The only way to truncate multi-line text that works everywhere, so it is in every card grid, every news teaser, every product tile and every design system's `.line-clamp-2` utility (Tailwind ships it as a first-class class). `momon-ga.com` alone hit it 48 times | ✅ fixed (tick 763) — the clamp itself was implemented and *recovered* through the Stylo merge (stylo 0.19 gates `-webkit-line-clamp` to `engine="gecko"`), but **`display:-webkit-box` is gated by `#[cfg(feature = "gecko")]` in the same vendored file** (`values/specified/box.rs:474`), so the servo build **rejected the whole declaration** and the element kept its default `inline` — and the clamp only ever runs on a block. ⚠⚠ **Two halves of one behaviour, one of them shipped**: the recovery line for `line_clamp` sits four lines above where the display recovery now goes, and had been a dead letter on every site it was written for. Measured vs live Chromium (200px card, `16px/20px`): `-webkit-box`+`line-clamp:2` Chrome **200×40** vs ours 195×57; `-webkit-box` alone Chrome **200×60** vs 182×57; `-webkit-inline-box` Chrome **108×20** vs 108×17 — all three Chrome-exact after. `momon-ga.com` shape **0.509 → 0.565**, `marktplaats.nl` control identical. ⚠ The recovery copies a dedicated `legacy_webkit_box` MARKER, never `m.display`: copying the MinimalCascade's display wholesale would hand the shipping path the weaker cascade's opinion on every element (the two-cascades trap). The marker is cleared by any other recognised display value, so `display:-webkit-box;display:flex` computes `flex` (asserted). Gated by **`webkit_box_display_recovers_through_the_stylo_cascade`**, RED-proven by neutering the recovery (`Inline` where the assertion wants `Block`). ⚠ Residue in `CONSTELLATION.tsv`: the legacy flex-container half (`-webkit-box-orient:horizontal` → children in a ROW) is deliberately not built — the dominant idiom is text-only or vertical, and the old behaviour stacked them anyway |

| pattern | where it shows up | status |
| --- | --- | --- |
| **The RTL page** — `<html dir="rtl">` / `body{direction:rtl}` with an ordinary flex nav bar, toolbar, breadcrumb or card row inside it | Every Arabic, Hebrew, Persian and Urdu site — a large, permanently-growing slice of the CrUX tail. In the 200-site representative sample the two worst `reading_order` scores in the whole corpus are both RTL pages (`mobile.ir` 874, `ta3lemkonline.com` 817) | ✅ partly fixed (tick 764) — **every RTL flex row ran backwards.** `row` is a LOGICAL direction: its main axis is the inline axis, which under `direction:rtl` points right-to-left (CSS Flexbox §5.1). **Taffy has no `direction` property at all**, so the mapping must carry it — RTL swaps `row` ⇄ `row-reverse`, which is exactly Chrome's geometry. Measured (`<html dir=rtl>`, 600px row of three 100px items, x within the row): Chrome **500 / 400 / 300**, ours 0 / 100 / 200. Real sites: `mobile.ir` shape **0.174 → 0.320**, `h_overflow` **268 → 1**, `reading_order` 874 → 820 (coverage and `shape_n` unchanged); LTR control `marktplaats.nl` byte-identical. Gated by **`an_rtl_flex_row_runs_right_to_left`**, RED-proven by dropping the `rtl` argument (`[0, 100, 200]`). ⚠⚠ **The map claimed this was DONE**: the row `bidi (Arabic/Hebrew) … gated … G_BIDI_BASE` had a receipt for a gate that asserts only the paragraph base direction inside `engine/text` — surface audit #48 downgraded it to `partial` and split out what is missing. ⚠ **Still missing, measured and named rather than discovered later**: a block box's inline-START edge is the RIGHT one (Chrome puts a 600px body at x=600 in a 1200px viewport, we put it at 0); `ul`/`ol` UA padding is `padding-inline-start` (Chrome `<li>` x=0, ours x=40); and an RTL grid's column order does not reverse (taffy has no grid equivalent of the row swap). The TEXT half — shaping, intra-run reordering, mixed Arabic+Latin, two spans on one line — is Chrome-exact and was already right |

| pattern | where it shows up | status |
| --- | --- | --- |
| **The RTL data table** — a `<table>` of specs, prices, listings or fixtures inside `<html dir="rtl">` | The layout the Persian/Arabic/Hebrew web still runs on: classifieds, price comparison, sports fixtures, government forms. `mobile.ir` — the worst `reading_order` site in the 200-site CrUX sample — is one table-heavy page, and 250+ of its divergences are `<td>` x-errors | ✅ fixed (tick 765) — **every RTL table read backwards.** `direction` on the table box orders the COLUMNS, not just the text inside them (CSS 2.1 §17.5.3: the column axis follows the inline direction), so the first `<td>` in source order is the RIGHTMOST cell. Measured (`<html dir=rtl>`, 600px table of four 150px cells, x within the table): Chrome **450 / 300 / 150 / 0**, ours 0 / 150 / 300 / 450. Real site: `mobile.ir` shape **0.320 → 0.493** (+17.4 pts), `reading_order` **820 → 87**, coverage and `shape_n` unchanged; LTR control byte-identical. ⚠ The axis is read from the TABLE's own computed style, so a `<table style="direction:ltr">` inside an RTL page keeps LTR column order — Chrome agrees and the gate asserts it, which is what makes this a *direction* fix rather than a *reverse the cells* fix. Mirroring the whole colspan SPAN (not the first column) is what keeps `colspan` on the right cells. Gated by **`an_rtl_table_orders_its_columns_right_to_left`**, RED-proven by forcing `rtl_cols = false`. ⚠⚠ **A Chrome-exact fix was REVERTED to get here**: the RTL block-margin rule (§10.3.3) built earlier in the same tick matched Chromium on 7 of 8 fixture rows and still took `mobile.ir`'s `h_overflow` 1 → 16, deterministically — flush-right on a block whose containing block is already the wrong width points its content off-screen, where flush-left had hidden the same error. The mechanism oracle named the real #1 in one command. **When a spec-correct change makes a real page worse, it is nearly always ORDER** |

| pattern | where it shows up | status |
| --- | --- | --- |
| **The RTL card / menu grid** — `display:grid` inside `<html dir="rtl">`: a brand list, a category menu, a dashboard, a product wall | Grid is now the default way to lay a card wall out, and RTL sites use it exactly as LTR sites do. `mobile.ir`'s brand menus are grids; so is most of the modern Arabic/Persian/Hebrew web's chrome | ✅ fixed (tick 766) — **an RTL grid filled backwards.** `direction` reverses a grid's inline-axis track order (CSS Grid §3: the column axis IS the inline axis), so the first item belongs in the RIGHTMOST column. ⚠ **Taffy has no `direction` property and the `row` ⇄ `row-reverse` swap that fixed flex (t764) has no grid equivalent** — `grid-auto-flow` is not a direction — so the mirror is applied to the placed SLOTS on the way out, recursively, each against its own content box (padding and border subtracted), which works because `extract_placed` positions every subtree relative to its slot. Measured (`<html dir=rtl>`, 600px `1fr 1fr` grid): Chrome **300 / 0 / 300** for items 1–3, ours 0 / 300 / 0; a `direction:ltr` grid in the same page stays 0 / 100 in both engines. Real site: `mobile.ir` shape **0.493 → 0.523**, `reading_order` 87 → **75**. Gated by **`an_rtl_grid_orders_its_columns_right_to_left`**, RED-proven by forcing `grid_is_rtl → false`. ⚠⚠ This is the THIRD RTL axis-order primitive in three ticks (flex row t764, table columns t765, grid t766) and the mechanism was the same every time: **an axis the spec defines as LOGICAL arriving at an engine that only speaks physical**. Across the three, `mobile.ir` went shape **0.174 → 0.523**, `reading_order` **874 → 75**, `h_overflow` **268 → 1** |

| pattern | where it shows up | status |
| --- | --- | --- |
| **(measurement, not a page pattern) The threshold metric vs the slope** — `shape ≥ 0.75` pass-count as the Phase-0 M1 gate | Every burndown reading the loop takes | 📏 measured (tick 767) — five landed render fixes (762–766) moved the **headline 5.4% → 5.4%, +0.0 pts**, and over the 79 sites scored in BOTH sweeps they moved `h_overflow` **2295 → 1103 (−52%)**, `reading_order` **2073 → 1246 (−40%)** and `shape_mean` **41.0% → 43.1%**. `mobile.ir` alone went **0.174 → 0.523** — the largest single-site move recorded here — and contributed **zero** passes. ⚠ The naive corpus-wide `reading_order` drop (2925 → 1255) is inflated: **817 of it is `ta3lemkonline.com` leaving the scored set** (458 nodes at t758, `thin-overlap-1` at t767), which is the scorability regression the ledger flagged, not layout. ⚠ The one lost pass, `www.tz.de` 0.750263 → 0.741026, sat **0.0003 above the bar** and rendered 48 MORE nodes this run (`shape_n` 1902 → 1950) — a hair, and the third such crossing this corpus has produced. **The gate is the pass rate and must not be softened; the jarring totals and `shape_mean` are the slope. Report the gate first, then the slope, then the attribution** — a loop reading only the gate re-ranks away from a working seam, and one reading only the slope declares victory with 117 sites still failing |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A page that mutates the DOM thousands of times while booting** — MediaWiki's ResourceLoader, any framework hydrating a large document, every "render markup then upgrade it" boot | Wikipedia is the canonical case and it is a top-10 site; the same shape is every SPA hydration, every `innerHTML`-then-enhance template, every widget library that walks the tree at startup | ✅ fixed (tick 768) — **it segfaulted the process.** `en.wikipedia.org/wiki/Terrier` died with SIGSEGV in **6 of 8** `hittest` runs and **3 of 3** `render`/`boxes` runs. `record_mutation` **built a source string and COMPILED IT — once per mutated node** (`format!` + `evaluate_script`), so MediaWiki's boot drove ~4 million parse+bytecode+`JSScript` allocations and SpiderMonkey died inside its own frames. ⚠⚠ The guard that decides whether the call is needed — `if(globalThis.__recordMutation)` — **lived inside the text being compiled**, so it could only run after the cost it exists to avoid had been paid. Fixed by CALLING the function (`JS_CallFunctionName` with a rooted argument vector) instead of compiling a program that calls it: **8 of 8 clean**. Four controls ran before any hypothesis was believed: not a regression (the pre-t762 engine crashes 6/6 on the same page — the PAGE changed, not the engine), not stack exhaustion (8× stack, 4/4 crash), not t766's new recursion (disabled, 5/6 crash), and stripping the page's scripts is 4/4 clean. Bisecting the five scripts: no single script, pair or triple crashes — only all four together, i.e. the whole ResourceLoader boot. ⚠ A first fix attempt failed and is recorded: a Rust-side early-out written from the function's doc comment (*"a no-op if MutationObserver was never touched"*) never fired, because `WINDOW_PRELUDE` installs `__recordMutation` unconditionally — **a guard written from a comment rather than from the code is a guard on a condition that cannot occur** |

| pattern | where it shows up | status |
| --- | --- | --- |
| **Measure-in-a-loop** — `getBoundingClientRect()` / `getClientRects()` per element per frame, and `dispatchEvent()` per interaction | Every scroll handler, sticky header, `IntersectionObserver` polyfill, drag-and-drop library, virtualised list and animation library on the web. It is the single most-called DOM method there is | ✅ fixed (tick 769) — all three were **invoking the JS compiler per call**. `getBoundingClientRect` compiled an eight-field object literal (20,000 calls: **131ms → 13ms**), `getClientRects` compiled a whole IIFE with an `item()` closure (**354ms → 16ms**, the slowest of the three), `dispatchEvent` compiled `__dispatchEvent(id, __pendingEvent)` per event, and `getBBox` had the same literal shape. This is the class behind tick 768's Wikipedia SIGSEGV, found by the grep that tick's PATTERN line called for. Two mechanisms: build the object **natively** (`JS_NewObject` + `JS_DefineProperty`), and for anything with behaviour compile the helper **once** into the window prelude and **call** it. Gated by **`G_HOT_DOM_NO_COMPILE`**. ⚠⚠ **The first version of that gate could not go red**: written as an absolute budget (6,000 calls under 1500ms) it passed at 11ms clean *and 35ms with the defect restored* — a budget loose enough not to flake is loose enough to prove nothing. Rebuilt to measure each hot call against `element.tagName` in the same loop and assert the RATIO: **65.5× with the compile, 7.0× without** (limit 15×). **A perf gate must name the control it divides by** |

| pattern | where it shows up | status |
| --- | --- | --- |
| **The pre-flexbox column** — `*{box-sizing:border-box}` plus `.card{width:50%;float:left;padding:0 5px}` | The WordPress card grid, and the layout of most of the pre-2017 web that is still online: every theme, every legacy CMS, every hand-written two/three/four-column page. `*{box-sizing:border-box}` is in every CSS reset written since 2011, so the pair is nearly universal on those pages | ✅ fixed (tick 770) — **`box-sizing: border-box` was never applied to a FLOAT.** `layout_block` has subtracted padding+border from a border-box width for many ticks (`bs_extra_w`); `layout_float` is a *separate width resolution* and never learned it, so it took the specified `50%` and used it as the CONTENT width. Every floated column on every border-box page was `padding-left + padding-right` too wide, and the next float was pushed by the same amount. Measured (704px container): Chrome float **352** border box / **342** content, ours 362 / 352 — and **the same box without `float`, in the same fixture, was already Chrome-exact**, which is what names it a float bug rather than a box-sizing bug. Real site: `possssno.sbs` (coverage 1.000, shape 0.123 — the sharpest target on the t767 ledger) went shape **0.123 → 0.430**; LTR control byte-identical. Gated by **`box_sizing_border_box_applies_to_a_float`**, RED-proven by dropping the arm (`float 0/362, inner 5/352`). ⚠ Two hypotheses died by fixture first: a blockified `<a>` ignoring its parent's padding (Chrome-exact, falsified) and an `<img width=352>` attribute driving the card width (the sheet sets `width:100%`). ⚠⚠ **The forgotten copy is never the main path — it is the VARIANT** (the float, the flex item, the table cell), written once for its special case and never revisited as ordinary properties land in the main path. Any function that resolves `s.width` itself owes every width-modifying property |

| pattern | where it shows up | status |
| --- | --- | --- |
| **Work in a document that is not the live one** — `document.implementation.createHTMLDocument('')` or `new DOMParser().parseFromString(html,'text/html')`, then call `doc.createRange()` / `doc.createNodeIterator()` / `doc.createEvent()` ON THAT DOCUMENT | Sanitisers, template engines, RSS/Atom readers, "parse this markup before inserting it" helpers, and Google's CSE `dynamic.js`, which is where this was observed live | ✅ fixed (tick 776) — **the second document had no methods.** Nineteen `document` members were OWN properties of the singleton, because every JS-side shim was written `document.createRange = …`. `Document.prototype` is real here (`__protoDocument`, built in Rust) and every document genuinely inherits from it — the methods were simply not on it, so `createRange` / `createNodeIterator` / `createTreeWalker` / `createAttribute` / `createAttributeNS` / `createEvent` / `evaluate` were a **`TypeError`, not a `false`**, for every document but one. Live evidence: `TypeError: b.createRange is not a function` in the t776 CrUX sweep log; plus every `Document.prototype.X = wrapper` patch was a silent no-op (the `G_PROTOTYPE` failure one interface over). Gated by **`G_DOC_PROTOTYPE`**, RED-proven three ways. ⚠⚠ **The gate that owned this ground passed, and the reason is the transferable finding**: `G_SECOND_DOCUMENT_IS_REAL` asserts `document.createNodeIterator.call(b.ownerDocument || b, …)` — faithfully transcribed from DOMPurify, which destructures the method off the ORIGINAL document and supplies only the receiver. That takes the function from the singleton, so it exercises the ALGORITHM over a second document and never performs the LOOKUP on one; it passes for exactly as long as `otherDoc.createNodeIterator` is `undefined`. **DOMPurify was therefore never broken by this** — the tempting version of this tick's story, checked against the shipped bundle and refuted. ⚠ The over-broad fix is worse than the throw: promoting the closures without honouring `this` makes them operate on the WRONG document (a range built from an inert parsed copy pointing into the live page), and it passes every "did it return a Range" check — which is why the load-bearing claims are the OWNERSHIP ones. ⚠ The prelude's plain-object `createTreeWalker` fallback had to move too: left on the singleton it would have SHADOWED the real walker for the main document — **a promotion that fixes the second document by regressing the first is not a fix** |

| pattern | where it shows up | status |
| --- | --- | --- |
| **Listen for a rotation** — `screen.orientation.addEventListener('change', …)`, guarded by nothing more than `if (screen.orientation)` | Every mobile-responsive bundle: video players deciding when to go fullscreen, maps re-projecting, carousels and virtualised lists re-measuring, PWAs re-laying-out. The CrUX tail this corpus is stratified over is mobile-heavy, and this is the standard idiom in it | ✅ fixed (tick 777) — **`screen.orientation` was the object literal `{ type: 'landscape-primary', angle: 0 }`.** `ScreenOrientation` is an `EventTarget` with `lock`/`unlock`/`onchange`; five of its six members were absent, so the detect passed (`if (screen.orientation)` is enthusiastically true about a two-field literal) and the very next line was `TypeError: screen.orientation.addEventListener is not a function`. Tick 772's half-installed `performance` one object over. ⚠⚠ **Neither instrument built after 772 could have found it**, and that is the transferable part: 772's own follow-up rule was *"grep the prelude for objects whose methods were added one at a time"* — this one was written complete-looking in a single line, in one sitting — and 773's re-probe of **262 platform globals** ranks by ABSENT top-level names, while `screen` and `screen.orientation` were both present. **A probe over NAMES cannot find a hole INSIDE an object it can reach.** Two more defects in the same four lines: `type` was the **constant** `'landscape-primary'` on every viewport including portrait ones (a wrong answer of the right type — now a getter over the live `innerWidth`/`innerHeight`, asserted against the independent Stylo answer to `matchMedia('(orientation: portrait)')`, because a plausible constant can only be caught by asking the question a second way); and `lock()` now **REJECTS with `NotSupportedError`** — desktop Chrome's own answer, which routes the caller into the `.catch()` it wrote, where an absent `lock` throws a synchronous `TypeError` out of a call the author expected to be thenable. Also fixed in the same family: `Screen`, `History`, `Location` and `VisualViewport` were **inert stubs of objects the engine builds on every page**, so `location instanceof Location` answered `false` about `window.location` itself — the defect tick 773 fixed for `CSSStyleRule` and went past, because it ranked by absent names and these were present-but-lying. Gated by **`G_SCREEN_ORIENTATION`**, RED-proven three ways, the third being the subtle one: `dispatchEvent` demoted to the `return true` stub `navigator.connection` still uses leaves `missing:none` passing and only `listen:3 → 0` catches it |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A component constructor assigning its own state** — `class W extends HTMLElement { constructor(){ super(); this.index = 0; this.options = {...}; this.mode = 'dark'; } }` | Every web-component framework and every hand-written custom element on the web: Lit, Stencil, FAST, Salesforce LWC, and the vanilla `class extends HTMLElement` that Google's own products ship. `index`/`options`/`mode` are among the most ordinary field names a widget picks | ✅ fixed (tick 778) — **six readonly IDL attributes were installed getter-only on `Node.prototype`, so an ordinary expando assignment threw and took the constructor with it.** `index`, `options`, `selectedOptions`, `mode`, `origin` and `wholeText` are readonly on exactly ONE interface each (`HTMLOptionElement`, `HTMLSelectElement`, `ShadowRoot`, `HTMLAnchorElement`, `Text`) and were sitting on the tier every element inherits. On a `<my-widget>` none of those names is in the prototype chain at all in a real browser, so `this.index = 0` just makes an own property; here it found an inherited accessor with no setter — and **a `class` body is always strict** — so it threw `TypeError: setting getter-only property "index"` out of the constructor, *before the element existed*. Live evidence: **18 of that exact message on `meet.google.com`** in the t777 CrUX sweep, 17 tagged `custom element ctor` / `attributeChangedCallback`; the site scored shape **0.126**. Fixed with a shared `expando_unless_owner` setter: on the owning interface the write is the platform's readonly no-op, everywhere else it becomes an ordinary own data property. ⚠ **The careless version trades a throw for a lie** — making all six plainly writable would let `option.index = 99` stick, so `G_EXPANDO_READONLY` asserts BOTH halves (the expando lands on a `<div>`, *and* `option.index` still reports its position, `a.origin` still reports the URL's origin, `shadowRoot.mode` is still `open`). ⚠ Accepted divergence, stated not discovered: a native accessor cannot see caller strictness, so readonly here means *ignored* rather than *throws in strict mode* — Chrome ignores sloppy and throws strict. ⚠⚠ **THIS IS A WRITE-ONLY DEFECT, AND THAT IS WHY NOTHING SAW IT.** `G_PROTOTYPE`, `G_IFACE_SURFACE` and the 262-name census all confirm `index` exists and reads correctly — and every gate in this repo READS. A property has two access shapes and the entire gate corpus exercises one; when a surface has more than one mode of use, a suite covering one mode reports full coverage of the surface |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A capability probe on the boot path** — `navigator.permissions.query({name:'…'})`, usually with no `.catch`, because in a real browser a known name does not reject | Consent managers, media players negotiating camera/mic/speaker, PWA install flows, clipboard-backed copy/paste buttons, and analytics fingerprinting the permission surface. It runs during boot, inside an `async` function, which is where an unhandled rejection is fatal to whatever was rendering | ✅ fixed (tick 779) — **seven names Chrome supports rejected here**, so an ordinary probe became an unhandled promise rejection inside a boot chain: `display-capture`, `background-fetch`, `periodic-background-sync`, `bluetooth`, `nfc`, `speaker-selection`, `top-level-storage-access`. They resolve `denied` now — a state Chrome itself returns, and NOT a capability claim (a page detects Web Bluetooth with `navigator.bluetooth`, never with `permissions.query`). Also: `clipboard-read` answered `denied` while `readText` genuinely pulls the real OS clipboard — a "no" stub that became a lie when the capability landed — now `granted`, and deliberately **not** `prompt`, because the table's own quoted rule is that `prompt` promises a dialog nothing here can show. ⚠⚠ **CONTROL BEFORE BLAME, and it nearly went the other way**: this was found from `trivago.de`'s 26 unhandled rejections reading *"'speaker' is not a valid enum value"*, with the top stack frame **in our own prelude** — but `speaker` is not valid in Chrome either (dropped for `speaker-selection`), so Chrome rejects it too and the site's own code is at fault. trivago's blank render is **load-budget starvation** (25.7s vs Chrome's 5.1s, budget exhausted 5×), a perf cause wearing an API cause's clothes. `G_PERMISSION_ENUM` is RED-proven two ways, and the second is load-bearing: accepting *every* name — the repair that makes the message disappear — is caught by a control asserting that `speaker` must still reject |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A responsive rule written with CSS Nesting** — `article { max-width: 423px; @media (min-width:1018px) { max-width: 974px } }`, an at-rule nested INSIDE a style rule rather than wrapping one | Every stylesheet authored after ~2023, and everything compiled from Sass/Less/Tailwind's `@apply`-era output, which has emitted this shape natively since browsers took it. Breakpoint overrides are the single most common thing anyone nests — a component's own responsive branch, written next to the property it overrides instead of in a media block at the bottom of the file | ✅ fixed (tick 785) — **the declarations inside a nested group rule were dropped whole.** CSS Nesting has two halves: a nested *style* rule (`& .c {}`) is a `CssRule::Style` with its own selectors, which t659 taught the rule-index walk to recurse into; declarations written *directly* inside a nested `@media`/`@supports`/`@container` have no selectors, so the spec wraps them in an implicit `& { … }` and Stylo materialises that as its own variant, **`CssRule::NestedDeclarations`** — a block and nothing else. The walker had no arm for it and a trailing `_ => {}`. **The rule that owns a selector survived; the one that borrows its parent's did not**, silently, on a page that still rendered. Live: `secure5.entertimeonline.com` lays its `<article>` out at Chrome's 487px (423 + 2×32) and ours at 1134px (974 + 2×80) — the whole content column — with the oracle's #1 cause on that site reading `displaced: x ~256px` on the *descendants*, not a width error on the parent. Shape **0.692 → 0.795**, crossing the M1 bar; `blog.rust-lang.org` unchanged to the decimal (1664 paths). Gated by **`G_CSS_NESTING`** (extended), RED-proven by restoring the empty arm. ⚠ **The careless fix is worse than the bug**: applying nested declarations *unconditionally* is identical to the correct fix on every matching query and differs only on a non-matching one, so the gate asserts a nested `@media (min-width:5000px)` does NOT apply on an 800px page. ⚠⚠ **A container's wrong width is a CASCADE question before it is a layout one.** The burndown ranks width-error-launders-into-`dy` as mechanism #1 and every prior attempt went looking for a sizing primitive; this box was the wrong width because a declaration that would have sized it never entered the cascade, and the evidence was not in the boxes at all — it was in the four lines of CSS the site served, one `curl` away |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A form the author did not size** — `<input>` with no CSS width, `<textarea>` with `rows`, a `<select>` in a footer | Search boxes, login forms, newsletter signups, comment boxes, contact forms, language pickers — the whole long tail of the web that styles its page and leaves its controls to the browser. A control's intrinsic size is the BROWSER's arithmetic: nothing on the page says how wide a search box should be | ✅ fixed (tick 787) — **three defects in one table, all found by asking Chrome for the numbers.** (1) **A control does not inherit the page's font**: Chrome gives every control `font: -webkit-small-control` (~13.3px system face), we inherited the document's 16px, so every control was ~20% too big in both axes on every page that sets a body font. (2) **The `<input>` width intercept was 26px short** — slope is exactly 8.0px/char in both engines, the constant is 45px border box and ours was 19; ⚠ the comment shipping with the old constant claimed `size=20 → ~173px` was *"the same approximation Chrome's own default ends up at"* and **Chrome ends up at 205**. (3) **`rows` was never read**, so an empty `<textarea>` sized to its empty content — one line, 22px, against Chrome's 36 — on every comment form on the web. ⚠ **One shared constant was wrong for one of them**: `<input>`'s intercept is 45px border box and `<textarea>`'s is 22, because a text field reserves caret-scroll room a textarea does not; the old code used one number for both and was 3px off for one and 26px off for the other simultaneously. Both terms are now `font-size`-relative (checked at 32px: within 2–5%, against ~135% for a fixed constant). Gated by **`G_FORM_CONTROL_METRICS`**, RED-proven twice. ⚠ Residual, measured and named: a `<select>`'s intrinsic width is short by **exactly 17px** (142 vs 159 long option, 13 vs 30 one-char — the same 17 either way, the dropdown arrow Chrome reserves), and `<input>` heights read 19 against 21. ⚠⚠ **A calibration constant whose comment claims it matches the reference is the easiest unmeasured claim to keep**: it reads as evidence, never throws, and is wrong by an amount nobody can see without running the reference. Any number here that claims Chrome parity should carry the command that produced it |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A `<select>` the author did not size** — a language picker, a country list, a filter dropdown, sized by the browser from its selected option | Every footer language switcher, every checkout country field, every legacy form. And its complement: the modern design-system select, `appearance: none` plus a custom SVG arrow, which is most of the restyled web | ✅ fixed (tick 789) — **the dropdown arrow was never reserved.** A select sizes to its selected option and every engine then adds the widget strip: Chrome 159 vs our 142 with a long option, 30 vs our 13 with a one-character one — **the same 17px either way**, which is what identifies it as a reserved slot rather than a text-measurement difference (a font metric scales with the text). ⚠ **It could not be reserved unconditionally**: `appearance: none` takes the native widget off and Chrome drops to 139 on the same text, so an unconditional constant fixes the classic select and newly breaks every restyled one — a TRADE, refused. The property had to be read first, and `clone_appearance()` is `engine="gecko"` in stylo 0.19 (compile-probed), so it is recovered from `MinimalCascade` and merged in `stylo_engine`, the same fence as `scrollbar-width`. Gated by **`G_FORM_CONTROL_METRICS`** (extended), RED-proven twice — delete the guard and the relation fails at 159 vs 159, return 0.0 and `#s1` fails at 142. ⚠ The arrow is RESERVED, not PAINTED (this engine draws no native widget): a deliberate Bar-2 gap, because the box is what every sibling is laid out against. ⚠⚠ **`G_APPEARANCE_NONE` had measured this property as worth nothing to read, and was right at the time** — its claim was about the VISUAL surface, and the new reader is geometric. A capability correctly priced at zero can acquire a value when a different subsystem starts asking. ⚠⚠ The site that motivated the lead (`chat.google.com`, select ours 236 vs Chrome 162) did NOT move: its `<form>` and wrapper `<div>` are also exactly 236 vs 162, so the select fills an ancestor we size wrong and never sizes itself — **a cluster row keyed by the tag it manifests on names the victim, not the culprit** |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A page that puts its framework in a cascade layer** — `@layer reset, theme;` at the top of the sheet, the vendor's CSS inside a layer, the page's own rules unlayered | Every design system shipped since 2022, Tailwind's `@layer base/components/utilities`, Bootstrap 5.3+, and any codebase that got tired of `!important` wars. The idiom is ALWAYS the statement form at the top of the file, because that is what lets the blocks below arrive in any order | ✅ fixed (tick 790) — **a layer exists to LOSE, and ours won.** Layers were flattened into document order, so `#h{width:100px}` followed by `@layer L{#h{width:333px}}` read Chrome **100** and ours **333** — the framework's styles beating the page's own, which is the exact outcome the author moved them into a layer to prevent. A layer rank now sits in the winner sort between ORIGIN and SPECIFICITY: unlayered takes the top rank, layers count up in declaration order. ⚠ **The `@layer a, b;` STATEMENT form is the load-bearing half** — it fixes the order before either block exists, so an engine ranking layers by first BLOCK reads the common idiom backwards (300 vs 111 in the gate). ⚠ **And "layers lose" must not become "layers are ignored"**: a declaration existing ONLY in a layer still applies, which is the half a fix aimed at the first symptom alone would break — asserted separately. Gated by **`G_CASCADE_LAYERS`** (five Chrome-measured cases), RED-proven two ways. ⚠ Residue, named: the PSEUDO-element index carries no layer rank, and `!important` does not yet REVERSE layer order as the spec requires. ⚠⚠ Found by AUDIT #50 while probing something else — the nested-`@layer` case of the t785 nesting fix. **A capability's neighbour is the cheapest place to find the next defect: the fixture was already open** |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A URL, file path or breadcrumb in body text** — `https://example.com/very/long/path`, `src/main.rs`, `Home / Products / Widgets` | Documentation, changelogs, forum posts, footers, API references, anything that prints a path. It is the single most common "long token" on the web | ✅ fixed (tick 791) — **we broke the line after `/` and Chrome does not.** UAX #14 offers the opportunity (class SY, only member U+002F) and `unicode-linebreak` reports it faithfully; Blink tailors it away, so a long URL overflows its box in Chrome instead of wrapping. Measured at 120px: `aaaa/bbbb/cccc/dddd` Chrome 19px tall, ours 38; a real URL Chrome 19, ours 77. ⚠ **The row that stops this being read as "Chrome wraps less"**: `one/two three/four five/six seven/eight` takes Chrome FOUR lines against our three — refusing the opportunity moves a whole token down, so the error is a different set of line boxes, not a bias. Every other separator already agreed (`- . _ ? = & , : +`, numeric dates, CJK, soft hyphens, U+200B), making this a one-character tailoring. Real sites: `en.wikipedia.org` shape 53.3% → 53.8%, four controls byte-identical. Gated by **`G_LINE_BREAK_SOLIDUS`**, RED-proven two ways — and the second is the guard: *widening* the rule to hyphens (the plausible "stop breaking inside words" version) fails while every solidus case passes. ⚠ `overflow-wrap: break-word` is a different path and still breaks the same URL. ⚠⚠ **Found by a broad differential probe whose FIRST half was a negative result**: ten flex/grid/overflow cases came back Chrome-exact, retiring the whole class the h-overflow metric points at, and the next fixture over the most boring input imaginable found this |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A right-floated box inside a fixed-width column** — `float:right` on a thumbnail, an infobox, a pull-quote, a "read more" link, inside a `width:300px` sidebar or a wrapper `div` | The entire pre-flexbox web, which is most of what is still online: every WordPress theme, every wiki, every forum, every documentation template. Wikipedia's articles are built out of it | ✅ fixed (tick 792) — **the float hugged the VIEWPORT's edge, not its own block's.** A float participates in its nearest BFC (so exclusion bands are shared across nested plain blocks, correctly), but CSS 2.1 §9.5.1 rules 1–2 pin it to its own CONTAINING BLOCK. We conflated them: a `float:right` 50px box in a `width:300px` div read Chrome **x=250** and ours **x=1150**. A 900px miss is never one wrong box — it spawns overlap and reading-order violations across everything the float was meant to sit beside. `en.wikipedia.org` shape **53.8% → 58.8%**; three other sites byte-identical. Gated by **`G_FLOAT_CONTAINING_BLOCK`** (seven Chrome-measured x positions), RED-proven two ways. ⚠ **The first draft clamped BOTH edges and would have traded a 900px error for a 100px one**: Chrome puts a 400px right float in a 300px block at **x = −100**, right edge pinned, overflowing LEFT. Only the hugged edge is clamped, and −100 is asserted so the plausible version cannot return. ⚠ Residue, measured in the same fixture: a BFC root must not overlap preceding floats (Chrome moves an `overflow:hidden` block down to clear them; we do not), so the gate asserts x only. ⚠⚠ **The fixture that finds the bug must also contain the case that constrains the fix** — this and the t789 `<select>` arrow both produced a reasonable, wrong first draft that a gate built only from the failing case would have passed |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A responsive layout that rearranges blocks with `order`** — `order:-1` to pull the hero image above the copy on mobile, `order:2` to send the sidebar after the article on desktop | Every design system's breakpoint CSS, every "mobile-first then reorder" template, and the standard way to change visual sequence without touching markup (which is precisely why the spec warns against carrying meaning in it) | ✅ fixed (tick 793) — **`order` was ignored, so every such layout rendered in source sequence.** Flexbox §5.4 / Grid §6.3 lay items out in *order-modified document order*; taffy has no `order` field, so the sort has to happen where items are collected, and it did not. ⚠ **This is a READING-ORDER defect, not a property that degrades quietly**: reading-order is scored over sibling PAIRS, so one `order` flips every comparison across the reordered group at once — and it is the jarring dimension this corpus is worst at (14.5% of in-scope sites clean at t786). Twelve Chrome-measured positions now exact, flex AND grid through the same collection. ⚠ **The tie is the whole specification of the sort**: equal `order` (every item on most pages — the initial value is 0) must keep DOCUMENT order, so the sort is STABLE and is skipped entirely when no item carries a non-zero order; an unstable sort would shuffle ordinary flex rows on every page, a far worse bug than the one being fixed. ⚠ **And the DOM must not move** — `order` is visual only, so the a11y tree and tab order keep source order; the gate asserts that alongside the boxes, because an engine that reordered the tree would pass every box assertion while rewriting what a screen reader announces. Gated by **`G_FLEX_ORDER`**, RED-proven two ways. ⚠⚠ Found by the SAME thirteen-case positioning probe as t792's float bug: one Chrome run, two defects, eleven boring rows — **a probe wide enough to be mostly boring is what makes the interesting rows visible** |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A row of text-bearing `inline-block`s** — nav items, tabs, chips, tags, badges, pill buttons, inline lists, footer link rows | Essentially every modern page. `display:inline-block` with text inside is the default way to lay a horizontal row of labelled things, and it predates and outlives flexbox | ✅ fixed (tick 795) — **the box sat entirely above the line's baseline.** CSS 2.1 §10.8.1 gives an `inline-block` the baseline of its LAST IN-FLOW LINE BOX, falling back to the bottom margin edge only when it has no in-flow line boxes or `overflow` is not `visible`. We implemented only the fallback, so every text-bearing inline-block added the whole strut descent to its line — ~4px per line, compounding down the page as `dy`. Measured: `<span style="display:inline-block">Ay</span>Ay` Chrome **19.19px** tall, ours **23**; with padding 29.19 vs 33. ⚠ **The three cases that already matched are the fallback ones — which is why it survived 690 ticks**: the rule we implemented is a real rule, applied to every box rather than to the two kinds it belongs to. A wrong rule that is right a third of the time and silent the rest is the hardest shape to see from the inside. Worth: `blog.rust-lang.org` shape **73.7% → 99.3%** on 1664 elements, `chat.google.com` **72.9% → 84.7%** (crosses the bar), 255md 69.8 → 72.1, wikipedia 58.8 → 60.4. Gated by **`G_INLINE_BLOCK_BASELINE`** (all five Chrome-measured heights + the line-box/placement inverse), RED-proven two ways, each mutation failing exactly the rows it should. ⚠⚠ **`blog.rust-lang.org` had been the loop's CONTROL site all session**, byte-identical through six fixes and quoted as evidence each time — because none of them touched what was wrong with it. **A control that never moves is telling you about your fixes, not about the site** |

| pattern | where it shows up | status |
| --- | --- | --- |
| **The gutter row** — `.row { margin: 0 -15px }` with `float: left` columns inside | Bootstrap 3 and 4's grid, and every framework that copied it: the negative margin cancels the columns' padding so the outer edges line up. One of the most-repeated CSS patterns on the pre-flexbox web, which is most of what is still online | ✅ fixed (tick 797) — **the float took its origin from the formatting context instead of from its own containing block.** A negative horizontal margin puts a block's content edges OUTSIDE the context that owns the exclusion bands, and `place()` folded that context's `left_edge` in as a floor: Chrome puts the first column at **-15**, we put it at **0**. t792 had taught this walk that the containing block is a LIMIT — and a limit is a no-op when the block starts further out, which is exactly this case. Now the containing block is the ORIGIN and overlapping floats push inward from it. Gated by **`G_FLOAT_CONTAINING_BLOCK`** (extended), RED-proven by restoring the context floor. ⚠ Two NEGATIVE probes in the same tick, each retiring a class: tables are Chrome-correct across collapse/fixed/colspan/rowspan/spacing (≤3px, all of it the collapsed-border convention), and negative margins on a plain block are exact in all four variants. ⚠⚠ **A measured number is only measured for the fixture it was measured in** — the right-float half reads 265 in isolation and 150 in the gate's own fixture (five earlier right floats share the band); both are Chrome, and porting the isolated one in was caught by the gate. ⚠⚠ **When a fix is a `max`/`min` against a bound, ask what it does when the bound is on the other side** — that question was available five ticks earlier and nobody asked it |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A percentage-height child of a flex row** — `height:50%` on a bar, a divider, a media thumbnail, a sidebar rail inside a fixed-height flex container | Cards with a fixed-height header, progress and rating bars, split panes, media rows, dashboard tiles — anywhere a design system fixes a container's height and sizes its parts as fractions of it | ✅ fixed (tick 798) — **the percentage was applied TWICE and the used height came out squared.** `layout_flex` hands each item its taffy slot as the parent's definite height, and `own_definite_h` re-resolved the item's own percentage against it: `height:50%` in a 200px row read **50** against Chrome's **100**, `height:25%` read **13** against 50. Instrumenting the bridge proved taffy's own answers were already right, so the squaring was entirely on our side of the seam. ⚠⚠ **This is the same defect the WIDTH axis had and fixed at tick 14** (*"a percentage width on a flex item resolved twice; used width came out squared"*) — `taffy_item_width` has sat next to the unfixed block axis, with a comment naming the failure mode, for 784 ticks. **When a fix exists for one axis, grep for its mirror before believing the class is closed.** Gated by **`G_FLEX_PERCENT_HEIGHT`** (nine Chrome-measured heights, read off the gate's own fixture), RED-proven. ⚠ The `pct_h` guard is a CONSERVATISM, not a proven necessity — recording the slot for `auto` items too passes every case including two written to break it, and the gate's RED list says so rather than claiming a red it cannot produce. ⚠ Residue: a `height:auto` row item whose content is taller than the container should stretch and overflow (Chrome 30 in a 30px row); we keep the content height |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A centred block that also contains a block-level element** — `<center>` or `text-align:center` around a heading, a caption, a byline, a button row, with a `<textarea>`, `<input class="form-control">`, `<div>`, `<hr>` or `<p>` among the inline copy | Every centred card, hero and footer on the web, and the whole `<center>`-and-a-form long tail that still makes up much of the CrUX corpus. The trigger is *"…and one block child"*, which is the normal shape of a real section rather than an edge case | ✅ fixed (tick 799) — **an anonymous block box inherited NOTHING from the container that generated it.** CSS 2.1 §9.2.1.1 wraps runs of inline content in anonymous blocks when a container mixes inline and block children, and those boxes inherit every inheritable property from that container. `flush_inline_run` passed `layout_inline` the literals `TextAlign::Left, 0.0, …, None` where the pure-IFC branch of the same file passes `bcs.text_align, text_indent, …, Some(&bcs)`. **Two paths, one formatting context, one of them blind.** So the same markup centred correctly right up until a block-level sibling joined it, at which point every inline run snapped to x=0 — Chrome 350, ours 0, on eight fixture shapes. ⚠ **The STRUT is the same omission's second symptom**: with `strut_style:None` a line whose only content is an atomic `inline-block` was exactly the box's height (20) where Chrome adds the containing block's font descent below the baseline (24). A *text* run was already right, because each fragment carries its own inherited `line-height` — only the atomic case exposed it, which is why it survived. ⚠⚠ **A test had frozen the missing descent as ground truth**, asserting `y=30` where Chrome says 34, with a comment claiming the number was *"verified numerically against Chrome by the parity harness"*. It was not. **A number asserted from an unverified claim of verification defends the defect.** Real site: `linkmake.in` (in-scope CrUX, coverage 1.00) is `<center><b>…</b><br>…<textarea class="form-control">…</center>` and rendered its whole centred column flush left — `<b>` at 170 against Chrome's 537 — shape **0.622 → 0.703**; four controls byte-identical. 31 of 33 fixture boxes now Chrome-exact. Gated by **`G_ANON_BLOCK_INHERITS`**, RED-proven twice and independently (restore the literal → the eight alignment rows fail and the heights pass; restore `None` → the heights fail and the alignments pass). ⚠ `text-indent` is the third literal and is deliberately NOT fixed: Chrome indents only the FIRST anonymous run (40, then 0), so passing it through would over-indent every run but the first — measured, written down, left for its own tick. ⚠ Residue: a `float:left` after an inline run drops a line instead of sharing it, so the run centres in the full width (350) not the float-narrowed band (Chrome 380) |

| pattern | where it shows up | status |
| --- | --- | --- |
| *(measurement, not a web pattern — recorded here because the pattern ledger is what the pre-commit hook reads and the finding must not be silent)* **A live-site fidelity band that goes negative** | Every sweep-to-sweep comparison this burndown makes | 📏 t800 — **the COMMON-SET BAND read −0.35 pts (9 sites down, 2 up) and NONE of it was the engine.** t794's rule fixed *which sites* the denominator contains; it cannot fix *what those sites served*. Rebuilding `engine/` at `3afc662b` — the exact tree behind the t796 sweep — reproduced TODAY's numbers, not t796's (`nysainfo.pl` 0.678→0.565 in the band, 0.562 on the old binary run today; `crm.majoo.id` 0.467→0.400, 0.400 on the old binary). So the band over live pages sums **two** deltas — engine and web — and publishes them under the engine's name. ⚠ **The only instrument that separates them is the OLD BINARY re-run today**, one 3-minute rebuild, now the required second step before any revert. NO REVERT was taken; t797/t798/t799 are all clean (an intermediate build with only t799 reverted read `0.565217 / 0.400000` to six digits, byte-identical). ⚠ Sites at low coverage or low `shape_n` dominate the noise: `crm.majoo.id` has n=30, where ONE element is 3.3 points |

| pattern | where it shows up | status |
| --- | --- | --- |
| **The centred content column** — `.container { max-width: 1200px; margin: 0 auto }`, `max-w-4xl mx-auto`, `.wrapper { max-width: 65ch; margin: auto }` | Every Bootstrap `.container`, every Tailwind `mx-auto max-w-*`, every blog/docs theme's article body, every marketing page's section wrapper. It is the reason a wide window shows margins instead of edge-to-edge text, and it is plausibly the single most-written layout rule on the modern web | ✅ fixed (tick 801) — **it rendered FLUSH LEFT.** CSS 2.1 §10.4 in one sentence: when the used width violates `max-width`, the §10.3.3 rules are *applied again* with the constraint as the computed width — and §10.3.3 is where a pair of `auto` margins splits the remainder. We did the clamp and skipped the re-run: the auto-margin block was guarded on `s.width != Dim::Auto || s.width_keyword.is_some()`, i.e. on whether the AUTHOR wrote a `width`, which for `max-width:1200px; margin:0 auto` they did not. So the box became definite at 1200 and the margins never learned. Chrome 200, ours 0, on a 400-in-800 fixture; 200 vs 48 inside a padded parent. ⚠⚠ **The `min-width` half of the SAME sentence looked fine, and the reason is the lesson**: a clamp UPWARD only binds when there is an explicit `width` (a `width:auto` box already fills its container), and an explicit width always took the guard's first term — so every `min-width` case in existence took the working path and nothing could tell "we implement §10.4's re-run" from "we don't". **One rule, two constraints, and the one that needed no help was the one that worked.** ⚠ `margin-left:auto` ALONE pushes the box fully right (Chrome 400, not 200), which is what proves the fix is §10.3.3's split rather than a `centre it when clamped` special case — asserted, so the plausible wrong version cannot return. Gated by **`G_MAX_WIDTH_AUTO_MARGIN`** (eight Chrome-measured x/width pairs read off the gate's own fixture), RED-proven: drop the third term and the four auto-margin cases snap to 0/48 while the two explicit-width cases still pass. ⚠ Found on `255md.com` (in-scope CrUX, n=43, jarring-clean, +0.029 from the M1 bar): its `.contact-form{max-width:400px;margin:auto}` sat at x=309 against Chrome's 400. The form is now Chrome-exact and **the site's score did not move**, because every element carrying the x error also carries a co-located HEIGHT error (a `<textarea>` 138 tall against Chrome's 97) and an element is wrong if ANY axis is — a masked fix, worth recording as such rather than as a null result |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A styled form on a page with body typography** — `body { line-height: 1.6 }` above any `<textarea>`, `<input>`, `<select>` or `<button>` | Every contact form, every login box, every comment field, every search bar, on every site that sets its own line-height — which is essentially every site built since CSS resets became normal | ✅ fixed (tick 802) — **a control does not inherit the page's `line-height` either.** t787 gave controls the UA `font-family` and `font-size` from Chrome's `font: -webkit-small-control`; `line-height` is the THIRD property of that shorthand, and a shorthand resets what it does not mention, so Chrome's controls carry `line-height: normal` as a UA *declared* value that beats inheritance. We set two of three and the page's value walked back in through the door the shorthand closes. A `<textarea>`'s height is **rows × line-height**, so the error is proportional: `rows=5` read 119 against Chrome's 81, the default 2-row 51 against 36, a `<select>` 27 against 19. ⚠ **The two constraints are what make it a fix rather than a trade**: an author's own `line-height` on the control still WINS (86 in both engines, before and after — the rule is UA-origin), and a plain block still INHERITS the body's 1.7 (1200×27) — a fix that reset line-height globally would correct every control and silently re-typeset every page. Both asserted. ⚠⚠ **The pre-existing gate could not see this, by construction**: its fixture set `font: 16px sans-serif` with no `line-height`, so the document's value and the control's were both `normal` and agreed by accident. **A fixture that does not vary the property cannot fail on it** — the second fixture varies exactly one thing. Gated by **`G_FORM_CONTROL_METRICS`** (new `g_control_line_height_is_normal` case, every before/after column read off the gate's own fixture), RED-proven: delete the declaration and `#lt5` reads 119.33 against Chrome's 81 while both constraints still pass. ⚠⚠ **`255md.com` CROSSED THE M1 BAR on this: shape 0.721 → 0.767, jarring-clean** — its `<textarea>` was 138 against Chrome's 97 and dragged the whole `<form>`/`<p>`/`<div>`/`<body>` chain with it. Six controls byte-identical. ⚠ Residual, named: at an author font-size of 16px the textarea is 96 against Chrome's 101 — `normal` resolving to 18/row where Chrome uses 19, a font-metric question independent of this rule |

| pattern | where it shows up | status |
| --- | --- | --- |
| **`<div style="position:absolute">Menu</div>`** — an out-of-flow box whose content is BARE TEXT | Every dropdown item, tooltip, popover label, notification badge, absolutely-positioned hero caption, carousel overlay and `.sr-only` accessibility string on the web. It is how `position:absolute` is actually written — a wrapper element around the text is the exception, not the rule | ✅ fixed (tick 803) — **the box measured 0×0. Not misplaced — sized to nothing.** `layout_children` filters a container's out-of-flow children by style, and **under the Stylo cascade a bare text node carries a CLONE of its parent's style**, so inside a `position:absolute` box the box's own text answers *yes, I am out of flow* and filters itself out of the content it IS. No children left to measure → `shrink_to_fit` 0 → content height 0. Chrome 62×20, ours 0×0; with `padding:10px` ours read 20×20, which is the padding alone and *looks like a box rather than like an absence*. ⚠⚠ **An ELEMENT child hid it completely** — `<div abspos><span>Menu</span></div>` was always correct, because the `<span>` carries its own `position:static`. **The bug fires on the shape people write and not on the shape a test-writer reaches for**, which is why it survived. ⚠ **The guard already existed one function away**: `max_content_width_uncached` documents this exact trap for `display:flex` ("a bare run inside `display:flex` reads back as `flex` here") and guards it with `is_element` — same cascade quirk, same guard, four more call sites that never got it (the in-flow filter, the has-a-float check, the static-position loop, the block-children dispatch). Fixed with two node-aware predicates, `kid_is_float`/`kid_is_out_of_flow`. Gated by **`G_ABSPOS_BARE_TEXT`** (nine Chrome-measured sizes), RED-proven twice and independently — restore the raw predicates in the pure-IFC filter and six cases go 0-size while the element-child and float cases pass; restore them in the block dispatch alone and only the mixed text+block+text case fails, at 70×12. ⚠⚠ **`www.dapam-sirius.fr` CROSSED the M1 bar: shape 0.633 → 0.800, jarring-clean.** ⚠ **AND IT EXPOSED A REAL DEFECT IT HAD BEEN HIDING**: `taffy_tree::flex_items` pushes every element child including out-of-flow ones, where Flexbox §4.1 says an abspos child **is not a flex item** — wikipedia's header div is now 248 against Chrome's 180, and it matched before only because the box inside it measured zero. Named, not folded in: excluding them also needs `layout_flex` to record a `static_pos`, or the box vanishes entirely |

| pattern | where it shows up | status |
| --- | --- | --- |
| **An absolutely-positioned child inside a flex or grid container** — a dropdown anchored in a nav row, a badge on a toolbar button, a tooltip in a card header | Every app chrome built after 2015: the container is flex, the popover inside it is abspos | ⛔ **REFUSED (tick 804) — spec-correct, fixture-exact, and it cost a control.** Flexbox §4.1 / Grid §9: an absolutely-positioned child **is not an item and does not contribute to the container's size**. `taffy_tree::flex_items` pushes every element child, and `layout_flex_or_grid` folds every placed item's bottom into the container height — so a `width:fit-content` flex row holding `ab` plus a 100px abspos label is **18×20** in Chrome and **18×100** here. Excluding out-of-flow items made all six fixture containers and all four children Chrome-exact from one edit. ⚠⚠ **On `en.wikipedia.org` it cost nine elements of 1074** (0.593110 → 0.584730, same `n`, same window, old binary rebuilt per t800's rule), with five controls byte-identical and NO site crossing. The readable new divergence is a 32×32 hamburger button becoming **100×36** — **a WIDTH change out of a HEIGHT-only edit**, so there is a coupling between a container's resolved height and a sibling's width resolution that is not traced. Same verdict as t695's spec-correct 8/8 fix that regressed its control: **a change whose blast radius on a real page is not understood is not a fix yet, however right the specification is.** Reverted. Banked for the retry: the fixture with its Chrome numbers, the spec clause, the expression (`max_h = max_h.max(bottom)` over `placed`), and the one question to answer first — why does a height-only edit move a sibling's width from 32 to 100? ⚠ It became visible only because t803 gave abspos bare text a real size; **one defect was hiding the other** |

| pattern | where it shows up | status |
| --- | --- | --- |
| **Justified body text** — `text-align: justify` on articles, columns and notices | Prose-heavy pages, newspapers and magazines, institutional/university/government sites, PDFs-turned-HTML, and a large share of the non-English long tail — justification is the default typographic taste in much of continental Europe, Latin America and South Asia, which is exactly where a CrUX-representative corpus lives | ✅ fixed (tick 805) — **the property was parsed and then ignored.** `TextAlign::Justify` reached `close_line`, fell through the `_ => 0.0` arm of the offset match, and rendered identically to `left` for the engine's whole life. Every other alignment IS a single translation of the line, which is precisely why this one fell through: it is the only value that is not an offset — the slack has to be distributed across the line's word gaps. Chrome 49/237 for the 2nd and 6th words of a justified line; ours 45/220, which are the *unjustified* control's numbers. ⚠ **It does not degrade gently**: every word after the first is misplaced and the error grows along the line, so one paragraph yields dozens of divergences. ⚠ **The three call sites ARE the specification** — CSS Text §7.3 exempts the last line and any line ended by a FORCED break, and `close_line` already had exactly three callers (`<br>`, wrap, final flush), so eligibility is one boolean per caller rather than a heuristic. RED-proven: pass `true` at the last-line site and a three-word line's word flies from 43 to **190**, which is the most recognisable rendering bug the property has. ⚠⚠ **Snapshot the gap positions before shifting**: reading `line[i-1].x` inside the loop that already moved it makes every gap after the first measure as closed, and *from the outside a shift that stops accumulating is indistinguishable from a slightly-wrong per-gap constant* — the 2nd word landed exact and the 6th was 10px short. Two words on one line separate them; both are gated. Gated by **`G_TEXT_ALIGN_JUSTIFY`** (eleven Chrome-measured x positions), RED-proven two ways. ⚠ **The lead that found it was WRONG about its own site**: `www.wdimax.com`'s 127 lagging spans looked exactly like missing justification and did not move (0.607 → 0.607). The property was genuinely unimplemented; that site's cause is still unidentified. Seven controls byte-identical |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A tracked run** — `letter-spacing: .05em` on a nav bar, a button, a heading, an uppercase label or a small-caps byline | Design-system standard. Every Tailwind `tracking-wide`, every Material/Bootstrap button, every uppercase section label and hero heading on the modern web | ✅ fixed (tick 806) — **the space is a character, and it was the one character that missed out.** `letter-spacing` adds a fixed advance after every character; we added it once per character of each WORD and stopped, so an inter-word space never got it. ⚠⚠ **The hardest shape a layout defect takes**: every word's own box stayed exactly right while its POSITION fell one `letter-spacing` behind per preceding space, cumulatively along the line — the quantity you would think to measure is the correct one. Chrome 39/115 for the 2nd and 4th words at `letter-spacing:2px`; ours 37/109. The arithmetic identifies it rather than a fudge: at the 4th word Chrome has advanced **12 characters × 2px** and we had advanced **9 × 2px**, exactly the three spaces missing. ⚠ **`word-spacing` — the sibling property, one line away in the same expression — was ALWAYS applied to the space**, so any probe that happened to use it reports everything working; it is in the gate as the control that says the bug was specific to `letter-spacing` and not to spacing in general. Gated by **`G_LETTER_SPACING_SPACE`** (ten Chrome-measured positions across three faces + a width assertion), RED-proven: drop it from the space and `#e1` reads 37 while every `word-spacing` and no-spacing row passes. `news.ycombinator.com` 0.797264 → 0.798507; seven other controls byte-identical. ⚠ Found while chasing `www.wdimax.com`'s 127 lagging spans, which this does NOT explain either — that site uses no `letter-spacing` and remains open |

| pattern | where it shows up | status |
| --- | --- | --- |
| *(measurement)* **Comparing two sweeps across a harness change** | Every burndown checkpoint | 📏 t807 — **48 false `crashed` rows became 0 and scorability leapt 53.1% → 78.6% in one sweep. None of that is the engine.** The `CHUNK_ROUNDS = 4` artefact filed 48 in-scope sites as crashes at t800 and zero at t807, so `scored 77 → 103` is mostly a harness defect disappearing; the in-scope denominator also moved 145 → 131 and `excluded` rose 55 → 69, almost all `bot-wall` 29 → 39. **Diff the UNSCORED REASON COUNTS before comparing denominators** — one `uniq -c` separates a harness change from an engine one, and the jump would otherwise have been the most quotable number of the session and a lie. The SHAPE half is paired and readable: M1 5.5% → **8.4%**, shape≥0.75 8.3% → **13.0%**, jarring-clean 17.2% → **27.5%** (flat for two sweeps before), COMMON-SET BAND **+2.71 pts, 20 up / 5 down**. ⚠ The old-binary control was run on the only decline with a stable element count (`gismart.com` −0.071): **0.693950 on both binaries**, with today's value between the two sweep readings — the site moved, we did not |

| pattern | where it shows up | status |
| --- | --- | --- |
| **The button-styled inline link** — `<a class="btn" style="padding:10px 20px">`, and every tag, badge, chip, nav pill and `<span class="label">` with padding | Universal. It is the default way a link is made to look like a button without `display:inline-block`, and it is what `linkmake.in`'s login/signup buttons, every Bootstrap `.badge`, and every design system's chip are built from | ✅ fixed (tick 808) — **the padded pill reported 18px where Chrome says 37, and PAINTED its background at 18.** CSS 2.1 §10.6.1: on a non-replaced inline, vertical padding and border do not affect LINE height — but the box still has them, so the pill *overflows* its line, which is the entire visual point of the idiom. We grew neither: the box was just its text's content area. Chrome `[0 -9 79x37]`, ours `[0 0 79x18]` — and note the y, because the box starts ABOVE its own text and a rect anchored at the line top cannot express that (hence a new `report_ascent` on the padding edge). ⚠ **An ATOMIC box (`inline-block`) with the same padding was always correct**, because it owns its own border box — that is the shape a test-writer reaches for, and it is why this survived. ⚠⚠ **The first working version got every 37 right and made the CONTAINING DIV 37 too**, because `close_line` folds a synthetic reporter's `line_height` in as a floor on the line box — right for an empty inline, catastrophic here: it relaid every line below. A padded edge now reports a tall RECT and a ZERO line-height, and the gate asserts the div at Chrome's 20 alongside every 37. ⚠ `padding: 10px 0` emits no horizontal edge, so it needs its own zero-width arm — which must NOT hold a line open. Gated by **`G_INLINE_VERTICAL_PADDING`** (nine Chrome-measured heights + a y assertion), RED-proven three ways. ⚠⚠ **`linkmake.in` CROSSED the M1 bar: 0.703 → 0.757, jarring-clean** — the third crossing of the session. Six controls byte-identical; `en.wikipedia.org` shape byte-identical (0.593110, n=1074) with `reading_order` 5 → 7, disclosed and unattributed to a mechanism, on a site already failing jarring at h_overflow=52 |

| pattern | where it shows up | status |
| --- | --- | --- |
| **`<picture><source srcset>`** — and its relatives `<track>`, `<area>`, `<noscript>` | Responsive images are `<picture><source>` on essentially every modern site; `<track>` on every captioned video; `<area>` on every image map. A shim or a feature-detect that asks `getComputedStyle(source).display` is reading exactly the value that was wrong | ✅ fixed (tick 809) — **not rendered is not `display: none`.** Eight elements were hidden with a UA `display:none`, which produced the right BOX and the wrong ANSWER for half of them. Measured with `getComputedStyle`, not recalled: `source`/`track`/`area`/`noscript` are **`inline`** in Chrome, while `param`/`datalist`/`template`/`rp` really are `none`. Those four generate no box because their PARENT consumes them — `<picture>`/`<video>` render their `<img>`/media, `<map>` is not a container, `<noscript>` holds raw text — which is a STRUCTURAL fact, not a stylesheet one. ⚠ **Half the list was already right**, which is exactly why the measurement was worth taking: a fix applied to "the metadata elements" as a class would have broken `<param>` and `<datalist>`. ⚠⚠ **The structural guard turned out not to be needed and was REMOVED before shipping** — a `never_rendered(tag)` check in `is_rendered` changed nothing on the fixture or the corpus (`mobcup.fm` 0.909091 either way) and its removal *improved* `en.wikipedia.org`'s coverage 0.998141 → **1.000000**. A guard that cannot be shown to do anything is not a safety margin, it is unexplained machinery. ⚠ Both cascades moved in the same tick — the list exists twice and the second one's own comment warns about exactly this drift. Gated by **`G_UNRENDERED_IS_NOT_DISPLAY_NONE`** (eight Chrome-measured computed values + two no-box assertions), RED-proven on the computed-value half; the gate states plainly that the no-box half has no mutation to flip. ⚠⚠ **`mobcup.fm` CROSSED the M1 bar: 0.710 → 0.909, jarring-clean** — the fourth crossing of the session. Six controls byte-identical |

| pattern | where it shows up | status |
| --- | --- | --- |
| **The media object** — a floated avatar/thumbnail with an `overflow:hidden`, `flow-root`, flex or grid content block beside it | Every comment thread, every card list, every article with a pull-quote or figure, and the standard pre-flexbox two-column idiom. It is what `overflow:hidden` next to a float has meant since 2004 | ⏳ **OPEN, specified (tick 810)** — CSS 2.1 §9.5: the border box of an element establishing a new BFC **must not overlap the margin box of any float in the same BFC**. It is placed BESIDE the intruding floats (shifted by the band's left offset and narrowed to the band), or moved DOWN until it clears if an explicit width will not fit. **We implement neither half.** Chrome vs ours, `float:left 80×40` then a BFC root in a 300px column: `overflow:hidden` **[80 0 220×20]** vs [0 … 300×20]; `flow-root` [160 20 140×20] vs [0 … 300×20]; `flex` [240 40 60×20] vs [0 … 300×20]; `grid` [240 60 60×20] vs [0 … 300×20]; `table` [160 80 35×20] vs [0 … 0×20]; and with an explicit `width:280px` Chrome DROPS the box to y=180 to clear while we leave it overlapping at y=120. ⚠ The PLAIN-block row is the control and is correct in both — a non-BFC block's border box legitimately overlaps floats and only its line boxes avoid them, which is already right. Named as residue at t792 (*"the gate asserts x only"*) and unmeasured until now. ⚠⚠ **Deliberately not built in the tick that measured it**: it changes the ORIGIN and AVAILABLE WIDTH of a whole class of blocks, and t804 refused a spec-correct fixture-exact change six ticks earlier for exactly the reason that its reach on a real page was untraced. Insertion point identified — the block-children loop, immediately before `layout_block(…)`, where `clear` is handled two lines above and `floats.available(y,h)` / `next_bottom_below(y)` already exist |

| pattern | where it shows up | status |
| --- | --- | --- |
| **The media object — REVERTED at t812, see the row's tail** — a floated avatar/thumbnail with an `overflow:hidden`, `flow-root`, flex or grid content block beside it | Every comment thread, every card list, every article with a pull-quote or figure, and the standard pre-flexbox two-column idiom. `overflow:hidden` next to a float has meant "content column" since 2004 | ✅ fixed (tick 811) — **the BFC root sat straight on top of the float.** CSS 2.1 §9.5: such a box's border box must not overlap a float's margin box; it goes BESIDE the floats (shifted to the band edge and narrowed to it) or DOWN past them when an explicit width will not fit. We did neither. Chrome vs ours on a `float:left 80×40` in a 300px column: `overflow:hidden` **[80 0 220×20]** vs [0 … 300×20], `flow-root` [80 50 220×20], `flex` [80 100 220×20], `grid` [80 150 220×20], a right float narrowing to [0 320 220×20], and `width:280px` DROPPING to [0 290 280×20]. **Seven of eight fixture boxes byte-identical to Chrome after, none before.** ⚠ **The plain-block row is the rule's boundary, not an oversight** — a non-BFC block's border box legitimately overlaps floats and only its LINE boxes avoid them, which was already right; a fix keyed on every block passes all five shifted rows and is badly wrong on the commonest layout on the web, and that is one of the three RED mutations. ⚠ `left_float_edge`/`right_float_edge` (the `Option` form, `None` when nothing overlaps) and NOT `left_offset`/`right_offset`, which fall back to the CONTEXT's edges and would move blocks with no float near them — t797's distinction, reused. ⚠⚠ **Nine controls byte-identical**, including the two this session's refused t804 change had cost. Gated by **`G_BFC_AVOIDS_FLOATS`**, RED-proven two ways. ⚠ `display:table` is excluded and says so: Chrome puts it at [80 200 35×20] and we produce a 0-wide box from an independent table intrinsic-width defect. Specified at t810, built at t811, **REVERTED at t812**. ⛔ It costs `www.ta3lemkonline.com` (`reading_order` 816, a float-heavy page **not** in the control set) **26 elements of 457** — 0.540481 → 0.483589, same `n`, bisected exactly against the t809 tree, and the revert restores 0.540481 to the digit. ⚠⚠ **It landed with nine byte-identical controls and that was TRUE** — nine byte-identical controls is not evidence of no regression, it is evidence about nine sites, and the controls this loop uses are the sites it has already fixed things on: the population LEAST likely to be disturbed by the next fix. The work is still right and still open; what it lacks is an account of why a float-band narrowing costs a float-heavy page 26 elements |

| pattern | where it shows up | status |
| --- | --- | --- |
| *(measurement)* **Attributing a regression across several ticks** | Every time a control moves and more than one candidate change is in flight | 📏 t813 — **a bisection across TREES is a sequence of spot readings and inherits every one of their drifts.** Walking `tukrd.com` back through four engine trees produced a clean, striking step — 1.000 at t806, **0.605** at t808, 0.974 at t809 — which reads as *"t808 cost 15 elements and t809 gave 14 back"* and is wrong. Disabling t808's hunk **on the HEAD tree** (one build, one window) leaves the site at **0.973684, identical** — t808 exonerated, and the 0.605 was the page moving between two builds four minutes apart. ⚠⚠ **Isolate the hunk on ONE tree; do not walk the trees.** Both cost one build; the walk gives N readings at N different times on a page changing under you, and the differences read exactly like the effect being hunted. **The seductive part is that the walk produces a number per tick and therefore looks like the more thorough method.** ⚠ t800's rule (rebuild the old binary and measure NOW) is what the walk quietly violates while appearing to extend it |

| pattern | where it shows up | status |
| --- | --- | --- |
| **`display:table` used as a layout tool, not for tabular data** — `display:table; margin:0 auto` to shrink-wrap and centre, `display:table-cell; vertical-align:middle` to centre vertically, `display:inline-table` for a shrink-wrapped inline block | The pre-flexbox layout vocabulary, still everywhere in the CrUX tail: legacy themes, CMS templates, email-derived markup, and any site whose CSS predates 2016 | ⏳ **OPEN, specified (tick 814)** — **a `display:table` box with no table-structured children renders NOTHING.** Not narrow — absent. Chrome vs ours: bare text `[0 0 36×20]` vs **0×0**; a longer run `[0 20 213×20]` vs **0×0**; `display:inline-table` `[0 106 72×20]` vs **0×0**; and `width:200px` with bare text `[0 86 200×20]` vs **0×0** — an EXPLICIT width does not save it, which is what rules out sizing and names the cause. With real `table-row`/`table-cell` children it is Chrome-exact (109×20), so the table path itself works. `collect_table_rows` keeps only `TableRow`/`TableRowGroup` ELEMENTS and drops everything else (`_ => {}`, *"stray content: skipped"*), so a text-only table has zero rows; CSS 2.1 §17.2.1 wraps such content in an anonymous cell inside an anonymous row. ⚠⚠ **Third time this session a bare text node fell through a structural filter** — t799 (anonymous block inherited nothing), t803 (text cloned its parent's `position:absolute` and filtered itself out of the box it WAS), now this. **The recurring shape is a filter written for elements, applied to a child list containing text.** ⚠ Banked not built: `collect_table_rows` returns real DOM ids so there is no anonymous node to return, and the fix changes the contract of a subsystem whose doc deliberately bounds its scope — t811 is two ticks old and cost an unchosen site 26 elements while being spec-correct and clean on nine controls. Build it with `www.ta3lemkonline.com` in the controls from the start. ⚠ It also corrects t811's own residue label, which called this "a table intrinsic-width defect" — plausible, adjacent, and wrong |

| pattern | where it shows up | status |
| --- | --- | --- |
| **`display:table` as a layout tool** — `display:table; margin:0 auto` to shrink-wrap and centre, `display:inline-table`, `display:table-cell; vertical-align:middle` | The pre-flexbox layout vocabulary: legacy themes, CMS templates, email-derived markup, anything whose CSS predates 2016 — i.e. much of the CrUX tail | ✅ fixed (tick 815) — **it rendered NOTHING.** Not narrow, absent: Chrome `[0 0 36×20]` for `display:table` around bare text, ours **0×0**; a longer run 213×20 vs 0×0; `display:inline-table` 72×20 vs 0×0; and `width:200px` 200×20 vs **0×0** — an EXPLICIT width did not save it, which rules out sizing and names the cause. `collect_table_rows` keeps only `table-row`/`table-row-group` ELEMENTS, so a text-only table has zero rows and `layout_table` built an empty box. ⚠ **The fix is not a patch on the table formatter**: CSS 2.1 §17.2.1 wraps such content in an anonymous cell inside an anonymous row, and a one-anonymous-cell table is exactly a shrink-to-fit block over the same content — so the style clone gets `width:fit-content` and the generic block path runs, while anything with real rows still goes to the formatter (asserted, and one of the reds). ⚠⚠ **Third time in one session a bare text node fell through a structural filter** (t799, t803, this) — *a filter written for elements, applied to a child list containing text*. Gated by **`G_ROWLESS_TABLE`** (six Chrome-measured sizes), RED-proven on the routing; the gate states plainly that the explicit-width guard has NO red because `layout_block` already ignores `width_keyword` when the width is definite. ⚠⚠ **`www.ta3lemkonline.com` — the adversarial control t811's revert bought — IMPROVED 0.540481 → 0.551422**, and eight other controls are byte-identical |

| pattern | where it shows up | status |
| --- | --- | --- |
| **`display:table-cell` WITHOUT a table wrapper** — the legacy vertical-centring trick and the equal-height-column idiom, plus orphaned `table-row`/`table-row-group` | The same pre-flexbox vocabulary as t815, but the CELL rather than the table: legacy themes, CMS templates, email-derived markup, anything whose CSS predates 2016 | ✅ fixed (tick 816) — **an orphaned table-internal box was laid out as a plain run of inline TEXT.** CSS 2.1 §17.2.1 wraps it in anonymous table objects and the result is ATOMIC — laid out as a block, flowed like a word. ⚠⚠ **We had it in NEITHER of the two places that make a box atomic, and the two omissions were the SAME omission**: the inline collector's atomic list read `InlineBlock \| Flex \| Grid \| InlineFlex \| InlineGrid`, so the cell fell through to the TEXT RECURSION; and the `width:auto` shrink-to-fit arm carried an identical copy of that list, so once the box *was* atomic it filled its container. **An inline box is sized to its GLYPH BOX (17), an atomic one to its LINE BOX (20)**, and the leftover half-leading also pushed `y` down by 1 — so every orphan cell was ~3px short AND a pixel low, accumulating downward. Chrome vs before: `[0 30 85×20]` vs `[0 31 85×17]`; two sibling cells SIDE BY SIDE `[0 60 21×20]`/`[21 60 31×20]` vs stacked-and-short; a cell inside an orphan `table-row` `[0 180 87×20]` vs `[0 181 87×17]`. 4 of 12 boxes Chrome-exact before, **11 of 12 after**. ⚠⚠⚠ **THE CONTROL IS WHAT MADE IT A DIAGNOSIS**: `#ib`, an `inline-block` with BYTE-IDENTICAL content, was already exact at 85×20 — without it the table reads exactly like a general line-height/half-leading error, and the fix would have been aimed at the strut. Gated by **`G_ORPHAN_TABLE_CELL`** (ten Chrome-measured boxes), RED-proven on BOTH halves. ⚠ **The second copy of the list was INERT until the first was fixed** — adding the shrink-to-fit arm alone would have been a no-op, demonstrated by reverting the atomic half and reproducing the untouched baseline to the pixel. ⚠ Residue asserted at OUR number: a cell must STRETCH to fill a table with an explicit height (Chrome `[0 90 300×80]`, ours `[0 92 67×20]`) — anonymous-row generation INSIDE a real table, a different mechanism. ⚠ `www.ta3lemkonline.com` misplaced **422 → 420**; eight controls byte-identical |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A sub-pixel float excess breaking a flex line** — accumulated item widths compared against the container with an exact `>` | Every Bootstrap grid on the web: `.col-8` ships as `width:66.66666667%`, `.col-4` as `33.33333333%`. Also any hand-written `33.333333%` thirds — i.e. the single most common layout idiom there is | ✅ fixed (tick 817) — **the columns STACKED instead of sitting side by side.** taffy collects flex lines with a bare `>` and no tolerance (`taffy-0.12.1/src/compute/flexbox.rs:930`); `66.66666667%` is not representable in binary, so in `f32` it resolves against a 1200px row to `800.00004`, its sibling to `400.00002`, and the pair sums to a HAIR over 1200 — enough. ⚠⚠⚠ **Chrome never sees it because Blink quantises every resolved length to `LayoutUnit` = 1/64 px BEFORE anything compares them**, so the same pair is exactly `800+400`. ⚠ **The sharpest row is `33.33333333% × 3`, which sums to UNDER 100% in decimal and still overflowed** — each third rounds UP in `f32`. The defect is binary representability, not the digit count and not the decimal sum. Fixed on OUR side of the boundary (taffy is a crates.io dep, its resolver is not ours to patch): `solve_subtree` already knows the container's content width, so each direct child's percentage main-axis width is snapped to the 1/64 grid before the solve. ⚠ **BOUNDS**: direct children of a `row` container only — a flex container nested inside a flex item has a width taffy itself decides. ⚠ **The gate CANNOT prove the 1/64 constant** and says so: `LAYOUT_UNIT = 1.0` passes identically because this fixture's percentages land on integers. ⚠ Four always-passing rows (`50/50`, `75/25`, and two pairs summing under 100%) plus `70%+40%` asserted to STILL wrap bound the fix from both sides, so it cannot degenerate into "never wrap". Anchor `www.puentedemando.com` visual **50.7% → 54.8%**, cards no longer overlapping; seven controls byte-identical. ⚠ **A browser needs a QUANTUM, not an epsilon** — wherever accumulated lengths are compared against a container, ask what grid the operands live on. ⚠ **CORRECTED AT t819 — this row originally called Bootstrap 4's defect 'a flex-BASIS defect'. It is not.** t819 extended the snap to `flex_basis` (`flex:0 0 <pct>` never touches `width`; the hypothetical main size comes from the BASIS, so those rows were the right WIDTHS on the WRONG LINES) and they are now Chrome-exact. ✅ **CLOSED AT t823**: `flex: 0 0 66.666667%; **max-width**: 66.666667%` gave `533`/`133` vs `800`/`400` — and dropping the `max-width` makes the same row exact, which is what proves the basis was never the culprit. The defect was `max-width: <pct>` on a flex item resolving against the item's OWN taffy-assigned width instead of its containing block (`800 × 0.666667 = 533`) — the documented `taffy_item_height` shape, on the width axis. Fixed t823 (see below), which also caught the item's MARGINS being applied twice |

| pattern | where it shows up | status |
| --- | --- | --- |
| **`flex: 0 0 <pct>` / `flex-basis: <pct>` as the column width** — the main size coming from the BASIS rather than from `width` | Bootstrap 4's grid and every design system that copied it; any hand-written `flex: 0 0 33.333333%` | ✅ fixed (tick 819) — **the same sub-pixel line-break defect as t817, on the one property t817 did not cover.** `flex: 0 0 <pct>` never touches `width`; the hypothetical main size comes from the BASIS, so these rows came out the RIGHT WIDTHS (800/400 on a 1200px row) on the WRONG LINES. `snap_row_item_percent_widths` now snaps `flex_basis` to the same 1/64 px grid. Gate `G_FLEX_PERCENT_LINEBREAK` EXTENDED (not duplicated), asserting the shorthand and the longhand separately so a shorthand-parsing change cannot take both; RED-proven (`#fb2` → `[0 160]`, right width wrong line). ⚠⚠⚠ **AND IT FALSIFIED t817's OWN RESIDUE LABEL**: t817 wrote in three files that Bootstrap 4's `533`/`133` was "the percentage applied twice, a flex-BASIS defect". **Drop the `max-width` and the same row is exactly `800`/`400`** — the basis was never the culprit. The live defect is `max-width: <pct>` on a flex item resolving against the item's OWN taffy-assigned width rather than its containing block (`800 × 0.666667 = 533`), the documented `taffy_item_height` shape on the width axis. Corrected in the gate, the wiki and the ledger. ⚠⚠ **A RESIDUE MEASURED WITH TWO PROPERTIES PRESENT NAMES NEITHER** — re-run it with each suspect removed in turn (two fixture rows). **A wrong FIX is caught by the next gate; a wrong LABEL is caught by nothing** — it is prose, it passes every test, and it aims the next tick at the wrong organ. ⚠ `en.wikipedia.org` read 1017 two ticks ago and 1020 now — **the OLD BINARY also reads 1020**, so the site moved, not the engine |

| pattern | where it shows up | status |
| --- | --- | --- |
| *(measurement)* **A shape DROP that is actually a COVERAGE WIN** | Any tick that makes previously-absent boxes render — t815 (rowless `display:table` drew nothing), t816 (orphan `table-cell`), and every future missing-box fix | 📏 t821 — `oilprice.com` fell **0.8005 → 0.5857** in the t820 sweep and read as the window's worst regression. The old-binary control (rebuild `d82da7f2`, measure NOW) says otherwise: **coverage 61.3% → 98.0%, missing 253 → 13** — *240 elements we previously did not draw at all*. They then enter the placement score, so `misplaced` rises 399 → 639 and the RATIO falls **while the page is strictly more correct**: the old 0.8005 was over 61% of the page, the new 0.5857 is over 98%. ⚠⚠⚠ **`shape` is a ratio over elements BOTH engines render, so a fix that draws more boxes moves it DOWN.** A crossed-down row is NOT a regression until its COVERAGE column is read beside it. The mirror of t743's denominator lesson from the other side — there the metric flattered us by dividing by too much; here it punishes us for rendering more. ⚠ The ranked burndown must EXPECT coverage-raising fixes to lower shape first, and must not read that as grounds to revert one. ⚠ Same tick: the t820 sweep itself is UNBANKABLE — `118 of 200 filed crashed` (t812: 25) with `bot-wall` FALLING 33 → 10, which is the tell that those chunks never ran; third consecutive sweep invalidated by `CHUNK_ROUNDS`, reported to the observer, not patched |

| pattern | where it shows up | status |
| --- | --- | --- |
| **The visually-hidden idiom — `position:absolute; clip:rect(1px,1px,1px,1px)`** (`.screen-reader-text`) | **WordPress core boilerplate — essentially every WordPress theme on the web**, plus most hand-rolled a11y utility classes | ⏳ **OPEN, measured + specified (tick 822), DECLINED for the render phase with reason.** `clip` is unimplemented end to end: `grep` finds only `"clip" => Overflow::Clip` (a different property) and **no `clip` field in `ComputedStyle`**. Chrome gives such an element a FULL-SIZE box and hides it purely by clipping the paint — `.screen-reader-text` on `www.5movierulz.discount` computes `clip=rect(1px,1px,1px,1px)`, `width=68.48px`, `height=21px`, rect `[743 46 68x21]` — so we paint "Search for:" where Chrome paints nothing. ⚠⚠⚠ **BUT THE GEOMETRY IS ALREADY CHROME-EXACT** (`[0 40 78x20]` both; the WP-core block `[-1 · 1x1]` both), because `clip` never changes the box rect. **A fix would move `shape` by EXACTLY ZERO** — it is a JARRING/visual item needing a paint/display-item gate, not a box gate, and it must not be taken as an M1 lever while the render bar is the gate. ⚠⚠⚠ **THE MOST VISIBLE DIVERGENCE ON A COMPOSITE CAN BE THE ONE THE METRIC CANNOT SEE** — the composite chain finds DEFECTS and says nothing about which term of the metric a fix lands in. Ask that BEFORE building; it costs one fixture. (Second time this loop has paid for it — see the t772-775 window.) |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A flex/grid item's `max-width`/`min-width` percentage, and its MARGINS** — anything that re-reads taffy's slot as if it were the containing block | **Bootstrap 4's entire grid** (`.col-8` is literally `flex: 0 0 66.666667%; max-width: 66.666667%`) and every design system that copied it; the margin half needs no framework at all — **one `margin-left` on one flex item** | ✅ fixed (tick 823) — **taffy's slot is a FINISHED ANSWER and two things were still recomputed on top of it.** Taffy resolves the item's width, applies its `min-width`/`max-width` clamp against the REAL containing block, and positions the slot with the item's margins already taken out of the line; `layout_block` then did two of those three AGAIN with the SLOT as the containing block. Chrome vs before: `flex:0 0 90%; max-width:50%` **600 → 300**; Bootstrap 4's pair **800/400 → 533/133**; a grid item in an 800px track with `max-width:50%` **400 → 200**; `margin-left:100px` on a flex item put the box at **x=200 instead of 100**; a grid item's `margin-left:10%` at **876 instead of 840**. ⚠⚠⚠ **A PERCENTAGE CLAMP RE-APPLIED TO THE SLOT ALWAYS BINDS AGAIN; A PIXEL ONE NEVER DOES** — of the four min/max × px/pct combinations exactly ONE is observable (`max-width:<pct>`, wrong by the percentage SQUARED: 50% of the 50% answer is 25% of the container). `max-width:300px` against an already-300px slot is a no-op and `min-width:<pct>` of a slot can never exceed that slot, **so the two rows anyone checks first to sanity-check a clamp cannot fail** — both are now asserted as guards. ⚠⚠⚠ **A GUARD IS WRITTEN FOR THE PROPERTY THAT WAS FAILING, NOT FOR THE RULE IT ENFORCES**: `taffy_item_width` arrived ~120 ticks earlier with exactly the right sentence ("do not resolve it a second time") applied to `width` ALONE, while the clamp ten lines below and the margin twenty lines above went on doing it. **When a value is guarded, grep every other CONSUMER of it** — "one rule, N implementations" arriving as one rule with N READERS. Gate **`G_FLEX_ITEM_SLOT_IS_FINAL`** (14 Chrome-measured rows: 8 defect, 4 guard, 2 plain-block control), RED-proven once per half. Anchors PAIRED old-binary/new: four byte-identical, three up (a11yproject 0.335→0.358, wikipedia 0.523→0.538, martinfowler 0.697→0.703), coverage unchanged on all seven, every jarring dim that moved moved DOWN. ⚠ HONEST SCOPE: no site claimed to cross 0.75; `getbootstrap.com` is Bootstrap **5** (`flex:0 0 auto; width:X%`, no `max-width`) and is a CONTROL here, correctly identical |

| pattern | where it shows up | status |
| --- | --- | --- |
| *(instrument)* **A constant re-spawn budget against a workload that scales with the corpus** | The chunked fidelity sweep — i.e. the producer of the Phase-0 headline. Any parent/child work-splitter where the child exits deliberately on a per-item condition | ✅ fixed (tick 824) — **the sweep's `crashed` rows were its OWN re-spawn cap, and for three sessions they were read as a mozjs crash.** A chunk child exits ONCE PER SLOW SITE (the per-site watchdog writes the `timeout` row, then `process::exit(0)` because the main thread is wedged); `CHUNK_ROUNDS = 4` allowed four of those per 100-site bucket and filed the ~90 sites BEHIND THEM — most never opened — as `crashed`, a **Bar 0** event. t820: **118 of 200 `crashed`** vs t812's 25, ⚠ **with `bot-wall` FALLING 33 → 10**, which is the tell (a site cannot be *classified* bot-walled if its chunk never opened it, so a `crashed` count that quadruples while every other reason shrinks is arithmetic about the instrument). ⚠⚠⚠ **AND `pthread_mutex_destroy failed: Device or resource busy` IS NOT A CRASH** — it is what `process::exit` looks like skipping `JS_ShutDown()`, and `engine/js/src/spidermonkey.rs` predicts that exact string in its own doc comment. It was the LAST line before each death, so t820 and t821 both named it the cause; the line that actually named the cause (`UNMEASURABLE [timeout-150s]`) was ONE LINE HIGHER, three times. Fixed three ways: `chunk_round_budget(n) = n + 4` (budget scales with the work; costs nothing, since every round makes ≥1 site of progress), `CHUNK_STALL_LIMIT = 2` consecutive no-progress rounds as the real terminator, and `Unmeasurable::NeverRan` split out of `Crashed` so an instrument budget and an engine fault stop sharing a string (still counts against the bar — `fidelity-progress.sh` lands unrecognised reasons in-scope). Verified LIVE on the two sites that killed chunks: 10 sampled → **10 rows, zero crashed, zero never-ran**; `www.bilibili.com` now SCORES 0.549 and `janitorai.com` — previously *recovered as crashed* — classifies as `bot-wall-403`, the direct receipt for t821's inference. Gate `chunk_spawn_budget`, RED-proven by restoring the constant. ⚠⚠⚠ **A CONSTANT BUDGET AGAINST A VARIABLE WORKLOAD DEGRADES SILENTLY, INTO THE MOST ALARMING WORD THE LEDGER HAS.** ⚠⚠ **THE LAST LINE BEFORE A DEATH IS NOT THE CAUSE OF IT** — make every deliberate exit ANNOUNCE ITSELF, so an unlabelled fault is the only kind left. ⚠ And it had to be RUN: the mechanism lives in the ORDERING of the live log, which the banked rows cannot carry — t821 refused the run and inferred correctly but incompletely |

| pattern | where it shows up | status |
| --- | --- | --- |
| *(measurement)* **A measurement that has failed three times is a CAPABILITY GAP, not a chore to retry** | Any loop whose headline is produced by an instrument the loop also owns — here, the chunked fidelity sweep that prices every render fix | 📏 t825 — the board said "MEASURE NOW" for ~12 ticks; three sessions obeyed it literally (ran the sweep, found it contaminated, refused it, moved on to engine work) and the burndown stayed blind. The FOURTH run treated **the contamination itself** as the tick (t824), and the sweep then completed **200 of 200 in ~40 minutes**, absorbing 5 deliberate watchdog exits without losing a site. Histogram: `crashed` **118 → 1**, `never-ran` 0, `bot-wall` **10 → 39**. THE PRICE OF FIVE UNPRICED FIXES (t815/816/817/819/823), vs t812: **M1 count 11 → 13 sites (8.0% → 10.0%)**, jarring-clean **29 → 36**, scored 87 → 101, shape_mean 48.6% → 50.6%, cov_mean 83.8% → 85.2%; common-set band over 85 sites **+1.8 pts**, 3 crossed UP (`mobcup.fm`, `www.puentedemando.com`, `lms.sltc.ac.lk`), 2 down. ⚠⚠⚠ **THE GAINS HAVE THE SIGNATURE OF LAYOUT MATH, NOT COVERAGE** — the four largest are at UNCHANGED coverage (`celeb.gate.cc` 0.2295→0.7284 at cov 0.983 flat; `lms.sltc.ac.lk` 0.3952→0.7661 at cov 1.000 flat), and a 40-50pt shape gain on a page whose box COUNT did not move cannot be a denominator effect. ⚠⚠ **THE PERCENTAGES ARE PART COMPOSITION AND THE COUNTS ARE NOT**: in-scope fell 138 → 130 (bot-wall 33 → 39), so every ratio is flattered — print the COUNT beside the percentage or the denominator trap reads as progress. ⚠ Both crossed-DOWN rows read with coverage beside them: `oilprice.com` 0.8005→0.6231 is the known **coverage win** (cov 0.613→0.982, 241 boxes newly drawn); `www.freesupertips.com` 0.7637→0.6674 is NOT explained and is named as owing an old-binary control rather than absorbed into the good news. ⚠ **AN UNPRICED BATCH IS NOT A NEUTRAL STATE** — five fixes sat eight ticks; had one been a regression nothing would have said so, and the old-binary control is a per-site tool that does not scale to a batch |

| pattern | where it shows up | status |
| --- | --- | --- |
| **The `font:` shorthand, unparsed by the TEST cascade only** — and, generally, two cascades meaning every cheap probe has a second silent subject | Every `manuk-layout` unit test and every reduced fixture written against `MinimalCascade`; `font: <size>/<line-height> <family>` is common enough to appear in almost any hand-written fixture | ⏳ **OPEN, measured (tick 826), DECLINED for the render phase with reason.** Chrome vs the two cascades on five rows: `font:13.2px/17.16px Arial` → Chrome **17.16**, `MinimalCascade` **18.00** (= 2 lines at the DEFAULT 16px metrics — the shorthand is dropped whole, size AND line-height), **Stylo 17.16 ✓**; the longhand equivalent is 17.16 in both. **The shipping cascade is Chrome-exact on all five**, so a fix moves `shape` by EXACTLY ZERO and is not an M1 lever (t822's rule). ⚠⚠⚠ **THE COST IS PAID BY DIAGNOSIS, NOT BY RENDERING**: a reduction of `www.kicktipp.com`'s near-bar divergence reproduced perfectly under the cheap harness (`103.00x49.20` vs Chrome's `103.00x30.34`) and evaporated on `Page::load`. **A fixture that reproduces under the test cascade reads as a confirmed engine defect.** ⚠⚠ **A REDUCTION IS NOT CONFIRMED UNTIL IT HAS RUN ON THE SHIPPING CASCADE** — the cheap harness proposes, `Page::load` disposes. ⚠ And the near miss has the worst possible shape: it reproduced *the right number for the wrong reason* (49.20 = 2×18 + 13.2, two lines at default 16px metrics), which is the most convincing kind of wrong answer there is. Related: [Live cascade is Stylo not Minimal] |

| pattern | where it shows up | status |
| --- | --- | --- |
| **`max-height: <pct>` on a flex/grid item** — the block-axis twin of t823 | An item carrying BOTH a percentage `height` and a percentage `max-height`; narrower than the inline twin, which reaches all of Bootstrap 4 | ✅ fixed (tick 827) — `pch` for a taffy item is the SLOT taffy produced, so the clamp squared its percentage: `flex:0 0 50%; height:100%; max-height:50%` in a 400px row came out **100 against Chrome's 200**. Same one-line `!taffy_item` guard as t823, gate **EXTENDED not duplicated** (the t819 precedent), RED-proven. ⚠⚠⚠ **ONE ROW OF SIX WAS OBSERVABLE AND IT TOOK TWO MASKS.** First the px/pct asymmetry t823 named. Second, and new: a percentage `max-height` STILL hides unless the item also has a percentage `height`, because with `height:auto` the box is overwritten by `extract_placed`'s slot adoption **after** the clamp runs — **the wrong arithmetic produced the right box.** ⚠⚠ **A DEFECT MASKED BY A LATER ASSIGNMENT IS INVISIBLE TO EVERY TEST THAT CHECKS THE FINAL BOX** — unlike the px/pct mask (which hides behind an input that cannot express the bug, and so yields to input variation), this one hides behind a later WRITE and yields only to asking why a row that SHOULD be wrong is right. ⚠ HONEST SCOPE: all seven anchors byte-identical; no site claimed to move. A mechanism completed, not a corpus lever. ⚠ Process note: the residue was closed because t823 wrote it down as a GREP ("when a value is guarded, grep every other CONSUMER of it"), not as a hunch — it arrived four ticks later already named and already scoped, and took one fixture to convict |

| pattern | where it shows up | status |
| --- | --- | --- |
| **The off-canvas drawer** — `position:fixed` + `transform: translateX(-100%)`, slid entirely off-screen until a menu button reveals it | The mobile-menu idiom of the whole web; Tailwind ships it as `fixed … transform -translate-x-full`. Seen on `desiviral.net` (overlap 5) and `www.puentedemando.com` (overlap 18), both **already over the shape bar and held out of M1 by overlap alone** | 📏 t829 — **FOUR NAMED MECHANISMS, ALL REFUTED, EIGHTEEN Chrome-measured rows, all on the SHIPPING cascade.** The composite named the organ in one probe (Chrome: `<aside>` at `[-256 0 256x813]`, `position=fixed`, `transform=matrix(1,0,0,1,-256,0)`; ours places it ON the page, hence the overlap). Refuted: **(1)** transform reaching `getBoundingClientRect` on fixed/absolute/sticky/in-flow boxes (6 rows exact); **(2)** `var()` INSIDE `transform` — Tailwind's entire transform system — including a fallback on a MISSING var (5 rows exact); **(3)** a class selector STARTING WITH A HYPHEN, i.e. every negative Tailwind utility (4 rows exact); **(4)** the real Tailwind v3 chain — 6 `var()`s across 7 functions defaulted by `*,::before,::after{…}`, where ONE unresolvable var would invalidate the whole declaration at computed-value time (3 rows exact). ⚠ RESIDUE, narrowed and named: the next probe starts from **whether the rule reaches the element at all on the LIVE render** (the site's Tailwind is external; a sheet that does not arrive is `css-starved`), NOT from the transform pipeline. ⚠⚠⚠ **A NEGATIVE RESULT IS ONLY WORTH BANKING IF WRITTEN AS SPECIFICALLY AS A POSITIVE ONE** — "transforms work" is worthless; the row-by-row table is what stops four ticks being re-spent. ⚠⚠ **THE COMPOSITE NAMES THE ORGAN RELIABLY AND THE MECHANISM NEVER** — t814's rule (*a stated cause is a guess until measured on its own*) applies to a HYPOTHESIS exactly as it does to a residue |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A shrink-to-fit box that hugs its text ONE PADDING too tightly** — and its twin, **a box that re-wraps the run it was measured from** | Every shrink-to-fit context there is (flex item · grid item · inline-block · float · `position:absolute` · `display:table`) wrapping a padded/margined block child; and separately, ANY box sized to its own max-content whose content has a soft break. A footer link, a nav item, a button — anything that hugs text through a wrapper | ✅ fixed (tick 830) — **TWO defects, one symptom, aimed by t826's residue and priced against an OLD-BINARY CONTROL in the same hour.** ⓵ `content_right_extent` measures max-content by laying the subtree out at 1e6 and discarding any box that FILLED that width (its `rect.width` ≈ 1e6 is meaningless). That discard is **asymmetric**: the discarded box's LEFT padding/border/margin survive for free (baked into where its descendants were laid out, so they show up in the fragment's `x`), its RIGHT ones have nothing after them to carry them and are simply lost. Chrome vs ours, a `13.2px/17.16px Arial` run in a `border-box; padding:6.6px` block: flex item / inline-block / float / absolute / table **86.5 → we said 80.0**, all five; `margin:0 10px` on the child **93.3 → 83.3** (same loss, other property); padding on the box ITSELF was always right (the guard row that says which half was broken). Fix: carry the skipped box's right insets down the walk. ⓶ AND THE WIDTH BEING RIGHT WAS NOT ENOUGH — with ⓵ landed, `kicktipp.com`'s `<a>` matched Chrome's `103` exactly and was **still two lines tall**. max-content is read by laying the run out unbounded; the box is then given exactly that number and the run is laid out AGAIN, accumulating the same advances in a different order — and landing a few thousandths of a pixel OVER. Bisected: max-content `89.520px`, own re-layout needs `89.525px`. ⚠⚠⚠ **A BOX SIZED TO ITS OWN MAX-CONTENT MUST FIT ITS OWN CONTENT, AND ON A BARE `f32` IT DOES NOT.** Blink cannot reach this state because a preferred width is a `LayoutUnit` built with `FromFloatCeil` — quantised **outward**, never inward. Fix: `ceil_to_layout_unit` (1/64px, the t813-818 quantum, opposite rounding) on every intrinsic width — max-content, min-content, table-cell pair. PRICED, 14 sites, OLD BINARY vs NEW, same hour: **3 M1 CROSSINGS** (`www.kicktipp.com` 0.7349→**0.8313**, `celeb.gate.cc` 0.7284→**0.7832**, `www.library.chiyoda.tokyo.jp` 0.7472→**0.7528**), `en.wikipedia.org` +0.0177, mean **+0.0127**, **zero regressions, coverage byte-identical on all 14**. Two gates, each RED-proven by its own mutation. ⚠⚠⚠ **THE SECOND DEFECT WAS ONLY VISIBLE BECAUSE THE FIRST ONE LANDED** — a residue narrowed to "the width is wrong" would have been closed by ⓵ alone, and the site would still have failed; the aim held because the check was *the height against Chrome*, not *the width against my hypothesis*. ⚠⚠ **AND THE SECOND GATE WAS VACUOUS ON ITS FIRST WRITING** — the exact fixture that fails on the SHIPPING path passed under `MinimalCascade` with the fix mutated out (t826's two-cascade trap, arriving from the other direction: not a false positive but a false GREEN). It took a brute-force scan of 8 strings × 8 sizes under the mutated build to find one case (`13.2px "Terms and conditions"`) that the unit harness can actually see. **A gate written from a live-site reduction must be re-falsified under the harness it will live in** |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A FLOATED box is sized by a SECOND width/height resolution that never learned three of the block path's rules** — no intrinsic ratio, no `min/max-*` at all, and `box-sizing` on one axis only | `float:left` on an `<img>` is how the entire legacy web puts a logo or an icon in a header (`.logo a img{float:left}`); `.col{float:left;width:50%;max-width:600px}` is the pre-flexbox responsive column; `img{max-width:100%}` is in every CSS reset since 2011. The CrUX **tail** is legacy-heavy, so this is concentrated exactly where the burndown now measures | ✅ fixed (tick 831) — **THREE defects in one function, found together because they are one grep apart.** Aimed by `fidelity --shape-dump` (new: the per-element misses the scorer ALREADY computes and threw away) on the smallest member of a six-site cohort sharing `coverage 1.000000 · h_overflow 0 · overlap 0 · shape 0.55–0.66`. ⓵ **A floated replaced element has no content, so without its ratio it has no size** — `<img>` has no children, so the float path's content-height was **0** and its `auto` width was `shrink_to_fit` = **0**. Chrome vs ours: `float, no dims` **101x32 → we said 101x0**; `float, height=16` **16x16 → 0x16**; **the SAME image unfloated was already Chrome-exact** (the control that turns a symptom into a diagnosis — the two resolutions disagreed *inside one document*). ⓶ **`min-width`/`max-width`/`min-height`/`max-height` did not exist on this path** — not mis-applied, the words were not in the function. On plain floated `<div>`s (no replaced machinery to explain it away): `max-width:50px` **50x10 → 200x10**; `min-width:80px` **80x10 → 20x10**; `max-height:50px` **10x50 → 10x200**; `min-height:80px` **10x80 → 10x20**. With the clamp comes **CSS 2.1 §10.4 in BOTH directions**: `max-width:50px` on a 101×32 image is **50x16** (not 50x32), and `max-height:14px` over `<img height=16>` is **14x14** — which IS `app.ordertime.com`'s `.help img{max-height:14px;max-width:14px}`, the source of its `0x16`. ⓷ **`box-sizing` on the block axis** — the width arm got it in an earlier tick, the height arm four lines below did not (Chrome 100x100, ours 100x120); the existing float box-sizing test passed the whole time **because it only ever asked about width**. PRICED, 16 sites, OLD BINARY vs NEW, same hour: **2 M1 CROSSINGS** (`app.ordertime.com` 0.6552→**0.8621**, `littlecaesarsbcs.libellum.com.mx` 0.6154→**0.9487**), mean **+0.0338**, **zero regressions, coverage byte-identical on all 16**. Four gates, each RED-proven by mutating out its own half AND proven to leave the other three green. ⚠⚠⚠ **A SECOND IMPLEMENTATION OF A RULE DOES NOT INHERIT THE FIRST ONE'S FIXES — IT ACCUMULATES THE FIRST ONE'S BACKLOG.** `layout_float` has been acquiring `layout_block`'s rules one measured defect at a time for the life of this project, and each arrival looked like a small local fix rather than what it is: evidence that the *set* is incomplete. When a fix lands in one of two parallel resolutions, **enumerate the other one's rules against it** instead of waiting for the next site to name the next missing rule. ⚠⚠ **A TEST NAMED FOR A RULE GUARDS ONLY THE AXIS IT ASKED ABOUT** — `box_sizing_border_box_applies_to_a_float` is the *correct* name for a test that checked half the rule, so the name gave no signal that the other half was unguarded. A gate's NAME is a claim about scope, and nothing checks it. ⚠ **A COHORT IS A LEAD, NOT A CAUSE, AND BOTH HALVES ARE THE RESULT**: `littlecaesarsbcs` was never looked at, shared the mechanism, and moved **+0.33** on its own — while the four *other* cov=1.000 members did not move at all. ⚠ RESIDUE, banked with its numbers: `admin.zoomph.com` (unmoved, cov 1.000, 15 of 34 misses) is **one `<img>` inside a `<center>` at `320x30` where Chrome gives `113x30`** — we stretch a replaced element to its container instead of its intrinsic width, on the NON-float path this time; everything below inherits it. ⚠ but the page also carries an independent `-50` height and a `-30` body `y` that the img cannot explain, so the next tick measures **one suspect at a time** (t826's rule) rather than assuming one cause |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A per-fix old-binary control cannot see the regressions a fix causes** — and a cross-sweep delta cannot attribute the ones it finds | Every fix priced against a hand-picked site list, which is every render fix this loop has landed since t807 | 📏 t832 — **THE SWEEP FOUND A REGRESSION SIX PER-TICK CONTROLS COULD NOT, AND FOUR OF THE SEVEN CANDIDATES WERE NOT REGRESSIONS AT ALL.** The clean `--jobs 2` CrUX sweep (200/200) banked in-scope `shape>=0.75` **13.1% → 19.1%** and the M1 gate **10.0% → 13.0%** — the six unpriced fixes were worth what their controls said. But 7 sites fell >2pt against t825, and t830's and t831's old-binary controls had BOTH honestly reported zero regressions, over 14 and 16 sites, **none of which was any of these seven**. Attribution needed three binaries: `777juegos` 0.7317→0.7439, `beb88run` 0.5724→0.5847, `crm.majoo.id` and `dashboard.twitch.tv` byte-identical — **four were pure SITE DRIFT**, and `777juegos`'s 1.2pt swing on IDENTICAL CODE is larger than two of the three real regressions. The three real ones (`www.crazyshop.pl` -5.6pt, `www.kroftools.com` -2.2pt, `portagelearning.edu` -10pt) bisect to **t830, and specifically to its half ⓵** — neutering half ⓶ (`ceil_to_layout_unit`) moved them by exactly zero. Half ⓵ carries a discarded box's right insets into max-content, i.e. it makes intrinsic widths WIDER by construction, and all three victims are pages already 40–94% mis-placed (crazyshop's mega-menu `<ul>` is `185x1612` against Chrome's `740x468`, 610 of 1402 elements wrong *before* t830). **NOT REVERTED, and the call is stated rather than taken quietly**: half ⓵ is Chrome-verified on nine fixture rows across six shrink-to-fit contexts and bought three M1 crossings; no victim is near the bar or lost a crossing; reverting a Chrome-correct primitive to flatter three broken pages makes the engine LESS like Chrome, which is the north star inverted. ⚠⚠⚠ **A PER-FIX CONTROL IS EVIDENCE ABOUT THE SITES IT CONTAINS AND NOTHING ELSE.** It answers *"did my fix do what I aimed it at"*; it CANNOT answer *"did my fix cost anything"*, because the sites a fix costs are by definition the ones you were not thinking about. This is the real argument for the sweep cadence — stronger than the throughput argument the board has been making for it. ⚠⚠ **A SWEEP-TO-SWEEP DROP IS A QUESTION; ONLY A SAME-HOUR OLD-BINARY RUN IS AN ANSWER.** Acting on the cross-sweep list alone would have chased four ghosts and mis-attributed the rest. ⚠ Bisecting to a HALF of a tick (three binaries plus one neutering, four builds) is what turned "t830 did something" into a named mechanism — cheap against reverting a primitive worth three crossings. Related: [Session 654-672: the score's ERROR BAR] |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A `max-height` on a replaced element does not pull its width back through the ratio** — CSS 2.1 §10.4 run in one direction only | Any `<img>`/`<video>`/`<canvas>` under a height cap with an intrinsic ratio. `max-width:100%; max-height:<px>` is the standard "fit this logo in a header" pair; **AWS Cognito's hosted login UI ships it verbatim** (`.logo-customizable`), so every Cognito-authenticated app carries it | ✅ fixed (tick 833) — `layout_block` has had the inline→block half for a long time (a `max-width` that moves the used width recomputes the height); the block→inline half was never written, so a height clamp left the box with its PRE-CLAMP width and the picture rendered stretched. Chrome-measured, 1000×266 image in a 320px block: `max-width:100%+max-height:30px` **113x30 → we said 320x30**; `max-height:30px` alone **113x30 → 1000x30**; `max-width:100%` ALONE **320x85, already correct** ← the control that names which half was missing; `+display:block;margin:0 auto` **113x30 @x=104 → we said @x=0**. That last row is why the fix re-runs the auto-margin split instead of only assigning a width — §10.4 says the §10.3.3 rules are applied AGAIN and §10.3.3 is where two `auto` margins share the remainder, so a width assigned without re-splitting leaves a correctly-sized image flush left: **a new bug wearing the old one's fix**. Moving the width that late is safe only under the `is_replaced_element` guard (a replaced box has no children, so nothing was laid out against the old width). Gate RED-proven TWICE — once by removing the transfer, once by removing ONLY the auto-margin re-run while keeping the width fix, so the halves are separately falsifiable. ⚠⚠⚠ **TWO IMPLEMENTATIONS OF ONE RULE DRIFT IN WHICHEVER DIRECTION THE LAST FIX LANDED — SO THE GREP IS SYMMETRIC OR IT IS NOT A GREP.** t831 landed BOTH §10.4 directions in `layout_float` and concluded the float path was the one accumulating the block path's backlog; one tick later the block path was missing a direction the float path now had. The t831 lesson as written was too narrow. ⚠⚠ **HONEST SCOPE — ZERO M1 CROSSINGS.** Priced against the t831 binary, 16 sites, same hour: `admin.zoomph.com` +0.0294 (to 0.5882, still far under the bar), `crazyshop.pl` +0.0007, 14 unchanged, zero regressions, coverage byte-identical. A completed spec rule, not a corpus lever — same shape as t827 and labelled that way rather than dressed up. ⚠ **A DOM PATH IS WHERE A SYMPTOM WAS FOUND, NOT EVIDENCE ABOUT ITS MECHANISM**: t832's residue named `<center>` because that is where the bad box lived, and the first fixture refuted it in one line (the same image with no `<center>` measured identically). A residue should name what was MEASURED — natural size vs used size — not what was merely nearby |

| pattern | where it shows up | status |
| --- | --- | --- |
| **An absolutely positioned replaced element is ZERO PIXELS TALL** — the third implementation of replaced-element sizing, and the only one that produced no box at all | `position:absolute; top:0; left:0` on an `<img>` — the hero / overlay / card-thumbnail / lightbox idiom of the whole web, whenever it is written WITHOUT both insets | ✅ fixed (tick 834) — `layout_abs` took its height from `definite_ch` (explicit `height`, or both `top` AND `bottom`) or else from the CONTENT height, and **a replaced element has no children**, so the box measured `<w>x0` and painted nothing. Chrome-measured, 1000×266 image absolutely positioned in a 320×200 block: `max-width:100%` **320x85 → we said 320x0**; `max-height:30px` **113x30 → 1000x0**; both **113x30 → 320x0**; `min-width:1500px` **1500x399 → 1500x0**. Every `before` HEIGHT is 0 and every `before` WIDTH but one is already right — the min/max clamps reached this path in an earlier tick and the ratio never did. ⚠⚠⚠ **THE `inset:0` VARIANT HAPPENED TO WORK, WHICH IS WHY IT SURVIVED**: both insets make `definite_ch`, so the most-cited spelling of the idiom was fine while the other equally-common spelling was invisible. **A DEFECT WHOSE CANONICAL FORM WORKS IS NOT DISCOVERABLE FROM THE CANONICAL FORM** — enumerating the WAYS a pattern is written is a different search from enumerating the patterns, and only the first finds this class. ⚠⚠⚠ **THE FALSIFICATION PASS DELETED A THIRD OF THE FIX.** All three rules were written onto this path for symmetry; mutating each out separately, the §10.4 inline→block half left the gate **GREEN**, because the auto-height arm already derives from the width *after* the clamp — so it could only recompute the number it had just computed. Removed, with the reason left where the code would have gone so the next symmetric grep does not re-add it. **A copy of a rule added for SYMMETRY is unreachable code guarded by a test that cannot fail** — this project's own definition of a vacuous gate, nearly committed while quoting the lesson about them. **FALSIFY EVERY HALF SEPARATELY, INCLUDING THE HALVES ADDED FOR TIDINESS**: the whole-fix test passed either way. ⚠⚠ **A LESSON RECORDED IS NOT A LESSON EXECUTED** — t833 wrote *"the grep is symmetric or it is not a grep"*; running it one tick later found a worse defect than the one that produced the lesson. ⚠ HONEST SCOPE: **zero movement on the 16-site control** (t833 binary vs new, same hour, all byte-identical) — none of those sites absolutely-positions an image without both insets, which is exactly the limitation t832 named. The witness is the Chrome table and the next full sweep, not a control padded until something moves |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A flex item `<img>` reports its content size as ZERO, so `min-width:auto` floors it at nothing and the row shrinks to slivers** | Every `display:flex` + `overflow-x:scroll` carousel of logos/badges/thumbnails — the standard mobile-first horizontal scroller. Seen on `promo.golesliga1max.pe`, where ONE such row is 15 of the site's 26 shape misses | ✅ fixed (tick 835) — CSS Flexbox §4.5: a flex item's `min-width:auto` is its content-based minimum, which for a replaced element is its INTRINSIC width, so Chrome refuses to shrink and lets the container overflow (the whole point of the carousel). Cause: `replaced_default_size`, the seam that tells taffy how big a replaced item is, listed `svg\|canvas\|video\|object\|embed` and **not `<img>`**. Chrome vs ours, four 1000×266 images in a 320px flex row: **1000x266 each → we said 68x266 each**. PRICED vs the t834 binary, 16 sites, same hour: `promo.golesliga1max.pe` **0.5873 → 0.8254 ★ CROSSED 0.75**, adversarial control `ta3lemkonline` +0.0284, mean +0.0163, zero engine regressions. ⚠⚠⚠ **AN EXCLUSION WRITTEN FOR ONE REASON WAS SILENTLY DOING A SECOND JOB.** `<img>` was off that list because it has no DEFAULT OBJECT SIZE — correct — and the same line also kept it from reporting its INTRINSIC size, a completely different question. **When a guard tests a TAG rather than the PROPERTY it cares about, it is read as authoritative for every property that tag has**; the fix is two guards on two properties, not one list. ⚠⚠⚠ **AND THE FIRST VERSION OF THE FIX SHIPPED A REGRESSION NO FIXTURE CAUGHT.** Admitting `<img>` also handed it the `300×150` fallback. The guard written pre-emptively for that — return `None` when NEITHER axis nor a ratio is known — was **the wrong guard**: the live failure is an image with a definite WIDTH and no ratio (`777juegos.com`'s unloaded footer icons, which Chrome measures at height **0**), which sailed past it and took the 150px height, costing **-8.75 shape points**. **A CORRECTIVE GUARD WRITTEN FROM THE SAME HYPOTHESIS AS THE FIX CANNOT FALSIFY THE FIX** — and I believed it precisely because I had written it. Only the 16-site control found it, one tick after t834 recorded that a control is evidence about the sites it contains. ⚠⚠ **A KNOWN-DRIFTY SITE STILL DESERVES THE REPEAT, NOT THE BENEFIT OF THE DOUBT**: `777juegos` had a recorded ±1.2pt spread (t832), which is exactly the reason to run it twice per binary rather than to wave an 8.75pt drop away as more of the same — two runs per binary made it deterministic and ~6pt. ⚠ RESIDUE, bounded WITH refutations: a `min-width:0` flex image is `160x43` in Chrome and `160x266` here — **not** the measure seam (that derivation moved no number and was deleted under t834's rule), **not** cross-axis `stretch` (`align-items:flex-start` is identical). It is taffy applying `aspect_ratio` to the SPECIFIED width rather than the FLEXED one; the fix is in the slot adoption on the way out |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A flexed replaced item keeps the cross size of its UNFLEXED width** — the flex line's cross size is computed from specified rather than flexed main sizes | Any `display:flex` row containing an image that actually shrinks (`min-width:0`, or a `flex-shrink` that bites) — i.e. every responsive image row that is not a carousel | 📏 t836 — **THE OBVIOUS FIX IS WRONG AND ONE FIXTURE PROVED IT.** t835 banked this residue having already refuted the measure seam and `align-items`, and named the slot adoption as the remaining suspect. The mechanism is exactly there: `extract_placed` calls `layout_block`, which correctly derives `160/3.759 = 42.6` from taffy's flexed width, and three lines later the slot-height adoption (*"when its own height is auto, adopt taffy's slot height so it fills its flex line"*) sees `266 > 42.6` and **overwrites the correct number**. Guarding that adoption for ratio-sized replaced items produced Chrome-exact output on THREE fixtures — `min-width:0` images **160x43 ✓**, with `align-items:flex-start` **160x43 ✓**, unshrunk carousel row **1000x266 ✓ control**, `<div>` flex item still stretches **60x120 ✓ control**. ⚠⚠⚠ **AND THE FOURTH FIXTURE KILLED IT**: an image beside a 120px-tall sibling is **308x120 in Chrome** — Chrome DOES stretch a ratio-sized replaced item — so the rule the guard would encode (*"a ratio-sized replaced item never adopts the line's cross size"*) is FALSE. **THE FIX WAS REFUSED, NOT DEFERRED**: it makes two families exact and moves the third from 146px wrong to 38px wrong, a real net improvement, but it encodes a rule this tick MEASURED to be false behind a comment that would read as authoritative — and t817's rule decides it, *a wrong fix is caught by the next gate, a wrong LABEL is caught by nothing.* ⚠⚠⚠ **TWO FIXTURES AGREED WITH THE WRONG RULE BECAUSE BOTH WERE DEGENERATE**: when all the flex items are the images themselves, the line's cross size IS the ratio-derived height, so *"don't stretch"* and *"stretch to a line that happens to equal the ratio height"* produce identical output. **A FIXTURE FAMILY THAT CANNOT DISTINGUISH TWO HYPOTHESES IS ONE FIXTURE, HOWEVER MANY ROWS IT HAS** — ask what varies BETWEEN the rows; mine varied `align-items` and item count, neither of which changes the line's cross size. ⚠⚠ **A RESIDUE CAN NAME THE RIGHT MECHANISM AND STILL POINT AT THE WRONG LAYER** — t835 was right that taffy uses the specified width and wrong that the fix lives in the slot adoption, because the symptom surfaces one item at a time while the cause is a property of the LINE. RESIDUE, re-banked one layer up: the flex-line cross size must come from HYPOTHETICAL cross sizes computed on FLEXED main sizes; the fix is upstream of taffy or a correction pass over the placed line, and **anything touching one item cannot fix a number that belongs to the line** |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A percentage height on an out-of-flow box resolves against the DOCUMENT, not the viewport** | Every full-height drawer, modal backdrop, off-canvas menu, overlay and scrim — `position:fixed; height:100%` — on any page long enough to scroll. Seen on `possssno.sbs`, whose `#aside` came out **4462px** tall against Chrome's **713** | ✅ fixed (tick 837) — the out-of-flow pass built its containing block as `Rect { width: viewport_w, height: root.content_bottom() }`; **the variable was named `viewport` and held the whole scrolled document.** CSS 2.1 §10.1: the ICB has the VIEWPORT's dimensions, and a `position:fixed` box's containing block IS the viewport. Chrome-measured on a 3000px page in an 800px window (innerHeight 713): `fixed height:100%` **300x713 → we said 300x3000**; `fixed height:50%` **100x357 → 100x1500**; `absolute height:100%` **100x713 → 100x3000**; `fixed height:auto` **100x50 → 100x50 ✓ control**. ⚠⚠ **THE IN-FLOW ICB ALREADY HAD THIS RIGHT** (`icb_height` from `viewport_size()`, sixty lines up, with a comment explaining why a root `height:100%` must fill the window) — one rule, two implementations, only one ever corrected, the third instance of that shape in a week (t831, t833, t837). ⚠⚠⚠ **THE NAME IS WHY IT SURVIVED**: every reader downstream took `viewport` as the specification. *A wrong fix is caught by the next gate; a wrong LABEL is caught by nothing* — recorded about wiki prose (t817) and about a data-column string (t824), and here it is in an **identifier**, the one place a wrong label also compiles. PRICED vs the t835 binary, 19 sites, same hour: `app.ordertime.com` **0.8621 → 1.0000** (perfect), golesliga +0.0159, mean +0.0065, **zero crossings, zero regressions**. ⚠⚠ **THE SITE THAT AIMED THE TICK MOVED 0.0035** — `possssno.sbs` has 172 misses and `#aside` was the LARGEST, but a score counts elements within tolerance, so one subtree is worth almost nothing. **RANK BY FREQUENCY, AIM BY MAGNITUDE, AND DO NOT CONFUSE THEM.** ⚠⚠ **TWO ROWS READ AS REGRESSIONS IN THE SINGLE-DRAW CONTROL AND NEITHER WAS ONE** — repeated twice per binary, `ta3lemkonline` is deterministic 0.5448→0.5492 (an IMPROVEMENT; the old 0.5733 was the outlier) and `777juegos` is 0.7439 on both. Third time this session a single row was not a result: **a single row is a draw; the repeat is the measurement.** ⚠ REFUTED en route, and worth more than the fix: `possssno.sbs`'s footer is horizontally MIRRORED under `<html lang="fa" dir="rtl">`, and **RTL is NOT the cause** — a `dir="rtl"` inline fixture measures `a1` at **492 against Chrome's 493**, with the LTR and `text-align:left` rows also exact. Our inline base direction is Chrome-correct; the mirroring is still unexplained and `reading_order 515/575` is its CONSEQUENCE, not evidence about it |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A 1×1 lazy-load placeholder is not a missing image — it is a WRONG RATIO** | The dominant lazy-load idiom of the WordPress/theme web: `src="data:image/gif;base64,…1×1…"` + `data-src` + `data-srcset`, swapped by the site's own JS. `gismart.com`: `data-lazy` **41** · `data-src` **41** · `data-srcset` 17 · `loading="lazy"` **0** | 📏 t838 — **MEASURED, DELIBERATELY NOT DIAGNOSED.** Aimed at the nearest scorable site to the bar (shape 0.7153, gap **0.0347**, cov 0.983). `--shape-dump` showed a cluster pointing BOTH WAYS: `258x258` in Chrome vs **258x851 / 258x928 / 258x1005** ours, and `419x851` in Chrome vs **419x419** ours. `boxes --images` gave it in one line — **`natural 1x1`** on every one. The placeholder is a real, decoded 1×1 GIF, so `apply_natural_size` hands the element an intrinsic **ratio of 1:1**, and then every sizing rule this session fixed works perfectly from a wrong premise. ⚠⚠⚠ **THE BOXES ARE NOT MISSING — COVERAGE IS 0.983.** They are the shape a 1×1 implies instead of the shape the real asset implies, so **the error's SIGN depends on the real image**, which is why the cluster ledger reads it as scattered geometry noise and never as one cause. **A lazy image that never loads does not render nothing; it renders confidently at the wrong aspect ratio.** Fourth sighting of *a wrong answer of the RIGHT TYPE* (`typeof null==='object'`, the correct-but-empty Array, half-installed `performance.mark`, now a correct-but-placeholder image) — invisible to every check that asks whether a thing is THERE. ⚠⚠ **A CONFIRMED API IS NOT A CONFIRMED CAPABILITY WHEN THE CAPABILITY IS A CHAIN**: `IntersectionObserver` has been `confirmed` on the surface map for hundreds of ticks and the map still could not predict that 41 images on a near-bar site would render at the wrong shape. Map rows must name the CHAIN (*a lazy image reaches its real `src`*), not the API that is one link of it. ⚠ **THE CORPUS SHIPS THE ATTRIBUTE THE ECOSYSTEM INVENTED, NOT THE ONE THE SPEC ADDED** — ranking by spec surface ranks the wrong thing. ⚠ RESIDUE with its discriminator named: two candidates survive — (a) the site's lazy script never completes for us, or (b) it is `IntersectionObserver`-gated and all 41 images are below the fold (y = 1261…14216 against a 720px viewport) so the observer legitimately never fires for a page we never scroll, while the oracle's Chrome rasterises full-page. Different fixes, so t826's rule applies: **one suspect at a time**. **The discriminator is cheap — find one `data-lazy` image ABOVE the fold and see whether its `src` swapped**; none of these 41 is, which is exactly why they cannot be separated on this page. This is a FUNCTION-leg row, and check #70 had just recorded the function leg (scorability flat at 101/131 = 77.1%) as the real ceiling |

| pattern | where it shows up | status |
| --- | --- | --- |
| **`IntersectionObserver.observe()` never delivered its INITIAL observation** — so on a static page the callback never fired at all | Every lazy-load image, virtualized list, sticky latch and "load more" sentinel that is ALREADY on screen at first paint — i.e. the above-the-fold half of the entire real-time web | ✅ fixed (tick 839) — `observe()` was `function(el){ this._targets.push(el); }`: it recorded the target and waited, and `__runObservers` is called by the engine *after a layout or a scroll*. Page scripts run after the initial layout and a headless render never scrolls, **so for a static page the callback never ran once**. Intersection Observer §3.2 queues the update-observations steps on `observe()`, which is why every browser delivers one callback per observed element with no scroll, resize or layout change. Fixture of three `data-src` + 1×1-GIF images in an 800px viewport (`real.png` 400×100, so box geometry reads the answer): DOMContentLoaded swap **400x100 ✓ CONTROL**, IO above the fold **400x100 → we said 400x400 ✗→✓**, IO below the fold (see residue). ⚠⚠⚠ **THE CONTROL ROW WAS THE WHOLE DIAGNOSIS**: the `DOMContentLoaded` image swapped BEFORE this tick, so our JS runs, reaches the DOM and sets `src`, and the engine fetches it — refuting the "the site's script never completes" branch in one line, exactly as the unfloated image did at t831 and the `max-width`-alone row at t833. **Put the already-working spelling of the thing in the same fixture, always.** ⚠⚠⚠ **AND IT HID BEHIND ITS OWN GATE FOR 780 TICKS.** `G_VIEWPORT` has asserted "the whole lazy-load loop" since t59 and **its probe scrolls** — it proved *viewport moves → scrollY → IO fires → src swaps → engine fetches*, all real. Its `seen:` string was `io-fired,scroll@2000`: one firing, after the scroll. It is now `io-fired,io-fired,scroll@2000`. **A GATE'S SHAPE CAN ASSERT A CASE AWAY** — the gate did not merely omit the no-scroll path, its SETUP made the question unaskable, and the falsifier for this class is not *"does it go red?"* (it did) but *"what does this gate's setup make unaskable?"* Gate EXTENDED not duplicated (one `#[test]` per JS gate — a second SIGSEGVs), RED-proven by mutating out the schedule call (`eager:NONE`). ⚠⚠ HONEST SCOPE: **21-site control, mean +0.0002, ZERO crossings, zero regressions** — a real Chrome-verified capability fix with essentially no corpus movement, **and the reason is the measurement frame, not the fix**: all 41 of `gismart.com`'s lazy images sit below the fold (y = 1261…14216) and the initial observation, run against the real 720px viewport, correctly reports them as NOT intersecting, while the oracle's headless Chrome effectively observes the whole document. ⚠ RESIDUE, a POLICY question not a bug: **we lay out the entire document while telling IO the viewport is 720px tall** — both halves defensible alone, inconsistent together. Options: observe against the laid-out document during a full-page render (matches the oracle, wrong for a real browser) · drive a synthetic scroll before scoring (matches a real browser, costs render time) · accept the divergence and stop scoring below-fold lazy images. It decides how much of the corpus's image geometry is reachable at all, so it wants a MEASUREMENT, not a preference |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A flex item grows to its content where Chrome holds it to the viewport-height line** — `div#app-container` on `777juegos.com` | An app-shell root inside a `height:100vh` flex wrapper — the standard SPA mount point | 📏 t840 — **NOT DIAGNOSED; TWO OBVIOUS MECHANISMS REFUTED.** The site is the cheapest crossing on the board (shape **0.7439**, **0.006** from the bar, 80 scored elements) and its dominant miss is `1200x713` in Chrome vs `1200x1001` for us — **713 is exactly Chrome's `innerHeight`**, and the site's only viewport rule is `.load-app{display:flex;height:100vh}`. ⓵ **`height:100vh` is CORRECT — refuted**: flex, block, and flex-with-1000px-overflowing-child all measure 720 for us against Chrome's 713 (the 7px is our harness viewport vs `innerHeight`, the same seen at t837). ⓶ **flex-item stretch against a definite line is CORRECT — refuted**: an `auto`-height item with 900px of content in a 400px line is **300x400 in both**, with and without `overflow:hidden`. RESIDUE, narrowed: the wrong box is `div#app-container` itself (`position:relative`, a flex item) inside a **correctly-sized** `1200x720` flex parent, so the error originates AT that element rather than in its chain; untried and cheapest first are `position:relative` on a flex item, a `min-height` interaction, and a post-load JS resize (this page's coverage reads 0.941 on one run and 0.965 on the next, so it mutates after load). ⚠⚠⚠ **THE CHEAPEST CROSSING ON THE BOARD IS NOT THE CHEAPEST TICK** — distance to the bar ranks the PRIZE and says nothing about the SEARCH. ⚠⚠ **A NUMBER THAT MATCHES A NUMBER FROM LAST TICK IS A MAGNET**: `713` had just been t837's answer and pulled this tick straight at the viewport-height family; **a coincidence of magnitudes is not evidence of a shared mechanism**, and it cost one fixture to refute only because the fixture was sharp. ⚠ `boxes --why` earned its place beside `--shape-dump`: the dump ranks misses but flattens the tree, and only the chain showed the bad box's parent was already Chrome-exact — turning *"a subtree is wrong"* into *"one element is wrong"* |

| pattern | where it shows up | status |
| --- | --- | --- |
| **UAX #9 rule L2 was never applied to a line's INLINE BOXES** — bidi was implemented for glyphs inside one text run and stopped there, so one Arabic word was right and twenty Arabic links were exactly backwards | Every RTL page — Arabic, Hebrew, Persian, Urdu. Any nav bar, footer link list, breadcrumb, tag row or sentence containing `<a>`/`<span>`/`<em>`/`inline-block`. `possssno.sbs` (`<html lang="fa" dir="rtl">`): **503 of 575 elements misplaced at coverage 1.000**, `reading_order` 524 | ✅ fixed (tick 841) — `FontContext::shape_bidi` runs `unicode_bidi` over a run's text and is gated by `g_bidi_base_direction`; that is UAX #9 **inside one run**. A LINE is a sequence of BOXES, each measured and placed separately by `layout_inline`, and nothing reordered those. `close_line` now runs L2 over the line's fragments, after justification and before the alignment offset. Chrome-measured, 400px containers, x relative to container: `dir=rtl` three RTL-script `<a>` **370/343/312 → we said 312/343/370 ✗→✓**; the same runs inside `dir=ltr` **58/31/0 → we said 0/34/61 ✗→✓**; `dir=rtl` three **LATIN** `<a>` **303/334/364 ✓ CONTROL, unchanged**. ⚠⚠⚠ **A SPEC IMPLEMENTED AT ONE GRANULARITY READS AS IMPLEMENTED** — bidi was present, correct, cached and gated, and every instrument that asked *"do we do bidi?"* answered yes truthfully. t838's *"a confirmed API is not a confirmed capability when the capability is a CHAIN"*, with the chain running **up**: ask of every algorithm *at what granularity is this implemented, and does the spec define it one level up?* ⚠⚠ **THE CONTROL ROW REFUTED THE CHEAP VERSION OF MY OWN FIX** — Latin-in-RTL is one LTR run at level 2 and keeps source order, so *"the container is RTL, reverse the boxes"* gets rows 2 and 3 wrong; all three fall out of the LEVELS. Fourth consecutive tick where the control row WAS the diagnosis (t831, t833, t839). ⚠⚠ **SUPERSEDES t837's refutation**, which read *"RTL is NOT the cause — a `dir=rtl` inline fixture measures a1 at 492 against Chrome's 493… our inline base direction is Chrome-correct"*: both halves true, conclusion false. The base direction IS correct (which is why the line is flush right) and **that fixture was Latin text** — the one case that must not reorder. **A fixture built from the alphabet you type cannot ask a question about the script the page is written in.** ⚠⚠ IMPLEMENTATION: spaces are modelled as ITEMS (`Frag(i)` \| `Space(w)`), not as gaps — reversing positions in place and mirroring gaps is correct for one level and composes **wrongly** with two, because the array stays in logical order while the nested reversal has already moved its members. Total line advance exactly conserved. Inert on LTR by construction: no odd level ⇒ empty L2 range ⇒ no `x` touched. PRICED vs the t840 binary, 20 sites, same hour: **possssno 0.6974 → 0.8783, CROSSING the bar, reading_order 524 → 1**; `ta3lemkonline` (the standing adversarial control) 0.5492 → 0.5733; 17 flat. ⚠⚠ **THREE APPARENT REGRESSIONS, ALL REFUTED BY SAME-BINARY SPREAD** — `nysainfo` produces *the identical pair* 0.749263/0.748159 under BOTH binaries, and `fragrantica`'s own spread (0.011) is larger than its "drop" (0.0128) and brackets it. **The cheap instrument is not more runs of the comparison, it is two runs of the OLD binary alone.** ⚠ HONEST SCOPE: only **3 of 200** corpus sites carry `<html dir=rtl>`, so the corpus cannot price this; the corpus is a CrUX sample of the English-reading web and RTL is a population, not a tail. RESIDUE: a fixed-width block in an RTL containing block is flush LEFT for us, flush RIGHT in Chrome (CSS 2.1 §10.3.3 ignores `margin-left` under `rtl`) — one miss per block, but it is why an RTL page's whole sidebar is on the wrong side |

| pattern | where it shows up | status |
| --- | --- | --- |
| **M1's two conjuncts are not interchangeable, and the ranked worklist ranks only one of them** — nine ticks of SHAPE work moved `shape≥0.75` by zero net sites while one fix to the READING-ORDER term moved the gate 1.6 points | The burndown itself. `PHASE0-RENDER-BURNDOWN.md` §3 ranks *shape* mechanisms (width→dy laundering, flex/grid column sizing, sub-pixel line-count drift…) and has **no row for the jarring conjunct at all**, though M1 is `shape≥0.75 AND jarring-clean` | 📏 t842 — clean `--jobs 2` CrUX sweep, 200/200. **M1 13.0% → 14.6%** (17 → 19 of 130) — beating the stated pre-run expectation of 13.0-13.7% — while `shape≥0.75` went **25 → 25 (zero net)** and the common-set band over 100 sites scored in both sweeps was **−0.26 pts (flat)**. Every one of the +1.6 points came through **jarring-clean 26.7% → 29.2%**, and the row diff names it: `possssno.sbs` reading_order **515 → 1**, shape 0.6974 → 0.7896 (t841's UAX #9 rule L2); `app.ordertime.com` reading_order 1 → 0, shape 0.8621 → 1.0000 (t837). ⚠⚠⚠ **RANK BY WHAT THE GATE IS A CONJUNCTION OF, NOT BY WHAT THE LARGEST COLUMN IS.** `reading_order` is non-zero on **5 of the 6** sites in the fresh crossing-ranked near-bar table, so the term the plan does not rank is the term the near-bar cohort is actually blocked on. ⚠⚠ **THE BOARD'S OWN STEER WAS RIGHT IN ITS ARITHMETIC AND WRONG IN ITS CONCLUSION** — *"the RTL arc crossed ~0 because corpus-crux-trend is RTL-light"*: the corpus IS RTL-light (3 of 200), and t841 still produced the window's only clean crossing and all of its M1 movement. **A corpus with few instances of a defect can still have that defect's METRIC TERM as its binding constraint.** Frequency ranks how many sites a fix touches; it does not rank how much of the GATE each touch buys. ⚠⚠⚠ **THE LOUDEST ROW IN A SWEEP IS THE ONE MOST LIKELY TO BE THE SWEEP.** Four sites fell below the bar, headed by `secure5.entertimeonline.com` **0.8205 → 0.0000** at coverage 1.000 → 0.731 — the largest single-site drop this loop has recorded, and under the ratchet the loudest possible row. Re-measured alone with the t840 binary and the t841 binary, twice each: **0.820513 on all four runs, to six decimals**; `simplepdf.com` likewise deterministic at 0.742138 on both binaries; `sestra.cc` and `www.puentedemando.com` inside their own same-binary spread, with the new binary at the TOP of the old one's range. **Zero attributable engine regressions.** A shape of *exactly zero* at coverage 0.73 is a page state, not a layout engine — **re-measure an outlier alone before diagnosing it**, the second-cheapest instrument in this loop after the control row. ⚠ Scorability 77.1% → **79.2%** (scored 101 → 103; `www.freesupertips.com` unscored → 0.7517). Other gain: `promo.golesliga1max.pe` 0.5873 → 0.8413 |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A `position:relative` ancestor inside an out-of-flow subtree was invisible as a containing block** — so every abspos box under it escaped to the OUTER positioned ancestor | Every off-canvas menu, drawer, dropdown panel and fixed toolbar whose rows carry their own badges, carets or absolutely-placed icons. AdminLTE 2.4.5 is the canonical instance: `.main-sidebar{position:absolute}` > `section` > `ul` > `li` > `a{position:relative}` > `span.pull-right-container{position:absolute;right:10px;top:50%}`. `ubys.bingol.edu.tr`: 14 carets, `reading_order` 19 | ✅ fixed (tick 843) — `position_absolutes` builds its rect map from the IN-FLOW fragment tree, and `abs_containing_block` tests `position != Static` and *then* requires a rect, **walking straight past any ancestor it cannot find**. `353` is `viewport/2 − 7`: `top:50%` against the drawer, whose `min-height:100%` makes it viewport-tall. Reduced from the real AdminLTE stylesheets to 12 lines of CSS; Chrome-exact after on all three rows plus a no-positioned-ancestor control. ⚠⚠⚠ **A WRONG CONTAINING BLOCK IS ONLY AS VISIBLE AS THE INSETS THAT DISTINGUISH IT** — `right:10px` is a LENGTH and the drawer and the row share a right edge, so `x` came out **correct from the wrong containing block** (210 in both engines, every row), and the defect presented as a `top:50%` percentage bug in the exact family t837 had just worked. **When one axis of a two-axis primitive is wrong, check whether the other is right BY ACCIDENT before naming the axis as the subject.** ⚠⚠⚠ **A CONSTANT IS NOT A LAYOUT ERROR, IT IS A CONTAINING-BLOCK ERROR** — fourteen elements at fourteen different `y` in Chrome and ONE `y` in ours; no per-element arithmetic produces that. **Read the VARIANCE of a cluster before its magnitude: the magnitude names a number, the variance names a scope.** ⚠⚠ **THE OBVIOUS ONE-LINE FIX BROKE TWO GATES, ONE OF WHOSE CONTROL ROWS WAS WRITTEN FOR EXACTLY THIS MISTAKE.** `rects.extend(b.node_rects(dom))` looks right: `node_rects` **LIFTS** a boxless element's geometry up the DOM until an ancestor has a box *in the tree it was called on* — correct for the whole-document call, inverted from inside an out-of-flow subtree where EVERY ancestor is boxless. `#modal`'s rect propagated onto its own containing block, so the next abspos sibling resolved against `[100 100 200x200]` instead of `[0 0 400x400]`, and a `position:static` inline acquired geometry it must never have. The lift must STAY (a relative INLINE inside a drawer is a legal containing block, CSS 2.1 §10.1), so the union is kept and everything above the box is filtered out. **A helper that walks UP the DOM has an implicit precondition about WHICH TREE it is walking, and that precondition is not in its signature.** PRICED vs the t842 binary, 22 sites, same hour: `ubys.bingol.edu.tr` **0.8434 → 0.9277, reading_order 19 → 1 (M1 CROSSING)**; `www.unoeste.br` 0.7766 → 0.7855 with reorder 3→1 and overlap 3→2 (**M1 CROSSING**); `payb.jp` +0.0223; `www.tz.de` +0.0124; 14 flat. **M1 19 → 21 of 130 (14.6% → 16.2%), zero attributable regressions** — the three sites that went down were each refuted by their own same-binary spread, including `possssno.sbs`, whose `position:fixed` aside is exactly this shape and which read **0.897391 / reord 1 on all four runs, both binaries** |

| pattern | where it shows up | status |
| --- | --- | --- |
| **`-webkit-line-clamp` DELETES the lines past the clamp; Chrome lays them out and CLIPS** | Every Tailwind `line-clamp-N` card title, teaser and excerpt — the dominant truncation idiom of the modern content web. `desiviral.net`: **68 hits**, `h2 > a` is `197x58` in Chrome and `197x38` for us, plus 72 downstream `y ~44px` drift hits that are the same difference cascading | 📏 t844 — MEASURED AND NARROWED, deliberately NOT fixed. The `<h2>` carries `display:-webkit-box; -webkit-line-clamp:2; -webkit-box-orient:vertical; overflow:hidden`. Same x, same WIDTH, one line short — so it is **not** a width error laundering into line count (burndown family #1), it is the clamp itself: the clamped BLOCK is the right height in both engines, and only the descendant inline's rect disagrees, because Chrome's inline unions all three of its line boxes and ours only has two to union. ⚠ **NOT FIXED, and the reason is tick selection rather than difficulty**: it is a SHAPE term on a site whose M1 blocker is OVERLAP, so fixing it crosses nothing, and making the clamp clip rather than drop changes the paint path and the premise of the existing `line_clamp_caps_lines_and_appends_ellipsis` gate. **A cluster's size ranks how much of a page it explains, not whether fixing it moves the gate.** ⚠⚠ **A COHORT SELECTED BY A METRIC IS NOT A COHORT WITH A MECHANISM** — the three sites that are already `shape≥0.75` and fail M1 on jarring alone (`www.freesupertips.com` reord 4, `payb.jp` reord 6, `desiviral.net` overlap 5) have **three unrelated causes**. The selection is still right (each is one mechanism from a crossing) but *"find the shared root cause"* had no answer, and hunting one would have manufactured a false family |

| pattern | where it shows up | status |
| --- | --- | --- |
| **The agent's click point IS layout geometry — so a containing-block bug is a silent MIS-ACTUATION, and no agent-side gate could see it** | Every drawer, sidebar, menu and toolbar an agent actuates. Before t843, all 14 carets of an AdminLTE sidebar reported the **same centre**: an agent told to expand the seventh menu item would have clicked the same pixel as the first | ✅ gated (tick 845) — `every_drawer_row_exposes_its_own_click_point_to_the_agent`, in `manuk-page` so it runs the whole chain (HTML → Stylo cascade → layout → a11y tree → click point). Two assertions: three rows expose three DISTINCT click points, **and** each point falls inside its own row — distinctness alone would pass if every control were merely displaced by a *different* wrong amount. RED-proven by reverting t843's `rects.extend(...)`: carets 0 and 1 collapse to `(215, 66)`. ⚠⚠⚠ **THE CLAIM THAT MOTIVATED THIS GATE WAS WRONG, AND CHECKING IT BEFORE BUILDING ON IT IS THE POINT.** Constitution check #71 (written one tick earlier) asserted that `reading_order` is the order the agent walks the page in, so t841's RTL fix meant `manuk-agent` had been reading RTL navigation *backwards*. Three greps refuted it: `A11yNode::iter()` is **DOM pre-order** (`automation.rs:61`, `forms.rs:63`, `translate.rs:76` all say "document order"), and DOM order is already correct for RTL. **I reasoned from the instrument's VOCABULARY instead of from the CONSUMER's code** — the same failure as a wrong wiki label (t817) and a wrong data-column name (t824), one level up. ⚠⚠ **THE CORRECTED CLAIM IS THE BETTER ONE**: "the agent reads RTL backwards" was alarming, false, and *ungateable* (a correct thing cannot be RED-proven); "the agent's click point is layout geometry" is narrower, true, and gateable. ⚠⚠ **I3's REAL SHAPE, NAMED**: the semantic model does not need its own copy of layout's numbers, it needs an assertion that the numbers it PUBLISHES are the ones layout computed for the element the agent NAMED — a per-subsystem contract, not a per-feature checkbox. ⚠ **A FIXTURE THAT REPRODUCES THE RIGHT SYMPTOM FROM THE WRONG STRUCTURE IS THE MOST EXPENSIVE KIND OF GREEN** — the first draft put the caret as a SIBLING of the row instead of its child, so it failed against a correct engine with both coordinates reading `(215, 66)`, the real bug's exact signature from a different cause. It only failed loudly because the fix was already in |

| pattern | where it shows up | status |
| --- | --- | --- |
| **CSS 2.1 §10.3.3 — the over-constrained block equation ignores `margin-left` under `rtl`, so a fixed-width block is flush RIGHT in an RTL page** | Every sidebar, card, fixed-width panel and `width`-without-`margin:auto` wrapper on the Arabic/Hebrew/Persian/Urdu web. Named as residue at t841, where rule L2 fixed how such a block's CONTENT reads while the block itself stayed on the wrong side | ✅ fixed (tick 846) — Chrome-exact on six rows including four controls. ⚠ **Row 2 (`dir=ltr` on the block itself → still 800) is what makes this a CONTAINING-BLOCK rule rather than "RTL elements go right"**: `direction` is inherited, so reading the element's own style agrees with the spec everywhere except the one case that distinguishes the two readings. Row 6 inverts it (an LTR wrapper puts its child back on the left inside an RTL document). ⚠⚠⚠ **THE FIRST DRAFT REGRESSED THE ADVERSARIAL RTL CONTROL AND `delta × n` CAME OUT A WHOLE NUMBER.** `www.ta3lemkonline.com` is BIMODAL — identical coverage (0.982796), element count (457) and reading_order (815) every run, shape on one of two values — and the draft shifted **both modes by the same amount**: `0.601751−0.595186 = 0.006565`, `0.573304−0.566740 = 0.006564`, and **`0.006565 × 457 = 3.0`**. Exactly three elements, deterministically, on a site whose own spread (0.028) is FOUR TIMES the delta. **CONVERT A SHAPE DELTA INTO AN ELEMENT COUNT BEFORE DECIDING IT IS NOISE** — `delta × n` is an integer iff a definite number of elements changed verdict, and **a per-site delta smaller than the site's spread can still be exactly attributable**: the spread bounds what a single READING proves, not what the arithmetic does. ⚠⚠ **A BIMODAL SITE IS A BETTER INSTRUMENT THAN A STABLE ONE** — two modes give the same comparison twice for free, and a change that shifts both by an identical amount cannot be the thing that makes it bimodal. ⚠⚠ **ZERO FIXED / N BROKEN IS A CLASS ERROR, NOT A TUNING ERROR**: `comm` over two `--shape-dump` runs gave 16 newly-missing paths and **not one newly-correct**, every one an `svg` or `svg/path` — §10.3.3 is for a block-level **non-replaced** box, and an `<svg>` is an atomic inline placed by its LINE BOX. Guarded with `!is_replaced_element`, after which the site returns to exactly the old binary's value set. **The `comm` of two shape-dumps should be the first move after any attributed regression.** ⚠ HONEST SCOPE: **zero corpus movement, reason measured** — 3 of 200 sites are RTL and none uses a fixed-width block this rule moves (two are fluid, one bot-walled). Banked because it is the spec, the fixture is exact, and t841 named it as owed |

| pattern | where it shows up | status |
| --- | --- | --- |
| **`delta × n` proves a verdict change and NOT its cause — the integer test is not a control, and 5 of 5 "clean" losses were the SWEEP** | Every per-site sweep comparison. The strictest same-page filter the TSV supports — identical `coverage` **and** identical `shape_n` — still admits a page that served one more ad slot, and admits the sweep's own two-process contention | 📏 t847 — clean `--jobs 2` CrUX sweep, 200/200, then the old-binary control on the rows that filter said were clean. **M1 14.6% → 15.4%** (19 → 20 of 130) while **both conjuncts stayed flat or fell** (`shape≥0.75` 25 → 24, `jarring-clean` 38 → 38): one site crossed and one left the denominator, so the gate moved by COMPOSITION and the common-set band (+1.00 pts over 100 sites) is the real reading. ⚠⚠⚠ **THE METHOD I BANKED ONE TICK AGO IS HALF A METHOD.** t846's row says an integer `delta × n` "settles in one multiplication a question three more sweep runs would have left ambiguous". It settles *that* a definite set of elements flipped — that part stands and does rule out a rounding artefact — and it says **nothing about WHO flipped them**. The 21 rows with identical coverage AND identical `shape_n` gave 10 clean losses; same-hour A/B of both binaries on five of them reproduced **not one**. `gismart.com` read **0.679715, 0.654804 and 0.729537 (twice)** on the same page at byte-identical coverage and byte-identical element count; `developers.google.com`'s two sweep rows are its two binaries' values **with the labels inverted**. **Identical coverage plus identical `shape_n` is not "the same page."** The rule: *the integer test separates a real verdict change from a rounding artefact; only a same-hour run of the OLD BINARY separates the ENGINE from the DAY.* ⚠⚠⚠ **A THREE-POINT LADDER ATTRIBUTES A WINDOW TO A COMMIT, NOT JUST TO "US".** Two engine commits landed in this window; building the intermediate tree put **the whole of every win on t843** — `ubys.bingol.edu.tr` reading_order **19 → 1** (+14 elements), `pivaldi.restoplace.ws` **6 → 4** (+16), `developers.google.com` **6 → 3** (+7), each already fully present in the t843-only binary. Two builds and seven site-runs turned "the window gained a point" into "this commit gained it". ⚠⚠ **A SWEEP ROW IS A LOWER BOUND, NEVER A POINT ESTIMATE.** HEAD re-run alone against its own t847 rows: `gismart` +0.075, `possssno` +0.108, `developers.google` +0.018, `pivaldi` +0.005, and `celeb.gate`/`ubys`/`mobcup` byte-identical — **four up, three equal, ZERO down.** Contention depresses some sites and inflates none, so M1 15.4% is a floor. This also disposed of the window's one apparent RTL regression: `possssno.sbs` read `overlap 19 / reorder 20` in the sweep and **`0.897391 / reorder 1` on both solo runs**. ⚠⚠ **A CORPUS THAT CANNOT EXERCISE A MECHANISM CANNOT PRICE IT — IN EITHER DIRECTION.** t846 moved nothing, and the population says why rather than the fix being wrong: **5 of the 101 scored sites carry any RTL markup at all**. The board's *"a fix must raise in-scope-pass or be reverted"* is aimed at fixes that do not work; t846 is Chrome-measured on six fixture rows with four controls and RED-proven. Say "the corpus cannot price this" out loud instead of quietly counting it either way. ⚠ The t843 row's own 22-site same-hour control predicted **21 of 130**; the full sweep landed **20** — a per-site control predicts a per-site delta, not a corpus crossing count |

| pattern | where it shows up | status |
| --- | --- | --- |
| **An insetless `position:absolute` box landed at the line START instead of its STATIC POSITION** — CSS 2.1 §10.3.7/§10.6.4 put it after everything already on the line, and the code said so and left it unbuilt | Bootstrap's `.sr-only` on every framework page that ships it, plus every badge, caret, chevron and tooltip written as `position:absolute` after inline content. `ubys.bingol.edu.tr` (AdminLTE 2.4.5): `nav > a > span:1 ⇄ span:2`, the `.sr-only` span at `[-1 -1 1x1]` | ✅ fixed (tick 848) — Chrome-exact on the fixture: `.sr-only` after a span **35** (was −1), a plain abspos span **36** (was 0), and after a WRAPPED first span **61** (was 0), with three controls flat. ⚠⚠⚠ **THE CODE NAMED THIS DEFECT IN PROSE AND NOBODY TRIAGED IT** — *"Text preceding it on the line should push the static position along that line; that refinement is not modelled here, and the box lands at the line start instead."* **A build spec whose second half is unbuilt is an untriaged tick with good prose**, and it is invisible to every instrument precisely BECAUSE the code is honest about it — honest text does not fail a gate. `"not modelled here"` is worth grepping for as a defect CLASS. ⚠⚠ **THE MARGIN IS APPLIED AFTER THE STATIC POSITION AND THE TWO MUST NOT BE CONFLATED** — row 1's answer is 35 rather than 36 only because `.sr-only` carries `margin:-1px`; reading 35 as "the static position" would have encoded the margin into the rule. ⚠⚠ **THE SEARCH IS `(line_top, THEN x)`, NOT `max(x)`** — a fragment that wrapped onto a later line is genuinely later even though its right edge is further LEFT, which is the row a max-x search gets wrong and the reason the fixture carries a wrapped case. ⚠⚠ **THE FIXTURE FOUND A SECOND DEFECT IN THE SAME SECTION AND IT WAS DELIBERATELY NOT FIXED IN THE SAME TICK**: `left:200px; top:auto` belongs at **y=294** and lands at **234**, because `position_absolutes` anchors to the static position only when **all four** insets are `auto` while §10.3.7 is written PER AXIS. It is the bigger prize (a full-size box 60px out, against a 1×1 `.sr-only`) and it is one `all_auto` → two per-axis booleans — which is exactly why it gets its own binary and its own attribution instead of being smuggled into this one. ⚠⚠ **THE ONE APPARENT REGRESSION WAS THE SITE, AND t847's LESSON PAID OFF ON THE VERY NEXT TICK**: `payb.jp` fell 0.825662 → 0.733612, a `delta × n` of **exactly 66.0 elements** at identical coverage and identical `shape_n` — and the OLD binary alone, twice more, read **0.677824** and **0.733612**, a spread of 0.148 that brackets both new readings. `gismart.com` 0.729537 and `celeb.gate.cc` 0.783158 byte-identical on both binaries are the clean controls. ⚠ **HONEST SCOPE: zero movement on the ten-site cohort, and the reason is the BOX SIZE.** The fix moves the real page (`ubys` `.sr-only` `[-1 -1]` → `[56 -1]`) but a 1×1 clipped box is below what `shape` registers. Banked on the fixture and the spec, NOT on a corpus delta — and said plainly rather than implying a crossing it did not buy |

| pattern | where it shows up | status |
| --- | --- | --- |
| **The static position of an absolutely positioned box was ALL-OR-NOTHING, and CSS 2.1 writes it PER AXIS** — naming one inset threw the static position away on BOTH axes | Every `position:absolute; right:8px` badge and close button, every `left:0` full-bleed underline, every `top:100%` dropdown — the idiom keeps a static position on the axis it does not name, and every one of them lost it | ✅ fixed (tick 849) — §10.3.7 solves the horizontal equation and §10.6.4 the vertical one *separately*; `position_absolutes` tested all four insets in one boolean. Chrome-measured, 400px `position:relative` wrappers with 60px of spacer above the line, `y` relative to the wrapper top: `left:200px` **[200,+60]** (was [200,0], **60px out**), `top:0` **[36,0]** (was [0,0]), `right:10px` **[309,+60]** (was [309,0]), with all-four-auto and top+left-both-set flat as controls. ⚠⚠ **ROWS 2 AND 3 ARE WHAT MAKE IT *PER AXIS* RATHER THAN "USE THE STATIC POSITION MORE OFTEN"** — row 2 takes `x` from flow and `y` from the containing block, row 3 does exactly the opposite, and **a single boolean however tuned cannot produce both**. A fixture carrying only one direction would have admitted a wrong fix; this is "one rule, N implementations" INVERTED — one implementation where the rule has two independent instances. ⚠ **THE DROP-GUARD HAD TO NARROW IN THE SAME EDIT**: the `continue` that discards a box flow never recorded a cursor for is now conditioned on BOTH axes wanting the static position, because a box with a real inset on one axis is placeable and dropping it would turn a placement bug into a MISSING BOX — strictly worse, and the exact failure that guard was written to prevent. ⚠⚠⚠ **THE ONE CLEAN NEGATIVE WAS THE SITE, AND THE OLD-BINARY CONTROL SETTLED IT FOR THE SECOND TICK RUNNING**: `www.taphouse23.com` fell by a `delta × n` of **exactly −18.00 elements** at identical coverage and identical `shape_n` — the strongest form of the integer signal — and the OLD binary alone read `0.408292 / 0.407590 / 0.395643` with `overlap` wandering 10–13, a range that contains every new reading, the lowest value being produced by BOTH binaries. **t847's rule has now saved two consecutive correct fixes from being reverted on their own arithmetic.** ⚠ HONEST SCOPE: **+2 attributable elements across 28 sites** (12-site M1 cohort byte-identical including the adversarial RTL control; 10 of the 16 largest scored sites byte-identical). A scored divergence needs the abspos box to be both mis-placed AND large enough to fail the tolerance, so the spec win is real and the corpus win is small — said plainly |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A `<button>` centres its content VERTICALLY, and no stylesheet can say so** — the UA sheet's `text-align:center` covers the horizontal half and the vertical half is not expressible in CSS at all | Every design-system button, because every design system fixes a button height: the label sat 5-20px too high on essentially every button on the web. Reached from `littlecaesarsbcs.libellum.com.mx`, whose single M1-blocking `overlap` is between two `<span>` children of one `<button>` | ✅ fixed (tick 850) — Blink lays a button's children out in an anonymous flex-like box with `align-items:center`; the HTML rendering spec says the same. Chrome-measured, label y relative to the border box: `height:50px` one line **16** (was 0), `height:80px` two block spans **22** (was 0), `height:20px` nearly-full **1** (was 0), `display:inline-block` **16** (was 0), with auto-height and a plain `<div>` flat as controls. ⚠⚠ **THE CONTENT MOVES, NOT THE BOX, AND AS ONE GROUP** — row 2's two block children keep their own 18px separation and travel 22 together, which is what makes this CENTRING rather than per-line alignment; **a fixture with a single child cannot tell the two apart**. Expectations are derived from the AUTO-height button's own height rather than from `18`, so the UA font's metrics cannot make the gate lie. ⚠⚠ **THE REDUCTION'S CONTROLS ARE WHAT FOUND IT**: the same block-span + inline-span shapes inside a plain `<div>` came out Chrome-exact, so the anonymous-block machinery for mixed inline+block children was already right and the 7px was the BUTTON, not the mixing. ⚠⚠ **MEASURED RESIDUE — `box-sizing` on form controls**: Chrome computes `border-box` for `button`/`input[type=submit|reset|button]`/`select` and `content-box` for `input[type=text]`/`textarea`/everything else; at `height:50px;padding-top:20px` Chrome reads **50/50/70/50/70/70** and we read **70** for all six. Three controls 20px too tall whenever they carry padding AND a height — one UA-sheet rule, and a padded button's centring cannot be right until its content box is. ⚠⚠⚠ **THE ONE CLEAN NEGATIVE WAS THE BATCH, NOT EVEN THE SITE — A THIRD FORM OF THE TRAP.** `www.kicktipp.com` read a `delta × n` of **exactly −8.00 elements** at identical coverage and identical `shape_n`; alone it reads 0.852632 on BOTH binaries twice each, and in a paired 4-site batch it reads 0.852632 on both — six agreeing readings against one dissenting 14-site run. t847 said a SWEEP row is a lower bound; this is the same effect inside a **14-site batch in one process**. **THE BATCH SIZE IS PART OF THE MEASUREMENT.** ⚠⚠⚠ **AND THE SITE THAT MOTIVATED THE SEARCH DID NOT MOVE** — littlecaesars' two spans are byte-identical on both binaries, so the centring never fired on that button and why its content box has no slack is unestablished. **Three consecutive ticks (t848/t849/t850) landed spec-correct, Chrome-exact, RED-proven primitives and moved the M1 cohort by nothing**: the next render tick should diagnose ONE cohort site end to end with `--why` until its specific failing pair is understood, instead of reducing to a mechanism FAMILY and hoping the family is the cause |

| pattern | where it shows up | status |
| --- | --- | --- |
| **Buttons and `<select>` are `border-box`; text fields and `<textarea>` are not — and BOTH our UA sheets were wrong, in OPPOSITE directions** | Every page that sets a height AND padding on a button, which is what every design system does. A button, a submit input and a select were too tall by exactly their vertical padding plus borders | ✅ fixed (tick 851) — Chrome-measured at `height:50px;padding-top:20px`: **50 / 50 / 70 / 50 / 70 / 70** for button / submit / text / select / textarea / div, against our **70** for all six; now all six exact, with an author's own `box-sizing:content-box` still winning at 71 as the UA-origin control. ⚠⚠⚠ **THE TWO HAND-MAINTAINED UA SHEETS WERE WRONG IN OPPOSITE DIRECTIONS**: `MinimalCascade`'s `apply_ua_defaults` set `border-box` for ALL FOUR form tags (too many) and `stylo_engine.rs` — **the SHIPPING sheet** — had **no rule at all** (too few). The known hazard predicts one sheet going STALE; what actually happened is worse, because **each sheet's error concealed the other's from whichever test you happened to write**. ⚠⚠⚠ **AND THAT IS THE TRAP I NEARLY WALKED INTO ONE TICK AFTER WRITING THE WARNING DOWN**: `manuk-layout`'s `layout_html` runs `MinimalCascade`, so a layout-crate unit test would have gone GREEN on the half the browser does not use. The gate lives in `engine/page/tests/g_form_control_metrics.rs` (`Page::load`, the real pipeline) and is RED-proven by commenting the rule out of `stylo_engine.rs`. ⚠ Running that gate without `--features stylo,spidermonkey` fails its two PRE-EXISTING assertions with completely different numbers (`#a1` width 194 vs Chrome's 53) — the same fact from the other side, and worth recognising before mistaking it for a regression. ⚠⚠⚠ **A CONTROL EARNS ITS STATUS RUN BY RUN, NOT ONCE.** `celeb.gate.cc` had been byte-identical (0.783158) in four A/Bs this session — the most stable control I had — which is exactly why its `delta × n = −7.00` looked real. Re-running the **OLD** binary alone produced the NEW value (0.768421) twice: the SITE moved. `www.library.chiyoda.tokyo.jp`'s `−1.00` was likewise the page loading further (n 342→356, coverage 0.886→0.922, and the score went UP). 9 of 12 cohort sites byte-identical, zero attributable regressions |

| pattern | where it shows up | status |
| --- | --- | --- |
| **An icon-wrapping inline (`<span class="icon"><i></i></span>`) reported its ICON's box, not its own line box — and that box is the AGENT'S CLICK POINT** | Every icon button, sprite, chevron, avatar and badge written as an inline wrapping an atomic child, plus every `<a><em>x</em></a>` — an inline whose content belongs entirely to a descendant. Named as t851's residue and ranked by CONSTITUTION-CHECK #72 as an I3 (actuation) defect rather than a shape term | ✅ fixed (tick 853) — a non-replaced inline's border box is its **own content area** (font ascent+descent, on the line's baseline), resolved **PER AXIS**: height from its own metrics, width from its contents' advance. Chrome-measured, `16px/1.2 sans-serif`: `<span><i 8x4></i></span>` **[11,1,8,17]** (was [11,11,8,4]), `<span><i 8x40></i></span>` **[11,93,8,17]** (was [11,70,8,40]), `<span 10px><b 40px>x</b></span>` **[11,48,22,11]** (was [11,21,22,44]), with a plain text span flat at 17 as the calibration control. ⚠⚠⚠ **THE TALL-DESCENDANT ROWS ARE WHAT MAKE IT A RULE RATHER THAN A COINCIDENCE** — the common icon is *smaller* than its line, so a both-axes union of the child is correct on row 1 and wrong on rows 2 and 3, where the child **overflows** its parent inline box and Chrome reports the parent unmoved. A fixture carrying only the icon case would have admitted a wrong fix; this is the t849 per-axis shape again, one level down. ⚠⚠⚠ **THE ELEMENT HAD NO FRAGMENT OF ITS OWN, WHICH IS WHY THREE EXISTING REPORTERS ALL DECLINED TO FIRE**: no `Word` (the text is the descendant's), no edge spacer (no padding), and not empty (so the empty-inline reporter's `out.len()==mark` test is false). **Three correct guards composing into a gap is not something any one of them is wrong about** — the missing case only appears when you ask *which items claim MY node?*, which is now an `InlineItem::owner()` accessor so a new variant must answer it. ⚠⚠ **TWO reporters, head and tail**, because a boxless inline that wraps spans several lines and Chrome's rect runs from the first line's content top to the last line's bottom; both `holds_line:false` with `report_ascent:Some(..)` so the line boxes come out byte-identical (118/118 layout tests unmoved). ⚠⚠⚠ **THE GATE ASSERTS THE CLICK POINT, NOT JUST THE BOX** — `node_rects → build_tree_with_rects → A11yNode.bbox` is the agent's target, so the icon button's click point was the centre of a 4px-tall box, **3.5px low**. Five consecutive geometry ticks passed I3 *because the producer is shared*, and a fix to the producer itself is where that accident stops protecting us. ⚠ HONEST SCOPE, per check #72 steer 2: **the instrument cannot price this** — the burndown ranks by `in-scope sites × dy` and nothing in it computes an actuation cost. That is not the same claim as "it bought nothing"; VI.3 ranks by usage weight, and this is a universal idiom. ⚠ Residue named: with three or more lines the reporters bound the first and last, so a middle line's horizontal extent is unrepresented — strictly better than lifting the child's box, and said out loud |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A `<form>` was a BOXLESS INLINE under `MinimalCascade`, so once inlines reported their own box the form became SMALLER than the button inside it and STOLE THE AGENT'S CLICK** | Every form, fieldset, `<dl>`, `<menu>` and `<center>` on the web, in every build that does not enable the `stylo` feature — which is `manuk-agent`, the crate whose entire job is clicking things | ✅ fixed (tick 853) — `stylo_engine.rs`'s UA sheet says `form, fieldset, table, caption, center, menu, dl { display: block }` and `summary { display: block }`; `apply_ua_defaults` carried the **table family and none of the rest**. Under `MinimalCascade` the form had no box, so `node_rects` lifted the button's: `[8,8,43.34,25]` for both, a tie that `hit_test` (smallest-area-wins, deepest breaks the tie) resolved to the button only because pre-order sees it later. ⚠⚠⚠ **THE t853 INLINE FIX DID NOT CAUSE THIS — IT MADE IT VISIBLE**, which is the useful shape: giving the inline its own 17px content area made the form's box `43.34×17 = 737` against the button's `43.34×25 = 1084`, so the *wrapper* became the smallest box containing the point and swallowed the coordinate click on the control. A latent tie became a wrong answer. ⚠⚠⚠ **THIRD SIGHTING IN THREE TICKS OF THE HAND-MAINTAINED TWIN SHEETS** (t851 had them wrong in *opposite* directions on `box-sizing`; here the minimal one is simply short), so it is fixed as a **gate rather than a comment**: `both_ua_sheets_agree_on_which_elements_are_block` reads the shipping sheet's own `display:block` selector list out of `UA_CSS` and asserts `apply_ua_defaults` agrees tag by tag. RED-proven by deleting `form` — `["form -> Inline"]`. ⚠⚠ **THE GATE ASSERTS ITS OWN PARSE** (`tags.len() > 20 && contains form && div`): a lockstep gate that silently reads an empty selector list passes forever, which is the vacuous-gate shape `falsify.sh` exists to catch. ⚠ **A COMMENT CANNOT GO RED** — the note above `apply_ua_defaults` has read *"keep in lockstep with the UA sheet in stylo_engine.rs"* the entire time the two were drifting |

| pattern | where it shows up | status |
| --- | --- | --- |
| **`hit_test` resolved an ANCESTOR/DESCENDANT pair by AREA, so a wrapper a third of a pixel smaller stole the click on the link inside it** | Every nested clickable on the web — `<li><a>`, `<span><a>`, `<div><button>` — wherever the wrapper's box is not strictly larger. Wikipedia's `.hlist li { display: inline }` nav rows are the measured instance | ✅ fixed (tick 853) — the rule was *"highest `z`, then smallest box, deeper node breaks an EXACT tie (pre-order sees it later)"*. It only ever gave the right ancestor/descendant answer while an ancestor's rect could not be smaller than its child's — which held **by accident**, because a boxless inline's rect was lifted verbatim from its children and so was byte-identical to them. Give each inline its own content area (same tick) and `<li>` `[740.33,3193.75,34.43×16.00]` vs `<a>` `[740.00,3193.75,34.50×16.25]` inverts it: the shell walks **up** from the hit node looking for an `<a href>`, and above an `<li>` there is none. ⚠⚠⚠ **THE GEOMETRY WAS RIGHT AND THE TIE-BREAK WAS WRONG** — a rule written to order *unrelated overlapping* boxes was being asked a *containment* question. Chrome's `elementFromPoint` has no such ambiguity: topmost, then deepest, full stop. `hit_test` is now a recursion that resolves the relationship **structurally** (a subtree reports its own best; a hitting node loses to any hitting descendant on the same layer), with area comparing only *sibling subtrees* — the one place it was ever the right question. `z` still wins first, so occlusion and `pointer-events:none` are unmoved. ⚠⚠⚠ **THE THREE-POINT LADDER, ON ONE PINNED FILE IN ONE HOUR** (`/tmp/manuk-g6.html`, 476 links — the G6 page is fetched live, so a re-`curl` would have made the readings incomparable): HEAD **99.7% / 1 missed**, geometry-only **95.6% / 16**, geometry+`hit_test` **99.7% / 1** — the *same* residual miss and the *same* 365 found. **Exactly restored, not improved**, and said that way rather than banking the 4.1 points the middle row makes available. ⚠⚠ **A CAPABILITY FIX CAN INVERT A LATENT TIE SOMEWHERE ELSE ENTIRELY** — nothing about `node_rects` is wrong here, and no amount of testing the inline rule would have found it. The wall's own G6 did, which is what a gate on a *real page* buys that a fixture cannot |

| pattern | where it shows up | status |
| --- | --- | --- |
| **BAR 0 — an unclamped `colspan` is an infinite loop with a number in it, and it survived because A HANG IS NOT A RED** | Every `<table>` on the web that carries a junk or hostile span attribute — and every fuzzer, CMS export and hand-written legacy table that produces one. It is also the *adversarial-input* axis STATUS.md lists as one the oracle has never observed | ✅ fixed (tick 854) — `colspan`/`rowspan` are HTML **clamped unsigned longs**; `LayoutBox::cell_span` was a bare `parse::<usize>()`, so `<td colspan="2147483648">` parsed cleanly on a 64-bit target and the table builder was asked for **two billion columns**. Bisected by scaling the attribute: 2 / 10 / 50 / 200 / **1000** all load in ~250ms, `2147483648` never returns. Chrome-measured bounds: `colspan` → **1000**, `rowspan` → **65534** (a *different* bound, so one shared constant is wrong for one of them — the t851 two-intercepts shape). ⚠⚠⚠ **ONE RULE, TWO IMPLEMENTATIONS, AND ONLY ONE HAD IT**: `reflect_js.rs` implements `clamped unsigned long` correctly and its own comment says *"a colspan of a billion is 1000, not the default"* — so `td.colSpan` answered 1000 while the layout that builds the table read 2,147,483,648. The IDL was right, the geometry hung, and nothing compared them. ⚠⚠⚠ **IT SURVIVED ITS WHOLE EXISTENCE BECAUSE A HANG IS NOT AN ASSERTION.** `g_reflect_numeric` has carried this exact value since it was written and **did not fail — it spun**: `user 2m57s` of a 3m00s cap on a four-element fixture, which reads as *a slow gate*. The wall runs **19 of 104** gates, so nothing else was looking; it surfaced only when t853 ran the whole suite for an unrelated sweep. After the fix that gate runs in **0.40s**. ⚠⚠⚠ **AND IT BECAME A DEFECT RATHER THAN "MY REGRESSION" ONLY BECAUSE OF THE OLD-BINARY CONTROL** — a stashed tree, rebuilt and run in the same hour, hung identically (3m00.2s / `user 2m56.7s`). The control that saved four correct fixes from being reverted this window also stopped a real bug from being written off. ⚠⚠ **SO THE GATE FOR A HANG MUST NOT HANG**: `G_SPAN_CLAMP` runs the load on its own thread behind a 20s `recv_timeout`, RED-proven to produce `FAILED … finished in 20.00s` *with a message* rather than silence. **A gate whose failure mode is silence is not a gate** — it recreates the exact condition that hid the bug. ⚠ Widths are asserted against a **control cell in the same document** (our collapsed-border cell is 24px to Chrome's 23), so the gate asks *"did the span apply and get bounded?"* rather than freezing a 1px residual. ⚠ **RESIDUAL, named**: HTML integer parsing stops at the first non-digit, so Chrome reads `colspan="3px"` as 3 and we read 1 — a wrong answer, not a hang, and a different rule |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A five-clause `is_empty()` outlived the decision that made ONE clause false, so it failed PERMANENTLY and said nothing true about the other four** | Every ESM page: the static-import scanner decides which modules get pre-fetched, and `ModuleLink` is synchronous, so a specifier the scan misses is a module the page cannot import | ✅ fixed (tick 855) — `static_import_scanner_finds_specifiers_and_skips_the_rest` asserted in ONE `is_empty()` that five different things contribute no specifier (a `from` in a line comment, in a block comment, inside a string literal, a dynamic `import(...)`, and `import.meta`). t624 changed the fourth **on purpose**: a *literal* `import("m")` specifier is collected now, because `module_dynamic_import_hook` resolves from `MODULE_GRAPH_SOURCES` — the same pre-fetched map the static graph seeds (`js/src/dom_bindings.rs:12374`) — so a specifier missing from it is one `import()` cannot satisfy. ⚠⚠ **THE COMMENT WAS TREATED AS A HYPOTHESIS AND CHECKED AT THE CONSUMER**, per the standing rule that a claim about another subsystem is a hypothesis even when we wrote it: the code is right and the test was stale. Measured before touching anything, the scanner returns exactly `["./dynamic.js"]` — **every other clause was working perfectly the whole time, invisibly, behind a red.** ⚠⚠⚠ **A LUMPED ASSERTION DOES NOT FAIL LOUDLY WHEN ONE CLAUSE IS SUPERSEDED — IT FAILS PERMANENTLY, AND A PERMANENT RED IS READ AS BACKGROUND NOISE.** Worse than no test: it trains every reader to expect this suite red, which is how the *next* real regression gets waved through. Same shape as a gate that cannot go red, inverted — a gate that can only *be* red. ⚠⚠ **THE RULE: when you supersede a decision, GREP FOR WHAT ASSERTED IT.** t624 documented its reasoning beside the code it changed — the discipline as written — and it was not enough, because the contradiction lived three thousand lines away in a test. ⚠ Five rules are now five assertions, each RED-proven separately (dropping the `import(` marker gives `left: [] right: ["./dynamic.js"]`; disabling line-comment skipping gives `got ["./comment.js","./dynamic.js"]` on a **different** one), so neither mutation can hide inside the other. ⚠ It survived because the wall runs **19 of 104** gates and `manuk-page --lib` is not among them |

| pattern | where it shows up | status |
| --- | --- | --- |
| **The fidelity oracle renders ONE CURL'd FILE from `file://`, so every RELATIVE bundle 404s — and 10 of the 12 `shell-only` sites the board ranks as "do not render at all" render FINE** | Every client-rendered site in the corpus: Angular (`src="main-5UYZQ2ZL.js"`), Next.js (`src="/esaj/_next/…"`), and anything whose bundle is referenced relatively — which is nearly all of them | 📏 measured (tick 856), fix ranked and NOT taken. Cohort-wide: raw tags in the curl'd body vs Chrome-on-that-file (the oracle's own path) vs Chrome LIVE — `house.udn.com` **5 / 5 / 949**, `allticketscol.com` **34 / 36 / 1115**, `pt88.app` **39 / 40 / 435**, `esaj.tjsp.jus.br` **30 / 30 / 300**. All twelve answer **HTTP 200**, so the code's own instruction (*"check whether the snapshot fetch was bot-walled before treating the row as evidence"*) is satisfied and the rows still are not evidence. ⚠⚠⚠ **THE MECHANISM IS NOT THE ORIGIN**: from `file:///tmp/x.html` a relative bundle resolves to `file:///tmp/main-…js` and a root-relative one to `file:///esaj/…`; both 404. **Absolute URLs still load** — `vk.com` goes `0 → 215` on exactly that — which is why the shortfall varies per site and is the tell separating this from a blanket scheme restriction. ⚠⚠⚠ **TWO COMMENTS IN ONE FILE ASSERTED CONTRADICTORY CAUSES AND THE TRUE CAUSE WAS A THIRD THING NEITHER TESTED**: `unscoreable_reason` claimed the `file://` null origin, while `ShellOnly`'s own docs 1,300 lines above record t674 **refuting that claim** by serving the identical document over `http://127.0.0.1`. t674's experiment was sound and its conclusion over-broad — the same single file over localhost 404s `/esaj/_next/…` just as hard, so it could not distinguish *"the origin blocks the fetch"* from *"the files are not there."* **A refuted cause left standing in a second comment is a wrong answer with a citation.** ⚠⚠ **I WENT LOOKING FOR THE ROW THAT REFUTES ME AND IT IS THE MOST INFORMATIVE ONE**: `forums.moneysavingexpert.com` is INVERTED (curl 2583 · on-file 2345 · live 48) — the oracle had a full document and still recorded `shell-only-9`, so its gap is **probe-side**, and headless Chrome is walled on the live URL while curl is not. **The two channels disagree in both directions; neither is authoritative alone.** ⚠ **UNITS**: raw/on-file/live are TAG counts, `probed` is a box-bearing path-keyed ELEMENT count — not the same quantity, which is why only the ten rows where on-file is 5–60 tags against 239–1115 live are conclusive (no element count exceeds the tags it is drawn from). ⚠ **ONE ROW IS HONEST** (`awlyaa.education.dz`, 9 tags in every channel), which is what stops this being *"the reason is always wrong"*. ⚠ **THE FIX AND ITS TRADE, STATED NOT TAKEN**: a `<base href="ORIGINAL_URL">` on the saved document makes ten sites visible, but the oracle's render then depends on live subresources — *"three repeats are three renders of the same bytes"* (asserted by a determinism test) weakens to *whatever the CDN served this minute*. **That changes what the certificate MEANS**, so it is the contract-owner's call. ⚠ **CONSEQUENCE: do not spend throw-killer ticks on this cohort** — our own row reads `coverage 1.000000` against a ONE-ELEMENT reference. The honest ceiling is smaller than 29, and the residue that is ours is `thin-overlap`/`tree-divergence` |

| pattern | where it shows up | status |
| --- | --- | --- |
| **I criticised a comment for being "a wrong answer with a citation" and shipped one in the same tick — the oracle ALREADY inserts `<base href>`, and the real mechanism is `document.URL`** | Every `shell-only` row, and more generally every site whose boot code branches on `location.hostname` / `document.URL` / `location.protocol` — which the oracle answers `file:///tmp/manuk-shape-….html` | ✅ corrected (tick 858). t856 claimed *"the oracle renders from `file://`, so every relative bundle resolves to `file:///tmp/…` and 404s"*. **`chrome::capture_seen_all_paths` — the very function producing the `probed` count the reason is computed from — injects `<base href="{url}">` at `chrome.rs:520`.** Re-measured with the tag inserted exactly as the oracle inserts it: `allticketscol.com` **38 tags · sheets 10 · font Lato** — relative stylesheets resolve and load, which alone falsifies the stated mechanism. t856 had measured a plain `file://` copy WITHOUT the base tag, i.e. a different pipeline from the one it was explaining. ⚠⚠⚠ **RIGHT ANSWER, WRONG REASON IS THE FAILURE MODE THAT SURVIVES LONGEST, BECAUSE THE CONCLUSION KEEPS CHECKING OUT** — and it is the third cause proposed for this one reason string (null origin → refuted t674; relative-404 → refuted t858). ⚠⚠⚠ **THE DEMONSTRATED MECHANISM IS `document.URL`**: `house.udn.com`'s whole document is a five-tag stub guarded by `if (document.URL.indexOf("house.udn.com") != -1) location.href = "/house/index"`, which is **-1** for the oracle, so the redirect never fires. `<base href>` cannot fix it — it changes URL *resolution*, not `document.URL`. ⚠⚠ **THE CONCLUSION SURVIVES AND IS BETTER SUPPORTED**: even with the base tag the oracle builds 38 tags for allticketscol against **1115** live and 8 for house.udn.com against **949**, so *do not spend throw-killer ticks on this cohort* still holds. ⚠⚠ **A NUMBER WITHDRAWN RATHER THAN CAVEATED**: a mid-tick count of "36 of 101 scored sites carry a relative stylesheet href" was nearly published as *"36% are compared against an unstyled Chrome"* — with a base tag present that measures href SHAPE, not CSS delivery, and the settling probe returned no reporter on all 20 sample sites (a probe failure, not a result). Published instead: `trivago.be` has five `<link rel=stylesheet>` and the oracle loads **zero** — one site, named, not a rate. ⚠ **THE RULE: REPLICATE THE INSTRUMENT, NOT YOUR MODEL OF IT** — reconstruct the pipeline from the function that produces the number, never from the comment that describes it |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A float or abspos box BEFORE the first in-flow block** (`<div class=illu style="float:right"><img></div>` then the prose — the pull-quote / article-figure / sidebar-thumbnail idiom) | every content page that floats an image beside its text, every WordPress `alignright` figure, every card with a floated icon — and it composes with tick 151, so it silently un-did §8.3.1 on exactly the pages t151 was built for | ✅ (tick 859) — all four §8.3.1 search helpers `return`ed on `is_float \|\| is_out_of_flow_positioned`, commented *"conservative"*. **There is no conservative direction**: declining a collapse leaves the child's margin INSIDE the parent — a visible band of parent background above the first paragraph. CSS 2.1 §8.3.1 collapses with the first **in-flow** child, so an out-of-flow box is SKIPPED (`continue`, not `return`). Chrome-measured (`p{margin:15px 0}`, parent y / first `<p>` y): float-first `15`/`15`, abspos-first `68`/`68`, **text**-first `159`/`192` (text does separate). `layout_children`'s placement loop already had this right (`first_block` clears only on a block-level child) — **the hoist computation and the placement disagreed with each other for 700 ticks and nothing compared them.** Measured same-hour vs the t858 binary: `kicktipp.com` reading-order **1 → 0**, shape 85.3 → 87.4; eight control sites (possssno ×3 interleaved pairs, marktplaats, ubys, wikipedia, HN, a11yproject, blog.rust-lang, martinfowler) byte-identical on shape and all four jarring terms. Gated by `an_out_of_flow_first_child_does_not_cancel_the_parent_child_margin_collapse` (float + abspos + the text-first guard) and `a_trailing_float_does_not_cancel_the_bottom_margin_collapse`; RED-proven by restoring the `return 0.0` arm. layout 118→120, HANG/CRASH 0 |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A `<link rel=stylesheet>` the origin 404s** (a dead link in the author's own HTML — `cuneocronaca.it` ships two, `m.youm7.com` one, `nortenoticia` a wrong-version cdnjs tailwind) | far more common than it sounds: a CMS theme upgrade leaves stale `<link>`s behind, and a pinned CDN version 404s the day it is yanked. Chrome renders these pages every day without complaint | ✅ (tick 860) — `subresource_text` reads `status >= 400` and **discards the status**, so a sheet the origin does not have hit the same `None` arm as one that died on the wire, and that arm books both into `failed_css` — the number that decides whether a site can be measured at all. Three in-scope sites were tagged `css-starved` and counted against us by a reason string that said *"cut by our own load deadline, NOT refused by the origin"*; `curl` says **404 on 3 of 3**. A 404 is the same answer for Chrome, so both engines render the same page and it is perfectly scorable. Exemption is **404/410 ONLY** (a 403 is often a bot-wall answering US differently; a 5xx may have served Chrome fine) and needed a second half — `absent_css`, because a 404 never enters `external_css`, so the re-fetch filter re-requested it every re-entry until the load deadline cut it and the *deadline* re-blamed it. **An exemption keyed on a fetch's OUTCOME is undone by anything that stops the fetch from having one.** All 3 now score; `m.youm7.com` is `cov 1.000 · shape 0.870 · jarring 0/0/0/0` — **an outright M1 PASS**. Gated by `a_stylesheet_the_origin_does_not_have_is_not_counted_as_unstyled` (404 must not count, 503 must) — RED-proven in both directions; controls byte-identical |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A page the REFERENCE browser hangs on** (`bbs.ruliweb.com`, `www.friulioggi.it` — both heavy ad/social-embed news-and-forum pages: 4780 tags / 987 `<li>` / 1199 `<a>`, Facebook-plugin iframes) | any differential instrument, on any corpus — the oracle is a subprocess and subprocesses fail on their own account. These two carried a bare `timeout-150s` for NINE consecutive sweeps (t800→t857) and were counted as engine backlog the whole time | ✅ (tick 861) — measured with a control, same snapshot, same flags, **one binary apart**: `chromium` 1.0s, `google-chrome-stable` **>120s killed**, our engine 27.5s/34.0s, scoring control 2.26s. `chrome_bin()` prefers `google-chrome-stable`, so the oracle runs the half that hangs. Four hypotheses were refuted first (slow origin — curl 1.3s; our net stack — refuted by a tight interleave once the site proved *intermittent*; the O(n²) `pathOf` probe — 1.04s; the screenshot — 1.05s, and the run's own **75.0% visual score** on an UNMEASURABLE row proves it succeeded) and **all four were measured against `chromium`, the binary the instrument never invokes**. New `Unmeasurable::OracleTimeout`; the bare `Timeout` now says it bounds BOTH engines. ⚠ **the denominator is deliberately UNMOVED** — counted and unscored, asserted against the EXCLUDED partition, because "the reference failed" is the most tempting licence to launder hard sites out and raise the headline for free. Third consecutive named-as-ours cohort proven not-ours (shell-only t856, css-starved t860). Gated by `an_unscored_site_must_name_its_cause`, RED-proven both directions |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A library BRAND-CHECKS a DOM node** — `Object.prototype.toString.call(el).indexOf('Element]') > -1`, or `el.constructor.name` | the oldest duck-typing idiom on the web, and it is everywhere: tippy.js's `isElement` (which decides whether `tippy()` returns an instance or an ARRAY), lodash `isElement`/`isPlainObject`, jQuery, every structured serializer. Nothing feature-detects it — a caller simply believes the answer | ✅ (tick 862) — ours answered `[object Object]` and `"Object"` for **every node on every page**, and it cost `www.otomoto.pl` its whole server-rendered DOM: false brand → tippy returns an array → `r.props is undefined` + `r.destroy is not a function` → React error boundary → Next.js renders `/_error` → the ~1,300-tag document is torn down. `render-failed` · coverage **0.004** · blank screenshot, in NINE consecutive sweeps; after the fix coverage **0.968** · shape 0.762 over 1,047 elements · scored. Invisible to every existing instrument because `typeof`, `nodeType` and **`instanceof`** were all already right — a wrong answer of the RIGHT type (t733-736). Fixed as ONE accessor at the root of the DOM prototype chain (WebIDL's per-interface-prototype form is unavailable: this engine has five DOM prototypes, so a data property on `HTMLElement.prototype` would brand `<div>` and `<a>` identically); the tag→interface table is taught by the existing `iface(name, tagIs(TAG))` calls rather than written a second time. Gated by `G_BRAND` with Chrome's own answers transcribed from `chromium --dump-dom` — RED-proven (every row read `[object Object]`, `tippy-isElement` read `false`) |

| pattern | where it shows up | status |
| --- | --- | --- |
| **Differential loading (`type="module"` + `nomodule`)** — the same application shipped twice, the browser picking exactly one half | the DEFAULT `ng build` output for years (`runtime`/`polyfills`/`main` × es2015+es5), Vite's legacy plugin, the webpack module/nomodule recipe; also the inline "please upgrade your browser" banner | ✅ (tick 864) — `nomodule` appeared **nowhere** in the engine, so we ran BOTH halves: two framework runtimes bootstrapping over one root element. `pogoda.by` went `render-failed` (cov **0.009**, blank screenshot) → **SCORED cov 0.634 · shape 0.761**; `www.otomoto.pl`, which ships its polyfills `nomodule defer`, moved shape 0.762 → 0.797 on the same change. Honouring `type="module"` WITHOUT honouring `nomodule` is not a partial implementation — the two are a matched pair of mutually exclusive rules and one alone inverts the outcome. Checked before the FETCH as well as before execution, so the legacy bundle no longer spends the load budget. ⚠ `nomodule` is CLASSIC-only: `<script type="module" nomodule>` is inert per spec and must still run — a fix that skipped both halves blanks the page just as thoroughly. Gated by `G_NOMODULE` (membership both directions + execution order, the order transcribed from `chromium --dump-dom`) — RED-proven |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A `type="module"` SPA measured from a COPY of its document** | every Vite / Angular 16+ / esbuild / modern-Rollup app — i.e. the whole current SPA ecosystem — whenever a differential harness renders a fetched snapshot instead of the live page | ⚠ INSTRUMENT LIMIT, named not fixed (tick 865). A module script is ALWAYS CORS-fetched, and a site does not send `Access-Control-Allow-Origin` for its own bundle (`allticketscol.com/main-*.js`: 200, no ACAO), so from any foreign origin the entry bundle never loads and the app never boots. Chrome renders the same pages perfectly from their LIVE url (allticketscol 0 divs snapshot vs **312** live; comix.to 1 vs **1258**; pt88.app 2 vs **147**). **8 of the 13 in-scope `shell-only` sites** — the largest unscored cohort — are this. ⚠ t674 recorded this cause as MEASURED FALSE on a control that could not test it (`http://127.0.0.1` is just as cross-origin as `file://`); the control that varies the mechanism (`--disable-web-security`) moves pt88.app 2 → 98 divs. ⚠⚠ INLINING the bundles half-boots the apps (pt88 2→71 of 147) and was REFUSED: a half-built reference clears the shell floor, so the instrument would start charging Chrome's missing half to us. Landed as the LABEL `oracle-module-shell-N` (counted, unscored, denominator unmoved); the real fix is a loopback reverse proxy giving document+bundle+XHR ONE origin |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A page reads the DEPRECATED half of an API whose modern replacement exists** — `performance.timing.navigationStart`, `performance.navigation.type` | every RUM/analytics/bootstrap path written before the Navigation Timing L2 entry landed, which is most of them: the old interface is still in every shipping browser, so nobody rewrote it | ✅ (tick 866) — `performance.timing` was **undefined** while its modern replacement was fully built, and the comment above that replacement said so approvingly. `dashboard.twitch.tv` died on `TypeError: can't access property "navigationStart", performance.timing is undefined` at module scope and rendered **2** elements; after the fix, **59**. The half-installed-API wall with the two halves being two GENERATIONS of one API — **shipping the successor is not shipping the feature**. Implemented as ACCESSORS over the same `__navTiming` instants the navigation entry reports (relative doubles → absolute epoch ms), never a second copy, and the gate asserts the two views agree. ⚠ `redirect*`/`unload*`/`secureConnectionStart` are **0** (the spec's "did not occur", and Chrome's own answer); the unobserved network phases stay **ABSENT** rather than 0, because a 0 there is indistinguishable from a real 0ms and yields a confident wrong TTFB. Gated by `G_PERF_TIMING`, expectations transcribed from `chromium --dump-dom` — RED-proven |

| pattern | where it shows up | status |
| --- | --- | --- |
| **An icon-and-label link — `<a><i class="icon"></i><span>Label</span></a>`** | every navigation bar on the web; the `<i>` is an EMPTY inline carrying an icon-font glyph or a background sprite | ✅ (tick 868) — an empty inline SHARING a line reported the **line box** (height 24, anchored to the line top) instead of its own **content area** (17, on the baseline), and the error GROWS with `line-height` (48 vs 17 at `line-height:3`). Six of six cases now byte-identical to `chromium --dump-dom`, including the two where Chrome reports `0x0` because an empty inline **alone** brings no line box into existence (CSS2 §9.4.2) — that half is what a careless fix regresses into a phantom 17px box. The code's own comment claimed the line-height rect WAS Chrome's measured behaviour; it was measured only in the alone-case, where the field is never consulted. ⚠ **It did NOT clear the three `reading-order 1` sites it was reduced from** — Chrome-exact, RED-proven, high-usage, and zero corpus movement: *"the instrument cannot price this"* ≠ *"this bought nothing"* (VI.3). Gated by `G_EMPTY_INLINE_RECT` (rects AND the unchanged block heights) |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A page's accessible NAME and computed ROLE** — what a screen reader announces, and what an agent identifies an element BY | every assistive technology, and every agent that grounds "click the Save button" in something other than pixel coordinates; `I3` calls this tree *"the single most durable moat"* | ⚠ **MEASURED AT LAST — 797/1250 = 63.8%** (tick 870): `accname` 306/481 · `wai-aria` 238/434 · `html-aam` 253/335, zero crashes, zero hangs. Unmeasured since tick **618** for one reason: the three WPT suites were never in our sparse-checkout, and the runner then skipped them anyway on a rule reading *"needs testdriver (synthetic input)"* — true of the FILE, false of the TEST, since they import `testdriver.js` for exactly two READ-ONLY accessors. Those two exist because every other engine reaches its a11y tree only through a **WebDriver round-trip**; ours answers them synchronously in-process (`__axRoleName` → `manuk_a11y::role_of`/`::accessible_name`), which is I3 being cashed rather than asserted. ⚠⚠ The first THREE readings were **0/1250 and entirely the harness** — see the wiki: read a failure MESSAGE before believing a score. RESIDUE for the burndown: `accname/name/shadowdom` **0/6** (name computation does not cross a shadow boundary), the whole `aria-owns` family (no content relocation), and `wai-aria/role` as a straight role-mapping burndown |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A shrink-to-fit box whose line mixes text with an ATOMIC inline** — `<a class="float"><i class="icon"></i> <span>Label</span></a>`, a chip, a tab, a button with a badge, a logo beside a wordmark | every navigation bar, tab strip and pill on the web; `float`/`inline-block` + `text-align:center` is the pre-flexbox centring idiom and still ships in the CSS of most non-framework sites | ✅ (tick 871) — TWO defects, one family. (a) **`text-align` was changing an INTRINSIC measurement**: the probe lays the subtree out at a 1e6 available width, `text-align:center` distributes the leftover, and at 1e6 the leftover IS the measurement. `content_right_extent` already discarded that offset for TEXT (each line spanned from its own leftmost fragment) — but an atomic inline leaves the line as its own `LayoutBox`, so it was spanned ALONE and gave its width without its place on the line. **A centred float was sized to its widest single ITEM** (`[max-content] pref=62` where the content is 97). (b) **The space before an atomic was `measure(" ", SansSerif, 16.0)`** — a constant ~5px in every font at every size, under a comment saying so; the error does not scale with the font, which is the tell (at 32px it owes 19 and paid 5). The two compose into a WIDTH error that laundered into `reading-order`: `possssno.sbs`'s nav anchor came out 123x56 against Chrome's 152x38, so the label wrapped ABOVE the icon. OLD-vs-NEW binary, same hour, identical denominators: possssno **0.897 → 0.991** (503 misplaced → 4) and marktplaats **0.952 → 0.967**, both `reading-order 1 → CLEAN` = **two M1 crossings**; wikipedia/apple/a11yproject all rose; two controls byte-identical. Gated by `text_align_does_not_change_a_floats_intrinsic_width` and `the_space_before_an_atomic_inline_is_measured_in_the_font_that_owns_it`, both SELF-comparisons (no font metric hard-coded), each RED-proven by restoring only its own half |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A screen-reader-only span inside a floated toolbar button** — `.sr-only{position:absolute;width:1px;margin:-1px}` inside `.sidebar-toggle{float:left;padding:15px}` | Bootstrap ships `.sr-only` and every framework copied it; AdminLTE/Bootstrap admin headers put one in every toolbar button. The same shape carries React portal roots, dropdowns and tooltips anchored inside a floated or `inline-block` card | ✅ (tick 872) — a `float` and an `inline-block` are **sized before they are placed**, so both lay content out at a provisional `(0,0)` and translate it. `layout_float` shifts the boxes and the fragments; **`static_pos` is a THIRD output of the same inner layout** and was left in the provisional space, so every insetless `position:absolute` descendant was placed against the wrong origin. Chrome vs before: inside `float:left` `[56,15]` vs `[41,0]`; inside `inline-block` `[56,109]` vs `[56,15]`; inside `block` and `flex` **already exact** — the two that were wrong are exactly the two that lay out provisionally. ⚠ The guard `static_pos.len() grew` is WRONG and silently: a float's `shrink_to_fit` records the same key first and the real pass OVERWRITES it, so the first build fixed `inline-block` and left `float` — *a guard on a SIZE cannot see a REPLACEMENT*; it is a monotone write counter now. `ubys.bingol.edu.tr` **reading-order 1 → CLEAN** (M1 crossing), `littlecaesarsbcs` 0.9487 → 0.9615 (a four-reading byte-identical control moving in the allowed direction), five other controls byte-identical. Gated by `an_out_of_flow_childs_static_position_survives_its_containers_translate`, a self-comparison against `display:block`, each half RED-proven alone |

| pattern | where it shows up | status |
| --- | --- | --- |
| **The media object — `float:left` image beside an `overflow:hidden` text block** | the two-column layout of the entire pre-flexbox web: sidebar-and-content, avatar-and-comment, icon-and-description, label-beside-list. `overflow:hidden`/`auto`/`flow-root`/`display:table` are all the same idiom — "make this a BFC so it sits next to the float instead of under it" | ✅ (tick 873) — **CSS 2.1 §9.5 has two halves and only one was built.** A PLAIN block correctly overlaps a float (only its line boxes shorten) and that half was gated; *"the border box of … an element in the normal flow that establishes a new block formatting context must not overlap the margin box of any floats"* did not exist, so the text block rendered UNDER the image. Chrome vs before, a 100px left float in a 400px container: `overflow:hidden` `[100,300]` vs `[0,400]`; `flow-root`, `overflow:auto`, `display:table`, right floats, both-sides and margin cases all likewise, eleven fixture rows now byte-identical. ⚠ Two details the fixture pinned that reasoning would have got wrong: the band is read at the box's **TOP edge only** (a short float does not widen the box lower down), and `margin-left` is **ABSORBED not added** (20px margin ⇒ Chrome `[100,300]`, not `[120,280]`). ⚠ A SPECIFIED width is deliberately unchanged — Chrome shifts it only while it still fits (`300px` shifts, `301px` does not), we never shift; measured, named, left for its own tick. `www.library.chiyoda.tokyo.jp` **overlap 1 → 0 CLEAN** (M1 crossing); eight controls byte-identical; `sestra.cc`'s apparent −0.01 sits inside its own three-run spread on one binary. Gated by `a_bfc_root_is_placed_beside_a_float_and_a_plain_block_is_not`, which asserts BOTH halves so that shortening every block beside a float fails rather than passes |

| pattern | where it shows up | status |
| --- | --- | --- |
| **An off-canvas drawer — `class="fixed inset-y-0 left-0 w-64 flex flex-col -translate-x-full"`** | the mobile/side navigation of every Tailwind, Bootstrap-5 and Material site; the same shape carries hover-lift cards, carousel tracks and centred modals — anything that is a flex container inside a flex container and uses `transform` | ✅ (tick 874) — **`transform` was SILENTLY DISCARDED on a flex/grid item that is itself a flex or grid container.** The rule is written in `layout_block` (every ordinary box) and in the out-of-flow pass (every abspos box); `extract_placed`, the third emitter, had neither. A LEAF flex item goes through `layout_block` and was always right — so the same page transforms correctly or not at all depending only on whether the box carries `display:flex`. Chrome vs before, a 120x40 flex item in a flex row: `display:block translateX(50px)` `[170,0]`/`[170,0]` ✓, `display:flex` same transform `[170,0]`/`[120,0]`, `display:grid translateY(10px)` `[120,142]`/`[120,132]`, `display:flex scale(2)` `[60,178 240x80]`/`[120,198 120x40]`. On `desiviral.net` the off-canvas `aside` **never left the screen** and sat on top of the header and footer: `overlap 5 → 0 CLEAN`, shape 0.819 → 0.847, M1 crossing. ⚠ Found via a number that made no sense — the `aside` reported **512px wide against a `w-64` (256px)** — which was a UNION of two boxes: an out-of-flow child of a flex container is emitted TWICE (flex path + out-of-flow pass) and the copies disagreed by the transform. That duplicate is named-not-fixed (Flexbox §4 says it should not be emitted by the flex path at all, but deleting a box is how elements vanish). ⚠ Also sighted: `height:100%` on a `position:fixed` box resolves against the DOCUMENT, not the viewport (Chrome 713, ours 3000). Gated by `a_transform_applies_to_a_flex_item_that_is_itself_a_flex_container`, a self-comparison with an "it must move at all" clause — RED-proven |

| pattern | where it shows up | status |
| --- | --- | --- |
| **One reused `<template>`, written repeatedly — `tpl.innerHTML = html; tpl.content.firstChild`** | the DOM factory of every compiler-based framework: Vue 3's `runtime-dom` keeps ONE module-level template and writes it per static block, and lit-html / Svelte / Solid all parse markup once through a template and clone the result. Also every hand-rolled `renderTo(el, html)` helper that caches its template | ✅ (tick 882) — **`innerHTML` on a `<template>` wrote to the ELEMENT'S CHILD LIST, and a template's child list is always empty in a browser.** DOM Parsing redirects the setter to the template CONTENTS; ours did not, and it survived only because `template_content` materialises the fragment **lazily and once**, moving the direct children in on first access. So the single ordering anyone had gated — *set `innerHTML`, then read `.content`* — worked by accident and nothing else did. Chrome vs before: `.content` read BEFORE the write `1`/**`0`** (and `.childNodes` **1** where Chrome says 0); a SECOND write of two nodes `2`/**`1`** — the first write's node; `t.innerHTML = t.innerHTML` **kept**/**ERASED**, because the getter walked the child list too. On `pt88.app` Vue's `insertStaticContent` reads the stale fragment and throws *"can't access property firstChild, l is null"* **inside an async render where nothing is listening** — one throw, and the app is over: **3 comparable elements → 132 scored at 0.629**, `portal.ensuretyfinance.com` **0.864 with coverage 100% (M1 crossing)**. Controls unmoved (`news.ycombinator` 0.801/805, `blog.rust-lang` 0.996/1664). ⚠ Named residue, a DIFFERENT mechanism: the FRAGMENT parser drops foreign content — an `<svg>` in `template.innerHTML` returns `nodeName "SVG"` in the xhtml namespace where Chrome gives `svg` in the SVG namespace (document parsing is right; `parse_fragment_in` → `clone_into` is not). Gated by `G_TEMPLATE_CONTENT`'s four new claims, each Chrome's byte-exact answer and RED-proven in both halves |

| pattern | where it shows up | status |
| --- | --- | --- |
| **An injected inline SVG icon — `el.innerHTML = '<svg>…</svg>'`, or `tpl.content.cloneNode(true)`** | how essentially every icon reaches the page outside of markup: Lucide/Feather/Heroicons injection, D3 and Chart.js building charts, Snap.svg, and every compiler-based framework instantiating a component through a `<template>` clone | ✅ (tick 883) — **the PARSER was right and every COPY was wrong.** Two implementations of one rule, in two different crates — `manuk_html::clone_into` (innerHTML · insertAdjacentHTML · createContextualFragment) and `dom_bindings::clone_node` (cloneNode · importNode) — both built every node with `create_element`, the HTML namespace unconditionally. Cloning an element the parser had got RIGHT produced a wrong one, which is what ruled the parser out. Chrome vs before: `innerHTML '<svg>'` `2000/svg\|svg` vs **`1999/xhtml\|SVG`** (a foreign element's `nodeName` is not uppercased — the tell a namespace-only check misses), `cloneNode` of a correct parsed `<svg>` likewise, `tpl.content.cloneNode(true)` likewise, `importNode` likewise; `createElementNS` was already right, which is what made the two halves disagree. ⚠ **The obvious consequence was measured and is FALSE**: geometry is byte-identical before and after (`<svg viewBox>` in a 400px block `400x200`, bare `<svg>` `300x150`, both matching Chrome) because our layout keys on the TAG. What it buys is the property `parsedEqMade` names — the same markup reached two ways produced two different DOMs, so every library branching on `namespaceURI` or `instanceof SVGElement` was right about parsed SVG and wrong about injected SVG. ⚠ Residue: `getComputedStyle(rect).fill` is `undefined` vs Chrome's `rgb(255, 0, 0)`. Gated by `G_FOREIGN_CONTENT_NS`'s eight new claims incl. `tplclone` (crosses BOTH emitters) and `cloneplain` (the guard — a copied `<div>` must stay XHTML and still uppercase), each RED-proven per emitter |

| pattern | where it shows up | status |
| --- | --- | --- |
| **An IndexedDB wrapper that feature-detects on the PROTOTYPE — `'getAll' in IDBObjectStore.prototype`** | `idb` is the ecosystem's dominant IndexedDB wrapper: Firebase's offline persistence, Workbox's precache and expiration, and a large share of every PWA's local store are built on it. The same prototype-membership idiom is how ad-blockers, error trackers and polyfills hook any platform object | ✅ (tick 884) — **the methods were OWN properties, so `typeof store.getAll === 'function'` was true and nobody asks that.** `idb` creates `db.get` / `db.getAll` / `db.put` only if the name is `in` `IDBObjectStore.prototype` (or `IDBIndex.prototype`); ours were EMPTY, so the API was never built and the page died on `this.idb.getAll is not a function` — harvested live from `coinmarketcap.com`, which also logged `getFromLocalDB TypeError: t.get is not a function` four times in one load. Chrome answers `true` to all ten `in` tests, we answered `false` to all ten. The closures now live in a private slot with a DISPATCHER on the prototype, which also makes **prototype patching take effect** (an own method silently shadowed it) and makes a foreign receiver throw `TypeError` like Chrome's "Illegal invocation". ⚠ Two traps, both hit: a LAZY prototype (populated when a store is built) passes a probe that opens a database first and fails the return visit, where no `upgradeneeded` fires and `db.get(...)` is the FIRST call; and `iface()` runs AFTER this prelude, so the eager block found `undefined` and skipped all four interfaces in silence until it began creating the constructor itself. ⚠ The gate's own `stubs=none` claim caught SIX names with nothing behind them on its first run, including two the author had swept in from a regex. **M1 did not move** (coinmarketcap 26.2% → 26.3% on the same 2046 elements): the failure was on a cached-data path, and the page now fails one layer deeper at `no object store named local-key-val`. Gated by `G_INDEXEDDB_PROTOTYPE`, both halves RED-proven |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A promise rejected with something that is not an `Error`** | how most real failures reach a page's error reporter: `fetch` handlers reject with a `Response`, jQuery's `$.ajax` with the jqXHR object, and a large share of ad/analytics bundles with a bare config object. It is also the ONLY diagnostic a silently-blank SPA emits | ✅ (tick 891) — **the reporter printed `String(reason)`, which is `[object Object]` for every one of them.** On `beb88run.xyz` that was the page's entire output: sixteen identical lines naming a count and nothing else, while a 458-element carousel subtree was missing. Now described — constructor name, first six own keys, a JSON body clipped at 300 chars — and the same sixteen resolved to **XHR objects at `readyState: 0` (UNSENT)**: sixteen AJAX calls that never opened, which is why Slick had nothing to build from. ⚠ Bounded on purpose (a log that dumps an object graph is as unreadable as `[object Object]`), a primitive passes through untouched, a host object falls back to its tag, and `__`-prefixed keys are filtered so OUR `__nodeId` is not advertised as the page's own state. ⚠ It immediately surfaced a second defect, named and NOT folded in: `getResponseHeader`/`setRequestHeader`/`overrideMimeType` are own enumerable properties on our XHR instances where Chrome has them on `XMLHttpRequest.prototype` — the same defect as tick 884's IndexedDB row, on a different interface. Gated by `G_REJECTION_DESCRIBES_ITS_VALUE`, RED-proven by restoring `String(reason)` |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A page that serialises, clones or `for…in`s an `XMLHttpRequest`** | every error reporter (Sentry, Bugsnag, homegrown `window.onerror` handlers) snapshots the failing request; jQuery's `$.ajax` rejects with the jqXHR object, so it lands in whatever the app logs; and any `structuredClone`/`JSON.stringify` of an app's state that holds a request does the same | ✅ (tick 892) — **this engine's private slots were ENUMERABLE**, so `JSON.stringify(xhr)` returned `…"_ls":null,"_m":"GET","_u":"","_id":null,"_h":[],"_respHeaders":[]` where Chrome returns `{}`. A page saw our internals as its own fields with no way to tell whose were whose. Found by t891's rejection describer, which printed sixteen rejected XHRs on `beb88run.xyz` with our slots sitting inside the page's data. Fixed by defining the six slots `enumerable:false`; assignment to an existing non-enumerable writable property keeps its attributes, so the delivery path's later writes needed no change (checked at all four write sites). ⚠⚠⚠ **AND t891's OTHER CLAIM ABOUT THIS OBJECT WAS WRONG AND IS NOW PINNED AS FALSE BY GATE CLAIMS**: it read `getResponseHeader`/`setRequestHeader` in the JSON and concluded the METHODS were own properties ("t884's IndexedDB defect on another interface"). Probed against Chrome: `'open' in XMLHttpRequest.prototype` true/true, `hasOwnProperty(xhr,'open')` false/false, and a page's `XMLHttpRequest.prototype.open` patch IS observed (1/1) — so every analytics hook and ad-blocker already works. A wrong FIX is caught by the next gate; a wrong LABEL by nothing, so `protoPatch:1` and `ownOpen:false` are asserted to stop a later tick "fixing" what works. ⚠ Still open, a different mechanism: the spec-visible fields (`readyState`, `status`, `responseText`, `on*`) are own data properties where Chrome has prototype ACCESSORS — which is why Chrome's `JSON.stringify(xhr)` is `{}` and ours is still populated. Gated by `G_XHR_EVENTTARGET`'s four new claims, RED-proven (`privKeys:6`) |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A jQuery page that talks to an API on another origin — `$.ajax({url: 'https://api.example.com/…'})`** | jQuery is still on a large majority of the web, and this is how essentially every jQuery site reaches a CDN price feed, an analytics endpoint, a widget backend or its own `api.` subdomain. The same one-property idiom is how any library decides whether the platform can do a thing at all | ✅ (tick 895) — **jQuery gates its ENTIRE cross-origin capability on `support.cors = "withCredentials" in new XMLHttpRequest()`, and we had no `withCredentials`.** With `support.cors` false its transport factory returns nothing for any `crossDomain` request and `$.ajax` calls `done(-1, "No Transport")` — which sets `jqXHR.readyState = 0` and rejects. That is exactly what t891 harvested from `beb88run.xyz` and t894 identified: sixteen `readyState: 0` rejections per navigation, a 4-second `await $.ajax()` poll of a cross-origin jackpot API, and a counter stuck on `UPDATING`. ⚠ **Everything else jQuery checks was ALREADY RIGHT**, which is why this took three ticks to reach: `new XMLHttpRequest()` succeeds, `<a>.protocol`/`.host` resolve, and jQuery's `crossDomain` computation is correct for relative, same-origin-absolute, protocol-relative and foreign URLs. A probe asking *"can we do cross-origin requests?"* answers yes — we can, and do not even enforce CORS. The library was not asking that: **ask what a library BELIEVES, not what it can detect.** ATTRIBUTED by the OLD BINARY, rebuilt from the reverted tree and run in the same hour on the same site: **16 unhandled rejections → 0**, shape 86.3% both (no geometry movement — the carousel needs the data to actually arrive, which is the next layer). The same block landed the five readyState constants on both the interface object and the prototype: they were absent from both, so `xhr.readyState === XMLHttpRequest.DONE` — the completion branch of every hand-rolled XHR wrapper — was `4 === undefined`, false silently forever. ⚠ HONEST BOUND: `withCredentials = true` is correct today; the `false` half (a cross-origin request must then send NO cookies, and ours still sends `SameSite=None` ones) is the pre-existing behaviour and needs a credentials-mode field through `take_fetches`'s tuple. `responseURL`/`responseXML` still absent (host plumbing), and `upload`/`XMLHttpRequestUpload` stays deliberately absent — `G_IFACE_SURFACE_2` asserts that, because we do not stream a request body. Gated by `G_XHR_CORS_GATE`, twenty-two claims against Chrome's measured answers, RED-proven (`jq-support.cors=false`, `jq-transport-for-crossdomain=MISSING`, `done-idiom=false`) |

| pattern | where it shows up | status |
| --- | --- | --- |
| **`$('<div/>')` — creating an element with jQuery** | the element factory of the entire jQuery-built web, and of every plugin on top of it: Slick, Owl, Bootstrap's own JS, Select2, DataTables, Fancybox. The underlying rule — `node.textContent = ''` — is *also* the most common way any page, framework or not, empties a subtree | ✅ (tick 896) — **the DOM standard's "string replace all" puts the empty case FIRST and we skipped it:** *"Let node be null. **If string is not the empty string**, set node to a new Text node…"*. We created one unconditionally, so a cleared node held `1` child where Chrome holds `0`. That is not a count — `jQuery.parseHTML` → `buildFragment` ends with `fragment.textContent = ""` immediately before appending the parsed nodes, so **`$('<div class="x"/>')[0]` came back a TEXT NODE**. Slick's `buildOut` does `$slides.wrapAll('<div class="slick-track"/>')`; `wrapAll` takes `.eq(0)` — the text node — descends `firstElementChild` (null on a text node, so it stays there) and `.append(this)`s every slide into it. On `beb88run.xyz`, the top site of t888's crossing cohort: **coverage 79.2% → 97.9%, missing boxes 458 → 46** (458 is t888's exact count), `div.banner-carousel` `1185×0` → a real `slick-list > slick-track > slide > a > img` subtree. ⚠ **SHAPE FELL 86.3% → 71.0% AND THAT IS THE COVERAGE WIN, NOT A REGRESSION** (t813-818's rule): `shape_n` 1739 → 2155, every previously-rendered box still renders, and you cannot have a reading-order violation on a node you do not render. The site was never an M1 pass — overlap 14 disqualified it at 0.868 exactly as it does at 0.710. ⚠ Next layer NAMED not taken: the restored subtree is MIS-SIZED (`slick-track [592 146 0×1]`, `<img>` `0×600` vs Chrome `1183×378`) — Slick's inline `width` from `setDimensions` is not landing, a *width* mechanism and a different subsystem. ⚠ **What made this survive: `innerHTML = ''` was ALREADY right** (it parses an empty string to no children), so one rule had two implementations and probing either alone exonerated the pair — both are asserted now. ⚠ Coercions are Chrome's and are not falsiness: `null` and `undefined` clear, `0` and `false` write `"0"`/`"false"`. Control panel (4 sites vs SWEEP-t887): three gain shape, every jarring dimension flat or better. WHOLE `manuk-page` suite run — 146 binaries, zero failures. Gated by `G_TEXT_CONTENT_REPLACE_ALL`, sixteen Chrome-captured claims, RED-proven (8 fail, incl. `jq-first=#text`) |

| pattern | where it shows up | status |
| --- | --- | --- |
| **`parseInt(getComputedStyle(el).width)` — measure an element, then size something from it** | the measure-then-size step of every carousel, tooltip, dropdown, sticky header, masonry grid and virtual list; `$(el).css('width')` is the jQuery spelling and `el.style.width = getComputedStyle(el).width` is how every animation library pins a start value before a transition | ✅ (tick 897) — **CSSOM makes `width`/`height` resolve to the USED value once the element has a box, and we returned the author's string.** Chrome vs before: `width:auto` in a 600px block `580px`/**`auto`**; `width:50%` `300px`/**`50%`**; an abspos sized by `left`+`right` `560px`/**`auto`**; a `flex:1` item in a 400px row `400px`/**`auto`**; a `33.333%` width `199.984px`/**`calc(-0.016662598px + 33.333336%)`**; and **every height was `auto`**. So `parseInt($(el).css('width'))` was `NaN` web-wide — jQuery only survives it by falling back to `offsetWidth` *when it sees `auto`*, a fallback itself gated on `getClientRects().length`. ⚠ **It was never a layout gap: `offsetWidth` was already exact (594/300 vs Chrome 594/300) and the layout `rect` was already a parameter of the very function that answered `auto`** — the identical shape to the `transform` defect, which moved the box correctly for sixty ticks before the number reached JS. ⚠ The box reported is the one the element's own `box-sizing` NAMES, measured because the plausible answer is wrong: `border-box; width:200; pad:10; bd:5` → `200px` (the border box, offsetWidth 200) while `content-box` with the same declaration → `200px` (the content box, offsetWidth 230). ⚠ Two guards that "always report the rect" would break: `display:none` reports its COMPUTED value (`70px`, not 0) and a non-replaced INLINE reports `auto` despite having a real border box. ⚠ **HONEST NEGATIVE: it did not fix the carousel it was found through** — `beb88run.xyz`'s `slick-track` is still `[592 146 0×1]`; what it did was eliminate that suspect, since jQuery's `.width()` was already returning 1185 via the offsetWidth fallback. ⚠ Control panel, same-day old-binary A/B: **two of four byte-identical**, one up; `sestra.cc`'s apparent −0.005 was re-read three times solo and sits inside its own spread (0.9249/0.9224/0.9176 on one binary). Whole `manuk-page` suite run — zero failures. Gated by `G_RESOLVED_WIDTH_HEIGHT`, 22 Chrome-captured claims incl. two RECONCILIATION clauses against `offsetWidth`, RED-proven (9 fail, incl. `jq-parse-width=NaN`) |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A stylesheet authored in LOGICAL properties, read back by script — `getComputedStyle(el).marginInlineStart`** | how every RTL-aware design system is written: Tailwind's `ms-*`/`me-*`/`ps-*`/`pe-*` utilities, Bootstrap 5's `ms`/`me` spacing, and every component library that ships one stylesheet for both directions. The same object is read by every measure-then-size widget, every animation library pinning a start value, and every `parseFloat(cs.<prop>)` layout calculation | ✅ (tick 902) — **`getComputedStyle` had been failing ONE PROPERTY PER TICK, so this tick ran the diff that lists the class:** 132 properties × 7 representative elements against Chrome, **411 differing readings of 924**, and the dominant shape was `undefined` rather than a wrong value — 51 properties absent from the object entirely. Named by t901's constitution check as an **I3** defect class (*the semantic model declining to publish what the pipeline already computed*), after `transform`, `width`/`height` (t897) and `zoom`/`containerType` (t900) had each been found separately. ⚠ **THE SPLIT IS THE DELIVERABLE, ASSERTED FROM BOTH SIDES:** the properties the cascade GENUINELY holds are published (`order`, `background-size`, `object-position`, `text-shadow`, `inset`, `grid-column-start`/`-end`, and the whole logical family incl. `inline-size`/`block-size`), and the **41 with no cascade field stay ABSENT on purpose** — emitting an initial value for a property this engine does not honour is @supports-style false presence, which routes a feature-detecting caller into a wall instead of its fallback. **411 → 321 differing, 14 properties fixed, ZERO newly broken.** ⚠ `grid-template-columns` is the instructive omission: the cascade holds it, but Chrome reports the USED track sizes in px (`98.6562px 197.344px` for `1fr 2fr`), so emitting `1fr 2fr` would be a **wrong answer of the RIGHT TYPE** — it stays absent and is asserted absent. ⚠ **This tick's own first version was wrong and the re-sweep caught it in one line**: `max-inline-size` went out through `dim_css` (unset → `auto`) while the physical `max-width` uses `max_dim` (unset → `none`), so the logical spelling disagreed with the physical one about the same box — two spellings of one box now share one serialiser and the gate asserts the identity, not the values. ⚠ And a LUMPED comment hid a real asymmetry: an unset `letter-spacing` is `normal` (it permits kerning) but an unset `word-spacing` is `0px`. Gated by `G_COMPUTED_STYLE_PUBLISHES_THE_CASCADE`, 34 claims — Chrome-captured except one deliberately labelled honesty-boundary row — incl. four RECONCILIATION clauses (`inlineSize === width`) and an enumeration clause (`item(i)` must reach the new names). RED-proven: 22 fail |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A `float:left` image or ad slot beside a FIXED-WIDTH card that establishes a BFC** | the media-object idiom and the entire pre-flexbox two-column web, still shipping: a floated thumbnail followed by a `width:300px` / `width:50%` panel with `overflow:hidden`, `flow-root` or `display:flex`. Every WordPress theme's `alignleft` image beside a sized widget, every sidebar-and-article layout written before 2017, and every modern `flex` section that happens to follow a legacy floated block | ✅ (tick 906) — **CSS 2.1 §9.5 says such a box must not overlap the float, and we shifted it only when its width was `auto`.** The `width:auto` half landed at t859 and `bfc_float_band`'s own comment named the other half and declined it: *"A SPECIFIED width is deliberately NOT handled here … Chrome shifts such a box beside the float only while it still fits (`width:300px` shifts to 100, `width:301px` stays at 0) … Measured, named, and left as its own tick rather than guessed at."* Measured against Chrome, a 100px `float:left` in a 400px `flow-root`, **8 of 14 rows wrong**: `width:300px` (fits the band exactly) `x=100`/**`x=0`**; `width:200px` `x=100`/**`x=0`**; `width:50%` `x=100`/**`x=0`**; `margin-left:20px` `x=100`/**`x=20`** (the margin is ABSORBED); a 10px-tall float under a 60px box `x=100`/**`x=0`** (the band is read at the TOP); `box-sizing:border-box` with padding `x=100`/**`x=0`**. ⚠ **The six rows that were already right are half the deliverable**: `width:301px` and `width:400px` are the spec's *"if necessary, implementations should clear"* half, and a fix that shifted unconditionally would satisfy the other eight and break these two — both directions are asserted. ⚠ **`cw` is returned UNNARROWED, and that is the whole difference from the `auto` arm.** The stated reason for declining this work was that narrowing the containing block *"would also change every percentage the child resolves"* — a real objection, answered by not narrowing it rather than by declining the shift: an auto box takes the band as its containing block because the band is what SIZES it, while a specified box keeps its width and only its ORIGIN moves. `width:50%` proves it from outside — Chrome resolves 50% against the 400px container, gets 200, and still shifts the result to 100. ⚠⚠⚠ **FOUND BY A CONFOUNDED FIXTURE, AND t905's REPORT OF IT IS RETRACTED IN THIS COMMIT.** t905 filed this as *"a BFC box fails to avoid a float that ESCAPED a previous sibling"* — that fixture set `width:400px` on its boxes AND wrapped the float in a plain `<div>`, two variables in one reading. With `width:auto` restored, all five escaped-float cases are Chrome-exact and always were. **Three defects across two ticks and all three were the fixture** (a missing `--hide-scrollbars` worth 15px, floats leaking between un-isolated rows worth 120px, and this confounded width): *a differential probe is only a control if each case varies ONE thing.* The gate header, the wiki section and the `CONSTELLATION.tsv` row are corrected here rather than left to age. ⚠ Still open and still named: `display:table` applies its `height` as a MINIMUM in Chrome (24 against our 20). `manuk-layout` 125/125; t905's own fixtures re-measured — escaped-float set 3 diffs → **0**, combined gate fixture 52 → **53** exact. Gated by `G_BFC_SPECIFIED_WIDTH_FLOAT_BAND`, 14 Chrome-captured claims in both directions, RED-proven by restoring the exact pre-t906 early return (`#s1 expected x=100, got x=0`); `G_RATIO_INSET_FLOAT` gains `#a9` and is RED-proven by the same revert, so the retraction is itself gated |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A `display:table` / `table-cell` box used for vertical centring or equal-height columns, with a `height` on it** | the pre-flexbox vertical-centring idiom (`display:table` + `display:table-cell; vertical-align:middle`), still shipping in every email-derived template, every legacy CMS theme, and every "equal height columns" layout written before 2016 — and `display:table-cell` remains the standard trick for a cell that must fill its row | ✅ (tick 907) — **CSS 2.1 §17.5.3 makes a table box's `height` a MINIMUM: *"the table's height is the maximum of the value of [the] 'height' property … and the sum of the row heights"*. We used it as a used value like any other block's, so a table whose content outgrew its declared height was CLIPPED to it** and everything below slid up. Chrome-captured, a 200px box at `16px/1.5`: `height:20px` with 24px of content → **24**/`20`; the same with three lines (72px) → **72**/`20`; `display:inline-table` → **24**/`20`; `display:table-cell` → **24**/`20`; `border-box` with `padding:5px` → **34**/`20`; and `max-height:10px` → **24**/`10`, because a table's `max-height` has no effect at all. ⚠ **The guard is half the deliverable**: `display:block; height:20px` must still clamp to 20 and overflow, so a fix phrased as *"let boxes grow"* rather than *"this is the table box's own rule"* would satisfy every other row and silently break every fixed-height block on the web — it is asserted beside the rows that moved. ⚠⚠ **FOUND TWICE, TWO TICKS APART, BY TWO UNRELATED PROBES**: `display:table` came out of t905's float battery as an open row nobody could explain, and `display:table-cell` came out of t907's missing-box battery with the identical 24-against-20 reading. A second sighting under a different subject is what turned a curiosity into a family worth a rule (t720-724's *"three sightings under three subjects were ONE bug"*). ⚠ Measured, named and NOT fixed, because they are the table ALGORITHM rather than the box's own height rule: a real `<table>`'s `border-spacing` (Chrome 30 against our 26 for the same cell; `<td>` 196 against our 200) and a `<td>` stretching to fill a taller table (Chrome 56, ours 26) — their numbers are in the gate header so the next tick need not re-measure. ⚠ `manuk-layout` 125/125, and every fixture from t905-t906 re-measured is now exact with ZERO diffs (the combined battery 54/54, float/BFC 12/12, specified-width 14/14, escaped-float 5/5). Gated by `G_TABLE_HEIGHT_IS_A_MINIMUM`, 11 Chrome-captured claims incl. the `display:block` guard, RED-proven by disabling both halves (`#t1 expected h=24, got h=20`) |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A plain `<table>` with no author CSS for its cell spacing** | every data table, pricing grid, spec sheet, wiki infobox and documentation table on the web, plus the entire email-derived and legacy-CMS layout tradition — none of which declares `border-spacing`, because the UA default is what they are written against | ✅ (tick 908) — **`table { border-spacing: 2px }` is in Chrome's UA stylesheet and was not in ours**, so the separated-borders inset was simply absent: every cell 4px too wide and flush against the table edge, and the table 4px too short per row. Chrome-measured on a 200px table with one `padding:0` cell: `<td>` **x=2 w=196**/`x=0 w=200`, table **h=28**/`h=24`, two cells **100/94**/`103/97`, two rows **h=54**/`h=48`. ⚠⚠⚠ **The property itself was already perfect, which is exactly why nothing caught it**: `border-spacing:10px` matched Chrome to the pixel and always had, `border-spacing:0` matched, `border-collapse:collapse` matched — parser, cascade and layout consumer all correct. **A capability that is correct whenever anyone asks for it, and wrong when nobody does, is invisible to every test that sets the property**; every fixture this engine had declared it. One line, and 19 of 23 measured rows went from wrong to exact. ⚠⚠ Found by a probe aimed at something else: t907 measured these rows, called them *"the table ALGORITHM rather than the box's own height rule"* and deferred them — **naming something out of scope is a hypothesis about its size, and that one was wrong by two orders of magnitude.** t907's header is corrected here and its `#t7`/`#t8` claims folded in (11 → 13). ⚠ The guards are asserted beside the fix, because a UA declaration is the easiest change to over-apply: `border-spacing:0` still collapses, an author's `10px` still wins, `border-collapse:collapse` still ignores spacing — all three as INSET relationships (`cell.x - table.x`) rather than coordinates, so ten stacked tables cannot make one regression print as twenty-three. ⚠ Still open, measured and named: the two-value form `border-spacing:10px 20px` drops its VERTICAL component (`ComputedStyle::border_spacing` is a single `f32` off `clone_border_spacing().horizontal()`, table 44 tall against Chrome's 64), and a `<td>` does not STRETCH to fill a taller table (Chrome 56 for one cell in a `height:60px` table, 27 each for two; ours stay at 24) — that second one genuinely is the height-distribution algorithm. Gated by `G_TABLE_BORDER_SPACING_UA_DEFAULT`, 19 Chrome-captured claims + 4 inset relationships, RED-proven by deleting the UA declaration (`#c1 expected w=196, got w=200`) |

| pattern | where it shows up | status |
| --- | --- | --- |
| **`<sup>` / `<sub>`, and `vertical-align` on inline TEXT** | footnote markers, citations, TM and (R), ordinals (1<sup>st</sup>), currency superscripts on every pricing page, chemical formulae and simple maths — plus `vertical-align` as the standard idiom for nudging an inline icon or badge against its label | ✅ (tick 914, PARTIAL and labelled) — **the line fragment for a word was built with `valign: VerticalAlign::Baseline` HARD-CODED, so the eight `vertical-align` match arms in `line_metrics` were UNREACHABLE for text** and ran only for atomic inlines (images, inline-blocks). t913 measured the family (thirteen cases, every one 24px where Chrome grows the line) and located the consumer branch — **and wiring the shift into that branch changed NOTHING**, because the producer was the bug: the builder is a `move` closure and nobody had captured the word's own `vertical-align`. ⚠⚠⚠ **A branch that ignores a field and a field that can only hold one value are indistinguishable from inside the branch, and only one of them is fixed by changing the branch** — same shape as t897, where the layout rect was already a parameter of the function that answered `auto`. ⚠⚠ **And `<sup>`/`<sub>` had NO UA RULE AT ALL** (Chrome: `sup { vertical-align: super; font-size: smaller }`), so every footnote marker and ™ on the web rendered as plain baseline text at full size — the third UA-default gap this window after t908's `border-spacing`. ⚠ **CHROME-EXACT and asserted**: the UA shrink (a `<sup>`'s own box is 18x15 against a plain span's 21x17, both engines) and four CONTROLS that must NOT grow — a plain line, `vertical-align:top`, and `super` on a 10px span and a 10px `<img>`. Those last two are the RULE: a raised inline that still fits inside the strut must not grow the line, because CSS 2.1 §10.8 is a UNION and not an addition, and a fix that added the offset would break them. ⚠ **NAMED, not asserted, because asserting it would bank an approximation as measured**: `super` lands 29 against Chrome's 30, `sub` 26 against 28, `middle` 26 against 25 — the keyword offsets reuse the constants the ATOMIC arms already use (shared through one `valign_text_shift`, so the two implementations cannot drift) and approximate what Chrome derives from the font's own `OS/2` superscript offsets. `vertical-align: <length>`/`<percentage>` is a third job: the enum has eight keyword variants and no length, so `10px` parses to `Baseline` and cannot be represented. ⚠⚠⚠ **BOTH HALVES SHIPPED IN ONE CHANGE, which is the ratchet and not a preference**: growing the line box without moving the glyphs would make every `<sup>` line taller with its text still on the baseline — a metric win bought with a visible regression. ⚠ manuk-layout 125/125; every fixture from t905-t913 re-measured with ZERO rows moving the wrong way, and the div-height probe went 18 to 19 exact. Gated by `G_VERTICAL_ALIGN_ON_TEXT`, RED-proven by restoring the hard-coded literal. ⚠⚠⚠ **CALIBRATED AT t915, ONE TICK LATER, AND THE OPEN NUMBER IS NOW CLOSED FOR `super`/`sub`.** One experiment found the rule: Chrome's growth over the same line without the shift is **+6 / +9 / +12 at 16 / 24 / 32px** — `6/16 = 9/24 = 12/32 = 0.375` exactly — and `+4 / +6` for `sub` (`× 0.25`); **the `line-height: 3` row is unchanged at +6, which settles the other half: the offset does not move with the line box.** It is the FONT SIZE, and per CSS 2.1 §10.8.1 (*"an appropriate offset for superscripts of the PARENT's baseline"*) the parent's — measurable rather than doctrinal, since a `<sup>` at `font-size: smaller` raises by the same amount as a full-size span. All ten calibration rows are Chrome-exact and `super`/`sub` in the original family are now 30 and 28 (from 24 before t914, 29/26 after it). ⚠ **The strut tuple gained a fourth member rather than the constant gaining a fudge**: `strut_ascent × 0.405` reproduces every row for THIS font and bakes its ascent/em ratio (0.927) into a constant that would be wrong for the next one, so `close_line`'s strut is now `(ascent, descent, line_height, font_size)` and the offset comes from the size the spec names. ⚠ Still open and named: `<sup>`/`<sub>` land 24 against 27 (their box is byte-exact at 18×15, so the 3px is the smaller fragment's half-leading and not the offset), `middle` 26 against 25 (DIRECTION-only), `text-top`/`text-bottom`, and the unrepresentable length/percentage. The div-height probe is now **19 of 20 exact**. ⚠⚠⚠ **AND `text-top`/`text-bottom` CLOSED AT t916, WHERE THE OLD FORMULA WAS A NO-OP IN DISGUISE**: `strut_ascent - a` is exactly zero whenever the fragment and the strut share a font — nearly every `<span>` on the web — so the arm existed, was reachable after t914, and did nothing. CSS 2.1 §10.8.1 aligns **two different boxes**: the *content area* (`ascent + descent`, the glyphs) and the *inline box* (`line-height` tall, the glyphs PLUS half-leading). Aligning the box's top with the content area's top shifts the fragment DOWN by the half-leading (~2.5px at `line-height:1.5`) and the line grows below; `text-bottom` is the mirror. Both now exact — **27** and **28** against 24 before — using the SAME floored `half_leading` the line itself uses, because a shift computed against a different rounding lands the box outside the box it asked for. **A no-op formula is worse than a missing one: a missing arm is visible, while one that cancels to zero on the common case reads as implemented in every review and passes every same-font fixture.** Family state after four ticks: `super` 30 ✓ · `sub` 28 ✓ · `text-top` 27 ✓ · `text-bottom` 28 ✓ · four controls at 24 ✓ · `middle` 26 vs 25 (open, direction-only) · `<sup>`/`<sub>` 24 vs 27 (open — their own box is byte-exact at 18×15 and the offset is verified at three sizes, so the residual is a smaller fragment's half-leading) · length/percentage unrepresentable |

| pattern | where it shows up | status |
| --- | --- | --- |
| **A form control on a line of text — `<label>Name <input></label>`, a search box in a nav bar, a button beside a link** | every login form, every search bar, every filter row, every newsletter signup and every inline "Apply"/"Go" button on the web: a control that shares a line with text, which is how controls are laid out unless the author has explicitly blocked them out | ✅ (tick 918) — **`last_line_baseline` returns `None` for an `<input>`, because its value lives on the ELEMENT and not in the tree**, so CSS 2.1 §10.8.1's fallback applied and the bottom margin edge became the baseline. Every text field, button and select therefore sat ENTIRELY above the line's baseline and made the line that held it too tall: `<div><input></div>` was **26** against Chrome's **24**. Chrome gives each control the baseline of its internal editor text; ours is synthesised as border + padding + the ascent of ITS OWN font (Chrome's UA gives these 13.333px Arial, not the page's 16px). ⚠⚠⚠ **THIS IS THE HALF t917 WAS MISSING, AND IT IS WHY THAT TICK WAS REVERTED WHOLE.** t917 corrected the controls' UA boxes to Chrome's measured values — all ten heights went exact — and the composite case got **worse**, 26 → 28, because a taller control pushes further below a baseline already in the wrong place. The ratchet says revert rather than trade, so it was reverted; the baseline **stands alone** (24 with the UA boxes untouched) and is what makes the UA correction landable beside it. ⚠⚠ **THE GUARDS ARE WHY THIS IS NARROW**: the synthesis fires ONLY where the real rule cannot — a text-bearing `inline-block` still uses its own last line (24), an `overflow:hidden` one still takes the §10.8.1 fallback (**31**, not 24), an empty one is unchanged, and `textarea` is excluded entirely because it is multi-line, Chrome takes its LAST line, and t917 measured it byte-exact at 36. **A row that is already right is not a row to route through a new mechanism.** A fix that gave every atomic a synthetic baseline would satisfy the three control rows and break all three guards. ⚠ Named, not asserted: an input with an explicit `height:40px` reads 47 against Chrome's 46 — Chrome centres the internal editor in a taller control, we place the baseline at border+padding+ascent regardless. ⚠ `manuk-layout` 125/125, `manuk-css` 28/28, `g_form_control_metrics` (the gate that caught t917's regression) green, and the twenty-case div-height probe is now **20 of 20 exact**. ⚠⚠⚠ **REVERTED AT t919 UNDER THE RATCHET, ONE TICK LATER.** The t919 sweep caught it: `secure5.entertimeonline.com` fell **0.872 → 0.692** on 39 elements. The pre-committed control resolved it in one pass — three solo runs on the current binary byte-identical at 0.692, two on the t913 tree at 0.872, and a bisect that cleared t914, t915 and t916 and landed on **t918**; reverting its layout hunk restored 0.871795. **The rule is Chrome-exact on nine isolated fixtures, four of them guards, and costs 0.18 shape on a real page** — and the ratchet does not weigh those against each other: a tick that buys one face by degrading another is a trade, and trades are refused. The engine hunk and `G_FORM_CONTROL_BASELINE` are both removed; `<div><input></div>` is back to 26 against Chrome's 24. **A fixture that refutes your hypothesis is the cheapest outcome; being refuted by the CORPUS is the second-cheapest and the one no fixture can substitute for** (t853's shape, where `hit_test`'s smallest-wins rule cost sixteen clickable links and was found by G6 on a real page). The next attempt has a sharper question — not *what is a control's baseline*, which the nine fixtures answer, but *which real-page control does the formula get wrong, and why* — with a named candidate (an input at explicit `height:40px` reads 47 against Chrome's 46, because Chrome centres the internal editor in a taller control) and `secure5.entertimeonline.com` as the reproducer. Status: **missing**, not gated |

| pattern | where it shows up | status |
| --- | --- | --- |
| **`vertical-align: -2px` on an inline icon or badge** | the standard idiom for optically centring an SVG icon, a status dot, a "NEW" badge or a chevron against the label beside it — every design system ships it, and `vertical-align: <percentage>` is how a raised marker is expressed relative to its own line | ✅ (tick 922) — **the `VerticalAlign` enum had eight keyword variants and NO LENGTH, so the parser's `_ =>` arm swallowed both the length and percentage forms into `Baseline`.** Not dropped with a warning, not stored and ignored — **parsed to a different, VALID value**, which is the shape this project rates most dangerous (a wrong answer of the right type), and it meant `vertical-align:-2px` was silently a no-op for the whole life of the engine. Chrome-measured on 16px/1.5 text: `10px` **34**/`24`, `-10px` **34**/`24`, `50%` **36**/`24`. ⚠⚠ **The percentage is of the element's OWN `line-height` (CSS 2.1 §10.8.1) — not the strut's and not the font size.** Here those are 24 and 16, and Chrome's 36 is `24 + 0.5 × 24`; resolving at parse time against the font size gives 32 and would have looked close enough to bank, which is why the variant keeps a RATIO and resolves in layout. ⚠ Three real match sites, which is why this was a tick and not a subsystem: the enum and its parser, the `line_metrics` atomic arms plus the `box_top` placement arms that mirror them, and the computed-style serialisation; the text path needed two lines. `Eq` comes off the enum because the variants now carry an `f32`. ⚠ The family is now **11 of 14 exact** (from 13-of-14 wrong at t913): `super` 30 · `sub` 28 · `text-top` 27 · `text-bottom` 28 · lengths and percentages exact · four controls at 24 · open: `middle` 26 vs 25 and `<sup>`/`<sub>` 24 vs 27, both rounding questions, and `<sup>`'s 3px is the same half-leading quantity t916 had to get exactly right for `text-top`. `manuk-layout` 125/125, `manuk-css` 28/28, `manuk-wpt` lib 98/98, and every prior fixture unchanged. Gated by `G_VERTICAL_ALIGN_ON_TEXT`, RED-proven by restoring the parser's `Baseline` fallback |

| pattern | where it shows up | status |
| --- | --- | --- |
| **`<sup>` / `<sub>` from the UA sheet, beside an authored `vertical-align`** | footnote markers, citations, ordinals, TM/(R), chemical formulae and simple maths — the elements the UA stylesheet is *supposed* to style so no author has to | ✅ (tick 923) — **the FONT SIZE arrived and the ALIGNMENT did not, because only one of them takes the recovery path.** `vertical_align` is one of the handful of properties `stylo_engine.rs` **recovers from MinimalCascade** into the Stylo map (stylo 0.19 exposes no computed longhand), and the recovery is an unconditional `cs.vertical_align = m.vertical_align`. t914 added `sup { vertical-align: super; font-size: smaller }` to the **Stylo** UA sheet and not to MinimalCascade's — so the size came from Stylo and was right, and the alignment came from MinimalCascade, which had never heard of `<sup>`, and its `Baseline` was written **straight over Stylo's correct `super`**. ⚠⚠⚠ **For a RECOVERED property the minimal sheet is not a fallback, it is the AUTHORITY** — adding a rule to the Stylo sheet alone is *worse* than not adding it, because the element then differs from its authored equivalent in exactly one property, which is the hardest shape to see. t851's pattern and t846-852's "BOTH UA sheets wrong in OPPOSITE directions" with a new edge; and `engine/css/src/lib.rs` already carried *"Keep in lockstep with the UA sheet in stylo_engine.rs"* in its own comments while the drift happened anyway, nine ticks later, in a tick that read that file. ⚠⚠ **Diagnosed by twelve mixed-font cases of which ELEVEN were already exact**: an authored `<span style="font-size:13.333px;vertical-align:super">` grows its line to Chrome's 27 and `<sup>` — the same size, the same raise — stayed at 24, so the question stopped being "how does half-leading fold" and became "what does the UA sheet do differently". Our `<sup>`'s box was 36×15, byte-identical to Chrome's, the whole time. ⚠ The family is now **13 of 14 exact**, from 13-of-14 WRONG at t913: super 30 · sub 28 · text-top 27 · text-bottom 28 · lengths/percentages exact · `<sup>`/`<sub>` 27 · four controls at 24 · open: `middle` 26 vs 25, a 1px rounding question. Gated by `G_VERTICAL_ALIGN_ON_TEXT`, which gains the mixed-font fixture as a **LOCKSTEP GUARD** — a `<sup>` and an authored span at the same size and alignment must produce the same line, so if the two sheets drift again those twelve disagree while every keyword claim still passes. RED-proven by removing the MinimalCascade arm |

| pattern | where it shows up | status |
| --- | --- | --- |
| **`border-spacing: <h> <v>` — a table that spaces its rows differently from its columns** | the two-value form is how every hand-styled data table, pricing grid and calendar separates rows from columns, and it is the only way to do it in the separated-borders model | ✅ (tick 925) — **`ComputedStyle::border_spacing` was ONE `f32`, fed from `clone_border_spacing().horizontal()`**, so the vertical half was dropped and the ROWS were inset by the COLUMN value: `border-spacing: 10px 20px` is **64** tall in Chrome and was **44** here. The parser's own comment said so — *"Only the first (horizontal) length is used in this slice"* — which is the useful half of a comment that documents a gap and the dangerous half of one nobody re-reads. ⚠⚠ **The single-value rows are what make the new claim assertable**: `10px` alone must still set BOTH axes (44), `0` must still collapse (24), and the UA `2px` default must still hold (28) — so a fix that read the second value and forgot the shorthand would satisfy the new claim and break four old ones. ⚠⚠⚠ **AND THE FIRST RED PROOF PASSED, WHICH IS THE LESSON**: mutating MinimalCascade's parser left the gate GREEN, because `stylo_map.rs` reads the pair from Stylo's own `clone_border_spacing()` and a `manuk-page` gate runs the SHIPPING cascade. The proof only bites when the Stylo mapping is reverted. **Falsification has to hit the path the gate actually runs** — the standing `live-cascade-is-stylo-not-minimal` note has been about fixes and applies identically to RED proofs, where a proof aimed at the other cascade is indistinguishable from a gate that cannot fail. Both cascades are updated here, because t923 landed one tick earlier on exactly the drift between them. ⚠ `manuk-layout` 125/125, `manuk-css` 28/28; the border-spacing battery goes 19 → **20 of 23** exact, with the three remaining rows the same ones t908 named (a `<td>` does not STRETCH to fill a taller table — the height-distribution algorithm, not a length). Gated by `G_TABLE_BORDER_SPACING_UA_DEFAULT`, RED-proven against the shipping cascade |

| pattern | where it shows up | status |
| --- | --- | --- |
| **`max-width: min-content` / `min-width: max-content` / `max-height: fit-content` — an intrinsic sizing keyword used as a BOUND rather than as the size** | the "never let this wrap" toolbar or nav row (`min-width: max-content`), the "keep this column as narrow as its longest word" table cell (`max-width: min-content`), the panel that stops growing when its content does (`max-height: fit-content`), and the modern-CSS habit of writing `min-width: fit-content` instead of a magic pixel number. **5 of 85 cached corpus snapshots carry one, and that is a LOWER BOUND** — the snapshots are the `curl`'d HTML, so external stylesheets, where nearly all CSS lives, are not in the count | ✅ (tick 930) — **the keywords resolved exactly on `width`/`height` and were UNREPRESENTABLE on all four min/max properties.** `ComputedStyle` has carried `width_keyword` since t153 and `height_intrinsic` since t146; the four min/max properties were plain `Dim`, and `Dim` has no intrinsic variant — so on **both** cascades `min-content` fell through to `Dim::Auto`, which the clamp reads as **0 on a min** and as **no limit on a max**. The declaration parsed to a different, VALID value and silently did nothing: `max-width:min-content` left the box filling its container, `min-width:max-content` let it crush below its own content. Same shape as t922's `vertical-align: <length>` — **a wrong answer of the right type** — and the second time in nine ticks that the defect was an enum unable to say what the author wrote. ⚠⚠ **Twelve of twenty Chrome-differential cases wrong, every CONTROL exact**: `max-width:min-content` 400 → **48** (Chrome 48.17) · `max-width:max-content` 400 → **164** (163.77) · `min-width:max-content` in a 20px CB 20 → **164** · `min-width:fit-content` in a 20px CB 20 → **48** (NOT max-content — the stretch-fit is 20, which is the row that proves the three keywords are not aliased) · `width:400px; max-width:min-content` 400 → **48** · `height:200px; max-height:min-content` 200 → **48** · `height:1px; min-height:min-content` 1 → **48**. ⚠ **The fix is representation, not new sizing code**: four `Option<IntrinsicSize>` sidecars, parsed on both cascades and consumed by the min/max clamp in **three** layout paths (`layout_block`, `layout_float`, abspos — each carries its own copy of §10.4). The inline axis calls the same `min_content_width`/`max_content_width`/`shrink_to_fit` the `width` arm has always used, so the Bar-0 and recursion profile is unchanged; the block axis is ONE value, because a box's min-content and max-content *block* sizes are the same quantity. ⚠⚠⚠ **`fit-content(<length>)` IS INVALID ON ALL FOUR, and only a measurement says so** — the grammar reads as though the functional form goes wherever the keyword goes, and Chrome drops it (`min-width:fit-content(50px)` → `0px`, `max-width:fit-content(50px)` → `none`). The `width` parser deliberately accepts it, so the min/max arms needed a SEPARATE, narrower parser; sharing one would have made us **more permissive than Chrome**, laying out a box Chrome does not, and no fixture written from the fix would ever have shown it. ⚠ **The I3 half landed in the same tick**: `getComputedStyle(el).maxWidth` returned **`"none"`** — the string that means *there is no cap* — while the box was capped; all eight spellings (four physical, four logical) are now byte-identical to Chrome and both spellings route through ONE serialiser. Gated by `G_INTRINSIC_MIN_MAX` (14 layout claims) + `G_INTRINSIC_MIN_MAX_CSSOM` (16 claims), RED-proven three ways — deleting the `layout_block` arms, deleting **only** the Stylo sidecar (the fixture runs the shipping cascade, so a `MinimalCascade`-aimed proof would have passed against a broken engine, per t925), and making the serialisers ignore their keyword. Both carry CONTROL rows that a fix taking the new branch unconditionally fails. ⚠ **Bound, named with its number:** a FLEX ITEM's intrinsic min/max is still dropped (`flex:1; max-width:min-content` measures 400 against Chrome's 48.17) — taffy 0.12 has no intrinsic-keyword `Dimension`, so the keyword must be resolved to px before `to_taffy_style`, which today takes only a `ComputedStyle` and has no measurer |

| pattern | where it shows up | status |
| --- | --- | --- |
| **An intrinsic sizing keyword on a FLEX or GRID ITEM — `width: min-content` on a card title, `max-width: min-content` on a toolbar cell, `min-width: max-content` on a nav row that must not wrap** | this is where the keywords are actually *used*: the block-level `max-width:min-content` of t930 is the rarer spelling, because the layouts that need "hug your longest word" or "never wrap these buttons" are flex rows and grid tracks. **8 of the 85 cached corpus snapshots (9.4%) carry an intrinsic keyword AND a `display:flex\|grid`** — 30 carry flex/grid at all — and that is a LOWER BOUND, since the snapshots are the `curl`'d HTML and external stylesheets are not in the count | ✅ (tick 931) — **the sidecar stopped at the taffy border.** t930 taught `ComputedStyle` to hold the keyword on all four min/max properties and taught the BLOCK path to honour it, naming "a flex item's intrinsic min/max is still dropped" as its bound. The Chrome differential written before the fix says **that bound was half its true size**: `width_keyword` has existed since t153 and the flex/grid path never read it either, so plain `width:min-content` was wrong on a flex item and on a **grid** item too. Three formatting contexts, one sidecar, one reader. ⚠⚠ **The before-state is legible from the numbers, which is what identifies the mechanism**: an intrinsic width is `Dim::Auto` PLUS a sidecar, the sidecar did not cross, so every keyword became `Dimension::Auto` — *"size me from my flex basis"*. Hence **109.30 (max-content) in a wide container, 37.33 in a narrow one, 400 when the item also grew**: not one wrong number but the flex-basis answer, a different and VALID one. `flex width:min-content` 109→**37** · `flex max-width:min-content` 109→**37** · `flex min-width:max-content` in a 20px CB 37→**109** · `flex:1; max-width:min-content` 400→**37** (t930's own row) · `grid width:min-content` 400→**37** · `grid max-width:min-content` 400→**37**. ⚠⚠⚠ **taffy 0.12 CAN hold a `CompactLength::min_content()` and `Dimension::from_raw` accepts one — the obvious fix, and it compiles — but `Dimension` validates as `LENGTH\|PERCENT\|AUTO`, so the flexbox algorithm reads a tag it does not answer.** That is not "more permissive than Chrome"; it is **asking a dependency a question outside its grammar**, which has no defined answer at all. Option 3 of the borrowed-engine table instead: resolve to px through the measure callback already threaded through `TaffyDom`, which bottoms out in the same `measure_intrinsic` the block path uses — so the two contexts cannot drift apart later. ⚠⚠ **`box-sizing` has NO effect on an intrinsic keyword, and only a measurement says so** — the grammar invites the opposite: with `padding:0 10px` Chrome gives the same **57.33** border box under `content-box` AND `border-box`. taffy subtracts the frame from `size` under border-box, so it is added back there; the gate asserts the two spellings are EQUAL, so a fix that skips it is wrong in exactly one row and cannot pass both. ⚠⚠ **`fit-content` is deliberately left as `Dimension::Auto`, and that is a measurement rather than an omission**: it is `min(max-content, max(min-content, stretch-fit))` and the stretch-fit inside a flex line does not exist at style-build time — taffy's `auto` + `flex-basis:auto` + `flex-shrink` IS that clamp, Chrome-exact in a wide container (109.30) and a narrow one (37.33). Three of the six CONTROL rows exist to catch the over-generalisation that resolves it anyway. Gated by `G_INTRINSIC_FLEX_GRID` (15 claims, 6 CONTROLs), RED-proven three ways — delete the resolver call, drop the `frame` term (the two box-sizing rows split 57 vs 37), resolve `fit-content` eagerly (the `min-width:fit-content` CONTROL goes 37→109). ⚠ **Two bounds, and one is a Bar-0 question not a scope one:** the BLOCK axis on a flex item (`height:200px; max-height:min-content` measures 200 vs Chrome's 18 — a block-axis intrinsic size is the content height AT the resolved width, which does not exist yet), and an item that is **itself a flex/grid container** (`display:flex; width:min-content` measures 109.30 vs 37.33) — resolving THAT one **re-enters**, because the measure callback answers a container's intrinsic width by building a second `TaffyDom` whose `add` reaches the resolver again on the same node, unbounded. The `container` guard at the call site is what keeps the recursion profile identical to before |

| pattern | where it shows up | status |
| --- | --- | --- |
| **`display:table` + `display:table-cell` with NO `table-row` between them — the pre-flexbox vertical-centring and equal-height-columns idioms** | `.outer{display:table;height:100px} .inner{display:table-cell;vertical-align:middle}` is how a decade of CSS centred a box vertically, and the same pair without the height is how it made columns equal-height. Both are everywhere in the legacy/CMS/theme markup that makes up the **CrUX tail** the corpus switch is deliberately steering into. ⚠ **Usage weight here is NO INFORMATION rather than zero**: 0 of 85 cached snapshots carry `display:table-cell` in inline CSS and 5 carry `display:table`, but the snapshots are the `curl`'d HTML and a layout idiom like this one always lives in an external stylesheet — a lower bound of zero measures the instrument, not the web | ✅ (tick 932) — **the cell was not mis-sized, it was DROPPED ON THE FLOOR.** CSS 2.1 §17.2.1 generates an anonymous table-row around a `table-cell` whose parent is a `table`; `collect_table_rows` recognised only `table-row` and `table-row-group`, so a bare cell matched no arm, the table had no rows at all, and it took the rowless shrink-to-fit path and collapsed onto its own text. ⚠⚠⚠ **A 392px container-width error — burndown family #1 (`PHASE0-RENDER-BURNDOWN.md` §3.1, width errors launder into wrap → line-count → dy) in the grossest form available.** `width:50%` inside a bare cell came out **4px against Chrome's 200**, the percentage resolving against a container that had collapsed from 400px to the **8px width of the letter it contained**; every line of prose in such a container re-wraps and the whole subtree's height is wrong beneath it. ⚠⚠ **And in one arrangement it is a MISSING_BOX, not a geometry error**: `bare cell · real row · bare cell` produced **no box at all** for the first and third cells (Chrome lays out three rows, we emitted one) — the coverage-killing class, reached through a shape-shaped door. ⚠⚠ **Chrome's semantics have three separate clauses and each needed its own fixture row**: consecutive cells share **ONE** anonymous row (side by side at x=0 and x=200, not stacked) · a real `table-row` **BREAKS** the run (`bare · row · bare` is three rows in document order, so the accumulator flushes when a real row or row-group is seen) · the anonymous row carries **`None`** for its node, which is what an anonymous box is — no style lookup, no background of its own, no node on the emitted `LayoutBox`; the consumer already took an `Option<NodeId>` from the `<tr>`-has-real-geometry fix, so this slotted into the shape that was there. **The discriminator was in the first fixture and is exact: with an explicit `display:table-row` we were already correct.** Gated by `G_ANONYMOUS_TABLE_ROW` (10 claims, 2 CONTROLs), RED-proven three ways — stop accumulating bare cells (two cells report 8 and 8) · give each bare cell its **own** row (they report 400 and 400 — the plausible wrong fix, which satisfies "a bare cell is no longer dropped" completely and gets the arrangement wrong) · drop the flush before a real row (`#run_a` reports 200). ⚠ **The same missing algorithm remains, reached by a new door**: a cell does not STRETCH to fill a taller table (`display:table;height:100px` + one bare cell is 400×**24** vs Chrome's 400×**100**), so the `vertical-align:middle` half of the centring idiom still does not centre — the box is merely the right width now instead of 2% of it. That is the height-distribution residue t908 and t925 already named on real `<table>` markup |
