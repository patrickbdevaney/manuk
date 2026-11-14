//! R1.4.a.c.b gate: `extract(include_hidden=true)` emits text in DOM order. A page-top hidden nav that the
//! layout tree orders after the footer (out-of-flow boxes) must still print before it.
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

#[test]
fn include_hidden_extract_is_in_dom_order() {
    let mut c = Client::start();
    let url = format!(
        "file://{}/tests/fixtures/dom_order.html",
        env!("CARGO_MANIFEST_DIR")
    );
    let (err, nav) = c.tool("navigate", json!({"url": url}));
    assert!(!err, "{nav}");
    let (err, t) = c.tool("extract", json!({"format": "text", "include_hidden": true}));
    assert!(!err, "{t}");
    let pos = |s: &str| {
        t.find(s)
            .unwrap_or_else(|| panic!("{s:?} missing in {t:?}"))
    };
    let order = [
        pos("Jump to content"),
        pos("Main menu"),
        pos("Sidebar mobile"),
        pos("Body paragraph"),
        pos("Cookie statement"),
        pos("Mobile view"),
    ];
    assert!(
        order.windows(2).all(|w| w[0] < w[1]),
        "not DOM order {order:?}: {t:?}"
    );
}
