//! **G_DOWNLOAD_GESTURE (SEC.DID.3) — a page cannot write a file to disk without a user gesture.**
//!
//! A server answers with `Content-Disposition: attachment`. With the gesture requirement on (as the
//! GUI shell sets it): no gesture -> error and nothing in the download dir (A); after a gesture -> the
//! file is written (B, vacuity guard); requirement off (agent/headless path) -> written (C).
//!
//! Mutation (chrome_ver=145.0.7632.116): deleting the `gesture_permits()` check in
//! `fetch_document_or_download` turns claim A RED.

use std::io::{Read, Write};
use std::net::TcpListener;

fn server() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { break };
            let mut buf = [0u8; 4096];
            let _ = s.read(&mut buf);
            let b = "PAYLOAD";
            let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Disposition: attachment; filename=\"f.txt\"\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{b}", b.len()).as_bytes());
        }
    });
    port
}

fn files(d: &std::path::Path) -> usize {
    std::fs::read_dir(d).map(|r| r.count()).unwrap_or(0)
}

#[test]
fn download_needs_a_user_gesture() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let port = server();
    let dir = std::env::temp_dir().join(format!("manuk-dlg-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);

    manuk_net::downloads::require_user_gesture(true);
    let url = |n: u32| format!("http://127.0.0.1:{port}/a{n}");

    // A: no gesture -> refused, nothing written.
    let r = rt.block_on(manuk_net::fetch_document_or_download(&url(1), &dir));
    assert!(r.is_err(), "download without a gesture must be refused");
    assert_eq!(files(&dir), 0, "no file may be written without a gesture");

    // B: gesture -> written.
    manuk_net::downloads::note_user_gesture();
    let r = rt.block_on(manuk_net::fetch_document_or_download(&url(2), &dir));
    assert!(matches!(r, Ok(manuk_net::DocOrDownload::Download { .. })));
    assert_eq!(std::fs::read(dir.join("f.txt")).unwrap(), b"PAYLOAD");

    // C: requirement off (agent path) and no fresh gesture needed -> written (deduped name).
    manuk_net::downloads::require_user_gesture(false);
    let r = rt.block_on(manuk_net::fetch_document_or_download(&url(3), &dir));
    assert!(matches!(r, Ok(manuk_net::DocOrDownload::Download { .. })));
    assert_eq!(files(&dir), 2);
    let _ = std::fs::remove_dir_all(&dir);
}
