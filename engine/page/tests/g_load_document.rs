//! **G_LOAD (part 2) — the DOCUMENT gets a longer deadline than its subresources.**
//!
//! In its own binary, and that is not cosmetic. `request_timeout()` and `load_budget()` are process-wide
//! `OnceLock`s: the first caller to read them wins, forever. This test and `g_load_budget.rs` both set
//! `MANUK_NET_TIMEOUT_MS`, to *different values*, and cargo runs tests in parallel — so they were racing
//! for a global, and whichever got there first silently decided the other's timeout.
//!
//! That race is why `G_LOAD` could not be made to depend on its own budget: `scripts/falsify.sh` deleted
//! `load_budget()` outright and the gate stayed green, because the effective request deadline was 1s
//! (this test's value) rather than the 5s the other test had asked for. The gate was measuring a number
//! it did not control.
//!
//! One test file is one binary. Separated, each owns its own process and its own `OnceLock`.

use std::io::Read;
use std::net::TcpListener;
use std::time::{Duration, Instant};

use manuk_text::FontContext;

/// A blackhole: accepts the connection, reads the request, and never replies. Exactly what a dead ad
/// host does — and far worse for us than a refused connection, which fails fast.
fn blackhole() -> String {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in l.incoming() {
            let Ok(mut s) = stream else { continue };
            std::thread::spawn(move || {
                let mut buf = [0u8; 1024];
                let _ = s.read(&mut buf);
                std::thread::sleep(Duration::from_secs(3600));
            });
        }
    });
    format!("http://{addr}")
}

/// The other half, and it is not symmetric: the *document* must NOT share the subresource deadline.
///
/// Bounding the document at 8s would trade "some sites hang" for "some sites are unreachable" — a
/// slow-but-alive origin (a big page on a bad link, a cold cache) would simply fail to open. That is
/// not a fix, it is a different bug. The document gets a human-patience deadline; the enhancements
/// get a machine one.
#[test]
fn the_document_gets_a_longer_deadline_than_its_subresources() {
    // Both tests in this binary must agree on this value before either reads it: `request_timeout`
    // memoises into a `OnceLock`, so whichever test touches it first decides for both, and a gate
    // whose verdict depends on thread scheduling is not a gate.
    std::env::set_var("MANUK_NET_TIMEOUT_MS", "1000");
    let sub = manuk_net::request_timeout();
    // **Ask the engine, do not re-derive it.**
    //
    // This test used to carry its own copy of the `30` — so it was asserting a relationship between two
    // constants it had itself written down. Change `fetch_document`'s real default to 5s and this would
    // have kept passing, against its private copy, while the browser became unable to open a slow site.
    // A test that re-derives the value it is checking is not checking anything.
    let doc = manuk_net::document_timeout();
    assert!(
        doc > sub,
        "the document deadline ({doc:?}) must exceed the subresource deadline ({sub:?}) — otherwise \
         a slow but perfectly healthy site becomes unreachable, which is not a trade, it is a \
         second bug"
    );
}
