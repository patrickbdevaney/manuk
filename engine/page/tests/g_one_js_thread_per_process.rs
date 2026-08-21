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
//! once **SIGSEGV outright** rather than going quiet. `libtest` runs a binary's tests concurrently by
//! default, so two tests that both build a `Page` are in exactly that shape — see
//! `docs/wiki/js-engine.md`.
//!
//! ⭐⭐ **BUT THE TRIGGER IS `<script>`, NOT `Page` — measured at t1342, and it corrects t1341's own
//! write-up.** Two concurrent `Page`s were run twice, changing one variable:
//!
//! ```text
//!     two concurrent Pages, NO <script>     both threads fine, js_available=true on both
//!     two concurrent Pages, WITH <script>   SIGSEGV (signal 11) before either test returns
//! ```
//!
//! The engine is not initialised by parsing or layout; it is initialised by a script actually
//! running. So t1341's claim that the twelve multi-test binaries were "a live Bar-0 hazard surviving
//! on scheduling luck" was **over-scoped, and this gate's own subject was the thing it got wrong**.
//! Counting `<script>` per test across all twelve at their pre-merge revision: every one of them had
//! **at most one** scripted test, so not one was actually crashing. They were one `<script>` away.
//!
//! ⚠ The scan arm below keys on `manuk_page::Page` anyway — DELIBERATELY WIDER THAN THE MEASURED
//! HAZARD. Keying on `<script>` would make the gate go green the moment someone deletes a script tag
//! and red again when a later tick adds one back, in a file whose author was not thinking about
//! SpiderMonkey at all. The whole point of the rule is that it holds without anyone having to hold
//! the mechanism in their head. A wider predicate that costs nothing (495 of 506 files already obey
//! it) beats a precise one that has to be re-checked on every edit.
//!
//! ## G_ONE_PAGE_TEST_PER_BINARY — the same finding, enforced instead of described
//!
//! A rule that only exists in prose is a rule that drifts, which is how twelve binaries acquired a
//! second `#[test]`. The second arm of this gate SCANS THE TREE: every integration-test binary that
//! names `manuk_page::Page` must hold exactly one test attribute. The repair for a violation is
//! always to **MERGE** — call the extra tests as plain functions from the one that remains — and
//! never to split, because the wall is link-bound at ~520 static mozjs binaries and a new binary is
//! the single most expensive thing a gate can cost.
//!
//! ⚠ This arm lives inside this gate's own single test on purpose. Adding a binary to enforce a rule
//! about not adding binaries would have been the joke version.
//!
//! **To watch it go RED:** *(the JS arm)* make `js_available()` return `true` unconditionally — the
//! second thread still runs no script, and the arm asserting that the predicate AGREES with the
//! observable fails. *(the scan arm)* put a second `#[test]` back on any of the twelve merged
//! functions — e.g. `a_clip_path_applies_to_the_subtree_against_the_declaring_box` — and this gate
//! names the file and its test count.

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

    // ── G_ONE_PAGE_TEST_PER_BINARY ────────────────────────────────────────────────────────────
    //
    // Everything above measures the hazard once. This arm stops it from coming back. A
    // `Page`-building binary that grows a second `#[test]` puts itself in the CONCURRENT regime —
    // the one that SIGSEGVs — and, whenever it is lucky enough not to crash, silently runs one of
    // its tests with a dead JS engine. Twelve binaries drifted into exactly that state before
    // t1342 because the one-test-per-binary rule was a workaround nobody had written down.
    //
    // ⭐ The repair is to MERGE, never to split: the wall is link-bound (~500 gate binaries) and a
    // new binary costs a full static mozjs link. This arm lives inside this gate's single test for
    // the same reason — it eats its own rule.
    let scanned = scan_test_binaries();

    // Anti-vacuity, first: a scanner that found nothing, or that cannot tell a one-test file from a
    // many-test one, passes this gate perfectly while asserting nothing at all.
    assert!(
        scanned.len() >= 300,
        "G_ONE_PAGE_TEST_PER_BINARY: only {} test binaries scanned. The tree holds ~520; a count \
         this low means SCANNED_DIRS or the walk is broken, and a broken scan PASSES.",
        scanned.len()
    );
    let page_building = scanned.iter().filter(|s| s.builds_page).count();
    assert!(
        page_building >= 300,
        "G_ONE_PAGE_TEST_PER_BINARY: only {page_building} binaries look like they build a `Page`. \
         The `manuk_page::Page` needle has stopped matching — the gate is now scanning a population \
         it cannot see the hazard in."
    );
    let multi = scanned.iter().filter(|s| s.tests > 1).count();
    assert!(
        multi >= 1,
        "G_ONE_PAGE_TEST_PER_BINARY: not one scanned file has more than a single test attribute. \
         Either the tree really is uniform — or the per-line attribute count is broken and every \
         file reads as 1. The counter must be shown to DISCRIMINATE before its zeroes mean anything."
    );

    let offenders: Vec<&Scanned> = scanned
        .iter()
        .filter(|s| s.builds_page && s.tests > 1)
        .collect();
    assert!(
        offenders.is_empty(),
        "G_ONE_PAGE_TEST_PER_BINARY: {} test binaries build a `Page` and hold more than one test \
         attribute:\n{}\n\n  \
         `libtest` gives every test its own thread and SpiderMonkey allows one JS thread per \
         process. Concurrently (the default) these SIGSEGV; serially, all but one run with a \
         torn-down engine and assert against scripts that never executed.\n  \
         FIX BY MERGING the extra tests into the one that remains — call them as plain functions \
         from its body. Do NOT split them into new binaries: each costs a static mozjs link on a \
         wall that is already link-bound.",
        offenders.len(),
        offenders
            .iter()
            .map(|s| format!("    {} — {} tests", s.rel, s.tests))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// THE SOURCE-SCANNING ARM
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// Crate-relative test directories this gate is responsible for. Anything else is out of scope and
/// says so, rather than being silently unscanned.
const SCANNED_DIRS: &[&str] = &[
    "engine/css/tests",
    "engine/dom/tests",
    "engine/html/tests",
    "engine/media/tests",
    "engine/net/tests",
    "engine/page/tests",
    "engine/text/tests",
    "tests/wpt/tests",
];

/// `<repo>/engine/page` is this crate; the root is two levels up.
fn repo_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("engine/page must sit two directories below the repo root")
        .to_path_buf()
}

/// One scanned integration-test binary.
struct Scanned {
    rel: String,
    /// Counts lines that ARE the attribute, not lines that mention it — every doc comment in this
    /// file talks about `#[test]`, and a substring match would count those too.
    tests: usize,
    builds_page: bool,
}

fn scan_test_binaries() -> Vec<Scanned> {
    // Assembled at compile time so this file contains no line that IS the attribute apart from its
    // own single real one. A literal here would make the gate fail against itself.
    let attr = concat!("#[", "test]");
    let root = repo_root();
    let mut out = Vec::new();

    for dir in SCANNED_DIRS {
        let path = root.join(dir);
        let entries = std::fs::read_dir(&path).unwrap_or_else(|e| {
            panic!(
                "G_ONE_PAGE_TEST_PER_BINARY: cannot read {dir}: {e}. \
                 A directory that moved must be re-listed in SCANNED_DIRS, never silently dropped."
            )
        });

        for entry in entries {
            let entry = entry.expect("a readable directory entry");
            let file = entry.path();
            if file.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let src = std::fs::read_to_string(&file)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", file.display()));

            out.push(Scanned {
                rel: format!(
                    "{dir}/{}",
                    file.file_name().unwrap_or_default().to_string_lossy()
                ),
                tests: src
                    .lines()
                    .filter(|l| l.trim_start().starts_with(attr))
                    .count(),
                // Textual on purpose, and the honest limit of this gate: a binary that reaches a
                // `Page` through a helper crate is not caught. Every one in the tree today names
                // the type directly.
                builds_page: src.contains("manuk_page::Page"),
            });
        }
    }
    out
}
