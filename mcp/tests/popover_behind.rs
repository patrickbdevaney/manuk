//! RG.2.0.b.a gate: acting on a handle hidden behind an open popover says so and names the control that
//! closes it, instead of a bare "no longer on the page". `MANUK_POPOVER_MUTATE=1` restores the bare
//! refusal (the mutant).
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
            cmd.env("MANUK_POPOVER_MUTATE", "1");
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

fn run(mutate: bool) -> (bool, String) {
    run_on("popover_behind", "Price", "Filters", mutate)
}

fn run_on(fix: &str, bg: &str, open: &str, mutate: bool) -> (bool, String) {
    let mut c = Client::start(mutate);
    let url = format!(
        "file://{}/tests/fixtures/{fix}.html",
        env!("CARGO_MANIFEST_DIR")
    );
    let (err, nav) = c.tool("navigate", json!({"url": url}));
    assert!(!err, "{nav}");
    let (_, obs) = c.tool("observe", json!({"mode": "interactive"}));
    let price = handle_of(&obs, bg);
    let open = handle_of(&obs, open);
    let (err, t) = c.tool("act", json!({"handle": open, "action": "click"}));
    assert!(!err, "{t}");
    c.tool("act", json!({"handle": price, "action": "click"}))
}

#[test]
fn background_handle_names_the_popover_and_its_close_control() {
    if std::env::var("MANUK_POPOVER_MUTATE").is_ok() {
        return;
    }
    let (err, t) = run(false);
    assert!(err, "act on a hidden background handle should refuse: {t}");
    assert!(
        t.contains("STALE") && t.contains("Filters popover") && t.contains("Close filters"),
        "{t}"
    );
}

#[test]
fn popover_mutant_gives_the_bare_refusal() {
    let (err, t) = run(true);
    assert!(
        err && t.contains("STALE") && !t.contains("Filters popover"),
        "{t}"
    );
}

#[test]
fn roleless_popover_is_named_as_a_layer() {
    if std::env::var("MANUK_POPOVER_MUTATE").is_ok() {
        return;
    }
    let (err, t) = run_on("popover_roleless", "+3 more", "Event", false);
    assert!(err, "{t}");
    assert!(
        t.contains("STALE") && t.contains("hidden behind an open layer") && t.contains("\"Close\""),
        "{t}"
    );
}

#[test]
fn roleless_popover_mutant_is_bare() {
    let (err, t) = run_on("popover_roleless", "+3 more", "Event", true);
    assert!(
        err && t.contains("STALE") && !t.contains("open layer"),
        "{t}"
    );
}
