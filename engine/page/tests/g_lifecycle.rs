//! **G_LIFECYCLE — the document lifecycle, the clock, and the loop that must not die.**
//!
//! Every assertion in this file was found by wiring up **upstream Web Platform Tests** (tick 43), and
//! every one of them is a defect that the 265-site Chromium differential crawl had been unable to see
//! for forty ticks — because none of them *move a box*. They break behaviour, silently.
//!
//! What WPT found, in one run:
//!
//!   1. **`window.parent` was undefined.** At the top level the spec says `window.parent === window`;
//!      that self-reference is how a page knows it is the top. The universal idiom
//!      `while (w != w.parent) w = w.parent;` terminates *because* the top is its own parent — with
//!      `parent` undefined it walks straight off the end and throws. **This alone failed 100% of WPT**,
//!      and it presented as "our JS engine cannot run testharness.js".
//!
//!   2. **`DOMContentLoaded` and `load` were NEVER DISPATCHED.** Not once, anywhere in the engine.
//!      A site whose init lives in `window.addEventListener('load', …)` simply never initialised.
//!      jQuery survived it by checking `document.readyState` — which is exactly why it went unnoticed:
//!      **it worked often enough to look fine.**
//!
//!   3. **`setTimeout` threw its delay away.** Every timer was a FIFO push, so `setTimeout(f, 10000)`
//!      ran *before* a `setTimeout(g, 0)` queued after it. That mis-orders every debounce, throttle
//!      and retry-backoff on the web — and it never errors, it just happens in the wrong order.
//!
//!   4. **A throwing task killed the whole event loop.** One bad callback and every task queued after
//!      it never ran. The spec says: report the exception and keep going.
//!
//!   5. **`insertAdjacentText` was missing** while its two siblings shipped. *Nobody feature-detects
//!      the third member of a family when the first two are present.*
//!
//! The gate exists so that none of these can silently come back — and because a conformance suite that
//! lives outside the wall is an instrument, not a guarantee.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body><div id="out">-</div>
  <script>
    var R = [];
    var seen = [];

    // ── (1) The browsing-context tree. The self-reference is a TERMINATION CONDITION.
    R.push('parentSelf:' + (window.parent === window));
    R.push('topSelf:' + (window.top === window));
    R.push('framesSelf:' + (window.frames === window));
    R.push('openerNull:' + (window.opener === null));   // null, NOT undefined
    // The idiom that 100% of WPT dies on if `parent` is undefined:
    var w = self, hops = 0;
    while (w != w.parent && hops < 5) { w = w.parent; hops++; }
    R.push('walkTerminates:' + (hops === 0));

    // ── (2) The document lifecycle. At INLINE-SCRIPT time we are still parsing.
    R.push('readyLoading:' + (document.readyState === 'loading'));
    window.addEventListener('load', function () { seen.push('load'); });
    document.addEventListener('DOMContentLoaded', function () { seen.push('dcl-doc'); });
    window.addEventListener('DOMContentLoaded', function () { seen.push('dcl-win'); });

    // ── (3) The delay is not decoration. A later timer must not outrun an earlier one.
    var order = [];
    setTimeout(function () { order.push('late'); }, 100);
    setTimeout(function () { order.push('early'); }, 0);
    R.push('timersDeferred:' + (order.length === 0));   // …and neither ran synchronously

    // ── (4) A throwing task must not kill the loop. This one throws; the next MUST still run.
    var afterThrow = false;
    setTimeout(function () { throw new Error('deliberate'); }, 0);
    setTimeout(function () { afterThrow = true; }, 1);

    // ── (5) insertAdjacentText — the third sibling.
    R.push('iaText:' + (typeof document.body.insertAdjacentText === 'function'));

    // ── (6) DOMException exists (every DOM method that FAILS fails by throwing one).
    R.push('domException:' + (typeof DOMException === 'function'));

    // Report once the page has SETTLED — which is strictly later than `load`.
    //
    // Reporting from inside the `load` listener is too early, and the reason is the whole design:
    // during loading the virtual clock's budget is 0, so only *immediate* tasks run. `load` is what
    // OPENS the budget — and it opens it at the END of its own dispatch. So a listener running
    // during that dispatch has not yet seen the page's delayed timers fire.
    //
    // Scheduling the report far in the future orders it last (virtual time: it costs nothing), so it
    // observes the page in its final state — which is the state that actually matters.
    window.addEventListener('load', function () {
      setTimeout(function () {
        R.push('order:' + order.join(','));
        R.push('afterThrow:' + afterThrow);
        R.push('seen:' + seen.join(','));
        R.push('readyComplete:' + (document.readyState === 'complete'));
        document.getElementById('out').textContent = R.join(' ');
      }, 5000);
    });
  </script></body></html>"#;

#[test]
fn the_document_lifecycle_fires_the_clock_orders_and_a_throwing_task_does_not_kill_the_loop() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://lifecycle.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    // If `load` never fired, the reporting listener never ran and `out` is still "-". That IS the bug.
    assert_ne!(
        got.trim(),
        "-",
        "G_LIFECYCLE: the `load` event NEVER FIRED, so the reporting listener never ran.\n  \
         `DOMContentLoaded` and `load` were not dispatched ANYWHERE in this engine for 40+ ticks. A \
         site whose init lives in `window.addEventListener('load', …)` — a very large fraction of the \
         web — simply never initialised, in silence, with nothing in any log to say so."
    );

    for claim in [
        // (1) the browsing-context tree
        "parentSelf:true",
        "topSelf:true",
        "framesSelf:true",
        "openerNull:true",
        "walkTerminates:true",
        // (2) the lifecycle
        "readyLoading:true",     // an inline script runs while the document is still parsing
        "readyComplete:true",    // …and `load` means loading is over
        "seen:dcl-doc,dcl-win,load",
        // (3) the clock
        "timersDeferred:true",
        "order:early,late",      // 0ms before 100ms, REGARDLESS of the order they were queued in
        // (4) the loop survives a throwing task
        "afterThrow:true",
        // (5)/(6) the DOM surface WPT needs
        "iaText:true",
        "domException:true",
    ] {
        assert!(
            got.contains(claim),
            "G_LIFECYCLE: expected `{claim}`\n  got: {got}\n\n  \
             Each of these was found by upstream WPT and NONE of them move a box — which is why the \
             265-site Chromium differential could not see any of them. `order:late,early` means the \
             delay is being thrown away and every debounce and retry-backoff on the web runs in the \
             wrong order. `afterThrow:false` means one bad callback stops the page's clock forever. \
             `seen:` missing `load` means no site's onload handler has ever run here."
        );
    }
}
