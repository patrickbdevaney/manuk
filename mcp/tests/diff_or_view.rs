//! W5.T4 gate: observe(diff) and a plain act's effect diff fall back to a labelled compact full view when
//! the diff exceeds 40% of a full view, or the URL changed. A small diff stays a diff.
//! `MANUK_DIFFVIEW_MUTATE=off` removes the rule (the RED mutant).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{json, Value};

const LABEL: &str = "full view instead of a diff";

const PAGE: &str = r##"<!doctype html><title>T4</title><body><h1>Lab</h1>
<p><label><input type="checkbox" id="small"> Gift</label></p><p><button id="big">Rewrite</button></p><p><button id="route">Route</button></p>
<div id="f"><p><a href="#f1">Feature one of the lab</a></p><p><a href="#f2">Feature two of the lab</a></p><p><a href="#f3">Feature three of the lab</a></p><p><a href="#f4">Feature four of the lab</a></p><p><a href="#f5">Feature five of the lab</a></p><p><a href="#f6">Feature six of the lab</a></p><p><a href="#f7">Feature seven of the lab</a></p><p><a href="#f8">Feature eight of the lab</a></p></div>
<p><button id="late">Later</button></p>
<ul id="l"><li><a href="#a">Alpha</a></li><li><a href="#b">Beta</a></li></ul><p id="s">status one</p>
<script>
document.getElementById('route').onclick=function(){history.pushState({},'','/routed');};

document.getElementById('late').onclick=function(){setTimeout(function(){document.getElementById('big').click();},9000);};
document.getElementById('big').onclick=function(){var l=document.getElementById('l');l.innerHTML='';
for(var i=0;i<30;i++){var li=document.createElement('li');li.innerHTML='<a href="#n'+i+'">Brand new item '+i+'</a> <button>Drop '+i+'</button>';l.appendChild(li);}};
</script></body>"##;

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
    fn start(mutate: bool) -> Client {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"));
        if mutate {
            cmd.env("MANUK_DIFFVIEW_MUTATE", "off");
        }
        cmd.env(
            "MANUK_ACTLOG_DIR",
            std::env::temp_dir().join(format!("w5t4-log-{}-{mutate}", std::process::id())),
        );
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
        .find(|l| l.contains("[h") && l.contains(needle))
        .unwrap_or_else(|| panic!("no line with {needle:?} in:\n{text}"));
    let a = line.find("[h").unwrap();
    let b = line[a..].find(']').unwrap() + a;
    line[a + 1..b].to_string()
}

fn click(c: &mut Client, nav: &str, label: &str) -> String {
    c.tool(
        "act",
        json!({"handle": handle_of(nav, label), "action": "click", "detail": true}),
    )
}

#[test]
fn small_observe_diff_stays_a_diff() {
    let mut c = Client::start(false);
    let nav = c.tool("navigate", json!({"url": serve()}));
    c.tool("observe", json!({"mode": "full"}));
    c.tool(
        "act",
        json!({"handle": handle_of(&nav, "checkbox"), "action": "click"}),
    );
    let d = c.tool("observe", json!({"mode": "diff"}));
    assert!(!d.contains(LABEL), "{d}");
}

#[test]
fn large_observe_diff_becomes_a_labelled_full_view() {
    let mut c = Client::start(false);
    let nav = c.tool("navigate", json!({"url": serve()}));
    c.tool("observe", json!({"mode": "full"}));
    // the rewrite fires 9s after the act returned, so only observe(diff) sees it
    c.tool(
        "act",
        json!({"handle": handle_of(&nav, "Later"), "action": "click"}),
    );
    c.tool(
        "wait_for",
        json!({"text": "Brand new item 29", "timeout": 20}),
    );
    let d = c.tool("observe", json!({"mode": "diff"}));
    assert!(
        d.contains(LABEL) && d.contains("over 40%"),
        "observe diff: {d}"
    );
    assert!(
        d.contains("Brand new item 29"),
        "full view carries the content: {d}"
    );
}

#[test]
fn large_act_effect_becomes_a_labelled_full_view() {
    let mut c = Client::start(false);
    let nav = c.tool("navigate", json!({"url": serve()}));
    let r = click(&mut c, &nav, "Rewrite");
    assert!(
        r.contains(LABEL) && r.contains("over 40%"),
        "act effect: {r}"
    );
}

#[test]
fn small_act_effect_stays_a_diff() {
    let mut c = Client::start(false);
    let nav = c.tool("navigate", json!({"url": serve()}));
    let r = click(&mut c, &nav, "checkbox");
    assert!(!r.contains(LABEL), "{r}");
    assert!(r.contains("checked") || r.contains("~[h"), "{r}");
}

#[test]
fn a_same_document_route_change_is_a_labelled_full_view() {
    let mut c = Client::start(false);
    let nav = c.tool("navigate", json!({"url": serve()}));
    let r = click(&mut c, &nav, "Route");
    assert!(
        r.contains("routed (same document") && r.contains(LABEL) && r.contains("the URL changed"),
        "{r}"
    );
}

#[test]
fn mutant_without_the_threshold_returns_the_big_diff_raw() {
    let mut c = Client::start(true);
    let nav = c.tool("navigate", json!({"url": serve()}));
    let r = click(&mut c, &nav, "Rewrite");
    assert!(!r.contains(LABEL), "mutant must not label: {r}");
}
