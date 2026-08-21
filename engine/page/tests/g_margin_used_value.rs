//! **G_MARGIN_USED_VALUE — `getComputedStyle(el).marginLeft` is the USED value, and a percentage
//! margin never reached px.**
//!
//! CSSOM-View puts `margin-*` in the resolved-value **used** bucket for an element that generates a
//! box. `width` and `height` already went through `used_dim_css` for exactly that reason; the four
//! margins were still publishing what the cascade holds. Chrome-measured, containing block 1000px:
//!
//! ```text
//!     declared                              Chrome     ours (before)
//!     margin-left: calc(50% - 10px)         490px      calc(-10px + 50%)
//!     margin-top: 10%                       100px      10%
//!     margin-right: 25%                     250px      25%
//!     margin-left: 40px                      40px       40px       <- CONTROL, agreed already
//! ```
//!
//! ⭐ **A script reading a margin gets a STRING, and `parseFloat("calc(-10px + 50%)")` is `NaN`.**
//! Every layout script that measures its own gutters — a masonry, a sticky-offset calculator, a
//! carousel step — takes that path, so the symptom is a silently wrong POSITION rather than a
//! visible error. `"10%"` is worse: `parseFloat` succeeds and hands back `10`.
//!
//! ⚠ **The serialization was the visible half of a two-half fix.** Writing it alone compiled and
//! moved nothing (t1339), because `computed_style_js` receives `cb` and it was `None` for every
//! element here: the call site walks for a containing block only when
//! `inset_needs_containing_block` says so — a positioned element with a percentage inset — since
//! `getComputedStyle` is hot and the walk is a tree walk. The fix is three parts: a
//! `margin_needs_containing_block` predicate as narrow as the inset one, a `Position::Static` arm on
//! `containing_block_size` (a normal-flow box's containing block is its parent's content box; the
//! arm answered `None` only because no caller could ever ask), and then the four call sites.
//!
//! ⚠ `auto` is deliberately NOT converted and is asserted as `auto` below so the residue is on the
//! record: its used value is a real number too (`e` reads `400px` in Chrome), but deriving it needs
//! the used width and the sibling margin. A confident wrong number is worse than an honest `auto`
//! precisely because `parseFloat` cannot tell them apart.
//!
//! **To watch it go RED:** revert `margin_css` to `dim_css` in `computed_style_js` — `a`, `b` and
//! `c` all revert to the author's text while the `d` control stays green. Reverting only the
//! `Position::Static` arm reds the same three, which is the half t1339 was missing.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body style="margin:0">
  <div style="width:1000px">
    <div id="a" style="margin-left:calc(50% - 10px)">a</div>
    <div id="b" style="margin-top:10%">b</div>
    <div id="c" style="margin-right:25%">c</div>
    <div id="d" style="margin-left:40px">d</div>
    <div id="e" style="margin-left:auto;margin-right:auto;width:200px">e</div>
  </div>
  <div id="out">-</div>
  <script>globalThis.__report = function () {
    var g = function (id, p) { return getComputedStyle(document.getElementById(id))[p]; };
    document.getElementById('out').textContent =
      'a[' + g('a', 'marginLeft') + '] b[' + g('b', 'marginTop') + '] c[' + g('c', 'marginRight') +
      '] d[' + g('d', 'marginLeft') + '] e[' + g('e', 'marginLeft') + ']';
  };</script></body></html>"#;

#[test]
fn a_percentage_margin_reports_the_used_px_value() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://margin.test/", &fonts, 1200.0);
    page.eval_for_test("globalThis.__report && __report()");

    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    // Chrome-measured, same markup, same 1000px containing block.
    for claim in [
        "a[490px]", // calc() resolves against the containing block's INLINE size
        "b[100px]", // the BLOCK-axis margin resolves against the inline size too (CSS 2.1 §8.3)
        "c[250px]", // a plain percentage
        "d[40px]",  // CONTROL: a pure length is untouched by the used-value path
        "e[auto]",  // RESIDUE, on the record: Chrome says `400px` here and we do not derive it
    ] {
        assert!(
            got.contains(claim),
            "G_MARGIN_USED_VALUE: expected `{claim}`\n  got: {got}\n\n  \
             CSSOM-View resolves `margin-*` to the USED value for an element with a box. A script \
             reading a margin gets a string, and `parseFloat(\"calc(-10px + 50%)\")` is NaN — a \
             silently wrong position rather than a visible failure."
        );
    }
}
