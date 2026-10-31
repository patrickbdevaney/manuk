//! W4.C4 gate: every act result lists what the act wrote (storage keys, non-safe requests), so the agent
//! can verify a state change instead of guessing. `MANUK_NO_RECEIPTS=1` is the baseline: no receipt (red).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{json, Value};

const PAGE: &str = r#"<!doctype html><html><head><title>Shop</title></head><body>
<h1>Shop</h1>
<button aria-label="Add Blue Mug" onclick="var c=JSON.parse(localStorage.getItem('cart')||'[]');c.push('Blue Mug');localStorage.setItem('cart',JSON.stringify(c));fetch('/api/cart',{method:'POST',body:'mug'});document.getElementById('o').textContent='added'">Add Blue Mug</button>
<button aria-label="Add Ghost" disabled>Add Ghost</button>
<button aria-label="Misses">Misses</button>
<h2 id="o">idle</h2></body></html>"#;

fn serve() -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let _ = s.read(&mut buf);
                let body = PAGE;
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
    fn start(chrome: bool, no_receipts: bool) -> Client {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"));
        if chrome {
            cmd.args(["--backend", "chrome"]);
        }
        if no_receipts {
            cmd.env("MANUK_NO_RECEIPTS", "1");
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

fn run(no_receipts: bool, labels: &[&str]) -> Vec<String> {
    let url = serve();
    let mut c = Client::start(false, no_receipts);
    let nav = c.tool("navigate", json!({"url": url}));
    labels
        .iter()
        .map(|l| {
            let h = handle_of(&nav, l);
            c.tool("act", json!({"handle": h, "action": "click"}))
        })
        .collect()
}

#[test]
fn an_add_to_cart_click_reports_the_key_it_wrote_and_the_post_it_sent() {
    let r = run(false, &["Add Blue Mug", "Add Blue Mug"]);
    assert!(
        r[0].contains("writes:") && r[0].contains("cart") && r[0].contains("Blue Mug"),
        "{}",
        r[0]
    );
    assert!(r[0].contains("sent POST /api/cart"), "{}", r[0]);
    assert!(
        r[0].contains("+ local"),
        "first add creates the key: {}",
        r[0]
    );
    assert!(
        r[1].contains("~ local") && r[1].contains("sent POST /api/cart"),
        "second add changes it: {}",
        r[1]
    );
}

#[test]
fn a_click_that_wrote_nothing_says_so() {
    let r = run(false, &["Misses"]);
    assert!(r[0].contains("writes: none"), "{}", r[0]);
}

#[test]
fn baseline_without_receipts_says_nothing_about_writes() {
    let r = run(true, &["Add Blue Mug"]);
    assert!(!r[0].contains("writes"), "{}", r[0]);
}
