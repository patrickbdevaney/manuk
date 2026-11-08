//! CB.TMP.a: a zygote session must end its process when the client hangs up. The forked child runs its
//! session on a thread that was never the leader, and SpiderMonkey's helper threads it restarts never
//! exit, so a session that just returns leaves a defunct `manuk-main` leader behind (96 on the bench box).
//! Gate: after the client closes, the session's pid is gone from /proc within 15 s. RED without the
//! `process::exit` in mcp/src/main.rs (the pid stays, state Z).

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::json;

/// (pid, state) of every process whose parent is `ppid`.
fn children_of(ppid: u32) -> Vec<(u32, char)> {
    let mut out = vec![];
    for e in std::fs::read_dir("/proc").unwrap().flatten() {
        let Ok(pid) = e.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
            continue;
        };
        // The comm is in parentheses and may hold spaces: fields resume after the last ')'.
        let rest = &stat[stat.rfind(')').unwrap() + 2..];
        let mut f = rest.split_whitespace();
        let state = f.next().unwrap().chars().next().unwrap();
        let parent: u32 = f.next().unwrap().parse().unwrap();
        if parent == ppid {
            out.push((pid, state));
        }
    }
    out
}

#[test]
fn a_session_process_exits_when_its_client_hangs_up() {
    let dir = std::env::temp_dir().join(format!("manuk-zexit-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let sock = dir.join("z.sock");
    let page = dir.join("p.html");
    std::fs::write(&page, "<html><body><p>exit probe</p></body></html>").unwrap();
    let url = format!("file://{}", page.display());
    let mut z = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"))
        .args(["--no-confine", "--zygote-after-load"])
        .arg(&sock)
        .arg(&url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn zygote");
    for _ in 0..600 {
        if sock.exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(sock.exists(), "zygote never became ready");
    let session = {
        let mut tx = UnixStream::connect(&sock).expect("connect zygote");
        let mut rx = BufReader::new(tx.try_clone().unwrap());
        let req = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "t", "version": "0"}}});
        writeln!(tx, "{req}").unwrap();
        let mut line = String::new();
        rx.read_line(&mut line).unwrap();
        assert!(line.contains("\"tools\""), "bad initialize reply: {line}");
        // Find the forked child while it is alive, then hang up (both halves of the socket).
        let kids = children_of(z.id());
        assert_eq!(kids.len(), 1, "expected one session child, got {kids:?}");
        kids[0].0
    };
    let start = Instant::now();
    loop {
        // A defunct leader (state Z) still has a /proc entry, so the entry itself is the test.
        if !std::path::Path::new(&format!("/proc/{session}")).exists() {
            break;
        }
        assert!(
            start.elapsed() < Duration::from_secs(15),
            "session pid {session} still running 15 s after the client hung up"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = z.kill();
    let _ = z.wait();
}
