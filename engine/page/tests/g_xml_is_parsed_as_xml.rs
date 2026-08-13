//! # G_XML_IS_PARSED_AS_XML — `parseFromString(s, 'text/xml')` runs the XML parser, not the HTML one
//!
//! **The failure this gate exists for: an SVG string parsed through `DOMParser` came back with no
//! `<clipPath>`, no `<linearGradient>` and no `<textPath>` in it** — because the HTML tokenizer
//! lowercases every tag name, so those elements became `clippath`, `lineargradient` and `textpath`.
//! They match no selector, resolve against no SVG attribute set, and paint nothing. Nothing
//! reported an error; the icons were simply blank.
//!
//! `DOMParser.parseFromString` ignored its second argument entirely. Every MIME type — the four XML
//! ones included — was answered by HTML-parsing into a `createHTMLDocument()`. That is four
//! distinct wrongs on one line:
//!
//! 1. **case is destroyed** (HTML lowercases tag names; XML is case-sensitive);
//! 2. **namespaces are invented** (everything landed in XHTML regardless of `xmlns`);
//! 3. **malformed input is silently recovered** where XML says it is fatal;
//! 4. **`contentType` reported `text/html`** — overriding the type the caller had just named,
//!    because the getter was a hardcoded string and the prelude's `doc.contentType = type`
//!    assignment hit a native getter with no setter and died inside its own `catch`.
//!
//! This is the line JS uses to read an RSS/Atom feed, an SVG string, a SOAP body or a sitemap, so
//! the blast radius is every feed reader and every icon set that injects markup.
//!
//! ## What this gate deliberately does NOT claim
//!
//! `parseFromString` must **not** return an `XMLDocument` — `DOMParser-parseFromString-xml`
//! asserts `assert_false(doc instanceof XMLDocument)` for all four XML types. Only
//! `DOMImplementation.createDocument()` produces one. Both halves are asserted below, because a
//! fix that branded every XML parse as `XMLDocument` would look right and be wrong.
//!
//! ⚠ **The unclosed-tag case is a KNOWN, NAMED GAP, not an oversight.** xml5ever silently
//! auto-closes `<foo>` at EOF and its open-element stack is private, so we cannot see it — see the
//! table on `manuk_html::XmlParse::well_formed`. This gate therefore asserts the parsererror
//! behaviour with a MISMATCHED END TAG, which is reported. `manuk_html`'s
//! `known_wellformedness_gap_is_pinned` fails the day that gap closes.
//!
//! ## RED probes run against this gate
//!
//! All four were run, and each is quoted with the values it actually produced.
//!
//! | mutation | result |
//! |---|---|
//! | route XML types back to the HTML path (`if (false)` on the `__parseXML` branch) | RED — and it reproduced the ORIGINAL bug exactly: `caseKept:HTML childCase:HEAD ns:…/xhtml ctype:text/html err:html` |
//! | `doc_get_content_type` restored to the hardcoded `"text/html"` | RED — `ctype`, `svgType` **and `errType`** fail; case and namespace stay green, isolating the getter |
//! | `createDocument` restored to `__createHTMLDocument()` | RED — `cdRoot:HTML:…/xhtml` and `cdNoBody:false`, nothing else moves |
//! | brand `parseFromString`'s XML result as `XMLDocument` too (key the predicate off `contentType`) | RED — `notXmlDoc:true`, alone. This is the claim that keeps the fix honest: it catches the plausible-looking, wrong "more complete" version |
//!
//! ⚠ A process note worth keeping: the first probe's edit landed even though the shell command
//! that carried it errored out, so the "restore" that followed backed up an ALREADY-MUTATED file
//! and probe 2's first reading was probes 1+2 stacked. It was caught because the output showed
//! `caseKept:HTML` — a symptom probe 2 has no way to cause. **Re-read the tree, not the intent,
//! before trusting a RED.**

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
  <div id="out">-</div>
  <script>
    var R = [];
    function p(s) { R.push(s); document.getElementById('out').textContent = R.join(' '); }
    var P = new DOMParser();

    // ── 1. CASE SURVIVES. The whole SVG failure is this one claim.
    var d = P.parseFromString('<Foo xmlns="urn:x"><clipPath id="c"/></Foo>', 'text/xml');
    p('caseKept:' + d.documentElement.tagName);
    p('childCase:' + d.documentElement.firstChild.tagName);

    // ── 2. The declared namespace is honoured, not replaced with XHTML.
    p('ns:' + d.documentElement.namespaceURI);

    // ── 3. The document reports the type it was ASKED for.
    p('ctype:' + d.contentType);
    p('svgType:' + P.parseFromString('<svg/>', 'image/svg+xml').contentType);
    p('htmlType:' + P.parseFromString('<b>x</b>', 'text/html').contentType);

    // ── 4. Malformed XML is REPLACED by a parsererror document, not recovered from.
    var bad = P.parseFromString('<foo></bar>', 'text/xml');
    p('err:' + bad.documentElement.localName);
    p('errNs:' + (bad.documentElement.namespaceURI ===
                  'http://www.mozilla.org/newlayout/xml/parsererror.xml'));
    p('errType:' + bad.contentType);

    // ── 5. It is still a REAL document — the G_SECOND_DOCUMENT_IS_REAL contract must hold for
    //    the XML path too, or DOMPurify-shaped walks break exactly as they did for HTML.
    p('xmlOwner:' + (d.documentElement.ownerDocument === d));
    p('xmlType:' + d.nodeType);

    // ── 6. XMLDocument EXISTS (its absence threw and took the rest of the file with it) and means
    //    the right thing: createDocument yes, parseFromString NO.
    p('hasXmlDoc:' + (typeof XMLDocument === 'function'));
    p('notXmlDoc:' + (d instanceof XMLDocument));
    var cd = document.implementation.createDocument('urn:y', 'root');
    p('isXmlDoc:' + (cd instanceof XMLDocument));
    p('cdIsDoc:' + (cd instanceof Document));

    // ── 7. createDocument builds the SPECIFIED tree: the named document element, and NO
    //    html/head/body skeleton. It used to discard both arguments.
    p('cdRoot:' + cd.documentElement.tagName + ':' + cd.documentElement.namespaceURI);
    p('cdNoBody:' + (cd.body == null));
    // ── 7b. createDocument's contentType is DERIVED FROM THE NAMESPACE, not a constant.
    p('cdType:' + cd.contentType);
    p('cdXhtmlType:' + document.implementation
        .createDocument('http://www.w3.org/1999/xhtml', 'html').contentType);
    p('cdSvgType:' + document.implementation
        .createDocument('http://www.w3.org/2000/svg', 'svg').contentType);

    // ── 8. THE RATCHET CLAUSE. The HTML path must not move.
    p('htmlStillWorks:' + P.parseFromString('<b>hi</b>', 'text/html').body.firstChild.nodeName);
    p('mainType:' + document.contentType);
  </script>
</body></html>"##;

#[test]
fn xml_types_are_parsed_by_the_xml_parser() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://xmlparse.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("XML PARSE: {got}");

    for (claim, why) in CLAIMS {
        assert!(
            got.contains(claim),
            "G_XML_IS_PARSED_AS_XML: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}

const CLAIMS: &[(&str, &str)] = &[
    (
        "caseKept:Foo",
        "THE LOAD-BEARING CLAIM. XML is case-sensitive and the HTML tokenizer lowercases every tag \
         name, so `<Foo/>` came back as `foo`. In the real world this is `<clipPath>`, \
         `<linearGradient>` and `<textPath>` — parse an SVG string through DOMParser and those \
         elements match no selector and paint nothing, with no error anywhere",
    ),
    (
        "childCase:clipPath",
        "stated separately from the document element so the claim cannot be satisfied by a fix \
         that only special-cases the root",
    ),
    (
        "ns:urn:x",
        "the declared `xmlns` is the element's namespace. The HTML parser forces XHTML on \
         everything, so every library that branches on `namespaceURI` or matches an `svg|rect` \
         selector was wrong about parsed XML",
    ),
    (
        "ctype:text/xml",
        "the document reports the type the CALLER NAMED. The getter was a hardcoded `text/html` \
         for every document in existence, and the prelude's `doc.contentType = type` assignment \
         was a silent no-op swallowed by its own `catch` — the engine was told the answer and \
         threw it away",
    ),
    (
        "svgType:image/svg+xml",
        "a second type, so `ctype` cannot be satisfied by hardcoding a different constant",
    ),
    (
        "htmlType:text/html",
        "and the default is unchanged — this is the value the old hardcoded getter returned, so \
         its presence here is what proves the fix is per-document rather than a global swap",
    ),
    (
        "err:parsererror",
        "XML has NO error recovery: a malformed document is REPLACED, not fixed up. The HTML \
         parser's recovery meant a broken feed silently produced a plausible-looking tree",
    ),
    (
        "errNs:true",
        "and the parsererror element carries the namespace every engine agreed on — WPT asserts \
         this exact string, so it is not ours to choose",
    ),
    (
        "errType:text/xml",
        "a parsererror document still reports the REQUESTED content type",
    ),
    (
        "xmlOwner:true",
        "the XML path must satisfy the G_SECOND_DOCUMENT_IS_REAL contract too: nodes know which \
         document they belong to. A new document producer that skipped the node-cache seeding \
         would reintroduce the exact bug that made DOMPurify return the empty string",
    ),
    ("xmlType:9", "and the result is a DOCUMENT_NODE"),
    (
        "hasXmlDoc:true",
        "the `XMLDocument` global must EXIST. Its absence is not a failed assertion — \
         `doc instanceof XMLDocument` THROWS `XMLDocument is not defined` and takes the rest of \
         the file with it, which is why one missing global cost 113 subtests",
    ),
    (
        "notXmlDoc:false",
        "⚠ THE CLAIM THAT KEEPS THE FIX HONEST. `parseFromString` returns a Document that is NOT \
         an XMLDocument, whatever type it was given — `DOMParser-parseFromString-xml` asserts \
         exactly this for all four XML types. Branding every XML parse as XMLDocument would look \
         like a more complete fix and be a wrong one",
    ),
    (
        "isXmlDoc:true",
        "while `createDocument` — the ONLY producer of one — does return an XMLDocument, so \
         `notXmlDoc` cannot be satisfied by never defining the brand at all",
    ),
    ("cdIsDoc:true", "and an XMLDocument is still a Document"),
    (
        "cdRoot:root:urn:y",
        "`createDocument(ns, qualifiedName)` builds the element the caller NAMED. Both arguments \
         used to be discarded — it called `__createHTMLDocument()` and returned an `HTML` root",
    ),
    (
        "cdNoBody:true",
        "and an XML document has NO html/head/body skeleton. This is the half that proves the \
         previous implementation was substituted rather than merely renamed",
    ),
    (
        "cdType:application/xml",
        "the default for a namespace that is neither HTML nor SVG",
    ),
    (
        "cdXhtmlType:application/xhtml+xml",
        "⚠ DERIVED FROM THE NAMESPACE, not fixed. DOM §createDocument gives the HTML namespace \
         `application/xhtml+xml` and SVG `image/svg+xml`; a flat `application/xml` looks entirely \
         reasonable and fails `Document-contentType` by name. Found by reading the failing \
         assertion after the first version of this fix shipped a constant",
    ),
    ("cdSvgType:image/svg+xml", "the SVG arm of the same rule"),
    (
        "htmlStillWorks:B",
        "THE RATCHET CLAUSE. `text/html` must go on taking the HTML path, error recovery and all",
    ),
    (
        "mainType:text/html",
        "and the PAGE's own document still reports text/html — the per-document lookup must not \
         disturb the main document, which is the one every site actually reads",
    ),
];
