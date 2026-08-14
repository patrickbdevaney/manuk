//! **G_INLINE_SCRIPT_PREEMPTION — a blocking `<script>` that never returns is CUT, and the scripts
//! AFTER it still run.**
//!
//! This closes the third and last host→JS entry point. `ScriptDeadline` — the watchdog thread
//! without which `JS_AddInterruptCallback` is inert (t1197's measured negative) — reaches script
//! from exactly three places, and they were armed one at a time by three separate ticks:
//!
//! ```text
//!   event_loop::run_deferred / run_with_fetcher   timers, microtasks, fetch settlement   t1198
//!   PageContext::eval  (Page::fire_lifecycle)     DOMContentLoaded / load handlers        t1228
//!   run_one_script                                 inline + blocking <script> at parse    THIS
//! ```
//!
//! ⚠⚠⚠ **t1198 NAMED THIS ONE AS ITS OWN RESIDUE — *"inline `<script>` still unreachable"* — and it
//! then went unwritten for thirty ticks while the fidelity sweep kept scoring the sites it kills as
//! `timeout-150s`.** A page's blocking scripts run before anything is painted, so a spin here is
//! strictly worse than a spin in a lifecycle handler: there is no partial page to fall back to.
//!
//! **`run_one_script` is the ONE place both passes call** — `PageContext::load` runs the
//! paint-blocking scripts through it and `run_deferred_scripts` runs the `defer`/`async`/module ones
//! through it — which is why arming it once covers both, and why its own doc comment says two copies
//! of *"how to run a script"* is how the two passes silently stop agreeing.
//!
//! **THE PROMISE COMES FIRST**, as in both sibling gates: ARM 1 asserts a slow-but-finishing script
//! is not touched. ARM 2 is the counterfactual (`MANUK_MAX_DRAIN_MS=0` must stay genuinely
//! unbounded). ARM 3 is the cut.
//!
//! ⚠⚠⚠ **ARM 4 IS THE ONE THAT MATTERS AND IT IS NOT THE SAME QUESTION t1228 ANSWERED.** There,
//! the `load` round that survived a cut `DOMContentLoaded` was a *separate host re-entry* with a
//! drain in between. Here the next `<script>` is evaluated by the very next iteration of the same
//! loop, with nothing in between — so if terminating a script leaves the context in a state that
//! poisons the next `evaluate_script`, **one runaway script would silently kill every script after
//! it on the page**, which is a capability regression bought with a hang fix. The ratchet refuses
//! that trade, so the gate asserts the negative directly.
//!
//! **To watch it go RED:** delete the `ScriptDeadline::arm` block from `run_one_script`
//! (`engine/js/src/dom_bindings.rs`) — ARM 3 then reports `SPIN-COMPLETED`, which is HEAD.

use manuk_text::FontContext;

/// Three sibling `<script>` elements: one before the spin, the spin, one after. The spin is a
/// **blocking, parse-time** script — not a task, not a lifecycle handler — so neither the task
/// ceiling, nor the drain clock, nor t1228's lifecycle arm can reach it. A pass here is preemption
/// of `run_one_script` or it is nothing.
const SPINNER: &str = r##"<!doctype html><html><body>
<div id="sink">hello</div>
<div id="before">BEFORE-DID-NOT-RUN</div>
<div id="out">SPIN-DID-NOT-COMPLETE</div>
<div id="after">AFTER-DID-NOT-RUN</div>
<script>
  document.getElementById('before').textContent = 'BEFORE-RAN';
</script>
<script>
  var t = Date.now();
  while (Date.now() - t < SPIN_MS) { /* no task boundary is in reach of this */ }
  document.getElementById('out').textContent = 'SPIN-COMPLETED';
</script>
<script>
  document.getElementById('after').textContent = 'AFTER-RAN';
</script>
</body></html>"##;

fn fixture(spin_ms: u32) -> String {
    SPINNER.replace("SPIN_MS", &spin_ms.to_string())
}

fn text_of(page: &manuk_page::Page, sel: &str) -> String {
    let root = page.dom().root();
    let hits = manuk_css::query_selector_all(page.dom(), root, sel);
    assert!(!hits.is_empty(), "fixture is missing {sel}");
    page.dom().text_content(hits[0])
}

#[test]
fn a_blocking_script_that_never_returns_is_cut_and_the_scripts_after_it_still_run() {
    let fonts = FontContext::new();

    // ── ARM 1, THE PROMISE. A 300ms blocking script under a 5s budget is slow, ordinary and
    //    entirely legitimate — a large inline framework bundle parses and boots for longer than
    //    that. It must run to completion and its DOM write must land.
    std::env::set_var("MANUK_MAX_DRAIN_MS", "5000");
    let ok = manuk_page::Page::load(&fixture(300), "https://inline.test/under", &fonts, 800.0);
    assert_eq!(
        text_of(&ok, "#before"),
        "BEFORE-RAN",
        "VACUITY: the fixture's first script never ran, so nothing below measures the engine"
    );
    assert_eq!(
        text_of(&ok, "#out"),
        "SPIN-COMPLETED",
        "a blocking script that FINISHES inside its budget must never be preempted — cutting \
         honest work is the 'fast because we never ran the script' trap, not an optimisation"
    );

    // ── ARM 2, THE COUNTERFACTUAL. `MANUK_MAX_DRAIN_MS=0` is the documented no-clock-bound mode and
    //    must stay genuinely unbounded. Without this, ARM 3 could be satisfied by nothing happening.
    std::env::set_var("MANUK_MAX_DRAIN_MS", "0");
    let t0 = std::time::Instant::now();
    let unbounded =
        manuk_page::Page::load(&fixture(6000), "https://inline.test/free", &fonts, 800.0);
    let t_unbounded = t0.elapsed();
    assert_eq!(
        text_of(&unbounded, "#out"),
        "SPIN-COMPLETED",
        "VACUITY: with the budget DISABLED the 6s blocking script must run to completion \
         ({t_unbounded:?}). If it did not, the cut below proves nothing about preemption."
    );

    // ── ARM 3, THE CUT.
    std::env::set_var("MANUK_MAX_DRAIN_MS", "500");
    let t1 = std::time::Instant::now();
    let bounded =
        manuk_page::Page::load(&fixture(6000), "https://inline.test/capped", &fonts, 800.0);
    let t_bounded = t1.elapsed();
    assert_eq!(
        text_of(&bounded, "#out"),
        "SPIN-DID-NOT-COMPLETE",
        "the 6s blocking script must be TERMINATED at the 500ms budget. It was not, which means \
         `run_one_script` still calls `evaluate_script` with no `ScriptDeadline` armed — t1198's \
         own recorded residue, the last of the three entry points ({t_bounded:?} vs {t_unbounded:?} \
         unbounded)."
    );
    assert!(
        t_bounded * 2 < t_unbounded,
        "the cut must show up on the CLOCK too, not only in the DOM: bounded {t_bounded:?} vs \
         unbounded {t_unbounded:?}. Comparable times mean the script still burned the thread."
    );

    // ── ARM 4, AND THE REST OF THE PAGE STILL RUNS. The next `<script>` is evaluated by the very
    //    next iteration of the same loop, with no drain and no host round-trip in between — so a
    //    termination state that outlives the script it terminated would make ONE runaway script
    //    silently kill every script after it. That is a capability regression bought with a hang
    //    fix, and the ratchet refuses it outright.
    assert_eq!(
        text_of(&bounded, "#after"),
        "AFTER-RAN",
        "⚠ THE SCRIPTS AFTER THE CUT MUST STILL RUN. Terminating one script must not poison the \
         context for the next one in the same pass — otherwise a single runaway takes the whole \
         page's remaining JavaScript with it, silently."
    );
    assert_eq!(
        text_of(&bounded, "#before"),
        "BEFORE-RAN",
        "work the page completed BEFORE the preemption must still be in the DOM"
    );
    assert_eq!(
        text_of(&bounded, "#sink"),
        "hello",
        "the document must survive the cut intact — we abandon the script, not the page"
    );

    // Leave the process on the shipped default, so nothing after this reads a test-only value.
    std::env::remove_var("MANUK_MAX_DRAIN_MS");

    println!(
        "inline preemption: bounded {t_bounded:?} (script cut) vs unbounded {t_unbounded:?} \
         (script ran to completion); the script AFTER the cut still ran"
    );
}
