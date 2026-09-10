//! **G_A_HANDLED_REJECTION_IS_NOT_REPORTED — the report fired at the wrong moment, and two alarms
//! in three were false.**
//!
//! HTML §8.1.7.5 keeps a list of *about-to-be-notified rejected promises* and drains it in
//! **"notify about rejected promises"**, which runs at the **end of a microtask checkpoint**. The
//! delay is not an optimisation — it is the substance of the algorithm, because the overwhelmingly
//! common shape
//!
//! ```js
//!   (async function () { … })().catch(handle);
//! ```
//!
//! rejects the promise *before* `.catch` is attached: the async function returns an already-rejected
//! promise and the handler goes on the next expression. This engine reported at the instant
//! SpiderMonkey's tracker fired, so it reported failures the page had fully handled.
//!
//! ## Arbitrated against headless Chrome, three cases, one fixture
//!
//! ```text
//!   A  rejected, `.catch` attached SYNCHRONOUSLY on the same line
//!   B  rejected, `.catch` attached from a LATER MICROTASK (still inside the checkpoint)
//!   C  rejected, never handled at all
//!
//!            Chrome              before                        after
//!   n=       1                   3                             1
//!   msgs     [C-never]           [A-caught, B-late, C-never]   [C-never]
//! ```
//!
//! **Two false alarms out of three**, on the one event every error-reporting SDK (Sentry, Bugsnag,
//! Rollbar) and every app's own *"something went wrong"* UI listens to. An app that BRANCHES on it —
//! showing an error screen, logging the user out, retrying a request — did the wrong thing on a page
//! that was working.
//!
//! ⚠ **FOUND BY A VACUITY ARM, NOT BY READING THE TRACKER.** `g_a_page_reports_its_own_boot_failure`
//! asserts that a clean page reports ZERO boot errors; the tick that routed rejections into that
//! harvest added a caught async throw to the clean fixture, and the clean page reported one. The
//! harvest did not create this bug — it made an eight-tick-old one observable at a boundary that
//! already had an assertion on it.
//!
//! ## How it is decided
//!
//! The parked list holds the promise **object** (traced, so a GC between rejection and checkpoint
//! cannot collect it) and asks SpiderMonkey at drain time whether it is *still* unhandled
//! (`GetPromiseIsHandled`) — rather than tracking the `Handled` transition ourselves, which would be
//! a second and weaker copy of a fact the engine already owns.
//!
//! ⚠ The drain runs AFTER the checkpoint's second microtask pass, not between the two: a microtask
//! run by that pass is exactly where case B's `.catch` is attached.
//!
//! Mutations that must turn this red:
//!   1. report from the tracker directly instead of parking   -> n=3
//!   2. drain the list BEFORE the second microtask pass        -> n=2 (B reappears)
//!   3. skip the `GetPromiseIsHandled` check at drain time     -> n=3
//!   4. never drain the list                                   -> n=0, and the vacuity arm fires

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><meta charset=utf-8></head><body><div id="out">-</div>
<script>
var n = 0, msgs = [];
window.addEventListener('unhandledrejection', function (e) {
  n++; msgs.push(String(e.reason && e.reason.message));
});
// A — rejected, then `.catch` attached SYNCHRONOUSLY in the same statement. Handled.
(async function () { throw new Error('A-caught'); })().catch(function () {});
// B — rejected, `.catch` attached from a LATER MICROTASK. Chrome does not report this either.
var pB = (async function () { throw new Error('B-late'); })();
Promise.resolve().then(function () { pB.catch(function () {}); });
// C — never handled at all. This one is a real failure and MUST be reported.
(async function () { throw new Error('C-never'); })();
window.addEventListener('load', function () {
  document.getElementById('out').textContent = 'n=' + n + ' [' + msgs.join(',') + ']';
});
</script></body></html>"##;

#[test]
fn a_rejection_handled_before_the_checkpoint_is_not_an_unhandled_rejection() {
    let fonts = FontContext::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let page = rt.block_on(async {
        let mut p = manuk_page::Page::load_async(HTML, "https://rej.test/", &fonts, 800.0).await;
        p.finish_loading(&fonts, 800.0).await;
        p
    });
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("UNHANDLED REJECTIONS: {got}");

    // ── VACUITY, AND IT IS THE HALF THIS GATE MOST NEEDS. Deferring the report is trivially
    //    "correct" if you defer it forever; case C is what forbids that. A gate that only asserted
    //    A and B are silent would be passed by deleting the reporter.
    assert!(
        got.contains("C-never"),
        "VACUOUS: the genuinely unhandled rejection was not reported either, so this gate is \
         passing on an engine that has simply stopped reporting — got {got:?}"
    );

    // Chrome headless, exactly.
    let want = "n=1 [C-never]";
    assert_eq!(
        got, want,
        "\n  a rejection handled before the microtask checkpoint ends is NOT an unhandled \
         rejection — reporting it makes two alarms in three false on the one event every error \
         reporter listens to\n  want: {want}\n  got:  {got}"
    );

    // …and the harvest agrees, so the boot histogram counts one real failure and not three.
    let harvest: Vec<String> = page
        .boot_errors()
        .iter()
        .map(|e| format!("{}/{}", e.phase, e.message.lines().next().unwrap_or("")))
        .collect();
    assert_eq!(
        harvest.len(),
        1,
        "`Page::boot_errors` must carry exactly the one real failure — a histogram fed three rows \
         here would rank a handled `.catch` above a genuine missing API: {harvest:?}"
    );
    assert!(
        harvest[0].starts_with("rejection/") && harvest[0].contains("C-never"),
        "and it must be the UNHANDLED one, tagged as a rejection: {harvest:?}"
    );
}
