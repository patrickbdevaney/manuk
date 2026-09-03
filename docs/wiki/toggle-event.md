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
