//! **G_USER_ACTIVATION — `navigator.userActivation` exists and tracks real gesture state.**
//!
//! **The failure this gate exists for.** `navigator.userActivation` did not exist. A large and
//! growing class of gesture-gated features guards itself by reading it *inside a click handler*:
//!
//! ```js
//! button.onclick = () => {
//!     if (navigator.userActivation.isActive) video.play();   // has a fresh gesture — allowed
//!     else showPlayButton();
//! };
//! ```
//!
//! With the object `undefined`, `navigator.userActivation.isActive` is a SYNCHRONOUS `TypeError`
//! (a property read on `undefined`) thrown out of the handler. The gated action never runs, and
//! because it *threw* rather than returned falsy, the `else` fallback never runs either — the
//! button is dead. Autoplay-with-sound, `requestFullscreen`, `window.open` popups,
//! `navigator.clipboard.write`, Web Share and `PaymentRequest.show` are all commonly guarded this
//! way.
//!
//! **What is asserted is real gesture tracking, not a hardcoded value.** A constant `false` would
//! be worse than the crash: a site testing `isActive` inside its own click handler would take the
//! "no gesture" branch *during a real click*. So the two booleans reflect true state:
//!
//!   * `hasBeenActive` — STICKY: `false` until the first real gesture, `true` forever after.
//!   * `isActive` — TRANSIENT: `true` only while a real engine-originated gesture (a click the host
//!     dispatched) is being handled; `false` at load and `false` again once the handler returns.
//!   * A page's own synthetic `el.click()` / `dispatchEvent` is UNTRUSTED and grants nothing —
//!     matching the spec, and the reason the discriminator is an engine-private gesture marker, not
//!     `isTrusted` (engine mouse/key events carry a supplied object and so read `isTrusted===false`
//!     exactly like a page's synthetic click).
//!
//! **RED direction.** Remove the `navigator.userActivation` surface and `#load` reads
//! `t:undefined` and the `.isActive` read throws — the original hard wall. Remove the
//! `__dispatchEvent` activation bracket (or drop the `__actgesture` marker from mouse dispatch) and
//! `#during` reports `active:false` / `hasBeenActive:false` — the gesture grants nothing. Gate the
//! bracket on plain dispatch instead of the marker and `#synth` (a page-synthetic click) reports
//! `active:true` — activation leaks to untrusted events.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body>
<button id="btn">go</button>
<button id="syn">syn</button>
<div id="load">-</div><div id="during">-</div><div id="synth">-</div>
<script>
    // At load, before any gesture: the surface is present and reads a truthful "no activation yet".
    var ua = navigator.userActivation;
    document.getElementById('load').textContent =
        't:' + (typeof ua) + ' active:' + (ua && ua.isActive) + ' sticky:' + (ua && ua.hasBeenActive);

    // A real, host-dispatched click lands here and reads a LIVE transient activation.
    document.getElementById('btn').addEventListener('click', function () {
        document.getElementById('during').textContent =
            'active:' + navigator.userActivation.isActive + ' sticky:' + navigator.userActivation.hasBeenActive;
    });
    // A page-synthetic click must grant NOTHING.
    document.getElementById('syn').addEventListener('click', function () {
        document.getElementById('synth').textContent =
            'active:' + navigator.userActivation.isActive + ' sticky:' + navigator.userActivation.hasBeenActive;
    });
    document.getElementById('syn').click();   // untrusted — fires now, at load
</script></body></html>"#;

#[test]
fn user_activation_surface_tracks_real_gesture_state() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://activation.test/", &fonts, 800.0);

    let read = |page: &manuk_page::Page, sel: &str| {
        let root = page.dom().root();
        let n = manuk_css::query_selector_all(page.dom(), root, sel)[0];
        page.dom().text_content(n)
    };

    // (1) Present + honest at load + no crash. If the surface were absent, the `.isActive` read in
    // the load script would throw and `#load` would still read `-` (or `t:undefined`).
    let load = read(&page, "#load");
    assert_eq!(
        load, "t:object active:false sticky:false",
        "G_USER_ACTIVATION: navigator.userActivation must be a present object reading false/false at \
         load (no gesture yet), and reading `.isActive` must not throw.\n  got #load: {load}"
    );

    // (2) A page-synthetic `el.click()` (untrusted) grants NO activation — proves the gesture
    // discriminator is not fooled by page-driven dispatch.
    let synth = read(&page, "#synth");
    assert_eq!(
        synth, "active:false sticky:false",
        "G_USER_ACTIVATION: a page's own synthetic click must NOT grant user activation.\n  got #synth: {synth}"
    );

    // (3) A real, engine-dispatched click grants transient activation for the handler's duration and
    // latches the sticky bit.
    let root = page.dom().root();
    let btn = manuk_css::query_selector_all(page.dom(), root, "#btn")[0];
    page.dispatch_click(btn, &fonts, 800.0);
    let during = read(&page, "#during");
    assert_eq!(
        during, "active:true sticky:true",
        "G_USER_ACTIVATION: inside a real host-dispatched click handler, isActive must be true and \
         hasBeenActive must have latched true.\n  got #during: {during}"
    );

    // (4) Once the gesture is over, the transient bit clears but the sticky bit persists.
    page.eval_for_test(
        "document.getElementById('load').textContent = 'active:' + navigator.userActivation.isActive \
         + ' sticky:' + navigator.userActivation.hasBeenActive",
    );
    let after = read(&page, "#load");
    assert_eq!(
        after, "active:false sticky:true",
        "G_USER_ACTIVATION: after the gesture ends, isActive returns to false but hasBeenActive stays \
         true (sticky).\n  got: {after}"
    );
}
