//! R4.1 gate: the Chrome (CDP) backend of manuk-mcp, driven over real stdio on local fixtures.
//! Taint (hidden text withheld), dispatch-time hit-test re-validation, and re-mount identity.
//! `MANUK_CDP_MUTATE=<no-taint|no-revalidate|no-remount>` switches one discipline off; each test has
//! a twin that must go red under its mutation (run by `scripts`-free `cargo test` env, see bottom).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{json, Value};

struct Client {
    child: Child,
    tx: ChildStdin,
    rx: BufReader<ChildStdout>,
    id: u64,
}

impl Client {
    fn start(mutate: Option<&str>) -> Client {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"));
        cmd.args(["--backend", "chrome"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        if let Some(m) = mutate {
            cmd.env("MANUK_CDP_MUTATE", m);
        }
        let mut child = cmd.spawn().expect("spawn manuk-mcp --backend chrome");
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
    fn tool(&mut self, name: &str, args: Value) -> (bool, String) {
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
        (res["isError"].as_bool().unwrap_or(false), text)
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

fn fixture(name: &str) -> String {
    format!(
        "file://{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    )
}

fn handle_of(text: &str, needle: &str) -> String {
    let line = text
        .lines()
        .find(|l| l.contains(needle))
        .unwrap_or_else(|| panic!("no line with {needle:?} in:\n{text}"));
    let start = line.find("[h").unwrap();
    let end = line[start..].find(']').unwrap() + start;
    line[start + 1..end].to_string()
}

fn taint(mutate: Option<&str>) -> (String, String) {
    let mut c = Client::start(mutate);
    let (err, nav) = c.tool("navigate", json!({"url": fixture("injection.html")}));
    assert!(!err, "{nav}");
    let (_, ex) = c.tool("extract", json!({}));
    (nav, ex)
}

#[test]
fn taint_withholds_what_a_human_cannot_see() {
    if std::env::var("MANUK_CDP_MUTATE").is_ok() {
        return;
    }
    let (nav, ex) = taint(None);
    assert!(nav.contains("⚠hidden-text"), "no warning marker:\n{nav}");
    for visible in [
        "Revenue grew 12 percent",
        "Margins improved",
        "VISIBLE_OVERFLOW",
    ] {
        assert!(
            ex.contains(visible),
            "visible text {visible:?} was withheld:\n{ex}"
        );
    }
    for hidden in [
        "HIDDEN_OPACITY",
        "HIDDEN_VISIBILITY",
        "HIDDEN_OFFSCREEN",
        "HIDDEN_TINY",
        "HIDDEN_CONTRAST",
        "HIDDEN_ZERO",
        "HIDDEN_DARKCONTRAST",
        "HIDDEN_DISPLAY",
    ] {
        assert!(
            !ex.contains(hidden) && !nav.contains(hidden),
            "{hidden} leaked:\n{ex}\n{nav}"
        );
    }
}

/// Twin: with the taint layer off the hidden text must leak (proves the test above can go red).
#[test]
fn taint_mutant_leaks_hidden_text() {
    let (nav, ex) = taint(Some("no-taint"));
    assert!(
        ex.contains("HIDDEN_OPACITY") || nav.contains("HIDDEN_OPACITY"),
        "mutant did not leak:\n{ex}"
    );
}

fn late(mutate: Option<&str>) -> (bool, String, bool, String) {
    let mut c = Client::start(mutate);
    let (err, nav) = c.tool("navigate", json!({"url": fixture("cdp_late.html")}));
    assert!(!err, "{nav}");
    let pay = handle_of(&nav, "button \"Pay now\"");
    let save = handle_of(&nav, "button \"Save\"");
    std::thread::sleep(std::time::Duration::from_millis(2600)); // the page re-renders Save and lands an overlay
    let (pay_err, pay_txt) = c.tool("act", json!({"handle": pay, "action": "click"}));
    let (save_err, save_txt) = c.tool("act", json!({"handle": save, "action": "click"}));
    (pay_err, pay_txt, save_err, save_txt)
}

#[test]
fn dispatch_revalidation_refuses_a_click_under_a_late_overlay() {
    if std::env::var("MANUK_CDP_MUTATE").is_ok() {
        return;
    }
    let (pay_err, pay_txt, _, _) = late(None);
    assert!(
        pay_err && pay_txt.contains("OCCLUDED at dispatch"),
        "expected a dispatch-time refusal, got err={pay_err}: {pay_txt}"
    );
}

#[test]
fn revalidation_mutant_dispatches_into_the_overlay() {
    let (pay_err, pay_txt, _, _) = late(Some("no-revalidate"));
    assert!(
        !pay_err,
        "mutant should have clicked blindly, got: {pay_txt}"
    );
}

/// Remount: the overlay also covers Save, so this test uses a page where only the re-render happens.
fn remount(mutate: Option<&str>) -> (bool, String) {
    let mut c = Client::start(mutate);
    let (err, nav) = c.tool("navigate", json!({"url": fixture("cdp_remount.html")}));
    assert!(!err, "{nav}");
    let save = handle_of(&nav, "button \"Save\"");
    std::thread::sleep(std::time::Duration::from_millis(2600));
    c.tool("act", json!({"handle": save, "action": "click"}))
}

#[test]
fn a_handle_survives_the_page_replacing_its_element() {
    if std::env::var("MANUK_CDP_MUTATE").is_ok() {
        return;
    }
    let (err, txt) = remount(None);
    assert!(!err, "{txt}");
    assert!(
        txt.contains("saved-new"),
        "the NEW button was not clicked: {txt}"
    );
    assert!(txt.contains("re-mounted"), "no re-mount evidence: {txt}");
}

#[test]
fn remount_mutant_goes_stale() {
    let (err, txt) = remount(Some("no-remount"));
    assert!(
        err && txt.contains("STALE"),
        "mutant should report a stale handle, got err={err}: {txt}"
    );
}

const POSTER: &str = r#"<!doctype html><html><body><button onclick="fetch('/add',{method:'POST',body:'x'}).then(()=>{document.getElementById('o').textContent='posted'})">Add</button><p id="o">idle</p></body></html>"#;

/// A loopback server: any request gets the poster page (a POST gets the same, which is enough).
fn poster() -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            std::thread::spawn(move || {
                let mut s = s;
                let mut buf = [0u8; 4096];
                let _ = s.read(&mut buf);
                let b = POSTER.as_bytes();
                let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", b.len());
                let _ = s.write_all(b);
            });
        }
    });
    format!("http://{addr}/")
}

/// Navigate to a page, POST from it, checkpoint, restore: returns (checkpoint text, restore text).
fn ckpt_after_post(mutate: Option<&str>) -> (String, String) {
    let mut c = Client::start(mutate);
    let (e, txt) = c.tool("navigate", json!({"url": poster()}));
    assert!(!e, "{txt}");
    let h = handle_of(&txt, "Add");
    let (e, txt) = c.tool("act", json!({"handle": h, "action": "click"}));
    assert!(!e, "{txt}");
    let (e, cp) = c.tool("checkpoint", json!({}));
    assert!(!e, "{cp}");
    let id = cp.split_whitespace().nth(1).unwrap().to_string();
    let (e, rs) = c.tool("restore", json!({"checkpoint": id}));
    assert!(!e, "{rs}");
    (cp, rs)
}

#[test]
fn chrome_checkpoint_after_a_post_does_not_claim_client() {
    if std::env::var("MANUK_CDP_MUTATE").is_ok() {
        return;
    }
    let (cp, rs) = ckpt_after_post(None);
    assert!(!cp.contains("[CLIENT]") && cp.contains("[UNKNOWN]"), "{cp}");
    assert!(!rs.contains("[CLIENT]") && rs.contains("[UNKNOWN]"), "{rs}");
    assert!(rs.contains("divergence unknown"), "{rs}");
}

#[test]
fn claim_client_mutant_is_red() {
    let (cp, _) = ckpt_after_post(Some("claim-client"));
    assert!(
        cp.contains("[CLIENT]"),
        "mutant should reproduce the CLIENT lie: {cp}"
    );
}

// ---- CB.SANDBOX: the Chrome we launch is sandboxed and is the pinned build ----

struct Probe {
    browser_cmdline: String,
    renderer_seccomp: Vec<String>,
    renderer_nnp: Vec<String>,
    version: String,
}

fn status_field(pid: &str, key: &str) -> String {
    std::fs::read_to_string(format!("/proc/{pid}/status"))
        .unwrap_or_default()
        .lines()
        .find_map(|l| l.strip_prefix(key).map(|v| v.trim_start_matches(':').trim().to_string()))
        .unwrap_or_default()
}

fn probe(mutate: Option<&str>) -> Probe {
    let mut c = Client::start(mutate);
    let (err, nav) = c.tool("navigate", json!({"url": fixture("injection.html")}));
    assert!(!err, "{nav}");
    let marker = format!("manuk-cdp-{}-", c.child.id());
    let (mut browser, mut port_dir) = (String::new(), String::new());
    let (mut sec, mut nnp) = (vec![], vec![]);
    for e in std::fs::read_dir("/proc").unwrap().flatten() {
        let pid = e.file_name().to_string_lossy().to_string();
        if !pid.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let Ok(raw) = std::fs::read(format!("/proc/{pid}/cmdline")) else { continue };
        let cmd = String::from_utf8_lossy(&raw).replace('\0', " ");
        if !cmd.contains(&marker) {
            continue;
        }
        if cmd.contains("--type=renderer") {
            sec.push(status_field(&pid, "Seccomp"));
            nnp.push(status_field(&pid, "NoNewPrivs"));
        } else if !cmd.contains("--type=") {
            port_dir = cmd.split("--user-data-dir=").nth(1).unwrap().split(' ').next().unwrap().to_string();
            browser = cmd;
        }
    }
    assert!(!browser.is_empty(), "no browser process carrying {marker}");
    let ap = std::fs::read_to_string(format!("{port_dir}/DevToolsActivePort")).unwrap();
    let mut l = ap.lines();
    let (port, path) = (l.next().unwrap(), l.next().unwrap());
    let stream = std::net::TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
    let (mut ws, _) = tungstenite::client(format!("ws://127.0.0.1:{port}{path}"), stream).unwrap();
    ws.send(tungstenite::Message::text(
        json!({"id": 1, "method": "Browser.getVersion"}).to_string(),
    ))
    .unwrap();
    let j: Value = loop {
        if let tungstenite::Message::Text(t) = ws.read().unwrap() {
            let v: Value = serde_json::from_str(t.as_str()).unwrap();
            if v["id"] == 1 {
                break v["result"].clone();
            }
        }
    };
    Probe {
        browser_cmdline: browser,
        renderer_seccomp: sec,
        renderer_nnp: nnp,
        version: j["product"].as_str().unwrap().to_string(),
    }
}

#[test]
fn chrome_backend_is_sandboxed_and_pinned() {
    if std::env::var("MANUK_CDP_MUTATE").is_ok() {
        return;
    }
    let p = probe(None);
    assert!(!p.browser_cmdline.contains("--no-sandbox"), "{}", p.browser_cmdline);
    assert!(!p.renderer_seccomp.is_empty(), "no renderer process found");
    for (s, n) in p.renderer_seccomp.iter().zip(&p.renderer_nnp) {
        assert_eq!(s, "2", "renderer has no seccomp filter (unsandboxed)");
        assert_eq!(n, "1", "renderer lacks NoNewPrivs (unsandboxed)");
    }
    let pin = include_str!("../../docs/sprint/CHROME_PIN").trim();
    assert!(p.version.ends_with(pin), "ran {} not pinned {pin}", p.version);
}

#[test]
fn no_sandbox_mutant_is_red() {
    let p = probe(Some("sandbox"));
    assert!(p.browser_cmdline.contains("--no-sandbox"), "mutant did not re-add the flag");
    assert!(
        p.renderer_seccomp.iter().any(|s| s != "2"),
        "the unsandboxed renderer should show no seccomp filter: {:?}",
        p.renderer_seccomp
    );
}
