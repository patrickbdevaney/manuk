//! W5.L1 gate (chrome_ver=145.0.7632.116): the session task ledger (`note`) survives navigate, checkpoint, restore,
//! fork and a fresh process restoring from disk; its observe header is <= 60 tokens (<= 240 B).
//! Mutation record: making `from_ckpt` skip the notes merge, or fork build its page with an empty ledger, turns
//! `notes_survive_navigate_restore_fork` RED; dropping `notes` from the checkpoint head turns
//! `notes_survive_a_new_process_restore` RED.

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
    fn start(log: &std::path::Path) -> Client {
        let mut child = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"))
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
    format!("file://{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn tmp(tag: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("manuk-notes-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn page_of(text: &str) -> String {
    text.lines().rev().find_map(|l| l.strip_prefix("page: ")).unwrap_or_else(|| panic!("no page in:\n{text}")).to_string()
}

fn ckpt_of(t: &str) -> String {
    t.split_whitespace().skip_while(|w| *w != "checkpoint").nth(1).unwrap().to_string()
}

fn header(t: &str) -> &str {
    t.lines().next().unwrap()
}

#[test]
fn notes_survive_navigate_restore_fork() {
    let dir = tmp("a");
    let mut c = Client::start(&dir);
    let (e, r) = c.tool("note", json!({"key": "goal", "value": "buy the red mug under $20"}));
    assert!(!e, "{r}");
    c.tool("note", json!({"key": "seen", "value": "shop A: $24"}));
    let (_, nav) = c.tool("navigate", json!({"url": fixture("shop.html")}));
    let (_, ck) = c.tool("checkpoint", json!({"page": page_of(&nav)}));
    let cid = ckpt_of(&ck);
    let (_, obs) = c.tool("observe", json!({"mode": "full"}));
    assert!(header(&obs).starts_with("notes(2): goal=buy the red mug"), "{obs}");
    assert!(header(&obs).len() <= 240, "header too long: {}", header(&obs));
    // a note added after the checkpoint, then restore: nothing dropped
    c.tool("note", json!({"key": "later", "value": "x"}));
    let (e, r) = c.tool("restore", json!({"checkpoint": cid}));
    assert!(!e, "{r}");
    let (_, obs) = c.tool("observe", json!({"mode": "full", "page": page_of(&r)}));
    assert!(header(&obs).starts_with("notes(3):") && obs.contains("later=x"), "{obs}");
    let (e, f) = c.tool("fork", json!({"checkpoint": cid}));
    assert!(!e, "{f}");
    let (_, obs) = c.tool("observe", json!({"mode": "full", "page": page_of(&f)}));
    assert!(header(&obs).contains("goal=buy the red mug"), "fork dropped notes:\n{obs}");
    let (_, all) = c.tool("note", json!({}));
    assert!(all.contains("seen: shop A: $24"), "{all}");
    // delete + no-notes => no header
    for k in ["goal", "seen", "later"] {
        c.tool("note", json!({"key": k, "value": ""}));
    }
    let (_, obs) = c.tool("observe", json!({"mode": "full"}));
    assert!(!header(&obs).starts_with("notes("), "{obs}");
}

#[test]
fn notes_survive_a_new_process_restore() {
    let dir = tmp("b");
    let mut c = Client::start(&dir);
    c.tool("note", json!({"key": "plan", "value": "step 2 of 5"}));
    let (_, nav) = c.tool("navigate", json!({"url": fixture("shop.html")}));
    let (_, ck) = c.tool("checkpoint", json!({"page": page_of(&nav)}));
    let cid = ckpt_of(&ck);
    let mut c2 = Client::start(&dir);
    let (e, r) = c2.tool("fork", json!({"checkpoint": cid}));
    assert!(!e, "{r}");
    let (_, obs) = c2.tool("observe", json!({"mode": "full", "page": page_of(&r)}));
    assert!(header(&obs).contains("plan=step 2 of 5"), "{obs}");
}

#[test]
fn long_notes_header_is_capped() {
    let dir = tmp("c");
    let mut c = Client::start(&dir);
    for i in 0..6 {
        c.tool("note", json!({"key": format!("k{i}"), "value": "v".repeat(300)}));
    }
    let (_, nav) = c.tool("navigate", json!({"url": fixture("shop.html")}));
    let (_, obs) = c.tool("observe", json!({"mode": "full", "page": page_of(&nav)}));
    assert!(header(&obs).chars().count() <= 200 && header(&obs).ends_with('…'), "{}", header(&obs));
    let (_, all) = c.tool("note", json!({}));
    assert!(all.lines().count() == 6 && all.len() > 1800, "full list must be untruncated");
}
