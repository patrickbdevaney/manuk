//! **G_MODERN_COLOR_SERIALIZATION — `getComputedStyle(el).color` answered `rgb(…)` for colours that
//! are not sRGB and have no `rgb()` form.**
//!
//! CSS Color 4 splits colour serialization in two. A **legacy** sRGB colour — a hex, a named colour,
//! `rgb()`, `hsl()`, `hwb()` — serializes as `rgb()`/`rgba()` with 0–255 integer channels. Everything
//! else — `color()`, `lab()`, `oklch()`, `color-mix()`, and every relative `rgb(from …)` — keeps its
//! own function and its own **0–1 float** channels. The two are not interchangeable: a wide-gamut
//! `color(display-p3 1 0 0)` has no `rgb()` spelling at all, and quantising it to 8-bit sRGB is a
//! lossy answer wearing the right type.
//!
//! ⚠⚠⚠ **Every colour in this engine reached the CSSOM through `Rgba { r, g, b, a: u8 }`**, so the
//! colour SPACE was discarded at the `stylo_map` boundary and one `format!("rgb({}, {}, {})")` served
//! them all. `color: rgb(from rebeccapurple r g b)` — the identity relative colour — read back as
//! `rgb(102, 51, 153)` where every browser says `color(srgb 0.4 0.2 0.6)`: the same colour, in the
//! wrong space, off by a factor of 255 to anything that parses the numbers.
//!
//! **Measured before building: three files in `css/css-color/parsing` carry 2,585 failing subtests**
//! (`color-computed-relative-color` 1,169, `color-computed-color-mix-function` 948,
//! `color-computed-color-function` 468) and every one of them fails the same way — *"expected 0.4 but
//! got 102"*. `102 = 0.4 × 255`.
//!
//! ⭐ **The whole rule is BORROWED, per the ladder — option 1, no fork.** Stylo's
//! `impl ToCss for AbsoluteColor` already implements CSS Color 4 §Serializing exactly: the legacy
//! branch, the `lab()`/`lch()`/`oklab()`/`oklch()` branch, the `color(<space> …)` branch, and the
//! spec's alpha rule. The engine was computing that value and throwing it away. This publishes it.
//!
//! ⚠ **The legacy path is DELIBERATELY untouched.** `color_css` is set only when the computed colour
//! is NOT legacy sRGB, so every hex, named colour, `rgb()` and `hsl()` on the web keeps the exact
//! byte-for-byte answer it had — including t1205's alpha serialization, which was fitted against
//! Chrome and must not be re-derived by a second implementation. `n1`–`n4` are that control.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
 #t1 { color: rgb(from rebeccapurple r g b); }
 #t2 { color: color-mix(in srgb, red, blue); }
 #t3 { color: color(display-p3 0.1 0.2 0.3); }
 #t4 { color: lab(50% 20 30); }
 #t5 { color: oklch(0.5 0.1 200); }
 #t6 { color: color(srgb 0.4 0.2 0.6 / 0.5); }
 #n1 { color: rebeccapurple; }
 #n2 { color: #663399; }
 #n3 { color: hsl(120deg 20% 50%); }
 #n4 { color: rgba(0, 0, 0, 0.5); }
</style></head><body>
 <div id="t1"></div><div id="t2"></div><div id="t3"></div><div id="t4"></div>
 <div id="t5"></div><div id="t6"></div>
 <div id="n1"></div><div id="n2"></div><div id="n3"></div><div id="n4"></div>
 <div id="out">-</div>
 <script>
 window.addEventListener('load', function(){
   var ids=['t1','t2','t3','t4','t5','t6','n1','n2','n3','n4'], r=[];
   for (var i=0;i<ids.length;i++){
     r.push(ids[i]+'='+getComputedStyle(document.getElementById(ids[i])).color);
   }
   document.getElementById('out').textContent=r.join(' | ');
 });
 </script></body></html>"##;

/// **One test, on purpose** — see `g_defer`.
#[test]
fn a_non_legacy_color_keeps_its_own_function_and_float_channels() {
    let fonts = FontContext::new();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let page = rt.block_on(manuk_page::Page::load_async(
        HTML,
        "https://color.test/",
        &fonts,
        800.0,
    ));
    let root = page.dom().root();
    let hits = manuk_css::query_selector_all(page.dom(), root, "#out");
    assert!(!hits.is_empty(), "#out must exist");
    let got = page.dom().text_content(hits[0]);
    println!("MODERN-COLOR {got}");
    // RED, run: make `stylo_map::modern_color_css` return `None` unconditionally — all six `t*`
    // rows collapse onto the 8-bit sRGB quantisation (`t1=rgb(102, 51, 153)`, `t3=rgb(15, 52, 79)`,
    // `t4=rgb(161, 105, 69)`) while the four `n*` controls do not move, which is what says this
    // gate measures the serialization and not the colour pipeline at large.
    assert_eq!(
        got,
        "t1=color(srgb 0.4 0.2 0.6) | t2=color(srgb 0.5 0 0.5) | \
         t3=color(display-p3 0.1 0.2 0.3) | t4=lab(50 20 30) | t5=oklch(0.5 0.1 200) | \
         t6=color(srgb 0.4 0.2 0.6 / 0.5) | n1=rgb(102, 51, 153) | n2=rgb(102, 51, 153) | \
         n3=rgb(102, 153, 102) | n4=rgba(0, 0, 0, 0.5)",
        "a colour that is not legacy sRGB must serialize in its OWN function with 0–1 float \
         channels. `t1` is the core claim: `rgb(from rebeccapurple r g b)` is the identity relative \
         colour and every browser reports `color(srgb 0.4 0.2 0.6)` — we reported \
         `rgb(102, 51, 153)`, the same colour with its channels multiplied by 255, which is the \
         exact failure the 2,585 subtests in `css/css-color/parsing` were reporting as \
         *\"expected 0.4 but got 102\"*. `t3` is the row that shows why this is not cosmetic: \
         `color(display-p3 …)` is OUTSIDE sRGB, so the 8-bit answer is not a different spelling of \
         the colour, it is a different colour. `t4`/`t5` prove the `lab()`/`oklch()` branches are \
         reached and NOT converted to sRGB. `t6` carries alpha through the modern branch. \
         ⚠ THE FOUR CONTROLS ARE THE NO-REGRESSION ARGUMENT and they cover the whole legacy web: \
         `n1` named, `n2` hex, `n3` `hsl()` — which is a NON-sRGB colour space that nevertheless \
         carries `IS_LEGACY_SRGB`, so it is the row that proves the discriminator is the flag and \
         not the colour space — and `n4` the translucent case whose alpha serialization was fitted \
         against Chrome at t1205 and must not be re-derived by a second implementation"
    );
}
