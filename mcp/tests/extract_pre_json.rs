//! RG.2.0.a.a.a.a.a gate (chrome_ver=145.0.7632.116): `extract` of a page that is one `<pre>` of pretty JSON
//! returns that JSON intact (parseable), even when the text repeats a word that is also the page's name.
//! Before the fix the label weaving inserted the clipped name of the `<pre>` itself into the middle of the
//! JSON (networkin-3 /finish: 'Join us for NetLink { "contactListDiff"...'), so the shim's state read
//! failed. Mutant `MANUK_EXTRACT_MUTATE=off` (no weaving) keeps it intact, so the test passes there too;
//! the RED proof is the pre-fix tree.

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
                let mut buf = [0u8; 8192];
                let _ = s.read(&mut buf);
                let json = serde_json::to_string_pretty(&json!({
                    "feedPostsDiff": {"added": [{
                        "author": {"name": "Div Garg", "details": "AGI Inc."},
                        "description": "Join us for NetLink Evening\n\nWhen: Thursday, 6:00\u{2013}8:30 PM.\nWho it's for: engineers.",
                        "interactions": 0
                    }], "removed": []}
                })).unwrap();
                let body = format!(
                    "<!doctype html><html><head><title>NetLink</title></head><body><button>Home</button><pre>{}</pre></body></html>",
                    json.replace('&', "&amp;").replace('<', "&lt;")
                );
                let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
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
    fn start(chrome: bool, env: &[(&str, &str)]) -> Client {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"));
        if chrome {
            cmd.args(["--backend", "chrome"]);
        }
        for (k, v) in env {
            cmd.env(k, v);
        }
        cmd.env(
            "MANUK_ACTLOG_DIR",
            std::env::temp_dir().join(format!("l4-log-{}-{chrome}", std::process::id())),
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
    fn call(&mut self, name: &str, args: Value) -> (bool, String) {
        let r = self.rpc("tools/call", json!({"name": name, "arguments": args}));
        let res = &r["result"];
        let blocks = res["content"].as_array().cloned().unwrap_or_default();
        (
            res["isError"].as_bool().unwrap_or(false),
            blocks
                .iter()
                .filter_map(|b| b["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n"),
        )
    }
    fn ok(&mut self, name: &str, args: Value) -> String {
        let (e, t) = self.call(name, args);
        assert!(!e, "{name} failed: {t}");
        t
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

fn read_state(env: &[(&str, &str)]) -> Result<Value, String> {
    let base = serve();
    let mut c = Client::start(false, env);
    c.ok("navigate", json!({"url": base}));
    let t = c.ok("extract", json!({}));
    let (a, b) = (t.find('{').unwrap(), t.rfind('}').unwrap());
    serde_json::from_str(&t[a..=b]).map_err(|e| format!("{e}: {t}"))
}

#[test]
fn extract_of_a_json_pre_stays_parseable() {
    let v = read_state(&[]).unwrap();
    assert_eq!(v["feedPostsDiff"]["added"][0]["interactions"], json!(0));
}
