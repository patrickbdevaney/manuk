//! **G_COMPARE_POSITION — Node type/position CONSTANTS + `compareDocumentPosition`.**
//!
//! `n.nodeType === Node.ELEMENT_NODE` and `a.compareDocumentPosition(b) & Node.DOCUMENT_POSITION_FOLLOWING`
//! are everywhere in real code (focus managers, selection, sort-by-DOM-order). The constants were absent
//! (`=== undefined` → silently false) and the method threw.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<div id="p"><span id="a"></span><span id="b"></span></div>
<script>
  var R = [], p = document.getElementById('p'), a = document.getElementById('a'), b = document.getElementById('b');
  R.push('elc:' + Node.ELEMENT_NODE);                                          // 1
  R.push('txc:' + a.TEXT_NODE);                                                // 3 (inherited)
  R.push('posc:' + Node.DOCUMENT_POSITION_FOLLOWING);                          // 4
  R.push('self:' + a.compareDocumentPosition(a));                              // 0
  R.push('follow:' + !!(a.compareDocumentPosition(b) & Node.DOCUMENT_POSITION_FOLLOWING));    // true
  R.push('precede:' + !!(b.compareDocumentPosition(a) & Node.DOCUMENT_POSITION_PRECEDING));   // true
  R.push('contains:' + !!(p.compareDocumentPosition(a) & Node.DOCUMENT_POSITION_CONTAINED_BY)); // true
  R.push('contained:' + !!(a.compareDocumentPosition(p) & Node.DOCUMENT_POSITION_CONTAINS));    // true
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn node_constants_and_compare_document_position() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://cdp.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "elc:1",
            "Node.ELEMENT_NODE is the constant 1, not undefined",
        ),
        (
            "txc:3",
            "instances inherit the constants (a.TEXT_NODE === 3)",
        ),
        ("posc:4", "DOCUMENT_POSITION_FOLLOWING is 4"),
        ("self:0", "compareDocumentPosition against self is 0"),
        (
            "follow:true",
            "an earlier node reports its later sibling as FOLLOWING",
        ),
        (
            "precede:true",
            "…and the later node reports the earlier one as PRECEDING",
        ),
        (
            "contains:true",
            "a parent reports a descendant as CONTAINED_BY",
        ),
        (
            "contained:true",
            "…and the descendant reports the ancestor as CONTAINS",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_COMPARE_POSITION: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
