//! R1.6.a gate (manuk-bidi): after confinement the agent process cannot write outside its cache dir, cannot exec,
//! and cannot read `$HOME`. The control run (`--no-confine`) can do all three, so the assertions can
//! go red.

use std::process::Command;

fn run(extra: &[&str]) -> String {
    let base = std::env::temp_dir().join(format!(
        "manuk-confine-bidi-{}-{}",
        std::process::id(),
        extra.len()
    ));
    let inside = base.join("cache");
    std::fs::create_dir_all(&inside).unwrap();
    // Outside: under the test's own HOME-like dir, not in /tmp-granted cache.
    let home = std::env::var("HOME").unwrap_or_default();
    let outside = format!("{home}/.manuk-confine-bidi-probe-{}", std::process::id());
    let out = Command::new(env!("CARGO_BIN_EXE_manuk-bidi"))
        .arg("--confine-selftest")
        .arg(&outside)
        .arg(&inside)
        .args(extra)
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&outside);
    let _ = std::fs::remove_dir_all(&base);
    String::from_utf8_lossy(&out.stdout).to_string()
}

#[test]
fn confined_process_is_read_only_outside_cache_and_cannot_exec() {
    let c = run(&[]);
    assert!(
        c.contains("confine=Enforced"),
        "landlock not enforced here: {c}"
    );
    assert!(c.contains("write_outside=false"), "{c}");
    assert!(c.contains("write_inside=true"), "{c}");
    assert!(c.contains("exec=false"), "{c}");
    assert!(c.contains("read_usr=true"), "{c}");
    assert!(c.contains("read_home=false"), "{c}");
    assert!(c.contains("jit=baseline"), "{c}");
}

#[test]
fn control_without_confinement_can_do_all_of_it() {
    let c = run(&["--no-confine"]);
    assert!(c.contains("write_outside=true"), "{c}");
    assert!(c.contains("exec=true"), "{c}");
    assert!(c.contains("read_home=true"), "{c}");
    assert!(c.contains("jit=full"), "{c}");
}

#[test]
fn jitless_flag_selects_no_jit() {
    let c = run(&["--jitless"]);
    assert!(c.contains("jit=none"), "{c}");
}
