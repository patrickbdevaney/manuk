//! **G_PAGE_FETCH_PREFLIGHT (W3.10.a) — the headless/MCP page-fetch executor enforces the egress guard
//! and CORS preflight, not only the GUI pump.**
//!
//! Real sockets, `Page::load_async` (the path `manuk-mcp` and every headless consumer use). (1) A page
//! at a public origin that `fetch`es `PUT http://127.0.0.1:<port>` never reaches the server (hit 0).
//! (2) A page at a *cross* origin issuing a non-simple PUT is never sent unless OPTIONS allows it; the
//! granting server sees OPTIONS then PUT, the refusing one sees OPTIONS only.

#![cfg(feature = "spidermonkey")]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use manuk_text::FontContext;

/// (port, total hits, non-OPTIONS hits)
fn server(allow: bool) -> (u16, Arc<AtomicUsize>, Arc<AtomicUsize>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let (hits, real) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
    let (h, r) = (hits.clone(), real.clone());
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { break };
            h.fetch_add(1, Ordering::SeqCst);
            let mut buf = [0u8; 4096];
            let n = s.read(&mut buf).unwrap_or(0);
            let is_opts = buf[..n].starts_with(b"OPTIONS");
            if !is_opts {
                r.fetch_add(1, Ordering::SeqCst);
            }
            let extra = if allow {
                "Access-Control-Allow-Origin: http://app.example\r\nAccess-Control-Allow-Methods: PUT\r\nAccess-Control-Allow-Headers: x-token\r\n"
            } else {
                ""
            };
            let _ = write!(
                s,
                "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n{extra}\r\nok"
            );
        }
    });
    (port, hits, real)
}

fn html(target: &str) -> String {
    format!(
        "<!doctype html><body><script>fetch('{target}', {{method:'PUT', headers:{{'x-token':'1'}}, body:'x'}}).catch(function(){{}});</script></body>"
    )
}

#[test]
fn headless_executor_applies_egress_guard_and_preflight() {
    let rt = manuk_net::runtime();
    let fonts = FontContext::new();

    // 1. public page -> loopback: refused by the egress guard.
    let (p, hits, _) = server(true);
    manuk_net::set_top_level_document("http://public.example/");
    rt.block_on(async {
        let mut pg = manuk_page::Page::load_async(
            &html(&format!("http://127.0.0.1:{p}/")),
            "http://public.example/",
            &fonts,
            800.0,
        )
        .await;
        pg.finish_loading(&fonts, 800.0).await;
    });
    assert_eq!(
        hits.load(Ordering::SeqCst),
        0,
        "public page reached loopback"
    );

    // Control: opt-out lets it land (the gate can go red).
    manuk_net::egress::set_allow_private_network(true);
    rt.block_on(async {
        let mut pg = manuk_page::Page::load_async(
            &html(&format!("http://127.0.0.1:{p}/")),
            "http://app.example/",
            &fonts,
            800.0,
        )
        .await;
        pg.finish_loading(&fonts, 800.0).await;
    });
    assert!(hits.load(Ordering::SeqCst) >= 1);
    assert_eq!(
        hits.load(Ordering::SeqCst),
        2,
        "OPTIONS + PUT when the server grants"
    );
    manuk_net::egress::set_allow_private_network(false);

    // 2. preflight: refusing server sees OPTIONS only, the real PUT is never sent.
    manuk_net::set_top_level_document("http://localhost:3000/");
    let (p2, hits2, real2) = server(false);
    rt.block_on(async {
        let mut pg = manuk_page::Page::load_async(
            &html(&format!("http://127.0.0.1:{p2}/")),
            "http://app.example/",
            &fonts,
            800.0,
        )
        .await;
        pg.finish_loading(&fonts, 800.0).await;
    });
    assert_eq!(
        (hits2.load(Ordering::SeqCst), real2.load(Ordering::SeqCst)),
        (1, 0)
    );
}
