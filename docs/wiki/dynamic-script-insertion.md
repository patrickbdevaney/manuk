# A `<script>` inserted by script — the path that ran nothing

*Landed tick 1397. Gate: `engine/page/tests/g_dynamic_script_insertion.rs`.*

`document.createElement('script')` + `appendChild` is how **every** analytics tag, ad tag, A/B
framework, payment SDK and lazily-loaded widget on the web boots. In this engine it did nothing at
all, silently — a page whose loader injected the real application script rendered an empty shell.

## The finding that nearly went the wrong way

`html/semantics/scripting-1` was 657/1823 with **399 failures sharing one assertion message**, and the
failing test names read like a legacy-MIME list:

```text
  Script should run with type="text/livescript"
  Script should run with type="text/jscript"
  Script should run with type="application/ecmascript"
```

The obvious tick was a lookup table. What stopped it was the **shape** of the failing set: 193
distinct type values, **every one a "should run", and not one "should not run"**.

> ⭐⭐⭐ A missing table entry produces failures in **both** directions as the list drifts. A path that
> runs *nothing* produces exactly this — all positives fail, all negatives pass vacuously.
>
> **A signature histogram names the TESTS; only a probe names the MECHANISM.** Reading the test file
> settled it in one line: every one of them does `createElement('script')`, sets `textContent`,
> appends, and asserts **synchronously**. The MIME type was their parameter space, not their subject.

## The rules, each Chrome-measured in its own probe

| | Chrome |
|---|---|
| `appendChild` into the document | runs **synchronously** — true on the very next line |
| appended to a **detached** parent | does not run |
| …then connecting that parent | **runs then** |
| re-appending an already-run script | does not run again |
| `.textContent` set after insertion | runs |
| `innerHTML` | **never** runs |
| `text/javascript;charset=utf-8` | does not run |

**The trigger is becoming connected, not `appendChild`.**

## Hook the choke point, not the method

There are nine insertion natives — `appendChild`, `insertBefore`, `replaceChild`, `append`,
`prepend`, `before`, `after`, `replaceWith`, `insertAdjacentElement` — and a tenth would be added
without anyone remembering this rule.

The hook hangs off `record_mutation` instead: the one call site every insertion already makes, because
**MutationObserver has to be complete by construction**. The observer's answer to *"what just got
connected"* and this one cannot drift apart.

## ⚠⚠⚠ The first implementation was a Bar 0

```text
  html/semantics                    HANG/CRASH 0  ->  HANG/CRASH 1
  tabular-data/…/span-limits.html   HANG
```

It walked the mutation's `added` list and every descendant, asking each node whether it was a script —
O(nodes inserted) on **every** childList mutation. That test inserts **65,532 `<tr><td>` rows in a
single `innerHTML +=`**.

> ⭐⭐⭐ **Iterate the small set, not the large one.** The loop was inverted: walk the set of
> script-created `<script>` elements that have not yet run — usually empty, always a handful — instead
> of the inserted subtree. Cost per mutation became O(pending scripts), independent of how much was
> inserted, and `HANG/CRASH` returned to 0 **with the subtest count going up**.

A capability bought with a hang is refused. The inversion also got the semantics right for free:
re-checking every pending script after any mutation *is* the "did anything become connected" question,
so a script connected because its **detached parent** was appended is found without noticing that
specifically.

## The eligibility flag is positive, and the direction is the argument

The spec states the rule negatively: the HTML **fragment parsing** algorithm marks script elements
"already started" so they never run. Implemented that way here, it would mean marking at all **seven**
`set_inner_html` call sites.

The two designs fail in opposite directions:

* a missed mark in the negative design makes **`innerHTML` execute a script** — the one thing
  `innerHTML` must never do, and a load-bearing security invariant;
* a missed case in the positive design merely leaves a script not running.

**Fail-safe wins**, and it needs one marking site rather than seven a future caller can skip.

⚠ Known gap, measured and named: `cloneNode` of a parser-created `<script>` runs in Chrome and does
not here. (`<template>` content is `false` in Chrome even when cloned, so that half agrees.)

## One predicate, both callers

The parser path carried its own two-entry list (`text/javascript`, `application/javascript`), so a
**parser-inserted** `<script type="application/ecmascript">` was dropped on the floor exactly like a
script-inserted one. Both now call one predicate.

⭐⭐ `text/javascript;charset=utf-8` does **not** run, and that row is what makes this an **essence**
match rather than a prefix test — `starts_with` passes every other row and is wrong about precisely
the one authors get wrong.

## The third inert guard in one arc

A separate "already ran" set was written first; the mutation deleting its check stayed **green**.
Removing a script from the pending set when it runs already *is* the already-started flag — one fact,
one place — so the second table asserted a rule it did not implement.

Deleted, not kept as belt-and-braces: **an inert guard makes a rule look enforced in two places when
it is enforced in one.**

## Not built yet

* the **external** half — a script-inserted `<script src>` needs a fetch, so it belongs to the host
  drain and is not synchronous;
* dynamic `type="module"` (deferred, so it never runs synchronously);
* `cloneNode` eligibility.
