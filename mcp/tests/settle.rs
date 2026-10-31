//! W3.12 gate: the act result waits for the act's own consequences (click → fetch(400 ms) → timer
//! 1500 ms → render) and ends with a settle line read from the backend's own queues. Both backends.
//! `MANUK_SETTLE_MUTATE=no-settle` skips the wait: the content test must then go red.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::Instant;

use serde_json::{json, Value};

const PAGE: &str = r#"<!doctype html><html><head><title>Settle</title></head><body>
<h1>Settle fixture</h1>
<button id="go" onclick="fetch('/api/data').then(function(r){return r.text()}).then(function(t){setTimeout(function(){document.getElementById('o').textContent='RESULT '+t},1500)})">Go</button>
<button id="slow" onclick="setTimeout(function(){document.getElementById('o').textContent='LATE'},10000)">Slow</button>
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
                let body = if req.starts_with("GET /api/data") {
                    // W3.12.a: MANUK_SETTLE_FETCH_MS=0 serves the fetch instantly (the --deterministic wall).
                    let ms = std::env::var("MANUK_SETTLE_FETCH_MS")
                        .ok()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(400);
                    std::thread::sleep(std::time::Duration::from_millis(ms));
                    "ORDER-42"
                } else {
                    PAGE
                };
                let ct = if body == PAGE {
                    "text/html"
                } else {
                    "text/plain"
                };
                let _ = write!(
                    s,
                    "HTTP/1.1 200 OK\r\nContent-Type: {ct}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
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
    fn start(chrome: bool) -> Client {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"));
        if chrome {
            cmd.args(["--backend", "chrome"]);
        } else if std::env::var("MANUK_SETTLE_DETERMINISTIC").is_ok() {
            cmd.arg("--deterministic");
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

fn run(chrome: bool) {
    let url = serve();
    let mut c = Client::start(chrome);
    let nav = c.tool("navigate", json!({"url": url}));
    assert!(
        nav.lines().any(|l| l.starts_with("settled:")),
        "navigate lacks a settle line:\n{nav}"
    );
    let go = handle_of(&nav, "button \"Go\"");
    let t0 = Instant::now();
    let r = c.tool("act", json!({"handle": go, "action": "click"}));
    eprintln!("go took {:?}\n{r}", t0.elapsed());
    let obs = c.tool("extract", json!({}));
    eprintln!("OBS after:\n{obs}");
    assert!(
        r.contains("RESULT ORDER-42"),
        "first observation misses the late result:\n{r}"
    );
    assert!(
        r.lines()
            .any(|l| l.starts_with("settled: 0 fetch, 0 timers<=2s")),
        "{r}"
    );

    let slow = handle_of(&nav, "button \"Slow\"");
    let t0 = Instant::now();
    let r = c.tool("act", json!({"handle": slow, "action": "click"}));
    assert!(
        r.lines()
            .any(|l| l.starts_with("settled") && l.contains("pending later: 1 timer")),
        "10 s timer not reported as pending:\n{r}"
    );
    assert!(!r.contains("LATE"), "{r}");
    assert!(
        t0.elapsed().as_secs() < 6,
        "waited on the 10 s timer: {:?}",
        t0.elapsed()
    );
}

#[test]
fn settle_oracle_on_the_manuk_backend() {
    run(false);
}

#[test]
fn settle_oracle_on_the_chrome_backend() {
    run(true);
}
