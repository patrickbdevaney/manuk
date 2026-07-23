//! **G_COLOR_SCHEME — `color-scheme` reaches getComputedStyle AND darkens the canvas default.**
//!
//! A page declares `color-scheme: dark` (via the property or `<meta name="color-scheme">`) to opt
//! its DEFAULT surfaces into a dark appearance. The most visible effect — and the one this engine
//! models — is the **canvas background**: CSS propagates the root's background to the whole viewport,
//! and when a dark-only page sets no explicit background, the UA paints the canvas dark. Before this,
//! `color-scheme` did not exist in the engine (Stylo's servo build gates it behind the shared
//! `layout.unimplemented` pref), so `getComputedStyle(el).colorScheme` was `undefined` and a
//! dark-only page painted its content on a dark box floating in a WHITE void below the fold.
//!
//! Two teeth, one property:
//!   1. **CSSOM** — `getComputedStyle(el).colorScheme` resolves `normal`/`light`/`dark`/`light dark`
//!      from the cascade (a dark-logo swap reads it back). RED: without the `stylo_map` mapping or
//!      the pref flip, every element reads `normal`.
//!   2. **Paint** — a `:root { color-scheme: dark }` page with no background paints a DARK canvas
//!      below its content; a normal page stays white. RED: revert the `canvas_background` branch and
//!      the dark page paints white — the exact "dark content in a white void" regression.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
  :root { color-scheme: light dark; }
  #d { color-scheme: dark; }
  #l { color-scheme: light; }
</style></head><body>
<div id="root-probe">root</div>
<div id="d">dark</div>
<div id="l">light</div>
<div id="out">-</div>
<script>
var r = [];
try {
  var g = function (id) { return getComputedStyle(document.getElementById(id)).colorScheme; };
  r.push('root:' + g('root-probe'));   // inherits `light dark` from :root
  r.push('dark:' + g('d'));
  r.push('light:' + g('l'));
  r.push('gpv:' + getComputedStyle(document.getElementById('d')).getPropertyValue('color-scheme'));
} catch (e) {
  r.push('THREW:' + e);
}
document.getElementById('out').textContent = r.join(' ');
</script></body></html>"##;

#[test]
fn color_scheme_reaches_cssom() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://colorscheme.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("COLOR-SCHEME CSSOM RESULT: {got}");

    for claim in [
        "root:light dark", // `:root { color-scheme: light dark }` inherits to children
        "dark:dark",       // a dark-only element
        "light:light",     // a light-only element
        "gpv:dark",        // getPropertyValue('color-scheme') serves the keyword
    ] {
        assert!(
            got.contains(claim),
            "G_COLOR_SCHEME(cssom): expected `{claim}`\n  got: {got}\n\n  \
             `color-scheme` must cascade and reach getComputedStyle. If every element reads \
             `normal`, the `layout.unimplemented` pref flip or the `stylo_map` mapping was lost."
        );
    }
}

// A dark-only page with NO explicit background must paint a DARK canvas below its content — not the
// white void the hard-coded `Rgba::WHITE` produced. The sibling assertion (a plain page stays white)
// proves the branch is gated on the scheme, not painting dark unconditionally.
const DARK: &str = r##"<!doctype html><html style="color-scheme: dark"><body style="margin:0"><p>short</p></body></html>"##;
const PLAIN: &str = r##"<!doctype html><html><body style="margin:0"><p>short</p></body></html>"##;

#[test]
fn dark_color_scheme_paints_a_dark_canvas() {
    let fonts = FontContext::new();

    let dark = manuk_page::Page::load(DARK, "https://dark.test/", &fonts, 400.0);
    let dc = dark.paint(&fonts, 400, 600);
    let dpx = dc.rgba_bytes();
    // Sample a pixel deep below the one-line content — the canvas void.
    let idx = ((550 * 400) + 200) * 4;
    let (r, g, b) = (dpx[idx], dpx[idx + 1], dpx[idx + 2]);
    assert!(
        r < 60 && g < 60 && b < 60,
        "G_COLOR_SCHEME(paint): a `color-scheme: dark` page painted rgb({r},{g},{b}) below its \
         content, expected a dark canvas. A dark-only page's canvas default must follow the scheme, \
         not fall through to white."
    );

    // The control: an identical page WITHOUT color-scheme keeps the white canvas — so the branch is
    // driven by the scheme, and this is not a test that darkens everything.
    let plain = manuk_page::Page::load(PLAIN, "https://plain.test/", &fonts, 400.0);
    let pc = plain.paint(&fonts, 400, 600);
    let ppx = pc.rgba_bytes();
    let (pr, pg, pb) = (ppx[idx], ppx[idx + 1], ppx[idx + 2]);
    assert!(
        pr > 200 && pg > 200 && pb > 200,
        "G_COLOR_SCHEME(paint control): a plain page painted rgb({pr},{pg},{pb}), expected white — \
         the dark canvas must be gated on `color-scheme: dark`, not applied unconditionally."
    );
}
