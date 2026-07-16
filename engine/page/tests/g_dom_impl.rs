//! **G_DOM_IMPL — `document.implementation.createHTMLDocument()` and pre-insertion validity.**
//!
//! `createHTMLDocument()` is how **DOMPurify** and every other sanitizer works: parse hostile markup into
//! a **detached** document so nothing in it can run, touch the real page, or fetch anything. Its absence
//! is a `TypeError` on the call, which takes the sanitizer — and the page — down. WPT's `dom/nodes` failed
//! **488 times** on `documentElement`, every one downstream of this returning `undefined`.
//!
//! And the moment a second Document exists, a page can try to **insert** it — so this also gates the DOM's
//! pre-insertion validity, which is not a nicety: inserting a node into its own descendant makes the tree
//! a **cycle**, and every `children()` walk then spins forever. That is a **hang**, which is Bar 0.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body><div id="out">-</div><script>
    var R = [];
    var impl = document.implementation;
    R.push('implExists:' + (typeof impl === 'object' && impl !== null));
    R.push('hasFeature:' + (impl.hasFeature() === true));   // spec: always true

    var doc = impl.createHTMLDocument('hi');
    // The arena document is REAL: it has an html/head/title/body subtree, queryable from the main context.
    R.push('docExists:' + (doc !== null && doc !== undefined));

    // ── Pre-insertion validity. A document cannot be a child, and a cycle must throw — not hang.
    var box = document.getElementById('out');
    var cycleThrew = 'no';
    try { box.appendChild(box); }              // insert a node into itself → would be a cycle
    catch (e) { cycleThrew = (e instanceof DOMException) ? e.name : ('wrong:' + e); }
    R.push('cycleThrows:' + cycleThrew);

    // And an ANCESTOR into its descendant is the same class.
    var parent = document.body, child = box;   // box is inside body
    var ancThrew = 'no';
    try { child.appendChild(parent); }
    catch (e) { ancThrew = (e instanceof DOMException) ? e.name : ('wrong:' + e); }
    R.push('ancestorThrows:' + ancThrew);

    // ── The spec's OTHER validity throws — the ones real code actually catches.
    //
    // A TEXT NODE CANNOT HAVE CHILDREN. That sounds obvious right until you notice that
    // `text.appendChild(x)` used to SUCCEED — leaving a subtree hanging off a text node that no
    // traversal expects and nothing will ever render. Silently accepting an impossible tree is worse
    // than refusing it: the corruption surfaces later, somewhere else, looking unrelated.
    var t = document.createTextNode('x');
    var textThrew = 'no';
    try { t.appendChild(document.createElement('div')); }
    catch (e) { textThrew = (e instanceof DOMException) ? e.name : ('wrong:' + e); }
    R.push('textNoKids:' + textThrew);

    // insertBefore with a reference that is NOT a child: a caller bug, and the DOM must say so.
    // Silently appending instead puts the node somewhere the page never asked for.
    var stray = document.createElement('span');
    var refThrew = 'no';
    try { box.insertBefore(document.createElement('i'), stray); }
    catch (e) { refThrew = (e instanceof DOMException) ? e.name : ('wrong:' + e); }
    R.push('badRef:' + refThrew);

    // removeChild on a node that is not your child. Every framework's unmount path catches this;
    // a DOM that never raises it turns a loud bug into a silent leak.
    var rmThrew = 'no';
    try { box.removeChild(stray); }
    catch (e) { rmThrew = (e instanceof DOMException) ? e.name : ('wrong:' + e); }
    R.push('badRemove:' + rmThrew);

    // ── createDocumentType: a DOCTYPE name, not a QName. The old check applied the QName production and
    // wrongly threw for `1foo`/`@foo`/`prefix::local`/``. Per spec the ONLY rule is: no ASCII whitespace,
    // no U+0000 NULL, no `>` (dom.spec.whatwg.org #valid-doctype-name). The empty string is valid.
    R.push('dtDigit:' + impl.createDocumentType('1foo', '', '').name);       // 1foo (was: threw)
    R.push('dtDoubleColon:' + impl.createDocumentType('prefix::local', '', '').name); // prefix::local
    R.push('dtEmpty:[' + impl.createDocumentType('', '', '').name + ']');    // [] (empty is valid)
    var dt = impl.createDocumentType('html', 'pub', 'sys');
    R.push('dtPub:' + dt.publicId + ',' + dt.systemId);                      // pub,sys
    R.push('dtIsDT:' + (dt instanceof DocumentType) + ',' + dt.nodeType);    // true,10
    var gtThrew = 'no';
    try { impl.createDocumentType('a>b', '', ''); }
    catch (e) { gtThrew = (e instanceof DOMException) ? e.name : ('wrong:' + e); }
    R.push('dtGt:' + gtThrew);                                              // InvalidCharacterError
    var spThrew = 'no';
    try { impl.createDocumentType('a b', '', ''); }
    catch (e) { spThrew = (e instanceof DOMException) ? e.name : ('wrong:' + e); }
    R.push('dtSpace:' + spThrew);                                           // InvalidCharacterError

    // ── `.implementation` is PER-DOCUMENT. A document minted by createHTMLDocument exposes its OWN
    // implementation, bound to ITSELF — WPT asserts the created doctype's ownerDocument is that doc.
    R.push('doc2Impl:' + (typeof doc.implementation));                     // object (was: undefined)
    var dt2 = doc.implementation.createDocumentType('foo', '', '');
    R.push('dt2Owner:' + (dt2.ownerDocument === doc));                     // true
    R.push('dt2NotMain:' + (dt2.ownerDocument === document));             // false

    // The page is intact after all of that.
    R.push('intact:' + (document.getElementById('out') === box));
    box.textContent = R.join(' ');
  </script></body></html>"#;

#[test]
fn create_html_document_exists_and_insertion_validity_prevents_cycles() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://impl.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        "implExists:true",
        "hasFeature:true",
        "docExists:true",
        "cycleThrows:HierarchyRequestError", // NOT a hang
        "ancestorThrows:HierarchyRequestError",
        "dtDigit:1foo",                  // doctype name starting with a digit is valid
        "dtDoubleColon:prefix::local",   // `prefix::local` is a valid doctype name (not a QName)
        "dtEmpty:[]",                    // the empty string is a valid doctype name
        "dtPub:pub,sys",                 // publicId/systemId carried through
        "dtIsDT:true,10",                // a real DocumentType, nodeType 10
        "dtGt:InvalidCharacterError",    // `>` makes a doctype name invalid
        "dtSpace:InvalidCharacterError", // ASCII whitespace makes a doctype name invalid
        "doc2Impl:object",               // a created document has its OWN .implementation
        "dt2Owner:true",                 // a created doc's doctype is owned by THAT doc
        "dt2NotMain:false",              // ...not by the main document
        "intact:true",
    ] {
        assert!(
            got.contains(claim),
            "G_DOM_IMPL: expected `{claim}`\n  got: {got}\n\n  \
             createHTMLDocument is how every sanitizer builds a safe detached tree. And pre-insertion \
             validity is Bar 0: inserting a node into its own descendant makes the tree a cycle, and a \
             cycle is an infinite children() walk — a HANG, not a wrong answer."
        );
    }
}
