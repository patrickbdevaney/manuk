//! W5.T1 gate: the lean default observation. On a fixed 20-page set, on BOTH backends, the median
//! tokens (chars/4, the server's own estimate) per `observe` falls >= 40% against the old format,
//! which `MANUK_OBSERVE_MUTATE=legacy` restores (that is the baseline and the RED mutant). Handles
//! appear only on operable elements, a table row is one line, handles stay stable across modes.
//!
//! chrome_ver=145.0.7632.116 (the chrome backend's pin).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{json, Value};

const N_PAGES: usize = 20;

/// Twenty page shapes: prose, tables, nav-heavy lists, forms, cards, mixed. Deterministic.
fn page(i: usize) -> String {
    let mut b = format!("<!doctype html><html><head><title>Page {i}</title></head><body>");
    b.push_str("<header><nav><a href=\"/\">Home</a> <a href=\"/a\">About</a> <a href=\"/b\">Docs</a></nav></header><main>");
    b.push_str(&format!("<h1>Page number {i}</h1>"));
    match i % 5 {
        0 => {
            for p in 0..12 {
                b.push_str(&format!("<p>Paragraph {p} of page {i}: the quick brown fox jumps over the lazy dog while the committee reviews the quarterly figures.</p>"));
            }
            b.push_str("<a href=\"/more\">Read more</a>");
        }
        1 => {
            b.push_str("<table><tr><th>Name</th><th>Qty</th><th>Price</th><th>Status</th></tr>");
            for r in 0..25 {
                b.push_str(&format!(
                    "<tr><td>Item {r}</td><td>{}</td><td>${}.99</td><td>in stock</td></tr>",
                    r + 1,
                    r * 3 + 2
                ));
            }
            b.push_str("</table><button>Export</button>");
        }
        2 => {
            b.push_str("<ul>");
            for r in 0..30 {
                b.push_str(&format!("<li><a href=\"/s/{r}\">Story {r} headline for page {i}</a> <span>{r} points by user{r}</span></li>"));
            }
            b.push_str("</ul>");
        }
        3 => {
            b.push_str("<form><label>Email <input type=\"text\" name=\"e\"></label><label>Name <input type=\"text\" name=\"n\"></label><label><input type=\"checkbox\"> Subscribe</label><button>Send</button></form>");
            for p in 0..6 {
                b.push_str(&format!("<p>Terms clause {p}: you agree to the conditions stated herein on page {i}.</p>"));
            }
        }
        _ => {
            for c in 0..10 {
                b.push_str(&format!("<article><h2>Product {c}</h2><p>A durable thing number {c} with a long description for page {i}.</p><span>${c}9.00</span><button>Add to cart</button></article>"));
            }
        }
    }
    b.push_str("</main><footer><p>Copyright notice and legal text for the site.</p><a href=\"/privacy\">Privacy</a></footer></body></html>");
    b
}

fn serve() -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let path = req.split_whitespace().nth(1).unwrap_or("/p0");
                let i: usize = path.trim_start_matches("/p").parse().unwrap_or(0);
                let body = page(i);
                let _ = write!(
                    s,
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = s.write_all(body.as_bytes());
            });
        }
    });
    format!("http://{addr}")
}

struct Client {
    child: Child,
    tx: ChildStdin,
    rx: BufReader<ChildStdout>,
    id: u64,
}

impl Client {
    fn start(chrome: bool, legacy: bool) -> Client {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"));
        if legacy {
            cmd.env("MANUK_OBSERVE_MUTATE", "legacy");
        }
        if chrome {
            cmd.args(["--backend", "chrome"]);
        }
        cmd.env(
            "MANUK_ACTLOG_DIR",
            std::env::temp_dir().join(format!("w5t1-log-{}-{chrome}-{legacy}", std::process::id())),
        );
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
    fn tool(&mut self, name: &str, args: Value) -> String {
        let r = self.rpc("tools/call", json!({"name": name, "arguments": args}));
        let res = &r["result"];
        let text = res["content"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|b| b["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !res["isError"].as_bool().unwrap_or(false),
            "{name} failed: {text}"
        );
        text
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

fn tokens(s: &str) -> usize {
    s.len().div_ceil(4)
}

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(|a, b| a.total_cmp(b));
    v[v.len() / 2]
}

/// Tokens per observe for every page, in page order.
fn per_page(base: &str, chrome: bool, legacy: bool) -> Vec<usize> {
    let mut c = Client::start(chrome, legacy);
    (0..N_PAGES)
        .map(|i| {
            c.tool("navigate", json!({"url": format!("{base}/p{i}")}));
            tokens(&c.tool("observe", json!({"budget_tokens": 100000})))
        })
        .collect()
}

/// (median per-page cut, total cut)
fn cuts(chrome: bool) -> (f64, f64) {
    let base = serve();
    let old = per_page(&base, chrome, true);
    let new = per_page(&base, chrome, false);
    let mut cuts: Vec<f64> = old
        .iter()
        .zip(&new)
        .map(|(o, n)| 1.0 - *n as f64 / *o as f64)
        .collect();
    eprintln!("W5T1 per-page old={old:?} new={new:?}");
    let m = median(&mut cuts);
    eprintln!(
        "W5T1 backend={} median_cut={:.3} old_total={} new_total={}",
        if chrome { "chrome" } else { "manuk" },
        m,
        old.iter().sum::<usize>(),
        new.iter().sum::<usize>()
    );
    (
        m,
        1.0 - new.iter().sum::<usize>() as f64 / old.iter().sum::<usize>() as f64,
    )
}

fn chrome_present() -> bool {
    std::env::var_os("MANUK_CHROME").is_some()
        || ["google-chrome", "chromium", "chromium-browser", "chrome"]
            .iter()
            .any(|b| Command::new(b).arg("--version").output().is_ok())
}

// The pre-registered gate: the MEDIAN per-page cut >= 40% on both backends. W5.T1 measured 0.226 /
// 0.284; W5.T1.a closed it (bare paragraphs, `#` headings, article context dropped under its own
// heading): 0.412 manuk / 0.421 chrome on the unchanged page set.
#[test]
fn gate_median_cut_is_40_percent_on_both_backends() {
    assert!(cuts(false).0 >= 0.40, "manuk median cut < 0.40");
    if chrome_present() {
        assert!(cuts(true).0 >= 0.40, "chrome median cut < 0.40");
    }
}

#[test]
fn lean_observe_cuts_total_tokens_on_the_manuk_backend() {
    let (_, t) = cuts(false);
    assert!(t >= 0.40, "total cut {t:.3} < 0.40");
}

#[test]
fn lean_observe_cuts_total_tokens_on_the_chrome_backend() {
    if !chrome_present() {
        eprintln!("W5T1 chrome backend: no chrome on this machine, stood down");
        return;
    }
    let (_, t) = cuts(true);
    assert!(t >= 0.40, "total cut {t:.3} < 0.40");
}

#[test]
fn handles_mark_operable_elements_only_and_stay_stable() {
    let base = serve();
    let mut c = Client::start(false, false);
    c.tool("navigate", json!({"url": format!("{base}/p1")}));
    let full = c.tool("observe", json!({"budget_tokens": 100000}));
    for l in full.lines().filter(|l| l.contains("[h")) {
        assert!(
            ["link", "button", "textbox", "checkbox"]
                .iter()
                .any(|r| l.contains(&format!("] {r}"))),
            "handle on a non-operable line: {l}"
        );
    }
    // a table row is one line carrying its cells
    assert!(
        full.lines()
            .any(|l| l.starts_with("- row") && l.contains("Item 3 | 4 |")),
        "no one-line row in:\n{full}"
    );
    assert!(
        !full.contains("- cell"),
        "cells not folded into rows:\n{full}"
    );
    let h = full
        .lines()
        .find(|l| l.contains("button \"Export\""))
        .map(|l| l[l.find("[h").unwrap()..l.find(']').unwrap() + 1].to_string())
        .unwrap();
    let inter = c.tool(
        "observe",
        json!({"mode": "interactive", "budget_tokens": 100000}),
    );
    assert!(inter.contains(&format!("{h} button \"Export\"")), "{inter}");
    assert!(!inter.contains("- row"), "{inter}");
    let content = c.tool(
        "observe",
        json!({"mode": "content", "budget_tokens": 100000}),
    );
    assert!(
        content.contains("- row") && !content.contains("[h"),
        "{content}"
    );
    // the handle still resolves after the other modes
    c.tool(
        "act",
        json!({"handle": h.trim_matches(|ch| ch == '[' || ch == ']'), "action": "click"}),
    );
}
