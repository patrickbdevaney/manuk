//! **G_ROOT_CLIENT_BOX — the viewport, asked the way the web actually asks for it.**
//!
//! Its own binary because a `Page` owns a SpiderMonkey runtime and two of them in one test process
//! abort on teardown (*"There are outstanding JS engine handles"*) — the same shared-runtime reuse
//! the WPT runner reports as its `ACCUM` bucket. This gate needs a document of its own shape, so it
//! gets a process of its own.

use manuk_text::FontContext;

/// **G_ROOT_CLIENT_BOX_IS_THE_VIEWPORT — `documentElement.clientHeight` was the height of the whole
/// DOCUMENT, and that one number is what decides "is this on screen?" for the entire web.**
///
/// CSSOM-View gives the root element its own rule: *"if the element is the root element and the
/// document is not in quirks mode … return the viewport width/height excluding the size of a
/// rendered scroll bar."* Every OTHER element reports its padding box — so the one element with a
/// different rule was being handed the general one, and reported its own box.
///
/// Measured against headless Chrome, viewport 800×800 over a 5,000px document:
///
/// ```text
///                                    Chrome      before
///     documentElement.clientWidth       800         784
///     documentElement.clientHeight      800        5800   <- the DOCUMENT height
///     100vw / 100vh                800 / 800   800 / 800  <- already right, which is what hid it
/// ```
///
/// ⭐ **`vw`/`vh` were correct the whole time**, from the same `viewport_size()` this now reads. The
/// CSS half of the viewport was right in every test that looked; only the CSSOM half was wrong, and
/// nothing compared the two.
///
/// The third claim below is the one that matters in the field. `scrollTop + clientHeight >=
/// scrollHeight` is *the* infinite-scroll test, and with `clientHeight` equal to the document height
/// it is **true on the first frame of every page** — the feed loads its next page before the user
/// has scrolled, the lazy-loader fetches every image at once, and a virtualised list divides by a
/// screen height that is the whole list and renders all of it.
///
/// **To watch it go RED:** delete the root-element arm at the end of
/// `manuk_page::scroll_geometry_of` — the fallback then hands back the `<html>` box, which is what
/// every browser build before t1320 did.
#[test]
fn the_root_elements_client_box_is_the_viewport_not_the_document() {
    const TALL: &str = r#"<!doctype html><html><body style="margin:0">
      <div id="out">-</div>
      <div style="height:5000px">tall</div>
      <script>globalThis.__report = function () {
        var de = document.documentElement;
        document.getElementById('out').textContent =
          'cw:' + de.clientWidth + ' ch:' + de.clientHeight +
          ' sh:' + de.scrollHeight +
          ' innerW:' + window.innerWidth + ' innerH:' + window.innerHeight +
          ' atEnd:' + (de.scrollTop + de.clientHeight >= de.scrollHeight);
      };</script></body></html>"#;

    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(TALL, "https://viewport.test/", &fonts, 800.0);
    // Establish a viewport the way a host does, then report. Nothing scrolls: the claim is about
    // the FIRST frame, which is when the lazy-loader and the feed both make their decision.
    page.view_changed(0.0, 800.0, 600.0, true);
    page.eval_for_test("globalThis.__report && __report()");

    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    // The root's client box IS the viewport — the same viewport `window.innerWidth/Height` reports.
    // Asserting the two AGREE rather than asserting a literal is deliberate: a host is free to pick
    // any viewport, and what must never happen is the engine answering the same question two ways.
    let n = |k: &str| -> f64 {
        got.split(k)
            .nth(1)
            .and_then(|r| r.split(' ').next())
            .and_then(|v| v.parse().ok())
            .unwrap_or(-1.0)
    };
    assert_eq!(
        (n("cw:"), n("ch:")),
        (n("innerW:"), n("innerH:")),
        "G_ROOT_CLIENT_BOX: documentElement.clientWidth/Height must be the VIEWPORT, and \
         `window.innerWidth/Height` is the same viewport asked a different way.\n  got: {got}"
    );
    // …and it is emphatically NOT the document, which is 5,000px of filler tall.
    assert!(
        n("ch:") > 0.0 && n("ch:") < 1000.0,
        "G_ROOT_CLIENT_BOX: clientHeight is {} on a 5,000px document — that is the <html> box, not \
         the viewport.\n  got: {got}",
        n("ch:")
    );
    // The field consequence, stated as a claim rather than left implied.
    assert!(
        got.contains("atEnd:false"),
        "G_ROOT_CLIENT_BOX: `scrollTop + clientHeight >= scrollHeight` is TRUE at the top of a \
         5,000px document. That is the infinite-scroll test every feed on the web runs, and it \
         fires on the first frame — before the user has scrolled at all.\n  got: {got}"
    );
}
