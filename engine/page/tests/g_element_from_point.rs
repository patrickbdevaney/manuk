//! **G_ELEMENT_FROM_POINT — `document.elementFromPoint(x, y)` returns the topmost element, not undefined.**
//!
//! A genuinely missing DOM API (drag-and-drop, tooltips, custom controls, and every WPT hit-test suite
//! call it). It bridges to the layout-rect snapshot: among laid-out element boxes containing the client
//! point, the deepest wins (children paint over parents). A miss returns `null`; a non-finite coord too.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<div id="out" style="position:absolute; left:400px; top:400px">-</div>
<div id="outer" style="position:absolute; left:0; top:0; width:200px; height:200px">
  <div id="inner" style="position:absolute; left:50px; top:50px; width:50px; height:50px"></div>
</div>
<script>
  var R = [], efp = document.elementFromPoint.bind(document);
  R.push('fn:' + (typeof document.elementFromPoint));               // function
  R.push('inner:' + (efp(75, 75) === document.getElementById('inner')));  // deepest box wins
  R.push('outer:' + (efp(10, 10) === document.getElementById('outer')));  // parent where child absent
  R.push('miss:' + (efp(500, 500) === null));                       // outside everything -> null
  R.push('nan:' + (efp(NaN, 10) === null));                         // non-finite -> null
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn element_from_point_returns_the_deepest_hit_element() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://efp.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "fn:function",
            "elementFromPoint must exist as a method, not be undefined",
        ),
        (
            "inner:true",
            "the DEEPEST element whose box contains the point wins — children paint over parents",
        ),
        (
            "outer:true",
            "…and the parent is hit where no child covers the point",
        ),
        (
            "miss:true",
            "a point outside every box returns null, not the root or undefined",
        ),
        (
            "nan:true",
            "a non-finite coordinate returns null (CSSOM-View)",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_ELEMENT_FROM_POINT: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
