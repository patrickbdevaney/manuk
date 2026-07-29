//! **G_ADOPT_NODE — `document.adoptNode` did not exist, so the call threw `TypeError` and took its
//! caller with it.**
//!
//! It is the sibling of `importNode` and the opposite trade: `importNode` returns a **clone** and
//! leaves the original where it was; `adoptNode` returns **the same node**, detached. Code depends on
//! that difference — a library that adopts and then compares `adopted === original`, or that has
//! already stashed the node in a `Map`/`WeakMap`, gets the wrong answer from a clone. Its usual
//! callers are the ones that cannot route around it: moving `template.content` children into the live
//! tree, and pulling a node out of an `<iframe>`'s document.
//!
//! **Chrome-measured, one fixture — and every row below is Chrome's answer, not a reading of the
//! spec:**
//!
//! ```text
//!   adoptNode(p)      returns the SAME node (identity, not a clone)
//!   .ownerDocument    becomes this document
//!   .parentNode       becomes null — adoption DETACHES
//!   adoptNode(null)   throws TypeError
//! ```
//!
//! ⚠ **NAMED NON-CLAIM, pinned by assertion (4): a node from ANOTHER document's arena is REFUSED,
//! loudly.** Each document owns its own `Dom` arena and a `NodeId` is only meaningful inside one —
//! `node_and_dom` exists precisely because reading an iframe's node #7 in the parent's arena returned
//! the parent's node #7 *"with total confidence"*. Moving a node between arenas is a transplant
//! (subtree copy + reflector re-binding), not a re-parent, and until that is built the honest answer
//! is a throw that says so. **Silently returning a node still owned by the other document is the
//! failure this refusal prevents**: the caller appends it and gets a node that no longer resolves.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body>
 <template id="tpl"><p class="x">hi</p></template>
 <div id="host"><i id="child">c</i></div>
 <iframe id="f" srcdoc="<html><body><b id='far'>far</b></body></html>"></iframe>
 <div id="out">-</div>
 <script>
   window.addEventListener('load', function () {
     function T(n, f) { try { return n + '=' + f(); } catch (e) { return n + '=' + e.name; } }
     var parts = [
       // Identity, not a clone — the whole reason this is not `importNode`.
       T('same', function () {
         var b = document.createElement('b');
         return document.adoptNode(b) === b;
       }),
       // The template case: the node the callers actually move.
       T('tpl', function () {
         var p = document.getElementById('tpl').content.querySelector('p');
         return document.adoptNode(p).className;
       }),
       // Adoption DETACHES. A version that only returned the node would pass `same` and leave the
       // node in two places at once.
       T('detaches', function () {
         var c = document.getElementById('child');
         document.adoptNode(c);
         return c.parentNode === null;
       }),
       T('owner', function () {
         var b = document.createElement('b');
         document.adoptNode(b);
         return b.ownerDocument === document;
       }),
       T('null', function () { document.adoptNode(null); return 'NOTHROWN'; }),
       // (4) The cross-document refusal, exercised against a real frame document (t717 gave
       // `srcdoc` frames a live document, so this is reachable).
       T('cross', function () {
         var d = document.getElementById('f').contentDocument;
         if (!d) { return 'NO-FRAME-DOC'; }
         var far = d.getElementById('far');
         if (!far) { return 'NO-FRAME-NODE'; }
         document.adoptNode(far);
         return 'NOTHROWN';
       })
     ];
     document.getElementById('out').textContent = parts.join(' ');
   });
 </script>
</body></html>"#;

fn out(page: &manuk_page::Page) -> String {
    let root = page.dom().root();
    let hits = manuk_css::query_selector_all(page.dom(), root, "#out");
    assert!(!hits.is_empty(), "#out must exist");
    page.dom().text_content(hits[0])
}

/// **One test, on purpose** — see `g_defer`.
#[test]
fn adopt_node_moves_the_node_itself_and_refuses_across_documents() {
    let fonts = FontContext::new();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let page = rt.block_on(async {
        let mut p = manuk_page::Page::load_async(HTML, "https://adopt.test/", &fonts, 800.0).await;
        p.finish_loading(&fonts, 800.0).await;
        p
    });
    let got = out(&page);
    println!("ADOPT-NODE {got}");
    let has = |s: &str| got.contains(s);

    // (1) **Identity.** RED: implement it as `importNode` (return a clone) → `same=false`, and every
    // caller holding a reference to the original silently keeps the wrong node.
    assert!(
        has("same=true"),
        "adoptNode must return the SAME node, not a clone — that is the entire difference from \
         importNode — got {got:?}"
    );

    // (2) **The template case**, which is what the callers actually do.
    assert!(
        has("tpl=x"),
        "adopting a node out of `template.content` must work — got {got:?}"
    );

    // (3) **It DETACHES.** RED: return the node without detaching → `detaches=false`, and the node is
    // in two places at once: still a child of its old parent, and about to be appended somewhere new.
    assert!(
        has("detaches=true") && has("owner=true"),
        "adoption must detach the node from its parent and leave ownerDocument as this document — \
         got {got:?}"
    );

    // (4) **`adoptNode(null)` throws TypeError**, Chrome-measured — a page that passes null by
    // accident finds out on that line rather than three frames later. RED: return null instead →
    // `null=NOTHROWN`.
    assert!(
        has("null=TypeError"),
        "adoptNode(null) must throw TypeError — got {got:?}"
    );

    // (5) **THE NON-CLAIM, PINNED.** A node from another document's arena is refused loudly. If this
    // ever reads `NOTHROWN`, cross-document adoption either landed (delete this and say so) or is
    // silently returning a node the other document still owns — which is the bug the refusal exists
    // to prevent, and it would present as an element that renders nowhere.
    assert!(
        has("cross=TypeError"),
        "adopting across documents must THROW while each document owns its own node arena — a \
         silent success hands back a node that no longer resolves — got {got:?}"
    );
}
