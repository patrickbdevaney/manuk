//! **G_NODE_NAME — `node.nodeName` is per node type, and case-sensitive outside the HTML namespace.**
//!
//! The getter uppercased **unconditionally** and returned `"#text"` for every non-element. Per DOM §Node,
//! an element's nodeName is its `tagName` — ASCII-uppercased ONLY in the HTML namespace — and every other
//! node type has its own name (`#comment`, `#document`, `#document-fragment`, the doctype's name).
//!
//! The failing mass was `Document-createElementNS.html`: `createElementNS('http://example.com/', 'foo')`
//! is a NON-HTML element whose nodeName must stay `"foo"`, not become `"FOO"`.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<span id="h1">x</span>
<script>
  var R = [];
  function ck(label, got, want) { R.push(label + ':' + (got === want ? 'OK' : ('BAD(' + got + ')'))); }
  try {
    // An HTML element: nodeName is ASCII-UPPERCASE.
    ck('html_upper', document.getElementById('h1').nodeName, 'SPAN');
    // Non-HTML-namespace elements keep their case (the whole createElementNS cluster).
    ck('ns_case', document.createElementNS('http://example.com/', 'foo').nodeName, 'foo');
    ck('ns_case_prefix', document.createElementNS('http://example.com/', 'a:Foo').nodeName, 'a:Foo');
    ck('xml_ns_case', document.createElementNS('http://www.w3.org/XML/1998/namespace', 'Bar').nodeName, 'Bar');
    ck('svg_case', document.createElementNS('http://www.w3.org/2000/svg', 'linearGradient').nodeName, 'linearGradient');
    // Other node types each have their own name.
    ck('text', document.createTextNode('t').nodeName, '#text');
    ck('comment', document.createComment('c').nodeName, '#comment');
    ck('fragment', document.createDocumentFragment().nodeName, '#document-fragment');
    ck('document', document.nodeName, '#document');
  } catch (e) { R.push('THREW:' + e); }
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn node_name_is_type_specific_and_namespace_cased() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://nn.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for label in [
        "html_upper",
        "ns_case",
        "ns_case_prefix",
        "xml_ns_case",
        "svg_case",
        "text",
        "comment",
        "fragment",
        "document",
    ] {
        assert!(
            got.contains(&format!("{label}:OK")),
            "G_NODE_NAME: `{label}` — nodeName is wrong for this node type\n  got: {got}"
        );
    }
}
