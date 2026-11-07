//! W5.T1: the "no listeners" case uses a role=button div, because the lean observation hands out handles to operable elements only (a plain heading no longer has one).
//! W3.6 gate: a click that changes nothing returns its CAUSE (handler threw X / 0 listeners) and a repeat
//! counter, on both backends. `MANUK_NOEFF_MUTATE=1` freezes the counter (red).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{json, Value};

const PAGE: &str = r#"<!doctype html><html><head><title>Lab</title></head><body>
<h1>Lab</h1>
<button onclick="callAnUnimplementedApi()">Broken save</button>
<div id="plain" role="button">Just a label</div>
<div style="height:3000px">tall</div>
</body></html>"#;

fn serve(page: &'static str) -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let _ = s.read(&mut buf);
                let body = page;
                let _ = write!(
                    s,
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
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
    fn start(chrome: bool, mutate: bool) -> Client {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"));
        if chrome {
            cmd.args(["--backend", "chrome"]);
        }
        if mutate {
            cmd.env("MANUK_NOEFF_MUTATE", "1");
        }
        cmd.env(
            "MANUK_ACTLOG_DIR",
            std::env::temp_dir().join(format!("w36-log-{}-{chrome}", std::process::id())),
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

fn click_twice(chrome: bool, mutate: bool, label: &str) -> Vec<String> {
    let url = serve(PAGE);
    let mut c = Client::start(chrome, mutate);
    let nav = c.tool("navigate", json!({"url": url}));
    let h = handle_of(&nav, label);
    (0..2)
        .map(|_| c.tool("act", json!({"handle": h, "action": "click"})))
        .collect()
}

fn check(chrome: bool) {
    let r = click_twice(chrome, false, "Broken save");
    assert!(r[0].contains("NO EFFECT"), "{}", r[0]);
    assert!(r[0].contains("handler threw ReferenceError"), "{}", r[0]);
    assert!(
        !r[0].contains("identical act"),
        "first is not a repeat: {}",
        r[0]
    );
    assert!(
        r[1].contains("2nd identical act") && r[1].contains("unchanged since step 1"),
        "{}",
        r[1]
    );
    let r = click_twice(chrome, false, "Just a label");
    assert!(
        r[0].contains("NO EFFECT") && r[0].contains("0 click listeners"),
        "{}",
        r[0]
    );
}

#[test]
fn manuk_no_effect_acts_name_their_cause_and_count_repeats() {
    check(false);
}

#[test]
fn chrome_no_effect_acts_name_their_cause_and_count_repeats() {
    check(true);
}

#[test]
fn frozen_counter_mutant_goes_red() {
    let r = click_twice(false, true, "Broken save");
    assert!(
        !r[1].contains("2nd identical act"),
        "mutant must not count: {}",
        r[1]
    );
}

#[test]
fn scroll_to_the_bottom_says_at_end() {
    let url = serve(PAGE);
    let mut c = Client::start(false, false);
    c.tool("navigate", json!({"url": url}));
    let r = c.tool("scroll", json!({"dy": 100000}));
    assert!(r.contains("AT-END"), "{r}");
    let r = c.tool("scroll", json!({"dy": -100000}));
    assert!(!r.contains("AT-END"), "{r}");
}
