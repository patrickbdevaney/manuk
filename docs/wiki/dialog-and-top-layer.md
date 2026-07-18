# `<dialog>` and the top layer (tick 194)

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

## Gates

- `engine/page/tests/g_dialog.rs` — the JS surface: 13 claims (branding, open/close, `returnValue`,
  the `close` event, `InvalidStateError` on re-`showModal()`, `form method=dialog`, Escape's
  cancelable `cancel`, and `show()` *not* joining the top layer).
- `engine/page/tests/g_dialog_render.rs` — the half script cannot see: a closed dialog produces no
  box and no display item; an open modal paints after a `z-index: 50` overlay.

Both were proven red by reverting each half independently (`dialog { display: none }` → `block`, and
`TOP_LAYER_Z` → `z`). Run them with `--features stylo,spidermonkey`; they are **not yet registered in
`scripts/verify.sh`** (harness is observer-owned — see the tick 194 journal entry).

## Known gaps, deliberately not in this tick

- **`::backdrop`** — the dimming layer behind a modal. Needs a pseudo-element box with no DOM node.
- **Inertness** — a modal should make the rest of the document non-interactive (no clicks, no focus,
  no hit-test) and trap focus. Today the page behind a modal is still clickable.
- **Light-dismiss / `popover`** — the `popover` attribute API (`showPopover`/`hidePopover`,
  `popovertarget`) shares the top layer and is still absent.
- **Auto-centering** — Chrome's UA sheet gives a modal `position: fixed; inset: 0; width: fit-content`
  so it floats centered in the viewport. Ours is a centered-by-`margin: auto` block in flow, so an
  open modal still occupies layout space rather than overlaying it. `z_index_map` puts it on top; the
  *geometry* is still in-flow.

## An unrelated bug this tick surfaced

A `position: absolute` element with no background emits **no display item at all** — its text never
reaches the display list (probed while building the stacking fixture, which is why that fixture uses
`position: relative`). Not caused by this tick and not fixed by it; logged for a future tick.
