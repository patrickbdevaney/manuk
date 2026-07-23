//! **G_INERT — the `inert` IDL attribute reflects the `inert` content attribute (boolean).**
//!
//! `HTMLElement.inert` is a boolean reflecting the `inert` content attribute. It was absent — reading
//! `el.inert` on any element answered `undefined`, and assigning `el.inert = true` (the canonical way a
//! modal library neutralises the page: `document.body.inert = true`) neither added the attribute nor
//! took effect. `undefined` is not `false`: a library that branches on `if (el.inert)` and a polyfill
//! that feature-detects `'inert' in el` both misread the platform. The fix adds `inert` to the global
//! (`"*"`) reflection row so every element gets the getter/setter. Each claim is a way this goes RED:
//!
//!   * an element with no `inert` attribute reads `el.inert === false` (NOT `undefined`).
//!   * an element carrying the attribute reads `el.inert === true`.
//!   * assigning `el.inert = true` adds the content attribute (`getAttribute('inert') === ""`).
//!   * assigning `el.inert = false` removes it (`hasAttribute('inert') === false`).

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<div id="off">off</div>
<div id="on" inert>on</div>
<div id="out">-</div>
<script>
  var R = [];
  var off = document.getElementById('off'), on = document.getElementById('on');
  R.push('offInit:' + off.inert);                 // false — an unset boolean is false, never undefined
  R.push('onInit:' + on.inert);                   // true  — reflects the present attribute
  off.inert = true;
  R.push('setAttr:' + (off.getAttribute('inert') === '' ? 'ok' : 'no'));  // assigning adds the attribute
  R.push('setIdl:' + off.inert);                  // true after assignment
  on.inert = false;
  R.push('clear:' + (on.hasAttribute('inert') ? 'no' : 'ok'));            // clearing removes the attribute
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn inert_idl_attribute_reflects() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://inert.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "offInit:false",
            "an element with no `inert` attribute must read `el.inert === false` — `undefined` is the \
             bug (a library's `if (el.inert)` and a `'inert' in el` polyfill check both misfire)",
        ),
        ("onInit:true", "an element carrying the `inert` attribute must read `el.inert === true`"),
        (
            "setAttr:ok",
            "assigning `el.inert = true` must add the content attribute — this is how a modal library \
             neutralises the page (`document.body.inert = true`)",
        ),
        ("setIdl:true", "the IDL value must be true after assignment"),
        ("clear:ok", "assigning `el.inert = false` must remove the content attribute"),
    ] {
        assert!(
            got.contains(claim),
            "G_INERT: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
