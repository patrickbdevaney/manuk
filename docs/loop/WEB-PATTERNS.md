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
| `<canvas>` 2D | Charts, games, visualisations | ✅ **it rasterizes** (`G_CANVAS`). Fills, strokes, paths (incl. `arc`), the transform stack, `clearRect`, real `getImageData`, real `toDataURL` — on tiny-skia, the same rasterizer that paints the page. **And the pixels reach the screen**: a canvas is composited as an image the page drew into, through the very map an `<img>` lands in. Not done: `fillText`, `drawImage`, `clip`, real gradients (each an honest no-op, not a lie). |
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
| **HTML attribute reflection** — `a.href`, `input.disabled`, `img.width`, `td.colSpan`, `form.action`, `option.selected` … | **How ordinary page code touches the DOM.** `if (input.disabled)` reading `undefined` does not throw — it silently takes the wrong branch | ✅ **generic** (`G_REFLECT`). **They were ALL `undefined`.** ~38,000 WPT subtests — 80% of `html/dom`'s failures — behind one mechanism. `html/dom` **21.0% → 37.7% (+9,940 subtests)**. Boolean is *presence* (`el.disabled = false` **removes** the attribute); URLs resolve against the base. **Numeric coercion made spec-correct (tick 117, `G_REFLECT_NUMERIC`, +437):** `-0`→`+0` (JS `parseInt("-0")` is `-0` and `Object.is` fails on it), overflow *falls back* to the default rather than ToInt32-wrapping, `maxLength`/`minLength` default `-1`, and `clamped unsigned long` (`colSpan`) *clamps to max* instead of falling back. **Not done:** `tokenlist` (`relList`, `sandbox`) — skipped rather than stubbed, because a string where a `DOMTokenList` belongs is worse than `undefined`. |
| `setAttributeNS` / `getAttributeNS` / `hasAttributeNS` / `removeAttributeNS` | SVG's `xlink:href`, MathML, and every XML-ish document — it is how they set an attribute **at all** | ✅ (`G_CAPABILITY`). `setAttributeNS is not a function` was **160 failing subtests**, found by *reading the failure messages the harness had been printing all along*. **+170 subtests in one fix.** Honest limit: the namespace is validated then ignored for storage (attributes are keyed by qualified name), which no real page can tell apart. |
| `DocumentType` / `createDocumentType` / `document.doctype` | quirks-mode branching, XML/XHTML tooling, DOM serializers that must re-emit the doctype | ✅ (`G_CAPABILITY`). `createDocumentType()` returned a **plain object literal** — prototype `Object`, so `instanceof DocumentType` was false — and validated nothing. `document.doctype` was `null` on every page, including one that plainly declares `<!doctype html>`. |
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
