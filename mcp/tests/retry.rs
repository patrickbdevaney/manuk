//! W4.C2 gate: the layer retries a click that verifiably did nothing (keyboard, then the target's own
//! interactive descendant, undoing an attempt that lands on an error page) so the model never has to ask.
//! `MANUK_NO_RETRY=1` is the baseline: the same fixtures must then come back "no change" (red).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{json, Value};

const PAGE: &str = r#"<!doctype html><html><head><title>Retry</title></head><body>
<h1>Retry fixture</h1>
<div role="button" tabindex="0" aria-label="Keyboard only" onkeydown="if(event.key==='Enter')document.getElementById('o').textContent='KEY-OK'">Keyboard only</div>
<div role="button" tabindex="0" aria-label="Card" style="width:400px;padding:20px"><a role="link" style="float:left" onclick="document.getElementById('o').textContent='INNER-OK'">Open</a></div>
<div role="button" tabindex="0" aria-label="Broken" onclick="location='/gone'" onkeydown="if(event.key==='Enter')document.getElementById('o').textContent='ERR-RECOVERED'">Broken</div>
<button aria-label="Dud">Dud</button>
<h2 id="o">idle</h2></body></html>"#;

fn serve() -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let (status, body) = if req.starts_with("GET /gone") {
                    ("404 Not Found", "<html><body><h1>404 Not Found</h1></body></html>")
                } else {
                    ("200 OK", PAGE)
                };
                let _ = write!(
                    s,
                    "HTTP/1.1 {status}\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
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
    fn start(chrome: bool, no_retry: bool) -> Client {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"));
        if chrome {
            cmd.args(["--backend", "chrome"]);
        }
        if no_retry {
            cmd.env("MANUK_NO_RETRY", "1");
        }
        cmd.env(
            "MANUK_ACTLOG_DIR",
            std::env::temp_dir().join(format!("w312-log-{}-{chrome}", std::process::id())),
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

fn act(no_retry: bool, label: &str) -> String {
    let url = serve();
    let mut c = Client::start(false, no_retry);
    let nav = c.tool("navigate", json!({"url": url}));
    let h = handle_of(&nav, label);
    c.tool("act", json!({"handle": h, "action": "click"}))
}

#[test]
fn a_dead_click_is_recovered_by_the_keyboard_in_one_call() {
    let r = act(false, "Keyboard only");
    assert!(r.contains("KEY-OK"), "{r}");
    assert!(r.contains("recovered by Enter"), "{r}");
}

#[test]
fn a_dead_click_is_recovered_by_the_interactive_descendant() {
    let r = act(false, "Card");
    assert!(r.contains("INNER-OK"), "{r}");
    assert!(r.contains("recovered by descendant"), "{r}");
}

#[test]
fn an_error_page_is_undone_and_the_next_method_tried() {
    let r = act(false, "Broken");
    assert!(r.contains("ERR-RECOVERED"), "{r}");
    assert!(!r.contains("404 Not Found"), "{r}");
}

#[test]
fn a_click_nothing_can_fix_still_reports_no_change() {
    let r = act(false, "Dud");
    assert!(r.contains("no change"), "{r}");
    assert!(!r.contains("recovered"), "{r}");
}

#[test]
fn baseline_without_the_layer_retry_is_a_dead_click() {
    for l in ["Keyboard only", "Card"] {
        let r = act(true, l);
        assert!(r.contains("no change") && !r.contains("OK"), "{l}: {r}");
    }
}
