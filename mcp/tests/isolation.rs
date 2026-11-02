//! SEC.DID.1 mutation record (chrome_ver=145.0.7632.116): `mcp/src/isolation.rs` grant loop
//! `for origin in &o.grant_origins` -> `.take(0)` turns human_profile_cookie_is_invisible RED.
//! SEC.DID.1.a: removing `manuk_net::set_ephemeral(true)` (the `None =>` arm of `apply`) was GREEN
//! under the old tests because the session process cannot write to MANUK_STATE anyway (no file
//! appears even in the mutant), so `ephemeral_session_leaves_nothing_on_disk` never saw it. The pin
//! is the read side: `ephemeral_session_does_not_load_the_human_jar` seeds MANUK_STATE/cookies.json
//! and the mutant turns it RED (chrome_ver=145.0.7632.116).
//!
//! W3.4 gate: session isolation over real stdio against the built `manuk-mcp` binary and a local
//! cookie server. Each session is its own process.

use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::net::TcpListener;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{json, Value};

const SECRET: &str = "sidval-7f3a9c41e2b85d06";

/// `/login` sets a persistent cookie; every other path prints the Cookie header it received.
fn server() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            std::thread::spawn(move || {
                let mut r = BufReader::new(s.try_clone().unwrap());
                let mut first = String::new();
                r.read_line(&mut first).unwrap();
                let mut cookie = String::new();
                loop {
                    let mut h = String::new();
                    if r.read_line(&mut h).unwrap() == 0 || h.trim().is_empty() {
                        break;
                    }
                    if let Some(v) = h.to_ascii_lowercase().strip_prefix("cookie:") {
                        cookie = v.trim().to_string();
                    }
                }
                let set = if first.contains("/login") {
                    format!("Set-Cookie: sid={SECRET}; Path=/; Max-Age=3600\r\n")
                } else {
                    String::new()
                };
                let body = format!("<html><body><p>cookie-seen:[{cookie}]</p></body></html>");
                let mut s = s;
                let _ = write!(
                    s,
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n{set}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            });
        }
    });
    port
}

struct Client {
    child: Child,
    tx: ChildStdin,
    rx: BufReader<ChildStdout>,
    id: u64,
}

impl Client {
    fn start(args: &[&str]) -> Client {
        Self::try_start(args).expect("spawn manuk-mcp")
    }
    fn try_start(args: &[&str]) -> Option<Client> {
        let mut child = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"))
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let tx = child.stdin.take().unwrap();
        let rx = BufReader::new(child.stdout.take().unwrap());
        let mut c = Client {
            child,
            tx,
            rx,
            id: 0,
        };
        let init = c.rpc("initialize", json!({"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "t", "version": "0"}}))?;
        init["result"]["capabilities"]["tools"].as_object()?;
        c.send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        Some(c)
    }
    fn send(&mut self, v: Value) {
        let _ = writeln!(self.tx, "{v}");
        let _ = self.tx.flush();
    }
    fn rpc(&mut self, method: &str, params: Value) -> Option<Value> {
        self.id += 1;
        let id = self.id;
        self.send(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        loop {
            let mut line = String::new();
            if self.rx.read_line(&mut line).ok()? == 0 {
                return None;
            }
            let v: Value = serde_json::from_str(&line).ok()?;
            if v["id"] == json!(id) {
                return Some(v);
            }
        }
    }
    fn tool(&mut self, name: &str, args: Value) -> String {
        let r = self
            .rpc("tools/call", json!({"name": name, "arguments": args}))
            .expect("server alive");
        r["result"]["content"]
            .as_array()
            .map(|c| {
                c.iter()
                    .filter_map(|b| b["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default()
    }
    fn seen(&mut self, port: u16) -> String {
        self.tool(
            "navigate",
            json!({"url": format!("http://127.0.0.1:{port}/whoami")}),
        );
        self.tool("extract", json!({"format": "text"}))
    }
    /// Does this process's memory contain `needle`?
    fn mem_contains(&self, needle: &str) -> bool {
        let pid = self.child.id();
        let maps = std::fs::read_to_string(format!("/proc/{pid}/maps")).unwrap();
        let mut mem = std::fs::File::open(format!("/proc/{pid}/mem")).expect("open mem");
        let mut buf = Vec::new();
        for l in maps.lines() {
            let mut f = l.split_whitespace();
            let (range, perms) = (f.next().unwrap(), f.next().unwrap());
            if !perms.starts_with("rw") || l.contains("[vvar") {
                continue;
            }
            let (a, b) = range.split_once('-').unwrap();
            let (a, b) = (
                u64::from_str_radix(a, 16).unwrap(),
                u64::from_str_radix(b, 16).unwrap(),
            );
            if mem.seek(SeekFrom::Start(a)).is_err() {
                continue;
            }
            buf.clear();
            if (&mut mem).take(b - a).read_to_end(&mut buf).is_err() && buf.is_empty() {
                continue;
            }
            if buf.windows(needle.len()).any(|w| w == needle.as_bytes()) {
                return true;
            }
        }
        false
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn tmp(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("manuk-iso-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn task_b_sees_no_cookie_of_task_a_and_survives_its_death() {
    let port = server();
    let mut a = Client::start(&[]);
    let mut b = Client::start(&[]);
    a.tool(
        "navigate",
        json!({"url": format!("http://127.0.0.1:{port}/login")}),
    );
    assert!(a.seen(port).contains(SECRET), "A logged in");
    assert!(
        b.seen(port).contains("cookie-seen:[]"),
        "B must not see A's cookie"
    );
    // The secret is in A's address space and never in B's (positive control: the check can go red).
    assert!(
        a.mem_contains(SECRET),
        "positive control: A holds the cookie value in memory"
    );
    assert!(
        !b.mem_contains(SECRET),
        "B's memory must not contain A's cookie value"
    );
    drop(a);
    assert!(
        b.seen(port).contains("cookie-seen:[]"),
        "B keeps serving after A is killed"
    );
}

#[test]
fn ephemeral_session_leaves_nothing_on_disk() {
    let port = server();
    let state = tmp("state");
    let mut a = Client::try_start_env(&state, port);
    assert!(a.seen(port).contains(SECRET));
    drop(a);
    assert!(
        std::fs::read_dir(&state).unwrap().next().is_none(),
        "ephemeral session wrote into the profile dir"
    );
}

#[test]
fn ephemeral_session_does_not_load_the_human_jar() {
    let port = server();
    let state = tmp("humanjar");
    let mut jar = manuk_net::cookies::CookieJar::new();
    let url = url::Url::parse(&format!("http://127.0.0.1:{port}/")).unwrap();
    jar.store(&url, &format!("gui={SECRET}; Path=/; Max-Age=3600"));
    jar.save_to(&state.join("cookies.json")).unwrap();
    // Positive control: a profile session on the same dir does load it (the seed is readable).
    let mut p = Client::start_with_state(&state, &["--profile", state.to_str().unwrap()]);
    assert!(
        p.seen(port).contains(SECRET),
        "control: seeded jar is loadable"
    );
    drop(p);
    let mut a = Client::start_with_state(&state, &[]);
    assert!(
        a.seen(port).contains("cookie-seen:[]"),
        "ephemeral session must not inherit the human's persisted cookies"
    );
}

impl Client {
    fn start_with_state(state: &std::path::Path, args: &[&str]) -> Client {
        let mut child = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"))
            .env("MANUK_STATE", state)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let tx = child.stdin.take().unwrap();
        let rx = BufReader::new(child.stdout.take().unwrap());
        let mut c = Client {
            child,
            tx,
            rx,
            id: 0,
        };
        c.rpc("initialize", json!({"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "t", "version": "0"}})).unwrap();
        c.send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        c
    }
    fn try_start_env(state: &std::path::Path, port: u16) -> Client {
        let mut child = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"))
            .env("MANUK_STATE", state)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let tx = child.stdin.take().unwrap();
        let rx = BufReader::new(child.stdout.take().unwrap());
        let mut c = Client {
            child,
            tx,
            rx,
            id: 0,
        };
        c.rpc("initialize", json!({"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "t", "version": "0"}})).unwrap();
        c.send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        c.tool(
            "navigate",
            json!({"url": format!("http://127.0.0.1:{port}/login")}),
        );
        c
    }
}

#[test]
fn human_profile_cookie_is_invisible_until_a_per_origin_grant() {
    let port = server();
    let dir = tmp("human");
    let human = dir.join("cookies.json");
    let mut jar = manuk_net::cookies::CookieJar::new();
    let url = url::Url::parse(&format!("http://127.0.0.1:{port}/")).unwrap();
    jar.store(&url, &format!("gui={SECRET}; Path=/; Max-Age=3600"));
    jar.save_to(&human).unwrap();
    let h = human.to_str().unwrap();

    let mut plain = Client::start(&["--human-cookies", h]);
    assert!(
        plain.seen(port).contains("cookie-seen:[]"),
        "no grant -> invisible"
    );

    let origin = format!("http://127.0.0.1:{port}");
    let mut granted = Client::start(&["--human-cookies", h, "--grant-origin", &origin]);
    assert!(
        granted.seen(port).contains(SECRET),
        "granted origin -> visible"
    );

    let mut other = Client::start(&[
        "--human-cookies",
        h,
        "--grant-origin",
        "http://other.invalid",
    ]);
    assert!(
        other.seen(port).contains("cookie-seen:[]"),
        "grant is per-origin"
    );
}

#[test]
fn second_process_on_a_profile_is_refused() {
    let dir = tmp("profile");
    let d = dir.to_str().unwrap();
    let mut first = Client::start(&["--profile", d]);
    let second = Client::try_start(&["--profile", d]);
    assert!(
        second.is_none(),
        "second manuk-mcp on the same profile must refuse to start"
    );
    // The first still serves, and persists across a restart of the profile.
    let port = server();
    first.tool(
        "navigate",
        json!({"url": format!("http://127.0.0.1:{port}/login")}),
    );
    drop(first);
    let mut again = Client::start(&["--profile", d]);
    assert!(
        again.seen(port).contains(SECRET),
        "profile persisted the login"
    );
}

#[test]
fn storage_state_round_trips_between_sessions() {
    let port = server();
    let dir = tmp("state-json");
    let f = dir.join("state.json");
    let fs = f.to_str().unwrap();
    let mut a = Client::start(&["--allow-write", dir.to_str().unwrap()]);
    a.tool(
        "navigate",
        json!({"url": format!("http://127.0.0.1:{port}/login")}),
    );
    let msg = a.tool("storage_state", json!({"action": "export", "path": fs}));
    assert!(msg.contains("written"), "{msg}");
    let state: Value = serde_json::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
    assert_eq!(state["cookies"][0]["name"], "sid");
    assert_eq!(state["cookies"][0]["value"], SECRET);
    let mut c = Client::start(&["--storage-state", fs]);
    assert!(
        c.seen(port).contains(SECRET),
        "imported state restores the login"
    );
    let mut d = Client::start(&[]);
    assert!(d.seen(port).contains("cookie-seen:[]"));
}

/// SEC.DID.11: the storage-state path is agent-supplied. Under confinement an export outside the
/// write set fails loudly and writes nothing; `--no-confine` is the control that can write there.
#[test]
fn storage_state_export_outside_the_write_set_fails_loudly() {
    let dir = tmp("ss-deny");
    let f = dir.join("state.json");
    let mut a = Client::start(&[]);
    let msg = a.tool(
        "storage_state",
        json!({"action": "export", "path": f.to_str().unwrap()}),
    );
    assert!(!msg.contains("written"), "confined export reported success: {msg}");
    assert!(!f.exists(), "confined export wrote {}", f.display());
}

#[test]
fn storage_state_export_outside_the_write_set_succeeds_unconfined() {
    let dir = tmp("ss-control");
    let f = dir.join("state.json");
    let mut a = Client::start(&["--no-confine"]);
    let msg = a.tool(
        "storage_state",
        json!({"action": "export", "path": f.to_str().unwrap()}),
    );
    assert!(msg.contains("written"), "{msg}");
    assert!(f.exists());
}

/// SEC.DID.11: valid JSON that is not storage state is refused, both as a tool import and as the
/// `--storage-state` startup seed. The old code imported it as a silent "0 cookies" no-op.
#[test]
fn storage_state_import_of_a_non_storage_state_file_fails_loudly() {
    let dir = tmp("ss-notstate");
    let f = dir.join("notes.json");
    std::fs::write(&f, r#"{"hello": "world"}"#).unwrap();
    let fs = f.to_str().unwrap();
    let mut a = Client::start(&[]);
    let msg = a.tool("storage_state", json!({"action": "import", "path": fs}));
    assert!(!msg.contains("imported"), "non-state file imported: {msg}");
    assert!(msg.contains("not Playwright storage state"), "{msg}");
    let out = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"))
        .args(["--storage-state", fs])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!out.status.success(), "startup seed accepted a non-state file");
}

/// SEC.DID.11: an export whose path is inside a profile dir may not overwrite `.manuk-profile.lock`,
/// directly or through a symlink. An ordinary file in the same dir still exports (not over-broad).
#[test]
fn storage_state_export_cannot_overwrite_the_profile_lock() {
    let dir = tmp("ss-lock");
    let lock = dir.join(".manuk-profile.lock");
    let mut a = Client::start(&["--profile", dir.to_str().unwrap()]);
    let lock_now = std::fs::read(&lock).unwrap();
    let direct = a.tool(
        "storage_state",
        json!({"action": "export", "path": lock.to_str().unwrap()}),
    );
    assert!(direct.contains("profile lock"), "{direct}");
    let alias = dir.join("alias.json");
    std::os::unix::fs::symlink(&lock, &alias).unwrap();
    let via_link = a.tool(
        "storage_state",
        json!({"action": "export", "path": alias.to_str().unwrap()}),
    );
    assert!(via_link.contains("profile lock"), "{via_link}");
    assert_eq!(std::fs::read(&lock).unwrap(), lock_now, "lock rewritten");
    let ok = dir.join("state.json");
    let fine = a.tool(
        "storage_state",
        json!({"action": "export", "path": ok.to_str().unwrap()}),
    );
    assert!(fine.contains("written"), "{fine}");
}

/// SEC.DID.11.a: an export may not overwrite an existing non-state file in the profile
/// (`cookies.json`) nor a hard link to the profile lock, which the name guard cannot see.
#[test]
fn storage_state_export_cannot_overwrite_profile_files_or_a_lock_hard_link() {
    let dir = tmp("ss-clobber");
    let mut a = Client::start(&["--profile", dir.to_str().unwrap()]);
    let lock = dir.join(".manuk-profile.lock");
    let cookies = dir.join("cookies.json");
    std::fs::write(&cookies, r#"[{"human":"cookie jar"}]"#).unwrap();
    let hard = dir.join("hard.json");
    std::fs::hard_link(&lock, &hard).unwrap();
    for target in [&cookies, &hard] {
        let msg = a.tool(
            "storage_state",
            json!({"action": "export", "path": target.to_str().unwrap()}),
        );
        assert!(!msg.contains("written"), "overwrote {}: {msg}", target.display());
    }
    assert_eq!(
        std::fs::read_to_string(&cookies).unwrap(),
        r#"[{"human":"cookie jar"}]"#
    );
    assert_eq!(std::fs::metadata(&lock).unwrap().len(), 0, "lock rewritten");
    // Re-exporting over a previous export is still allowed.
    let ok = dir.join("state.json");
    for _ in 0..2 {
        let m = a.tool(
            "storage_state",
            json!({"action": "export", "path": ok.to_str().unwrap()}),
        );
        assert!(m.contains("written"), "{m}");
    }
}
