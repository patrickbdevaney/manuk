//! **G_COMPUTED_STYLE — getComputedStyle exposes ALREADY-COMPUTED properties, not `undefined`.**
//!
//! `computed_style_js` built a fixed ~30-property object and dropped several fields the cascade already
//! computes — `visibility`, `white-space`, `opacity`. A test (or real script) reading
//! `getComputedStyle(el).visibility` got **`undefined`**, not `"hidden"`. These are not new capabilities;
//! the values existed in `ComputedStyle` and were simply not surfaced to JS. Both the camelCase property
//! and `getPropertyValue('white-space')` must resolve.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<div id="v" style="visibility:hidden"></div>
<div id="w" style="white-space:pre-wrap"></div>
<div id="o" style="opacity:0.5"></div>
<div id="plain"></div>
<script>
  var R = [], cs = function (id) { return getComputedStyle(document.getElementById(id)); };
  R.push('vis:' + cs('v').visibility);                       // "hidden"
  R.push('ws:' + cs('w').whiteSpace);                        // "pre-wrap"
  R.push('wsPV:' + cs('w').getPropertyValue('white-space')); // "pre-wrap" via kebab accessor
  R.push('op:' + cs('o').opacity);                           // "0.5"
  R.push('visDflt:' + cs('plain').visibility);               // "visible" (initial), NOT undefined
  R.push('opDflt:' + cs('plain').opacity);                   // "1" (initial), NOT undefined
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn getcomputedstyle_exposes_visibility_whitespace_opacity() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://cs.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("vis:hidden", "getComputedStyle(el).visibility must resolve the computed keyword, not undefined"),
        ("ws:pre-wrap", "…and whiteSpace — a property the cascade already computes and JS could not read"),
        ("wsPV:pre-wrap", "getPropertyValue('white-space') (kebab) must map to the same value"),
        ("op:0.5", "opacity serializes as a bare number, not undefined"),
        (
            "visDflt:visible",
            "the INITIAL value must resolve too — `undefined` for an unset property is the bug, not a value",
        ),
        ("opDflt:1", "initial opacity is the number 1, serialized without trailing zeros"),
    ] {
        assert!(
            got.contains(claim),
            "G_COMPUTED_STYLE: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
