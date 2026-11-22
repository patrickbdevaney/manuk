//! W6.D1.a gate: with no backend flag manuk-mcp serves Chrome (cdp-layer); `--backend engine` serves the
//! in-process engine; `--backend chrome` is still accepted. The session reports it as
//! `serverInfo.title = "backend=<name>"` on `initialize`. Reverting the default in main.rs turns this red.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

fn reported_backend(args: &[&str]) -> String {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"));
    // The repo's .cargo/config.toml sets MANUK_BACKEND=engine for the other suites; this test wants the real default.
    cmd.env_remove("MANUK_BACKEND").args(args);
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn manuk-mcp");
    let mut tx = child.stdin.take().unwrap();
    let mut rx = BufReader::new(child.stdout.take().unwrap());
    writeln!(
        tx,
        r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":"2025-06-18","capabilities":{{}},"clientInfo":{{"name":"t","version":"0"}}}}}}"#
    )
    .unwrap();
    tx.flush().unwrap();
    let mut line = String::new();
    rx.read_line(&mut line).unwrap();
    drop(tx);
    let _ = child.wait();
    let v: serde_json::Value = serde_json::from_str(&line).expect("initialize reply");
    v["result"]["serverInfo"]["title"].as_str().unwrap_or("").to_string()
}

#[test]
fn no_flag_serves_chrome() {
    assert_eq!(reported_backend(&[]), "backend=chrome");
}

#[test]
fn explicit_chrome_still_accepted() {
    assert_eq!(reported_backend(&["--backend", "chrome"]), "backend=chrome");
}

#[test]
fn engine_is_opt_in() {
    assert_eq!(reported_backend(&["--backend", "engine"]), "backend=engine");
}
