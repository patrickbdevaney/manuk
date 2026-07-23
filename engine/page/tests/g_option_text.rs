//! **G_OPTION_TEXT — `option.text` + the `Option()` constructor's `defaultSelected` argument.**
//!
//! `select.options[i].text` — the single most common way a page recovers the LABEL of the chosen option —
//! was `undefined` (a plain expando: assigning to it left the text content untouched). And
//! `new Option(text, value, defaultSelected)` ignored its 3rd argument, so a constructed pre-selected
//! option came back unselected. Each claim is a way this goes RED:
//!
//!   * `option.text` collapses ASCII-whitespace runs and trims (spec) — `"  Hello   World  "` → `"Hello World"`.
//!   * setting `option.text` replaces the text content.
//!   * `new Option('t','v',true)` is selected and `defaultSelected`; `new Option(text)` alone lets `value`
//!     fall back to the text.
//!   * the fix must NOT eat the ordinary `div.text = x` expando on a non-option element.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body>
<select id="sel"><option id="o1">  Hello   World  </option></select>
<div id="d">D</div>
<div id="out">-</div><script>
var r=[];
function t(k,v){ r.push(k+':'+v); }
var o1=document.getElementById('o1');
t('getText', JSON.stringify(o1.text));
o1.text='New Label';
t('setText', o1.textContent+'|'+JSON.stringify(o1.text));
var c=new Option('Txt','val',true);
t('ctor', c.text+'/'+c.value+'/'+c.selected+'/'+c.defaultSelected);
var c2=new Option('Only');
t('ctorFallback', c2.text+'/'+c2.value+'/'+c2.selected);
// a plain <option> reads its own label; a <div> has no .text until assigned (Chrome parity)
var d=document.getElementById('d');
t('divBefore', d.text===undefined?'undefined':String(d.text));
d.text='EXPANDO';
t('divAfter', d.text);
document.getElementById('out').textContent=r.join(' ');
</script></body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn option_text_and_constructor_default_selected() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://option-text.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        "getText:\"Hello World\"",         // collapse + trim
        "setText:New Label|\"New Label\"", // setter replaces content
        "ctor:Txt/val/true/true", // defaultSelected arg honoured → selected + defaultSelected
        "ctorFallback:Only/Only/false", // value falls back to text; not selected
        "divBefore:undefined",    // a <div> has no .text
        "divAfter:EXPANDO",       // ...but the ordinary expando still works
    ] {
        assert!(
            got.contains(claim),
            "G_OPTION_TEXT: expected {claim} in {got:?}\n  \
             option.text must return the collapsed/trimmed label and be settable, and \
             new Option(text,value,defaultSelected) must honour defaultSelected, without eating the \
             ordinary .text expando on non-option elements."
        );
    }
}
