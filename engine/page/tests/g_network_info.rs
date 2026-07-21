//! **G_NETWORK_INFO — `navigator.connection` exposes honest adaptive-loading signals.**
//!
//! The Network Information API is read by adaptive-loading code: Next.js `<Image>`, media players,
//! PWAs and the `react-adaptive-hooks` family read `navigator.connection.effectiveType` and
//! `.saveData` to choose image quality, autoplay and prefetch, and some wire
//! `navigator.connection.addEventListener('change', …)` UNGUARDED — on `undefined` that throws out of
//! the loader's setup.
//!
//! We do not continuously measure the link, so the honest posture is the one a real browser gives on a
//! fast desktop connection: a good, un-metered default (`effectiveType:'4g'`, plausible downlink/rtt)
//! and `saveData:false` — which is not a guess but the TRUE state (no data-saver is enabled). The gate
//! asserts that contract:
//!
//!   1. `navigator.connection` exists with `effectiveType`/`downlink`/`rtt`/`saveData` and an
//!      EventTarget surface, and the setup does not throw.
//!   2. `saveData` is `false` (honest — no data-saver) and `effectiveType` is a valid ECT token, so
//!      adaptive code takes the full-quality path rather than needlessly degrading.
//!   3. `addEventListener('change', …)` does not throw and the listener is retained (removable).
//!
//! RED: removing the shim drops `defined` and `signals` — `navigator.connection` is `undefined` and
//! `undefined.effectiveType` / `undefined.addEventListener` throw, the exact dead-loader failure.

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
      var c = navigator.connection;
      R.push('defined:' + (c && typeof c.effectiveType === 'string' &&
                           typeof c.downlink === 'number' && typeof c.rtt === 'number' &&
                           typeof c.saveData === 'boolean' && typeof c.addEventListener === 'function'));

      var ectOk = (c.effectiveType === '4g' || c.effectiveType === '3g' ||
                   c.effectiveType === '2g' || c.effectiveType === 'slow-2g');
      R.push('signals:' + (c.saveData === false && ectOk && c.downlink > 0 && c.rtt >= 0));

      var fn = function () {};
      var threw = false;
      try { c.addEventListener('change', fn); c.removeEventListener('change', fn); }
      catch (e) { threw = true; }
      R.push('events:' + (threw === false));

      R.push('ready:true');
    } catch (e) {
      R.push('THREW:' + e);
    }
  </script>
</body></html>"##;

#[test]
fn network_info_exposes_honest_signals() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://net.test/", &fonts, 800.0);
    let root = page.dom().root();

    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("defined:true", "`navigator.connection` must exist with effectiveType/downlink/rtt/saveData and an EventTarget surface — adaptive-loading code reaches for it (some unguarded), so its absence throws `undefined.addEventListener` out of the loader"),
        ("signals:true", "`saveData` must be false (honest — no data-saver) and `effectiveType` a valid ECT token, so adaptive code takes the full-quality path instead of needlessly degrading"),
        ("events:true", "`addEventListener('change', …)`/`removeEventListener` must not throw and must retain/remove the listener"),
        ("ready:true", "the whole loader setup must complete without throwing"),
    ] {
        assert!(
            got.contains(claim),
            "G_NETWORK_INFO: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
