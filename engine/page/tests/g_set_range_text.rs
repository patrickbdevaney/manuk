//! **G_SET_RANGE_TEXT — `<input>`/`<textarea>` `setRangeText`.**
//!
//! Replace text IN the value through the selection — what autocomplete, "insert at cursor", and text
//! editors reach for. With no range it replaces the current selection; with a range, a specific span;
//! `selectMode` decides where the selection lands. It was absent (`is not a function`). Builds on the
//! tick-302 selection store.
//!
//! The teeth are the resulting VALUE and selection (a stub can't fake them):
//!   * `replace-selection` — `setSelectionRange(0,5); setRangeText('HI')` → value `HI world`.
//!   * `range` — `setRangeText('X', 6, 11)` replaces that span.
//!   * `select-mode` — `setRangeText(..., 'select')` selects the inserted text.
//!   * `insert` — an empty-range `setRangeText` at a caret inserts without deleting.
//!
//! Proven RED: unregister the method and `present` is false while the call throws.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<input id="a" value="hello world">
<input id="b" value="hello world">
<input id="c" value="abcdef">
<input id="d" value="XYZ">
<div id="out">-</div>
<script>
var r = [];
function push(s) { r.push(s); }
try {
  var a = document.getElementById('a');
  push('present:' + (typeof a.setRangeText === 'function'));

  a.setSelectionRange(0, 5);
  a.setRangeText('HI');
  push('replace-selection:' + (a.value === 'HI world'));

  var b = document.getElementById('b');
  b.setRangeText('X', 6, 11);   // replace "world"
  push('range:' + (b.value === 'hello X'));

  var c = document.getElementById('c');
  c.setRangeText('ZZ', 2, 4, 'select'); // abZZef, selection over "ZZ"
  push('select-mode:' + (c.value === 'abZZef' && c.selectionStart === 2 && c.selectionEnd === 4));

  var d = document.getElementById('d');
  d.setSelectionRange(1, 1);    // caret between X and Y
  d.setRangeText('__');         // empty-range insert
  push('insert:' + (d.value === 'X__YZ'));
} catch (e) {
  push('THREW:' + e);
}
document.getElementById('out').textContent = r.join(' ');
</script></body></html>"##;

#[test]
fn set_range_text_replaces_through_the_selection() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://rangetext.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("SET-RANGE-TEXT RESULT: {got}");

    for claim in [
        "present:true",
        "replace-selection:true", // replaces the current selection
        "range:true",             // replaces an explicit span
        "select-mode:true",       // 'select' selects the inserted text
        "insert:true",            // empty-range insert at the caret
    ] {
        assert!(
            got.contains(claim),
            "G_SET_RANGE_TEXT: expected `{claim}`\n  got: {got}\n\n  \
             `setRangeText` must splice the replacement into the value (current selection or an \
             explicit range) and land the selection per selectMode."
        );
    }
}
