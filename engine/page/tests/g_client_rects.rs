//! **G_CLIENT_RECTS — `element.getClientRects()` returns a DOMRectList, not `undefined`.**
//!
//! A genuinely missing DOM API (CSSOM-View), used broadly for measuring element geometry. A laid-out
//! element yields ONE rect (its bounding box); a `display:none` / unlaid-out element yields an EMPTY
//! list — never a zero rect, which is the distinction from `getBoundingClientRect()`.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<div id="out">-</div>
<div id="b" style="position:absolute; left:10px; top:20px; width:100px; height:50px"></div>
<div id="hidden" style="display:none"></div>
<script>
  var R = [], b = document.getElementById('b'), h = document.getElementById('hidden');
  R.push('fn:' + (typeof b.getClientRects));               // function
  var r = b.getClientRects();
  R.push('len:' + r.length);                               // 1
  R.push('w:' + r[0].width + ',l:' + r[0].left);           // 100,10 — indexed access
  R.push('item:' + (r.item(0).top));                       // 20 — .item()
  R.push('itemOob:' + (r.item(5) === null));               // out-of-range -> null
  R.push('none:' + h.getClientRects().length);             // 0 — display:none yields no rects
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn get_client_rects_returns_a_domrectlist() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://gcr.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("fn:function", "getClientRects must exist as a method, not be undefined"),
        ("len:1", "a laid-out element yields exactly one client rect (its bounding box)"),
        ("w:100,l:10", "…carrying the element's real geometry, indexable as rects[0]"),
        ("item:20", "…and reachable via the DOMRectList .item(i) accessor"),
        ("itemOob:true", "item(i) out of range returns null"),
        (
            "none:0",
            "a display:none element yields an EMPTY list — not a zero rect (the getBoundingClientRect \
             distinction)",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_CLIENT_RECTS: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
