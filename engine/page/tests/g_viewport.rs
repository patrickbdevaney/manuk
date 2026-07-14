//! **G_VIEWPORT — the live viewport, and the whole lazy-load loop it unlocks.**
//!
//! The platform map called this *"the single biggest breadth-per-tick item on the board"*, because **one**
//! missing primitive blocks **five** features at once: lazy-loading, list virtualization, sticky headers,
//! scroll-linked animation, and infinite scroll. They are not five gaps. They are one gap, seen five times.
//!
//! **It turned out to be already built — and nothing proved it.** A probe written before implementing
//! anything (the methodology's own first rule) found the entire chain working end to end. That is the
//! *fourth* time this project has come within a tick of rebuilding a feature it already had
//! (`localStorage`, `FormData`, `position: sticky`, and now this), and the reason is always the same:
//!
//! > **An absent measurement is not a negative measurement.** *"The ledger says it's missing"* is a claim,
//! > and claims get verified. **A capability with no gate is indistinguishable from a capability that does
//! > not exist** — so this file exists to make the difference visible, permanently.
//!
//! What it asserts, which is the complete loop a real lazy-loading feed depends on:
//!
//!   1. the viewport moving **tells the page** — `window.scrollY` updates and `scroll` fires;
//!   2. an **`IntersectionObserver` actually FIRES** when its target crosses into view (this is the
//!      primitive *most* modern content-loading is built on — not `scroll` handlers);
//!   3. the observer's callback sets `img.src` from `data-src` — **the universal lazy-load pattern**;
//!   4. and **the engine NOTICES that new URL and queues it for fetching.** Step 4 is the one everybody
//!      forgets: an engine that fires the observer but never fetches what the observer asked for has
//!      implemented the *appearance* of lazy-loading and none of it.

use manuk_text::FontContext;

/// A tall page with an image far below the fold — exactly the shape of every feed on the web.
const HTML: &str = r#"<!doctype html><html><body style="margin:0">
  <div id="out">-</div>
  <div style="height:2000px">spacer</div>
  <img id="lazy" data-src="https://cdn.test/below-the-fold.png" loading="lazy"
       style="width:50px;height:50px">
  <script>
    var R = [], seen = [];
    R.push('ioExists:' + (typeof IntersectionObserver === 'function'));
    R.push('scrollY0:' + window.scrollY);          // before the scroll: the top of the document

    var io = new IntersectionObserver(function (entries) {
      entries.forEach(function (e) {
        if (e.isIntersecting) {
          seen.push('io-fired');
          var img = e.target;
          img.src = img.getAttribute('data-src');  // THE universal lazy-load pattern
        }
      });
    });
    io.observe(document.getElementById('lazy'));
    window.addEventListener('scroll', function () { seen.push('scroll@' + Math.round(window.scrollY)); });

    globalThis.__report = function () {
      R.push('scrollY1:' + Math.round(window.scrollY));
      R.push('src:' + (document.getElementById('lazy').getAttribute('src') || 'NONE'));
      R.push('seen:' + seen.join(','));
      document.getElementById('out').textContent = R.join(' ');
    };
  </script></body></html>"#;

#[test]
fn the_viewport_moves_the_observer_fires_and_the_engine_fetches_what_it_asked_for() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://viewport.test/", &fonts, 800.0);

    // The user scrolls. The image is now on screen.
    page.view_changed(2000.0, 800.0, 600.0, true);
    page.eval_for_test("globalThis.__report && __report()");

    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        "ioExists:true",
        "scrollY0:0",    // the page starts at the top…
        "scrollY1:2000", // …and the page is TOLD when that changes
        "seen:io-fired,scroll@2000",
        "src:https://cdn.test/below-the-fold.png", // the observer swapped data-src → src
    ] {
        assert!(
            got.contains(claim),
            "G_VIEWPORT: expected `{claim}`\n  got: {got}\n\n  \
             One missing primitive here blocks FIVE features at once — lazy-loading, virtualization, \
             sticky, scroll-linked animation, infinite scroll. `seen:` without `io-fired` means the \
             observer never ran, and every feed on the web is built on it."
        );
    }

    // ── STEP 4, and it is the one everybody forgets.
    //
    // Firing the observer is not lazy-loading. The engine must NOTICE the URL the callback wrote and go
    // and fetch it. An engine that does step 3 and not step 4 has implemented the *appearance* of
    // lazy-loading and none of the substance: the image is requested by the page and never arrives.
    let pending = page.pending_image_urls();
    assert!(
        pending.iter().any(|u| u.contains("below-the-fold.png")),
        "G_VIEWPORT: the IntersectionObserver set `img.src`, and the engine did NOT queue that URL for \
         fetching.\n  pending: {pending:?}\n\n  \
         The observer firing is not the feature. The IMAGE ARRIVING is the feature."
    );
}
