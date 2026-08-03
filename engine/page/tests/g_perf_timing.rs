//! **G_PERF_TIMING — `performance.timing` is deprecated, still in every browser, and its absence
//! kills a page at module scope.**
//!
//! ⚠⚠⚠ **MEASURED on `dashboard.twitch.tv` (t866):**
//!
//! ```text
//!   TypeError: can't access property "navigationStart", performance.timing is undefined
//! ```
//!
//! …thrown from the bundle's top level, so nothing after it runs and nothing renders. The sweep
//! filed the site `thin-overlap-9` — *"the oracle rendered the page and we did not"* — which is true
//! and says nothing about why.
//!
//! **The shape is the half-installed API, one rung out.** This engine deliberately built the MODERN
//! replacement (`performance.getEntriesByType('navigation')`) and its own comment calls that "the
//! modern, non-deprecated replacement for `performance.timing`". It is — but the deprecated one is
//! still in every shipping browser, and the code written against it did not disappear when the
//! replacement landed. A page's feature-detect finds `performance`, succeeds, and the very next
//! property read throws.
//!
//! **The expectations here are Chrome's, captured from `chromium --dump-dom` on an equivalent
//! fixture** — including which fields Chrome reports as **0** (`redirect*`, `unload*`,
//! `secureConnectionStart` on a same-origin navigation with no redirect), which is what makes 0 an
//! answer rather than a stand-in.
//!
//! **Teeth:**
//! * `navigationStart` is real, absolute epoch milliseconds — the failing read itself;
//! * the two views of one dataset AGREE: `timing.loadEventEnd` is the navigation entry's
//!   `loadEventEnd` plus `timeOrigin`, because they are accessors over one source, not two copies;
//! * an unobserved network phase stays **ABSENT**, not 0 — a `0` there is indistinguishable from a
//!   real 0ms and makes every RUM library report a confident, wrong TTFB;
//! * `performance.navigation.type` exists — the same interface's other half and the same throw.
//!
//! Proven RED: with `timing` absent the fixture's first line throws and `#out` never changes.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<script>
  var R = [];
  // THE FAILING READ, verbatim from the twitch bundle's top level.
  var ns = performance.timing.navigationStart;
  R.push('navigationStart-is-number:' + (typeof ns === 'number'));
  // Absolute epoch milliseconds, not a relative double. 1.6e12 is 2020-09; any relative value
  // (or a 0) fails this, which is the difference between the legacy and modern interfaces.
  R.push('navigationStart-is-epoch:' + (ns > 1600000000000));
  R.push('timeOrigin-agrees:' + (Math.abs(ns - performance.timeOrigin) < 2));
  R.push('brand:' + Object.prototype.toString.call(performance.timing));
  // Did not happen -> 0, per spec and per Chrome on a same-origin no-redirect navigation.
  R.push('redirectStart:' + performance.timing.redirectStart);
  R.push('unloadEventEnd:' + performance.timing.unloadEventEnd);
  R.push('secureConnectionStart:' + performance.timing.secureConnectionStart);
  // NOT OBSERVED at this layer -> absent, never 0. `undefined` propagates to NaN, which is loud;
  // a 0 is indistinguishable from a real 0ms and is silently wrong.
  R.push('responseStart-absent:' + (performance.timing.responseStart === undefined));
  R.push('fetchStart-absent:' + (performance.timing.fetchStart === undefined));
  // The other half of the same interface, and the same throw.
  R.push('navigation-type:' + performance.navigation.type);
  R.push('navigation-redirectCount:' + performance.navigation.redirectCount);
  window.__R = R;
</script>
<script>
  window.addEventListener('load', function () {
    // ONE SOURCE, TWO VIEWS: the legacy interface must be the navigation entry plus timeOrigin.
    // If these ever disagree, one of them grew its own copy of the data.
    var nav = performance.getEntriesByType('navigation')[0];
    var t = performance.timing;
    window.__R.push('views-agree:' +
      (nav.loadEventEnd
        ? Math.abs(t.loadEventEnd - (performance.timeOrigin + nav.loadEventEnd)) <= 1
        : t.loadEventEnd === 0));
    document.getElementById('out').textContent = window.__R.join(' ');
  });
</script>
</body></html>"##;

#[test]
fn the_legacy_performance_timing_interface_exists_and_agrees_with_the_modern_one() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://timing.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("PERF-TIMING RESULT: {got}");

    for (claim, why) in [
        (
            "navigationStart-is-number:true",
            "the read that kills dashboard.twitch.tv at module scope",
        ),
        (
            "navigationStart-is-epoch:true",
            "the LEGACY interface is absolute epoch ms; a relative double here would be the modern \
             interface wearing the old name",
        ),
        ("timeOrigin-agrees:true", "navigationStart IS timeOrigin, by definition"),
        (
            "brand:[object PerformanceTiming]",
            "Chrome's brand — a library that narrows on it must still narrow",
        ),
        ("redirectStart:0", "did not happen -> 0, per spec and per Chrome"),
        ("unloadEventEnd:0", ""),
        ("secureConnectionStart:0", ""),
        (
            "responseStart-absent:true",
            "NOT OBSERVED at this layer, so ABSENT — a 0 is indistinguishable from a real 0ms and \
             makes every RUM library report a confident, wrong TTFB",
        ),
        ("fetchStart-absent:true", "same rule, and it must not drift to 0 later"),
        ("navigation-type:0", "TYPE_NAVIGATE — the other half of the same interface"),
        ("navigation-redirectCount:0", ""),
        (
            "views-agree:true",
            "ONE SOURCE, TWO VIEWS: the legacy fields are ACCESSORS over the same instants the \
             navigation entry reports. A disagreement here means one of them grew its own copy",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_PERF_TIMING: expected `{claim}`{}\n  got: {got}\n\n  \
             `performance.timing` is deprecated and still present in every shipping browser. \
             Building only the modern replacement is the half-installed-API shape one rung out: the \
             feature-detect that finds `performance` succeeds, and the next property read throws. \
             Measured on dashboard.twitch.tv (t866): `TypeError: can't access property \
             \"navigationStart\", performance.timing is undefined`, at module scope, so nothing \
             renders.",
            if why.is_empty() {
                String::new()
            } else {
                format!(" — {why}")
            }
        );
    }
}
