//! G_CLICK_ACTIVATION — a click on a checkbox actually ticks it.
//!
//! **The failure this gate exists for.** `dispatch_click` fired the *event* and stopped there: it
//! ran no **activation behaviour**, so clicking a checkbox left it unchecked, clicking a radio
//! selected nothing, and no `input`/`change` ever fired. Tick 199 gave the agent the ability to
//! *read back* control state and flagged this as the thing that made the read-back only half
//! useful — an agent could see a checkbox was unchecked, click it, and see it still unchecked.
//!
//! **Ordering is the subtle half.** The toggle happens BEFORE the click event is dispatched, which
//! is why a real handler reading `this.checked` sees the NEW state. Toggling afterwards would hand
//! every handler on the web the stale value — and it would still pass a naive "is it checked at the
//! end" test, which is why this gate asserts what the handler SAW.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html>
<html><body>
  <input type="checkbox" id="cb">
  <input type="checkbox" id="veto">
  <input type="radio" name="plan" id="r1" checked>
  <input type="radio" name="plan" id="r2">
  <input type="radio" name="other" id="r3" checked>
  <div id="log"></div>
  <script>
    var $ = function(id) { return document.getElementById(id); };
    var log = [];
    // What the handler SEES is the whole ordering claim.
    $('cb').addEventListener('click', function() { log.push('click:' + $('cb').checked); });
    $('cb').addEventListener('input', function() { log.push('input:' + $('cb').checked); });
    $('cb').addEventListener('change', function() { log.push('change:' + $('cb').checked); });
    // A page that validates before allowing a toggle cancels the click.
    $('veto').addEventListener('click', function(e) { e.preventDefault(); });
    window.__dump = function() { $('log').textContent = log.join(' '); };
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
fn clicking_a_checkbox_ticks_it_and_a_radio_selects_its_group() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://form.test/", &fonts, 800.0);

    // ── A checkbox toggles on, then off. ────────────────────────────────────────────────────
    assert!(!checked(&page, "#cb"), "starts unchecked");
    let cb = node(&page, "#cb");
    page.dispatch_click(cb, &fonts, 800.0);
    assert!(
        checked(&page, "#cb"),
        "G_CLICK_ACTIVATION: clicking a checkbox must TICK it. Firing the click event without \
         running activation behaviour leaves the box exactly as it was — so an agent (or a user) \
         clicks, reads the state back, and finds nothing happened."
    );
    page.dispatch_click(cb, &fonts, 800.0);
    assert!(!checked(&page, "#cb"), "clicking again unticks it");

    // ── The ORDERING claim: what did the handler see? ───────────────────────────────────────
    let dump = node(&page, "#log");
    let _ = dump;
    page.eval_for_test("__dump()");
    let log = page.dom().text_content(node(&page, "#log"));
    assert_eq!(
        log, "click:true input:true change:true click:false input:false change:false",
        "G_CLICK_ACTIVATION: the click handler must observe the NEW checked state — the toggle \
         happens BEFORE dispatch. `input` then `change` follow, in that order, both committed. \
         Toggling after the event would still end in the right final state while handing every \
         handler on the web a stale value, which is why this asserts what was SEEN."
    );

    // ── preventDefault() undoes the toggle. ─────────────────────────────────────────────────
    let veto = node(&page, "#veto");
    page.dispatch_click(veto, &fonts, 800.0);
    assert!(
        !checked(&page, "#veto"),
        "G_CLICK_ACTIVATION: preventDefault() on the click means the box does NOT tick. A page \
         that validates before allowing a toggle depends on exactly this."
    );

    // ── A radio is a GROUP, not a toggle. ───────────────────────────────────────────────────
    assert!(checked(&page, "#r1") && !checked(&page, "#r2"));
    let r2 = node(&page, "#r2");
    page.dispatch_click(r2, &fonts, 800.0);
    assert!(checked(&page, "#r2"), "the clicked radio becomes selected");
    assert!(
        !checked(&page, "#r1"),
        "G_CLICK_ACTIVATION: selecting a radio must DESELECT its group peer. Two checked radios \
         in one group means the form submits the wrong value."
    );
    assert!(
        checked(&page, "#r3"),
        "a radio in a DIFFERENT name group is untouched — grouping is by name, which is how the \
         form serialises"
    );

    // Clicking an already-selected radio leaves it selected (it is not a toggle).
    page.dispatch_click(r2, &fonts, 800.0);
    assert!(checked(&page, "#r2"), "a radio never unchecks itself");
}
