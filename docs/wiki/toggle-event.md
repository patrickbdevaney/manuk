# The `toggle` event — one interface, three elements, opposite batching to its neighbour

*`ToggleEvent` landed for `[popover]` at tick 1395; `<details>` and `<dialog>` at tick 1400.
Gates: `g_toggle_event_and_popover_open.rs`, `g_toggle_event_details_dialog.rs`.*

| | `beforetoggle` | `toggle` |
|---|---|---|
| `[popover]` | yes, synchronous, **cancelable** | yes |
| `<dialog>` | yes, synchronous | yes |
| `<details>` | **no** | yes |

⭐⭐ **`<details>` has no `beforetoggle`, and firing one is not a harmless extra.** A component that
listens for `beforetoggle` in order to veto — which is the `[popover]` idiom, where the event *is*
cancelable — would believe it had a veto on an element whose spec has no cancel point.

## It is queued, and it is COALESCED

```text
  d.open = true;  read the listener log on the very next line   ->  []   (a queued task)
  d.setAttribute('open',''); d.removeAttribute('open')          ->  ONE event, `closed > closed`
```

The coalescing keeps the **first** transition's `oldState` and the **last** one's `newState`.

⭐ **This is the exact opposite of the [`select` event](text-field-selection.md#8-the-select-event--queued-uncoalesced-and-owned-by-the-api)**, which is queued and explicitly *not* coalesced: two
different selection changes in one task fire two events. Two async notifications, in adjacent
subsystems, with opposite batching rules — **neither is inferable from the other**, and both look like
"it fires once" from a badly-shaped probe. Measure the batching rule of every async notification
separately, with two *different* changes in one task.

## Two entrances, and only one of them was hooked

`details.open = true` and `details.setAttribute('open','')` are the same state change. A first
implementation hooked the IDL reflection setter: every hand-written probe passed, and WPT's
`toggleEvent.html` moved by **one** — because all eleven of its cases write the attribute form.

The choke point is the attribute. The boolean IDL setter is literally:

```js
if (v) el.setAttribute(a, ''); else el.removeAttribute(a);
```

so hooking `setAttribute`/`removeAttribute` covers both spellings with one implementation — and the
exclusive-accordion sibling, which removes the attribute directly, gets its `toggle` for free instead
of needing a second hand-written dispatch that could drift. Both duplicates were then **deleted**.

> ⭐⭐⭐ **When N surfaces can cause one state change, hook what they funnel through, not the N of
> them.** The same lesson as hanging the [dynamic-script](dynamic-script-insertion.md) hook off the
> mutation record rather than off nine insertion methods.

## ⚠⚠ `isTrusted` has now been overridden four times

`assert_true: event is trusted expected true got false` was the last thing between this and the file.
The dispatcher infers `isTrusted` from *"was an event object supplied"* — and an object must be
supplied to carry `oldState`/`newState`.

That inference has now been corrected, one line at a time, for:

* the `select` event (t1394),
* the popover `ToggleEvent` (t1395),
* the `<img>` `load` event (t1399),
* and `<details>`/`<dialog>` `toggle` (t1400).

⭐⭐ **A default that is wrong for every engine-synthesised event is a default pointing the wrong
way.** The seam wants an explicit *trusted* argument rather than an inference from the shape of the
call. Recorded rather than changed, because flipping it touches every page-initiated `dispatchEvent`
as well — but four call sites is the evidence, not a hunch.

## Pricing note — what was refused to get here

Tick 1400 began as the image re-fetch (`img.src = img.dataset.src`). Priced on 36 freshly-fetched
corpus pages first:

```text
  1,002 <img> across 18 image-bearing pages
    data-src style           118  (11.8%)
    data-src AND NO src        0  ( 0.0%)   ← would render nothing without a re-fetch
    loading=lazy             528  (52.7%)
  usemap / <map>   0% of pages   ← 162 WPT subtests in ONE file
```

Every `data-src` image also carried a real `src`, so the re-fetch would swap a placeholder for a
sharper file rather than make a missing image appear: **priced at ~0 for coverage, and refused.**
Image maps were the biggest single failing file in the area and are refused on **0% corpus weight**.
The same sweep found `createElement("script")` on **56%** of pages, which is what tick 1397 fixed.

## The THIRD entrance — the one a human uses, and the one the choke point cannot see (t1403)

The two entrances above are both **script**. The way a person opens a disclosure is neither: a click
on the `<summary>` runs the UA's activation behaviour in `Page::dispatch_click`, which flips the
attribute on the **Rust `Dom`** directly and never enters a JS binding — so it never reaches
`dom_bindings::queue_open_toggle` at all. It carried its own hand-written pair of dispatches, and
that copy stayed frozen at the pre-t1400 shape.

Headless Chrome 145.0.7632.116, one fixture, four phases — a `name="faq"` group (`#a` open, `#b`
closed) plus an unnamed `#c`:

```text
  LOAD  {T:a:closed>open:ToggleEvent:true}
  OPEN  {T:b:closed>open:ToggleEvent:true  T:a:open>closed:ToggleEvent:true}
  CLOSE {T:b:open>closed:ToggleEvent:true}
  SOLO  {T:c:closed>open:ToggleEvent:true}
```

| click a `<summary>` in a `name` group | chrome | here, before |
|---|---|---|
| `beforetoggle`, clicked panel and auto-closed peer | **neither** | **both**, spurious |
| the event | trusted `ToggleEvent`, both states | plain `Event`, states `undefined` |
| delivery | queued | synchronous |
| order | the **clicked panel**, then the peer | the peer, then the panel |

Every row is silent. `e.newState` — the idiomatic way to branch on which way a panel went — read
`undefined` on the click path and the correct string on the script path, **in the same page, for the
same element**.

> ⭐⭐⭐ **"Hook what they funnel through" is only as good as the enumeration of what *they* are.**
> t1400 enumerated the entrances by asking *"how does SCRIPT change this?"* and got a complete answer
> to **that** question. The UA's own actuation is not script; it is the one entrance the browser
> itself owns. **When you find a choke point, ask which callers CANNOT reach it** — for a Rust-side
> engine with a JS-side choke point, that set is never empty.

The fix routes the click path to the same `__queueToggleById` the attribute hook uses: every `open`
state change the activation causes is recorded as `(node, is_open_now)` **in the order it happened**
(clicked panel, then any accordion sibling it closed) and queued in one `eval_in_page`, whose
end-of-script drain is where the queued tasks fire. Both hand-written dispatch loops are deleted.

Gate: `g_details_click_toggle_event.rs`, eight arms, every asserted row taken from the measurement
above, red under all five mutations.

### The guard that deleted itself, and the arm that replaced it

`__queueToggleById` resolves its element through `__nodes`, and the per-element `dispatch_event` it
replaces reflected its target for free — so the first version primed the map with one
`document.getElementsByTagName('details')`. **The mutation that deletes that line came back green.**
At load `__nodes` already holds every `<details>` and every `<summary>` in the parsed document,
including two the fixture's script never names; an `innerHTML`-inserted pair is reflected too. The
line was inert. It was deleted, and the property it was defending put under a live arm instead.

⭐ **That arm looked impossible to write.** `toggle` does not bubble, so *"does an untouched
`<details>` still get its event?"* appears to require resolving the element first — which answers the
question by asking it. It does not: **a non-bubbling event still runs the CAPTURE phase down to its
target**, so `document.addEventListener('toggle', fn, true)` hears a panel nobody holds. Chrome on a
two-panel `name` group no script touches: `CAP:q:closed>open CAP:p:open>closed`.

> **When an observation seems to require the very handle whose absence you are testing, look for the
> phase that runs before the handle is needed.**

### The arm that was measured and removed rather than shipped

The same test one construction further out — a `<details name>` group built by `innerHTML` and never
held — passed here and disagreed with Chrome:

```text
  ours    CAP:s:closed>open  CAP:r:open>closed
  chrome  CAP:s:closed>open  CAP:r:closed>closed
```

Chrome queues a `toggle` at **insertion** for a `<details open>` — from the parser at load and from an
`innerHTML` write — and that pending event coalesces with the accordion's later close, keeping the
FIRST `oldState`. The arm was asserting *our* value; a green arm that asserts our own divergence is
how a gate pins the engine to a bug, so it was removed.

### The two gates that were red, and had different owners

* **`g_details_beforetoggle` (t470) asserted a behaviour Chrome does not have**, and it
  **contradicted** `g_toggle_event_details_dialog` (t1400) directly. WPT's
  `the-details-element/toggleEvent.html` contains `beforetoggle` **zero** times across eleven cases.
  Replaced by the click-path gate above rather than merely deleted.
* **`g_details_open_idl` (t468) held a SYNCHRONOUS expectation of a now-queued event** — it read the
  listener log in the same `eval` that set `.open`. In Chrome that read is empty too. The engine got
  more correct; the gate now reads the log in a second round.

⭐ Nothing compares gates to each other, so a contradicting pair coexists silently until the engine
moves and the older one goes red. **Measure a red gate against Chrome before touching the engine.**

**Named non-claim, two witnesses:** we fire no `toggle` when a `<details open>` is INSERTED — by the
parser at load (the `LOAD` row above) or by an `innerHTML` write. Chrome does, and it coalesces.
Measured, out of scope for t1403, written down so the next reader does not have to re-measure it.
