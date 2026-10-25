//! W3.5 gate: action log + stateless page handles, over real stdio against the built `manuk-mcp`.

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

fn log_lines(dir: &std::path::Path) -> Vec<Value> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        for l in std::fs::read_to_string(p).unwrap().lines() {
            out.push(serde_json::from_str(l).unwrap());
        }
    }
    out
}

/// One scripted run; returns (final page handle, diff hashes).
fn script(dir: &std::path::Path) -> (String, Vec<String>) {
    let mut c = Client::start(dir);
    let (e, nav) = c.tool("navigate", json!({"url": fixture("shop.html")}));
    assert!(!e, "{nav}");
    let add = handle_of(&nav, "button \"Add to cart\"");
    let (e, eff) = c.tool(
        "act",
        json!({"handle": add, "action": "click", "page": page_of(&nav)}),
    );
    assert!(!e && eff.contains("Cart (1 item)"), "{eff}");
    let (e, d) = c.tool("observe", json!({"mode": "diff", "page": page_of(&eff)}));
    assert!(!e, "{d}");
    let hashes = log_lines(dir)
        .iter()
        .map(|v| v["diff_hash"].as_str().unwrap().to_string())
        .collect();
    (page_of(&d), hashes)
}

#[test]
fn log_entries_carry_seq_handle_hash_and_storage_hash_and_replay_is_identical_3_of_3() {
    let mut runs = Vec::new();
    for i in 0..3 {
        let dir = tmp(&format!("rep{i}"));
        let (handle, hashes) = script(&dir);
        let log = log_lines(&dir);
        assert_eq!(log.len(), 3);
        for (n, e) in log.iter().enumerate() {
            assert_eq!(e["seq"], json!(n + 1));
            assert_eq!(
                e["handle"].as_str().unwrap(),
                format!("{}:{}", handle.split(':').next().unwrap(), n + 1)
            );
            assert!(e["storage_hash"].is_string() && e["req"].is_array());
        }
        // a fresh server replays the whole log against the fixture and verifies every hash
        let mut c = Client::start(&dir);
        let (err, t) = c.tool("observe", json!({"mode": "full", "page": handle}));
        assert!(!err, "replay failed:\n{t}");
        assert!(t.contains("Cart (1 item)"), "{t}");
        runs.push(hashes);
    }
    assert_eq!(
        runs[0], runs[1],
        "diff hashes differ between identical runs"
    );
    assert_eq!(runs[1], runs[2]);
}

#[test]
fn replay_refuses_a_log_whose_hashes_no_longer_match() {
    let dir = tmp("tamper");
    let (handle, _) = script(&dir);
    let f = std::fs::read_dir(&dir)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let text = std::fs::read_to_string(&f).unwrap();
    let mut lines: Vec<Value> = text
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    lines[1]["diff_hash"] = json!("0000000000000001");
    std::fs::write(
        &f,
        lines
            .iter()
            .map(|v| v.to_string() + "\n")
            .collect::<String>(),
    )
    .unwrap();
    let mut c = Client::start(&dir);
    let (err, t) = c.tool("observe", json!({"page": handle}));
    assert!(err && t.contains("REPLAY DIVERGED"), "{t}");
}

#[test]
fn a_fresh_client_with_only_the_handle_continues_the_diff_from_the_same_state() {
    let dir = tmp("resume");
    let handle;
    {
        let mut a = Client::start(&dir);
        let (_, nav) = a.tool("navigate", json!({"url": fixture("shop.html")}));
        let add = handle_of(&nav, "button \"Add to cart\"");
        let (_, eff) = a.tool(
            "act",
            json!({"handle": add, "action": "click", "page": page_of(&nav)}),
        );
        handle = page_of(&eff);
        // client A is killed here (Drop) mid-task
    }
    let mut b = Client::start(&dir);
    let (err, d) = b.tool("observe", json!({"mode": "diff", "page": handle}));
    assert!(!err, "{d}");
    assert!(
        d.contains("no change"),
        "diff baseline was not restored:\n{d}"
    );
    let (_, full) = b.tool("observe", json!({"mode": "full", "page": page_of(&d)}));
    assert!(full.contains("Cart (1 item)"), "{full}");
    // and it can keep acting on handles minted before the crash
    let qty = handle_of(&full, "textbox");
    let (err, eff) = b.tool(
        "type",
        json!({"handle": qty, "text": "7", "page": page_of(&full)}),
    );
    assert!(!err && eff.contains("~[h") && eff.contains("=2→7"), "{eff}");
}

#[test]
fn stale_or_malformed_handles_are_refused() {
    let dir = tmp("stale");
    let mut c = Client::start(&dir);
    let (_, nav) = c.tool("navigate", json!({"url": fixture("shop.html")}));
    let p = page_of(&nav);
    let (id, _) = p.split_once(':').unwrap();
    let (_, o) = c.tool("observe", json!({"page": p}));
    assert!(!o.is_empty());
    let (err, t) = c.tool("observe", json!({"page": format!("{id}:1")}));
    assert!(err && t.contains("STALE"), "{t}");
    let (err, t) = c.tool("observe", json!({"page": "garbage"}));
    assert!(err && t.contains("bad page handle"), "{t}");
    let (err, t) = c.tool("observe", json!({"page": "pffffff:3"}));
    assert!(err && t.contains("unknown page"), "{t}");
}

#[test]
fn two_pages_in_one_server_do_not_cross_talk() {
    let dir = tmp("two");
    let mut c = Client::start(&dir);
    let (_, a) = c.tool(
        "navigate",
        json!({"url": fixture("shop.html"), "page": "new"}),
    );
    let (_, b) = c.tool(
        "navigate",
        json!({"url": fixture("result.html"), "page": "new"}),
    );
    let (pa, pb) = (page_of(&a), page_of(&b));
    assert_ne!(pa.split(':').next(), pb.split(':').next());
    assert!(
        a.contains("Widget Shop") && !b.contains("Widget Shop"),
        "{b}"
    );
    let add = handle_of(&a, "button \"Add to cart\"");
    let (err, eff) = c.tool("act", json!({"handle": add, "action": "click", "page": pa}));
    assert!(!err && eff.contains("Cart (1 item)"), "{eff}");
    let (_, ob) = c.tool("observe", json!({"mode": "full", "page": pb}));
    assert!(
        !ob.contains("Cart (1 item)") && !ob.contains("Widget Shop"),
        "page B saw page A:\n{ob}"
    );
    let (_, oa) = c.tool("observe", json!({"mode": "full", "page": page_of(&eff)}));
    assert!(
        oa.contains("Cart (1 item)") && oa.contains("Widget Shop"),
        "{oa}"
    );
    // page A's log holds only A's calls
    let log_a =
        std::fs::read_to_string(dir.join(format!("{}.jsonl", pa.split(':').next().unwrap())))
            .unwrap();
    assert_eq!(log_a.lines().count(), 3, "{log_a}");
    assert!(!log_a.contains("result.html"));
}

/// A loopback server: `/` is a page with a stylesheet and a script (two subresources) and one
/// button with no handler. Every connection is answered once and closed, so the HTTP cache (which
/// is per-process) never serves the navigation from memory.
fn serve_two_subresource_page() -> String {
    use std::io::{Read, Write as _};
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in l.incoming() {
            let Ok(mut s) = stream else { continue };
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let n = s.read(&mut buf).unwrap_or(0);
                let head = String::from_utf8_lossy(&buf[..n]).to_string();
                let path = head.split_whitespace().nth(1).unwrap_or("/").to_string();
                let (ct, body) = match path.as_str() {
                    "/style.css" => ("text/css", "p { color: red }".to_string()),
                    "/app.js" => ("text/javascript", "void 0".to_string()),
                    _ => ("text/html", "<!doctype html><title>Sub</title><link rel=stylesheet href=\"/style.css\"><script src=\"/app.js\"></script><h1>Sub</h1><button>Go</button>".to_string()),
                };
                let resp = format!("HTTP/1.1 200 OK\r\nContent-Type: {ct}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                let _ = s.write_all(resp.as_bytes());
            });
        }
    });
    format!("http://{addr}/")
}

#[test]
fn req_field_carries_the_wire_request_ids_of_each_call() {
    let dir = tmp("req");
    let url = serve_two_subresource_page();
    let mut c = Client::start(&dir);
    let (e, nav) = c.tool("navigate", json!({"url": url}));
    assert!(!e, "{nav}");
    // the document + stylesheet + script: three requests issued by one navigate
    let go = handle_of(&nav, "button \"Go\"");
    let (e, t) = c.tool(
        "act",
        json!({"handle": go, "action": "click", "page": page_of(&nav)}),
    );
    assert!(!e, "{t}");
    let log = log_lines(&dir);
    assert_eq!(log.len(), 2, "{log:?}");
    let nav_req = log[0]["req"].as_array().unwrap();
    assert!(
        nav_req.len() >= 3,
        "navigate of a 2-subresource page must log its requests: {nav_req:?}"
    );
    // ids are unique and increasing within the navigate
    let ids: Vec<u64> = nav_req.iter().map(|v| v.as_u64().unwrap()).collect();
    assert!(ids.windows(2).all(|w| w[0] < w[1]), "{ids:?}");
    // a click with no handler touches no network: nothing from the navigate leaks into it
    assert_eq!(log[1]["req"], json!([]), "{:?}", log[1]);
}
