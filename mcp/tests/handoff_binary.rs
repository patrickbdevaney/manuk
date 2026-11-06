//! R3.3.a gate: the SHIPPED `manuk-mcp` binary has a human surface when `MANUK_HANDOFF_SCRIPT=<file>` names
//! a scripted human (test-only: its text is JS run in the live page). A `handoff` call over JSON-RPC then
//! returns the effect diff; without the env var it still errors rather than pretending.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{json, Value};

struct Client {
    child: Child,
    tx: ChildStdin,
    rx: BufReader<ChildStdout>,
    id: u64,
}

impl Client {
    fn start(script: Option<&std::path::Path>) -> Client {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"));
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        match script {
            Some(p) => cmd.env("MANUK_HANDOFF_SCRIPT", p),
            None => cmd.env_remove("MANUK_HANDOFF_SCRIPT"),
        };
        let mut child = cmd.spawn().expect("spawn manuk-mcp");
        let tx = child.stdin.take().unwrap();
        let rx = BufReader::new(child.stdout.take().unwrap());
        let mut c = Client {
            child,
            tx,
            rx,
            id: 0,
        };
        c.rpc("initialize", json!({"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "t", "version": "0"}}));
        c.send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        c
    }
    fn send(&mut self, v: Value) {
        writeln!(self.tx, "{v}").unwrap();
        self.tx.flush().unwrap();
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
    /// (is_error, text)
    fn tool(&mut self, name: &str, args: Value) -> (bool, String) {
        let r = self.rpc("tools/call", json!({"name": name, "arguments": args}));
        let text = r["result"]["content"]
            .as_array()
            .map(|c| {
                c.iter()
                    .filter_map(|b| b["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();
        (r["result"]["isError"] == json!(true), text)
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

fn url() -> String {
    format!(
        "file://{}/tests/fixtures/handoff_gate.html",
        env!("CARGO_MANIFEST_DIR")
    )
}

fn script(name: &str, body: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("manuk-handoff-{}-{name}", std::process::id()));
    std::fs::write(&p, body).unwrap();
    p
}

#[test]
fn handoff_works_over_stdio_with_a_scripted_human() {
    let p = script("login.js", "signin()");
    let mut c = Client::start(Some(&p));
    c.tool("navigate", json!({"url": url()}));
    let (err, r) = c.tool("handoff", json!({"reason": "log in", "timeout_ms": 5000}));
    assert!(!err, "{r}");
    assert!(r.contains("resumed"), "{r}");
    assert!(
        r.contains("Welcome back, Pat"),
        "diff must carry the human's change: {r}"
    );
    let (_, o) = c.tool("observe", json!({}));
    assert!(o.contains("Account settings"), "{o}");

    let p = script("timeout.js", "//timeout\nsignin()");
    let mut c = Client::start(Some(&p));
    c.tool("navigate", json!({"url": url()}));
    let (_, r) = c.tool("handoff", json!({"reason": "x", "timeout_ms": 50}));
    assert!(r.contains("TIMED OUT after 50 ms"), "{r}");
}

#[test]
fn without_the_env_handoff_still_errors() {
    let mut c = Client::start(None);
    c.tool("navigate", json!({"url": url()}));
    let (err, r) = c.tool("handoff", json!({"reason": "x"}));
    assert!(err && r.contains("no human surface"), "{r}");
}
