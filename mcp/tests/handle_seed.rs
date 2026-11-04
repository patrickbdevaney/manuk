//! R2.3.a gate over real stdio: (1) a node keeps its `hN` across visits and fresh server processes,
//! seeded from the page store; (2) an observation after an act records a transition edge in the store.
//! Mutation: `MANUK_PAGESTORE_SEED_MUTATE=1` skips the seeding and the first test must go red.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use serde_json::{json, Value};

/// First visit to "/" shows a skip link above the button; later visits do not. Without seeding, the
/// button would be numbered differently on the second visit.
const PAGE_FIRST: &str = r#"<!doctype html><html><head><title>Seed</title></head><body>
<h1>Seed fixture</h1><a href="/skip">Skip intro</a><button>Add to cart</button><a href="/x">Footer link</a></body></html>"#;
const PAGE: &str = r#"<!doctype html><html><head><title>Seed</title></head><body>
<h1>Seed fixture</h1><button>Add to cart</button><a href="/x">Footer link</a></body></html>"#;
const OTHER: &str = r#"<!doctype html><html><head><title>Other</title></head><body>
<h1>Other page</h1><p>Somewhere else entirely.</p></body></html>"#;

/// Serves "/" (first visit variant, then the plain one; plain every time when `skip_first` is false),
/// and "/x" as another document.
fn serve(skip_first: bool) -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    let root_hits = Arc::new(AtomicUsize::new(0));
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            let hits = root_hits.clone();
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let body = if req.starts_with("GET /x ") {
                    OTHER
                } else if req.starts_with("GET / ") {
                    if skip_first && hits.fetch_add(1, Ordering::SeqCst) == 0 { PAGE_FIRST } else { PAGE }
                } else {
                    ""
                };
                let _ = write!(
                    s,
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = s.write_all(body.as_bytes());
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
    fn start(store: &std::path::Path, tag: &str) -> Client {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"));
        cmd.env("MANUK_PAGE_STORE", store);
        cmd.env(
            "MANUK_ACTLOG_DIR",
            std::env::temp_dir().join(format!("r23a-log-{}-{tag}", std::process::id())),
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
    fn call(&mut self, name: &str, args: Value) -> (bool, String) {
        let r = self.rpc("tools/call", json!({"name": name, "arguments": args}));
        let res = &r["result"];
        let text = res["content"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|b| b["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n");
        (res["isError"].as_bool().unwrap_or(false), text)
    }
    fn tool(&mut self, name: &str, args: Value) -> String {
        let (err, t) = self.call(name, args);
        assert!(!err, "{name} failed: {t}");
        t
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The `hN` printed on the line that holds `needle`.
fn handle_of(text: &str, needle: &str) -> String {
    let line = text
        .lines()
        .find(|l| l.contains(needle))
        .unwrap_or_else(|| panic!("no {needle:?} in:\n{text}"));
    let s = line.find("[h").unwrap();
    let e = line[s..].find(']').unwrap() + s;
    line[s + 1..e].to_string()
}

fn host_of(url: &str) -> String {
    url.trim_start_matches("http://").trim_end_matches('/').to_string()
}

#[test]
fn seeded_handles_survive_restart() {
    let url = serve(true);
    let store = std::env::temp_dir().join(format!("r23a-seed-{}.redb", std::process::id()));
    let _ = std::fs::remove_file(&store);
    let first = {
        let mut c = Client::start(&store, "seed-a");
        let nav = c.tool("navigate", json!({"url": url}));
        handle_of(&nav, "button \"Add to cart\"")
    };
    // a fresh server process, a fresh visit: the server now serves the page without the skip link,
    // so an unseeded session would number the button differently
    let second = {
        let mut c = Client::start(&store, "seed-b");
        let nav = c.tool("navigate", json!({"url": url}));
        handle_of(&nav, "button \"Add to cart\"")
    };
    assert_eq!(first, second, "the same button must keep its handle across visits");
    let _ = std::fs::remove_file(&store);
}

#[test]
fn observe_after_act_records_an_edge() {
    let url = serve(false);
    let store = std::env::temp_dir().join(format!("r23a-edge-{}.redb", std::process::id()));
    let _ = std::fs::remove_file(&store);
    let site = host_of(&url);
    {
        let mut c = Client::start(&store, "edge-a");
        c.tool("navigate", json!({"url": url}));
        c.tool("observe", json!({}));
    }
    // the server process holds the store's lock until it exits
    let root_a = manuk_pagestore::PageStore::open(&store).unwrap().recall(&site).unwrap().expect("observe stores the page").root;
    {
        let mut c = Client::start(&store, "edge-b");
        let nav = c.tool("navigate", json!({"url": url}));
        c.tool("observe", json!({}));
        c.tool("act", json!({"handle": handle_of(&nav, "link \"Footer link\""), "action": "click"}));
        c.tool("observe", json!({}));
    }
    let s = manuk_pagestore::PageStore::open(&store).unwrap();
    let edges = s.edges(&root_a).unwrap();
    assert_eq!(edges.len(), 1, "one act, one edge: {edges:?}");
    let (action, to) = &edges[0];
    assert!(action.starts_with("click h"), "{action}");
    assert_ne!(to, &root_a, "the click moved to another document");
    let _ = std::fs::remove_file(&store);
}
