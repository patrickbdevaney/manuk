//! **G_ACCESSKIT_STATE_COMPLETE — the projection carried four of the tree's ten state fields, and
//! named the ROOT as focused on every page.**
//!
//! t1452 stood the AccessKit projection up and carried `checked`, `expanded`, `selected` and
//! `disabled`. It dropped `pressed`, `required`, `readonly`, `invalid` and `value` — and set
//! `TreeUpdate::focus` to the document root unconditionally.
//!
//! ⭐⭐⭐ **`focus` IS A REQUIRED FIELD, SO NAMING THE ROOT IS AN ANSWER RATHER THAN AN ABSTENTION.**
//! AccessKit's contract is that every update names the focused node; there is no `None`. Pointing it
//! at the root tells a screen reader *"the document has focus"* while the caret sits in a text field,
//! and tells an agent reading its own tree back that its `focus()` call went nowhere. **A required
//! field with a plausible default is the most dangerous shape a projection has**: the consumer cannot
//! tell *"we computed the root"* from *"we did not compute"*.
//!
//! ⭐⭐ **`pressed` is the one this crate's own doc comment calls a toggle button's ONLY observable
//! state.** `Follow`, `Bold`, `Mute`, a filter chip, a "show password" eye are all
//! `<button aria-pressed>` and never checkboxes — so without it the projected tree reads
//! `button "Follow"` before and after a click, identically, which is the exact failure the
//! accessibility tree was built to prevent.
//!
//! ```text
//!   <button aria-pressed=true>Follow</button>     Toggled::True     (was: no toggle at all)
//!   <input required>                              is_required       (was: dropped)
//!   <input readonly value="RO">                   is_read_only + value
//!   <input aria-invalid=true>                     Invalid::True     (was: dropped)
//!   focus on the text input                       focus == that node (was: the ROOT)
//! ```
//!
//! ⚠ `checked` and `pressed` are two ARIA sources for ONE AccessKit property (`toggled`), and a node
//! carries at most one of them meaningfully. `checked` wins where both somehow appear.
//!
//! ⚠ `invalid` is a BOOL in this crate and an ENUM in AccessKit (`True | Grammar | Spelling`).
//! `aria-invalid="spelling"` is a real authored value the tree does not yet distinguish, so it maps
//! to `True` and the narrowing is recorded rather than guessed at.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><meta charset=utf-8></head><body>
<button id=follow aria-pressed="true">Follow</button>
<button id=mute aria-pressed="false">Mute</button>
<input id=req required aria-label="Email">
<input id=ro readonly value="RO" aria-label="Locked">
<input id=bad aria-invalid="true" aria-label="Broken">
<input id=focusme aria-label="Search">
</body></html>"##;

#[test]
fn every_state_the_tree_computes_reaches_the_projection() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://akstate.test/", &fonts, 800.0);

    let focus_node = manuk_css::query_selector_all(page.dom(), page.dom().root(), "#focusme")
        .first()
        .copied()
        .expect("the fixture's focus target");
    let tree = page.a11y_tree_with_focus(Some(focus_node));
    let update = manuk_a11y::accesskit_bridge::tree_update(&tree);

    // ── VACUITY. If the widgets are not in the projection at all, every row below is vacuously
    //    about nothing — and t1452's gate already proved role+label survive, so this is the floor
    //    those rows established rather than a new claim.
    let by_label = |label: &str| -> &accesskit::Node {
        update
            .nodes
            .iter()
            .map(|(_, n)| n)
            .find(|n| n.label().as_deref() == Some(label))
            .unwrap_or_else(|| panic!("VACUOUS: no projected node labelled {label:?}"))
    };

    // ── `pressed` — the field a toggle button's whole observability rests on.
    assert_eq!(
        by_label("Follow").toggled(),
        Some(accesskit::Toggled::True),
        "⭐⭐ `aria-pressed=true` must reach `toggled`. Without it a toggle button reads identically \
         before and after a click, which is the failure the tree exists to prevent."
    );
    assert_eq!(
        by_label("Mute").toggled(),
        Some(accesskit::Toggled::False),
        "⚠ FALSE IS NOT ABSENT — 'this button is not pressed' and 'this is not a toggle button' are \
         different facts, and only one of them tells an agent it may click."
    );

    // ── the form fields.
    assert!(
        by_label("Email").is_required(),
        "`required` must reach the projection — it is which field a blocked submission is about"
    );
    let ro = by_label("Locked");
    assert!(
        ro.is_read_only(),
        "`readonly` must reach the projection — an agent that types into a readonly field waits \
         forever for a change that cannot happen"
    );
    assert_eq!(
        ro.value().as_deref(),
        Some("RO"),
        "⭐ THE VALUE IS WHAT THE FIELD CURRENTLY CONTAINS, and an agent verifying its own typing has \
         nothing else to compare against"
    );
    assert_eq!(
        by_label("Broken").invalid(),
        Some(accesskit::Invalid::True),
        "`aria-invalid` must reach the projection — it is how an agent finds the field the page is \
         complaining about"
    );

    // ── FOCUS. The required field that had a plausible default.
    let focused = update
        .nodes
        .iter()
        .find(|(id, _)| *id == update.focus)
        .map(|(_, n)| n)
        .expect("the focused id must name a node that is actually in the update");
    assert_eq!(
        focused.label().as_deref(),
        Some("Search"),
        "⭐⭐⭐ `TreeUpdate::focus` must name the FOCUSED node, not the document root. AccessKit has no \
         `None` here, so the root is an ANSWER — and a wrong one on every page with a caret in it."
    );
    assert_ne!(
        update.focus,
        update.tree.as_ref().unwrap().root,
        "the focused node must not be the root while a real element holds focus — this is the \
         assertion that distinguishes 'computed' from 'defaulted'"
    );
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// B1  drop `pressed` from the toggled source (`n.state.checked` alone)
//       -> both button rows; every form row and the focus row stay green, which separates the two
//          halves of this tick.
// B2  set `focus` to the root again (t1452's behaviour)
//       -> the two focus rows alone.
// B3  drop `set_required` / `set_read_only` / `set_invalid` / `set_value`
//       -> the four form rows, one per dropped call.
// B4  prefer `pressed` over `checked` instead of the other way round
//       -> nothing in THIS fixture moves, and that is reported rather than claimed: no element here
//          carries both, because an element that carries both is an authoring error. The precedence
//          is asserted by construction in the source and is not gated.
