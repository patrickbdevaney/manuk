//! **G_FULLSCREEN — `element.requestFullscreen()` must be a real DOM fullscreen state machine, not
//! an absent method.**
//!
//! Every video player, slide deck, browser game and image lightbox toggles fullscreen with
//! `el.requestFullscreen()` from a click. When the method is missing it is the silent-handler
//! failure this project keeps naming: `requestFullscreen` is `undefined`, `undefined()` throws out of
//! the click handler, the fullscreen button does nothing, and the throw can take the rest of the
//! handler down with it. The page cannot see a *missing* method coming — it does not feature-detect
//! `requestFullscreen`, it assumes it.
//!
//! The claims are the page-OBSERVABLE contract, which is the whole surface this API exposes to script:
//!
//!   * **`requestFullscreen()` returns a promise** and does not throw.
//!   * **`document.fullscreenElement`** becomes the element, and is `null` before and after.
//!   * **`fullscreenchange` fires on the document** — the event players listen for to swap in their
//!     fullscreen controls — asynchronously, not inline.
//!   * **`exitFullscreen()`** clears the state and fires a second `fullscreenchange`.
//!   * **The webkit-prefixed aliases** resolve to the same state (players feature-detect them first).
//!
//! HONEST LIMIT (documented, not hidden): the OS window is the shell's to resize. This models the DOM
//! fullscreen *state* — all a page can read through this API — not the compositor, and `:fullscreen`
//! CSS matching is a separate cascade concern. That is not the canvas-stub "told yes, renders blank"
//! shape: the player's own content enters its fullscreen view off this state; only the browser window
//! is unchanged, which no page can observe through this API.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body><div id="out">-</div><div id="box">x</div><script>
    var r = [];
    var out = document.getElementById('out');
    var box = document.getElementById('box');

    r.push('has:' + (typeof box.requestFullscreen === 'function'));
    r.push('enabled:' + (document.fullscreenEnabled === true));
    r.push('initial:' + (document.fullscreenElement === null));

    var changes = 0;
    document.addEventListener('fullscreenchange', function () {
        changes++;
        if (changes === 1) {
            r.push('entered:' + (document.fullscreenElement === box));
            r.push('webkit:' + (document.webkitFullscreenElement === box));
            document.exitFullscreen();
        } else if (changes === 2) {
            r.push('exited:' + (document.fullscreenElement === null));
            out.textContent = r.join(' ');
        }
    });

    var p = box.requestFullscreen();
    // The event is asynchronous: `changes` is still 0 here, so a synchronous shim would already
    // have pushed 'entered' before this line.
    r.push('async:' + (changes === 0));
    r.push('promise:' + (p && typeof p.then === 'function'));
  </script></body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn fullscreen_is_a_real_dom_state_machine() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://fullscreen.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        "has:true",     // requestFullscreen is a real method, not undefined
        "enabled:true", // fullscreenEnabled reports the capability
        "initial:true", // fullscreenElement is null before any request
        "async:true",   // fullscreenchange is delivered on a microtask, never inline
        "promise:true", // requestFullscreen returns a promise
        "entered:true", // fullscreenElement becomes the element after the request
        "webkit:true",  // the webkit-prefixed accessor sees the same state
        "exited:true",  // exitFullscreen clears the state (and fired a second change)
    ] {
        assert!(
            got.contains(claim),
            "G_FULLSCREEN: expected {claim} in {got:?}\n  \
             `element.requestFullscreen()` must be a real DOM fullscreen state machine — a resolved \
             promise, a truthful `document.fullscreenElement`, and the `fullscreenchange` event. Its \
             absence is not a reported failure: the fullscreen button throws inside its own click \
             handler and does nothing."
        );
    }
}
