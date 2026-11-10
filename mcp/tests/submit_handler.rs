//! RG.2.0.a.a gate: a click on a submit button fires the form's `submit` event first, so a page handler
//! (React `onSubmit` via root delegation) that calls `preventDefault()` takes over; a form with no handler
//! still submits natively. Before the fix the agent skipped the event and always did the native GET
//! (fly-unified "Find flights" reloaded `/?tripType=...`).
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
        let mut args = args;
        if name == "act" {
            args["detail"] = json!(true);
        }
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

fn click(c: &mut Client, label: &str) -> String {
    let (err, obs) = c.tool("observe", json!({"mode": "interactive"}));
    assert!(!err, "{obs}");
    let h = handle_of(&obs, label);
    // A file:// page cannot load a `?query` URL, so a native submit surfaces as an error naming it.
    c.tool("act", json!({"handle": h, "action": "click"})).1
}

fn open() -> Client {
    let mut c = Client::start();
    let url = format!(
        "file://{}/tests/fixtures/submit_handler.html",
        env!("CARGO_MANIFEST_DIR")
    );
    let (err, nav) = c.tool("navigate", json!({"url": url}));
    assert!(!err, "{nav}");
    c
}

#[test]
fn a_page_submit_handler_takes_over_the_click() {
    let mut c = open();
    let t = click(&mut c, "Find flights");
    assert!(!t.contains("submitted form"), "native submit ran: {t}");
    let (_, out) = c.tool("extract", json!({"format": "text"}));
    assert!(out.contains("Please enter a valid origin."), "handler did not run: {out}");
}

#[test]
fn a_form_without_a_handler_still_submits_natively() {
    let mut c = open();
    let t = click(&mut c, "Plain search");
    assert!(t.contains("submit_handler.html?q=y"), "no native submit: {t}");
}
