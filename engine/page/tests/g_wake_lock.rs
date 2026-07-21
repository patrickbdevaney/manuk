//! **G_WAKE_LOCK — `navigator.wakeLock.request('screen')` grants a real, releasable sentinel.**
//!
//! Video players (YouTube et al.), presentation/slides apps, recipe and reading UIs, kiosks and
//! dashboards `await navigator.wakeLock.request('screen')` to keep the display awake, and hold the
//! returned sentinel. The request is awaited in the play/present handler, so an absent
//! `navigator.wakeLock` is `undefined` and `undefined.request(...)` throws out of that handler.
//!
//! The display's sleep behaviour is host/OS-owned, so — like `mediaSession` — the honest posture is to
//! GRANT and retain a real sentinel (a working handle the player can `release()`, and a seam a future
//! host binding can enforce) while stating the limit that the OS sleep timer is not yet driven. The
//! gate asserts:
//!
//!   1. `navigator.wakeLock.request` is callable and does not throw synchronously.
//!   2. It resolves to a `WakeLockSentinel` with `type === 'screen'`, `released === false`, and a
//!      `release()` method.
//!   3. `release()` resolves and flips `released` to true and fires the `release` event — the player's
//!      cleanup path.
//!
//! RED: removing the shim drops `defined` and `granted` — `navigator.wakeLock` is `undefined` and
//! `undefined.request` throws, the exact dead-player-handler failure.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html>
<html><body>
  <div id="out">-</div>
  <script>
    var R = {
      a: [],
      push: function (s) { this.a.push(s); var o = document.getElementById('out');
                           if (o) { o.textContent = this.a.join(' '); } }
    };
    try {
      R.push('defined:' + (navigator.wakeLock && typeof navigator.wakeLock.request === 'function'));

      var p = navigator.wakeLock.request('screen');
      R.push('promise:' + (p && typeof p.then === 'function'));

      p.then(function (sentinel) {
        var okShape = (sentinel && sentinel.type === 'screen' && sentinel.released === false &&
                       typeof sentinel.release === 'function');
        var fired = false;
        sentinel.addEventListener('release', function () { fired = true; });
        sentinel.release().then(function () {
          R.push('granted:' + (okShape === true));
          R.push('released:' + (sentinel.released === true && fired === true));
        });
      }, function () { R.push('granted:REJECTED'); });

      R.push('ready:true');
    } catch (e) {
      R.push('THREW:' + e);
    }
  </script>
</body></html>"##;

#[test]
fn wake_lock_grants_and_releases_a_sentinel() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://wl.test/", &fonts, 800.0);
    let root = page.dom().root();

    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("defined:true", "`navigator.wakeLock.request` must exist and be callable — players await it in the play handler, so its absence throws `undefined.request` out of that handler"),
        ("granted:true", "`request('screen')` must resolve a WakeLockSentinel with type 'screen', released false, and a release() method — the handle the player holds"),
        ("released:true", "`release()` must resolve, flip `released` to true and fire the `release` event — the player's cleanup path"),
        ("ready:true", "the play handler must complete without throwing"),
    ] {
        assert!(
            got.contains(claim),
            "G_WAKE_LOCK: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
