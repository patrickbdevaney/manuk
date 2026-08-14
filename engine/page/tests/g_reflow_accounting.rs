//! **G_REFLOW_ACCOUNTING — the drain must be able to say where a long task went.**
//!
//! The event loop's time budget is **exact between tasks** and blown by a *single* task, and for
//! several ticks nothing could say what that task was doing. Measured t1235 across the CrUX
//! `timeout-150s` bucket:
//!
//! ```text
//!   neutypechic.com   count=1331  elapsed_ms=5001   budget_ms=5000   <- EXACT
//!   7info.ru          count=1     elapsed_ms=9326   budget_ms=5000   <- 9.3s in ONE task
//!   bhramarah.in      count=2     elapsed_ms=21572  budget_ms=5000
//! ```
//!
//! The drain arms `ScriptDeadline`, so the SCRIPT half of such a task is preemptible. What is not is
//! **native work the script triggers** — `JS_RequestInterruptCallback` is polled at interpreter
//! back-edges, and there are none while the thread is inside layout. `dom_bindings::REFLOW_COST`
//! measures exactly that, and t1236 used it to attribute the bucket: **95–99% of every overrun is
//! forced reflow.**
//!
//! This gate keeps that instrument honest, because an accounting counter that silently stops
//! counting reports "no reflow" — which reads as *"reflow is not the problem"* and would send the
//! next tick at the wrong subsystem. It asserts both polarities:
//!
//! 1. a page that does `measure → mutate → measure` **reports reflow**, and
//! 2. a page that never reads geometry **reports none** — so the counter is measuring forced reflow
//!    and not simply "a page loaded".
//!
//! **How to break it:** delete the `REFLOW_COST` update in `force_reflow_if_stale`, or move the
//! timing outside the `IN_REFLOW` guard (which double-counts nested reads and breaks arm 2's zero).
//!
//! ⚠ The counter is **monotonic** and this gate brackets it. The first cut had the drain *reset* it,
//! which is wrong twice: drains NEST, so an inner drain erased the outer one's accounting; and the
//! reset made the counter unreadable from outside a drain, which is how this gate found it — arm 1
//! read a hard 0 for a fixture that visibly forced reflow.

use manuk_text::FontContext;

/// `measure → mutate → measure`, the shape every virtualized list is built out of: each read after a
/// write must lay out before it can answer, so the loop forces a reflow per iteration.
const FORCES_REFLOW: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<div id="host"></div>
<script>
document.addEventListener('DOMContentLoaded', function () {
  var host = document.getElementById('host'), total = 0;
  for (var i = 0; i < 30; i++) {
    var d = document.createElement('div');
    d.style.height = (10 + i) + 'px';
    host.appendChild(d);
    total += host.getBoundingClientRect().height;
  }
  document.getElementById('out').textContent = 'done ' + (total > 0);
});
</script></body></html>"##;

/// The CONTROL: the same shape of work with no geometry read anywhere. Without it, arm 1 only proves
/// the counter is non-zero after *a page*, which a counter incremented in the wrong place also is.
const NO_GEOMETRY_READ: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<div id="host"></div>
<script>
document.addEventListener('DOMContentLoaded', function () {
  var host = document.getElementById('host'), n = 0;
  for (var i = 0; i < 30; i++) {
    var d = document.createElement('div');
    d.style.height = (10 + i) + 'px';
    host.appendChild(d);
    n += d.tagName.length;
  }
  document.getElementById('out').textContent = 'done ' + n;
});
</script></body></html>"##;

#[test]
fn the_drain_can_attribute_a_long_task_to_forced_reflow() {
    let fonts = FontContext::new();

    // Bracketed, not reset — the counter is monotonic on purpose (drains nest, and a reset makes an
    // inner drain erase the outer one's accounting).
    let (before, _) = manuk_js::dom_bindings::reflow_cost();
    let page = manuk_page::Page::load(FORCES_REFLOW, "https://reflow.test/", &fonts, 800.0);
    let calls = manuk_js::dom_bindings::reflow_cost().0 - before;
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    assert!(
        got.starts_with("done"),
        "G_REFLOW_ACCOUNTING: the fixture did not run — {got:?}"
    );
    assert!(
        calls > 0,
        "G_REFLOW_ACCOUNTING: 30 rounds of measure→mutate→measure reported {calls} forced \
         reflows.\n\n  The drain reports `reflow_ms` to say where a task that blew the time budget \
         went. A counter that has stopped counting reports ZERO, which reads as \"reflow is not the \
         problem\" — and t1236 measured that it is 95-99% of every overrun in the timeout bucket."
    );

    let (before, _) = manuk_js::dom_bindings::reflow_cost();
    let page = manuk_page::Page::load(NO_GEOMETRY_READ, "https://noreflow.test/", &fonts, 800.0);
    let control = manuk_js::dom_bindings::reflow_cost().0 - before;
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    assert!(
        page.dom().text_content(out).starts_with("done"),
        "G_REFLOW_ACCOUNTING: the control fixture did not run"
    );
    assert_eq!(
        control, 0,
        "G_REFLOW_ACCOUNTING: a page that never reads geometry reported {control} forced \
         reflows.\n\n  This is the CONTROL, and it is what makes arm 1 mean anything: a counter \
         that fires for every page would satisfy arm 1 while measuring nothing about reflow."
    );
}
