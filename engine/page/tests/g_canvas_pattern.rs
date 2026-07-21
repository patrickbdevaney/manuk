//! **G_CANVAS_PATTERN — `createPattern` tiles a source image across the fill.**
//!
//! `ctx.createPattern` returned `null` for its whole life, so `fillStyle = ctx.createPattern(img,
//! 'repeat')` set the fill to `null` (→ black) and every hatch fill, textured background and repeating
//! sprite painted a black block. This gate proves the pattern actually TILES the only way that cannot be
//! faked: fill a wide rect with a 4px two-colour source and read pixels a full tile apart — a repeating
//! red/blue must recur, where the old stub paints solid black.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<canvas id="src" width="4" height="4"></canvas>
<canvas id="c" width="16" height="8"></canvas>
<div id="out">-</div>
<script>
  var R = [];
  // Source tile: left 2px red, right 2px blue.
  var s = document.getElementById('src').getContext('2d');
  s.fillStyle = '#ff0000'; s.fillRect(0, 0, 2, 4);
  s.fillStyle = '#0000ff'; s.fillRect(2, 0, 2, 4);

  var x = document.getElementById('c').getContext('2d');
  function px(cx, cy) { var d = x.getImageData(cx, cy, 1, 1).data; return d[0]+','+d[1]+','+d[2]+','+d[3]; }

  var pat = x.createPattern(document.getElementById('src'), 'repeat');
  R.push('type:' + (pat && typeof pat === 'object' ? 'ok' : String(pat)));
  x.fillStyle = pat;
  x.fillRect(0, 0, 16, 8);
  R.push('t0:' + px(1, 2));    // first tile, red half
  R.push('t1:' + px(3, 2));    // first tile, blue half
  R.push('rep:' + px(5, 2));   // SECOND tile, red half — proves it repeats past one image width
  R.push('rep2:' + px(7, 2));  // second tile, blue half

  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

/// **One test, on purpose** (PROCESS #17).
#[test]
fn create_pattern_tiles_a_source_across_the_fill() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://cpat.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("type:ok", "createPattern must return a CanvasPattern object, not null (the old stub returned null)"),
        (
            "t0:255,0,0,255",
            "the pattern fill must show the source's RED half — a null pattern fills black, so this is \
             exactly the assertion the old stub fails",
        ),
        ("t1:0,0,255,255", "the source's BLUE half tiles too — the whole image, not one colour"),
        (
            "rep:255,0,0,255",
            "a full source-width (4px) further along, RED recurs — proving the source actually REPEATS, \
             not stretched or drawn once",
        ),
        ("rep2:0,0,255,255", "and blue recurs in the second tile — a stable repeating pattern"),
    ] {
        assert!(got.contains(claim), "G_CANVAS_PATTERN: expected `{claim}`\n  got: {got}\n\n  {why}.");
    }
}
