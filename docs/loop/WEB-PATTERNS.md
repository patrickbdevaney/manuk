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
