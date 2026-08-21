//! **G_ONE_JS_THREAD_PER_PROCESS — exactly one thread in a process can run JavaScript, and every
//! later thread gets a `Page` whose scripts SILENTLY never execute.**
//!
//! SpiderMonkey's engine may be initialised once per process and `JS_ShutDown()` is terminal. Our
//! `TeardownGuard` runs it when a thread that owns the engine exits — so the **first thread to
//! finish tears JavaScript down for the whole process**. A `Page` built afterwards constructs
//! successfully, parses fine, lays out fine, and its `<script>` elements do nothing. The error
//! (`"SpiderMonkey has already been shut down in this process"`) is produced and swallowed on the way
//! out.
//!
//! ⭐⭐⭐ **`libtest` SPAWNS A THREAD PER `#[test]`, INCLUDING AT `--test-threads=1`.** Measured:
//!
//! ```text
//!     PAGE t1: js=ran  thread=ThreadId(2)
//!     PAGE t2: js=x    thread=ThreadId(4)      <- second test, silently JS-dead
//!     PAGE t3: js=x    thread=ThreadId(5)
//!     PAGE t3-child-thread: js=x thread=ThreadId(6)
//! ```
//!
//! So in any test binary with more than one test, **exactly one test gets working JS and the rest
//! silently do not — and which one is thread-scheduling order.** That is the real reason the gate
//! suite is one-`#[test]`-per-binary. It was a workaround for this, and because nobody wrote down
//! what it was working around, it reads as a style choice and the 12 binaries listed in
//! `docs/wiki/js-engine.md` grew a second test anyway.
//!
//! ⚠⚠⚠ **THE SILENCE IS A LOAD-BEARING SAFETY NET.** The obvious repair — keep the engine alive so
//! later threads can use it — was measured at t1341 and it is not a repair: with shutdown suppressed
//! so the engine stays alive and its handle valid, a **second thread constructing a `Page` while the
//! first is still parked and holding its engine SIGSEGVs immediately.** The constraint is ONE JS
//! THREAD PER PROCESS; it is not our drop order. Removing the flag converts a quiet dead engine into
//! a crash, which is strictly worse.
//!
//! This gate therefore PINS the contract rather than the wish: `js_available()` is the honest
//! predicate, it agrees with what the page actually did, and no caller may treat "the script did
//! nothing" as evidence about the script.
//!
//! ⚠ There is a SECOND regime, and this gate deliberately does not enter it: two JS threads ALIVE at
//! once **SIGSEGV outright** rather than going quiet. That is a live Bar-0 hazard for the twelve gate
//! binaries that hold more than one `#[test]` and build a `Page`, because `libtest` runs them
//! concurrently by default — see `docs/wiki/js-engine.md`.
//!
//! **To watch it go RED:** make `js_available()` return `true` unconditionally — the second thread
//! still runs no script, and the arm asserting that the predicate AGREES with the observable fails.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body><div id="o">no</div>
  <script>document.getElementById('o').textContent = 'ran';</script></body></html>"#;

/// Builds a page on the current thread and reports whether its inline script executed.
fn script_ran() -> bool {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://jsthread.test/", &fonts, 800.0);
    let root = page.dom().root();
    let o = manuk_css::query_selector_all(page.dom(), root, "#o")[0];
    page.dom().text_content(o).contains("ran")
}

#[test]
fn js_available_agrees_with_whether_scripts_actually_run() {
    // ⚠ Thread A, then A EXITS, then thread B. The order is the point: this gate pins the SEQUENTIAL
    // regime, which is the quiet one. It deliberately does NOT run two JS threads CONCURRENTLY —
    // that regime SIGSEGVs (measured t1341), and a gate that crashes the process asserts nothing and
    // takes the wall down with it.
    let a = std::thread::spawn(|| (manuk_js::js_available(), script_ran()));
    let (a_available, a_ran) = a.join().expect("the first JS thread must not crash");

    assert!(
        a_available && a_ran,
        "G_ONE_JS_THREAD_PER_PROCESS: the FIRST thread to ask must get JavaScript and run the \
         script — available={a_available} ran={a_ran}."
    );

    // A's exit ran `TeardownGuard`, which ran `JS_ShutDown()` for the WHOLE PROCESS. Thread B is
    // therefore JS-dead — and the contract is that `js_available()` SAYS SO, rather than leaving a
    // caller to infer it from a page that merely looks inert.
    let b = std::thread::spawn(|| (manuk_js::js_available(), script_ran()));
    let (b_available, b_ran) = b.join().expect("a JS-dead thread must refuse, not crash");

    assert!(
        !b_available,
        "G_ONE_JS_THREAD_PER_PROCESS: `js_available()` still says yes on a thread created after the \
         engine owner exited. Either the teardown stopped running — which would restore the SIGSEGV \
         it prevents — or the predicate has stopped tracking it."
    );
    assert_eq!(
        b_ran, b_available,
        "G_ONE_JS_THREAD_PER_PROCESS: `js_available()` = {b_available} but the script ran = {b_ran}.\n\n  \
         The predicate must agree with the observable. A page whose scripts silently did not run is \
         indistinguishable from a page whose scripts ran and had no effect, and that ambiguity is \
         how a gate passes vacuously."
    );
}
