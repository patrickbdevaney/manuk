//! W3.13 gate: world-honest rollback. A real loopback HTTP server holds the cart, so a restore that
//! only rewinds the client is caught, and a forked branch's writes are held until `commit`.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use serde_json::{json, Value};

struct Client {
    child: Child,
    tx: ChildStdin,
    rx: BufReader<ChildStdout>,
    id: u64,
}

impl Client {
    fn start(log: &std::path::Path) -> Client {
        let state = log.join("state");
        std::fs::create_dir_all(&state).unwrap();
        let mut child = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"))
            .env("MANUK_ACTLOG_DIR", log)
            .env("MANUK_STATE", &state)
            .env("MANUK_ALLOW_PRIVATE_NETWORK", "1")
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

const PAGE: &str = r#"<!doctype html><html><head><title>Shop</title></head><body>
<button id="a">Set A</button><button id="b">Set B</button><button id="show">Show</button>
<button id="add">Add to cart</button><button id="order">Place order</button><button id="beacon">Beacon</button>
<script>
function dlg(t){var d=document.createElement('div');d.setAttribute('role','dialog');d.setAttribute('aria-label',t);document.body.appendChild(d);}
document.getElementById('a').addEventListener('click',function(){localStorage.setItem('v','first');});
document.getElementById('b').addEventListener('click',function(){localStorage.setItem('v','second');});
document.getElementById('show').addEventListener('click',function(){dlg('value='+localStorage.getItem('v'));});
document.getElementById('add').addEventListener('click',function(){fetch('/add',{method:'POST',body:'x'}).then(function(){dlg('added');}).catch(function(){dlg('add failed');});});
document.getElementById('order').addEventListener('click',function(){fetch('/order',{method:'POST',body:'x'}).then(function(){dlg('ordered');}).catch(function(){dlg('order failed');});});
document.getElementById('beacon').addEventListener('click',function(){navigator.sendBeacon('/beacon','x');});
</script></body></html>"#;

/// A tiny loopback server that counts the POSTs it receives. Returns its base URL + the counters
/// (add, order, beacon).
fn server() -> (String, Arc<[AtomicUsize; 3]>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let n = Arc::new([
        AtomicUsize::new(0),
        AtomicUsize::new(0),
        AtomicUsize::new(0),
    ]);
    let n2 = n.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let n = n2.clone();
            std::thread::spawn(move || serve(s, &n));
        }
    });
    (base, n)
}

fn serve(mut s: TcpStream, n: &[AtomicUsize; 3]) {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    loop {
        let Ok(k) = s.read(&mut tmp) else { return };
        if k == 0 {
            return;
        }
        buf.extend_from_slice(&tmp[..k]);
        let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") else {
            continue;
        };
        let head = String::from_utf8_lossy(&buf[..end]).to_string();
        let clen = head
            .lines()
            .find_map(|l| {
                l.to_ascii_lowercase()
                    .strip_prefix("content-length:")
                    .map(|v| v.trim().parse::<usize>().unwrap_or(0))
            })
            .unwrap_or(0);
        if buf.len() < end + 4 + clen {
            continue;
        }
        let line = head.lines().next().unwrap_or("");
        let (status, ctype, body) = match line.split(' ').take(2).collect::<Vec<_>>()[..] {
            ["GET", "/shop"] => (200, "text/html", PAGE.to_string()),
            ["GET", "/state"] => (
                200,
                "text/plain",
                format!(
                    "add={} order={} beacon={}",
                    n[0].load(Ordering::SeqCst),
                    n[1].load(Ordering::SeqCst),
                    n[2].load(Ordering::SeqCst)
                ),
            ),
            ["POST", "/add"] => {
                n[0].fetch_add(1, Ordering::SeqCst);
                (200, "text/plain", "ok".into())
            }
            ["POST", "/order"] => {
                n[1].fetch_add(1, Ordering::SeqCst);
                (200, "text/plain", "ok".into())
            }
            ["POST", "/beacon"] => {
                n[2].fetch_add(1, Ordering::SeqCst);
                (204, "text/plain", String::new())
            }
            _ => (404, "text/plain", "no".into()),
        };
        let resp = format!("HTTP/1.1 {status} X\r\ncontent-type: {ctype}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len());
        let _ = s.write_all(resp.as_bytes());
        return;
    }
}

fn state(base: &str) -> String {
    let mut s = TcpStream::connect(base.trim_start_matches("http://")).unwrap();
    write!(
        s,
        "GET /state HTTP/1.1\r\nhost: x\r\nconnection: close\r\n\r\n"
    )
    .unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    out.rsplit("\r\n\r\n").next().unwrap().to_string()
}

fn tmp(tag: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!(
        "manuk-world-{tag}-{}-{:?}",
        std::process::id(),
        std::time::Instant::now()
    ));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
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

fn page_of(text: &str) -> String {
    text.lines()
        .rev()
        .find_map(|l| l.strip_prefix("page: "))
        .unwrap_or_else(|| panic!("no page handle in:\n{text}"))
        .to_string()
}

fn ckpt_of(t: &str) -> String {
    t.split_whitespace()
        .skip_while(|w| *w != "checkpoint")
        .nth(1)
        .unwrap()
        .to_string()
}

/// click the button named `name` on page `p` (re-observing for fresh handles); returns (effect, new page handle).
fn click(c: &mut Client, p: &str, name: &str) -> (String, String) {
    let (e, obs) = c.tool("observe", json!({"mode": "full", "page": p}));
    assert!(!e, "{obs}");
    let h = handle_of(&obs, &format!("button \"{name}\""));
    let (e, eff) = c.tool(
        "act",
        json!({"handle": h, "action": "click", "page": page_of(&obs)}),
    );
    assert!(!e, "{eff}");
    let np = page_of(&eff);
    (eff, np)
}

#[test]
fn local_storage_set_checkpoint_set_restore_returns_the_first_value() {
    let (base, _) = server();
    let mut c = Client::start(&tmp("ls"));
    let (_, nav) = c.tool("navigate", json!({"url": format!("{base}/shop")}));
    let (_, p) = click(&mut c, &page_of(&nav), "Set A");
    let (e, ck) = c.tool("checkpoint", json!({"page": p}));
    assert!(!e && ck.contains("[CLIENT]"), "{ck}");
    let (_, p) = click(&mut c, &page_of(&ck), "Set B");
    let (shown, _) = click(&mut c, &p, "Show");
    assert!(shown.contains("value=second"), "{shown}");
    let (e, r) = c.tool("restore", json!({"checkpoint": ckpt_of(&ck)}));
    assert!(!e && r.contains("not diverged"), "{r}");
    let (eff, _) = click(&mut c, &page_of(&r), "Show");
    assert!(
        eff.contains("value=first"),
        "restore did not bring back the first value:\n{eff}"
    );
}

#[test]
fn server_side_cart_restore_reports_server_diverged_and_server_count_stays() {
    let (base, _) = server();
    let mut c = Client::start(&tmp("srv"));
    let (_, nav) = c.tool("navigate", json!({"url": format!("{base}/shop")}));
    let (e, ck) = c.tool("checkpoint", json!({"page": page_of(&nav)}));
    assert!(!e && ck.contains("[CLIENT]"), "{ck}");
    let (eff, _) = click(&mut c, &page_of(&ck), "Add to cart");
    assert!(eff.contains("added"), "{eff}");
    assert_eq!(state(&base), "add=1 order=0 beacon=0");
    let (e, r) = c.tool("restore", json!({"checkpoint": ckpt_of(&ck)}));
    assert!(!e, "{r}");
    assert!(
        r.contains("SERVER-DIVERGED") && r.contains("POST /add"),
        "restore claimed a clean rollback:\n{r}"
    );
    assert_eq!(
        state(&base),
        "add=1 order=0 beacon=0",
        "the server count must be untouched by restore"
    );
    // a checkpoint taken after the write is tagged WORLD
    let (_, ck2) = c.tool("checkpoint", json!({"page": page_of(&r)}));
    assert!(ck2.contains("[WORLD]"), "{ck2}");
}

#[test]
fn forked_branch_holds_post_and_beacon_until_commit_which_sends_exactly_once() {
    let (base, _) = server();
    let mut c = Client::start(&tmp("fork"));
    let (_, nav) = c.tool("navigate", json!({"url": format!("{base}/shop")}));
    let (_, ck) = c.tool("checkpoint", json!({"page": page_of(&nav)}));
    let (e, f) = c.tool("fork", json!({"checkpoint": ckpt_of(&ck)}));
    assert!(!e && f.contains("write-fence ON"), "{f}");
    let (eff, p) = click(&mut c, &page_of(&f), "Place order");
    assert!(eff.contains("held=[POST /order]"), "{eff}");
    assert_eq!(
        state(&base),
        "add=0 order=0 beacon=0",
        "a fenced branch must send zero POSTs"
    );
    let (eff, p) = click(&mut c, &p, "Beacon");
    assert!(eff.contains("held=[POST /order, POST /beacon]"), "{eff}");
    assert_eq!(state(&base), "add=0 order=0 beacon=0");
    let (e, cm) = c.tool("commit", json!({"page": p}));
    assert!(!e && cm.contains("sent 2"), "{cm}");
    assert_eq!(
        state(&base),
        "add=0 order=1 beacon=1",
        "commit must send exactly one of each"
    );
    // after commit the branch is live: a second order goes straight through
    let (_, p) = click(&mut c, &page_of(&cm), "Place order");
    assert_eq!(state(&base), "add=0 order=2 beacon=1", "{p}");
    // a second commit has nothing left to send
    let (_, cm2) = c.tool("commit", json!({"page": p}));
    assert!(cm2.contains("sent 0"), "{cm2}");
    assert_eq!(state(&base), "add=0 order=2 beacon=1");
}

/// Every place in manuk-net that can put bytes on the wire, and why each is covered. A new one that
/// is not listed here fails this test: that is the enumeration the node asks for.
#[test]
fn every_wire_path_in_manuk_net_is_fenced_or_cannot_mutate() {
    // (file, primitive, how it is covered)
    const KNOWN: &[(&str, &str, &str)] = &[
        ("lib.rs", "Client::builder", "send_raw: ledger::gate before the hyper client (fetch/XHR/beacon/form/streaming/redirects)"),
        ("websocket.rs", "TcpStream::connect", "WebSocketConn::connect: ledger::gate(\"WS\") first"),
        ("proxy.rs", "TcpStream::connect", "SOCKS test server only (cfg(test))"),
        ("proxy.rs", "send_request", "fetch_via_proxy: GET only, safe by construction"),
        ("aia.rs", "TcpStream::connect", "AIA certificate fetch: GET only, safe by construction"),
    ];
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../engine/net/src");
    let mut found = Vec::new();
    for ent in std::fs::read_dir(&dir).unwrap().flatten() {
        let name = ent.file_name().to_string_lossy().to_string();
        if !name.ends_with(".rs") || name == "ledger.rs" {
            continue;
        }
        let text = std::fs::read_to_string(ent.path()).unwrap();
        for prim in [
            "TcpStream::connect",
            "Client::builder",
            "UdpSocket",
            "send_request",
            "WebTransport",
            "reqwest::",
        ] {
            if text.contains(prim) {
                found.push((name.clone(), prim));
            }
        }
    }
    for (f, p) in &found {
        assert!(
            KNOWN.iter().any(|(kf, kp, _)| kf == f && kp == p),
            "{f} uses {p}, a wire path that is not in the fence enumeration: gate it with ledger::gate or prove it GET-only, then list it"
        );
    }
    // and the gate really is on the two mutating paths
    let lib = std::fs::read_to_string(dir.join("lib.rs")).unwrap();
    let send_raw = lib.split("async fn send_raw(").nth(1).unwrap();
    assert!(
        send_raw[..send_raw.find("\n}\n").unwrap()]
            .lines()
            .any(|l| !l.trim_start().starts_with("//") && l.contains("ledger::gate(")),
        "send_raw lost its fence"
    );
    let ws = std::fs::read_to_string(dir.join("websocket.rs")).unwrap();
    assert!(
        ws.lines()
            .any(|l| !l.trim_start().starts_with("//") && l.contains("ledger::gate(\"WS\"")),
        "WebSocket connect lost its fence"
    );
}
