//! W6.C1 gate: the OCCLUDED escalation ladder on the Chrome backend. Each rung (wait, escape, dismiss) has a
//! fixture where it alone clears the cover, and a twin run with `MANUK_LADDER_MUTATE=<rung>` that must be refused.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{json, Value};

struct Client {
    child: Child,
    tx: Option<ChildStdin>,
    rx: BufReader<ChildStdout>,
    id: u64,
}

impl Client {
    fn start(mutate: Option<&str>) -> Client {
        Client::start_on("chrome", mutate)
    }
    /// `backend` = `chrome` (CDP) or `manuk` (the in-process engine, no flag).
    fn start_on(backend: &str, mutate: Option<&str>) -> Client {
        Client::start_env(backend, mutate, &[])
    }
    /// [`Self::start_on`] with extra environment for the child (a test-only switch must not leak into the
    /// parent test process, which runs tests in parallel).
    fn start_env(backend: &str, mutate: Option<&str>, env: &[(&str, &str)]) -> Client {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"));
        if backend == "chrome" {
            cmd.args(["--backend", "chrome"]);
        }
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        if let Some(m) = mutate {
            cmd.env("MANUK_CDP_MUTATE", m);
            // The session has its own occlusion check and handle re-mount above the backend; switch those
            // off too so the mutant isolates the backend discipline it names.
            match m {
                "no-revalidate" => cmd.env("MANUK_LANDING_MUTATE", "1"),
                "no-remount" => cmd.env("MANUK_REMOUNT_MUTATE", "1"),
                _ => &mut cmd,
            };
        }
        for (k, v) in env {
            cmd.env(k, v);
        }
        let mut child = cmd.spawn().expect("spawn manuk-mcp --backend chrome");
        let tx = child.stdin.take().unwrap();
        let rx = BufReader::new(child.stdout.take().unwrap());
        let mut c = Client {
            child,
            tx: Some(tx),
            rx,
            id: 0,
        };
        c.rpc("initialize", json!({"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "t", "version": "0"}}));
        c.send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        c
    }
    fn send(&mut self, v: Value) {
        let tx = self.tx.as_mut().unwrap();
        writeln!(tx, "{v}").unwrap();
        tx.flush().unwrap();
    }
    fn rpc(&mut self, method: &str, params: Value) -> Value {
        self.id += 1;
        let id = self.id;
        self.send(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        loop {
            let mut line = String::new();
            assert!(
                self.rx.read_line(&mut line).unwrap() > 0,
                "server closed stdout"
            );
            let v: Value = serde_json::from_str(&line).unwrap();
            if v["id"] == json!(id) {
                return v;
            }
        }
    }
    fn tool(&mut self, name: &str, args: Value) -> (bool, String) {
        let r = self.rpc("tools/call", json!({"name": name, "arguments": args}));
        let res = &r["result"];
        let text = res["content"]
            .as_array()
            .map(|c| {
                c.iter()
                    .filter_map(|b| b["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();
        (res["isError"].as_bool().unwrap_or(false), text)
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

fn fixture(name: &str) -> String {
    format!(
        "file://{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    )
}

fn handle_of(text: &str, needle: &str) -> String {
    let line = text
        .lines()
        .find(|l| l.contains(needle))
        .unwrap_or_else(|| panic!("no line with {needle:?} in:\n{text}"));
    let start = line.find("[h").unwrap();
    let end = line[start..].find(']').unwrap() + start;
    line[start + 1..end].to_string()
}

fn go(page: &str, env: &[(&str, &str)]) -> (bool, String, String) {
    let mut c = Client::start_env("chrome", None, env);
    let (err, nav) = c.tool("navigate", json!({"url": fixture(page)}));
    assert!(!err, "{nav}");
    let go = handle_of(&nav, "button \"Go\"");
    if page == "ladder_wait.html" {
        let arm = handle_of(&nav, "button \"Arm\"");
        c.tool("act", json!({"handle": arm, "action": "click"}));
    }
    let (err, txt) = c.tool("act", json!({"handle": go, "action": "click"}));
    let (_, obs) = c.tool("observe", json!({"mode": "full"}));
    (err, txt, obs)
}

#[test]
fn wait_rung_clears_a_transient_cover() {
    let (err, txt, obs) = go("ladder_wait.html", &[]);
    assert!(!err && obs.contains("went"), "err={err}: {txt}\n{obs}");
}
#[test]
fn wait_rung_mutant_is_refused() {
    let (err, txt, obs) = go("ladder_wait.html", &[("MANUK_LADDER_MUTATE", "wait")]);
    assert!(err && txt.contains("OCCLUDED") && !obs.contains("went"), "err={err}: {txt}");
}
#[test]
fn escape_rung_closes_a_covering_menu() {
    let (err, txt, obs) = go("ladder_escape.html", &[]);
    assert!(!err && obs.contains("went"), "err={err}: {txt}\n{obs}");
}
#[test]
fn escape_rung_mutant_is_refused() {
    let (err, txt, obs) = go("ladder_escape.html", &[("MANUK_LADDER_MUTATE", "escape")]);
    assert!(err && txt.contains("OCCLUDED") && !obs.contains("went"), "err={err}: {txt}");
}
#[test]
fn dismiss_rung_rejects_a_consent_layer_never_accepts() {
    let (err, txt, obs) = go("ladder_dismiss.html", &[]);
    assert!(!err && obs.contains("went") && !obs.contains("ACCEPTED"), "err={err}: {txt}\n{obs}");
}
#[test]
fn dismiss_rung_mutant_is_refused() {
    let (err, txt, obs) = go("ladder_dismiss.html", &[("MANUK_LADDER_MUTATE", "dismiss")]);
    assert!(err && txt.contains("OCCLUDED") && !obs.contains("went"), "err={err}: {txt}");
}
#[test]
fn accept_only_layer_is_a_typed_refusal_naming_the_rungs() {
    let (err, txt, obs) = go("ladder_accept_only.html", &[]);
    assert!(
        err && txt.contains("OCCLUDED") && txt.contains("tried: scroll, wait 2s")
            && txt.contains("accept is never clicked") && !obs.contains("ACCEPTED") && !obs.contains("went"),
        "err={err}: {txt}\n{obs}"
    );
}
