//! **G_PATH2D — `Path2D` is a real, reusable path and `ctx.fill(path)` rasterizes it.**
//!
//! `Path2D` was ABSENT: `new Path2D(...)` threw `Path2D is not defined`, and every icon system
//! (Lucide/Feather/Material), Chart.js shape generator and "draw this glyph on a canvas" helper that
//! hands a pre-built path to `ctx.fill(path)` died in its constructor. The most-used form is
//! `new Path2D("M… L… A… Z")` — an SVG path-data string — so this gate builds one three ways and
//! proves it paints, the only question that cannot be faked: **draw through the Path2D, then read the
//! pixels back.** `typeof Path2D === 'function'` proves nothing — a stub would pass that.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<div id="out">-</div>
<canvas id="c" width="30" height="30"></canvas>
<script>
  var R = [];
  var c = document.getElementById('c');
  var x = c.getContext('2d');
  function px(cx, cy) {
    var d = x.getImageData(cx, cy, 1, 1).data;
    return d[0] + ',' + d[1] + ',' + d[2] + ',' + d[3];
  }

  R.push('type:' + (typeof Path2D));

  // ── 1. An imperatively-built Path2D, filled via ctx.fill(path). This is a solid triangle covering
  //       the top-left; sample well inside it.
  var p = new Path2D();
  p.moveTo(0, 0);
  p.lineTo(20, 0);
  p.lineTo(0, 20);
  p.closePath();
  x.fillStyle = '#ff0000';
  x.fill(p);
  R.push('imperative:' + px(4, 4));       // deep inside the triangle
  R.push('untouched:' + px(28, 28));      // far corner never covered → transparent

  // ── 2. The form that matters most: a Path2D from an SVG path-data string. A filled 12x12 box at
  //       (14,14) written with relative + H/V commands and closed with Z.
  var s = new Path2D('M14 14 h12 v12 h-12 Z');
  x.fillStyle = '#0000ff';
  x.fill(s);
  R.push('svg:' + px(20, 20));

  // ── 3. Copy-construct and confirm the copy carries the same geometry (fill it green over the red).
  var q = new Path2D(p);
  x.fillStyle = '#00ff00';
  x.fill(q);
  R.push('copy:' + px(4, 4));

  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

/// **One test, on purpose** (PROCESS #17): two SpiderMonkey contexts in one gate binary tear down
/// messily and the binary flake-segfaults. A flaky gate is worse than a missing one.
#[test]
fn path2d_is_real_and_ctx_fill_rasterizes_it() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://path2d.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("type:function", "Path2D must be a constructor — `new Path2D(...)` threw `not defined` before"),
        (
            "imperative:255,0,0,255",
            "an imperatively-built Path2D handed to ctx.fill(path) must PAINT — the whole point of a \
             reusable path object is that fill(path) rasterizes ITS commands, not the current path",
        ),
        (
            "untouched:0,0,0,0",
            "a Path2D covers only its own area — outside the triangle the canvas stays transparent",
        ),
        (
            "svg:0,0,255,255",
            "`new Path2D(\"M14 14 h12 v12 h-12 Z\")` — the SVG path-data form icon systems use — must \
             parse (relative h/v + Z) and fill the box it describes",
        ),
        (
            "copy:0,255,0,255",
            "`new Path2D(otherPath)` must copy the geometry: filling the copy green must repaint the \
             same triangle the original drew",
        ),
    ] {
        assert!(got.contains(claim), "G_PATH2D: expected `{claim}`\n  got: {got}\n\n  {why}.");
    }
}
