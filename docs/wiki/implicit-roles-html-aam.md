# The implicit roles that were falling through to a plausible default

> Landed t1384. Gate: `implicit_roles_match_html_aam` (`agent/tests/g_ax_role_html_aam.rs`),
> 28 rows, oracle = CDP `Accessibility.getFullAXTree`. Surface audit #80's ranked #1, executed.

## Why CDP and not WPT

Interop 2026 lists **accessibility testing as an INVESTIGATION effort**, not a focus area — the four
engine vendors' own position is that there is not yet a suite that can decide a11y-tree correctness.
So the oracle for every row here is Chrome's own tree, read through CDP on `--headless=new
--force-renderer-accessibility`. Audit #80 records why.

## The mechanism — a default that answers plausibly instead of correctly

`role_of`'s `<input>` dispatch ended `_ => Role::TextBox`, and its element dispatch ends
`_ => Role::Generic`. Both are reasonable-looking fallbacks, and **both produce an answer an agent
will act on**.

```text
                           chrome (CDP)          before         after
  <input type=file>        button                textbox        button
  <input type=color>       ColorWell (internal)  textbox        generic
  <select multiple>        listbox               combobox       listbox
  <select size=4>          listbox               combobox       listbox
  <fieldset>               group                 generic        group
  <details>                group                 generic        group
  <address>                group                 generic        group
  <hgroup>                 group                 generic        group
```

⭐⭐⭐ **`<input type=file>` is the row with teeth.** As a `textbox` an upload control is invisible to
*"click Choose File"* — and, much worse, `type_into` ACCEPTS it and silently does nothing, because a
file input has no text to type into.

> **A wrong role that an actuator will act on is a lie the actuator cannot detect.**

Same family as t1380's phantom menu link: the perception layer hands the driver a plan that cannot
execute.

⭐⭐ **`<select multiple>` and `<select size=4>` are two different widgets, not one.** A combobox is
opened and one option chosen; a listbox is a visible list whose selection may be plural. HTML-AAM
makes `multiple` OR `size > 1` the discriminator, and both spellings are asserted because either
alone would pass against an implementation that read only the other. `size=1` is asserted as the
boundary, so the rule is `> 1` and not *"has a size attribute"*.

⭐ **`<fieldset>` is the row with corpus weight** — every multi-section form on the web is built out
of it, and it is what an agent walks to find *"the Billing address fields"*. Its NAME already came
from `<legend>` correctly, so what shipped was **a correct name on a meaningless role**.

## ⚠ Measured and deliberately not changed

```text
  <input type=date/time/datetime-local/month/week>   chrome: Date / InputTime / DateTime
                                                      ours:   textbox   (KEPT, and asserted)
```

Chrome's roles here are internal, with no ARIA equivalent — and unlike a colour well these controls
really do accept typed text, so `textbox` is both the useful answer and a non-harmful one. The five
rows are asserted **as** `textbox` so the decision is a claim rather than an omission.

⚠ `<summary>` (Chrome `DisclosureTriangle`), `<figcaption>`, `<legend>`, `<dl>`, `<abbr>`, `<video>`
and `<audio>` also get Chrome INTERNAL role names with no ARIA counterpart. Adopting them would put
Chrome internals into a vocabulary that is otherwise ARIA's, so they stay `generic`. `<summary>`'s
empty NAME (Chrome says `"More"`) is a separate gap, first surfaced by t1380's own control row, and
still open.

## Both entrances

The rows are asserted through `role_of` AND through the published a11y tree, because the agent reads
the tree — and a mapping wired to one entrance is the shape this file's neighbours have been caught
by four times (t1097, t1350, t1355, t1365).

## How it was proven red

- **N1** — delete the `"file" => Role::Button` arm: only `i_file` fails, at `textbox`. Every
  date/time row stays green, which is what says the fall-through was wrong for ONE input type and
  deliberately kept for five others.
- **N2** — make the `<select>` test `size.is_some()` instead of `> 1`: only `s_size1` fails. The
  boundary row is the one that separates the rule from the attribute's presence.
- **N3** — map only `<fieldset>` to `Group`: `details`, `address` and `hgroup` fail while `fieldset`
  passes. Four elements, one HTML-AAM row, and a partial fix looks identical to a complete one on the
  fixture that motivated it.
