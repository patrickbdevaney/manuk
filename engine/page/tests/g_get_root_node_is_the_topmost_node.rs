//! **G_GET_ROOT_NODE_IS_THE_TOPMOST_NODE — `getRootNode()` returns the root of the node's own tree.**
//!
//! `document.getRootNode()` answered `null` and a detached `<p>`'s answered `document`. React 19 asks
//! `container.getRootNode()` to find its hoistable root; `null` there throws its error #446
//! ("resourceRoot was expected to exist"), which blanked six Next.js apps (REAL clones, W3.18.a).
//!
//! Hermetic: an inline document, the answers are written into the DOM by the page itself.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body><div id="a"><b id="c">x</b></div>
<pre id="out"></pre>
<script>
  var r = [
    document.getRootNode() === document,
    document.documentElement.getRootNode() === document,
    document.getElementById('c').getRootNode() === document,
    (function () { var p = document.createElement('p'); return p.getRootNode() === p; })(),
    (function () { var p = document.createElement('p'), s = document.createElement('i');
                   p.appendChild(s); return s.getRootNode() === p; })()
  ];
  document.getElementById('out').textContent = r.join(',');
</script></body></html>"##;

#[test]
fn get_root_node_returns_the_topmost_node_of_the_tree() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://example.invalid/index.html", &fonts, 800.0);
    let dom = page.dom();
    let n = manuk_css::query_selector_all(dom, dom.root(), "#out")[0];
    assert_eq!(
        dom.text_content(n),
        "true,true,true,true,true",
        "getRootNode: document/html/attached-child must give the document; a detached node and its \
         detached child must give the detached tree's top (not window.document)"
    );
}
