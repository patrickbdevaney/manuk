//! W5.G1.a.a gate (chrome_ver=145.0.7632.116): on the CDP backend a form container whose only text is
//! withheld (a visually-hidden live region) is not dropped from the tree with its visible controls: the
//! gomail compose dialog lost To, Subject and the body editor that way (gomail-3 0/3 on cdp-layer).
//! `MANUK_CDP_MUTATE=drop-form-containers` (the old rule) turns it RED: the fields are missing.

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
    fn start(mutate: Option<&str>) -> Client {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"));
        cmd.args(["--backend", "chrome"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        if let Some(m) = mutate {
            cmd.env("MANUK_CDP_MUTATE", m);
        }
        let mut child = cmd.spawn().expect("spawn manuk-mcp --backend chrome");
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
    fn tool(&mut self, name: &str, args: Value) -> String {
        let r = self.rpc("tools/call", json!({"name": name, "arguments": args}));
        r["result"]["content"]
            .as_array()
            .map(|c| {
                c.iter()
                    .filter_map(|b| b["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default()
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

fn observe(mutate: Option<&str>) -> String {
    let mut c = Client::start(mutate);
    let url = format!(
        "file://{}/tests/fixtures/cdp_hidden_form.html",
        env!("CARGO_MANIFEST_DIR")
    );
    c.tool("navigate", json!({"url": url}));
    c.tool("observe", json!({"mode": "full"}))
}

#[test]
fn a_form_beside_a_hidden_live_region_stays_in_the_tree() {
    let obs = observe(None);
    for want in ["Email input", "textbox \"Subject\"", "[h3] textbox"] {
        assert!(obs.contains(want), "{want:?} missing:\n{obs}");
    }
    assert!(
        !obs.contains("Message saved"),
        "the hidden text must stay withheld:\n{obs}"
    );
}

#[test]
fn the_old_rule_drops_the_form() {
    let obs = observe(Some("drop-form-containers"));
    assert!(
        !obs.contains("textbox \"Subject\""),
        "mutant should lose the Subject field:\n{obs}"
    );
}
