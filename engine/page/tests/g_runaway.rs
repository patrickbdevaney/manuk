//! **G_RUNAWAY — a self-rescheduling timer must not hang the browser (Bar 0, Part 23).**
//!
//! The event loop drains to quiescence. That is correct — right up until a page schedules work that
//! reschedules itself. `setInterval(fn, 0)` is the obvious case, and it is on carousels, clocks,
//! pollers and progress bars all over the web; a self-reposting `requestAnimationFrame` is another.
//!
//! Without a ceiling, "drain to quiescence" means "never return", and the tab is gone with no recourse.
//! That is exactly the failure Bar 0 forbids: **a page that renders nine times out of ten and freezes
//! the browser the tenth violates the floor, no matter how good the nine looked.**
//!
//! The ceiling is deliberately generous — a real page's load-time task chain is tens of tasks, not tens
//! of thousands. Crossing it means the page is not converging, and the right answer is to render what we
//! have. A page that renders slightly early beats a browser that never comes back.

use std::time::{Duration, Instant};

use manuk_text::FontContext;

#[test]
fn a_runaway_interval_does_not_hang_the_browser() {
    // A timer that reschedules itself forever, and never stops. This is not a synthetic hazard: it is
    // one line of ordinary JavaScript, and it is on real pages.
    let html = r#"
        <div id="content">the page must still render</div>
        <script>
          setInterval(function(){ /* forever */ }, 0);
          var f = function(){ setTimeout(f, 0); };  // and the hand-rolled version of the same thing
          f();
        </script>"#;

    let fonts = FontContext::new();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let started = Instant::now();
    let page = rt.block_on(async {
        let mut p = manuk_page::Page::load_async(html, "http://localhost/", &fonts, 800.0).await;
        p.finish_loading(&fonts, 800.0).await;
        p
    });
    let elapsed = started.elapsed();

    assert!(
        elapsed < Duration::from_secs(20),
        "a self-rescheduling timer took {elapsed:?} — the event loop is draining forever and the tab \
         is frozen with no recourse. This is the Bar 0 floor, not a performance target."
    );

    // And it must still have RENDERED. A ceiling that returns a blank page has traded a hang for a
    // different kind of nothing, which is not an improvement.
    let rects = page.root_box.node_rects(page.dom());
    let content = page
        .dom()
        .descendants(page.dom().root())
        .find(|&n| page.dom().element(n).and_then(|e| e.attr("id")) == Some("content"))
        .expect("#content is in the tree");
    let r = rects.get(&content).expect("#content must have a box");
    assert!(
        r.width > 0.0 && r.height > 0.0,
        "#content has an empty box {r:?} — the page 'finished' by giving up on everything, which is \
         not what the ceiling is for"
    );

    manuk_js::shutdown();
}
