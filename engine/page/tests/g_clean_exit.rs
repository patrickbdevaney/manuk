//! **G_CLEAN_EXIT — a process that ran JavaScript must exit with code 0, without being asked to.**
//!
//! For sixty ticks this project carried an open Bar 0 residual: a binary boots SpiderMonkey, does its
//! work perfectly, prints correct output, and then **SIGSEGVs after `main` returns** —
//!
//! ```text
//! mozilla::detail::MutexImpl::~MutexImpl: pthread_mutex_destroy failed: Device or resource busy
//! process didn't exit successfully (signal: 11, SIGSEGV: invalid memory reference)
//! ```
//!
//! SpiderMonkey requires `JS_ShutDown()` before the process exits. Skip it, and its C++ static
//! destructors run against a still-initialized engine and die inside `__run_exit_handlers`.
//!
//! **The workaround was a rule: "every binary must call `manuk_js::shutdown()` last."** And the rule did
//! what rules do — `g_runaway`, `g_alloc`, `g_load_budget` and the shell remembered; `g_globals` and
//! `g_dedup` did not, and crashed. *A convention that half the callers forget is not a fix; it is a list
//! of the places you have not been bitten yet.*
//!
//! It is not cosmetic. A crash in the exit handlers **aborts the handlers that follow it** — and that is
//! exactly where a browser flushes its cookie jar and `localStorage` to the profile (ADR-009). The
//! user-visible bug is **silent data loss on quit**.
//!
//! So the engine now shuts *itself* down — see `spidermonkey::TeardownGuard`, and the one rule that
//! makes it work: **to run first at teardown, register last.**
//!
//! ## How this gate observes it, and why not the obvious way
//!
//! An exit code is only visible from *outside* a process, so this needs a child. The obvious child is a
//! `cargo run --example`, and that is what the first version did — at a cost of **215 seconds** on the
//! verify wall, because it drove a whole second cargo build. Rigor bought with a 6× slower loop is a bad
//! trade, and the ratchet does not permit it.
//!
//! So the child is **this very test binary, re-executed**. It is already built, already linked against
//! SpiderMonkey, and one `MANUK_CLEAN_EXIT_CHILD=1` in its environment turns it into the child. Same
//! evidence — a real process that really runs JavaScript and really returns from `main` without ever
//! calling `shutdown()` — for about a second.

use std::process::Command;

/// Marks the re-executed self as the child.
const CHILD_ENV: &str = "MANUK_CLEAN_EXIT_CHILD";

#[test]
fn a_process_that_ran_javascript_exits_zero_without_being_told_to_shut_down() {
    if std::env::var(CHILD_ENV).is_ok() {
        run_javascript_then_simply_return();
        return;
    }

    // The parent. Re-exec ourselves, as the child.
    let out = Command::new(std::env::current_exe().expect("our own path"))
        .env(CHILD_ENV, "1")
        .args([
            "--exact",
            "a_process_that_ran_javascript_exits_zero_without_being_told_to_shut_down",
            "--nocapture",
        ])
        .output()
        .expect("re-exec ourselves as the child");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    // The exit code proves nothing unless the child ACTUALLY RAN the JavaScript. A child that failed
    // to boot SpiderMonkey would also "exit cleanly", and would certify the opposite of the truth.
    assert!(
        stdout.contains("sum:499500"),
        "G_CLEAN_EXIT: the child never ran the JavaScript, so its exit code proves nothing.\n\
         stdout: {stdout}\nstderr: {stderr}"
    );

    assert!(
        out.status.success(),
        "G_CLEAN_EXIT: the child ran JavaScript correctly and then **died on the way out** ({:?}).\n\n\
         stderr:\n{stderr}\n\
         SpiderMonkey needs `JS_ShutDown()` before the process exits. If the only thing that provides \
         it is a caller remembering to call `manuk_js::shutdown()`, then every binary that forgets \
         crashes in `__run_exit_handlers` — which is where a browser flushes its cookie jar and \
         localStorage. **The user-visible bug is silent data loss on quit.** The engine must tear \
         itself down; it must not depend on being asked.",
        out.status
    );
}

/// The child: run real JavaScript, touch the DOM, print the answer, and then **just return**.
///
/// Note what is deliberately *absent*: `manuk_js::shutdown()`. That is the entire point of this gate.
/// A correct engine does not need to be reminded to stop.
fn run_javascript_then_simply_return() {
    let fonts = manuk_text::FontContext::new();
    let mut page = manuk_page::Page::load(
        "<!doctype html><html><body><div id=out>-</div><script>\
           var n = 0; for (var i = 0; i < 1000; i++) n += i;\
           document.getElementById('out').textContent = 'sum:' + n;\
         </script></body></html>",
        "https://clean-exit.test/",
        &fonts,
        800.0,
    );
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let text = page.dom().text_content(out);
    assert_eq!(
        text, "sum:499500",
        "the child must actually have RUN JavaScript, or its exit code says nothing"
    );
    println!("{text}");
}
