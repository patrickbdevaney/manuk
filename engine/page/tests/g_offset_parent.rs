//! **G_OFFSET_PARENT — `offsetLeft`/`offsetTop` are offsetParent-relative, and `offsetParent` exists.**
//!
//! CSSOM-View. `offsetLeft`/`offsetTop` are measured from the **offsetParent's padding edge**, not the
//! viewport — the property they exist to answer is "where is this box inside its positioned container".
//! Returning the absolute page coordinate (what this engine did before) is wrong for every element whose
//! offsetParent is not at the page origin, which is the norm: `check-layout-th.js` compares `offsetLeft`
//! against a container-relative `data-offset-x`, so the whole flex/grid/sizing layout suite failed on it,
//! and every popup/tooltip/drag library that positions at `el.offsetLeft` landed in the wrong place.
//!
//! The container here is `position:relative` with a `5px` border and offset by `left:30px; top:40px`. The
//! inner `position:absolute` item at `left:10px; top:20px` therefore sits `(10,20)` from the container's
//! **padding** edge, wherever the container itself ended up on the page. `offsetLeft == 10` proves both
//! properties at once: the value is offsetParent-relative (the container's `left:30` is subtracted) AND
//! the container's border is subtracted (the padding edge, not the border-box edge, is the origin).

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<div id="out">-</div>
<div id="rel" style="position:relative; left:30px; top:40px; width:300px; height:200px; border:5px solid black">
  <div id="item" style="position:absolute; left:10px; top:20px; width:50px; height:50px"></div>
  <div id="stat" style="width:40px; height:40px"></div>
</div>
<script>
  var R = [];
  var item = document.getElementById('item');
  var rel  = document.getElementById('rel');
  var stat = document.getElementById('stat');
  R.push('op:'   + (item.offsetParent === rel));                 // positioned ancestor is the offsetParent
  R.push('ol:'   + item.offsetLeft);                             // 10 — relative to rel's padding edge
  R.push('ot:'   + item.offsetTop);                              // 20
  R.push('sop:'  + (stat.offsetParent === rel));                 // a static child still resolves to rel
  R.push('body:' + (document.body.offsetParent === null));       // the body has no offsetParent
  R.push('html:' + (document.documentElement.offsetParent === null)); // nor the root element
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn offset_left_top_are_offset_parent_relative() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://gop.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("op:true", "offsetParent is the nearest positioned ancestor"),
        (
            "ol:10",
            "offsetLeft is measured from the offsetParent's padding edge (container left:30 AND its 5px \
             border subtracted), not the viewport",
        ),
        ("ot:20", "offsetTop likewise measured from the padding edge"),
        (
            "sop:true",
            "a statically-positioned child still resolves its offsetParent to the positioned ancestor",
        ),
        ("body:true", "the body element's offsetParent is null (spec step 1)"),
        ("html:true", "the root element's offsetParent is null (spec step 1)"),
    ] {
        assert!(
            got.contains(claim),
            "G_OFFSET_PARENT: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
