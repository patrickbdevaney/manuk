//! RG.S1.a.d gate (chrome_ver=145.0.7632.116): on the CDP backend an `overflow:hidden` carousel's
//! clipped-away slide does not occlude the button it overflows onto (the A11yNode `clip` is filled from CDP,
//! as the in-process engine does). `MANUK_CDP_MUTATE=no-clip` (clip: None) turns it RED: `Link` reads OCCLUDED.
//! The control `Under` (same slide, no clipping ancestor) must stay OCCLUDED, as in Chrome.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{json, Value};

struct Client {
    child: Child,
    tx: Option<ChildStdin>,
    rx: BufReader<ChildStdout>,
    id: u64,
}

impl Client {
    /// `backend` = `chrome` (CDP) or `manuk` (the in-process engine, no flag).
    fn start_on(backend: &str, mutate: Option<&str>) -> Client {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"));
        if backend == "chrome" {
            cmd.args(["--backend", "chrome"]);
        }
        cmd.stdin(Stdio::piped())
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
            tx: Some(tx),
            rx,
            id: 0,
        };
        c.rpc("initialize", json!({"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "t", "version": "0"}}));
        c.send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        c
    }
    fn send(&mut self, v: Value) {
        let tx = self.tx.as_mut().unwrap();
        writeln!(tx, "{v}").unwrap();
        tx.flush().unwrap();
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


fn click(mutate: Option<&str>, name: &str) -> (bool, String) {
    let mut c = Client::start_on("chrome", mutate);
    let (err, nav) = c.tool("navigate", json!({"url": fixture("cdp_clip.html")}));
    assert!(!err, "{nav}");
    let h = handle_of(&nav, &format!("button \"{name}\""));
    c.tool("act", json!({"handle": h, "action": "click", "detail": true}))
}

#[test]
fn a_button_a_clipped_slide_overflows_onto_is_clickable() {
    let (err, txt) = click(None, "Link");
    assert!(!err && txt.contains("clicked-link"), "err={err}: {txt}");
}

#[test]
fn unclipped_overflow_still_occludes() {
    let (err, txt) = click(None, "Under");
    assert!(err && txt.contains("OCCLUDED"), "err={err}: {txt}");
}

/// Twin: with `clip` left None the clipped slide occludes `Link` (proves the gate can go red).
#[test]
fn no_clip_mutant_reads_the_clipped_slide_as_occluding() {
    let (err, txt) = click(Some("no-clip"), "Link");
    assert!(err && txt.contains("OCCLUDED"), "err={err}: {txt}");
}
