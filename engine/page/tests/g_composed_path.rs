//! **G_COMPOSED_PATH — `event.composedPath()` stopped at the document, one entry short of the
//! window.**
//!
//! Chrome-measured on three shapes, and the third is why this is a condition and not an append:
//!
//! ```text
//!                      CHROME                          BEFORE                  AFTER
//!   connected   t > BODY > HTML > document > window   ...> document (4 of 5)   matches
//!   detached    I                                     I                        I
//!   in fragment U > #document-fragment                U > #document-fragment   unchanged
//! ```
//!
//! **`path[path.length - 1] === window` is the standard test** an event-delegation library uses to
//! ask *"did this event escape my root / is this node connected?"*. Missing the window answers **no**
//! for every connected node — and appending it unconditionally would answer **yes** for a node that
//! is in no document at all, which is the worse bug. So the global is pushed only when the walk
//! actually reached the document.
//!
//! ⚠ Named residue, measured beside this and NOT fixed: a **composed event is not retargeted** —
//! `event.target` on a `document` listener reads the node inside the shadow tree where Chrome reads
//! the HOST. `composedPath` is the *shape* of the path; retargeting is *whose* node each listener
//! sees, and it is a dispatch-path change of its own.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body>
 <div id="t">t</div>
 <div id="out">-</div>
 <script>
   window.addEventListener('load', function () {
     function names(p) {
       return p.map(function (n) {
         return n === window ? 'window' : (n === document ? 'document' : (n.id || n.nodeName));
       }).join('>');
     }
     var r = {}, t = document.getElementById('t');
     t.addEventListener('a', function (e) { r.conn = names(e.composedPath()); });
     t.dispatchEvent(new Event('a', { bubbles: true }));

     var d = document.createElement('i');
     d.addEventListener('b', function (e) { r.det = names(e.composedPath()); });
     d.dispatchEvent(new Event('b', { bubbles: true }));

     var frag = document.createDocumentFragment(), k = document.createElement('u');
     frag.appendChild(k);
     k.addEventListener('c', function (e) { r.frag = names(e.composedPath()); });
     k.dispatchEvent(new Event('c', { bubbles: true }));

     // The idiom the whole thing is for.
     var last = null;
     t.addEventListener('d', function (e) {
       var p = e.composedPath(); last = p[p.length - 1] === window;
     });
     t.dispatchEvent(new Event('d', { bubbles: true }));

     document.getElementById('out').textContent =
       'conn=' + r.conn + ' det=' + r.det + ' frag=' + r.frag + ' lastIsWindow=' + last;
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
fn composed_path_reaches_the_window_and_stops_where_the_tree_does() {
    let fonts = FontContext::new();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let page = rt.block_on(manuk_page::Page::load_async(
        HTML,
        "https://cp.test/",
        &fonts,
        800.0,
    ));
    let got = out(&page);
    println!("COMPOSED-PATH {got}");
    let has = |s: &str| got.contains(s);

    // (1) **The connected path ends at the window.** RED: drop the append → `...>document`, four of
    // five, which is what shipped.
    assert!(
        has("conn=t>BODY>HTML>document>window") && has("lastIsWindow=true"),
        "composedPath on a connected node must end at the window — `path[path.length-1] === window` \
         is how a delegation library asks whether the event escaped its root — got {got:?}"
    );

    // (2) **THE CONDITION, and it is the whole reason this is not a plain append.** A detached node's
    // path is itself; a node in a fragment stops at the fragment. RED: append unconditionally →
    // `det=I>window`, and the idiom above answers "connected" for a node in no document at all.
    // ⚠ The FIELD BOUNDARY is load-bearing. The first version of these two asserted `has("det=I")`
    // and `has("frag=U>#document-fragment")` — and the unconditional-append mutation produced
    // `det=I>window` and `frag=U>#document-fragment>window`, **both of which CONTAIN those
    // substrings**, so the gate went green on the exact bug it was written to catch. A prefix is not
    // a value. Matching through to the next field's name pins the end of this one.
    assert!(
        has("det=I frag="),
        "a DETACHED node's composedPath is just itself — appending the window there is worse than \
         the bug being fixed, because `path[path.length-1] === window` would then report a node in \
         no document as connected — got {got:?}"
    );
    assert!(
        has("frag=U>#document-fragment lastIsWindow="),
        "a node inside a DocumentFragment stops at the fragment — got {got:?}"
    );
}
