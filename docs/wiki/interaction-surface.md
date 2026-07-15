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
