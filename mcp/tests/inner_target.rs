//! RG.S1.a.c gate: a click on a named wrapper whose pointer lands on an inner element must reach the
//! inner element's `mousedown` handler, as a real pointer does (MUI Select: the labelled combobox is the
//! outer InputBase, the handler that opens the list is on the inner select div). Fixture names no site.
//! `MANUK_RETARGET_MUTATE=1` dispatches at the outer node only (the mutant, red).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{json, Value};

const PAGE: &str = r#"<!doctype html><html><head><title>Sel</title></head><body>
<div role="combobox" aria-label="Pick time" style="display:block;padding:4px;border:1px solid #888;width:200px">
  <div id="inner" role="combobox" tabindex="0" style="display:block;padding:6px">1:00 PM</div>
</div>
<ul id="list" role="listbox" style="display:none"><li role="option">7:00 AM</li><li role="option">7:30 AM</li></ul>
<script>document.getElementById('inner').addEventListener('mousedown',function(e){
  if(e.button===0){e.preventDefault();document.getElementById('list').style.display='block';}});</script>
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
            cmd.env("MANUK_RETARGET_MUTATE", "1");
        }
        cmd.env(
            "MANUK_ACTLOG_DIR",
            std::env::temp_dir().join(format!("rgret-log-{}-{chrome}", std::process::id())),
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

fn click_outer(mutate: bool) -> String {
    let url = serve(PAGE);
    let mut c = Client::start(false, mutate);
    let nav = c.tool("navigate", json!({"url": url}));
    let h = handle_of(&nav, "Pick time");
    c.tool("act", json!({"handle": h, "action": "click"}))
}

#[test]
fn a_click_on_the_wrapper_reaches_the_inner_mousedown_handler() {
    let r = click_outer(false);
    assert!(r.contains("7:00 AM"), "the list must open: {r}");
}

#[test]
fn dispatching_at_the_wrapper_only_goes_red() {
    let r = click_outer(true);
    assert!(
        !r.contains("7:00 AM"),
        "mutant must leave the list shut: {r}"
    );
}
