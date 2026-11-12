//! RG.S1.c.b.a.a.a gate (chrome_ver=145.0.7632.116): observe full carries a one-line CAROUSEL notice with the
//! count when an overflow:hidden track holds aria-hidden="true" slides, and none on a page without one.
//! Mutant `MANUK_CAROUSEL_MUTATE=always` (notice on every page) turns the no-carousel case RED.

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
                let n = s.read(&mut buf).unwrap_or(0);
                let first = String::from_utf8_lossy(&buf[..n]).lines().next().unwrap_or("").to_string();
                let path = first.split(' ').nth(1).unwrap_or("/").to_string();
                let body = page(&path);
                let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            });
        }
    });
    format!("http://{addr}")
}

fn track(style: &str, hidden: usize, open: usize) -> String {
    let mut b = format!("<!doctype html><html><head><title>T</title></head><body><h1>Home</h1><div style=\"{style}\"><ul style=\"display:flex;width:3000px;margin:0;padding:0\">");
    for i in 0..open {
        b.push_str(&format!("<li aria-hidden=\"false\" style=\"width:200px\"><a href=\"/r{i}\">Open {i}</a></li>"));
    }
    for i in 0..hidden {
        b.push_str(&format!("<li aria-hidden=\"true\" style=\"width:200px\"><a href=\"/h{i}\">Away {i}</a></li>"));
    }
    b.push_str("</ul></div><button>Next</button></body></html>");
    b
}

fn page(path: &str) -> String {
    match path {
        "/clipped" => track("overflow:hidden;width:600px", 5, 1),
        "/open" => track("width:600px", 5, 1),
        "/plain" => track("overflow:hidden;width:600px", 0, 6),
        _ => "<html><body>x</body></html>".into(),
    }
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
            std::env::temp_dir().join(format!("s1cbaaa-log-{}-{chrome}", std::process::id())),
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

fn observe_of(env: &[(&str, &str)], path: &str) -> String {
    let base = serve();
    let mut c = Client::start(false, env);
    c.ok("navigate", json!({"url": format!("{base}{path}")}));
    c.ok("observe", json!({"mode": "full"}))
}

#[test]
fn a_clipped_track_with_aria_hidden_slides_gets_a_counted_notice() {
    let obs = observe_of(&[], "/clipped");
    assert!(obs.contains("CAROUSEL: 5 slides"), "no notice with count 5:\n{obs}");
    assert!(!obs.contains("Away 0"), "notice must not carry page text:\n{obs}");
}

#[test]
fn no_clipped_aria_hidden_track_means_no_notice() {
    for p in ["/open", "/plain"] {
        let obs = observe_of(&[], p);
        assert!(!obs.contains("CAROUSEL"), "{p}: spurious notice:\n{obs}");
    }
}

#[test]
fn mutant_notice_always_on_goes_red() {
    let obs = observe_of(&[("MANUK_CAROUSEL_MUTATE", "always")], "/plain");
    assert!(obs.contains("CAROUSEL"), "mutant should emit the notice");
}
