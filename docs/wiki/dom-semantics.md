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
