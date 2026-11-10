//! RG.2.S1 gate: a `div` the page styles `cursor: pointer` and handles by root-delegated listener (React's
//! "Compose" button on gomail) is found, listed as clickable and clickable. `MANUK_CURSOR_MUTATE=1` drops the
//! cursor signal (the mutant).
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
    fn start(mutate: bool) -> Client {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"));
        if mutate {
            cmd.env("MANUK_CURSOR_MUTATE", "1");
        }
        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn manuk-mcp");
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

fn handle_of(text: &str, needle: &str) -> String {
    let line = text
        .lines()
        .find(|l| l.contains(needle))
        .unwrap_or_else(|| panic!("no line with {needle:?} in:\n{text}"));
    let s = line.find("[h").unwrap();
    let e = line[s..].find(']').unwrap() + s;
    line[s + 1..e].to_string()
}

fn run(mutate: bool) -> (String, (bool, String), String) {
    let mut c = Client::start(mutate);
    let url = format!(
        "file://{}/tests/fixtures/cursor_clickable.html",
        env!("CARGO_MANIFEST_DIR")
    );
    let (err, nav) = c.tool("navigate", json!({"url": url}));
    assert!(!err, "{nav}");
    let (_, obs) = c.tool("observe", json!({"mode": "interactive"}));
    let found = c.tool("find", json!({"query": "Compose"}));
    let click = if found.1.contains("[h") {
        let h = handle_of(&found.1, "Compose");
        c.tool("act", json!({"handle": h, "action": "click"}))
    } else {
        (true, String::new())
    };
    let (_, after) = c.tool("observe", json!({"mode": "content"}));
    let _ = click;
    (obs, found, after)
}

#[test]
fn cursor_pointer_div_is_found_and_clickable() {
    if std::env::var("MANUK_CURSOR_MUTATE").is_ok() {
        return;
    }
    let (obs, (_, found), after) = run(false);
    assert!(
        obs.contains("Compose") && obs.contains("[clickable]"),
        "{obs}"
    );
    assert!(
        found.contains("Compose") && !found.contains("no match"),
        "{found}"
    );
    assert!(
        after.contains("composing"),
        "click did not reach the page: {after}"
    );
}

#[test]
fn cursor_mutant_loses_the_control() {
    let (obs, (_, found), after) = run(true);
    assert!(!obs.contains("Compose"), "{obs}");
    assert!(found.contains("no match"), "{found}");
    assert!(after.contains("idle"), "{after}");
}
