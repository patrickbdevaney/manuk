//! **G_TEMPLATE_CONTENT — a parsed `<template>`'s `.content` must hold its children, not be empty.**
//!
//! `<template>.content` is the fast path every compiler-based framework instantiates DOM through:
//! lit-html, Svelte, Solid and Vue's compiled render functions parse a `<template>` once and then
//! `template.content.cloneNode(true)` (or `.content.firstChild.cloneNode(true)`) per instance, because
//! cloning a parsed subtree is far cheaper than rebuilding it. The element already existed and `.content`
//! already returned a fragment — but for a PARSED template that fragment was **empty**, because the HTML
//! parser puts a template's children in its `template_contents` fragment (per the tree-construction
//! rules), while the accessor built a fresh fragment from the template's own (therefore empty) direct
//! children. So `template.content.querySelector(...)` was `null`, `.content.cloneNode(true)` cloned
//! nothing, and the framework rendered an empty component with no error.
//!
//! The claims check the fragment's actual contents, each a way the empty-fragment bug goes RED:
//!
//!   * **`.content.childNodes`** holds the parsed children (a `<div>` and a `<span>`).
//!   * **`.content.querySelector('.x')`** finds a node inside the fragment.
//!   * **`.content.cloneNode(true)`** appended into the live tree brings BOTH children with it.
//!   * **An imperatively-built template** (`createElement` + `innerHTML`) still exposes its content.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body><template id="tpl"><div class="x">hi</div><span>s</span></template><div id="host"></div><div id="out">-</div><script>
    var r = [];
    var tp = document.getElementById('tpl');
    // The parsed children live in the content fragment (NOT as direct children of <template>).
    r.push('cn:' + tp.content.childNodes.length);
    var x = tp.content.querySelector('.x');
    r.push('qs:' + (x ? x.textContent : 'null'));
    r.push('fec:' + (tp.content.firstElementChild ? tp.content.firstElementChild.className : 'null'));
    // Clone the fragment into the live tree — the whole point of a template.
    document.getElementById('host').appendChild(tp.content.cloneNode(true));
    var hx = document.querySelector('#host .x'), hs = document.querySelector('#host span');
    r.push('clone:' + (hx ? hx.textContent : 'null') + '/' + (hs ? hs.textContent : 'null'));
    // An imperatively-built template exposes its content too.
    var it = document.createElement('template');
    it.innerHTML = '<b>y</b>';
    var b = it.content.querySelector('b');
    r.push('imp:' + (b ? b.textContent : 'null'));
    document.getElementById('out').textContent = r.join(' ');
  </script></body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn template_content_holds_the_parsed_children() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://template-content.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        "cn:2",       // the content fragment holds the two parsed children
        "qs:hi",      // querySelector reaches into the fragment
        "fec:x",      // firstElementChild is the parsed <div class=x>
        "clone:hi/s", // cloneNode(true) brings BOTH children into the live tree
        "imp:y",      // an imperatively-built template also exposes its content
    ] {
        assert!(
            got.contains(claim),
            "G_TEMPLATE_CONTENT: expected {claim} in {got:?}\n  \
             A parsed <template>'s .content must hold its children — an empty .content makes every \
             framework that clones it (lit-html, Svelte, Solid, Vue) render nothing, silently."
        );
    }
}
