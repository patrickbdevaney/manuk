//! W4.A6.a: an infinite-scroll page that loads its next batch only when scrollTop lands at
//! (document height - window height) must get that batch when the agent scrolls to the bottom.
//! Reproduces L12 (quotes.toscrape.com/scroll) without the network or a model.

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
        let mut child = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn manuk-mcp");
        let tx = child.stdin.take().unwrap();
        let rx = BufReader::new(child.stdout.take().unwrap());
        let mut c = Client { child, tx, rx, id: 0 };
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
            assert!(self.rx.read_line(&mut line).unwrap() > 0, "server closed stdout");
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
            .map(|c| c.iter().filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("\n"))
            .unwrap_or_default()
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

#[test]
fn scrolling_to_the_bottom_loads_the_next_batch() {
    let mut c = Client::start();
    let url = format!("file://{}/tests/fixtures/scroll_batch.html", env!("CARGO_MANIFEST_DIR"));
    c.tool("navigate", json!({"url": url}));
    let mut last = String::new();
    for _ in 0..4 {
        last = c.tool("scroll", json!({"dy": 100000}));
    }
    let text = c.tool("extract", json!({"format": "text"}));
    assert!(text.contains("Quote 11"), "second batch never loaded.\nscroll: {last}\n{text}");
}
