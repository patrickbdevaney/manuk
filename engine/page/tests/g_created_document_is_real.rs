//! **G_CREATED_DOCUMENT_IS_REAL — a document from `DOMImplementation` is a real Document.**
//!
//! `document.implementation.createHTMLDocument()` returned a reflector built by `new_reflector`, which
//! gives every node `HTMLElement.prototype` (the element member set) — so the created Document had
//! `setAttribute` but none of the factory surface (`createElement`, `getElementById`, …), and every
//! `dom/nodes` test that called one got `TypeError: doc.createElement is not a function`.
//!
//! Two things make this safe as well as present, and this gate proves both go RED on revert:
//!   * **Document.prototype on the reflector** — the factory methods resolve at all.
//!   * **subtree-scoped `body`/`documentElement`** — a SECOND document in the same arena must resolve
//!     its OWN `<body>`, not the main page's. Without the scoping fix, `doc.body` aliased the page body
//!     (here carrying `#marker`), and a write through it corrupted the real document.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="marker">PAGE-BODY-CONTENT</div>
<div id="out">-</div>
<script>
  var R = [], push = function (k, v) { R.push(k + '=' + v); };
  var doc = document.implementation.createHTMLDocument('Hello');
  // The factory surface resolves (was `TypeError: ... is not a function`).
  push('createEl', typeof doc.createElement);                       // function
  push('tag', doc.createElement('DIV').tagName);                    // DIV
  push('createText', typeof doc.createTextNode);                    // function
  push('createComment', typeof doc.createComment);                  // function
  push('getById', typeof doc.getElementById);                       // function
  // The created document resolves its OWN structure.
  push('docEl', doc.documentElement && doc.documentElement.tagName);// HTML
  push('hasBody', !!doc.body);                                      // true
  push('title', doc.title);                                         // Hello
  // SAFETY: the created doc's body is NOT the page's body. The page body carries #marker; a leaked
  // alias would report a non-zero child count here.
  push('bodyEmpty', doc.body.childNodes.length);                    // 0
  push('bodyIsolated', doc.body === document.body);                 // false
  // Spec structure + metadata.
  push('kids', doc.childNodes.length);                              // 2  (doctype + html)
  push('compat', doc.compatMode);                                   // CSS1Compat
  push('ctype', doc.contentType);                                   // text/html
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn created_document_is_a_real_document() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://cd.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "createEl=function",
            "createElement resolves on the created document",
        ),
        ("tag=DIV", "the created document actually mints elements"),
        (
            "createText=function",
            "createTextNode resolves on the created document",
        ),
        (
            "createComment=function",
            "createComment resolves on the created document",
        ),
        (
            "getById=function",
            "getElementById resolves on the created document",
        ),
        (
            "docEl=HTML",
            "documentElement is the created doc's own <html>",
        ),
        ("hasBody=true", "the created doc has a <body>"),
        ("title=Hello", "the title argument reached the created doc"),
        (
            "bodyEmpty=0",
            "SAFETY: the created body is subtree-scoped, not the page body",
        ),
        (
            "bodyIsolated=false",
            "SAFETY: the created body is a distinct node from document.body",
        ),
        ("kids=2", "childNodes is [doctype, html]"),
        ("compat=CSS1Compat", "compatMode is standards mode"),
        ("ctype=text/html", "contentType is text/html"),
    ] {
        assert!(
            got.contains(claim),
            "G_CREATED_DOCUMENT_IS_REAL: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
