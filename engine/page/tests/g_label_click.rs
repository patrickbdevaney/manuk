//! G_LABEL_CLICK — clicking the words "Remember me" ticks the box.
//!
//! **The failure this gate exists for.** A `<label>` forwards its click to the control it labels.
//! Without that, clicking the label did nothing — and the label is how most checkboxes on the web
//! are *actually* clicked, because the visible target is the text, not the 12px box. For an agent
//! it is worse than for a person: the label is what carries the accessible name, so "click the
//! Remember me checkbox" resolves to the label, clicks it, and nothing happens.
//!
//! Both association forms are gated (`for="id"` and wrapping), plus the two ways to get it wrong:
//! recursing forever on a control nested inside its own label, and forwarding a click the label's
//! own handler cancelled.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html>
<html><body>
  <input type="checkbox" id="remember">
  <label for="remember" id="lbl">Remember me</label>

  <label id="wrap"><input type="checkbox" id="inner"> Keep me signed in</label>

  <input type="checkbox" id="vetoed">
  <label for="vetoed" id="vlbl">Vetoed</label>

  <label for="nothing" id="orphan">Labels nothing</label>
  <script>
    document.getElementById('vlbl').addEventListener('click', function(e) { e.preventDefault(); });
  </script>
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
fn clicking_a_label_activates_the_control_it_labels() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://form.test/", &fonts, 800.0);

    // ── `for="id"` — the common form. ───────────────────────────────────────────────────────
    assert!(!checked(&page, "#remember"));
    let lbl = node(&page, "#lbl");
    page.dispatch_click(lbl, &fonts, 800.0);
    assert!(
        checked(&page, "#remember"),
        "G_LABEL_CLICK: clicking a `for=` label must tick its checkbox. The label is the visible \
         target and the accessible name, so an agent told to click 'Remember me' lands here — if \
         it does nothing, the agent has no way to tell a failed click from a no-op control."
    );
    page.dispatch_click(lbl, &fonts, 800.0);
    assert!(
        !checked(&page, "#remember"),
        "and clicking it again unticks"
    );

    // ── The wrapping form, and the recursion trap. ──────────────────────────────────────────
    let wrap = node(&page, "#wrap");
    page.dispatch_click(wrap, &fonts, 800.0);
    assert!(
        checked(&page, "#inner"),
        "a label WRAPPING its control forwards to the first labelable descendant"
    );
    // Clicking the control directly must activate it once — not forward back to the label.
    let inner = node(&page, "#inner");
    page.dispatch_click(inner, &fonts, 800.0);
    assert!(
        !checked(&page, "#inner"),
        "G_LABEL_CLICK: clicking the control INSIDE its own label toggles exactly once. If the \
         control's click forwarded back through the label this would recurse, or double-toggle \
         and appear to do nothing at all."
    );

    // ── A cancelled label click does not reach the control. ─────────────────────────────────
    let vlbl = node(&page, "#vlbl");
    page.dispatch_click(vlbl, &fonts, 800.0);
    assert!(
        !checked(&page, "#vetoed"),
        "G_LABEL_CLICK: preventDefault() on the LABEL stops the control being activated — the \
         label's own handler gets to veto, exactly as on the control itself"
    );

    // ── A label pointing at nothing activates nothing, and must not panic. ──────────────────
    let orphan = node(&page, "#orphan");
    page.dispatch_click(orphan, &fonts, 800.0);
}
