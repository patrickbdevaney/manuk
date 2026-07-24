//! **G_INPUTMODE_REFLECT — `inputMode` and `enterKeyHint` reflect on every element (they are GLOBAL).**
//!
//! `inputMode` and `enterKeyHint` are IDL attributes of **`HTMLElement`** — global, valid on every
//! element, exactly like `dir`, `hidden`, `tabIndex`, `accessKey` and `lang` (which `g_global_reflect`
//! already covers). They are how a page steers the on-screen keyboard: `inputmode="numeric"` brings up a
//! digit pad, `inputmode="decimal"` a decimal pad, `enterkeyhint="search"` relabels the Enter key. Every
//! serious mobile form and every `contenteditable` custom field sets them.
//!
//! They were **dead**: the reflection table keyed both under a bogus tag name `"undefinedelement"` — a
//! string that matches no element — instead of the `"*"` global bucket that the reflection mechanism
//! actually applies to every element. So `input.inputMode` / `el.enterKeyHint` read back `undefined`
//! (the property did not exist at all), and setting `el.inputMode = 'tel'` threw or no-opped. This is the
//! same defect class the global-reflection table was created to fix: a global HTMLElement attribute that
//! never reached the `*` bucket.
//!
//! Four things to prove — and the enum handling (last two) is what makes it *spec* reflection rather than
//! a raw string passthrough:
//!
//! 1. a present, valid value reads back (`inputmode="numeric"` → `"numeric"`; `enterkeyhint="go"` → `"go"`);
//! 2. **absent** reads back `""` — a limited-to-known-values enum with no attribute is the empty string,
//!    not `undefined` and not some default keyword;
//! 3. setting the IDL property writes the lowercase content attribute and round-trips;
//! 4. an **invalid** value reads back `""` — "limited to only known values" means an unknown token is
//!    dropped to the empty string, not echoed.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<input id="inp" type="text" inputmode="numeric" enterkeyhint="go">
<div id="bare"></div>
<div id="out">-</div>
<script>
  var R = [];
  var inp = document.getElementById('inp');
  R.push('im:' + inp.inputMode);          // numeric — reflects on <input>
  R.push('ekh:' + inp.enterKeyHint);      // go
  var b = document.getElementById('bare');
  R.push('bareim:' + (b.inputMode === '' ? 'EMPTY' : JSON.stringify(b.inputMode))); // absent -> ""
  R.push('ekhGlobal:' + (typeof b.enterKeyHint));  // 'string' — it exists on a <div> too (global)
  b.inputMode = 'tel';
  R.push('attr:' + b.getAttribute('inputmode'));   // tel — property write hits the content attribute
  R.push('back:' + b.inputMode);                    // tel — round-trips
  b.inputMode = 'BOGUS';
  R.push('bad:' + (b.inputMode === '' ? 'EMPTY' : b.inputMode));  // invalid -> ""
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn a_inputmode_and_enterkeyhint_reflect_as_global_html_element_attributes() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://inputmode.test/", &fonts, 400.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "im:numeric",
            "`inputMode` must reflect the `inputmode` content attribute. It was keyed under the bogus \
             tag `undefinedelement` (matches no element) instead of the `*` global bucket, so the \
             property did not exist and read `undefined`",
        ),
        ("ekh:go", "`enterKeyHint` reflects `enterkeyhint` — same bug, same fix"),
        (
            "bareim:EMPTY",
            "an ABSENT limited-enum attribute reflects as the empty string, not `undefined` — proving \
             the property now exists on an element that never carried the attribute",
        ),
        (
            "ekhGlobal:string",
            "`enterKeyHint` exists on a plain `<div>` too: it is a GLOBAL HTMLElement attribute, so it \
             must live in the `*` bucket that applies to every element, not on one tag",
        ),
        (
            "attr:tel",
            "setting the IDL property `el.inputMode = 'tel'` must write the lowercase `inputmode` \
             content attribute",
        ),
        ("back:tel", "…and read back the same value — the set round-trips"),
        (
            "bad:EMPTY",
            "an INVALID value reflects as `\"\"` — `inputMode` is limited to only known values, so an \
             unknown token is dropped to the empty string rather than echoed verbatim",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_INPUTMODE_REFLECT: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
