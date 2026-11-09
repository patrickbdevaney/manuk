//! RG.E1.b gate (chrome_ver=145.0.7632.116): `extract` text carries a result card's star rating when the
//! rating lives only in an `aria-label` on an empty box (a star widget), so an agent comparing ratings
//! across a list reads every card's rating AND review count, on 3 independent captures. Mutant
//! `MANUK_EXTRACT_MUTATE=off` (no weaving) drops the rating and turns it RED.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{json, Value};

const NAMES: [(&str, &str, &str); 4] = [
    ("Alpha Grill", "4.51", "20"),
    ("Beta Bistro", "3.74", "22"),
    ("Cafe Gamma", "4.20", "15"),
    ("Delta Diner", "4.02", "18"),
];

fn serve() -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            std::thread::spawn(move || {
                let mut buf = [0u8; 8192];
                let _ = s.read(&mut buf);
                let cards: String = NAMES
                    .iter()
                    .map(|(n, r, c)| {
                        format!(
                            r#"<li><a href="/d/{n}"><div>{n}</div><div class="stars" aria-label="Rating: {r}" style="display:inline-block;width:80px;height:16px;background:#fc0"></div><span>Good</span><span>({c})</span><div>Cuisine</div></a></li>"#
                        )
                    })
                    .collect();
                let body = format!(
                    "<!doctype html><html><head><title>Results</title></head><body><h1>Tables</h1><ul>{cards}</ul></body></html>"
                );
                let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
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
    fn start(chrome: bool, env: &[(&str, &str)]) -> Client {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"));
        if chrome {
            cmd.args(["--backend", "chrome"]);
        }
        for (k, v) in env {
            cmd.env(k, v);
        }
        cmd.env(
            "MANUK_ACTLOG_DIR",
            std::env::temp_dir().join(format!("l4-log-{}-{chrome}", std::process::id())),
        );
        cmd.arg("--allow-write").arg(std::env::temp_dir());
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
    fn call(&mut self, name: &str, args: Value) -> (bool, String) {
        let r = self.rpc("tools/call", json!({"name": name, "arguments": args}));
        let res = &r["result"];
        let blocks = res["content"].as_array().cloned().unwrap_or_default();
        (
            res["isError"].as_bool().unwrap_or(false),
            blocks
                .iter()
                .filter_map(|b| b["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n"),
        )
    }
    fn ok(&mut self, name: &str, args: Value) -> String {
        let (e, t) = self.call(name, args);
        assert!(!e, "{name} failed: {t}");
        t
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

fn run(env: &[(&str, &str)]) -> Vec<String> {
    let base = serve();
    let mut missing = Vec::new();
    for k in 0..3 {
        let mut c = Client::start(false, env);
        c.ok("navigate", json!({"url": base}));
        let t = c.ok("extract", json!({}));
        for (n, r, cnt) in NAMES {
            let at = t.find(n).unwrap_or_else(|| panic!("no {n} in:\n{t}"));
            let card = &t[at..];
            let card = &card[..card.find("Cuisine").map_or(card.len(), |i| i + 7)];
            for want in [format!("Rating: {r}"), format!("({cnt})")] {
                if !card.contains(&want) {
                    missing.push(format!("capture {k}: {n} lacks {want}: {card:?}"));
                }
            }
        }
    }
    missing
}

#[test]
fn extract_text_carries_every_cards_rating_and_review_count() {
    let m = run(&[]);
    assert!(m.is_empty(), "{m:#?}");
}

#[test]
fn mutant_without_label_weaving_loses_the_rating() {
    let m = run(&[("MANUK_EXTRACT_MUTATE", "off")]);
    assert!(
        m.iter().any(|l| l.contains("Rating:")),
        "mutant should drop ratings, got {m:#?}"
    );
}
