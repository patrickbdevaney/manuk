//! W3.9.a gate: origin sets hold below the hub. An in-set page whose redirect, <img>, <iframe>,
//! <script src>, fetch() and form-POST targets are all on evil.test must produce 0 egress there, on
//! BOTH backends. `MANUK_SCOPE_MUTATE=1` disables the check and this file must go red under it.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

type Log = Arc<Mutex<Vec<String>>>;

fn serve(evil: &str) -> (String, Log) {
    let log: Log = Arc::default();
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    let evil = evil.to_string();
    let lg = log.clone();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            let (lg, evil) = (lg.clone(), evil.clone());
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let n = s.read(&mut buf).unwrap_or(0);
                let first = String::from_utf8_lossy(&buf[..n])
                    .lines()
                    .next()
                    .unwrap_or("")
                    .to_string();
                let mut it = first.split_whitespace();
                let (m, p) = (it.next().unwrap_or(""), it.next().unwrap_or(""));
                // A bare TCP connect with no request line (Chrome opening a socket for a blocked frame
                // navigation) carries no request; it is a named residual, not counted as egress.
                if p != "/favicon.ico" && !first.is_empty() {
                    lg.lock().unwrap().push(format!("{m} {p}"));
                }
                if p.starts_with("/redir") {
                    let _ = write!(s, "HTTP/1.1 302 Found\r\nLocation: {evil}/landed\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                    return;
                }
                let page = format!(
                    r#"<!doctype html><html><head><title>Scope</title></head><body>
<h1>Scope page</h1>
<img src="{evil}/img.png">
<iframe src="{evil}/frame"></iframe>
<script src="{evil}/s.js"></script>
<script>fetch("{evil}/fetch"); var x = new XMLHttpRequest(); x.open("GET", "{evil}/xhr"); x.send();</script>
<form method="post" action="{evil}/post"><button id="p">Post out</button></form>
<p>EVILFRAMEMARKER-not-here</p>
</body></html>"#
                );
                let page = if evil.is_empty() {
                    "<html><body>EVILFRAMEMARKER-evil-side</body></html>".to_string()
                } else {
                    page
                };
                let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{page}", page.len());
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
    fn start(chrome: bool, flags: &[String]) -> Client {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"));
        cmd.args(flags);
        if chrome {
            cmd.args(["--backend", "chrome"]);
        }
        cmd.env(
            "MANUK_ACTLOG_DIR",
            std::env::temp_dir().join(format!("w39-log-{}-{chrome}", std::process::id())),
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
    fn ok(&mut self, name: &str, args: Value) -> String {
        let (e, t) = self.call(name, args);
        assert!(!e, "{name} failed: {t}");
        t
    }
    fn refused(&mut self, name: &str, args: Value, needle: &str) {
        let (e, t) = self.call(name, args);
        assert!(
            e && t.contains(needle),
            "{name}: expected an error containing {needle:?}, got error={e}: {t}"
        );
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

fn count(l: &Log) -> usize {
    l.lock().unwrap().len()
}

fn run(chrome: bool) {
    let (evil, evil_log) = serve("");
    let (app, app_log) = serve(&evil);
    let flags = vec![
        "--read-origins".to_string(),
        app.clone(),
        format!("--write-origins={app}"),
    ];
    let mut c = Client::start(chrome, &flags);
    let nav = c.ok("navigate", json!({"url": format!("{app}/")}));
    std::thread::sleep(std::time::Duration::from_millis(800));
    assert!(
        app_log.lock().unwrap().iter().any(|r| r == "GET /"),
        "page itself did not load"
    );
    // the page's own form POST to evil
    let (err, why) = c.call(
        "act",
        json!({"handle": handle_of(&nav, "button \"Post out\""), "action": "click"}),
    );
    assert!(
        err && why.contains("write set"),
        "cross-origin form POST was not refused: {why}"
    );
    // a redirect from the in-set origin into the evil one
    let _ = c.call("navigate", json!({"url": format!("{app}/redir")}));
    std::thread::sleep(std::time::Duration::from_millis(800));
    assert_eq!(
        count(&evil_log),
        0,
        "egress to the attacker origin: {:?}",
        evil_log.lock().unwrap()
    );
    let seen = c.ok("observe", json!({}));
    assert!(
        !seen.contains("EVILFRAMEMARKER-evil-side"),
        "evil frame content reached observe:\n{seen}"
    );
}

#[test]
fn page_initiated_loads_stay_in_set_on_the_manuk_backend() {
    run(false);
}

#[test]
fn page_initiated_loads_stay_in_set_on_the_chrome_backend() {
    run(true);
}
