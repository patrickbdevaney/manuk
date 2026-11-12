//! W5.L4 gate (chrome_ver=145.0.7632.116): observe flags a consent/modal overlay and `act(action="dismiss")`
//! clicks its own reject/necessary-only/close control (never accept/subscribe) on BOTH backends: 3 consent
//! patterns + 1 modal trap are flagged and dismissed; a CAPTCHA fixture is flagged BOT-WALL, `dismiss` is
//! refused and nothing on it is clicked. Mutant `MANUK_OVERLAY_MUTATE=off` (no flag) turns it RED.

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
                let first = String::from_utf8_lossy(&buf[..n])
                    .lines()
                    .next()
                    .unwrap_or("")
                    .to_string();
                let path = first.split(' ').nth(1).unwrap_or("/").to_string();
                let body = page(&path);
                let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            });
        }
    });
    format!("http://{addr}")
}

fn shell(overlay: &str) -> String {
    format!(
        r#"<!doctype html><html><head><title>Shop</title></head><body><h1>Shop</h1>
<button id="buy" onclick="document.getElementById('st').textContent='bought'">Buy now</button><p id="st">idle</p>
{overlay}</body></html>"#
    )
}

fn page(path: &str) -> String {
    let hide = "this.closest('[role]').style.display='none'";
    match path {
        // 1: banner with an explicit Reject all next to Accept all
        "/c1" => shell(&format!(
            r#"<div role="dialog" aria-label="Cookie banner"><p>We use cookies to improve your experience.</p>
<button onclick="document.getElementById('st').textContent='accepted';{hide}">Accept all</button>
<button onclick="{hide}">Reject all</button></div>"#
        )),
        // 2: preference dialog: Manage / Accept / Necessary only
        "/c2" => shell(&format!(
            r#"<div role="alertdialog" aria-label="Privacy choices"><p>Your privacy matters. We ask for consent to use cookies.</p>
<button>Manage preferences</button><button onclick="document.getElementById('st').textContent='accepted';{hide}">I agree</button>
<button onclick="{hide}">Use necessary only</button></div>"#
        )),
        // 3: GDPR notice whose only exit is a link "Continue without accepting"
        "/c3" => shell(&format!(
            r##"<div role="dialog" aria-label="GDPR notice"><p>GDPR consent: we would like to set cookies.</p>
<button onclick="document.getElementById('st').textContent='accepted';{hide}">Accept</button>
<a href="#" onclick="{hide};return false">Continue without accepting</a></div>"##
        )),
        // 4: modal trap: a big subscribe button, a tiny close glyph
        "/m1" => shell(&format!(
            r#"<div role="dialog" aria-label="Join our newsletter"><p>Get 10% off your first order!</p>
<input aria-label="Email"><button onclick="document.getElementById('st').textContent='subscribed';{hide}">Subscribe and save</button>
<button aria-label="x" onclick="{hide}">×</button></div>"#
        )),
        // 5: CAPTCHA: must go to handoff, never clicked
        "/bot" => shell(
            r#"<div role="dialog" aria-label="Security check"><p>Please verify you are human.</p>
<button onclick="document.getElementById('st').textContent='captcha-clicked'">I'm not a robot</button>
<button onclick="document.getElementById('st').textContent='captcha-clicked'">Close</button></div>"#,
        ),
        // 6: an app shell: one positioned, z-indexed, viewport-sized wrapper around ALL the content (every
        // React/MUI app has one). Nothing sits behind it, so it is not a modal.
        "/wrap" => format!(
            r#"<!doctype html><html><head><title>App</title></head><body style="margin:0">
<div style="position:relative;z-index:1;width:100%;height:100vh"><h1>Calendar</h1>
<button id="a" onclick="document.getElementById('st').textContent='a-clicked'">Event A</button>
<button>Event B</button><p id="st">idle</p></div></body></html>"#
        ),
        // 7: a real trap with no dialog role: a fixed, z-indexed full-page layer over page controls.
        "/fixed" => shell(
            r#"<div style="position:fixed;left:0;top:0;width:100%;height:100%;z-index:50;background:#fff">
<p>Sign in to continue</p><button>Continue</button></div>"#,
        ),
        // 8: zilloft-2 shape: a positioned results landmark whose box spans the filter bar, pointer-events
        // none, so every filter control is still clickable. Not an overlay.
        "/results" => shell(
            r#"<div style="position:relative;height:60px"><button>Beds &amp; Baths</button><button>Price</button></div>
<main aria-label="Search results for homes" style="position:absolute;left:0;top:0;width:100%;height:100vh;z-index:5;pointer-events:none"><a href="/h1">Home one</a></main>"#,
        ),
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

fn handle_of(text: &str, needle: &str) -> String {
    let line = text
        .lines()
        .find(|l| l.contains(needle) && l.contains("[h"))
        .unwrap_or_else(|| panic!("no {needle:?} in:\n{text}"));
    let s = line.find("[h").unwrap();
    let e = line[s..].find(']').unwrap() + s;
    line[s + 1..e].to_string()
}

fn overlay_handle(obs: &str) -> String {
    let line = obs
        .lines()
        .find(|l| l.contains("OVERLAY [h"))
        .unwrap_or_else(|| panic!("no overlay flag in:\n{obs}"));
    let s = line.find("[h").unwrap();
    let e = line[s..].find(']').unwrap() + s;
    line[s + 1..e].to_string()
}

fn run(chrome: bool, env: &[(&str, &str)]) {
    let base = serve();
    let mut c = Client::start(chrome, env);
    // Four dismissable interruptions: flagged, dismissed through their own control, page then usable.
    for (path, kind, control) in [
        ("/c1", "CONSENT", "Reject all"),
        ("/c2", "CONSENT", "Use necessary only"),
        ("/c3", "CONSENT", "Continue without accepting"),
        ("/m1", "MODAL", "x"),
    ] {
        c.ok("navigate", json!({"url": format!("{base}{path}")}));
        let obs = c.ok("observe", json!({}));
        assert!(
            obs.contains(&format!("{kind} OVERLAY [h")),
            "{path}: not flagged as {kind}:\n{obs}"
        );
        assert!(
            obs.contains(&format!("{control:?}")),
            "{path}: wrong dismiss control (want {control}):\n{obs}"
        );
        let h = overlay_handle(&obs);
        c.ok("act", json!({"handle": h, "action": "dismiss"}));
        let after = c.ok("observe", json!({}));
        assert!(
            !after.contains("OVERLAY [h"),
            "{path}: still flagged after dismiss:\n{after}"
        );
        assert!(
            after.contains("idle"),
            "{path}: accept/subscribe was clicked instead:\n{after}"
        );
        eprintln!("L4 chrome={chrome} {path} ok");
    }
    // A CAPTCHA: flagged, dismiss refused, nothing clicked.
    c.ok("navigate", json!({"url": format!("{base}/bot")}));
    let obs = c.ok("observe", json!({}));
    assert!(
        obs.contains("BOT-WALL") && obs.contains("handoff"),
        "bot wall not routed to handoff:\n{obs}"
    );
    let (err, txt) = c.call(
        "act",
        json!({"handle": handle_of(&obs, "Close"), "action": "dismiss"}),
    );
    assert!(
        err && txt.contains("BOT-WALL"),
        "dismiss on a CAPTCHA must be refused: {err} {txt}"
    );
    let after = c.ok("observe", json!({}));
    assert!(
        after.contains("idle") && !after.contains("captcha-clicked"),
        "CAPTCHA fixture was clicked:\n{after}"
    );
}

/// RG.E1: the box heuristic named the app's own wrapper a MODAL OVERLAY ("no reject/close control: handoff"),
/// so gocalendar-3, staynb-2, fly-unified-* and zilloft-5 ended in a handoff on a page Chrome showed no
/// overlay on. A wrapper holds ALL the controls: nothing is behind it. A fixed layer over the page still is.
fn wrapper_run(env: &[(&str, &str)]) {
    let base = serve();
    let mut c = Client::start(false, env);
    c.ok("navigate", json!({"url": format!("{base}/wrap")}));
    for mode in ["viewport", "interactive", "full"] {
        let obs = c.ok("observe", json!({"mode": mode}));
        assert!(
            !obs.contains("OVERLAY [h"),
            "wrapper flagged as an overlay in {mode}:\n{obs}"
        );
    }
    c.ok("navigate", json!({"url": format!("{base}/fixed")}));
    let obs = c.ok("observe", json!({}));
    assert!(
        obs.contains("MODAL OVERLAY [h"),
        "fixed full-page layer over page controls not flagged:\n{obs}"
    );
}

/// RG.2.E1.a.b: a positioned results main spanning the filter bar, with the bar's controls clickable
/// through it, is not an overlay; the fixed layer above still is.
#[test]
fn a_pass_through_results_landmark_is_not_an_overlay() {
    let base = serve();
    let mut c = Client::start(false, &[]);
    c.ok("navigate", json!({"url": format!("{base}/results")}));
    let obs = c.ok("observe", json!({}));
    assert!(
        !obs.contains("OVERLAY [h"),
        "results landmark flagged:\n{obs}"
    );
    c.ok("navigate", json!({"url": format!("{base}/fixed")}));
    let obs = c.ok("observe", json!({}));
    assert!(
        obs.contains("MODAL OVERLAY [h"),
        "real layer not flagged:\n{obs}"
    );
}

#[test]
fn an_app_wrapper_is_not_a_modal_overlay() {
    wrapper_run(&[]);
}

#[test]
fn overlays_on_engine() {
    run(false, &[]);
}

#[test]
fn overlays_on_chrome() {
    run(true, &[]);
}

#[test]
#[should_panic(expected = "not flagged")]
fn mutant_no_flag_goes_red() {
    run(false, &[("MANUK_OVERLAY_MUTATE", "off")]);
}
