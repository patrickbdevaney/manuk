//! **G_SCHEDULER_POSTTASK — `scheduler.postTask(cb, options)`, priority-ordered main-thread work.**
//!
//! The scheduler modern frameworks use to keep the UI responsive: run work at a stated PRIORITY so a
//! `user-blocking` click handler pre-empts a `background` prefetch. React's scheduler and cooperative-
//! yielding loops feature-detect it; absent, `scheduler.postTask(...)` threw on `undefined`.
//!
//! Teeth an inert `setTimeout` alias cannot pass:
//!   * `priority-order` — three tasks posted `background, user-blocking, user-visible` RUN in the order
//!     `user-blocking, user-visible, background`. A wrapper that ignores priority runs them in post
//!     order and fails this.
//!   * `value` — `postTask` resolves with the callback's return value.
//!   * `abort` — a task whose `signal` aborts before it runs is REMOVED and its promise rejects.
//!
//! Proven RED: delete the `scheduler` block and `present` reads `undefined` while the first call throws.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<script>
var r = [];
function push(s) { r.push(s); }
function finish() { document.getElementById('out').textContent = r.join(' '); }

try {
  push('present:' + (typeof scheduler === 'object' && scheduler !== null &&
                     typeof scheduler.postTask === 'function'));

  var order = [];
  var pv = scheduler.postTask(function () { order.push('bg'); }, { priority: 'background' });
  scheduler.postTask(function () { order.push('ub'); }, { priority: 'user-blocking' });
  scheduler.postTask(function () { order.push('uv'); }, { priority: 'user-visible' });

  // value: the promise resolves with the callback's return.
  var pval = scheduler.postTask(function () { return 7 * 6; });

  // abort: a task whose signal is already aborted must reject and never run.
  var ac = new AbortController();
  ac.abort('nope');
  var pab = scheduler.postTask(function () { order.push('SHOULD-NOT-RUN'); }, { signal: ac.signal });

  pval.then(function (v) {
    push('value:' + (v === 42));
    return pab.then(function () { return 'resolved'; }, function (e) { return 'rejected:' + e; });
  }).then(function (abResult) {
    push('abort:' + (abResult === 'rejected:nope' && order.indexOf('SHOULD-NOT-RUN') < 0));
    // by now all priority tasks have drained; assert the ORDER.
    push('priority-order:' + (order.join(',') === 'ub,uv,bg'));
    finish();
  }, function (e) { push('CHAIN-THREW:' + e); finish(); });
} catch (e) {
  push('THREW:' + e);
  finish();
}
</script></body></html>"##;

#[test]
fn scheduler_post_task_runs_in_priority_order() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://sched.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("SCHEDULER RESULT: {got}");

    for claim in [
        "present:true",
        "value:true",          // resolves with the callback's return value
        "abort:true",          // an aborted-signal task rejects and never runs
        "priority-order:true", // user-blocking, then user-visible, then background
    ] {
        assert!(
            got.contains(claim),
            "G_SCHEDULER_POSTTASK: expected `{claim}`\n  got: {got}\n\n  \
             `scheduler.postTask` must run tasks in PRIORITY order (user-blocking > user-visible > \
             background), resolve with the callback's value, and reject a task aborted before it ran. \
             A setTimeout alias that ignores priority fails `priority-order`."
        );
    }
}
