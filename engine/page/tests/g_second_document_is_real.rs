//! # G_SECOND_DOCUMENT_IS_REAL — a parsed document is a NODE, and its nodes know they belong to it
//!
//! **The failure this gate exists for: `DOMPurify.sanitize('<b>hi</b>')` returned the empty
//! string.** Not the escaped text, not the tag stripped — *nothing*. Every site that renders
//! user-supplied HTML through a sanitizer (comments, CMS bodies, rendered markdown, rich-text
//! fields) displayed blank content, silently, while `sanitize('plain text')` worked and hid the
//! problem from any quick check.
//!
//! Two defects stacked, and neither is visible from the sanitizer's side:
//!
//! 1. **`DOMParser.parseFromString` returned an object literal wearing `nodeType: 9`** — a duck
//!    with `documentElement`/`body`/`querySelector` around a *detached `<html>` element*. It was
//!    not in the node arena at all, so nothing that treats the result as a node worked.
//! 2. **`ownerDocument` returned `window.document` for every node**, the arena holding several
//!    roots. A node parsed into a throwaway document claimed to belong to the live page.
//!
//! DOMPurify's walk is `createNodeIterator.call(root.ownerDocument || root, root, …)`. With the
//! wrong document it iterated a tree the root was not in, found no nodes, and emitted nothing.
//!
//! ## The load-bearing claim is transcribed from the library, not invented
//!
//! `iterFinds` runs DOMPurify's exact expression — `root.ownerDocument || root` as the iterator's
//! `this`, over a parsed root — and requires it to find the elements. Asserting
//! `ownerDocument === doc` alone would pass on an engine where the iterator still walked the wrong
//! tree.
//!
//! ## RED probes run against this gate
//!
//! | mutation | result |
//! |---|---|

//! | `ownerDocument`'s secondary-document walk removed (the t642 state) | RED — `dpOwner`, `dpNotMain`, `chdOwner` and `chdNotMain` all fail together, while every main-document claim stays green |
//! | the walk made unconditional (drop `cur != root()`) | **GREEN — this probe did NOT land**, and it corrected the gate's own doc: the node cache already returns one reflector per node, so main-document identity survives without the guard. Recorded rather than quietly deleted. |

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
  <div id="here"></div><div id="out">-</div>
  <script>
    var R = [];
    function p(s) { R.push(s); document.getElementById('out').textContent = R.join(' '); }

    var d = new DOMParser().parseFromString('<b>hi</b><i>x</i>', 'text/html');
    var b = d.body.firstChild;

    // ── 1. The parsed document is a NODE in the arena, not a duck.
    p('dpType:' + d.nodeType);
    var m = b, hops = 0;
    while (m.parentNode && hops < 9) { m = m.parentNode; hops++; }
    p('dpIsNode:' + (m === d));
    p('dpOwner:' + (b.ownerDocument === d));
    p('dpNotMain:' + (b.ownerDocument !== document));
    p('dpBody:' + d.body.nodeName + ' kids:' + d.body.childNodes.length);

    // ── 2. DOMPurify's EXACT expression, transcribed. This is the claim that matters.
    var iter = document.createNodeIterator.call(b.ownerDocument || b, d.body, 0xFFFFFFFF, null, false);
    var names = [], n;
    while ((n = iter.nextNode())) { if (n.nodeType === 1) names.push(n.nodeName); }
    p('iterFinds:' + names.length + ':' + names.join(','));

    // ── 3. createHTMLDocument, the other producer of a second document.
    var c = document.implementation.createHTMLDocument('');
    c.body.innerHTML = '<p>x</p>';
    p('chdOwner:' + (c.body.firstChild.ownerDocument === c));
    p('chdNotMain:' + (c.body.firstChild.ownerDocument !== document));

    // ── 4. THE MAIN DOCUMENT MUST NOT MOVE. `el.ownerDocument === document` is object identity,
    //    and half the web compares against it. A fix that minted a fresh document object for the
    //    page's own nodes would satisfy every claim above and break everything.
    p('mainIdentity:' + (document.getElementById('here').ownerDocument === document));
    p('detachedIdentity:' + (document.createElement('div').ownerDocument === document));
    p('docOwnerNull:' + (document.ownerDocument === null));
  </script>
</body></html>"##;

#[test]
fn a_parsed_document_is_a_node_and_owns_its_children() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://seconddoc.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("SECOND DOCUMENT: {got}");

    for (claim, why) in CLAIMS {
        assert!(
            got.contains(claim),
            "G_SECOND_DOCUMENT_IS_REAL: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}

const CLAIMS: &[(&str, &str)] = &[
    (
        "iterFinds:3:BODY,B,I",
        "THE LOAD-BEARING CLAIM, and it is DOMPurify's own expression transcribed: \
         `createNodeIterator.call(root.ownerDocument || root, root, …)`. It must find the root and both parsed elements — THREE, because a NodeIterator yields \
         its own root first, which my first draft of this claim got wrong (the engine was right). When it found none, DOMPurify returned the EMPTY STRING for any input \
         containing a tag — every comment thread, CMS body and rendered markdown on such a site \
         rendered blank, silently",
    ),
    (
        "dpIsNode:true",
        "the parsed document is the ROOT OF THE PARENT CHAIN from a parsed node. It used to be an \
         object literal with `nodeType: 9` wrapped around a detached <html> element, so the chain \
         ended at that element and the returned object was not in the arena at all",
    ),
    (
        "dpOwner:true",
        "and a parsed node reports THAT document as its owner. `ownerDocument` returned \
         `window.document` unconditionally, so a node in a sanitizer's throwaway document claimed \
         to belong to the live page",
    ),
    ("dpNotMain:true", "stated as its own claim so `dpOwner` cannot be satisfied by aliasing"),
    ("dpType:9", "and the parsed document is a DOCUMENT_NODE (see G_NODE_TYPE_ENUMERATION)"),
    (
        "dpBody:BODY kids:2",
        "the real body, with both parsed children — a document with no body is how the previous \
         shim's `body || documentElement` fallback hid its own absence",
    ),
    (
        "chdOwner:true",
        "the OTHER producer of a second document must agree. Two ways to make a document, one real \
         and one pretend, is exactly what this tick deleted",
    ),
    ("chdNotMain:true", "same non-aliasing check for that path"),
    (
        "mainIdentity:true",
        "THE RATCHET CLAUSE. `el.ownerDocument === document` is OBJECT IDENTITY and half the web \
         compares against it. ⚠ The RED probe corrected what I first wrote here: I claimed the \
         main-root guard in `el_get_owner_document` is what holds this, and removing that guard \
         leaves this claim GREEN — the NODE CACHE returns the same reflector for the main document \
         either way. The guard is cheap belt-and-braces over a subtler contract, not the load. The \
         assertion still earns its place: it is what would catch a change that mints a fresh \
         document object per lookup",
    ),
    (
        "detachedIdentity:true",
        "and a DETACHED node still reports the document that created it, per spec — it has no \
         document root to walk to, so it must fall through rather than answer null",
    ),
    ("docOwnerNull:true", "a document has no owner document (tick 642)"),
];
