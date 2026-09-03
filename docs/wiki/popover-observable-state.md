# The popover's observable state — a capability that worked and could not be asked about

*Landed tick 1395. Gate: `engine/page/tests/g_toggle_event_and_popover_open.rs`.*

`html/semantics/popovers` sat at **9 of 153 (5.9%)** — an Interop 2026 focus area, and the backbone of
every modern menu, tooltip and dropdown on the web. The survey found the subsystem was not missing. It
was **half-installed**: it worked, and three of the four ways to observe it did not.

| | manuk (before) | Chrome |
|---|---|---|
| `showPopover()` | function | function |
| `display` when open | `block` | `block` |
| `beforetoggle` / `toggle` | fires, right states | fires |
| **`:popover-open`** | **never matches** | `true` |
| **`ToggleEvent`** | **absent** | present |
| `ev.constructor.name` | `""` | `"ToggleEvent"` |
| `popoverTargetElement` | `undefined` | element |

## 1. A private marker is not a public name

`showPopover()` writes an internal attribute, `data-manuk-popover-open`, and the UA stylesheet keys
`display` off **that attribute**. So the popover opened and painted perfectly — while the selector for
its state had never been wired to anything.

The state was real. It was load-bearing for paint. And it was unaskable: every
`el.matches(':popover-open')` and every `#menu:popover-open { … }` rule on the web read `false`.

> ⭐⭐ **A capability implemented through a private marker is finished when the PUBLIC name for that
> marker resolves — not when the marker works.** The internal attribute made the feature look complete
> to every instrument that renders the page, and to none that queries it.

## 2. One rule, two matchers

`:popover-open` had to be taught to **both** selector engines:

* the minimal one, behind `element.matches()` and `querySelectorAll`;
* the Stylo one, behind the live cascade (`NonTSPseudoClass::PopoverOpen`).

The gate asks through both doors on the same element in the same state, and the two mutations that
remove one arm each go red on **different rows**. A rule that reaches a stylesheet but not a script —
or the reverse — is the twin-drift this codebase keeps finding.

## 3. `ToggleEvent`, and three general rules hiding inside one interface

`ToggleEvent is not defined` was 38 subtests in a single file. But three of the four things that test
file wanted turned out to be rules the engine was breaking for **every** event. Chrome-measured across
four constructors:

```text
  new Event() · new CustomEvent() · new MouseEvent() · new ToggleEvent()   ALL TypeError
  e.type = 'y' · e.bubbles = false · e.detail = 9                          ALL IGNORED (readonly)
  Ctor.name · Object.prototype.toString.call(e)   "ToggleEvent" · "[object ToggleEvent]"
```

### `arguments.length`, not `type === undefined`

`new ToggleEvent(undefined)` is a **legal** call whose type is the string `"undefined"`, and
`new ToggleEvent()` is a `TypeError`. WPT asserts both in the same file. The two are indistinguishable
by value and only distinguishable by **arity**.

### The readonly fix is scoped, and the divergence is stated

WebIDL declares every event attribute readonly. But `__dispatchEvent` writes `type`, `target`,
`currentTarget`, `eventPhase`, `bubbles` and `isTrusted` on the event as it propagates: a real engine
keeps those in internal slots behind prototype getters, ours are own data properties, and freezing
them would freeze the dispatcher out.

So the **per-interface extras** are locked (`oldState`, `newState`, `source`, `detail`) and the base
fields are not. ⚠ That is a known, deliberate divergence — written where the code is, so it is a
documented limit rather than a bug somebody finds later.

## 4. `toggle` is coalesced — and `select`, one tick earlier, is not

Chrome-measured: show → hide → show fires three synchronous `beforetoggle`s and exactly **one**
`toggle`, carrying the net `closed > open`.

⭐⭐ That is the **exact opposite** of the [`select` event](text-field-selection.md#8-the-select-event--queued-uncoalesced-and-owned-by-the-api),
landed one tick earlier, which is queued and explicitly **not** coalesced — two different changes in
one task fire two events.

Two notifications, in two adjacent subsystems, both asynchronous, with opposite batching rules. Which
is exactly why neither can be inferred from the other, and why the `select` event's batching had to be
settled by measurement rather than by reading the suite. *Measured here, deliberately not built.*

## 5. What this cost, and what the constitution check said about it

```text
  html/semantics/popovers   9/153 = 5.9%  ->  62/153 = 40.5%
```

⚠⚠ And the same suite that scored the win named what the tick did **not** do:

```text
  popover-minimum-role.html   assert_equals: role starts as none, expected "none" but got "generic"
```

HTML-AAM maps a **visible** `[popover]` with no implicit role to **`group`**, and an invisible one to
`none`. We say `generic` for both. So after this tick a popover opens, paints, matches
`:popover-open`, and announces itself with a real `ToggleEvent` — **and the agent's perception layer
still cannot tell it from an ordinary `<div>`.**

> ⭐⭐⭐ Constitution check #132 made this the window's finding: five consecutive capability ticks
> landed the DOM and CSS halves of their subsystems and none touched the semantic half. **The bend is
> invisible per-tick and obvious per-window** — each tick was individually defensible, and across eight
> the shape is that the loop optimised the channel WPT scores. The a11y tree has no suite.
> **An invariant with no instrument loses to one with a scoreboard.**

## 6. The semantic half — `group`, and the pair that names the rule

*Landed tick 1396, chosen by constitution check #132 rather than by the histogram.*

HTML-AAM raises a **visible** `[popover]` to `group` — but only when the element has **no role mapping
of its own**. Every row measured through CDP `Accessibility.getPartialAXTree`:

| element (forced visible) | Chrome |
|---|---|
| `<div popover>` **closed** | `none` (node ignored) |
| `<div popover>` | **`group`** |
| `<span popover>` | **`group`** |
| `<section popover>` unnamed | `generic` |
| `<section popover>` named | `region` |
| `<button popover>` | `button` |
| `<nav popover>` | `navigation` |
| `role="none"` | `none` (ignored) |
| `role="alert"` | `alert` |
| `+ visibility: hidden` | `none` (ignored) |

⭐⭐ **`<div>` and an unnamed `<section>` both compute to `generic` without the attribute, and the
popover raises only one of them.** A rule written against the *computed* role would have raised both.
The discriminator is *does HTML-AAM map this tag at all* — `<section>` has a mapping
(region-when-named, generic otherwise), `<div>` and `<span>` have none.

⭐⭐⭐ **So the rule belongs in the role function's DEFAULT ARM, which IS the set of unmapped tags
rather than a list of them.** Anywhere else needs a hand-maintained tag list that drifts the first
time a tag gains a mapping. The arm cannot drift: it is defined as *everything with no arm*.

### ⚠⚠⚠ And the suite still reads `generic`, because a second entrance disagrees with the tree

The a11y **tree** is already right: a closed popover is not in it, and an open one is, as a `group`.
But `host_ax_role_name` — the seam behind `test_driver.get_computed_role`, which is how the entire
`accname` / `wai-aria` / `html-aam` surface (457 tests) asks — **calls the role function directly and
bypasses the tree.** So it reports a role for a node the tree excludes.

That function's own doc comment already records this exact class happening once before (generated
`::before` content reached the tree and not this entrance, so a button was announced `"Save"` here and
`"★ Save"` there). **One rule, two entrances, and the weaker one is what the conformance suite reads.**

It is deliberately not patched at that entrance, because a closed popover is unexposed for the same
reason every `display: none` element is: the value is wrong in **all** such elements, so it is the
shared path, not a special case. Fixing it there would encode the wrong shape.

> ⭐ This is why the popover role fix scored **zero** WPT subtests while being real, gated and
> Chrome-arbitrated — which is constitution check #132's claim demonstrated instead of argued.

## Not built yet

* the `get_computed_role` entrance answering from the TREE, so an excluded node reports `none` for
  every reason a node is excluded (§6) — the shared path for 457 conformance tests;
* `popoverTargetElement` / `popoverTargetAction` reflection and the declarative invoker (27 subtests);
* the queued-and-coalesced `toggle` event described in §4.
