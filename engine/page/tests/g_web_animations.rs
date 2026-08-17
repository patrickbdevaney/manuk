//! **G_WEB_ANIMATIONS — `element.animate` runs, resolves `finished`, and lands the end state.**
//!
//! The Web Animations API — `element.animate(keyframes, options)` — is the imperative animation
//! primitive the web uses constantly: fade/slide/scale on interaction, list reordering, toast in and
//! out, focus transitions. It is far more common than the declarative View Transitions API, and its
//! absence is the same silent-handler failure — `element.animate is not a function` throws out of a
//! click or mount callback and takes the whole interaction with it.
//!
//! This engine has no compositor timeline, so it does not render in-between frames as time passes,
//! and it does not pretend to. But **"cannot animate over time" is not the same as "cannot say what
//! value an animation HAS"**, and conflating the two cost this gate its most load-bearing claim for a
//! long while (t1301). An unplayed animation still has a well-defined value at any `currentTime`, and
//! that value is what the web's own animation tests — and any code that pauses and scrubs — ask for.
//! So the gate asserts the fast-forward end state AND the sampled value:
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
//!   6. A PAUSED animation seeked with `currentTime` reports the value AT THAT TIME — not the end
//!      state. This is how WPT's entire interpolation harness samples (see `docs/wiki/css-cascade.md`).
//!   7. A pair that cannot interpolate is DISCRETE and flips at progress 0.5.
//!   8. The effect's `easing` is APPLIED to the progress before the value is taken.
//!
//! RED: removing the `animate` shim drops `defined`, `finishedresolved`, `endstate` and `tracked`
//! together — the exact dead-interaction state a missing WAAPI produces. Three more, each measured:
//! dropping the `paused` guard in `_settle` → `midsample:[1]` (the queued fast-forward stamps over
//! the sample); moving the discrete flip off 0.5 → `discretelow:monospace`; ignoring `easing` →
//! `steps:[0.5]`.
//!
//! ⚠ **Cases 6 and 8 bracket their numeric readings (`midsample:[0.5]`, not `midsample:0.5`) and the
//! reason is a defect this gate shipped for one iteration:** the assertions are `contains` checks, and
//! a bare `steps:0` is a PREFIX of the wrong answer `steps:0.5`, so the easing mutation stayed GREEN.
//! Case 6 likewise reads its value after a microtask, because read inline it is correct even while the
//! fast-forward is still armed. **A green that cannot go red measured nothing — including this one.**

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

      // ── 6. **A PAUSED, SEEKED ANIMATION REPORTS THE VALUE AT THAT TIME (t1301).** This is the
      //       shape WPT's whole interpolation harness uses — `pause()` then write `currentTime` —
      //       and it is the one no fast-forward can produce, because the answer it wants is
      //       explicitly NOT the end state.
      var mid = document.createElement('div');
      document.body.appendChild(mid);
      var am = mid.animate([{ opacity: '0' }, { opacity: '1' }],
                           { duration: 100000, fill: 'forwards', easing: 'linear' });
      am.pause();
      am.currentTime = 50000;
      // ⚠ Recorded AFTER a microtask, on purpose. `animate()` queues its fast-forward on the
      // microtask queue, so a value read synchronously here is correct even when the fast-forward
      // is still armed — the first version of this case read it inline and stayed GREEN against a
      // mutation that removed the guard. WPT's harness measures in a separate later phase for its
      // own reasons, which is exactly the window where an unguarded settle overwrites the sample.
      Promise.resolve().then(function () {
          R.push('midsample:[' + getComputedStyle(mid).opacity + ']');
      });

      // ── 7. **A NON-INTERPOLABLE PAIR IS DISCRETE and flips at progress 0.5**, holding `from`
      //       below it. Getting this wrong is invisible: interpolating nonsense would still answer
      //       with a plausible string, and half of every interpolation file's assertions expect
      //       exactly the `from` value.
      var disc = document.createElement('div');
      document.body.appendChild(disc);
      var ad = disc.animate([{ fontFamily: 'serif' }, { fontFamily: 'monospace' }],
                            { duration: 100000, fill: 'forwards', easing: 'linear' });
      ad.pause();
      ad.currentTime = 30000;
      R.push('discretelow:' + getComputedStyle(disc).fontFamily);
      ad.currentTime = 70000;
      R.push('discretehigh:' + getComputedStyle(disc).fontFamily);

      // ── 8. The easing is APPLIED, not ignored — `steps(1, end)` holds the start value for the
      //       whole active interval, which is how the harness asks for progress 0.
      var st = document.createElement('div');
      document.body.appendChild(st);
      var as_ = st.animate([{ opacity: '0' }, { opacity: '1' }],
                           { duration: 100000, fill: 'forwards', easing: 'steps(1, end)' });
      as_.pause();
      as_.currentTime = 50000;
      R.push('steps:[' + getComputedStyle(st).opacity + ']');

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
        ("midsample:[0.5]", "a PAUSED animation seeked to its midpoint must report the MIDPOINT value, not the end state — `pause()` then writing `currentTime` is how WPT's entire interpolation harness samples, and a fast-forward answers `1` here forever"),
        ("discretelow:serif", "a pair that cannot interpolate is DISCRETE and holds the FROM value below progress 0.5 — interpolating it instead would still produce a plausible string, which is why this is asserted rather than eyeballed"),
        ("discretehigh:monospace", "…and takes the TO value at and above 0.5, which is the flip WPT's `expectFlip` encodes"),
        ("steps:[0]", "the effect's easing must be APPLIED to the progress — `steps(1, end)` holds the start value across the active interval, and ignoring easing silently turns every sampled test into a linear one"),
        ("ready:true", "the whole sequence must complete without throwing"),
    ] {
        assert!(
            got.contains(claim),
            "G_WEB_ANIMATIONS: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
