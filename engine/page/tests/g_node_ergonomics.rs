//! **G_NODE_ERGONOMICS — `isConnected`, `toggleAttribute`, `webkitMatchesSelector`.**
//!
//! Three high-usage DOM methods that were simply absent. Every modern framework checks `node.isConnected`
//! before touching an element it may have detached; `toggleAttribute` is the ergonomic add-or-remove;
//! `webkitMatchesSelector` is the legacy alias for `matches` still shipped in a lot of code.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<div id="a"></div>
<script>
  var R = [], a = document.getElementById('a'), d = document.createElement('div');
  R.push('conn:' + a.isConnected);                 // true — in the document
  R.push('detached:' + d.isConnected);             // false — created but not appended
  document.body.appendChild(d);
  R.push('appended:' + d.isConnected);             // true — now connected
  R.push('tglAdd:' + a.toggleAttribute('hidden') + ',' + a.hasAttribute('hidden'));   // true,true
  R.push('tglRm:' + a.toggleAttribute('hidden') + ',' + a.hasAttribute('hidden'));    // false,false
  R.push('force1:' + a.toggleAttribute('x', true) + ',' + a.toggleAttribute('x', true)); // true,true (idempotent)
  R.push('force0:' + a.toggleAttribute('x', false) + ',' + a.hasAttribute('x'));       // false,false
  R.push('wms:' + a.webkitMatchesSelector('#a'));  // true — matches alias
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn is_connected_toggle_attribute_and_webkit_matches() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://ne.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "conn:true",
            "an element in the document tree is isConnected",
        ),
        (
            "detached:false",
            "a createElement'd-but-unappended node is NOT connected",
        ),
        ("appended:true", "…and becomes connected once inserted"),
        (
            "tglAdd:true,true",
            "toggleAttribute adds an absent attribute and returns true",
        ),
        (
            "tglRm:false,false",
            "…and removes a present one, returning false",
        ),
        (
            "force1:true,true",
            "force=true is idempotent — ensures present",
        ),
        ("force0:false,false", "force=false ensures absent"),
        ("wms:true", "webkitMatchesSelector is the matches alias"),
    ] {
        assert!(
            got.contains(claim),
            "G_NODE_ERGONOMICS: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
