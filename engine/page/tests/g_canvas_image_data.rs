//! **G_CANVAS_IMAGE_DATA — `ImageData` + `putImageData` must move real pixels, not no-op.**
//!
//! Direct pixel access is the whole of the canvas's image-processing surface: filters (blur, grayscale,
//! threshold), histograms, barcode/QR readers, image editors, chart libraries that composite layers, and
//! the software fallback path of WebGL demos all build a `Uint8ClampedArray`, construct an `ImageData`,
//! and blit it with `putImageData`. Two gaps closed here:
//!
//!   * **`new ImageData(...)` did not exist** — the constructor global was absent, so `new ImageData(w,h)`
//!     and `new ImageData(dataArray, w[, h])` threw `ImageData is not defined` and the pixel pipeline died
//!     on its first line.
//!   * **`putImageData` was an honest no-op** — `getImageData` read real pixels (via the tiny-skia
//!     surface) but nothing could WRITE them back, so every round-trip silently discarded the edit. A
//!     grayscale filter ran, wrote nothing, and left the image untouched with no error.
//!
//! The claims are checked on OBSERVABLE pixels read back through `getImageData` (which the pre-existing
//! `fillRect` path already proved reads the real surface), each a way the old no-op / missing ctor go RED:
//!
//!   * **`new ImageData(w,h)`** is a real object with a `w*h*4` `Uint8ClampedArray`.
//!   * **`new ImageData(array, w)`** infers height and adopts the bytes.
//!   * **`putImageData` then `getImageData`** round-trips exact RGBA at the written offset.
//!   * **The dirty-rectangle overload** writes only its sub-region.
//!   * **`putImageData` does not disturb neighbouring pixels.**

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body><canvas id="c" width="8" height="8"></canvas><div id="out">-</div><script>
    var r = [];
    var cx = document.getElementById('c').getContext('2d');
    var px = function (x, y) { var g = cx.getImageData(x, y, 1, 1).data; return g[0] + ',' + g[1] + ',' + g[2] + ',' + g[3]; };

    // new ImageData(w, h): real zeroed buffer of the right length + type.
    var a = new ImageData(2, 3);
    r.push('ctor:' + a.width + 'x' + a.height + '/' + a.data.length + '/' + (a.data instanceof Uint8ClampedArray));

    // new ImageData(array, width): height inferred from length.
    var arr = new Uint8ClampedArray(2 * 2 * 4);
    var b = new ImageData(arr, 2);
    r.push('fromarr:' + b.width + 'x' + b.height + '/' + (b.data === arr));

    // putImageData writes pixels that getImageData reads back — exactly.
    var d = new ImageData(1, 1);
    d.data[0] = 200; d.data[1] = 100; d.data[2] = 50; d.data[3] = 255;
    cx.putImageData(d, 2, 3);
    r.push('put:' + px(2, 3));

    // A neighbour of the written pixel is untouched (still transparent black).
    r.push('neighbour:' + px(3, 3));

    // Dirty-rectangle overload: only pixel (1,1) of a 2x2 source is written, at canvas (5,5).
    var e = new ImageData(2, 2);
    e.data[(1 * 2 + 1) * 4] = 77; e.data[(1 * 2 + 1) * 4 + 3] = 255; // pixel (1,1) red=77, opaque
    e.data[0] = 9; e.data[3] = 255;                                  // pixel (0,0) red=9 — must NOT be written
    cx.putImageData(e, 4, 4, 1, 1, 1, 1);                           // dirtyX/Y/W/H = 1,1,1,1 → writes src(1,1) at (5,5)
    r.push('dirty:' + px(5, 5) + '|' + px(4, 4));                    // (5,5)=77..., (4,4) stays transparent

    document.getElementById('out').textContent = r.join(' ');
  </script></body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn image_data_and_put_image_data_move_real_pixels() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://canvas-imagedata.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        "ctor:2x3/24/true",         // new ImageData(2,3) → 24-byte Uint8ClampedArray
        "fromarr:2x2/true",         // new ImageData(arr,2) infers height 2 and adopts the array
        "put:200,100,50,255",       // putImageData → getImageData round-trips the exact RGBA
        "neighbour:0,0,0,0",        // an adjacent pixel is untouched (no bleed)
        "dirty:77,0,0,255|0,0,0,0", // dirty-rect writes only src(1,1) at (5,5); (4,4) stays transparent
    ] {
        assert!(
            got.contains(claim),
            "G_CANVAS_IMAGE_DATA: expected {claim} in {got:?}\n  \
             `new ImageData(...)` must construct a real pixel buffer and `putImageData` must WRITE \
             pixels the surface reads back — a no-op putImageData silently discards every filter, \
             histogram and editor edit with no error."
        );
    }
}
