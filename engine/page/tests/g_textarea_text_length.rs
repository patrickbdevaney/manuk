//! **G_TEXTAREA_TEXT_LENGTH — `<textarea>.textLength` is the code-unit length of the value.**
//!
//! `textLength` is the read-only IDL attribute a character counter reads on every keystroke to render
//! "120 / 280". It was `undefined`, so a counter got `undefined` (rendering "undefined / 280", or NaN'ing
//! any arithmetic against a `maxlength`). It is exactly `value.length` — `<textarea>.value` already returns
//! the control's current text — so it costs no new state.
//!
//! Three things to prove:
//! 1. it equals the length of the initial value (the text content of the `<textarea>`);
//! 2. it tracks the value after a script assignment (it is not a stale snapshot);
//! 3. it is read-only and TEXTAREA-only — an `<input>` (which has `value` but no `textLength`) reads
//!    `undefined`, so the property is not a blanket addition to every control.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<textarea id="ta">hello</textarea>
<input id="in" value="world">
<div id="out">-</div>
<script>
  var R = [];
  var ta = document.getElementById('ta');
  R.push('init:' + ta.textLength);            // 5 — length of "hello"
  ta.value = 'twelve chars';                   // 12
  R.push('afterSet:' + ta.textLength);
  // read-only: assignment ignored
  try { ta.textLength = 999; } catch (e) {}
  R.push('ro:' + ta.textLength);               // still 12
  // input has value but NOT textLength (textarea-only)
  R.push('input:' + (document.getElementById('in').textLength === undefined ? 'UNDEF' : document.getElementById('in').textLength));
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn a_textarea_text_length_is_the_value_length_and_textarea_only() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://ta.test/", &fonts, 400.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("init:5", "textLength must equal the length of the initial value (`hello` → 5), not `undefined`"),
        (
            "afterSet:12",
            "it tracks the value after a script assignment — it reads `value.length` live, it is not a \
             stale snapshot",
        ),
        ("ro:12", "textLength is read-only — assigning to it is ignored"),
        (
            "input:UNDEF",
            "an <input> has `value` but NO `textLength` (it is HTMLTextAreaElement-only), so the property \
             must read `undefined` there rather than being a blanket addition to every control",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_TEXTAREA_TEXT_LENGTH: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
