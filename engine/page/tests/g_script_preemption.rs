//! **G_SCRIPT_PREEMPTION — the drain budget can stop a SINGLE task that never returns.**
//!
//! `G_DRAIN_BUDGET` bounds the event loop by the clock, and its own comment says exactly what that
//! bound cannot do:
//!
//! > *"Checked only on the task boundary, so a single long-running task is not interrupted
//! > mid-flight — this bounds a runaway CHAIN, and never preempts JS."*
//!
//! Tick 1196 priced that hole against the real corpus: the fidelity sweep's 150s per-site timeout —
//! the largest engine-owned bucket in the exit metric — is **four consecutive drain-budget
//! overruns**, none of which the budget could cut, because each was one task that did not return.
//!
//! Tick 1197 built the fix and proved it **inert**: `JS_AddInterruptCallback` merely *registers* a
//! callback. SpiderMonkey polls it only after `JS_RequestInterruptCallback` has been called, which
//! in Firefox is a **watchdog thread**. A 60s spin ran to completion with the callback installed,
//! twice. That negative is why this gate exists in this shape: *the mechanism must be observed
//! stopping something*, never inferred from the API being present.
//!
//! **THE PROMISE COMES FIRST, and it is the harder half.** Any preemption is one bad constant away
//! from the North Star's *"fast because we never ran the script"* trap, so this gate asserts the
//! negative before the cut: a task that runs a long time and **finishes inside its budget must not
//! be touched**, and its DOM writes must land. Only then does it assert the cut, and it asserts it
//! as a **counterfactual** — the same page with `MANUK_MAX_DRAIN_MS=0` must run the spin to
//! completion — rather than against a wall-clock threshold that a loaded build box can fake.

use manuk_text::FontContext;

/// A single `setTimeout` task that busy-waits. One task, so the 20,000-task ceiling can never be the
/// thing that stops it — the only bound that can reach this shape is preemption of the running
/// script. `SPIN_MS` is a placeholder the fixture builder substitutes.
const SPINNER: &str = r##"<!doctype html><html><body>
<div id="sink">hello</div>
<div id="out">SPIN-DID-NOT-COMPLETE</div>
<div id="booted">INLINE-DID-NOT-RUN</div>
<script>
  document.getElementById('booted').textContent = 'INLINE-RAN';
  setTimeout(function () {
    var t = Date.now();
    while (Date.now() - t < SPIN_MS) { /* the shape no task-boundary check can reach */ }
    document.getElementById('out').textContent = 'SPIN-COMPLETED';
  }, 0);
</script></body></html>"##;

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
fn a_single_never_returning_task_is_cut_without_cutting_the_page() {
    let fonts = FontContext::new();

    // ── ARM 1, THE PROMISE. A task that busy-waits for 300ms under a 5s budget is slow, ordinary,
    //    and entirely legitimate — carousels and hydration do worse. It must run to completion and
    //    its DOM write must land. If a later tick tightens the deadline to make a load number look
    //    better, this fails before the cut below passes.
    std::env::set_var("MANUK_MAX_DRAIN_MS", "5000");
    let ok = manuk_page::Page::load(&fixture(300), "https://preempt.test/under", &fonts, 800.0);
    assert_eq!(
        text_of(&ok, "#booted"),
        "INLINE-RAN",
        "VACUITY: the fixture's script never ran, so nothing below measures the engine"
    );
    assert_eq!(
        text_of(&ok, "#out"),
        "SPIN-COMPLETED",
        "a task that FINISHES inside its budget must never be preempted — cutting honest work is \
         the 'fast because we never ran the script' trap, not an optimisation"
    );

    // ── ARM 2, THE COUNTERFACTUAL. `MANUK_MAX_DRAIN_MS=0` is the documented no-clock-bound mode, and
    //    it must stay genuinely unbounded. This is what proves the spin is real: if the 6s task did
    //    not actually run to completion here, ARM 3 would be satisfied by nothing happening.
    std::env::set_var("MANUK_MAX_DRAIN_MS", "0");
    let t0 = std::time::Instant::now();
    let unbounded =
        manuk_page::Page::load(&fixture(6000), "https://preempt.test/free", &fonts, 800.0);
    let t_unbounded = t0.elapsed();
    assert_eq!(
        text_of(&unbounded, "#out"),
        "SPIN-COMPLETED",
        "VACUITY: with the budget DISABLED the 6s spin must run to completion ({t_unbounded:?}). \
         If it did not, the cut below proves nothing about preemption."
    );

    // ── ARM 3, THE CUT. Same page, same spin, a 500ms budget. Nothing on the task boundary can stop
    //    this — there is exactly one task — so a pass here is preemption or it is nothing.
    std::env::set_var("MANUK_MAX_DRAIN_MS", "500");
    let t1 = std::time::Instant::now();
    let bounded =
        manuk_page::Page::load(&fixture(6000), "https://preempt.test/capped", &fonts, 800.0);
    let t_bounded = t1.elapsed();
    assert_eq!(
        text_of(&bounded, "#out"),
        "SPIN-DID-NOT-COMPLETE",
        "the 6s task must be TERMINATED at the 500ms budget. It was not, which means the interrupt \
         callback is registered but never REQUESTED — exactly the inert state tick 1197 measured \
         ({t_bounded:?} vs {t_unbounded:?} unbounded)."
    );
    assert!(
        t_bounded * 2 < t_unbounded,
        "the cut must show up on the CLOCK too, not only in the DOM: bounded {t_bounded:?} vs \
         unbounded {t_unbounded:?}. Comparable times mean the page was abandoned for some other \
         reason and the spin still burned the thread."
    );

    // ── AND THE PAGE SURVIVES. Preemption costs the runaway, never the document: the inline script
    //    that ran BEFORE the spin kept its write, and the static DOM is intact. A cut that also
    //    blanked the page would be a capability regression bought with a performance fix, which the
    //    ratchet refuses outright.
    assert_eq!(
        text_of(&bounded, "#sink"),
        "hello",
        "the document must survive the cut intact — we abandon the task, not the page"
    );
    assert_eq!(
        text_of(&bounded, "#booted"),
        "INLINE-RAN",
        "work the page completed BEFORE the preemption must still be in the DOM — terminating a \
         script must not roll back what earlier script already did"
    );

    // Leave the process on the shipped default, so nothing after this reads a test-only value.
    std::env::remove_var("MANUK_MAX_DRAIN_MS");

    println!(
        "preemption: bounded {t_bounded:?} (task cut) vs unbounded {t_unbounded:?} (task ran to \
         completion); page intact"
    );
}
