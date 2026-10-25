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

impl Client {
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
    let mut a = Client::start(&[]);
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
