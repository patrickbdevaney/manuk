//! **G_PERCENTAGE_SERIALIZATION — a percentage must survive the Stylo→`Dim` mapping as a
//! PERCENTAGE, and with the author's precision.**
//!
//! `stylo_map::lp_to_dim` reconstructed every `LengthPercentage` by sampling
//! `to_used_value` at two bases and differencing. That is exactly right for `calc()` and wrong for
//! everything else, because `to_used_value` returns `Au` — 1/60px integers — so the decomposition
//! carried the quantisation into the VALUE:
//!
//! ```text
//!     declared               Chrome        reconstructed
//!     flex-basis: 0%         0%            0px           <- pct==0 is indistinguishable from
//!                                                            px==0 by differencing
//!     flex-basis: 16.6667%   16.6667%      16.666668%    <- Au quantisation, printed to script
//! ```
//!
//! ⭐ The first is information LOSS, not a rounding error: a zero percentage and a zero length are
//! the same used value at every basis, so **no amount of sampling can tell them apart.** It reached
//! script through `getComputedStyle` on every percentage-valued property that mapping serves —
//! `width`, `height`, `margin`, `padding`, `inset`, `flex-basis`.
//!
//! Stylo can simply be asked: `to_length()` and `to_percentage()` answer exactly for the two pure
//! variants and `None` for `Calc`, which is the only case that needs the decomposition at all.
//!
//! ⚠ `flex-basis` is the property under test because its resolved value is the COMPUTED value — a
//! percentage stays a percentage. `width`'s resolved value is the USED value (px), so it cannot see
//! this defect, which is why the defect survived: the properties everyone checks report px.
//!
//! **To watch it go RED:** delete the `to_length()`/`to_percentage()` early-returns from
//! `lp_to_dim` — `0%` reverts to `0px` and `16.6667%` to `16.666668%`.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body>
  <div id="z" style="flex-basis:0%"></div>
  <div id="p" style="flex-basis:16.6667%"></div>
  <div id="s" style="flex:2"></div>
  <div id="l" style="flex-basis:40px"></div>
  <div id="out">-</div>
  <script>globalThis.__report = function () {
    var g = function (id) { return getComputedStyle(document.getElementById(id)).flexBasis; };
    document.getElementById('out').textContent =
      'z[' + g('z') + '] p[' + g('p') + '] s[' + g('s') + '] l[' + g('l') + ']';
  };</script></body></html>"#;

#[test]
fn a_percentage_stays_a_percentage_and_keeps_its_precision() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://pct.test/", &fonts, 800.0);
    page.eval_for_test("globalThis.__report && __report()");

    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    // Chrome-measured, same markup.
    for claim in [
        "z[0%]",       // a ZERO percentage is a percentage, not `0px`
        "p[16.6667%]", // the author's precision, not Au quantisation
        "s[0%]",       // `flex: 2` sets a 0% basis — same defect through the shorthand
        "l[40px]",     // CONTROL: a pure length is unchanged by the exact path
    ] {
        assert!(
            got.contains(claim),
            "G_PERCENTAGE_SERIALIZATION: expected `{claim}`\n  got: {got}\n\n  \
             A percentage reconstructed by differencing Au-quantised samples loses its identity at \
             zero and its precision everywhere else. `to_length()`/`to_percentage()` answer exactly \
             for the pure variants; only `calc()` needs the decomposition."
        );
    }
}
