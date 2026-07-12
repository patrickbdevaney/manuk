//! **G_TEARDOWN** — no exit path may bypass `Drop`.
//!
//! A workaround that hides a crash is a data-loss bug wearing a disguise, and this project has now
//! shipped that exact bug twice:
//!
//!  * `libc::_exit()` in `main`, added to skip SpiderMonkey's teardown crash. It skipped every
//!    `atexit` handler — including the ones that flush buffered state.
//!  * `std::process::exit(0)` at the end of the GUI event loop, added for the same reason. It
//!    skipped every destructor **and every line of `main`'s ordered shutdown**, which is where the
//!    cookie jar and `localStorage` are written to the profile. The browser discarded the user's
//!    data on every single quit, and reported exit code 0 while doing it.
//!
//! Both were invisible to every other gate, because every other gate measures what happens while
//! the browser is *running*. This one measures what happens when it stops.
//!
//! The rule: **the process exits by returning from `main`.** If SpiderMonkey (or anything else)
//! crashes on the way out, that is a bug to fix at its cause, never to skip past.

use std::path::Path;

/// Source files whose exit behaviour this gate governs. A `process::exit`/`_exit` in a *tool* is
/// fine (a CLI reporting a failed check); in the **shipping browser's** startup/shutdown path it is
/// a data-loss vector.
const SHIPPING_EXIT_PATHS: &[&str] = &["src/main.rs", "src/gui.rs"];

#[test]
fn no_exit_path_bypasses_drop() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut offenders: Vec<String> = Vec::new();

    for rel in SHIPPING_EXIT_PATHS {
        let path = root.join(rel);
        let Ok(src) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (i, line) in src.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            if code.contains("process::exit") || code.contains("libc::_exit") {
                offenders.push(format!("{rel}:{}: {}", i + 1, line.trim()));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "G_TEARDOWN: the shipping browser must exit by RETURNING from `main`, so that destructors \
         run and the profile (cookies, localStorage, session) is flushed.\n\n\
         A `process::exit` / `_exit` here skips all of that, and both times this was added it was \
         to paper over a teardown crash — which is a data-loss bug wearing a disguise.\n\n\
         Offending lines:\n  {}",
        offenders.join("\n  ")
    );
}

/// The profile flush must happen on the *normal* exit path, not only in some handler that a fast
/// exit could skip. This asserts the ordering exists in `main` at all: flush, then tear the JS
/// engine down, then return.
#[test]
fn main_flushes_the_profile_before_tearing_down_js() {
    let src = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main.rs"))
        .expect("shell main.rs");
    let pos = |needle: &str| src.find(needle);

    let cookies = pos("save_cookies()").expect("G_TEARDOWN: main must flush the cookie jar on exit");
    let storage =
        pos("webstorage::save()").expect("G_TEARDOWN: main must flush localStorage on exit");
    let shutdown =
        pos("manuk_js::shutdown()").expect("G_TEARDOWN: main must shut SpiderMonkey down on exit");

    assert!(
        cookies < shutdown && storage < shutdown,
        "G_TEARDOWN: the profile must be flushed BEFORE the JS engine is torn down — tearing down \
         first can abort the process and take the unflushed profile with it"
    );
}
