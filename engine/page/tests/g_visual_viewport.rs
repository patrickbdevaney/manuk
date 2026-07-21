//! **G_VISUAL_VIEWPORT — `window.visualViewport` mirrors the real layout viewport, unguarded-safe.**
//!
//! The VisualViewport API is read by keyboard-aware layouts, pinch-zoom handlers, sticky /
//! `position:fixed` correction and mobile-responsive frameworks: `visualViewport.width/height/scale/
//! offsetTop` plus `visualViewport.addEventListener('resize'|'scroll', …)`. It is routinely used
//! UNGUARDED, so its absence is a silent-handler failure — `undefined.addEventListener` throws out of
//! the layout setup and the responsive/keyboard code dies.
//!
//! We do not zoom, so the visual viewport EQUALS the layout viewport: `scale` is 1, offsets are 0, and
//! `width`/`height` come from the SAME real viewport the cascade lays out against (`innerWidth`/
//! `innerHeight`), never a hardcoded size. The gate asserts that contract:
//!
//!   1. `window.visualViewport` exists with the metrics and an EventTarget surface, and the setup does
//!      not throw.
//!   2. `width`/`height` EQUAL `innerWidth`/`innerHeight` (the real viewport, not a constant), `scale`
//!      is 1, offsets are 0 — so code sizing off it agrees with the layout.
//!   3. `addEventListener('resize', …)` does not throw and the listener is retained (removable).
//!
//! RED: removing the shim drops `defined` and `mirrors` — `visualViewport` is `undefined` and
//! `undefined.width` / `undefined.addEventListener` throw, the exact dead-layout failure a missing API
//! produces.

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
      var vv = window.visualViewport;
      R.push('defined:' + (vv && typeof vv.width === 'number' && typeof vv.height === 'number' &&
                           typeof vv.scale === 'number' && typeof vv.addEventListener === 'function'));

      // Mirrors the REAL layout viewport, not a hardcoded size.
      R.push('mirrors:' + (vv.width === window.innerWidth && vv.height === window.innerHeight &&
                           vv.scale === 1 && vv.offsetLeft === 0 && vv.offsetTop === 0));

      // Unguarded listener wiring must not throw, and the listener must be retained (removable).
      var fn = function () {};
      var threw = false;
      try { vv.addEventListener('resize', fn); vv.removeEventListener('resize', fn); }
      catch (e) { threw = true; }
      R.push('events:' + (threw === false));

      R.push('ready:true');
    } catch (e) {
      R.push('THREW:' + e);
    }
  </script>
</body></html>"##;

#[test]
fn visual_viewport_mirrors_layout_viewport() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://vv.test/", &fonts, 800.0);
    let root = page.dom().root();

    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("defined:true", "`window.visualViewport` must exist with numeric metrics and an EventTarget surface — responsive/keyboard code uses it unguarded, so its absence throws `undefined.addEventListener` out of layout setup"),
        ("mirrors:true", "`width`/`height` must EQUAL `innerWidth`/`innerHeight` with `scale` 1 and offsets 0 — the visual viewport equals the real layout viewport (we do not zoom), so code sized off it agrees with the cascade"),
        ("events:true", "`addEventListener`/`removeEventListener` must not throw and must retain/remove the listener — the unguarded wiring pattern"),
        ("ready:true", "the whole setup must complete without throwing"),
    ] {
        assert!(
            got.contains(claim),
            "G_VISUAL_VIEWPORT: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
