//! **G_CH_UNIT — the `ch` unit is the font's real `0` advance, not the spec `0.5em` fallback.**
//!
//! `ch` is defined as the advance of the `0` (ZERO) glyph. Stylo asks its `FontMetricsProvider`
//! for `zero_advance_measure`; when the provider answered `None` (a no-op stub) Stylo fell back to
//! the spec's *"impossible to determine"* value of `0.5em`. So a `width: 10ch` box computed to
//! `10 * 0.5em = 80px` at `font-size:16px`, while the text laid INTO it used the font's true
//! advance — a monospace `0` is ~`0.6em` — and overflowed. `max-width: 65ch`, the ubiquitous
//! readable-column idiom, was ~17% too narrow on every article.
//!
//! The provider now measures `0` through the SAME shaper layout uses, so the number the box is
//! sized by is the number the glyphs occupy. The falsifiable check:
//!
//!   * `#chbox { width: 10ch }` (monospace) must equal `#ref` — a span of exactly ten `0`s in the
//!     same monospace font. Ten `ch` and ten monospace chars are the same width BY DEFINITION, and
//!     only agree if `ch` is the real advance. Under the old `0.5em` stub the box was `80px` and the
//!     text `~96px`: they disagree, and this assertion goes red.
//!   * `#chbox` must be **wider than the `0.5em` fallback** (`10 * 0.5 * 16 = 80px`). A monospace
//!     `0` is always wider than half an em, so a real metric clears `88px`; the stub's `80px` does
//!     not. This pins the direction of the fix, so a change that merely made both boxes equally
//!     *wrong* could not pass.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<style>
  body { font: 16px monospace; }
  #chbox { width: 10ch; height: 10px; }
  #ref { white-space: pre; }
</style>
<div id="out">-</div>
<div id="chbox"></div>
<span id="ref">0000000000</span>
<script>
  var w = function (id) { return Math.round(document.getElementById(id).getBoundingClientRect().width); };
  var box = w('chbox'), ref = w('ref');
  document.getElementById('out').textContent =
    'box:' + box + ' ref:' + ref + ' eq:' + (box === ref) + ' real:' + (box > 88);
</script></body></html>"##;

#[test]
fn ch_unit_is_the_real_zero_advance_not_the_half_em_fallback() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://ch.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    assert!(
        got.contains("eq:true"),
        "G_CH_UNIT: `width:10ch` must equal ten monospace `0`s — ten `ch` IS ten monospace chars \
         by definition. Under the `0.5em` stub the box is 80px and the text ~96px, which disagree.\n  \
         got: {got}"
    );
    assert!(
        got.contains("real:true"),
        "G_CH_UNIT: a `10ch` box must clear the `0.5em` fallback (80px) — a monospace `0` is wider \
         than half an em, so a real metric lands ~96px. `real:false` means `ch` is still the stub.\n  \
         got: {got}"
    );
}
