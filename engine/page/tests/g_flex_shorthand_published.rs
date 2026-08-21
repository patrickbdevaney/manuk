//! **G_FLEX_SHORTHAND_PUBLISHED — `getComputedStyle(el).flex` was `undefined` while flex layout
//! worked.**
//!
//! The three longhands (`flexGrow`, `flexShrink`, `flexBasis`) have been published since flex
//! landed. The **shorthand every author actually writes** was not, so a script reading
//! `getComputedStyle(x).flex` got `undefined` where every browser returns a string. Chrome answers
//! `0 1 auto` for the initial value — all three components, always, in that order (CSS Flexbox
//! §7.1.1), never collapsed.
//!
//! ⭐ **This is the FOURTH instance of the reported-vs-applied class** that check #124 named as I3,
//! the invariant under most pressure: `scrollbar-width` was reported and not applied (t1314),
//! `field-sizing` applied and not reported (t1314), `documentElement.clientHeight` computed and
//! mis-published (t1320), and now a shorthand applied and not reported. `map-reconcile.sh` checks
//! that a gate EXISTS, not that a capability answers in BOTH channels, so nothing was going to find
//! this except asking.
//!
//! ⚠ And it was found by one of this project's own probes: t1336 read
//! `getComputedStyle(x).flex` on a corpus anchor to ask which pass had claimed a box, and the
//! instrument's ordinary question came back `undefined`.
//!
//! **To watch it go RED:** remove `"flex"` from `CS_PROPS` in `dom_bindings.rs`, or drop the
//! `flex:{}` slot from the object literal beside it.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body>
  <article id="a" style="display:flex"><figure id="f" style="flex:0 1 40px"></figure></article>
  <div id="b" style="flex:2 3 auto"></div>
  <div id="out">-</div>
  <script>globalThis.__report = function () {
    var g = function (id) { return getComputedStyle(document.getElementById(id)).flex; };
    document.getElementById('out').textContent =
      'a[' + g('a') + '] f[' + g('f') + '] b[' + g('b') + ']';
  };</script></body></html>"#;

#[test]
fn the_flex_shorthand_is_published_with_all_three_components() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://flex.test/", &fonts, 800.0);
    page.eval_for_test("globalThis.__report && __report()");

    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    // Chrome-measured, same markup: `0 1 auto` · `0 1 40px` · `2 3 auto`.
    for claim in ["a[0 1 auto]", "f[0 1 40px]", "b[2 3 auto]"] {
        assert!(
            got.contains(claim),
            "G_FLEX_SHORTHAND_PUBLISHED: expected `{claim}`\n  got: {got}\n\n  \
             `getComputedStyle(el).flex` must return all THREE components in grow/shrink/basis \
             order (CSS Flexbox §7.1.1) — Chrome never collapses it, not even for the initial \
             value. `undefined` here is the reported-vs-applied defect: flex LAYOUT works and the \
             property the author wrote is invisible to script."
        );
    }
    // ⚠ The negative half: `undefined` must not be what a reader gets, and a String "undefined"
    // would satisfy a naive `contains` check on the values above if the format ever changed.
    assert!(
        !got.contains("undefined"),
        "G_FLEX_SHORTHAND_PUBLISHED: the shorthand read back as `undefined`.\n  got: {got}"
    );
}
