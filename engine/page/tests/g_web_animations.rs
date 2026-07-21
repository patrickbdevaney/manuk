//! **G_WEB_ANIMATIONS — `element.animate` runs, resolves `finished`, and lands the end state.**
//!
//! The Web Animations API — `element.animate(keyframes, options)` — is the imperative animation
//! primitive the web uses constantly: fade/slide/scale on interaction, list reordering, toast in and
//! out, focus transitions. It is far more common than the declarative View Transitions API, and its
//! absence is the same silent-handler failure — `element.animate is not a function` throws out of a
//! click or mount callback and takes the whole interaction with it.
//!
//! This engine has no compositor timeline, so it cannot render the in-between frames, and it does not
//! pretend to. What it does honestly is FAST-FORWARD the animation to its end state: run the keyframes
//! to completion, apply the final frame's styles when the fill mode persists them (`forwards`/`both`),
//! and settle `finished`. So the gate asserts exactly those load-bearing behaviours, not a tween:
//!
//!   1. `element.animate` is callable and returns an `Animation` — `finished` is a thenable and
//!      `play`/`pause`/`cancel`/`finish` are methods (the surface libraries drive).
//!   2. `finished` RESOLVES — `await el.animate(...).finished` is the canonical "then do the next
//!      thing" pattern, and a promise that never settles hangs the whole sequence.
//!   3. A `fill: 'forwards'` animation LANDS ITS END STATE — the final keyframe's styles are applied
//!      and visible in the computed style, which is the outcome most imperative animations exist for.
//!   4. `getAnimations()` reports the running animation, so a library can find and cancel it.
//!   5. `cancel()` rejects `finished` with an AbortError — code that races animations relies on this.
//!
//! RED: removing the `animate` shim drops `defined`, `finishedresolved`, `endstate` and `tracked`
//! together — the exact dead-interaction state a missing WAAPI produces.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html>
<html><head><style>#box { opacity: 0.1; }</style></head><body>
  <div id="box">animate me</div>
  <div id="out">-</div>
  <script>
    var R = {
      a: [],
      push: function (s) { this.a.push(s); var o = document.getElementById('out');
                           if (o) { o.textContent = this.a.join(' '); } },
      join: function (sep) { return this.a.join(sep); }
    };
    var $ = function (id) { return document.getElementById(id); };

    try {
      var box = $('box');

      // ── 1. The call is real and returns an Animation with the surface libraries drive.
      var anim = box.animate(
        [{ opacity: '0.1' }, { opacity: '0.85' }],
        { duration: 300, fill: 'forwards' }
      );
      R.push('defined:' + (typeof box.animate === 'function' &&
                           anim && typeof anim.finished.then === 'function' &&
                           typeof anim.play === 'function' && typeof anim.cancel === 'function' &&
                           typeof anim.finish === 'function'));

      // ── 4. The animation is tracked while it exists.
      R.push('tracked:' + (box.getAnimations().length >= 1 && box.getAnimations()[0] === anim));

      // ── 2 + 3. `finished` resolves (microtask fast-forward), and the fill:forwards end state is
      // applied. Both are recorded from inside the resolution so we prove they actually settled.
      anim.finished.then(function () {
        R.push('finishedresolved:true');
        R.push('endstate:' + (getComputedStyle(box).opacity === '0.85'));
      }, function () {
        R.push('finishedresolved:REJECTED');
      });

      // ── 5. cancel() rejects finished with AbortError — animation-racing code relies on it.
      var box2 = document.createElement('div');
      document.body.appendChild(box2);
      var a2 = box2.animate([{ opacity: '0' }, { opacity: '1' }], 200);
      var cancelled = false;
      a2.finished.then(function () {}, function (e) { cancelled = (e && e.name === 'AbortError'); });
      a2.cancel();
      // record after a microtask so the rejection has settled
      Promise.resolve().then(function () { R.push('cancelrejected:' + cancelled); });

      R.push('ready:true');
    } catch (e) {
      R.push('THREW:' + e);
    }
  </script>
</body></html>"##;

#[test]
fn element_animate_runs_resolves_and_lands_the_end_state() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://waapi.test/", &fonts, 800.0);
    let root = page.dom().root();

    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("defined:true", "`element.animate` must be callable and return an Animation with `finished`/`play`/`cancel`/`finish` — this is the surface animation libraries and hand-rolled code drive, and its absence throws out of the interaction handler"),
        ("tracked:true", "`getAnimations()` must report the running animation so a library can find and cancel it"),
        ("finishedresolved:true", "`finished` must resolve — `await el.animate(...).finished` is the canonical sequencing pattern, and a promise that never settles hangs the whole chain"),
        ("endstate:true", "a `fill: 'forwards'` animation must land its final keyframe in the computed style — the end state is the outcome most imperative animations exist to produce"),
        ("cancelrejected:true", "`cancel()` must reject `finished` with an AbortError, which animation-racing code depends on to unwind"),
        ("ready:true", "the whole sequence must complete without throwing"),
    ] {
        assert!(
            got.contains(claim),
            "G_WEB_ANIMATIONS: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
