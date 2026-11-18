//! **G_WIRE_BYTES (W4.E3.b.a) — `lean::WIRE_BYTES` counts only bytes that crossed the wire.**
//!
//! Before this node a `304 Not Modified` was counted as the stored body's length (the stored copy is
//! served, but no body was read), and the top-level document was not counted at all. Claims, each a
//! delta on the process-wide counter around one request:
//!   A. a cold GET of a 1000-byte body counts exactly 1000;
//!   B. a revalidation of that now-stale entry (`no-cache` + `ETag`, server answers `304`) counts 0, and
//!      the caller still gets the stored 1000-byte body (the cache is still honoured);
//!   C. a cold top-level document (`fetch_document_or_download`) counts its 500-byte body;
//!   D. a second document fetch inside its freshness lifetime is a cache hit and counts 0.
//!
//! Mutations (chrome_ver=145.0.7632.116): re-adding `WIRE_BYTES += r.body.len()` on the success path of
//! `fetch_from` turns claim B RED (1000 instead of 0); deleting the `WIRE_BYTES` line in
//! `fetch_document_or_download` turns claim C RED (0 instead of 500).
//!
//! One `#[test]` on purpose: the counter is process-global, and this file's claims must not interleave.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

const SUB_BODY: &[u8] = &[b's'; 1000];
const DOC_BODY: &[u8] = &[b'd'; 500];

/// A server that answers `/sub` with 1000 bytes (`no-cache`, `ETag: "v1"`) and `304` to a matching
/// `If-None-Match`, and `/doc` with 500 bytes of HTML that is fresh for an hour. Returns the port and a
/// counter of `304`s it sent.
fn server() -> (u16, Arc<AtomicUsize>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let not_modified = Arc::new(AtomicUsize::new(0));
    let nm = not_modified.clone();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { break };
            let mut buf = [0u8; 4096];
            let n = s.read(&mut buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]).to_ascii_lowercase();
            let first = req.lines().next().unwrap_or("").to_string();
            let out: Vec<u8> = if first.contains("/doc") {
                let mut v = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nCache-Control: max-age=3600\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    DOC_BODY.len()
                )
                .into_bytes();
                v.extend_from_slice(DOC_BODY);
                v
            } else if req.contains("if-none-match: \"v1\"") {
                nm.fetch_add(1, Ordering::SeqCst);
                b"HTTP/1.1 304 Not Modified\r\nETag: \"v1\"\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n".to_vec()
            } else {
                let mut v = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nETag: \"v1\"\r\nCache-Control: no-cache\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    SUB_BODY.len()
                )
                .into_bytes();
                v.extend_from_slice(SUB_BODY);
                v
            };
            let _ = s.write_all(&out);
        }
    });
    (port, not_modified)
}

#[test]
fn wire_bytes_count_only_what_crossed_the_wire() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (port, not_modified) = server();
    let wire = || manuk_net::lean::WIRE_BYTES.load(Ordering::Relaxed);
    let sub = format!("http://127.0.0.1:{port}/sub");
    let doc = format!("http://127.0.0.1:{port}/doc");
    let dir = std::env::temp_dir().join(format!("manuk-wire-bytes-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    // A: a cold subresource counts its body.
    let before = wire();
    let a = rt.block_on(manuk_net::fetch(&sub)).expect("cold GET");
    assert_eq!(a.body.len(), SUB_BODY.len(), "cold body length");
    assert_eq!(
        wire() - before,
        SUB_BODY.len() as u64,
        "A: cold GET must count its body"
    );

    // B: the stale entry revalidates to a 304; the stored body is served, and no bytes are counted.
    let before = wire();
    let b = rt
        .block_on(manuk_net::fetch(&sub))
        .expect("revalidating GET");
    assert_eq!(
        not_modified.load(Ordering::SeqCst),
        1,
        "B: the second GET must have been a conditional request answered 304"
    );
    assert_eq!(
        b.body.as_ref(),
        SUB_BODY,
        "B: the stored body is still served"
    );
    assert_eq!(
        wire() - before,
        0,
        "B: a 304 crossed no body, so it must count 0 (the stored copy is not traffic)"
    );

    // C: a cold top-level document counts its body.
    let before = wire();
    let c = rt
        .block_on(manuk_net::fetch_document_or_download(&doc, &dir))
        .expect("cold document");
    match c {
        manuk_net::DocOrDownload::Document(r) => assert_eq!(r.body.len(), DOC_BODY.len()),
        _ => panic!("C: a text/html 200 is a document, not a download"),
    }
    assert_eq!(
        wire() - before,
        DOC_BODY.len() as u64,
        "C: the top-level document must count its body (it was never counted)"
    );

    // D: a fresh document inside its lifetime is served from the cache and counts 0.
    let before = wire();
    let d = rt
        .block_on(manuk_net::fetch_document_or_download(&doc, &dir))
        .expect("cached document");
    match d {
        manuk_net::DocOrDownload::Document(r) => assert_eq!(r.body.as_ref(), DOC_BODY),
        _ => panic!("D: a cached document is a document"),
    }
    assert_eq!(wire() - before, 0, "D: a cache hit crosses no wire");

    let _ = std::fs::remove_dir_all(&dir);
}
