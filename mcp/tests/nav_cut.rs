//! W5.T2.a.a.a gate: with MANUK_NAV_CUT=1 a navigated act result is the outcome line plus the URL
//! change plus a pointer to observe (<= 6 lines with the receipt trailer); without it the new page's body is present
//! (the RED arm: the cut removed). After the cut, observe still yields the new page's handles.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{json, Value};

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
                let page = if req.starts_with("GET /two") {
                    let items: String = (0..30)
                        .map(|i| format!("<p>Item {i} <a href=\"#i{i}\">Open {i}</a></p>"))
                        .collect();
                    format!("<!doctype html><title>Two</title><h1>Second</h1>{items}<button>Buy now</button>")
                } else {
                    "<!doctype html><title>One</title><h1>First</h1><a id=g href=\"/two\">Go two</a>".into()
                };
                let _ = write!(
                    s,
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{page}",
                    page.len()
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
    fn start(cut: bool) -> Client {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"));
        if cut {
            cmd.env("MANUK_NAV_CUT", "1");
        }
        cmd.env(
            "MANUK_ACTLOG_DIR",
            std::env::temp_dir().join(format!("w5t2aaa-log-{}-{cut}", std::process::id())),
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
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|b| b["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !res["isError"].as_bool().unwrap_or(false),
            "{name} failed: {text}"
        );
        text
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

fn handle_of(obs: &str, needle: &str) -> String {
    let l = obs
        .lines()
        .find(|l| l.contains(needle))
        .unwrap_or_else(|| panic!("no {needle} in\n{obs}"));
    let a = l.find("[h").unwrap();
    l[a + 1..l[a..].find(']').unwrap() + a].to_string()
}

fn navigate_act(cut: bool) -> (String, String) {
    let base = serve();
    let mut c = Client::start(cut);
    c.tool("navigate", json!({"url": format!("{base}/")}));
    let obs = c.tool("observe", json!({}));
    let g = handle_of(&obs, "Go two");
    let res = c.tool("act", json!({"handle": g, "action": "click"}));
    let after = c.tool("observe", json!({}));
    (res, after)
}

#[test]
fn navigated_act_is_cut_to_url_change_behind_flag() {
    let (full, _) = navigate_act(false);
    assert!(
        full.contains("navigated: ") && full.contains("Buy now"),
        "default keeps the body: {full}"
    );
    let (cut, after) = navigate_act(true);
    eprintln!(
        "W5T2aaa full_lines={} cut_lines={}\n{cut}",
        full.lines().count(),
        cut.lines().count()
    );
    assert!(
        cut.contains("navigated: ") && cut.contains("/two"),
        "URL change kept: {cut}"
    );
    // outcome + URL change + pointer, then the settle/writes/page receipt lines every act result ends with
    assert!(cut.lines().count() <= 6, "cut result is <= 6 lines: {cut}");
    assert!(
        cut.len() * 3 <= full.len(),
        "cut {} bytes must be <= 1/3 of full {}",
        cut.len(),
        full.len()
    );
    assert!(!cut.contains("Buy now"), "body omitted: {cut}");
    assert!(
        after.contains("Buy now"),
        "observe still reads the new page: {after}"
    );
}
