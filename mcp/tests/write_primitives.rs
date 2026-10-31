//! W3.8 gate: the write primitives (`key`, `upload`, `drag`, `dialog`, `tabs`, `download`) against six
//! local fixture tasks: upload, Escape-to-close modal, arrow-key combobox, confirm(), OAuth popup, and a
//! download. The first three run on both backends; the last three need Chrome (the engine implements no
//! JS dialogs, no second window and no agent-mode download), and a test pins that refusal.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const REPORT: &[u8] = b"id,total\n1,42\n2,17\n";

fn page(path: &str) -> Option<&'static str> {
    Some(match path {
        "/upload" => {
            r#"<!doctype html><title>Upload</title><label>Resume <input type="file" id="f"></label>
<p id="o">none</p><script>var f=document.getElementById('f');f.addEventListener('change',function(){document.getElementById('o').textContent='got:'+f.files[0].name+':'+f.files.length;});</script>"#
        }
        "/modal" => {
            r#"<!doctype html><title>Modal</title><button id="open">Open settings</button>
<div id="m" role="dialog" aria-label="Settings" hidden><p>Settings body</p><button>Inner</button></div><p id="o">state:open-me</p>
<script>document.getElementById('open').onclick=function(){document.getElementById('m').hidden=false;document.getElementById('o').textContent='state:shown';};
document.addEventListener('keydown',function(e){if(e.key==='Escape'){document.getElementById('m').hidden=true;document.getElementById('o').textContent='state:closed';}});</script>"#
        }
        "/combo" => {
            r#"<!doctype html><title>Combo</title><input id="c" role="combobox" aria-label="Fruit" aria-expanded="false"><ul id="l" role="listbox" hidden><li role="option">Alpha</li><li role="option">Beta</li><li role="option">Gamma</li></ul><p id="o">picked:none</p>
<script>var items=['Alpha','Beta','Gamma'],idx=-1,c=document.getElementById('c');
c.addEventListener('keydown',function(e){if(e.key==='ArrowDown'){idx=Math.min(idx+1,2);document.getElementById('l').hidden=false;e.preventDefault();}
else if(e.key==='ArrowUp'){idx=Math.max(idx-1,0);e.preventDefault();}
else if(e.key==='Enter'&&idx>=0){document.getElementById('o').textContent='picked:'+items[idx];document.getElementById('l').hidden=true;e.preventDefault();}});</script>"#
        }
        "/confirm" => {
            r#"<!doctype html><title>Confirm</title><button id="d">Discard draft</button><p id="o">draft:present</p>
<script>document.getElementById('d').onclick=function(){document.getElementById('o').textContent=confirm('Really discard the draft?')?'draft:discarded':'draft:kept';};</script>"#
        }
        "/oauth" => {
            r#"<!doctype html><title>App</title><button id="s">Sign in with Acme</button><p id="o">signed:out</p>
<script>document.getElementById('s').onclick=function(){window.open('/popup','acme','width=400,height=300');};
window.addEventListener('message',function(e){document.getElementById('o').textContent='signed:'+e.data;});</script>"#
        }
        "/popup" => {
            r#"<!doctype html><title>Acme</title><p>Acme wants access</p><button id="a">Approve access</button>
<script>document.getElementById('a').onclick=function(){opener.postMessage('token-abc123','*');window.close();};</script>"#
        }
        "/dl" => {
            r#"<!doctype html><title>Files</title><a href="/report.csv" download>Get report</a> <a href="/big.bin">Get big file</a> <a href="/evil">Get sneaky</a>"#
        }
        _ => return None,
    })
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
                let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
                if path == "/report.csv" {
                    let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: text/csv\r\nContent-Disposition: attachment; filename=\"report.csv\"\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", REPORT.len());
                    let _ = s.write_all(REPORT);
                } else if path == "/evil" {
                    let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Disposition: attachment; filename=\"../../escaped.sh\"\r\nContent-Length: 4\r\nConnection: close\r\n\r\n#!sh");
                } else if path == "/big.bin" {
                    let total: u64 = 200 * 1024 * 1024;
                    let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Disposition: attachment; filename=\"big.bin\"\r\nContent-Length: {total}\r\nConnection: close\r\n\r\n");
                    let chunk = vec![0u8; 1 << 20];
                    for _ in 0..200 {
                        if s.write_all(&chunk).is_err() {
                            break;
                        }
                    }
                } else if let Some(b) = page(&path) {
                    let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{b}", b.len());
                } else {
                    let _ = write!(
                        s,
                        "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    );
                }
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
    fn start(chrome: bool, extra: &[&str]) -> Client {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"));
        if chrome {
            cmd.args(["--backend", "chrome"]);
        }
        cmd.env(
            "MANUK_ACTLOG_DIR",
            std::env::temp_dir().join(format!("cb1-log-{}-{chrome}", std::process::id())),
        );
        cmd.args(extra);
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
    /// (is_error, text, base64 image data if any)
    fn call(&mut self, name: &str, args: Value) -> (bool, String, Option<String>) {
        let r = self.rpc("tools/call", json!({"name": name, "arguments": args}));
        let res = &r["result"];
        let blocks = res["content"].as_array().cloned().unwrap_or_default();
        let text = blocks
            .iter()
            .filter_map(|b| b["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let img = blocks
            .iter()
            .find(|b| b["type"] == "image")
            .and_then(|b| b["data"].as_str().map(String::from));
        (res["isError"].as_bool().unwrap_or(false), text, img)
    }
    fn tool(&mut self, name: &str, args: Value) -> String {
        let (err, t, _) = self.call(name, args);
        assert!(!err, "{name} failed: {t}");
        t
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

fn handle_of(text: &str, needle: &str) -> String {
    let line = text
        .lines()
        .find(|l| l.contains(needle))
        .unwrap_or_else(|| panic!("no {needle:?} in:\n{text}"));
    let s = line.find("[h").unwrap();
    let e = line[s..].find(']').unwrap() + s;
    line[s + 1..e].to_string()
}

fn tmp(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!(
        "w38-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn upload(chrome: bool) {
    let base = serve();
    let dir = tmp("up");
    let file = dir.join("cv.txt");
    std::fs::write(&file, "hello resume").unwrap();
    let mut c = Client::start(chrome, &[]);
    let nav = c.tool("navigate", json!({"url": format!("{base}/upload")}));
    let h = handle_of(&nav, "Resume");
    let r = c.tool(
        "upload",
        json!({"handle": h, "path": file.display().to_string()}),
    );
    let seen = c.tool("extract", json!({}));
    assert!(
        seen.contains("got:cv.txt:1"),
        "upload (chrome={chrome}) not seen by the page:\n{r}\n{seen}"
    );
    // A page-steered agent must not be able to attach files from outside the allowed roots.
    let (err, t, _) = c.call("upload", json!({"handle": h, "path": "/etc/passwd"}));
    assert!(
        err && t.contains("REFUSED"),
        "etc/passwd upload must be refused (chrome={chrome}): {t}"
    );
}

fn escape_modal(chrome: bool) {
    let base = serve();
    let mut c = Client::start(chrome, &[]);
    let nav = c.tool("navigate", json!({"url": format!("{base}/modal")}));
    let h = handle_of(&nav, "Open settings");
    let r = c.tool("act", json!({"handle": h, "action": "click"}));
    let seen = c.tool("extract", json!({}));
    assert!(
        seen.contains("state:shown"),
        "modal did not open (chrome={chrome}):\n{r}\n{seen}"
    );
    let r = c.tool("key", json!({"key": "Escape"}));
    let seen = format!("{r}\n{}", c.tool("extract", json!({})));
    assert!(
        seen.contains("state:closed"),
        "Escape did not close the modal (chrome={chrome}):\n{seen}"
    );
}

fn combobox(chrome: bool) {
    let base = serve();
    let mut c = Client::start(chrome, &[]);
    let nav = c.tool("navigate", json!({"url": format!("{base}/combo")}));
    let h = handle_of(&nav, "Fruit");
    c.tool("key", json!({"key": "ArrowDown", "handle": h}));
    c.tool("key", json!({"key": "ArrowDown"}));
    c.tool("key", json!({"key": "Enter"}));
    let seen = c.tool("extract", json!({}));
    assert!(
        seen.contains("picked:Beta"),
        "arrow-key combobox (chrome={chrome}):\n{seen}"
    );
}

#[test]
fn upload_manuk() {
    upload(false);
}
#[test]
fn upload_chrome() {
    upload(true);
}
#[test]
fn escape_closes_modal_manuk() {
    escape_modal(false);
}
#[test]
fn escape_closes_modal_chrome() {
    escape_modal(true);
}
#[test]
fn arrow_combobox_manuk() {
    combobox(false);
}
#[test]
fn arrow_combobox_chrome() {
    combobox(true);
}

#[test]
fn confirm_dialog_is_answered_by_the_agent_chrome() {
    let base = serve();
    let mut c = Client::start(true, &[]);
    let nav = c.tool("navigate", json!({"url": format!("{base}/confirm")}));
    let h = handle_of(&nav, "Discard draft");
    // Unarmed: a confirm is dismissed, the draft survives, and the result says so.
    let r = c.tool("act", json!({"handle": h, "action": "click"}));
    assert!(
        r.contains("dialog: confirm") && r.contains("dismissed") && r.contains("draft:kept"),
        "unarmed confirm:\n{r}"
    );
    c.tool("dialog", json!({"action": "accept"}));
    let r = c.tool("act", json!({"handle": h, "action": "click"}));
    assert!(
        r.contains("accepted") && r.contains("draft:discarded"),
        "armed accept:\n{r}"
    );
}

#[test]
fn dialogs_are_refused_loudly_on_the_manuk_backend() {
    let mut c = Client::start(false, &[]);
    let (err, t, _) = c.call("dialog", json!({"action": "accept"}));
    assert!(err && t.contains("no JS dialogs"), "{t}");
}

#[test]
fn oauth_popup_is_surfaced_and_the_agent_returns_when_it_closes_chrome() {
    let base = serve();
    let mut c = Client::start(true, &[]);
    let nav = c.tool("navigate", json!({"url": format!("{base}/oauth")}));
    let h = handle_of(&nav, "Sign in with Acme");
    let r = c.tool("act", json!({"handle": h, "action": "click"}));
    assert!(
        r.contains("popup opened: tab 2"),
        "popup not surfaced:\n{r}"
    );
    let seen = c.tool("observe", json!({}));
    let ap = handle_of(&seen, "Approve access");
    let tabs = c.tool("tabs", json!({}));
    assert!(tabs.contains("* 2") && tabs.contains("  1"), "{tabs}");
    let r = c.tool("act", json!({"handle": ap, "action": "click"}));
    assert!(
        r.contains("back on tab 1"),
        "did not return to the opener:\n{r}"
    );
    let seen = c.tool("extract", json!({}));
    assert!(
        seen.contains("signed:token-abc123"),
        "opener never got the token:\n{seen}"
    );
}

fn mode(p: &std::path::Path) -> u32 {
    std::fs::metadata(p).unwrap().permissions().mode() & 0o777
}

#[test]
fn download_lands_in_the_task_dir_and_nowhere_else_chrome() {
    let base = serve();
    let dl = tmp("dl");
    let dls = dl.display().to_string();
    let mut c = Client::start(true, &["--download-dir", &dls]);
    let nav = c.tool("navigate", json!({"url": format!("{base}/dl")}));
    let h = handle_of(&nav, "Get report");
    let r = c.tool("download", json!({"handle": h}));
    let want: String = Sha256::digest(REPORT)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert!(
        r.contains("\"report.csv\"")
            && r.contains(&want)
            && r.contains("bytes: 19")
            && r.contains("text/csv"),
        "{r}"
    );
    let f = std::fs::canonicalize(&dl).unwrap().join("report.csv");
    assert_eq!(std::fs::read(&f).unwrap(), REPORT);
    assert_eq!(mode(&f), 0o600, "download must be 0600, never executable");
    // A hostile filename cannot escape the directory.
    let h = handle_of(&nav, "Get sneaky");
    let r = c.tool("download", json!({"handle": h}));
    assert!(
        r.contains("escaped.sh") && !r.contains("name: \"../"),
        "{r}"
    );
    assert!(!std::fs::canonicalize(&dl)
        .unwrap()
        .parent()
        .unwrap()
        .join("escaped.sh")
        .exists());
    let landed = std::fs::read_dir(&dl)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.to_string_lossy().contains("escaped.sh"))
        .expect("sneaky file not in the download dir");
    assert_eq!(mode(&landed), 0o600);
    // 200 MB response: aborted at the cap, nothing kept.
    let h = handle_of(&nav, "Get big file");
    let (err, t, _) = c.call("download", json!({"handle": h}));
    assert!(
        err && t.contains("cap"),
        "200 MB download must abort at the cap: {t}"
    );
    std::thread::sleep(std::time::Duration::from_millis(500));
    assert!(
        !std::fs::canonicalize(&dl).unwrap().join("big.bin").exists(),
        "oversized file kept"
    );
    let total: u64 = std::fs::read_dir(&dl)
        .unwrap()
        .map(|e| e.unwrap().metadata().unwrap().len())
        .sum();
    assert!(
        total < 1 << 20,
        "directory holds {total} bytes after an aborted download"
    );
}

#[test]
fn download_without_the_capability_writes_nothing_chrome() {
    let base = serve();
    let home_dl = std::env::var("HOME")
        .map(|h| std::path::PathBuf::from(h).join("Downloads/report.csv"))
        .unwrap();
    let existed = home_dl.exists();
    let mut c = Client::start(true, &[]);
    let nav = c.tool("navigate", json!({"url": format!("{base}/dl")}));
    let h = handle_of(&nav, "Get report");
    let (err, t, _) = c.call("download", json!({"handle": h}));
    assert!(err && t.contains("no Download capability"), "{t}");
    c.call("act", json!({"handle": h, "action": "click"}));
    std::thread::sleep(std::time::Duration::from_millis(800));
    assert_eq!(
        home_dl.exists(),
        existed,
        "a download escaped to ~/Downloads"
    );
}
