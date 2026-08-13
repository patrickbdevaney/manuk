//! # G_CREATE_DOCUMENT_VALIDATION — one rule, and it was written out in ONE of its two callers
//!
//! DOM specifies `createElementNS(ns, qname)` and `DOMImplementation.createDocument(ns, qname)`
//! against **the same algorithm** — *validate and extract*. The engine had that algorithm written
//! out inside `createElementNS` and **not at all** inside `createDocument`, so:
//!
//! ```text
//!   createDocument('http://example.com/', 'xmlns')   → a Document   (spec: NamespaceError)
//!   createDocument(null, 'p:q')                      → a Document   (spec: NamespaceError)
//!   createDocument('http://example.com/', 'a:b:c')   → a Document   (spec: InvalidCharacterError)
//! ```
//!
//! 39 `dom` subtests assert exactly those throws, and every one of them arrived at
//! `assert_throws_dom … did not throw`.
//!
//! ## The fix is an EXTRACTION, not a second copy — and that is the point
//!
//! *One rule, two implementations* is a shape this project has paid for repeatedly (t720-724, and
//! `event_loop`'s two drain loops most recently). Copying `createElementNS`'s validation into
//! `createDocument` would have passed this gate on the day and let the two diverge on the next spec
//! change. `validate_and_extract()` is now the single implementation, and **both** callers route
//! through it — which is why this gate asserts the `createElementNS` side too. If a later tick
//! "optimises" one caller back into its own copy, the arm it forgets fails here.
//!
//! ## ⚠ The one difference between the callers is SPECIFIED, and it is a parameter
//!
//! `createDocument(null, "")` is **valid** — it means *a document with no document element* — while
//! `createElementNS(null, "")` is an `InvalidCharacterError`. That is a real asymmetry in the spec,
//! not a shortcut, so it is an explicit `allow_empty` flag rather than a silent behaviour difference.
//! `emptyNameIsADocument` and `emptyNameIsNotAnElement` below pin **both halves**, because a fix that
//! shared the rule *without* the parameter would break one of them while looking tidy.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
  <div id="out">-</div>
  <script>
    var R = [];
    function p(s) { R.push(s); document.getElementById('out').textContent = R.join(' '); }
    var impl = document.implementation;
    function doc(ns, q) {
      try {
        var d = impl.createDocument(ns, q);
        return d.documentElement ? d.documentElement.tagName : 'NO-ROOT';
      } catch (e) { return 'THROW:' + e.name; }
    }
    function el(ns, q) {
      try { return document.createElementNS(ns, q).tagName; }
      catch (e) { return 'THROW:' + e.name; }
    }

    // ── 1. THE CUT — namespace rules, none of which fired on `createDocument`.
    p('xmlnsWrongNs:' + doc('http://example.com/', 'xmlns'));
    p('prefixNoNs:' + doc(null, 'p:q'));
    p('xmlPrefixWrongNs:' + doc('http://example.com/', 'xml:q'));
    p('xmlnsNsWrongName:' + doc('http://www.w3.org/2000/xmlns/', 'foo'));

    // ── 2. THE CUT — name rules.
    p('twoColons:' + doc('http://example.com/', 'a:b:c'));
    p('leadingColon:' + doc('http://example.com/', ':foo'));
    p('trailingColon:' + doc('http://example.com/', 'foo:'));

    // ── 3. ⚠ THE SPECIFIED ASYMMETRY, both halves.
    p('emptyNameIsADocument:' + doc(null, ''));
    p('emptyNameIsNotAnElement:' + el(null, ''));

    // ── 4. THE SIBLING CALLER must still enforce the same rule — this is what makes the fix an
    //    EXTRACTION rather than a copy, and it is the arm a re-divergence fails.
    p('elXmlnsWrongNs:' + el('http://example.com/', 'xmlns'));
    p('elPrefixNoNs:' + el(null, 'p:q'));
    p('elTwoColons:' + el('http://example.com/', 'a:b:c'));

    // ── 5. THE RATCHET — every VALID pair must still build what it built before.
    p('validNsDoc:' + doc('http://example.com/', 'foo'));
    p('validPrefixed:' + doc('http://example.com/', 'p:q'));
    p('xmlOk:' + doc('http://www.w3.org/XML/1998/namespace', 'xml:q'));
    p('xmlnsOk:' + doc('http://www.w3.org/2000/xmlns/', 'xmlns'));
    p('svgOk:' + doc('http://www.w3.org/2000/svg', 'svg'));
    p('validEl:' + el('http://www.w3.org/2000/svg', 'linearGradient'));
    p('contentType:' + impl.createDocument('http://www.w3.org/1999/xhtml', 'html').contentType);
  </script>
</body></html>"##;

#[test]
fn create_document_and_create_element_ns_share_one_validate_and_extract() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://createdoc.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("CREATE-DOCUMENT: {got}");

    for (claim, why) in CLAIMS {
        assert!(
            got.contains(claim),
            "G_CREATE_DOCUMENT_VALIDATION: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}

const CLAIMS: &[(&str, &str)] = &[
    (
        "xmlnsWrongNs:THROW:NamespaceError",
        "THE LOAD-BEARING CLAIM. `createDocument` ran NO validation at all — the algorithm was \
         written out inside `createElementNS` and nowhere else — so this built a Document. 39 `dom` \
         subtests assert this family of throws",
    ),
    ("prefixNoNs:THROW:NamespaceError", "a prefix with no namespace"),
    (
        "xmlPrefixWrongNs:THROW:NamespaceError",
        "the `xml` prefix outside the XML namespace",
    ),
    (
        "xmlnsNsWrongName:THROW:NamespaceError",
        "and THE CONVERSE, which a half-written rule drops: the XMLNS namespace with any name that \
         is not `xmlns`",
    ),
    (
        "twoColons:THROW:InvalidCharacterError",
        "a qualified name has at most one colon — an InvalidCharacterError, NOT a NamespaceError, \
         and the two are separate assertions in WPT",
    ),
    ("leadingColon:THROW:InvalidCharacterError", "neither end may be a colon"),
    ("trailingColon:THROW:InvalidCharacterError", "the other end"),
    (
        "emptyNameIsADocument:NO-ROOT",
        "⚠ THE SPECIFIED ASYMMETRY. `createDocument(null, '')` is VALID and means a document with no \
         document element. A shared rule WITHOUT the `allow_empty` parameter breaks this while \
         looking tidy",
    ),
    (
        "emptyNameIsNotAnElement:THROW:InvalidCharacterError",
        "…and the other half of that asymmetry: the SAME empty name is an error for \
         `createElementNS`. Both are pinned so the difference stays deliberate",
    ),
    (
        "elXmlnsWrongNs:THROW:NamespaceError",
        "⚠ THE SIBLING CALLER. This is what makes the fix an EXTRACTION rather than a copy: if a \
         later tick gives one caller its own validation again, the arm it forgets fails here. *One \
         rule, two implementations* is the shape this project keeps paying for",
    ),
    ("elPrefixNoNs:THROW:NamespaceError", "the sibling, same rule"),
    ("elTwoColons:THROW:InvalidCharacterError", "the sibling, same rule"),
    (
        "validNsDoc:foo",
        "THE RATCHET. A valid pair must still build the document element it names",
    ),
    ("validPrefixed:p:q", "THE RATCHET. A prefix WITH a namespace is fine"),
    ("xmlOk:xml:q", "THE RATCHET. The `xml` prefix IN the XML namespace"),
    ("xmlnsOk:xmlns", "THE RATCHET. `xmlns` IN the XMLNS namespace"),
    ("svgOk:svg", "THE RATCHET. The SVG namespace"),
    (
        "validEl:linearGradient",
        "THE RATCHET, and a case this engine got wrong once: `createElementNS` keeps the name's CASE, \
         so SVG's `linearGradient` is not uppercased into nothing",
    ),
    (
        "contentType:application/xhtml+xml",
        "THE RATCHET. The content type is still derived from the NAMESPACE — adding validation ahead \
         of it must not disturb what it produces",
    ),
];
