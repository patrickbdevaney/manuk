//! **G_NAMESPACE_LOOKUP — `Node.lookupNamespaceURI` / `Node.isDefaultNamespace`.**
//!
//! Both were **`undefined`** on every node — `node.lookupNamespaceURI is not a function`, a `TypeError`
//! that kills whatever script reached for it. They implement the DOM "locate a namespace" algorithm
//! (DOM §Node), which is more than a field read; the branches this gate pins, each a real spec subtlety:
//!
//!   * The `xml` / `xmlns` prefixes are **always** bound on an element and cannot be overridden — even
//!     after `setAttributeNS(XMLNS_NS, 'xmlns', ...)`, `lookupNamespaceURI('xmlns')` is still `XMLNS_NS`.
//!   * An HTML element stores no namespace but **is** in the XHTML namespace with a null prefix, so the
//!     element's own namespace wins over an `xmlns` attribute it carries.
//!   * "Parent element" means the parent *iff it is an element* — a comment whose parent is the document
//!     resolves to `null`, it does not climb to the document element.
//!   * A DocumentFragment / DocumentType has no namespace and no element to climb to → always `null`.
//!
//! Ported from WPT `dom/nodes/Node-lookupNamespaceURI.html`.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<script>
  var R = [];
  const XMLNS_NS = 'http://www.w3.org/2000/xmlns/';
  const XML_NS = 'http://www.w3.org/XML/1998/namespace';
  function ck(label, got, want) { R.push(label + ':' + (got === want ? 'OK' : ('BAD(' + got + ')'))); }
  try {
    // A DocumentFragment has no namespace and no element to climb to.
    var frag = document.createDocumentFragment();
    ck('frag_null', frag.lookupNamespaceURI(null), null);
    ck('frag_xml', frag.lookupNamespaceURI('xml'), null);           // NOT XML_NS: not an element
    ck('frag_def_null', frag.isDefaultNamespace(null), true);
    ck('frag_def_foo', frag.isDefaultNamespace('foo'), false);

    // A namespaced element: own prefix resolves; xml/xmlns are hard-wired.
    var foo = document.createElementNS('fooNamespace', 'prefix:elem');
    ck('foo_ownprefix', foo.lookupNamespaceURI('prefix'), 'fooNamespace');
    ck('foo_nullpre', foo.lookupNamespaceURI(null), null);          // ns is fooNamespace w/ prefix, not default
    ck('foo_empty', foo.lookupNamespaceURI(''), null);              // '' normalises to null
    ck('foo_xml', foo.lookupNamespaceURI('xml'), XML_NS);
    ck('foo_xmlns', foo.lookupNamespaceURI('xmlns'), XMLNS_NS);
    ck('foo_bynsval', foo.lookupNamespaceURI('fooNamespace'), null);// prefix != namespace-value

    // xmlns:* declarations resolve; the default xmlns overrides lookupNamespaceURI(null);
    // but 'xmlns' as a PREFIX stays hard-wired to XMLNS_NS.
    foo.setAttributeNS(XMLNS_NS, 'xmlns:bar', 'barURI');
    foo.setAttributeNS(XMLNS_NS, 'xmlns', 'bazURI');
    ck('foo_default_baz', foo.lookupNamespaceURI(null), 'bazURI');
    ck('foo_bar', foo.lookupNamespaceURI('bar'), 'barURI');
    ck('foo_xmlns_fixed', foo.lookupNamespaceURI('xmlns'), XMLNS_NS);
    ck('foo_def_baz', foo.isDefaultNamespace('bazURI'), true);
    ck('foo_def_bar', foo.isDefaultNamespace('barURI'), false);

    // A comment inherits its parent element's declarations.
    var c = document.createComment('c');
    foo.appendChild(c);
    ck('cmt_baz', c.lookupNamespaceURI(null), 'bazURI');
    ck('cmt_bar', c.lookupNamespaceURI('bar'), 'barURI');
    ck('cmt_def_baz', c.isDefaultNamespace('bazURI'), true);

    // A child element climbs to the parent for an inherited prefix.
    var child = document.createElementNS('childNamespace', 'childElem');
    foo.appendChild(child);
    ck('child_own', child.lookupNamespaceURI(null), 'childNamespace');
    ck('child_inherit', child.lookupNamespaceURI('prefix'), 'fooNamespace');
    ck('child_xmlns', child.lookupNamespaceURI('xmlns'), XMLNS_NS);

    // The document resolves via its document element; the element's OWN namespace (XHTML) wins over
    // an xmlns attribute set on <html>.
    document.documentElement.setAttributeNS(XMLNS_NS, 'xmlns:bar', 'barURI');
    document.documentElement.setAttributeNS(XMLNS_NS, 'xmlns', 'bazURI');
    ck('doc_xhtml', document.lookupNamespaceURI(null), 'http://www.w3.org/1999/xhtml');
    ck('doc_bar', document.lookupNamespaceURI('bar'), 'barURI');
    ck('doc_missprefix', document.lookupNamespaceURI('prefix'), null);
    ck('doc_def_xhtml', document.isDefaultNamespace('http://www.w3.org/1999/xhtml'), true);
    ck('doc_def_xmlns', document.isDefaultNamespace(XMLNS_NS), false);

    // A comment whose parent is the document does NOT climb to the document element.
    var c2 = document.createComment('c2');
    document.appendChild(c2);
    ck('cmt_doc_null', c2.lookupNamespaceURI('bar'), null);
  } catch (e) { R.push('THREW:' + e); }
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn node_lookup_namespace_uri_and_is_default_namespace() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://ns.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    // Every check must have reported OK — a single BAD(...) prints the offending actual value.
    for label in [
        "frag_null",
        "frag_xml",
        "frag_def_null",
        "frag_def_foo",
        "foo_ownprefix",
        "foo_nullpre",
        "foo_empty",
        "foo_xml",
        "foo_xmlns",
        "foo_bynsval",
        "foo_default_baz",
        "foo_bar",
        "foo_xmlns_fixed",
        "foo_def_baz",
        "foo_def_bar",
        "cmt_baz",
        "cmt_bar",
        "cmt_def_baz",
        "child_own",
        "child_inherit",
        "child_xmlns",
        "doc_xhtml",
        "doc_bar",
        "doc_missprefix",
        "doc_def_xhtml",
        "doc_def_xmlns",
        "cmt_doc_null",
    ] {
        assert!(
            got.contains(&format!("{label}:OK")),
            "G_NAMESPACE_LOOKUP: `{label}` did not resolve per the locate-a-namespace algorithm\n  got: {got}"
        );
    }
}
