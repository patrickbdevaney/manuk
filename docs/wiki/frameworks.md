# FRAMEWORKS — React, Svelte, Next, jQuery: what actually breaks

## Frameworks are debugged by RUNNING them, not by cloning their repos (settled)

The bug is never in React. It is in the platform API React assumes. **Cloning framework source teaches
nothing that running the framework against the engine does not teach faster and more honestly.**

## A missing constructor is a THROWN EXCEPTION, and its blast radius is whatever was rendering

This is the most expensive shape of gap in the project and it keeps recurring:

- `canvas.getContext` was used by **3%** of sites and **broke 100% of them** — `ctx.fillRect(…)` on the
  next line is a `TypeError`, and a charting library that boots on load takes the whole bundle with it.
- **`WebSocket` was missing and took an entire news front page with it.** aljazeera.com's 2,591
  server-rendered elements became **141**: a live-blog client constructed one at boot, React's render
  threw, its error boundary showed a skeleton, and the article was gone.

Fixing that one revealed `Blob`. Fixing `Blob` revealed `FileList`. **Each was a different library's
first line.** A page does not get to run its fallback path **if the check for the fallback throws.**

> **The rule: construct successfully, and answer honestly.** A blank canvas, an unopened socket, an
> empty `Blob` are all survivable — every library on the web is written to survive them, because real
> browsers produce exactly those behind captive portals, in private windows, and with permissions
> denied. **A `ReferenceError` is survivable by nothing.**

## USAGE IS NOT DAMAGE — rank by what happens to the user

`srcset` is used by **34%** of sites and damages **~1%** (the fallback `src` works). `canvas` was used
by **3%** and broke **100%** of them (it threw). *Ranking a backlog by usage frequency is ranking it by
the wrong number.*

## Server-rendered HTML renders with NO JavaScript at all

Hydration only attaches handlers — so Next.js/SvelteKit SSR pages are already *readable* before any JS
runs. **What breaks is interactivity and client-side routing**, not the content. This re-ordered the
whole roadmap: the History API turned out to be pure host state with zero SpiderMonkey dependency.

## `ownerDocument` must be read from the ROOTED global

Our `DOC_REFLECTOR` was an unrooted `*mut JSObject` — a use-after-GC. React got one of its own
MutationRecords back. Read `globalThis.document` instead; it is rooted by construction.

## jQuery is on ~74% of pages

Which makes the **jQuery-core surface** the empirically-justified first tranche of any binding work:
`querySelector`/`getElementById`, `createElement`/`appendChild`/`textContent`/`innerHTML`,
`addEventListener`, XHR/`fetch`, `classList`/`style`.

**jQuery survives a missing `DOMContentLoaded` by checking `document.readyState`** — which is precisely
why the missing document lifecycle went unnoticed for 40+ ticks: *it worked often enough to look fine.*

---
# Backfill — mechanisms recovered from ticks 1–42 (pre-wiki)

## Eight real framework bundles rendered NOTHING and threw ZERO exceptions — the silence WAS the finding

Vue, React, Preact, Svelte, Solid, Lit and vanilla Vite output **all mounted an empty `<div id="root">`
with no error of any kind.**

> **A framework that fails LOUDLY gets fixed. One that fails SILENTLY becomes a permanent, unexplained
> "that site just doesn't work."**

The chain — **each step naming the next** — was **entirely additive substrate, not a missing scheduling
subsystem**:

`import.meta` → **`nodeType`** (React's `isValidContainer` checks it; without it you get React error #299)
→ **`ownerDocument`** (React then indexes the result: `undefined["_reactListening…"]` — *an error naming
neither `ownerDocument` nor the DOM*) → **DOM interface constructors** (`node instanceof HTMLIFrameElement`
**throws** `invalid 'instanceof' operand` when the constructor is undefined) →
`createElementNS`/`createComment`/`createDocumentFragment` →
`performance.now`/`MessageChannel`/`requestIdleCallback`.

**0/8 → 8/8 on roughly ten additive WebIDL fixes and NO new architecture.**

> **No amount of spec-reading would have picked `nodeType` out of the DOM standard as *the* load-bearing
> property.** *This settles the binary question the whole schedule hung on: hydration failure does NOT
> cascade into needing a scheduling architecture.*

## When a framework fails silently, the bug is BELOW the framework

**Four of the five real blockers were in our own primitives**: a use-after-GC in `ownerDocument`, an
unsupported `file://` scheme, a missing `CharacterData.data`, and a shadow root reporting `nodeType 8`
instead of 11. **The framework was never once the thing that was broken** — yet *"React renders nothing"*
and *"Lit's template doesn't commit"* sat in the ledger as **framework** problems for several ticks.

> **The prior is now: test your OWN primitives before blaming the framework. It has paid three times.**

## An empty `catch` around `connectedCallback` silently deletes a whole component library

**Lit does its ENTIRE first render from `connectedCallback`** — that is where `attachShadow` happens and
where the component's content comes into existence. `try { el.connectedCallback(); } catch (e) {}` meant a
Lit component produced **no shadow root, no boxes and no message**, and it cost two ticks of looking in the
wrong place.

> **The general form: every swallowed exception must report its message.** *"A page `<script>` threw;
> continuing"* **is a shrug.** Printing the message turned an hour of bisecting into two exact TypeErrors.
> **The browser was naming its own bugs out loud and the messages were being discarded.**

**Errors were being discarded in THREE distinct places:** empty `catch` blocks, swallowed exception
messages, and **unhandled promise rejections.**

## `attributeChangedCallback` must fire on a LIVE `setAttribute`, not only at upgrade (tick 460)

The upgrade path fired `attributeChangedCallback` for the observed attributes PRESENT at boot, but a later
script mutation — `el.setAttribute('checked','')`, `el.removeAttribute(...)`, `el.toggleAttribute(...)` —
wrote the attribute into the DOM and never told the element. This is the reactive-attribute idiom *every*
design-system web component is built on (`<my-toggle checked>` flipping from script, `aria-expanded` driving
open/closed, Lit's own attribute→property reflection), so a component's rendered state froze at whatever it
was at boot. The MutationObserver feed can't substitute: `record_mutation` fires BEFORE the attribute is
written (and carries no new value), and a custom-element reaction is **synchronous** — the component has
re-rendered by the next line of script, not on a later microtask.

Fixed in `engine/js/src/collections_js.rs`: wrap `setAttribute`/`removeAttribute`/`toggleAttribute` where they
actually live (`Node.prototype` — `Element.prototype` is an empty link, hence `ownerOf`) and, AFTER the native
call writes the attribute, fire `attributeChangedCallback(localName, old, new)` when `this` is an upgraded
custom element (`__ceUpgraded`) observing that attribute (`this.constructor.observedAttributes`). `setAttribute`
reacts on every call (per the CE spec, even to the same value); `removeAttribute`/`toggleAttribute` only on an
actual presence/value change; an unobserved attribute never fires. The `__ceUpgraded` fast-path keeps the two
`getAttribute` reads off every *plain* element's set-attribute hot path (reflectors are identity-cached, so the
upgrade expando is always visible on the element the page holds). Gate `G_CE_ATTR_CHANGED`. Honest scope: the
JS `setAttribute` family is covered; property-reflector attribute writes (`id=`/`className=`) are a separate,
rarely-observed edge left for later.

## The web FEATURE-DETECTS and *grades* the browser — one missing BOM object downgrades whole platforms

MediaWiki's startup script runs
`isCompatible() = 'querySelector' in document && 'localStorage' in window && …` and, on failure, reverts
`client-js` → `client-nojs` and **ships the degraded page**.

Failing on `localStorage` meant **every MediaWiki site on earth was serving the no-script fallback**:
Wikipedia's table of contents never collapsed (**1,949px instead of 364px**) and dragged the whole page
**~5,000px out of alignment.** *It looked like a layout bug for an hour. It was a missing BOM object.*

> **Corollary — the "admissions test" bug class:** the web probes `'localStorage' in window` and friends,
> then falls back to a no-script path — **so the engine looks catastrophically broken when it is merely
> incomplete.** *Honest present-but-inert stubs for admission-test properties are worth shipping BEFORE the
> underlying implementation, purely to close that gap.*

## Constructable stylesheets are a PREREQUISITE for web-component libraries

`new CSSStyleSheet()` + `replaceSync` is how **every** modern web-component library ships styles; Lit's
`static styles = css\`…\`` needs it to **exist** before the component renders a single node. Alongside it,
`document.createTreeWalker` + `NodeFilter` is how lit-html finds the holes in a cloned template, and
`document.importNode` is how it commits one.

⚠ **`adoptedStyleSheets` accepted-but-dropped** means components render **unstyled** — the deliberate
trade-off recorded at the time was *"legible beats absent"*, and it is why design-system sites (banks, gov,
enterprise portals) render **without their styles**.

## SSR'd HTML renders with NO JavaScript at all — what breaks is client-side ROUTING, not rendering

Hydration only attaches handlers. **This reordered the whole roadmap.** The History API turned out to be
**pure host state with zero SpiderMonkey dependency** beyond binding it:

- `pushState` **updates the URL WITHOUT fetching**
- it does **NOT** fire `popstate` — **only traversal does**. *(Firing it on `pushState` would make a router
  RECURSE.)*
- it is **same-origin only**
- **`pushState` creates an entry even for an UNCHANGED URL** (a router pushes the same URL with new state) —
  *a naive session-history `push` that dedupes will silently swallow it.*

## The Framework Exception Miner — an unthrown exception is a discovery signal you threw away

Drive barebones starter templates (CRA, Next App+Pages Router, Vite+React, Vue/SvelteKit, Angular)
headlessly; **the engine must PRINT thrown exceptions rather than discard them**; parse each stack;
auto-generate a ticket for the missing IDL member named in the error; and **rank the backlog by how many of
the N templates each missing item unblocks.**

The same pipeline catches non-standard-API gaps (`Error.captureStackTrace is not a function`) — **an
identical signal shape, so no second mechanism is needed.**

## Prioritise the binding surface from Chrome UseCounter + HTTP Archive, not by instrumenting Chrome

And since **jQuery is on ~74% of pages**, the empirically-justified first tranche is the **jQuery-core
surface**: `querySelector`/`getElementById`, `createElement`/`appendChild`/`textContent`/`innerHTML`,
`addEventListener`, XHR/`fetch`, `classList`/`style`.

## The measured capability list that ranked the whole backlog

`<form>` submit **50%** · `<picture>`/`srcset` **47%** · CSS transition/`@keyframes` **38%** · `<iframe>`
**23%** · `position:sticky` **14%** · WebSocket **5%** · Service Worker **5%** · `<dialog>`/`showModal`
**3%** · Web Worker **2%** · IndexedDB **1%**.

> **A capability that THROWS is strictly worse than one that is MISSING.** *"A missing feature degrades a
> page. A thrown `TypeError` at the top of a bundle kills every line of script after it — so a
> 27%-of-the-web feature that throws is a 27%-of-the-web **outage**, not a 27%-of-the-web **gap**."*

## The first named hydration failure: aljazeera, and React discarding its own server-rendered tree

`remove_child` took out **2,131 elements in one call** — which is **normal**: `createRoot` clears its
container. **The bug was that the client re-render then came up empty**, because a chain of
`ReferenceError`s (WebSocket → Blob → FileList) threw inside React's render and hit an **error boundary**,
leaving **141 of 2,131 elements (5%)**. Fixing the ~40-name interface surface brought it to **470 (3.3×)**.

**Every other measured site — github, HN, bbc, wikipedia, techcrunch, vimeo, deviantart, replit — retained
100%** of its parsed elements through script execution. *So this is one site's class, not a general
hydration collapse.*

## SSR hydration — measured working, and pinned (tick 217)

Next.js, Nuxt, SvelteKit, Remix and Astro all ship **server-rendered HTML plus a client bundle that
attaches to it** rather than building the DOM from scratch. The failure mode is uniquely nasty: when
hydration breaks, the page still *looks* right — the SSR markup is sitting there — and nothing on it
responds. "Every modern site looks perfect and no button works" is the hardest bug report to act on.

**It already works.** Probed tick 217, every primitive a hydrator is built out of:

| Step a real hydrator takes | Result |
|---|---|
| walk SSR tree by `childNodes`/`nodeType`, text nodes included | ✅ 6 elements, 4 text nodes |
| read server attributes back out | ✅ |
| `querySelectorAll` to locate mount points | ✅ |
| compare server output vs client expectation (the mismatch check) | ✅ concludes MATCH |
| **attach a listener to the EXISTING node** | ✅ **fires on a real click** |
| patch a node in place | ✅ |

The text-node row is the one worth calling out: a hydrator that cannot see text nodes cannot match
them, and that is exactly where React's `#418 hydration failed` comes from.

**Do not re-probe this.** It is the sixth feature assumed missing and found built (`localStorage`,
`FormData`, `position: sticky`, `IntersectionObserver`, per-glyph font fallback, hydration) — and it
is consistent with this file's standing finding that **framework failures are bugs in our own
primitives**, not framework internals. Four of five app-web blockers were ours.

Pinned by `G_HYDRATION`, proven RED two ways (alter the SSR tree shape → the walk assertions fail;
never register the listener → the click assertion fails). **Residue:** no real framework bundle runs
here; `Suspense` streaming boundaries, selective hydration, islands, and the mismatch *recovery* path
are unmeasured.

## Hydration works, and only a driven click can prove it (tick 229)

Hydration — Next.js, Nuxt, Remix, SvelteKit, Astro — ships real HTML from the server and has the
client **adopt** it: walk the existing markup, compare it against what it would have rendered, attach
listeners to the nodes already there. Measured in tick 229 and working: markup present pre-script,
node identity preserved across attach, listeners on server markup firing, mismatch detectable and
patchable.

**It is the canonical silent failure, and that shapes how it must be tested.** Every step is ordinary
DOM work, so a broken hydration throws nothing. The page looks *right* — the server's markup is on
screen — and is dead: inert buttons, menus that never open. No error, no blank screen, no missing
API. Rendering a page and inspecting it cannot tell hydrated from un-hydrated, so `g_hydration`'s
decisive assertion lives outside the page script: `Page::dispatch_click` on the server-sent button,
then read the text back. Disabling the JS dispatch yields `Clicked 0 times` — the inert page exactly.

**Node identity is the claim that carries the weight.** Adoption means the *same object*, so the gate
stamps a JS property on the node before attaching and requires it after. A framework that re-created
the node would produce a byte-identical DOM while discarding the server's work and every listener on
it — indistinguishable by inspection, caught by the stamp.

## pagereveal — the MPA activation hook (tick 372)

`__fireLoad` now dispatches `pagereveal` after `load`, carrying `.viewTransition === null` — the
spec's OWN value for the no-transition case (the event fires on every page activation, transition
or not), so entry-animation code hooked on it runs correctly. Not a stub: null is what Chrome
hands the same listener on a plain navigation. `pageswap` landed one tick later (t373): `Page::fire_pageswap` — a dispatch through the existing
`eval_in_page` plumbing, no new PageContext surface — called at the shell's `stash_current`, the
outgoing document's last moment alive before every navigation. Same honest null. The
`@view-transition` MPA animation itself is a subsystem and is not claimed. Claims
pr-/ps-fired/vt-null all ride g_mse_join.

## Promise-returning scrolls + the synchronous scrollY contract (tick 378)

`scrollTo/scroll/scrollBy` return `Promise.resolve()` — truthful because our scrolls are instant.
The tick's real find: the gate falsified the premise first (`scrollp:false`) — under the
request-model, `scrollY` lagged until the host drained, so the awaited continuation read stale
state, and so did any next-line read (a silent divergence carried since the scroll request model
landed). `__scrollTo` now optimistically updates the page-visible position (`scrollTo(0,40);
scrollY===40` holds synchronously, as in a real browser); the host's application overwrites with
the clamped truth, so out-of-range requests over-report only transiently. A future smooth-scroll
threads its settle notification through the same promise.
