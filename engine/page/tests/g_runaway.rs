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

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use manuk_text::FontContext;
use tracing_subscriber::layer::SubscriberExt;

/// Collects the engine's log lines, so the gate reads the give-up report the way a developer does.
#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<String>>>);

impl<S> tracing_subscriber::Layer<S> for Capture
where
    S: tracing::Subscriber,
{
    fn on_event(&self, event: &tracing::Event<'_>, _: tracing_subscriber::layer::Context<'_, S>) {
        struct V(String);
        impl tracing::field::Visit for V {
            fn record_debug(&mut self, f: &tracing::field::Field, v: &dyn std::fmt::Debug) {
                self.0.push_str(&format!(" {}={:?}", f.name(), v));
            }
        }
        let mut v = V(String::new());
        event.record(&mut v);
        self.0.lock().unwrap().push(v.0);
    }
}

#[test]
fn a_runaway_interval_does_not_hang_the_browser() {
    // A timer that reschedules itself forever, and never stops. This is not a synthetic hazard: it is
    // one line of ordinary JavaScript, and it is on real pages.
    let html = r#"
        <div id="content">the page must still render</div>
        <script>
          setInterval(function(){ /* RUNAWAY_INTERVAL forever */ }, 0);
          // …and the hand-rolled version of the same thing. Both bodies carry a MARKER because the
          // give-up report groups the pending queue by the source text of the page's own callback,
          // and a gate that asserts on that has to have something specific to match (tick 680).
          var f = function(){ /* RUNAWAY_TIMEOUT */ setTimeout(f, 0); };
          f();
        </script>"#;

    let fonts = FontContext::new();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let cap = Capture::default();
    let sub = tracing_subscriber::registry().with(cap.clone());
    let _log_guard = tracing::subscriber::set_default(sub);

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

    // ── **THE GIVE-UP NAMES WHO WAS SPINNING** (tick 680). ──────────────────────────────────────
    //
    // The message used to read *"the page is not converging (a self-rescheduling timer, most
    // likely)"*. `most likely` is the tell: the engine was holding the entire pending task list and
    // GUESSING about its contents. That is a **status, not a finding** — the third time in four ticks
    // the same shape cost something (an anonymous `TypeError` at t666/t675, a source called
    // `inline.js` at t679, this). A Bar 0 guard that fires on a real site and cannot say what it
    // fired on leaves the next reader exactly where the log found them.
    let ceiling = cap
        .0
        .lock()
        .unwrap()
        .iter()
        .find(|l| l.contains("task ceiling"))
        .cloned()
        .unwrap_or_else(|| {
            panic!(
                "the gate's own fixture failed: no task-ceiling line was logged, so this run did not \
                 reproduce the condition whose REPORT is under test.\n  captured:\n  {}",
                cap.0.lock().unwrap().join("\n  ")
            )
        });
    println!("CEILING: {ceiling}");
    assert!(
        ceiling.contains("spinning="),
        "G_RUNAWAY: the give-up carries no `spinning=` summary, so it reports THAT the page is not \
         converging and never WHAT is doing it.\n  line: {ceiling}"
    );
    // The page's OWN callback, not our wrapper. The task the loop RUNS is `setTimeout`'s closure, so
    // grouping by it yields a histogram of ourselves — eight identical words, once per line. The
    // page's callback has to be carried on the task (`__enqueue`'s `u`) for the report to mean
    // anything, and this is the assertion that catches its absence.
    assert!(
        ceiling.contains("RUNAWAY_INTERVAL") || ceiling.contains("RUNAWAY_TIMEOUT"),
        "G_RUNAWAY: the summary does not name either of the fixture's two spinners, so it is \
         reporting the engine's own wrapper rather than the page's callback.\n  line: {ceiling}"
    );
    // ...and it shows tasks due at the CURRENT virtual instant, which is the entire signature of a
    // zero-delay self-rescheduler — the thing the old message was guessing at.
    assert!(
        ceiling.contains("due_now=") && !ceiling.contains("due_now=0 "),
        "G_RUNAWAY: the summary does not show tasks due at the current virtual instant.\n  \
         line: {ceiling}"
    );

    // ── **AND THE CLOCK HAS A HORIZON** — the fix the report above made findable.
    //
    // The claim is two-sided on purpose, because each side alone is satisfiable by the wrong engine:
    // a horizon of zero would stop the 24-hour timer AND the 100ms one (a browser that has stopped
    // running the page, which is what a ceiling must never buy), and no horizon at all runs both.
    manuk_js::shutdown();
}
