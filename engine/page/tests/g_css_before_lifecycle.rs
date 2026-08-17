//! **G_CSS_BEFORE_LIFECYCLE — the author's stylesheets were applied AFTER `load` had already fired,
//! so every script the page has measured a document with no CSS in it.**
//!
//! `load_async` — the path the agent, the oracle and every fidelity measurement navigate with —
//! fetched and applied external stylesheets in `finish_loading`, which runs *after* `DOMContentLoaded`
//! and *after* `load`. The navigation ledger printed the order plainly for eight ticks and nobody read
//! it as an ordering:
//!
//! ```text
//!   cascade+layout+blocking scripts
//!   DOMContentLoaded
//!   load event                 <- every script the page has has now run
//!   initial images+masks
//!   external CSS               <- the site's CSS arrives HERE
//! ```
//!
//! **This is not a styling bug. It is a MEASUREMENT bug, and the page is the one measuring.**
//! The final painted layout was always correct — the sheets did arrive, just late — so every
//! screenshot, every box dump and every fidelity score looked exactly as it should. What was wrong
//! was what the *page* could see while it was running: on a three-line fixture whose only sheet is
//! one `<link>`, `getComputedStyle(el).display` read `block` where the engine's own final layout is
//! `flex`, and a `width:70%` child measured the full `1184px`, at `DOMContentLoaded`, at `load`, and
//! from every timer after them.
//!
//! Every carousel, sticky header, virtualised list, masonry grid, chart library and "is this element
//! on screen" check reads geometry and writes the answer back into the DOM. Fed UA-default geometry
//! they write a wrong answer that **no later cascade can undo** — the stylesheet arrives afterwards
//! and restyles a tree the page has already mis-built.
//!
//! The spec is not subtle: a `<link rel=stylesheet>` is render-blocking *and* script-blocking, and
//! `load` does not fire until the style sheets have loaded.
//!
//! ⚠ **The named non-claim CLOSED at t1296, and claim (4) below is now positive.** It read: *"a
//! script that BLOCKS the parser still runs before the sheets are applied — closing that means
//! fetching the sheets before the `Page` exists."* That is exactly what happens now: the already-
//! fetched sheets are **seeded into `from_dom`** (`PENDING_EXTERNAL_CSS`, the same shape as the CSP
//! and external-script seeds), so they are in the FIRST cascade rather than the one after the
//! scripts. The design that made it affordable is in the table below.
//!
//! ⚠⚠ **Written at t714, reverted twice, landed at t719 on the THIRD design.** The first two are
//! recorded because the arithmetic that killed them is the reason this one looks the way it does.
//! `G_LOAD` bounds the whole page at **2x the load budget**; one budget is already spent in
//! `load_async` (enhancements) and one in `finish_loading`, so a third *waiting* phase has no room:
//!
//! ```text
//!   its own budget          load_async spends 2x alone, page 3x   ->  G_LOAD red, 5.4s against 2s
//!   a shared slice          the phase it shares with starves      ->  keirin.jp SHAPE 60.2 -> 53.0
//!   concurrent with scripts the fixture has dead sheets and NO     ->  G_LOAD red, that phase 0s -> 2s
//!                           scripts, so there was no wait to hide behind
//!   fetched at parse,       nothing waits, so nothing can starve  ->  G_LOAD 3.51s · keirin 60.2%
//!   taken if FINISHED                                                 ikea 97.1% -> 100.0% coverage
//! ```
//!
//! The landed design starts the sheet fetches immediately after the parse and, at the apply point,
//! takes only the handles that have **already finished** — never awaiting one. A page's stylesheets
//! get the external-script fetch, the module-graph prefetch, the cascade, layout and every blocking
//! script of head start; a sheet that has not arrived by then falls through to `finish_loading`
//! exactly as it did before. Strictly more capability, zero added latency, nothing to starve.
//!
//! ⚠⚠⚠ **t1296 — AND THE FREE HARVEST ALONE DOES NOT REACH THE BLOCKING SCRIPTS, MEASURED.** Moving
//! that harvest one phase earlier (before `from_dom`, which is where the blocking scripts run) is
//! free and was tried first. On a loopback origin it banks **nothing**: `html parse` 0ms, `external
//! scripts` 2ms, `module graph prefetch` 0ms, and the sheet lands at ~5ms. So a *wait* is required,
//! and the row that killed the third design above — *"the fixture has dead sheets and NO scripts, so
//! there was no wait to hide behind"* — is answered by making the wait conditional on exactly that:
//!
//! ```text
//!   waits ONLY when a blocking       the dead-sheets/no-scripts fixture waits ZERO, because with
//!   <script> exists to observe it    no blocking script nothing can observe the difference
//!   never past nav_start+budget      G_LOAD's 2x page bound is arithmetically untouched
//!   never more than budget/4         a slow origin cannot convert this into the phase t715 reverted
//! ```
//!
//! ⚠ `www.welt.de` collapsed 95.6% -> 0.03% under the first two designs and does **not** under this
//! one. That collapse was never the ordering: t715 reproduced it on the *unmodified* engine with
//! nothing changed but `MANUK_LOAD_BUDGET_MS=12000 -> 40000`. welt blanks itself whenever our engine
//! lets its anti-adblock guard reach a verdict, and the earlier designs got it there by freeing
//! budget. This one frees none, so welt is unchanged at 95.7%.

use std::io::{Read, Write};
use std::net::TcpListener;

use manuk_text::FontContext;

/// Serves ONE stylesheet. Everything else 404s, so a pass cannot come from anywhere but this sheet.
fn css_origin() -> String {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in l.incoming() {
            let Ok(mut s) = stream else { continue };
            std::thread::spawn(move || {
                let mut buf = [0u8; 8192];
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                let path = req
                    .lines()
                    .next()
                    .and_then(|l| l.split_whitespace().nth(1))
                    .unwrap_or("/")
                    .to_string();
                let (status, ctype, body): (&str, &str, &str) = match path.as_str() {
                    "/site.css" => (
                        "200 OK",
                        "text/css",
                        ".shell{display:flex}\n.main{width:70%}\n.side{width:30%}\n",
                    ),
                    _ => ("404 Not Found", "text/plain", "no"),
                };
                let _ = s.write_all(
                    format!(
                        "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\n\
                         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                );
            });
        }
    });
    format!("http://{addr}")
}

fn text(page: &manuk_page::Page, sel: &str) -> String {
    let root = page.dom().root();
    let hits = manuk_css::query_selector_all(page.dom(), root, sel);
    assert!(!hits.is_empty(), "{sel} must exist in the document");
    page.dom().text_content(hits[0])
}

/// **One test, on purpose** — two SpiderMonkey contexts in one binary tear down messily and the
/// binary segfaults *sometimes*, which is worse than failing (see `g_defer`).
#[test]
fn a_script_measuring_the_page_sees_the_authors_stylesheets() {
    let fonts = FontContext::new();
    let origin = css_origin();

    // Each observation is written into its own element the instant it is taken, so a later phase
    // cannot overwrite an earlier one's evidence — the failure this gate is about is precisely a
    // phase ordering, and a single accumulating string hides which phase produced what.
    let html = format!(
        r#"<!doctype html><html><head><meta charset="utf-8">
             <link rel="stylesheet" href="{origin}/site.css">
           </head><body>
             <div class="shell" id="shell"><div class="main" id="main">M</div><div class="side">S</div></div>
             <div id="at-blocking">-</div>
             <div id="at-dcl">-</div>
             <div id="at-load">-</div>
             <div id="at-timer">-</div>
             <script>
               function obs() {{
                 var s = getComputedStyle(document.getElementById('shell')).display;
                 var w = Math.round(document.getElementById('main').getBoundingClientRect().width);
                 return s + '/' + w;
               }}
               document.getElementById('at-blocking').textContent = obs();
               document.addEventListener('DOMContentLoaded', function () {{
                 document.getElementById('at-dcl').textContent = obs();
               }});
               window.addEventListener('load', function () {{
                 document.getElementById('at-load').textContent = obs();
                 setTimeout(function () {{
                   document.getElementById('at-timer').textContent = obs();
                 }}, 0);
               }});
             </script>
           </body></html>"#
    );

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    // `load_async` ONLY — deliberately. `finish_loading` is where the sheets used to be applied, so
    // calling it would let the bug's own late apply satisfy the gate. The whole claim is that the
    // page is styled by the time `load_async` has fired its lifecycle events.
    let page = rt.block_on(manuk_page::Page::load_async(
        &html,
        &format!("{origin}/index.html"),
        &fonts,
        1200.0,
    ));

    let dcl = text(&page, "#at-dcl");
    let load = text(&page, "#at-load");
    let timer = text(&page, "#at-timer");
    let blocking = text(&page, "#at-blocking");
    println!("CSS-BEFORE-LIFECYCLE  blocking={blocking} dcl={dcl} load={load} timer={timer}");

    // The engine's own final answer, which was ALWAYS right — this is the value the page should have
    // been able to read, and quoting it in the failure message is what stops the next reader
    // concluding the cascade is broken when it is the schedule that is.
    // `1184` is the 1200px viewport less the UA `body{margin:8px}`; `829` is 70% of it. Both are
    // MEASURED on this fixture, not derived — a gate whose expected value came from arithmetic in
    // someone's head tests the arithmetic.
    let want = "flex/829";

    // (1) **`DOMContentLoaded`.** RED: move the `fetch_and_apply_stylesheets` call in `load_async`
    // back below `fire_lifecycle("DOMContentLoaded")` → `block/1184` (the UA-default full width).
    assert_eq!(
        dcl, want,
        "at DOMContentLoaded a script must see the AUTHOR's layout, not the UA default \
         (the engine's own final layout here is {want})"
    );

    // (2) **`load`.** Separately asserted, because the two lifecycle events are fired from different
    // places and a fix that reaches one need not reach the other.
    assert_eq!(
        load, want,
        "at `load` a script must see the AUTHOR's layout — the spec does not even let `load` fire \
         until the style sheets have loaded"
    );

    // (3) **A timer after `load`.** This is where the real damage lands: a library that defers its
    // measurement to the next task (which most do, to avoid a forced reflow) was measuring an
    // unstyled document *after* the page had finished loading.
    assert_eq!(
        timer, want,
        "a task queued after `load` must see the AUTHOR's layout"
    );

    // (4) **THE NAMED NON-CLAIM, NOW A CLAIM (t1296).** This assertion used to pin the honest
    // failing value — `assert_eq!(blocking, "block/1184")` — with a note saying that if it ever read
    // `flex/829` the remaining half had landed and the gate should be updated. It reads `flex/829`.
    //
    // RED, three ways, each isolating one link of the chain:
    //
    // ```text
    //   (a) drop `set_pending_external_css` in `load_async`   ->  block/1184
    //   (b) drop the bounded wait (free harvest only)         ->  block/1184  (loopback is TOO FAST:
    //                                                              2ms of head start, sheet at ~5ms)
    //   (c) restore `no_external_css` in `from_dom`'s
    //       ReflowScope, keeping (a) and (b)                  ->  block/1184  (the initial cascade IS
    //                                                              styled; `getComputedStyle` forces
    //                                                              a reflow that un-styles it)
    // ```
    //
    // ⚠ (c) is the one worth keeping: the fix was fully present and fully invisible for one build,
    // because a third copy of the sheet list — the forced-reflow scope's — was still empty.
    assert_eq!(
        blocking, want,
        "a parser-BLOCKING script must see the AUTHOR's layout: `<link rel=stylesheet>` is \
         script-blocking, and a page that measures at parse time is the reason the spec says so"
    );
}
