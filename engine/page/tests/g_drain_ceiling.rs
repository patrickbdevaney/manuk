//! **G_DRAIN_CEILING_SHORTENS — a page that has already given up does not get a fresh full grace,
//! and a NEW navigation does.**
//!
//! Measured on a page whose entire content is two runaway timers, from the engine's own phase ledger
//! (`manuk-wpt boxes --fetch … --build`, release, `RUST_LOG=manuk_page=info`):
//!
//! ```text
//!     phase                              before        after
//!     cascade+layout+blocking scripts   2280ms       2319ms      <- the full grace, once
//!     deferred scripts                  2286ms         28ms
//!     DOMContentLoaded                  2331ms         29ms
//!     load event                        2405ms         28ms
//!                                      ───────       ──────
//!                                        9.3s          2.4s
//! ```
//!
//! ⚠⚠⚠ **`page_stopped_converging()` existed since t666 and had exactly ONE consumer** — the
//! dynamic-script ROUND loop, which `G_DRAIN_BOUNDS_THE_PAGE` covers. The load PHASE sequence never
//! asked it, so a page that only spins (injecting no scripts, so the round loop is never entered)
//! paid the full 20,000-task ceiling once per phase. That gate's own header explains why its fixture
//! must both spin AND inject to reach the round loop; nobody then asked what a spin-only page costs
//! on the other axis, which is where it costs everything.
//!
//! ⚠ **And `clear_convergence_state()` had NO production caller at all.** Its doc predicted the
//! hazard exactly — *"a flag whose reset lives somewhere else is a flag that eventually leaks across
//! navigations and silently stops a healthy page from running its scripts"* — and the leak had
//! shipped: one non-converging page poisoned every later navigation in the process. It stayed latent
//! while the flag had a single consumer. **A second consumer is what makes a stale flag visible.**
//! The reset now sits beside `manuk_net::begin_navigation()`.
//!
//! ### ⚠⚠ What this gate does NOT do, stated rather than implied
//!
//! It asserts the POLICY, not the end-to-end saving. An end-to-end assertion needs a
//! machine-independent unit, and the obvious one — macrotasks run per navigation — was built,
//! measured and **thrown away**: the counter read 20,000 whether the fix was in or out, because the
//! drains run on more than one thread and a thread-local sees only its own. **A gate that cannot go
//! red is not a gate**, so it was deleted rather than shipped. The table above is a measurement, not
//! an assertion.
//!
//! ⚠ It lives here rather than in `manuk-js` because `verify.sh`'s `T · crate tests` list does not
//! include that crate — a test placed there is documentation.
//!
//! **To watch it go RED:** make `drain_ceiling` return `MAX_TASKS_PER_DRAIN` unconditionally.

use manuk_js::event_loop::{
    clear_convergence_state, drain_ceiling, page_stopped_converging, MAX_TASKS_AFTER_GIVING_UP,
    MAX_TASKS_PER_DRAIN,
};

#[test]
fn a_page_that_has_given_up_gets_a_shorter_ceiling_and_a_new_navigation_gets_a_full_one() {
    assert_eq!(
        drain_ceiling(false),
        MAX_TASKS_PER_DRAIN,
        "a page that has not yet given up must get the FULL grace — a page converging on its \
         8,000th task is a real page, and shortening its first drain is how a working site stops \
         working"
    );
    assert_eq!(
        drain_ceiling(true),
        MAX_TASKS_AFTER_GIVING_UP,
        "a page that has ALREADY hit a ceiling this navigation must not get a fresh full grace: it \
         has answered, and asking again cost 9.3s instead of 2.4s on the spin-only fixture"
    );
    assert!(
        MAX_TASKS_AFTER_GIVING_UP > 0,
        "SHORTENED, not skipped — `DOMContentLoaded` and `load` must still fire and their handlers \
         must still run. A page whose listeners never execute is a different and worse failure than \
         a slow one."
    );
    assert!(
        MAX_TASKS_AFTER_GIVING_UP * 10 < MAX_TASKS_PER_DRAIN,
        "the shortened ceiling must be a different ORDER of grace, not a trim — otherwise the \
         four-phase cost is unchanged and the whole mechanism is decoration"
    );

    // The flag itself: cleared, set, observed, cleared. `clear_convergence_state` had no production
    // caller before t1330, so this is also the first assertion that it does anything at all.
    clear_convergence_state();
    assert!(
        !page_stopped_converging(),
        "cleared state must read as converging"
    );
    assert_eq!(
        drain_ceiling(page_stopped_converging()),
        MAX_TASKS_PER_DRAIN
    );
    manuk_js::event_loop::note_drain_stopped_short_for_test();
    assert!(page_stopped_converging(), "a give-up must be observable");
    assert_eq!(
        drain_ceiling(page_stopped_converging()),
        MAX_TASKS_AFTER_GIVING_UP
    );
    clear_convergence_state();
    assert!(
        !page_stopped_converging(),
        "a NEW navigation must not inherit the previous page's verdict — this is the leak the \
         flag's own doc predicted and that shipped anyway"
    );
    assert_eq!(
        drain_ceiling(page_stopped_converging()),
        MAX_TASKS_PER_DRAIN
    );
}
