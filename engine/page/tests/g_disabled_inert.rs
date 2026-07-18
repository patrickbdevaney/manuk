//! G_DISABLED_INERT — a disabled control does nothing, and says so.
//!
//! **The failure this gate exists for.** Activation behaviour (tick 208) and label forwarding (tick
//! 209) both ran without checking whether the control was disabled, so clicking a disabled checkbox
//! ticked it. A disabled control is not "styled grey" — it is **inert**, and clicking it must leave
//! the page exactly as it was.
//!
//! **This is worse than cosmetic for an agent**, which is why it is worth its own gate: the agent
//! ticks a disabled consent box, reads the state back (tick 199 gave it that), sees it ticked, and
//! reports success — on a form the server will reject. A wrong *observation* is more expensive than
//! a failed action, because nothing downstream questions it.
//!
//! Also gates the INHERITED case: `<fieldset disabled>` is the idiomatic way to disable a whole step
//! of a multi-step form, and checking only the control's own attribute leaves every control in it
//! live.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html>
<html><body>
  <input type="checkbox" id="off" disabled>
  <label for="off" id="offlbl">Disabled via label</label>

  <fieldset disabled>
    <input type="checkbox" id="instep">
    <label for="instep" id="steplbl">Inside a disabled fieldset</label>
  </fieldset>

  <fieldset>
    <input type="checkbox" id="live">
  </fieldset>
</body></html>"#;

fn checked(page: &manuk_page::Page, sel: &str) -> bool {
    let n = manuk_css::query_selector_all(page.dom(), page.dom().root(), sel)[0];
    page.dom()
        .element(n)
        .is_some_and(|e| e.attr("checked").is_some())
}

fn node(page: &manuk_page::Page, sel: &str) -> manuk_dom::NodeId {
    manuk_css::query_selector_all(page.dom(), page.dom().root(), sel)[0]
}

#[test]
fn a_disabled_control_neither_activates_nor_reports_itself_as_actionable() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://form.test/", &fonts, 800.0);

    // ── Directly disabled. ──────────────────────────────────────────────────────────────────
    let off = node(&page, "#off");
    page.dispatch_click(off, &fonts, 800.0);
    assert!(
        !checked(&page, "#off"),
        "G_DISABLED_INERT: clicking a disabled checkbox must NOT tick it. An agent that ticks it, \
         reads the state back and sees it ticked will report success on a form the server rejects \
         — a wrong observation is more expensive than a failed action."
    );

    // ── ...and not via its label either. ────────────────────────────────────────────────────
    page.dispatch_click(node(&page, "#offlbl"), &fonts, 800.0);
    assert!(
        !checked(&page, "#off"),
        "routing through the label must not be a way around the control being inert"
    );

    // ── INHERITED from <fieldset disabled>. ─────────────────────────────────────────────────
    page.dispatch_click(node(&page, "#instep"), &fonts, 800.0);
    assert!(
        !checked(&page, "#instep"),
        "G_DISABLED_INERT: a control inside <fieldset disabled> is disabled too. Disabling a whole \
         step of a form with one fieldset is the idiomatic way to do it; checking only the \
         control's own attribute leaves every control in that step live."
    );
    page.dispatch_click(node(&page, "#steplbl"), &fonts, 800.0);
    assert!(!checked(&page, "#instep"), "nor via its label");

    // ── The control in an ENABLED fieldset still works — inertness must not leak. ───────────
    page.dispatch_click(node(&page, "#live"), &fonts, 800.0);
    assert!(
        checked(&page, "#live"),
        "a control in a normal fieldset still activates — the disabled check must not make \
         everything inert, which would pass every assertion above for the wrong reason"
    );

    // ── The a11y tree AGREES, which is what the agent actually reads. ───────────────────────
    let lines = page.a11y_tree().to_observation_lines();
    let disabled_count = lines.iter().filter(|l| l.contains("disabled")).count();
    assert_eq!(
        disabled_count, 2,
        "G_DISABLED_INERT: exactly the two disabled checkboxes report `disabled` — the directly \
         disabled one AND the one inheriting it from the fieldset. If the tree disagreed with the \
         activation path, the agent would be told it can act on something inert. Lines: {lines:#?}"
    );
}
