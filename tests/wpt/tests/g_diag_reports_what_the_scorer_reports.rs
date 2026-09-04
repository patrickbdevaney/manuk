//! **G_DIAG_REPORTS_WHAT_THE_SCORER_REPORTS — the diagnostic answered 0 for every file it was ever
//! pointed at.**
//!
//! `diag` exists to answer one question: *a test file produced nothing — why?* Its headline field
//! counted the file's tests itself:
//!
//! ```js
//!   testsCreated: (globalThis.tests && globalThis.tests.length) || 0
//! ```
//!
//! testharness.js keeps `tests` **inside its own closure** and `expose()`s only its public API, so
//! `globalThis.tests` is `undefined`, `undefined || 0` is `0`, and the field could not report
//! anything else. ⚠⚠ **And reading `tests.tests` — the field the array actually lives in — still
//! answers 0**, which is the part worth remembering: it was never one typo away from working.
//!
//! Measured: `css/cssom-view/elementFromPoint.html` scores **8 of 11** in the real runner, and `diag`
//! said `testsCreated: 0`.
//!
//! ⭐⭐⭐ **A DIAGNOSTIC THAT CANNOT REPORT A NON-ZERO IS WORSE THAN NO DIAGNOSTIC, BECAUSE IT DOES
//! NOT STAY SILENT — IT ACCUSES.** It sent t1412 hunting `<body onload>` (which found a real defect,
//! by luck) and very nearly sent t1414 hunting 1,546 phantom `cssom-view` bugs.
//!
//! ⭐⭐⭐ **AND THE RUNNER ALREADY HAD THE ANSWER.** `harness.rs`'s `REPORT_JS` registers an
//! `add_completion_callback` and emits `<script id="__wpt_results__">` with every test's name and
//! status — the payload the SCORE is computed from. `diag` had invented a second way to count the
//! same thing and got a worse one: *one rule, two implementations*, in the diagnostic itself.
//!
//! `onloadCalls` was removed rather than fixed — `globalThis.__onCalls` has no writer anywhere in the
//! repository, so it was a second permanent zero. **A field nothing populates is not a measurement.**

use manuk_wpt::harness::DIAG_PROBE_JS;

#[test]
fn the_diagnostic_reads_the_scorer_s_own_payload() {
    // ── 1. IT READS THE RUNNER'S PAYLOAD. This is the whole fix: one source of truth for "how many
    // tests did this file create", shared with the thing that computes the score.
    assert!(
        DIAG_PROBE_JS.contains("__wpt_results__"),
        "the probe must read `<script id=__wpt_results__>` — the payload `harness.rs`'s \
         `add_completion_callback` emits and the SCORE is computed from. Counting the tests a second, \
         independent way is what produced a permanent zero."
    );

    // ── 2. THE TWO DEAD FIELDS ARE GONE, BY NAME. Each was a permanent 0 that read as a finding.
    assert!(
        !DIAG_PROBE_JS.contains("globalThis.tests"),
        "`globalThis.tests` is UNDEFINED — testharness keeps `tests` in its closure and exposes only \
         its public API. Any expression rooted there answers 0 forever, including the `tests.tests` \
         spelling that looks like the fix."
    );
    assert!(
        !DIAG_PROBE_JS.contains("__onCalls"),
        "`globalThis.__onCalls` has NO WRITER anywhere in this repository, so `onloadCalls` was a \
         second permanent zero. A field nothing populates is not a measurement."
    );

    // ── 3. THE CONTROL: the fields that DO have writers are still reported. Without this, arms 1-2
    // pass by deleting the probe.
    for field in ["errors", "loadFired", "harness", "hasIframe", "frameNodes"] {
        assert!(
            DIAG_PROBE_JS.contains(field),
            "CONTROL: `{field}` is still reported — `__errors` and `__loadFired` are both written \
             (`dom_bindings.rs` sets `__loadFired`), so they are real measurements and must survive \
             the removal of the fake ones."
        );
    }

    // ── 4. AND `results: null` MUST BE POSSIBLE. A bare `diag` does not install the hook, and the
    // absence of the payload is a statement about the TOOL — not an accusation that the file created
    // nothing, which is exactly the false accusation this tick removes.
    assert!(
        DIAG_PROBE_JS.contains("return null"),
        "the probe must be able to answer `results: null` when the hook was not installed. A \
         diagnostic that reports 0 where it means 'I did not look' is how this defect read as a \
         finding for its whole life."
    );
}
