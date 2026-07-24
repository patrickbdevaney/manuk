//! **G_EX_UNIT — the `ex` unit is the font's real x-height, not the spec `0.5em` fallback.**
//!
//! `ex` is the x-height of the font. Like `ch` (see `g_ch_unit`), Stylo asked its
//! `FontMetricsProvider` for `x_height`; the no-op stub returned `None`, so Stylo used the spec's
//! *"impossible to determine"* value of `0.5em`. Most real faces have an x-height slightly OVER half
//! an em (DejaVu/Liberation sans ≈ `0.52em`), so an `ex`-sized box was a few percent too short —
//! invisible on one element, cumulative down a form or an icon column sized in `ex`.
//!
//! The provider now reads the face's OS/2 `sxHeight` through swash (the same design-unit value Chrome
//! uses) off the same resolved face the shaper draws with. Amplified ×100 to make the difference
//! unmistakable at 16px sans-serif:
//!
//!   * `#exbox { width: 100ex }` must clear the `0.5em` fallback (`100 * 0.5 * 16 = 800px`). A real
//!     x-height of ~`0.52em` lands ~`832px`; the stub's `800px` does not clear `810`.
//!   * …and it must stay BELOW `900px` — a guard so that returning the *wrong* metric (cap-height
//!     ~`0.72em` → ~`1150px`, or a whole em → `1600px`) fails instead of passing. The window pins the
//!     value to a genuine x-height, not merely "something bigger than the fallback".

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<style>
  body { font: 16px sans-serif; }
  #exbox { width: 100ex; height: 10px; }
</style>
<div id="out">-</div>
<div id="exbox"></div>
<script>
  var box = Math.round(document.getElementById('exbox').getBoundingClientRect().width);
  document.getElementById('out').textContent =
    'ex100:' + box + ' real:' + (box > 810) + ' sane:' + (box < 900);
</script></body></html>"##;

#[test]
fn ex_unit_is_the_real_x_height_not_the_half_em_fallback() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://ex.test/", &fonts, 900.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    assert!(
        got.contains("real:true"),
        "G_EX_UNIT: `width:100ex` must clear the `0.5em` fallback (800px) — a real face x-height is \
         wider than half an em (~832px at 16px sans). `real:false` means `ex` is still the stub.\n  \
         got: {got}"
    );
    assert!(
        got.contains("sane:true"),
        "G_EX_UNIT: `100ex` must stay below 900px — cap-height (~1150px) or a whole em (1600px) would \
         mean the WRONG metric was returned. The window pins the value to a genuine x-height.\n  \
         got: {got}"
    );
}
