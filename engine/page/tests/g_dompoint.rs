//! **G_DOMPOINT — the `DOMPoint` geometry class (and its `DOMMatrix` interop).**
//!
//! The point that pairs with `DOMMatrix`: canvas / graphics code maps a coordinate through a transform
//! with `point.matrixTransform(matrix)`, and `matrix.transformPoint(point)` returns one. It was ABSENT,
//! so `new DOMPoint(...)` threw. The teeth are computed coordinates (a wrong transform is caught), plus
//! the interop tooth that `DOMMatrix.transformPoint` now returns a REAL DOMPoint (chainable), not a
//! bare object.
//!
//! Proven RED: delete the `DOMPoint` block and `present` reads `undefined` while `new DOMPoint()`
//! throws.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<script>
var r = [];
function push(s) { r.push(s); }
function near(a, b) { return Math.abs(a - b) < 1e-9; }

try {
  push('present:' + (typeof DOMPoint === 'function'));

  var p = new DOMPoint(3, 4);
  push('fields:' + (p.x === 3 && p.y === 4 && p.z === 0 && p.w === 1)); // w defaults to 1

  // matrixTransform through a translate(10,20) matrix.
  var m = new DOMMatrix().translate(10, 20);
  var q = p.matrixTransform(m);
  push('matrixTransform:' + (near(q.x, 13) && near(q.y, 24)));

  // DOMMatrix.transformPoint returns a REAL DOMPoint — chainable and carrying w.
  var tp = new DOMMatrix().scale(2, 2).transformPoint(new DOMPoint(5, 5));
  push('mtx-returns-point:' + (tp instanceof DOMPoint && near(tp.x, 10) && near(tp.y, 10) && tp.w === 1));

  // fromPoint + toJSON.
  var fp = DOMPoint.fromPoint({ x: 1, y: 2, z: 3, w: 4 });
  var j = fp.toJSON();
  push('fromPoint-toJSON:' + (fp.x === 1 && fp.w === 4 && j.x === 1 && j.z === 3));
} catch (e) {
  push('THREW:' + e);
}
document.getElementById('out').textContent = r.join(' ');
</script></body></html>"##;

#[test]
fn dompoint_and_matrix_interop() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://point.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("DOMPOINT RESULT: {got}");

    for claim in [
        "present:true",
        "fields:true",            // x/y/z and w defaulting to 1
        "matrixTransform:true",   // point through a matrix
        "mtx-returns-point:true", // DOMMatrix.transformPoint returns a real chainable DOMPoint
        "fromPoint-toJSON:true",
    ] {
        assert!(
            got.contains(claim),
            "G_DOMPOINT: expected `{claim}`\n  got: {got}\n\n  \
             `DOMPoint` must carry {{x,y,z,w}} (w defaulting to 1), transform through a matrix \
             correctly, and `DOMMatrix.transformPoint` must return a real DOMPoint instance."
        );
    }
}
