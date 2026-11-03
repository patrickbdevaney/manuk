//! **G_TLS_HANDSHAKE_RETRY (R0.1.a.a.a.a) — a GET survives a server that drops TLS handshakes.**
//!
//! ebanking.bdm-sa.com resets most handshakes from every client (openssl, curl, manuk) in bursts;
//! Chrome retries, so it loads. Real sockets: a loopback listener closes every connection at once
//! (a handshake EOF). A GET must redial (up to `1 + CONNECT_RETRIES`, bounded by the document deadline) before giving up; a POST must
//! dial exactly once (not safe to replay). Mutant: `CONNECT_RETRIES = 0` turns the GET arm red
//! (1 hit, not >=5). chrome_ver=145.0.7632.116

use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

fn dropper() -> (u16, Arc<AtomicUsize>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let hits = Arc::new(AtomicUsize::new(0));
    let h = hits.clone();
    std::thread::spawn(move || {
        for s in l.incoming() {
            if s.is_err() {
                break;
            }
            h.fetch_add(1, Ordering::SeqCst);
        }
    });
    (port, hits)
}

#[tokio::test]
async fn get_retries_dropped_handshakes_post_does_not() {
    manuk_net::egress::set_allow_private_network(true);
    let (port, hits) = dropper();
    let r = manuk_net::fetch(&format!("https://127.0.0.1:{port}/")).await;
    assert!(r.is_err());
    let n = hits.load(Ordering::SeqCst);
    // The document deadline (~8s) may cut the last backoffs short: 7 dials measured, 9 at most.
    assert!(
        (5..=9).contains(&n),
        "GET must retry dropped handshakes, dialled {n}x"
    );

    let (port, hits) = dropper();
    let r = manuk_net::post_document(
        &format!("https://127.0.0.1:{port}/"),
        "text/plain",
        bytes::Bytes::from_static(b"x"),
        None,
    )
    .await;
    assert!(r.is_err());
    assert_eq!(hits.load(Ordering::SeqCst), 1, "POST is never replayed");
}
