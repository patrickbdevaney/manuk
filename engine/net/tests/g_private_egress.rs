//! **G_PRIVATE_EGRESS (W3.10) — a public page cannot make the browser dial loopback/LAN.**
//!
//! Class of CVE-2025-49596. Real sockets: a loopback server counts the connections it receives.
//! With a public top-level document, a page-initiated PUT to `127.0.0.1:<port>`, a GET to a
//! hostname that resolves to loopback (`localhost`, the rebinding shape) and a redirect-free
//! subresource fetch are all refused with the server hit count still 0. The control arms prove the
//! gate can go red: `--allow-private-network` and a local top document both let the request land.
//! Also the CORS preflight: a non-simple cross-origin request is allowed only by a granting OPTIONS.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// A tiny HTTP server; `opts_headers` is appended to the answer to OPTIONS. Returns (port, hits, options_hits).
fn server(opts_headers: &'static str) -> (u16, Arc<AtomicUsize>, Arc<AtomicUsize>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let hits = Arc::new(AtomicUsize::new(0));
    let opts = Arc::new(AtomicUsize::new(0));
    let (h, o) = (hits.clone(), opts.clone());
    std::thread::spawn(move || {
        for s in l.incoming() {
            let mut s = match s {
                Ok(s) => s,
                Err(_) => break,
            };
            h.fetch_add(1, Ordering::SeqCst);
            let mut buf = [0u8; 4096];
            let n = s.read(&mut buf).unwrap_or(0);
            let is_opts = buf[..n].starts_with(b"OPTIONS");
            if is_opts {
                o.fetch_add(1, Ordering::SeqCst);
            }
            let extra = if is_opts { opts_headers } else { "" };
            let _ = write!(
                s,
                "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n{extra}\r\nok"
            );
        }
    });
    (port, hits, opts)
}

static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn hdrs(p: &[(&str, &str)]) -> Vec<(String, String)> {
    p.iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[test]
fn public_page_cannot_reach_private_network() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let rt = manuk_net::runtime();
    let (port, hits, _) = server("");
    let lit = format!("http://127.0.0.1:{port}/");
    let named = format!("http://localhost:{port}/");
    let page = "http://public.example/";

    manuk_net::set_top_level_document(page);
    assert!(manuk_net::egress::guard_active());

    // 1. page-initiated PUT to a loopback literal
    let r = rt.block_on(manuk_net::request_from(
        "PUT",
        &lit,
        &[],
        manuk_net::Bytes::from("x"),
        Some(page),
    ));
    assert!(r.is_err(), "PUT to loopback literal must be refused");
    // 2. a name that resolves to loopback (rebinding shape), page-initiated and plain subresource
    let r = rt.block_on(manuk_net::request_from(
        "GET",
        &named,
        &[],
        manuk_net::Bytes::new(),
        Some(page),
    ));
    assert!(
        r.is_err(),
        "name resolving to 127.0.0.1 must be refused: {r:?}"
    );
    let r = rt.block_on(manuk_net::fetch_from(&named, Some(page)));
    assert!(
        r.is_err(),
        "subresource GET to a rebound name must be refused"
    );
    assert_eq!(
        hits.load(Ordering::SeqCst),
        0,
        "the loopback server must never be dialled"
    );

    // Control: the embedder's own (initiator-less) API call may reach loopback.
    let r = rt
        .block_on(manuk_net::request(
            "GET",
            &lit,
            &[],
            manuk_net::Bytes::new(),
        ))
        .unwrap();
    assert_eq!(r.status, 200);
    assert_eq!(hits.load(Ordering::SeqCst), 1);

    // Control: opt-out flag lets the page request land (the gate can go red).
    manuk_net::egress::set_allow_private_network(true);
    let r = rt.block_on(manuk_net::request_from(
        "PUT",
        &lit,
        &[],
        manuk_net::Bytes::from("x"),
        Some(page),
    ));
    assert!(
        r.is_ok(),
        "--allow-private-network must let it through: {r:?}"
    );
    assert_eq!(hits.load(Ordering::SeqCst), 2);
    manuk_net::egress::set_allow_private_network(false);

    // Control: a page the user opened on localhost may talk to localhost.
    manuk_net::set_top_level_document("http://localhost:3000/");
    assert!(!manuk_net::egress::guard_active());
    let r = rt.block_on(manuk_net::request_from(
        "GET",
        &named,
        &[],
        manuk_net::Bytes::new(),
        Some("http://localhost:3000/"),
    ));
    assert!(r.is_ok(), "{r:?}");
    assert_eq!(hits.load(Ordering::SeqCst), 3);
}

#[test]
fn cors_preflight_gates_non_simple_requests() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let rt = manuk_net::runtime();
    // Local top document so the (loopback) fixture is reachable; this test is about CORS.
    manuk_net::set_top_level_document("http://localhost:3000/");
    let origin = "http://app.example";

    // Simple request: no preflight, nothing sent.
    let (p0, h0, _) = server("");
    let url = format!("http://127.0.0.1:{p0}/");
    assert!(rt
        .block_on(manuk_net::cors_preflight(origin, &url, "GET", &[]))
        .unwrap());
    assert_eq!(h0.load(Ordering::SeqCst), 0);

    // Non-simple (PUT + custom header), server grants nothing -> refused, only OPTIONS seen.
    let (p1, h1, o1) = server("");
    let url = format!("http://127.0.0.1:{p1}/");
    let req = hdrs(&[("x-token", "1")]);
    assert!(!rt
        .block_on(manuk_net::cors_preflight(origin, &url, "PUT", &req))
        .unwrap());
    assert_eq!(
        (h1.load(Ordering::SeqCst), o1.load(Ordering::SeqCst)),
        (1, 1)
    );

    // Granting preflight -> allowed.
    let (p2, _, _) = server("Access-Control-Allow-Origin: http://app.example\r\nAccess-Control-Allow-Methods: PUT\r\nAccess-Control-Allow-Headers: x-token\r\n");
    let url = format!("http://127.0.0.1:{p2}/");
    assert!(rt
        .block_on(manuk_net::cors_preflight(origin, &url, "PUT", &req))
        .unwrap());

    // Granting origin+method but not the header -> refused.
    let (p3, _, _) = server(
        "Access-Control-Allow-Origin: http://app.example\r\nAccess-Control-Allow-Methods: PUT\r\n",
    );
    let url = format!("http://127.0.0.1:{p3}/");
    assert!(!rt
        .block_on(manuk_net::cors_preflight(origin, &url, "PUT", &req))
        .unwrap());
}
