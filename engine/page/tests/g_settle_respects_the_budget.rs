//! **G_SETTLE_RESPECTS_THE_BUDGET — settling a round of fetch results must stop when the load budget
//! is gone, not only between rounds.**
//!
//! `pump_page_fetches` performs up to `MAX_PER_ROUND` (40) of the page's own `fetch`/XHR requests per
//! round and then settles them in a loop. **Settling one runs the page's promise continuation, which
//! drains the event loop** — so a page that is not converging pays a full drain ceiling *per settled
//! request*, and that loop had no clock in it at all. The outer per-round check cannot see any of it.
//!
//! Measured on `www.agoda.com` with the per-phase ledger (tick 670), against a **12-second** budget:
//!
//! ```text
//!   before   phase="page fetches"  ms=36061  gave_up=15      finish_loading 39.9s · TOTAL 43.6s
//!   after    phase="page fetches"  ms=13190  gave_up=5       finish_loading 15.4s · TOTAL 19.0s
//! ```
//!
//! **This takes nothing the budget was not already discarding.** Past the deadline `finish_loading`
//! skips images, masks and background images outright — so continuing to settle buys the page nothing
//! and costs it those phases. Stopping is the same promise `finish_loading` already makes (*whatever
//! has arrived is what the page gets*), applied where the time is actually spent.
//!
//! The gate asserts **wall clock**, because that is the promise and it is what the user feels. It is
//! deliberately not an assertion about give-up counts: those are an implementation's way of spending
//! the budget, and a future implementation that spends it differently should still pass.
//!
//! Hermetic: one loopback socket, no live origin.

use manuk_text::FontContext;
use std::io::{Read, Write};
use std::net::TcpListener;

/// Enough requests that settling them one-by-one without a clock overruns by MULTIPLES of the budget,
/// and few enough that the honest path stays quick. Sized from the RED-proof: at twelve the gap was
/// 1.9s against 3.4s, which a ceiling loose enough not to flake could not straddle. **A gate whose
/// fixture cannot separate the two answers by more than its own noise is not a gate**, so the fixture
/// was made harder rather than the threshold tighter — the same choice this project made when a
/// tightened threshold was the tempting fix (`measurement-first`, `vary the MECHANISM not the
/// threshold`).
const N_FETCHES: usize = 30;
/// The load budget for this gate.
const BUDGET_MS: u64 = 1500;

/// Each settled response starts a self-rescheduling timer, so **every settle** costs a full drain.
/// That is the shape `pump_page_fetches` had no clock for.
const HTML: &str = r##"<!doctype html><html><body><div id="d">x</div>
<script>
  for (var i = 0; i < 30; i++) {
    fetch('data' + i + '.json').then(function (r) { return r.text(); }).then(function () {
      var n = 0;
      (function spin() { n++; setTimeout(spin, 0); })();
    });
  }
</script>
</body></html>"##;

#[test]
fn a_round_of_settles_stops_when_the_budget_is_gone() {
    let tmp = std::env::temp_dir().join(format!("manuk-settle-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).ok();
    unsafe { std::env::set_var("MANUK_STATE", &tmp) };
    unsafe { std::env::set_var("MANUK_LOAD_BUDGET_MS", &BUDGET_MS.to_string()) };
    // Each spinning drain reaches its bound quickly, so thirty of them is a visible overrun rather
    // than an overnight one. The gate is about whether the CLOCK is consulted, not about its value.
    unsafe { std::env::set_var("MANUK_MAX_DRAIN_MS", "250") };

    let served = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let counter = served.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut sock) = stream else { continue };
            let counter = counter.clone();
            std::thread::spawn(move || {
                let mut buf = vec![0u8; 8192];
                let n = sock.read(&mut buf).unwrap_or(0);
                if n == 0 {
                    return;
                }
                counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let body = b"{}";
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\
                     Connection: close\r\n\r\n",
                    body.len()
                );
                let _ = sock.write_all(head.as_bytes());
                let _ = sock.write_all(body);
                let _ = sock.flush();
            });
        }
    });

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let fonts = FontContext::new();
    let base = format!("http://{addr}/index.html");

    let started = std::time::Instant::now();
    rt.block_on(async {
        let mut p = manuk_page::Page::load_async(HTML, &base, &fonts, 800.0).await;
        p.finish_loading(&fonts, 800.0).await;
    });
    let elapsed = started.elapsed();
    let fetched = served.load(std::sync::atomic::Ordering::SeqCst);
    println!("SETTLE PROBE: finish_loading path took {elapsed:?}; {fetched} request(s) served");

    // ── PRECONDITION: the page really did issue its requests. A green with nothing served would mean
    //    the pump had nothing to settle, and the assertion below would be about the empty case.
    assert!(
        fetched >= N_FETCHES / 2,
        "the gate's own fixture failed: only {fetched} of {N_FETCHES} requests reached the server, so \
         the settle loop never had a round to work through and this run tested nothing."
    );

    // ── THE CLAIM: wall clock. The budget is 1.5s; the whole load — parse, the budgeted phases, and
    //    one settle that cannot be preempted mid-JavaScript — must land well inside a multiple of it.
    //    Without the check, thirty spinning settles run to their own ceilings back to back with no
    //    clock between them, and the phase overruns by many multiples. RED-proof, same machine:
    //    **1.90s with the check, 7s+ without** — a margin the 4x ceiling straddles comfortably in both
    //    directions, which is the property that makes a timing assertion honest rather than flaky.
    let ceiling = std::time::Duration::from_millis(BUDGET_MS * 4);
    assert!(
        elapsed < ceiling,
        "G_SETTLE_RESPECTS_THE_BUDGET: the load took {elapsed:?} against a {BUDGET_MS}ms budget. \
         `pump_page_fetches` settles up to 40 results per round, and SETTLING ONE RUNS THE PAGE'S OWN \
         JS — so a non-converging page pays a full drain ceiling per settled request while the outer \
         per-round check sees none of it. Measured on a real site: 36.1s and 15 give-ups in this one \
         phase, against a 12s budget. Check the clock BETWEEN SETTLES; it takes nothing the budget was \
         not already discarding, because past the deadline the image, mask and background phases are \
         skipped anyway."
    );
}
