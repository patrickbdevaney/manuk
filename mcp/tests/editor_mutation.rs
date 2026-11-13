//! RG.2.S1.a.b gate: text the host types into a contenteditable reaches the editor's MutationObserver
//! (Quill refreshes its model from the records), so Send posts the typed body instead of an empty one.
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
    fn start() -> Client {
        let mut child = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"))
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
    fn handle(&mut self, query: &str) -> String {
        let (_, found) = self.tool("find", json!({"query": query}));
        found
            .lines()
            .find(|l| l.contains(query))
            .and_then(|l| l.find("[h").map(|s| &l[s + 1..]))
            .and_then(|l| l.find(']').map(|e| l[..e].to_string()))
            .unwrap_or_else(|| panic!("no {query} handle: {found}"))
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

#[test]
fn typed_editor_text_reaches_the_mutation_observer() {
    let mut c = Client::start();
    let url = format!(
        "file://{}/tests/fixtures/editor_mutation.html",
        env!("CARGO_MANIFEST_DIR")
    );
    let (err, nav) = c.tool("navigate", json!({"url": url}));
    assert!(!err, "{nav}");
    let ed = c.handle("textbox");
    let (err, t) = c.tool(
        "type",
        json!({"handle": ed, "text": "Please find the meeting notes attached."}),
    );
    assert!(!err, "{t}");
    let send = c.handle("Send");
    let (err, t) = c.tool("act", json!({"handle": send, "action": "click"}));
    assert!(!err, "{t}");
    let (_, seen) = c.tool("extract", json!({}));
    assert!(
        seen.contains("model:Please find the meeting notes attached."),
        "editor model never saw the typed text: {seen}"
    );
}

#[test]
fn created_elements_serialize_with_lowercase_html_names() {
    // an editor's `getHTML()` is `innerHTML` of blocks made with `createElement('P')`; Chrome gives `<p>`
    let mut c = Client::start();
    let url = format!(
        "file://{}/tests/fixtures/editor_html.html",
        env!("CARGO_MANIFEST_DIR")
    );
    let (err, nav) = c.tool("navigate", json!({"url": url}));
    assert!(!err, "{nav}");
    let (_, seen) = c.tool("extract", json!({}));
    assert!(
        seen.contains("out:<p>Please find the notes.</p>"),
        "innerHTML of a created P: {seen}"
    );
}
