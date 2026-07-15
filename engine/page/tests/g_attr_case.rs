//! **G_ATTR_CASE — HTML attribute qualified-name ASCII-lowercasing.**
//!
//! DOM Living Standard §Element: `setAttribute` / `getAttribute` / `removeAttribute` / `hasAttribute` /
//! `toggleAttribute` all ASCII-lowercase the qualified name when the element is in the HTML namespace and
//! its document is an HTML document. We did not — so `el.setAttribute('accessKey', v)` stored the name
//! **verbatim** (`accessKey`), and then:
//!   * `getAttribute('accesskey')` (exact-case match) missed it → `null`,
//!   * the reflected IDL getter (`el.accessKey`, which reads the lowercase *content* attribute) missed it → `""`.
//!
//! That single mismatch failed **every** `setAttribute()` subtest for every mixed-case IDL attribute
//! (`accessKey`, `tabIndex`, `noValidate`, …) across the whole WPT reflection suite — thousands of subtests.
//! The fix lowercases the qualified name for HTML elements (namespace `None`) at both store and lookup;
//! SVG/MathML (namespace `Some`) keep their case so `viewBox` survives.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<script>
  var R = [];
  try {
    var el = document.createElement('form');
    el.setAttribute('accessKey', 'foo');                     // mixed-case set → must store lowercase
    R.push('lowGet:' + el.getAttribute('accesskey'));        // foo  (lowercase content name resolves)
    R.push('mixGet:' + el.getAttribute('accessKey'));        // foo  (query is lowercased too)
    R.push('idl:' + el.accessKey);                           // foo  (reflected getter reads 'accesskey')
    R.push('name:' + el.getAttributeNames().join(','));      // accesskey (stored lowercased)
    R.push('has:' + el.hasAttribute('accessKey'));           // true (query lowercased)
    el.removeAttribute('accessKey');                         // remove via mixed case
    R.push('removed:' + el.hasAttribute('accesskey'));       // false
    var t = document.createElement('input');
    t.setAttribute('tabIndex', '5');                         // another mixed-case IDL attr
    R.push('tab:' + t.tabIndex);                             // 5 (long reflection via lowercased attr)
  } catch (e) { R.push('THREW:' + e); }
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn html_attribute_qualified_names_are_ascii_lowercased() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://ac.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "lowGet:foo",
            "setAttribute('accessKey') must be findable by the lowercase content name",
        ),
        (
            "mixGet:foo",
            "getAttribute('accessKey') lowercases the query for HTML elements",
        ),
        (
            "idl:foo",
            "the reflected IDL getter (el.accessKey) resolves after the name is lowercased",
        ),
        (
            "name:accesskey",
            "the stored attribute name (via getAttributeNames) is ASCII-lowercased at set time",
        ),
        ("has:true", "hasAttribute lowercases its query"),
        (
            "removed:false",
            "removeAttribute lowercases its query and actually removes it",
        ),
        (
            "tab:5",
            "tabIndex (mixed-case long attr) reflects through the lowercased content attribute",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_ATTR_CASE: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
