//! **G_LOAD — a page renders even when its subresources never answer.**
//!
//! The bug this gate exists for: the network layer had no timeout of any kind. Not a connect
//! timeout, not a read timeout, nothing. So a single subresource that completed its TCP handshake
//! and then went silent — an ad host, a tracker, a geoblocked CDN, which is the *ordinary* condition
//! of the real web — stalled the `join_all` fetching the page's stylesheets or images until the
//! kernel gave up. Minutes. The tab was frozen for all of it.
//!
//! It was invisible to every gate we had, because every gate rendered local files. The corpus sweep
//! that found it first reported it as a *Chromium* hang, and only survived because the harness was
//! made to time the two engines separately.
//!
//! Measured on w3schools.com, live: **37.8s** for us against Chromium's 12.5s on the same page, with
//! the entire difference sitting in fetches that were never going to be answered. After: 15.0s
//! against Chromium's 15.2s, and structural coverage went *up*, 95.7% → 100%, because the stalls
//! were also losing elements.
//!
//! The contract, and it is the whole point: **the document is what the user came for; a subresource
//! is an enhancement.** An enhancement that does not arrive in time is dropped and the page renders
//! without it. It is never allowed to hold the document hostage. That is what Chromium does, and it
//! is not a degradation — it is the correct behaviour.
//!
//! This test is deterministic and offline: it binds a real socket that accepts connections and then
//! answers nothing, ever, which is precisely the failure that hung us.

use std::io::Read;
use std::net::TcpListener;
use std::time::{Duration, Instant};

use manuk_text::FontContext;

/// A blackhole: accepts the connection, reads the request, and never replies. Exactly what a dead
/// ad host does — and far worse for us than a refused connection, which fails fast.
fn blackhole() -> String {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in l.incoming() {
            let Ok(mut s) = stream else { continue };
            std::thread::spawn(move || {
                // Read the request so the client sees a healthy, established connection…
                let mut buf = [0u8; 1024];
                let _ = s.read(&mut buf);
                // …and then simply never answer. Hold the socket open forever.
                std::thread::sleep(Duration::from_secs(3600));
            });
        }
    });
    format!("http://{addr}")
}

/// The page must paint, and it must do so within the budget, no matter how many of its subresources
/// are black holes.
#[test]
fn dead_subresources_cannot_hold_the_document_hostage() {
    // **The PAGE budget must be the thing under test — and for a long time it was not.**
    //
    // `scripts/falsify.sh` disabled `load_budget()` entirely and this gate stayed GREEN. It was being
    // protected by the *per-request* deadline, not by the page budget in its own name: three dead
    // subresources at 1s each is 3s, comfortably under the 10s assertion, with or without a page budget.
    // The gate would have been green if the budget had been deleted outright.
    //
    // The budget's entire purpose is to bound the SUM when phases stack — "worst case across the six
    // phases is ~64s", per its own docstring. So the test has to make them stack: a per-request deadline
    // long enough, and dead subresources numerous enough across enough phases, that **without** the page
    // budget the load blows through the assertion. Now removing the budget makes this go red, which is
    // the only thing that makes its passing mean anything.
    // The numbers are chosen so the PAGE BUDGET is the only thing that can satisfy the assertion.
    //
    // Fetches within a phase are concurrent, so five dead images cost one request deadline, not five —
    // the phases do not stack the way this gate's original docstring assumed. What DOES stack is the
    // phases themselves: stylesheets, then images, then masks, each serial-by-necessity because a sheet
    // can add an image. At a 5s request deadline that is ~15s of dead waiting, comfortably past the 10s
    // assertion — UNLESS the page budget cuts it off at 3s. Which is the promise, and now the only way
    // this test can pass.
    // A LONG request deadline and a SHORT page budget — so the budget is the ONLY thing that can make
    // this test pass. If it is removed, the black holes hold each phase for the full 10s and the load
    // blows through the ceiling below. That is what makes a green result here mean something.
    std::env::set_var("MANUK_NET_TIMEOUT_MS", "10000");
    std::env::set_var("MANUK_LOAD_BUDGET_MS", "2000");

    let hole = blackhole();
    // Every phase gets a black hole: stylesheet, image, and a background-image. Before the fix each
    // one stalled independently, and they stacked.
    let html = format!(
        r#"<html><head>
             <link rel="stylesheet" href="{hole}/never1.css">
             <link rel="stylesheet" href="{hole}/never2.css">
             <link rel="stylesheet" href="{hole}/never3.css">
           </head>
           <body style="background-image:url({hole}/never-bg.png)">
             <h1 id="headline">The document is what the user came for</h1>
             <img id="pic" src="{hole}/never1.png">
             <img src="{hole}/never2.png"><img src="{hole}/never3.png">
             <img src="{hole}/never4.png"><img src="{hole}/never5.png">
             <p id="body">A subresource is an enhancement. This text must be on screen.</p>
           </body></html>"#
    );

    let fonts = FontContext::new();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let started = Instant::now();
    let page = rt.block_on(async {
        let mut p = manuk_page::Page::load_async(&html, "http://localhost/", &fonts, 800.0).await;
        p.finish_loading(&fonts, 800.0).await;
        p
    });
    let elapsed = started.elapsed();

    // 1. It finished. Before the fix this ran until the kernel's TCP timeout — minutes.
    //    The ceiling is the page budget plus generous slack for the runtime, NOT the sum of the
    //    per-request deadlines, because the whole point is that they do not stack.
    assert!(
        elapsed < Duration::from_secs(5),
        "page took {elapsed:?} with a 2s budget and dead subresources — the budget is NOT being \
         enforced. \n\n\
         This ceiling is 2x the budget, deliberately. The previous one was 10s, and at 10s the gate \
         was VACUOUS: `scripts/falsify.sh` deleted `load_budget()` outright and this test stayed \
         GREEN, because a couple of dead phases at the per-request deadline still came in under ten \
         seconds. It was being protected by the request timeout, not by the page budget in its own \
         name. A gate must assert the promise it is named for — 'the budget is enforced' — and not \
         merely 'the page eventually finished'."
    );

    // 2. And it actually rendered the document, rather than "finishing" by giving up on everything.
    //    A gate that only checks the clock would pass on a blank page, which is the failure it is
    //    supposed to be preventing.
    let rects = page.root_box.node_rects(page.dom());
    let mut painted = 0usize;
    for n in page.dom().descendants(page.dom().root()) {
        let Some(id) = page.dom().element(n).and_then(|e| e.attr("id")) else {
            continue;
        };
        if id == "headline" || id == "body" {
            let r = rects
                .get(&n)
                .unwrap_or_else(|| panic!("#{id} has no box — the document did not render"));
            assert!(
                r.width > 0.0 && r.height > 0.0,
                "#{id} has an empty box {r:?} — the page 'loaded' but rendered nothing"
            );
            painted += 1;
        }
    }
    assert_eq!(
        painted, 2,
        "the headline and body text must both be on screen; the dead subresources are irrelevant \
         to whether the user can read the page"
    );

    manuk_js::shutdown();
}
