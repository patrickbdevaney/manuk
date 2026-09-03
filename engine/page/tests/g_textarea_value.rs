//! **G_TEXTAREA_VALUE — `<textarea>.value` is its text content, and the selection API edits it correctly.**
//!
//! `<textarea>abcdef</textarea>.value` returned `""` — every value/selection path read the `value`
//! ATTRIBUTE, which a textarea does not have (its raw value is the child text content). So a server-
//! rendered pre-filled textarea (edit-comment / edit-bio / edit-post) read an EMPTY field, and
//! `setRangeText` — sharing the same broken value source — replaced the WHOLE value instead of the
//! selected range. Each claim is a way this goes RED:
//!
//!   * `textarea.value` is the text content (`"abcdef"`), whitespace and newlines preserved.
//!   * setting `.value` dirties it (a later read returns the set value).
//!   * `setSelectionRange(1,3)` + `setRangeText('XY')` on `"abcdef"` splices to `"aXYdef"` — NOT `"XY"`.
//!   * `<input>` is unaffected (it really does read its `value` attribute).
//!
//! ⚠⚠ **`taLen` asserted `6` until tick 1392, and that claim was holding the engine to a bug.** It
//! read `selectionStart` on a *pristine* textarea and pinned the answer to the value's LENGTH, with a
//! comment reasoning it out — "default caret at end == value length (6), not 0". Headless Chrome says
//! `0`, and so does WPT (`html/semantics/forms/textfieldselection/defaultSelection.html` asserts
//! `selectionStart === 0` for both a `<textarea>` and an `<input>`). The number 6 is real, but it is
//! the caret's place AFTER a `.value` write, not before one — so this gate had turned a consequence
//! into a definition and then defended it. The row is corrected here **against a measurement**, and
//! `taSelAfterSet` is added beside it so the fact the old claim was reaching for is still asserted,
//! in the place it actually holds. `spliceSel` was computed and never asserted; it is a real claim now.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body>
<textarea id="t">abcdef</textarea>
<textarea id="ws">  keep
nl</textarea>
<textarea id="edit">abcdef</textarea>
<input id="i" value="Hello">
<div id="out">-</div><script>
var r=[];
function k(n,v){ r.push(n+':'+JSON.stringify(v)); }
var t=document.getElementById('t');
k('taValue', t.value);
k('taLen', t.selectionStart);              // the caret RESTS at 0 (Chrome-measured), not at 6
k('ws', document.getElementById('ws').value);
t.value='NEW'; k('taSet', t.value);        // dirtied
k('taSelAfterSet', t.selectionStart+'-'+t.selectionEnd); // …and THAT is what moves the caret to 3
var e=document.getElementById('edit');
e.setSelectionRange(1,3); e.setRangeText('XY');
k('splice', e.value);                       // 'aXYdef', not 'XY'
var e2=document.getElementById('edit');
k('spliceSel', e2.selectionStart+'-'+e2.selectionEnd); // preserve mode
k('inValue', document.getElementById('i').value);
document.getElementById('out').textContent=r.join(' ');
</script></body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn textarea_value_is_text_content_and_setrangetext_splices() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://textarea-value.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        "taValue:\"abcdef\"",    // value == text content, not ""
        "taLen:0",               // ⭐ the caret RESTS at 0 — see below
        "taSelAfterSet:\"3-3\"", // …and a `.value` write is what moves it to the end
        "ws:\"  keep\\nnl\"",    // whitespace + newline preserved
        "taSet:\"NEW\"",         // setting .value dirties it
        "splice:\"aXYdef\"",     // setRangeText edits the SELECTION, not the whole value
        "spliceSel:\"1-3\"",     // …and `preserve` leaves the range wrapping the insertion
        "inValue:\"Hello\"",     // input still reads its value attribute
    ] {
        assert!(
            got.contains(claim),
            "G_TEXTAREA_VALUE: expected {claim} in {got:?}\n  \
             textarea.value must be its text content (until dirtied), and setRangeText must splice into \
             the selection — reading the value attribute returns empty and corrupts the field."
        );
    }
}
