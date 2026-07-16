//! **G_GET_BY_TAG_NS — `Element` / `Document` `.getElementsByTagNameNS(namespace, localName)`.**
//!
//! It was **`undefined`** on both element and document — `getElementsByTagNameNS is not a function`, a
//! `TypeError` that takes down every SVG/MathML/foreign-content tool that enumerates by namespace. This is
//! the namespace-aware sibling of `getElementsByTagName`, and it is more than a filter: the branches this
//! gate pins, each a real spec rule (DOM §Document/Element `getElementsByTagNameNS`):
//!
//!   * **`"*"` is a wildcard** in either position — any namespace, any local name.
//!   * **local name is the post-prefix part**, matched exactly as `element.localName`: an HTML element's
//!     tag is ASCII-lowercased, a foreign element keeps its case, and `createElementNS("test","test:body")`
//!     has local name `"body"`. So `("test","BODY")` and `("test","body")` are different queries.
//!   * **an HTML element is in the XHTML namespace** — `(XHTML, "div")` finds the page's divs, and a
//!     `null`/`""` namespace query does *not* (those elements are not in the null namespace).
//!   * **the result is a live `HTMLCollection`** (not a `NodeList`): appending a matching element grows its
//!     `length`, removing one shrinks it — the property that keeps `while (c.length) …` from spinning.
//!
//! Ported from WPT `dom/nodes/Document-Element-getElementsByTagNameNS.js`. (The one edge this deliberately
//! does not serve — a *genuinely* empty-string-namespace element, which stores as `None` and is thus
//! indistinguishable from XHTML — is documented on the native and is not asserted here.)
//!
//! **Falsifiable:** before the native existed the very first `getElementsByTagNameNS` call threw
//! `TypeError`, the `try` bailed to `THREW:…`, and no `label:OK` token was ever written — every assert
//! below RED. The full method turns them GREEN.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<div id="host"><p><a></a><b></b></p></div>
<script>
  var R = [];
  const XHTML = 'http://www.w3.org/1999/xhtml';
  function ck(label, got, want) { R.push(label + ':' + (got === want ? 'OK' : ('BAD(' + got + ')'))); }
  try {
    var host = document.getElementById('host');

    // An HTML element is in the XHTML namespace; the query finds it by (XHTML, localName).
    ck('xhtml_a', host.getElementsByTagNameNS(XHTML, 'a').length, 1);
    ck('xhtml_p', host.getElementsByTagNameNS(XHTML, 'p').length, 1);
    // Wildcard namespace, specific local name.
    ck('star_b', host.getElementsByTagNameNS('*', 'b').length, 1);
    // Wildcard both → every descendant element (p, a, b).
    ck('star_star', host.getElementsByTagNameNS('*', '*').length, 3);

    // The collection is a live HTMLCollection, not a NodeList.
    var col = host.getElementsByTagNameNS(XHTML, 'a');
    ck('is_htmlcol', col instanceof HTMLCollection, true);
    ck('not_nodelist', col instanceof NodeList, false);

    // Foreign namespace + case sensitivity: BODY matches BODY, not body.
    host.appendChild(document.createElementNS('test', 'BODY'));
    ck('test_BODY', host.getElementsByTagNameNS('test', 'BODY').length, 1);
    ck('test_body_none', host.getElementsByTagNameNS('test', 'body').length, 0);

    // Prefixed foreign element: the local name is the part after the colon.
    host.appendChild(document.createElementNS('test', 'test:body'));
    ck('prefix_body', host.getElementsByTagNameNS('test', 'body').length, 1);

    // A null-namespace query does NOT match the page's (XHTML-namespace) HTML elements.
    ck('null_div', document.getElementsByTagNameNS(null, 'div').length, 0);

    // Live collection: length tracks tree mutation.
    var liveCol = host.getElementsByTagNameNS('test', 'abc');
    ck('live_0', liveCol.length, 0);
    var a1 = host.appendChild(document.createElementNS('test', 'abc'));
    ck('live_1', liveCol.length, 1);
    host.removeChild(a1);
    ck('live_back0', liveCol.length, 0);

    // Works document-scoped too (the two page divs #out and #host).
    ck('doc_divs', document.getElementsByTagNameNS(XHTML, 'div').length, 2);
  } catch (e) { R.push('THREW:' + e); }
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn get_elements_by_tag_name_ns_matches_namespace_and_is_live() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://ns.test/gebtn/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for label in [
        "xhtml_a",
        "xhtml_p",
        "star_b",
        "star_star",
        "is_htmlcol",
        "not_nodelist",
        "test_BODY",
        "test_body_none",
        "prefix_body",
        "null_div",
        "live_0",
        "live_1",
        "live_back0",
        "doc_divs",
    ] {
        assert!(
            got.contains(&format!("{label}:OK")),
            "G_GET_BY_TAG_NS: `{label}` did not match the getElementsByTagNameNS algorithm\n  got: {got}"
        );
    }
}
