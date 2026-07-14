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
