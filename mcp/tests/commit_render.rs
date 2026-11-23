//! W6.S1.b gate: a fenced fork's held requests are RENDERED for the human before release. The page has a
//! button that POSTs a 14-byte body. checkpoint -> fork (fence up) -> click: the server must see nothing.
//! `commit` without `confirm` must answer CONFIRM REQUIRED naming method + origin + path + body size and still
//! send nothing; `commit confirm=true` sends exactly once; a second confirmed commit sends nothing more.
//! `MANUK_MUTATE=commit-no-render` drops the render and releases at once; this test must go red under it.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

type Log = Arc<Mutex<Vec<String>>>;

const PAGE: &str = r#"<!doctype html><title>Order</title><button id="b" onclick="fetch('/order',{method:'POST',body:'hello-world-12'})">Place order</button>"#;

fn serve() -> (String, Log) {
    let log: Log = Arc::default();
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    let lg = log.clone();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            let lg = lg.clone();
            std::thread::spawn(move || {
                let mut buf = [0u8; 8192];
                let n = s.read(&mut buf).unwrap_or(0);
                let first = String::from_utf8_lossy(&buf[..n])
                    .lines()
                    .next()
                    .unwrap_or("")
                    .to_string();
                let mut it = first.split_whitespace();
                let (m, p) = (it.next().unwrap_or(""), it.next().unwrap_or(""));
                if p == "/favicon.ico" {
                    return;
                }
                lg.lock().unwrap().push(format!("{m} {p}"));
                let (ct, body) = if p == "/" {
                    ("text/html", PAGE)
                } else {
                    ("text/plain", "ok")
                };
                let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: {ct}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            });
        }
    });
    (format!("http://{addr}"), log)
}

struct Client {
    child: Child,
    tx: ChildStdin,
    rx: BufReader<ChildStdout>,
    id: u64,
}

impl Client {
    fn start(flags: &[String]) -> Client {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"));
        cmd.args(flags);
        if let Ok(m) = std::env::var("MANUK_MUTATE") {
            cmd.env("MANUK_MUTATE", m);
        }
        cmd.env(
            "MANUK_ACTLOG_DIR",
            std::env::temp_dir().join(format!("w6s1b-log-{}", std::process::id())),
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
    fn call(&mut self, name: &str, args: Value) -> (bool, String) {
        let r = self.rpc("tools/call", json!({"name": name, "arguments": args}));
        let res = &r["result"];
        let blocks = res["content"].as_array().cloned().unwrap_or_default();
        (
            res["isError"].as_bool().unwrap_or(false),
            blocks
                .iter()
                .filter_map(|b| b["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n"),
        )
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

fn posts(log: &Log) -> usize {
    log.lock()
        .unwrap()
        .iter()
        .filter(|l| l.starts_with("POST /order"))
        .count()
}

fn page_of(t: &str) -> String {
    t.lines()
        .rev()
        .find_map(|l| l.strip_prefix("page: "))
        .unwrap()
        .trim()
        .to_string()
}

#[test]
fn held_requests_are_rendered_before_release_and_sent_once() {
    let (base, log) = serve();
    let mut c = Client::start(&[]);
    let (_, nav) = c.call("navigate", json!({"url": format!("{base}/")}));
    let (_, cp) = c.call("checkpoint", json!({"page": page_of(&nav)}));
    let cid = cp
        .split_whitespace()
        .skip_while(|w| *w != "checkpoint")
        .nth(1)
        .unwrap()
        .to_string();
    let (_, fk) = c.call("fork", json!({"checkpoint": cid}));
    let line = fk
        .lines()
        .find(|l| l.contains("Place order"))
        .map(str::to_string)
        .unwrap_or_else(|| {
            c.call("observe", json!({"page": page_of(&fk)}))
                .1
                .lines()
                .find(|l| l.contains("Place order"))
                .expect("button")
                .to_string()
        });
    let h = &line[line.find("[h").unwrap() + 1..];
    let h = &h[..h.find(']').unwrap()];
    let (_, act) = c.call(
        "act",
        json!({"handle": h, "action": "click", "page": page_of(&fk)}),
    );
    std::thread::sleep(std::time::Duration::from_millis(700));
    assert_eq!(posts(&log), 0, "the fence must hold the POST: {act}");

    let (_, ask) = c.call("commit", json!({"page": page_of(&act)}));
    std::thread::sleep(std::time::Duration::from_millis(500));
    assert!(
        ask.contains("CONFIRM REQUIRED"),
        "no confirm prompt:\n{ask}"
    );
    assert!(ask.contains("POST"), "prompt lacks the method:\n{ask}");
    assert!(
        ask.contains(&base),
        "prompt lacks the origin {base}:\n{ask}"
    );
    assert!(ask.contains("/order"), "prompt lacks the path:\n{ask}");
    assert!(
        ask.contains("body=14 bytes"),
        "prompt lacks the body size:\n{ask}"
    );
    assert_eq!(posts(&log), 0, "an unconfirmed commit must send nothing");

    let (_, done) = c.call("commit", json!({"page": page_of(&ask), "confirm": true}));
    std::thread::sleep(std::time::Duration::from_millis(500));
    assert!(done.contains("committed: sent 1"), "{done}");
    assert_eq!(posts(&log), 1, "release must send exactly once");
    let (_, again) = c.call("commit", json!({"page": page_of(&done), "confirm": true}));
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(again.contains("sent 0"), "{again}");
    assert_eq!(posts(&log), 1, "a second commit must not resend");
}
