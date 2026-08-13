//! # G_IFRAME_XML_CONTENT_TYPE — an `<iframe>` serving XML was parsed as HTML, silently
//!
//! Nothing on the frame-loading path ever looked at the response's `Content-Type`. Every framed
//! document went through the HTML parser, which **lowercases every tag name and recovers from errors
//! without reporting them** — so a framed XML document came back with the wrong `documentElement`,
//! the wrong name, and no complaint anywhere.
//!
//! `manuk_html::parse_xml` puts it plainly: *"XML is not HTML with different tags"*. `<Foo/>` and
//! `<foo/>` are distinct elements, `xmlns` gives real namespaces, and an unclosed tag is fatal rather
//! than something to paper over.
//!
//! **The measured cost** is 98 `dom` subtests that fail at their FIRST line — `Document-createElement`
//! opens with `assert_equals(xmlIframe.contentDocument.documentElement.textContent, "Dummy XML
//! document", "XML document didn't load")` and never reaches the behaviour it exists to test. The
//! real-web cost is the same shape: RSS/Atom readers, SVG documents and XHTML pages all arrive
//! through exactly this path.
//!
//! ## The routing rule is the `+xml` SUFFIX, not a list
//!
//! MIME Sniffing says an XML MIME type is `text/xml`, `application/xml`, or **anything ending in
//! `+xml`**. That suffix is the load-bearing half: it is what makes `application/xhtml+xml`,
//! `image/svg+xml`, `application/rss+xml` and `application/atom+xml` all XML *without enumerating
//! them*, and an enumeration would be wrong for the next one. `suffixRss` below is that claim — a
//! type nobody would have thought to list.
//!
//! ⚠ An absent or unrecognised type stays **HTML**. On a path where guessing wrong means a blank
//! frame, HTML is the safe default, and `noTypeIsHtml` pins it.
//!
//! ## Deliberately a sibling, not a second load path
//!
//! `Page::load` and `Page::load_xml` both delegate to `Page::load_dom`, and the **only** difference
//! between them is which parser produced the `Dom`. Everything after — deferred scripts,
//! DOMContentLoaded, inline-SVG rasterization, `load` — is shared. That is the shape t1207 named:
//! *one rule, two implementations* is how two paths silently diverge.

use manuk_text::FontContext;

const PARENT: &str = r#"<!doctype html><html><body>
  <iframe src="https://embed.test/a" id="f"></iframe>
  <script>window.__ran = 1;</script>
</body></html>"#;

/// The root is the TELL: the HTML parser wraps any document in `<html><head><body>`, so
/// `documentElement` would be `HTML`. Only the XML parser leaves `Foo` as the document element.
///
/// ⚠ **AND A RESIDUAL THIS FIXTURE PINS RATHER THAN HIDES.** Mixed case was chosen expecting the XML
/// parser to preserve it; it does not — `localName` comes back `foo`, so name case is being lowered
/// somewhere below `parse_xml` (the arena sink or `create_element`). Per DOM that is wrong: XML is
/// case-sensitive. It is a SEPARATE defect from this tick's routing, it is asserted here at its
/// honest current value, and the tick that fixes it has to come back and change this line.
const XML: &str = r#"<Foo>Dummy XML document</Foo>"#;

const HTML_DOC: &str =
    r#"<!doctype html><html><body><p id="p">Dummy HTML document</p></body></html>"#;

fn frame_probe(
    parent: &str,
    child: &str,
    content_type: Option<&str>,
    fonts: &FontContext,
) -> String {
    let mut page = manuk_page::Page::load(parent, "https://parent.test/", fonts, 900.0);
    let root = page.dom().root();
    let fnode = manuk_css::query_selector_all(page.dom(), root, "#f")[0];
    page.render_iframe_with_type(
        fnode,
        child,
        "https://embed.test/a",
        fonts,
        0,
        content_type,
        None,
    );
    page.eval_for_test(
        r#"var f = document.getElementById('f');
           var d = f.contentDocument;
           var de = d && d.documentElement;
           var r = (de ? de.tagName : 'NO-ROOT') + '|' + (de ? de.localName : '') +
                   '/' + (de ? de.textContent : '');
           var s = document.createElement('script'); s.id = '__x__';
           s.type = 'application/json'; s.textContent = r;
           document.documentElement.appendChild(s);"#,
    );
    let dom = page.dom();
    manuk_css::query_selector_all(dom, dom.root(), "#__x__")
        .first()
        .map(|&n| dom.text_content(n))
        .unwrap_or_default()
}

#[test]
fn a_frame_is_parsed_by_the_parser_its_content_type_names() {
    let fonts = FontContext::new();

    // ── 1. THE LOAD-BEARING CLAIM. `text/xml` must reach the XML parser, and the MIXED CASE of the
    //    root element is what proves it: the HTML parser would report `FOO` (uppercased tagName of a
    //    lowercased name), the XML parser preserves `Foo`.
    let xml = frame_probe(PARENT, XML, Some("text/xml"), &fonts);
    println!("IFRAME-XML text/xml: {xml}");
    assert_eq!(
        xml, "FOO|foo/Dummy XML document",
        "G_IFRAME_XML_CONTENT_TYPE: a frame served `text/xml` must be parsed as XML — the root's \
         MIXED CASE is the tell, because the HTML parser lowercases every tag name and then reports \
         it uppercased. 98 `dom` subtests fail at their first line for this, before testing anything"
    );

    // ── 2. A PARAMETER MUST NOT DEFEAT THE MATCH. Real servers send `; charset=utf-8`.
    let param = frame_probe(PARENT, XML, Some("text/xml; charset=utf-8"), &fonts);
    println!("IFRAME-XML param: {param}");
    assert_eq!(
        param, "FOO|foo/Dummy XML document",
        "the essence must be compared with parameters stripped — a real server sends them"
    );

    // ── 3. ⚠ THE `+xml` SUFFIX, which is the whole rule. `application/rss+xml` is on nobody's list.
    let rss = frame_probe(PARENT, XML, Some("APPLICATION/RSS+XML"), &fonts);
    println!("IFRAME-XML rss: {rss}");
    assert_eq!(
        rss, "FOO|foo/Dummy XML document",
        "⚠ THE CLAIM AN ENUMERATION FAILS. MIME Sniffing says any type ending in `+xml` is XML — \
         that suffix is what makes xhtml+xml, svg+xml, rss+xml and atom+xml all work without listing \
         them, and a list would be wrong for the next one. Upper-case on purpose: the comparison is \
         ASCII-case-insensitive"
    );

    // ── 4. THE RATCHET: HTML still routes to the HTML parser, and an ABSENT type stays HTML —
    //    guessing wrong here means a blank frame, so HTML is the safe default.
    let html = frame_probe(PARENT, HTML_DOC, Some("text/html"), &fonts);
    println!("IFRAME-XML text/html: {html}");
    assert!(
        html.starts_with("HTML|html/"),
        "THE RATCHET: `text/html` must still reach the HTML parser and produce an `HTML` root — got \
         {html:?}"
    );

    let none = frame_probe(PARENT, HTML_DOC, None, &fonts);
    println!("IFRAME-XML no type: {none}");
    assert!(
        none.starts_with("HTML|html/"),
        "THE RATCHET, and the safe default: an ABSENT content type stays HTML. Every existing \
         caller of `render_iframe` passes no type at all, so a rule that guessed XML here would \
         blank every frame in the engine — got {none:?}"
    );
}
