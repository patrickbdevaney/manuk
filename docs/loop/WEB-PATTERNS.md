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
| **parent↔child margin collapsing** (`<div class=card><h2>…</h2></div>` — a heading/first-block margin, and the symmetric last-block bottom margin) | every content page's vertical rhythm: a first/last child's margin must **escape** a border/padding-less, `overflow:visible`, auto-height parent, not sit inside it as a gap — the single most common vertical-layout difference from Chrome on ordinary document pages | ✅ (tick 151) — CSS2 §8.3.1's last unmodeled case (`layout_block` did adjacent-sibling collapse only). A left/right-spine peek (`collapse_through_top`/`collapse_through_bottom`) folds the first/last in-flow block child's collapse-through margin into the box's own edge margin (top: box raised + child placed flush; bottom: trailing margin removed from content height + collapsed into `margin_bottom`). Conservative eligibility (`display:block`, `overflow:visible`, no BFC, no border/padding on that edge; bottom also auto-height; clearance/out-of-flow first-child declines). `overflow:hidden` correctly still contains. css-flexbox 26.5→26.6%, css-sizing 14.5→14.8%, position/overflow/normal-flow flat, HANG/CRASH 0; gated by `parent_child_top_margin_collapses` + `parent_child_bottom_margin_collapses` (RED on disabling eligibility) and the guards `overflow_hidden_contains_child_margin` + `top_border_blocks_margin_collapse` |
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
