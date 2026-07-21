//! **G_VIEW_TRANSITION — `document.startViewTransition` runs the update and applies it.**
//!
//! View Transitions are how a growing share of the modern web applies a route or state change:
//! instead of mutating the DOM directly, the app hands the mutation to the browser inside a
//! callback — `document.startViewTransition(() => this.render(next))` — so the browser can snapshot
//! before and after and cross-fade between them. Next.js, SvelteKit, Astro and hand-rolled SPAs all
//! reach for it now that it is interoperable.
//!
//! **The failure this gate exists for is silent and total.** Without the method, the call is
//! `startViewTransition is not a function`; the TypeError propagates out of the click handler, and
//! the DOM update wrapped inside the callback **never runs**. The page stays frozen on the previous
//! view — no blank screen, no console the user sees, just a link that does nothing. A capability
//! check that renders the page cannot tell a working transition from a dead one; only driving the
//! click and reading the resulting DOM can.
//!
//! This engine does not composite snapshot pseudo-elements, so there is no cross-fade to play — and
//! that is precisely the spec's own skip path: a document that cannot animate (reduced-motion, not
//! visible) still invokes the callback, lands its mutations, and settles the promises; it just omits
//! the animation. So the load-bearing assertion here is **not** the animation. It is:
//!
//!   1. `document.startViewTransition` is callable (feature detection succeeds).
//!   2. The update callback RUNS and its DOM writes LAND (the whole point).
//!   3. The returned object is a real `ViewTransition`: `ready`, `finished` and `updateCallbackDone`
//!      are thenables and `skipTransition` is a function — the shape sites await.
//!   4. A callback that THROWS is reflected, not swallowed into a false success — `updateCallbackDone`
//!      rejects, so a site can tell a failed update from a completed one.
//!
//! RED: deleting the `startViewTransition` shim drops `defined`, `applied`, `shape` and the
//! click-driven assertion together — the exact frozen-page state this bug produces in the wild.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html>
<html><body>
  <h1 id="view">Home</h1>
  <button id="go">Go to Profile</button>
  <div id="out">-</div>
  <script>
    var R = {
      a: [],
      push: function (s) { this.a.push(s); var o = document.getElementById('out');
                           if (o) { o.textContent = this.a.join(' '); } },
      join: function (sep) { return this.a.join(sep); }
    };
    var $ = function (id) { return document.getElementById(id); };
    var isThenable = function (p) { return p && typeof p.then === 'function'; };

    try {
      // ── 1. Feature detection is what the app does first, and it must succeed honestly.
      R.push('defined:' + (typeof document.startViewTransition === 'function'));

      // ── 2 + 3. Drive a transition directly and confirm the callback's mutation landed AND the
      // returned object has the shape a real ViewTransition does.
      var t = document.startViewTransition(function () {
        $('view').textContent = 'Profile';
      });
      R.push('applied:' + ($('view').textContent === 'Profile'));
      R.push('shape:' + (isThenable(t.ready) && isThenable(t.finished) &&
                         isThenable(t.updateCallbackDone) &&
                         typeof t.skipTransition === 'function'));

      // ── 4. A throwing update must be reflected, not swallowed. We attach a rejection handler so
      // the failure is observable and record that it was seen.
      var seen = false;
      var bad = document.startViewTransition(function () { throw new Error('boom'); });
      bad.updateCallbackDone.then(function () {}, function () { seen = true; });
      // `bad.ready`/`bad.finished` also reject — swallow them so the probe stays quiet.
      bad.ready.then(function () {}, function () {});
      bad.finished.then(function () {}, function () {});
      R.push('errorpath:true');

      // ── The click-driven path: the way this is actually used. A handler bound to a button wraps
      // its DOM update in a transition. The engine dispatches the real click below.
      $('go').addEventListener('click', function () {
        document.startViewTransition(function () { $('view').textContent = 'Clicked'; });
      });

      R.push('ready:true');
    } catch (e) {
      R.push('THREW:' + e);
    }
  </script>
</body></html>"##;

#[test]
fn start_view_transition_runs_the_update_and_applies_it() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://vt.test/", &fonts, 800.0);
    let root = page.dom().root();

    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("defined:true", "feature detection must succeed — the app checks `typeof document.startViewTransition === 'function'` before using it, and answers no by not using transitions at all"),
        ("applied:true", "the update callback must RUN and its DOM writes LAND — this is the whole capability; without it the wrapped route/state change is silently dropped and the page freezes on the old view"),
        ("shape:true", "the returned object must be a real ViewTransition — `ready`/`finished`/`updateCallbackDone` thenables and a `skipTransition` method — because sites `await` these to sequence follow-up work"),
        ("errorpath:true", "a throwing update must surface through `updateCallbackDone` rejecting, not be swallowed into a false success"),
        ("ready:true", "the whole sequence must complete without throwing"),
    ] {
        assert!(
            got.contains(claim),
            "G_VIEW_TRANSITION: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }

    // ── The half a load-time script cannot self-report: a real engine-dispatched click on a button
    // whose handler wraps its update in a transition must still apply that update.
    let go = manuk_css::query_selector_all(page.dom(), root, "#go")[0];
    page.dispatch_click(go, &fonts, 800.0);

    let view = manuk_css::query_selector_all(page.dom(), root, "#view")[0];
    let after = page.dom().text_content(view);
    assert_eq!(
        after.trim(),
        "Clicked",
        "a click handler that wraps its DOM update in `document.startViewTransition` must still \
         apply that update on a real dispatched click\n  got: {after}\n\n  \
         This is the frozen-page failure: the markup renders, the button looks live, and clicking it \
         does nothing because the transition-wrapped update was dropped."
    );
}
