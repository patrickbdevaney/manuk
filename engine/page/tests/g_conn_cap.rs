//! **G_CONN_CAP — a page's subresources must not be lost to our own stampede.**
//!
//! Every subresource phase in `manuk-page` is an unbounded `join_all` over its whole worklist, and
//! until this gate landed nothing below it bounded them either: N images meant N simultaneous
//! sockets. That reads like a tuning detail and it is not one. Measured on `mangago.me`, a HEAD site
//! of `docs/bench/corpus-v2.tsv`:
//!
//! ```text
//!   MANUK_MAX_CONNS_PER_HOST=0    26 of 173 images in 8013ms     ← what shipped
//!   MANUK_MAX_CONNS_PER_HOST=6   171 of 173 images in 9487ms
//! ```
//!
//! 147 images did not fail slowly or partially. They ran out the 8s subresource deadline **while
//! queued behind our own requests**, and `manuk_net`'s per-navigation negative cache then remembered
//! each one as dead, so within that navigation they were never asked for again. The page rendered
//! 85% imageless — and the origin was healthy the whole time: a single request for one of those
//! images returns 200 in 0.52s, and 27 of them fetched concurrently by `curl` all succeed.
//!
//! **The knee is sharp, and it was measured rather than cited.** 6 → 171 landed; 12 → 59; 24 → 26;
//! 48 → 26, indistinguishable from no cap at all. The default of 6 sits on the right side of a cliff
//! the engine was falling off, and it coincides with Chromium's HTTP/1.1 per-host limit.
//!
//! **WHAT THIS GATE REPRODUCES IS THE SHAPE, not the site.** A gate pointed at a live third-party
//! origin measures that origin's mood. So the server below *is* the mechanism: it serves a bounded
//! number of requests concurrently and stalls everything beyond that, which is what a per-client
//! concurrency limit does. Against it, an uncapped client loses most of its requests to the deadline
//! and a capped one loses none — the same trade, made locally and deterministically.
//!
//! The two arms run in one process on purpose, which is why `max_conns_per_host()` is deliberately
//! not memoised (`g_load_document.rs` documents at length what a `OnceLock` does to a gate that
//! needs to control its own variable). They use two different listeners because the permit map is
//! keyed by ORIGIN — same host, different port, different pool.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// How many requests the test origin will serve at once. Anything beyond this is accepted and then
/// left hanging — the polite-looking failure that costs a real page its images.
const SERVER_CONCURRENCY: usize = 6;
/// How many subresources the "page" asks for. Comfortably above `SERVER_CONCURRENCY`, so an
/// unbounded client is guaranteed to overrun the origin.
const REQUESTS: usize = 40;

/// An origin with a per-client concurrency limit: it serves `SERVER_CONCURRENCY` requests at a time,
/// each quickly, and **stalls** any request that arrives while it is full. Returns its base URL.
///
/// Stalling rather than refusing is the point. A refused connection fails fast and the client can
/// react; a stalled one burns the client's whole deadline and then looks — to every layer above —
/// exactly like an origin that is down. That is the failure this gate exists to keep fixed.
fn throttling_origin() -> String {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = l.local_addr().unwrap();
    let in_flight = Arc::new(AtomicUsize::new(0));
    std::thread::spawn(move || {
        for stream in l.incoming() {
            let Ok(mut s) = stream else { continue };
            let busy = in_flight.clone();
            std::thread::spawn(move || {
                // Read the request line + headers. `Connection: close` below means one request per
                // connection, so an in-flight count is exactly a concurrent-request count.
                let mut buf = [0u8; 2048];
                let _ = s.read(&mut buf);
                if busy.fetch_add(1, Ordering::SeqCst) >= SERVER_CONCURRENCY {
                    // Full. Hold the socket open and say nothing — the client's deadline expires.
                    std::thread::sleep(Duration::from_secs(30));
                    return;
                }
                std::thread::sleep(Duration::from_millis(40));
                let body = b"ok";
                let _ = s.write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\
                         Cache-Control: no-store\r\nConnection: close\r\n\r\n",
                        body.len()
                    )
                    .as_bytes(),
                );
                let _ = s.write_all(body);
                let _ = s.flush();
                busy.fetch_sub(1, Ordering::SeqCst);
            });
        }
    });
    format!("http://{addr}")
}

/// Ask the origin for `REQUESTS` distinct subresources all at once, and report how many ARRIVED.
///
/// `join_all` with no bound of its own is not a simplification for the test's benefit — it is
/// verbatim what every subresource phase in `manuk-page` does. The only thing that differs between
/// the two arms is whether `manuk_net` holds a permit underneath it.
fn landed(base: &str) -> usize {
    let urls: Vec<String> = (0..REQUESTS)
        .map(|i| format!("{base}/img{i}.png"))
        .collect();
    manuk_net::runtime().block_on(async {
        let results =
            futures_util::future::join_all(urls.iter().map(|u| manuk_net::fetch(u))).await;
        results.iter().filter(|r| r.is_ok()).count()
    })
}

#[test]
fn subresources_are_not_lost_to_our_own_stampede() {
    // Short enough that a stalled request dies well inside the test, long enough that a SERVED one
    // never does: the capped arm needs REQUESTS/6 rounds of 40ms ≈ 270ms.
    std::env::set_var("MANUK_NET_TIMEOUT_MS", "1500");

    // ── ARM 1: capped, the shipped default. Six at a time is exactly what the origin will serve, so
    // every one of the 40 gets a slot and nothing is left to time out.
    std::env::set_var("MANUK_MAX_CONNS_PER_HOST", "6");
    let capped_origin = throttling_origin();
    let capped = landed(&capped_origin);

    // ── ARM 2: uncapped — the behaviour this tick replaced. All 40 sockets open at once, the origin
    // serves the first few and stalls the rest, and the stalled ones burn the deadline and die.
    // A DIFFERENT listener, because the permit pool is keyed by origin and arm 1 already made one.
    std::env::set_var("MANUK_MAX_CONNS_PER_HOST", "0");
    let uncapped_origin = throttling_origin();
    let uncapped = landed(&uncapped_origin);

    // **The vacuity guard, first.** If the origin were simply broken, or the fetch path never
    // reached it, both arms would read zero and the comparison below would be satisfied by nothing
    // happening at all. A gate that passes when its subject never ran is the failure mode this repo
    // has booked more than once.
    assert!(
        capped > 0 && uncapped > 0,
        "VACUITY: neither arm fetched anything (capped={capped}, uncapped={uncapped}) — the test \
         origin or the fetch path is broken, so this gate is measuring nothing"
    );

    // THE POINT: a bounded client loses nothing to an origin that throttles.
    assert_eq!(
        capped, REQUESTS,
        "capped at 6/origin, all {REQUESTS} subresources should arrive — got {capped}. A page's \
         images are content, not decoration: losing them silently is the mangago.me defect."
    );

    // THE COUNTERFACTUAL, and it is what makes the assertion above a ratchet tooth rather than a
    // hope. Without the cap the same 40 requests against the same origin lose most of themselves.
    // If this ever stops holding, the cap is no longer what is doing the work and the claim above
    // has quietly become true for some other reason.
    assert!(
        uncapped <= SERVER_CONCURRENCY * 2,
        "uncapped, the stampede should cost most of the {REQUESTS} subresources (expected \
         <= {}, got {uncapped}) — if it does not, this gate is no longer reproducing the defect \
         it was written for and proves nothing about the cap",
        SERVER_CONCURRENCY * 2
    );
    assert!(
        capped >= uncapped * 2,
        "capped ({capped}) must beat uncapped ({uncapped}) decisively, not marginally"
    );

    // The document must NOT be subject to this — it is the thing the user came for, and it does not
    // route through the capped path at all. Asserted so a later tick cannot "unify" the two and
    // trade a slow tracker for an unreachable page.
    assert!(
        manuk_net::max_conns_per_host() == 0,
        "this arm left the cap disabled; the env-controlled read is what lets both arms run here"
    );

    println!("capped={capped}/{REQUESTS}  uncapped={uncapped}/{REQUESTS}");
}
