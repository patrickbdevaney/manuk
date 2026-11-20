//! W6.J2 gate: event-sequence conformance. One logging page (document-level capture listeners for ~40
//! event types), the same MCP actions (click, type, key Enter, key Tab, hover, focus) driven on the in-process
//! engine and on `--backend chrome` (which uses CDP Input.dispatch*), and the per-action event sequences diffed.
//! Each action prints `EVSEQ <action>: same|DIFF ...` lines. Known, named diffs live in KNOWN below; any other
//! diff fails the test. `MANUK_EVSEQ_MUTATE=drop-focus` removes `focus`/`focusin` from the manuk log (stand-in for
//! the CB.1.a.a gap: the engine moving focus without firing events), and the harness must go red on it.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{json, Value};

const EVENTS: &[&str] = &[
    "pointerover",
    "pointerenter",
    "pointerdown",
    "pointermove",
    "pointerup",
    "pointerout",
    "pointerleave",
    "mouseover",
    "mouseenter",
    "mousedown",
    "mousemove",
    "mouseup",
    "mouseout",
    "mouseleave",
    "click",
    "dblclick",
    "contextmenu",
    "auxclick",
    "focus",
    "blur",
    "focusin",
    "focusout",
    "keydown",
    "keypress",
    "keyup",
    "beforeinput",
    "input",
    "change",
    "select",
    "compositionstart",
    "compositionend",
    "submit",
    "reset",
    "invalid",
    "scroll",
    "wheel",
    "touchstart",
    "touchend",
    "selectionchange",
    "dragstart",
    "cut",
];

fn page() -> String {
    let evs = EVENTS
        .iter()
        .map(|e| format!("'{e}'"))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        r#"<!doctype html><html><head><title>EvSeq</title></head><body>
<form id="fm" action="javascript:void(0)"><input id="a" type="text" aria-label="Alpha"><input id="b" type="text" aria-label="Beta"><button id="btn" type="button">Press</button></form>
<div id="hv" role="button" tabindex="0" aria-label="Hover">hover me</div>
<pre id="log"></pre>
<script>
var L=[];var pre=document.getElementById('log');
function id(t){{return t===document?'doc':t===window?'win':(t.id||t.nodeName)}}
[{evs}].forEach(function(e){{
 window.addEventListener(e,function(ev){{L.push('w.'+e+'@'+id(ev.target));pre.textContent=L.join(' ');}},true);
 document.addEventListener(e,function(ev){{L.push('d.'+e+'@'+id(ev.target));pre.textContent=L.join(' ');}},true);
}});
document.getElementById('fm').addEventListener('submit',function(ev){{ev.preventDefault()}});
</script></body></html>"#
    )
}

fn serve() -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    let body = page();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            let body = body.clone();
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let _ = s.read(&mut buf);
                let _ = write!(
                    s,
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
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
    fn start(chrome: bool) -> Client {
        Client::start_env(chrome, None)
    }
    fn start_env(chrome: bool, mutate: Option<&str>) -> Client {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"));
        if let Some(m) = mutate {
            cmd.env("MANUK_CDP_MUTATE", m);
        }
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

/// Event tokens (`type@target`) logged on the page so far.
fn log_of(c: &mut Client) -> Vec<String> {
    std::thread::sleep(std::time::Duration::from_millis(300));
    let t = c.tool("extract", json!({}));
    t.split_whitespace()
        .filter(|w| (w.starts_with("w.") || w.starts_with("d.")) && w.contains('@'))
        .map(String::from)
        .collect()
}

/// Run one action on a fresh page; return the events the ACTION caused (setup events cut off).
fn run_action(chrome: bool, url: &str, name: &str) -> Vec<String> {
    let mut c = Client::start(chrome);
    let nav = c.tool("navigate", json!({"url": url}));
    let a = handle_of(&nav, "textbox \"Alpha\"");
    let btn = handle_of(&nav, "button \"Press\"");
    let hv = handle_of(&nav, "button \"Hover\"");
    // setup (not part of the compared sequence)
    if name == "tab" || name == "enter" {
        c.tool("act", json!({"handle": a, "action": "focus"}));
    }
    let before = log_of(&mut c).len();
    match name {
        "click" => drop(c.tool("act", json!({"handle": btn, "action": "click"}))),
        "click-input" => drop(c.tool("act", json!({"handle": a, "action": "click"}))),
        "type" => drop(c.tool("type", json!({"handle": a, "text": "hi"}))),
        "enter" => drop(c.tool("key", json!({"key": "Enter"}))),
        "tab" => drop(c.tool("key", json!({"key": "Tab"}))),
        "hover" => drop(c.tool("act", json!({"handle": hv, "action": "hover"}))),
        "focus" => drop(c.tool("act", json!({"handle": a, "action": "focus"}))),
        _ => unreachable!(),
    }
    let mut all = log_of(&mut c);
    let mut seq = all.split_off(before.min(all.len()));
    if !chrome && std::env::var("MANUK_EVSEQ_MUTATE").as_deref() == Ok("drop-focus") {
        seq.retain(|e| !(e.contains(".focus@") || e.contains(".focusin@")));
    }
    seq
}

fn ty(tok: &str) -> &str {
    tok[2..].split('@').next().unwrap()
}

/// Diff classes between manuk's and chrome's sequence for one action. Each class string is a gate:
/// KNOWN holds the ones already measured (each has a follow-up row in docs/sprint/GRAPH.tsv); a class not in
/// KNOWN fails the test, and a KNOWN class that no longer appears also fails (it was fixed: delete it).
fn classes(m: &[String], ch: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let w = |v: &[String]| v.iter().filter(|t| t.starts_with("w.")).count();
    if w(m) == 0 && w(ch) > 0 {
        out.push("window-layer-silent".to_string());
    }
    let d = |v: &[String]| -> Vec<String> {
        v.iter()
            .filter(|t| t.starts_with("d."))
            .map(|t| ty(t).to_string())
            .collect()
    };
    let (dm, dc) = (d(m), d(ch));
    let mut types: Vec<&String> = dm.iter().chain(dc.iter()).collect();
    types.sort();
    types.dedup();
    for t in types {
        let (a, b) = (
            dm.iter().filter(|x| *x == t).count(),
            dc.iter().filter(|x| *x == t).count(),
        );
        if a == 0 && b > 0 {
            out.push(format!("missing-on-manuk:{t}"));
        } else if b == 0 && a > 0 {
            out.push(format!("extra-on-manuk:{t}"));
        } else if a != b && t != "selectionchange" {
            // selectionchange is queued and coalesced by a task in Chrome: its count is timing-dependent
            // (measured 2v1 and 1v1 on identical runs), so only presence/absence of it is a class.
            out.push(format!("count:{t}:{a}v{b}"));
        }
    }
    let pos = |v: &[String], t: &str| v.iter().position(|x| x == t);
    if let (Some(f), Some(fi)) = (pos(&dm, "focus"), pos(&dm, "focusin")) {
        if fi < f {
            out.push("order:focusin-before-focus".to_string());
        }
    }
    out
}

/// (action, class) pairs measured against chrome 145.0.7632.116; each class is a follow-up row.
const KNOWN: &[(&str, &str)] = &[
    ("click", "missing-on-manuk:focus"),
    ("click", "missing-on-manuk:focusin"),
    // W6.J2.d decision (type semantics, re-pinned on purpose): `type` on manuk replays a keystroke stream per char
    // (keydown, beforeinput, input, keyup, as a user typing would, so key handlers and per-char validators run);
    // the Chrome backend uses Input.insertText (one beforeinput+input, no key events), the fast agent path.
    // Neither is wrong; the divergence is the backend's shortcut and is pinned so a change in either is seen.
    ("type", "extra-on-manuk:keydown"),
    ("type", "extra-on-manuk:keyup"),
    ("type", "extra-on-manuk:select"),
    ("type", "extra-on-manuk:change"),
    ("type", "count:beforeinput:2v1"),
    ("type", "count:input:2v1"),
];

#[test]
fn event_sequences_match_chrome() {
    let url = serve();
    let mut bad = Vec::new();
    for act in [
        "click",
        "click-input",
        "type",
        "enter",
        "tab",
        "hover",
        "focus",
    ] {
        let m = run_action(false, &url, act);
        let ch = run_action(true, &url, act);
        let cl = classes(&m, &ch);
        if m == ch {
            println!("EVSEQ {act}: same ({} events)", m.len());
        } else {
            println!(
                "EVSEQ {act}: DIFF classes={cl:?}\n  manuk : {}\n  chrome: {}",
                m.join(" "),
                ch.join(" ")
            );
        }
        for c in &cl {
            if !KNOWN.contains(&(act, c.as_str())) {
                bad.push(format!("NEW {act}:{c}"));
            }
        }
        for (a, c) in KNOWN.iter().filter(|(a, _)| *a == act) {
            if !cl.iter().any(|x| x == c) {
                bad.push(format!("GONE {a}:{c}"));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "event-sequence classes changed vs the measured set: {bad:?}"
    );
}
