//! **G_CANVAS_CONIC — `createConicGradient` sweeps colour by ANGLE, not radius or a flat block.**
//!
//! Conic gradients used to fall back to a flat last-stop fill (there was no sweep shader). This gate
//! proves the real thing: colour varies with the ANGLE around the centre — the property that
//! distinguishes a conic gradient from both a radial (constant at a given radius) and a flat fill. It
//! reads four points at the same radius and confirms they differ the way a sweep does.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<canvas id="c" width="20" height="20"></canvas>
<div id="out">-</div>
<script>
  var R = [];
  var x = document.getElementById('c').getContext('2d');
  function d(cx, cy) { return x.getImageData(cx, cy, 1, 1).data; }

  // A red→blue sweep from angle 0, centred at (10,10). Offset 0 (red) sits at the +x direction (right);
  // offset 0.5 (purple) is a half-turn away (left), regardless of sweep direction.
  var g = x.createConicGradient(0, 10, 10);
  g.addColorStop(0, '#ff0000');
  g.addColorStop(1, '#0000ff');
  x.fillStyle = g;
  x.fillRect(0, 0, 20, 20);

  var rt = d(18, 10), lf = d(2, 10), tp = d(10, 2), bt = d(10, 18);
  R.push('right_red:' + (rt[0] > 180 && rt[2] < 80));           // angle 0 → the red stop
  R.push('left_mix:' + (lf[2] > 60 && lf[0] > 40));             // half-turn → purple, carries blue
  R.push('sweep_tb:' + (Math.abs(tp[2] - bt[2]) > 40));         // top≠bottom in blue → it sweeps by angle

  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

/// **One test, on purpose** (PROCESS #17).
#[test]
fn conic_gradient_sweeps_colour_by_angle() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://cconic.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "right_red:true",
            "offset 0 (red) must land at the +x direction — the start angle places the first stop, and a \
             flat fallback would paint the whole canvas the LAST stop (blue) here",
        ),
        (
            "left_mix:true",
            "a half-turn from the start the sweep has reached ~offset 0.5 (purple) — it carries blue, \
             proving colour changes with angle",
        ),
        (
            "sweep_tb:true",
            "top and bottom (same radius, opposite angles) differ in blue — a radial would be identical \
             there and a horizontal linear would match top-to-bottom; only a sweep varies this way",
        ),
    ] {
        assert!(got.contains(claim), "G_CANVAS_CONIC: expected `{claim}`\n  got: {got}\n\n  {why}.");
    }
}
