//! **G_SCHEDULER_POSTTASK — `scheduler.postTask`/`yield` genuinely schedule by priority.**
//!
//! Surface Audit #26 (tick 528) added `scheduler.postTask/yield` as `unknown`, with the caution that
//! it "may be genuinely absent — needs a real priority queue, not a setTimeout alias." A presence
//! probe found the methods DO exist (dom_bindings.rs) — but presence is not function: a
//! `postTask` that resolved without running the callback, or ran tasks in post-order instead of
//! priority-order, would be exactly the setTimeout-alias stub the audit warned about. This gate
//! measures FUNCTION and pins it: `unknown` → `gated`.
//!
//! The main-thread scheduler is what React's scheduler, cooperative-yielding libraries and
//! `scheduler.yield()` loops feature-detect to keep a click handler responsive while a background
//! prefetch waits. Asserted:
//!   * `postTask(cb)` returns a promise that resolves to the callback's RETURN VALUE.
//!   * a `user-blocking` task posted AFTER a `background` task in the same turn runs FIRST — real
//!     priority ordering, the whole point of the API.
//!   * `scheduler.yield()` resolves (hands control back and resumes).
//!   * a task posted with an already-aborted signal REJECTS and never runs.
//!
//! **RED, run:** in `dom_bindings.rs`'s `__drain`, iterate the priority buckets in reverse
//! (`pr = 2 → 0`) — `order:ub>bg` flips to `bg>ub` and the gate fails; or resolve `postTask` with
//! `undefined` instead of `cb()` and `vals:b,42,7` flips.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html>
<html><body>
  <div id="out">-</div>
  <script>
    var R = {
      a: [],
      push: function (s) { this.a.push(s); var o = document.getElementById('out');
                           if (o) { o.textContent = this.a.join(' '); } },
    };
    try {
      var order = [];
      Promise.all([
        scheduler.postTask(function () { order.push('bg'); return 'b'; }, { priority: 'background' }),
        scheduler.postTask(function () { order.push('ub'); return 42; }, { priority: 'user-blocking' }),
        scheduler.postTask(function () { return 7; })   // default: user-visible
      ]).then(function (vals) {
        R.push('vals:' + vals.join(','));    // array order preserved -> b,42,7
        R.push('order:' + order.join('>'));  // priority order -> ub>bg
        return scheduler.yield();
      }).then(function () {
        R.push('yield:ok');
        var ac = new AbortController();
        ac.abort();
        return scheduler.postTask(function () { return 1; }, { signal: ac.signal })
          .then(function () { R.push('abort:RAN-bad'); },
                function () { R.push('abort:rejected'); });
      }).then(function () {
        R.push('done:true');
      }).catch(function (e) { R.push('threw:' + (e && e.name ? e.name : e)); });
    } catch (e) { R.push('threw:' + (e && e.name ? e.name : e)); }
  </script>
</body></html>"##;

#[test]
fn scheduler_posttask_runs_by_priority_and_yield_resolves() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://sched.test/", &fonts, 800.0);

    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("SCHEDULER PROBE: {got}");

    for (claim, why) in CLAIMS {
        assert!(
            got.contains(claim),
            "G_SCHEDULER_POSTTASK: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}

const CLAIMS: &[(&str, &str)] = &[
    (
        "vals:b,42,7",
        "postTask returns a promise that resolves to the callback's RETURN VALUE — a stub that \
         resolved with undefined would fail here",
    ),
    (
        "order:ub>bg",
        "a user-blocking task posted AFTER a background task runs FIRST — real priority scheduling, \
         not a FIFO setTimeout alias; this is the whole reason the API exists",
    ),
    (
        "yield:ok",
        "scheduler.yield() resolves — a cooperative-yield loop depends on it resuming",
    ),
    (
        "abort:rejected",
        "a task posted with an already-aborted signal rejects and never runs — the cancellation path",
    ),
    (
        "done:true",
        "the whole async chain ran to completion inside Page::load's event loop",
    ),
];
