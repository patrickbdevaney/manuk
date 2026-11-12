//! RG.2.S1.a.a gate: a picker modal opened from inside an already-open modal (gomail's schedule-send
//! 'Pick date & time') is reachable — its field takes typed text, a day click lands, and observe shows it.
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

fn open(fixture: &str) -> Client {
    let mut c = Client::start();
    let url = format!(
        "file://{}/tests/fixtures/{fixture}",
        env!("CARGO_MANIFEST_DIR")
    );
    let (err, nav) = c.tool("navigate", json!({"url": url}));
    assert!(!err, "{nav}");
    for q in ["Compose", "Schedule send"] {
        let h = c.handle(q);
        let (err, click) = c.tool("act", json!({"handle": h, "action": "click"}));
        assert!(!err, "{q}: {click}");
    }
    c
}

#[test]
fn picker_field_takes_text_and_day_click_lands() {
    let mut c = open("nested_picker.html");
    let (_, obs) = c.tool("observe", json!({"mode": "interactive"}));
    assert!(
        obs.contains("Date") && obs.contains("Save"),
        "picker not listed:\n{obs}"
    );
    let h = c.handle("Date");
    let (err, typed) = c.tool("type", json!({"handle": h, "text": "11/16/2026"}));
    assert!(!err, "type refused: {typed}");
    let (_, seen) = c.tool("extract", json!({}));
    assert!(
        seen.contains("date:11/16/2026"),
        "typed date not seen: {seen}"
    );
    let d = c.handle("16");
    let (err, click) = c.tool("act", json!({"handle": d, "action": "click"}));
    assert!(!err, "{click}");
    let (_, seen) = c.tool("extract", json!({}));
    assert!(seen.contains("day:16"), "day click did nothing: {seen}");
}

#[test]
fn key_and_focus_on_the_already_focused_field_are_not_refused_as_inert() {
    // Clicking a field focuses it; the agent then presses a key on that handle (or asks to focus it
    // again). `set_focus` answers "focus did not move" with false, and the layer reported that as
    // "refused focus (inert or disabled)" — the picker's field was neither.
    let mut c = open("nested_picker.html");
    let h = c.handle("Date");
    let (err, click) = c.tool("act", json!({"handle": h, "action": "click"}));
    assert!(!err, "{click}");
    let (err, key) = c.tool("key", json!({"handle": h, "key": "Tab"}));
    assert!(!err, "key refused on the focused field: {key}");
    let (err, foc) = c.tool("act", json!({"handle": h, "action": "focus"}));
    assert!(!err, "focus refused: {foc}");
    let (err, foc) = c.tool("act", json!({"handle": h, "action": "focus"}));
    assert!(!err, "second focus refused: {foc}");
}
