//! G_INLINE_RETURN_FALSE — `onclick="return false"` cancels the default action.
//!
//! The legacy idiom `<a href=#x onclick="return false">` / `<input type=checkbox onclick="return false">`
//! must veto the click's default (navigation, toggling). The dispatcher used to drop the handler's
//! return value, so the default ran anyway.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html>
<html><body>
  <input type="checkbox" id="vetoed" onclick="return false">
  <input type="checkbox" id="plain" onclick="return true">
  <a id="a1" href="#x" onclick="return false">x</a>
  <a id="a2" href="#y" onclick="return true">y</a>
  <script>
    window.seen = {};
    ['a1','a2'].forEach(function (id) {
      document.getElementById(id).addEventListener('click', function (e) { window.seen[id] = e.defaultPrevented; });
    });
  </script>
</body></html>"##;

fn node(page: &manuk_page::Page, sel: &str) -> manuk_dom::NodeId {
    manuk_css::query_selector_all(page.dom(), page.dom().root(), sel)[0]
}
fn checked(page: &manuk_page::Page, sel: &str) -> bool {
    page.dom()
        .element(node(page, sel))
        .is_some_and(|e| e.attr("checked").is_some())
}

#[test]
fn inline_return_false_cancels_default() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://form.test/", &fonts, 800.0);
    let v = node(&page, "#vetoed");
    page.dispatch_click(v, &fonts, 800.0);
    assert!(
        !checked(&page, "#vetoed"),
        "G_INLINE_RETURN_FALSE: onclick=\"return false\" must leave the checkbox unticked"
    );
    let p = node(&page, "#plain");
    page.dispatch_click(p, &fonts, 800.0);
    assert!(checked(&page, "#plain"), "return true must not veto");
    let a1 = node(&page, "#a1");
    page.dispatch_click(a1, &fonts, 800.0);
    let a2 = node(&page, "#a2");
    page.dispatch_click(a2, &fonts, 800.0);
    let r = {
        page.eval_for_test("document.body.setAttribute('data-seen', JSON.stringify(window.seen))");
        page.dom()
            .element(node(&page, "body"))
            .and_then(|e| e.attr("data-seen").map(String::from))
            .unwrap_or_default()
    };
    assert!(
        r.contains("\"a1\":true") && r.contains("\"a2\":false"),
        "defaultPrevented per anchor: {r}"
    );
}
