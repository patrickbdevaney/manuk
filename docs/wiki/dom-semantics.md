# DOM SEMANTICS — spec behaviour, mutation, and tree edge cases

## Inserting a node BEFORE ITSELF is a move, not a no-op — and getting it wrong is a hang

**DOM spec, "pre-insert", step 2:** *"If referenceChild is node, then set referenceChild to node's next
sibling."*

Our `insert_before(parent, X, X)` skipped that step, and the consequence is not a wrong answer — it is
an **infinite loop**:

```
detach(X)                       → X's sibling links are cleared
X.next_sibling = Some(sibling)  → sibling IS X, so  X.next_sibling = X   ← self-cycle
```

Every subsequent `children()` walk spins forever. **That is a hang, so it takes the whole page down
(Bar 0), not just the one call.**

> **The generalisable point — and it is the argument for conformance testing in one bug:** *no real
> site inserts a node before itself.* A 265-site differential crawl against Chromium could never have
> found this. **WPT found it in the first 25 tests** (`ChildNode-after`, which calls `child.after(child)`
> **on purpose**). Adversarial self-referential input is precisely what a real-traffic corpus never
> generates and a conformance suite always does.

## An INCOMPLETE family of methods is worse than an absent one

We shipped `insertAdjacentHTML` and `insertAdjacentElement`. We did not ship **`insertAdjacentText`** —
two of three siblings.

**Nobody feature-detects the third sibling when the first two are present.** So the call throws, and
the blast radius is whatever was running: `testharness.js` uses it to render its results table, so the
throw aborted the loop invoking the completion callbacks, and **29 of the first 40 WPT files reported
nothing at all** — every one of them looking like a conformance failure rather than one missing method.

> **Rule: when implementing one member of a spec'd family (`insertAdjacent*`, `before/after/replaceWith`,
> `append/prepend`), implement the family.** Partial families fail silently and at a distance.

## The document lifecycle: `readyState`, `DOMContentLoaded`, `load`

**None of it existed.** Grep found *zero* occurrences of `DOMContentLoaded` or `load` dispatch in the
whole engine. These are the two most-used lifecycle hooks on the web: a site whose init lives in
`window.addEventListener('load', …)` or `document.addEventListener('DOMContentLoaded', …)` **simply
never initialised** — in silence, with no error to see.

**The worst part is the failure shape.** Libraries that *check* `document.readyState` (jQuery does) got
away with it, because the property was `undefined` and their "already loaded?" test fell through to
running immediately. Libraries that *only listen for the event* got nothing. **So it worked often
enough to look fine** — which is why it survived 40+ ticks unnoticed.

The host must fire these, because **only the host knows when they are true**: *"the document finished
parsing"* and *"the subresources finished"* are facts about the loader, not about JS. The two real
moments:

- **`DOMContentLoaded`** — after parsing completes **and the deferred scripts have executed**.
- **`load`** — after subresource loading settles. **It fires either way**, including when the load
  budget is exhausted: a real browser does not withhold `load` forever because one subresource was
  slow, and withholding it leaves every `window.onload` handler on the page unrun.

Both must be **idempotent** (several load paths can reach them) and `DOMContentLoaded` must reach
**both** registries — jQuery listens on `document`, `testharness.js` listens on `window`, and in a real
browser the event bubbles document → window.

## CharacterData offsets are UTF-16 CODE UNITS — not bytes, not `char`s

`"😀".length === 2` in JavaScript. An offset of 1 lands **inside the surrogate pair**. Rust strings are
UTF-8, so an implementation that counts `char`s produces the wrong answer for **every emoji, every CJK
surrogate and every combining sequence** — silently, and **only for the users who write in those
scripts**, which is the worst possible distribution of a bug. Convert through `encode_utf16`.

The same unit applies to `Range` offsets, `Selection`, `splitText`, and `normalize`. **Get it wrong once
and it is wrong everywhere text is addressed by index.**

## A native can throw a real `DOMException`

Evaluate the `throw` in the current global and return `false`: the exception is left **pending on the
context**, and returning `false` from a `JSNative` propagates it. That is the sanctioned failure path —
`JS_ReportErrorUTF8` would throw a plain `Error`, which fails `e instanceof DOMException` and, more
importantly, is not what real code catches.

## A CONSTANT is an infinite loop for any code that waits for it to change

`event.timeStamp` was hardcoded to `0`. `Event-timestamp-safe-resolution` does
`do { … } while (delta == 0)` — it **busy-waits for the clock to advance**. A frozen clock is not a
wrong value; it is a **hang**. The same trap exists for `performance.now()`, `Date.now()` under a
virtual clock, and any monotonically-increasing counter a page polls.

---
# Backfill — mechanisms recovered from ticks 1–42 (pre-wiki)

## The FLAT TREE and the node tree are different trees, and every renderer must walk the flat one

A **shadow root is NOT a child of its host** (it hangs off the host in its own field), while the host's
**light children REMAIN its children in the node tree** even though they render at a `<slot>`.

`Dom::flat_children` was **correct, tested, and used by the HTML crate** — while layout and the cascade
walked `children()`. So the cascade never styled a single node inside any web component, and **an
unstyled node is dropped from the render tree outright: zero boxes for every custom element on the web**
(Material, Fluent, Shoelace, Spectrum, every `<x-y>` on a bank or government site).

**The mechanism existed. Nothing had drawn a line from it to the renderer, and no gate asked.**

**Scoping rule:** matching is scoped, **inheritance is not**. A document `p{}` cannot reach inside a shadow
root and a shadow `p{}` cannot escape — but the cascade recurses over the **flat** tree, so a slotted node
is visited at its slot and inherits from its **flat** ancestors. `::slotted(<compound>)` is the one
selector that deliberately crosses; written outside a shadow tree it matches nothing.

## html5ever ALREADY implements Declarative Shadow DOM — the hook just defaults to `false`

html5ever's tree builder checks `shadowrootmode` on a `<template>` start tag and calls
`TreeSink::attach_declarative_shadow` — but **that trait method defaults to `false`**, and
`markup5ever_rcdom` never overrides it. So `<template shadowrootmode="open">` parses as an ordinary
template and the shadow root is **silently dropped**. *A mis-wired reuse, not a missing capability.*

**The non-obvious second half:** the hook fires at the template's **START tag**, and html5ever then keeps
inserting content into `get_template_contents(template)`. So a hook that tries to *move* the template's
children into the shadow root **moves zero nodes — none exist yet.** You must point the template's
**contents at the shadow root**.

## A DocumentFragment's defining property is what happens when you INSERT it

**Its children move into the parent and the fragment itself does not.** That single rule is why every
framework builds a subtree in a fragment and commits it in **one** insertion.

We had a `NodeData::Fragment` type documented in our own source — while `createDocumentFragment()`
returned a **`<div>`**, `template.content` returned the `<template>` **element** (which is `display:none`,
so inserting it inserted an inert wrapper), a fragment reported **`nodeType 8`** (comment) instead of
**11**, and `cloneNode`/`importNode` fell through to `create_element("div")` for anything that was not an
element or text.

`importNode(template.content, true)` is the single call **every compiler-based framework** commits a
template through.

## Comment nodes are load-bearing INFRASTRUCTURE, not annotations

**lit-html** finds the dynamic holes in a cloned template with `createTreeWalker(SHOW_ELEMENT | SHOW_COMMENT)`
and reads `node.data`. **Vue and Svelte** anchor every `v-if`/`{#if}` and every list on comment nodes.

**A comment draws no box — which is precisely why frameworks use it as an anchor: a position in the tree
that costs nothing.** `document.createComment()` was returning an empty **text** node, which is invisible
to that walk, so lit-html found **zero parts**, rendered nothing, and threw nothing.

A shadow root must be `nodeType` **11**, not 8 — reporting 8 is how a component wrongly concludes it is
**not** in a shadow tree.

## `textContent` is a node-tree API, so any "visible text" built on it is wrong

Switching `visible_text` to read the **fragment tree** made it respect `display:none`, exclude `<head>`
content, and honour shadow DOM and slot assignment **for free**. The bug that exposed this: shadow content
laid out correctly with real geometry but was missing from both `visible_text` **and the a11y tree**,
because both were walking the node tree.

## `innerText` is the RENDERED text, and the binding CAN compute it — it holds the styles already

The JS `el.innerText` getter returned `textContent` for a long time, with a comment claiming the true
value "means asking the layout tree, which the binding layer cannot reach from here." **The premise was
false.** The binding layer holds the **pre-script computed styles** the whole time — `STYLES_PTR` is a
thread-local `*const HashMap<NodeId, ComputedStyle>` set by `set_view_maps`, read via `with_style(node,
|cs| …)` — which is exactly what `innerText` needs and what `getComputedStyle` already uses. So innerText
is a faithful **structural** approximation with zero new plumbing:

* **`display:none` subtrees are skipped** — this is the #1 divergence from `textContent`, which happily
  returns text a page has hidden. `with_style(child).display == None` ⇒ don't descend.
* `<br>` ⇒ `\n`; **block/flex/grid/table** display inserts a newline before and after its content.
* whitespace is **collapsed** in normal flow, **preserved** under `white-space: pre*` (carried down the
  recursion as an `in_pre` flag).

`outerText` reads the *same* rendered text (its getter is defined that way), and was `undefined` — which
failed **every** innerText subtest, because the suite asserts innerText and outerText together. Its setter
replaces the element with the text, `\n` becoming `<br>`.

> **The transferable lesson for every future binding:** before writing "the binding can't reach X," check
> `STYLES_PTR` / the view maps. Computed style and layout geometry are already marshalled across the FFI
> for `getComputedStyle`/`getBoundingClientRect`; a getter that needs display, position, or a box can use
> them too. **What is layout-exact stays out of reach** (innerText's required-line-break-count rendering,
> `::first-letter`, multicol) — the pre-script *computed style* is available; the *fragment tree* is not,
> from the binding.

## Generational `NodeId` buys use-after-free safety while staying a bare integer for JS

The arena packs `generation<<32 | index`. A freed slot bumps its generation, so a stale handle to a reused
slot **fails `is_alive` (returns `None`) instead of aliasing a new node**. Crucially, **generation-0
(never-reused) nodes are byte-identical to a bare index**, so JS reflectors' `i32` slot encoding stays
valid.

There is deliberately **no auto-free** — the parser reparents and JS `removeChild` often re-inserts — so
reclamation is opt-in at proven-discard sites.

> **This also dissolves the classic C++↔JS cycle**: because the DOM is `NodeId`-indexed rather than
> refcounted, a JS wrapper holding a `NodeId` **cannot form a native refcount cycle** — the problem
> `nsCycleCollector` exists to solve largely does not arise. Gecko's cycle collector was declined for
> exactly this reason.

## `document.readyState` is the most-checked property on the web, and `undefined` makes half of it work BY ACCIDENT

Half the scripts on the internet open with
`if (document.readyState === 'loading') { wait } else { init() }`. An **undefined** value makes that
comparison false, so those scripts take the `else` and initialise immediately — **right by accident.** The
libraries that instead wait for `'complete'` **wait forever.**

**This masking is why nobody noticed that `DOMContentLoaded` and `load` were never dispatched anywhere in
the engine** (grep returned zero) for forty ticks. jQuery — on ~74% of pages — survived by checking
`readyState`. Any site whose init lived in `addEventListener('load', …)` **simply never initialised**.

> *A gap that works often enough to look fine is the hardest kind to find, and the population that hides it
> is disjoint from the population it destroys.*

## The "detached document" checks frameworks use are string/identity comparisons that `undefined` fails silently

- **`document.defaultView`** — frameworks get `window` from a **node** (`el.ownerDocument.defaultView`)
  precisely so they work inside an iframe. `null` makes them think they are in a **detached document** and
  skip everything.
- **`document.visibilityState`** — video players and animation loops compare against the *string*
  `'visible'`. `undefined !== 'visible'` makes a player believe the tab is **backgrounded** and refuse to
  start.
- **`nodeType`** — React's `isValidContainer` checks it; without it you get **React error #299**, *"Target
  container is not a DOM element"*.
- **`isConnected`** — React and Vue check it **before every commit**.

## Registering a DOM property twice lets the later registration silently win

`content` was registered once for `<meta content>` and once for `<template>.content`; **the later one won
and broke the other.** The fix is one dispatching getter. *This is a general hazard of a flat
property-registration table with no collision check.*

## Two form-encoding details servers actually branch on

- **A checked checkbox with NO `value` submits the string `"on"`**, not `""`. *"The box was ticked"
  arriving as an empty string reads at the far end as "ticked, and the user typed nothing" — a different
  claim.* An **unchecked** box is not a successful control at all and contributes **nothing**.
- **`application/x-www-form-urlencoded` encodes a space as `+`, not `%20`.** `encodeURIComponent` alone
  gets this wrong — quietly, and **only for values containing spaces**.

`form.submit()` and `form.requestSubmit()` differ exactly as spec'd: **`requestSubmit()` fires `submit`
(the page may cancel); `submit()` does not** — a script calling it has already decided.

## A HANDLE FROM ANOTHER DOCUMENT IS A DEAD BROWSER, not a wrong answer

A JS reflector stores its node as a **bare integer**, and **the arena it indexes is not necessarily the
arena it came from**: one process loads many documents and the current-DOM pointer is swapped on every
re-entry into script. A handle held from an earlier document therefore indexes into a **different, smaller**
arena, and `self.nodes[id.index()]` **walks off the end.**

**And the panic does not unwind — it ABORTS.** DOM accessors are reached from `extern "C"` natives, which
are **`nounwind`**, so a Rust panic inside one is *"panic in a function that cannot unwind"* → **SIGSEGV.
Every tab the user had open dies because one page held a stale node.**

**The invariant:** validate every incoming handle against **this** arena (bounds **and** generation) at the
single choke point where JS hands one in. A stale or foreign handle then reads as **"no such node"** and the
operation no-ops — *which is the spec-shaped answer anyway: an operation on a node that is not there does
nothing.*

> **It is perfectly clean in isolation.** The failing WPT file passes on its own, and a 120-file batch
> passes; **it only dies when it runs AFTER other documents.** *No single-page test can catch this class —
> which is why it survived every gate.* **Any engine that reuses one process for many documents has this
> bug until it proves otherwise.**

## A detached DOCUMENT is how every sanitizer works — and the moment it exists, you need cycle checks

`document.implementation.createHTMLDocument()` builds a **second, detached document**: DOMPurify and every
other sanitizer parses hostile markup into one so that nothing in it can run, touch the real page, or fetch.
Its absence is a `TypeError` on the call that takes the sanitizer — and the page — down.

**One arena, several roots.** A document is not special storage; it is a node whose *type* is `Document`, so
everything that already walks the tree works on it unchanged. `html`/`head`/`title`/`body` are all real
nodes in the same arena.

> **The moment a page can obtain a second Document, it can try to INSERT it — and inserting a node into its
> own descendant makes the tree a CYCLE**, i.e. an infinite `children()` walk: a **hang**, Bar 0. So
> `createHTMLDocument()` cannot land without **pre-insertion validity** (the spec's `HierarchyRequestError`:
> a Document cannot be a child; a node cannot be inserted into its own inclusive ancestor). **Enforce it at
> BOTH layers** — the JS native throws, and the arena itself refuses, because the arena is reachable from
> the parser and from Rust callers too.

**The failure was invisible until the door unlocked:** five WPT files passed until `createHTMLDocument`
existed, then killed the process instantly — *the validity check was always missing; nothing could reach the
bad state before.*

## A DOM that never throws turns a loud caller bug into a silent leak

The spec's pre-insertion validity steps are not pedantry — each one prevents a **specific corruption that
surfaces somewhere else**:

| Spec rule | What silently accepting it produces |
|---|---|
| *parent must be a Document, DocumentFragment or Element* | **`text.appendChild(div)` succeeds** — a subtree hanging off a **text node**, which no traversal expects and nothing will ever render |
| *referenceChild's parent must be parent* (`NotFoundError`) | `insertBefore` **appends somewhere else instead**, putting the node where the page never asked, **with no way for it to find out** |
| *child's parent must be parent* (`NotFoundError`) | `removeChild` **does nothing** — and **every framework's unmount path catches this exception**, so a DOM that never raises it converts a loud bug into a **leak** |

> **Silently accepting an impossible tree is worse than refusing it.** The corruption does not surface where
> it was created. It surfaces later, somewhere else, looking like something unrelated.

## `<body onload>` is `window.onload`, and it must fire EXACTLY once — dispatch OR explicit, never both

`<body onload="…">` migrates to the Window: the inline-handler wiring sets `g.onload = fn`. Firing `load`
then went through TWO paths in `__fireLoad` (dom_bindings.rs) that **both** reached that handler:

1. `g.dispatchEvent(ev)` → `__fireWindowEvent(type, ev)`, which runs the `addEventListener('load')` list
   **and then reads `g['on' + type]` and calls it** (dom_bindings.rs ~6805). So dispatch alone already
   invokes `window.onload`.
2. an explicit `if (typeof g.onload === 'function') g.onload(ev)` immediately after.

Result: **every `<body onload>` handler fired twice.** The tell in the `diag` instrument is `onloadCalls:2`.

**Why it survived ~96 ticks — the failure mode is asymmetric by handler idempotency:**

| Handler style | Double-fire effect | Example |
|---|---|---|
| Idempotent, no `done()` | harmless — does the same work twice | encoding suite (decode + assert), 720k subtests — **why the crown jewel never flagged it** |
| Non-idempotent / calls `done()` | **fatal** — second run creates duplicate `test()`s and a second `done()` *after the harness completed* → the whole file reports a harness error instead of its real pass/fails | every `check-layout-th.js` suite (css-flexbox et al.), form submits, single-run bootstraps |

**Fix:** dispatch is the single source of truth — remove the explicit `g.onload(ev)` from `__fireLoad`.
Dispatch still invokes the property handler, so `<body onload>` (and `window.onload = …`, and
`addEventListener('load', …)`) each fire once. **Verification that this is safe, not just smaller:** the
probe's `onloadCalls` drops 2 → 1 AND the encoding sanity holds (55k passes / 0 crashes) — proving dispatch
alone still bootstraps the handler.

**The reusable lesson:** an event handler reachable from *both* a dispatch path and an explicit call is a
latent double-fire. When a metric (flexbox 5.5%) won't move, the cause is often not the feature under test
(flex layout) but a **lifecycle bug upstream of it** that makes the test never report honestly. Build the
probe (`diag` + a minimal instrumented page); measure which link in load→onload→checkLayout→done breaks;
do not theorize from the score. [[js-engine]]

## `offsetWidth/Height/Top/Left`, `client*`, `scroll{Width,Height}` are integers — `scrollTop/Left` are not

CSSOM-View types these metrics as `long`: they return the used pixel value **rounded to the nearest
integer**. Returning the raw float (a flex item at `400/3 = 133.3333`) is wrong two ways: it mismatches every
real browser, and it fails any test doing an *exact* `assert_equals(el.offsetWidth, 133)`. Only
`scrollTop/scrollLeft` are `double` (fractional), and only `getBoundingClientRect()` (a `DOMRect`) stays
fractional — those must NOT be rounded. Fix lives in `el_metric` (offset*) and the `scroll_getter!` macro
(a `$round` flag: true for client*/scroll{W,H}, false for scrollTop/Left).

**Caveat that made this ratchet-neutral:** `check-layout-th.js` compares with a **±1px tolerance**
(`assert_tolerance`), so it already passed the fractional value — rounding is correct but does not move the
WPT number there. When a fix is spec-correct yet the score is flat, the metric was already tolerating the
bug; the real lever is elsewhere (for flex/grid: geometry errors >1px, or computed-style mismatches). [[js-engine]]

## `classList` is an ordered SET, and a no-op operation must not rewrite the attribute

`DOMTokenList` (`classList`, and the pattern behind `relList`/`sandbox`) has two behaviours that naive
string handling gets wrong, and both broke high-usage code:

1. **It is a SET.** The token list is the *ordered set parse* of the attribute — **deduplicated**. So
   `class="a b a"` → `remove('a')` must strip **every** `a` (→ `"b"`), and any modifying op on
   `class="a a b"` serializes the set `"a b"`, never `"a a b"`. Ours split-without-dedup and `remove`
   spliced only the first index, leaving `"b a"`.
2. **A no-op must not touch the raw attribute.** Per the spec, `add`/`remove` always run the "update
   steps" (serialize the set → normalizes whitespace, expected), **but `toggle`/`replace` run them ONLY
   when they change the set.** `toggle('x', false)` when `x` is absent must leave `class="a  b"` — double
   space and all — byte-for-byte. Ours re-serialized unconditionally and collapsed the whitespace.

And the RAW-vs-SET split on the getters: **`value` and the stringifier return the raw attribute string**
(`"a  b"`), while **`length`, indexed access (`classList[0]`), `contains`, and iteration use the deduped
set** (`length` of `"a a b"` is 2). Conflating them (serializing the set for `value`) is a third bug.

`dom/nodes/Element-classlist.html`'s "wrong class after modification" cluster (~180 subtests × five node
types) was all of this at once; the fix moved **dom 2498 → 2739 (+241)**, crash-free, html/dom unchanged.
Implementation: `engine/js/src/dom_bindings.rs` `__mkClassList` — a `raw()` (attribute string) separate
from `read()` (deduped ordered set via `Object.create(null)` so a `__proto__` token can't corrupt the
seen-map), and `toggle` returns without `write()` on the no-op branches. [[js-engine]]

## `Range.createContextualFragment` is the fragment parser you already have, wearing a Range

`range.createContextualFragment(html)` runs the HTML fragment-parsing algorithm **in the context of the
range's start node** and returns a `DocumentFragment`. It is how sanitizers, `jQuery.parseHTML`, and every
"turn this string into nodes then insert them" idiom work — so its absence silently breaks that whole
class, and the failures land as *unhandled promise rejections* two callbacks downstream, not as a clean
"method missing".

The implementation is deliberately NOT a new parser: it reuses `innerHTML` (which is `set_inner_html`, the
same fragment parser `insertAdjacentHTML` calls) into a scratch element of the **context tag** (the start
element, with the root `<html>` element falling back to `<body>` per the algorithm's special case), then
moves the children into a `createDocumentFragment()`. One parser, two entry points.

Two spec details that are easy to miss: the `fragment` argument is **required WebIDL** — calling with zero
arguments is a `TypeError`, *not* a parse of the string `"undefined"` (distinguish via `arguments.length`,
not `html === undefined`); and the result's `nodeType` must be **11** (a fragment), not a stray wrapper
element. `domparsing/createContextualFragment.html` 2 → 34/35 (the last is `<script>` execution on
insertion, a separate capability); the area moved **149 → 182 (+33)**, crash-free. [[js-engine]]

## getComputedStyle must expose the properties the cascade ALREADY computed — undefined is a bug, not a value

`computed_style_js` built a fixed ~30-property snapshot and silently dropped several `ComputedStyle`
fields the cascade already resolves — `visibility`, `white-space`, `opacity`. `getComputedStyle(el).visibility`
returned **`undefined`**, and `getPropertyValue('white-space')` likewise. These are not new capabilities;
the values existed, they were just not surfaced to JS. Exposing them (camelCase key + the kebab entry in
the `getPropertyValue` map + the initial value for unset elements) is mechanical and additive.

**Honest note (tick 102): ratchet-NEUTRAL.** No *failing* WPT subtest read these three as undefined (the
undefined-computed clusters in `css/css-ui` are `appearance` and `caret-color`, which need new
`ComputedStyle` fields + Stylo extraction — deferred). Landed anyway, tick-97-style: it is strictly more
correct with zero regression, and real scripts read `visibility`/`opacity`/`white-space` constantly. The
lever for a *scored* win here is the properties tests actually assert on — appearance/caret-color — not
the ones that happened to be easy to expose. [[js-engine]]

## isConnected / toggleAttribute / webkitMatchesSelector — the ergonomics frameworks call hourly

Three high-usage DOM methods that were simply absent (0 refs each). `node.isConnected` = walk parents to
the top; connected iff that top is the document root (a `createElement`'d-but-unappended node is not).
`toggleAttribute(name, force)` = add-if-absent / remove-if-present, `force` pins the direction, returns
presence (records an `attributes` mutation like set/removeAttribute). `webkitMatchesSelector` = the legacy
alias for `matches`. **Method lesson (tick 107→108):** a *neutral* niche API (getClientRects) vs a
*flipping* high-usage one (isConnected/toggleAttribute, +6 dom) — target what the FAILING tests call, not
what is easy to add. [[interaction-surface]]

## The Node interface CONSTANTS were absent — and `n.nodeType === Node.ELEMENT_NODE` silently ran false

`Node.ELEMENT_NODE` (1), `TEXT_NODE` (3), `COMMENT_NODE` (8), `DOCUMENT_FRAGMENT_NODE` (11), … and the
`DOCUMENT_POSITION_*` bitmask were never defined. The failure was invisible: `n.nodeType === Node.ELEMENT_NODE`
compares a number to `undefined` → **false, silently**, so type-dispatch code took the wrong branch with no
error; and `compareDocumentPosition` threw outright. Defined all 12 node-type + 6 position constants on BOTH
`Node` and `Node.prototype` (instances inherit them), and implemented `compareDocumentPosition` in the
prelude (ancestor-chain containment + common-ancestor child order). **+146 subtests (html/dom +128)** — the
constants are referenced by a large swath of the suite. A cross-cutting primitive missing in plain sight,
found by probing *what the failing tests reference most*, not by area. [[interaction-surface]]

## DOMException legacy codes + Event phase constants — the same undefined-comparison trap as Node constants

`DOMException` defined its codes by NAME (`NotFoundError`) but not the legacy numeric constants
(`DOMException.NOT_FOUND_ERR` = 8) that `e.code === DOMException.NOT_FOUND_ERR` compares against — so the
check silently ran false. Same for `Event.AT_TARGET`/`CAPTURING_PHASE`/`BUBBLING_PHASE`. Added the 25
DOMException legacy codes + 4 Event phase constants (ctor + prototype). **Prelude-ordering gotcha:** `Event`
is created by `defEvent` in the dom_bindings prelude, NOT event_loop's — attach constants where the object
is actually defined, or they silently no-op. +7 (narrower than the Node constants' +146). [[interaction-surface]]

## The reflection GAP was the GLOBAL attributes — one "*" row beat 400 per-attribute edits (+18k)

html/dom's `IDL get … undefined` mass looked like a per-attribute grind, but the reflection *mechanism*
and per-element table (`reflect_table.rs`, ~400 attrs) were already comprehensive. The hole was the
**global HTMLElement attributes** — `dir`, `hidden`, `tabIndex`, `accessKey`, `autocapitalize`, `autofocus`,
`nonce`, `draggable`, `spellcheck`, `translate` — reflected by EVERY element but absent from the per-tag
table, so `div.dir` etc. returned `undefined`. Fix: a `"*"` row in the table + `descFor` falling back to it
(`byTag[tag] || byTag['*']`). **+18,245 html/dom subtests, crashes=0, nothing else moved.** Two lessons:
(1) probe the biggest failing cluster for its SHARED cause before editing one entry at a time; (2) the
tick-95 mass-reflector Bar-0 did NOT trip at 10 global accessors — the remaining reflection mass (ARIA +
whole-tree access) stays crash-gated on the stack-quota fix, but a large crash-free chunk was reachable
without it. [[js-engine]]

## A getter-only attribute fallback silently drops the setter — and double-defining a native one CRASHES

html/dom's `got "test-valueOf"` cluster was reflection *value* correctness: `el.lang` returned the
attribute (a generic getter fallback) but `el.lang = x` was silently dropped — no setter, because lang is
neither a named native accessor nor in the per-tag reflection table, so reflect_js never installed one.
Fix: add `lang` to the `"*"` global row → reflect_js installs a real getter+setter (+4560 html/dom).
**Two cautions banked:** (1) a getter without a setter is a silent write-drop, worse than absence; (2)
adding a reflected `title` alongside the EXISTING native `title` accessor caused a hard crash (css-grid
crashes=35) — reverted. Never define a reflected accessor over a working native one; and the mass-reflector
Bar-0 has SOME headroom (lang, the 11th global accessor, is fine) but it is finite. [[js-engine]]

## HTML attribute qualified names are ASCII-lowercased — the root of the reflection value-mismatch mass

The single biggest reflection cluster was NOT missing accessors — it was `setAttribute()`. DOM Living
Standard §Element makes `setAttribute` / `getAttribute` / `removeAttribute` / `hasAttribute` /
`toggleAttribute` **ASCII-lowercase the qualified name** when the element is HTML-namespaced in an HTML
document. We stored/looked-up the name **verbatim**. So `el.setAttribute('accessKey', v)` stored an
attribute literally named `accessKey`; then `getAttribute('accesskey')` (exact-case) missed it → `null`,
and the reflected getter `el.accessKey` (which reads the lowercase *content* name) missed it → `""`. Every
`setAttribute()` subtest for every mixed-case IDL attribute (`accessKey`, `tabIndex`, `noValidate`, …)
across the whole WPT reflection suite failed on this one line. Fix: a shared `attr_qname(dom, node, name)`
helper in `dom_bindings.rs` that lowercases the name iff the element's namespace slot is `None` (HTML) —
SVG/MathML (`Some`) keep their case so `viewBox`/`preserveAspectRatio` survive — applied at both store and
lookup in all five attribute natives. **+10,249 html/dom subtests (45,495→55,744), crashes=0, no other
area regressed.** Gate `G_ATTR_CASE`.

**Method note (banked):** the `reflection-*.html` files reported `testsCreated:0` under `diag` — this was
a **diag artifact**, not reality. Reproducing the file's own scripts in a same-directory copy (so its
relative `<script src>` resolve) ran all 8,272 of its subtests and exposed the `accessKey → ""` pattern in
the FULL run that every isolated probe had hidden. When an isolated repro passes but the aggregate fails,
**rebuild the aggregate's real environment** (its actual scripts, its real path) rather than trusting a
diagnostic's summary counter. [[conformance-and-oracles]] [[js-engine]]

## The HTMLDocument named collections — `document.forms`/`images`/`links`/`scripts`/`embeds`/`anchors` + `getElementsByName`

These seven getters plus `getElementsByName` were **all `undefined`** — not incomplete, absent. That is not
a pedantic miss: `document.forms.length` is a **`TypeError`** that kills the rest of the bundle on the load
path. Every form library and serializer enumerates `document.forms`; analytics/ad/prerender tooling walks
`document.links`/`images`/`scripts`; legacy control-resolution calls `getElementsByName`. One `undefined`
here is the [[conformance-and-oracles]] YES-then-throw class — the page renders nothing and says nothing.

Each is a **static Array** (identical shape to the already-working `getElementsByTagName`) over a shared
`doc_collection(cx, vp, selector)` helper: `query_selector_all` walks descendants once, so tree order and
de-dup are free. `getElementsByName` enumerates `"*"` and filters on the stored `name` **content
attribute** (exact string, any element type) rather than a `[name="…"]` selector — robust against values
that would need attribute-selector escaping, and it resolves at all only because tick 113 now lowercases
HTML attribute names (`name` is always keyed lowercase). Three spec subtleties, each gated:
`document.links` is `a`/`area` **with `href`** (a bare `<a name>` anchor is not a link); `document.anchors`
is `a` **with `name`**; `plugins` is a synonym for `embeds`. **+39 html/dom (55,744 → 55,783), crashes=0.**
Gate `g_doc_collections`, proven falsifiable (RED = `document.forms is undefined`). [[js-engine]]

## `lookupNamespaceURI` / `isDefaultNamespace` — the "locate a namespace" algorithm

Both were `undefined` on every node (`node.lookupNamespaceURI is not a function`, a `TypeError`). They
implement DOM §Node's "locate a namespace", which is more than a field read. The algorithm lives in the DOM
crate (`Dom::locate_namespace(node, prefix)`, direct `NodeData` match); the JS side is two thin natives on
**`Node.prototype`** so Document/Fragment/Comment/Element inherit through the chain. The four subtleties,
each of which is a separate way to get it wrong:

1. **`xml`/`xmlns` are always bound on an element and are NOT overridable.** `lookupNamespaceURI('xmlns')`
   is `XMLNS_NS` even after `setAttributeNS(XMLNS_NS,'xmlns',v)`. Checked *first*, and only in the Element
   branch — a bare DocumentFragment/DocumentType returns `null` even for `'xml'`.
2. **HTML elements store `namespace: None` but ARE in the XHTML namespace with a null prefix.** So an
   element's own namespace (xhtml) wins over an `xmlns` attribute it carries: `document.lookupNamespaceURI
   (null)` is xhtml, not the `<html>`'s `xmlns`. Mirror `namespaceURI`'s `None → xhtml` convention.
3. **"Parent element" is the parent iff it is an element** (`node.parentElement`), so a comment whose
   parent is the *document* resolves to `null` — it does not climb to the document element.
4. **The prefix arg is nullable** (`DOMString?`). `lookupNamespaceURI(null)` means "no prefix", so it must
   NOT be ToString-coerced to `"null"` — `arg_string_nullable` maps JS `null`/`undefined` → `None`.

`isDefaultNamespace(ns)` is `locate_namespace(node, None) == ns` (with `""` normalised to null). Gate
`g_namespace_lookup` ports all 27 branch cases from WPT `Node-lookupNamespaceURI.html`. `lookupPrefix` is
NOT implemented: its WPT file is `.xhtml`, gated behind XML document loading, so it would flip nothing.
[[js-engine]] [[conformance-and-oracles]]

## `nodeName` is per node type, and case-sensitive outside the HTML namespace

`el_get_node_name` uppercased the tag name **unconditionally** and returned `"#text"` for every
non-element. Both are wrong: DOM §Node makes `nodeName` **per node type**, and an element's nodeName is its
`tagName` — ASCII-uppercased **only in the HTML namespace**. So `createElementNS('http://example.com/',
'foo').nodeName` is `"foo"`, not `"FOO"` (the whole `Document-createElementNS.html` nodeName cluster), and
SVG's `linearGradient` keeps its case. The rule now lives in `Dom::node_name` (the DOM crate), mirroring the
namespace-aware casing `el_get_tag_name` already had, plus the right constant per kind: `#text` / `#comment`
/ `#document` / `#document-fragment` / the doctype's name. The getter is a thin seam over it. Gate
`g_node_name`. **The lesson: a rule duplicated across two getters drifts — `tagName` had the namespace
check, `nodeName` (which is *defined as* tagName for elements) silently did not.** [[js-engine]]

## `moveBefore` — the atomic move, and why it is stricter than `insertBefore`

`parent.moveBefore(node, child)` (WHATWG DOM) relocates a **connected** node **without** the
remove-then-insert side effects that reset the moved subtree's state — an iframe would reload, a running
CSS animation/transition would restart, focus and selection would be lost. Framework reconcilers
(React/Preact/lit) reach for it to move a subtree while preserving that state. Manuk has none of that state
to lose, so the *observable relocation* is identical to `insertBefore` (both `insert_before`/`append_child`
already `detach` the node from its old parent first — no new arena code). What the platform gains is the
method's **existence** and its **stricter pre-move validity**, the throws real code branches on:

1. **WebIDL arg coercion** — `moveBefore(Node node, Node? child)`, both required. A non-`Node` first arg, a
   missing second arg, or a non-`Node`/non-null second arg is a **`TypeError`** before any DOM step.
2. **both `parent` and `node` must be connected** — the rule that separates an atomic move from
   `insertBefore` (which happily inserts a freshly-created, disconnected node). Disconnected either side →
   `HierarchyRequestError`.
3. **same shadow-including root** — a node from another document lives in a distinct arena, so a `Dom`
   pointer compare is the cross-document check → `HierarchyRequestError`.
4. **no cycle** (`node` not an inclusive ancestor of `parent`), **valid kinds** (`node` is Element or
   CharacterData; `parent` is Document/DocumentFragment/Element) → `HierarchyRequestError`.
5. **reference child belongs to `parent`** → else `NotFoundError`.

It is defined on the flat `Node.prototype` beside `insertBefore`, so Element + Document (inherited) +
DocumentFragment all get it; Text/Comment/DocumentType inherit it too (calling it still throws — wrong
parent kind), so the four `"moveBefore" in <non-ParentNode>` presence subtests are the only ones out of
reach until the Element/Document/Fragment member tiering lands (its own tick, named in `dom_protos`).

**The latent hazard it surfaced:** `node_and_dom` reads `SLOT_NODE` **blindly**, and a plain `{a: 1}`
stores its `1` in fixed slot 0 — which `SLOT_NODE` aliases — so it was mistaken for node #1 and reached a
*validity* throw instead of the WebIDL `TypeError`. Any argument that must be a genuine Node now goes
through `is_node_reflector` (a `NODE_CLASS` class check via `mozjs::rust::get_object_class`), not a bare
slot read. Gate `g_move_before`. [[js-engine]] [[conformance-and-oracles]]

## `ProcessingInstruction` — a whole missing node type, found by histogramming failure *messages*

The single largest one-mechanism cluster in `dom/nodes` was not a wrong value — it was
`document.createProcessingInstruction is not a function`, ~88 subtests that threw before their first
assertion (plus ~40 that then died on `pi is undefined`). **The lever was invisible to a failing-*count*
histogram and obvious to a failing-*message* one** — the method simply did not exist, so every test that
minted a PI to test something else collapsed. This is the recurring shape (`[[parity-methodology]]`): the
biggest flip is often one missing primitive, not a hard bug.

**The node.** A `ProcessingInstruction` (`<?target data?>`, `nodeType` 7) is a `CharacterData` node — a
`data` body plus a `target` (its `nodeName`). It became a `NodeData::ProcessingInstruction { target, data }`
arena variant; adding the variant made the Rust compiler enumerate every match arm that had to learn it
(`character_data`, `set_character_data`, `node_name`, the debug + HTML serializers, plus a new
`is_processing_instruction`) — **exhaustive-match discipline is the safety net that makes adding a node
type a bounded, compiler-guided change rather than a hunt.**

**The factory + validity.** `document.createProcessingInstruction(target, data)` mints one after the
WHATWG "create a processing instruction" checks: `target` must be a valid XML `Name` (colons allowed —
`xml:fail` is legal), `data` must not contain the PI-close `?>`; either violation is an
`InvalidCharacterError`. `.data`/`nodeValue`/`textContent` fall out of `character_data` for free; `.target`
dispatches on the flat `Node.prototype` — a PI answers its target, every other node the `target`
**attribute** reflection — the same by-kind dispatch `content` and `data` already use.

**Two named limits.** (1) `pi instanceof ProcessingInstruction` is *false*: every node reflector shares one
flat `Node.prototype` (`NODE_CLASS`), so per-interface `instanceof` awaits the member-tiering tick. (2) The
three exotic non-ASCII invalid targets (`·A`/`×A`/`A×`) do not throw — `is_valid_xml_name` treats all
non-ASCII as valid NameChars (ASCII-precise tables only), a ~3-subtest miss not worth a Unicode table.

**The latent bug it closed.** `nodeValue` read `null` for a PI *and a Comment*: its getter knew only Text.
The spec says `nodeValue` is the character data of *every* `CharacterData` node, so it now routes through
`character_data` (Text/Comment/PI) — Comment `nodeValue` is fixed as a free correctness gain. Gate
`g_processing_instruction`. [[js-engine]] [[conformance-and-oracles]] [[parity-methodology]]

## The typed Event hierarchy — flat members over a real `instanceof` chain

Events here are **flat JS objects** minted by a prelude factory `defEvent(name, defaults, parent)` — there
is no C++ interface per event type. `Event-subclasses-constructors` demands two things of every typed
event: the **member set** (`new MouseEvent().view` must exist, inherited from UIEvent) *and* the
**`instanceof` chain** (`new MouseEvent() instanceof UIEvent instanceof Event`). Those pull in opposite
directions for a flat model:

- **Members** — because there is no accessor inheritance, the flat constructor must set *every* ancestor's
  member as an OWN property. So `defEvent` **merges** the parent's default dictionary into the child's
  before the constructor's assignment loop: `MouseEvent`'s effective defaults are its own plus UIEvent's
  `view`/`detail` plus Event's.
- **`instanceof`** — that is the ONE thing a real prototype chain is still needed for, so after building
  `g[name]`, `Object.setPrototypeOf(g[name].prototype, g[parent].prototype)`. Instances carry their methods
  as own properties (set in the constructor), so the chain is *only* consulted by `instanceof`, never for
  property lookup — which is why the flat/own-property duplication is harmless.

Define **parents first** (`Event → UIEvent → MouseEvent → WheelEvent`, etc.) so each `setPrototypeOf` sees
a defined parent prototype. WebIDL `UIEventInit.view` is `Window?`: a supplied non-null non-object is a
constructor `TypeError` (the check accepts any object as a Window — enough for the tested `{view: 7}`
rejection; a strict `instanceof Window` is not worth the branch). This is a **pure-JS-prelude** capability
— zero arena/native risk, so it cannot regress dispatch. Gate `g_event_constructors`. [[interaction-surface]]
[[js-engine]]

## Constructable node interfaces — when `iface()`'s inert constructor is the wrong default

The prelude's generic `iface(name, test)` gives every DOM interface global a constructor that is
**constructible and inert** — `function(){ return this; }` returning an empty object, with a
`Symbol.hasInstance` predicate so `instanceof` works. That is deliberately right for the interfaces the web
platform makes **un-constructable** (`new Element()`/`new Node()` throw "Illegal constructor" — an inert
stub is a gentler, framework-friendlier version of that). But three node interfaces ARE constructable and
the inert default silently breaks them: `new Text('x')`, `new Comment('x')`, `new DocumentFragment()` must
each mint a **real detached node owned by the current document**. Left inert, `new Text('x').data` was
`undefined` and `.nodeType` `undefined` — a dead object that every library building nodes via the
constructors (rather than `document.createTextNode`) silently received.

The fix delegates to the factories that already exist: after `iface()` runs, replace those three globals
with constructors that `return globalThis.document.createTextNode(...)` / `createComment(...)` /
`createDocumentFragment()` (evaluated at call time, when `document` is fully wired), re-applying the
nodeType `hasInstance` predicate so the flat-prototype node still tests `instanceof Text`. **The general
lesson: a generic "make it constructible and inert" default is correct only for the un-constructable half
of the interface list; the constructable half needs the real factory wired in.** Gate
`g_node_constructors`. [[js-engine]] [[conformance-and-oracles]]

## `Text.splitText()` and `wholeText` — the split and its inverse

`splitText(offset)` cuts a Text node in two at `offset` (UTF-16 units): the node keeps `[0, offset)`, a new
Text node takes `[offset, len)` and is inserted as the original's **next sibling**; the new node is
returned. `offset > length` is an `IndexSizeError`. `wholeText` is its inverse view — the concatenated
`data` of the maximal run of **contiguous** Text siblings containing this node (walk `prev_sibling` to the
run's start, then concatenate forward until a non-Text sibling). Both reuse the `char_units` UTF-16 helper
that the other CharacterData methods already share; both guard on the node actually being Text (the flat
`Node.prototype` means Comment/PI inherit the members but they must no-op there). **Deferred, named:** the
spec's final `splitText` step adjusts any live `Range` boundary points that fall inside the split region —
not yet modelled (Selection/Range liveness is its own surface). Gate `g_split_text`. [[js-engine]]

## `getElementsByTagNameNS` — match on (namespace, localName), and `None` means XHTML

The namespace-aware sibling of `getElementsByTagName`. It matches descendant elements by a **pair** —
`(namespace, localName)` — where `"*"` is a wildcard in *either* slot, and the local name is derived
**exactly as `element.localName` derives it**, because the two must agree: the part after the prefix for a
namespaced element (`createElementNS("test","test:body")` → local `"body"`), and the ASCII-lowercased tag
for an HTML element. So `("test","BODY")` and `("test","body")` are different queries — foreign content is
case-sensitive, HTML is folded. Implemented as `el_get_by_tag_ns` on both the Element and Document
prototypes: enumerate descendants with `query_selector_all(root, "*")` (self excluded, same as
`getElementsByTagName`), filter on the pair, and hand the static array to `collections_js`, which wraps it
into a **live `HTMLCollection`** — so `while (c.length) …` over the result terminates.

**The load-bearing subtlety is the namespace representation, and it is a deliberate, stated trade.** An
HTML element stores `namespace: None`, which is treated as the XHTML namespace for matching — this is why
`getElementsByTagNameNS("http://www.w3.org/1999/xhtml", "div")` finds the page's divs and a `null`/`""`
namespace query does **not** (those elements are not in the null namespace). But a *genuinely*
empty-string-namespace element — `createElementNS("", "x")`, which essentially no real page creates — also
stores `None`, so it is **indistinguishable from XHTML** here. That single WPT edge
(`getElementsByTagNameNS("", "*")` finding such an element) is the one query left unserved; every
real-namespace query — XHTML, SVG, MathML, a custom URI — is exact. Serving the empty-string edge needs the
full null-vs-XHTML *storage* split, which would ripple into `namespaceURI` (must answer `null` not `""`),
`tagName` casing (a null-namespace element is not uppercased), and the HTML-parser path (596
`createElementNS` subtests already green depend on `None`==XHTML) — a subsystem, not a bounded query tick.
**The general lesson: a query can be spec-correct for every case the storage can represent, and honest
about the one case it cannot — the fix for that case is a storage change, not a query change.** Gate
`g_get_by_tag_ns` (dom 3052 → 3096, +44). [[js-engine]] [[conformance-and-oracles]]

## DOM validation errors must be REAL `DOMException`s, not decorated `Error`s (tick 127, +420 dom)

A whole class of DOM validation throws — `classList.add('a b')` and `.add('')`, `createAttribute('')`,
`setAttributeNS(ns,'',v)`, `removeNamedItem`/`removeAttributeNode` on an absent attribute, `Range.setStart`
past a node's length, `compareBoundaryPoints` with a bad `how` — was implemented in JS as **`var e = new
Error(msg); e.name = 'InvalidCharacterError'; throw e;`**. That decorates the *name* but nothing else, and
it is wrong on two properties that matter far more than the name:

- **`e.code` is `undefined`.** The legacy numeric `DOMException` codes (`INVALID_CHARACTER_ERR` = 5,
  `SYNTAX_ERR` = 12, `INDEX_SIZE_ERR` = 1, …) never got set. Real code does
  `catch (e) { if (e.code === DOMException.SYNTAX_ERR) … }`; a decorated `Error` silently takes the wrong
  branch.
- **`e.constructor === Error`, not `DOMException`**, and `e instanceof DOMException` is `false`.

**Why it was a ~420-subtest lever, all behind one mechanism.** WPT's `assert_throws_dom` — used by a very
large fraction of `dom/` — does NOT just check the name. Reading `resources/testharness.js`: it builds
`required_props.code = name_code_map[name]` and asserts `'code' in e && e.code == required_props.code`
**for every throw**, then finally asserts `e.constructor === constructor` (the test realm's
`DOMException`). A decorated `Error` fails the `code` check first, so the test reports the *right name* and
still fails — e.g. classList whitespace validation alone was **360** such failures, empty-token **45**,
qualified-name **58**, namespace **5**. The histogram signature was unmistakable:
`threw object "InvalidCharacterError: …" that is not a DOMException InvalidCharacterError: property "code"
is equal to undefined, expected 5`. The word "threw" is the tell — the site *did* throw, the object was
just the wrong type.

**The fix is uniform: `throw new DOMException(message, name)`.** The engine already installs a spec-shaped
`DOMException` polyfill on the global (`event_loop.rs`) whose constructor sets `.name`, maps `.code` from
`DOM_CODES[name]`, and whose `.prototype` chains to `Error.prototype` (so `instanceof Error` still holds).
Because it is `globalThis.DOMException`, the instance's `.constructor` **is** the object the WPT test
compares against — same realm, so `e.constructor === DOMException` passes. The Rust-side `throw_dom` helper
(`dom_bindings.rs`) already did exactly this (`throw new DOMException(...)`); the gap was purely the
JS-authored throw sites in `attrs_js` / `dom_bindings` (classList) / `range_js` / (and, for `TypeError`,
`mutation_js`, converted to `new TypeError`). **dom 3096 → 3516 (47.5% → 53.9%), Bar 0 clean.**

**The general lesson:** a thrown error's *identity* (`constructor`, `instanceof`) and its *legacy code* are
load-bearing API surface, not decoration — the spec's own conformance harness checks them before it checks
the name, and real `catch` blocks branch on them. Setting only `.name` is the shape of a fix that passes an
eyeball test and every `assert_throws_dom`. Gate `g_dom_exception` (proven red: without the fix,
`code=undefined|isDE=false|ctorDE=false`). [[js-engine]] [[conformance-and-oracles]]

## `Node.lookupPrefix` and the DocumentType namespace-lookup surface (tick 128, +20 dom)

`node.lookupPrefix(namespace)` (DOM §Node "locate a namespace prefix") is the inverse of
`lookupNamespaceURI` — given a namespace URI, find the prefix that maps to it in scope at `node`. It was
registered as a native on **no** node type (its siblings `lookupNamespaceURI`/`isDefaultNamespace` were),
so every `foo.lookupPrefix(ns)` was a `TypeError` — a whole `dom/nodes` file (`Node-lookupPrefix`) plus
namespace-aware SVG/MathML/XML code and every XML serializer that must choose a prefix.

**The algorithm shares `locate_namespace`'s walk, inverted.** `Dom::lookup_prefix(node, ns)` (with `""`
normalised to `None`, which returns `None`) on an Element: (1) if the element's own namespace equals the
target and it has a non-null prefix (from `name.split_once(':')`), return that prefix; (2) scan the
element's attributes for an `xmlns:<p>` declaration whose *value* equals the target and return `<p>`;
(3) recurse to the parent element. A Document defers to its `documentElement`; a
DocumentType/DocumentFragment/ShadowRoot has none; Text/Comment/PI defer to the parent element. The
reflector seam is `el_lookup_prefix`, registered on the shared prototype beside `lookupNamespaceURI`.

**The second half is a shim gap, and its answers are constant.** A `DocumentType` is a JS shim
(`Object.create(DocumentType.prototype)`), not a reflector, so it had none of the three namespace-lookup
methods — and `dom/nodes` calls them directly on a doctype. Per spec the answers are constant, because
"locate a namespace" routes a non-Element/Document node to its parent **element** and a doctype's parent is
at most a Document: `lookupNamespaceURI`/`lookupPrefix` are always `null`, `isDefaultNamespace` is true only
for the null/empty namespace. Three constant methods on `DocumentType.prototype` close it.

**MEASURED:** dom 3516 → 3536 (+20), Bar 0 clean. Gate `g_lookup_prefix` (proven red: without the native,
the script throws at the first `lookupPrefix` and `textContent` never updates).

**The lesson, again:** a "missing method" histogram row splits into two very different fixes — a real
algorithm on real reflectors (the +11 element/text/document part) and a constant-answer stub on an exotic
JS shim (the +9 doctype part). Both are spec-required Node surface; neither is the other. [[js-engine]]

## `HTMLCollection` is a WebIDL legacy platform object, not a plain indexed proxy (tick 129)

`document.getElementsByTagName(...)` returns a **live `HTMLCollection`**, backed by a `Proxy` (see
[[js-engine]]) whose traps recompute the node list on every access. It handled indices and a bare
`namedItem`, but the object-model surface — the part `dom/collections/` checks hardest — was wrong, and it
was **one mechanism failing a whole file cluster** (the flip-rate signal): `HTMLCollection-supported-
property-names` 0/6, `-empty-name` 0/7, `-own-props` 4/8, `-supported-property-indices` 0/7, `-delete` 2/4.

**What a legacy platform object owes (WebIDL §3.9 + HTML §HTMLCollection):**
- **Supported property names** = every element's `id`, plus every **HTML-namespace** element's `name`, in
  tree order, deduped, no empty strings. (A non-HTML `name` contributes nothing — that is why
  `getElementsByTagName('foo')` over `<foo name=x>` in a random namespace has no named property.)
- `Object.getOwnPropertyNames` = `[...indices, ...supported names, ...expandos]` and **never `length`** —
  `length` is a prototype accessor, not an own property. Our old `ownKeys` pushed `'length'` and no names.
- Named properties are **`[LegacyUnenumerableNamedProperties]`**: present, `writable:false`,
  `enumerable:false`, `configurable:true`. An expando may **not** shadow a live index/named property
  (`coll["some-id"] = 5` is a silent no-op in sloppy mode, `TypeError` in strict; same for
  `defineProperty`/`delete`). But an expando set on a name **before** it becomes supported is a real own
  property that shadows the named property appearing later (named-property *visibility*).

**The receiver subtlety that bit back (the `as-prototype` regression).** Making named descriptors
`writable:false` broke `Object.create(coll).named = "foo"`: an inherited assignment consults the collection's
descriptor and a non-writable data property rejects it. But WebIDL's legacy `[[Set]]` passes
`ownDesc = undefined` when the **receiver is not the collection**, so the assignment falls through to an
ordinary own property on the receiver. The `set` trap must branch on `receiver !== proxy`. And `length` is a
branded IDL attribute — reading it on a mere inheritor is a `TypeError`, not the count.

**The heap-churn trap that made this a two-attempt tick (Bar 0).** The first cut routed **`NodeList`
(`childNodes` — the engine's hottest proxy)** through the richer traps too. It measured +19 dom, **but the
extra allocation shifted the shared-batch-runtime heap enough to surface the tracked cross-file UAF on three
unrelated `ranges`/`traversal` files (batch Bar 0 0 → 3), each of which passes in isolation.** The fix was to
gate every new behaviour on `HTMLCollection` and keep `NodeList`'s traps **byte-for-byte** on their original
bodies — zero added churn on the hot path. Batch Bar 0 returned to **0**. The lesson: on an engine with a
known heap-layout-sensitive UAF ([[js-engine]]), *a correct change that perturbs a hot allocation path can
trip Bar 0 far from where it was made* — measure the full batch, not just the subdir, and keep hot paths
allocation-neutral until the UAF is fixed.

**MEASURED:** dom 3536 → 3557 (**+21**), collections 9/48 → 30/48, Bar 0 **0** (deterministic ×3), no
regressions. Gate `g_collection_named_props` (proven red on the committed proxy). Named-property parity for
`NamedNodeMap` (`.attributes`) and `DOMStringMap` (`.dataset`) is the same shape on **different** objects —
still 0/5 and 0/3, a separate follow-on tick. [[js-engine]]

## `DOMStringMap` (`dataset`) and `NamedNodeMap` (`attributes`) enumerate their names (tick 130)

The same legacy-platform-object gap as [[dom-semantics]]'s tick-129 `HTMLCollection`, on two more proxy-backed
objects — and the completion of the `dom/collections/` cluster.

- **`el.dataset`** is a `Proxy` over `{}` with `get`/`set`/`has`/`deleteProperty` but **no
  `ownKeys`/`getOwnPropertyDescriptor`** — so `Object.getOwnPropertyNames(el.dataset)` saw the empty target
  (`[]`). Fixed: `ownKeys` = each `data-*` attribute, prefix stripped and **dash→camel-cased** via the same
  `camel()` the accessor already uses (`data-date-of-birth` → `dateOfBirth`; `data-` → `""`; `data-id-` →
  `"id-"` because the trailing hyphen has no following lowercase letter). Named props are ordinary
  enumerable/writable data properties (DOMStringMap has **no** `[LegacyUnenumerableNamedProperties]`).
- **`el.attributes`** (`NamedNodeMap`) `ownKeys` pushed indices ++ `'length'` and no names; the spec wants
  indices ++ the **attribute qualified names**, and `length` is a prototype accessor (not an own key).
  NamedNodeMap **does** have `[LegacyUnenumerableNamedProperties]`, so a named descriptor is
  `enumerable:false, writable:false` over the `Attr`.

**MEASURED:** dom 3557 → 3566 (**+9**), `domstringmap-supported-property-names` 0/5 → 5/5,
`namednodemap-supported-property-names` 0/3 → 3/3, Bar 0 **0** (deterministic ×3), no regressions. Gate
`g_dataset_attrs_enum`. Both are far colder proxies than `NodeList`, so — unlike tick 129 — routing them
through the richer traps did not perturb the tracked cross-file UAF. [[js-engine]]

## `HTMLCollection` iterable surface + numeric `namedItem` (tick 131)

Two smaller `HTMLCollection` correctness gaps left after ticks 129–130, both from the shared collection proxy:

- **HTMLCollection is not a WebIDL `iterable<>`.** It has a default `@@iterator` (the get trap yields one,
  so `for..of` works) and `item`/`namedItem`, but must NOT carry `values`/`entries`/`keys`/`forEach` — those
  are the *generated* members of `NodeList` (which IS `iterable<Node>`). The shared `methods` object exposed
  all four on both, so `"values" in coll` was wrongly true. Now `methods` is built per-type: HTMLCollection
  gets `item`+`namedItem`; NodeList gets `item`+the four iterable methods. And the `has` trap now reports
  `Symbol.iterator` (the get trap already served it, but `Symbol.iterator in coll` read false — a
  trap-consistency gap). NodeList's hot path is untouched, so no UAF perturbation.
- **`namedItem` coerces to string.** `namedItem(-2)` passed the *number* `-2` into an `id === key` compare
  against the string `"-2"` → never matched. `collection[-2]` already worked (property keys are strings);
  the method did not. `namedProp` now does `String(key)` (after the null guard, before the empty check).

**MEASURED:** dom 3566 → 3573 (**+7**), `HTMLCollection-iterator` 2/6 → 6/6, `-supported-property-indices`
2/7 → 5/7, Bar 0 **0** (deterministic ×3), no regressions. Gate `g_collection_iterator_indices`. [[js-engine]]

## `getElementsByClassName` splits on ASCII whitespace, not Unicode (tick 132)

`el.getElementsByClassName(arg)` parses `arg` (and each element's `class` attribute) as a DOM **ordered
set**, split on **ASCII whitespace only** — TAB (U+0009), LF (U+000A), FF (U+000C), CR (U+000D), SPACE
(U+0020) — and nothing else. A class of a single non-ASCII "space" character (U+00A0 no-break space, U+2003
em space, and critically U+000B LINE TABULATION, which is *not* ASCII whitespace) is a real, matchable token.

Our binding used Rust `str::split_whitespace()`, which splits on the **Unicode White_Space** property
(U+00A0, U+1680, U+2000–200A, U+2028/2029, U+202F, U+205F, U+3000, U+0085, U+000B, U+000C). Every such class
name split into empty tokens → no match → the entire `dom/nodes/getElementsByClassName-whitespace-class-names`
file (26 subtests) failed, plus getElementsByClassName-driven setup in neighbouring files.

Two fixes in one: (1) split on the five ASCII whitespace chars via an explicit `matches!` closure; (2) stop
building a `.{class}` **CSS selector string** and instead enumerate `*` and filter on the element's class
set (the pattern `getElementsByName` already uses) — a class name containing `.`/`#`/`:`/`[`/quotes/spaces is
now matched literally instead of mis-parsed as a selector.

**MEASURED:** dom 3573 → 3603 (**+30**), the whitespace file 0/26 → 26/26, Bar 0 **0** (deterministic ×3), no
regressions (a one-off `Node-lookupNamespaceURI` 69-vs-71 sample was an async TH_TIMEOUT flake, stable at
71/75 across re-runs). Gate `g_class_ascii_whitespace`. [[dom-semantics]]

## A document from `DOMImplementation` is a REAL Document (tick 134)

`document.implementation.createHTMLDocument()`/`createDocument()` returned a reflector built by
`new_reflector`, which gives EVERY node `HTMLElement.prototype` (the element member set). So the created
Document had `setAttribute` but **none of the factory surface** — `doc.createElement`/`createTextNode`/
`createComment`/`createProcessingInstruction`/`getElementById` were all `TypeError: ... is not a function`,
and every `dom/nodes` test that created a second document to test something else aborted on the first call.

**Why the earlier attempt was (correctly) feared, and what actually blocked it.** A prior comment recorded
that handing a Document node the document method set "breaks the real document — 5 WPT files stop
reporting… something is written against the page's one true document, not `this`." The culprit was the
**arena-wide `find_first`**: `documentElement`/`body`/`head`/`title` searched from `self.root` — the MAIN
document — so a SECOND document in the same arena resolved the PAGE's `<body>`, and a write through
`doc.body` mutated the real tree (and the WPT harness lives in that tree → "stopped reporting").

**The mechanism, in four parts:**
1. **`Dom::find_first_in(root, name)`** — a subtree-scoped tag search. `documentElement`/`body`/`head`/
   `title` (get + set) now scope to the `this` document node. The main document is unaffected because its
   `this` node IS `self.root`.
2. **`doc_create_html_document` builds its reflector with `Document.prototype`** (mirroring the iframe path
   `el_content_document`, which has always done this and worked) and **seeds the identity cache**, so
   `el.ownerDocument === doc` and `ownerDocument` hands back the real Document rather than a second
   element-proto object for the same node id. It also appends a `<!DOCTYPE html>` first, so
   `doc.childNodes` is `[doctype, html]` (length 2, as the spec requires).
3. **`instanceof Document` matches nodeType 9**, not `o === document`. The singleton-only predicate made
   `createHTMLDocument() instanceof Document` false — the FIRST assertion in the test.
4. **`HTMLHtmlElement`/`HTMLHeadElement`/`HTMLBodyElement`/`HTMLTitleElement` ifaces** (the structural
   elements the test asserts on) + **`compatMode` ("CSS1Compat") / `contentType` ("text/html")** constants.

**The reusable rule:** *a reflector's prototype is chosen at the ONE place it is minted; a Document node
reached through the generic `new_reflector` path is wrong, so the two callers that mint documents (iframe,
createHTMLDocument) set `Document.prototype` directly and seed the cache.* And: *any document getter that
searches the arena root silently binds to the main document — scope every one of them to `this`.*

**MEASURED:** dom 3612 → 3632 (**+20**), total 6524 → 6528 (early-aborts now run their bodies), Bar 0 **0**
(deterministic ×3), no regressions. Gate `g_created_document_is_real`. **Open follow-on:** documents from
`new DOMParser().parseFromString(...)` and `createDocument` (XML) still don't carry `Document.prototype`
(same mechanism, different mint site), and `createAttribute`/`createCDATASection`/`adoptNode` are absent on
ALL documents — each a separate bounded flip. [[js-engine]]

## The `CharacterData` abstract base interface (tick 133)

`CharacterData` is the WebIDL base of `Text` (nodeType 3), `Comment` (8), `ProcessingInstruction` (7) and
`CDATASection` (4). The engine installed `Text`/`Comment`/`ProcessingInstruction` (ticks 120–122) but never
`CharacterData` — so `node instanceof CharacterData` threw a `ReferenceError`. `dom/nodes/Document-create*`
(and CharacterData-mutation tests) assert `c instanceof CharacterData` as their FIRST check, so its absence
aborted the whole subtest before the (already-correct) `data`/`nodeType`/`nodeName`/`childNodes` assertions.

Fix: one `iface('CharacterData', o => nodeType ∈ {3,8,7,4})` line, using the existing `iface()` machinery —
`instanceof` resolves through `Symbol.hasInstance`, so a nodeType predicate is sufficient (no prototype-chain
rewiring needed for these tests).

**MEASURED:** dom 3603 → 3612 (**+9**), `Document-createTextNode` 0/6 → 6/6, Bar 0 **0** (deterministic ×3),
no regressions. Gate `g_characterdata_iface`. **Open follow-on:** `Document-createComment` stays 0/6 in the
*batch* despite an isolated probe showing Comment nodes fully correct (instanceof/nodeName/nodeType/data all
pass) — a Comment-specific shared-runtime-reuse artifact, pre-existing and unrelated to this fix. [[js-engine]]

## `createDocumentType` name validation + per-document `.implementation` (tick 135)

Two DOMImplementation bugs, one shared corner of the object model, both surfaced by histogramming
`dom/nodes --show-failures` for the biggest same-signature cluster after tick 134.

**A DOCTYPE name is NOT a QName.** `createDocumentType(name, publicId, systemId)`'s argument is a *doctype
name*, and the current DOM spec's `#valid-doctype-name` rule is deliberately tiny: **a string is a valid
doctype name iff it contains no ASCII whitespace, no U+0000 NULL, and no U+003E `>`.** The empty string is
valid; `1foo`, `@foo`, `prefix::local`, `:foo`, `foo:` are all valid. The old code applied the *QName*
production (letter-start, single colon, non-empty prefix/local) and threw `InvalidCharacterError`/
`NamespaceError` for all of those — the exact opposite of the spec. The lesson: **the DOM has several
name-validity productions (QName, Name, doctype-name, custom-element-name) and they are NOT
interchangeable — match the one the algorithm actually cites.** (Verified against Ladybird's
`is_valid_doctype_name`: `!name.contains_any_of({\t,\n,\f,\r,space,\0,>})`.)

**`.implementation` is per-document, not a singleton.** A single `g.__DOMImplementation` closed over the
top-level `document` meant (a) created documents had **no `.implementation`** at all and (b) any doctype it
minted was owned by the *main* document, not the caller's. WPT's `createDocumentType` calls
`createdDoc.implementation.createDocumentType(...)` and asserts `doctype.ownerDocument === createdDoc`, so
the implementation must bind to *its* document. Fix: a `__makeImpl(ownerDoc)` factory + an `implementation`
**getter on `Document.prototype`** (shared by main/created/iframe docs since tick 134) that mints and caches
a per-document impl in a non-enumerable expando. The general pattern: *a DOM object exposed by every
document must be defined on the shared prototype and bound to `this`, never a global closed over the one
true document* — the same lesson tick 134 learned for `body`/`title`/`documentElement`.

**MEASURED:** dom 3632 → 3822 (**+190**), entirely in `dom/nodes` (2990 → 3180) with every other subdirectory
byte-identical; `createDocumentType … should work` and `implementation is undefined` both 0 remaining. Bar 0
**0** (deterministic ×2). Rate dipped 55.6% → 54.9% (denominator +432) as previously-aborting files now run
their full bodies — exposure, not regression. Gate `g_dom_impl` (extended, +11 claims). **Open follow-on:**
`createDocument` (XML) still ignores its namespace/qualifiedName/doctype args and returns an HTML document —
the XMLDocument surface (lowercase tags, `application/xhtml+xml`, namespaced root) is a separate bounded
tick. [[js-engine]]

## CharacterData offsets are `unsigned long` = ToUint32, not clamp-to-0 (tick 136)

`substringData`/`appendData`/`insertData`/`deleteData`/`replaceData` take **WebIDL `unsigned long`** offset
and count arguments, and the coercion is **ECMAScript `ToUint32` (§7.1.7): modular, NOT clamped.** The whole
CharacterData bounds behaviour hangs off this one distinction:

- `-1` does not become `0` — it becomes **`4294967295`**. So `deleteData(-1, 10)` has an offset past the
  end and is an **`IndexSizeError`**, and `substringData(-1, 0)` throws too.
- A large negative *wraps back in bounds*: `insertData(-0x100000000 + 2, "X")` → offset `2` → `"teXst"`,
  and `substringData(0x100000000 + 1, 1)` → offset `1` → `"e"`. WPT tests exactly these wrap values.
- A giant count *clamps to the remaining length* (`substringData(0, -1)` → count `4294967295` → `"test"`),
  because the spec's step is `if offset + count > length, set count to length − offset` — the count is
  ToUint32'd *first*, then clamped by the algorithm, never by the coercion.

The bug was `arg_u32`: it did `to_int32().max(0)` / `d < 0.0 → 0`, silently turning every out-of-range or
negative call into an in-bounds no-op — the failure that hides because the method *appears* to work. The fix
is one helper: `int32 as u32` (two's-complement bit pattern) and `d.trunc().rem_euclid(2^32)` for doubles.
`arg_u32`'s only callers are these five methods plus `splitText`, all `unsigned long`, so the correction is
contained to `dom/nodes`.

Two smaller sibling bugs in the same cluster: (1) **required arguments are a `TypeError` before any DOM
step** — `node.appendData()` / `node.substringData()` must throw (WebIDL "not enough arguments"), not append
`""` / return from offset 0; the fix is an `argc < N` guard. (2) **`data` is `[LegacyNullToEmptyString]
DOMString`** — `node.data = null` sets `""`, not the literal `"null"` a bare ToString produces (but
`data = undefined` *does* stringify to `"undefined"`, and `data = 0` to `"0"` — only *null* is special).

**MEASURED:** dom/nodes 3212 → 3245 (**+33**), zero new failures (before/after FAIL sets diffed), Bar 0
**0** (deterministic). Gate `g_chardata` (extended). **Open follow-on:** the 8 remaining CharacterData
failures are all *"splitting surrogate pairs"* — reading/writing a lone surrogate through `substringData`
etc. That is **structurally gated on the text-storage layer**: the DOM stores `data` as a UTF-8 Rust
`String`, which cannot represent a lone surrogate, and `from_utf16_lossy` replaces it with U+FFFD. Fixing it
needs WTF-8 / UTF-16 text storage plus a `JS_NewUCStringCopyN` return path — a subsystem, not a bounded
tick. [[js-engine]]

## `<details>`/`<summary>` — the disclosure widget is entirely the UA's job (tick 216)

`details` and `summary` appeared **nowhere** in the engine. Two consequences, the first worse than it
sounds:

- Every collapsible on the web rendered **permanently expanded** — GitHub's folded diffs and
  collapsed review threads, MDN's collapsible sections, every docs FAQ. A page of collapsed sections
  becomes a wall of everything at once and the summary stops meaning anything.
- Clicking the summary did **nothing**, so a section could never be opened *or* closed. For an agent
  driving the page, "click Show more" was unactionable.

There is no script behind any of this: the browser is the entire implementation.

**Rendering** follows the `<dialog>` precedent exactly — a UA rule pair, mirrored in both cascades:

- Stylo (shipping): `summary { display: block } · details > *:not(summary) { display: none } ·
  details[open] > * { display: block }` in `UA_CSS`.
- `MinimalCascade`: `summary` gets `Block` in `apply_ua_defaults`, and the collapse lives in
  **`cascade_node`** — it needs the PARENT's `open` attribute, which a per-element function cannot
  see. ⚠ Lockstep is by convention; `G_DETAILS` exercises the Stylo path only.

**Toggling** is *activation behaviour* on `<summary>`, in `dispatch_click`: it runs AFTER the click
event and only if nothing cancelled it, so `preventDefault()` keeps the section shut (how a page
implements its own animated disclosure). Then `toggle` is dispatched on the `<details>`, after the
attribute changes, so a handler reading `details.open` sees the new state.

`summary_details_target` **walks up** from the clicked node. This is load-bearing: a click lands on
whatever is under the cursor — a text node's element, a `<span>`, an `<svg>` chevron — essentially
never on the `<summary>` box itself. Matching only an exact hit works in a test and fails on every
real page, because real summaries have markup inside them. Only the **first** `summary` child of a
`details` toggles it; a second one is ordinary content.

### The bug underneath it — `remove_attr` never marked the tree dirty

Found by the closing half of the gate, and it is **not** specific to `<details>`: `set_attr` called
`mark_dirty` and `remove_attr` did not. So *unsetting* any boolean content attribute — `open`,
`checked`, `hidden`, `disabled` — changed the DOM and never triggered a restyle.

The asymmetry is invisible in one direction, which is why it survived: **things could always be
turned ON and never back OFF.** A closing `<details>`, an unchecking box and an un-hiding `hidden`
all render stale until something else in the page happens to dirty the tree — so it presents as an
intermittent "sometimes the UI doesn't update", not as a reproducible bug.

Held by `G_DETAILS` (`engine/page/tests/g_details.rs`), whose four assertions falsify **three
independent mechanisms**: the UA collapse rule (closed body renders), the summary toggle (first click
does nothing), and `remove_attr`'s dirty marking (second click does not close). It also pins
`details[open]` rendering its body — without that, "details never renders children" would pass the
closed-case check while making the element useless.

## A missing property is not neutral — it picks a side, and `document.hidden` picked the wrong one

`document.visibilityState` and `document.hidden` did not exist (tick 244, `G_VISIBILITY`). The
tempting reading is that the page simply "could not check", and would therefore be conservative.
The opposite happened, and the mechanism generalises well beyond this property.

The idiom on the real web is:

```js
function frame() { if (document.hidden) return; draw(); requestAnimationFrame(frame); }
```

**`undefined` is falsy.** So the guard did not fail closed and it did not throw — it evaluated,
cleanly, to *"the tab is in front"*, forever. Every animation loop, poll, autoplay decision and
analytics heartbeat on every page kept running in a backgrounded tab: the exact CPU and battery cost
the API was added to prevent, arrived at by the API's own absence, with nothing in any log.

**The general form: an absent boolean-ish property does not abstain from the branch, it votes.** It
votes `false`, and whether that is the safe answer is pure luck of how the spec named the property.
Had the platform named it `document.visible` instead of `document.hidden`, the identical absence
would have paused every animation in every foreground tab — loudly, and fixed in a day. `hidden` is
the spelling that fails *quietly*, which is why it survived two hundred ticks.

So when adding a property whose consumers are `if (x)` guards, **ask which way `undefined` votes**
before deciding the absence is harmless.

### Whose fact is it? The host owns visibility, the same way it owns the lifecycle

`Page::set_visibility(hidden)` pushes the state in, exactly as `fire_lifecycle` pushes
`DOMContentLoaded` and `load`. The reason is identical and worth stating once: *"this tab was
backgrounded"* is a fact about the **shell's window**, not about the document. No amount of
introspection inside the JS realm can discover it, so a self-answering shim would necessarily be a
constant — and a constant is an infinite loop for any code that waits for it to change (L80 above).

It is **idempotent by value**: setting the state we are already in fires nothing, because
`visibilitychange` asserts that it *changed*. A shell republishing its state each frame would
otherwise flood every listener on the page with events that changed nothing.

## Two answers to the same question must agree — `permissions.query` vs `Notification.permission`

`navigator.permissions.query()` was absent (tick 244, same gate). Restoring it is not interesting;
**what it must say is.**

A permission state is a fact the platform already exposes twice. `Notification.permission` has read
`'denied'` here for many ticks. A caller that asks `permissions.query({name:'notifications'})` is
very often not trying to learn the answer — it already has it — but to check whether the two
**agree**. Headless Chrome historically answered `'prompt'` to the first and `'denied'` to the
second, and that internal contradiction, not either value alone, is what made it identifiable.

The rule that follows is a correctness rule and not a defensive one: **a browser is allowed to be
unusual and is not allowed to disagree with itself.** So the notifications state is *read off*
`Notification.permission` at query time rather than duplicated as a literal — two constants in two
files agree only until someone edits one of them, and the gate that would catch the drift is the
gate nobody writes.

The second half is the value itself. Everything unimplemented answers `'denied'`, never `'prompt'`:

* `'denied'` makes the page take its no-permission path immediately — a real path, exercised on the
  real web, that works.
* `'prompt'` makes the page put up permission UI and **wait for a decision nothing here can
  deliver**. That is a hang dressed as a feature, and it is worse than the `TypeError` the absent
  property used to throw, because the `TypeError` at least said something.

And an unrecognised name must **reject** with a `TypeError` rather than throw synchronously: the
spec's shape is a Promise on every path, and a synchronous throw is visible to any caller that only
wrote a `.catch`.

## The Sanitizer API — `Element.setHTML` / `setHTMLUnsafe` (tick 288)

`el.innerHTML = untrusted` is an XSS hole; `el.setHTML(untrusted)` is the platform's own replacement
for DOMPurify — the safe way to inject a comment body, a CMS-authored field, or pasted rich text. It
parses the string like `innerHTML` **and then removes the parts that turn markup into code**. It was
absent, so a page that reached for it got `el.setHTML is not a function`.

Two methods, installed as **native per-reflector methods** beside `insertAdjacentHTML` — *not* on
`Element.prototype`, which the reflector does not consult (the same lesson the CSSOM `.sheet` shim
taught). `setHTMLUnsafe(html)` is the explicit opt-out: identical to the `innerHTML` setter here (the
only other thing it adds is declarative-shadow-root parsing, which we do not model yet), and the
`Unsafe` in the name is the contract. `setHTML(html)` runs `sanitize_subtree` over the freshly-parsed
children and strips exactly three things:

- **`<script>` elements** — removed entirely; a sanitized fragment whose script still ran would defeat
  the point of choosing `setHTML` over `innerHTML`.
- **event-handler content attributes** — any `on*` (`onclick`, `onerror`, …), because
  `<img src=x onerror=alert(1)>` is the canonical payload.
- **`javascript:` URLs** in the navigational/loading attributes (`href`/`src`/`action`/`formaction`/
  `xlink:href`/`srcdoc`/`background`).

It is deliberately conservative — it only ever REMOVES, never rewrites, so it cannot introduce a value
the page did not author, and ordinary markup (`<b>`, text, a normal `href`) is preserved untouched.

### The teeth `G_SANITIZER` uses

`script-gone` / `handler-gone` / `jsurl-gone` prove the strip actually happens (a stub that aliases
`setHTML` to `innerHTML` fails all three); `safe-kept` proves it is not delete-everything; and
`unsafe-keeps-script` proves `setHTMLUnsafe` genuinely keeps the `<script>` — the two paths differ.
Commenting out the `sanitize_subtree` call was demonstrated to flip the three `*-gone` claims red
before the tick landed.

**Honest limit:** the safe baseline only. The full configurable Sanitizer (`options` allow/block/drop
lists, a `Sanitizer` config object, `Document.parseHTMLUnsafe`) is the follow-on, and declarative
shadow roots are not parsed — so the constellation row stays `partial`, not `works`. [[js-engine]]

## `Element.checkVisibility()` — is it actually rendered? (tick 291)

Every UI library reinvents the same guard before it scrolls an element into view, lazily mounts it, or
reports it to an a11y layer: "is this thing actually on screen?" The manual version is a tangle of
`getComputedStyle`, `offsetParent` and an ancestor walk. `element.checkVisibility([options])` is the one
call that answers it — and it was absent, so the call threw `checkVisibility is not a function`.

Installed as a **native per-reflector method** (like `setHTML`) — NOT on `Element.prototype` in a
prelude, because `Element` is created lazily on the first element reflector and does not yet exist when
the window prelude runs (`AbortSignal.any` hit the same ordering wall from the other direction). The
default returns `false` only when the element is disconnected or `display:none` anywhere up the ancestor
chain — the two ways an element leaves the box tree. The walk is essential: a descendant of a
`display:none` element keeps its own computed `display`, so reading self is not enough. The option flags
`visibilityProperty` / `opacityProperty` (and their `checkVisibilityCSS` / `checkOpacity` aliases)
additionally fold in `visibility:hidden|collapse` and `opacity:0`, read off the element itself since
`visibility` is inherited and `opacity` resolves down the chain.

### The teeth `G_CHECK_VISIBILITY` uses

`shown`/`none`/`child-of-none` (display:none, self OR ancestor), `vis-default` + `vis-opt`
(visibility:hidden is visible by DEFAULT, hidden only with the option), `op-default` + `op-opt` (same
for opacity:0). A stub returning a constant fails several at once. Un-registering the method was
demonstrated to make the first call throw before the tick landed.

**Honest limit:** `contentVisibilityAuto` is not modelled (no `content-visibility` layout containment
in the engine). [[js-engine]]
