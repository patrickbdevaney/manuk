//! **G_DEFER — `defer` / `async` / `type=module` must not block first paint, and must still RUN.**
//!
//! `defer` and `is_async` were parsed into a struct and used for **nothing**. Every script blocked first
//! paint, including the ones whose entire purpose is to say *"do not wait for me"*, and including
//! `type="module"` — which is **deferred by default** in every real browser and is what every Vite /
//! Rollup bundle on the internet ships as. Measured on nytimes.com: ~1MB of JavaScript executing while
//! the window sat blank, with the document already parsed, cascaded and laid out.
//!
//! This gate asserts **both halves**, because each without the other is a bug:
//!
//!   1. A deferred script does **not** run on the paint path (`from_prefetched_blocking_only`).
//!   2. It **does** run afterwards, and its DOM mutations land.
//!   3. A *blocking* script still runs before paint — pages depend on that, and "everything is deferred
//!      now" would be a far worse bug than the one being fixed.
//!   4. **Every non-shell path still runs all of them.** `Page::load` must behave exactly as it always
//!      has. When the split first landed I forgot this on `load_async`, and **every SPA in the suite
//!      silently stopped mounting** — a Vite bundle is a module, nothing ran the deferred pass, and the
//!      root element sat there, correctly sized, empty. Assertion (4) is that bug, pinned.
//!
//! Half of this gate is about speed and half is about *not lying about speed*. "Fast because we never
//! ran the script" is the same class of lie as "fast because we never loaded the images" — which is the
//! disguise G_FIRST_PAINT was written to strip off, one tick earlier.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body>
    <div id="blocking">no</div>
    <div id="deferred">no</div>
    <div id="asyncd">no</div>
    <div id="module">no</div>
    <script>document.getElementById('blocking').textContent = 'ran';</script>
    <script defer>document.getElementById('deferred').textContent = 'ran';</script>
    <script async>document.getElementById('asyncd').textContent = 'ran';</script>
    <script type="module">document.getElementById('module').textContent = 'ran';</script>
  </body></html>"#;

fn text(page: &manuk_page::Page, sel: &str) -> String {
    let root = page.dom().root();
    let hits = manuk_css::query_selector_all(page.dom(), root, sel);
    assert!(!hits.is_empty(), "{sel} must exist in the document");
    page.dom().text_content(hits[0])
}

/// **One test, on purpose.** Two tests in one binary each stand up a SpiderMonkey context, and the
/// leaked per-process runtime tears down messily when they co-run — the binary segfaults *sometimes*,
/// which is worse than failing: a flaky gate gets ignored, and an ignored gate protects nothing. The
/// `js_conformance` suite is one giant test for exactly this reason.
#[test]
fn defer_async_module_do_not_block_paint_but_do_run() {
    let fonts = FontContext::new();

    // The paint path: `from_dom` runs ONLY the scripts that block paint.
    let mut page = manuk_page::Page::from_dom(
        manuk_html::parse(HTML),
        "https://defer.test/",
        &fonts,
        800.0,
    );

    // (3) A classic script with neither `defer` nor `async` blocks the parser, so it has already run.
    //     Pages depend on this — a blocking script that writes into the DOM must have done so before
    //     anything is measured or shown. "Everything is deferred now" would be a worse bug than the one
    //     this gate exists to fix.
    assert_eq!(
        text(&page, "#blocking"),
        "ran",
        "G_DEFER: a BLOCKING <script> did not run before paint. Blocking scripts must still block."
    );

    // (1) And the deferred ones have NOT — which is the entire point. On nytimes this is ~1MB of
    //     JavaScript that is no longer standing between the user and the article.
    for (sel, what) in [
        ("#deferred", "<script defer>"),
        ("#asyncd", "<script async>"),
        ("#module", "<script type=module> (deferred by DEFAULT in every real browser)"),
    ] {
        assert_eq!(
            text(&page, sel),
            "no",
            "G_DEFER: {what} RAN on the paint path. It must not — that is what the attribute means, \
             and it is ~1MB of JavaScript on a news front page standing between the user and the article."
        );
    }

    // (2) Now the shell has painted. The deferred scripts run, and their mutations land.
    let ran = page.run_deferred_scripts(&fonts, 800.0);
    assert_eq!(
        ran, 3,
        "G_DEFER: {ran} deferred scripts ran, expected 3 (defer, async, module). \
         Speed achieved by never running the script is not speed, it is a missing feature with good \
         benchmarks."
    );
    for (sel, what) in [("#deferred", "defer"), ("#asyncd", "async"), ("#module", "module")] {
        assert_eq!(
            text(&page, sel),
            "ran",
            "G_DEFER: the {what} script never ran at all. Deferred means LATER, not NEVER."
        );
    }

    // ── (4) **Every non-shell path still runs all of them.**
    //
    // This is the bug the split actually caused: `load_async` called `from_dom` and nothing else, so a
    // Vite bundle — a module, deferred by default — never executed, and **every SPA in the suite
    // silently stopped mounting**. The root element was still there, still the right size, and
    // completely empty.
    let page = manuk_page::Page::load(HTML, "https://defer.test/", &fonts, 800.0);
    for (sel, what) in [
        ("#blocking", "blocking"),
        ("#deferred", "defer"),
        ("#asyncd", "async"),
        ("#module", "module"),
    ] {
        assert_eq!(
            text(&page, sel),
            "ran",
            "G_DEFER: `Page::load` did not run the {what} script.\n  \
             Every path that used to run all the scripts must STILL run all the scripts. There is \
             exactly one caller allowed to split the two phases — the shell — because it is the only \
             one with a human waiting. When this was forgotten on `load_async`, every SPA in the suite \
             stopped mounting, silently."
        );
    }
}
