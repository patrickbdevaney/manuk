//! **G_GEOLOCATION — `navigator.geolocation` exists and fails honestly, not with a TypeError.**
//!
//! Weather sites, store locators, delivery/ride apps and "near me" search call
//! `navigator.geolocation.getCurrentPosition(success, error)` directly from a load or click handler.
//! In a real browser `navigator.geolocation` is ALWAYS present, so this code does NOT feature-detect
//! the object — which means its absence is not a silent no-op: `undefined.getCurrentPosition` throws a
//! TypeError that takes the rest of the handler (and often the page's boot) down with it.
//!
//! We have no location provider and we already model the geolocation PERMISSION as `'denied'`, so the
//! honest, self-consistent behaviour is to fail ASYNCHRONOUSLY with a `GeolocationPositionError` whose
//! `code` is `PERMISSION_DENIED` (1) — the same answer the permission layer gives, in the shape the
//! API promises. The site's error branch then runs and it shows its manual fallback. The gate asserts
//! exactly that contract:
//!
//!   1. `navigator.geolocation.getCurrentPosition` is callable and does NOT throw synchronously.
//!   2. The error callback fires (async) with `err.code === 1` (PERMISSION_DENIED), and the constants
//!      are readable off the error object — `err.code === err.PERMISSION_DENIED` is how real code branches.
//!   3. `getCurrentPosition` returns before the callback runs (the delivery is asynchronous), so a
//!      site relying on that ordering is not surprised.
//!   4. `watchPosition` returns a numeric id (for `clearWatch`) and also reports the denial.
//!   5. The result is consistent with `navigator.permissions.query({name:'geolocation'})` → 'denied'.
//!
//! RED: removing the geolocation shim drops `defined`, `denied` and `asyncorder` together — the call
//! throws `undefined.getCurrentPosition` synchronously, which is the exact dead-handler failure a
//! missing Geolocation API produces.

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
      var geo = navigator.geolocation;
      R.push('defined:' + (geo && typeof geo.getCurrentPosition === 'function' &&
                           typeof geo.watchPosition === 'function' &&
                           typeof geo.clearWatch === 'function'));

      // getCurrentPosition must not throw synchronously, and must deliver an async denial.
      var ranSyncAfter = false;
      geo.getCurrentPosition(
        function () { R.push('denied:UNEXPECTED_SUCCESS'); },
        function (err) {
          R.push('denied:' + (err && err.code === 1 && err.code === err.PERMISSION_DENIED &&
                              err.PERMISSION_DENIED === 1 && err.POSITION_UNAVAILABLE === 2 &&
                              err.TIMEOUT === 3));
          // The synchronous line after the call ran BEFORE this callback → async delivery.
          R.push('asyncorder:' + (ranSyncAfter === true));
        }
      );
      ranSyncAfter = true;

      // watchPosition returns an id and also denies.
      var wid = geo.watchPosition(function () {}, function () {});
      R.push('watchid:' + (typeof wid === 'number'));
      geo.clearWatch(wid);

      // Consistency with the permission layer.
      navigator.permissions.query({ name: 'geolocation' }).then(function (st) {
        R.push('permconsistent:' + (st && st.state === 'denied'));
      }, function () { R.push('permconsistent:REJECTED'); });

      R.push('ready:true');
    } catch (e) {
      R.push('THREW:' + e);
    }
  </script>
</body></html>"##;

#[test]
fn geolocation_exists_and_denies_honestly() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://geo.test/", &fonts, 800.0);
    let root = page.dom().root();

    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("defined:true", "`navigator.geolocation` must be present with getCurrentPosition/watchPosition/clearWatch — real sites do not guard the object, so its absence throws `undefined.getCurrentPosition` out of the handler"),
        ("denied:true", "the error callback must fire with a GeolocationPositionError whose code is PERMISSION_DENIED (1), matching the permission layer, with the interface constants readable off it"),
        ("asyncorder:true", "delivery must be ASYNCHRONOUS — the line after getCurrentPosition() runs before the callback, and code that relies on that ordering must not break"),
        ("watchid:true", "watchPosition must return a numeric id so `clearWatch(id)` and the store-the-watch-id pattern work"),
        ("permconsistent:true", "the denial must be consistent with navigator.permissions.query({name:'geolocation'}) → 'denied' — the browser must not contradict itself"),
        ("ready:true", "the whole sequence must complete without throwing"),
    ] {
        assert!(
            got.contains(claim),
            "G_GEOLOCATION: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
