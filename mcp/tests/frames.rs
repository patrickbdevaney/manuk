//! W3.7 gate (Chrome backend): `observe` merges the documents of an iframe under `[hN] iframe <origin>`, same-site
//! and cross-site (out-of-process) alike, and `act type` reaches the child's field. The in-process engine has no
//! browsing contexts for frames, so it cannot be gated here. `MANUK_CDP_MUTATE=no-frames` drops the merge (red).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{json, Value};

const CARD: &str = r#"<!doctype html><html><body><label>Card number <input id="cc" name="cc"></label>
<button id="pay" onclick="document.getElementById('echo').textContent='paid '+document.getElementById('cc').value">Pay now</button>
<p id="echo"></p>
<script>document.getElementById('cc').addEventListener('input',e=>{document.getElementById('echo').textContent='typed '+e.target.value})</script>
</body></html>"#;
const SAME: &str = r#"<!doctype html><html><body><label>Promo code <input id="pc" name="pc"></label></body></html>"#;

/// Serves CARD at /card and SAME at /same on a fresh port; the parent page is built once both ports are known.
fn serve(f: impl Fn(&str) -> String + Send + 'static) -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            let mut buf = [0u8; 4096];
            let n = s.read(&mut buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]).to_string();
            let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
            let body = f(&path);
            let _ = write!(
                s,
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    port
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
        if std::env::var("FRAMES_BACKEND").as_deref() != Ok("manuk") {
            cmd.args(["--backend", "chrome"]);
        }
        if mutate {
            cmd.env("MANUK_CDP_MUTATE", "no-frames");
        }
        cmd.env(
            "MANUK_ACTLOG_DIR",
            std::env::temp_dir().join(format!("w37-log-{}-{mutate}", std::process::id())),
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

/// Parent on 127.0.0.1, card form on `localhost` (another site: out-of-process), promo form same-origin.
fn page(mutate: bool) -> (Client, String) {
    let child = serve(|p| {
        if p.starts_with("/card") {
            CARD.into()
        } else {
            String::new()
        }
    });
    let parent = serve(move |p| {
        if p.starts_with("/same") {
            SAME.into()
        } else {
            format!(
                "<!doctype html><html><head><title>Checkout</title></head><body><h1>Checkout</h1>\
                 <label>Email <input id=em></label>\
                 <iframe title=card src=\"http://localhost:{child}/card\" width=400 height=150></iframe>\
                 <iframe title=promo src=\"/same\" width=400 height=100></iframe></body></html>"
            )
        }
    });
    let mut c = Client::start(mutate);
    let nav = c.tool(
        "navigate",
        json!({"url": format!("http://127.0.0.1:{parent}/")}),
    );
    (c, nav)
}

#[test]
fn child_documents_nest_under_their_iframe_and_type_reaches_them() {
    let (mut c, _) = page(false);
    std::thread::sleep(std::time::Duration::from_millis(800));
    let obs = c.tool("observe", json!({}));
    assert!(
        obs.contains("iframe http://localhost:"),
        "cross-site frame label:\n{obs}"
    );
    assert!(
        obs.contains("iframe http://127.0.0.1:"),
        "same-site frame label:\n{obs}"
    );
    let card = handle_of(&obs, "Card number");
    let promo = handle_of(&obs, "Promo code");
    assert!(
        obs.lines()
            .find(|l| l.contains("Card number"))
            .unwrap()
            .contains("iframe http://localhost:"),
        "card field is nested under its frame:\n{obs}"
    );
    c.tool("type", json!({"handle": card, "text": "4242424242424242"}));
    c.tool("type", json!({"handle": promo, "text": "SAVE10"}));
    let obs = c.tool("observe", json!({}));
    assert!(
        obs.contains("4242424242424242"),
        "child field value not visible in the parent observation:\n{obs}"
    );
    assert!(
        obs.contains("typed 4242424242424242"),
        "the child page did not receive the input event:\n{obs}"
    );
    assert!(obs.contains("SAVE10"), "same-site frame field:\n{obs}");
    // The frame's button is clickable through the parent.
    let pay = handle_of(&obs, "Pay now");
    c.tool("act", json!({"handle": pay, "action": "click"}));
    let obs = c.tool("observe", json!({}));
    assert!(
        obs.contains("paid 4242424242424242"),
        "click inside the cross-site frame did not land:\n{obs}"
    );
}

#[test]
fn without_the_merge_the_card_field_is_invisible() {
    let (mut c, nav) = page(true);
    std::thread::sleep(std::time::Duration::from_millis(800));
    let obs = c.tool("observe", json!({}));
    assert!(
        !nav.contains("Card number") && !obs.contains("Card number"),
        "mutant must not see the frame:\n{obs}"
    );
}

/// W3.7.a: a frame whose origin is outside `--read-origins` keeps its `iframe <origin>` line but none of its children.
fn grant_run(read: &[&str], mutate: bool) -> String {
    let child = serve(|_| CARD.into());
    let parent = serve(move |_| {
        format!(
            "<!doctype html><html><head><title>Checkout</title></head><body><h1>Checkout</h1>\
             <iframe title=card src=\"http://localhost:{child}/card\" width=400 height=150></iframe></body></html>"
        )
    });
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"));
    cmd.args(["--backend", "chrome", "--read-origins"]);
    let mut set = vec![format!("http://127.0.0.1:{parent}")];
    set.extend(read.iter().map(|r| r.replace("CHILD", &child.to_string())));
    cmd.arg(set.join(","));
    if mutate {
        cmd.env("MANUK_CDP_MUTATE", "no-hide");
    }
    cmd.env(
        "MANUK_ACTLOG_DIR",
        std::env::temp_dir().join(format!(
            "w37a-{}-{}-{mutate}",
            std::process::id(),
            read.len()
        )),
    );
    let mut c = {
        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let tx = child.stdin.take().unwrap();
        let rx = BufReader::new(child.stdout.take().unwrap());
        Client {
            child,
            tx,
            rx,
            id: 0,
        }
    };
    c.rpc("initialize", json!({"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "t", "version": "0"}}));
    c.send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
    c.tool(
        "navigate",
        json!({"url": format!("http://127.0.0.1:{parent}/")}),
    );
    std::thread::sleep(std::time::Duration::from_millis(800));
    c.tool("observe", json!({}))
}

#[test]
fn frame_outside_read_origins_is_hidden_until_granted() {
    let out = grant_run(&[], false);
    assert!(
        out.contains("iframe http://localhost:"),
        "frame line kept:\n{out}"
    );
    assert!(!out.contains("Card number"), "child leaked:\n{out}");
    assert!(
        !out.contains("in region \"iframe"),
        "a child line under the frame:\n{out}"
    );
    let inn = grant_run(&["http://localhost:CHILD"], false);
    assert!(inn.contains("Card number"), "granted frame shown:\n{inn}");
}

#[test]
fn no_hide_mutant_leaks_the_frame() {
    let out = grant_run(&[], true);
    assert!(
        out.contains("in region \"iframe"),
        "mutant should leak (gate can go red):\n{out}"
    );
}
