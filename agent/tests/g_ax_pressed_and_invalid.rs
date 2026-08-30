//! **G_AX_PRESSED_AND_INVALID — the two states a toggle button and a rejected form field have, and
//! the tree had neither.**
//!
//! Surface audit #80's ranked #1, second pass: sweep the a11y tree against CDP
//! `Accessibility.getFullAXTree` — the oracle, because Interop 2026 lists accessibility testing as
//! an INVESTIGATION effort and there is no suite to ask.
//!
//! ## ⭐⭐⭐ `A11yState`'s OWN DOC COMMENT DESCRIBED THE DEFECT
//!
//! > *"Without it the tree says `checkbox "Remember me"` before a click and `checkbox "Remember me"`
//! > after it — identical. An agent that cannot observe the result of its own action cannot verify
//! > it, so it either proceeds on faith or re-clicks and toggles the setting back off."*
//!
//! That sentence was **still true of every toggle button on the web**. `Follow`, `Bold`, `Mute`, a
//! filter chip, a "show password" eye — they are `<button aria-pressed>`, not checkboxes, so
//! `checked` never applied and the tree read `button "Follow"` in both states. The struct had eight
//! fields and the ninth was the one its own rationale was about.
//!
//! `aria-invalid` is the twin of a field that already exists: `required`'s doc says *"which field a
//! blocked form submission is complaining about"*, and `invalid` is how the page ANSWERS that after
//! the submission is refused. Without it an agent that submits, is rejected, and re-reads the tree
//! has one signal — the page did not navigate — and no way to find the field.
//!
//! ## THE BATTERY — Chrome via CDP `Accessibility.getFullAXTree`
//!
//! ```text
//!                                    chrome                    before        after
//!   aria-pressed=true                pressed: 'true'           (no field)    pressed
//!   aria-pressed=false               pressed: 'false'          (no field)    unpressed
//!   aria-pressed=mixed               pressed: 'mixed'          (no field)    partially-pressed
//!   a plain <button>       CONTROL   no `pressed` property     —             None
//!   aria-invalid=true                invalid: 'true'           (no field)    invalid
//!   aria-invalid=spelling            invalid: 'true'           (no field)    invalid
//!   aria-invalid=grammar             invalid: 'true'           (no field)    invalid
//!   aria-invalid=false     CONTROL   invalid: 'false'          —             false
//!   no aria-invalid        CONTROL   invalid: 'false'          —             false
//! ```
//!
//! ⭐⭐ **`mixed` is a real authored value, not a defensive third case** — a `Bold` button over a
//! selection that is partly bold. Flattening it to `false` tells an agent the opposite of what the
//! page means, which is the same argument `Checked` already carries and the reason `pressed` reuses
//! that tri-state rather than being a `bool`.
//!
//! ⭐⭐ **`aria-invalid` is an ENUMERATION, and `grammar` / `spelling` are TRUTHY.** They say what
//! KIND of wrong, not whether — Chrome reports `invalid: 'true'` for both, measured. A `!= "false"`
//! test would agree by accident and then disagree on a typo'd token, which ARIA's enumerated-value
//! rule makes `false`; the rows for `spelling` and `grammar` are what force the truthy SET.
//!
//! ⭐ **`pressed` renders as its own word (`pressed` / `unpressed` / `partially-pressed`), not as
//! `checked`.** An agent reading `[checked]` on a `button` is being told about a control that is not
//! there. Same tri-state, different vocabulary, and the render row asserts it.
//!
//! ⚠ Both entrances: `state_of` AND the published a11y tree, because the agent reads the tree.

use manuk_a11y::{state_of, Checked, Role};

const HTML: &str = r##"<!doctype html><html><head></head><body>
<button id="p_true" aria-pressed="true">Follow</button>
<button id="p_false" aria-pressed="false">Follow</button>
<button id="p_mixed" aria-pressed="mixed">Bold</button>
<button id="p_none">Plain</button>
<button id="p_junk" aria-pressed="yes">Junk</button>
<input id="v_true" aria-invalid="true">
<input id="v_spell" aria-invalid="spelling">
<input id="v_gram" aria-invalid="grammar">
<input id="v_false" aria-invalid="false">
<input id="v_absent">
<input id="v_junk" aria-invalid="sortof">
<input id="c_check" type="checkbox" checked>
<input id="c_req" required>
<div id="c_dis" role="button" aria-disabled="true">D</div>
</body></html>"##;

#[test]
fn a_toggle_button_and_a_rejected_field_report_their_state() {
    let fonts = manuk_text::FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://state.test/", &fonts, 1200.0);
    let dom = page.dom();
    let tree = page.a11y_tree();
    let node = |id: &str| {
        dom.get_element_by_id(dom.root(), id)
            .unwrap_or_else(|| panic!("VACUOUS: the fixture has no #{id}"))
    };
    let state = |id: &str| {
        let n = node(id);
        let role = manuk_a11y::role_of(dom, n)
            .unwrap_or_else(|| panic!("VACUOUS: #{id} maps to no ARIA role"));
        state_of(dom, n, &role)
    };

    // ── VACUITY. The fields that already existed must still work, or a struct-wide regression would
    //    read as this gate passing on two new ones.
    assert_eq!(
        state("c_check").checked,
        Some(Checked::True),
        "VACUOUS: `checked` stopped working, so nothing here is about the NEW fields"
    );
    assert!(
        state("c_req").required,
        "VACUOUS: `required` stopped working"
    );
    assert!(
        state("c_dis").disabled,
        "VACUOUS: `disabled` stopped working"
    );

    // (id, expected pressed, what the row decides)
    let pressed: &[(&str, Option<Checked>, &str)] = &[
        ("p_true", Some(Checked::True), "THE DEFECT — a `Follow` button read identically before and after the click that changed it"),
        ("p_false", Some(Checked::False), "the OFF state is a state, not an absence: `Some(False)` means \"this toggles and is currently off\""),
        ("p_mixed", Some(Checked::Mixed), "`mixed` is a real authored value (a Bold button over a partly-bold selection), and flattening it to false says the opposite of what the page means"),
        ("p_none", None, "CONTROL — a plain button has no pressedness at all; `None` is \"not applicable\", which is what stops an agent reading `unpressed` on an ordinary button"),
        ("p_junk", None, "CONTROL — an out-of-vocabulary token is not a state. ARIA's enumerated-value rule makes it the default, and the default here is `not a toggle`"),
    ];
    for (id, want, why) in pressed {
        let got = state(id).pressed;
        assert_eq!(
            got, *want,
            "G_AX_PRESSED_AND_INVALID #{id} pressed: Chrome reports {want:?}, got {got:?}.\n  {why}"
        );
    }

    // (id, expected invalid, what the row decides)
    let invalid: &[(&str, bool, &str)] = &[
        ("v_true", true, "the field a blocked submission is complaining about — the twin of `required`, which already names that job"),
        ("v_spell", true, "`spelling` is TRUTHY: it says what KIND of wrong, not whether. Chrome reports invalid:'true'"),
        ("v_gram", true, "…and so is `grammar`. These two rows are what force a truthy SET rather than `!= \"false\"`"),
        ("v_false", false, "CONTROL — the explicit negative"),
        ("v_absent", false, "CONTROL — no attribute at all"),
        ("v_junk", false, "CONTROL — an out-of-vocabulary token falls back to the default, which is why `!= \"false\"` is the wrong test even though it agrees on every row above"),
    ];
    for (id, want, why) in invalid {
        let got = state(id).invalid;
        assert_eq!(
            got, *want,
            "G_AX_PRESSED_AND_INVALID #{id} invalid: Chrome reports {want}, got {got}.\n  {why}"
        );
    }

    // ── THE RENDERED LINE, which is what an agent actually reads. `pressed` must not borrow
    //    `checked`'s vocabulary: `[checked]` on a `button` describes a control that is not there.
    let rendered: &[(&str, &str)] = &[
        ("p_true", " [pressed]"),
        ("p_false", " [unpressed]"),
        ("p_mixed", " [partially-pressed]"),
        ("p_none", ""),
        ("v_true", " [invalid]"),
        ("v_absent", ""),
    ];
    for (id, want) in rendered {
        let got = state(id).render();
        assert_eq!(
            &got, want,
            "G_AX_PRESSED_AND_INVALID #{id} render: expected {want:?}, got {got:?} — the observation \
             line is the agent's whole view of state"
        );
    }

    // ── AND THROUGH THE TREE, not only through `state_of`.
    for (id, want_pressed) in [
        ("p_true", Some(Checked::True)),
        ("p_mixed", Some(Checked::Mixed)),
    ] {
        let n = node(id);
        let in_tree = tree
            .iter()
            .find(|x| x.node == n)
            .unwrap_or_else(|| panic!("VACUOUS: #{id} is not in the a11y tree"));
        assert_eq!(
            in_tree.state.pressed, want_pressed,
            "G_AX_PRESSED_AND_INVALID #{id} (AX TREE): the tree publishes {:?} — a state wired to \
             `state_of` and not to the tree is not wired",
            in_tree.state.pressed
        );
        assert_eq!(
            in_tree.role,
            Role::Button,
            "the subject must still be a button"
        );
    }
    let v = node("v_true");
    assert!(
        tree.iter()
            .find(|x| x.node == v)
            .is_some_and(|x| x.state.invalid),
        "G_AX_PRESSED_AND_INVALID #v_true (AX TREE): the tree must publish `invalid` too"
    );
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// N1  delete the `pressed` arm from `state_of` (the pre-tick state)
//       -> the three pressed rows and both tree rows fail; `p_none`/`p_junk` stay green, because
//          "not a toggle" was already the answer for them.
// N2  compute `invalid` as `attr("aria-invalid").is_some_and(|v| v != "false")`
//       -> only `v_junk` fails. Every other row agrees, which is exactly why the junk row is in the
//          fixture: the wrong rule is right five times out of six.
// N3  render `pressed` through `checked`'s words
//       -> the three render rows fail while every state row stays green: the vocabulary is a
//          separate claim from the value.
