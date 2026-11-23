//! W6.D1.b: the window's DEFAULT human view is Chrome. `manuk view-frame` is the shell's frame path
//! run headless: with the default config the frame must come from the Chrome backend (a CDP target
//! id) and show the fixture; `--engine-view` must NOT (that is the red case).

use std::io::Write;
use std::process::Command;

const FIXTURE: &str = "<html><body style='margin:0'><div style='background:#12ab34;height:200px'>\
<h1>manuk-human-view-fixture-7731</h1></div></body></html>";

fn run(extra: &[&str], envs: &[(&str, &str)]) -> String {
    let dir = std::env::temp_dir().join(format!("w6d1b-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("fixture.html");
    std::fs::File::create(&f)
        .unwrap()
        .write_all(FIXTURE.as_bytes())
        .unwrap();
    let mut c = Command::new(env!("CARGO_BIN_EXE_manuk"));
    c.arg("view-frame")
        .arg(format!("file://{}", f.display()))
        .args(extra);
    for (k, v) in envs {
        c.env(k, v);
    }
    c.env_remove("MANUK_HUMAN_VIEW");
    for (k, v) in envs {
        c.env(k, v);
    }
    let o = c.output().expect("spawn manuk");
    assert!(
        o.status.success(),
        "view-frame failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    String::from_utf8_lossy(&o.stdout).to_string()
}

fn assert_chrome_frame(out: &str) {
    let t = out
        .lines()
        .find_map(|l| l.strip_prefix("source=chrome target="))
        .unwrap_or_else(|| panic!("frame did not come from the Chrome backend:\n{out}"));
    assert!(t.trim().len() >= 16, "CDP target id missing/short: {t:?}");
    assert!(
        out.contains("manuk-human-view-fixture-7731"),
        "fixture text not in the frame's page:\n{out}"
    );
    assert!(
        out.contains("pixel0=[18, 171, 52, 255]"),
        "Chrome frame lacks the fixture's green:\n{out}"
    );
}

#[test]
fn default_config_shows_the_chrome_backend() {
    assert_chrome_frame(&run(&[], &[]));
}

#[test]
fn forcing_the_engine_view_is_not_a_chrome_frame() {
    let out = run(&["--engine-view"], &[]);
    assert!(out.contains("source=engine"), "{out}");
    assert!(!out.contains("target="), "{out}");
}

#[test]
fn engine_view_turns_the_chrome_assertion_red() {
    let out = run(&[], &[("MANUK_HUMAN_VIEW", "engine")]);
    let r = std::panic::catch_unwind(|| assert_chrome_frame(&out));
    assert!(
        r.is_err(),
        "the Chrome assertion must go red on an engine frame:\n{out}"
    );
}
