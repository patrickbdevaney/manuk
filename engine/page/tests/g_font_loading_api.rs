//! **G_FONT_LOADING_API — `await document.fonts.ready` is on 27.72% of page loads and `document.fonts`
//! was `undefined`, so the standard "wait for webfonts, THEN measure" prologue threw.**
//!
//! Measured live on `www.welt.de` at tick 719, as an unhandled rejection that killed its boot:
//!
//! ```text
//!   can't access property "ready", document.fonts is undefined
//!     o@module.js:1:8922   init@module.js:1:9205   jo@module.js:39:42277
//! ```
//!
//! The usage number is from the Blink use-counter dump (surface audit #32), and the map's row has
//! carried the reason it is Bar-0 adjacent since then: *"a missing property throws, **and a
//! never-resolving promise HANGS the app**."*
//!
//! ⚠⚠ **That second clause is the whole design.** The dangerous direction is a promise that never
//! settles, not one that settles early — so `ready` is a RESOLVED promise rather than one wired to a
//! loading signal this engine does not expose. Faces are loaded during the load phase, before page
//! script runs, so *"the fonts are done"* is true by the time anything can ask.
//!
//! **Chrome-measured semantics, one fixture. The non-obvious rows are the point:**
//!
//! ```text
//!                                      CHROME    MANUK
//!   typeof document.fonts              object    object
//!   status                             loaded    loaded
//!   check('16px sans-serif')           true      true
//!   check('16px NoSuchFamily')         TRUE      true     <- an UNKNOWN family needs no loading
//!   check('notafont')                  SyntaxError  SyntaxError
//!   ready resolves to the SET          true      true
//!   ── documented scope, not parity ──────────────────────
//!   size (one @font-face declared)     1         0
//!   check('16px Fake')  (declared)     false     true
//!   typeof FontFace                    function  undefined
//! ```
//!
//! ⚠ **The three divergences are stated, not hidden.** `size`/iteration do not model `FontFace`
//! objects: a page that ENUMERATES the set gets nothing, which is visibly empty, whereas a fabricated
//! entry would be believed. And `check` answers `true` even for a declared face that failed to load,
//! because **`false` is the answer that makes a page wait**, and waiting is the failure mode this
//! whole block exists to remove.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><head><style>
 @font-face { font-family: Fake; src: url(data:font/woff2;base64,AA) }
</style></head><body>
 <div id="out">-</div>
 <script>
   window.addEventListener('load', function () {
     function T(n, f) { try { return n + '=' + f(); } catch (e) { return n + '=' + e.name; } }
     var d = document.fonts;
     var parts = [
       T('type', function () { return typeof d; }),
       T('status', function () { return d.status; }),
       T('check_sans', function () { return d.check('16px sans-serif'); }),
       T('check_missing', function () { return d.check('16px NoSuchFamily'); }),
       T('check_bad', function () { return d.check('notafont'); }),
       T('load', function () { return typeof d.load; }),
       T('forEach', function () { return typeof d.forEach; }),
       T('iterable', function () { return typeof d[Symbol.iterator]; })
     ];
     // The load-bearing assertion, and it must be observed by ACTUALLY AWAITING — a `ready` that is
     // a thenable but never settles passes every `typeof` check above and hangs the page.
     var settled = false;
     d.ready.then(function (v) {
       settled = true;
       document.getElementById('out').textContent =
         parts.concat(['readySettled=true', 'readyIsSet=' + (v === d)]).join(' ');
     });
     setTimeout(function () {
       if (!settled) {
         document.getElementById('out').textContent =
           parts.concat(['readySettled=NEVER']).join(' ');
       }
     }, 0);
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
fn document_fonts_ready_settles_and_the_set_answers() {
    let fonts = FontContext::new();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let page = rt.block_on(async {
        let mut p = manuk_page::Page::load_async(HTML, "https://fonts.test/", &fonts, 800.0).await;
        p.finish_loading(&fonts, 800.0).await;
        p
    });
    let got = out(&page);
    println!("FONT-LOADING {got}");
    let has = |s: &str| got.contains(s);

    // (1) **THE ONE THAT MATTERS: `ready` SETTLES.** RED: define `ready` as `new Promise(function(){})`
    // — a thenable that never resolves → `readySettled=NEVER`. That mutation passes every `typeof`
    // assertion in this gate and is the exact failure the map calls Bar-0 adjacent, so it is asserted
    // by AWAITING rather than by inspecting.
    assert!(
        has("readySettled=true"),
        "`document.fonts.ready` never settled. A promise that never resolves does not degrade — it \
         HANGS the page at the line before it measures text — got {got:?}"
    );

    // (2) **…and it resolves to the SET**, per spec: `fonts.ready.then(s => s.check(...))` is a real
    // idiom and resolving to `undefined` breaks it one line after the wait succeeded.
    assert!(
        has("readyIsSet=true"),
        "`ready` must resolve to the FontFaceSet itself — got {got:?}"
    );

    // (3) **The surface a page touches before it awaits.** RED: remove the block → `type=undefined`
    // and every row below it throws, which is what shipped and what killed welt.de's boot.
    assert!(
        has("type=object")
            && has("status=loaded")
            && has("load=function")
            && has("forEach=function")
            && has("iterable=function"),
        "the FontFaceSet surface is incomplete — got {got:?}"
    );

    // (4) **`check` is not a stub that answers `true` to anything.** Chrome throws SyntaxError on an
    // argument that is not a CSS `font` shorthand, and a typo'd call that returns a confident `true`
    // is a page that never finds out. RED: drop the shorthand validation → `check_bad=true`.
    assert!(
        has("check_sans=true") && has("check_missing=true") && has("check_bad=SyntaxError"),
        "`check` must answer true for a resolvable font, true for an UNKNOWN family (it needs no \
         loading — Chrome-measured), and THROW SyntaxError on a non-shorthand — got {got:?}"
    );
}
