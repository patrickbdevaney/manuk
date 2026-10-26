//! R1.1 gate: a scripted MCP session over real stdio (JSON-RPC against the built `manuk-mcp`
//! binary), on local fixtures: navigate, click, read the diff, fail loudly on a covered target.

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

#[test]
fn lists_the_tool_surface() {
    let mut c = Client::start();
    let r = c.rpc("tools/list", json!({}));
    let names: Vec<&str> = r["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    for want in [
        "navigate",
        "observe",
        "act",
        "type",
        "scroll",
        "extract",
        "find",
        "screenshot",
    ] {
        assert!(names.contains(&want), "missing {want}: {names:?}");
    }
}

#[test]
fn scripted_session_navigates_clicks_and_reads_the_diff() {
    let mut c = Client::start();
    let (err, nav) = c.tool("navigate", json!({"url": fixture("shop.html")}));
    assert!(!err, "{nav}");
    assert!(nav.contains("Widget Shop"), "{nav}");
    let add = handle_of(&nav, "button \"Add to cart\"");

    // click → the effect comes back with the call: a dialog appears, qty changes.
    let (err, eff) = c.tool("act", json!({"handle": add, "action": "click"}));
    assert!(!err, "{eff}");
    assert!(
        eff.contains("+[h") && eff.contains("dialog \"Cart (1 item)\""),
        "no dialog in diff:\n{eff}"
    );
    assert!(eff.contains("verified: target was topmost"), "{eff}");
    assert!(eff.contains("url: unchanged"), "{eff}");

    // diff after the action is empty (the act already consumed the change) …
    let (_, d) = c.tool("observe", json!({"mode": "diff"}));
    assert!(d.contains("no change"), "{d}");
    // … and a typed value shows as =old→new
    let (_, full) = c.tool("observe", json!({"mode": "full"}));
    let q = handle_of(&full, "textbox \"Qty\"");
    let (err, t) = c.tool("type", json!({"handle": q, "text": "7"}));
    assert!(!err, "{t}");
    assert!(t.contains("=2→7") || t.contains("=1→7"), "{t}");
    // R1.2: the change is a `~` line carrying the SAME handle the agent typed into
    assert!(
        t.lines()
            .any(|l| l.starts_with(&format!("~[{q}]")) && l.contains('→')),
        "no ~[{q}] line:\n{t}"
    );

    // checkbox toggles and is verified through state, idempotently
    let gift = handle_of(&full, "checkbox");
    let (_, on) = c.tool("act", json!({"handle": gift, "action": "check"}));
    assert!(
        on.contains("unchecked]→[checked") || on.contains("checked"),
        "{on}"
    );
    assert!(
        on.lines().any(|l| l.starts_with(&format!("~[{gift}]"))),
        "no ~[{gift}] line:\n{on}"
    );
    let (_, again) = c.tool("act", json!({"handle": gift, "action": "check"}));
    assert!(again.contains("already checked"), "{again}");

    // a link navigates: handles reset, new page observed
    let (_, full) = c.tool("observe", json!({"mode": "full"}));
    let co = handle_of(&full, "link \"Checkout\"");
    let (err, go) = c.tool("act", json!({"handle": co}));
    assert!(!err, "{go}");
    assert!(
        go.contains("navigated:") && go.contains("Order received"),
        "{go}"
    );
    // the old page's handle is gone
    let (err, stale) = c.tool("act", json!({"handle": add}));
    assert!(err && stale.contains("unknown"), "{stale}");
}

#[test]
fn covered_target_fails_loudly_with_the_reason() {
    let mut c = Client::start();
    let (_, nav) = c.tool("navigate", json!({"url": fixture("shop.html")}));
    let covered = handle_of(&nav, "Covered button");
    let (err, msg) = c.tool("act", json!({"handle": covered}));
    assert!(
        err,
        "clicking a covered button must be an error, got:\n{msg}"
    );
    assert!(
        msg.contains("covered by [h") && msg.contains("cookie banner"),
        "{msg}"
    );
}

#[test]
fn find_scroll_extract_and_budget() {
    let mut c = Client::start();
    let (_, nav) = c.tool("navigate", json!({"url": fixture("shop.html")}));
    assert!(
        nav.contains("↓offscreen"),
        "far content should be marked offscreen:\n{nav}"
    );
    let (_, f) = c.tool("find", json!({"query": "footer link"}));
    assert!(f.lines().nth(1).unwrap().contains("Footer link"), "{f}");
    let h = handle_of(&f, "Footer link");
    let (err, s) = c.tool("scroll", json!({"to": h}));
    assert!(!err && s.contains("scrolled to y="), "{s}");
    let (_, vp) = c.tool("observe", json!({"mode": "viewport"}));
    assert!(
        vp.contains("Footer link") && !vp.contains("Add to cart"),
        "{vp}"
    );
    let (_, md) = c.tool("extract", json!({"format": "markdown"}));
    assert!(md.contains("# Widget Shop"), "{md}");
    let (_, tiny) = c.tool("observe", json!({"mode": "full", "budget_tokens": 40}));
    assert!(
        tiny.contains("… cut ") && tiny.len() < 40 * 4 + 600,
        "{tiny}"
    );
    let (err, bad) = c.tool("act", json!({"handle": "h9999"}));
    assert!(err, "{bad}");
    let (err, shot) = c.rpc_image_len();
    assert!(!err && shot > 100);
}

impl Client {
    fn rpc_image_len(&mut self) -> (bool, usize) {
        let r = self.rpc("tools/call", json!({"name": "screenshot", "arguments": {}}));
        let img = r["result"]["content"]
            .as_array()
            .unwrap()
            .iter()
            .find(|b| b["type"] == "image")
            .map(|b| b["data"].as_str().unwrap().len())
            .unwrap_or(0);
        (r["result"]["isError"].as_bool().unwrap_or(false), img)
    }
}

#[test]
fn modal_over_a_button_is_occluded_until_dismissed() {
    let mut c = Client::start();
    let (_, nav) = c.tool("navigate", json!({"url": fixture("modal.html")}));
    let buy = handle_of(&nav, "button \"Buy now\"");
    let name = handle_of(&nav, "textbox");
    let (err, msg) = c.tool("act", json!({"handle": buy}));
    assert!(
        err && msg.contains("OCCLUDED")
            && msg.contains("covered by [h")
            && msg.contains("Sign up dialog"),
        "{msg}"
    );
    // typing into a covered field is refused too, and nothing was changed
    let (err, msg) = c.tool("type", json!({"handle": name, "text": "x"}));
    assert!(
        err && msg.contains("OCCLUDED") && msg.contains("nothing was typed"),
        "{msg}"
    );
    // dismiss the dialog, and the same handle now clicks
    let close = handle_of(&nav, "Dismiss");
    let (err, eff) = c.tool("act", json!({"handle": close}));
    assert!(!err, "{eff}");
    let (err, eff) = c.tool("act", json!({"handle": buy}));
    assert!(
        !err && eff.contains("verified: target was topmost") && eff.contains("bought"),
        "{eff}"
    );
    // the dismissed dialog's own button is gone: stale, loudly
    let (err, msg) = c.tool("act", json!({"handle": close}));
    assert!(err && msg.contains("STALE"), "{msg}");
}

/// R1.4 gate: text a human cannot see never reaches observe/extract by default.
#[test]
fn injection_shield_withholds_invisible_text() {
    let mut c = Client::start();
    c.tool("navigate", json!({"url": fixture("injection.html")}));
    let hidden = [
        "HIDDEN_OPACITY",
        "HIDDEN_VISIBILITY",
        "HIDDEN_OFFSCREEN",
        "HIDDEN_TINY",
        "HIDDEN_CONTRAST",
        "HIDDEN_ZERO",
        "HIDDEN_DARKCONTRAST",
        "HIDDEN_DISPLAY",
    ];
    for (tool, args) in [
        ("observe", json!({})),
        ("extract", json!({})),
        ("extract", json!({"format": "markdown"})),
    ] {
        let (err, text) = c.tool(tool, args.clone());
        assert!(!err, "{text}");
        for h in hidden {
            assert!(!text.contains(h), "{tool} {args} leaked {h}:\n{text}");
        }
        assert!(
            text.contains("Revenue grew 12 percent") || tool == "observe",
            "{text}"
        );
        assert!(
            text.contains("Margins improved") || tool == "observe",
            "{text}"
        );
        assert!(text.contains("⚠hidden-text"), "{tool}: no marker:\n{text}");
        // R1.4.a.b: a zero-size wrapper with overflow:visible still paints its text.
        assert!(
            text.contains("VISIBLE_OVERFLOW") || tool == "observe",
            "{tool} withheld painted text:\n{text}"
        );
    }
    let (_, obs) = c.tool("observe", json!({}));
    assert!(
        obs.contains("Quarterly report") && obs.contains("Subscribe"),
        "{obs}"
    );
    // Opt-out returns everything except display:none (which has no box at all).
    let (_, all) = c.tool("extract", json!({"include_hidden": true}));
    for h in &hidden[..7] {
        assert!(all.contains(h), "include_hidden lost {h}:\n{all}");
    }
    assert!(!all.contains("⚠hidden-text"), "{all}");
}
