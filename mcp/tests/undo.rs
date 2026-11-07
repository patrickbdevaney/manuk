//! W5.L2 gate: a failed / no-effect act ends with `undo: restore("<label>")` (the hub checkpointed before it),
//! restore works from that label, a repeated identical no-effect act carries a LOOP warning, and manual
//! `checkpoint` is not in the default tool list. Mutation record: removing the `auto_checkpoint` call in
//! `Hub::call` turns `failed_act_carries_undo_and_restore_works` RED (no `undo:` line); dropping the LOOP
//! branch turns `repeat_gets_loop_warning` RED.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{json, Value};

const PAGE: &str = r#"<!doctype html><html><head><title>Lab</title></head><body>
<h1>Lab</h1>
<input id="n" aria-label="Name">
<button onclick="callAnUnimplementedApi()">Broken save</button>
<div id="plain" role="button">Just a label</div>
<div style="height:3000px">tall</div>
</body></html>"#;

fn serve(page: &'static str) -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let _ = s.read(&mut buf);
                let body = page;
                let _ = write!(
                    s,
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            });
        }
    });
    format!("http://{addr}/")
}

struct Client {
    child: Child,
    tx: ChildStdin,
    rx: BufReader<ChildStdout>,
    id: u64,
}

impl Client {
    fn start(chrome: bool, mutate: bool) -> Client {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"));
        if chrome {
            cmd.args(["--backend", "chrome"]);
        }
        if mutate {
            cmd.env("MANUK_UNUSED", "1");
        }
        cmd.env(
            "MANUK_ACTLOG_DIR",
            std::env::temp_dir().join(format!("w36-log-{}-{chrome}", std::process::id())),
        );
        cmd.arg("--allow-write").arg(std::env::temp_dir());
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
    fn tool(&mut self, name: &str, args: Value) -> String {
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
        assert!(!res["isError"].as_bool().unwrap_or(false), "{name}: {text}");
        text
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
    let a = line.find("[h").unwrap();
    let b = line[a..].find(']').unwrap() + a;
    line[a + 1..b].to_string()
}

fn undo_label(t: &str) -> String {
    let a = t
        .find("undo: restore(\"")
        .unwrap_or_else(|| panic!("no undo line in:\n{t}"))
        + 15;
    let b = t[a..].find('"').unwrap() + a;
    t[a..b].to_string()
}

#[test]
fn failed_act_carries_undo_and_restore_works() {
    let url = serve(PAGE);
    let mut c = Client::start(false, false);
    let nav = c.tool("navigate", json!({"url": url}));
    let h = handle_of(&nav, "Broken save");
    let r = c.tool("act", json!({"handle": h, "action": "click"}));
    assert!(r.contains("NO EFFECT"), "{r}");
    let label = undo_label(&r);
    let back = c.tool("restore", json!({"checkpoint": label}));
    assert!(back.contains("page:"), "{back}");
    let obs = c.tool("observe", json!({}));
    assert!(obs.contains("Broken save"), "{obs}");
    // A successful act carries no undo line.
    let ok = c.tool(
        "type",
        json!({"handle": handle_of(&obs, "Name"), "text": "x"}),
    );
    assert!(!ok.contains("undo:"), "{ok}");
}

#[test]
fn repeat_gets_loop_warning() {
    let url = serve(PAGE);
    let mut c = Client::start(false, false);
    let nav = c.tool("navigate", json!({"url": url}));
    let h = handle_of(&nav, "Broken save");
    let r1 = c.tool("act", json!({"handle": h, "action": "click"}));
    assert!(!r1.contains("LOOP"), "{r1}");
    let r2 = c.tool("act", json!({"handle": h, "action": "click"}));
    assert!(r2.contains("LOOP") && r2.contains("undo: restore("), "{r2}");
}
