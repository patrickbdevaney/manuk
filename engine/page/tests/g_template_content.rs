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
//!
//! ⚠⚠⚠ **AND THE `imp:` CLAIM ABOVE WAS SATISFIED BY THE ONE ORDERING THAT HAPPENED TO WORK (t882).**
//! `innerHTML` wrote to the ELEMENT'S CHILD LIST — DOM Parsing redirects it to the template CONTENTS
//! — and that survived only because `Dom::template_content` materialises the fragment **lazily and
//! once**, moving the direct children in on first access. *Set, then read* therefore worked; every
//! other order did not, and a template's own child list is **always empty** in a real browser.
//! Measured against Chrome: `.content` read BEFORE the write `1` vs **`0`** (`.childNodes` **1** vs
//! 0); a SECOND write of two nodes `2` vs **`1`** — the first write's node; `t.innerHTML =
//! t.innerHTML` **kept** vs **ERASED**, because the getter walked the child list too. Vue 3 keeps ONE
//! module-level template and writes it per static block, so `pt88.app` died on *"can't access
//! property firstChild, l is null"* inside an async render. The four claims below are the orderings
//! `imp:` is structurally incapable of seeing.
//!
//! **A lazily-materialised cache turns an ORDERING bug into a bug only one ordering can see** — so a
//! gate over a lazy accessor must write the state in EVERY order, not the order the implementation
//! happened to make work.

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
    // ⚠⚠⚠ innerHTML TARGETS THE TEMPLATE CONTENTS, NOT THE CHILD LIST — the three orderings that
    // separate "it happened to work" from "it is implemented". `imp:` above only ever exercised the
    // first one.
    //   (1) .content read BEFORE the write. A lazy fragment materialised empty and then never saw
    //       the write, because the write went to the element's child list.
    var pre = document.createElement('template');
    var cached = pre.content;                      // materialise it FIRST
    pre.innerHTML = '<b>z</b>';
    r.push('pre:' + cached.childNodes.length + '/' + pre.content.childNodes.length);
    //   (2) A template's own child list is ALWAYS empty in a browser.
    r.push('kids:' + pre.childNodes.length);
    //   (3) A SECOND write replaces the content. Vue 3 keeps ONE module-level template and writes it
    //       per static block, so this is the ordering every Vue page depends on.
    pre.innerHTML = '<i>1</i><i>2</i>';
    r.push('twice:' + pre.content.childNodes.length);
    //   (4) The getter reads the contents, so `t.innerHTML = t.innerHTML` is not an ERASER.
    r.push('rt:' + (pre.innerHTML.indexOf('<i>1</i>') === 0));
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
        // The three orderings `imp:` cannot see — innerHTML must target the template CONTENTS.
        "pre:1/1", // .content read BEFORE the write still receives it (was 0/0)
        "kids:0",  // the template element's own child list stays empty (was 1)
        "twice:2", // a SECOND write replaces the contents (was 1 — the FIRST write's node)
        "rt:true", // the getter reads the contents, so a round-trip is not an eraser
    ] {
        assert!(
            got.contains(claim),
            "G_TEMPLATE_CONTENT: expected {claim} in {got:?}\n  \
             A parsed <template>'s .content must hold its children — an empty .content makes every \
             framework that clones it (lit-html, Svelte, Solid, Vue) render nothing, silently."
        );
    }
}
