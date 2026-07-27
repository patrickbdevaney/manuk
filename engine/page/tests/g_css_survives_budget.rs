//! **G_CSS_SURVIVES_BUDGET — a stylesheet we already downloaded must reach the cascade even when
//! the load budget expires while chasing its `@font-face`.**
//!
//! `finish_loading` enforces the load budget as a **hard deadline** — dropped mid-fetch, wherever it
//! runs out — on the stated ground that *"a dropped future loses that phase's ENHANCEMENT and never a
//! half-mutated document"*. For the stylesheet phase that was false. The phase's order was
//!
//! ```text
//!   fetch every <link> sheet  →  walk @import (3 network rounds)  →  fetch every @font-face src
//!   (a SEQUENTIAL loop)  →  ...and only THEN cascade
//! ```
//!
//! so a cancellation anywhere in the middle threw away every author stylesheet that was **already
//! fully downloaded and sitting in memory**. A page with no author CSS is not a lost enhancement: it
//! is the entire design of the site gone. Every element becomes a full-width block in the default
//! serif at 16px, stacked, and the document runs several times too tall.
//!
//! `keirin.jp` is the measured case, and its log is the bug's confession: nine sheets, 375KB, all
//! nine logged `stylesheet applied` at +0.2s — then eleven and a half seconds inside font-awesome's
//! per-face `src` ladder, then `load budget of 12.0s exhausted mid-phase`, with the future dying two
//! stages above the cascade. **Coverage 98% against SHAPE 2.1%**: we rendered every element Chromium
//! did, in `serif/16` where Chromium had `{Meiryo UI/…}`, and the stylesheets had been on the machine
//! the whole time.
//!
//! **How this gate makes the deadline fire on purpose.** The socket serves `site.css` immediately and
//! then *stalls forever* on the font it declares — the exact shape of a slow font CDN, and the shape
//! that made keirin's phase overrun. With `MANUK_LOAD_BUDGET_MS` set small, the phase is guaranteed
//! to be cancelled inside the font fetch, which is precisely where the old code lost the sheet. So
//! the assertion is not "the page is styled" in general; it is **"the page is styled even though the
//! budget expired after the sheet arrived and before the font did"**.
//!
//! Hermetic: one local socket, no live origin, and the stall is ours to control — this gate cannot
//! false-RED on the network, and it cannot pass by the deadline failing to fire (that is asserted
//! separately, below).

use manuk_text::FontContext;
use std::io::{Read, Write};
use std::net::TcpListener;

/// Nothing like the 800px viewport, so "the sheet cascaded" and "the sheet was discarded and the div
/// became a full-width UA block" can never be read as the same number.
const AUTHOR_WIDTH: i64 = 275;

const HTML: &str = r##"<!doctype html><html><head>
<link rel="stylesheet" href="site.css">
</head><body>
<div id="box">styled by a sheet that arrived BEFORE the budget ran out</div>
</body></html>"##;

/// The sheet declares a webfont, so the phase goes to the network again *after* this sheet is
/// already in hand — and `/slow.woff2` never answers.
const CSS: &str = "@font-face { font-family: Stall; src: url(slow.woff2) format('woff2'); }\n\
                   #box { width: 275px; height: 30px; font-family: Stall, serif; }\n";

/// The laid-out width of `#box`, or `None` if it has no box.
fn width_of(page: &manuk_page::Page) -> Option<i64> {
    let root = page.dom().root();
    let node = *manuk_css::query_selector_all(page.dom(), root, "#box").first()?;
    page.root_box
        .node_rects(page.dom())
        .get(&node)
        .map(|r| r.width.round() as i64)
}

#[test]
fn a_downloaded_stylesheet_is_not_discarded_when_the_budget_dies_fetching_its_font() {
    let tmp = std::env::temp_dir().join(format!("manuk-cssbudget-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).ok();
    unsafe { std::env::set_var("MANUK_STATE", &tmp) };
    // Small enough that the stalled font MUST outlast it, large enough that the sheet itself (served
    // from a loopback socket in microseconds) is comfortably inside it. If this were so small that
    // the sheet also missed, the gate would be asserting nothing — which the `served` check below
    // is what actually rules out.
    unsafe { std::env::set_var("MANUK_LOAD_BUDGET_MS", "1200") };

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
                let req = String::from_utf8_lossy(&buf[..n]);
                let path = req
                    .lines()
                    .next()
                    .and_then(|l| l.split_whitespace().nth(1))
                    .unwrap_or("/")
                    .to_string();
                if path == "/site.css" {
                    counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let body = CSS.as_bytes();
                    let head = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/css\r\nContent-Length: {}\r\n\
                         Connection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = sock.write_all(head.as_bytes());
                    let _ = sock.write_all(body);
                    let _ = sock.flush();
                    return;
                }
                // **THE STALL.** A slow font CDN: the connection is accepted and then never answered,
                // so the phase is still awaiting it when the load budget expires. This is the shape
                // that made keirin's stylesheet phase overrun, reproduced deterministically.
                std::thread::sleep(std::time::Duration::from_secs(30));
            });
        }
    });

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let base = format!("http://{addr}/index.html");
    let fonts = FontContext::new();
    let started = std::time::Instant::now();
    let page = rt.block_on(async {
        let mut p = manuk_page::Page::load_async(HTML, &base, &fonts, 800.0).await;
        p.finish_loading(&fonts, 800.0).await;
        p
    });
    let elapsed = started.elapsed();
    println!(
        "CSS-BUDGET PROBE: sheet_served={} elapsed={:?} box_width={:?}",
        served.load(std::sync::atomic::Ordering::SeqCst),
        elapsed,
        width_of(&page)
    );

    // ── PRECONDITIONS, asserted rather than assumed — otherwise a green here could mean
    // "the sheet never arrived" or "the deadline never fired", both of which test nothing.
    assert!(
        served.load(std::sync::atomic::Ordering::SeqCst) >= 1,
        "the gate's own fixture failed: `site.css` was never requested, so nothing was in hand to \
         be discarded and this test would be asserting the empty case."
    );
    assert!(
        elapsed < std::time::Duration::from_secs(20),
        "the load did not return on the budget at all ({elapsed:?}) — the stalled font was awaited \
         to completion, so this run never exercised a mid-phase cancellation."
    );

    // ── THE CLAIM.
    assert_eq!(
        width_of(&page),
        Some(AUTHOR_WIDTH),
        "G_CSS_SURVIVES_BUDGET: `#box` is not {AUTHOR_WIDTH}px, so the stylesheet that had ALREADY \
         been downloaded never reached the cascade — the load budget expired while the phase was \
         still fetching that sheet's `@font-face`, and the apply sat below the fetch. Commit the \
         sheets where they are complete (`apply_stylesheets` right after the top-level fetch loop) \
         and let `@import`/`@font-face` be the enhancements the budget is allowed to drop. \
         Rendering a site with no author CSS is not a degraded page; it is a different page."
    );
}
