//! **G_LIFECYCLE_PREEMPTION — a `DOMContentLoaded`/`load` handler that never returns is CUT, and the
//! page survives the cut.**
//!
//! `G_SCRIPT_PREEMPTION` (t1198) proved the watchdog can stop a single task that does not return, and
//! it proved it through a `setTimeout` — i.e. through a **drain**, which is where t1198 armed
//! `ScriptDeadline`: `event_loop::run_deferred` and `event_loop::run_with_fetcher`, and nowhere else.
//! Its own recorded residue said the job was not finished — *"inline `<script>` still unreachable"*.
//!
//! ⚠⚠⚠ **THE LIFECYCLE IS A THIRD ENTRY POINT, AND IT IS THE ONE THE CORPUS LANDS IN.** t1226 named
//! SCORABILITY (74.2%) as the binding cap on M1 and `timeout-150s` (13 sites) as its largest
//! engine-owned bucket. t1227 reduced one of those sites on our own clock, with no Chromium in the
//! picture, and the answer was singular — `payb.jp` completes thirteen seconds of load phases:
//!
//! ```text
//!   external scripts                    ms=5103
//!   cascade+layout+blocking scripts     ms=721
//!   deferred scripts                    ms=7338
//!   <nothing, ever again>               <- 97% of a core until the sweep's own timeout kills it
//! ```
//!
//! The next statement after that marker is `page.fire_lifecycle("DOMContentLoaded", …)`, which builds
//! a `ReflowScope` and calls `eval_for_test` — `PageContext::eval` — **raw**. So the single most
//! common place on the web to put initialisation code (`DOMContentLoaded`, `window.onload`) was the
//! one script round with no clock able to reach it. **A site that times out scores ZERO**, so this
//! raises the M1 *cap* rather than the fill.
//!
//! **THE PROMISE COMES FIRST, exactly as in `G_SCRIPT_PREEMPTION`**, because preemption is one bad
//! constant away from the North Star's *"fast because we never ran the script"* trap. ARM 1 asserts a
//! slow-but-finishing lifecycle handler is **not touched** and its DOM writes land. ARM 2 is the
//! counterfactual (`MANUK_MAX_DRAIN_MS=0` must stay genuinely unbounded, so ARM 3 cannot be satisfied
//! by nothing happening). ARM 3 is the cut. ARM 4 is the half t1227 flagged as the reason this is not
//! a one-liner: a preempted `DOMContentLoaded` must leave the page **renderable and still
//! progressing**, not half-initialised — the static DOM intact, the pre-handler writes kept, and the
//! `load` event still delivered afterwards.
//!
//! **To watch it go RED:** delete the `ScriptDeadline::arm` block from `PageContext::eval`
//! (`engine/js/src/dom_bindings.rs`) and ARM 3 hangs for the full spin instead of being cut — which
//! is HEAD's behaviour and is what the sweep was measuring as a 150s timeout.

use manuk_text::FontContext;

/// The spin lives in a `DOMContentLoaded` listener — **not** a `setTimeout`, which is the shape
/// `G_SCRIPT_PREEMPTION` already covers via the drain. There is no task boundary anywhere in this
/// page's lifecycle round, so the task ceiling and the drain's own clock check are both structurally
/// unable to stop it; a pass here is preemption of the host's synchronous re-entry or it is nothing.
const SPINNER: &str = r##"<!doctype html><html><body>
<div id="sink">hello</div>
<div id="out">DCL-DID-NOT-COMPLETE</div>
<div id="booted">INLINE-DID-NOT-RUN</div>
<div id="loaded">LOAD-DID-NOT-FIRE</div>
<script>
  document.getElementById('booted').textContent = 'INLINE-RAN';
  window.addEventListener('load', function () {
    document.getElementById('loaded').textContent = 'LOAD-FIRED';
  });
  document.addEventListener('DOMContentLoaded', function () {
    var t = Date.now();
    while (Date.now() - t < SPIN_MS) { /* one host re-entry, no task boundary in reach */ }
    document.getElementById('out').textContent = 'DCL-COMPLETED';
  });
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
fn a_dom_content_loaded_handler_that_never_returns_is_cut_and_the_page_survives() {
    let fonts = FontContext::new();

    // ── ARM 1, THE PROMISE. 300ms of work in a `DOMContentLoaded` handler under a 5s budget is slow,
    //    ordinary and entirely legitimate — hydration and analytics boot do worse on real sites. It
    //    must run to completion and its write must land. If a later tick tightens the deadline to
    //    make a load number look better, this fails before the cut below passes.
    std::env::set_var("MANUK_MAX_DRAIN_MS", "5000");
    let ok = manuk_page::Page::load(&fixture(300), "https://lifecycle.test/under", &fonts, 800.0);
    assert_eq!(
        text_of(&ok, "#booted"),
        "INLINE-RAN",
        "VACUITY: the fixture's inline script never ran, so nothing below measures the engine"
    );
    assert_eq!(
        text_of(&ok, "#out"),
        "DCL-COMPLETED",
        "VACUITY + PROMISE: a `DOMContentLoaded` handler that FINISHES inside its budget must run \
         and must not be preempted. If this is the placeholder, the lifecycle never fired at all \
         and ARM 3 would pass for the wrong reason."
    );

    // ── ARM 2, THE COUNTERFACTUAL. `MANUK_MAX_DRAIN_MS=0` is the documented no-clock-bound mode and
    //    must stay genuinely unbounded. This is what proves the 6s spin is real.
    std::env::set_var("MANUK_MAX_DRAIN_MS", "0");
    let t0 = std::time::Instant::now();
    let unbounded =
        manuk_page::Page::load(&fixture(6000), "https://lifecycle.test/free", &fonts, 800.0);
    let t_unbounded = t0.elapsed();
    assert_eq!(
        text_of(&unbounded, "#out"),
        "DCL-COMPLETED",
        "VACUITY: with the budget DISABLED the 6s lifecycle spin must run to completion \
         ({t_unbounded:?}). If it did not, the cut below proves nothing about preemption."
    );

    // ── ARM 3, THE CUT. Same page, same spin, a 500ms budget. `fire_lifecycle` is ONE synchronous
    //    host re-entry containing ONE never-returning handler: no task boundary exists for the
    //    ceiling or the drain clock to check.
    std::env::set_var("MANUK_MAX_DRAIN_MS", "500");
    let t1 = std::time::Instant::now();
    let bounded = manuk_page::Page::load(
        &fixture(6000),
        "https://lifecycle.test/capped",
        &fonts,
        800.0,
    );
    let t_bounded = t1.elapsed();
    assert_eq!(
        text_of(&bounded, "#out"),
        "DCL-DID-NOT-COMPLETE",
        "the 6s `DOMContentLoaded` handler must be TERMINATED at the 500ms budget. It was not, \
         which means `PageContext::eval` is still calling `evaluate_script` with no `ScriptDeadline` \
         armed — the unguarded third entry point t1227 named ({t_bounded:?} vs {t_unbounded:?} \
         unbounded)."
    );
    assert!(
        t_bounded * 2 < t_unbounded,
        "the cut must show up on the CLOCK too, not only in the DOM: bounded {t_bounded:?} vs \
         unbounded {t_unbounded:?}. Comparable times mean the handler still burned the thread."
    );

    // ── ARM 4, AND THE PAGE MUST STILL BE A PAGE. This is the half that made t1227 refuse to bank
    //    the arm blind: `fire_lifecycle` is called from several phases and a cut in the middle of
    //    the lifecycle must not leave the document half-initialised.
    assert_eq!(
        text_of(&bounded, "#sink"),
        "hello",
        "the document must survive the cut intact — we abandon the handler, not the page"
    );
    assert_eq!(
        text_of(&bounded, "#booted"),
        "INLINE-RAN",
        "work the page completed BEFORE the preemption must still be in the DOM — terminating a \
         handler must not roll back what earlier script already did"
    );
    assert_eq!(
        text_of(&bounded, "#loaded"),
        "LOAD-FIRED",
        "⚠ THE LIFECYCLE MUST KEEP GOING. Cutting the `DOMContentLoaded` handler must not poison \
         the context for the `load` round that follows it — a cut that silently ends all further \
         script would trade a hang for a dead page, which the ratchet refuses outright."
    );

    // Leave the process on the shipped default, so nothing after this reads a test-only value.
    std::env::remove_var("MANUK_MAX_DRAIN_MS");

    println!(
        "lifecycle preemption: bounded {t_bounded:?} (handler cut) vs unbounded {t_unbounded:?} \
         (handler ran to completion); page intact and `load` still fired"
    );
}
