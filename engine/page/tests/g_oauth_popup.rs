//! **G_OAUTH_POPUP — the popup login: `window.open` → provider → `postMessage` back to the opener.**
//!
//! Media step for logins, OAuth **O3**. Tick 226 gated the *redirect* flow, which is what most "Sign
//! in with…" buttons do. The other half of the modern web's logins never navigates the page at all:
//! **Google Identity Services, Stripe Checkout, Auth0's `loginWithPopup`, GitHub's OAuth popup** all
//! open a provider window, let the user authenticate there, and hand the result back over
//! `postMessage`. The opener never leaves the page, which is exactly why sites prefer it — no lost
//! form state, no full reload.
//!
//! **What has to compose, and why the failure is silent.** Four separate mechanisms, each of which
//! the engine has, none of which is proof the flow works:
//!
//!   1. `window.open` must return a **real window handle** with a distinct id — the value the opener
//!      keeps in order to talk to the popup at all.
//!   2. The popup must know its **`window.opener`**, or it has nothing to answer to.
//!   3. `postMessage` from the popup must be **routed to the opener** by the host, carrying the
//!      sender's origin.
//!   4. The opener's `message` handler must **run and mutate the DOM**, and the handler must be able
//!      to **reject a wrong origin** — the check every one of those SDKs performs, and the one that
//!      makes the difference between an auth callback and a cross-origin injection.
//!
//! If any of them is missing, nothing throws. The popup opens, the user authenticates, the popup
//! closes, and the opener sits on its spinner forever. That is the same shape as the redirect flow's
//! hung-callback bug, and it is why this is gated end to end rather than per-mechanism.
//!
//! **Two live `Page`s in one process**, routed exactly as `gui.rs` routes them — which is also the
//! point: the shell holds many pages at once, and this is the first gate to prove two of them can
//! talk.
//!
//! RED, run two ways: dropping the origin check makes the hostile message land (`signedin:attacker`);
//! not routing the popup's message at all leaves the opener on `waiting`.

use manuk_text::FontContext;

/// The opener: a page with a "Sign in" button that opens a popup and waits for a token to be
/// messaged back. The origin check is written the way every real SDK writes it.
const OPENER_HTML: &str = r##"<!doctype html>
<html><body>
  <div id="out">waiting</div>
  <script>
    var PROVIDER = 'https://idp.test';
    var popup = window.open('https://idp.test/authorize?client_id=demo', 'login', 'width=500');
    window.__popupHandle = popup;
    document.getElementById('out').textContent = popup ? 'opened' : 'nopopup';

    window.addEventListener('message', function (e) {
      var out = document.getElementById('out');
      // THE check. A handler without it accepts a token from any origin that can reach it.
      if (e.origin !== PROVIDER) {
        out.textContent = 'rejected:' + e.origin;
        return;
      }
      var d = e.data;
      if (d && d.type === 'oauth' && d.token) {
        out.textContent = 'signedin:' + d.user;
      }
    });
  </script>
</body></html>"##;

/// The popup, served by the provider origin: authenticate, then hand the result to the opener.
const POPUP_HTML: &str = r##"<!doctype html>
<html><body>
  <div id="state">authenticating</div>
  <script>
    var hasOpener = !!window.opener;
    document.getElementById('state').textContent = hasOpener ? 'hasopener' : 'noopener';
    if (hasOpener) {
      window.opener.postMessage({ type: 'oauth', token: 't0ken', user: 'ada' }, 'https://app.test');
    }
  </script>
</body></html>"##;

/// A page on a DIFFERENT origin that also messages the opener — the injection the origin check must
/// refuse. Served last so it cannot be confused with a race.
const ATTACKER_HTML: &str = r##"<!doctype html>
<html><body>
  <script>
    if (window.opener) {
      window.opener.postMessage({ type: 'oauth', token: 'stolen', user: 'attacker' }, '*');
    }
  </script>
</body></html>"##;

const OPENER_WIN: u64 = 1;
const POPUP_WIN: u64 = 2;
const ATTACKER_WIN: u64 = 3;

#[test]
fn a_popup_login_messages_its_token_back_to_the_opener() {
    let fonts = FontContext::new();

    // ── The opener. `set_identity` is what the shell does on load: tell the page which window it
    // is and who opened it. The opener has no opener of its own (0).
    let mut opener = manuk_page::Page::load_with_identity(
        OPENER_HTML,
        "https://app.test/",
        &fonts,
        800.0,
        OPENER_WIN,
        0,
    );

    let root = opener.dom().root();
    let out = manuk_css::query_selector_all(opener.dom(), root, "#out")[0];
    assert_eq!(
        opener.dom().text_content(out).trim(),
        "opened",
        "window.open must return a real window handle — the opener keeps that value to address the \
         popup, and a null handle ends the flow before it starts"
    );

    // ── The popup, on the provider's origin, told that the opener opened it.
    let mut popup = manuk_page::Page::load_with_identity(
        POPUP_HTML,
        "https://idp.test/authorize",
        &fonts,
        800.0,
        POPUP_WIN,
        OPENER_WIN,
    );

    let proot = popup.dom().root();
    let state = manuk_css::query_selector_all(popup.dom(), proot, "#state")[0];
    assert_eq!(
        popup.dom().text_content(state).trim(),
        "hasopener",
        "the popup must see window.opener — without it the provider page has nothing to answer to \
         and the opener waits forever"
    );

    // ── Route the popup's message, exactly as the shell's pump does: drain the sender, deliver to
    // the target window.
    let msgs = popup.take_messages();
    assert_eq!(
        msgs.len(),
        1,
        "the popup's postMessage must be queued for the host to route; nothing queued means the \
         token never leaves the provider page"
    );
    let (target, json, origin, source) = &msgs[0];
    assert_eq!(
        *target, OPENER_WIN,
        "the message must be addressed to the opener"
    );
    assert_eq!(
        *source, POPUP_WIN,
        "the opener must be able to tell who sent it"
    );
    assert!(
        json.contains("t0ken"),
        "the payload must survive routing intact — a structured-clone that drops fields hands the \
         opener a token-shaped object with no token in it\n  json: {json}"
    );
    assert_eq!(
        origin, "https://idp.test",
        "the sender's ORIGIN is what the opener's handler checks; a wrong or empty origin either \
         breaks every real SDK's guard or defeats it"
    );

    opener.deliver_message(json, origin, *source, &fonts, 800.0);
    let got = opener.dom().text_content(out);
    assert_eq!(
        got.trim(),
        "signedin:ada",
        "the opener's message handler must run and mutate the DOM\n  got: {got}\n\n  \
         'waiting' means the message never arrived — the popup opens, the user authenticates, the \
         popup closes, and the opener spins forever with nothing thrown anywhere."
    );

    // ── The injection. A third window on another origin sends the same shape; the handler's origin
    // check must refuse it, and the signed-in state must not change.
    let attacker = manuk_page::Page::load_with_identity(
        ATTACKER_HTML,
        "https://evil.test/",
        &fonts,
        800.0,
        ATTACKER_WIN,
        OPENER_WIN,
    );
    let bad = attacker.take_messages();
    assert_eq!(
        bad.len(),
        1,
        "the attacker page must genuinely have sent a message — otherwise the rejection below \
         proves nothing"
    );
    let (_t, bad_json, bad_origin, bad_src) = &bad[0];
    assert_eq!(
        bad_origin, "https://evil.test",
        "a message must carry its SENDER's origin, not its target's — the whole guard rests on this"
    );

    opener.deliver_message(bad_json, bad_origin, *bad_src, &fonts, 800.0);
    let after = opener.dom().text_content(out);
    assert_eq!(
        after.trim(),
        "rejected:https://evil.test",
        "the opener must be able to reject a wrong-origin message\n  got: {after}\n\n  \
         'signedin:attacker' means any page that can reach this window can hand it a token — the \
         exact injection every real SDK's origin check exists to stop."
    );
}
