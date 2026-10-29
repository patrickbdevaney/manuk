//! R1.6 / JIT.BASE gate: agent mode runs SpiderMonkey with the baseline tiers only (Ion, native regexp,
//! wasm JIT off); `--jitless` has none. Each mode runs in a child process (the switch is
//! process-wide). Gate can go red: `MANUK_TEST_BREAK=1` makes the "baseline" and "jitless" children
//! skip their switch (Ion comes back on), and the assertions below then fail.

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
    if std::env::var("MANUK_TEST_BREAK").is_err() {
        match mode.as_str() {
            "jitless" => manuk_js::hardening::enable_jitless(),
            "baseline" => manuk_js::hardening::enable_baseline_only(),
            _ => {}
        }
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
fn baseline_is_on_ion_is_off_and_jitless_is_slow() {
    let (full_tiers, full_ms) = run("jit");
    let (base_tiers, base_ms) = run("baseline");
    let (off_tiers, off_ms) = run("jitless");
    eprintln!(
        "full: {full_tiers:?} {full_ms} ms | baseline: {base_tiers:?} {base_ms} ms | jitless: {off_tiers:?} {off_ms} ms"
    );
    assert!(full_tiers.iter().any(|&t| t == 1), "{full_tiers:?}");
    // TIERS order: ion, baseline, baseline-interp, native regexp, wasm baseline, wasm optimizing.
    assert_eq!(
        base_tiers,
        vec![0, 1, 1, 0, 0, 0],
        "agent mode must be baseline-only (Ion off)"
    );
    assert!(
        off_tiers.iter().all(|&t| t == 0),
        "--jitless left a JIT tier on: {off_tiers:?}"
    );
    assert!(
        off_ms >= base_ms * 5,
        "baseline ({base_ms} ms) not >=5x faster than jitless ({off_ms} ms)"
    );
}
