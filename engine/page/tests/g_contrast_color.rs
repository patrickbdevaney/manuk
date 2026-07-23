//! **G_CONTRAST_COLOR — `contrast-color(<color>)` resolves to its legible black/white companion.**
//!
//! CSS Color 5's `contrast-color()` (Baseline 2026) returns whichever of black/white contrasts more
//! with the given color — the accessible-theming idiom for picking legible text over a dynamic
//! background without JS (`color: contrast-color(var(--brand))`). Stylo 0.19 parses it only with
//! `layout.css.contrast-color.enabled` on (off by default → the declaration is dropped at parse and
//! the color falls back). This engine already resolves Stylo's `ComputedColor::ContrastColor` to an
//! absolute color through `resolve_to_absolute`, so flipping the pref is enough.
//!
//! Proven RED: with the pref off, `contrast-color(black)` fails to parse and the element keeps its
//! inherited/initial color — the `bg-on-black`/`color-on-white` reads land on the wrong value.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="a" style="background-color: contrast-color(black)">a</div>
<div id="b" style="background-color: contrast-color(white)">b</div>
<div id="c" style="color: contrast-color(black)">c</div>
<div id="out">-</div>
<script>
var r = [];
try {
  var g = function (id, p) { return getComputedStyle(document.getElementById(id))[p]; };
  // contrast-color(black) => white (white contrasts more with black); on white => black.
  r.push('bg-on-black:' + g('a', 'backgroundColor'));
  r.push('bg-on-white:' + g('b', 'backgroundColor'));
  r.push('color-on-black:' + g('c', 'color'));
} catch (e) {
  r.push('THREW:' + e);
}
document.getElementById('out').textContent = r.join(' ');
</script></body></html>"##;

#[test]
fn contrast_color_resolves_to_the_legible_companion() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://contrastcolor.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("CONTRAST-COLOR RESULT: {got}");

    // Serialized rgb — white is rgb(255, 255, 255), black is rgb(0, 0, 0).
    for claim in [
        "bg-on-black:rgb(255, 255, 255)", // most-contrasting companion of black is white
        "bg-on-white:rgb(0, 0, 0)",       // and of white is black
        "color-on-black:rgb(255, 255, 255)", // the `color` property resolves it too
    ] {
        assert!(
            got.contains(claim),
            "G_CONTRAST_COLOR: expected `{claim}`\n  got: {got}\n\n  \
             `contrast-color(<c>)` must resolve to the black/white that contrasts more with <c>. If \
             the value did not resolve, the `layout.css.contrast-color.enabled` pref flip was lost \
             and the declaration was dropped at parse."
        );
    }
}
