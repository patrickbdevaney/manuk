//! **G_CLASS_ASCII_WHITESPACE — `getElementsByClassName` splits on ASCII whitespace only.**
//!
//! The class argument (and each element's `class` attribute) is parsed as an ordered set on the five
//! ASCII whitespace characters — TAB, LF, FF, CR, SPACE — and NOTHING else. A class of a single
//! non-ASCII "space" character (U+00A0 no-break space, U+2003 em space, U+000B line tabulation, …) is a
//! real, matchable token. Our impl used Rust `split_whitespace()` (Unicode White_Space), which split
//! those into nothing and dropped the whole `dom/nodes/getElementsByClassName-whitespace-class-names`
//! file. It also built a `.{class}` CSS selector string, which broke on class names containing selector
//! metacharacters.

use manuk_text::FontContext;

const HTML: &str = "<!doctype html><html><body>\
<div id=\"out\">-</div>\
<span id=\"nbsp\" class=\"\u{00A0}\">nbsp</span>\
<span id=\"em\" class=\"\u{2003}\">em</span>\
<span id=\"vt\" class=\"\u{000B}\">vt</span>\
<span id=\"multi\" class=\"a  b\">multi</span>\
<span id=\"dotty\" class=\"a.b\">dotty</span>\
<script>\
  var R = [], push = function (k, v) { R.push(k + '=' + v); };\
  var g = function (c) { return document.getElementsByClassName(c); };\
  push('nbsp', g('\\u00A0').length + ',' + (g('\\u00A0')[0] && g('\\u00A0')[0].id));\
  push('em', g('\\u2003')[0] && g('\\u2003')[0].id);\
  push('vt', g('\\u000B')[0] && g('\\u000B')[0].id);\
  push('multi', g('a b').length + ',' + g('a b')[0].id);\
  push('dotty', g('a.b').length + ',' + g('a.b')[0].id);\
  push('empty', g('').length);\
  document.getElementById('out').textContent = R.join(' ');\
</script></body></html>";

#[test]
fn get_elements_by_class_name_splits_on_ascii_whitespace_only() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://cls.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "nbsp=1,nbsp",
            "a class of a single U+00A0 (non-ASCII space) is a real token and matches",
        ),
        (
            "em=em",
            "U+2003 EM SPACE is not ASCII whitespace — matchable class",
        ),
        (
            "vt=vt",
            "U+000B LINE TABULATION is NOT ASCII whitespace (only TAB/LF/FF/CR/SPACE are)",
        ),
        (
            "multi=1,multi",
            "`getElementsByClassName('a b')` requires both classes present",
        ),
        (
            "dotty=1,dotty",
            "a class name containing '.' matches (no CSS-selector-string fragility)",
        ),
        ("empty=0", "an empty argument matches nothing"),
    ] {
        assert!(
            got.contains(claim),
            "G_CLASS_ASCII_WHITESPACE: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
