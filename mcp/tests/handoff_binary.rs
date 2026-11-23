//! R3.3.a gate: the SHIPPED `manuk-mcp` binary has a human surface when `MANUK_HANDOFF_SCRIPT=<file>` names
//! a scripted human (test-only: its text is JS run in the live page). A `handoff` call over JSON-RPC then
//! returns the effect diff; without the env var it still errors rather than pretending.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{json, Value};

struct Client {
    child: Child,
    tx: ChildStdin,
    rx: BufReader<ChildStdout>,
    id: u64,
}

impl Client {
    fn start(script: Option<&std::path::Path>) -> Client {
        Client::start_env(script, None)
    }
    fn start_env(script: Option<&std::path::Path>, sock: Option<&std::path::Path>) -> Client {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"));
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        match script {
            Some(p) => cmd.env("MANUK_HANDOFF_SCRIPT", p),
            None => cmd.env_remove("MANUK_HANDOFF_SCRIPT"),
        };
        match sock {
            Some(p) => cmd.env("MANUK_HANDOFF_SOCKET", p),
            None => cmd.env_remove("MANUK_HANDOFF_SOCKET"),
        };
        let mut child = cmd.spawn().expect("spawn manuk-mcp");
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
    /// (is_error, text)
    fn tool(&mut self, name: &str, args: Value) -> (bool, String) {
        let r = self.rpc("tools/call", json!({"name": name, "arguments": args}));
        let text = r["result"]["content"]
            .as_array()
            .map(|c| {
                c.iter()
                    .filter_map(|b| b["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();
        (r["result"]["isError"] == json!(true), text)
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

fn url() -> String {
    format!(
        "file://{}/tests/fixtures/handoff_gate.html",
        env!("CARGO_MANIFEST_DIR")
    )
}

fn script(name: &str, body: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("manuk-handoff-{}-{name}", std::process::id()));
    std::fs::write(&p, body).unwrap();
    p
}

#[test]
fn handoff_works_over_stdio_with_a_scripted_human() {
    let p = script("login.js", "signin()");
    let mut c = Client::start(Some(&p));
    c.tool("navigate", json!({"url": url()}));
    let (err, r) = c.tool("handoff", json!({"reason": "log in", "timeout_ms": 5000}));
    assert!(!err, "{r}");
    assert!(r.contains("resumed"), "{r}");
    assert!(
        r.contains("Welcome back, Pat"),
        "diff must carry the human's change: {r}"
    );
    let (_, o) = c.tool("observe", json!({}));
    assert!(o.contains("Account settings"), "{o}");

    let p = script("timeout.js", "//timeout\nsignin()");
    let mut c = Client::start(Some(&p));
    c.tool("navigate", json!({"url": url()}));
    let (_, r) = c.tool("handoff", json!({"reason": "x", "timeout_ms": 50}));
    assert!(r.contains("TIMED OUT after 50 ms"), "{r}");
}

#[test]
fn without_the_env_handoff_still_errors() {
    let mut c = Client::start(None);
    c.tool("navigate", json!({"url": url()}));
    let (err, r) = c.tool("handoff", json!({"reason": "x"}));
    assert!(err && r.contains("no human surface"), "{r}");
}

/// R3.3.a.a: a shell-side surface stub over a unix socket. The shell is a separate process from the
/// stdio server; the human's actions arrive as `eval` lines, Resume as `resume`.
#[cfg(unix)]
mod socket {
    use super::*;
    use std::os::unix::net::UnixListener;

    fn sock_path(n: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("manuk-hs-{}-{n}", std::process::id()));
        let _ = std::fs::remove_file(&d);
        d
    }

    /// `act`: what the stub human does after the handoff arrives (None = stays silent).
    fn stub(path: &std::path::Path, act: Option<&'static str>) -> std::thread::JoinHandle<String> {
        let l = UnixListener::bind(path).unwrap();
        std::thread::spawn(move || {
            let (s, _) = l.accept().unwrap();
            let mut rd = BufReader::new(s.try_clone().unwrap());
            let mut line = String::new();
            rd.read_line(&mut line).unwrap();
            let mut w = s;
            if let Some(js) = act {
                writeln!(w, "{}", json!({"type": "eval", "js": js})).unwrap();
                writeln!(w, "{}", json!({"type": "resume"})).unwrap();
            }
            std::thread::sleep(std::time::Duration::from_millis(1500));
            line
        })
    }

    #[test]
    fn handoff_forwards_to_a_shell_over_the_socket() {
        let p = sock_path("a");
        let h = stub(&p, Some("signin()"));
        let mut c = Client::start_env(None, Some(&p));
        c.tool("navigate", json!({"url": url()}));
        let (err, r) = c.tool("handoff", json!({"reason": "log in", "timeout_ms": 5000}));
        assert!(!err, "{r}");
        assert!(r.contains("resumed"), "{r}");
        assert!(
            r.contains("Welcome back, Pat"),
            "diff must carry the human's change: {r}"
        );
        let got = h.join().unwrap();
        assert!(
            got.contains("\"reason\":\"log in\"") && got.contains("\"type\":\"handoff\""),
            "{got}"
        );
    }

    #[test]
    fn a_silent_shell_times_out() {
        let p = sock_path("b");
        let _h = stub(&p, None);
        let mut c = Client::start_env(None, Some(&p));
        c.tool("navigate", json!({"url": url()}));
        let (_, r) = c.tool("handoff", json!({"reason": "x", "timeout_ms": 200}));
        assert!(r.contains("TIMED OUT after 200 ms"), "{r}");
    }

    #[test]
    fn no_shell_listening_fails_loudly_and_never_pretends() {
        let p = sock_path("c"); // nothing bound
        let out = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"))
            .env("MANUK_HANDOFF_SOCKET", &p)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(!out.status.success());
        assert!(String::from_utf8_lossy(&out.stderr).contains("no shell listening"));
    }
}
