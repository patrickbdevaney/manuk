//! **G_INTERACTION_DRAIN_BUDGET — a click's task may run longer than a load's, because cutting it
//! wedges the page.**
//!
//! The drain budget (`MANUK_MAX_DRAIN_MS`, 5 s) exists so a page that never converges cannot freeze the
//! tab at load. It is enforced by a watchdog that TERMINATES the running script. Applied to a click
//! handler's task that has a worse failure than the one it prevents: a framework that was inside its
//! render or commit never reaches its `finally`, so it believes it is still working and refuses all
//! later work. Measured on a real MUI/React clone (a filter panel in a body-level portal): one click on a
//! 560-node page took 5-10 s, was cut at 5 s, React threw #327 "Should not already be working", and the
//! panel's buttons ran their `onClick` and then did nothing, for the rest of the session.
//!
//! The shape without a framework: task A raises a "busy" flag, does slow work, lowers the flag. If A is
//! terminated the flag stays up, and the next input's handler B sees a page that is stuck.
//!
//! Expectations: A finishes inside the interaction allowance (6x the load budget) and B then finds the
//! page idle; a task that overruns even that is still cut, so the allowance does not unbound the
//! drain. The same fixture under `MANUK_INTERACTION_MUTATE=1` (no allowance) must go red.

use manuk_text::FontContext;

const PAGE: &str = r##"<!doctype html><html><body>
<button id="a">slow</button><button id="b">probe</button>
<div id="outa">A-NOT-DONE</div><div id="outb">B-NOT-RUN</div>
<script>
  var busy = false;
  document.getElementById('a').addEventListener('click', function () {
    busy = true;
    setTimeout(function () {
      var t = Date.now();
      while (Date.now() - t < SPIN_MS) { }
      busy = false;
      document.getElementById('outa').textContent = 'A-DONE';
    }, 0);
  });
  document.getElementById('b').addEventListener('click', function () {
    document.getElementById('outb').textContent = busy ? 'B-WEDGED' : 'B-IDLE';
  });
</script></body></html>"##;

fn text_of(page: &manuk_page::Page, sel: &str) -> String {
    let root = page.dom().root();
    let hits = manuk_css::query_selector_all(page.dom(), root, sel);
    assert!(!hits.is_empty(), "fixture is missing {sel}");
    page.dom().text_content(hits[0])
}

/// Load, click A, click B; return (outa, outb).
fn run(spin_ms: u32, name: &str) -> (String, String) {
    let fonts = FontContext::new();
    let src = PAGE.replace("SPIN_MS", &spin_ms.to_string());
    let mut page = manuk_page::Page::load(
        &src,
        &format!("https://interact.test/{name}"),
        &fonts,
        800.0,
    );
    let node = |page: &manuk_page::Page, sel: &str| {
        let root = page.dom().root();
        manuk_css::query_selector_all(page.dom(), root, sel)[0]
    };
    let a = node(&page, "#a");
    page.dispatch_click(a, &fonts, 800.0);
    let b = node(&page, "#b");
    page.dispatch_click(b, &fonts, 800.0);
    (text_of(&page, "#outa"), text_of(&page, "#outb"))
}

#[test]
fn a_slow_click_task_finishes_and_a_runaway_is_still_cut() {
    // Base budget 300 ms: the interaction allowance is 1800 ms.
    std::env::set_var("MANUK_MAX_DRAIN_MS", "300");

    // Over the load budget, under the allowance: honest slow work. It must finish and leave the page idle.
    let (a, b) = run(900, "slow");
    assert_eq!(
        a, "A-DONE",
        "a click's task that takes 3x the load budget (900 ms vs 300 ms) was terminated"
    );
    assert_eq!(
        b, "B-IDLE",
        "the task that was cut left the page 'busy' forever: the next input found it wedged"
    );

    // Over the allowance too: still stopped, so the drain is not unbounded.
    let (a, _) = run(5000, "runaway");
    assert_eq!(
        a, "A-NOT-DONE",
        "a click task that overran even the interaction allowance must still be terminated"
    );

    std::env::remove_var("MANUK_MAX_DRAIN_MS");
}
