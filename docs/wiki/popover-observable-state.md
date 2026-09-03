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

## Not built yet

* the popover's **a11y half** — the `group` role mapping and the popover's presence in the tree (this
  is the next tick, chosen by the constitution check rather than by the histogram);
* `popoverTargetElement` / `popoverTargetAction` reflection and the declarative invoker (27 subtests);
* the queued-and-coalesced `toggle` event described in §4.
