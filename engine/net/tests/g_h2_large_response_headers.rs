//! **G_H2_LARGE_RESPONSE_HEADERS — an origin whose response header block is bigger than 16 KiB must
//! still load.**
//!
//! `h2`'s default `SETTINGS_MAX_HEADER_LIST_SIZE` is **16 KiB**, and a response that exceeds it is not
//! truncated, downgraded or retried: the client sends `RST_STREAM(PROTOCOL_ERROR)` and the request
//! fails outright. The page does not load slowly. It does not load.
//!
//! **The whole cost of this was hidden behind a word.** `playhop.com` — a HEAD site of
//! `corpus-v2.tsv` — booked `unreachable` in the tick-657 certificate sweep, and the instrument said
//! out loud that `unreachable` means *"a corpus or network problem, not a rendering one."* `curl`
//! fetched the same URL in 2.5s and 978 KB. The trace named the organ:
//!
//! ```text
//!   connected to [2a02:6b8::549]:443 · TLS ok · h2 ok · request sent
//!   h2::proto::streams::recv: stream error REQUEST_HEADER_FIELDS_TOO_LARGE
//!                             -- recv_headers: frame is over size; stream=StreamId(1)
//!   send_reset(reason=PROTOCOL_ERROR, initiator=Library)
//! ```
//!
//! `initiator=Library` is the confession: **we** reset the stream. Every origin that answers with a
//! long `Set-Cookie` ladder, a fat CSP, a `Link:` preload list — the ordinary output of any
//! session-heavy portal — was unreachable to this browser and read as a dead host.
//!
//! **What this gate proves, stated exactly.** It runs a real HTTP/2 exchange over loopback: a server
//! that answers with a ~40 KiB header block, and a client built from
//! [`manuk_net::HTTP2_MAX_HEADER_LIST_SIZE`] — *the same constant the process client is built with*.
//! So it proves the VALUE is sufficient, through a genuine h2 handshake and a genuine oversize
//! response, and it is RED-proven against `h2`'s own default. What it does not prove, and does not
//! claim, is the one line that passes the constant to the process client; that line is visible in the
//! diff beside this file. (A cleartext loopback socket cannot exercise the process client itself —
//! without ALPN it negotiates HTTP/1.1, which has different limits and is not the defect.)

use http_body_util::{BodyExt, Empty, Full};
use hyper::body::Bytes;
use hyper_util::rt::{TokioExecutor, TokioIo};

/// Comfortably over h2's 16 KiB default and comfortably under Chrome's 256 KiB — so a run that
/// passes cannot be passing because the server's headers happened to be small, and a run that fails
/// cannot be failing because we asked for something no browser accepts either.
const HEADER_BYTES: usize = 40 * 1024;
const MARKER: &str = "the body behind forty kilobytes of headers";

/// Answer every request with `HEADER_BYTES` worth of `Set-Cookie`-shaped headers, then the body.
async fn serve(
    _req: hyper::Request<hyper::body::Incoming>,
) -> Result<hyper::Response<Full<Bytes>>, std::convert::Infallible> {
    let mut resp = hyper::Response::builder().status(200);
    // ~1 KiB per header, so the block crosses 16 KiB long before it reaches 40.
    let value = "x".repeat(1000);
    let n = HEADER_BYTES / 1024;
    for i in 0..n {
        resp = resp.header(format!("x-manuk-pad-{i}"), value.clone());
    }
    Ok(resp.body(Full::new(Bytes::from(MARKER))).unwrap())
}

/// One h2 request through a client built with `limit`. `Ok(body)` or `Err(the transport error)`.
fn fetch_over_h2(limit: u32) -> Result<String, String> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async move {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            // One connection is all the gate needs; serving in a loop would outlive the runtime.
            let Ok((stream, _)) = listener.accept().await else {
                return;
            };
            let _ = hyper::server::conn::http2::Builder::new(TokioExecutor::new())
                // The SERVER must be willing to SEND what it is about to send; its own outgoing
                // limit is not the thing under test.
                .max_header_list_size(u32::MAX)
                .serve_connection(TokioIo::new(stream), hyper::service::service_fn(serve))
                .await;
        });

        // `http2_only` gives us h2 with prior knowledge, so the exchange is real HTTP/2 without
        // needing TLS/ALPN on a loopback socket.
        let client: hyper_util::client::legacy::Client<_, Empty<Bytes>> =
            hyper_util::client::legacy::Client::builder(TokioExecutor::new())
                .http2_only(true)
                .http2_max_header_list_size(limit)
                .build_http();

        let uri: hyper::Uri = format!("http://{addr}/").parse().unwrap();
        let resp = client.get(uri).await.map_err(|e| format!("{e:?}"))?;
        let status = resp.status();
        let body = resp
            .into_body()
            .collect()
            .await
            .map_err(|e| format!("body: {e:?}"))?
            .to_bytes();
        if !status.is_success() {
            return Err(format!("status {status}"));
        }
        Ok(String::from_utf8_lossy(&body).to_string())
    })
}

#[test]
fn an_origin_with_a_forty_kilobyte_header_block_still_loads() {
    // ── THE CONTROL, and it runs FIRST so a green cannot mean "the fixture never made big headers".
    //    With h2's own 16 KiB default this exchange MUST fail — if it does not, the server did not
    //    send an oversize block and the claim below would be about the empty case.
    let with_default = fetch_over_h2(16 * 1024);
    println!("H2 HEADER PROBE: at h2's 16KiB default -> {with_default:?}");
    assert!(
        with_default.is_err(),
        "the gate's own fixture failed: a {HEADER_BYTES}-byte header block was accepted at the \
         16 KiB default, so this run never reproduced the defect and the assertion below would be \
         vacuous."
    );

    // ── THE CLAIM: at the value this browser announces, the same response arrives intact.
    let got = fetch_over_h2(manuk_net::HTTP2_MAX_HEADER_LIST_SIZE);
    println!(
        "H2 HEADER PROBE: at HTTP2_MAX_HEADER_LIST_SIZE={} -> {:?}",
        manuk_net::HTTP2_MAX_HEADER_LIST_SIZE,
        got.as_ref().map(|b| b.len())
    );
    assert_eq!(
        got.as_deref(),
        Ok(MARKER),
        "G_H2_LARGE_RESPONSE_HEADERS: a response with a {HEADER_BYTES}-byte header block did not \
         arrive. h2's default SETTINGS_MAX_HEADER_LIST_SIZE is 16 KiB and a response over it is \
         RST_STREAM(PROTOCOL_ERROR) — the page does not load at all, and it presents as a dead host \
         rather than as a browser defect. Announce Chrome's 256 KiB."
    );

    // ── AND THE VALUE ITSELF, because the number is the capability. A later edit that quietly
    //    lowered it toward the default would still pass the exchange above if the fixture shrank
    //    with it; this cannot.
    assert!(
        manuk_net::HTTP2_MAX_HEADER_LIST_SIZE >= 256 * 1024,
        "HTTP2_MAX_HEADER_LIST_SIZE is {} — below Chrome's announced 256 KiB. On capability \
         Chromium is the ceiling to MATCH, and this is the number the web is built against.",
        manuk_net::HTTP2_MAX_HEADER_LIST_SIZE
    );
}
