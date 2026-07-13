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
| A fault *inside* SpiderMonkey's C++ frames | Browser dies | ⚠️ **not containable in-process.** Needs a per-tab process. Deferred, and stated rather than hidden. |
| ~1 site in 4 still **hangs** (CPU + duplicate work) | Unusable | ❌ **the top open item** |

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
| `@layer`, `var()`, `calc()` | Modern design systems | ✅ |
| **`font-family`** | **Literally every page** — was never mapped from the cascade at all | ✅ |
| `line-height: normal` from real font metrics | Every line box on every page | ✅ |
| **Intrinsic aspect ratio** (`img { max-width:100% }`) | **Every responsive image on the web** | ✅ |
| Background tiling / `background-size` / `-repeat` | Every sprite, texture, pattern, icon | ✅ |
| `font-size: 0` (inline-block gap killer, image replacement) | Painted **glyph-shaped continents** across the page | ✅ |
| `<source>`, `<track>`, `<picture>` | Responsive images — got phantom boxes | ✅ |
| SVG (inline, `<img src=*.svg>`) | Icons everywhere | ✅ renders; namespaces not modelled |
| `<canvas>` 2D | Charts, games, visualisations | ❌ **not implemented** |
| `<video>` / `<audio>` playback | Media sites | ❌ **no codecs.** Element boxes lay out; nothing plays. Graceful, not crashing. |
| Web fonts (`@font-face`) | Typography-heavy sites | ✅ |
| `display: contents` | Layout-transparent wrappers | ❌ |
| CSS transforms / transitions / animations | Motion, and *layout* when transforms shift boxes | ❌ **`transform` not in computed style** — a real gap |

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
| React committing its render | React | ❌ **still silent.** Mounts, schedules, throws nothing, renders nothing. |
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
| `btoa`/`atob`, `TextEncoder`, `crypto.randomUUID` | Data URLs, JWTs, request ids, React keys | ✅ |
| Event bubbling / capture / `stopPropagation` | All delegation-based UIs | ✅ |
| `fetch` / XHR | Every dynamic page | ✅ |
| `MutationObserver` / `IntersectionObserver` / `ResizeObserver` | Lazy-loading, infinite scroll, sticky headers | ✅ |
| `localStorage` / `sessionStorage` / cookies | Sessions, preferences | ✅ (partitioned; RFC 6265) |
| `history.pushState` (client-side routing) | Every SPA's navigation | ✅ |
| `append`/`prepend`/`before`/`after`/`replaceWith` | Modern DOM mutation — very common | ❌ |
| `insertAdjacentHTML` / `insertAdjacentElement` | Extremely common — every hand-rolled "load more", all of htmx | ✅ (tick 25) |
| `append` `prepend` `before` `after` `replaceWith` `replaceChildren` | The ChildNode/ParentNode mixins — what any script reaches for to place a node *next to* another | ✅ (tick 25) — all eleven were missing |
| `outerHTML` (get + set) · `innerText` · `getAttributeNames` | Ubiquitous | ✅ (tick 25) — `innerText` is honestly approximated as `textContent`; the true definition needs layout |
| `outerHTML`, `innerText` | Common | ❌ |
| `scrollTop`/`scrollLeft` | Scroll containers, virtualised lists | ❌ |
| `getSelection` / `Range` | Editors, copy handling | ❌ |
| `Blob` / `File` / `FileReader` | Uploads, downloads, image preview | ❌ |
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
| **Canvas/WebGL** | Games, maps, editors | ❌ |

---

## The roadmap, in order of web-coverage bought per unit of work

1. **The hangs** (~1 site in 4). Bar 0. Nothing else matters at this ratio.
2. **React's commit.** One framework, but it is *the* framework — and the fix will be another missing
   primitive, on the evidence of the last four ticks.
3. **`append`/`insertAdjacentHTML`/`outerHTML`/`scrollTop`** — cheap, and used by an enormous amount of
   ordinary page JS.
4. **`adoptedStyleSheets` → the cascade.** Turns "web components render unstyled" into "web components
   render". Every design system on the web.
5. **CSS `transform`.** Not in computed style at all — and it *moves boxes*, so it is a layout bug, not
   just a visual one.
6. **`<canvas>` 2D.** Charts and visualisations are everywhere in docs and dashboards.
7. **Media.** Codecs are a large, separate problem. The right first step is *graceful*: a `<video>` that
   lays out, shows a poster, and says it cannot play — rather than one that breaks the page around it.

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
