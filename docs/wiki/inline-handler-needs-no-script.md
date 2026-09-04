# A document whose only JavaScript is an attribute got no JavaScript at all

> Landed t1412. Gate: `an_inline_handler_runs_on_a_document_with_no_script_element`
> (`engine/page/tests/g_inline_handler_without_script.rs`), 4 arms, red under 3 mutations.

The JS context was stood up only for documents containing a `<script>` **element**, under a comment
ending:

> *"With no initial script, no listener can ever be registered, so there is nothing to lose."*

**That sentence is false.** An inline event-handler attribute IS a listener registration and needs no
`<script>` element:

```text
  <body onload="…">                                how the CSS-WG's own layout tests bootstrap
  <button onclick> <a onclick> <form onsubmit>     ordinary legacy markup
  <img onerror="this.src='fallback.png'">          a rendering consequence
```

## ⭐⭐⭐ The empty script is the whole proof

```text
  <body onload="…">  on a script-free document          DID NOT RUN
  the same document plus an EMPTY <script></script>     RAN
  <div onclick> on a script-free document               DID NOT RUN   (never load-specific)
```

An empty script adds no behaviour, so it cannot be what fixed it — the only thing it changed is
whether a context existed. **When a no-op addition fixes something, it names the missing precondition.**

The predicate now also asks whether any element carries an `on…` attribute, using the same test
`dom_bindings::inline_handler_nodes` uses to FIND those handlers — so the decision to build a context
and the decision to wire the handlers cannot disagree.

## Priced, small, and said so

**0 of 53** freshly-fetched CrUX pages carry an inline handler with no `<script>`, against **27 of
400** sampled WPT `css/` files. A correctness tick with ~0 corpus weight. It lands because a comment
asserting something false is a defect in its own right, and because **an optimisation that silently
removes a capability is the one trade the ratchet exists to refuse** — not because a number moved.

## How it was found, and two things found first

Constitution check #134 steered to Track A. Before the defect, the survey found:

* **The board's Track A list is stale in both headline items.** `writing-mode / logical geometry`
  (*"UNIMPLEMENTED — the biggest single unlock"*) is built — `engine/layout/src/writing_mode.rs`, with
  orthogonal roots and `transpose_in_place`. `FUZZY reftest scoring` is built — `parse_fuzzy`,
  per-CHANNEL `maxDifference`, the test's own declared allowance. **A tick that trusted the board would
  have re-implemented one of them.**
* **A measured ranking to replace it**, `css/css-grid` by subdirectory: `alignment` 880 failing /
  42.0% · `grid-items` 700 / 22.3% (the lowest pass rate) · `grid-lanes/items` 0/538 · `parsing` 426 /
  73.3% · `layout-algorithm` 305 / 42.2%. ⚠ `grid-lanes/*` is Grid Level 3 masonry — several dirs at
  exactly 0.0%, the spec-frontier signature, not a mechanism. ⚠ Much of `alignment` is REFTESTS, scored
  by the pixel runner, so the 880 is not 880 assertions to fix.

`grid-items-minimum-width-001.html` was the file that walked into the defect: `testsCreated: 0`,
`errors: []`, `loadFired: true` — it bootstraps with `<body onload="checkLayout('.grid')">`.

⚠ **An arm written, measured, and moved out:** `<img onerror>` still fails after this fix *and* fails
with a `<script>` present, so it is a different mechanism — the `error` event for a failed image
fetch. Asserting it here would have made this gate red for a reason it does not own.
