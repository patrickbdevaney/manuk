//! R3.2 gate: checkpoint / restore / fork (replay-based), over real stdio against the built `manuk-mcp`.

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
    format!(
        "file://{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    )
}

fn tmp(tag: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!(
        "manuk-actlog-{tag}-{}-{:?}",
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

/// The observation body without the trailing `page:` handle line.
fn body(t: &str) -> String {
    t.lines()
        .filter(|l| !l.starts_with("page: "))
        .collect::<Vec<_>>()
        .join("\n")
}

fn ckpt_of(t: &str) -> String {
    t.split_whitespace()
        .skip_while(|w| *w != "checkpoint")
        .nth(1)
        .unwrap()
        .to_string()
}

#[test]
fn checkpoint_before_add_to_cart_act_restore_gives_empty_cart_and_equal_digest() {
    let dir = tmp("ckpt");
    let mut c = Client::start(&dir);
    let (_, nav) = c.tool("navigate", json!({"url": fixture("shop.html")}));
    let add = handle_of(&nav, "button \"Add to cart\"");
    let (e, ck) = c.tool("checkpoint", json!({"page": page_of(&nav)}));
    assert!(!e, "{ck}");
    let cid = ckpt_of(&ck);
    let (_, pre) = c.tool("observe", json!({"mode": "full", "page": page_of(&ck)}));
    let (e, eff) = c.tool(
        "act",
        json!({"handle": add, "action": "click", "page": page_of(&pre)}),
    );
    assert!(!e && eff.contains("Cart (1 item)"), "{eff}");
    let (e, r) = c.tool("restore", json!({"checkpoint": cid}));
    assert!(!e && r.contains("restored"), "{r}");
    let (e, post) = c.tool("observe", json!({"mode": "full", "page": page_of(&r)}));
    assert!(!e, "{post}");
    assert!(
        !post.contains("Cart (1 item)"),
        "cart not empty after restore:\n{post}"
    );
    assert_eq!(body(&pre), body(&post), "a11y digest differs after restore");

    // fork leaves the default page alone; a brand-new client restores from the persisted file
    let (e, f) = c.tool("fork", json!({"checkpoint": cid}));
    assert!(
        !e && f.contains("forked") && page_of(&f) != page_of(&r),
        "{f}"
    );
    let mut c2 = Client::start(&dir);
    let (e, r2) = c2.tool("restore", json!({"checkpoint": cid}));
    assert!(!e, "{r2}");
    let (_, post2) = c2.tool("observe", json!({"mode": "full", "page": page_of(&r2)}));
    assert_eq!(body(&pre), body(&post2));
}

#[test]
fn restore_of_a_tampered_checkpoint_or_unknown_id_fails_loudly() {
    let dir = tmp("ckpt-bad");
    let mut c = Client::start(&dir);
    let (_, nav) = c.tool("navigate", json!({"url": fixture("shop.html")}));
    let (_, ck) = c.tool("checkpoint", json!({"page": page_of(&nav)}));
    let cid = ckpt_of(&ck);
    let (e, t) = c.tool("restore", json!({"checkpoint": "cdeadbe"}));
    assert!(e && t.contains("unknown checkpoint"), "{t}");
    let f = dir.join(format!("{cid}.ckpt"));
    let text = std::fs::read_to_string(&f).unwrap();
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let mut v: Value = serde_json::from_str(&lines[1]).unwrap();
    v["diff_hash"] = json!("0000000000000001");
    lines[1] = v.to_string();
    std::fs::write(&f, lines.join("\n")).unwrap();
    let mut c2 = Client::start(&dir);
    let (e, t) = c2.tool("restore", json!({"checkpoint": cid}));
    assert!(e && t.contains("REPLAY DIVERGED"), "{t}");
}
