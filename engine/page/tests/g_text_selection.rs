//! **G_TEXT_SELECTION — `<input>`/`<textarea>` text selection API.**
//!
//! `selectionStart`/`selectionEnd`/`selectionDirection`, `setSelectionRange(start,end[,dir])`, and
//! `select()` — how a page positions the cursor, selects a range, or "selects all on focus". The whole
//! surface was absent (`undefined`), so an input mask, a copy-button, or an editor that reads or sets
//! the caret got `setSelectionRange is not a function` / `undefined` offsets.
//!
//! The teeth are read-back values (a stub returning a constant fails):
//!   * `select-all` — `select()` selects the entire value (`0` .. value length).
//!   * `range` — `setSelectionRange(2, 5)` reads back `selectionStart===2`, `selectionEnd===5`.
//!   * `direction` — a `'backward'` direction round-trips.
//!   * `clamp` — offsets past the value length clamp to the length.
//!
//! Proven RED: unregister the accessors/methods and `present` is false while the calls throw.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<input id="i" value="hello world">
<div id="out">-</div>
<script>
var r = [];
function push(s) { r.push(s); }
var i = document.getElementById('i');
try {
  push('present:' + (typeof i.setSelectionRange === 'function' && typeof i.select === 'function' &&
                     typeof i.selectionStart === 'number'));

  i.select();
  push('select-all:' + (i.selectionStart === 0 && i.selectionEnd === 11)); // "hello world" is 11

  i.setSelectionRange(2, 5);
  push('range:' + (i.selectionStart === 2 && i.selectionEnd === 5));

  i.setSelectionRange(1, 4, 'backward');
  push('direction:' + (i.selectionDirection === 'backward'));

  i.setSelectionRange(50, 99); // past the end -> clamps to 11
  push('clamp:' + (i.selectionStart === 11 && i.selectionEnd === 11));
} catch (e) {
  push('THREW:' + e);
}
document.getElementById('out').textContent = r.join(' ');
</script></body></html>"##;

#[test]
fn input_text_selection_reads_and_writes() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://sel.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("TEXT-SEL RESULT: {got}");

    for claim in [
        "present:true",
        "select-all:true", // select() covers the whole value
        "range:true",      // setSelectionRange offsets read back
        "direction:true",  // direction round-trips
        "clamp:true",      // offsets past the end clamp
    ] {
        assert!(
            got.contains(claim),
            "G_TEXT_SELECTION: expected `{claim}`\n  got: {got}\n\n  \
             `setSelectionRange`/`select` must set `selectionStart`/`selectionEnd`/`selectionDirection` \
             (clamped to the value length), readable back. A stub with constant offsets fails."
        );
    }
}
