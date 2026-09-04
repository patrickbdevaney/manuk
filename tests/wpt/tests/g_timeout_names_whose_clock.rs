//! **G_TIMEOUT_NAMES_WHOSE_CLOCK — the site watchdog blamed this engine for every timeout, including
//! the ones where nothing of ours was running.**
//!
//! `timeout-150s` was the largest ENGINE-OWNED unscored bucket in the t1406 corpus sweep — 9 of 200
//! sites — and it is the row that sent t1408 hunting an engine bug. One of those nine, measured:
//!
//! ```text
//!   swiftspinus.com   our whole load           5.7 s   (the engine's own phase log, every phase)
//!                     Chromium's screenshot    8.6 s
//!                     the process, meanwhile   sleeping in hrtimer_nanosleep
//!                     the row we filed         timeout-150s: "this engine did not finish"
//! ```
//!
//! ⭐⭐⭐ **THE MACHINERY TO SAY IT HONESTLY ALREADY EXISTED, TWICE, AND THE WATCHDOG CONSULTED
//! NEITHER.** t861 built `Unmeasurable::OracleTimeout` for exactly this — *"the reference browser's
//! hang is not our timeout"* — and the per-side timing twenty lines below the watchdog opens with
//! *"time each engine separately, and attribute the cost to whoever actually spent it."* The
//! site-level watchdog fired on a wall clock and hard-coded the engine's name into the message.
//!
//! The fix publishes which side is running (`SITE_SIDE`) and the watchdog asks. This gate holds the
//! DECISION, which is the part that can be wrong quietly; the span itself is closed by a `Drop` guard
//! because the oracle block has four `continue` arms in it, and a store on the happy path alone would
//! leave the flag reading ORACLE for the whole of the NEXT site — the first mis-attribution replaced
//! by a second.
//!
//! ⚠ Side `2` (scoring/probing — neither engine) is filed AGAINST US on purpose. There is no honest
//! tag for the instrument's own cost yet, and inventing one would let it leave the denominator, which
//! is the `EXCLUDED-RISING` failure the fixed denominator exists to forbid. It is named in the
//! message instead, so it can be measured rather than laundered.

use manuk_wpt::fidelity::{timeout_reason, Unmeasurable};

#[test]
fn a_site_timeout_names_whose_clock_it_burned() {
    // ── 1. THE ORACLE'S HANG IS THE ORACLE'S.
    assert_eq!(
        timeout_reason(1, 150),
        Unmeasurable::OracleTimeout(150),
        "when Chromium is the side that was running, the row must say so — `swiftspinus.com` loads \
         here in 5.7s and screenshots in Chromium in 8.6s, and it was filed as OUR timeout."
    );
    assert!(
        timeout_reason(1, 150).explain().contains("ORACLE"),
        "and the explanation must carry it too: the tag is what a script reads, the explanation is \
         what a human reads, and the backlog was misled by the human-readable half."
    );

    // ── 2. OURS IS STILL OURS. Without this the fix is "stop counting hard sites", which is the
    // laundering §0's fixed denominator forbids.
    assert_eq!(
        timeout_reason(0, 150),
        Unmeasurable::Timeout(150),
        "a timeout while WE were rendering is still ours — `morikoshi.net` really did spend 191s in \
         one load phase, and that row must not be re-labelled away."
    );
    assert!(
        !timeout_reason(0, 150).explain().contains("ORACLE"),
        "our own timeout must not claim the oracle's name either — the mis-attribution has two \
         directions and only one of them was ever tempting."
    );

    // ── 3. THE INSTRUMENT'S OWN COST IS FILED AGAINST US, DELIBERATELY. This arm exists so the
    // decision is visible: if a future tick invents an `instrument-timeout` tag, this row is where it
    // has to argue for it, and the fixed-denominator rule is right here to argue with.
    assert_eq!(
        timeout_reason(2, 150),
        Unmeasurable::Timeout(150),
        "scoring/probing is NEITHER engine, and it is charged to us until it is measured properly — \
         a new tag would let the instrument's own slowness leave the denominator."
    );

    // ── 4. BOTH TAGS STAY IN THE IN-SCOPE DENOMINATOR. The whole point of naming the oracle is to
    // stop blaming the engine, NOT to stop counting the site.
    // The partition is `fidelity-progress.sh`'s, mirrored here in code exactly as fidelity.rs's own
    // sibling assertion mirrors it — a rename must not be able to move a reason across the line.
    let excluded_prefixes = ["bot-wall", "empty-"];
    let excluded_exact = ["probe-blocked", "unreachable", "http-404", "http-503"];
    for r in [Unmeasurable::OracleTimeout(150), Unmeasurable::Timeout(150)] {
        let tag = r.tag();
        assert!(
            !excluded_prefixes.iter().any(|p| tag.starts_with(p))
                && !excluded_exact.contains(&tag.as_str()),
            "{tag} must stay IN the in-scope denominator: the reference failing is not a reason to \
             stop counting the site, only a reason to stop blaming this engine for it."
        );
    }
}
