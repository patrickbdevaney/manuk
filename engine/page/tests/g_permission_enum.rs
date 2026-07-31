//! **G_PERMISSION_ENUM — `permissions.query` must RESOLVE for every name Chrome knows, and REJECT
//! only for a name that is not in the enum at all.**
//!
//! `navigator.permissions.query({name})` returns a rejected promise for an unknown `PermissionName`.
//! That is correct and it is what Chrome does — but the reject path is a scarce resource, and
//! spending it on a name Chrome *does* know turns a routine capability probe into an **unhandled
//! promise rejection** inside a boot chain, which is where framework failures go to die.
//!
//! Seven names Chrome supports were missing from the table, so seven ordinary probes rejected here
//! and resolved there. One entry was worse than missing:
//!
//! * **`clipboard-read` answered `denied`** while `readText`/`read` genuinely pull the real OS
//!   clipboard through `__clipboardRead`, with no UI in front of them. A paste button that checks
//!   the permission before offering itself disabled itself against a clipboard that works — the
//!   *"a 'no' stub becomes a lie when the capability lands"* shape, and the one this gate exists to
//!   keep from recurring. (`clipboard-write` was already `granted`, for the same reason.)
//!
//! ## ⚠ The claim that is a CONTROL, not a fix
//!
//! This was found from `www.trivago.de` in the t777 sweep, which logs 26 unhandled rejections reading
//! *"'speaker' is not a valid enum value of type PermissionName"*. **`speaker` is NOT a valid name in
//! Chrome either** — it was dropped from the spec in favour of `speaker-selection`, so Chrome rejects
//! that call too, and "fixing" it would have been a divergence engineered to move a number. So the
//! gate asserts the *rejection* as hard as it asserts the resolutions: `speaker` and an invented name
//! must still reject, and they must reject with a `TypeError` rather than throwing synchronously.
//!
//! (trivago's blank render is therefore **not** this bug. It is load-budget starvation — 25.7s to
//! Chrome's 5.1s, budget exhausted five times. Recorded in the journal so the next tick aims at the
//! right organ.)

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<div id="out">-</div>
<script>
  var R = [];

  // Every name in Chrome's PermissionName enum that this engine claims. A rejection here is an
  // unhandled-rejection risk on any page that probes it, so the assertion is "resolves", not "is
  // granted" — the STATE is allowed to be denied, the SHAPE is not allowed to be a rejection.
  var KNOWN = ['notifications','push','geolocation','camera','microphone','midi','background-sync',
               'persistent-storage','accelerometer','gyroscope','magnetometer','ambient-light-sensor',
               'screen-wake-lock','payment-handler','idle-detection','local-fonts','window-management',
               'storage-access','clipboard-read','clipboard-write','display-capture','background-fetch',
               'periodic-background-sync','bluetooth','nfc','speaker-selection','top-level-storage-access'];

  // ⚠ THE CONTROL. Chrome rejects both of these, so we must too. `speaker` is the one the live site
  // actually asked for — it was dropped from the spec for `speaker-selection`.
  var UNKNOWN = ['speaker', 'not-a-real-permission'];

  var rejected = [], resolved = [], states = {};
  var pending = KNOWN.length + UNKNOWN.length;

  function done() {
    if (--pending > 0) { return; }
    R.push('knownRejected:' + (rejected.length ? rejected.join(',') : 'none'));
    R.push('unknownResolved:' + (resolved.length ? resolved.join(',') : 'none'));
    R.push('clipboardWrite:' + states['clipboard-write']);
    R.push('clipboardRead:' + states['clipboard-read']);
    // The permission must agree with the capability it describes: `granted` about a clipboard we
    // cannot write to would be the same lie pointing the other way.
    R.push('clipboardReal:' + (typeof navigator.clipboard === 'object' &&
                               typeof navigator.clipboard.writeText === 'function'));
    document.getElementById('out').textContent = R.join('  ');
  }

  // Written the way a bundle writes it — no `.catch` on the probe itself, because in a real browser
  // a known name does not reject. The harness's `.catch` here is what turns the failure into a
  // reported name instead of an unhandled rejection that this fixture could not see.
  KNOWN.forEach(function (n) {
    try {
      navigator.permissions.query({ name: n }).then(function (s) {
        states[n] = s && s.state; done();
      }, function () { rejected.push(n); done(); });
    } catch (e) { rejected.push(n + '(SYNC-THROW)'); done(); }
  });
  UNKNOWN.forEach(function (n) {
    try {
      navigator.permissions.query({ name: n }).then(function () { resolved.push(n); done(); },
                                                    function (e) {
        if (!(e instanceof TypeError)) { resolved.push(n + '(not-a-TypeError)'); }
        done();
      });
    } catch (e) { resolved.push(n + '(SYNC-THROW)'); done(); }
  });
</script></body></html>"##;

#[test]
fn permissions_query_resolves_every_name_chrome_knows() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://perm.test/", &fonts, 1200.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "knownRejected:none",
            "⚠ EVERY NAME CHROME KNOWS MUST RESOLVE. The reject path is reserved for a name that is \
             not in the enum at all; spending it on one Chrome supports turns a routine probe into \
             an UNHANDLED PROMISE REJECTION inside a boot chain. Seven names were missing \
             (display-capture, background-fetch, periodic-background-sync, bluetooth, nfc, \
             speaker-selection, top-level-storage-access)",
        ),
        (
            "unknownResolved:none",
            "⚠ THE CONTROL, AND IT IS AS LOAD-BEARING AS THE CLAIM ABOVE. `speaker` — the name the \
             live site asked for — is NOT valid in Chrome either (dropped for `speaker-selection`), \
             so it must still reject, with a TypeError, asynchronously. Accepting everything would \
             turn this gate green while diverging from the reference to move a number",
        ),
        (
            "clipboardWrite:granted",
            "already correct before this tick and asserted so it STAYS correct: \
             `navigator.clipboard.writeText` is genuinely implemented, and the table's own comment \
             calls this one out as 'genuinely done, with no user gate in front of it'. It is here \
             because `clipboard-read` sat two lines below it answering `denied` about the same \
             object — the pair is what makes the inconsistency visible",
        ),
        (
            "clipboardRead:granted",
            "⚠ `readText` pulls the real OS clipboard with no UI in front of it, so `denied` was a \
             'no' stub that became a lie when the capability landed. It is `granted` and NOT \
             `prompt` because the table's own documented rule — quoted at the call site rather than \
             quietly dropped — is that `prompt` promises a permission dialog nothing here can show",
        ),
        (
            "clipboardReal:true",
            "the permission must agree with the capability it describes — `granted` about a \
             clipboard we could not write to would be the same lie pointing the other way, and this \
             is the claim that stops the state above from being tuned rather than earned",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_PERMISSION_ENUM: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
