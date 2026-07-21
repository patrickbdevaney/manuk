//! **G_POINTER_CAPTURE — `setPointerCapture`/`hasPointerCapture`/`releasePointerCapture` track state.**
//!
//! Custom sliders, drag-to-reorder lists, canvas drawing, image croppers, color pickers and resizable
//! panels call `el.setPointerCapture(e.pointerId)` in their `pointerdown` handler so a drag keeps
//! tracking after the pointer leaves the element. It is used UNGUARDED, so its absence throws
//! `setPointerCapture is not a function` mid-`pointerdown` and the whole drag dies on the first press.
//!
//! The host owns the live pointer pipeline, so this cannot yet re-route stray moves (the honest limit),
//! but it retains the captured pointer id per element so `hasPointerCapture` reflects the truth and
//! `got`/`lostpointercapture` fire — which is what stops the throw and lets the drag set up/tear down.
//! The gate drives that:
//!
//!   1. The three methods exist on the element and calling `setPointerCapture` does not throw.
//!   2. `hasPointerCapture(id)` is false before, true after `setPointerCapture(id)`, false again after
//!      `releasePointerCapture(id)`.
//!   3. `setPointerCapture` fires a `gotpointercapture` event (the drag's capture hook).
//!
//! RED: removing the shim drops `defined` and `tracks` — `setPointerCapture` is not a function and the
//! `pointerdown` call throws, the exact dead-drag failure.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html>
<html><body>
  <div id="slider">drag</div>
  <div id="out">-</div>
  <script>
    var R = {
      a: [],
      push: function (s) { this.a.push(s); var o = document.getElementById('out');
                           if (o) { o.textContent = this.a.join(' '); } }
    };
    try {
      var el = document.getElementById('slider');
      R.push('defined:' + (typeof el.setPointerCapture === 'function' &&
                           typeof el.releasePointerCapture === 'function' &&
                           typeof el.hasPointerCapture === 'function'));

      var gotFired = false;
      el.addEventListener('gotpointercapture', function () { gotFired = true; });

      var before = el.hasPointerCapture(1);
      el.setPointerCapture(1);
      var during = el.hasPointerCapture(1);
      el.releasePointerCapture(1);
      var after = el.hasPointerCapture(1);
      R.push('tracks:' + (before === false && during === true && after === false));
      R.push('gotevent:' + (gotFired === true));

      R.push('ready:true');
    } catch (e) {
      R.push('THREW:' + e);
    }
  </script>
</body></html>"##;

#[test]
fn pointer_capture_tracks_state_and_fires_event() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://pc.test/", &fonts, 800.0);
    let root = page.dom().root();

    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("defined:true", "`setPointerCapture`/`releasePointerCapture`/`hasPointerCapture` must exist — drag handlers call them unguarded in pointerdown, so absence throws `setPointerCapture is not a function`"),
        ("tracks:true", "`hasPointerCapture(id)` must be false before capture, true after `setPointerCapture(id)`, false after `releasePointerCapture(id)` — the state a drag reads back"),
        ("gotevent:true", "`setPointerCapture` must fire `gotpointercapture` — the capture hook drags wire"),
        ("ready:true", "the pointerdown-equivalent sequence must complete without throwing"),
    ] {
        assert!(
            got.contains(claim),
            "G_POINTER_CAPTURE: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
