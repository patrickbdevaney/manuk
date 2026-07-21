//! **G_CANVAS_GRADIENT — canvas linear/radial gradients rasterize as REAL gradients.**
//!
//! Before this tick a `CanvasGradient` was an honest flat approximation: `fillStyle = grad` painted the
//! whole shape in the gradient's LAST stop's colour. That is the worst shape of "working" — a chart's
//! area fill, a button's gloss, a progress bar all render as a flat block, no error. This gate proves
//! the gradient actually varies across the shape, the only way that cannot be faked: **fill, then read
//! pixels at both ends and confirm they differ in the right direction.** A flat fill fails `lin_red`
//! immediately (the whole rect would be the last stop, blue).

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<canvas id="c" width="40" height="20"></canvas>
<div id="out">-</div>
<script>
  var R = [];
  var x = document.getElementById('c').getContext('2d');
  function d(cx, cy) { return x.getImageData(cx, cy, 1, 1).data; }

  // ── 1. LINEAR gradient via fillRect: red at the left, blue at the right.
  var lg = x.createLinearGradient(0, 0, 40, 0);
  lg.addColorStop(0, '#ff0000');
  lg.addColorStop(1, '#0000ff');
  x.fillStyle = lg;
  x.fillRect(0, 0, 40, 20);
  var l = d(1, 10), r = d(38, 10);
  R.push('lin_red:' + (l[0] > 180 && l[2] < 80));          // left end is RED, not the flat last stop
  R.push('lin_blue:' + (r[2] > 180 && r[0] < 80));         // right end is BLUE
  R.push('lin_varies:' + (l[0] > r[0] && r[2] > l[2]));    // red falls, blue rises across → a real ramp

  // ── 2. LINEAR gradient via a PATH fill() (the other native entry point).
  x.clearRect(0, 0, 40, 20);
  var lg2 = x.createLinearGradient(0, 0, 40, 0);
  lg2.addColorStop(0, '#ff0000');
  lg2.addColorStop(1, '#0000ff');
  x.fillStyle = lg2;
  x.beginPath(); x.rect(0, 0, 40, 20); x.fill();
  var pl = d(1, 10), pr = d(38, 10);
  R.push('path_grad:' + (pl[0] > 180 && pr[2] > 180));

  // ── 3. RADIAL gradient: green at the centre, red at the rim (padded outside the radius).
  x.clearRect(0, 0, 40, 20);
  var rg = x.createRadialGradient(20, 10, 0, 20, 10, 12);
  rg.addColorStop(0, '#00ff00');
  rg.addColorStop(1, '#ff0000');
  x.fillStyle = rg;
  x.fillRect(0, 0, 40, 20);
  var cen = d(20, 10), edge = d(1, 10);
  R.push('rad_center_green:' + (cen[1] > 180 && cen[0] < 80));
  R.push('rad_edge_red:' + (edge[0] > 180 && edge[1] < 80));

  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

/// **One test, on purpose** (PROCESS #17).
#[test]
fn canvas_gradients_are_real_ramps_not_flat_fills() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://cgrad.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "lin_red:true",
            "the LEFT end of a red→blue linear gradient must be RED. A flat approximation paints the \
             whole rect in the last stop (blue), so this is exactly the assertion the old stub fails",
        ),
        ("lin_blue:true", "the right end must be BLUE — the gradient's far stop"),
        (
            "lin_varies:true",
            "red must fall and blue must rise from left to right — a real ramp, not two flat halves",
        ),
        ("path_grad:true", "a gradient must also fill a PATH (ctx.fill), not only fillRect"),
        (
            "rad_center_green:true",
            "a radial gradient's centre is its first stop (green) — proving the shader is radial, not \
             linear, and centred where asked",
        ),
        (
            "rad_edge_red:true",
            "outside the radius the pad spread clamps to the last stop (red) — the corners of the rect",
        ),
    ] {
        assert!(got.contains(claim), "G_CANVAS_GRADIENT: expected `{claim}`\n  got: {got}\n\n  {why}.");
    }
}
