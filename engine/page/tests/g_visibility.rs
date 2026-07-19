//! **G_VISIBILITY — the page can ask what state the browser is in, and gets answers that agree.**
//!
//! Two surfaces land together here because they fail the same way and are checked against each
//! other by the same code: **`document.visibilityState` / `document.hidden` /
//! `visibilitychange`**, and **`navigator.permissions.query()`**. Both were absent — grep found
//! zero occurrences of either — and both absences are *legible to the page*.
//!
//! ## Why `document.hidden` being `undefined` is worse than it looks
//!
//! The idiom on the real web is `if (document.hidden) return;` at the top of every animation loop,
//! poll, autoplay decision and analytics heartbeat. **`undefined` reads falsy**, so every one of
//! those guards failed *open*: a backgrounded tab kept animating, kept polling and kept decoding.
//! The API exists precisely to prevent that, and its absence broke it in the direction that costs
//! the user battery, silently, with nothing thrown to notice.
//!
//! ## Why `permissions.query` is a CONSISTENCY test and not a coverage test
//!
//! A bot detector does not ask `permissions.query({name:'notifications'})` to learn the answer. It
//! asks in order to **cross-check the answer against `Notification.permission`**, which it read a
//! line earlier. Headless Chrome historically said `'prompt'` to the first and `'denied'` to the
//! second, and that *internal contradiction* — not either value on its own — is the signal. A
//! browser is allowed to be unusual. It is not allowed to disagree with itself.
//!
//! So the assertion below is not "permissions.query returns something". It is **the two answers are
//! the same string**, which is a claim a stub cannot satisfy by accident.
//!
//! ## The RED probe (run, not imagined)
//!
//! * Deleting the `visibilityState`/`hidden` `defineProperty` block drops `vis:`, `hidden:`,
//!   `flip:`, `hidflip:` and `event:` — the state and the event both go.
//! * Making `__setVisibility` unconditional (dropping its same-value early return) flips
//!   `noreflip:` to false: a shell that republishes its state every frame would deliver a storm of
//!   change events that never changed anything.
//! * Hard-coding `'prompt'` for notifications — the exact headless-Chrome divergence — flips
//!   `agree:` to false while every other claim stays green. That claim is the one carrying the
//!   consistency property, and it is the one a plausible-looking stub fails.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<div id="ev">-</div>
<script>
  var R = [];
  var $ = function (i) { return document.getElementById(i); };
  try {
    // ── The state, at load. A document being rendered is by definition visible.
    R.push('vis:' + (document.visibilityState === 'visible'));
    R.push('hidden:' + (document.hidden === false));
    // The type matters as much as the value: `String(undefined)` is also a string, so a page
    // testing `typeof` is testing something a missing property cannot fake.
    R.push('type:' + (typeof document.visibilityState === 'string' &&
                      typeof document.hidden === 'boolean'));

    // ── The event. Recorded into a SEPARATE node, because it fires after this script returns —
    //    the host flips the state from outside, which is the only place the fact lives.
    var seen = [];
    document.addEventListener('visibilitychange', function () {
      seen.push(document.visibilityState);
      $('ev').textContent = 'event:' + (seen.length > 0) +
        ' hidflip:' + (document.hidden === true) +
        ' flip:' + (document.visibilityState === 'hidden') +
        ' count:' + seen.length;
    });

    // ── permissions.query. Everything below is a Promise, so the results land in the same node
    //    on a later microtask turn; the load drains them before the host reads it.
    var pend = [];
    var note = function (s) { R.push(s); $('out').textContent = R.join(' '); };

    R.push('permobj:' + (typeof navigator.permissions === 'object' &&
                         typeof navigator.permissions.query === 'function'));

    pend.push(navigator.permissions.query({ name: 'notifications' }).then(function (st) {
      note('status:' + (typeof st.state === 'string' && st.name === 'notifications'));
      // THE CONSISTENCY CLAIM. Not "what is the value" — "do our two answers agree".
      note('agree:' + (st.state === Notification.permission));
      note('denied:' + (st.state === 'denied'));
      // A real PermissionStatus is an EventTarget; consent code subscribes immediately.
      note('target:' + (typeof st.addEventListener === 'function'));
    }, function (e) { note('status:REJECTED-' + e); }));

    pend.push(navigator.permissions.query({ name: 'clipboard-write' }).then(function (st) {
      // We genuinely do this one with no user gate, so a blanket 'denied' would be its own lie.
      note('granted:' + (st.state === 'granted'));
    }, function (e) { note('granted:REJECTED-' + e); }));

    // An unrecognised name REJECTS with a TypeError — it does not throw synchronously, and it does
    // not resolve. A `query()` that can throw is visible to any caller that only wrote a `.catch`.
    var bogus;
    try {
      bogus = navigator.permissions.query({ name: 'not-a-real-permission' });
      R.push('async:' + (bogus && typeof bogus.then === 'function'));
    } catch (e) { R.push('async:THREW-' + e); }
    if (bogus && bogus.then) {
      pend.push(bogus.then(function () { note('unknown:RESOLVED'); },
                           function (e) { note('unknown:' + (e instanceof TypeError)); }));
    }

    $('out').textContent = R.join(' ');
  } catch (e) {
    $('out').textContent = 'THREW:' + e;
  }
</script>
</body></html>"##;

/// One test in the binary — a second `Page::load` stands up a second SpiderMonkey context in the
/// same process and does not survive teardown (see `g_mse`, `g_quirks_mode`, `g_globals`).
#[test]
fn the_page_can_read_its_visibility_and_its_permissions_and_they_agree() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://state.test/", &fonts, 800.0);

    let read = |page: &manuk_page::Page, sel: &str| {
        let root = page.dom().root();
        let n = manuk_css::query_selector_all(page.dom(), root, sel)[0];
        page.dom().text_content(n)
    };

    let got = read(&page, "#out");
    for (claim, why) in [
        ("vis:true", "document.visibilityState must be the string 'visible' for a document being rendered; it was undefined"),
        ("hidden:true", "document.hidden must be the BOOLEAN false — undefined is also falsy, which is exactly why this went unnoticed for so long"),
        ("type:true", "the types must be string and boolean; a page that feature-detects on typeof is testing something an absent property cannot fake"),
        ("permobj:true", "navigator.permissions.query must exist — calling it on undefined is a TypeError that takes the rest of the bundle with it"),
        ("async:true", "query() returns a Promise on EVERY path, including for a name we do not know; a synchronous throw is a divergence visible to any caller that only wrote .catch"),
        ("status:true", "the resolved PermissionStatus carries the name it was asked about and a string state"),
        ("agree:true", "THE CLAIM THIS GATE EXISTS FOR: permissions.query('notifications').state must equal Notification.permission. Headless Chrome said 'prompt' and 'denied' — that CONTRADICTION, not either value, is what a detector reads, and it is what a plausible stub gets wrong"),
        ("denied:true", "we do not deliver notifications, so 'denied' is the fact; 'prompt' would make the page put up a permission UI and wait for a decision nothing here can deliver"),
        ("granted:true", "clipboard-write genuinely works with no user gate — a blanket 'denied' would be the same lie in the other direction"),
        ("target:true", "PermissionStatus is an EventTarget and consent code subscribes to it immediately"),
        ("unknown:true", "an unrecognised permission name must REJECT with a TypeError, per spec — not resolve with a made-up state"),
    ] {
        assert!(
            got.contains(claim),
            "G_VISIBILITY: missing {claim:?} — {why}.\n  got: {got}"
        );
    }

    // ── The host flip. This is the half JS cannot do for itself: "this tab was backgrounded" is a
    //    fact about the shell's window, not about the document.
    page.set_visibility(true);
    let ev = read(&page, "#ev");
    for (claim, why) in [
        ("event:true", "visibilitychange must FIRE when the host backgrounds the tab — a state nobody is told about is a state no animation loop can pause on"),
        ("flip:true", "document.visibilityState must read 'hidden' inside the handler"),
        ("hidflip:true", "document.hidden must read true inside the handler — this is the guard every rAF loop actually tests"),
        ("count:1", "exactly one event for one transition"),
    ] {
        assert!(
            ev.contains(claim),
            "G_VISIBILITY: missing {claim:?} after Page::set_visibility(true) — {why}.\n  got: {ev}"
        );
    }

    // ── Idempotent by value. A shell that republishes its state on every frame must not deliver a
    //    storm of change events that never changed anything.
    page.set_visibility(true);
    let again = read(&page, "#ev");
    assert!(
        again.contains("count:1"),
        "G_VISIBILITY: setting the state we are ALREADY in fired a second visibilitychange — the \
         event asserts that it CHANGED. A shell republishing per frame would flood every listener \
         on the page.\n  got: {again}"
    );

    // ── ...and back. A one-way flip would satisfy every claim above and still be broken: the tab
    //    that comes back to the foreground is the one the user is actually looking at.
    page.set_visibility(false);
    let back = read(&page, "#ev");
    assert!(
        back.contains("count:2") && back.contains("flip:false") && back.contains("hidflip:false"),
        "G_VISIBILITY: raising the tab again must fire a SECOND visibilitychange and report \
         'visible'/false. A state that only ever goes one way leaves every paused animation loop \
         paused forever — the user brings the tab to the front and the page stays frozen.\n  \
         got: {back}"
    );
}
