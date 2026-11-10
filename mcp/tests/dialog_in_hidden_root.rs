//! RG.2.S1.a gate: a dialog opened INSIDE a root that the page marked `aria-hidden="true"` (MUI `Dialog`
//! with `disablePortal`, gomail's New Message window) keeps its controls in observe/find while focus
//! is inside it — Chrome ignores `aria-hidden` on an element that holds focus. With focus left on the
//! page the attribute is honoured and the tree is empty (the control, which also proves the gate can go
//! red).
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
    fn start() -> Client {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"));
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

fn open(fixture: &str) -> String {
    let mut c = Client::start();
    let url = format!(
        "file://{}/tests/fixtures/{fixture}",
        env!("CARGO_MANIFEST_DIR")
    );
    let (err, nav) = c.tool("navigate", json!({"url": url}));
    assert!(!err, "{nav}");
    let (_, found) = c.tool("find", json!({"query": "Compose"}));
    let h = found
        .lines()
        .find(|l| l.contains("Compose"))
        .and_then(|l| l.find("[h").map(|s| &l[s + 1..]))
        .and_then(|l| l.find(']').map(|e| l[..e].to_string()))
        .unwrap_or_else(|| panic!("no Compose handle: {found}"));
    let (err, click) = c.tool("act", json!({"handle": h, "action": "click"}));
    assert!(!err, "{click}");
    let (_, obs) = c.tool("observe", json!({"mode": "interactive"}));
    obs
}

#[test]
fn focused_dialog_inside_aria_hidden_root_is_observed() {
    let obs = open("dialog_in_hidden_root.html");
    for want in [
        "Minimize",
        "Close",
        "Recipients",
        "Subject",
        "Message body",
        "Send",
    ] {
        assert!(obs.contains(want), "{want} missing from:\n{obs}");
    }
}

#[test]
fn aria_hidden_root_without_focus_inside_stays_hidden() {
    let obs = open("dialog_in_hidden_root_nofocus.html");
    assert!(
        !obs.contains("Recipients") && !obs.contains("Send"),
        "{obs}"
    );
}
