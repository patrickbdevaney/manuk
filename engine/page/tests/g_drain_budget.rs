//! **G_DRAIN_BUDGET — the event loop is bounded by the CLOCK, not only by a task count.**
//!
//! `MAX_TASKS_PER_DRAIN` has capped `run_deferred` since Bar 0 was written, and its stated purpose is
//! a wall-clock one: *"the alternative is a frozen tab."* But a count is a poor proxy for a clock, and
//! the gap is not marginal. Measured per drain, with the clock bound disabled:
//!
//! ```text
//!   en.wikipedia.org       1 task,      4ms      ← a page that converges
//!   mangago.me         20000 tasks, 30216ms      ← and it did this FIVE times in one load
//!   theguardian.com                     did not finish inside 480s
//! ```
//!
//! Four orders of magnitude. The same declared policy — *"this page is not converging; paint what we
//! have"* — handed one page 4ms of grace and another half a minute of it, five times over, for
//! reasons unrelated to what either had cost the user.
//!
//! **AND BOUNDING IT RAISED FIDELITY, which is the part that is easy to get backwards.**
//! `theguardian.com` scores **22.9% visual with no clock bound and 52.4% with one**, while also
//! loading faster. The runaway drain was not doing the page's work, it was *starving* it: spinning a
//! self-rescheduling chain consumed the load budget the subresource phases and the settle needed. So
//! this is not the North Star's *"fast because we never ran the script"* trap — it is its opposite.
//! **We ran less of the runaway and got more of the page.**
//!
//! **The second half of the fix had no measurement because it had no bound at all.**
//! `run_with_fetcher` — which `run()` delegates to and `dom_bindings` drives — had neither a count
//! ceiling nor a clock. The exact runaway `MAX_TASKS_PER_DRAIN` exists to forbid ran forever there,
//! and its `did_io` arm makes it worse: a page that re-fetches on every delivery `continue`s past the
//! task check indefinitely. One rule, two implementations, one of them enforced.
//!
//! **WHAT THIS GATE MUST PROVE IS THE NEGATIVE.** Any bound on script execution is one bad constant
//! away from *"fast because we never ran the script"*, so the load-bearing claim here is not that a
//! runaway gets cut — it is that a page doing **thousands of real, converging tasks finishes every
//! one of them** and lands its DOM. The cut is the easy half; the not-cutting is the promise.

use manuk_text::FontContext;

/// A page whose script does a LOT of genuine, terminating work: 5,000 chained `setTimeout(…, 0)`
/// tasks, each appending a node. It converges — that is the point — but it is far past the "tens of
/// tasks" a typical load costs, so a budget set anywhere near real work would eat it.
const CONVERGING: &str = r##"<!doctype html><html><body>
<div id="sink"></div><div id="out">SCRIPT-DID-NOT-RUN</div>
<script>
  var done = 0;
  function step(n) {
    if (n === 0) {
      document.getElementById('out').textContent =
        'done=' + done + ' nodes=' + document.getElementById('sink').childNodes.length;
      return;
    }
    var d = document.createElement('i');
    d.textContent = 'x';
    document.getElementById('sink').appendChild(d);
    done++;
    setTimeout(function () { step(n - 1); }, 0);
  }
  step(5000);
</script></body></html>"##;

/// A page that does NOT converge: a task that reschedules itself forever. Not a contrived shape —
/// it is `setInterval(fn, 0)`, which is on carousels, clocks and pollers all over the web.
const RUNAWAY: &str = r##"<!doctype html><html><body>
<div id="sink">hello</div><div id="spins">0</div>
<script>
  var spins = 0;
  function spin() { spins++; document.getElementById('spins').textContent = String(spins); setTimeout(spin, 0); }
  spin();
</script></body></html>"##;

fn text_of(page: &manuk_page::Page, sel: &str) -> String {
    let root = page.dom().root();
    let hits = manuk_css::query_selector_all(page.dom(), root, sel);
    assert!(!hits.is_empty(), "fixture is missing {sel}");
    page.dom().text_content(hits[0])
}

#[test]
fn the_drain_is_bounded_by_the_clock_without_cutting_a_converging_page() {
    let fonts = FontContext::new();

    // ── THE PROMISE, and it is why this gate exists rather than the cut below.
    //
    // A budget that clipped honest work would be indistinguishable, on every timing number this
    // project reports, from an optimisation. So: 5,000 real chained tasks, and every one must run.
    // If a later tick tightens the budget to make a load number look better, this fails first.
    let page = manuk_page::Page::load(CONVERGING, "https://drain.test/ok", &fonts, 800.0);
    let got = text_of(&page, "#out");
    assert_ne!(
        got, "SCRIPT-DID-NOT-RUN",
        "VACUITY: the converging fixture never ran, so nothing below measures the engine"
    );
    assert_eq!(
        got, "done=5000 nodes=5000",
        "a CONVERGING page must run every one of its 5000 tasks AND land every node — got {got:?}. \
         A drain budget that clips real work is the 'fast because we never ran the script' trap, \
         not an optimisation."
    );

    // ── THE CUT, measured as a COUNTERFACTUAL rather than against a wall-clock constant.
    //
    // The first draft of this gate asserted "the runaway load finishes in under 20s" and it passed
    // with the clock bound DELETED — because a cheap self-rescheduling task hits the 20,000-task
    // ceiling quickly, so the count was doing the stopping and the gate never touched the clock at
    // all. That is this project's recurring "green for the wrong reason" shape, and it is why the
    // RED patch is run before the claim is believed rather than after.
    //
    // So both arms run here, in one process, and the gate compares them. `max_drain_ms()` reads its
    // env on every drain precisely so this is possible. The comparison is on SPINS — how much of the
    // runaway actually got to run — which is machine-independent in a way a millisecond threshold on
    // a loaded build box is not.
    std::env::set_var("MANUK_MAX_DRAIN_MS", "0");
    let t0 = std::time::Instant::now();
    let unbounded = manuk_page::Page::load(RUNAWAY, "https://drain.test/spin-free", &fonts, 800.0);
    let t_unbounded = t0.elapsed();
    let spins_unbounded: u64 = text_of(&unbounded, "#spins").trim().parse().unwrap_or(0);

    std::env::set_var("MANUK_MAX_DRAIN_MS", "500");
    let t1 = std::time::Instant::now();
    let bounded = manuk_page::Page::load(RUNAWAY, "https://drain.test/spin-capped", &fonts, 800.0);
    let t_bounded = t1.elapsed();
    let spins_bounded: u64 = text_of(&bounded, "#spins").trim().parse().unwrap_or(0);

    // VACUITY FIRST: if the spin never ran, every comparison below is satisfied by nothing happening.
    assert!(
        spins_unbounded > 100,
        "VACUITY: the runaway script barely ran ({spins_unbounded} spins), so this gate proves \
         nothing about bounding it"
    );

    // THE POINT: with a clock budget the runaway gets decisively less of the user's machine. Without
    // it, the only thing standing between a `setInterval(fn, 0)` and the tab is a task COUNT — which
    // on a real site (mangago.me) meant 20,000 tasks and 30 seconds, five times in one load.
    assert!(
        spins_bounded * 3 < spins_unbounded,
        "the clock budget must cut the runaway decisively: bounded ran {spins_bounded} spins vs \
         unbounded {spins_unbounded} (bounded {t_bounded:?} vs unbounded {t_unbounded:?}). If these \
         are comparable, the TASK CEILING is doing the stopping and this gate is not testing the \
         clock at all — which is exactly how its first draft passed with the bound deleted."
    );

    // And the document is still THERE. Bounding a runaway must cost the runaway, never the page —
    // the same promise the load budget makes about subresources.
    assert_eq!(
        text_of(&bounded, "#sink"),
        "hello",
        "the document must survive the cut intact — we abandon the spin, not the page"
    );

    // Leave the process on the shipped default, so nothing after this reads a test-only value.
    std::env::remove_var("MANUK_MAX_DRAIN_MS");

    println!(
        "converging: {got} · runaway spins {spins_bounded} (bounded, {t_bounded:?}) vs \
         {spins_unbounded} (unbounded, {t_unbounded:?})"
    );
}
