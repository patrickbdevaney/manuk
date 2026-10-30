//! W4.A1 gate: `type` must type like a user. A controlled input (React-style: its own state, updated
//! only from `input` events, written back into the field on every render) keeps typed text on BOTH
//! backends. The old engine path set only the `value` attribute and fired nothing, so the state
//! stayed empty and the next render erased the text.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{json, Value};

const PAGE: &str = r#"<!doctype html><html><head><title>Controlled</title></head><body>
<label>Phone <input id="p" type="text"></label>
<button id="save">Save</button>
<p id="o">state:;</p>
<script>
var state = '', log = [];
var el = document.getElementById('p');
function render() {
  if (el.value !== state) el.value = state;
  document.getElementById('o').textContent = 'state:' + state + ';keys=' + log.length;
}
el.addEventListener('keydown', function () { log.push('k'); });
el.addEventListener('input', function (e) { state = e.target.value; render(); });
el.addEventListener('change', function () { document.title = 'changed'; });
setInterval(render, 40);
</script></body></html>"#;

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

fn run(chrome: bool) {
    let url = serve();
    let mut c = Client::start(chrome);
    let nav = c.tool("navigate", json!({"url": url}));
    let p = handle_of(&nav, "textbox \"Phone\"");
    c.tool("type", json!({"handle": p, "text": "5551234"}));
    // Several render passes later the field (re-rendered from the page's own state) must still hold
    // the number: an input the page never heard about is reset to its empty state.
    std::thread::sleep(std::time::Duration::from_millis(300));
    let seen = c.tool("observe", json!({}));
    assert!(
        seen.contains("=5551234"),
        "controlled input lost the typed text (chrome={chrome}):\n{seen}",
    );
    // Retyping replaces, it does not append.
    c.tool("type", json!({"handle": p, "text": "42"}));
    std::thread::sleep(std::time::Duration::from_millis(300));
    let seen = c.tool("observe", json!({}));
    assert!(
        seen.contains("=42") && !seen.contains("5551234"),
        "retype (chrome={chrome}):\n{seen}"
    );
}

#[test]
fn controlled_input_keeps_typed_text_on_the_manuk_backend() {
    run(false);
}

#[test]
fn controlled_input_keeps_typed_text_on_the_chrome_backend() {
    run(true);
}
