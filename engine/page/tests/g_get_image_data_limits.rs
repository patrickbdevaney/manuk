//! **G_GET_IMAGE_DATA_LIMITS — the Bar 0 that asked for 85 GB, and the error surface around it.**
//!
//! ## ⚠⚠⚠ THE CRASH
//!
//! ```text
//!   html/canvas/element/pixel-manipulation/2d.imageData.get.large.crash.html
//!       CRASH (killed by a signal — Bar 0)
//! ```
//!
//! `ctx.getImageData(10, 0xffffffff, 2147483647, 10)`. The shim did `w = Math.max(1, w|0)` and
//! handed `2147483647 × 10 × 4` bytes to a `vec![0u8; …]` — **85 GB, and the process died.** A crash
//! loses every tab, so Bar 0 outranks every visual cluster. It was reachable from any page for as
//! long as this API has existed, and was invisible only because `html/canvas` was outside the
//! primary metric until t1388 opened the aperture.
//!
//! ## THE ERROR SURFACE, ALL OF IT CHROME-MEASURED (`--headless=new`)
//!
//! ```text
//!   getImageData(10, 0xffffffff, 2147483647, 10)   TypeError    outside the 'long' value range
//!   getImageData(0, 0, 1e10, 1)                    TypeError
//!   getImageData(NaN, 0, 10, 10)                   TypeError
//!   getImageData(Infinity, 0, 10, 10)              TypeError
//!   getImageData(0, 0, 2147483648, 1)              TypeError    one past int32
//!   getImageData(0, 0, 2147483647, 10)             RangeError   Out of memory at ImageData creation
//!   getImageData(0, 0, 32768, 32768)               RangeError   4.295e9 bytes
//!   getImageData(0, 0, 23000, 23000)               OK           2.116e9 bytes  ← the boundary
//!   getImageData(0, 0, 0, 10)                      DOMException "The source width is 0"
//!   getImageData(0, 0, -5, 10)                     OK  w=5 h=10  the rect NORMALISES
//!   getImageData(5, 5, -5, -5)                     OK  w=5 h=5
//!   getImageData(0, 0, 10, 10)                     OK  w=10 h=10
//! ```
//!
//! **The ceiling is `w × h × 4 ≤ 2³¹−1` bytes**, placed by the pair 23000² (allowed) and 32768²
//! (refused) rather than guessed.
//!
//! ⭐⭐ **THE NEGATIVE ROW IS THE ONE THAT STOPS THIS BEING A CLAMP.** `Math.max(1, …)` turned `-5`
//! into `1`; the spec NORMALISES the rectangle, so `-5` is a five-pixel-wide read starting five
//! pixels to the LEFT. **A clamp silently returns the wrong pixels where a normalisation returns the
//! right ones** — and a guard added only to stop the crash would very naturally have been a clamp.
//!
//! ⭐⭐ **THREE DIFFERENT FAILURE KINDS IN ONE FUNCTION**, and a feature-detecting library branches
//! on all three: `TypeError` for an argument outside `long`, `RangeError` for an allocation that
//! cannot be made, `DOMException`/`IndexSizeError` for a zero extent. Collapsing them into one
//! "invalid argument" throw passes any test that only checks *that* it throws.
//!
//! ⚠ `0xffffffff` is the row that shows why `v|0` is the wrong conversion: it WRAPS to `-1`, which
//! is a legal `long` and would have been silently accepted as a negative origin. WebIDL
//! `[EnforceRange]` THROWS instead, and Chrome's message says so.
//!
//! ## ⚠ WHERE THE GUARD LIVES — TWICE, ON PURPOSE
//!
//! The shim applies `[EnforceRange]` and the ceiling; `cv_get_image_data` applies the ceiling AGAIN
//! and answers `null`. A host function that will allocate `w × h × 4` bytes on request must not
//! depend on its only current caller staying its only caller — **the refusal belongs at the
//! allocation, not merely upstream of it.**
//!
//! ⚠ This gate lives in `engine/page/tests/`, which the wall does not run (surface audit #78). Its
//! wall-independent guard is the WPT row `html/canvas` — which became part of the primary metric at
//! t1388, one tick before this fix, and which reports `HANG/CRASH` as Bar 0 in its own summary line.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body style="margin:0">
  <div id="out">-</div>
  <canvas id="c" width="100" height="50"></canvas>
  <script>
    var R = [];
    var ctx = document.getElementById('c').getContext('2d');
    function t(label, f) {
      try {
        var r = f();
        R.push(label + '=OK:' + (r && r.width !== undefined ? (r.width + 'x' + r.height) : 'v'));
      } catch (e) {
        R.push(label + '=' + (e && e.constructor ? e.constructor.name : 'throw'));
      }
    }
    globalThis.__report = function () {
      t('crash',    function () { return ctx.getImageData(10, 0xffffffff, 2147483647, 10); });
      t('huge',     function () { return ctx.getImageData(0, 0, 2147483647, 10); });
      t('big',      function () { return ctx.getImageData(0, 0, 32768, 32768); });
      t('nan',      function () { return ctx.getImageData(NaN, 0, 10, 10); });
      t('inf',      function () { return ctx.getImageData(Infinity, 0, 10, 10); });
      t('past',     function () { return ctx.getImageData(0, 0, 2147483648, 1); });
      t('zerow',    function () { return ctx.getImageData(0, 0, 0, 10); });
      t('zeroh',    function () { return ctx.getImageData(0, 0, 10, 0); });
      t('negw',     function () { return ctx.getImageData(0, 0, -5, 10); });
      t('negboth',  function () { return ctx.getImageData(5, 5, -5, -5); });
      t('ok',       function () { return ctx.getImageData(0, 0, 10, 10); });
      t('cid_huge', function () { return ctx.createImageData(2147483647, 10); });
      t('cid_zero', function () { return ctx.createImageData(0, 10); });
      t('cid_ok',   function () { return ctx.createImageData(4, 4); });
      document.getElementById('out').textContent = R.join(' ');
    };
  </script></body></html>"#;

#[test]
fn get_image_data_refuses_what_it_cannot_allocate_and_says_which_kind_of_wrong() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://canvas.test/", &fonts, 800.0);
    page.eval_for_test("globalThis.__report && __report()");

    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    // ── VACUITY. The report must have run at all, and the HAPPY path must work — a canvas that
    //    throws for everything satisfies every refusal row below.
    assert!(
        got.contains("ok=OK:10x10"),
        "VACUOUS: the ordinary `getImageData(0, 0, 10, 10)` did not return a 10x10 ImageData, so \
         the refusals below prove nothing. Got: {got}"
    );
    assert!(
        got.contains("cid_ok=OK:4x4"),
        "VACUOUS: `createImageData(4, 4)` did not work either. Got: {got}"
    );

    // (claim, what the row decides)
    let rows: &[(&str, &str)] = &[
        ("crash=TypeError", "THE BAR 0 — 85 GB and a killed process. `0xffffffff` is outside `long`, so WebIDL's [EnforceRange] throws; `v|0` would WRAP it to -1 and silently accept a negative origin"),
        ("huge=RangeError", "an in-range `long` whose ALLOCATION cannot be made is a RangeError, not a TypeError — a different kind of wrong, and a library branches on it"),
        ("big=RangeError", "32768x32768 = 4.295e9 bytes, just past the ceiling that 23000x23000 (2.116e9) is inside"),
        ("nan=TypeError", "non-finite is out of range too"),
        ("inf=TypeError", "…and so is Infinity"),
        ("past=TypeError", "2147483648 is ONE past int32, which is where the boundary actually is"),
        ("zerow=DOMException", "a zero extent is an IndexSizeError — the THIRD failure kind in one function"),
        ("zeroh=DOMException", "…on either axis"),
        ("negw=OK:5x10", "NORMALISE, DO NOT CLAMP — `-5` is a five-pixel read starting five pixels LEFT. A clamp returns the wrong pixels where this returns the right ones"),
        ("negboth=OK:5x5", "…on both axes at once, which is what says it is a rectangle rule and not two independent clamps"),
        ("cid_huge=RangeError", "`createImageData` allocates the same buffer and must refuse the same sizes"),
        ("cid_zero=DOMException", "…and reject a zero extent the same way"),
    ];
    for (claim, why) in rows {
        assert!(
            got.contains(claim),
            "G_GET_IMAGE_DATA_LIMITS: expected {claim:?} in the report.\n  {why}\n  got: {got}"
        );
    }
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// N1  restore `w = Math.max(1, w|0); h = Math.max(1, h|0);` (the pre-tick shim)
//       -> the process is KILLED. The gate does not report a failure, it disappears — which is what
//          a Bar 0 looks like from the outside and why the WPT runner counts HANG/CRASH separately
//          from FAIL.
// N2  keep the ceiling but clamp negatives with `Math.max(1, …)` instead of normalising
//       -> `negw` and `negboth` fail, at 1x10 and 1x1. Every refusal row stays green: a guard that
//          only stops the crash passes eleven of thirteen rows.
// N3  throw a single `TypeError` for every invalid argument
//       -> the two `DOMException` rows and the three `RangeError` rows fail while the TypeError rows
//          pass, which is what says the KIND of wrong is a separate claim from the fact of it.
