//! G_FOCUS_EVENTS — giving a field focus fires `focusin` + `focus` at it (and losing it, `focusout`).
//!
//! React's `onFocus`/`onBlur` listen for `focusin`/`focusout` at the root. Nothing fired them, so a MUI
//! Autocomplete never marked itself focused and reset its input to the selected value after every
//! keystroke: udriver's pickup box kept offering the same 4 addresses (W4.P0.b). `focus` itself must not
//! bubble; `focusin` must.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html>
<html><body>
  <div id="w"><input id="i" type="text"></div>
  <script>
    window.log = [];
    var i = document.getElementById('i'), w = document.getElementById('w');
    ['focusin', 'focus', 'focusout', 'blur'].forEach(function (t) {
      i.addEventListener(t, function () { window.log.push('i:' + t); });
      w.addEventListener(t, function () { window.log.push('w:' + t); });
    });
  </script>
</body></html>"##;

fn node(page: &manuk_page::Page, sel: &str) -> manuk_dom::NodeId {
    manuk_css::query_selector_all(page.dom(), page.dom().root(), sel)[0]
}

#[test]
fn focusing_a_field_tells_the_page() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://focus.test/", &fonts, 800.0);
    let i = node(&page, "#i");
    assert!(page.set_focus(Some(i), false, &fonts, 800.0));
    page.dispatch_blur(i, false, &fonts, 800.0);
    page.eval_for_test("document.body.setAttribute('data-log', window.log.join(','))");
    let log = page
        .dom()
        .element(node(&page, "body"))
        .and_then(|e| e.attr("data-log").map(String::from))
        .unwrap_or_default();
    assert!(
        log.contains("i:focus,"),
        "G_FOCUS_EVENTS: no `focus` at the field: {log}"
    );
    assert!(log.contains("i:focusin"), "no focusin at the field: {log}");
    assert!(
        log.contains("w:focusin"),
        "focusin must bubble to the wrapper: {log}"
    );
    assert!(
        !log.contains("w:focus,") && !log.ends_with("w:focus"),
        "focus must not bubble: {log}"
    );
    assert!(
        log.contains("w:focusout"),
        "focusout must bubble on blur: {log}"
    );
}
