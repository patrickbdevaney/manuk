//! **G_LOOKUP_PREFIX — `Node.lookupPrefix` and the DocumentType namespace-lookup surface.**
//!
//! `node.lookupPrefix(namespace)` is DOM §Node's "locate a namespace prefix" — the inverse of
//! `lookupNamespaceURI`, and it was registered as a native on *no* node (its sibling
//! `lookupNamespaceURI`/`isDefaultNamespace` were). So every `foo.lookupPrefix(ns)` was a `TypeError`,
//! which is not exotic: namespace-aware code (SVG/MathML/XML tooling, XML serializers that must choose a
//! prefix) reaches for it. And on a **DocumentType** — a JS shim, not a reflector — none of the three
//! namespace-lookup methods existed at all, so `dom/nodes`' `Node-lookupNamespaceURI` failed at the first
//! call on a doctype.
//!
//! The spec answers for a detached/parented doctype are constant (a doctype has no element to climb to):
//! `lookupNamespaceURI`/`lookupPrefix` are `null`, `isDefaultNamespace` is true only for the null/empty
//! namespace. This gate pins the element algorithm (own-namespace prefix, xmlns declaration, and the
//! null/empty cases) and the doctype constants.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body><div id="out">-</div><script>
    var R = [];

    // An element whose own (namespace, prefix) resolves the prefix directly.
    var el = document.createElementNS('http://example.com/ns', 'x:foo');
    R.push('ownPrefix:' + el.lookupPrefix('http://example.com/ns'));   // 'x'
    R.push('ownNsUri:'  + el.lookupNamespaceURI('x'));                 // round-trips
    R.push('missPrefix:' + el.lookupPrefix('http://no.such/thing'));   // null (no mapping)
    R.push('emptyPrefix:' + el.lookupPrefix(''));                      // null ('' -> null per spec)
    R.push('nullPrefix:' + el.lookupPrefix(null));                     // null

    // A prefix bound by an xmlns:<p> declaration on an ancestor is found by the walk.
    var wrap = document.createElement('div');
    wrap.setAttribute('xmlns:p', 'http://declared.example/');
    var child = document.createElement('span');
    wrap.appendChild(child);
    R.push('declPrefix:' + child.lookupPrefix('http://declared.example/'));  // 'p'

    // A DocumentType has the whole namespace-lookup surface, with the spec's constant answers.
    var dt = document.implementation.createDocumentType('html', '', '');
    R.push('dtNsUri:' + dt.lookupNamespaceURI('p'));                    // null
    R.push('dtPrefix:' + dt.lookupPrefix('http://declared.example/'));  // null
    R.push('dtDefaultT:' + dt.isDefaultNamespace(''));                  // true (null/empty)
    R.push('dtDefaultF:' + dt.isDefaultNamespace('http://declared.example/')); // false

    document.getElementById('out').textContent = R.join(' ');
  </script></body></html>"#;

#[test]
fn lookup_prefix_and_doctype_namespace_surface() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://lookup.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        "ownPrefix:x",
        "ownNsUri:http://example.com/ns",
        "missPrefix:null",
        "emptyPrefix:null",
        "nullPrefix:null",
        "declPrefix:p",
        "dtNsUri:null",
        "dtPrefix:null",
        "dtDefaultT:true",
        "dtDefaultF:false",
    ] {
        assert!(
            got.contains(claim),
            "G_LOOKUP_PREFIX: expected `{claim}`\n  got: {got}\n\n  \
             lookupPrefix is DOM §Node's 'locate a namespace prefix' and was a TypeError on every node; \
             a DocumentType lacked the namespace-lookup surface entirely. Both are spec-required Node API."
        );
    }
}
