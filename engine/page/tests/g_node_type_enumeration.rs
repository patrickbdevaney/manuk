//! # G_NODE_TYPE_ENUMERATION — every node kind reports its own `nodeType`, not just the ones a framework complained about
//!
//! **The failure this gate exists for: `document.nodeType` was 8.** COMMENT_NODE, where the spec
//! says 9. jQuery's `setDocument` is guarded by `9 === n.nodeType`, so it declined to initialise its
//! selector engine, left its internal document handle `undefined`, and threw
//! `can't access property "createElement", T is undefined` on first use. **jQuery never defined
//! `window.jQuery` at all, and nothing reported an error** — a silent, total failure of the single
//! most widely deployed library on the web.
//!
//! Found by running the real 87KB `jquery-3.7.1.min.js` (tick 642), not by reading the DOM spec.
//!
//! ## Why the whole enumeration, and not just `document`
//!
//! `el_get_node_type` was written for React — `isValidContainer` checks
//! `nodeType === ELEMENT_NODE` — and then extended, one arm at a time, by whichever framework
//! complained next: `7` for processing instructions, `11` for fragments and shadow roots. Its own
//! comment says answering 8 for a fragment "is not a near-miss, because every framework's node
//! dispatch branches on this number." **The document had that identical defect the entire time,
//! one `else if` away, and survived because the function was only ever extended by symptom.**
//!
//! > A property fixed by chasing the framework that noticed keeps exactly the holes no framework
//! > has noticed yet. When the value is drawn from a small closed set, **assert the set**.
//!
//! So this gate enumerates all eight kinds a page can produce. It is deliberately more than the bug.
//!
//! ## RED probes run against this gate
//!
//! | mutation | result |
//! |---|---|
//! | remove the `is_document` arm (restore the pre-t642 state) | RED — `doc:9`, and `ownerDoc:9` with it |
//! | remove the `is_fragment \|\| is_shadow_root` arm | RED — `frag:11` and `shadow:11` |
//! | make the fallback `0` instead of `8` | RED — `comment:0`. **This probe disproved the rationale I had written for that claim**: comments DO ride the fallback (there is no `is_comment` predicate in `manuk_dom`), so 8 is a load-bearing default rather than a decision. Recorded, not glossed. |

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
  <div id="host"></div><div id="out">-</div>
  <script>
    var R = [];
    function p(s) { R.push(s); document.getElementById('out').textContent = R.join(' '); }

    p('doc:' + document.nodeType);
    p('el:' + document.getElementById('host').nodeType);
    p('text:' + document.createTextNode('t').nodeType);
    p('comment:' + document.createComment('c').nodeType);
    p('frag:' + document.createDocumentFragment().nodeType);
    p('attr:' + (document.createAttribute ? document.createAttribute('x').nodeType : 'n/a'));
    p('pi:' + (document.createProcessingInstruction
                ? document.createProcessingInstruction('t', 'd').nodeType : 'n/a'));
    p('doctype:' + (document.doctype ? document.doctype.nodeType : 'n/a'));
    var sr = document.getElementById('host').attachShadow({ mode: 'open' });
    p('shadow:' + sr.nodeType);

    // The document reached INDIRECTLY — jQuery gets there via `elem.ownerDocument`, never via the
    // `document` global, so this is the path that actually failed.
    p('ownerDoc:' + document.createElement('div').ownerDocument.nodeType);
    // getRootNode() on a node INSIDE the shadow tree — the host itself is in the document, so
    // asking the host is a different (and less interesting) question. This is how a component asks
    // "am I in a shadow tree".
    var inner = document.createElement('span'); sr.appendChild(inner);
    p('rootNode:' + inner.getRootNode().nodeType);
    p('hostRoot:' + document.getElementById('host').getRootNode().nodeType);
    p('docOwnerIs:' + (document.ownerDocument === document ? 'self'
                        : (document.ownerDocument === null ? 'null' : typeof document.ownerDocument)));

    // ── jQuery's ACTUAL guard, transcribed from `setDocument` in jquery-3.7.1.min.js:
    //      var n = e ? e.ownerDocument || e : ye;
    //      return n != T && 9 === n.nodeType && n.documentElement && ( … initialise … )
    //    All three terms, because the engine dies if ANY of them is falsy.
    var n = document.createElement('div').ownerDocument;
    p('jqGuard:' + !!(9 === n.nodeType && n.documentElement));

    // And the spec identity that pairs with it: a document has no owner document.
    p('docOwner:' + (document.ownerDocument === null));
  </script>
</body></html>"##;

#[test]
fn every_node_kind_reports_its_own_node_type() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://nodetype.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("NODE TYPE ENUMERATION: {got}");

    for (claim, why) in CLAIMS {
        assert!(
            got.contains(claim),
            "G_NODE_TYPE_ENUMERATION: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}

const CLAIMS: &[(&str, &str)] = &[
    (
        "doc:9",
        "THE BUG THIS GATE IS NAMED FOR. `document.nodeType` was 8 (COMMENT_NODE). jQuery's \
         setDocument is guarded by `9 === n.nodeType`; with 8 it silently declined to initialise, \
         left its document handle undefined, threw on first use, and never defined window.jQuery — \
         with NO error reported anywhere",
    ),
    (
        "ownerDoc:9",
        "and the path that actually failed: jQuery reaches the document through \
         `elem.ownerDocument`, never through the `document` global. Asserted separately because a \
         fix that special-cased the global would leave this one broken",
    ),
    (
        "jqGuard:true",
        "jQuery's guard transcribed whole — `9 === n.nodeType && n.documentElement` — since the \
         selector engine dies if EITHER term is falsy and asserting only the first would miss half \
         of it",
    ),
    ("el:1", "ELEMENT_NODE — React's isValidContainer, the arm this function was born for"),
    ("text:3", "TEXT_NODE"),
    (
        "comment:8",
        "COMMENT_NODE — and the RED probe DISPROVED what I first wrote here. I documented that this \
         is reached because the node IS a comment; changing the fallback from 8 to 0 turns it into \
         `comment:0`, so it rides the FALLBACK. `manuk_dom` has no `is_comment` predicate, so the \
         arm cannot be written today. The consequence is real and is recorded rather than glossed: \
         **any node kind this function does not recognise reports as a comment**, which is right for \
         comments and a guess for everything else. Correct answer, load-bearing default",
    ),
    (
        "frag:11",
        "DOCUMENT_FRAGMENT_NODE — the arm added when a framework complained; kept here so the \
         enumeration is whole",
    ),
    (
        "shadow:11",
        "a ShadowRoot IS a DocumentFragment to the spec, and `getRootNode().nodeType === 11` is how \
         a component asks whether it is inside a shadow tree",
    ),
    (
        "rootNode:11",
        "getRootNode() from INSIDE the shadow tree — how a component asks whether it is in one. My \
         first draft asked the HOST, which is in the document and correctly answers 9; the engine \
         was right and the expectation was wrong, which is worth recording because a gate written \
         against a wrong expectation is how a correct engine gets `fixed`",
    ),
    (
        "hostRoot:9",
        "and the host itself roots to the DOCUMENT — the pair is what distinguishes the two \
         questions people confuse",
    ),
    ("pi:7", "PROCESSING_INSTRUCTION_NODE"),
    ("doctype:10", "DOCUMENT_TYPE_NODE"),
    ("attr:2", "ATTRIBUTE_NODE — deprecated as a node, still enumerated, still branched on"),
    (
        "docOwner:true",
        "the spec identity that pairs with `doc:9` — a document has no owner document. A `document` \
         whose ownerDocument were itself would satisfy `ownerDoc:9` while being wrong",
    ),
];
