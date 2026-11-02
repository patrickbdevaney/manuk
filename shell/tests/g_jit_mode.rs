//! SEC.DID.9 gate: the shell binary runs a web page with baseline-only JIT (Ion, native regexp and
//! both wasm tiers off), local targets keep every tier, and `--jit-mode=` overrides. Mutant: with
//! `MANUK_TEST_BREAK=1` the child is told `--jit-mode=full`, which is what a skipped switch looks
//! like, and the web-page assertion goes red.
#![cfg(feature = "_sm")]

use std::process::Command;

fn status(args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_manuk"))
        .arg("jit-status")
        .args(args)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[test]
fn web_page_is_baseline_only_local_is_full() {
    let broken = std::env::var("MANUK_TEST_BREAK").is_ok();
    let web = if broken {
        status(&["https://example.com/", "--jit-mode=full"])
    } else {
        status(&["https://example.com/"])
    };
    // TIERS order: ion, baseline, baseline-interp, native regexp, wasm baseline, wasm optimizing.
    assert_eq!(
        web, "jit=baseline tiers=[0, 1, 1, 0, 0, 0]",
        "web page must be baseline-only"
    );
    assert!(status(&["http://127.0.0.1:8080/"]).starts_with("jit=full"));
    assert!(status(&["/tmp/x.html"]).starts_with("jit=full"));
    assert!(status(&["https://example.com/", "--jit-mode=none"])
        .starts_with("jit=none tiers=[0, 0, 0, 0, 0, 0]"));
    // userinfo cannot smuggle a local host past the check
    assert!(status(&["https://localhost@evil.example/"]).starts_with("jit=baseline"));
}
