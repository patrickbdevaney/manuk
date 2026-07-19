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
