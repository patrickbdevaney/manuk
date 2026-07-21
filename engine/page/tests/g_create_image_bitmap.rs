//! **G_CREATE_IMAGE_BITMAP — `createImageBitmap` returns a drawable `ImageBitmap`.**
//!
//! `createImageBitmap(...)` was ABSENT — the call threw `createImageBitmap is not a function`, and every
//! texture uploader (Pixi/Three), image editor and tile renderer that does
//! `createImageBitmap(imgOrCanvas).then(b => ctx.drawImage(b, …))` died. This gate proves the whole
//! round-trip the only way that cannot be faked: build a source canvas, bitmap it, blit the bitmap into
//! a second canvas, and **read the pixels back**. It also proves the crop overload selects the right
//! sub-region. `typeof createImageBitmap === 'function'` proves nothing — a stub would pass that.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<canvas id="src" width="20" height="20"></canvas>
<canvas id="c" width="20" height="20"></canvas>
<div id="out">-</div>
<script>
  var R = [];
  // Source: left half blue, right half red.
  var srcC = document.getElementById('src');
  var s = srcC.getContext('2d');
  s.fillStyle = '#ff0000'; s.fillRect(0, 0, 20, 20);
  s.fillStyle = '#0000ff'; s.fillRect(0, 0, 10, 20);

  var c = document.getElementById('c');
  var x = c.getContext('2d');
  function px(cx, cy) { var d = x.getImageData(cx, cy, 1, 1).data; return d[0]+','+d[1]+','+d[2]+','+d[3]; }

  R.push('type:' + (typeof createImageBitmap));
  R.push('ctor:' + (typeof ImageBitmap));

  createImageBitmap(srcC).then(function (bmp) {
    R.push('promise:ok');
    R.push('size:' + bmp.width + 'x' + bmp.height);
    x.drawImage(bmp, 0, 0);
    R.push('draw:' + px(15, 10));       // right half of the source → red
    R.push('drawblue:' + px(5, 10));    // left half of the source → blue
    // Crop the RED right half (10,0,10,20) and blit it at the origin.
    return createImageBitmap(srcC, 10, 0, 10, 20);
  }).then(function (crop) {
    R.push('cropsize:' + crop.width + 'x' + crop.height);
    x.clearRect(0, 0, 20, 20);
    x.drawImage(crop, 0, 0);
    R.push('cropdraw:' + px(2, 10));    // the cropped region is red, now at the top-left
    document.getElementById('out').textContent = R.join(' ');
  }).catch(function (e) {
    document.getElementById('out').textContent = 'ERR:' + e;
  });
</script></body></html>"##;

/// **One test, on purpose** (PROCESS #17): a second SpiderMonkey context in the same gate binary makes
/// it flake-segfault, and a flaky gate protects nothing.
#[test]
fn create_image_bitmap_round_trips_and_crops() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://cib.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("type:function", "createImageBitmap must exist — the call threw `not a function` before"),
        ("ctor:function", "ImageBitmap must be a named global so `instanceof ImageBitmap` resolves"),
        ("promise:ok", "createImageBitmap must return a Promise that RESOLVES for a canvas/img source"),
        ("size:20x20", "the bitmap must report the source's real decoded size, not a guess"),
        (
            "draw:255,0,0,255",
            "the whole point: `ctx.drawImage(bitmap, …)` must blit the source's pixels — a bitmap you \
             cannot draw is not a bitmap",
        ),
        ("drawblue:0,0,255,255", "the bitmap carries the FULL source, both halves, not just one colour"),
        ("cropsize:10x20", "the crop overload `createImageBitmap(src, sx, sy, sw, sh)` must size to the crop"),
        (
            "cropdraw:255,0,0,255",
            "the crop must select the RIGHT sub-region: bitmapping (10,0,10,20) of a left-blue/right-red \
             source then blitting at 0,0 must show RED, proving the crop offset is applied",
        ),
    ] {
        assert!(got.contains(claim), "G_CREATE_IMAGE_BITMAP: expected `{claim}`\n  got: {got}\n\n  {why}.");
    }
}
