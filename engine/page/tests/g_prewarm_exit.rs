//! **G_PREWARM_EXIT — a runtime built ahead of the page (PERF.1.4) must still tear down cleanly.**
//!
//! `manuk_page::prewarm_js()` builds the SpiderMonkey context while the document request is in
//! flight. It deliberately does not arm the teardown guard (the guard must register after every
//! thread-local the page's scripts create), so `prewarm_js_settle()` owes the arming. Forget it and a
//! page with no script exits with a live context: SIGSEGV in the exit handlers.
//!
//! Like `g_clean_exit`, the child is this test binary re-executed. Modes:
//!   `script`   prewarm -> page WITH a script -> settle   (must exit 0, script ran)
//!   `noscript` prewarm -> page WITHOUT a script -> settle (must exit 0)
//!   `nosettle` prewarm -> page WITHOUT a script, settle skipped (the mutant; must NOT exit 0)

use std::process::Command;

const CHILD_ENV: &str = "MANUK_PREWARM_EXIT_CHILD";

fn run_child(mode: &str) -> std::process::Output {
    Command::new(std::env::current_exe().expect("our own path"))
        .env(CHILD_ENV, mode)
        .args(["--exact", "prewarmed_runtime_exits_cleanly", "--nocapture"])
        .output()
        .expect("re-exec ourselves as the child")
}

#[test]
fn prewarmed_runtime_exits_cleanly() {
    if let Ok(mode) = std::env::var(CHILD_ENV) {
        child(&mode);
        return;
    }

    let out = run_child("script");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("sum:499500"),
        "G_PREWARM_EXIT: the prewarmed runtime never ran the page's script.\nstdout: {stdout}"
    );
    assert!(
        out.status.success(),
        "G_PREWARM_EXIT: script page after prewarm died on exit ({:?})\n{}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );

    let out = run_child("noscript");
    assert!(
        out.status.success(),
        "G_PREWARM_EXIT: a script-free page after prewarm + settle died on exit ({:?})\n{}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );

    // The gate can go red: without the settle, the same page leaves a live context behind.
    let out = run_child("nosettle");
    assert!(
        !out.status.success(),
        "G_PREWARM_EXIT: the unsettled child exited 0, so this gate cannot tell a missing settle \
         from a present one"
    );
}

fn child(mode: &str) {
    manuk_page::prewarm_js();
    let fonts = manuk_text::FontContext::new();
    let html = if mode == "script" {
        "<!doctype html><html><body><div id=out>-</div><script>\
           var n = 0; for (var i = 0; i < 1000; i++) n += i;\
           document.getElementById('out').textContent = 'sum:' + n;\
         </script></body></html>"
    } else {
        "<!doctype html><html><body><p>no script here</p></body></html>"
    };
    let page = manuk_page::Page::load(html, "https://prewarm-exit.test/", &fonts, 800.0);
    if mode == "script" {
        let root = page.dom().root();
        let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
        println!("{}", page.dom().text_content(out));
    }
    if mode != "nosettle" {
        manuk_page::prewarm_js_settle();
    }
}
