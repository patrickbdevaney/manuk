//! G_SUBMIT_CLICK — clicking "Sign in" actually submits the form.
//!
//! **The failure this gate exists for.** `element.click()` on a submit button fired a click event
//! and stopped. Nothing was queued, so the form never submitted. "Click Sign in" is the single most
//! common thing an agent is asked to do, and it silently did nothing — the agent clicks, sees no
//! navigation, and has no way to distinguish "the button is broken" from "we never submitted".
//!
//! The submit is queued as a **requested** submit rather than a direct one, which is the load-bearing
//! distinction: `requested` fires the `submit` event first, so the page's validation handler runs and
//! can cancel. A direct submit would skip every client-side validator on the web.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html>
<html><body>
  <form id="login" action="/session">
    <input type="text" name="user">
    <button id="go">Sign in</button>            <!-- no type: defaults to submit -->
    <button id="plain" type="button">Toggle</button>
    <button id="dead" type="submit" disabled>Disabled</button>
  </form>
  <form id="other" action="/other"></form>
  <button id="remote" type="submit" form="other">Remote</button>
</body></html>"#;

fn node(page: &manuk_page::Page, sel: &str) -> manuk_dom::NodeId {
    manuk_css::query_selector_all(page.dom(), page.dom().root(), sel)[0]
}

#[test]
fn clicking_a_submit_button_queues_its_form_for_submission() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://app.test/", &fonts, 800.0);
    let login = node(&page, "#login");
    let other = node(&page, "#other");

    // Nothing is queued until something is clicked.
    assert_eq!(page.take_form_submits(), (vec![], vec![]));

    // ── A bare <button> defaults to type=submit. ────────────────────────────────────────────
    page.dispatch_click(node(&page, "#go"), &fonts, 800.0);
    let (direct, requested) = page.take_form_submits();
    assert_eq!(
        requested,
        vec![(login, Some(node(&page, "#go")))],
        "G_SUBMIT_CLICK: clicking a submit button must submit its form. A bare <button> inside a \
         form defaults to type=submit — that default is the classic source of 'why did my page \
         reload', and skipping it means 'click Sign in' does nothing at all."
    );
    assert!(
        direct.is_empty(),
        "G_SUBMIT_CLICK: it must be a REQUESTED submit, not a direct one. `requested` fires the \
         `submit` event first so the page's validation handler runs and can cancel; `direct` would \
         skip every client-side validator on the web."
    );

    // The queue is a drain — the host must not submit the same form twice.
    assert_eq!(page.take_form_submits(), (vec![], vec![]));

    // ── type=button does NOT submit. ────────────────────────────────────────────────────────
    page.dispatch_click(node(&page, "#plain"), &fonts, 800.0);
    assert_eq!(
        page.take_form_submits(),
        (vec![], vec![]),
        "a <button type=button> is a plain button — submitting on it would reload the page under \
         every toggle and menu built with one"
    );

    // ── A disabled submit button is inert here too. ─────────────────────────────────────────
    page.dispatch_click(node(&page, "#dead"), &fonts, 800.0);
    assert_eq!(
        page.take_form_submits(),
        (vec![], vec![]),
        "a disabled submit button submits nothing"
    );

    // ── form="id" associates a button OUTSIDE the form it submits. ──────────────────────────
    page.dispatch_click(node(&page, "#remote"), &fonts, 800.0);
    let (_, requested) = page.take_form_submits();
    assert_eq!(
        requested,
        vec![(other, Some(node(&page, "#remote")))],
        "form=\"id\" submits the named form, not the nearest ancestor — a button can live outside \
         the form it drives"
    );
}
