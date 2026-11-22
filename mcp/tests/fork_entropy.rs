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
        Client::start_dir(chrome, det, log, !det)
    }
    fn start_dir(chrome: bool, det: bool, log: &std::path::Path, persist: bool) -> Client {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"));
        if chrome {
            cmd.args(["--backend", "chrome"]);
        }
        if det {
            cmd.arg("--deterministic");
        }
        // `--deterministic` page ids (a fork's entropy is derived from its id) are process-independent
        // only when nothing is persisted, so the det runs get no log dir.
        if persist {
            cmd.env("MANUK_ACTLOG_DIR", log);
        }
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
                json!({"handle": h, "action": "click", "detail": true, "page": page_of(&obs)}),
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

/// W4.C3.b.a (closes the gap recorded by W4.C3.b): under `--deterministic` each fork draws from a
/// stream derived from its own page id, so 3 forks roll 3 distinct values, and the same fork (same id
/// in an identical session) rolls the same value again.
fn det_rolls(mutate: Option<&str>, tag: &str) -> Vec<(String, String)> {
    if let Some(m) = mutate {
        std::env::set_var("MANUK_MUTATE", m);
    } else {
        std::env::remove_var("MANUK_MUTATE");
    }
    let r = roll_in_forks(false, true, 3, tag);
    std::env::remove_var("MANUK_MUTATE");
    r
}

static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn manuk_deterministic_forks_roll_distinct_dice_and_each_reproduces() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let a = det_rolls(None, "det-a");
    let b = det_rolls(None, "det-b");
    eprintln!("manuk --deterministic forks: {a:?} / rerun {b:?}");
    let mut seen: Vec<&String> = a.iter().map(|r| &r.0).collect();
    seen.sort();
    seen.dedup();
    assert_eq!(
        seen.len(),
        3,
        "deterministic forks rolled the same dice: {a:?}"
    );
    assert_eq!(a, b, "a fork id must re-roll exactly its own value");
}

/// Red proof: the pre-W4.C3.b.a behaviour (forks reseed from the parent) gives identical rolls.
#[test]
fn mutant_that_reseeds_forks_from_the_parent_gives_identical_rolls() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let rolls = det_rolls(Some("fork-salt-parent"), "det-mut");
    eprintln!("mutant forks: {rolls:?}");
    assert!(
        rolls.iter().all(|r| r.0 == rolls[0].0),
        "mutant should roll identically: {rolls:?}"
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

/// W4.C3.b.a.a: a fork's post-fork log survives a lazy resume. Process 1 forks, rolls in the fork
/// (the roll is recorded in the fork's persisted log); process 2 (same log dir, so the page is not in
/// memory) resumes the fork by its page handle and observes. The resume must replay the roll on the
/// fork's own stream (no REPLAY DIVERGED) and show the same value.
/// The number following the last `Roll: ` in a result (0.xxx).
fn num_after_roll(t: &str) -> String {
    let i = t.rfind("Roll: ").unwrap_or_else(|| panic!("no roll in {t}")) + 6;
    let n: String = t[i..].chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    assert!(n.starts_with("0."), "no rolled number in {t}");
    n
}

fn resume_after_roll(mutate: Option<&str>, tag: &str) -> (bool, String, String) {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(m) = mutate {
        std::env::set_var("MANUK_MUTATE", m);
    } else {
        std::env::remove_var("MANUK_MUTATE");
    }
    let dir = tmp(tag);
    let mut c = Client::start_dir(false, true, &dir, true);
    let (e, nav) = c.tool("navigate", json!({"url": fixture("dice.html")}));
    assert!(!e, "{nav}");
    let (e, ck) = c.tool("checkpoint", json!({"page": page_of(&nav)}));
    assert!(!e, "{ck}");
    let (e, f) = c.tool("fork", json!({"checkpoint": ckpt_of(&ck)}));
    assert!(!e, "{f}");
    let (_, obs) = c.tool("observe", json!({"mode": "full", "page": page_of(&f)}));
    let h = handle_of(&obs, "Roll");
    let (e, act) = c.tool(
        "act",
        json!({"handle": h, "action": "click", "detail": true, "page": page_of(&obs)}),
    );
    assert!(!e, "{act}");
    let rolled = num_after_roll(&act);
    let handle = page_of(&act);
    drop(c);
    let mut c2 = Client::start_dir(false, true, &dir, true);
    let (err, text) = c2.tool("observe", json!({"mode": "full", "page": handle}));
    std::env::remove_var("MANUK_MUTATE");
    (err, text, rolled)
}

#[test]
fn a_resumed_fork_replays_its_post_fork_roll_without_diverging() {
    let (err, text, rolled) = resume_after_roll(None, "resume");
    assert!(!err && !text.contains("REPLAY DIVERGED"), "{text}");
    assert_eq!(num_after_roll(&text), rolled, "{text}");
}

/// Red proof: a resume that replays at salt 0 (the pre-fix behaviour) diverges.
#[test]
fn mutant_that_resumes_at_salt_zero_diverges() {
    let (err, text, _) = resume_after_roll(Some("resume-salt-zero"), "resume-mut");
    assert!(err && text.contains("REPLAY DIVERGED"), "{text}");
}
