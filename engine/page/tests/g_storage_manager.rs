//! **G_STORAGE_MANAGER — `navigator.storage` reports truthful quota + durable persistence.**
//!
//! Offline-first apps and PWAs call `navigator.storage.estimate()` before caching to check they have
//! room, and `navigator.storage.persist()` to ask that their IndexedDB/Cache data not be evicted. Both
//! return Promises the app AWAITS in its boot path, so an absent `navigator.storage` is `undefined` and
//! `undefined.estimate()` throws out of startup.
//!
//! Unlike geolocation, this is a capability we HAVE — a real per-origin IndexedDB + Cache backend that
//! is durable on a single-user desktop — so the answers are truthful, not a denial. The gate asserts:
//!
//!   1. `navigator.storage` exists with `estimate`/`persist`/`persisted` callable; setup does not throw.
//!   2. `estimate()` resolves to `{quota, usage}` with a positive quota and `usage <= quota` — the
//!      headroom check apps make.
//!   3. `persist()` and `persisted()` resolve `true` — the backend is durable, not evicted.
//!
//! RED: removing the shim drops `defined` and `estimated` — `navigator.storage` is `undefined` and
//! `undefined.estimate` throws, the exact dead-startup failure.

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
      var st = navigator.storage;
      R.push('defined:' + (st && typeof st.estimate === 'function' &&
                           typeof st.persist === 'function' && typeof st.persisted === 'function'));

      Promise.all([st.estimate(), st.persist(), st.persisted()]).then(function (res) {
        var est = res[0];
        R.push('estimated:' + (est && typeof est.quota === 'number' && est.quota > 0 &&
                               typeof est.usage === 'number' && est.usage <= est.quota));
        R.push('persisted:' + (res[1] === true && res[2] === true));
      }, function (e) { R.push('estimated:REJECTED:' + e); });

      R.push('ready:true');
    } catch (e) {
      R.push('THREW:' + e);
    }
  </script>
</body></html>"##;

#[test]
fn storage_manager_reports_quota_and_persistence() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://sm.test/", &fonts, 800.0);
    let root = page.dom().root();

    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("defined:true", "`navigator.storage` must exist with estimate/persist/persisted callable — apps await it in boot, so its absence throws `undefined.estimate` out of startup"),
        ("estimated:true", "`estimate()` must resolve `{quota, usage}` with a positive quota and usage <= quota — the headroom check apps make before caching"),
        ("persisted:true", "`persist()`/`persisted()` must resolve true — the IndexedDB/Cache backend is durable on a single-user desktop, a true property, not a flattering guess"),
        ("ready:true", "the boot sequence must complete without throwing"),
    ] {
        assert!(
            got.contains(claim),
            "G_STORAGE_MANAGER: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
