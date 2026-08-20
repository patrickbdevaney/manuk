//! **G_ROOT_SCROLLBAR_ICB — `html { overflow: scroll }` takes the scrollbar out of the INITIAL
//! CONTAINING BLOCK, and the ICB is not the window.**
//!
//! CSS Values 4 §5.1.1 resolves every viewport-percentage unit against the initial containing block;
//! CSS Overflow puts a classic scrollbar *inside* the window and *outside* the ICB. So on a page
//! whose root element scrolls unconditionally the two differ by 15px — and `window.innerWidth` is
//! the one that keeps the window. Chrome, in a 200px-wide frame with `html { overflow: scroll }`:
//!
//! ```text
//!     100vw                          185px
//!     documentElement.clientWidth    185
//!     window.innerWidth              200      ← the window, not the ICB
//! ```
//!
//! We answered **185 nowhere and 200 everywhere**, which is why
//! `css/css-values/viewport-units-scrollbars-compute.html` scored **0/34**.
//!
//! ⚠ The split is the whole point. A gate that only checked `100vw == 185` would be satisfied by
//! narrowing *everything*, which breaks `innerWidth` — and `innerWidth` disagreeing with the window
//! is the defect the boot prelude's own comment already warns about ("it breaks every
//! canvas/virtual-list/chart sized off it"). So this asserts BOTH sides of one decision.
//!
//! Its own test binary: a `Page` owns a SpiderMonkey runtime and two in one process abort on
//! teardown ("There are outstanding JS engine handles").

use manuk_text::FontContext;

/// `overflow: scroll` on the ROOT element — a scrollbar that is always there, which is the only
/// case the spec asks us to reserve for. (`auto` is explicitly the other way: *"when the value of
/// `overflow` on the root element is `auto`, any scroll bars are assumed not to exist."*)
const HTML: &str = r#"<!doctype html><html><head><style>
  * { margin: 0 }
  html { overflow: scroll }
  #probe { width: 100vw; height: 100vh }
</style></head><body>
  <div id="probe"></div>
  <div id="out">-</div>
  <div style="height:3000px"></div>
  <script>globalThis.__report = function () {
    var de = document.documentElement;
    var p = getComputedStyle(document.getElementById('probe'));
    document.getElementById('out').textContent =
      'vw:' + p.width + ' vh:' + p.height +
      ' cw:' + de.clientWidth + ' ch:' + de.clientHeight +
      ' innerW:' + window.innerWidth + ' innerH:' + window.innerHeight;
  };</script></body></html>"#;

#[test]
fn the_root_scrollbar_comes_out_of_the_icb_and_not_out_of_the_window() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://icb.test/", &fonts, 800.0);
    page.view_changed(0.0, 800.0, 600.0, true);
    page.eval_for_test("globalThis.__report && __report()");

    let root = page.dom().root();
    let out = manuk_page::Page::dom(&page);
    let node = manuk_css::query_selector_all(out, root, "#out")[0];
    let got = out.text_content(node);

    let n = |k: &str| -> f64 {
        got.split(k)
            .nth(1)
            .and_then(|r| r.split(' ').next())
            .map(|v| v.trim_end_matches("px"))
            .and_then(|v| v.parse().ok())
            .unwrap_or(-1.0)
    };
    let gutter = f64::from(manuk_layout::scrollbar_gutter(
        manuk_css::ScrollbarWidth::Auto,
    ));
    assert!(
        gutter > 0.0,
        "the UA scrollbar metric is zero — this gate cannot measure anything"
    );

    // 1. The ICB is the window MINUS the root's gutter, and `100vw` reads it.
    assert_eq!(
        n("vw:"),
        n("innerW:") - gutter,
        "G_ROOT_SCROLLBAR_ICB: `100vw` must resolve against the ICB, which is the window less the \
         root element's {gutter}px scrollbar.\n  got: {got}"
    );
    // 2. `documentElement.clientWidth` is the SAME ICB — the CSSOM half of one question. Answering
    //    it differently from `100vw` is the defect t1320 fixed, re-introduced one layer up.
    assert_eq!(
        n("cw:"),
        n("vw:"),
        "G_ROOT_SCROLLBAR_ICB: documentElement.clientWidth and `100vw` are the same question asked \
         twice and must not disagree.\n  got: {got}"
    );
    // 3. …and `window.innerWidth` is still the WINDOW. This is the half a "just narrow it" fix
    //    silently breaks, and nothing else in the suite would notice.
    assert_eq!(
        n("innerW:"),
        800.0,
        "G_ROOT_SCROLLBAR_ICB: `window.innerWidth` must stay the window's inner width — the \
         scrollbar is INSIDE the window. Narrowing it too is how a correct ICB becomes a wrong \
         viewport for every canvas and chart sized off `innerWidth`.\n  got: {got}"
    );
    // The block axis carries no gutter here (`overflow-x` is `scroll` too, so it does — assert the
    // symmetric rule rather than a literal, so this survives a change of UA metric).
    assert_eq!(
        n("vh:"),
        n("innerH:") - gutter,
        "G_ROOT_SCROLLBAR_ICB: the block axis takes the horizontal scrollbar out of the ICB by the \
         same rule.\n  got: {got}"
    );
}
