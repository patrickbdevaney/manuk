# DOM SEMANTICS — spec behaviour, mutation, and tree edge cases

## I3 IS SATISFIED FOR FREE WHEN A FIX MOVES A BOX, AND NOT AT ALL WHEN IT ADDS TEXT (t1097)

Generated content is **not in the DOM by construction** — script must never see it — and
`manuk-layout` materialises it straight into inline items. The accessibility tree is built from the
DOM:

```text
   engine/a11y/src/lib.rs   references to ComputedStyle / manuk_css:   ZERO
   accessible_name(dom: &Dom, node, role)  →  dom.text_content(n)
   build_tree_with_geometry(dom, rects, z_index)   — geometry is threaded, style is not
```

So there is **no path** by which a `::before` reaches an accessible name. This is not a bug in the
name computation: the input never arrives.

accname §4.3 step 2F folds `::before`/`::after` content into name-from-content, and Chrome does it.
`button::before{content:"★ "}` is announced *"★ Save"*; ours is *"Save"*. It is worst where the
pseudo carries the **only** text — `a::after{content:" (opens in a new tab)"}`, a `counter(sec)`
section number, an icon-font glyph that IS the label. `accname/name` is 300/464 (64.7%), 15 of its
files exercising pseudo content.

### Why four consecutive constitution checks recorded I3 as "satisfied by accident of scope"

Element geometry flows through **one shared producer**, `LayoutBox::node_rects`, which feeds the AX
tree's `bbox` and therefore the agent's click point. Any fix that moves a box moves the semantic
model with it, for free — which is why checks #72, #100, #101 and #102 each found I3 intact without
anyone having threaded anything.

> **The shared producer is geometric.** A fix that changes what the page *says* rather than where it
> *sits* has to be threaded to the AX tree deliberately, and nothing in the loop notices when it is
> not — the accident looks exactly like the property until a text-adding tick arrives.

t1092, t1093 and t1096 (generated-box `display`, `display:none`, CSS counters) are three such ticks
in one window, all landed with no semantic-model exposure at all.

**The fix shape, priced:** `build_tree_with_*` already layers optional maps — `rects`, then
`z_index`, then `visibility`. Generated text is a fourth of exactly that shape,
`HashMap<NodeId, (String, String)>` for before/after, produced where layout already resolves the
counters. The layering pattern exists; nothing about this is architectural.

### LANDED at t1098 — and the AX tree immediately falsified a claim the pixel probe had certified

`manuk_layout::generated_text(dom, styles)` produces the rendered pseudo text; `manuk_a11y::
build_tree_generated{,_with_focus}` consumes it; name-from-content becomes `before + content +
after` per accname §4.3 step 2F. Counters resolve through `manuk_layout::counter_snapshots` — the
**same walk the painter uses**, extracted to a free function precisely so the announced section
number cannot drift from the printed one.

⚠⚠⚠ **The first thing it did was catch a real bug in the tick before it.** The end-to-end test read
`S0. Alpha` where Chrome says `S1.`: element-level `counter-increment` had never been mapped, because
t1096 set it inside the *pseudo* mapper, which early-returns unless the pseudo has `content`.

```text
                    Chrome     t1096 shipped     t1098
   the painted box    87            87             87      ← IDENTICAL, and that is the problem
   the announced text "S1."       "S0."          "S1."
```

> **A fixture measures what it measures.** In a monospace fixture `S0.` and `S1.` are the same number
> of characters, so t1096's "Chrome-exact 87 / 77 / 87" was true and did not mean what it said. The
> AX tree is the first instrument in this loop that reads the STRING rather than measuring it.
> `css/CSS2` 3,843 → 3,854 once the element mapper was fixed.

**Two entry points, and one of them is the one that matters.** `Page` builds its AX tree with and
without a known focus; `a11y_tree_with_focus` is what the shell and the agent's observation channel
use. Wiring only `a11y_tree` looks complete in the diff and leaves every focus-carrying caller
announcing pseudo-less names.

**Two tests, because a mutation came back green.** Replacing the producer with `&Default::default()`
compiles and leaves all 19 `manuk-a11y` tests passing — the unit test injects its own map, so it
proves the consumer and is structurally blind to the wiring. `engine/page/tests/g_ax_generated_name.rs`
rides the real cascade and layout. ⚠ The wall runs **no** a11y page gate, so that half is not in the
tick path.

**The gate that would have caught it does not exist**: *an accessible name must include its
`::before`/`::after` content.* There is no such assertion anywhere in the tree.

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

## `baseURI` did not exist ANYWHERE, and `URL`/`documentURI` were OWN properties of one object (t1168)

Probed on a page loaded from `https://dp.test/dir/page.html`:

```text
                                       BEFORE                            AFTER
  document.URL          CONTROL        "https://dp.test/dir/page.html"   same   ✓
  document.baseURI                      undefined                        ✓ ✗→✓
  DOMParser doc .URL / .documentURI     undefined                        ✓ ✗→✓
  DOMParser doc .baseURI                undefined                        ✓ ✗→✓
```

**The CONTROL row localises it.** `document.URL` worked, so this was never "URLs are broken":
`URL` and `documentURI` were defined as **own properties of `g.document`**, so every document that
is not the window's had none — and `baseURI` is a **`Node`** property that was never built at all.

⚠⚠ **THE ABSENCE WAS ALREADY WRITTEN DOWN AND ROUTED AROUND.** `reflect_js.rs` carries
`new URL(raw, document.baseURI || location.href)`. The `|| location.href` half *is* this gap,
shipped as a fallback rather than filed as a bug. **A work-around in the tree is a bug report nobody
wrote.**

**Placement:** `URL`/`documentURI` on `Document.prototype`, `baseURI` on `Node.prototype` — the same
placement `defaultView` uses. ⚠ The own-properties on `g.document` still **shadow** these for the
main document, deliberately: they are accessors onto the live `g.location`, which `__applyUrl`
replaces wholesale on every SPA `pushState`, so a prototype getter that won instead would go stale on
the first navigation. `g_document_url_base` pins that with a `pushState` row.

**What the values ARE.** A DOMParser document's URL is the **responsible document's** URL (DOM
§DOMParser) — not `about:blank`, not empty. `baseURI` is the node's OWN document's `<base href>`
resolved against that URL, or the URL itself when there is none (HTML §2.4.1) — read from
`this.ownerDocument`, so a node in a DOMParser document does not inherit the window's `<base>`.

**Measured:** `domparsing` **149 → 190** (+41) and `dom` +4. That +41 is what crossed the **188**
ratchet mark which had been holding the entire `WPT-AREAS.tsv` refresh since Jul 16.

⚠ **One gate row was VACUOUS on the first draft**: `d.URL === d.documentURI` compares two
`undefined`s and stays GREEN with the whole prelude severed. It now also requires the value to be
non-empty. Found by reading **every row** of the RED-proof rather than just its verdict — which is
the only way a vacuous row ever shows itself.

## …and the SAME GAP existed one level down: an `<iframe>` fired no `load` either (t1167)

The lifecycle section above is about the DOCUMENT's `load`. **An `<iframe>` ELEMENT has its own, and
it did not fire.** `contentDocument` has been populated and readable since t512 — the frame really is
there, and really is the right document — but nothing was ever dispatched on the element, so
`<iframe onload=…>`, `frame.addEventListener('load', …)` and every `loadPromise` built on them
silently never ran.

⚠⚠⚠ **THE TWO CONTROL ROWS ARE WHAT MAKE THIS A MISSING EVENT RATHER THAN A MISSING FRAME**, and the
probe was deliberately split one property per file so the pass count names the failure:

```text
                                         BEFORE   AFTER
  contentDocument non-null                PASS    PASS   <- CONTROL: the frame really is loaded
  contentDocument has the child's text    PASS    PASS   <- CONTROL: and it is the right document
  INLINE onload= fired                    FAIL    PASS
  addEventListener('load') fired          FAIL    PASS
  the onload PROPERTY is a function       FAIL    FAIL   <- a DIFFERENT mechanism, see below
```

Without the controls the same symptom reads as *"iframes don't work"*, which is false and sends the
fix into the loader instead of the event path.

**Where it fires: `render_iframe`, and only there.** That is the one place a child document is
installed — the fetched path (`fetch_and_load_iframes`), the network-free path
(`load_inline_frames`: `srcdoc`, `about:blank`, a bare `<iframe>`) and every direct caller all arrive
through it. The first draft dispatched from the two *callers* instead, which is the
`set_root_box` lesson (*"three call sites feeding one post-step is how a pass silently does not run
on the third"*) being re-learned rather than applied.

**Why it is worth a tick:** `<iframe onload>` is how an ad slot, an embed, a payment frame, an OAuth
frame and every lazy widget on the web announce readiness. Measured against a same-hour old binary:

```text
   WPT dom          4004/7193  ->  6366/10503   +2362 passes, +3310 ATTEMPTED
   WPT html/dom    56440/59922 -> 56441/59922   +1
   WPT domparsing    149/1273  ->   149/1293    +20 attempted, pass FLAT
```

⚠⚠ **THE ATTEMPTED TOTAL MOVING IS THE MEASUREMENT, NOT AN ARTEFACT.** A testharness file emits
subtests as it gets through them, so a file whose `loadPromise` never settles emits almost none.
`dom` gained **3,310 attempted** subtests — that is the honest count of tests that previously could
not start.

⚠⚠ **AND THE AREA THIS TICK WAS TAKEN FOR DID NOT MOVE ITS PASS COUNT.** `domparsing`'s four
`DOMParser-parseFromString-url*` files were `harness=TIMEOUT` at ~120 ms because their shared
`loadPromise` never resolved; they now RUN and then fail on their own merits, asserting
`doc.URL` / `documentURI` / `baseURI` on a `DOMParser`-created document. **That is a separate gap and
this fix must not claim it.**

**NOT covered, named:** the `onload` IDL attribute as a readable PROPERTY (`typeof frame.onload ===
'function'`). The handler *runs*; the content attribute is not reflected into a property. That is the
event-handler-IDL-attribute reflection surface — a different mechanism — and `g_iframe_load_event`
asserts it in its **failing** state so it cannot be quietly assumed fixed.

⚠ **The gate had to be taught to describe its own RED honestly.** `report()` is only called from the
handlers, so with the dispatch severed `#out` is never written and the *control* assertion fires,
announcing *"the frame's document is not installed"* — the exact false reading that cost this defect
three ticks of misattribution. It now checks for the unwritten sentinel first and says the true
thing: *neither handler ran; the document IS installed; the event is what is missing.*

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
## `textContent = ''` must create NOTHING — "string replace all" puts the empty case FIRST

The DOM standard's *"string replace all"* reads: **"Let node be null. If string is **not** the empty
string, set node to a new Text node whose data is string…"** — so clearing a node creates no child.
Ours created one unconditionally, which made `node.childNodes.length` **1** where Chrome gives **0**
after the most common clear-a-subtree idiom on the web.

**It is not a count. It destroys jQuery's element factory.** `jQuery.parseHTML` → `buildFragment`
finishes like this:

```js
  tmp.innerHTML = wrap[1] + html + wrap[2];
  jQuery.merge( nodes, tmp.childNodes );
  tmp = fragment.firstChild;  tmp.textContent = "";
  fragment.textContent = "";                          // <- HERE
  while ( ( elem = nodes[ i++ ] ) ) { fragment.appendChild( elem ); }
```

One leftover empty Text node and that fragment returns `[#text, <div>]`, so **`$('<div/>')[0]` is a
TEXT NODE** — and `$('<div/>')` is *the* jQuery element-creation idiom.

Traced live on `beb88run.xyz`, the top site of t888's crossing cohort. Slick's `buildOut` does
`$slides.wrapAll('<div class="slick-track"/>').parent()`; `wrapAll` takes `.eq(0)` (the text node),
descends `firstElementChild` (null on a text node, so it stays there) and `.append(this)`s the slides
into it:

```text
  div.banner-carousel.slick-initialized   ours [0 146 1185x0]   Chrome [0 146 1185x380]
  boxes in the whole subtree              ours 1                Chrome slick-list > slick-track > 8
```

**458 boxes — an entire carousel — moved into a text node and gone.**

### What made it survive: the idiom next to it was already right

`innerHTML = ''` goes through `set_inner_html`, which parses an empty string into no children and has
always been correct. One rule, two implementations, and only one wrong — so a probe of *either alone*
exonerates the pair. `G_TEXT_CONTENT_REPLACE_ALL` asserts both.

### The coercions, measured rather than assumed

`textContent` is `[LegacyNullToEmptyString] DOMString?`, and Chrome clears for **both** `null` and
`undefined` — so the setter reads its argument with `arg_string_nullable`, not `arg_string`, which
would have written the literal `"null"`. But `0` and `false` are **not** empty: they write `"0"` and
`"false"`. An emptiness test, never a falsiness test.

The MutationObserver record follows the same rule: clearing reports `addedNodes.length === 0`.
Telling an observer a node arrived when none did is the same lie one level up.

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

## …and then a break OPPORTUNITY was read as a space (tick 577)

Reading the fragment tree fixed the *selection* of text. It introduced a subtler error in its
*assembly*, which then sat there: `visible_text` concatenated with `words.join(" ")`.

**The line breaker emits one fragment per break OPPORTUNITY, not per line.** CSS puts one after a
hyphen, after `//`, and after `?` in a query string — so a word the layout merely *could* have broken
came back broken, on the same line, with a space wedged into it:

```text
rendered:  This site blocks non-mainstream browsers
observed:  This site blocks non- mainstream browsers
rendered:  https://walled.example/?a=1&b=2
observed:  https:// walled.example/? a=1&b=2
```

**Nothing about the rendering was wrong.** The pixels are right and the DOM `textContent` is right;
only this one string was wrong — and this string is `Observation.text`, what `manuk-agent` hands a
model, and the body `store::history_index` embeds for full-text history search. So a model asked to
find "non-mainstream" on the page found nothing, and a user searching their history for a URL found
nothing. Every hyphenated compound, every URL and every long token on the open web, silently, in
favour of a *plausible-looking* string.

**The geometry to tell the two cases apart was already on the fragment.** Two runs on the same
baseline whose boxes touch (`next.x <= prev.x + prev.width`) are one word — concatenate. A real gap on
the same line, or a different baseline, separates words — one space. A trailing space belonging to a
run is inside both its `text` and its `width`, so it survives either branch.

Both halves of that condition are load-bearing, and the gate proves it: drop the x-adjacency test and
`alpha beta gamma` glues into one token; drop the **baseline** test and `before<br>after` glues, because
a new line restarts at `x = 0` which trivially satisfies "touches the previous run's right edge."

> **Why no visual gate could see it, and how it was actually found.** Every instrument that scores
> rendering scores *boxes*. This defect produced correct boxes and a wrong string, so the entire
> visual apparatus was blind to it by construction. It surfaced as a `contains()` assertion failing in
> `hard_wall_detection_and_honest_interstitial` — a test about **honest error pages** — which the wall
> does not launch. The rendering was never the thing to check; the **consumer** was.

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

## Publish what the cascade HOLDS, and nothing it does not — the whole-object diff

`getComputedStyle` had been failing **one property per tick**: `transform` (applied for sixty ticks
before the number reached JS), `width`/`height` (t897), `zoom` and `containerType` (t900's surface
audit). t901's constitution check named it as an **I3 defect class** — *the semantic model silently
declining to publish what the pipeline already computed* — and ranked the enumeration above the next
member. **One diff of the whole object against Chrome, 132 properties × 7 representative elements:
411 differing readings of 924, and the dominant shape was `undefined`, not a wrong value.**

> **Four members found one at a time is four ticks spent on what one diff lists.** When a defect
> recurs on the same seam, stop fixing members and enumerate the set.

### The split is the deliverable

Chrome emits an initial value for every property it supports. **We must not**, for the ones this
engine does not honour — that is `@supports`-style false presence, and both of this project's
standing rules point at it: *"absence routes to the fallback; HALF-presence routes into a wall"*
(t772) and *"a name is defined IFF the thing it names exists"* (t608).

| | |
|---|---|
| **PUBLISHED** (cascade-held, was `undefined`) | `order` · `background-size` · `object-position` · `text-shadow` · `inset` · `grid-column-start`/`-end` · the logical family: `margin-inline-*`, `margin-block-*`, `padding-inline-*`, `padding-block-*`, `inset-inline-*`, `inset-block-*`, `inline-size`, `block-size`, `min`/`max-inline-size`, `min`/`max-block-size` |
| **DELIBERATELY ABSENT** (no cascade field) | `hyphens` · `touchAction` · `willChange` · `writingMode` · `tabSize` · `containerType` · `scrollBehavior` · `overscrollBehavior` · `caretColor` · `accentColor` · `isolation` · `contain` · `columnCount` · `breakInside` · `unicodeBidi` · `fontStretch` · … (41 in total) |

**Measured: 411 → 321 differing, fourteen properties fixed, zero newly broken.**

### The logical properties are exact aliases, and that is a STATEMENT about this engine

`writing-mode` has no `ComputedStyle` field, so every box is `horizontal-tb` and `inline` **is**
horizontal. The aliases are therefore exact, not approximate. The day a vertical writing mode lands,
they stop being aliases — and that block is where it shows up.

### `grid-template-columns` is the instructive OMISSION

The cascade holds it (`Vec<TrackComponent>`), so it looks publishable. But Chrome does not report the
author's track list: on a rendered grid it reports the **used track sizes in px** — `98.6562px
197.344px` for `1fr 2fr` in a 300px container — and `none` everywhere else. Emitting `1fr 2fr` would
be a **wrong answer of the right type**, the shape this project rates most dangerous, because a grid
library parsing px out of it gets `NaN` from a string that looked valid. The used sizes are not on
this seam; they need layout's track list. **Absence keeps the caller on its fallback.**

### Two spellings of one box must go through one serialiser

This tick's own first version sent `max-inline-size`/`max-block-size` through `dim_css` (unset →
`auto`) while the physical `max-width`/`max-height` use `max_dim` (unset → **`none`**). The re-sweep
caught it in one line: **the logical spelling disagreed with the physical one about the same box, the
moment a second serialiser was used.** `inline-size`/`block-size` therefore call the same
`used_dim_css` as `width`/`height`, and the gate asserts the *identity* (`inlineSize === width`)
rather than the values.

### `word-spacing` does NOT share `letter-spacing`'s rule, and a lumped comment hid that

An unset `letter-spacing` serialises as the keyword `normal` (it permits the font's own kerning;
`0px` does not). An unset **`word-spacing` serialises as `0px`** — its initial value is the length
zero. The two look symmetric, are not, and one comment written for both is exactly how the wrong one
survived. Same shape as the lumped assertion that failed permanently at t855.

## `getComputedStyle(el).width` is the *RESOLVED* value — the USED size in px, not the specified one

CSSOM makes `width`/`height` two of the handful of properties whose resolved value is the **used
value** whenever the element generates a box. We returned the computed value verbatim:

```text
                                     Chrome     ours (before)
  block, width:auto, pad 5, bd 2      580px        auto
  block, width:50% of 600             300px         50%
  abspos sized by left + right        560px        auto
  flex item, flex:1, in a 400px       400px        auto
  33.333% width               199.984px   calc(-0.016662598px + 33.333336%)
  ANY height                           20px        auto        <- uniformly
```

**It was never a layout gap.** `offsetWidth` on the same elements was already exact (594 and 300
against Chrome's 594 and 300), and `computed_style_js` has taken the element's layout `rect` since
the transform work — a percentage `translate` resolves against the border box. The binding was
declining to publish what layout had already computed, which is the *same shape* as the
`getComputedStyle(el).transform` defect one section up: the box really moved for sixty ticks before
the number reached JavaScript.

**What it costs.** `parseInt($(el).css('width'))` is `NaN` on every jQuery page. jQuery's
`getWidthOrHeight` survives only because it falls back to `offsetWidth` **when it sees `auto`** — and
that fallback is itself gated on `elem.getClientRects().length`, so an engine that answers `auto`
without a working `getClientRects` returns `0` and every measure-then-size widget sizes to nothing.
Animation libraries that pin a start value (`el.style.width = getComputedStyle(el).width` before a
transition) read it with no fallback at all.

### The box reported is the one the element's own `box-sizing` names

Measured, because the plausible answer — always the content box — is wrong:

| declared | `offsetWidth` | Chrome's resolved `width` |
|---|---|---|
| `box-sizing:border-box; width:200px; padding:10px; border:5px` | 200 | **`200px`** (the border box) |
| `box-sizing:content-box; width:200px; padding:10px; border:5px` | 230 | **`200px`** (the content box) |

So the border box is reported unadjusted for `border-box`, and border+padding are subtracted for
`content-box`.

### Two guards, and "always report the rect" breaks both

* **`display:none`** generates no box, so CSSOM says report the *computed* value — the author's own
  `70px`, not a used value of 0.
* **A non-replaced inline** reports `auto` in Chrome even though it has a real border box. Returning
  its rect would be a confident wrong answer on the commonest element on the page.

### One case is deliberately NOT resolved, and it is named rather than approximated

`width:auto` together with a **percentage padding**: that padding resolves against the containing
block's width, which this seam does not hold. Such an element keeps its specified value instead of
getting an invented number. (A percentage padding on a *content-box* element is fine by construction —
the specified width IS the content box, so the fallback lands on Chrome's answer, and
`G_RESOLVED_WIDTH_HEIGHT` asserts that.) A replaced element that is `display:inline` — an `<img>` with
no width attribute — falls into the inline guard for the same reason.

### The reconciliation clause, because two readings of one box must not drift

The gate asserts `resolved content width + border + padding === offsetWidth`, and for `border-box`
that the resolved width IS `offsetWidth`. Two numbers describing the same box that disagree mean one
of them is invented — the accounting-reconciliation mechanism, applied to a single element.

## `el.style` did not SERIALIZE — it ECHOED, and the fix is Stylo's serializer, not a regex

`getComputedStyle` is a *computed*-value surface; `el.style` is a live view over the `style`
**attribute**, and it handed back the exact bytes the author typed. CSSOM says `getPropertyValue`
returns *"the result of serializing the declaration's value"* — a **normal form**:

```text
   style="…"                          Chrome                   ours (before)
   background-position: 5% .5%        "5% 0.5%"                "5% .5%"
   background-position: 5% -0px       "5% 0px"                 "5% -0px"
   background-image: url(http://x/)   "url(\"http://x/\")"     "url(http://x/)"
```

### The refusal that shaped the fix, written a tick before it

t1220 sized this at 164 subtests and declined the obvious version in advance: *"a targeted 'prepend 0
to a leading dot' fix would pass all 164 and be a band-aid — `el.style` does not serialize at all, it
echoes, and every other CSSOM normalisation (unit case, colour form, shorthand ordering) is silently
wrong the same way."* So the value round-trips through `stylo_engine::serialize_declaration` —
**Stylo's own `parse_style_attribute` and its own `property_value_to_css`**. The leading zero, the
quoted URL and the negative zero fall out of it together, and so do the normalisations nobody has
written a test for yet. That is the entire difference between this and a regex: `-0px → 0px` and
`url(x) → url("x")` are not reachable from any rule about leading zeros.

It is also the third surface on one seam. `@supports`, `CSS.supports()` and now `el.style` all
answer questions about a single declaration, and they share **one** evaluator — because two of them
once disagreed, and which answer a page got depended on which it asked first.

### `''` means LEAVE IT ALONE, and the polarity is the opposite of `CSS.supports`'s

⚠ The seam returns the empty string for anything it declines, and the caller **echoes** rather than
clears. `eval_supports` defaults to a conservative `false` because guessing "yes" *invents a
capability*; `serialize_decl` defaults to echoing because returning empty would **delete a
declaration the page set**. Same seam shape, opposite safe direction, and picking the wrong one is
silent in both cases. A **custom property** is refused for a different reason: `--brand: .5rem` has
no grammar to normalise against, and normalising it would rewrite every design token on the page.

⚠ Memoised on `(property, value)`, like the setter's validator beside it — `el.style.transform` in a
`requestAnimationFrame` loop must not pay a Stylo parse per frame. Buying conformance with a
per-frame regression is a trade, and the ratchet refuses trades.

**GATE** `G_INLINE_STYLE_SERIALIZES` — `inline_style_reads_back_the_serialized_value_not_the_authors_text`,
10 claims. RED-proven: echo the raw text and **six** fail. The other four stay green *by design* —
they assert the refusals and the two-spellings reconciliation, which must hold whether the mechanism
is on or off. A gate whose every claim falls to one mutation is testing one thing.

## The INSETS are resolved values too, and the blocker was one missing input: the containing block

`top`/`right`/`bottom`/`left` sit in the **same used-value bucket** as `width`/`height` above — CSSOM's
resolved-value special case puts them there whenever the property *applies* to an element that
generates a box. They stayed on the computed value for one reason, and t1220 named it precisely
before the fix: *"resolving it needs the containing block's size, and the serializer has only the
element's own border box."* `computed_style_js` took `rect`; a percentage inset resolves against the
**containing block**, which is a different element entirely.

```text
                                                Chrome     ours (before)
  position:relative; top:10%   (CB 100px tall)   10px         10%
  position:absolute; top:10%   (CB 200px tall)   20px         10%
  top:calc(10% - 1px)          (CB 100px tall)    9px         calc(-1px + 10%)
  position:relative; bottom:3px, top:auto        -3px         auto
```

### Three positions, three different ancestors, three different boxes

`containing_block_size` walks the arena. Picking the wrong row is **silent**, because every row
returns a plausible number about a real element:

| `position` | basis element | area |
|---|---|---|
| `relative` | the nearest **element ancestor** (the in-flow parent) | **content** box |
| `sticky` | the nearest **scroll container** ancestor, else the viewport | **content** box |
| `absolute` | the nearest ancestor that is **positioned or transformed** | **padding** box |
| `fixed` | the nearest **transformed** ancestor, else the viewport | **padding** box |

⚠ **An IDENTITY transform still establishes the containing block.** `transform: scale(1)` is the
standard trick for pinning a `fixed` child, and WPT's `getComputedStyle-insets-fixed.html` is built
on exactly it — so an implementation that skips a numerically-identity transform answers that whole
file against the viewport and is wrong by a factor of the page.

⚠⚠⚠ **`sticky` is NOT `relative` here, and the first implementation had it as one.** CSS Position 3
§6.3: a sticky box's insets are insets from the edges of the **scrollport** — the nearest scroll
container's content box — not from its containing block. WPT states this as a *controlled
experiment* rather than as prose, which is why reading the family file-by-file finds it and reading
the spec from memory does not: `getComputedStyle-insets-sticky.html` and
`-sticky-container-for-abspos.html` each add `overflow: hidden` to the element they name as the
basis, and the `relative`/`absolute`/`fixed` files in the same family deliberately do not. The
difference is the whole point — a sticky table header or sidebar is almost never a direct child of
its scroller, so resolving against the parent is not a small error, it is a **different element**.
(`overflow: hidden` counts: it is a scroll container the user cannot scroll, not the absence of one.)

### Absolutize the COMPUTED value — this is NOT "report what layout used"

`position:relative; top:10%; bottom:50%` is **over-constrained** (layout applies `bottom = -top`), and
CSSOM's own special case says an over-constrained inset resolves to the *computed* value. So the two
sides absolutize **independently** — `10px` and `50px`, not `10px` and `-10px`. The intuitive
implementation ("ask layout what offset it applied") passes the simple rows and fails here.

### `auto` splits three ways, and only two are answerable at this seam

* **`relative`** — `auto` *is* resolved: `-(opposite)`, or `0px` when the opposite is also `auto`
  (CSS2 §9.4.3). Pure arithmetic on the computed values; no geometry needed.
* **`sticky`** — `auto` is **preserved**. A sticky box's offsets are a clamp range, not a
  displacement, so `auto` means *unclamped on this edge* and has no px equivalent.
* **`absolute`/`fixed`** — the used value is a **distance between two laid-out boxes**:
  `used top = element margin-box top − containing-block padding-box top`, and the mirror on each
  far edge. ⚠ The inset runs to the **margin** box, not the border box (CSS 2.1 §10.3.7), which with
  the zero margins the common case has is invisible — and wrong on a centred dialog.

  ⚠⚠⚠ **This arm was REFUSED for a tick on a claim that was overstated, and the correction is worth
  more than the fix.** The refusal read: *"a resolved `auto` is the used static position, which is
  layout output this seam does not receive."* But the seam **does** receive `layout_rect(n)`, and the
  containing block's box was one field away from carrying its origin. The refusal was right about the
  *specified* value and wrong about what was derivable from values already in hand. **The general
  form: "this seam does not have X" is a claim about the seam's INPUTS, and it has to be checked
  against them rather than recalled — a distance between two things you can both see is not a
  missing input.**

  ⚠ It is also the **one arm answered by LAYOUT** rather than by arithmetic on the cascade, and that
  is a different risk profile: everywhere else a wrong containing block yields an honest refusal;
  here a mis-placed box yields a confidently wrong px. The `auto`/`auto` row reports the used
  **static position**, so it asserts where layout put the box.

### The failure is worse than `NaN`, and the RED proof is what showed it

The expected story was *"`parseFloat` returns `NaN`"*. It does not — **`parseFloat("10%")` is `10`**.
Every tooltip, dropdown, drag handle and sticky-header polyfill that pins a start value got a
**plausible number in the wrong unit**: 10% of a 900px container silently became `10px`, and nothing
anywhere threw. Only the `calc()` spelling of the same offset is `NaN`. One property, two different
silent failures, chosen by how the author happened to write it.

### Twelve call sites, one function

Physical (`top`), logical (`inset-block-start`) and the `inset` shorthand are three spellings of one
box, and they all route through `inset_css`. That is not tidiness: `max-inline-size` already caught
this exact drift once, where the logical spelling said `auto` and the physical one said `none` about
the same element. The gate asserts all three agree.

### The static position ACCUMULATES, and one level of nesting cannot show it

An `auto`/`auto` abspos box sits at its static position — the **content** box of its in-flow parent,
which is *not* the padding box its containing block is measured from. The two differ by exactly the
containing block's padding, so a single-level fixture reads `8px` where the naive expectation is `0`
and both look plausible. Down a chain it accumulates: a wrapper with `margin 4 / border 2 / padding 1`
inside a containing block with `padding 8` puts the box at `8 + 4 + 2 + 1 = 15`. That is WPT's own
arithmetic for the same nesting (`staticPositionY: 1 + 2 + 4 + 8` in `getComputedStyle-insets.js`),
and the gate asserts the nested number rather than the single-level one for that reason.

**GATE** `G_RESOLVED_INSETS` — `computed_insets_are_the_used_offset_against_the_containing_block`,
38 claims. RED-proven twice, and the second proof is what makes the gate able to attribute: make
`used_inset_css` return `None` and **30 of 38** fail; keep the whole mechanism and drop only the
containing block's **origin** (`[0, 0, w, h]`) and **exactly the five `absau*` rows fail**. A
percentage needs the containing block's SIZE, an `auto` needs its POSITION — a gate that could not
tell those apart would credit one tick with both.

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

### `enterKeyHint`/`inputMode` were keyed under a tag named `"undefinedelement"` — a global that reached NO element (tick 490)

The same defect, one layer subtler. `inputMode` and `enterKeyHint` ARE in `reflect_table.rs` — but under
the key `"undefinedelement"`, a tag name that matches nothing, instead of the `"*"` global bucket the
mechanism applies to every element. So `descFor` never found them for any real tag, `input.inputMode` read
`undefined`, and `el.inputMode = 'tel'` no-opped. They are `HTMLElement` globals (the on-screen-keyboard
steer every mobile form and `contenteditable` field sets), so the fix is to move both rows into `"*"`. The
generic enum machinery then does the rest, spec-correctly: absent → `""`, invalid → `""` (limited to only
known values), a valid keyword round-trips through the lowercase content attribute. The lesson from the
`"*"` grind repeats — a global attribute keyed to a *specific* tag (here a nonexistent one) reaches nothing;
verify the bucket, not just the presence of the row. [[js-engine]]

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

## `form.elements` is a live `HTMLFormControlsCollection`, and a radio group is a `RadioNodeList` (tick 433)

`form.elements` was `undefined` **entirely** — not incomplete, absent. Every form-serialization and
validation library opens with `for (var i = 0; i < form.elements.length; i++)` or reaches for
`form.elements['field']`, and both threw `can't access property … form.elements is undefined`. It carried
in the constellation as an UNKNOWN; a RED probe pinned it.

It is a legacy platform object like [[dom-semantics]]'s `HTMLCollection`, but with **two form-specific rules
a plain collection gets wrong**, which is why it has its own builder in `collections_js` rather than reusing
`live()`:

- **Members are the LISTED controls** in tree order — `button` / `fieldset` / `input` / `object` / `output`
  / `select` / `textarea` — **minus `input[type=image]`** (an image button the collection omits). Not every
  descendant, and not the form itself.
- **The named getter returns a `RadioNodeList` when >1 control shares a name** — a radio group. That list's
  `.value` **reads** the checked radio's value and **writing** it selects the matching radio. Skip this and
  `form.elements.plan.value` silently returns the *first* radio, not the *selected* one — the single most
  common way form code reads a choice.

Indexed access, `.length`, `.item(i)`, `.namedItem(name)` and named access by `name` (HTML ns) then `id`
all resolve against that control list. `HTMLFormControlsCollection` and `RadioNodeList` are exposed on
`globalThis` so `instanceof` checks pass.

**Why NOT through `live()`.** `live()` is the hot `childNodes`/`children` proxy, and its own note records
that enriching its traps once surfaced a **cross-file UAF** on the shared-batch-runtime heap (see the
tick-129 heap-churn trap above). `form.elements` gets a **self-contained** builder so that path stays
byte-for-byte untouched.

**KNOWN LIMIT (honest).** Association is by **subtree** — a control the form contains — not the `form=`
attribute reassociating a control that lives elsewhere in the document. That is the ~99% case; `form=`
reassociation is a separate follow-on. Gate `g_form_elements` (13 claims, proven red — pre-impl probe
printed `type:undefined`). [[js-engine]]

## `control.labels` and `label.control` link a form field to its `<label>`s (tick 434)

Both were `undefined`. `input.labels` is the NodeList an accessibility helper reads to find the text that
**names** a control (`input.labels[0].textContent`); `label.control` is the inverse a "focus the field
when its label is clicked" handler walks. A control with neither is invisible to both.

- **`label.control`** = the `for=` target **if that target is labelable**, else the **first labelable
  descendant** (`<label><input></label>` with no `for=`). Labelable = `button` / `input` (NOT
  `type=hidden`) / `meter` / `output` / `progress` / `select` / `textarea`.
- **`element.labels`** = a **static** NodeList of every `<label>` whose `.control` resolves back to this
  element, in tree order — a control can carry more than one label. A hidden input is non-labelable, so
  its `.labels` is `null` (the `HTMLInputElement` contract), and a `<label for=hidden>` claims nothing.

**Why STATIC, not `live()`.** `.labels` is read far too rarely to earn a per-access proxy, and routing it
through the hot childNodes `live()` path is exactly the heap perturbation the tick-129 note warns causes a
cross-file UAF. It is a plain frozen array-like with `NodeList.prototype`, `.length`, `.item`, `forEach`
and an iterator. Gate `g_label_association` (8 claims, proven red by disabling the getters). [[js-engine]]

## The `<table>` DOM: `table.rows` is a live HTMLCollection in LOGICAL order (tick 435)

The whole `<table>` read surface was `undefined`. `table.rows` and `tr.cells` are how a data-grid /
sortable-table widget and every "what row/column is this cell" accessibility walk read a table.

- **`table.rows`** — a live HTMLCollection in **logical** order: **thead rows, then tbody + direct
  `<tr>` rows (tree order), then tfoot rows**. This is NOT document order, and the difference is
  load-bearing: the HTML spec lets `<tfoot>` be authored *before* `<tbody>`, so a document-order reading
  numbers the footer as a body row. `thead`/`tbody`/`tfoot.rows` is that section's own rows.
- **`table.tBodies`** (HTMLCollection), **`table.tHead`** / **`table.tFoot`** (the first such section or
  null), **`tr.cells`** (the `td`/`th` children).
- **`tr.rowIndex`** = index in `table.rows`; **`tr.sectionRowIndex`** = index within the row's section (a
  direct `<tr>` child of `<table>` is indexed within the implicit tbody); **`td.cellIndex`** = index in
  `tr.cells`. Each is `-1` when unparented and `undefined` on the wrong tag.

Collections reuse the existing `live(…, HTMLCollection)` factory (the `getElementsByTagName` path), never
the heap-sensitive `childNodes` NodeList path (tick-129 note). Gate `g_table_dom` (12 claims, proven red;
the fixture writes `<tfoot>` before `<tbody>` so document order fails). [[js-engine]]

## The `<table>` write API: `insertRow`/`insertCell` materialise structure (tick 436)

The write side of the `<table>` DOM (`table.insertRow`, `tr.insertCell`, the `create*`/`delete*` section
methods) was `undefined`, so a grid/spreadsheet widget that builds a table through the DOM instead of
`innerHTML` — still a common shipped pattern — threw. Built on the tick-435 read helpers.

- **`table.insertRow(index=-1)`** (and section `insertRow`) returns a new `<tr>`; `-1` appends. Inserting a
  row into an **empty** table **materialises a `<tbody>`** and puts the row in it — a bare `<tr>` appended
  to the `<table>` would then not even appear in `table.rows` (the logical-order reader). `tr.insertCell`
  is the same on a row.
- **An out-of-range index is an `IndexSizeError`** — a throw the caller branches on, *not* a clamp. Clamping
  looks friendlier and silently corrupts a widget that inserts at a computed position inside a `try`.
- **`createTHead`/`createTFoot`** REUSE an existing section (idempotent — libraries call them defensively);
  `createTBody` always makes a new one; `createCaption` inserts the `<caption>` as the **first** child;
  `deleteTHead`/`deleteTFoot`/`deleteCaption` remove them.

Gate `g_table_write` (10 claims, proven red — all methods undefined pre-impl). [[js-engine]]

## `element.form` resolves the form owner (tick 437)

`input.form` was `undefined`, so a form library grouping controls by their owner (`input.form === thisForm`)
got nothing — and it silently broke `ElementInternals.form`, which delegates to `host.form`.

The owner is computed, not stored: **if the element has a `form=` attribute, the element with that id iff
it is a `<form>`** — an id pointing at a *non-form* yields `null` (NOT the ancestor, per spec: the author
explicitly opted the control out of its ancestor into a form that here does not exist); **otherwise the
nearest ancestor `<form>`**. This is exactly what lets a control live *outside* its form and still belong
to it (`form="loginForm"` on an input in a modal). An `<option>` reports its `<select>`'s owner; a
`<label>` reports its labeled control's owner (via tick-434 `.control`); a non-form-associated element has
no such property. Defined on form-associated tags (input/select/textarea/button/fieldset/object/output).
Gate `g_form_owner` (8 claims, proven red). [[js-engine]]

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

### The config's first brick — `removeElements` (tick 545)

The baseline answers *"is this markup safe?"*. It does not answer the other question every real caller
asks, which is *"and also drop the things I don't want"* — a comment renderer that permits `<b>` and
`<a>` but never an `<img>` or an `<iframe>`, because the safe-baseline `<img src=…>` is still a
tracking pixel and a layout bomb. That is `setHTML(html, { sanitizer: { removeElements: [...] } })`,
and it is the **first configurable brick**: a block-list of element names removed **entirely**, applied
*on top of* the always-on baseline, never instead of it.

The shape matters more than the code. `sanitize_subtree` takes the block-list as a parameter, so the
baseline is not a default the config can turn off — `<script>` is stripped whether or not a config was
passed, and a config can only ever **add** removals. `read_sanitizer_remove_elements` reads
`options.sanitizer.removeElements` defensively: a missing options object, a missing `sanitizer`, a
non-array `removeElements` each yield an empty set, so **a malformed config degrades to the safe
baseline rather than to nothing**. Names are lowercased on both sides for HTML's case-insensitive
element match. This is the direction the whole config has to grow in: *the safe answer is the floor,
and configuration raises the floor.*

`G_SANITIZER`'s three new claims are `cfg-removes-img` (the `<img>` the baseline keeps is now gone),
`cfg-keeps-safe` (the `<b>` survives — a block-list, not delete-everything) and `cfg-baseline-still`
(the `<script>` is still stripped, so the config did not replace the baseline). RED-proven by changing
the config to `removeElements: ['nosuchtag']`: `cfg-removes-img` flips to `false` — the assertion reads
the real tree, and the *only* thing that changed was the config's content.

**Honest limit:** a block-list, not the whole config. The allow-list (`elements`),
`replaceWithChildrenElements`, the attribute lists (`attributes` / `removeAttributes`), a reusable
`Sanitizer` config *object*, and `Document.parseHTMLUnsafe` are the follow-ons, and declarative
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

## `<template>.content` is the parser's fragment, not the element's direct children (tick 425)

A `<template>`'s contents live in a SEPARATE fragment, not as children of the element — the HTML
tree-construction rules redirect everything parsed inside a `<template>` into its "template contents"
fragment (`html5ever` calls `get_template_contents` and appends there). So `template.childNodes` is
empty by design, and `template.content` must expose that fragment.

The bug fixed at tick 425: the DOM kept the parser's fragment in the node's `template_contents` field,
but the `.content` accessor (`Dom::template_content`) read a DIFFERENT field (`shadow_root`) and, finding
nothing, built a fresh fragment from the template's own DIRECT children — which are empty on the parser
path. So a parsed `<template>.content` came back empty, and every framework that clones it (lit-html,
Svelte, Solid, Vue's compiled output) instantiated nothing, silently. The accessor now returns
`template_contents` when present; the imperative `createElement`+`innerHTML` fallback (whose children DO
land as direct children) still works and now caches into the same field. Declarative shadow DOM already
aims `set_template_contents` at the shadow root, so `.content` returning `template_contents` stays
consistent with that path.

## The `<select>` write API — `add` / `remove(index)` + HTMLOptionsCollection (tick 438)

`select.options` (read) already worked, but the WRITE side was silently wrong. `select.add()` was
`undefined`, and — worse — `select.remove(0)` DETACHED THE WHOLE SELECT: `HTMLSelectElement` had no own
`remove(index)`, so the call fell through to the inherited `ChildNode.remove()`, which ignores its
argument and tore the control out of its `<form>`. The spec overloads `remove`: `select.remove(index)`
removes `options[index]`; `select.remove()` with no argument keeps the legacy detach-self behaviour.

Fixed in `engine/js/src/collections_js.rs` (the live-collections shim, alongside form.elements / the
`<table>` write API): `select.add(element[, before])` (before = null/omitted → append, a number → insert
before `options[n]`, an element → insert before it in its own parent), a delegating `remove` override on
the element prototype (only `select.remove(<index>)` diverts to option removal; every other element —
and argument-less `select.remove()` — still routes to the native `ChildNode.remove`), and the
HTMLOptionsCollection methods `namedItem`/`add`/`remove` hung on the array the native `options` getter
returns. `div.add` stays `undefined` (Chrome parity). Gate `G_SELECT_WRITE`.

## option.text + the Option() constructor's defaultSelected argument (tick 439)

`option.text` — how a page reads the LABEL of the chosen option (`select.options[select.selectedIndex].text`)
— was `undefined` (a plain expando; assigning to it left the text content untouched). Fixed in
`engine/js/src/collections_js.rs`: the getter returns the option's text content with ASCII-whitespace runs
collapsed and trimmed (spec), the setter replaces the text content. It is defined narrowly for `<option>`;
on any other element the getter is `undefined` and the setter materialises an ordinary own data property,
so `div.text = x` expandos do not regress.

`new Option(text, value, defaultSelected)` (`engine/js/src/event_loop.rs`) ignored its 3rd argument, so a
constructed pre-selected option came back unselected. The constructor now sets the `selected` content
attribute when `defaultSelected` is truthy — which is what `.selected` reads for an option not yet dirtied
in a rendered select, so `new Option('t','v',true)` comes back selected AND defaultSelected. Gate
`G_OPTION_TEXT`.

## textarea.value is its text content, not a value attribute (tick 440)

`<textarea>abcdef</textarea>.value` returned `""`. Every text-control value path in `dom_bindings.rs`
(`el_get_value`, `text_value_len`, `el_set_range_text`) read the `value` ATTRIBUTE unconditionally — correct
for `<input>`, wrong for `<textarea>`, whose raw value is the child TEXT CONTENT until the user or a script
dirties it. So a server-rendered pre-filled textarea (edit-comment / edit-bio / edit-post — the whole
"editing existing content" web) read an empty field, and `setRangeText` on it replaced the entire value
instead of the selected range (`setSelectionRange(1,3)` + `setRangeText('XY')` on `"abcdef"` gave `"XY"`,
not `"aXYdef"`).

Fixed with a single `text_control_value(dom, node)` helper that all three paths now share: for a textarea it
returns the `value` attribute IF present (our dirty-value store, written by `el_set_value`/`el_set_range_text`)
else the element's text content; for an input it returns the `value` attribute. Gate `G_TEXTAREA_VALUE`.
Remaining: `textarea.defaultValue` (still `undefined` — the text-content default, separate from the reflect
table's input.defaultValue) and `form.reset()` restoring a textarea to its content default.

## select.length is the option count and resizes the list (tick 441)

`select.length` returned `0` — the `length` property was wired to `el_char_length` (the CharacterData text
length, `0` for a non-text node), and the correct `el_get_select_length` was dead code. So the option count
was invisible and the classic `select.length = 0` "clear the dropdown" idiom (and `select.length = n`
resize) did nothing. Fixed in `dom_bindings.rs`: `length` now dispatches on the tag — a `<select>` reports
`select_options(...).len()`, every other node keeps the CharacterData length — and gains a setter that
truncates (removing trailing options from their own parents, so an option inside an `<optgroup>` is handled)
or grows (appending bare `<option>` elements). CharacterData.length stays read-only (the setter no-ops off a
select). Gate `G_SELECT_LENGTH`. (`select.options.length = n` — the same idiom via the collection — was
closed in tick 459, below.)

## select.options.length is a LIVE writable accessor — the collection clear-idiom (tick 459)

The tick-438 surface audit pinned `select.options.length = 0` as a MEASURED no-op, and tick 441 fixed only
the `select.length` form. `select.options` hands back a fresh Array decorated (in `collections_js.rs`) with
the HTMLOptionsCollection methods, but its `.length` was a plain Array length: the ubiquitous non-framework
"clear the dropdown before repopulating" idiom —

```js
sel.options.length = 0;                     // country/dependent picker: clear
for (const c of countries) sel.add(new Option(c.name, c.code));
```

— truncated the throwaway SNAPSHOT the getter had just returned and left the DOM untouched. The `<option>`s
stayed, the next `sel.options` read them back, and the "cleared" list showed every stale row under the fresh
ones. A dead expando (the same bug class as `option.text`/`select.length` before them).

Fixed in `engine/js/src/collections_js.rs` `decorateOptions`: the decorated array is now wrapped in a
`Proxy` whose `length` get returns the LIVE option count (`select.length`) and whose `length =` routes to
the already-correct native `select.length` setter (`el_set_select_length`: truncate removes trailing options
from their own parents, grow appends bare `<option>`s). Everything else — indexed access, iteration,
`namedItem`/`add`/`remove`, and the Array methods pages call on the snapshot — passes straight through to the
array target, so `Array.isArray(sel.options)` / `instanceof Array` / spread all still hold and no existing
use regresses. Gate `G_OPTIONS_LENGTH` (RED-proven by unwrapping the Proxy → the assignment hits the
snapshot only and the DOM keeps all three options).

## input.valueAsNumber + stepUp/stepDown for numeric inputs (tick 442)

Every numeric spinner, range slider and quantity stepper reads/writes the NUMBER behind the control, not
its string. `input.valueAsNumber` was `undefined` and `stepUp`/`stepDown` threw (not a function), so a
"+"/"−" quantity button or a `valueAsNumber = total` assignment did nothing. Added in `collections_js.rs`
over the existing `.value` accessor: `valueAsNumber` (get parses the value as a Number — NaN for empty/
invalid; set writes `String(n)`, or `""` for non-finite) for `type=number`/`type=range`; `NaN` for other
input types, `undefined` on non-inputs. `stepUp(n=1)`/`stepDown(n=1)` add/subtract `n × step` (step
defaults to 1) and clamp to `min`/`max`, trimming float fuzz. Gate `G_VALUE_AS_NUMBER`. Follow-on:
`valueAsDate` + date/time typed values (epoch arithmetic), left unbuilt.

## input.valueAsDate + valueAsNumber for date/time/month inputs (tick 443)

Completing tick 442's typed-value surface: every date picker reads `input.valueAsDate` for a real `Date`
(and `valueAsNumber` for the epoch) and writes `valueAsDate = d` to set the control; both were absent for
date-family types. Added in `collections_js.rs`: `type=date` → `valueAsDate` is UTC midnight, `valueAsNumber`
the epoch ms; `type=time` → ms since midnight + a 1970-01-01 Date; `type=month` → a month index. Setters
reformat back to the control's string. ALL arithmetic is UTC (a date control has no timezone, per spec) so a
`type=date` round-trips regardless of host timezone. `valueAsDate` is `null` on number/range/datetime-local
(does not apply). Gate `G_VALUE_AS_DATE`. Follow-on: `type=week` (ISO week) and `datetime-local`
valueAsNumber, left unbuilt.

## progress.position + output.value (tick 444)

Two display-control values that were missing. `progress.position` (added in `collections_js.rs`, guarded to
PROGRESS) is the completion fraction `value/max` clamped to `[0,1]`, or `-1` when the bar is INDETERMINATE
(no `value` attribute) — a script driving an upload/download bar off `position` got `undefined`. `output.value`
(fixed in `dom_bindings.rs` `el_get_value`/`el_set_value`, same class as textarea.value at t440) IS the
`<output>`'s displayed text content: the getter returns the text content, the setter replaces the children
with a text node (the spec's "value mode"), so `output.value = result` on a calculator both shows and reads
back. Previously it returned `""` and assignment was a dead expando. Gate `G_PROGRESS_OUTPUT_VALUE`.

## the .text property for a/script/title (tick 445)

Completing tick 439's `.text` accessor: `<a>.text` (link label), `<script>.text` (inline script source) and
`<title>.text` (page title) were dead expandos returning `undefined`, with assignment leaving the content
untouched. The `EP.text` getter/setter (`collections_js.rs`) now returns/sets the RAW text content
(whitespace preserved) for A/SCRIPT/TITLE, keeps `<option>.text` whitespace-collapsed, and preserves the
ordinary `div.text = x` expando on any other element. Gate `G_ELEMENT_TEXT`.

## datetime-local + week typed values (tick 446)

Closing the follow-on tick 443 named as unbuilt. `<input type="datetime-local">` and `<input type="week">`
returned `null` from `valueAsNumber`/`valueAsDate` and their setters were silent no-ops, so scheduling/booking
forms that compute a duration from `valueAsNumber` or seed a picker via `valueAsNumber = ms` got nothing.
Added in `collections_js.rs` over the existing typed-value helpers: `datetime-local` → `valueAsNumber` is the
UTC ms of the local datetime (the control has no timezone, so the local `YYYY-MM-DDTHH:MM` is read AS-IF UTC),
`valueAsDate` stays `null` (does not apply). `week` → `valueAsNumber` is the UTC ms of the Monday 00:00 that
starts the ISO week, `valueAsDate` is that Monday as a `Date`, and the setters run ISO-8601 week arithmetic
(weeks start Monday; week 1 holds the year's first Thursday / Jan 4) — `isoWeekStartMs(y,w)` for the forward
direction, `isoWeekOf(date)` for the reverse. All UTC, so a round-trip is host-timezone-independent. Gate
`G_DATETIME_WEEK_VALUE`. This completes the typed-input value surface begun at ticks 442/443.

## <a>/<area> URL-decomposition setters (tick 447)

The write-side of the URL-decomposition IDL. The getters (`a.protocol`/`hostname`/`port`/`host`/`pathname`/
`search`/`hash`/`origin`) had worked since the mdbook TOC fix, but every SETTER was registered with a `None`
setter in `dom_bindings.rs` — a silent no-op. So `link.search = '?utm=x'` (the canonical analytics-tag idiom)
and `a.hash = '#' + id` (in-page nav) changed nothing and `a.href` never moved. Added `anchor_url_set` +
`apply_url_part`: the assignment re-parses the element's resolved href (DOC_URL base joined via the real `url`
crate — the same parser the network stack uses, so it can't disagree with what gets fetched), mutates the one
component (`set_scheme`/`set_host`/`set_port`/`set_path`/`set_query`/`set_fragment`), and writes the
re-serialised URL back to the `href` attribute. A `?`-less value to `search` and a `#`-less value to `hash`
are normalised; `host` splits a trailing numeric port into hostname+port. Tag-guarded to `<a>`/`<area>` so a
plain element can never grow a spurious `href`; `origin` stays read-only (no setter). Gate
`G_ANCHOR_URL_SETTERS`.

## `<img>.currentSrc` reports the URL we actually load, honestly (tick 493)

`currentSrc` — the read-only "which resource is this `<img>` displaying" URL that lazy-load, lightbox,
gallery and analytics libraries read on every image — was absent (`'currentSrc' in img` was `false`), so
those reads returned `undefined`. The engine loads an `<img>`'s **`src`** attribute directly and does not
yet do srcset/`<picture>` candidate selection for the bitmap (`Page::pending_image_urls` reads `src` and
nothing else), so the honest `currentSrc` is exactly the resolved `src`: the getter returns `this.src` (the
url-type reflection already resolves it to absolute) when a non-empty `src` is present, and `''` otherwise —
which is what the spec wants before a source is selected. Reporting our own loaded URL is truthful;
diverging from Chrome's srcset pick is a *separate* responsive-images capability gap, not a `currentSrc`
bug — the moment we add srcset selection, this getter tracks the chosen candidate for free because it
follows `src`. Read-only (getter only, so assignment is ignored) and installed on `__protoHTMLElement`
IMG-guarded — instances chain through `HTMLElement.prototype`, not the per-tag `HTMLImageElement.prototype`
(which a probe confirmed is *not* in an instance's chain, matching the own-property reflector design), so a
non-image element reads `undefined`. Gate `G_IMG_CURRENT_SRC`. [[js-engine]]

## `document.activeElement` defaults to `<body>`, not `null` (tick 494)

`doc_get_active_element` returned `null` whenever the `ACTIVE_ELEMENT` thread-local was empty (nothing
focused). But `document.activeElement` is read on every interaction by focus-trap libraries, modals and
keyboard handlers, and they assume a real element: `document.activeElement.blur()`, `.tagName`, `=== el`. A
null is a `TypeError` the moment any of those runs — the opposite of graceful. The spec (and Chrome) return
the **body element** for a loaded document, so the getter now falls back to `find_first_in(root, "body")`
(document-correct via `root`, so a created document reads its own body) and to `null` only if there is no
body. It is safe for the engine's own focus logic because every in-engine reader gates on `ae &&
ae.isContentEditable`, and `body.isContentEditable` is false — so a body fallback and a null take the exact
same branch (the contenteditable execCommand host-resolution paths were the ones to check). Focus tracking
still wins: `.focus()` moves `activeElement` to the focused element. Gate `G_ACTIVE_ELEMENT_BODY_DEFAULT`.

## The same defect, three consumers, found by looking for it (tick 578)

Tick 577 fixed `visible_text`. The obvious follow-up question — *which other code assembles a string
from laid-out fragments?* — took one grep and found the identical bug twice more:

| consumer | what it broke |
|---|---|
| `Page::visible_text` | the agent's `Observation.text`, the history search index (t577) |
| `shell::find` | **Ctrl+F**: searching a page that reads `non-mainstream` searched `non- mainstream` |
| `shell::gui` selection | **Ctrl+C**: copying `non-mainstream` pasted `non- mainstream` |

Three authors, three times, the same wrong premise — `find.rs`'s comment even stated it outright:
*"runs joined by a single space (inline layout drops the original whitespace)"*. It does not; it emits
one run per break **opportunity**.

**So the rule moved to the data.** `TextFragment::continues(&prev)` answers *"is this run the same word
as the previous one?"* — same baseline, boxes touching — and all three consumers call it. That is the
difference between fixing three bugs and removing a way to have the bug: a fourth consumer written next
year gets the question handed to it at the point where the geometry is still in scope.

**Each consumer still owns its own assembly**, because they genuinely differ: `visible_text`
concatenates a whole document, `find` needs per-run byte spans so a hit maps back to rects, and
selection groups by line (so its baseline test is implicit and only x-adjacency remains). A shared
`join_runs()` helper would have forced all three into one shape and been abandoned by the first one
that needed spans. **The shared thing is the predicate, not the loop.**

> **The generalisation worth keeping.** When a defect is found in one consumer of a data structure, the
> question is not "are there other bugs like this" but "**what else reads this structure, and does it
> ask the same question?**" — a grep for the type, not for the symptom. Here `BoxContent::Inline` had
> seven readers; three assembled text and all three were wrong.

## The image an `<img>` wants is chosen, not read (tick 582)

`select_image_url` is one function with two callers — `collect_subresources` (the fetch worklist) and
`pending_image_urls` (the decode worklist) — **deliberately**, because two independent selections would
be the two-cascades trap in a different organ: fetch one URL, decode another.

Order: `<picture>`'s `<source>` children *before* the `<img>`, first match wins (so an author can put
their best format first); then the `<img>`'s own `srcset`; then `src`. A `<source>` is skipped when its
`media` does not match or its `type` is one we cannot decode.

Candidate choice is *the smallest that still covers the requirement*, falling back to the largest when
none does — **never the first listed**, which is the cheap wrong answer a naive fix reaches for and which
the gate RED-proves against. `w` descriptors win over `x` when both appear, per spec.

> **`type` matters more here than in a full-featured engine, not less.** `image` is built with
> png/jpeg/gif/webp/bmp/ico and **AVIF is off on purpose** (C dav1d, declined). So the load-bearing use of
> `<picture>` is *skipping* the format we cannot decode and taking the fallback the author shipped for
> exactly that case. Choosing the AVIF would render nothing at all.

## One hardcoded namespace in a four-line function disabled the parser's whole foreign-content mode (tick 603)

Every element inside an inline `<svg>` was in the **XHTML** namespace. The `<svg>` element itself was
correct — which is why this read for a long time as "SVG namespaces mostly work". **The defect
started one level down**, and that is the general shape worth keeping: a capability probed at its
entry point can be wrong everywhere past it.

**html5ever was never the problem.** It implements the tree builder's SVG/MathML *foreign content*
mode correctly, and it decides it is in that mode by asking the sink for the **current node's
qualified name**. `TreeSink::elem_name` answered `ns!(html)` for every element, so the tree builder
could never observe that it was inside an `<svg>`, never switched modes, and built every descendant
as HTML. `create_element` discarded `name.ns` on top of that.

**A borrowed library's correctness is only as good as the answers you give its callbacks.** This is
the same lesson as the Blitz layout contract (Taffy owns containers, the host owns leaf measurement):
the library asks questions, and a lazy answer silently disables machinery you believe you are
getting for free.

Foreign-content mode is not only `namespaceURI`. It also drives **attribute-name adjustment**
(`viewBox` staying camel-cased instead of lowercasing to `viewbox`, `xlink:href`), self-closing tag
handling, and the HTML-breakout rules. One wrong answer turned all of it off, and the gate asserts
`viewBox` survives precisely so the fix cannot be narrowed back to the namespace alone.

### The claim that matters is not "is it right" but "do the two ways agree"

`document.createElementNS(SVG_NS, 'rect')` has kept its namespace since t125. So a page that **builds**
SVG in script and a page that **ships** the same SVG in markup produced two different DOMs for the
same tree. Every library that branches on `namespaceURI`, matches an `svg|rect` selector, or asks
`instanceof SVGElement` was right about one half of the web and wrong about the other — and nothing
anywhere reported a disagreement. `G_FOREIGN_CONTENT_NS` asserts `parsedEqMade`, because that
equality is what a library actually depends on.

It also asserts an ordinary `<div>` stays XHTML: a "fix" that put everything in the SVG namespace
would otherwise pass every other claim in the gate.

⚠ Residue, measured and deliberately not fixed: SVG **children** still report CSS-box geometry from
`getBoundingClientRect` rather than user-space geometry, and `getBBox`/`ownerSVGElement` are absent —
so charting code that *measures* SVG nodes still fails. That is SVG layout, a subsystem, and it is
named rather than half-built.

## `getBBox()` is USER SPACE, and that is the whole reason it exists (tick 604)

`getBBox` was `undefined`, so `node.getBBox().width` was a `TypeError` that killed the caller's
frame — the same throw-class shape as the `getComputedStyle` defect, and it lands on the same code:
D3, Chart.js's SVG paths, and every hand-rolled label placer measures shapes this way.

**The alternative a page reaches for is worse than the missing method.**
`getBoundingClientRect` on an SVG child answers in **CSS-box** coordinates — a *wrong* number, not an
absent one, because SVG children are not CSS boxes here. `getBBox` is defined in the element's **own**
coordinate system: unaffected by where the `<svg>` sits on the page, by the viewport, or by scroll.

That is why it is computed from the element's geometry **attributes** rather than from the layout
snapshot, and the gate proves the distinction rather than asserting it: a `<rect x="10" y="20">`
inside an `<svg style="margin-left:40px">` reports bbox `x = 10` while its
`getBoundingClientRect().x` is `48`. Two numbers, two coordinate systems, both correct for what they
answer.

Exact for `rect`/`image`/`use`, `circle`, `ellipse`, `line`, `polygon`/`polyline`, and containers
(`g`/`svg`/`a`/`switch`) as the union of their measurable children. Two details worth keeping: a
**horizontal `<line>` has zero height** (a bbox routine that takes a max instead of an extent, or
clamps degenerate axes, fails exactly there), and `points` accepts commas *or* whitespace as
separators.

### `<text>` and `<path>` report zero size on purpose

Both need work this function cannot do — shaping, and path-data parsing. **A plausible-looking guess
silently mis-places every label that trusts it; a zero is visible.** That is the same choice
`clip-path` made when it left `shape()` unclipped rather than approximating it, and it is the
conclusion this project keeps arriving at from different directions: **a wrong answer costs more than
an obviously missing one.**

Stroke width is excluded, which is *correct* — `getBBox` is specified on fill geometry. `transform`
is not applied, which is a real gap, named here rather than hidden.

## A getter-only accessor is a TypeError, not a gap — and one site turns it into a white screen (tick 612)

`innerText` was registered with a getter and `None` for its setter:

```rust
prop_guarded!(prop, c"innerText", el_get_inner_text, None);
```

The reflex reading of that is *"writes are unsupported, so they do nothing."* **They do not do
nothing.** Assigning to an accessor that has only a getter raises `TypeError: setting getter-only
property "innerText"` under **strict mode** — so the assignment takes the assigning frame down with
it, along with everything that frame was going to do. Under sloppy mode it silently no-ops. Real
bundles are strict: a module body always is, and every minifier emits `'use strict'`. So the failure
mode a page actually experiences is the throw.

### The failure `www.welt.de` builds on top of it

```text
  ERROR page.console: Failed to load website due to adblock:
                      TypeError: setting getter-only property "innerText"
  structural: 0.0% (3182 paths, 3181 missing)
  MISSING by tag: div×940  a×447  li×390  span×346  h4×103  ul×100  article×98  svg×89
```

welt.de's anti-adblock check writes through `innerText`, **catches the throw**, concludes an ad
blocker is present, and **deliberately blanks its own document**. We were not failing to render the
page; the page was refusing to render for us — and on the evidence it had, it was right to conclude
something was wrong with the DOM. Chromium renders the same fetched bytes completely.

**This is a distinct and nastier class than a missing feature.** A missing feature degrades the thing
that uses it. A missing *setter* on a widely-written property is an exception that (a) kills unrelated
work in the same frame and (b) is a signal some sites deliberately interpret as hostile.

### The population, measured rather than assumed

Scanning each HEAD site's HTML **plus up to 12 of its external bundles** for `.innerText =`:

```text
  bbs.ruliweb.com   9      www.welt.de     2
  www.desitales2    6      www.aparat.com  1
  → 4 of 16 scanned sites WRITE innerText
```

A quarter of the stratum, and a **lower** bound: 12 bundles per site, computed access (`el[p] = v`)
uncounted, and five sites answered with bot-wall pages carrying none of their real scripts.

### The rest of the class, from the same sweep

`innerText`'s sibling `outerText` had a setter the entire time — one rule, two implementations. Of the
61 getter-only DOM properties, most are correctly read-only (`childNodes`, `tagName`, `firstChild`),
but these are **spec-settable and still throw here**:

| Property | Why it is settable |
|---|---|
| `nodeValue` | writes character data, like `data` |
| `style` | `[PutForwards=cssText]` — `el.style = 'color:red'` |
| `classList` | `[PutForwards=value]` — `el.classList = 'a b'` |
| `document.body` | replaces the body element |
| `selectionDirection` | on `<input>`/`<textarea>` |

### The setter itself

The HTML spec calls it the **rendered text fragment**: normalise `\r\n` and `\r` to `\n`, split on
`\n` into text nodes separated by `<br>`, then replace all children. That is the same construction
`outerText` performs — the two differ *only* in whether the result replaces the element's CHILDREN or
the ELEMENT — so the splitting lives in one shared helper. Two copies is how the two answers drift.

⚠ **`innerText` must never parse markup.** It is the safe sibling of `innerHTML` and is precisely
where pages put untrusted text. `G_INNER_TEXT_SET` asserts an assigned `<img src=x onerror=1>` yields
zero `<img>` elements; a RED probe that routed the setter through `set_inner_html` turns that into
`noParse:1`, which would be a stored-XSS sink wearing a safe name.

⚠⚠ **The gate's own headline assertion was vacuous on the first draft, and only the RED probe said so.**
`threw:false` was written at script top level — **sloppy mode** — where the getter-only assignment
no-ops instead of throwing, so it **passed with the bug still present** while every other assertion in
the same gate went correctly red. The claim the whole gate exists for could not fail. The fix is an
IIFE with `'use strict'`. *A gate can be green because a SECOND mechanism produces the same
observable* — vary the mechanism, not the threshold. [[honest-answer-is-not-a-fixed-answer]]

## `window.<id>` IS the element — named access on the Window object was absent (tick 677)

HTML §7.3.3 says the Window object exposes a **named property for every element with an `id`**, plus
the `name=` of a `form` / `img` / `embed` / `object` / `iframe` / `frame`, plus every nested browsing
context's name. All of it was `undefined` here, and `'myThing' in window` answered **false**.

### What it cost, measured on a HEAD-20 site

`playhop.com` renders **5 of the 107 elements Chrome builds** — 102 missing boxes, its entire
application subtree. The whole failure is two log lines:

```text
TypeError: can't access property "innerHTML", window.__appData__ is undefined  at inline.js:1:155
TypeError: can't access property "gamesStore", window.appData  is undefined    at inline.js:1:1466
```

The page ships its server state as a **data island** —

```html
<script id="__appData__" type="mime/invalid">{"advPartnerInfo":…}</script>
…
<script>window.appData = JSON.parse(unescape(window.__appData__.innerHTML))</script>
```

— deliberately a non-JS `type` so the payload is inert, read back through `window.<id>`. No named
access ⇒ no state ⇒ no app, and the certificate filed it as `thin-overlap`: *a coverage failure
wearing an unscored label.* **The pattern is not exotic.** It is how server-rendered state has been
handed to client JS since long before hydration had a name, and it needs no framework.

⚠ **Those two lines are readable at all because of tick 675 and tick 666.** Before the address was
lifted, this was `TypeError: can't access property "innerHTML", ... is undefined` with no file and no
line — a status. `inline.js:1:155` is what made it a lever in one run.

### The mechanism, and the three ways it could have been subtly wrong

`__publishNamed()` walks `[id],[name]` and defines an accessor on the global for each new name.
Incremental (one `querySelectorAll`, only unseen names defined), called at the two script entry
seams — `run_one_script` **before** each script, and `PageContext::eval` for runtime-fetched code.

1. **A real `Window` property WINS.** In the spec the named properties live on an object in Window's
   PROTOTYPE CHAIN, so `<div id="location">` must not shadow `window.location`. Here they can only be
   own properties, so an `if (nm in g) continue` guard enforces the same order. Removing that guard is
   one of the gate's two RED mutations, and it fails in the dangerous direction: navigation breaks on
   any page that happens to id an element `location`, `history` or `top`.
2. **The getter re-resolves by id at ACCESS time**, so it follows a replaced element instead of
   pinning the node that existed when the name was published. A cached node is a use-after-remove
   waiting for a framework to re-render.
3. **`enumerable: false`.** These are own properties here and are *not* in a real browser, where
   `Object.keys(window)` does not list a page's ids. Code that enumerates the global — feature
   detection, sandbox shims — must not start seeing every element on the page.

Assignment is honoured: `window.foo = 1` replaces the accessor with a plain value, which pages do
constantly (`window.appData = …` is the very next line of playhop's boot).

⚠ **RESIDUE, named rather than hidden:** a name becomes reachable at the next script entry, not the
instant the element is inserted. An element created and read back through `window.<id>` inside ONE
script still misses. A real browser resolves continuously; this resolves at a seam.

### What it bought, stated honestly

**playhop's two boot throws are GONE** — the site's script log is now empty where it carried two
TypeErrors. **Its coverage did not move: 4.7%, 102 missing, `thin-overlap-5`.** The blocker moved from
*"the app cannot start"* to *"the app does not converge inside our task ceiling"*: nine
`event loop hit its task ceiling · count=20000` warnings and a 12s load budget exhausted against a
30s load. `naukri` (3.5%) and `agoda` (7.2% / SHAPE 58.6%) are unchanged and carry the same shape —
`OURS IS SLOW: 23–32s` on all three.

That is a capability win buying a **better failure**, not a scored row, and it is the third time this
session that distinction has had to be drawn. It is still a ratchet tooth: the capability is general,
gated, and RED-proven, and the next question is now a *timing* question with an address rather than a
TypeError with none.

[[frameworks]] [[conformance-and-oracles]]

## The second document — a `document` method that belongs to the singleton (tick 776)

`Document.prototype` is not decorative in this engine. `dom_bindings::dom_protos` builds it in Rust as
`__protoDocument`, and **every** document genuinely inherits from it — the singleton, an `<iframe>`'s,
and one made by `DOMImplementation.createHTMLDocument()`. Measured, both directions:
`Object.getPrototypeOf(document) === Document.prototype` and the same for a created document.

Nineteen `document` methods were nevertheless **own properties of the singleton**, because every
JS-side shim was written the obvious way:

```js
document.createRange = function () { … };        // an own property of ONE document
```

So the family — `createRange`, `createNodeIterator`, `createTreeWalker`, `createAttribute`,
`createAttributeNS`, `createEvent`, `evaluate` — did not exist for any other document in the page, and
touching one was a `TypeError`, not a `false`.

**What it actually broke, measured rather than assumed.** The live evidence is one corpus throw:
`TypeError: b.createRange is not a function` out of Google's CSE `dynamic.js`, in the tick-776 sweep —
a direct call on a document that is not the singleton. Plus every `Document.prototype.X = wrapper`
patch, which was a silent no-op.

### ⚠ The gate that owns this ground passes, and the reason is worth more than the fix

`G_SECOND_DOCUMENT_IS_REAL` was built for exactly this territory. Its central claim is

```js
document.createNodeIterator.call(b.ownerDocument || b, d.body, …)
```

and its header says, correctly, that the expression is *transcribed from the library rather than
invented*: DOMPurify destructures `createNodeIterator` off the original document and `.call`s it with
the parsed root's document as `this`. So the transcription is faithful — and it takes the **function
from the singleton**, supplying only the receiver. It exercises the algorithm over a second document
while never performing the property lookup on one. It passes for precisely as long as
`otherDoc.createNodeIterator` is `undefined`.

Two consequences, and the first is the one this tick nearly got wrong in its own commit message:

* **DOMPurify was never broken by this.** Its `.call` idiom routes around the defect entirely. The
  affected callers are the ones that write `doc.createX(…)` — which is what Google's `dynamic.js`
  does, and what the spec expects of everyone.
* **A gate that reaches a method off a known-good receiver and `.call`s it onto the subject has
  tested the algorithm and skipped the lookup.** `G_DOC_PROTOTYPE` therefore calls through the
  subject's own property, every time.

### The trap: a `this`-blind promotion is WORSE than the throw

Hanging the existing closures off the prototype "fixes" every presence check while making them operate
on the **wrong** document. For a sanitiser-shaped caller that inverts a security property — a range built
from an inert parsed copy would point into the live page. So each promoted method resolves its document from
`this` (`__thisDoc`, which falls back to the singleton only for a detached call), and `G_DOC_PROTOTYPE`
asserts **ownership** — a range made from a created document is rooted in *that* document and not in
the live one. That claim is the one the over-broad fix fails; every "did it return a Range" claim
passes it.

Deliberately **not** promoted, and named rather than left to look like an oversight: `getSelection`,
`execCommand`, `hasFocus`, the fullscreen exits, `getAnimations`, `startViewTransition`. Those are about
the *displayed* document, and a created document has no selection to get.

### One source, two eval sites

`__defDoc`/`__thisDoc` live in `event_loop::DOC_PROTO_JS` and are evaluated from **both** install paths,
because the install order is not one order: `dom_bindings::install` runs `WINDOW_PRELUDE` (which defines
`createEvent`) before `event_loop::install` exists, while a bare JS context gets `event_loop::install`
with no `dom_bindings` at all. Putting the helper inside either prelude leaves the other path calling an
undefined function — and a `TypeError` at the top of a prelude takes the whole prelude with it.

The second thing this buys is **patchability**, the [[js-engine]] `G_PROTOTYPE` lesson one interface
over: `Document.prototype.createRange = wrapper` used to be a silent no-op, because the wrapper landed
on an object no document consulted. That is how every error tracker, ad-blocker and polyfill hooks the
DOM.

[[js-engine]] [[frameworks]] [[conformance-and-oracles]]

## A DOM node must NAME its interface — `[object Object]` is a wrong answer of the right type (tick 862)

`Object.prototype.toString.call(someDiv)` answered **`[object Object]`** here, where every browser
answers `[object HTMLDivElement]`. `node.constructor.name` answered the string **`"Object"`**. Both
were true of every node in every document, on every page, for the whole life of this engine.

That is not a cosmetic read. It cost `www.otomoto.pl` — a **server-rendered** page whose ~1,300-tag
document arrives complete over the wire — its entire DOM, in nine consecutive certification sweeps:

```text
  {}.toString.call(div)  ==  "[object Object]"        (Chrome: "[object HTMLDivElement]")
    -> tippy.js  isElement(t) = str.indexOf('Element]') > -1   ->  FALSE
    -> tippy() ends `return isElement(targets) ? instances[0] : instances`
    -> the caller is handed an ARRAY where it expects an instance
    -> TypeError: can't access property "popperOptions", r.props is undefined
    -> TypeError: r.destroy is not a function
    -> React error boundary -> Next.js "client-side exception" -> renders /_error
    -> THE SERVER-RENDERED DOM IS TORN DOWN AND REPLACED WITH NOTHING
```

Coverage `0.004` and a blank white screenshot, tagged `render-failed` — the one unscored reason the
fidelity instrument says out loud is *our* bug. After the fix: coverage **0.968**, `shape 0.762` over
1,047 elements, scored.

### Why nine sweeps did not find it

Every instrument that asks *"is this element there"* said yes. `typeof div === 'object'` — true.
`div.nodeType === 1` — true. `div instanceof Element` — **true**, because `Symbol.hasInstance` has
been correct here for many ticks. Only the *brand* was wrong, and a brand is not something a caller
feature-detects; it is something a caller believes. This is the [[conformance-and-oracles]] pattern
booked at t733-736: **the dominant bug shape is a wrong answer of the RIGHT type**, findable only by a
probe that carries the EXPECTED value — here, four lines of `toString.call(...)` run through our
engine and through `chromium --dump-dom`, diffed.

### The fix is ONE accessor at the ROOT of the chain, not WebIDL's shape

WebIDL puts a `Symbol.toStringTag` data property on each interface **prototype object**. That requires
per-tag prototypes, and this engine has five (`dom_bindings::dom_protos`): every element, whatever its
tag, is

```text
  instance -> HTMLElement.prototype -> Element.prototype -> Node.prototype -> EventTarget.prototype
```

A data property on `HTMLElement.prototype` would therefore brand a `<div>` and an `<a>` **identically**
— the right shape carrying the wrong answer. So the brand is an **accessor on `EventTarget.prototype`**,
resolving from the object it is called on, and the same pair resolves `constructor`. Tiering the
prototypes properly is a separate tick; the observable is what pages read.

### The table is taught by the `iface()` calls, not written twice

The only place that already knows a `DIV` is an `HTMLDivElement` is the ~70-entry
`iface('HTMLDivElement', tagIs('DIV'))` list. A parallel table beside it is [[js-engine]]'s
*"one rule, N implementations"* (t720-724) in its purest form, so `tagIs`/`tagIn` stamp their tags
onto the predicate and `iface()` harvests them. **A new interface added to that list gets its brand
for free.**

### The named limits

* An SVG element we do not name individually brands `SVGElement` where Chrome gives `SVGSVGElement` —
  coarser, and still a true statement about it. The fallback is chosen by `namespaceURI`, because
  calling an SVG element an `HTMLUnknownElement` would be actively wrong rather than merely vague.
* `<my-thing>` is `HTMLElement` (a valid custom-element name) and `<out>` is `HTMLUnknownElement`.
  Getting that pair backwards is the one way the element arm can be actively wrong, so it reuses the
  same `KNOWN_SET` that `iface('HTMLUnknownElement', …)` narrows on.
* `document` is **`HTMLDocument`**, not `Document` — a detail nobody recalls correctly, which is why
  `G_BRAND`'s expectations are transcribed from a real `chromium --dump-dom` run of the gate's own
  fixture rather than from memory.
* Non-node platform objects (`CSSStyleRule`, `IDBDatabase`, a 2D context) keep `[object Object]`.
  Their `instanceof` predicates are duck-typed, so there is no object to hang a brand on without
  inventing one.
* `document.doctype` needs a **second mechanism**: it is not a reflector but
  `Object.create(DocumentType.prototype)`, so it takes WebIDL's own form on its prototype. Two
  mechanisms for one rule is a smell, and it is written down at both sites rather than hidden.

[[js-engine]] [[frameworks]] [[conformance-and-oracles]] [[fidelity-instrument]]

## A `<template>`'s `innerHTML` is its CONTENTS — and a lazy fragment is what made one ordering work

DOM Parsing is explicit about the redirect: *"if context is a template element, then set context to
the template element's template contents"*. A `<template>` element's own child list is **always
empty** in a real browser; `.content` is the only place its markup lives. `manuk_html::set_inner_html`
wrote to the child list, and `serialize_inner` read from it.

**Why that survived for so long, and it is the instructive part.** `Dom::template_content`
materialises the content fragment **lazily and once**, moving the element's direct children into it
on first access. So the single ordering anybody had ever written a test for — *set `innerHTML`, then
read `.content`* — worked by accident, and nothing else did:

```text
                                                       Chrome   manuk (before)
  innerHTML, THEN read .content                            1       1    <- the only case tested
  read .content, THEN innerHTML                            1       0    ... and .childNodes was 1
  innerHTML TWICE (2nd writes two nodes)                   2       1    <- the FIRST write's node
  t.innerHTML = t.innerHTML  (getter half)                kept   ERASED
```

**What it cost.** Vue 3's `runtime-dom` keeps ONE module-level `<template>` and writes it on every
static block:

```js
  Pw.innerHTML = tI(s==="svg" ? `<svg>${t}</svg>` : t);
  const a = Pw.content;
  if (s==="svg" || s==="mathml") { const l = a.firstChild; for(; l.firstChild;) a.appendChild(l.firstChild); a.removeChild(l) }
```

From the second block onward `a` is the stale fragment, `a.firstChild` is `null`, and `l.firstChild`
throws *"can't access property firstChild, l is null"* — **inside an async render, where nothing is
listening**. One throw and the app is over: measured on `pt88.app`, which went from **three**
comparable elements to **132 scored** once the redirect landed, and `portal.ensuretyfinance.com`
crossed the M1 shape bar (0.864, coverage 100%).

**The shape to carry forward:** *a lazily-materialised cache turns an ORDERING bug into a bug that
only one ordering can see.* When a capability is implemented by a lazy accessor, the gate must write
the state in every order, not just the order the implementation happened to make work.

**Named residue, a DIFFERENT mechanism.** The fragment parser loses foreign content: an `<svg>` in
`template.innerHTML` comes back `nodeName: "SVG"` in the **xhtml** namespace where Chrome gives `svg`
in `http://www.w3.org/2000/svg`. Document parsing is correct (`g_foreign_content_ns`); the
`parse_fragment_in` → `clone_into` path drops the namespace. Vue's hoisting does not read the
namespace, so it is not what killed the page — it is its own tick.

## The parser was right and every COPY was wrong — namespaces across `clone_into` and `clone_node`

`G_FOREIGN_CONTENT_NS` has gated document-parsed foreign content since t602. What it did not ask is
whether a **copy** of a correct element is still correct. It was not, in either of the two places a
copy is made — `manuk_html::clone_into` (`innerHTML`, `insertAdjacentHTML`,
`createContextualFragment`) and `dom_bindings::clone_node` (`cloneNode`, `importNode`) — because both
built every node with `create_element`, which is the HTML namespace unconditionally.

```text
                                              Chrome              manuk (before)
  document-parsed <svg>                  2000/svg | svg      2000/svg | svg    ✓
  innerHTML '<svg>…'                     2000/svg | svg      1999/xhtml | SVG  ✗
  cloneNode of the CORRECT parsed svg    2000/svg | svg      1999/xhtml | SVG  ✗
  tpl.content.cloneNode(true)            2000/svg | svg      1999/xhtml | SVG  ✗
  importNode                             2000/svg | svg      1999/xhtml | SVG  ✗
  createElementNS control                2000/svg | svg      2000/svg | svg    ✓
```

**Cloning an element the parser got right produced a wrong one**, which rules the parser out without
reading a line of it. Two implementations of one rule, in two different crates — so no local reading
of either would have found it. `nodeName` is the tell a namespace-only check misses: a foreign
element's name is **not** uppercased, and ours read `SVG`.

### What it costs — measured, because the obvious answer is wrong

The tempting claim is that an `<svg>` in the HTML namespace is an unknown inline element with no
intrinsic ratio that never paints. **False here**, and checked on both binaries before it was
written down:

```text
                                                    Chrome    after    BEFORE
  innerHTML <svg viewBox="0 0 200 100"> in 400px    400x200   400x200  400x200
  innerHTML bare <svg>                              300x150   300x150  300x150
```

Our layout keys on the TAG, so geometry never depended on the namespace. The real cost is the one
`parsedEqMade` already names one layer up: **the same markup reached two ways produced two different
DOMs**, so every library that branches on `namespaceURI`, matches `svg|rect`, or asks `instanceof
SVGElement` — D3, Chart.js, Snap.svg, every icon set that injects markup — was right about parsed SVG
and wrong about injected SVG, with nothing reporting a disagreement.

**Named residue:** `getComputedStyle(rect).fill` is `undefined` where Chrome says `rgb(255, 0, 0)` —
SVG presentation attributes do not reach computed style, which is exactly what charting code reads
back.

## `element.style` is a raw-string Proxy: the setter validates nothing, and the two feature-detection idioms lie in OPPOSITE directions

`element.style` is a JS `Proxy` over the element's `style` **attribute text** (`CSSOM_PRELUDE` in
`engine/js/src/dom_bindings.rs`). Its `set` trap writes `String(v)` into that text unconditionally,
so **every value round-trips verbatim**:

```text
   e.style.color = "yelow"                 ->  "yelow"        Chrome: ""
   e.style.color = "rgb(255 0)"            ->  "rgb(255 0)"   Chrome: ""
   e.style.display = "not-a-thing"         ->  "not-a-thing"  Chrome: ""
```

**The cost is not the WPT subtests — it is the detection idiom the whole web is written in:**

```js
   const e = document.createElement('div');
   e.style[prop] = value;
   return e.style[prop] !== '';     // ← TRUE FOR EVERY VALUE, so every capability reads "supported"
```

A page therefore takes the modern branch for capabilities this engine does not have. And it is the
**exact mirror** of the other hole in the same object: `'display' in el.style` is `false` (t1172), so
the `in`-based idiom reads *unsupported* for everything we DO have. One object, two idioms, both
lying, in opposite directions — and neither is visible to a rendering test.

**Chrome's contract, measured over 40 rows:** invalid → `""`; valid → stored and **canonically
serialized** (`hsl(120 30% 50%)` → `rgb(89, 166, 89)`, `#ff0000` → `rgb(255, 0, 0)`, `RED` → `red`,
`"  red  "` → `red`). The outcome tracks `CSS.supports(prop + ': ' + value)` on every row **except**
`color: red !important`, which the IDL setter must reject even though `CSS.supports` says true — a
priority may only arrive via `setProperty(k, v, 'important')`.

### …but `CSS.supports` is not yet a safe validator, and a NEGATIVE row is what proved it

Our `CSS.supports` is Chrome-exact on every trap that matters — `inherit` / `initial` / `unset` /
`revert`, `var(--x)`, `var(--x, red)`, `rgb(255 0 0 / var(--a))`, `calc(100% - 10px)` vs the invalid
`calc(100% -10px)`, and **every custom-property row**. It answers `false` for six declarations Chrome
supports, and the split is the whole point:

```text
   -webkit-line-clamp: 3      false   ← WE RENDER IT (t413, gated)     -> a FALSE NO
   scrollbar-width: thin      false   ← constellation `gated`          -> a FALSE NO
   -webkit-box-orient         false   ← constellation `missing`        -> HONEST
   content-visibility: auto   false   ← constellation `missing`        -> HONEST
   text-wrap: balance         false   ← constellation `missing`        -> HONEST
   anchor-name: --a           false   ← constellation `unknown`        -> HONEST
```

> **`CSS.supports` is not a question about Chrome. It is a question about THIS engine.** Writing
> Chrome's answers into a battery's expectation column makes four correct answers look like bugs.
> Only where `CONSTELLATION.tsv`'s **`capability` column** says `gated` is Chrome's `true` also ours.

Validating the setter through this seam **today** would make `el.style.webkitLineClamp = 3` a silent
no-op and delete a shipped capability. The two false NOs come from properties recovered through the
MinimalCascade merge that Stylo's servo build cannot parse at all (`-webkit-line-clamp` is
`engine="gecko"` in stylo 0.19), so `@supports` never parses the condition and `unwrap_or(false)`
answers. `honest_supports`'s denylist turns false YESes into NOs; these need the missing opposite —
an allowlist applied to the RAW condition **before** Stylo sees it, value-validated through
`MinimalCascade`, with the constellation's `gated` status as the entry criterion.

**Order matters and each step gates the next:** allowlist → setter validation → canonical
serialization (the largest piece, a value serializer per property).

## A computed style must answer to its own CSS property name — the DASHED ATTRIBUTE (tick 1179)

`getComputedStyle(el)` returned a snapshot object with **camelCase slots only**. So this was false:

```js
  'margin-left' in getComputedStyle(el)     // false
  getComputedStyle(el)['margin-left']       // undefined
```

for `margin-left` — a property this engine has cascaded, laid out and painted correctly for a
thousand ticks. CSSOM's *"CSS property to IDL attribute"* rule defines **three** attributes per
supported property, and we had shipped one:

| attribute | example | had it |
|---|---|---|
| camel-cased | `marginLeft` | ✅ |
| webkit-cased | `webkitUserSelect` | ✅ |
| **dashed** — the CSS property name itself, for every name containing `-` | `'margin-left'` | ❌ |

### Why the third one is not a spelling convenience

It is the **first line** of `wpt/css/support/computed-testcommon.js`, which is how the CSS test
corpus asks every computed-value question, and it passes the **dashed** name:

```js
  assert_true(property in getComputedStyle(target),
              property + " doesn't seem to be supported in the computed style");
```

So the corpus reported *"margin-left doesn't seem to be supported in the computed style"* — and
every subtest under that helper died **before a value was ever read**. Histogramming the assertion
messages of `css/css-values` (not the test names) put this sentence at the top of the area:
`letter-spacing` 24, `background-image` 24, `object-position` 28+40, `z-index` 6, `margin-left` 6 —
all properties we have. The area's headline said *values*; the failure was *the object's IDL surface*.

> **Histogram the ASSERTION MESSAGE, not the test name.** The test names said `calc()`, `attr()`,
> `if()`. The messages said the helper never got past its own first line.

### The shape of the fix, and the two things that keep it honest

`computed_style_js` emits `(function(){var o={…};var a=[["margin-left","marginLeft"],…];for(…)
{var v=o[a[i][1]];if(v!==undefined)o[a[i][0]]=v;}return o;})()` — an object literal cannot reference
itself while it is being built, so the aliases are installed one statement later. Pairs are emitted
from Rust rather than a name list the JS re-derives, because this runs on **every `getComputedStyle`
call**, which is already a forced-reflow trigger; a per-call regex over ninety names is real work.

- **`if(v!==undefined)` is the honesty clause.** A name whose camel slot this build does not emit
  stays *absent*, so `'view-transition-name' in cs` is still **false**. `in` is a question about THIS
  engine — the same rule that governs `CSS.supports` one section above. A blanket `true` would be
  t1177's lie wearing a new hat.
- **A custom property is NOT a dashed attribute.** Chrome answers `'--brand' in cs` → `false` and
  routes custom properties through `getPropertyValue` alone. We match that.

`COMPUTED_STD_NAMES` was hoisted to module scope for this: the enumeration list (`length` / `item(i)`)
and the alias list now derive from **one** array, because a hand-copied second list is how
`length` drifted from the property count before (tick 597).

⚠ **The attribute set is deliberately LARGER than the enumeration set.** `user-select`,
`color-scheme` and the `-webkit-` spellings get dashed attributes without joining `__n`. That is not
sloppiness — it is Chrome's shape too: `length` counts *declarations*, while the IDL attributes exist
for every *supported property* regardless of whether one is set.

## `CSS.supports` answers a false NO for what we render — `RECOVERED_LONGHANDS` (tick 1180)

The section above (t1177) priced `el.style` setter validation and **refused it**, because the
`CSS.supports` seam it would validate through answers `false` for properties this engine ships. This
tick makes the seam honest. Two things it predicted turned out to be wrong, and both are worth
keeping because each would have cost real work.

### CORRECTION 1 — it is FOUR properties, not six, and the probe needed no expectation column

t1177's battery listed six declarations `CSS.supports` denies that Chrome supports. **Four of those
six were the instrument being correct.** The reason the count was wrong is the reason the whole
battery was refused: *Chrome's* answer had been written into the expectation column of a question
about *this* engine.

The replacement probe has **no expectation column at all**. For every property the computed-style
snapshot answers to, it asks `CSS.supports(p, cs[p])` — the value the engine itself just produced,
and therefore by construction one it supports:

```text
   ASKED 139 properties   FALSE_NO 4  (+1 absent)
     scrollbar-width · scrollbar-color · scroll-snap-type · scroll-snap-align
     -webkit-line-clamp — ABSENT from the computed snapshot entirely
```

> **A self-calibrating probe cannot inherit the author's beliefs.** Ask the engine about its own
> output and every `false` is a proven contradiction, with nothing recalled and nothing borrowed.

`-webkit-line-clamp` is the one the probe *could not* see — it is recovered by the merge and consumed
by layout but never reaches the computed snapshot, so it came back `ABSENT`, not `false`. Its
evidence is its own unit test. **A probe's blind spot is not an absence of the thing.**

### CORRECTION 2 — the plan named the wrong hook, and the existing one was already right

t1177 specified *"an allowlist applied to the RAW condition **before** Stylo sees it (Stylo cannot
parse these, so the existing `rewrite_parse_only` hook is too late)"*. That is a whole raw-text
pre-parser, and it was **not needed**: `SupportsCondition::Declaration` holds the raw `prop: value`
slice, so Stylo parses the *condition tree* perfectly well and merely evaluates the declaration
false. The existing hook was in exactly the right place, and `RECOVERED_LONGHANDS` is nine lines in
the same match arm as the denylist.

> **"Stylo cannot parse this property" and "Stylo cannot parse this condition" are different
> sentences.** The first is true; the plan carried it one level up, where it is false, and budgeted a
> subsystem for it.

The payoff is that composition is free — the declaration is swapped for `ALWAYS_SUPPORTED`
(`color: red`) or `NEVER_SUPPORTED` and **Stylo** resolves the surrounding `and`/`or`/`not`, exactly
as the denylist half already does. `not (scrollbar-width: thin)` is false and
`not (scrollbar-width: banana)` is true, with no boolean logic written here.

### The value half is not optional

A name-keyed allowlist would answer yes to `scrollbar-width: banana` — which is t1177's
`el.style.color = "yelow"` lie moved one layer down, and **worse here, because the next tick's setter
is going to trust this answer.** So `recovered_value_valid` writes the grammar out per property
rather than routing through `apply_declaration`:

> **The cascade's parser is LENIENT BY DESIGN and a `supports` answer must not be.**
> `-webkit-line-clamp`'s own arm says *"`none`/`0`/garbage → unclamped"*, because a cascade must not
> abort a page over a bad value. Reusing it would have made every value valid — the exact failure the
> function exists to prevent. `-webkit-line-clamp: 0` is the row where the two must differ.

⚠ **`scroll-snap-type: none` kills the cheap validator.** *"Apply it and see whether the computed
style changed"* answers NO for every property asked about its own initial value — and `none` is both
the initial value and a perfectly valid one.

### The entry criterion, so the list cannot grow by opinion

A property belongs iff **both**: it reaches a `ComputedStyle` field through the `MinimalCascade`
recovery merge, **and** its `CONSTELLATION.tsv` row says `gated` with the gate named. `text-wrap`,
`content-visibility`, `-webkit-box-orient` and `anchor-name` fail the second test and stay honest
NOs. All 30 claims — negatives first — are held by `G_SUPPORTS_HONESTY`.

⚠ **One drift recorded, not acted on:** `display: -webkit-box` IS applied by that same merge
(`legacy_webkit_box`), while constellation rows 353/423 say `missing`. The row is probably stale, but
widening an allowlist on a row nobody has measured is how the false YES gets back in.

## `el.style` validates: the setter drops what does not parse (tick 1181)

`element.style` is a `Proxy` over the `style` attribute text, and its `set` trap wrote `String(v)`
straight in. **Nothing was validated, ever** — `e.style.color = "yelow"` stuck. CSSOM says a
declaration whose value does not parse is simply not set.

The cost is not the conformance. It is the feature-detection idiom every CSS-touching library ships:

```js
   const e = document.createElement('div');
   e.style[prop] = value;
   return e.style[prop] !== '';        // TRUE FOR EVERY VALUE, ALWAYS
```

Every probe answered *supported*, so a page took the modern branch for capabilities this engine does
not have — **and threw away the fallback it had shipped for exactly that case.** It is the mirror of
t1172's `'display' in el.style === false`, the same object answering *unsupported* for everything we
DO have. One object, two detection idioms, both lying, in opposite directions.

### The method: price the LOSS, not the gain, against the corpus that defines both

The gain was never the question — `test_invalid_value` is 1,978 call sites under `~/wpt/css`, all
failing. The question was **how many working declarations the fix would delete.** So every
`(property, value)` pair from every `test_valid_value` and `test_invalid_value` call site was
extracted and asked of `CSS.supports`, and **both halves published**:

```text
   INVALID  n=1000   supports=false 1000   supports=TRUE   0
   VALID    n=1467   supports=true  1042   supports=FALSE 425   <- the declarations we would DELETE
```

> **COUNT the property, then READ WHAT THE VALUES SAY.** The 425's histogram is `display` 54,
> `color` 42, `width` 16, `text-indent` 8 — properties this engine unquestionably renders, which
> reads like a false-NO class big enough to kill the tick. The values say otherwise: all 42 `color`
> rows are CSS Color 5's `alpha(from …)`, all 54 `display` rows are multi-keyword (`run-in`, `ruby`,
> `list-item flow-root`), all 16 `width` rows are `calc-size()`. Not one is a value we support and
> deny. At the property axis the counts said *disaster*; at the value axis they said *honest*.

**The forecast held: zero areas down.** Those 425 were already failing their *serialization*
assertion — a value echoed back uncanonically never passed `test_valid_value` anyway — so declining
it costs a subtest that was not being scored. A forecast that names its own downside is what makes
the +2,714 believable.

### What the fix does not do, on purpose

| path | validates | why |
|---|---|---|
| `el.style.color = v` (IDL) | ✅ and rejects a `!important` in the value | Chrome-measured (t1177): the spec forbids a priority through this path |
| `el.style.setProperty(k, v, prio)` | ✅ value only | `setProperty(k,v,'important')` is the ONLY path to a priority |
| `--custom: anything` | ❌ never | a custom property has no grammar; validating one deletes every design token on the page |
| `el.setAttribute('style', …)` | ❌ still raw | needs a per-property serializer — t1177 step 3, and the larger job |

⚠ **Memoised per `(property, value)`, and the pair matters.** `__cssSupports` builds and parses a
Stylo stylesheet; `el.style.transform = …` in a rAF loop would pay that every frame, so conformance
would have been bought with a performance regression the ratchet refuses. One process-wide entry
serves every element and every frame because the answer is pure in the pair — and a cache keyed on
the *value* alone would serve `color: red`'s YES to `width: red`. `G_STYLE_SETTER_VALIDATES` has a
row for exactly that.

### The result, and what it says about the board

**+2,714 subtests across thirteen CSS areas, 0 down, HANG/CRASH 0 in every one** — including
**`css/css-grid` +300**, the board's #1 lever for fourteen ticks, which every steer on record reads
as the M1 layout slog to be ported from blitz/servo. It moved by twelve lines in a JS `Proxy`.

> **An area is a directory, not a cause.** Three times in one window a *shared mechanism* beat the
> per-area ranking — t1176's missing helper library, t1179's dashed attribute, and this — and all
> three were invisible to a ranker that reads areas. Rank by area to find the mass; histogram the
> assertion messages to find the organ.

## `DOMParser.parseFromString` ignored its second argument — XML was parsed by the HTML parser (tick 1189)

`new DOMParser().parseFromString(s, 'text/xml')` built a `createHTMLDocument()` and HTML-parsed the
string. The MIME type was accepted, stored nowhere, and acted on never. Four wrongs on one line:

| | HTML parser does | XML requires |
|---|---|---|
| **case** | lowercases every tag name | case-sensitive |
| **namespace** | forces XHTML on everything | whatever `xmlns` declared |
| **malformed input** | error recovery, always a tree | fatal → `parsererror` document |
| **`contentType`** | reported `text/html` | the type the caller named |

**The symptom is an SVG string with no `<clipPath>` in it.** Parse SVG markup through `DOMParser`
and `clipPath`, `linearGradient` and `textPath` come back as `clippath`, `lineargradient`,
`textpath` — matching no selector, resolving against no SVG attribute set, painting nothing, with
no error raised. The same line is how JS reads an RSS/Atom feed, a SOAP body and a sitemap.

### The engine was TOLD the answer and threw it away

The prelude did try: `try { doc.contentType = type || 'text/html'; } catch (e) {}`. But
`document.contentType` is a **native getter with no setter**, so the assignment threw, the `catch`
swallowed it, and the document went on reporting `text/html`. The getter itself was:

```rust
unsafe fn doc_get_content_type(cx, _argc, vp) -> bool {
    return_string(cx, vp, "text/html");   // every document, unconditionally
    true
}
```

> **A silent no-op wrapped in a `catch` is worse than an absent feature.** The code reads as though
> content type is handled. Nothing logs, nothing throws, and the wrong answer is a plausible one.

### Content type is per DOCUMENT, not per arena — and that is the non-obvious part

The tempting move is to copy `quirks`, a `bool` on `Dom` that reaches every consumer with no
signature change. **It would have been wrong.** `quirks` is a property of *the parse that built the
arena*, but **one arena holds many documents**: `create_document` mints a fresh `Document` node for
every `createHTMLDocument`, every `parseFromString` and every iframe. A single field on `Dom` would
make the newest parse retroactively rewrite what every earlier document claims to be.

It is therefore a `HashMap<NodeId, String>` keyed by node. `NodeId` **packs the slot generation**,
so a freed-and-reused slot mints a different key and cannot inherit the previous occupant's type —
the map is generation-safe by construction, and `free_slot` drops entries for hygiene only.

### The parser is a PORT, not a second implementation

`xml5ever` shares html5ever's version train *and* its `markup5ever::TreeSink` trait, so
`ArenaSink` — the existing sink that parses straight into the arena — backs both parsers
**unchanged**. `parse_xml` is the same sink, the same arena, a different tree builder. This is the
board's "port whole algorithms, don't reverse-engineer" rule paying out at near-zero cost.

### One producer of `XMLDocument`, and it is NOT `DOMParser`

The narrow predicate is the whole subtlety:

```js
iface('XMLDocument', function(o){ return !!o && o.nodeType === 9 && !!o.__isXMLDocument; });
```

It **cannot** key off `contentType`, because `DOMParser-parseFromString-xml` asserts
`assert_false(doc instanceof XMLDocument)` for all four XML types. Only
`DOMImplementation.createDocument()` produces one. Branding every XML parse as `XMLDocument` looks
like the more complete fix and is a wrong one — `G_XML_IS_PARSED_AS_XML`'s `notXmlDoc` claim exists
to catch exactly that, and was RED-proven against it.

**A missing global is not a failed assertion.** `doc instanceof XMLDocument` with no such global
throws `XMLDocument is not defined` and takes **the rest of the file** with it — which is why one
absent constructor accounted for 113 `dom` subtests.

### Two adjacent bugs this uncovered, both invisible until XML existed

1. **`documentElement` was `find_first_in(root, "html")`** — a *nominal* lookup where the spec says
   *positional* (the document's first element child). Identical for every HTML document, which is
   why it survived; `null` for an XML document rooted at `<rss>`, `<svg>` or `<soap:Envelope>`, so
   every walk died at the first property access.
2. **`createDocument(ns, qualifiedName)` discarded both arguments**, calling `__createHTMLDocument()`
   and returning an `<html><head><body>` skeleton. It now builds the specified tree: the named
   document element, no doctype, no html/head/body — and its content type is **derived from the
   namespace** (HTML → `application/xhtml+xml`, SVG → `image/svg+xml`, else `application/xml`). A
   flat `application/xml` reads as reasonable and fails `Document-contentType` by name.

### The measured well-formedness boundary, written down rather than implied

xml5ever reports mismatched end tags, EOF inside a tag, a stray end tag, a bad character reference,
two document elements, an empty document and duplicate attributes. It does **not** report two cases
strict XML rejects:

| input | strict XML | here |
|---|---|---|
| `<foo>` (unclosed at EOF) | fatal | **accepted** |
| `<f a=1/>` (unquoted attr value) | fatal | **accepted** |

The unclosed case is **not reachable from the sink**: xml5ever's `end()` drains its open-element
stack and pops each entry *before* `TreeSink::finish` runs, and `open_elems` is private — by the
time we can look, a well-formed and an unclosed parse are identical. So
`parseFromString('<foo>', 'text/xml')` yields the parsed tree where Chrome yields a `parsererror`.
`manuk_html`'s `known_wellformedness_gap_is_pinned` **asserts the gap**, so the day a future
xml5ever closes it the test fails and the loop is told, instead of the limitation quietly outliving
its own documentation.

### Result

**`domparsing` 190 → 219 subtests** (non-tentative 35.8% → 42.1%), `NO_REPORT 1 → 0`;
**`dom` 6370 → 6380**. HANG/CRASH 0 in both.

> **The +10 in `dom` is the honest headline, and it is much smaller than the 113 that "XMLDocument
> is not defined" suggested.** Defining a missing global stops a file dying at its first reference;
> it does not make the assertions that follow *pass*. A histogram of error messages ranks where the
> engine is SILENT, not how much is winnable — the two are different numbers and this tick is the
> gap between them. The remaining `Document-contentType` mass (a document navigated to a PNG, CSS
> or TXT URL must report *that* type) is now cheap for the first time, because the storage exists
> and only the load path has to write it.

## `sheet.cssRules` was a fresh array on every read — 201 "invalid selectors" that were nothing of the kind (tick 1191)

`css/selectors/parsing` scored **8/392 = 2.0%**, and the failure histogram named the culprits:

```
201  assert_equals: Sheet should have 1 rule expected 1 but got 0
```

with test names reading `"[att]" should be a valid selector`, `".pastoral" should be a valid
selector`, `"body > p" should be a valid selector`, `"h1 em" should be a valid selector`.

> **Those are the most basic selectors in CSS, and that is the tell.** An engine that could not
> parse `.pastoral` would not render a single page on the web. When a histogram indicts a mechanism
> that demonstrably works, the mechanism is not the defect — the HARNESS PATH is. Read the helper
> before believing the ranking.

The helper is WPT's own `css/support/parsing-testcommon.js`:

```js
const style = document.createElement("style");
document.head.append(style);
const {sheet} = style;
const {cssRules} = sheet;          // ← BOUND ONCE, here
sheet.insertRule(selector + "{}");
assert_equals(cssRules.length, 1); // ← read from the bound reference
```

`insertRule` worked. The selector engine was innocent. **`cssRules` returned a NEW ARRAY on every
read**, so the reference bound on line 4 was a snapshot frozen before the insert, and reported `0`
forever.

### The design was right; the identity was missing

The sheet is a **live projection over the `<style>` element's `textContent`** — the element's text
is the single source of truth, and `insertRule`/`deleteRule` are implemented by rewriting it. That
is a good design and it is *why* `cssRules` was rebuilt per read: rebuilding is what makes it live.
It just minted a fresh object each time, so `sheet.cssRules !== sheet.cssRules`, which the spec
forbids — a `CSSRuleList` has identity.

**The principle was already written down, one level too shallow.** `el.sheet` is cached per element,
with the comment *"ONE object per element: `el.sheet === el.sheet` is an assumption every CSSOM
consumer makes, and a library that stashes bookkeeping on the sheet loses it otherwise."* Exactly
that argument applies to the rule list, and had not been carried down to it.

### Live AND stable — either alone is a wrong fix

| fix | identity | liveness |
|---|---|---|
| rebuild per read (before) | ❌ | ✅ |
| cache and never refresh | ✅ | ❌ **dead list** |
| **cache + refresh in place** | ✅ | ✅ |

Both wrong answers were RED-probed against the gate, and the dead-list probe is the one worth
keeping: it passes every identity claim while making `freshAfterInsert` read `0`.

### A getter cannot refresh a reference nobody reads through

The first version refreshed inside the `cssRules` getter and **still failed the WPT idiom** —
`cssRules.length` on a bound array runs no accessor of ours, so the update was never asked for. The
refresh therefore has to be **pushed at mutation time**: `insertRule` and `deleteRule` call the same
`__syncRules` after rewriting the element's text.

⚠ **NAMED LIMIT, measured not assumed.** A raw `style.textContent = …` write also runs no accessor,
so a bound list observes that change at the next read *of the sheet* rather than at the instant of
the write. The two mutation paths that are ours push immediately, which is what the
captured-reference idiom needs. `G_CSSRULELIST_IS_LIVE_AND_STABLE` asserts both halves separately
(`freshAfterText` and `capturedAfterText`) so the boundary is pinned rather than implied.

### Result

**`css/selectors` 3119 → 3250 (+131)**, `css/selectors/parsing` **2.0% → 33.2%**, HANG/CRASH 0;
`css/css-values` and `dom` re-measured unchanged. PRIMARY WPT 69.78% → **69.89%**.

> Second time in two ticks that the area ranker pointed at the wrong organ: t1190's `domparsing`
> was 65% unshipped-spec `tentative/`, and this one's `css/selectors` was a CSSOM identity bug.
> **Rank by area to find the mass; read the failing test's HELPER to find the organ.**

## A computed style that THREW on a non-string argument — and what the 40-message count was really worth (tick 1192)

`getComputedStyle(el).getPropertyValue(0)` threw `TypeError: p.charCodeAt is not a function`. The
method opened by testing for a custom property:

```js
getPropertyValue: function (p) {
  if (p.charCodeAt(0) === 45 && p.charCodeAt(1) === 45) { … }   // "--" prefix?
```

`charCodeAt` exists on strings. Per CSSOM the parameter is a `CSSOMString`, and **WebIDL converts
whatever it is handed before the method body runs** — a number, `null` or an object is a
well-defined call returning `""`, not an exception.

This is the **throw class**, and it is why it matters more than the property being asked for: a
TypeError in a property read takes the rest of the script with it. Iterating a property list is
ordinary code — `props.forEach(p => cs.getPropertyValue(p))` over an array holding an index, a
`null` hole, or a `String` wrapper is enough to hit it.

**The LIVE `el.style` path was measured and was already correct** — `getPropertyValue`,
`removeProperty` and `getPropertyPriority` all coerce there. The defect was specific to the
computed-style object, which is a second implementation of the same interface. The gate asserts the
live path too, so a later unification cannot regress the half that was already right.

### `String(p)`, not `typeof p === 'string'`

The RED probe makes the distinction concrete. With the coercion removed, `num`, `null`, `obj` and
`bool` all throw — but **`new String('color')` still passes**, because a String object really does
have `charCodeAt`. A `typeof` guard would send that wrapper down the "not a string" path and return
`""`, satisfying every no-throw claim while silently answering the wrong thing. `wrapper` is a claim
of its own for exactly that reason.

### The honest size, and the lesson repeated

**`css/css-values` 1697 → 1705 (+8)** — not the +40 the error-message count suggested. The 40
messages sat inside files whose *other* assertions still fail for unrelated reasons: the rejected
values there are `calc-size(auto, size)`, `random-item(--x, serif, sans-serif)` and friends —
**unshipped CSS Values 5**, not properties we deny.

> **Message count is not flip count.** t1190 recorded this gap (113 `XMLDocument is not defined`
> messages bought +12) and here it is again at a different scale. A histogram of failure messages
> ranks where the engine is SILENT or THROWS; it does not say how many assertions become true when
> the noise stops. Both numbers are worth having — but only one of them is progress, and the write-up
> has to say which is which.

Also worth recording from the same sweep: **`css/css-values`' 40.4% is substantially unshipped
spec**, the same shape as t1190's `domparsing` (65% `tentative/`). Its `assert_not_equals: property
should be set` mass is `width: calc-size(…)` ×69, `font-family: random-item(…)` ×32,
`background-image` ×33. Ranking that area by its failure count over-promises what is winnable.

## A frame's window and document were rebuilt on every read — the same identity bug, fourth site (tick 1193)

`f.contentWindow === f.contentWindow` was **false**. So was `f.contentDocument ===
f.contentDocument`. Both minted a fresh object per access.

| what breaks | why |
|---|---|
| state stashed on the frame's window | written to an object discarded on the next line — the ready flag, message-port handle and resize callback every embed and OAuth frame keeps |
| `e.target.ownerDocument === frame.contentDocument` | the standard "is this event from my frame?" test, **never true** |
| a `WeakMap`/`Set` keyed on the frame's document | one entry per read, forever |
| `iframeDoc.defaultView` | flat `null` for any document that was not the singleton |

**This is the fourth site of one bug in a single window of ticks** — `sheet.cssRules` (t1191) was the
same defect one subsystem over, and `el.sheet` had the rule written down correctly the whole time:
*"ONE object per element … a library that stashes bookkeeping on the sheet loses it otherwise."*
The rule existed; it had not been carried to its neighbours.

**Live AND stable, again.** The cached window exposes `document` as a **getter**, not the value
captured when it was built — a frame that navigates gets a new document, and caching the value would
have bought identity by making the window permanently stale. Exactly the pair `cssRules` needed.

### `ownerDocument` asked the wrong question — and it is t643 one boundary further out

```rust
if (*dom).is_document(cur) && cur != (*dom).root() {   // ← "not the main document"
```

*"Not the main document"* is a question about the **ARENA**, not about the root node. Inside a
frame's arena the owning document **is** that arena's root, so the test was false exactly where the
answer mattered, and **every node in an `<iframe>` reported the PARENT's `document`**. That is the
DOMPurify failure of t643 repeated across the frame boundary: a walk keyed on
`root.ownerDocument || root` runs against the wrong tree and finds nothing. The guard is now
`foreign_arena || cur != root()`.

### What was deliberately NOT added, and why absence beats a stub

**`contentWindow.getComputedStyle` is still missing on purpose.** Measured this tick: computed style
does not work on a framed element *at all*. `STYLES_PTR` is a single thread-local holding ONE page's
style map, and `window_get_computed_style` keeps only the `NodeId` from `node_and_dom`, **discarding
the arena** — so a child node is looked up in the parent's map. On a frame whose own stylesheet sets
`visibility:hidden`:

```text
  gcsHidden = visible     ← the child's stylesheet never reached it
  gcsPlain  = undefined   ← a plain child element has no entry at all
```

Adding the method would convert *absent* into *silently wrong* — false-presence, which the
reliability doctrine ranks strictly worse. It stays off until the style lookup is arena-aware.
**That is the next tick, and it is worth ~480 subtests** in
`css/selectors/attribute-selectors/attribute-case`, whose helper iterates `[window, quirks, xml]` —
two of them frame windows — and dies on `global.getComputedStyle is not a function`.

### Result, and what the number cannot see

**`html/dom` 56443 → 56445 (+2)**, `dom` unchanged, HANG/CRASH 0.

> **+2 is the instrument's answer, not the fix's value, and the distinction is constitutional
> (VI.3: where usage-weight and measured-breadth disagree, usage-weight wins).** WPT's iframe tests
> need the frame-loading harness, so almost none of them exercise this path here. What actually
> changed is that a node in an `<iframe>` stopped reporting the parent's document — a wrong answer
> of the right type, delivered with total confidence, on the #1 platform-web capability. The honest
> report is *"the instrument cannot price this"*, not *"this bought +2"*.

## `querySelectorAll('.a :is(.b, .c)')` returned an EMPTY LIST — and `:is()` was not the bug (tick 1194)

Two defects, and the interesting one is not the missing feature.

**The visible half:** `manuk_css`'s selector matcher — the one behind `querySelector`,
`querySelectorAll`, `matches` and `closest` — had a `Pseudo` enum with `Not` and `Has` and **no
`Is`/`Where`**. Both fell through the parser's `_ => return None` arm, which drops the **whole
selector**, not the unknown part. `:is()`/`:where()` are Baseline and are how every modern
stylesheet writes a grouped rule (`.card :is(h1, h2, h3)`), so the silence was broad — and silent:
no error, no warning, an empty NodeList.

**The root cause, which has nothing to do with `:is()`:**

```rust
fn parse_selector_list(text: &str) -> Vec<Selector> {
    text.split(',')          // ← blind to parentheses
```

A comma inside a functional pseudo is an **argument** separator, not a **list** separator. So:

```text
  .a :is(.b, .c)        →  ".a :is(.b"   +  ".c)"
  p:has(> img, > svg)   →  "p:has(> img" +  "> svg)"
  :not(.a, .b)          →  ":not(.a"     +  ".b)"
```

The first fragment carries an unbalanced `(` and parses as though the list held only its first
member; the second is garbage and is dropped.

> **It did not fail loudly — it quietly matched a SUBSET.** That is the worst of the three possible
> outcomes and is exactly why it survived: `:is(.b, .c)` returned the `.b` elements and looked like
> it worked. A paren-aware `split_top_level_commas` was already in the same file, already used by
> the `:has()` arm, three thousand lines away.

### `:not()` had to become a list too — and it fails CLOSED where `:is()` fails open

`Not(Box<Compound>)` could not represent `:not(.a, .b)` (Baseline) or `:not(.a .b)` (complex
member). It is now `Not(Vec<Selector>)`, matching when **none** match.

The forgiveness rules are deliberately **opposite**, and the RED probe shows why:

| pseudo | invalid member | why |
|---|---|---|
| `:is()`, `:has()` | **dropped**, rest still apply | matching fewer things is a safe degradation |
| `:not()` | **whole pseudo invalid** | dropping a member **INVERTS** — it matches strictly MORE |

With the naive split restored, `.a span:not(.b, .c, .e)` returned **3 elements instead of 1**: it had
silently become `:not(.b)`. A dropped `:is()` member reads as "unsupported"; a dropped `:not()`
member reads as a correct answer to a different question.

### `:where()` shares `:is()`'s variant, and the boundary is stated

They differ only in **specificity** — `:where()` contributes zero — and this matcher answers *"does
it match"* for `querySelector`/`matches`/`closest`, where specificity is never consulted. The live
cascade is Stylo's and computes specificity itself. Folding them anywhere specificity IS read would
be wrong, which is why the gate carries a claim naming that boundary rather than leaving it implied.

### Result

**`css/selectors` 3250 → 3547 (+297)** — `css/selectors/query` **0/12 → 12/12 (100%)**,
`invalidation` 2031 → 2274 (+243), the area root +42. `dom` and `css/css-values` re-measured
**unchanged** as controls. HANG/CRASH 0. PRIMARY WPT **69.90% → 70.14%**, the first reading above 70.

> **The biggest tick of the session came from a `split(',')`.** Third time in five ticks that the
> area ranker named the wrong organ: `domparsing` was unshipped spec, `css/selectors/parsing` was a
> CSSOM identity bug, and here `css/selectors` was a **string-splitting** bug in the shared list
> parser. Rank by area to find the mass; read the failing test's helper — and then read what the
> code it accuses actually *does* — to find the organ.

## A frame's window had TWO properties, and one of them was `location` (t1201)

Measured inside the WPT harness, on a real loaded frame:

```text
  HAVE    [document, location]
  MISSING [DOMException, Node, Element, HTMLElement, Event, Document,
           NodeFilter, Range, getComputedStyle, window, self, Object, Array, Function]
```

`contentWindow` was a hand-rolled object literal carrying `document`, `frameElement`, `location`
and three no-ops. **Every platform interface object vanished at the frame boundary** — and a script
inside a frame, or reaching into one, addresses the platform through `d.defaultView`.

The cost concentrates in one line of WPT's own harness:

```js
  assert_throws_dom("SyntaxError", root.ownerDocument.defaultView.DOMException, () => …)
```

`defaultView.DOMException` was `undefined`, so `assert_throws_dom` died reading `.name` off it.
**204 `dom` and 76 `css/selectors` subtests failed before any behaviour was tested** — the tests were
not failing, they could not be *stated*. Both numbers were predicted from the histogram before the
fix and both came back exact: `dom` 6943 → **7147 (+204)**, `css/selectors` 3681 → **3757 (+76)**,
`html/dom` and `encoding` unchanged, 0 crashes.

### Inheriting the parent's globals is the TRUTH here, not a pretence

This engine gives a frame its own **document** (its own arena, `contentDocument` identity since
t1193) but **not its own JS realm** — a limit `iframe_js` already states. One realm means
`frameWin.Node` and `Node` genuinely *are* the same object, and `e instanceof frameWin.DOMException`
genuinely *is* the right answer for an exception this realm threw. So the frame window inherits the
parent global rather than being empty, and the gate asserts the **identity** (`sameConstructor`),
not merely the presence — if a per-frame realm ever lands, that is the claim that must change.

### A Proxy, not a prototype chain — and the RED probe shows why

Two things a prototype cannot do:

1. **`getComputedStyle` must stay ABSENT, not shadowed by `undefined`.** Its absence is reasoned:
   `STYLES_PTR` is a single thread-local holding ONE page's style map, so a frame node looked up
   there returns the **parent's** style. Exposing it converts a documented absence into a silently
   wrong answer, and a property that exists and answers `undefined` is a feature-detection trap.
2. **A write must land on the frame's OWN object**, and `Object.keys(frameWin)` must not enumerate
   the parent global. Every embed stashes a ready flag or a message-port handle on its window.

```text
  RED 1  the object literal (the state before)   FAILED `missing[]`
  RED 2  Object.setPrototypeOf(own, globalThis)  PASSES `missing[]`, FAILS `gcsAbsent`
```

RED 2 is the instructive one: the plausible one-line fix carries the platform across correctly and
silently re-exposes the wrong-answer surface the module docs spent a paragraph excluding.

⚠ **STILL OWED:** the 484 `css/selectors` subtests that die on `global.getComputedStyle is not a
function` in a frame are **not** closed by this — they need the style lookup to become arena-aware,
which is the deeper fix `iframe_js`'s module docs name. That is the next lever in this vein.

## The style lookup had ONE map, and the +0 named the next link (t1202)

`STYLES_PTR` is a single thread-local holding ONE page's computed-style map, and
`window_get_computed_style` discarded the arena on its first line:

```rust
  let node = arg_object(vp, argc, 0).and_then(|o| node_and_dom(o).map(|(_, n)| n));
                                                                    ^^^^^^ the arena, discarded
```

A frame element's `NodeId` was therefore looked up in the **parent's** map and answered about a
different element in a different document — the same one-arena assumption `node_and_dom` closed for
the DOM, one pass later in the pipeline. It is a *wrong answer of the right type*, which is why t1201
deliberately **withheld** `getComputedStyle` from a frame's window rather than ship it.

Each child `Page` already owns a full style map; nothing published it. `Page::publish_iframe_docs` —
the single site every `child_pages` mutation goes through, and the one that already publishes each
child arena's address — now publishes the style maps beside them. `iframe_js`'s deny list is empty:
the withholding is **retired**, not relaxed.

### ⚠⚠⚠ It landed, it is RED-proven, and it moved ZERO subtests

```text
  css/selectors 3757 → 3757 · dom 7147 → 7147 · html/dom 56445 → 56445 · css/css-color 6260 → 6260
```

The prediction was 484. The histogram says exactly which link cleared and which one now holds:

```text
  BEFORE   484 ×  global.getComputedStyle is not a function
  AFTER      0 ×  global.getComputedStyle is not a function      ← the fix DID land
           308 ×  expected (string) "hidden" but got (undefined) undefined
```

**A frame's style map is a LOAD-TIME SNAPSHOT.** The test does
`elm = global.document.createElement('div'); global.document.body.appendChild(elm)`, and a node the
script creates afterwards has no entry. The main document does not have this problem —
`force_reflow_if_stale` re-cascades it on every style read — but a frame gets no such pass, and
giving it one is not a line: `forced_reflow` re-cascades whatever arena it is handed while writing
the result into the MAIN page's `ReflowCtx` and resolving sheets against the PARENT's URL and
external CSS. **A frame needs its own reflow context.** That is the next lever, now a named subsystem
change rather than an estimate.

### Banked, where t1197 was reverted — and the distinction is the point

| t1197 (reverted) | t1202 (banked) |
|---|---|
| the mechanism **never ran** — a registered callback nothing requested | the mechanism **runs and is observed**: `hidden`, from the child's own sheet |
| banking it would be **false presence** — `grep` says yes, nothing works | the capability is real; a *different, named* gap sits downstream |
| nothing measurable changed anywhere | the blocking error class went **484 → 0** |

A chain has links, and clearing one that is genuinely cleared is progress even when the next link
holds the count still. Banking a mechanism that was never reachable is not.

**The gate's fixture is built so only the right implementation can pass:** the CHILD's stylesheet
hides `#inner` and the parent says nothing about that node, so `gcsChildAnswer:hidden` is unreachable
for a lookup that silently reads the parent's map — and `gcsNotParent` asserts the two documents
disagree, so a coincidence cannot pass either.

## `createEvent` accepted every name, so it could not be feature-detected (t1206)

`document.createEvent(iface)` was `g[String(iface)] || g.Event`. Both halves are wrong.

**1. The fallback accepted everything.** `createEvent('NotAnEvent')` returned a plain `Event` where
DOM §createEvent says throw `NotSupportedError` — and the throw is **how a page feature-detects an
event interface**. An engine that never throws answers *"supported"* for every name, so the library
takes the modern branch and gets an `Event` with the wrong prototype: a value of the right type that
fails at the first `instanceof`. Third instance of this shape in one session, after the selector
feature-detect (t1200) and jQuery's `support.cors`.

**2. The lookup was case-sensitive, and five table entries have no global at all.** DOM matches
**ASCII-case-insensitively** against a **fixed table**, and `Events`/`HTMLEvents`/`SVGEvents` →
`Event`, `MouseEvents` → `MouseEvent`, `UIEvents` → `UIEvent` are aliases with no interface of their
own:

```text
   createEvent('MouseEvents')  →  Event      (should be MouseEvent)
   createEvent('mouseevent')   →  Event      (should be MouseEvent)
   createEvent('UIEvents')     →  Event      (should be UIEvent)
```

`MouseEvents` and `HTMLEvents` are the spellings **jQuery's `trigger` and Google Analytics emit**, so
a synthesised click arrived as a bare `Event` — no `clientX`, no `button`, `instanceof MouseEvent`
false. The old code was right about `Events` **only by accident**, because its fallback was `Event`.

**Measured:** `dom` **7147 → 7306 (+159)**, `html/dom` and `css/selectors` unchanged, 0 crashes.

⚠ `G_CREATE_EVENT_ALIASES`'s last claim is `dispatches:yes` — an event built this way must still
reach a listener, because **a gate that only proved the throws would pass with `createEvent` deleted
entirely.**

⚠ **The honest edge:** `TextEvent`, `DeviceMotionEvent` and `DeviceOrientationEvent` are in the
spec's table and not in this engine, and `TouchEvent` is added to the table **only when the engine
exposes it** — the spec's own *"if the UA supports legacy touch events"* clause. All of them throw
`NotSupportedError`, which is the truthful answer; claiming them would be false presence.

## One rule, and it was written out in ONE of its two callers (t1207)

DOM specifies `createElementNS(ns, qname)` and `DOMImplementation.createDocument(ns, qname)` against
**the same algorithm** — *validate and extract*. The engine had it written out inside
`createElementNS` and **not at all** inside `createDocument`:

```text
   createDocument('http://example.com/', 'xmlns')   → a Document   (spec: NamespaceError)
   createDocument(null, 'p:q')                      → a Document   (spec: NamespaceError)
   createDocument('http://example.com/', 'a:b:c')   → a Document   (spec: InvalidCharacterError)
```

**The fix is an EXTRACTION, not a second copy.** *One rule, two implementations* is a shape this
project keeps paying for (t720-724; `event_loop`'s two drain loops, one bounded and one not).
Copying the validation across would pass a gate on the day and let the two diverge at the next spec
change. `validate_and_extract()` is the single implementation, and `G_CREATE_DOCUMENT_VALIDATION`
asserts **both** callers — so a later tick that gives one its own copy fails on the arm it forgets.

⚠ **The one difference is SPECIFIED, so it is a parameter.** `createDocument(null, "")` is valid — a
document with no document element — while `createElementNS(null, "")` is an `InvalidCharacterError`.
An explicit `allow_empty` flag, with both halves pinned. RED-proven twice, the second being the
plausible half-fix: *share the rule without the parameter* → `emptyNameIsADocument` fails.

**Measured:** `dom` **7306 → 7397 (+91)**, **`domparsing` 219 → 234 (+15)**, `html/dom` unchanged,
0 crashes.

⚠ **`domparsing` moving is the tell that the extraction was the right shape rather than the tidy
one:** a third caller nobody was thinking about — `DOMParser` — was already reaching the same rule
and got the fix for free. A copy would have left it where it was.

## The engine was told the encoding and threw it away (t1211)

`document.characterSet` / `.charset` / `.inputEncoding` returned the constant `"UTF-8"`, with the
comment *"we decode to UTF-8, so that is the answer"*. **That answers a different question.** The DOM
asks what the document's encoding **was**; `<meta charset=iso-8859-5>` must report `ISO-8859-5`
however the engine stores it internally.

The answer was already computed — `manuk_net::charset::sniff` picks the encoding on every load and
**every caller discarded it**. The same shape as `contentType` before t1075 and `compatMode` before
t241: a getter returning a constant beside a field that already knew, all three within a hundred
lines of each other.

### ⚠⚠⚠ The ordering was the fix, and the first version measured +0 without it

Setting the value at the call site — after `render_iframe_with_type` returned — moved nothing.
Applying the rule this session earned (*a zero from a reachable, observed mechanism is a question
about the diagnosis*), the probe said:

```text
   html=<html><head><meta charset="iso-8859-5"></head>…   ← the document was served correctly
   cs=UTF-8                                                ← and the getter did not see it
```

**`fire_frame_load` runs INSIDE `render_iframe_with_type`**, and every test — and every embed — reads
a child document inside its `load` handler. **A value written after the call is written after the
only moment anyone looks.**

```text
   value set AT THE CALL SITE        dom 7517 → 7517    (+0)
   value set BEFORE the load event   dom 7517 → 8142  (+625)
```

**The capability, the plumbing and the instrument were all correct in the first version, and it
bought nothing.** The difference is four lines of ordering.

### The instrument was blocking it too — `encoding.py`, the fifth mis-provisioned reference

636 subtests drive `iframe.src = "encoding.py?label=" + label`, a five-line wptserve handler
returning `<!doctype html><meta charset="LABEL">`. A static file server answers 404, so every one of
those iframes loaded nothing. One handler was added to the harness — ⚠ **deliberately one, not a
wptserve implementation**, with the boundary written into the code so the next one is a decision
rather than a slide.

`G_DOCUMENT_CHARACTER_SET` asserts all three aliases agree, that the name is the Encoding Standard's
canonical spelling (`encoding_rs::Encoding::name()`, so no table of ours can drift from the
decoder's), that an untold frame still reports `UTF-8`, and that **the child's encoding does not leak
onto the parent** — a single global would have passed the headline claim and broken the page around it.

## I swept the class instead of waiting for the next instance (t1212)

t1211 made `characterSet` the **third** getter in `dom_bindings.rs` returning a constant beside a
value the engine already had — after `contentType` (t1075) and `compatMode` (t241), all three within
a hundred lines. Three instances found three separate times, each by tripping over it, is the shape
PART VI already names: **every part of the platform that can be ENUMERATED should be enumerated
once.**

So: every native getter in that file whose whole body is a literal with **no lookup** in it
(`this_node`, `with_style`, `layout_rect`, `CURRENT_DOM` all absent).

```text
   native getters whose whole body is a CONSTANT:  1
     doc_get_referrer            return_string(cx, vp, "")
```

**One — and that is the more useful half of the result.** The class is now *swept* rather than
sampled; the next reader does not have to wonder whether there are twenty more.

### Why `document.referrer` survived three audits

`""` is the **correct** answer for a document nobody referred to — a typed URL, a bookmark — so the
constant is right about the top-level document most of the time. It is wrong about **a framed
document, whose referrer is its embedder**, which is the value analytics, attribution and paywall
scripts read first thing inside an embed. A constant `""` tells all of them the user arrived from
nowhere.

Set before `fire_frame_load`, applying t1211's ordering rule rather than rediscovering it. The gate
asserts **both** directions — the framed document reports its embedder, the top-level one still
reports `""` — because the default was never wrong, it was being used as the whole answer.

**+0 WPT**, stated plainly: the areas this loop measures do not exercise a framed referrer. A
real-web capability with a gate and no scoreboard movement.

⚠ **Instrument note:** the first reading was `html/dom` **56444**, a −1. Solo re-run: **56445**. The
−1 run showed `TH_TIMEOUT 56` against the solo run's `9` — a `cargo test` was compiling alongside it.
**A concurrent build makes an async test time out and reads as a regression.** Re-run solo before
believing a −1.
