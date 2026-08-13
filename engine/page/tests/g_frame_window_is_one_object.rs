//! # G_FRAME_WINDOW_IS_ONE_OBJECT — a frame's window and document are OBJECTS, not values
//!
//! **`f.contentWindow === f.contentWindow` was FALSE. So was `f.contentDocument ===
//! f.contentDocument`.** Both minted a fresh object on every read, so:
//!
//! * anything a script stashes on the frame's window — a ready flag, a message-port handle, a
//!   resize callback, the bookkeeping every embed and OAuth frame keeps — was written to an object
//!   discarded on the next line;
//! * `e.target.ownerDocument === frame.contentDocument`, the standard "is this event from my
//!   frame?" test, was **never true**;
//! * a `WeakMap`/`Set` keyed on the frame's document gained one entry per read.
//!
//! This is the **fourth** place in this window of ticks where a live view was rebuilt per access
//! instead of refreshed in place — `sheet.cssRules` (t1191) was the same defect one subsystem over,
//! and `el.sheet` had the rule written down correctly all along: *"ONE object per element … a
//! library that stashes bookkeeping on the sheet loses it otherwise."*
//!
//! **Live AND stable, again.** The cached window exposes `document` as a **getter**, not the value
//! captured when it was built: a frame that navigates gets a new document, and caching the value
//! would have bought identity by making the window permanently stale.
//!
//! `defaultView` is only implementable *because* `contentDocument` identity now holds — the owning
//! frame is found by comparing documents, and that comparison used to be false against the very
//! document it was handed.
//!
//! ## ⚠ WHAT THIS GATE DELIBERATELY DOES NOT ADD — `contentWindow.getComputedStyle`
//!
//! Measured this tick and **left absent on purpose**: `getComputedStyle` does not work on an
//! element in a frame's document *at all*. `STYLES_PTR` is a single thread-local holding ONE
//! page's style map, and `window_get_computed_style` keeps only the `NodeId` from `node_and_dom`,
//! discarding the arena. A child node is therefore looked up in the PARENT's map, which yields
//! either nothing or **the parent's node with that id, with total confidence** — the same
//! wrong-answer-of-the-right-type shape `el_content_document`'s own doc comment describes.
//! Measured on a frame whose stylesheet sets `visibility:hidden`:
//!
//! ```text
//!   gcsHidden = visible     ← the child's own stylesheet never reached it
//!   gcsPlain  = undefined   ← a plain child element has no entry at all
//! ```
//!
//! Putting `getComputedStyle` on the frame window would convert *absent* into *silently wrong*,
//! which the reliability doctrine ranks strictly worse (false-presence). It stays off until the
//! style lookup is arena-aware. **That is the next tick, and it is worth ~480 subtests in
//! `css/selectors/attribute-selectors/attribute-case` alone**, where the helper iterates
//! `[window, quirks, xml]` — two of them frame windows — and dies on
//! `global.getComputedStyle is not a function`.
//!
//! ## RED probes run against this gate
//!
//! | mutation | result |
//! |---|---|
//! Both were run, quoted with the values they produced.
//!
//! | mutation | result |
//! |---|---|
//! | remove the `__manukWin` cache | RED — `winIdentity=false`, `stash=undefined`, and `defaultView=[object Object]` (a *different* window, so the `===` fails) |
//! | remove the node-cache consult in `el_content_document` | RED — **five claims fall together**: `docIdentity=false docEqVar=false docThroughWin=false defaultView=null ownerIsFrameDoc=false`. This is the probe worth keeping: it demonstrates the dependency the gate documents — `defaultView` is findable ONLY because document identity holds, so removing identity silently removes the view too |

use manuk_text::FontContext;

#[test]
fn a_frames_window_and_document_have_stable_identity() {
    let fonts = FontContext::new();
    let mut fr = manuk_page::Page::load(
        r#"<!doctype html><html><body>
             <iframe src="https://embed.test/c" id="f"></iframe>
             <script>window.__x = 1;</script>
           </body></html>"#,
        "https://parent.test/",
        &fonts,
        900.0,
    );
    let froot = fr.dom().root();
    let fnode = manuk_css::query_selector_all(fr.dom(), froot, "#f")[0];
    fr.render_iframe(
        fnode,
        r#"<!doctype html><html><body><div id="d">x</div></body></html>"#,
        "https://embed.test/c",
        &fonts,
        0,
    );
    fr.eval_for_test(
        r#"var r = [];
           var f = document.getElementById('f');

           // ── 1. IDENTITY, both sides.
           r.push('winIdentity=' + (f.contentWindow === f.contentWindow));
           r.push('docIdentity=' + (f.contentDocument === f.contentDocument));
           var d = f.contentDocument;
           r.push('docEqVar=' + (d === f.contentDocument));

           // ── 2. What identity is FOR: state stashed on the window must survive.
           f.contentWindow.__ready = 'yes';
           r.push('stash=' + f.contentWindow.__ready);

           // ── 3. The window's document is the frame's document, reached through the window.
           r.push('docThroughWin=' + (f.contentWindow.document === d));

           // ── 4. `defaultView` — only findable because docIdentity holds.
           r.push('defaultView=' + (d.defaultView === f.contentWindow ? 'win' : String(d.defaultView)));
           r.push('mainView=' + (document.defaultView === globalThis));
           r.push('detachedView=' + document.implementation.createHTMLDocument('').defaultView);

           // ── 5. THE RATCHET CLAUSE — the child's own nodes still resolve, and node identity
           //    across the boundary still holds (the t776 contract).
           r.push('childDiv=' + (d.getElementById('d') ? d.getElementById('d').id : 'MISSING'));
           r.push('nodeIdentity=' + (d.getElementById('d') === f.contentDocument.getElementById('d')));
           r.push('ownerIsFrameDoc=' + (d.getElementById('d').ownerDocument === d));
           r.push('notMainDoc=' + (d !== document));

           var s = document.createElement('script'); s.id = '__fw__'; s.type = 'application/json';
           s.textContent = r.join(' '); document.documentElement.appendChild(s);"#,
    );
    let dom = fr.dom();
    let out = manuk_css::query_selector_all(dom, dom.root(), "#__fw__");
    let got = out
        .first()
        .map(|&n| dom.text_content(n))
        .unwrap_or_default();
    println!("FRAME WINDOW: {got}");

    for (claim, why) in CLAIMS {
        assert!(
            got.contains(claim),
            "G_FRAME_WINDOW_IS_ONE_OBJECT: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}

const CLAIMS: &[(&str, &str)] = &[
    (
        "winIdentity=true",
        "THE LOAD-BEARING CLAIM. `f.contentWindow === f.contentWindow` was FALSE — a fresh object \
         literal per read. Every embed, OAuth frame and payment field keeps bookkeeping on that \
         window",
    ),
    (
        "docIdentity=true",
        "and the same for `contentDocument`, which is what `e.target.ownerDocument === \
         frame.contentDocument` — the standard 'is this event from my frame?' test — compares",
    ),
    (
        "docEqVar=true",
        "stated separately: a document held in a variable must equal a fresh read, which is the \
         form the comparison actually takes in real code",
    ),
    (
        "stash=yes",
        "⚠ WHAT IDENTITY IS FOR. Writing a flag to the window and reading it back is the whole \
         point; two `===`-true claims could in principle be met by an object that is never written \
         to. This is the claim that would catch a fix that returned a frozen shared singleton",
    ),
    (
        "docThroughWin=true",
        "the window's `document` is the frame's document. ⚠ It is a GETTER, not the value captured \
         when the window was built — caching the value would buy identity by making the window \
         permanently stale across a frame navigation, which is the live-and-stable pair again",
    ),
    (
        "defaultView=win",
        "a framed document's view is its frame's window. It was a flat `null` for every document \
         that was not the singleton, so `d.defaultView.postMessage(…)` — how a script addresses \
         its own frame from the inside — died. Only implementable BECAUSE docIdentity holds: the \
         owning frame is found by comparing documents",
    ),
    (
        "mainView=true",
        "THE RATCHET CLAUSE for the main document — `document.defaultView === window` must not move",
    ),
    (
        "detachedView=null",
        "and a document with no frame genuinely has no view. `null` is the spec's answer, not a \
         fallback — a search that returned the main window for any unmatched document would \
         satisfy `defaultView` and be wrong everywhere else",
    ),
    ("childDiv=d", "the child's own nodes still resolve through the cached document"),
    (
        "nodeIdentity=true",
        "and node identity across the document boundary still holds — the t776 contract, re-asserted \
         because this tick changed how the document reflector is produced",
    ),
    (
        "ownerIsFrameDoc=true",
        "a child node's `ownerDocument` is the frame's document — the property that broke DOMPurify \
         when it was wrong, now sharing the cache the fix seeds",
    ),
    ("notMainDoc=true", "and the frame's document is still not the parent's"),
];
