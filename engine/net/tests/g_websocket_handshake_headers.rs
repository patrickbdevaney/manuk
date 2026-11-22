//! G_WEBSOCKET_HANDSHAKE_HEADERS — the page's WebSocket handshake carries the headers a browser
//! sends, and a server that gates on `Origin` sees it.
//!
//! **Why this gate exists.** The Wolfram|Alpha gateway (`gateway.wolframalpha.com`) answers a
//! WebSocket upgrade that carries no `Origin`, `User-Agent` or `Accept-Language` with `403`, so the
//! page showed its "doesn't understand your query" error state. The fix (`connect_from`) sends the
//! opening page's origin plus the honest UA and Accept-Language. Until now only an end-to-end agent
//! run guarded that, which is slow and cannot say WHICH header is missing. This test pins each
//! header on the wire, against a real `tungstenite` accept-side server that reads the request as a
//! server would, and mimics the gateway's `Origin` check.
//!
//! What it does not claim: it does not exercise the page-facing shell wiring
//! (`gui.rs::pump_websockets`); that is covered by `g_websocket_live`.

use std::sync::mpsc;

use manuk_net::websocket::WebSocketConn;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};

/// What the server saw in one handshake request, plus whether it let the upgrade through.
#[derive(Debug)]
struct Seen {
    origin: Option<String>,
    user_agent: Option<String>,
    accept_language: Option<String>,
    accepted: bool,
}

/// A loopback WebSocket server that records each handshake and rejects a request with no `Origin`
/// with `403`, the way the gateway behaves. Returns its address and the channel of observations.
fn start_gateway(rt: &tokio::runtime::Runtime) -> (String, mpsc::Receiver<Seen>) {
    let (tx, rx) = mpsc::channel();
    let addr = rt.block_on(async {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        tokio::spawn(async move {
            while let Ok((sock, _)) = listener.accept().await {
                let tx = tx.clone();
                tokio::spawn(async move {
                    let mut seen = Seen {
                        origin: None,
                        user_agent: None,
                        accept_language: None,
                        accepted: false,
                    };
                    let cb = |req: &Request, resp: Response| -> Result<Response, ErrorResponse> {
                        let h = |name: &str| {
                            req.headers()
                                .get(name)
                                .and_then(|v| v.to_str().ok())
                                .map(str::to_string)
                        };
                        seen.origin = h("origin");
                        seen.user_agent = h("user-agent");
                        seen.accept_language = h("accept-language");
                        if seen.origin.is_none() {
                            return Err(http::Response::builder()
                                .status(403)
                                .body(Some("Forbidden".to_string()))
                                .unwrap());
                        }
                        Ok(resp)
                    };
                    let ok = tokio_tungstenite::accept_hdr_async(sock, cb).await.is_ok();
                    seen.accepted = ok;
                    let _ = tx.send(seen);
                });
            }
        });
        addr
    });
    (addr, rx)
}

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap()
}

/// **With the page's origin, the gateway admits the handshake and every browser header is on the
/// wire.** RED if the `Origin` line is dropped from `connect_from` (the 403 above), or if the
/// User-Agent or Accept-Language are not sent.
#[test]
fn a_page_handshake_carries_origin_user_agent_and_accept_language() {
    let rt = rt();
    let (addr, rx) = start_gateway(&rt);
    let page = "https://www.wolframalpha.com";

    let conn = rt.block_on(WebSocketConn::connect_from(
        &format!("ws://{addr}/gateway"),
        &[],
        Some(page),
    ));
    assert!(conn.is_ok(), "the gateway admits a handshake that carries the page origin: {:?}", conn.err());

    let seen = rx.recv().unwrap();
    assert!(seen.accepted, "server accepted the upgrade");
    assert_eq!(
        seen.origin.as_deref(),
        Some(page),
        "the page's Origin is sent on the handshake (the gateway 403s without it)"
    );
    assert_eq!(
        seen.user_agent.as_deref(),
        Some(manuk_net::user_agent()),
        "the handshake's User-Agent is the one the HTTP path sends, not tungstenite's default"
    );
    assert_eq!(
        seen.accept_language.as_deref(),
        Some("en-US,en;q=0.9"),
        "Accept-Language is sent on the handshake"
    );
}

/// **Without an origin, the handshake carries no `Origin` and the gateway refuses it with 403.**
/// This is the pre-fix behaviour, pinned so a regression in `connect` is visible as the same error.
#[test]
fn a_handshake_with_no_origin_is_refused_with_403() {
    let rt = rt();
    let (addr, rx) = start_gateway(&rt);

    let err = rt
        .block_on(WebSocketConn::connect(&format!("ws://{addr}/gateway"), &[]))
        .err()
        .expect("the gateway refuses a handshake with no Origin");
    let seen = rx.recv().unwrap();
    assert_eq!(seen.origin, None, "no Origin header is sent by connect()");
    assert!(!seen.accepted, "server did not accept the upgrade");
    assert!(
        format!("{err:#}").contains("403"),
        "the failure names the 403 cause, not a generic handshake error: {err:#}"
    );
}

/// **An opaque or empty origin is not sent as the literal string `null` or `""`.** A browser omits
/// `Origin` for an opaque origin, and sending `Origin: null` would be a false identity claim; the
/// filter in `connect_from` must hold.
#[test]
fn an_opaque_or_empty_origin_is_not_sent() {
    let rt = rt();
    for origin in [Some("null"), Some("")] {
        let (addr, rx) = start_gateway(&rt);
        let _ = rt.block_on(WebSocketConn::connect_from(
            &format!("ws://{addr}/gateway"),
            &[],
            origin,
        ));
        let seen = rx.recv().unwrap();
        assert_eq!(
            seen.origin, None,
            "origin {origin:?} must not be sent on the handshake"
        );
    }
}
