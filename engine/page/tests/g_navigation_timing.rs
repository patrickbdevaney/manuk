//! **G_NAVIGATION_TIMING — `performance.getEntriesByType('navigation')` returned `[]`, so
//! `entries[0].loadEventEnd` threw on the first line of every RUM library.**
//!
//! `getEntriesByType` answered `[]` for every argument. That is the modern, non-deprecated
//! replacement for `performance.timing`, and reading `[0]` of it is usually the *first thing*
//! web-vitals, Google Analytics, Sentry and Datadog do. Chrome-measured: `length` is **1**; ours was
//! **0**, so `entries[0]` was `undefined` and the next property read threw.
//!
//! The instants come from `__fireDOMContentLoaded` / `__fireLoad` — the host calls those, and the
//! host is the only part of the system that knows when *"the document finished parsing"* and *"the
//! subresources finished"* are true. ⚠ They are recorded **after** dispatch, because the span a
//! library wants is *"how long did my handlers take"*, and recording before dispatch reports zero for
//! every page.
//!
//! ```text
//!                             CHROME     MANUK
//!   length                      1          1
//!   entryType / type       navigation/navigate  same
//!   startTime                   0          0
//!   domContentLoadedEventEnd  > 0        > 0
//!   loadEventEnd              > 0        > 0
//!   monotonically ordered      true      true
//!   duration                  > 0        > 0
//!   ── the deliberate divergence ────────────────────
//!   typeof responseStart      number   undefined
//! ```
//!
//! ⚠⚠ **The network-phase fields are ABSENT, not zero.** We do not observe
//! `responseStart`/`domainLookupEnd`/`connectEnd` at this layer, and a `0` there is
//! indistinguishable from a real 0ms — a library would report a confident, wrong TTFB and nobody
//! would ever find out. `undefined` propagates to `NaN` through the arithmetic every one of them
//! does, which is **loud**. Assertion (4) pins that choice so a future "fill in the zeros" cannot
//! land quietly.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body>
 <div id="out">-</div>
 <script>
   window.addEventListener('load', function () {
     // A task AFTER load, because `loadEventEnd` is recorded when the load handlers finish — reading
     // it from inside one would be reading a value that does not exist yet, and would pass on a
     // constant.
     setTimeout(function () {
       function T(n, f) { try { return n + '=' + f(); } catch (e) { return n + '=' + e.name; } }
       var e = performance.getEntriesByType('navigation')[0];
       document.getElementById('out').textContent = [
         T('len', function () { return performance.getEntriesByType('navigation').length; }),
         T('entryType', function () { return e.entryType; }),
         T('type', function () { return e.type; }),
         T('startTime', function () { return e.startTime; }),
         T('name_nonempty', function () { return e.name.length > 0; }),
         T('dclEnd_gt0', function () { return e.domContentLoadedEventEnd > 0; }),
         T('loadEnd_gt0', function () { return e.loadEventEnd > 0; }),
         T('ordered', function () {
           return e.domInteractive <= e.domContentLoadedEventEnd
               && e.domContentLoadedEventEnd <= e.loadEventEnd;
         }),
         T('duration_gt0', function () { return e.duration > 0; }),
         T('responseStart', function () { return typeof e.responseStart; }),
         // A type this engine does not collect must still be the EMPTY array, never the navigation
         // entry — `getEntriesByType('resource')` returning a navigation entry would be worse than
         // returning nothing.
         T('resource_empty', function () { return performance.getEntriesByType('resource').length; })
       ].join(' ');
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
fn the_navigation_timing_entry_exists_and_its_instants_are_real() {
    let fonts = FontContext::new();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let page = rt.block_on(async {
        let mut p =
            manuk_page::Page::load_async(HTML, "https://navtiming.test/", &fonts, 800.0).await;
        p.finish_loading(&fonts, 800.0).await;
        p
    });
    let got = out(&page);
    println!("NAV-TIMING {got}");
    let has = |s: &str| got.contains(s);

    // (1) **There is an entry at all.** RED: return `[]` from `getEntriesByType('navigation')` →
    // `len=0` and every row below reads `TypeError`, which is what shipped.
    assert!(
        has("len=1")
            && has("entryType=navigation")
            && has("type=navigate")
            && has("startTime=0")
            && has("name_nonempty=true"),
        "the navigation entry is missing or malformed — got {got:?}"
    );

    // (2) **The instants are MEASURED, not constants.** The ordering is what proves it: three
    // separately-recorded times from three different moments of the load, and they must come out
    // monotonic. A stub returning fixed numbers can satisfy `> 0` and will not satisfy this unless
    // it happens to order them, and a stub returning zeros fails both.
    assert!(
        has("dclEnd_gt0=true")
            && has("loadEnd_gt0=true")
            && has("ordered=true")
            && has("duration_gt0=true"),
        "the navigation timings are not real, ordered measurements — got {got:?}. \
         domInteractive <= domContentLoadedEventEnd <= loadEventEnd is what separates a recorded \
         instant from a plausible constant."
    );

    // (3) **A type we do not collect is the EMPTY array**, not this entry. RED: return the entry for
    // every type → `resource_empty=1`, and a library asking for resource timings gets a navigation
    // record, which is worse than getting nothing.
    assert!(
        has("resource_empty=0"),
        "`getEntriesByType('resource')` must be empty, not the navigation entry — got {got:?}"
    );

    // (4) **THE DELIBERATE NON-CLAIM, PINNED.** `responseStart` and the other network-phase fields
    // are ABSENT, not zero: a `0` is indistinguishable from a real 0ms and a library would report a
    // confident, wrong TTFB. `undefined` propagates to `NaN`, which is loud. RED: add
    // `responseStart: 0` → `responseStart=number` and this fires. If the network phases become
    // observable and these are filled in with REAL values, delete this assertion and say so.
    assert!(
        has("responseStart=undefined"),
        "`responseStart` is present. The network-phase fields are deliberately ABSENT rather than \
         zero — a plausible value is worse than an honest absence here, because nobody can tell a \
         fabricated 0ms TTFB from a real one. If these are now REAL, update the gate — got {got:?}"
    );
}
