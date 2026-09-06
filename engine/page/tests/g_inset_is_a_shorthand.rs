//! **G_INSET_IS_A_SHORTHAND — the four longhands were implemented and the shorthand was not, so
//! `inset: 0` set nothing and an `position:absolute; inset:0` box laid out `0x0`.**
//!
//! Chrome-measured (`google-chrome --headless --dump-dom`), each box absolutely positioned inside a
//! `200x30` relative parent, read back as `getBoundingClientRect()`:
//!
//! ```text
//!                                                  Chrome    before    after
//!   a  .sh   { inset: 0 }               stylesheet  200x30      0x0    200x30
//!   c  .sh2  { inset: 5px 10px }        stylesheet  180x20      0x0    180x20
//!   d  style="inset: 0"                 inline      200x30      0x0    200x30
//!   b  .lh   { top/right/bottom/left }  CONTROL     200x30   200x30    200x30  ✓
//! ```
//!
//! ⭐⭐ **`b` IS THE ROW THAT NAMES THE BUG.** The four longhands were already correct, from the same
//! stylesheet, on the same element — so this was never "absolute positioning is broken" or "the
//! parser drops the rule". Exactly one entry was missing from one match. The two-value form (`c`) is
//! there because a `0` cannot tell `Sides::all` from a function that only ever sets `top`.
//!
//! ⚠⚠⚠ **THIS IS INVISIBLE TO WPT, AND THAT IS THE REAL FINDING.** `manuk-agent` depends on
//! `manuk-page` **without the `stylo` feature**, so every `agent/tests/` fixture carrying a `<style>`
//! block is cascaded by `MinimalCascade` rather than by Stylo — while WPT, and every
//! `engine/page/tests/` gate that asks for `--features stylo`, runs the other one. A property
//! implemented in Stylo and missing here is correct everywhere the suites look and wrong everywhere
//! the agent looks. It surfaced as an inexplicable `0x0` box in an accessibility tree during a Track
//! C drive probe, was mis-attributed to layout for a whole tick, and only resolved when the same
//! fixture was run under both cascades and disagreed.
//!
//! This gate therefore runs **without** `stylo` on purpose. Under Stylo every row already passed.
//!
//! Mutations that must turn this red:
//!   1. remove the `"inset"` arm                     → a, c, d read 0x0
//!   2. `set_shorthand(..., allow_auto: false)`      → the auto row collapses to 0
//!   3. `s.inset.top = parse_dim(v)` instead of the  → c reads 180x30 (the second value is lost)
//!      shorthand expansion

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><meta charset=utf-8><style>
body { margin: 0 }
.rel { position: relative; width: 200px; height: 30px; margin: 4px }
.sh  { position: absolute; inset: 0 }
.lh  { position: absolute; top: 0; left: 0; right: 0; bottom: 0 }
.sh2 { position: absolute; inset: 5px 10px }
.sh3 { position: absolute; inset: auto 0 0 auto; width: 40px; height: 10px }
</style></head><body>
<div class="rel"><div id="a" class="sh"></div></div>
<div class="rel"><div id="b" class="lh"></div></div>
<div class="rel"><div id="c" class="sh2"></div></div>
<div class="rel"><div id="d" style="position:absolute;inset:0"></div></div>
<div class="rel"><div id="e" class="sh3"></div></div>
<div id="out">-</div>
<script>
function r(k){var e=document.getElementById(k),b=e.getBoundingClientRect(),p=e.parentNode.getBoundingClientRect();
 return k+'='+Math.round(b.width)+'x'+Math.round(b.height)+'@'+Math.round(b.left-p.left)+','+Math.round(b.top-p.top);}
document.getElementById('out').textContent=['a','b','c','d','e'].map(r).join(' ');
</script></body></html>"##;

#[test]
fn inset_is_a_shorthand_for_the_four_longhands() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://inset.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("INSET: {got}");

    // ── VACUITY. The longhand control must already be right, or every row below is measuring
    //    whether absolute positioning works at all rather than whether the shorthand expands.
    assert!(
        got.contains("b=200x30@0,0"),
        "VACUOUS: the top/right/bottom/left CONTROL does not fill its parent, so the shorthand rows \
         are not measuring the shorthand — got {got:?}"
    );

    // Chrome headless, all five rows.
    let want = "a=200x30@0,0 b=200x30@0,0 c=180x20@10,5 d=200x30@0,0 e=40x10@160,20";
    assert_eq!(
        got, want,
        "\n  `inset` must expand to the four longhands\n  want: {want}\n  got:  {got}"
    );
}
