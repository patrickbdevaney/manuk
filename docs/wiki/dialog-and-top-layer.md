# `<dialog>`, `popover`, and the top layer (ticks 194-195)

The modern web's modal. Since ~2022 the cookie banner, the confirm-delete, the command palette, the
Radix/Headless-UI dialog primitive and the shadcn `<Dialog>` all bottom out in `<dialog>` +
`showModal()`. Interop 2026 lists it as a focus area. Before this tick Manuk had **the attribute and
nothing else**: `dialog` appeared once in `reflect_table.rs` as `{"open": boolean}`.

## What "absent" actually looked like

Two independent failures, and the second is the one a JS-only conformance check never sees.

1. **`dlg.showModal()` was a TypeError.** Not a no-op — an exception, thrown inside the click
   handler, which takes the rest of that handler with it. The button did nothing at all, and the
   console message (if anyone were reading it) named the symptom, not the organ.
2. **A closed dialog rendered.** With no UA `display:none` rule a `<dialog>` is just an unknown
   element, i.e. `display: inline` in the minimal cascade and unstyled in Stylo — so the modal's
   contents were laid out and painted **into the page, in tree order, before anyone opened it**.
   "DELETE EVERYTHING?" as a paragraph in the middle of the article. This is the same shape as the
   `<source>` bug (a metadata element with no `display:none` becomes content) and the `<script>`
   that painted its own source down rust-lang.org.

Fixing only (1) yields a browser where the modal opens *and was already there*.

## The mechanism, in four places

| Concern | Where |
| --- | --- |
| `show` / `showModal` / `close` / `returnValue`, `close` + `cancel` events, `<form method=dialog>`, Escape | `engine/js/src/event_loop.rs` — the dialog block in the JS prelude |
| `HTMLDialogElement` branding | `event_loop.rs`, `iface('HTMLDialogElement', tagIs('DIALOG'))` |
| UA rendering (`dialog` hidden, `dialog[open]` a centered box) | `engine/css/src/stylo_engine.rs` `UA_CSS` **and** `engine/css/src/lib.rs` `apply_ua_defaults` |
| Top layer | `engine/page/src/lib.rs` — `TOP_LAYER_Z` + the modal branch in `z_index_map` |

### Why modality is an attribute (`data-manuk-modal`)

`showModal()` and `show()` differ in exactly one observable way that matters to rendering: the modal
joins the **top layer** and the non-modal one stays in flow. That distinction is decided in JS and
consumed in **Rust** (`z_index_map`), and a JS-side property is invisible across that boundary. So
`showModal()` sets `data-manuk-modal` alongside `open`, and `close()` removes it — the same device
as the existing `data-manuk-adopted` marker on adopted stylesheets. It is honest (the flag really is
part of the element's state) at the cost of being page-visible.

### The top layer is one line at one choke point

`Page::z_index_map` is the single place all three consumers (paint, hit-testing, a11y) read stacking
from, so promoting a modal to `TOP_LAYER_Z = 1_000_000_000` there covers all of them at once. The
subtree inherits it for free — the map already passes the resolved `z` down to children.

Without the promotion, a `<dialog>` declared early in `<body>` (where authors put it) paints *behind*
the sticky header and the `z-index: 50` overlay it exists to cover. `g_dialog_render` asserts exactly
that ordering against the real display list, and it goes red when the branch is removed.

### `<form method="dialog">` is markup, not script

`<form method="dialog"><button value="ok">OK</button></form>` must close the dialog with
`returnValue === 'ok'` and navigate **nowhere**. Handled by a capture-phase `click` listener on the
document, so it runs before the native submit path can treat the form as a GET. `formmethod` on the
button overrides the form's `method`, per spec.

### `requestClose()` is `close()` with a veto (tick 491)

`dlg.requestClose([returnValue])` (Baseline 2025) is what a Close button and the ✕ should call instead of
`close()`. The difference is one cancelable event: it fires a **cancelable `cancel`** first, so a "discard
unsaved changes?" guard can `preventDefault()` and keep the dialog open; if nothing vetoes, it runs the
normal close (which fires the non-cancelable `close`). It is the exact veto path Escape already used — Escape
dispatches a cancelable `cancel` then `close()`s if not prevented — now exposed as a method. Guards mirror
`close()`: no-op on a dialog without `open` (no `cancel`, no throw), and the returnValue argument threads
through. Absent, `dlg.requestClose()` was a TypeError that took the click handler with it.

## Gates

- `engine/page/tests/g_dialog_request_close.rs` — `requestClose()`: closes-when-unvetoed + `close` fires +
  `cancel` is cancelable + returnValue threads through; a `preventDefault()` in `cancel` keeps it open; a
  closed dialog is a silent no-op.
- `engine/page/tests/g_dialog.rs` — the JS surface: 13 claims (branding, open/close, `returnValue`,
  the `close` event, `InvalidStateError` on re-`showModal()`, `form method=dialog`, Escape's
  cancelable `cancel`, and `show()` *not* joining the top layer).
- `engine/page/tests/g_dialog_render.rs` — the half script cannot see: a closed dialog produces no
  box and no display item; an open modal paints after a `z-index: 50` overlay.
- `engine/page/tests/g_popover.rs` — 14 claims (detection, reflection, show/hide/toggle, the
  cancelable `beforetoggle`, `popovertarget`, `auto` exclusivity vs `manual`, light dismiss, Escape).
- `engine/page/tests/g_popover_render.rs` — a closed popover produces no box and no display item; an
  open one paints after a `z-index: 50` header.

Every one was proven red by reverting each half independently — `display: none` → `block` gave the
closed dialog/popover a real 18.4px box, and disabling the `TOP_LAYER_Z` branch put the modal/menu
behind the overlay. Run them with `--features stylo,spidermonkey`; they are **not yet registered in
`scripts/verify.sh`** (harness is observer-owned — see the tick 194/195 journal entries).

## The `popover` attribute API (tick 195)

The other half of the top layer, and the same two-part failure: `showPopover()` was a TypeError, and
with no `[popover]` UA rule the menu's items rendered inline in the middle of the page. Every menu,
tooltip, dropdown and toast that has stopped being a hand-rolled `<div class="dropdown">` plus an
outside-click listener is a popover.

Built on exactly the machinery tick 194 laid down, which is why it fits in one tick:

- **State flag** — `data-manuk-popover-open` **is** the `:popover-open` state. The UA sheet keys
  `display` off it (`[popover]` hidden, `[popover][data-manuk-popover-open]` a bordered block, in both
  cascades) and `z_index_map` reads it for the same `TOP_LAYER_Z` promotion a modal gets. Same
  JS↔Rust boundary problem, same solution as `data-manuk-modal`.
- **`showPopover` / `hidePopover` / `togglePopover(force)`**, and `el.popover` reflecting
  `auto`/`manual`/`null` (`auto` is the enumerated attribute's invalid-value default).
- **`beforetoggle` / `toggle`** with `oldState`/`newState`. `beforetoggle` is **cancelable**, which is
  the veto hook; `toggle` is the after-the-fact notification.
- **`<button popovertarget="menu" popovertargetaction="show|hide|toggle">`** — declarative, no script.
  This is how the API is meant to be used and the reason it shipped.
- **Light dismiss** — a click anywhere outside an open `auto` popover closes it, and so does Escape.
  A `manual` popover ignores both. Opening an `auto` popover closes the other `auto` ones (the flat
  exclusivity case; nested submenus are residue).

### `HTMLElement.prototype` is not `__protoHTMLElement`

`'popover' in HTMLElement.prototype` — the canonical detection for this API — was **false** while
every element in the page had the members. The custom-elements shim
(`dom_bindings.rs`) gives the `HTMLElement` constructor a fresh `{}` prototype on purpose, because
upgrade grafts methods onto the host object and a reflector's prototype cannot be swapped. So the
constructor's prototype and the real element prototype are different objects, and detection reads the
wrong one. Tick 195 mirrors the dialog + popover descriptors onto the constructor's prototype so both
reads agree.

**This is a plaster on a wider hole:** *every* `'x' in HTMLElement.prototype` feature detection has
the same blind spot. Logged as residue — unifying the two prototypes is its own tick, with the
custom-element gates as the thing that must stay green.

## Known gaps, deliberately not in this tick

- **`::backdrop`** — the dimming layer behind a modal. Needs a pseudo-element box with no DOM node.
- **Inertness** — a modal should make the rest of the document non-interactive (no clicks, no focus,
  no hit-test) and trap focus. Today the page behind a modal is still clickable.
- **Nested popovers** — a submenu inside its parent menu; today `auto` exclusivity is flat, so
  opening a child closes its parent.
- **Popover positioning** — anchor positioning (`anchor-name`/`position-area`) is absent, so a popover
  is a block in flow rather than floating next to its invoker.
- **Auto-centering** — Chrome's UA sheet gives a modal `position: fixed; inset: 0; width: fit-content`
  so it floats centered in the viewport. Ours is a centered-by-`margin: auto` block in flow, so an
  open modal still occupies layout space rather than overlaying it. `z_index_map` puts it on top; the
  *geometry* is still in-flow.

## An unrelated bug this tick surfaced

A `position: absolute` element with no background emits **no display item at all** — its text never
reaches the display list (probed while building the stacking fixture, which is why that fixture uses
`position: relative`). Not caused by this tick and not fixed by it; logged for a future tick.
