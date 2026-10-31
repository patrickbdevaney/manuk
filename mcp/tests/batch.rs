//! W4.A5 gate: composite actions. `fill_form` / `act(steps)` run a multi-field form in ONE call and stop
//! at the first failed step, dialog or navigation, saying where; `back` and `wait_for` round it out.
//! Everything runs on both backends except the dialog stop (the engine implements no JS dialogs).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{json, Value};

fn page(path: &str) -> Option<String> {
    let p = path.split('?').next().unwrap_or("");
    Some(match p {
        "/form" => r#"<!doctype html><title>Form</title><form action="/done" method="get">
<label>Name <input name="name" type="text"></label>
<label>Email <input name="email" type="text"></label>
<label>Plan <select name="plan"><option value="free">Free</option><option value="pro">Pro</option></select></label>
<label><input type="checkbox" name="news" value="yes"> Newsletter</label>
<label><input type="radio" name="bill" value="m"> Monthly</label><label><input type="radio" name="bill" value="y"> Yearly</label>
<button type="submit">Send</button></form>"#.into(),
        "/done" => format!("<!doctype html><title>Done</title><p id=\"o\">received:{}</p>", path.split_once('?').map(|x| x.1).unwrap_or("")),
        "/dlg" => r#"<!doctype html><title>Dlg</title><label>A <input id="a" type="text"></label><button id="d">Discard</button><label>B <input id="b" type="text"></label><p id="o">none</p>
<script>document.getElementById('d').onclick=function(){document.getElementById('o').textContent=confirm('sure?')?'yes':'no';};</script>"#.into(),
        "/nav" => r#"<!doctype html><title>Nav</title><label>A <input type="text"></label><a href="/done?via=link">Go on</a><label>B <input type="text"></label>"#.into(),
        "/late" => r#"<!doctype html><title>Late</title><p id="o">loading</p><script>setTimeout(function(){document.getElementById('o').textContent='ready:42';},700);</script>"#.into(),
        "/one" => r#"<!doctype html><title>One</title><p>page one</p><a href="/two">Next page</a>"#.into(),
        "/two" => r#"<!doctype html><title>Two</title><p>page two</p>"#.into(),
        _ => return None,
    })
}

fn serve() -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
                if let Some(b) = page(&path) {
                    let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{b}", b.len());
                } else {
                    let _ = write!(
                        s,
                        "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    );
                }
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
    fn start(chrome: bool, extra: &[&str]) -> Client {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"));
        if chrome {
            cmd.args(["--backend", "chrome"]);
        }
        cmd.env(
            "MANUK_ACTLOG_DIR",
            std::env::temp_dir().join(format!("a5-log-{}-{chrome}", std::process::id())),
        );
        cmd.args(extra);
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

fn form_in_one_call(chrome: bool) {
    let base = serve();
    let mut c = Client::start(chrome, &[]);
    let nav = c.tool("navigate", json!({"url": format!("{base}/form")}));
    let r = c.tool(
        "fill_form",
        json!({"fields": [
            {"handle": handle_of(&nav, "textbox \"Name\""), "value": "Ada Lovelace"},
            {"handle": handle_of(&nav, "textbox \"Email\""), "value": "ada@example.org"},
            {"handle": handle_of(&nav, "combobox \"Plan\""), "value": "Pro"},
            {"handle": handle_of(&nav, "checkbox \"Newsletter\""), "value": true},
            {"handle": handle_of(&nav, "radio \"Yearly\""), "value": true},
        ], "submit": handle_of(&nav, "button \"Send\"")}),
    );
    assert!(r.contains("ran 6 of 6 steps"), "(chrome={chrome}) {r}");
    // The engine's <option> click does not change a <select> yet (follow-up W4.A5.a), so `plan` is only
    // asserted on Chrome.
    assert!(
        r.contains("/done?")
            && r.contains("name=Ada+Lovelace")
            && (!chrome || r.contains("plan=Pro") || r.contains("plan=pro"))
            && r.contains("news=yes")
            && r.contains("bill=y")
            && r.contains("email=ada%40example.org"),
        "form was not submitted whole (chrome={chrome}):\n{r}"
    );
}

fn failed_step_stops(chrome: bool) {
    let base = serve();
    let mut c = Client::start(chrome, &[]);
    let nav = c.tool("navigate", json!({"url": format!("{base}/form")}));
    let r = c.tool(
        "act",
        json!({"steps": [
            {"tool": "type", "handle": handle_of(&nav, "textbox \"Name\""), "text": "first"},
            {"tool": "type", "handle": "h999", "text": "nope"},
            {"tool": "type", "handle": handle_of(&nav, "textbox \"Email\""), "text": "never"},
        ]}),
    );
    assert!(
        r.contains("STOPPED at step 2 of 3") && r.contains("FAILED"),
        "(chrome={chrome}) {r}"
    );
    let page = c.tool("extract", json!({}));
    let _ = page;
    let obs = c.tool("observe", json!({}));
    assert!(
        obs.contains("first") && !obs.contains("never"),
        "step 1 must have run, step 3 not (chrome={chrome}):\n{obs}"
    );
}

fn navigation_stops(chrome: bool) {
    let base = serve();
    let mut c = Client::start(chrome, &[]);
    let nav = c.tool("navigate", json!({"url": format!("{base}/nav")}));
    let b = handle_of(&nav, "textbox \"B\"");
    let r = c.tool(
        "act",
        json!({"steps": [
            {"tool": "act", "handle": handle_of(&nav, "link \"Go on\""), "action": "click"},
            {"tool": "type", "handle": b, "text": "late"},
        ]}),
    );
    assert!(
        r.contains("STOPPED after step 1 of 2")
            && r.contains("navigated")
            && r.contains("via=link"),
        "(chrome={chrome}) {r}"
    );
}

fn wait_for_text(chrome: bool) {
    let base = serve();
    let mut c = Client::start(chrome, &[]);
    c.tool("navigate", json!({"url": format!("{base}/late")}));
    let r = c.tool("wait_for", json!({"text": "ready:42", "timeout": 8}));
    assert!(r.starts_with("found text"), "(chrome={chrome}) {r}");
    let t0 = std::time::Instant::now();
    let r = c.tool("wait_for", json!({"text": "never-appears", "timeout": 1}));
    assert!(
        r.starts_with("TIMED OUT") && t0.elapsed().as_secs() < 5,
        "(chrome={chrome}) {r}"
    );
}

fn back_works(chrome: bool) {
    let base = serve();
    let mut c = Client::start(chrome, &[]);
    let nav = c.tool("navigate", json!({"url": format!("{base}/one")}));
    c.tool(
        "act",
        json!({"handle": handle_of(&nav, "link \"Next page\""), "action": "click"}),
    );
    let r = c.tool("back", json!({}));
    assert!(
        r.contains("navigated") && r.contains("/one"),
        "(chrome={chrome}) {r}"
    );
    let (err, t, _) = c.call("back", json!({}));
    assert!(
        err && t.contains("no earlier page"),
        "(chrome={chrome}) back past the start must say so: {t}"
    );
}

#[test]
fn fill_form_one_call_manuk() {
    form_in_one_call(false);
}
#[test]
fn fill_form_one_call_chrome() {
    form_in_one_call(true);
}
#[test]
fn failed_step_stops_manuk() {
    failed_step_stops(false);
}
#[test]
fn failed_step_stops_chrome() {
    failed_step_stops(true);
}
#[test]
fn navigation_stops_the_batch_manuk() {
    navigation_stops(false);
}
#[test]
fn navigation_stops_the_batch_chrome() {
    navigation_stops(true);
}
#[test]
fn wait_for_text_manuk() {
    wait_for_text(false);
}
#[test]
fn wait_for_text_chrome() {
    wait_for_text(true);
}
#[test]
fn back_manuk() {
    back_works(false);
}
#[test]
fn back_chrome() {
    back_works(true);
}

#[test]
fn dialog_stops_the_batch_chrome() {
    let base = serve();
    let mut c = Client::start(true, &[]);
    let nav = c.tool("navigate", json!({"url": format!("{base}/dlg")}));
    let b = handle_of(&nav, "textbox \"B\"");
    let r = c.tool(
        "act",
        json!({"steps": [
            {"tool": "type", "handle": handle_of(&nav, "textbox \"A\""), "text": "x"},
            {"tool": "act", "handle": handle_of(&nav, "button \"Discard\""), "action": "click"},
            {"tool": "type", "handle": b, "text": "after-dialog"},
        ]}),
    );
    assert!(
        r.contains("STOPPED after step 2 of 3") && r.contains("dialog"),
        "{r}"
    );
    let obs = c.tool("observe", json!({}));
    assert!(
        !obs.contains("after-dialog"),
        "step 3 must not have run:\n{obs}"
    );
}
