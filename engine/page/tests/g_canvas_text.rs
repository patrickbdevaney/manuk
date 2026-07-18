//! **G_CANVAS_TEXT — `fillText` puts real glyphs on the canvas, and `measureText` measures them.**
//!
//! **The failure this gate exists for.** `ctx.fillText` was `function(){}` — a hard no-op — and
//! `measureText` returned `text.length * 7`. Both halves are worse than they look:
//!
//!   * The no-op is the *silent* failure shape. A page feature-detects canvas, is told yes, draws
//!     its axis labels, its legend, its cell text, and gets a picture with **every label missing**
//!     and no error anywhere. Nothing throws, nothing warns, and the chart looks like a rendering
//!     bug rather than a missing API.
//!   * `length * 7` is worse than imprecise, because it is not merely a wrong width — it is a width
//!     with **no relationship to the glyphs**. Every layout derived from it is wrong in a way that
//!     compounds: centring, wrapping, column fitting, "does this label collide with the next one",
//!     and hit-testing a terminal cell. A proportional font makes `IIIIIIIIII` and `WWWWWWWWWW`
//!     identical under that formula, which is the single cheapest way to prove it is a fiction.
//!
//! **Why it is measured in PIXELS.** The whole point is that glyphs are rasterized, so the gate
//! reads the canvas back with `getImageData` and counts ink. A stub that recorded the call, or a
//! `measureText` that returned plausible numbers while `fillText` drew nothing, passes any
//! API-shaped assertion and fails this one.
//!
//! The claims, and what each would catch on its own: ink exists at all; ink lands in the drawn
//! **colour** (a glyph blitted with the wrong channel order or an unpremultiplied composite fails
//! here while still producing ink); the canvas outside the text stays **transparent** (a blit that
//! fills the glyph's bounding box rather than its coverage fails here); width tracks the actual
//! glyphs (`W`s wider than `I`s) and scales with font size; `textAlign` and `textBaseline` move the
//! ink, which is what makes a centred label centred rather than starting at the centre.
//!
//! RED: restoring `ctx.fillText = function(){}` drops `ink`, `inkcolor`, `align` and `baseline`
//! together while `measure*` still passes — precisely the half-working state that makes this bug so
//! hard to see from the API surface.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html>
<html><body>
  <canvas id="c" width="240" height="80"></canvas>
  <div id="out">-</div>
  <script>
    var R = {
      a: [],
      push: function (s) { this.a.push(s); var o = document.getElementById('out');
                           if (o) { o.textContent = this.a.join(' '); } },
      join: function (sep) { return this.a.join(sep); }
    };

    try {
      var cv = document.getElementById('c');
      var ctx = cv.getContext('2d');

      // Ink statistics over the whole canvas: how many pixels are non-transparent, the horizontal
      // extent of those pixels, and the colour of the strongest one. Counting is what makes this a
      // pixel gate rather than an API gate.
      var scan = function () {
        var d = ctx.getImageData(0, 0, 240, 80).data;
        var n = 0, minX = 1e9, maxX = -1, minY = 1e9, maxY = -1, best = -1, bc = [0,0,0];
        for (var y = 0; y < 80; y++) {
          for (var x = 0; x < 240; x++) {
            var i = (y * 240 + x) * 4;
            var a = d[i + 3];
            if (a > 8) {
              n++;
              if (x < minX) { minX = x; } if (x > maxX) { maxX = x; }
              if (y < minY) { minY = y; } if (y > maxY) { maxY = y; }
              if (a > best) { best = a; bc = [d[i], d[i+1], d[i+2]]; }
            }
          }
        }
        return { n: n, minX: minX, maxX: maxX, minY: minY, maxY: maxY, c: bc };
      };

      // ── Ink exists.
      ctx.font = '32px sans-serif';
      ctx.fillStyle = '#ff0000';
      ctx.textAlign = 'left';
      ctx.textBaseline = 'alphabetic';
      ctx.fillText('HW', 20, 50);
      var s1 = scan();
      R.push('ink:' + (s1.n > 40));

      // The ink is RED — the fill colour reached the glyph, and the composite did not mangle it.
      R.push('inkcolor:' + (s1.c[0] > 200 && s1.c[1] < 60 && s1.c[2] < 60));

      // Coverage, not a filled bounding box: a 2-glyph 32px string cannot cover the whole 240x80
      // surface, and the ink must sit near the baseline we asked for rather than the canvas top.
      // Both of these re-assert `s1.n > 0`, and that is deliberate: "the ink is not everywhere" and
      // "the ink is in the right place" are BOTH trivially true of a blank canvas, so without it a
      // no-op fillText would satisfy them and the gate would report two false greens next to its
      // real failure.
      R.push('sparse:' + (s1.n > 40 && s1.n < 240 * 80 * 0.25));
      R.push('placed:' + (s1.n > 40 && s1.minX >= 15 && s1.maxY <= 55 && s1.minY > 10));

      // ── measureText measures GLYPHS.
      // Under `length * 7` these two are identical; in any proportional font they are not.
      var wI = ctx.measureText('IIIIIIIIII').width;
      var wW = ctx.measureText('WWWWWWWWWW').width;
      R.push('proportional:' + (wW > wI * 1.3));
      R.push('nonzero:' + (wI > 0 && wW > 0));
      R.push('empty:' + (ctx.measureText('').width === 0));

      // Width scales with the font size — the same string at half the size is about half as wide.
      var w32 = ctx.measureText('Hamburgefonstiv').width;
      ctx.font = '16px sans-serif';
      var w16 = ctx.measureText('Hamburgefonstiv').width;
      R.push('scales:' + (Math.abs(w32 / w16 - 2) < 0.25));

      // Real font metrics, not the fixed 10/3 the stub returned for every font at every size.
      ctx.font = '32px sans-serif';
      var m = ctx.measureText('Hg');
      R.push('metrics:' + (m.fontBoundingBoxAscent > 12 && m.fontBoundingBoxDescent > 0));

      // ── textAlign moves the pen. 'center' must start the ink LEFT of where 'left' started it.
      ctx.clearRect(0, 0, 240, 80);
      ctx.font = '32px sans-serif';
      ctx.textAlign = 'left';
      ctx.fillText('HW', 120, 50);
      var left = scan();
      ctx.clearRect(0, 0, 240, 80);
      ctx.textAlign = 'center';
      ctx.fillText('HW', 120, 50);
      var centre = scan();
      R.push('align:' + (centre.minX < left.minX - 5));

      // ── textBaseline moves it vertically: 'top' at y=10 must sit BELOW an alphabetic baseline at
      // y=10 (which would put almost the whole glyph above the canvas edge).
      ctx.clearRect(0, 0, 240, 80);
      ctx.textAlign = 'left';
      ctx.textBaseline = 'top';
      ctx.fillText('HW', 20, 10);
      var top = scan();
      R.push('baseline:' + (top.n > 40 && top.minY >= 8));

      // ── clearRect really clears the glyphs (they are pixels in the same buffer, not a layer).
      ctx.clearRect(0, 0, 240, 80);
      R.push('cleared:' + (scan().n === 0));
    } catch (e) {
      R.push('THREW:' + e);
    }
  </script>
</body></html>"##;

/// One test in the binary — a second `Page::load` stands up a second SpiderMonkey context in the
/// same process and does not survive teardown (see `g_mse`, `g_overflow_cssom`, `g_globals`).
#[test]
fn fill_text_rasterizes_glyphs_and_measure_text_measures_them() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://chart.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("ink:true", "fillText must put actual pixels on the canvas — a no-op leaves every chart label, legend and cell blank with nothing thrown to notice"),
        ("inkcolor:true", "the glyph must be blitted in the fill colour, with channels in the right order and the composite premultiplied correctly"),
        ("sparse:true", "coverage must be the glyph's, not its bounding box — a box-fill blit produces ink and still looks nothing like text"),
        ("placed:true", "the ink must land at the requested pen origin and baseline, not at the canvas origin"),
        ("proportional:true", "ten Ws must be wider than ten Is; under the old `length * 7` they were identical, which is what makes that formula a fiction rather than an estimate"),
        ("nonzero:true", "a measured string has a real width — zero would poison every coordinate a page derives from it"),
        ("empty:true", "the empty string measures zero"),
        ("scales:true", "the same text at half the font size is about half as wide — a width that ignores size is not a measurement"),
        ("metrics:true", "ascent/descent must come from the font, not the stub's fixed 10/3 for every font at every size"),
        ("align:true", "textAlign='center' must move the pen left by half the text width; a page centring a label depends on exactly this"),
        ("baseline:true", "textBaseline='top' must place the glyph below the given y, not above it"),
        ("cleared:true", "glyphs are pixels in the canvas buffer — clearRect must erase them like any other drawing"),
    ] {
        assert!(
            got.contains(claim),
            "G_CANVAS_TEXT: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
