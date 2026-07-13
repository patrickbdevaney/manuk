//! **G_DEDUP — the same work must not be done twice for one navigation.**
//!
//! This gate has a body count. Tick 25 measured, on real sites:
//!
//! ```text
//! nytimes.com      813 fetches, 507 DUPLICATE   (62%)
//! theguardian.com  576 fetches, 431 DUPLICATE   (75%)
//! ```
//!
//! The cause was one line: the image skip-list was built from `self.images` — the map of *successfully
//! decoded* images — so an image that FAILED (a blocked tracker, a 404, a timeout) was never recorded,
//! and was therefore re-fetched on every one of the six script rounds. A news front page is *made* of
//! images that fail.
//!
//! It is fixed. **Nothing stopped it coming back**, and that is the entire reason this file exists.
//! Before adding a feature, name the gate that would have gone red if it were already broken — this one
//! would have been red for months, on the single most-visited class of site on the web.
//!
//! What it asserts, for ONE navigation of a page that references the same resources repeatedly:
//!
//!   1. **No duplicate fetches.** The same URL is not requested twice.
//!   2. **A FAILED subresource is not retried on every round.** This is the specific regression above,
//!      and it is tested with a URL that cannot resolve, precisely because a *success* would have been
//!      remembered by the old code too and would not have caught it.
//!   3. **Bounded passes.** Layout and cascade run a small number of times, not once per script round.
//!
//! The thresholds are deliberately loose. This gate is a **ratchet against a storm**, not a
//! micro-benchmark: it must fire on "we re-fetch everything six times", and it must not fire because a
//! legitimate second pass was added. A gate that cries wolf gets disabled, and then it protects nothing.

use manuk_text::FontContext;
use std::sync::atomic::Ordering;

/// A page that names the same subresources many times, and whose scripts mutate the DOM enough to
/// trigger several render rounds. A correct engine fetches each distinct URL exactly once.
const HTML: &str = r#"<!doctype html><html><head>
  <link rel="stylesheet" href="/style.css">
  <link rel="stylesheet" href="/style.css">
</head><body>
  <img src="/a.png"><img src="/a.png"><img src="/a.png">
  <img src="/missing-tracker.gif">
  <video poster="/a.png" width="100" height="60"></video>
  <div id="out"></div>
  <script>
    // Several rounds of DOM mutation. Each one gives the engine an opportunity to re-fetch, and the
    // old code took every one of them — for the images that had FAILED.
    for (var i = 0; i < 5; i++) {
      var d = document.createElement('div');
      d.innerHTML = '<img src="/a.png"><img src="/missing-tracker.gif">';
      document.getElementById('out').appendChild(d);
    }
  </script>
</body></html>"#;

#[test]
fn g_dedup_no_duplicate_work_for_one_navigation() {
    // **The first version of this gate passed while measuring NOTHING.**
    //
    // It called `Page::load` — the synchronous path, which parses, cascades and lays out but never
    // fetches a subresource. So `FETCHES` was 0, `FETCH_DUPES` was 0, `assert_eq!(dupes, 0)` passed,
    // and the gate protected nothing at all. It would have been green through the entire nytimes bug.
    //
    // A gate that passes vacuously is worse than no gate, because it is *trusted*. The fetches only
    // happen on `load_async` + `finish_loading`, driven inside a runtime — so that is what this drives.
    std::env::set_var("MANUK_NET_TIMEOUT_MS", "700");
    std::env::set_var("MANUK_LOAD_BUDGET_MS", "5000");

    let fonts = FontContext::new();

    manuk_net::reset_fetch_stats();
    manuk_layout::LAYOUTS.store(0, Ordering::Relaxed);
    // The cascade counter lives behind the `stylo` feature. Guard it, or this test target fails to
    // COMPILE under `--features spidermonkey` alone — which does not fail this gate, it fails whatever
    // gate happens to be running when cargo tries to build every test target in the crate. (It took
    // G2 down, and G2 has nothing to do with dedup.)
    #[cfg(feature = "stylo")]
    manuk_css::stylo_engine::CASCADES.store(0, Ordering::Relaxed);

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    // A host that does not resolve: every subresource here FAILS. That is deliberate and it is the
    // whole point — a *failure* is what the old skip-list forgot to record, so a gate built on
    // successful fetches would not have caught the bug it exists to catch.
    let _page = rt.block_on(async {
        let mut p =
            manuk_page::Page::load_async(HTML, "http://dedup.invalid/", &fonts, 800.0).await;
        p.finish_loading(&fonts, 800.0).await;
        p
    });

    let fetches = manuk_net::FETCHES.load(Ordering::Relaxed);
    let dupes = manuk_net::FETCH_DUPES.load(Ordering::Relaxed);
    let net = manuk_net::NET_REQUESTS.load(Ordering::Relaxed);
    let net_dupes = manuk_net::NET_DUPES.load(Ordering::Relaxed);
    let layouts = manuk_layout::LAYOUTS.load(Ordering::Relaxed);
    #[cfg(feature = "stylo")]
    let cascades = manuk_css::stylo_engine::CASCADES.load(Ordering::Relaxed);
    #[cfg(not(feature = "stylo"))]
    let cascades = 0usize;

    eprintln!(
        "  fetches {fetches} (repeat calls {dupes}) · NETWORK {net} (dupes {net_dupes}) \
         · layouts {layouts} · cascades {cascades}"
    );

    // **The gate must have measured something.** Otherwise it is the vacuous version again, and the
    // next person to break dedup will be told everything is fine.
    assert!(
        net >= 2,
        "G_DEDUP is VACUOUS: only {net} requests reached the network, so it cannot possibly detect a \
         duplicate. The page names 3 distinct URLs. Fix the harness, not the threshold."
    );

    // (1) THE regression. The page names `/a.png` five times, `/missing-tracker.gif` six, and
    //     `/style.css` twice. A browser fetches each ONCE per navigation.
    // **The wire, not the call.** A repeat `fetch()` served from the HTTP cache or the negative cache
    // costs no bandwidth; the same URL going to the NETWORK twice does, and on a metered connection it
    // is money. That is the number a browser must keep at zero.
    assert_eq!(
        net_dupes, 0,
        "G_DEDUP: {net_dupes} of {net} network requests were for a URL ALREADY fetched this \
         navigation. This is the nytimes bug (813 fetches, 507 duplicate): the same sprite pulled down \
         once per element that names it, and every failing tracker retried on every render round."
    );

    // (2) Bounded passes. Loose on purpose: a ratchet against a storm, not a micro-benchmark. A gate
    //     that fires on a legitimate extra pass gets disabled, and then it protects nothing.
    assert!(
        layouts <= 12,
        "G_DEDUP: {layouts} full layout passes for one navigation — render rounds are not coalescing."
    );
    assert!(
        cascades <= 12,
        "G_DEDUP: {cascades} full cascades for one navigation — the `last_cascade` fingerprint is not \
         holding."
    );
}
