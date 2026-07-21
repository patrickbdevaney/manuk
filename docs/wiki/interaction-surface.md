# THE INTERACTION SURFACE — clicks, scroll, focus, and automation

> This topic file is load-bearing for the agent-native mission, so **capture here is expected on every
> commit that touches the interaction/automation surface** — a stricter bar than the rest of the wiki,
> not a competing system.

## The engine paints at scroll 0 and never tells anything the viewport moved

**This is the single biggest breadth-per-tick item on the board**, because it is **one missing
primitive, not six missing features.** A live viewport unlocks, all at once:

- lazy-loading images (`loading=lazy`, and every JS lazy-loader)
- list virtualisation (every large feed, table, and infinite scroll)
- `position: sticky` behaviour under scroll
- scroll-linked animation
- `IntersectionObserver`-driven content (which is *most* modern content-loading)

**Ask before re-entering JS.** The overwhelming majority of pages have **no** scroll listener, no
IntersectionObserver and no ResizeObserver. Re-entering JS on every wheel event for those pages is pure
cost — a rect-map rebuild, a JS call, and a timer pump, sixty times a second, to tell a page that is not
listening. `wants_view_events()` asks first.

## A click is a hit-test against the LAID-OUT boxes, not a DOM query

A gate that scored "clickability" by counting `<a href>` elements scored a browser that found **zero
links** as *perfectly clickable*. The hit-test reads the fragment tree; the DOM query does not.

## `window` must be an EventTarget

`window.dispatchEvent(new Event('resize'))` is how a router tells the app it navigated — and it was a
`TypeError`, with a whole listener registry sitting behind it.

---
# Backfill — mechanisms recovered from ticks 1–42 (pre-wiki)

## A control whose only effect is a LOG LINE is a dead affordance — and to the user it is a broken browser

Find-in-page and the bookmark toggle both **worked underneath** — and **drew no UI at all**, so pressing the
key did nothing visible. **Both shipped.**

**This is a class no feature-level test catches**, because every feature test asks whether the **engine** did
the thing, **not whether the USER could SEE that it did.**

**Two mechanical defences:** assert that every user-reachable control has an **observable** effect (and
**explicitly fail any whose stated effect is a tracing line**), and **make the toolbar's draw and hit-test
share ONE geometry function**, so what a user sees and what responds to a click **can never drift apart.**

## Hit-testing must break ties toward the DEEPER element, and must respect STACKING ORDER

- Resolving an area tie toward the **ancestor** made a click on a lone `<button>` inside a same-size
  `<form>` **"hit" the form.** Pre-order with `<=` resolves to the **deeper** element.
- **Deepest-wins alone lets you click THROUGH a `position:fixed`, high-z modal** to the content beneath it.
  **"Smallest containing box" is not the same as "topmost painted box."** Prefer the **highest effective
  stacking layer** containing the point, **then deepest within that layer** — so non-positioned pages (all
  z=0) are unchanged.

**The geometry problem underneath:** inline elements produce no layout box, so **`<a>` and `<button>` — the
very things one clicks — had NO GEOMETRY AT ALL** until text fragments started carrying their owning node;
and `<a><img></a>` had none until a boxless element's rect became the **union of its subtree's fragments AND
boxes.**

## Agent actions must go through the REAL hit-test, or agent testing is a privileged bypass

An agent action resolves to an a11y-tree node → **spatial hit-test via the primary layout engine** → **if
the target is occluded, `display:none`, or off-screen, the hit-test FAILS and the action is REJECTED,
exactly as it would for a human.**

> **That is precisely what makes agent-driven testing a valid differential oracle for interaction parity.**
> *The cost, stated plainly: the a11y tree must be computed-on-read and kept synchronously consistent with
> layout.*

## ONE a11y tree, THREE consumers — and web-agent research independently converged on it

Building the DOM→role+name+geometry tree **once** serves:

1. **the screen-reader product** (bridged via `accesskit` → Windows UIA / macOS NSAccessibility / Linux
   AT-SPI),
2. **the agent's structured observation channel** — WebArena-class research uses the accessibility tree as
   the **preferred** channel (*"a subset of the DOM with role/text/properties, structured yet more
   compact"*), read via **semantic roles** rather than raw pixels or full DOM,
3. **the interaction-parity oracle.**

**HTML-AAM implicit-role rules that actually bite:** `<a>` is a `link` **only with `href`**; `<section>` is
a `region` **only when named**; `<th scope=row>` → `rowheader`; an explicit `role=` honours
**first-valid-token-wins**; **`<img alt="">` is presentational — the node is dropped but its CHILDREN are
REPARENTED.**

## Role + accessible-name is the right primary locator vocabulary

It is **semantic rather than structural** (it survives CSS and DOM refactors), it matches how an LLM reasons
(*"the **Sign in** button"*), and it is **injection-resistant in a way CSS selectors are not.**

**External a11y-first agents are sunk by legacy UIs that render invisibly to the a11y tree — but because we
COMPUTE the tree internally, we can offer role/name locators synchronously with a geometry/visibility
fallback when a11y metadata is missing.** A structural CSS escape hatch stays available, **but not as the
primary mode.**

## The stable agent handle should be the arena `NodeId` itself

**CDP's nodeId/backendNodeId split is accidental complexity** — it exists *because* it crosses a process
boundary. Ladybird's WebDriver element reference **IS** the DOM node's `UniqueNodeID` stringified, with a
staleness check.

Our arena already has `NodeId`, and `A11yNode.node` already carried it; it simply **was not surfaced as a
handle**, which forced the agent to **re-resolve the whole tree on every call.** The pairing that makes it
safe is the **generational `NodeId`** — a stale handle after slot reuse **fails loudly** rather than
silently aliasing a different element.

> **The in-process advantage IS the absence of a delta-serializer and a dual node-ID space.** Every
> CDP/WebDriver stack needs both *because* it crosses a process boundary. This is also what makes a live
> **`Handoff`** possible: it **MOVES the live `Page`** — DOM mutations, form values, scroll, nav stack —
> instead of re-fetching, *and re-fetching would lose exactly what matters (a logged-in page, a half-filled
> form).*

## Synchronous truthful READINESS is literally inexpressible over CDP/BiDi — that is the differentiation surface

**Readiness is the #1 flakiness source in all web automation.** Playwright's own docs concede network-idle
*"does not work for state updates triggered by client-side events, focus changes, or mount-time effects"* and
**never settles in SPAs holding long-lived websockets**; **BiDi still lacks an event distinguishing
navigation COMMIT from COMPLETE.**

**An in-process engine can block on the REAL lifecycle signal** — style clean, layout clean, microtask queue
drained, a *specific* fetch resolved — **because it owns the arena.** The same ownership enables a **race-free
semantic DOM diff computed BETWEEN JS turns**, which no socket-based protocol can offer.

## WebDriver BiDi, not CDP — and driving a REAL client found five bugs a spec read did not

CDP is **Chromium-only and not a standard**; **WebDriver BiDi is the W3C standard** (JSON-RPC over WebSocket,
WPT-tested), Firefox has dropped experimental CDP in its favour, and it makes the engine
**Puppeteer/Playwright-drivable.**

**Bugs a spec read missed:** `browser.getUserContexts` is required **before** any context work;
`browsingContext.contextCreated` **must be emitted BEFORE the `create` reply** (clients build their map from
the *event* and look the id up the instant the command resolves); context info must be **complete**
(`parent`, `userContext`) **or clients silently drop the event**; `newPage()` issues
`browsingContext.setViewport`; and **`goto` needs the full lifecycle** — `navigationStarted` before the
reply, then `domContentLoaded` + `load` carrying **the same `navigation` id** and a numeric timestamp, **or
`goto` never resolves.**

## The Action-Guard taxonomy follows one principle: PAGE-CHOSEN DESTINATIONS are the exfiltration channel

An action that can transmit page-derived or typed data to **a destination the page chose** is what a hidden
prompt injection reaches for.

| Action | Class | Why |
|---|---|---|
| `submit`, `click_text` on a **button** | **Sensitive** | can submit |
| `click_at` | **Sensitive** | **a raw coordinate cannot be checked against any target before it is clicked** |
| `type` | Safe | purely local — nothing leaves the machine until a submit or navigation |
| `back`/`forward`/`scroll` | Safe | |

**`method=post` forms must be REFUSED, not silently downgraded to GET** — *a downgrade pushes passwords and
tokens into the URL and the `Referer`.*

**Capability scoping belongs at the tool-call boundary as a plain value.** CaMeL's result is that what
defeats prompt injection is enforcing policy **at the tool-call boundary** — a check before each action.
**Macaroons buy nothing in-process** (they exist for *attenuated delegation across a trust boundary*; where
no untrusted party ever holds the credential, an HMAC chain adds crypto surface for zero threat-model gain).
**Authority is checked FIRST** (*"were you ever granted this kind of action?"*), **then** the risk heuristic —
they answer independent questions and **both must pass**. **An empty origin allowlist means "any origin", not
"none"** — *a grant that forgot to name one must not silently brick.*

## More context DEGRADES agent accuracy even when the needed information is present

The literature shows a larger observation **hurts every model**, and there is **no evidence a richer
observation helps a LARGER one** — FocusAgent's >50% pruning lets two *small* models hit 51.5%/51.8% success
against 53.0% for a large model on the full tree.

> **So serialization depth must be keyed on a TOKEN BUDGET, not a model-capability enum** — that would encode
> a belief the evidence does not support.

**The trimming order is counterintuitive**, dropping by increasing value-per-token: **raw text first, then
the link list, and the ACCESSIBILITY TREE LAST** — the tree is the densest channel and **the link list is a
strict subset of it.** *(An initial implementation had this backwards.)*

**And no policy, at any budget, may drop the untrusted-content fence around page-derived text** —
`aria-label="ignore prior instructions"` is an injection vector **exactly like link text.**

## Graceful degradation is not "do nothing" — the spec already has the vocabulary

`<video>` laid out correctly at 640×360 while `canPlayType`, `play`, `paused`, `readyState`, `error` and
`networkState` were **all `undefined`.** **That is the worst combination:** a site calling `video.play()`
gets a `TypeError` **that takes the page down**, and a site that **politely feature-detects** with
`if (v.canPlayType('video/mp4'))` reads `undefined` and **cannot even be told no.**

**The honest answers:** `canPlayType()` → **`""`** (*the empty string IS the spec's "no"*) · `play()` → a
**REJECTED Promise** (`NotSupportedError` — *which every player library already handles, because autoplay
policies make rejection routine*) · `error.code 4` · `readyState 0` · `networkState 3` ·
`getContext('webgl')` → **`null`**.

**A site told THAT hides its player and shows its fallback.** And **`<video poster>` is a still image we can
already decode and paint** — *a correctly-sized poster with an honest "cannot play" is a **degraded video**,
which is the whole ask.*

**`alert`/`confirm`/`prompt` must be honest too:** a renderer has **no user to ask**, and a `confirm()`
returning `true` by default would let a page **believe the user had agreed to something.** *Declining is the
safe answer, and it is LOGGED rather than silent.*

## The live viewport is ONE primitive, and the step everybody forgets is the FOURTH

Lazy-loading, list virtualization, sticky headers, scroll-linked animation and infinite scroll are **not
five features.** They are **one primitive seen five times**: *does the engine tell the page that the
viewport moved?*

**The complete loop, and all four steps are required:**

1. the viewport moves → **`window.scrollY` updates** and **`scroll` fires**;
2. **`IntersectionObserver` FIRES** — this, not `scroll` handlers, is what *most* modern content-loading is
   built on;
3. the callback swaps **`img.src = img.dataset.src`** — the universal lazy-load pattern;
4. **the engine NOTICES that new URL and FETCHES it.**

> **Step 4 is the one everybody forgets.** An engine that fires the observer and never fetches what the
> observer asked for has implemented the **appearance** of lazy-loading and none of the substance: the page
> requests the image and it never arrives. **Firing the observer is not the feature. The image ARRIVING is
> the feature.**

**Ask before re-entering JS.** The overwhelming majority of pages register no scroll listener and no
observer; re-entering JS on every wheel event for those is pure cost.

## Element scrolling — and the zero that broke every virtualised list

`element.scrollTop` was the roadmap's #2 item, and the gap was not absence. It was a **lie on both sides**:

* reading gave `undefined`;
* writing quietly created a plain JavaScript own-property. It scrolled nothing. It threw nothing.

So a virtualised list set it, read it back, got its own value, and believed it had worked.

### The worse bug underneath

`clientWidth`, `clientHeight`, `scrollWidth`, `scrollHeight` all **existed** — every one of them aliased to
`offsetWidth`/`offsetHeight`, i.e. the element's own border box. Which means:

> **`scrollHeight - clientHeight` was always ZERO.**

That is precisely the number every virtualised list divides by to decide which slice of the data to render.
`undefined` fails loudly. **Zero fails as *"there is nothing to scroll"*** — and the list renders one screen
of rows and stops, on a page that looks fine.

It surfaced only because two numbers disagreed: the setter's clamp computed 900 correctly (from the real
geometry) while the getter reported 100 (from the alias). **Two numbers that disagree about the same fact
mean one of them is not reading what it thinks it is.**

### How it works, and why the painter needed no changes

A scroll container's clip is **already** its padding box (that is what `overflow` has always done here). So
translating its subtree up by `scrollTop` slides content out of that clip exactly as a real scroll does, and
anything scrolled out of view is clipped away **for free, because it was always going to be**.

* `LayoutBox::translate(dx, dy)` moves a box and its whole subtree — including the **list marker**, which it
  did not before (a latent bug: a `<ul>` inside a float would have kept its bullets behind while its text
  moved).
* `Page::set_element_scroll` translates by the **delta**, never the absolute offset. The tree already carries
  the old one; translating by the absolute value on every assignment scrolls cumulatively, which looks
  exactly like a runaway-scroll bug.
* `Page::reapply_scroll_offsets()` runs after **every** re-layout. Layout starts from zero each time, so
  without this the user types in a chat box and the list jumps back to the top.

### The contract with JavaScript

The host owns the layout tree, so the host owns the numbers. `Page` publishes
`[scrollTop, scrollLeft, scrollHeight, scrollWidth, clientHeight, clientWidth]` per container before **every**
script round — including the *inline* one, which runs before a `Page` even exists, because a virtualised list
reads `clientHeight` at boot and a capability that only works after the deferred pass works on half the web.

The setter **clamps in the native**, so `el.scrollTop = 1e9; el.scrollTop` reads back the real maximum on the
very next line. A script that scrolls to a huge number to reach the bottom is idiomatic; reading back `1e9`
makes every `atBottom` check false forever.

Non-scroll-containers fall back to their own box for `clientHeight`/`scrollHeight` — a plain `<div>` still has
a `clientHeight`, and returning zero for every ordinary element would be a far bigger regression than the bug
being fixed.

## `document.elementFromPoint(x, y)` bridges the layout-rect snapshot, not a second hit-tester

A genuinely missing DOM API (`css-transforms` alone: 84 `is not a function` failures; also drag-and-drop,
tooltips, custom controls). Implemented over the binding's existing `LAYOUT_RECTS_PTR` snapshot: among
laid-out **element** boxes containing the client point, return the **deepest** — smallest border-box area,
later document order on a tie (children paint over their parents). A miss, or a non-finite/absent
coordinate, returns `null` (CSSOM-View). Registered on both document setups; wrapped via
`return_node_or_null` (the same reflector path as `querySelector`).

**Honest bounds, in the code and stated up front:** the rects are **pre-transform**, so a `transform`ed
hit area isn't yet accounted for, and scroll offset is assumed zero (client ≈ layout coords for an
unscrolled page). Even so it moved css-transforms 20 → 45 (+25) — the tests whose coords fall in the
untransformed box. Transform-aware hit-testing (apply the matrix to the box → point-in-quad) is the
follow-on for the remaining transformed cases. It reuses the SAME geometry the a11y `hit_test` uses, so
the agent surface and page JS agree on what is at a point. [[dom-semantics]]

## `element.getClientRects()` reuses the layout snapshot, like getBoundingClientRect

A missing CSSOM-View API. Returns a DOMRectList of the element's border boxes from the same `LAYOUT_RECTS`
snapshot `getBoundingClientRect` reads: a laid-out element → one rect (its bounding box); a `display:none`
/ unlaid-out element → an **empty** list (NOT a zero rect — that is the distinction from
`getBoundingClientRect`, which returns all-zeros). Provides `.item(i)` + indexed access. **Honest bound:**
an inline box that wraps across lines has several client rects; we return the single bounding box (the
block/replaced majority the snapshot holds). Ratchet-neutral at introduction (the lone WPT reference sits
in a multi-assertion test that fails elsewhere too) — landed as correct capability real sites call
constantly, tick-97-style. [[dom-semantics]]

## `offsetLeft`/`offsetTop` are offsetParent-relative, and `offsetParent` exists (tick 138)

**The single largest CSS-layout lever, and it was not a layout bug at all — it was a coordinate space.**
`offsetLeft`/`offsetTop` returned the element's **absolute page X/Y** (`LAYOUT_RECTS[node]` directly). But
CSSOM-View defines them relative to the **offsetParent's padding edge** — "where is this box inside its
positioned container". Absolute coords are only correct when the offsetParent sits at the page origin,
which is the exception, not the rule. So a flex/grid item inside any `position:relative` container reported
its viewport coordinate, and `check-layout-th.js` — which asserts `el.offsetLeft` against a
**container-relative** `data-offset-x` — failed across the whole layout suite. It also meant every popup /
tooltip / drag library that positions at `el.offsetLeft` landed in the wrong place.

**`offsetParent` (`offset_parent()` in `dom_bindings.rs`)** follows CSSOM-View exactly: `null` for the root
element, the body, a `position:fixed` box, or a boxless element (step 1); otherwise the nearest ancestor
that is **positioned**, is the **body**, or — only when the element itself is `static` — a `td`/`th`/`table`
(step 2). Tag detection leans on the arena storing HTML tag names lowercased.

**The offset value (`el_offset_pos()`):** the body/boxless → `0`; **no** offsetParent → the border-edge
coordinate relative to the ICB, i.e. the old absolute value (correct for that case); **with** an
offsetParent → `self.borderEdge − (offsetParent.borderBoxEdge + offsetParent.borderWidth)`, i.e. subtract
the offsetParent's **padding-edge origin**. Rounded to a `long` last (CSSOM rounds; `check-layout` tolerates
±1px). The gate `g_offset_parent` pins both facts with an abspos item in a bordered relative container
(`offsetLeft==10`, not the absolute `45`) — proving offsetParent-relativity AND the border subtraction in
one number. **MEASURED:** css-flexbox 6.2%→24.7% (+665), css-grid 5.3%→9.0% (+107), css-sizing 12.0%→13.6%,
css-position +5; Bar 0 clean; html/dom/dom unregressed. All four suites share `check-layout-th.js`, so one
coordinate-space fix flipped them together. [[dom-semantics]] [[css-cascade]]

## `IntersectionObserver.rootMargin` is a 4-side shorthand, and the BOTTOM side is the whole feature (tick 140)

`rootMargin` grows the observer's root rectangle so a sentinel fires *before* it is actually on screen —
the mechanism every infinite feed uses to load its next page early. It is a **CSS margin shorthand**
(`all` | `V H` | `T H B` | `T R B L`), px or `%`, and the sides are **asymmetric**: the near-universal
idiom `rootMargin: '0px 0px 300px 0px'` extends only the **bottom** edge. The old parse took a single token
(`.split(/\s+/)[0]`) and applied it symmetrically — so that idiom resolved to `0`, the bottom margin was
silently dropped, and the feed loaded **late or never** (the sentinel had to be fully visible before
`observe`'s callback saw `isIntersecting`). This is a **stub-shaped** failure: the API is present, the
option is accepted, and it just quietly does nothing — the library feature-detects fine and never fires.

**Fix (`dom_bindings.rs`, `g.IntersectionObserver`):** parse `rootMargin` into `{top,right,bottom,left}`,
each `{v, pct}`, with the standard shorthand fallbacks (`right←top`, `bottom←top`, `left←right`). In
`__runObservers`, resolve top/bottom per-side (a `%` is a fraction of the viewport **height**) and grow the
intersection band asymmetrically: `min(b, bottom+marginBottom) − max(t, top−marginTop)`.

**Tick 141 made the intersection 2-D** (the follow-on tick 140 named). The old test was vertical-overlap
only, so an element vertically in view but scrolled off to the **side** of a horizontal carousel reported
`isIntersecting=true` and every off-screen slide eager-loaded. Now `visX = min(right, vw+mRight) −
max(left, 0−mLeft)` runs alongside the vertical band (`%` on left/right is a fraction of viewport **width**),
`isIntersecting = visX>0 && visY>0`, and `intersectionRatio = visX·visY / (w·h)`. This is also the only
consumer of the `left`/`right` margins tick 140 parsed. The page is assumed **not** horizontally scrolled
(root x-band `[0, vw]`), which is ~all real layouts. Gate scenario 21c: an element at x=800 in a 400px
viewport must NOT intersect (`hplain:false`); a `'0px 500px 0px 0px'` right margin that reaches it must
(`hright:true`). Proven RED on the vertical-only code (`hplain:true`).

**Gate** (`js_conformance` scenario 21b): a sentinel 20px **below** a 600px viewport (top=620). A plain
`rootMargin:'0px'` observer must report it **not** intersecting; a `'0px 0px 200px 0px'` observer must
report it **intersecting with no scroll at all**. Proven RED on the old parse (`prefetch:false`), GREEN
after. No local WPT `intersection-observer/` suite exists, so this capability is pinned by the falsifiable
conformance gate, not a subtest count. [[dom-semantics]]

## `getComputedStyle` resolves the flexbox longhands, not just the box model (tick 142)

**The gap.** `computed_style_js` (`dom_bindings.rs`) surfaced the box model (width/margins/padding/inset),
colors, fonts, `display`/`position`/`transform` — but **no flex longhand**. So
`getComputedStyle(el).alignItems`, `.justifyContent`, `.flexDirection`, `.flexWrap`, `.flexGrow`,
`.flexShrink`, `.flexBasis`, `.alignSelf`, `.rowGap`/`.columnGap` all read back `undefined`. A framework
that measures a flex container, a CSS-in-JS lib re-reading resolved values, or an animation lib
interpolating `flex-grow` got `undefined` concatenated into its logic. `ComputedStyle` **already stored
every one of these fields** — they were computed at cascade time and simply never surfaced to JS. Pure
wiring, not new layout.

**Fix.** Serialize each stored field to its CSS resolved value — the exact keyword Chrome returns
(`flex-start`/`space-between`/`nowrap`/`column-reverse`/…), `flex-grow`/`flex-shrink` as bare numbers,
`flex-basis` via `dim_css`, `align-self: None → "auto"` — add the ten camelCase keys to the object literal,
and register the kebab→camel names in `getPropertyValue`'s map (so `getPropertyValue('flex-direction')`
reaches the same value). `align-content` and `order` are **not** on `ComputedStyle`, so they stay
unserialized rather than guessing a wrong default.

**Strictly non-regressing:** nothing read these keys before (they were `undefined`), so the change can only
flip failing reads, never break a green one. **Measured:** `css/css-flexbox` 888→945 (+57;
getcomputedstyle 2/78→59/78), `css/css-grid` 150→257 (+107) — grid's getcomputedstyle files read the *same*
`justify-content`/`align-items` resolved values, so one serialization fix flipped both suites (~+164).

**Gate** (`js_conformance` scenario 23): a `flex-direction:column;flex-wrap:wrap;justify-content:space-between;
align-items:center` container with a `flex-grow:2;flex-shrink:0;flex-basis:100px;align-self:flex-end` child
must read back `column|wrap|space-between|center|2|0|100px|flex-end|column` (the last via `getPropertyValue`).
Proven RED by stashing the serialization — the whole join was `undefined|…`. [[dom-semantics]]

## `getComputedStyle` exposes the box-model longhands too (tick 143)

Extending tick 142's pattern: `boxSizing`, `minWidth`/`maxWidth`/`minHeight`/`maxHeight` also read back
`undefined` before. `box-sizing` is the single most-read layout flag in framework measurement code (*is this
a border-box, so does my width math include padding?*). All four are stored+computed on `ComputedStyle`;
this is pure serialization wiring. **The subtle rule:** `max-*` uses `Dim::Auto` for *unconstrained*, whose
resolved value is **`none`**, not `auto` (only `min-*` → `auto`) — a `max_dim` helper maps `Auto → "none"`.
Non-regressing (nothing read these before). Measured +4 (`css-flexbox`); the bulk of box-model gCS tests are
the `css/cssom` battery, absent from the local corpus, so the capability is pinned by `js_conformance`
scenario 24 (`box-sizing:border-box;min-width:50px;max-width:300px;min-height:10px` →
`border-box|50px|300px|10px|none|border-box`), proven RED by stashing the fix. [[dom-semantics]]

## Typing must fire an `input` event, or every controlled component reverts the keystroke (tick 175)

A framework text field is a **controlled component**: `<input value={state} onChange={e =>
setState(e.target.value)}>`. The value the user sees is JS state, and the field learns a key was
pressed **only from the `input` event** — React's `onChange` (which is really the `input` event), Vue's
`v-model`, Svelte's `bind:value` all update their state there, then re-render and write the state back
into the field. So if a keystroke does not fire `input`, the sequence is: user types → the DOM `value`
changes → but state does not → the component re-renders from its *stale* state → and **overwrites the
field with the old value**. The keystroke is visibly reverted. Every controlled input in every SPA is
unusable.

The shell's `edit_focused_input` did exactly the wrong thing: it mutated the `value` attribute
directly (`dom_mut().set_attr(node, "value", …)`) and fired **nothing**. (There was a
`Page::dispatch_type` that fired `input`+`change`, but it had **zero callers** — a mechanism wired to
nothing, the recurring failure this project keeps catching. See [[architecture]] "the mechanism EXISTED
and was wired to nothing".)

**The fix is a focused `Page::dispatch_input(node, value)`** — set the value, then fire **`input`**
(and only `input`) — which the shell now calls per keystroke. It is deliberately not
`dispatch_type`'s `input`+`change`: `change` is a **commit** event (blur / Enter), and firing it on
every keystroke is wrong — a handler that validates or submits on `change` would run on every
character. The click path already worked this way (`dispatch_click` fires the real `click`); this is
the same contract for the keyboard. Gated by `js_conformance` scenario 27: an `input` listener mirrors
`event.target.value`, two keystrokes update it to `hi` then `hip`, and the `change` counter stays `0`.
Residue: `change`-on-blur (when the field loses focus) and `keydown`/`keyup`/`beforeinput` are still
unfired — separate keyboard-event mechanisms; `input` is the one the controlled-component contract
turns on.

## Blur fires `change` then `blur` — field-level validation runs on commit (tick 176)

The commit half of the input/change pair (tick 175 was the keystroke half). A form validates a field
the moment you *leave* it — "email invalid", "username taken", the red border — and it hangs that on
the `change` and `blur` events. The shell cleared `focused_input` on click-away / Escape / submit and
fired **nothing**, so on-blur validation never ran and the field never committed.

**`Page::dispatch_blur(node, value_changed)`** fires `change` (only when `value_changed`) then `blur`.
The `value_changed` guard is not optional: `change` fires *only if the value differs from when the
field gained focus* — a user who tabs through a field without editing must not trigger its
change-validator. The shell tracks that with a `focus_value` snapshot taken in `focus_input(node)`
(and on programmatic `.focus()`); `blur_focused_input()` compares the current value against it, fires
`dispatch_blur` with the result, and is now the single chokepoint every user-initiated focus loss goes
through — click-away (`PageAction::Link`/`Submit`/`Clear`), focusing a *different* field
(`focus_input` blurs the old first), Escape, and Enter (`submit_focused_form` commits before it
submits, so on-blur validation runs before the POST).

Gated by `js_conformance` scenario 28: a blur with no change fires `blur` only; after an edit, blur
fires `change` then `blur` (order matters). Residue: a **programmatic** focus move (page calls
`.focus()` on another field) records the new `focus_value` but does not yet fire `blur` on the old
field; `focus`/`focusin`/`focusout` and `keydown`/`keyup` are separate mechanisms. [[dom-semantics]]

## `keydown` fires with the real `key`, and `preventDefault()` suppresses the default (tick 178)

The keyboard's `click`: a page intercepts a key by listening for `keydown` and calling
`preventDefault()`. The canonical case is the chat/comment composer — `onKeyDown` catches **Enter**,
calls `preventDefault()`, and sends the message itself instead of letting the browser submit the form
(Shift+Enter inserts a newline). A combobox/listbox swallows **ArrowDown** to move its highlight. None
of this worked: the shell went straight from a keypress to its own default action (submit / edit /
blur) and dispatched no `keydown`, so the page never saw the key and could never pre-empt it.

**`Page::dispatch_key(node, "keydown", key)` fires a real keyboard event and returns whether the
default should proceed** (`false` = a handler called `preventDefault`). The event carries `key` (the
modern property — `"Enter"`, `"a"`, `"ArrowDown"`) *and* `keyCode`/`which` (the legacy pair handlers
still read — Enter is 13, a letter is its uppercase code), because `__dispatchEvent` already accepts an
event **object** and preserves its fields (see "its key... all have to survive"). The shell now fires
`keydown` on the focused field before acting, and if the page prevents the default it stops — the Enter
does not submit, the character is not inserted. Composes with the input/change/blur trilogy: `keydown`
(pre-empt) → the default (which fires `input`) → `change`/`blur` on commit.

Gated by `js_conformance` scenario 30: a handler reads `event.key`/`event.keyCode` (`a:65`), and
`preventDefault()` on Enter makes `dispatch_key` return false (`Enter:13`). Residue: `keyup` is not yet
fired (the shell fires `keydown` only — the pre-empt-the-default half is what the value is); `event.code`
(physical key) equals `key` for named keys but approximates for characters; keys the shell does not
surface (function keys, IME composition) dispatch nothing. [[dom-semantics]]

## `navigator.clipboard.writeText` — the "copy" button actually copies (tick 179)

Copy-to-clipboard is one of the most common single-purpose buttons on the web: the code-block copy
icon, "copy share link", "copy API key", "copy to clipboard" on a coupon. They all call
`navigator.clipboard.writeText(text)`. `navigator.clipboard` was **absent**, so that call threw on
`undefined` inside the click handler and the button silently did nothing — a dead affordance, the
exact §1.8 failure the project treats as "broken to the user."

The shell already owns a real OS clipboard (`arboard`, wired to Ctrl+C/V). The fix bridges the page to
it with the host-queue pattern used by `window.open`/`postMessage`: a native `__clipboardWrite(text)`
pushes onto a process-thread-local queue; `navigator.clipboard.writeText` calls it and returns the
spec's resolved `Promise<void>`; the shell drains the queue after a click dispatch (`pump_clipboard`,
beside `handle_window_opens`) and writes the last value to the OS clipboard. `readText` resolves with
the last text this page wrote — a genuine within-page round-trip — but does **not** read the OS
clipboard (reading what another app copied is a permission-gated capability, a follow-on; pretending
to would be a lie, not a feature).

Gated by `js_conformance` scenario 31: a copy button whose click calls `writeText('copied-value-42')`;
after `dispatch_click`, `take_clipboard_writes()` returns exactly that text (and nothing before the
click). Residue: the legacy `document.execCommand('copy')` path is not wired; a write triggered off
the click path (a timer, a fetch reaction) is not yet pumped. [[js-engine]]

## `navigator.clipboard.read`/`readText` — PASTE reads the real OS clipboard (tick 287)

The copy half above worked; PASTE did not. `readText()` returned only the text this page had itself
written, so a "paste from clipboard" button, a rich-text editor, or an AI-chat screenshot drop zone
that reads `navigator.clipboard.readText()` came back **empty** whenever the user had copied something
in another application — which is the entire point of paste.

The fix adds the READ direction of the clipboard bridge, symmetric to the write queue. A `HOST_CLIPBOARD`
process-thread-local holds the real OS-clipboard text; the host seeds it via
`manuk_js::set_host_clipboard(text)` (from `arboard`, the same OS clipboard the write side drains to),
and a native `__clipboardRead()` returns it. `readText()` pulls from that bridge, falling back to the
page's own last write; `clipboard_write` also updates the cell, so a same-page copy→paste round-trips
one clipboard. `read()` returns real `ClipboardItem`s — `.types` plus `.getType(mime)` → `Blob` — so a
paste handler that branches on `image/png` vs `text/plain` finds only the type actually present.

### The teeth `G_CLIPBOARD_READ` uses

`external` — `readText()` must resolve to host-seeded text the page never wrote (an echo of the page's
own write fails). `absent-rejects` — `getType('image/png')` on a text-only item must **reject**, because
a `ClipboardItem` is keyed by the types it holds (a shim that resolves every type fails). `roundtrip` —
`writeText(x)` then `readText()` returns `x`. The `external` claim was demonstrated red (reverting
`readText` to the self-echo) before the tick landed.

**Honest limit:** `text/plain` only. Binary image blobs on the OS clipboard (paste-a-screenshot) need a
binary bridge and are the follow-on; the constellation row is `partial`, not `works`. [[js-engine]]

## `keyup` fires on key release — the settled-value half of the keyboard trio (tick 180)

A large slice of the web — jQuery-era search boxes, character counters, keyboard-shortcut-release
logic — binds `keyup`, **not** `keydown`, because it wants the field's *settled* value after the
keystroke has applied. The shell fired `keydown` (tick 178) and `input` (tick 175) on key PRESS but
processed only `ElementState::Pressed`, dropping every `ElementState::Released` — so a `keyup`
listener never ran and those boxes stayed dead.

`Page::dispatch_key` was already generic over the event type (`"keydown"`/`"keyup"`), so the fix is
purely shell wiring: on key release, `dispatch_keyup` fires `keyup` on the focused field via the same
`key_name_for_dispatch` mapping keydown uses. No default action is associated with `keyup`, so
`dispatch_key`'s "should the default proceed" return is irrelevant and ignored. Completes the trio:
`keydown` (pre-empt) → `input` (per-keystroke) → `keyup` (release). Modifier-only releases
(Ctrl/Shift/Alt) surface no key name, so no spurious `keyup` fires; no focused field → nothing fires.

Gated by `js_conformance` scenario 32: a `keyup` handler reads `event.key`/`event.keyCode`;
`dispatch_key(node,"keyup","x",…)` fires it and the handler records `x:88` (nothing before release).
Residue: `keyup` fires only for a focused text field (not globally on the document), and `event.code`
inherits the same key-name approximation as `keydown`. [[dom-semantics]]

## A11y node STATES — the agent can confirm its own action (tick 199)

`A11yNode` carried `role`, `name`, `bbox`, `z` — and nothing about state. So the observation an agent
read was:

```text
checkbox "Remember me"      <- before the click
checkbox "Remember me"      <- after the click
```

Byte-identical. **An agent that cannot observe the result of its own action cannot verify it**: it
proceeds on faith, or re-clicks and toggles the setting straight back off. This is the agentic moat,
not an accessibility nicety — which is why the gate asserts the *difference between two snapshots*
rather than the presence of a field.

**`A11yState`** hangs off every node: `checked` (tri-state), `expanded`, `selected`, `disabled`,
`required`, `readonly`, `focused`, `value`.

**`Option` means NOT APPLICABLE, not false.** A link is not "unchecked" — it has no checkedness, and
reporting `checked: false` on it would be a lie an agent could act on. Only controls that can carry a
state report one.

**Checkedness is tri-state.** `mixed` is the real third value a "select all" parent checkbox shows;
flattening it to `false` tells an agent the opposite of what the page means.

**ARIA wins over the native attribute** where both are present — the cascade assistive tech uses. An
author who wrote `aria-checked="mixed"` on a checkbox means it, and the native attribute cannot
express `mixed` at all.

**Script-driven state is visible** because `el.checked = true` writes the `checked` *attribute*
through the reflector, so reading the attribute sees script state as well as authored state. That is
what makes "click, then read back" work at all.

**Rendering is signal, not noise**: `A11yState::render()` returns an empty string when there is no
state, so a static document's observation lines are unchanged; a control appends ` [checked disabled
value="ada"]`.

**Focus is host-owned.** The shell tracks the focused element and publishes it into the JS world via
`set_view_state`, so it cannot be read back out of the DOM. `build_tree_with_focus` /
`Page::a11y_tree_with_focus` take it from a caller that knows; the plain builders leave `focused`
false rather than guessing.

**Adding a field to a shared struct is a workspace-wide edit** — `A11yNode` literals live in
`agent/src/{targeting,grounding,automation}.rs` as well as the a11y crate's own tests. Grep every
crate for the constructor, not just the defining one.

Residue: `disabled` does not yet inherit from an ancestor `<fieldset disabled>` or `aria-disabled`
container; no `aria-valuemin`/`valuemax`/`valuetext`, `aria-invalid`, `aria-busy`, `aria-pressed`,
`aria-current`, or `aria-level`; `A11yDiff` still diffs on `(role, name)` only, so a pure state change
shows up in `to_observation_lines()` but not in `diff()`. **The larger gap this exposes:
`element.click()` fires the event but does not run activation behaviour** (a click does not itself
toggle a checkbox — see `el_click`), so the read-back confirms script-driven and authored state today;
native activation is its own tick.

## Click ACTIVATION behaviour — the checkbox actually ticks (tick 208)

`dispatch_click` fired the *event* and stopped there. It ran no **activation behaviour**, so clicking
a checkbox left it unchecked, clicking a radio selected nothing, and no `input`/`change` ever fired.
Tick 199 gave the agent the ability to read control state back and flagged this as the thing that
made the read-back only half useful: an agent could see a checkbox was unchecked, click it, and see
it still unchecked.

**The ordering is the subtle half, and getting it backwards still passes a naive test.** The toggle
happens **before** the click event is dispatched, which is why a real handler reading `this.checked`
sees the NEW state. Toggling afterwards would end in the same final state while handing every handler
on the web a stale value — so the gate asserts what the handler *saw*, not just where things landed.

- **checkbox** — toggles. `preventDefault()` on the click undoes it (the "canceled activation
  steps"), which is what a page validating before allowing a toggle depends on.
- **radio** — is a **group, not a toggle**. Clicking one deselects its peers, grouped by `name`
  (which is how the form serialises); a radio never unchecks itself; a different `name` group is
  untouched. Two checked radios in one group means the form submits the wrong value.
- **`input` then `change`**, in that order, both after the state is committed — every
  controlled-component binding is written for exactly that.

Gated by `g_click_activation`: the box ticks and unticks; the handler log reads
`click:true input:true change:true click:false input:false change:false` (the ordering claim);
`preventDefault()` leaves the box unticked; selecting a radio deselects its group peer and not the
other group; and an already-selected radio stays selected. Proven RED by returning no activation —
the box never ticks and the log's `click:true` collapses.

Residue: only checkbox and radio activate. A link still does not navigate and a submit button does
not submit **from `element.click()`** (the native GUI paths handle those separately, so this is a
gap in the scripted/agent path specifically); `<select>`/`<option>` selection, and the
`labels`→control forwarding (clicking a `<label>` should activate its control) are not done either.

## `<label>` forwards its click to the control (tick 209)

Clicking a `<label>` did nothing. That is the label being *how most checkboxes on the web are
actually clicked* — the visible target is the text, not the 12px box.

**For an agent it is worse than for a person.** The label is what carries the accessible name, so
"click the Remember me checkbox" resolves to the label, clicks it, and nothing happens — and a click
that does nothing is indistinguishable from a control that does nothing.

Both association forms are handled: `for="id"` (resolved to a **labelable** element — `input`,
`select`, `textarea`, `button`, `meter`), and a label **wrapping** its control (first labelable
descendant). A `for` naming nothing labelable labels nothing and does *not* fall back to a
descendant, because the author said which control they meant.

**The recursion trap.** A control nested inside its own label is the common markup, and forwarding
naively means the control's own click forwards back through the label forever — or double-toggles and
so appears to do nothing at all. Forwarding only happens when the clicked node *is* the label, which
is what stops it.

**The label's own click still fires and can still be cancelled.** `preventDefault()` on the label
stops the control being activated, exactly as it does on the control itself.

Gated by `g_label_click`: a `for=` label ticks and unticks its box; a wrapping label forwards to its
descendant; clicking the control *inside* its own label toggles exactly once; a cancelled label click
does not reach the control; a label pointing at nothing activates nothing and does not panic. Proven
RED by not forwarding — the box never ticks.

## A disabled control is inert — and a script-free form still works (tick 210)

Two things, found together because the gate for the first exposed the second.

**A disabled control does not activate.** Ticks 208/209 ran activation without checking, so clicking
a disabled checkbox ticked it, and so did clicking its label. A disabled control is not "styled grey"
— it is inert, and clicking it must leave the page exactly as it was.

**For an agent this is worse than cosmetic**, which is why it earns its own gate: the agent ticks a
disabled consent box, reads the state back (tick 199 gave it that), sees it ticked, and reports
success on a form the server will reject. **A wrong observation is more expensive than a failed
action, because nothing downstream questions it.** So the a11y tree was fixed in the same tick:
`disabled` now inherits from an ancestor `<fieldset disabled>` there too, and the gate asserts the
tree and the activation path **agree** — a tree that said "actionable" about something inert would
be the same failure one layer up.

**`<fieldset disabled>` inheritance is not an edge case.** Disabling a whole step of a multi-step
form with one fieldset is the idiomatic way to do it; checking only the control's own attribute
leaves every control in that step live. Only a `<fieldset>` propagates disabledness — a disabled
`<div>` means nothing.

**The second finding: activation was gated on having a JS context.** `dispatch_click` returned early
when `self.js` was `None`, so **a static form with no `<script>` had inert checkboxes** — they tick
in every real browser. Event *dispatch* needs JS; the toggle does not, and the two are now separate.
With no JS there is nothing to call `preventDefault()`, so activation always proceeds.

This surfaced only because the gate deliberately included a *control that must still work*
(`#live`, in an enabled fieldset) alongside the ones that must not. Without that positive case, an
implementation that made everything inert would have passed every other assertion.

Gated by `g_disabled_inert`: a disabled checkbox does not tick, directly or via its label; a control
inside `<fieldset disabled>` does not tick, directly or via its label; a control in a normal fieldset
still does; and exactly two nodes report `disabled` in the a11y tree. Proven RED by skipping the
disabled check — the disabled box ticks.

## Clicking "Sign in" submits the form (tick 211)

`element.click()` on a submit button fired a click event and stopped — nothing was queued, so the
form never submitted. **"Click Sign in" is the single most common thing an agent is asked to do**, and
it silently did nothing: the agent clicks, sees no navigation, and cannot distinguish "the button is
broken" from "we never submitted".

A submit-button click now pushes its form onto `Page::pending_submits`, which
`take_form_submits()` drains into the **`requested`** list the shell already services.

**`requested`, not `direct`, and that is the load-bearing choice.** `requested` fires the `submit`
event first, so the page's validation handler runs and can cancel — and a click-to-submit is exactly
the case pages validate. Queueing it as `direct` would skip every client-side validator on the web.

Details that decide whether real pages work:

- **A bare `<button>` inside a form defaults to `type=submit`.** That default is the classic source
  of "why did my page reload", and not honouring it means `Sign in` does nothing.
- **`type=button` and `type=reset` do not submit** — every toggle and menu built from a `<button>`
  would otherwise reload the page.
- **`form="id"` associates a button with a form it is not inside**, and wins over the ancestor.
- A **disabled** submit button submits nothing (tick 210's rule, applied here).
- The queue is a **drain**, so the host cannot submit the same form twice.

Gated by `g_submit_click`, covering each of those. Proven RED by not queueing — the form never
submits.

Residue: `formaction`/`formmethod`/`formnovalidate` on the button are not carried to the submission,
and the submitter is not recorded (so a form with two submit buttons cannot tell which one was used —
`<button name=action value=delete>` is a real pattern). Link navigation from `element.click()` is
still not wired.

## The submitter reaches the server — "Save" vs "Delete" (tick 212)

A submit button contributes its `name=value` to the submission **only when it is the control that
activated the form**, which is why the field walk skips every button. `agent/src/forms.rs` said so in
a comment: *"Buttons only submit their own value when they are the activating control; we do not
model that, so they are skipped."* This models it.

**The failure it closes is a silent wrong-action bug, not a missing field.**
`<button name="action" value="delete">` beside `<button name="action" value="save">` is how a great
many forms say what the user asked for. Without the submitter both buttons post a **byte-identical
body** — the server cannot tell the destructive action from the safe one, and an agent driving the
page has no way to detect it.

Threaded end to end: `Page::pending_submits` records `(form, submitter)` on click →
`take_form_submits()` yields `Vec<(NodeId, Option<NodeId>)>` → `gui.rs::navigate_form_with` →
`forms::urlencoded_submission_with_submitter` → `fields_with_submitter`.

- **`None` is the honest answer for a script's `requestSubmit()`** — it has no submitter unless one
  is passed (which is not modelled yet), so nothing is guessed.
- **The submitter goes LAST** in the entry list, matching the order a browser builds it.
- **A button with no `name` is not a successful control** and contributes nothing — its `value` must
  not be smuggled in under another key.
- A button that was **not** clicked still never appears.

Gated in `agent/src/forms.rs` (`the_clicked_button_contributes_its_name_and_value`): Save and Delete
must produce **different** bodies; the nameless button contributes nothing; and the submitter reaches
the **POST body**, not just the field list — the wire is what the server reads. Proven RED by
ignoring the submitter — Save and Delete collapse to the same body.

Residue: `formaction`/`formmethod`/`formnovalidate` on the button still do not override the form's,
and `requestSubmit(submitter)` does not carry its argument.

## Cross-window messaging: `e.origin` is the security boundary (tick 231)

A receiver of a `postMessage` has **no other way** to learn who sent it. Every popup-login SDK — Google
Identity Services, Stripe Checkout, Auth0 `loginWithPopup` — guards with
`if (e.origin !== PROVIDER) return;`. So the value in that slot is not a reporting detail, it is the
whole boundary.

Until tick 231 it carried the sender's own **`targetOrigin` argument**, which is caller-supplied. Any
page that could reach the window could therefore forge its identity by writing
`postMessage(payload, PROVIDER)` — the guard compares the attacker's string against itself and
passes. `e.origin` is now the sender's document origin.

`targetOrigin` is a delivery **restriction** (deliver only if the receiver's origin matches) and is
still not enforced. It never was; it was only misreported. Recorded as residue rather than dropped
silently.

## Window identity must be seeded BEFORE a document's scripts run

`Page::set_identity` operates on a finished `PageContext` — i.e. after `load_document` has already
executed every render-blocking script. That is too late for the case it exists to serve: a popup's
login script reads `window.opener` **at load time** to post its token back. With late seeding it
reads `null`, posts nothing, and the opener spins forever with nothing thrown.

The shell had a comment asserting identity resolved "before any load-time script posts a message"
while calling `set_identity` after the build — intent and ordering disagreed, which is the kind of
gap only an end-to-end gate finds. Use `Page::load_with_identity` (or `manuk_page::set_pending_identity`
before constructing the page); the identity is applied during prelude install, ahead of any script.

## Two live pages in one process

`g_oauth_popup` is the first gate to drive two `Page`s at once and route messages between them the way
`gui.rs` does. The shell has always held many pages; nothing had proven they could talk.

## Clicking into a frame: by point, not by node (tick 233)

The host hit-tests the parent document and gets the `<iframe>` **element**. Dispatching a click on
that fires a click on the frame *box* — not on whatever the user pressed inside it. So a frame could
render and re-render correctly (tick 232) and still be impossible to operate, which is the whole
point of the content the web puts in frames: 3-D Secure "approve", embedded OAuth "allow", a payment
form's "pay".

`Page::dispatch_click_at(doc_x, doc_y, …)` clicks by **point**. When the point lands on a frame with a
live document, it translates into the child's coordinate space and hit-tests there — at the
**frame's** width, because that is the width the child laid out at; hit-testing at the window's width
tests against a layout the child never had. It recurses for nested frames.

### A dirty-bit guard cannot see a child's own script round

This is the subtle half. `repaint_child_frames` skips frames whose DOM is clean — correct when the
*parent's* script reached in and mutated the child. But a click routed INTO a frame makes the child
run its own script round, and that round re-cascades, re-lays-out and **clears the child's dirty
bits**. By the time the parent looks, the child is clean, so the guard skips precisely the frame that
just changed: the handler ran, the child's DOM said `approved`, and the screen still said `pending`.

Hence `repaint_frame(…, force: true)` on the click path. The click is the signal; there is nothing
left to detect afterwards. `force` here is a correctness requirement, not a performance shortcut.

### Body background does not reach the frame's canvas

Recorded because it cost a debugging detour: mutating `document.body.style.background` in a child
does **not** change the frame's bitmap. A box *inside* the frame's viewport does. The canvas-background
propagation from the root/body element is unimplemented — an independent gap, and one that makes
`body { background }` a poor thing to assert a repaint with.

## File upload: the interaction with no door, and the encoder nobody fed (tick 247)

`Page::set_input_files(node, &[(name, mime, contents)], …)` is the actuation entry for
`<input type=file>` — `G_FILE_INPUT`. It stores the selection, sets `value`, and fires **`input`
then `change`**, in that order, as a real picker does.

**Why it needed an entry point at all.** Every other interaction has a scriptable analogue: a click
is an event, typing is an event. **Choosing a file is not** — the bytes arrive through a native OS
picker dialog with no scriptable surface. So avatar, attachment, document and photo flows were not
*broken*, they were **unreachable**, and no capability probe reports a missing door.

### The bytes were dropped one layer ABOVE the code that knew how to send them

`manuk-net::multipart` is real, tested and correct, and had **never once been handed a file.**
`new FormData(form)` harvested `e.value` for every control including `type=file` — and the spec makes
a file input's `value` the deliberately-useless `C:\fakepath\a.txt`, so the field was submitted as
that literal string.

**This is a silent corruption, not an absence, and the distinction decides how you gate it.** The
page can see the file perfectly — `files.length`, `name`, `size`, `type` all read correct — while the
server receives `"C:\fakepath\a.txt"` where a JPEG should be. The RED probe proves the separation:
restoring the `value` harvest flips **the multipart claim alone** and leaves every page-visible claim
green. **An upload that succeeds and delivers garbage is worse than one that fails**, so "the page can
see the file" is not the property worth asserting — *"the bytes reached the wire"* is.

`C:\fakepath\` is spec, not whimsy: the real path is withheld (it leaks the user's directory layout),
and that exact prefix is mandated because sites had already been written to parse a Windows path out
of `value`. Returning a bare filename broke them.

### Installing a getter with no `Element` binding to hang it on

There is no `globalThis.Element` in this prelude — the chain
(instance → `HTMLElement.prototype` → `Element.prototype` → `Node.prototype`) is built in Rust and is
real, but **unnamed in JS**. `Object.getPrototypeOf(document.createElement('input'))` is the live
link, so a getter defined on it is inherited by every element that already exists *and* every one
created later. Per-instance definition would have missed both. **The general move: when the prototype
is real but unnamed, fetch it from a probe instance rather than adding a binding.**

`files` returns **`null`** on a non-file control, not an empty list — pages branch on
`input.files === null` to tell a text field from a file field, so an empty `FileList` would answer
"a file input with nothing chosen" about an `<input type=text>`.

### The fourth dead-end wire in six ticks

242 (quirks verdict), 243 (index key), 246 (focus), and now the multipart encoder. Same shape every
time: **the engine holds the right answer and throws it away at the last hop**, invisible to any probe
because the feature appears present at every layer anyone inspects. The tick-246 audit shape — *grep
for values computed with exactly one reader, or none* — extends here to **capabilities with no
producer**: a correct, tested encoder that nothing ever calls with real input.

## The dropzone: a handler that threw, and an opt-in that looks like ceremony (tick 248)

`Page::dispatch_drop(node, files, …)` fires `dragenter`, `dragover`, `drop` with one shared, real
`DataTransfer` — `G_DROP_UPLOAD`. It is the second half of the upload story begun at tick 247, and on
the modern web it is the *more* common half.

**The inert `DataTransfer` did not make drops a no-op — it made drop handlers THROW.**
`e.dataTransfer.files` is the first line of essentially every dropzone, and `undefined.files` is a
TypeError *inside* the handler. A page that throws there falls back to nothing: the dashed rectangle
stays lit and the upload never starts. **"Feature absent" and "handler throws" are different failure
classes**, and the second is the one that leaves the UI in a lying state.

### Why the whole sequence, not just the interesting event

The HTML drag protocol makes a page **opt in** to being a drop target: **a dropzone that does not
`preventDefault()` its `dragover` never receives a `drop`.** So dispatching `drop` alone would test a
path *no real browser can reach*, and would skip the `dragenter`/`dragover` handlers that set the
"drag active" styling — silently omitting the visible half of the interaction. The `DataTransfer` is
shared across all three because a dropzone that stashes it on `dragenter` must find the same object
on `drop`.

The return value carries the page's `preventDefault()` verdict and the host must honour it: a browser
that performs its default action after the dropzone accepted the drop **navigates to the dropped
file**, replacing the page the user was uploading to. That is the classic "my app vanished when I
missed the drop target" bug.

`types` must contain the literal token `Files`. `types.indexOf('Files') >= 0` is the standard
file-drag-vs-text-drag test, and a dropzone that gets it wrong takes the text branch and never looks
at `files` at all.

### A gate over a sequential handler chain has coarser resolution

Both RED probes here (fire only `drop`; build `types` as `[]`) go red at the **same** claim, `enter:`
— including the one predicted to isolate `types:`. The dropzone's handlers run in order, so **the
first bad read masks every later claim.** Contrast tick 247's multipart probe, which flipped exactly
one claim because the claims there were independent. Worth carrying when designing a gate: **claims
asserted inside a sequential chain cannot pinpoint; claims asserted over independent surfaces can.**

## Mouse actuation (tick 251): the sequence, the verdict, and a ledger that was wrong about itself

Two dispatchers landed — `Page::dispatch_dblclick` and `Page::dispatch_contextmenu` — but the more
durable finding is *how they were chosen*.

### The ledger nominated a phantom as its own top priority

`DAILY-DRIVER-EDGES.md` §1c listed **A11y node STATES** as *"missing (verified)"* and, in the same
cell, **"Highest-leverage agentic fix"**. It was already built: `A11yNode.state: A11yState`, with a
tri-state `Checked`, gated by `g_a11y_state.rs`. Two more rows (`file-input actuation`,
`drag-and-drop`) were falsified by ticks 247 and 248 — *three ticks earlier in the same session*. A
fourth (`hover/dblclick/contextmenu`) was half-stale: hover landed at t245.

**The checklist goes stale fastest from our own landed ticks**, which is the least intuitive
direction and the reason process rule 2 exists. Re-probing cost minutes; building the phantom would
have cost a tick and produced nothing. This is the seventh time the ledger has been wrong about its
top item.

The genuinely-missing hole was the one *next to* the phantom, and only visible once the phantoms
were cleared: `dblclick` and `contextmenu` had **zero hits across the whole engine**.

### A double-click is a sequence, not an event

Firing `dblclick` alone would have been the third half-fix of the week. A real double-click is
`click` (detail 1) → `click` (detail 2) → `dblclick` (detail 2), and **`event.detail` is the click
count**. `if (e.detail === 2)` on an ordinary `click` listener is the idiomatic double-click handler
— used precisely because it needs no second listener.

The first implementation dispatched both clicks correctly and **carried no `detail` at all**, because
`dispatch_click` routed through the bare-type `dispatch_event`. The gate caught it: `clicks=2 dbl=1`
— every handler running, sequence perfect — with `details=` empty and the `detail === 2` branch
unreachable forever. The fix threads a click count through `dispatch_click_detail`, so
label-forwarding and activation still behave exactly as for a single click.

Confirmed by RED probe, and the two failures are instructively different:
- **dblclick alone** → `clicks=0 dbl=1`. The notification arrives; the interaction never happened.
- **`detail: 0`** → `clicks=2 details=0,0`. Every listener runs and the branch is dead.

### A right-click's return value IS the capability

`contextmenu` is cancelable, and cancelling it is how every custom right-click menu on the web works.
So `dispatch_contextmenu` returns the page's verdict, and a browser ignoring a `false` would draw its
native menu on top of the page's own. Same shape as tick 248's drop verdict. `button: 2` is passed
explicitly (menu code guards on it), and `buttons` is computed rather than aliased — it is a
**bitmask**, so the right button is index 2 but bit 4. They coincide for no button and differ for the
middle one, which is exactly the kind of coincidence that hides a bug.

### A harness Bar-0 that was not an engine Bar-0

The four-test version of this gate **SIGSEGV'd**, while each test passed in isolation. A
`PageContext` is per-process here, so a second `Page::load` in one test binary races the first one's
runtime. Every JS-driving gate in `engine/page/tests/` is a single `#[test]` for this reason — a
convention that was load-bearing and undocumented. The crash signature is Bar-0-shaped and the cause
is entirely the test harness; **reading it as an engine regression would have sent the next tick into
the wrong organ.**

A related trap on the way: `eval_for_test` silently does nothing on a page with **no `<script>`**,
because no JS context is created. Activation itself does not need JS (`dispatch_click` says so), so
the checkbox assertions observe through the **a11y tree** instead — which is also how a real agent
confirms its own action, and doubles as proof the `A11yState` row above is no longer a phantom.

## The pointer sequence (tick 252): the menu that opens on mousedown

Tick 251 established a double-click is a sequence. One layer down the same was true and worse:
`mousedown`, `mouseup` and `pointerdown` were dispatched **nowhere in the engine**.

A large class of real UI never listens for `click`. Dropdown menus, comboboxes, drag handles,
sliders and press-and-hold controls open on `mousedown` — deliberately, so the menu is up before the
button comes back up. All of it was inert, silently.

### `buttons` is a mask, `button` is an index, and they coincide enough to hide the bug

`button` is which button (primary 0, right 2). `buttons` is a bitmask of what is **currently held**
(primary 1, right 4). Two consequences that a single derived value gets wrong:

- On **`mouseup` the mask is 0** — the press is over. Deriving `buttons` from `button` reports a
  button still held after release.
- The derived form happens to be *correct* for `click` and `contextmenu`, which is exactly why the
  refactor to "just compute it once" is tempting and wrong.

### A label presses down once, on the element under the pointer

`<label>` forwarding re-enters `dispatch_click_inner`, not the outer entry point. A real browser
fires the pointer pair on the element the pointer is actually over and forwards only the *click* to
the labelled control. Re-entering at the top gives the control a press it never received — confirmed
by RED probe (`controlDowns=1`). This is why the change is a function split rather than three lines.

### `preventDefault()` on mousedown is not a click-cancel

It suppresses focus and text selection. Every toolbar button in every rich editor relies on this: it
prevents `mousedown` to keep the document selection alive, and still expects its `click`. Honouring
that verdict as a cancel made the click vanish (`seq=down>up`) — a breakage that would have looked
like "the editor's buttons stopped working" with no error anywhere.

Residue: no Pointer Events (`pointerdown`/`pointerup`/`pointermove`), which modern drag libraries
increasingly prefer; no `mousemove`, so drag *gestures* are still unreachable. This is the press,
not the drag.

## The select that submitted correctly and read as empty (tick 253)

`select.value` was `""`, `selectedIndex` `undefined`, `options` `undefined` — for every `<select>` on
the web. `HTMLSelectElement` existed as an interface marker with nothing behind it.

**Why nobody noticed: form submission was right.** Submission reads the DOM directly and had been
correct since t163-167. So the field submitted the right value while any script branching on
`select.value` got an empty string. **Two paths to the same question, one right, one silent — and
pages only read the silent one.**

### "Nothing selected" vs "nothing selected yet"

The one assertion that failed on first run, and the real content of the tick. `select.value = "x"`
where no option has value `x` must land on **index -1**. An untouched single-select must report
**index 0** — that is what the browser shows and what the form submits. Both states have *no option
marked*, so the `selected` content attributes cannot distinguish them.

The spec models selectedness as a per-option bit **separate from** the content attribute, precisely
for this. Deriving both from the same absence yields one of the two answers and is silently wrong
about the other — and which one you get depends on which case you happened to test.

### Two more traps, both confirmed by probe

- **An option's value falls back to its TEXT.** `<option>Blue</option>` submits and reports `"Blue"`.
  Dropping the fallback reports `""` for every unvalued option, and a great many real selects are
  written that way.
- **`<optgroup>` options still belong to the select.** A children-only walk reports a grouped select
  as having *zero* options — it reads as entirely empty rather than merely wrong.

### `input` then `change` — and React only hears the first

`Page::select_option` fires both, in that order. **React's `onChange` is really the `input` event.**
A host firing only `change` leaves every React select unchanged while vanilla pages keep working,
which presents as "it works on some sites" and is miserable to diagnose. Firing only `input` fails
the mirror image.

Residue: `data-manuk-noselection` is visible to `getAttribute`/`outerHTML` (same shape as t247's
`data-manuk-files`) — real selectedness needs per-element state the arena does not carry.
`select.options`/`selectedOptions` are still absent, so `s.options[i]` throws; a live
`HTMLOptionsCollection` is its own tick.

## `s.options[i]` (tick 254): when the empty answer throws too

`select.options` did not exist, so `s.options.length` was a TypeError and the script died there. The
usual consolation — "at least it reads as empty" — **does not apply here**, and the RED probe proved
it: a `selectedOptions` that correctly reports 0 for an untouched select makes the page throw on
`selectedOptions[0]`. The empty answer cascades into the same TypeError class.

That is worth stating generally: **for a collection, "reports nothing" and "throws" are often the
same bug one line apart**, because the caller's next move is to index it.

### The divergence class, reproduced by me in a day

`option.value` read the raw `value` attribute, while `select.value` used a helper that falls back to
the option's text. So `<option>Blue</option>` reported `"Blue"` via `s.value` and `""` via
`s.options[2].value` — **the same fact, two readers, disagreeing.** Tick 253's entire finding was
that form-submission and `select.value` disagreed; one layer down I built the identical shape. The
lesson is not "be careful" — it is that **any fact with a fallback rule needs exactly one function
computing it**, and both callers must route through it.

### `option.index` counts across optgroups

It is the position within the **owning select**, not within the immediate parent. A
child-index-within-parent answer makes the second `<optgroup>` restart at 0, so code keying on
`index` addresses the wrong option in every group but the first — confirmed by probe (`gIdx=0,1,0`).

Residue: these are **snapshot Arrays**, not live `HTMLOptionsCollection`s. Indexing and `length`
work; `item()`, `namedItem()`, `add()`/`remove()` do not exist, and a collection captured before a
mutation will not reflect it.

## Scroll snap — the carousel stops on a slide (tick 266)

`scroll-snap-type` on a scroll container plus `scroll-snap-align` on its children is what every paged
feed, story tray, gallery and mobile card row is built from. Measured absent by
`g_probe_capabilities` (`scrollsnap: no`); now gated by `G_SCROLL_SNAP` and pinned in that probe's
ratchet list.

### One transformation at one chokepoint

`Page::set_element_scroll` was already the single place a container's offset is decided — it clamps
the requested position to what there is to scroll and translates the subtree by the delta. Snapping
is therefore not a scrolling subsystem, it is one function inserted at that point:

```
let clamped = (left.clamp(0.0, max_x), top.clamp(0.0, max_y));
let new = self.snap_scroll(node, clamped, (max_x, max_y));
```

**The ordering against the clamp is the entire correctness question.** Snap first and a candidate
beyond the scrollable range gets chosen, then clamped back to an unaligned offset — so the container
**can never reach its own last slide.** That is the classic carousel bug: it is invisible unless a
test scrolls all the way to the end, and it presents as a content problem rather than a scrolling one.

### Three decisions

**Candidates come from the container's own subtree.** `container.walk(...)` rather than a
document-wide scan, so one carousel cannot snap to another carousel's slide.

**An empty candidate set must leave the offset alone.** A container declaring `scroll-snap-type` with
no aligned children has nothing to snap to, and "the nearest of nothing" degrades to pinning it at
zero — a declared-but-unused property turning into a scroller that cannot scroll. This is carried by
`nearest()` returning its own input when the candidate list is empty, and **that line is the feature**
— an explicit `!is_empty()` guard in front of it was dead code (see below).

**`mandatory` vs `proximity` is deliberately not modelled.** `proximity` lets the UA decide, and
"snap to the nearest point" is conforming for both. Modelling the axis decides whether a carousel
lands on a slide; modelling the strictness would only change *how often*, and picking a proximity
threshold would be inventing behaviour rather than implementing it.

### Property plumbing: recovered from MinimalCascade

`scroll-snap-type`/`scroll-snap-align` parse in `MinimalCascade` and are copied into the Stylo path
in `stylo_engine`, exactly as `text-overflow` and `overflow-wrap` already are — Stylo's servo build
models them as typed values we do not consume, and the shipping path needs plain keywords. They also
serialise back through `getComputedStyle` in both camelCase and `getPropertyValue` form, because a
carousel library reads `scrollSnapType` to decide whether to run its own polyfill.

### The probe that came back green

Four RED probes were run and **one passed** — removing the `!ys.is_empty()` guard changed nothing,
because `nearest()` already returns its input on an empty set. The guard was **dead code sitting in
front of the line that actually does the work**, so the assertion aimed at it could not fail. It was
deleted and the real failure shape probed instead (`unwrap_or(0.0)`, pinning the container at the
top), which fires. Sixth vacuous-assertion catch in six ticks, and the generalisation past assertions
is: **a redundant guard hides which line is load-bearing, so the probe aims at the wrong one.**

### Residue — the bigger half is still open

**Only the vertical axis works, and the gate says so by only testing that axis.** A horizontal one
could not be gated: an inline-block row yields no horizontal scroll range in layout today (`max_x`
comes back `0`), so `overflow-x: scroll` does not scroll at all. That is a pre-existing
**scroll-geometry** gap rather than a snap gap — the snap code handles `x` symmetrically and is
simply unexercised there — but the practical consequence is that **horizontal carousels, the
commonest kind, still do not scroll.** Fixing `scrollWidth` for inline rows is the next lever here.

## Horizontal rows: `white-space: nowrap` around atomic inlines (tick 267)

The residue above named the wrong organ, and a probe said so before a line was written. The claim was
a **scroll-geometry** gap — "an inline-block row yields no horizontal scroll range". Measuring four
container shapes instead of theorising from one failing case:

| shape | scrollWidth | verdict |
|---|---|---|
| `display:flex` row, `flex-shrink:0` | 500 | **already correct** |
| single wide block child (`width:500px`) | 500 | **already correct** |
| `inline-block` row, no `nowrap` | 200 | correct — it *should* wrap |
| `inline-block` row + `white-space:nowrap` | 200 | **the bug** |
| plain text + `nowrap` (control) | 490 | `nowrap` already worked for text |

So horizontal scroll geometry was never broken. `nowrap` was broken **for exactly one token type**.

### The mechanism

An inline formatting context is a run of tokens; `InlineItem::Word` and `InlineItem::Atomic` (an
`inline-block` / `inline-flex` / `inline-grid`) are both tokens in it, and the line breaker's rule is

```rust
let breakable = !(no_wrap && prev_no_wrap);
```

— a break opportunity belongs to *both* sides, so it is suppressed only when both are `nowrap`. The
`Word` arm read `white-space` off the text node's inherited style. The `Atomic` arm passed a
**hardcoded `false`**. One literal, and every atomic inline permanently advertised itself as a legal
break point, so `no_wrap && prev_no_wrap` could never hold across a row of them.

The fix carries `no_wrap` on `InlineItem::Atomic`, read from the atomic's own computed style at
collection time — `white-space` is *inherited*, so the container's `nowrap` is already sitting on the
child, the same source the `Word` path uses. Nav bars, tab strips, chip rows, breadcrumbs, toolbars
and carousels are all this shape.

### The failure was not "it doesn't scroll"

It was that the row **silently wrapped into a stack** — five 100px tabs in a 200px bar became three
rows, the bar grew to 3× its declared height and shoved the page down, and then `scrollWidth ==
clientWidth` so nothing scrolled *because, given the wrapped layout, there was correctly nothing to
scroll*. The engine was **self-consistent and wrong**, which is why no capability count could see it
and why the symptom pointed at scroll geometry rather than at line breaking. Same lesson as
[[symptom-names-wrong-organ]]: measure the boxes before theorising from the visible end of the chain.

### What this unblocked for free

Tick 266 wrote the x-axis of `snap_scroll` and **could never run it** — there was no horizontal
scroll range in the engine to run it against, so it was asserted correct by symmetry alone. With the
range present it is now gated: `x=120` lands on 100, `x=270` snaps to the nearest point (300, not
back to 0), and `x=9999` reaches the last tab rather than clamping to an unaligned offset.

### The control is the load-bearing assertion

`#wrap` — the identical row *without* `nowrap` — must **still wrap**. That is what separates "honours
`white-space`" from "never breaks inline-blocks", and passing `no_wrap: true` unconditionally is a
real RED probe that only this assertion catches. A blanket disable would have turned every ordinary
`inline-block` gallery on the web into one infinite line while making the headline assertion greener.

---

## `visibility:hidden` must not be hit-testable (tick 272)

A closed dropdown was **swallowing clicks on the article underneath it**.

```css
.dropdown-content { position:absolute; visibility:hidden; width:max-content }
```

The modern web hides every dropdown, popover, menu and tooltip this way — `visibility:hidden` rather
than `display:none`, so the panel keeps its box and can be revealed without a reflow. It is
therefore **laid out at full size, permanently, on top of real content**. Chrome lays it out and
neither paints nor hit-tests it. We hit-tested it.

### Why the a11y tree is the right place to fix it

`A11yNode::hit_test` consults only the box, and `is_hidden` reads the `hidden` / `aria-hidden`
**attributes** — `visibility` is a *style*, and the a11y builder never saw the cascade at all. Per
WAI-ARIA a `visibility:hidden` element is **not exposed in the accessibility tree**, so pruning it
there is both spec-correct and fixes hit-testing for free, with no new concept in the hit-test path.

`visibility` is a *style*, so it cannot be derived from the DOM: the page, which holds the computed
styles, passes a `HashSet<NodeId>` of hidden nodes into `build_tree_with_visibility` — the same
shape the z-index map already uses for occlusion awareness.

### `visibility` is the one hiding mechanism a descendant can UNDO

```html
<div style="visibility:hidden">          <!-- dropped from the tree -->
  <button>Menu item</button>             <!-- dropped -->
  <button style="visibility:visible">    <!-- SHOWN by Chrome — must survive -->
</div>
```

So the hidden node is dropped and **the walk continues into its children**. Writing `continue` (which
prunes the subtree) is the natural first implementation and it deletes the re-shown descendant; the
gate asserts this case explicitly so that implementation cannot pass. `display:none`,
`hidden` and `aria-hidden` are *not* undoable and still prune.

### How it was found

Not by looking for it. Tick 272's other change widened absolutely-positioned panels to their correct
`max-content` width, and G6 clickability fell from 98.9% to 97.9% — four more links with a hidden
Wikipedia menu on top of them. **The occlusion was always wrong; the panels had merely been too small
to cover much.** A gate on a metric nobody was aiming at is what surfaced it.

## `<input>`/`<textarea>` text selection — `setSelectionRange` / `select` (tick 302)

How a page positions the caret, selects a range, or "selects all on focus": `selectionStart` /
`selectionEnd` / `selectionDirection`, `setSelectionRange(start, end [, direction])`, and `select()`.
The whole surface was absent (`undefined` / `is not a function`), so an input mask, a copy-button that
`select()`s its field, or an editor that reads the caret position all broke.

The selection is stored per element in UTF-16 code units (a thread-local `NodeId → (start, end,
direction)` map): `setSelectionRange`/`select` write it (clamped so `start ≤ end ≤ value length`),
`selectionStart`/`selectionEnd` read it (defaulting to the end of the value when nothing has set a
selection), and `selectionDirection` reports `none`/`forward`/`backward`. `select()` covers the whole
value.

### The teeth `G_TEXT_SELECTION` uses

`select-all` (`select()` → `0..length`), `range` (`setSelectionRange(2,5)` reads back `2`/`5`),
`direction` (`'backward'` round-trips), `clamp` (offsets past the length clamp to it). A stub with
constant offsets fails; unregistering the accessors made the calls throw before landing.

**Honest limit:** this is the JS/IDL contract (a page can read and set the selection); the visual
highlight of the selected text is a rendering follow-on, and `setRangeText` (mutating the value through
the selection) is not yet wired. [[js-engine]]
