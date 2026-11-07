//! W5.T3 gate (chrome_ver=145.0.7632.116): the tool surface a client pays for falls >= 40%.
//! Baselines are HEAD~ (bb497243) serialized `tools/list` bytes, measured by the same function below:
//! all 24 tools 18246 B; the six always-load tools (navigate observe act fill_form type extract) 7032 B.
//! Mutation record: restoring any one old long description, or adding the old `page` text back, turns
//! `surface_is_40_percent_smaller` RED; dropping `observe` from the alwaysLoad set turns
//! `six_core_tools_always_load` RED; making a description depend on time/pid turns `list_is_byte_identical` RED.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{json, Value};

const CORE: [&str; 6] = ["navigate", "observe", "act", "fill_form", "type", "extract"];
const BASE_TOTAL: usize = 18246;
const BASE_CORE: usize = 7032;

struct Client {
    child: Child,
    tx: ChildStdin,
    rx: BufReader<ChildStdout>,
    id: u64,
}

impl Client {
    fn start() -> Client {
        let mut child = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn manuk-mcp");
        let tx = child.stdin.take().unwrap();
        let rx = BufReader::new(child.stdout.take().unwrap());
        let mut c = Client { child, tx, rx, id: 0 };
        c.rpc("initialize", json!({"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "t", "version": "0"}}));
        writeln!(c.tx, "{}", json!({"jsonrpc": "2.0", "method": "notifications/initialized"})).unwrap();
        c
    }
    fn rpc(&mut self, method: &str, params: Value) -> Value {
        self.id += 1;
        let id = self.id;
        writeln!(self.tx, "{}", json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})).unwrap();
        self.tx.flush().unwrap();
        loop {
            let mut line = String::new();
            assert!(self.rx.read_line(&mut line).unwrap() > 0, "server closed stdout");
            let v: Value = serde_json::from_str(&line).unwrap();
            if v["id"] == json!(id) {
                return v;
            }
        }
    }
    fn list(&mut self) -> Vec<Value> {
        self.rpc("tools/list", json!({}))["result"]["tools"].as_array().unwrap().clone()
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

fn size(t: &Value) -> usize {
    t.to_string().len()
}

#[test]
fn surface_is_40_percent_smaller() {
    let tools = Client::start().list();
    let total: usize = tools.iter().map(size).sum();
    let core: usize = tools.iter().filter(|t| CORE.contains(&t["name"].as_str().unwrap())).map(size).sum();
    println!("surface total={total} (base {BASE_TOTAL}) core={core} (base {BASE_CORE})");
    assert!(total * 100 <= BASE_TOTAL * 60, "total {total} B is not >= 40% below {BASE_TOTAL}");
    assert!(core * 100 <= BASE_CORE * 60, "core {core} B is not >= 40% below {BASE_CORE}");
}

#[test]
fn six_core_tools_always_load() {
    for t in Client::start().list() {
        let name = t["name"].as_str().unwrap();
        let always = t["_meta"]["anthropic/alwaysLoad"] == json!(true);
        assert_eq!(always, CORE.contains(&name), "alwaysLoad mismatch for {name}");
    }
}

#[test]
fn list_is_byte_identical() {
    let a = Client::start().list();
    let mut c = Client::start();
    let b = c.list();
    let again = c.list();
    assert_eq!(Value::Array(a).to_string(), Value::Array(b.clone()).to_string(), "differs across sessions");
    assert_eq!(Value::Array(b).to_string(), Value::Array(again).to_string(), "differs across calls");
}

/// Deferred (not always-loaded) tools are still listed and callable.
#[test]
fn deferred_tools_are_callable() {
    let mut c = Client::start();
    let tools = c.list();
    for n in ["drag", "upload", "dialog", "download", "restore", "fork", "handoff", "storage_state"] {
        assert!(tools.iter().any(|t| t["name"] == n), "{n} not listed");
    }
    let r = c.rpc("tools/call", json!({"name": "storage_state", "arguments": {}}));
    assert!(r["result"]["isError"] != json!(true), "{r}");
    let r = c.rpc("tools/call", json!({"name": "checkpoint", "arguments": {}}));
    let text = r["result"]["content"][0]["text"].as_str().unwrap_or("");
    assert!(!text.is_empty(), "{r}");
}

/// W5.L2: the manual checkpoint is not in the default tool set (the hub checkpoints for the model).
#[test]
fn manual_checkpoint_is_not_listed() {
    let mut c = Client::start();
    assert!(!c.list().iter().any(|t| t["name"] == "checkpoint"));
}
