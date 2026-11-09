//! RG.S1.b gate: a handle into a list survives a re-render that changes the label of the container it
//! sat in (a booking form retitled "for 2 people" -> "for 3 people"). The remount key is
//! role|name|container; when the container label moved, the lookup falls back to role|name and binds
//! only if exactly one node matches. `MANUK_REMOUNT_MUTATE=1` switches the rebind off (the mutant).

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
            cmd.env("MANUK_REMOUNT_MUTATE", "1");
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

fn run(mutate: bool) -> (bool, String) {
    let mut c = Client::start(mutate);
    let url = format!(
        "file://{}/tests/fixtures/rebind_ctx.html",
        env!("CARGO_MANIFEST_DIR")
    );
    let (err, nav) = c.tool("navigate", json!({"url": url}));
    assert!(!err, "{nav}");
    let slot = handle_of(&nav, "Book a slot at 2:00 PM");
    let more = handle_of(&nav, "3 people");
    let (err, t) = c.tool("act", json!({"handle": more, "action": "click"}));
    assert!(!err, "{t}");
    c.tool("act", json!({"handle": slot, "action": "click"}))
}

#[test]
fn a_handle_follows_its_element_when_the_container_label_changed() {
    if std::env::var("MANUK_REMOUNT_MUTATE").is_ok() {
        return;
    }
    let (err, t) = run(false);
    assert!(!err, "{t}");
    assert!(t.contains("booked 2:00 PM"), "wrong or no click: {t}");
    assert!(t.contains("rebound"), "no rebind evidence: {t}");
}

#[test]
fn rebind_mutant_goes_stale() {
    let (err, t) = run(true);
    assert!(
        err && t.contains("STALE"),
        "mutant should be stale, got err={err}: {t}"
    );
}
