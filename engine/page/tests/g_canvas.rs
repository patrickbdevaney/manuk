//! **G_CANVAS — `<canvas>` 2D actually rasterizes. The pixels are real.**
//!
//! Before tick 66, `getContext('2d')` returned a context whose every drawing operation was a `noop`.
//! That was a *deliberate* trade, and an honest one for its time: the alternative was `getContext` being
//! `undefined`, which made `ctx.fillRect(...)` on the next line a `TypeError` that took the whole bundle
//! down with it. **A blank chart on a working page beats an exception.** It even warned in the console.
//!
//! But it is the worst *shape* of failure that still counts as "working": a page feature-detects canvas,
//! is told **yes**, draws its chart, and nothing appears — with no error. `G_CAPABILITY` measured it
//! precisely: fill the canvas red, read the pixel back, get **`0,0,0,0`**.
//!
//! So this gate asks the only question that matters, and it asks it in the only way that cannot be
//! faked: **draw, then read the pixels back.** `typeof ctx.fillRect === 'function'` proves nothing — the
//! stub passed that test for sixty ticks.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<div id="out">-</div>
<canvas id="c" width="20" height="20"></canvas>
<script>
  var R = [];
  var c = document.getElementById('c');
  var x = c.getContext('2d');

  function px(cx, cy) {
    var d = x.getImageData(cx, cy, 1, 1).data;
    return d[0] + ',' + d[1] + ',' + d[2] + ',' + d[3];
  }

  // ── 1. A filled rect. The whole point.
  x.fillStyle = '#ff0000';
  x.fillRect(0, 0, 10, 10);
  R.push('fill:' + px(5, 5));
  R.push('outside:' + px(15, 15));      // untouched canvas is TRANSPARENT, not white

  // ── 2. Colour parsing that a real chart library actually emits.
  x.fillStyle = 'rgba(0, 128, 0, 1)';
  x.fillRect(10, 0, 10, 10);
  R.push('rgba:' + px(15, 5));

  // ── 3. clearRect erases to transparent — it is not a white fill. A canvas composited over a
  //       background would show the difference instantly.
  x.fillStyle = '#0000ff';
  x.fillRect(0, 10, 10, 10);
  x.clearRect(0, 10, 10, 10);
  R.push('cleared:' + px(5, 15));

  // ── 4. A path, filled. This is how every chart draws anything that is not a bar.
  x.fillStyle = '#ffff00';
  x.beginPath();
  x.moveTo(10, 10);
  x.lineTo(20, 10);
  x.lineTo(20, 20);
  x.closePath();
  x.fill();
  // Sample (18,12): solidly INSIDE the triangle (10,10)-(20,10)-(20,20). (18,18) sits exactly on the
  // hypotenuse and comes back anti-aliased at alpha 96 — which is the rasterizer being RIGHT, and the
  // test being wrong. A gate that samples an edge pixel is measuring its own arithmetic.
  R.push('path:' + px(18, 12));

  // ── 5. The transform stack must actually move what is drawn.
  x.save();
  x.translate(0, 10);
  x.fillStyle = '#ff00ff';
  x.fillRect(0, 0, 4, 4);               // lands at y=10 because of the translate
  x.restore();
  R.push('xform:' + px(2, 12));

  // ── 6. toDataURL must encode what was drawn, not an empty image.
  var u = c.toDataURL();
  R.push('dataurl:' + (u.indexOf('data:image/png;base64,') === 0 && u.length > 100));

  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

/// **One test, on purpose** — see PROCESS #17. Two `#[test]`s in a JS gate binary each stand up a
/// SpiderMonkey context, and the leaked per-process runtime tears down messily when they co-run: the
/// binary passes, then segfaults, then passes. **A flaky gate is worse than a missing one**, because it
/// gets ignored, and an ignored gate protects nothing. I re-learned this by doing it again, here.
#[test]
fn a_canvas_paints_real_pixels_and_they_reach_the_page() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://canvas.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "fill:255,0,0,255",
            "fillRect must PAINT. The stub returned 0,0,0,0 here for sixty ticks while `fillRect` was a \
             function and `getContext` returned a context — a page is told YES and renders nothing",
        ),
        (
            "outside:0,0,0,0",
            "an untouched canvas is TRANSPARENT, not white — a canvas composited over a page background \
             would show the difference immediately",
        ),
        ("rgba:0,128,0,255", "rgba() is what real chart libraries emit; it must parse"),
        (
            "cleared:0,0,0,0",
            "clearRect erases to transparent. If it painted white instead, every canvas over a coloured \
             background would grow a white hole",
        ),
        ("path:255,255,0,255", "a filled path — how every chart draws anything that is not a bar"),
        ("xform:255,0,255,255", "translate() must actually move what is drawn, or every chart is at 0,0"),
        ("dataurl:true", "toDataURL must encode the real pixels, not an empty image"),
    ] {
        assert!(
            got.contains(claim),
            "G_CANVAS: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }

    // ── AND THE PIXELS MUST REACH THE SCREEN, not merely be readable from JavaScript.
    //
    // A canvas that `getImageData` can see but the page cannot is a canvas that does not exist as far as
    // the user is concerned — and it would satisfy every assertion above. So: paint the page, and look.
    let page = manuk_page::Page::load(
        r##"<!doctype html><html><body style="margin:0;background:#fff">
            <canvas id="c" width="40" height="40" style="width:40px;height:40px"></canvas>
            <script>
              var x = document.getElementById('c').getContext('2d');
              x.fillStyle = '#ff0000';
              x.fillRect(0, 0, 40, 40);
            </script></body></html>"##,
        "https://canvas.test/",
        &fonts,
        200.0,
    );

    let canvas = page.paint(&fonts, 60, 60);
    let rgba = canvas.rgba_bytes();
    // Sample the middle of where the <canvas> element sits.
    let (x, y) = (20usize, 20usize);
    let i = (y * 60 + x) * 4;
    let (r, g, b, a) = (rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]);

    assert!(
        r > 200 && g < 60 && b < 60 && a > 200,
        "G_CANVAS: the script filled the canvas red, `getImageData` agrees — and the PAGE was painted \
         `{r},{g},{b},{a}` at ({x},{y}).\n\n  \
         A canvas that only JavaScript can see is not a canvas. The bitmap has to reach the compositor, \
         which here means landing in the same image map an `<img>` lands in."
    );
}
