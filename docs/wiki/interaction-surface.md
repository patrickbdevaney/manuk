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
click). Residue: OS-clipboard `readText`, the `permissions` query, and the legacy
`document.execCommand('copy')` path are not wired; a write triggered off the click path (a timer, a
fetch reaction) is not yet pumped. [[js-engine]]

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
