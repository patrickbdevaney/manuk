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
