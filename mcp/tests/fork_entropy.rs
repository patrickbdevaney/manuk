//! W4.C3.b: do forks of one checkpoint diverge on client-side `Math.random` / `Date.now`?
//!
//! Best-of-N (W4.C3) only helps if the N forks of a checkpoint can end in different states. A page
//! whose click draws from `Math.random()` gives N identical branches on an engine that seeds its dice
//! the same way in every fork. This test forks one checkpoint three ways, clicks the same button in
//! each, and reads the value each fork rolled. Run on both backends: `manuk` (the engine) and
//! `chrome` (the cdp-layer over the pinned Chrome, where a fork is a prefix replay in a fresh Chrome).
//!
//! The fixture is `fixtures/dice.html`; the value sits in `Roll: <random> at <Date.now()>`.

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
    fn start(chrome: bool, det: bool, log: &std::path::Path) -> Client {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"));
        if chrome {
            cmd.args(["--backend", "chrome"]);
        }
        if det {
            cmd.arg("--deterministic");
        }
        let mut child = cmd
            .env("MANUK_ACTLOG_DIR", log)
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
        let init = c.rpc("initialize", json!({"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "t", "version": "0"}}));
        assert!(
            init["result"]["capabilities"]["tools"].is_object(),
            "{init}"
        );
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

fn tmp(tag: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!(
        "manuk-forkent-{tag}-{}-{:?}",
        std::process::id(),
        std::time::Instant::now()
    ));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// The handle of the first line containing `needle`.
fn handle_of(text: &str, needle: &str) -> String {
    let line = text
        .lines()
        .find(|l| l.contains(needle))
        .unwrap_or_else(|| panic!("no line with {needle:?} in:\n{text}"));
    let start = line.find("[h").unwrap();
    let end = line[start..].find(']').unwrap() + start;
    line[start + 1..end].to_string()
}

/// The `page: <id>:<seq>` trailer of a tool result.
fn page_of(text: &str) -> String {
    text.lines()
        .rev()
        .find_map(|l| l.strip_prefix("page: "))
        .unwrap_or_else(|| panic!("no page handle in:\n{text}"))
        .to_string()
}

fn ckpt_of(t: &str) -> String {
    t.split_whitespace()
        .skip_while(|w| *w != "checkpoint")
        .nth(1)
        .unwrap()
        .to_string()
}

/// The `Roll: <random> at <now>` line, as (random, now). Panics if the page did not roll.
fn roll_of(text: &str) -> (String, String) {
    let line = text
        .lines()
        .find(|l| l.contains("Roll: ") && l.contains(" at "))
        .unwrap_or_else(|| panic!("no roll line in:\n{text}"));
    let after = &line[line.find("Roll: ").unwrap() + "Roll: ".len()..];
    let (r, rest) = after.split_once(" at ").unwrap();
    let now: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    (r.trim().to_string(), now)
}

/// Fork one checkpoint `n` ways, click the same button in each, return what each fork rolled.
fn roll_in_forks(chrome: bool, det: bool, n: usize, tag: &str) -> Vec<(String, String)> {
    let dir = tmp(tag);
    let mut c = Client::start(chrome, det, &dir);
    let (e, nav) = c.tool("navigate", json!({"url": fixture("dice.html")}));
    assert!(!e, "{nav}");
    let (e, ck) = c.tool("checkpoint", json!({"page": page_of(&nav)}));
    assert!(!e, "{ck}");
    let cid = ckpt_of(&ck);
    (0..n)
        .map(|i| {
            let (e, f) = c.tool("fork", json!({"checkpoint": cid}));
            assert!(!e, "fork {i}: {f}");
            let (_, obs) = c.tool("observe", json!({"mode": "full", "page": page_of(&f)}));
            let h = handle_of(&obs, "Roll");
            let (e, act) = c.tool(
                "act",
                json!({"handle": h, "action": "click", "page": page_of(&obs)}),
            );
            assert!(!e, "fork {i} click: {act}");
            roll_of(&act)
        })
        .collect()
}

#[test]
fn manuk_forks_of_one_checkpoint_roll_distinct_dice() {
    let rolls = roll_in_forks(false, false, 3, "manuk");
    eprintln!("manuk forks: {rolls:?}");
    let mut seen: Vec<&String> = rolls.iter().map(|r| &r.0).collect();
    seen.sort();
    seen.dedup();
    assert_eq!(seen.len(), 3, "manuk forks rolled the same dice: {rolls:?}");
}

/// The gap, recorded: `--deterministic` seeds `Math.random` per page install, so every fork of a
/// checkpoint draws the same first value. Red if that ever stops holding (it is then a gap closed).
#[test]
fn manuk_deterministic_forks_roll_the_same_dice_the_recorded_gap() {
    let rolls = roll_in_forks(false, true, 3, "manuk-det");
    eprintln!("manuk --deterministic forks: {rolls:?}");
    let first = &rolls[0].0;
    assert!(
        rolls.iter().all(|r| &r.0 == first),
        "deterministic forks diverged: {rolls:?}"
    );
}

/// The red check for the parser the tests above trust: a page that did not roll must fail loudly.
#[test]
#[should_panic(expected = "no roll line")]
fn a_page_that_did_not_roll_is_not_read_as_a_value() {
    roll_of("Dice roller\nRoll: none yet");
}

#[test]
fn chrome_forks_of_one_checkpoint_roll_distinct_dice() {
    let rolls = roll_in_forks(true, false, 3, "chrome");
    eprintln!("chrome forks: {rolls:?}");
    let mut seen: Vec<&String> = rolls.iter().map(|r| &r.0).collect();
    seen.sort();
    seen.dedup();
    assert_eq!(
        seen.len(),
        3,
        "chrome forks rolled the same dice: {rolls:?}"
    );
}
