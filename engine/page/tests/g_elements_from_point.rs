//! **G_ELEMENTS_FROM_POINT — the singular answers *"what did the user click?"*; the plural answers
//! *"what is IN THE WAY?"*, and it did not exist.**
//!
//! `document.elementsFromPoint(x, y)` returns **every** element whose border box contains the point,
//! topmost first, ending at `<html>`. Chrome returns 3 on an ordinary `<div>` in `<body>` in
//! `<html>`; this engine threw `TypeError` — the method was absent while its singular sibling had
//! worked for hundreds of ticks.
//!
//! **Why the plural is not a convenience.** With only the singular, a page can see the topmost
//! element and has no way to look past it — which is exactly the question three common libraries ask:
//!
//! - **drag & drop**: find the drop target under the cursor *while a drag ghost sits on top of it*.
//! - **overlays/tooltips**: decide whether this floating element is occluding its own anchor.
//! - **click-through affordances**: forward an event to the layer beneath the one that received it.
//!
//! Each of those is a stack question, and the singular collapses the stack to its first entry.
//!
//! **Measured against headless Chrome on this exact fixture** (an absolutely-positioned target over a
//! full-bleed underlay, with a `pointer-events:none` drag ghost on top of both):
//!
//! ```text
//!   Chrome  target>under>wrap>BODY>HTML   first===singular: true   outside: []   isArray: true
//!   Manuk   target>under>wrap>BODY>HTML   first===singular: true   outside: []   isArray: true
//! ```
//!
//! ⚠ **`#ghost` is absent from both stacks** — `pointer-events: none` makes an element transparent to
//! hit-testing, and the plural inherits that filter from the singular rather than reimplementing it.
//! A plural that reported the ghost would break the very drag-and-drop case it exists to serve.
//!
//! ⚠⚠ **A non-finite coordinate THROWS, and this was measured, not assumed.** The first draft of the
//! implementation returned the empty list *"per CSSOM-View"* — an invented citation for a plausible
//! answer. CSSOM-View types both parameters as `double`, **not `unrestricted double`**, so WebIDL
//! rejects NaN/Infinity before the method body runs. Chrome:
//! `TypeError: Failed to execute 'elementsFromPoint' on 'Document': The provided double value is
//! non-finite.` The same correction was applied to the singular and to `g_element_from_point`, whose
//! assertion had demanded `null` and cited CSSOM-View for it since it was written.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><head><style>
 body { margin:0 }
 #wrap   { position:relative; width:300px; height:200px }
 #under  { position:absolute; left:0;  top:0;  width:300px; height:200px }
 #target { position:absolute; left:50px; top:50px; width:100px; height:100px }
 /* the drag ghost: on top of everything, and transparent to hit-testing */
 #ghost  { position:absolute; left:0; top:0; width:300px; height:200px; pointer-events:none }
</style></head><body>
 <div id="wrap"><div id="under">u</div><div id="target">t</div><div id="ghost">g</div></div>
 <div id="out">-</div>
 <script>
   window.addEventListener('load', function () {
     function ids(a) { return a.map(function (e) { return e.id || e.tagName; }).join('>'); }
     var s = Array.prototype.slice;
     function T(n, f) { try { return n + '=' + f(); } catch (e) { return n + '=' + e.name; } }
     document.getElementById('out').textContent = [
       T('stack', function () { return ids(s.call(document.elementsFromPoint(100, 100))); }),
       // The invariant a library will check, and the reason the two share an ordering rule rather
       // than each having one: `elementsFromPoint(x,y)[0]` must BE `elementFromPoint(x,y)`.
       T('first_is_singular', function () {
         return document.elementsFromPoint(100, 100)[0] === document.elementFromPoint(100, 100);
       }),
       T('outside', function () { return ids(s.call(document.elementsFromPoint(1000, 1000))); }),
       T('isArray', function () { return Array.isArray(document.elementsFromPoint(100, 100)); }),
       T('nan', function () { document.elementsFromPoint(NaN, 10); return 'NOTHROWN'; })
     ].join(' ');
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
fn elements_from_point_returns_the_whole_stack_topmost_first() {
    let fonts = FontContext::new();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let page = rt.block_on(manuk_page::Page::load_async(
        HTML,
        "https://efp.test/",
        &fonts,
        800.0,
    ));
    let got = out(&page);
    println!("ELEMENTS-FROM-POINT {got}");
    let has = |s: &str| got.contains(s);

    // (1) **The whole stack, topmost first, and NOT the ghost.** RED: remove the method → `TypeError`;
    // drop the `pointer-events` filter → `ghost>target>under>…`, which breaks the drag-and-drop case
    // the API exists for; sort ascending by depth → `HTML>BODY>wrap>under>target`.
    // Chrome measures exactly this string.
    assert!(
        has("stack=target>under>wrap>BODY>HTML"),
        "elementsFromPoint must return the full stack topmost-first, with the \
         `pointer-events:none` ghost EXCLUDED — got {got:?}"
    );

    // (2) **The invariant between the siblings.** RED: give the plural its own ordering rule → the
    // first entry stops matching the singular, and every library that cross-checks them sees drift.
    assert!(
        has("first_is_singular=true"),
        "elementsFromPoint(x,y)[0] must BE elementFromPoint(x,y) — got {got:?}"
    );

    // (3) **A point over nothing is the EMPTY list**, not null and not a throw.
    assert!(
        has("outside=") && !has("outside=HTML"),
        "a point outside every box must give the empty list — got {got:?}"
    );

    // (4) **A real Array**, per WebIDL `sequence<Element>` — libraries call `.map`/`.filter` on it
    // directly, and an array-like would pass a `.length` check and fail on the very next line.
    assert!(
        has("isArray=true"),
        "the return must be a real Array (WebIDL sequence<Element>) — got {got:?}"
    );

    // (5) **A non-finite coordinate THROWS TypeError**, measured against Chrome. RED: return the
    // empty list "per CSSOM-View" (the first draft) → `nan=NOTHROWN`.
    assert!(
        has("nan=TypeError"),
        "a non-finite coordinate must throw TypeError — CSSOM-View types both parameters as \
         `double`, not `unrestricted double`, so WebIDL rejects NaN before the method runs — got \
         {got:?}"
    );
}
