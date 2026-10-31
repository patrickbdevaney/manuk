//! CB.1 gate: every manuk-mcp tool, driven over real stdio against BOTH backends (the in-process
//! engine and `--backend chrome`) on one loopback page. The matrix in docs/sprint/BACKENDS.md is
//! this file's pass/fail. `MANUK_CDP_MUTATE=no-screenshot` removes the chrome screenshot path, and
//! `MANUK_CDP_MUTATE=no-hover` makes chrome's hover a no-op: the matrix must then go red on chrome.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{json, Value};

const PAGE: &str = r#"<!doctype html><html><head><title>Backends</title></head><body>
<h1>Backend parity</h1>
<a href="/other">Other page</a>
<button id="b" onclick="document.getElementById('o').textContent='clicked-ok'">Press me</button>
<p id="o">idle</p>
<label>Name <input id="n" type="text"></label>
<label><input id="c" type="checkbox"> Agree</label>
<input id="f" type="text" aria-label="Probe">
<button id="hv" aria-label="Hover probe">Press hover</button>
<label>Size <select id="s"><option>Small</option><option>Large</option></select></label>
<table><tr><th>K</th><th>V</th></tr><tr><td>alpha</td><td>1</td></tr></table>
<div style="height:3000px">tall</div><p id="end">bottom marker</p>
<script>localStorage.setItem('seeded','yes');document.getElementById('hv').addEventListener('mouseover',function(){this.setAttribute('aria-label','hovered-ok')});setInterval(function(){document.getElementById('f').setAttribute('aria-label',document.activeElement===document.getElementById('f')?'focused-ok':'Probe')},20)</script></body></html>"#;

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
    fn start(chrome: bool) -> Client {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"));
        if chrome {
            cmd.args(["--backend", "chrome"]);
        }
        cmd.env(
            "MANUK_ACTLOG_DIR",
            std::env::temp_dir().join(format!("cb1-log-{}-{chrome}", std::process::id())),
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

fn matrix(chrome: bool) {
    let url = serve();
    let mut c = Client::start(chrome);
    let nav = c.tool("navigate", json!({"url": url}));
    assert!(nav.contains("Press me"), "navigate:\n{nav}");

    // observe (full / viewport / diff)
    assert!(c.tool("observe", json!({"mode": "full"})).contains("Agree"));
    assert!(c
        .tool("observe", json!({"mode": "viewport"}))
        .contains("Press me"));
    c.tool("observe", json!({"mode": "diff"}));

    // extract text / markdown / table, find
    assert!(c.tool("extract", json!({})).contains("Backend parity"));
    let q = c.tool("extract", json!({"query": "backend parity"}));
    assert!(
        q.contains("blocks shown") && q.contains("Backend parity"),
        "query extract:\n{q}"
    );
    assert!(c
        .tool("extract", json!({"format": "markdown"}))
        .contains("Other page"));
    assert!(c
        .tool("extract", json!({"format": "table"}))
        .contains("alpha"));
    assert!(c
        .tool("find", json!({"query": "press"}))
        .contains("Press me"));

    // act: click / check / select
    let b = handle_of(&nav, "button \"Press me\"");
    c.tool("act", json!({"handle": b, "action": "click"}));
    assert!(
        c.tool("extract", json!({})).contains("clicked-ok"),
        "click had no effect"
    );
    let ck = handle_of(&nav, "checkbox \"Agree\"");
    c.tool("act", json!({"handle": ck, "action": "check"}));
    let sel = handle_of(&nav, "combobox \"Size\"");
    c.tool(
        "act",
        json!({"handle": sel, "action": "select", "value": "Large"}),
    );

    // type
    let n = handle_of(&nav, "textbox \"Name\"");
    c.tool("type", json!({"handle": n, "text": "Ada"}));
    assert!(
        c.tool("observe", json!({})).contains("Ada"),
        "typed text not observed"
    );

    // focus / hover: the page's own handlers must see both (CB.1.a)
    let f = handle_of(&nav, "textbox \"Probe\"");
    let focus_reply = c.tool("act", json!({"handle": f, "action": "focus"}));
    assert!(
        focus_reply.contains("focused h"),
        "focus reply: {focus_reply}"
    );
    // The page sees focus from a timer on chrome. manuk accepts the focus action (checked above by
    // `tool`), but its engine fires no focus events, so the page's probe stays idle: BACKENDS.md row.
    if chrome {
        let focused = (0..20).any(|_| {
            std::thread::sleep(std::time::Duration::from_millis(50));
            c.tool("observe", json!({})).contains("focused-ok")
        });
        assert!(focused, "focus did not reach the page");
    }
    let hv = handle_of(&nav, "button \"Hover probe\"");
    c.tool("act", json!({"handle": hv, "action": "hover"}));
    assert!(
        c.tool("observe", json!({})).contains("hovered-ok"),
        "hover did not reach the page"
    );

    // scroll by dy
    c.tool("scroll", json!({"dy": 1500}));

    // screenshot: a real PNG
    let (err, t, img) = c.call("screenshot", json!({}));
    assert!(!err, "screenshot failed: {t}");
    let raw = base64_decode(&img.expect("no image block"));
    assert_eq!(&raw[..8], b"\x89PNG\r\n\x1a\n", "not a PNG");
    assert!(raw.len() > 500, "suspiciously small PNG: {}", raw.len());

    // storage_state: export has the page's localStorage, import round-trips a cookie
    let ex: Value = serde_json::from_str(json_part(
        &c.tool("storage_state", json!({"action": "export"})),
    ))
    .expect("export is JSON");
    let has_seed = ex["origins"].as_array().unwrap().iter().any(|o| {
        o["localStorage"]
            .as_array()
            .unwrap()
            .iter()
            .any(|kv| kv["name"] == "seeded" && kv["value"] == "yes")
    });
    assert!(
        has_seed,
        "localStorage seeded=yes missing from export: {ex}"
    );
    let path = std::env::temp_dir().join(format!("cb1-state-{}-{chrome}.json", std::process::id()));
    let state = json!({"cookies": [{"name": "cb1", "value": "v", "domain": "example.com", "path": "/", "expires": -1, "httpOnly": false, "secure": false, "sameSite": "Lax"}], "origins": []});
    std::fs::write(&path, state.to_string()).unwrap();
    let imp = c.tool(
        "storage_state",
        json!({"action": "import", "path": path.to_str().unwrap()}),
    );
    assert!(imp.contains("imported 1 cookie"), "{imp}");
    let ex2 = c.tool("storage_state", json!({"action": "export"}));
    assert!(
        ex2.contains("\"cb1\""),
        "imported cookie not exported back: {ex2}"
    );
    let _ = std::fs::remove_file(&path);
}

/// The tool appends a `page: <handle>` footer after the JSON body.
fn json_part(t: &str) -> &str {
    t.rsplit_once("\npage: ").map_or(t, |(j, _)| j)
}

/// checkpoint / restore / fork / commit on one backend.
fn rollback(chrome: bool) {
    let url = serve();
    let mut c = Client::start(chrome);
    let nav = c.tool("navigate", json!({"url": url}));
    let page = |t: &str| {
        t.lines()
            .rev()
            .find_map(|l| l.strip_prefix("page: "))
            .unwrap()
            .trim()
            .to_string()
    };
    let cp = c.tool("checkpoint", json!({"page": page(&nav)}));
    let id = cp
        .split_whitespace()
        .skip_while(|w| *w != "checkpoint")
        .nth(1)
        .unwrap()
        .to_string();
    let b = handle_of(&nav, "button \"Press me\"");
    let eff = c.tool(
        "act",
        json!({"handle": b, "action": "click", "page": page(&cp)}),
    );
    assert!(c
        .tool("extract", json!({"page": page(&eff)}))
        .contains("clicked-ok"));
    let r = c.tool("restore", json!({"checkpoint": id}));
    let after = c.tool("extract", json!({"page": page(&r)}));
    assert!(
        after.contains("idle") && !after.contains("clicked-ok"),
        "restore did not rewind:\n{after}"
    );
    let f = c.tool("fork", json!({"checkpoint": id}));
    c.tool("commit", json!({"page": page(&f)}));
}

#[test]
fn rollback_on_the_manuk_backend() {
    if std::env::var("MANUK_CDP_MUTATE").is_ok() {
        return;
    }
    rollback(false);
}

#[test]
fn rollback_on_the_chrome_backend() {
    if std::env::var("MANUK_CDP_MUTATE").is_ok() {
        return;
    }
    rollback(true);
}

fn base64_decode(s: &str) -> Vec<u8> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(s)
        .expect("base64")
}

/// The screenshot mutant breaks every matrix run on purpose, so those runs stand down for it; the
/// screenshot twin below is its gate. Other mutants leave the matrix running, so it can go red.
fn matrix_stands_down() -> bool {
    std::env::var("MANUK_CDP_MUTATE").is_ok_and(|v| v == "no-screenshot")
}

#[test]
fn every_tool_on_the_manuk_backend() {
    if matrix_stands_down() {
        return;
    }
    matrix(false);
}

#[test]
fn every_tool_on_the_chrome_backend() {
    if matrix_stands_down() {
        return;
    }
    matrix(true);
}

/// Twin: with the chrome screenshot path removed the chrome screenshot call must fail.
#[test]
fn chrome_screenshot_mutant_fails() {
    let url = serve();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"));
    cmd.args(["--backend", "chrome"])
        .env("MANUK_CDP_MUTATE", "no-screenshot");
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
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
    c.tool("navigate", json!({"url": url}));
    let (err, _, img) = c.call("screenshot", json!({}));
    assert!(err && img.is_none(), "mutant still produced a screenshot");
}
