//! W5.T2 gate: act/type/fill_form results are one line by default. On a fixed act sequence the median
//! tokens (chars/4) per act result falls >= 50% against the full form (`MANUK_CONCISE_MUTATE=full`
//! restores it: the baseline), and an act that had no effect keeps the full diff automatically
//! (`MANUK_CONCISE_MUTATE=always` removes that exception: the RED mutant). `detail=true` forces full.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{json, Value};

const PAGE: &str = r##"<!doctype html><title>T2</title><body><h1>Order</h1>
<label>Name <input id="n" type="text"></label><label>Email <input id="e" type="text"></label>
<label><input id="c" type="checkbox"> Gift</label>
<button id="add">Add row</button><button id="dead">Inert</button><ul id="l"><li>seed</li></ul>
<script>var k=0;document.getElementById('add').onclick=function(){k++;var li=document.createElement('li');li.innerHTML='<a href="#r'+k+'">Row '+k+'</a> <button>Remove '+k+'</button>';document.getElementById('l').appendChild(li);};</script></body>"##;

fn serve() -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let _ = s.read(&mut buf);
                let _ = write!(
                    s,
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{PAGE}",
                    PAGE.len()
                );
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
    fn start(mutate: Option<&str>) -> Client {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"));
        if let Some(m) = mutate {
            cmd.env("MANUK_CONCISE_MUTATE", m);
        }
        cmd.env(
            "MANUK_ACTLOG_DIR",
            std::env::temp_dir().join(format!("w5t2-log-{}-{}", std::process::id(), mutate.unwrap_or("none"))),
        );
        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn manuk-mcp");
        let tx = child.stdin.take().unwrap();
        let rx = BufReader::new(child.stdout.take().unwrap());
        let mut c = Client { child, tx, rx, id: 0 };
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
            assert!(self.rx.read_line(&mut line).unwrap() > 0, "server closed stdout");
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
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|b| b["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!res["isError"].as_bool().unwrap_or(false), "{name} failed: {text}");
        text
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

fn tokens(s: &str) -> usize {
    s.len().div_ceil(4)
}

fn handle_of(obs: &str, needle: &str) -> String {
    let l = obs.lines().find(|l| l.contains(needle)).unwrap_or_else(|| panic!("no {needle} in\n{obs}"));
    let a = l.find("[h").unwrap();
    l[a + 1..l[a..].find(']').unwrap() + a].to_string()
}

/// Results of the fixed act sequence: (successful acts' texts, the no-effect act's text).
fn run(mutate: Option<&str>) -> (Vec<String>, String) {
    let base = serve();
    let mut c = Client::start(mutate);
    c.tool("navigate", json!({"url": format!("{base}/")}));
    let obs = c.tool("observe", json!({}));
    let (name, email, gift) = (handle_of(&obs, "Name"), handle_of(&obs, "Email"), handle_of(&obs, "Gift"));
    let (add, dead) = (handle_of(&obs, "Add row"), handle_of(&obs, "Inert"));
    let mut ok = vec![
        c.tool("type", json!({"handle": name, "text": "Ada Lovelace"})),
        c.tool("type", json!({"handle": email, "text": "ada@example.com"})),
        c.tool("act", json!({"handle": gift, "action": "check"})),
    ];
    for _ in 0..4 {
        ok.push(c.tool("act", json!({"handle": add, "action": "click"})));
    }
    let noeff = c.tool("act", json!({"handle": dead, "action": "click"}));
    let detail = c.tool("act", json!({"handle": add, "action": "click", "detail": true}));
    assert!(detail.contains("url: unchanged") && detail.contains("verified:"), "detail=true is full: {detail}");
    (ok, noeff)
}

fn median(v: &mut [usize]) -> usize {
    v.sort();
    v[v.len() / 2]
}

#[test]
fn concise_acts_cut_median_tokens_and_failures_stay_full() {
    let (full_ok, full_noeff) = run(Some("full"));
    let (lean_ok, lean_noeff) = run(None);
    let mut f: Vec<usize> = full_ok.iter().map(|t| tokens(t)).collect();
    let mut l: Vec<usize> = lean_ok.iter().map(|t| tokens(t)).collect();
    let (mf, ml) = (median(&mut f), median(&mut l));
    eprintln!("W5T2 median_full={mf} median_concise={ml} cut={:.3}", 1.0 - ml as f64 / mf as f64);
    eprintln!("W5T2 sample concise: {}", lean_ok[3]);
    for t in &lean_ok {
        let body = t.split("\npage: ").next().unwrap();
        assert!(!body.contains('\n'), "concise result is one line: {t}");
    }
    assert!(ml as f64 <= 0.5 * mf as f64, "median tokens {ml} must be <= 50% of {mf}");
    // The no-effect act carries the full diff in both forms, identical in kind.
    assert!(lean_noeff.contains("NO EFFECT") && lean_noeff.contains("no change") && lean_noeff.contains("url: unchanged"), "no-effect act must stay full: {lean_noeff}");
    assert!(full_noeff.contains("NO EFFECT"));
}

/// The gate can go red: with the auto-detail exception removed the failed act is condensed to a
/// success-looking line, which the assertion in the main test rejects. Proven here by running it.
#[test]
fn removing_auto_detail_is_red() {
    let (_, noeff) = run(Some("always"));
    assert!(
        !(noeff.contains("NO EFFECT") && noeff.contains("url: unchanged")),
        "mutant should have lost the failure detail: {noeff}"
    );
    assert!(noeff.starts_with("ok:"), "the lie the main test catches: {noeff}");
}
