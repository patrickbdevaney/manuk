//! W6.C2 gate (chrome_ver=145.0.7632.116): `evaluate` (read-only, grant-gated, always loaded), `act` with
//! `force` / `at`, and extract marking withheld text in place. Both backends are exercised: the engine
//! refuses `evaluate` and `at` cleanly; chrome runs them.
//! Mutations (each turns a test RED): drop `throwOnSideEffect` from `page_eval_readonly` ->
//! `evaluate_cannot_write`; drop the `self.force` skip in `aim` -> `force_clicks_a_covered_button`;
//! make `page_text_marked` return `sh.text` -> `withheld_text_is_marked_in_place`.

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
    fn start(backend: &str, extra: &[&str]) -> Client {
        let mut child = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"))
            .env_remove("MANUK_BACKEND")
            .args(["--backend", backend])
            .args(extra)
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
        writeln!(
            c.tx,
            "{}",
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"})
        )
        .unwrap();
        let url = format!(
            "file://{}/tests/fixtures/eval_force_at.html",
            env!("CARGO_MANIFEST_DIR")
        );
        c.tool("navigate", json!({"url": url}));
        c
    }
    fn rpc(&mut self, method: &str, params: Value) -> Value {
        self.id += 1;
        let id = self.id;
        writeln!(
            self.tx,
            "{}",
            json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
        )
        .unwrap();
        self.tx.flush().unwrap();
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
    /// `(is_error, text)`
    fn call(&mut self, name: &str, args: Value) -> (bool, String) {
        let r = self.rpc("tools/call", json!({"name": name, "arguments": args}));
        let text = r["result"]["content"]
            .as_array()
            .map(|c| {
                c.iter()
                    .filter_map(|b| b["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_else(|| r["error"].to_string());
        (
            r["result"]["isError"] == json!(true) || r.get("error").is_some(),
            text,
        )
    }
    fn tool(&mut self, name: &str, args: Value) -> String {
        self.call(name, args).1
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

fn handle_of(obs: &str, name: &str) -> String {
    let line = obs
        .lines()
        .find(|l| l.contains(name))
        .unwrap_or_else(|| panic!("{name} not in:\n{obs}"));
    line[line.find("[h").unwrap() + 1..]
        .split(']')
        .next()
        .unwrap()
        .to_string()
}

#[test]
fn evaluate_reads_the_page_on_chrome() {
    let mut c = Client::start("chrome", &[]);
    let t = c.tool("evaluate", json!({"expression": "document.title"}));
    assert!(t.contains("evalpage"), "{t}");
    let t = c.tool(
        "evaluate",
        json!({"expression": "JSON.stringify([1,2,3].map(x=>x*2))"}),
    );
    assert!(t.contains("[2,4,6]"), "{t}");
    let t = c.tool(
        "evaluate",
        json!({"expression": "document.querySelectorAll('p').length"}),
    );
    assert!(t.contains('4'), "non-string values come back as JSON: {t}");
}

#[test]
fn evaluate_cannot_write() {
    let mut c = Client::start("chrome", &[]);
    let (err, t) = c.call("evaluate", json!({"expression": "document.title='pwned'"}));
    assert!(err, "a write must be refused: {t}");
    let t = c.tool("evaluate", json!({"expression": "document.title"}));
    assert!(t.contains("evalpage") && !t.contains("pwned"), "{t}");
}

#[test]
fn evaluate_needs_the_grant_under_limits() {
    let mut c = Client::start("chrome", &["--confirm=download"]);
    let (err, t) = c.call("evaluate", json!({"expression": "document.title"}));
    assert!(err && t.contains("not granted"), "{t}");
    let mut c = Client::start("chrome", &["--confirm=download", "--allow-eval"]);
    let (err, t) = c.call("evaluate", json!({"expression": "document.title"}));
    assert!(!err && t.contains("evalpage"), "{t}");
}

#[test]
fn engine_refuses_evaluate_and_at_cleanly() {
    let mut c = Client::start("engine", &[]);
    let (err, t) = c.call("evaluate", json!({"expression": "document.title"}));
    assert!(err && t.contains("chrome backend"), "{t}");
    let (err, t) = c.call("act", json!({"at": {"x": 10, "y": 10}}));
    assert!(err && t.contains("chrome backend"), "{t}");
}

#[test]
fn force_clicks_a_covered_button() {
    let mut c = Client::start("chrome", &[]);
    let obs = c.tool("observe", json!({}));
    let h = handle_of(&obs, "button \"Buy\"");
    let (err, t) = c.call("act", json!({"handle": h, "action": "click"}));
    assert!(
        err && t.contains("OCCLUDED"),
        "unforced click must be refused: {t}"
    );
    let t = c.tool(
        "act",
        json!({"handle": h, "action": "click", "force": true}),
    );
    assert!(!t.contains("OCCLUDED"), "{t}");
    let t = c.tool("extract", json!({}));
    assert!(t.contains("idle") || t.contains("clicked-b"), "{t}");
}

#[test]
fn at_clicks_coordinates() {
    let mut c = Client::start("chrome", &[]);
    let _ = c.tool("act", json!({"at": {"x": 550, "y": 450}}));
    let t = c.tool("extract", json!({}));
    assert!(t.contains("pad-hit"), "{t}");
}

#[test]
fn withheld_text_is_marked_in_place() {
    for backend in ["chrome", "engine"] {
        let mut c = Client::start(backend, &[]);
        let t = c.tool("extract", json!({}));
        assert!(
            !t.contains("SECRETWORD"),
            "{backend}: withheld words must stay out:\n{t}"
        );
        let i = t
            .find("Price is 10 dollars.")
            .unwrap_or_else(|| panic!("{backend}:\n{t}"));
        let j = t
            .find("Shipping is free.")
            .unwrap_or_else(|| panic!("{backend}:\n{t}"));
        let between = &t[i..j];
        assert!(
            between.contains("[withheld:"),
            "{backend}: marker missing between the two paragraphs:\n{t}"
        );
    }
}
