//! **G_DOMQUAD — the `DOMQuad` geometry class (four points + bounding box).**
//!
//! The shape `element.getBoxQuads()` and transform code produce when a rectangle has been rotated or
//! skewed into a general quadrilateral. It completes the geometry family (DOMMatrix/DOMPoint/DOMRect).
//! It was ABSENT, so `DOMQuad.fromRect(...)` / `new DOMQuad(...)` threw. The teeth are computed —
//! `getBounds()` must return the axis-aligned bounding box, which a stub cannot fake.
//!
//! Proven RED: delete the `DOMQuad` block and `present` reads `undefined` while `new DOMQuad()` throws.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<script>
var r = [];
function push(s) { r.push(s); }

try {
  push('present:' + (typeof DOMQuad === 'function'));

  // fromRect -> four corners.
  var q = DOMQuad.fromRect({ x: 10, y: 20, width: 100, height: 50 });
  push('corners:' + (q.p1.x === 10 && q.p1.y === 20 && q.p3.x === 110 && q.p3.y === 70));
  push('points-are-dompoints:' + (q.p1 instanceof DOMPoint));

  // getBounds of the axis-aligned rect is the rect itself.
  var b = q.getBounds();
  push('bounds-rect:' + (b.x === 10 && b.y === 20 && b.width === 100 && b.height === 50));

  // a SKEWED quad's bounds is the enclosing box (min/max over the four points).
  var sk = new DOMQuad(new DOMPoint(0, 0), new DOMPoint(30, 10),
                       new DOMPoint(40, 60), new DOMPoint(-10, 50));
  var sb = sk.getBounds();
  push('skew-bounds:' + (sb.x === -10 && sb.y === 0 && sb.width === 50 && sb.height === 60));

  // toJSON is the four points.
  var j = q.toJSON();
  push('toJSON:' + (j.p1.x === 10 && j.p3.y === 70));
} catch (e) {
  push('THREW:' + e);
}
document.getElementById('out').textContent = r.join(' ');
</script></body></html>"##;

#[test]
fn domquad_four_points_and_bounds() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://quad.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("DOMQUAD RESULT: {got}");

    for claim in [
        "present:true",
        "corners:true", // fromRect places the four corners
        "points-are-dompoints:true",
        "bounds-rect:true", // getBounds of an axis-aligned quad is the rect
        "skew-bounds:true", // getBounds of a skewed quad is the enclosing box
        "toJSON:true",
    ] {
        assert!(
            got.contains(claim),
            "G_DOMQUAD: expected `{claim}`\n  got: {got}\n\n  \
             `DOMQuad` must hold four DOMPoints, place `fromRect` corners correctly, and `getBounds` \
             must return the axis-aligned enclosing DOMRect (min/max over the four points)."
        );
    }
}
