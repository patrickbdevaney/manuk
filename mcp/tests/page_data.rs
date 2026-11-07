//! W5.L3 gate (chrome_ver=145.0.7632.116): `page_data` returns the JSON the page itself fetched (same-origin, <= 64 KB)
//! with its request URL on BOTH backends; a cross-origin response and an over-cap body are refused with a
//! reason; a value read through it is value-flow tainted (typing it toward another origin is EGRESS-DENIED).
//! Mutants: `MANUK_PAGEDATA_MUTATE=no-origin` (cross-origin body returned) and `=no-taint` (no taint record)
//! each turn a test RED; `MANUK_CDP_MUTATE=no-data` (chrome hook off) turns the chrome run RED.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Mutex;

use serde_json::{json, Value};

const SECRET: &str = "tok_pd_5c4b3a29";

fn serve(page: String) -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            let page = page.clone();
            std::thread::spawn(move || {
                let mut buf = [0u8; 8192];
                let n = s.read(&mut buf).unwrap_or(0);
                let first = String::from_utf8_lossy(&buf[..n])
                    .lines()
                    .next()
                    .unwrap_or("")
                    .to_string();
                let path = first.split(' ').nth(1).unwrap_or("/").to_string();
                let (ct, body) = if path.starts_with("/api/products") {
                    (
                        "application/json",
                        format!(
                            r#"{{"items": [{{"id": 1, "name": "Widget", "price": 9.5}}, {{"id": 2, "name": "Gadget", "price": 12}}], "session_token": "{SECRET}"}}"#
                        ),
                    )
                } else if path.starts_with("/api/cart") {
                    (
                        "application/json",
                        r#"{"cart": ["Widget"], "total": 9.5}"#.to_string(),
                    )
                } else if path.starts_with("/api/big") {
                    (
                        "application/json",
                        format!(r#"{{"pad": "{}"}}"#, "x".repeat(70_000)),
                    )
                } else if path.starts_with("/ext/") {
                    (
                        "application/json",
                        r#"{"foreign": "OTHER-ORIGIN-DATA"}"#.to_string(),
                    )
                } else if path.starts_with("/favicon") {
                    ("text/plain", String::new())
                } else {
                    let a = s.local_addr().unwrap();
                    ("text/html", page.replace("ORIGIN", &format!("http://{a}")))
                };
                let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: {ct}\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            });
        }
    });
    format!("http://{addr}")
}

struct Client {
    child: Child,
    tx: ChildStdin,
    rx: BufReader<ChildStdout>,
    id: u64,
}

impl Client {
    fn start(chrome: bool, env: &[(&str, &str)]) -> Client {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"));
        if chrome {
            cmd.args(["--backend", "chrome"]);
        }
        for (k, v) in env {
            cmd.env(k, v);
        }
        cmd.env(
            "MANUK_ACTLOG_DIR",
            std::env::temp_dir().join(format!("l3-log-{}-{chrome}", std::process::id())),
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

static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Returns `(page_data text, EGRESS-DENIED on typing the API-only secret toward B)`.
fn run(chrome: bool, env: &[(&str, &str)]) -> (String, bool) {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let b = serve("<html><body>B</body></html>".into());
    let page = format!(
        r#"<!doctype html><html><head><title>Shop</title></head><body><h1>Shop</h1><div id="out">loading</div>
<form method="post" action="{b}/collect"><label>Note <input name="n" aria-label="to-b"></label><button type="button">Send</button></form>
<script>
fetch('ORIGIN/api/products?page=1').then(function(r){{return r.json()}}).then(function(j){{document.getElementById('out').textContent='n='+j.items.length}});
var x=new XMLHttpRequest();x.open('GET','ORIGIN/api/cart');x.send();
fetch('{b}/ext/prices').then(function(r){{return r.json()}}).catch(function(){{}});
fetch('ORIGIN/api/big').then(function(r){{return r.text()}}).catch(function(){{}});
</script></body></html>"#
    );
    let a = serve(page);
    let mut c = Client::start(chrome, env);
    c.ok("navigate", json!({"url": format!("{a}/")}));
    let obs = c.ok("observe", json!({}));
    assert!(
        !obs.contains(SECRET),
        "fixture: the secret is visible in the DOM:\n{obs}"
    );
    let (e, data) = c.call("page_data", json!({}));
    assert!(!e, "page_data failed: {data}");
    let obs = c.ok("observe", json!({}));
    let to_b = handle_of(&obs, "to-b");
    let (err, txt) = c.call("type", json!({"handle": to_b, "text": SECRET}));
    eprintln!(
        "L3 chrome={chrome} env={env:?}\n{data}\ntype->B err={err} {}",
        txt.lines().next().unwrap_or("")
    );
    (data, err && txt.contains("EGRESS-DENIED"))
}

fn check_good(data: &str, denied: bool) {
    assert!(
        data.contains("GET http://127.0.0.1") && data.contains("/api/products?page=1 200"),
        "products url/status missing:\n{data}"
    );
    assert!(
        data.contains(r#""name":"Widget""#) && data.contains(r#""price":12"#),
        "products body missing:\n{data}"
    );
    assert!(
        data.contains("/api/cart 200") && data.contains(r#""total":9.5"#),
        "XHR cart missing:\n{data}"
    );
    assert!(
        !data.contains("OTHER-ORIGIN-DATA"),
        "cross-origin body leaked:\n{data}"
    );
    assert!(
        data.contains("refused") && data.contains("/ext/prices") && data.contains("cross-origin"),
        "cross-origin not refused with a reason:\n{data}"
    );
    assert!(
        data.contains("/api/big") && data.contains("over 65536 B cap"),
        "over-cap not refused with a reason:\n{data}"
    );
    assert!(
        !data.contains("xxxxxxxx"),
        "over-cap body returned:\n{data}"
    );
    assert!(
        denied,
        "a value read through page_data must be tainted against another origin"
    );
}

#[test]
fn page_data_on_engine() {
    let (d, t) = run(false, &[]);
    check_good(&d, t);
}

#[test]
fn page_data_on_chrome() {
    let (d, t) = run(true, &[]);
    check_good(&d, t);
}

#[test]
fn mutant_no_origin_filter_goes_red() {
    let (d, _) = run(false, &[("MANUK_PAGEDATA_MUTATE", "no-origin")]);
    assert!(d.contains("OTHER-ORIGIN-DATA"), "mutant stayed green:\n{d}");
}

#[test]
fn mutant_no_taint_goes_red() {
    let (_, denied) = run(false, &[("MANUK_PAGEDATA_MUTATE", "no-taint")]);
    assert!(
        !denied,
        "mutant stayed green: secret still blocked without the page_data taint record"
    );
}
