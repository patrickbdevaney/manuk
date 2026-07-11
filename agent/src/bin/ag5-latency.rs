//! AG5 — measure the in-process agent advantage vs. a CDP-over-socket baseline.
//!
//! The agent-native thesis is that driving the engine *in process* (a stable arena
//! `NodeId` and a typed `BrowserAction` resolved by a direct function call) avoids the
//! per-command cost that every socket protocol — CDP/WebDriver-BiDi over a WebSocket —
//! pays: serialize the command to JSON, cross a socket, deserialize on the far side,
//! serialize the (often large) result, cross back, deserialize again. That claim needs a
//! number. This binary produces one honestly:
//!
//!   * **In-process** — the command hands back the accessibility snapshot as a native Rust
//!     value (an owned clone; the consumer gets owned data, same as a real query). No
//!     serialization, no transport.
//!   * **CDP-over-socket** — the same snapshot round-trips a real localhost TCP connection
//!     with length-prefixed JSON framing: request encoded, sent, decoded; result encoded,
//!     sent back, decoded. This is exactly the overhead a `getAccessibilityTree`-style CDP
//!     command incurs that the in-process path does not.
//!
//! Both paths compute the *same* snapshot; the delta is purely transport + serialization,
//! i.e. the number the thesis is about. Run: `cargo run --release -p manuk-agent --bin
//! ag5-latency`.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Instant;

use serde::{Deserialize, Serialize};

/// One accessibility node, the unit a query returns. Fields mirror what an automation
/// client actually consumes: identity, semantics, accessible name, and hit-test geometry.
#[derive(Clone, Serialize, Deserialize)]
struct AxNode {
    id: u64,
    role: String,
    name: String,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    focusable: bool,
}

/// A realistic page snapshot: ~300 nodes is a content article's accessibility tree
/// (Wikipedia's rendered a11y tree is a few hundred exposed nodes).
fn build_snapshot(n: usize) -> Vec<AxNode> {
    let roles = ["link", "button", "heading", "paragraph", "textbox", "listitem", "image"];
    (0..n)
        .map(|i| AxNode {
            id: i as u64,
            role: roles[i % roles.len()].to_string(),
            name: format!("accessible label for element number {i} on the page"),
            x: (i % 40) as f32 * 24.0,
            y: (i / 40) as f32 * 18.0,
            w: 120.0,
            h: 16.0,
            focusable: i % 3 == 0,
        })
        .collect()
}

/// The CDP command envelope: a JSON request with a method + params, mirroring the shape
/// of a `{"id":N,"method":"Accessibility.getFullAXTree","params":{...}}` message.
#[derive(Serialize, Deserialize)]
struct Command {
    id: u64,
    method: String,
    params: serde_json::Value,
}

fn read_frame(s: &mut TcpStream) -> std::io::Result<Vec<u8>> {
    let mut len = [0u8; 4];
    s.read_exact(&mut len)?;
    let mut buf = vec![0u8; u32::from_le_bytes(len) as usize];
    s.read_exact(&mut buf)?;
    Ok(buf)
}

fn write_frame(s: &mut TcpStream, bytes: &[u8]) -> std::io::Result<()> {
    s.write_all(&(bytes.len() as u32).to_le_bytes())?;
    s.write_all(bytes)?;
    s.flush()
}

fn main() {
    const ITERS: usize = 3000;
    let snapshot = build_snapshot(300);
    let payload_bytes = serde_json::to_vec(&snapshot).unwrap().len();

    // ---- CDP-over-socket server: for each command, decode the request, serialize the
    // snapshot as the result, and frame it back. A real localhost TCP hop.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server_snapshot = snapshot.clone();
    let server = std::thread::spawn(move || {
        let (mut conn, _) = listener.accept().unwrap();
        conn.set_nodelay(true).unwrap(); // honest transport cost, not Nagle/delayed-ACK stalls
        loop {
            match read_frame(&mut conn) {
                Ok(req) => {
                    let _cmd: Command = serde_json::from_slice(&req).unwrap();
                    let result = serde_json::to_vec(&server_snapshot).unwrap();
                    if write_frame(&mut conn, &result).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let mut client = TcpStream::connect(addr).unwrap();
    client.set_nodelay(true).unwrap();

    // ---- Baseline: CDP-over-socket round trip, per command.
    let t = Instant::now();
    let mut sink = 0usize;
    for i in 0..ITERS {
        let cmd = Command {
            id: i as u64,
            method: "Accessibility.getFullAXTree".to_string(),
            params: serde_json::json!({ "depth": -1 }),
        };
        let req = serde_json::to_vec(&cmd).unwrap();
        write_frame(&mut client, &req).unwrap();
        let resp = read_frame(&mut client).unwrap();
        let tree: Vec<AxNode> = serde_json::from_slice(&resp).unwrap();
        sink = sink.wrapping_add(tree.len());
    }
    let cdp = t.elapsed();
    drop(client);
    let _ = server.join();

    // ---- In-process: the command hands the snapshot back as a native owned value. This is
    // what `AgentBrowser::a11y_tree()` / `observe()` do — a direct call, no transport.
    let source = snapshot.clone();
    let t = Instant::now();
    for _ in 0..ITERS {
        let tree: Vec<AxNode> = source.clone();
        sink = sink.wrapping_add(tree.len());
    }
    let inproc = t.elapsed();

    let cdp_us = cdp.as_secs_f64() * 1e6 / ITERS as f64;
    let inproc_us = inproc.as_secs_f64() * 1e6 / ITERS as f64;

    println!("AG5 — in-process vs CDP-over-socket, per automation command");
    println!("  snapshot:        300 a11y nodes ({payload_bytes} bytes JSON)");
    println!("  iterations:      {ITERS}");
    println!("  in-process:      {inproc_us:8.2} µs/command  (native value, no transport)");
    println!("  CDP-over-socket: {cdp_us:8.2} µs/command  (JSON encode + TCP round trip + decode)");
    println!("  overhead avoided:{:8.2} µs/command  ({:.1}× faster in-process)", cdp_us - inproc_us, cdp_us / inproc_us);
    println!("  (checksum {sink})");
}
