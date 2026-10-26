//! R1.6 gate: agent mode runs SpiderMonkey with no JIT tier. Each mode runs in a child process
//! (the switch is process-wide). Gate can go red: `MANUK_TEST_BREAK=1` makes the "jitless" child skip
//! `enable_jitless`, and the assertions below then fail.

#![cfg(feature = "spidermonkey")]

use manuk_js::{spidermonkey::SpiderMonkeyRuntime, JsRuntime};
use std::process::Command;

/// A hot numeric loop: the shape Baseline/Ion exist for. Only a JIT makes it fast.
const BENCH: &str = "var s=0; for (var i=0;i<10000000;i++){ s=(s+i*3)|0; } s";

#[test]
fn child() {
    let Ok(mode) = std::env::var("MANUK_TEST_MODE") else {
        return;
    };
    if mode == "jitless" && std::env::var("MANUK_TEST_BREAK").is_err() {
        manuk_js::hardening::enable_jitless();
    }
    let mut rt = SpiderMonkeyRuntime::new().unwrap();
    let tiers = rt.jit_tiers();
    let t = std::time::Instant::now();
    rt.eval(BENCH, "bench.js").unwrap();
    println!("RESULT tiers={:?} ms={}", tiers, t.elapsed().as_millis());
    std::process::exit(0);
}

fn run(mode: &str) -> (Vec<u32>, u128) {
    let out = Command::new(std::env::current_exe().unwrap())
        .args(["child", "--exact", "--nocapture", "--test-threads=1"])
        .env("MANUK_TEST_MODE", mode)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let line = text
        .lines()
        .find(|l| l.contains("RESULT tiers"))
        .unwrap_or_else(|| {
            panic!(
                "no RESULT: {text}\n{}",
                String::from_utf8_lossy(&out.stderr)
            )
        });
    let tiers = line
        .split("RESULT tiers=[")
        .nth(1)
        .unwrap()
        .split(']')
        .next()
        .unwrap()
        .split(", ")
        .map(|n| n.parse().unwrap())
        .collect();
    let ms = line.split(" ms=").nth(1).unwrap().trim().parse().unwrap();
    (tiers, ms)
}

#[test]
fn agent_mode_has_no_jit_and_a_jit_only_bench_is_slower() {
    let (jit_tiers, jit_ms) = run("jit");
    let (off_tiers, off_ms) = run("jitless");
    eprintln!("jit: tiers={jit_tiers:?} {jit_ms} ms | jitless: tiers={off_tiers:?} {off_ms} ms");
    assert!(
        jit_tiers.iter().any(|&t| t == 1),
        "default mode should have a JIT tier on: {jit_tiers:?}"
    );
    assert!(
        off_tiers.iter().all(|&t| t == 0),
        "agent mode left a JIT tier on: {off_tiers:?}"
    );
    assert!(
        off_ms > jit_ms * 3,
        "jitless bench ({off_ms} ms) not clearly slower than JIT ({jit_ms} ms)"
    );
}
