# `pressed` and `invalid` — the two states the tree did not have

> Landed t1385. Gate: `a_toggle_button_and_a_rejected_field_report_their_state`
> (`agent/tests/g_ax_pressed_and_invalid.rs`), oracle = CDP `Accessibility.getFullAXTree`.
> Surface audit #80's ranked #1, second pass.

## ⭐⭐⭐ `A11yState`'s own doc comment described the defect

> *"Without it the tree says `checkbox "Remember me"` before a click and `checkbox "Remember me"`
> after it — identical. An agent that cannot observe the result of its own action cannot verify it,
> so it either proceeds on faith or re-clicks and toggles the setting back off."*

That sentence was **still true of every toggle button on the web**. `Follow`, `Bold`, `Mute`, a
filter chip, a "show password" eye — they are `<button aria-pressed>`, not checkboxes, so `checked`
never applied and the tree read `button "Follow"` in both states. The struct had eight fields, and
the ninth was the one its own rationale was about.

`aria-invalid` is the twin of a field that already existed: `required`'s doc says *"which field a
blocked form submission is complaining about"*, and `invalid` is how the page ANSWERS that once the
submission is refused. Without it, an agent that submits, is rejected, and re-reads the tree has one
signal — the page did not navigate — and no way to find the field.

## The battery

```text
                                   chrome                    before        after
  aria-pressed=true                pressed: 'true'           (no field)    pressed
  aria-pressed=false               pressed: 'false'          (no field)    unpressed
  aria-pressed=mixed               pressed: 'mixed'          (no field)    partially-pressed
  a plain <button>       CONTROL   no `pressed` property     —             None
  aria-pressed="yes"     CONTROL   no `pressed` property     —             None
  aria-invalid=true                invalid: 'true'           (no field)    invalid
  aria-invalid=spelling            invalid: 'true'           (no field)    invalid
  aria-invalid=grammar             invalid: 'true'           (no field)    invalid
  aria-invalid=false     CONTROL   invalid: 'false'          —             false
  no aria-invalid        CONTROL   invalid: 'false'          —             false
  aria-invalid=sortof    CONTROL   invalid: 'false'          —             false
```

⭐⭐ **`mixed` is a real authored value, not a defensive third case** — a `Bold` button over a
selection that is partly bold. Flattening it to `false` tells an agent the opposite of what the page
means, which is the argument `Checked` already carries and the reason `pressed` reuses that tri-state
rather than being a `bool`.

⭐⭐ **`aria-invalid` is an ENUMERATION, and `grammar` / `spelling` are TRUTHY** — they say what KIND
of wrong, not whether. Chrome reports `invalid: 'true'` for both, measured. The obvious rule
`!= "false"` agrees on **five of six rows** and disagrees only on an out-of-vocabulary token, which
ARIA's enumerated-value rule makes the default. `v_junk` is in the fixture for exactly that reason,
and it is the only row mutation N2 fails.

⭐ **`pressed` renders as its own word** (`pressed` / `unpressed` / `partially-pressed`), not as
`checked`. An agent reading `[checked]` on a `button` is being told about a control that is not
there. Same tri-state, different vocabulary — and the render rows assert it separately from the
values, because they are separate claims.

⚠ Both entrances: `state_of` AND the published a11y tree.

## How it was proven red

- **N1** — delete the `pressed` arm: the three pressed rows and both tree rows fail; `p_none` and
  `p_junk` stay green, because *"not a toggle"* was already the answer for them.
- **N2** — compute `invalid` as `!= "false"`: only `v_junk` fails. The wrong rule is right five times
  out of six.
- **N3** — render `pressed` through `checked`'s words: the three render rows fail while every state
  row stays green.
