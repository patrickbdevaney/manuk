//! **G_ELEMENT_TEXT — the `.text` property for `<a>` / `<script>` / `<title>` (raw text content).**
//!
//! A small family of elements expose their text content under `.text`, and (apart from `<option>`, gated
//! at tick 439) all were dead expandos: `a.text` (link label), `script.text` (inline script source) and
//! `title.text` (page title) returned `undefined`, and assigning to them left the content untouched. Each
//! claim is a way this goes RED:
//!
//!   * `<a>` / `<script>` / `<title>` `.text` is the RAW text content (whitespace preserved).
//!   * setting `.text` on one of them replaces the text content.
//!   * `<option>.text` stays whitespace-collapsed (tick 439 unchanged); a plain `<div>` keeps its expando.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body>
<a id="a" href="/x">Click  here</a>
<script id="s" type="application/json">{"k": 1}</script>
<title id="t">My  Title</title>
<option id="o">  Opt  Label </option>
<div id="dv">D</div>
<div id="out">-</div><script>
var r=[];
function k(n,v){ r.push(n+':'+JSON.stringify(v)); }
k('a', document.getElementById('a').text);      // "Click  here" (raw)
k('s', document.getElementById('s').text);      // {"k": 1}
k('t', document.getElementById('t').text);      // "My  Title"
k('opt', document.getElementById('o').text);    // "Opt Label" (collapsed, t439)
var a=document.getElementById('a'); a.text='new';
k('set', a.textContent);                         // "new"
var dv=document.getElementById('dv'); dv.text='EX';
k('div', dv.text);                               // "EX" (expando preserved)
document.getElementById('out').textContent=r.join(' ');
</script></body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn element_text_property_for_a_script_title() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://element-text.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        "a:\"Click  here\"",    // raw text content, whitespace preserved
        "s:\"{\\\"k\\\": 1}\"", // inline script source
        "t:\"My  Title\"",      // title text
        "opt:\"Opt Label\"",    // option stays collapsed (tick 439)
        "set:\"new\"",          // setting .text replaces the content
        "div:\"EX\"",           // a plain element keeps its .text expando
    ] {
        assert!(
            got.contains(claim),
            "G_ELEMENT_TEXT: expected {claim} in {got:?}\n  \
             a.text / script.text / title.text must be the raw text content (read and settable), without \
             changing option.text (collapsed) or the plain-element .text expando."
        );
    }
}
