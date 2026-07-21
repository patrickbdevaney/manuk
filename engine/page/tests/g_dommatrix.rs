//! **G_DOMMATRIX — the `DOMMatrix` 2D affine transform math class.**
//!
//! `canvas.getContext('2d').getTransform()` returns one, charting/graphics libraries build transforms
//! with it, CSS Typed OM hands it back. It was ABSENT, so `new DOMMatrix(...)` threw `DOMMatrix is not
//! defined`. This is real, honest 2D matrix math (not a stub): the teeth are computed results, so a
//! wrong multiply/inverse/rotate is caught, not just the presence of the methods.
//!
//! Proven RED: delete the `DOMMatrix` block and `present` reads `undefined` while `new DOMMatrix()`
//! throws.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<script>
var r = [];
function push(s) { r.push(s); }
function near(a, b) { return Math.abs(a - b) < 1e-9; }

try {
  push('present:' + (typeof DOMMatrix === 'function'));

  var id = new DOMMatrix();
  push('identity:' + (id.isIdentity === true && id.a === 1 && id.d === 1 && id.e === 0));

  // translate then read a transformed point.
  var t = new DOMMatrix().translate(10, 20);
  var tp = t.transformPoint({ x: 0, y: 0 });
  push('translate:' + (near(tp.x, 10) && near(tp.y, 20)));

  // scale.
  var s = new DOMMatrix().scale(2, 3).transformPoint({ x: 5, y: 5 });
  push('scale:' + (near(s.x, 10) && near(s.y, 15)));

  // rotate 90deg maps (1,0) -> (0,1).
  var rp = new DOMMatrix().rotate(90).transformPoint({ x: 1, y: 0 });
  push('rotate:' + (near(rp.x, 0) && near(rp.y, 1)));

  // compose: translate(10,0) then scale(2) — point (1,1) -> (12, 2).
  var cp = new DOMMatrix().translate(10, 0).scale(2).transformPoint({ x: 1, y: 1 });
  push('compose:' + (near(cp.x, 12) && near(cp.y, 2)));

  // inverse of scale(2,4) maps (2,4) back to (1,1).
  var ip = new DOMMatrix().scale(2, 4).inverse().transformPoint({ x: 2, y: 4 });
  push('inverse:' + (near(ip.x, 1) && near(ip.y, 1)));

  // string + array construction and serialisation.
  var fromStr = new DOMMatrix('matrix(1, 0, 0, 1, 40, 50)');
  push('fromString:' + (fromStr.e === 40 && fromStr.f === 50));
  push('toString:' + (new DOMMatrix([1, 2, 3, 4, 5, 6]).toString() === 'matrix(1, 2, 3, 4, 5, 6)'));
} catch (e) {
  push('THREW:' + e);
}
document.getElementById('out').textContent = r.join(' ');
</script></body></html>"##;

#[test]
fn dommatrix_does_real_2d_affine_math() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://matrix.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("DOMMATRIX RESULT: {got}");

    for claim in [
        "present:true",
        "identity:true",
        "translate:true",
        "scale:true",
        "rotate:true",
        "compose:true", // translate then scale composes correctly
        "inverse:true", // real matrix inversion
        "fromString:true",
        "toString:true",
    ] {
        assert!(
            got.contains(claim),
            "G_DOMMATRIX: expected `{claim}`\n  got: {got}\n\n  \
             `DOMMatrix` must do real 2D affine math: translate/scale/rotate/multiply/inverse and \
             transformPoint must produce correct coordinates, and construct from an array or a \
             `matrix(...)` string. A wrong-math stub fails the computed claims."
        );
    }
}
