//! **G_REPLACED_RATIO — the dimension attributes are an aspect ratio, and a `max-width` clamp
//! transfers through it.**
//!
//! Two mechanisms, and the web only works when both are present at once.
//!
//! 1. **`<img width="800" height="400">` is an aspect-ratio hint** (HTML §"dimension attributes":
//!    `aspect-ratio: auto 800 / 400`). Those attributes exist *precisely* to let the box be laid out
//!    at the right shape **before the bitmap arrives** — that is the entire anti-layout-shift story
//!    Next.js/`<Image>`, WordPress and GitHub all ship. Deriving the ratio only from a decoded image
//!    (which is what this engine did) means `<canvas>` and `<video>` — which never have a decode —
//!    have no ratio *ever*, and a not-yet-loaded `<img>` has none *yet*.
//!
//! 2. **A min/max-width clamp on a replaced element is a CSS2.1 §10.4 constraint violation**, so the
//!    used height is recomputed from the used width rather than keeping the specified one. Without
//!    this, `img { max-width: 100% }` — the single most common rule in any CSS reset — narrows the
//!    box to the column and leaves the height alone: the picture renders **squashed to half its
//!    width at its full height**, on every responsive page, at every viewport narrower than the
//!    image.
//!
//! The bar is the responsive-image case exactly: an 800x400 asset in a 400px column is 400x200.
//! `#c` proves it independently of any image pipeline (a `<canvas>` has no bitmap to decode), and
//! `#z` pins the degenerate end of the same rule — `max-width: 0` collapses BOTH axes, which is the
//! `css/css-sizing` expectation that first exposed all of this.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<style>
  .col { width: 400px }
  img, canvas { max-width: 100% }
</style>
<div id="out">-</div>
<div class="col"><img id="i" width="800" height="400"></div>
<div class="col"><canvas id="c" width="800" height="400"></canvas></div>
<div class="col"><canvas id="z" width="15" height="15" style="max-width:0px"></canvas></div>
<div class="col"><img id="u" width="800" height="400" style="max-width:none"></div>
<script>
  var R = [];
  var r = function (id) { return document.getElementById(id).getBoundingClientRect(); };
  R.push('iw:' + Math.round(r('i').width) + 'x' + Math.round(r('i').height));
  R.push('cw:' + Math.round(r('c').width) + 'x' + Math.round(r('c').height));
  R.push('zw:' + Math.round(r('z').width) + 'x' + Math.round(r('z').height));
  R.push('uw:' + Math.round(r('u').width) + 'x' + Math.round(r('u').height));
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn a_clamped_replaced_element_keeps_the_ratio_its_attributes_declare() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://grr.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "iw:400x200",
            "an 800x400 <img> under `max-width:100%` in a 400px column is 400x200 — the width \
             attribute's ratio survives the clamp. 400x400 means the clamp did not transfer (the \
             picture renders squashed); 400x0 means the attributes gave no ratio at all",
        ),
        (
            "cw:400x200",
            "the same for a <canvas>, which has NO decoded bitmap — so this passes only if the \
             ratio came from the width/height ATTRIBUTES, not from an image pipeline",
        ),
        (
            "zw:0x0",
            "`max-width:0` on a 15x15 canvas collapses both axes: §10.4's adjustment is \
             proportional, so a zero used width forces a zero used height",
        ),
        (
            "uw:800x400",
            "and with the clamp removed the element keeps its attribute size unchanged — the \
             transfer fires on a constraint VIOLATION, it does not rewrite unclamped boxes",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_REPLACED_RATIO: expected `{claim}` — {why}.\n  got: {got}"
        );
    }
}
