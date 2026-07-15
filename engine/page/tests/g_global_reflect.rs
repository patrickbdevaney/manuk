//! **G_GLOBAL_REFLECT — the GLOBAL HTMLElement reflected attributes, on every element.**
//!
//! `dir`, `hidden`, `tabIndex`, `accessKey`, … are reflected by *every* HTMLElement, but the per-tag
//! reflection table only carried per-element attributes — so `el.dir` / `el.hidden` / `el.tabIndex`
//! returned `undefined` on a plain `<div>`. A `"*"` (global) row in the table, dispatched as a fallback
//! in `reflect_js`, applies them everywhere. This was the single largest reflection hole (+18k html/dom).

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<div id="d" dir="ltr" hidden tabindex="5" accesskey="k"></div>
<span id="s"></span>
<script>
  var R = [], d = document.getElementById('d'), s = document.getElementById('s');
  R.push('dir:' + d.dir);                                   // ltr
  R.push('hidden:' + d.hidden);                             // true (boolean reflection)
  R.push('tab:' + d.tabIndex);                              // 5 (long)
  R.push('ak:' + d.accessKey);                              // k
  s.dir = 'rtl';       R.push('setDir:' + s.getAttribute('dir'));        // rtl — setter reflects to attr
  s.hidden = true;     R.push('setHidden:' + s.hasAttribute('hidden'));  // true
  R.push('getBack:' + s.hidden);                                         // true — round-trips
  R.push('inert:' + (d.disabled === undefined));            // true — a tag-specific attr stays inert on div
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn global_htmlelement_attributes_reflect_on_every_element() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://gr.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("dir:ltr", "the global `dir` attribute reflects on a plain div, not undefined"),
        ("hidden:true", "`hidden` reflects as a boolean"),
        ("tab:5", "`tabIndex` reflects as a long"),
        ("ak:k", "`accessKey` reflects as a string"),
        ("setDir:rtl", "the setter writes through to the content attribute"),
        ("setHidden:true", "boolean setter adds the attribute"),
        ("getBack:true", "…and the getter round-trips it"),
        (
            "inert:true",
            "a tag-specific attribute (div.disabled) stays inert — the global fallback must not clobber it",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_GLOBAL_REFLECT: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
