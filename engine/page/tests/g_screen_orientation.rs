//! **G_SCREEN_ORIENTATION — `screen.orientation` is an `EventTarget`, and a two-field object literal
//! passes every detect that would have routed a page around it.**
//!
//! `ScreenOrientation` was `{ type: 'landscape-primary', angle: 0 }`. That object satisfies
//! `if (screen.orientation)`, `'orientation' in screen` and `typeof screen.orientation === 'object'`
//! — every guard anyone writes — and then `screen.orientation.addEventListener('change', …)`, which
//! is how a video player learns to go fullscreen and how a carousel learns to re-measure, was
//! `TypeError: … is not a function`.
//!
//! This is t772's half-installed `performance` one object over, and the rule it established is the
//! reason this gate exists: **absence routes a caller to the fallback it wrote and tested;
//! half-presence routes it into a wall it had no way to see.** A gate that only asked "is
//! `screen.orientation` there" would have been green throughout.
//!
//! ## The two claims that are not presence checks
//!
//! * **`type` must track the real viewport.** It was the constant `'landscape-primary'`, which is a
//!   *wrong answer of the right type* — present, correctly-shaped, and false on any portrait
//!   viewport. No feature detect can see that, so the assertion is made against a **second,
//!   independent evaluator**: `matchMedia('(orientation: portrait)')`, which is answered by the same
//!   Stylo `@media` machinery the cascade uses. One question, two instruments, and the gate fails if
//!   they ever disagree — the discipline `matchMedia` itself had to adopt when the prelude was
//!   carrying its own feature table.
//!
//! * **`lock()` must REJECT, not be absent.** Desktop Chrome rejects with `NotSupportedError`
//!   because a desktop window has no orientation to lock, so rejecting *matches the reference* and
//!   hands the caller the `.catch()` it already wrote. An absent `lock` throws a synchronous
//!   `TypeError` out of a call the author expected to be thenable, which is worse than the
//!   platform's own "no".
//!
//! The last claim covers the other half of the tick: `Screen`, `History`, `Location` and
//! `VisualViewport` sat in the `__inertNames` list — whose own comment forbids exactly this — so
//! `location instanceof Location` answered `false` about the only `Location` there is.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<div id="out">-</div>
<script>
  var R = [];
  function t(n, f) { try { R.push(n + ':' + f()); } catch (e) { R.push(n + ':THROW(' + e + ')'); } }

  // ── 1. THE FAMILY. `ScreenOrientation` is `EventTarget` + `lock`/`unlock`/`onchange`; the literal
  // carried two of the six members and passed every guard a page knows how to write.
  var FAM = ['addEventListener','removeEventListener','dispatchEvent','lock','unlock'];
  var o = screen.orientation;
  var missing = FAM.filter(function (n) { return typeof o[n] !== 'function'; });
  R.push('missing:' + (missing.length ? missing.join(',') : 'none'));
  R.push('onchangeSlot:' + ('onchange' in o));

  // ── 2. **THE UNGUARDED CALL THAT KILLED PAGES.** Written exactly as a bundle writes it — no
  // `typeof` around it, because the presence of `screen.orientation` IS the guard people use.
  t('listen', function () {
    var hits = 0;
    var h = function () { hits++; };
    screen.orientation.addEventListener('change', h);
    screen.orientation.onchange = h;
    // A real EventTarget, not a `return true` stub: dispatching must actually invoke both the
    // listener and the handler slot, which is what lets a host that CAN rotate drive this.
    screen.orientation.dispatchEvent({ type: 'change' });
    screen.orientation.removeEventListener('change', h);
    screen.orientation.dispatchEvent({ type: 'change' });
    return hits;   // 2 from the first dispatch (listener + onchange), 1 from the second (onchange)
  });

  // ── 3. ⚠ `type` TRACKS THE VIEWPORT, and it is checked against a DIFFERENT evaluator. This page is
  // loaded portrait on purpose; the old constant `'landscape-primary'` fails here while remaining
  // invisible to every presence check.
  t('type', function () { return screen.orientation.type; });
  t('agreesWithCSS', function () {
    var jsPortrait  = screen.orientation.type.indexOf('portrait') === 0;
    var cssPortrait = matchMedia('(orientation: portrait)').matches;
    return jsPortrait === cssPortrait;
  });
  // A window that is merely taller than it is wide has not been ROTATED, so the angle is 0 — the
  // value Chrome reports on a desktop, portrait window or not.
  t('angle', function () { return screen.orientation.angle; });

  // ── 4. `lock()` REJECTS with the reference's own error, rather than being absent. Async, so the
  // result lands in a second field the assertion reads separately.
  var lockOut = 'pending';
  try {
    var p = screen.orientation.lock('portrait');
    lockOut = (typeof p.then === 'function') ? 'thenable' : 'not-a-promise';
    p.then(function () { lockOut = 'RESOLVED'; },
           function (e) { lockOut = 'rejected/' + (e && e.name); });
  } catch (e) { lockOut = 'SYNC-THROW(' + e + ')'; }

  // ── 5. `screen`'s own CSSOM-View extensions. `screen.availLeft + x` is `NaN + x` when absent,
  // which places a popup nowhere.
  R.push('availLeft:' + (typeof screen.availLeft) + ',' + (typeof screen.availTop));

  // ── 6. THE INERT-STUB HALF. All four name an object this engine builds on every page, so an inert
  // constructor made `instanceof` answer `false` about the real thing — the defect t773 fixed for
  // `CSSStyleRule`, still sitting in the list that forbids it.
  // Reported as the LIST OF LIARS rather than a row of booleans: a bare `X instanceof Y` throws a
  // ReferenceError the moment `Y` is missing entirely, which would mask every name after it.
  t('instances', function () {
    var bad = [['Screen', screen], ['History', history], ['Location', location],
               ['VisualViewport', visualViewport], ['ScreenOrientation', screen.orientation]]
      .filter(function (p) {
        var C = globalThis[p[0]];
        return typeof C !== 'function' || !(p[1] instanceof C);
      }).map(function (p) { return p[0]; });
    return bad.length ? bad.join(',') : 'none';
  });

  // The lock promise settles on a microtask; report from one so the DOM carries the outcome.
  Promise.resolve().then(function () {
    R.push('lock:' + lockOut);
    document.getElementById('out').textContent = R.join('  ');
  });
</script></body></html>"##;

#[test]
fn screen_orientation_is_a_real_event_target() {
    let fonts = FontContext::new();
    // **A PORTRAIT VIEWPORT ON PURPOSE.** `type` was the constant `'landscape-primary'`, so a
    // landscape fixture would have agreed with the bug and this gate could not go red on the claim
    // that matters most. `Page::load` sets the width; the height comes from here.
    manuk_css::values::set_viewport(390.0, 844.0);
    let page = manuk_page::Page::load(HTML, "https://orientation.test/", &fonts, 390.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "missing:none",
            "`ScreenOrientation` is an `EventTarget` with `lock`/`unlock`. It shipped as a \
             `{ type, angle }` literal, which satisfies `if (screen.orientation)` — the guard every \
             page writes — and then throws on the call. Half-presence defeats feature detection in \
             a way absence never does (t772)",
        ),
        (
            "onchangeSlot:true",
            "the `onchange` handler slot is half the API's surface; assigning it is the older idiom \
             and is still what a lot of shipped code uses",
        ),
        (
            "listen:3",
            "⚠ THE UNGUARDED CALL. `addEventListener` + `onchange` must BOTH fire on dispatch (2), \
             and after `removeEventListener` only `onchange` remains (1). A `dispatchEvent` that \
             just returns `true` — the `navigator.connection` posture — would report 0 and leave a \
             future host with no way to drive a rotation it can observe",
        ),
        (
            "type:portrait-primary",
            "⚠ A WRONG ANSWER OF THE RIGHT TYPE. `type` was the constant `'landscape-primary'` on \
             every viewport including this portrait one, so a page laying out from it took the \
             landscape branch on a phone. It is now derived from the live viewport",
        ),
        (
            "agreesWithCSS:true",
            "…and the check is against a SECOND evaluator: `matchMedia('(orientation: portrait)')` \
             is answered by the same Stylo `@media` machinery the cascade uses. A page that branches \
             in JS and styles in CSS on the identical question must not get two answers",
        ),
        (
            "angle:0",
            "a window taller than it is wide has not been ROTATED — Chrome reports angle 0 for a \
             portrait desktop window, and fabricating 90 would be a second wrong-but-plausible value",
        ),
        (
            "lock:rejected/NotSupportedError",
            "⚠ `lock()` REJECTS rather than being absent, which is what desktop Chrome does: the \
             caller lands in the `.catch()` it already wrote. An absent `lock` throws a synchronous \
             `TypeError` out of a call the author expected to be thenable — strictly worse than the \
             platform's own no",
        ),
        (
            "availLeft:number,number",
            "`screen.availLeft`/`availTop` are the CSSOM-View extensions Chrome ships; a \
             popup-placement helper computes `screen.availLeft + x`, and `undefined + x` is `NaN`",
        ),
        (
            "instances:none",
            "⚠ `Screen`, `History`, `Location` and `VisualViewport` were INERT STUBS while the engine \
             builds all four on every page, so `location instanceof Location` answered `false` about \
             `window.location` itself. The inert doctrine is sound only for a name whose object we \
             never build — the same distinction t773 drew for `CSSStyleRule`",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_SCREEN_ORIENTATION: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
