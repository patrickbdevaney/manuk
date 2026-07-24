//! **G_CAP_UNIT — the `cap` unit is the face's real cap-height; it used to collapse to 0px.**
//!
//! `cap` is the cap-height of the font (the height of a flat-topped capital like `H`). Stylo asks its
//! `FontMetricsProvider` for `cap_height`, and its documented fallback when that is `None` is *the
//! font's ascent* — but this provider never set `ascent` either (it defaulted to `0`), so `cap`
//! resolved to **0px** and any `cap`-sized box collapsed to nothing. That is a harder failure than the
//! `ch`/`ex` `0.5em`-fallback bugs: not "slightly wrong" but "gone".
//!
//! The provider now reads the face's OS/2 `sCapHeight` through swash (the same design-unit value Chrome
//! uses) off the same resolved face the shaper draws with. Amplified ×100 at 16px sans-serif:
//!
//!   * `#capbox { width: 100cap }` must be a real cap-height (~`0.72em` → ~`1150px`), i.e. clearly
//!     non-zero — `> 900px`. The old value was `0`, so this pins the fix.
//!   * …and it must stay BELOW the em (`100 * 16 = 1600px`): cap-height is below the em by
//!     construction, so an upper guard catches a provider that returned the wrong (larger) metric.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<style>
  body { font: 16px sans-serif; }
  #capbox { width: 100cap; height: 10px; }
</style>
<div id="out">-</div>
<div id="capbox"></div>
<script>
  var box = Math.round(document.getElementById('capbox').getBoundingClientRect().width);
  document.getElementById('out').textContent =
    'cap100:' + box + ' nonzero:' + (box > 900) + ' sane:' + (box < 1600);
</script></body></html>"##;

#[test]
fn cap_unit_is_the_real_cap_height_not_a_collapsed_zero() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://cap.test/", &fonts, 1700.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    assert!(
        got.contains("nonzero:true"),
        "G_CAP_UNIT: `width:100cap` must be a real cap-height (~1150px at 16px sans), not the old \
         collapsed 0px — `cap`'s fallback is `ascent`, which the provider left at 0, so `cap` was \
         nothing.\n  got: {got}"
    );
    assert!(
        got.contains("sane:true"),
        "G_CAP_UNIT: `100cap` must stay below the em (1600px) — cap-height sits below the em, so a \
         larger value means the WRONG metric was returned.\n  got: {got}"
    );
}
