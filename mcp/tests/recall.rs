//! R2.3 gate over real stdio: observe stores the page; `recall` returns the last tree and its age,
//! also from a fresh server process (the store is on disk); an unknown site recalls nothing.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{json, Value};

const PAGE: &str = r#"<!doctype html><html><head><title>Recall</title></head><body>
<h1>Recall fixture</h1><button>Add to cart</button><a href="/x">Footer link</a></body></html>"#;

fn serve() -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let _ = s.read(&mut buf);
                let body = PAGE.as_bytes();
                let _ = write!(
                    s,
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = s.write_all(body);
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
    fn start_env(chrome: bool, mutate: Option<&str>, store: &std::path::Path) -> Client {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"));
        if let Some(m) = mutate {
            cmd.env("MANUK_CDP_MUTATE", m);
        }
        cmd.env("MANUK_PAGE_STORE", store);
        let _ = chrome;
        cmd.env(
            "MANUK_ACTLOG_DIR",
            std::env::temp_dir().join(format!("r23-log-{}-{chrome}", std::process::id())),
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
    /// (is_error, text, base64 image data if any)
    fn call(&mut self, name: &str, args: Value) -> (bool, String, Option<String>) {
        let r = self.rpc("tools/call", json!({"name": name, "arguments": args}));
        let res = &r["result"];
        let blocks = res["content"].as_array().cloned().unwrap_or_default();
        let text = blocks
            .iter()
            .filter_map(|b| b["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let img = blocks
            .iter()
            .find(|b| b["type"] == "image")
            .and_then(|b| b["data"].as_str().map(String::from));
        (res["isError"].as_bool().unwrap_or(false), text, img)
    }
    fn tool(&mut self, name: &str, args: Value) -> String {
        let (err, t, _) = self.call(name, args);
        assert!(!err, "{name} failed: {t}");
        t
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
        .unwrap_or_else(|| panic!("no {needle:?} in:\n{text}"));
    let s = line.find("[h").unwrap();
    let e = line[s..].find(']').unwrap() + s;
    line[s + 1..e].to_string()
}


#[test]
fn recall_returns_last_tree_and_age() {
    let url = serve();
    let store = std::env::temp_dir().join(format!("r23-store-{}.redb", std::process::id()));
    let _ = std::fs::remove_file(&store);
    {
        let mut c = Client::start_env(false, None, &store);
        let (err, t, _) = c.call("recall", json!({}));
        assert!(err || t.contains("nothing stored"), "{t}");
        c.tool("navigate", json!({"url": url}));
        c.tool("observe", json!({}));
    }
    // a fresh server process recalls what the first one stored
    let mut c = Client::start_env(false, None, &store);
    let host = url.trim_start_matches("http://").trim_end_matches('/').to_string();
    let r = c.tool("recall", json!({"site": host}));
    assert!(r.contains("Add to cart") && r.contains("Recall fixture"), "{r}");
    assert!(r.contains("stored ") && r.contains("s ago"), "{r}");
    assert!(c.tool("recall", json!({"site": "never.test"})).contains("nothing stored"));
    let _ = std::fs::remove_file(&store);
}
