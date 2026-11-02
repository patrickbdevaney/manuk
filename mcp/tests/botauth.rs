//! W3.19 gate: Web Bot Auth. With `--agent-key` + `--signature-agent`, every request a page load makes
//! (document, stylesheet, script, image, fetch POST, XHR, form POST) must verify under Cloudflare's own
//! `web-bot-auth` package run locally (scripts/botauth-verify), on BOTH backends; a tampered @authority
//! must be rejected, and an unsigned run must fail the same verifier (the gate can go red).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};

use base64::Engine;
use serde_json::{json, Value};

type Log = Arc<Mutex<Vec<Value>>>;

const PAGE: &str = r#"<!doctype html><html><head><title>WBA</title><link rel="stylesheet" href="/s.css"></head><body>
<h1>Signed page</h1>
<img src="/img.png">
<script src="/s.js"></script>
<script>
fetch("/fetch", {method: "POST", headers: {"content-type": "text/plain"}, body: "fetch-body"});
var x = new XMLHttpRequest(); x.open("POST", "/xhr"); x.setRequestHeader("content-type", "text/plain"); x.send("xhr-body");
</script>
<form method="post" action="/post"><input name="a" value="1"><button id="p">Post it</button></form>
</body></html>"#;

/// Records every request in full (headers and body) as the server received it.
fn serve() -> (String, Log) {
    let log: Log = Arc::default();
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    let lg = log.clone();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            let lg = lg.clone();
            std::thread::spawn(move || {
                let mut buf = Vec::new();
                let mut chunk = [0u8; 4096];
                let head_end = loop {
                    let n = s.read(&mut chunk).unwrap_or(0);
                    if n == 0 {
                        return; // a bare connect carries no request
                    }
                    buf.extend_from_slice(&chunk[..n]);
                    if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        break i + 4;
                    }
                };
                let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
                let mut lines = head.lines();
                let mut it = lines.next().unwrap_or("").split_whitespace();
                let (method, path) = (
                    it.next().unwrap_or("").to_string(),
                    it.next().unwrap_or("").to_string(),
                );
                let mut headers = serde_json::Map::new();
                for l in lines.filter(|l| !l.is_empty()) {
                    if let Some((k, v)) = l.split_once(':') {
                        headers.insert(k.trim().to_ascii_lowercase(), json!(v.trim()));
                    }
                }
                let want: usize = headers
                    .get("content-length")
                    .and_then(|v| v.as_str())
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                while buf.len() < head_end + want {
                    let n = s.read(&mut chunk).unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                }
                let body = &buf[head_end..];
                if path != "/favicon.ico" {
                    lg.lock().unwrap().push(json!({
                        "method": method, "path": path,
                        "host": headers.get("host").cloned().unwrap_or(json!("")),
                        "headers": headers,
                        "body_b64": base64::engine::general_purpose::STANDARD.encode(body),
                    }));
                }
                let (ctype, resp) = match path.split('?').next().unwrap() {
                    "/s.css" => ("text/css", "body{color:#123}"),
                    "/s.js" => ("text/javascript", "window.__s=1;"),
                    "/img.png" => ("image/png", ""),
                    "/xhr" | "/post" | "/fetch" => ("text/plain", "ok"),
                    _ => ("text/html", PAGE),
                };
                let _ = write!(
                    s,
                    "HTTP/1.1 200 OK\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{resp}",
                    resp.len()
                );
            });
        }
    });
    (format!("http://{addr}"), log)
}

struct Client {
    child: Child,
    tx: ChildStdin,
    rx: BufReader<ChildStdout>,
    id: u64,
}

impl Client {
    fn start(chrome: bool, flags: &[String], mutate: Option<&str>) -> Client {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"));
        cmd.args(flags);
        if let Some(m) = mutate {
            cmd.env("MANUK_CDP_MUTATE", m);
        }
        if chrome {
            cmd.args(["--backend", "chrome"]);
        }
        cmd.env(
            "MANUK_ACTLOG_DIR",
            std::env::temp_dir().join(format!("w39-log-{}-{chrome}", std::process::id())),
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
fn handle_of(text: &str, needle: &str) -> String {
    let line = text
        .lines()
        .find(|l| l.contains(needle))
        .unwrap_or_else(|| panic!("no {needle:?} in:\n{text}"));
    let s = line.find("[h").unwrap();
    let e = line[s..].find(']').unwrap() + s;
    line[s + 1..e].to_string()
}

// RFC 9421 Appendix B.1.4 test key.
const JWK: &str = r#"{"kty":"OKP","crv":"Ed25519","alg":"EdDSA","kid":"test-key-ed25519","d":"n4Ni-HpISpVObnQMW0wOhCKROaIKqKtW_2ZYb2p9KcU","x":"JrQLj5P_89iXES9-vFgrIy29clF9CC_oPPsw3c5D0bs"}"#;
const AGENT: &str = "https://agent.test";

fn work_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("w319-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Load the page, fire the form POST, and hand back the node verifier's (exit ok, stdout) over what the
/// server saw. `sign=false` runs the same session with no key (the red run).
fn run(chrome: bool, sign: bool, mutate: Option<&str>) -> (bool, String, Vec<Value>) {
    let dir = work_dir(&format!("{chrome}-{sign}-{}", mutate.unwrap_or("")));
    let keyfile = dir.join("key.jwk");
    std::fs::write(&keyfile, JWK).unwrap();
    let (app, log) = serve();
    let mut flags = vec![];
    if sign {
        flags = vec![
            "--agent-key".to_string(),
            keyfile.display().to_string(),
            "--signature-agent".into(),
            AGENT.into(),
        ];
    }
    let mut c = Client::start(chrome, &flags, mutate);
    let nav = c.ok("navigate", json!({"url": format!("{app}/")}));
    std::thread::sleep(std::time::Duration::from_millis(1500));
    c.ok(
        "act",
        json!({"handle": handle_of(&nav, "button \"Post it\""), "action": "click"}),
    );
    std::thread::sleep(std::time::Duration::from_millis(1000));
    drop(c);
    let reqs = log.lock().unwrap().clone();
    std::fs::write(
        dir.join("reqs.json"),
        Value::Array(reqs.clone()).to_string(),
    )
    .unwrap();
    // The key directory the operator would host, produced by the shipped binary.
    let out = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"))
        .arg("--agent-key")
        .arg(&keyfile)
        .args(["--signature-agent", AGENT, "--print-key-directory"])
        .output()
        .unwrap();
    assert!(out.status.success(), "--print-key-directory failed");
    std::fs::write(dir.join("dir.json"), &out.stdout).unwrap();
    let vdir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/botauth-verify");
    if !vdir.join("node_modules").exists() {
        let st = Command::new("npm")
            .args(["ci", "--silent"])
            .current_dir(&vdir)
            .status()
            .unwrap();
        assert!(st.success(), "npm ci in scripts/botauth-verify");
    }
    let v = Command::new("node")
        .arg(vdir.join("verify.mjs"))
        .arg(dir.join("reqs.json"))
        .arg(dir.join("dir.json"))
        .output()
        .unwrap();
    (
        v.status.success(),
        String::from_utf8_lossy(&v.stdout).to_string(),
        reqs,
    )
}

fn all_verify(chrome: bool) {
    let (ok, out, reqs) = run(chrome, true, None);
    let v: Value =
        serde_json::from_str(out.trim()).unwrap_or_else(|_| panic!("verifier said: {out}"));
    eprintln!("W3.19 chrome={chrome}: {out}");
    let paths: Vec<_> = reqs
        .iter()
        .map(|r| {
            format!(
                "{} {}",
                r["method"].as_str().unwrap(),
                r["path"].as_str().unwrap()
            )
        })
        .collect();
    for want in [
        "GET /",
        "GET /s.css",
        "GET /s.js",
        "GET /img.png",
        "POST /fetch",
        "POST /xhr",
        "POST /post",
    ] {
        assert!(
            paths.iter().any(|p| p == want),
            "server never saw {want}: {paths:?}"
        );
    }
    assert!(ok, "verifier rejected a request: {out}");
    assert_eq!(
        v["verified"], v["total"],
        "not every request verified: {out}"
    );
    assert!(
        v["bodies"].as_u64().unwrap() >= 3,
        "bodies not exercised: {out}"
    );
    assert_eq!(
        v["digest_ok"], v["bodies"],
        "Content-Digest mismatch: {out}"
    );
    assert_eq!(
        v["tamper_rejected"], v["total"],
        "a tampered @authority was accepted: {out}"
    );
}

#[test]
fn every_request_verifies_on_the_manuk_backend() {
    all_verify(false);
}

#[test]
fn every_request_verifies_on_the_chrome_backend() {
    all_verify(true);
}

#[test]
fn an_unsigned_manuk_run_fails_the_verifier() {
    let (ok, out, reqs) = run(false, false, None);
    assert!(!reqs.is_empty());
    assert!(!ok, "unsigned requests passed the verifier: {out}");
}

#[test]
fn a_chrome_run_with_signing_disabled_fails_the_verifier() {
    let (ok, out, reqs) = run(true, true, Some("no-sign"));
    assert!(!reqs.is_empty());
    assert!(!ok, "unsigned requests passed the verifier: {out}");
}
