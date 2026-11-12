//! RG.S1.b.b gate: a click on a listbox option scrolled out of its own `overflow-y:auto` list must
//! scroll the LIST to the option first (Playwright's scrollIntoViewIfNeeded), then click it. Before, the
//! pointer aimed at the option's unscrolled box and the handler of a different row ran (gocalendar's
//! start-time menu, 96 rows in a 200px list). Fixture names no site.
//! `MANUK_REVEAL_MUTATE=1` skips the container scroll (the mutant, red).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{json, Value};

const PAGE: &str = r#"<!doctype html><html><head><title>Lst</title></head><body>
<div id="menu" role="menu" style="overflow-y:scroll;height:200px;width:200px"></div>
<p id="out">none</p>
<script>var m=document.getElementById('menu');
for(var i=0;i<60;i++){var d=document.createElement('div');d.setAttribute('role','menuitem');
d.setAttribute('aria-label','Pick item '+i);d.style.cssText='padding:12px;height:17px';d.textContent='Item '+i;
d.addEventListener('click',(function(n){return function(){document.getElementById('out').textContent='picked '+n+(m.scrollTop>0?' scrolled':' unscrolled');};})(i));
m.appendChild(d);}</script>
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
            cmd.env("MANUK_REVEAL_MUTATE", "1");
        }
        cmd.env(
            "MANUK_ACTLOG_DIR",
            std::env::temp_dir().join(format!("rgrev-log-{}-{chrome}", std::process::id())),
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

fn click_item(mutate: bool) -> String {
    let url = serve(PAGE);
    let mut c = Client::start(false, mutate);
    let nav = c.tool("navigate", json!({"url": url}));
    let h = handle_of(&nav, "Pick item 45");
    let act = c.tool("act", json!({"handle": h, "action": "click"}));
    format!("{act}\n{}", c.tool("observe", json!({"mode": "full"})))
}

#[test]
fn a_click_on_an_option_scrolled_out_of_its_list_picks_that_option() {
    let r = click_item(false);
    assert!(
        r.contains("picked 45 scrolled"),
        "the named option must be picked: {r}"
    );
}

#[test]
fn skipping_the_container_scroll_goes_red() {
    let r = click_item(true);
    assert!(
        !r.contains("picked 45 scrolled"),
        "mutant must not scroll the list: {r}"
    );
}
