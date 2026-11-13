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
  the veto hook; `toggle` is the after-the-fact notification. As of **tick 526** they also carry
  **`ToggleEvent.source`** (Baseline 2024) — the invoker that caused the toggle: the
  `<button popovertarget>` for a declarative open, the `{source}` option for `showPopover({source})` /
  `hidePopover({source})`, and `null` for a bare imperative call. It threads through `__popToggleEvent`'s
  new `source` argument; `__popClick` passes the button (`{source: t}`), and `togglePopover` accepts
  both `togglePopover(force)` and `togglePopover({force, source})`. A menu/tooltip framework reads it to
  anchor and focus the popover on the control the user clicked — without it, one menu opened from three
  buttons can't tell which fired. Gated `g_toggle_event_source` (imperative-with-source / bare-null /
  declarative-button); RED: drop `ev.source`. Residue: the `<dialog>` toggle path does not yet carry
  `source` (a separate command-invoker brick).
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

## RG.2.S1.a: `aria-hidden` on an element that holds focus is ignored, and the agent never drained focus

gomail's New Message window is a MUI `Dialog` with `disablePortal`: the Modal manager sets `aria-hidden="true"` on the `<body>` child that CONTAINS the dialog (the app root) and focuses the dialog. Chrome ignores `aria-hidden` on an element that holds the focused node, so Minimize/Close/Send and the fields stay in its tree. manuk dropped the whole subtree: after the click `observe(interactive)` listed nothing. Two causes: (1) `build_children` honoured the attribute unconditionally; it now keeps an `aria-hidden` ancestor-or-self of `dom.focused()` (`focus_retained`); (2) `el.focus()` only queues a request that the shell drains, and the agent/MCP path never drained it, so `dom.focused()` was empty. `AgentBrowser::settle` now applies the last request. An `aria-hidden` root with focus left outside it is still pruned (fixture `dialog_in_hidden_root_nofocus`).

Same node, second cause: the message body is Quill's `<div class="ql-editor" contenteditable>` with no role. Chrome exposes an editing host as `textbox`; `role_of` gave `generic`, so it was not in the interactive tree and `type` refused it ("not a text field"). `role_of` now returns `TextBox` for a `contenteditable` root (not `false`, not an editable child of an editable parent), and `Page::type_text` inserts into the host's first block (dropping the placeholder `<br>`) between `beforeinput` and `input`. Gate `contenteditable_host_without_a_role_is_a_textbox`.

## RG.2.S1.a.a.a: moving focus blurs the field that lost it

`Page::set_focus` fired `focusin`/`focus` on the new element but nothing on the old one, so a component that commits on blur never committed. gomail's Recipients box is a MUI Autocomplete: the typed address becomes the mail's `to` on `focusout`. Under manuk the agent typed the address, moved on to Subject, and the scheduled mail was stored with `to: [""]` (pinned Chrome stores `["jane.doe@example.com"]`). Now the previous holder gets `blur` then `focusout` before the new one gets `focusin`/`focus`. `change` is not fired here; `type_text` already fires it when its typing ends. Gate: `mcp/tests/blur_commit.rs`.

## RG.2.S1.a.b: typed editor text reaches the editor's MutationObserver, and created blocks serialize lowercase

gomail-3 sent the email but the saved body was empty, then `<P>…</P>`. Two causes, both found from the final app state of a manuk run. (1) `Page::type_text` into an editing host (Quill's `.ql-editor`) edited the DOM natively, which queues no `MutationObserver` records; Quill refreshes its model from those records, so Send posted an empty body. The edit now runs as page script (`removeChild` of the placeholder `<br>`, `appendChild` of the text node) via `insert_into_host_via_script`, so the wrapped DOM methods record it; the native edit stays as the no-script fallback. (2) `document.createElement('P')` stored the name as given, so `innerHTML` serialized `<P>`; an HTML document now lowercases the name (XML documents keep case). Gates: `mcp/tests/editor_mutation.rs` (two tests, both RED before).

## RG.2.S1.a.a.b: typed characters are real InputEvents with the caret at the end

gomail's schedule-send date field (MUI sectioned field) wiped to its `MM/DD/YYYY` placeholder when the agent typed `11/16/2026`, so Schedule stayed disabled and the agent fell back to calendar clicks. `Page::type_text` fired `beforeinput`/`input` as bare objects (no `InputEvent`, no `inputType`, no `data`) and left `selectionStart` at 0; such a field keys off `inputType: insertText`, `data` and the caret. Each typed character now dispatches an `InputEvent` (`insertText`, `data` = the char, `composed`, `beforeinput` cancelable) via `PageContext::dispatch_input_event`, with the caret moved to the end of the value first (before `beforeinput`: end of the old value; before `input`: end of the new one), as Chrome does. `dispatch_input` (non-typing callers) is unchanged. Gate: `engine/page/tests/g_type_text_inputevent.rs` (RED: `Object:undefined:undefined:0`, field wiped; GREEN: `InputEvent:insertText:<char>:<caret>`).

## RG.2.S1.a.a.a.a: typing over existing text is select-all-then-type, not clear-then-type

The first fix (InputEvents) was not enough on the real gomail picker: `type_text` still fired an `input` announcing an empty value before the first character when the field held text, and MUI's sectioned date field reads an empty `input` as "clear the value" and wipes to the placeholder (Schedule send stayed disabled). Chrome (pinned 145.0.7632.116, Playwright click, Ctrl+A, type) never announces an empty value: the page hears the Ctrl+A keydown (MUI answers with `setSelectedSections("all")`), the first `beforeinput` sees the whole old value selected, and its `input` carries the old value with that selection replaced; MUI then moves the selection to the next section itself. `type_text` now dispatches keydown/keyup `a` with ctrl and, if no handler prevented it, selects all; each typed `input` is applied by `setRangeText(data, selectionStart, selectionEnd, 'end')` inside `dispatch_input_event` (the native route, so React's value tracker still sees the change) instead of the Rust side overwriting the whole value. Fixture `typing_over_existing_text_replaces_the_selection_without_an_empty_input` in `engine/page/tests/g_type_text_inputevent.rs`. Real check: manuk-mcp drives evals-gomail Compose, Send arrow, Pick a custom date and time, types `11/16/2026 09:00 AM`; the field reads `11/16/2026 09:00 AM` and Schedule send is enabled (before: placeholder, disabled).
