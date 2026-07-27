//! **G_SILENT_FAIL — an error on the load/render/script path must never be swallowed.**
//!
//! This gate is named by a specific, expensive failure. For several ticks the ledger recorded
//! *"React mounts, schedules, throws nothing, renders nothing"* as a **React** problem. It was not.
//! React was throwing — `o.createElement is not a function`, entirely truthfully — and the error was
//! going nowhere, because it was raised inside an async render and nothing was listening. The engine
//! reported a clean load of an empty page.
//!
//! A browser that fails silently is worse than one that fails loudly, because a silent failure is
//! indistinguishable from *"the site is like that"*. It sends you looking in the wrong codebase, and it
//! is why the promise-rejection tracker was the single highest-value thing added to the JS layer: the
//! moment it existed, Lit and Svelte both stopped being mysteries and started being **error messages**.
//!
//! What this asserts:
//!
//!   1. A **synchronous** script error is surfaced (not swallowed by the script runner).
//!   2. An **unhandled promise rejection** is surfaced. *Every modern framework renders inside an async
//!      function*, so this is where their failures go to die.
//!   3. A **dead subresource** is surfaced, not silently treated as an empty stylesheet.
//!   4. The page still renders through all of it — surfacing an error is not the same as giving up.
//!
//! Errors are surfaced through `tracing`, so the gate installs a subscriber and reads what came out.
//! Asserting on the *log* is deliberate: the log is what a developer actually sees, and a gate should
//! test what the user of the thing experiences, not an internal flag they will never look at.

use std::sync::{Arc, Mutex};

use manuk_text::FontContext;
use tracing::subscriber::DefaultGuard;
use tracing_subscriber::layer::SubscriberExt;

/// Collects every log line the engine emits, so the gate can assert that a failure was *said out loud*.
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
        let line = format!("{} {}", event.metadata().level(), v.0);
        self.0.lock().unwrap().push(line);
    }
}

fn capture() -> (Capture, DefaultGuard) {
    let cap = Capture::default();
    let sub = tracing_subscriber::registry().with(cap.clone());
    let guard = tracing::subscriber::set_default(sub);
    (cap, guard)
}

impl Capture {
    fn saw(&self, needle: &str) -> bool {
        self.0
            .lock()
            .unwrap()
            .iter()
            .any(|l| l.to_lowercase().contains(&needle.to_lowercase()))
    }
    fn dump(&self) -> String {
        self.0.lock().unwrap().join("\n  ")
    }
    /// The first captured line containing `needle`. An assertion that a message and its ADDRESS both
    /// appear *somewhere* in the log is satisfied by two unrelated lines; this makes the gate ask the
    /// question it means — *did the line that named the error also name where it happened?*
    fn line_with(&self, needle: &str) -> Option<String> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .find(|l| l.contains(needle))
            .cloned()
    }
}

#[test]
fn a_script_error_is_never_swallowed() {
    let (cap, _g) = capture();
    let fonts = FontContext::new();

    // (1) A synchronous throw, and (2) an unhandled rejection from an async function — which is where
    //     every framework's failures actually land, and where React's went for several ticks.
    // (5) and (6) are tick 675: the two JS-side error funnels, which stored errors instead of saying
    //     them and threw away the address on the way. See the assertions below for what each one is.
    let html = r#"<!doctype html><html><body>
        <h1 id="headline">The document still renders</h1>
        <div id="out">-</div>
        <script>
          // (5) The page reports its OWN exception. Its own handler must run, and must be told WHERE.
          var R = [];
          addEventListener('error', function (ev) {
            if (!ev || !ev.message || ev.message.indexOf('REPORTED_BOOM') < 0) { return; }
            R.push('caught=yes');
            R.push('file=' + (ev.filename ? 'yes' : 'no'));
            R.push('line=' + (ev.lineno > 0 ? 'yes' : 'no'));
          });
          try { throw new TypeError('REPORTED_BOOM: the page reported this itself'); }
          catch (e) { reportError(e); }
          // (6) A throw from a QUEUED task — the funnel every deferred framework failure takes.
          setTimeout(function () { throw new RangeError('DEFERRED_BOOM: a timer callback threw'); }, 0);
          setTimeout(function () {
            document.getElementById('out').textContent = R.length ? R.join(' ') : 'never-caught';
          }, 1);
        </script>
        <script>
          (async function(){ throw new TypeError('ASYNC_BOOM: this is where framework errors die'); })();
        </script>
        <script>
          throw new ReferenceError('SYNC_BOOM: a plain script error');
        </script>
      </body></html>"#;

    let page = manuk_page::Page::load(html, "https://silent.test/", &fonts, 800.0);

    // (4) The page renders anyway. Surfacing an error is not the same as giving up on the document —
    //     a browser that blanks the page on a script error is not a safer browser, it is a worse one.
    let root = page.dom().root();
    let h = manuk_css::query_selector_all(page.dom(), root, "#headline");
    assert!(
        !h.is_empty(),
        "the document must still render when a script throws"
    );
    assert_eq!(page.dom().text_content(h[0]), "The document still renders");

    // (1) The synchronous error was SAID OUT LOUD.
    assert!(
        cap.saw("SYNC_BOOM") || cap.saw("ReferenceError"),
        "G_SILENT_FAIL: a script threw and the engine said NOTHING.\n  captured:\n  {}",
        cap.dump()
    );

    // (2) THE one. An unhandled rejection inside an async function is exactly how "React renders
    //     nothing, throws nothing" happened — the error was real, truthful, and inaudible.
    assert!(
        cap.saw("ASYNC_BOOM") || cap.saw("UNHANDLED PROMISE REJECTION"),
        "G_SILENT_FAIL: an async function rejected and the engine said NOTHING.\n  \
         Every modern framework renders inside an async function, so this is where their failures go \
         to die. A browser that fails silently sends you looking in the wrong codebase — for several \
         ticks this exact hole had 'React renders nothing' recorded as a REACT bug.\n  captured:\n  {}",
        cap.dump()
    );

    // ── (5) `reportError()` MUST REPORT. ────────────────────────────────────────────────────────
    //
    // `globalThis.reportError(e)` is the WHATWG API whose entire definition is *"report the
    // exception to the global scope"*. It was a one-line console shim: it never fired `onerror` and
    // never dispatched `error`, so **the page's own handler — every telemetry client, every error
    // boundary, every retry path on the web — did not run.** A probe pinned it `WORKS` at tick 599
    // because `typeof reportError` said `function`: PRESENCE answered for BEHAVIOUR, which is the
    // false-YES this project has now paid for five times.
    let out = manuk_css::query_selector_all(page.dom(), root, "#out");
    let got = if out.is_empty() {
        String::from("(no #out)")
    } else {
        page.dom().text_content(out[0])
    };
    println!("REPORT_ERROR: {got}");
    for (claim, why) in [
        (
            "caught=yes",
            "`reportError(e)` did not report — the page's own `error` listener never ran. That \
             listener is Sentry, it is React's error boundary, and it is every retry path on the web.",
        ),
        (
            "file=yes",
            "the reported error carried no `filename`. Tick 666 lifted `fileName`/`lineNumber` off \
             an exception on the NATIVE boundary and left the JS paths stringifying the message \
             alone — one rule, three implementations. On minified code a message with no address is \
             a sentence about every variable on the page.",
        ),
        ("line=yes", "the reported error carried no `lineno`."),
    ] {
        assert!(got.contains(claim), "G_SILENT_FAIL: {why}\n  got: {got}");
    }

    // ── (6) A THROW FROM A QUEUED TASK IS SAID OUT LOUD, WITH ITS ADDRESS. ──────────────────────
    //
    // `__reportError` is the single funnel every DEFERRED throw on the app web goes through — a
    // `setTimeout` callback, a microtask, a `MutationObserver` callback, an inline `on*` handler, an
    // event listener. It pushed the error into `globalThis.__errors` and stopped, and that array is
    // read by exactly one caller in the tree (`manuk-wpt`'s diag JSON). So on a **real site** every
    // one of those throws was *stored* and never *said* — this gate's own subject, one step past
    // where it was found. It is also the storage the unhandled-error HARVESTER wants
    // (meta-instrument #1), and a harvester cannot harvest what is never emitted.
    let deferred = cap.line_with("DEFERRED_BOOM").unwrap_or_else(|| {
        panic!(
            "G_SILENT_FAIL: a `setTimeout` callback threw and the engine said NOTHING — the error \
             went into `globalThis.__errors`, which nothing on the render path ever reads.\n  \
             captured:\n  {}",
            cap.dump()
        )
    });
    // ...and the SAME line named the place, with a stack. Asserting the message and the address
    // separately would pass on two unrelated log lines, which is the accounting error this project
    // keeps catching. The frame name can only come from the engine's own `Error.stack`, so it cannot
    // be satisfied by formatting a guess.
    //
    // The file names the DOCUMENT and this script's own ordinal — `https://silent.test/ inline#N`
    // (tick 679). It was the constant `inline.js` for every inline script on every page, which this
    // gate recorded as its honest remainder rather than asserting away: the line, the column and the
    // stack frames were real and specific, and the FILE named a line in an unnamed one of forty.
    // `inline.js:1:155` is not an address, it is an address-shaped string.
    //
    // Line and column stay SCRIPT-relative — SpiderMonkey compiles each inline script as its own
    // source, and claiming document-relative numbers we do not compute would be a worse lie than the
    // one being fixed. See `g_script_error_has_a_location`, which says so explicitly.
    assert!(
        deferred.contains(" at ") && deferred.contains("silent.test") && deferred.contains(":13:"),
        "G_SILENT_FAIL: the deferred throw was logged WITHOUT its address. An error that is \
         REPORTED but not ATTRIBUTABLE is a status, not a finding — an anonymous `TypeError` sat in \
         this project's log for ten ticks for exactly this reason.\n  line: {deferred}"
    );
    assert!(
        deferred.contains("setTimeout"),
        "G_SILENT_FAIL: the deferred throw carries no stack frame, so the report can say WHERE but \
         not THROUGH WHAT.\n  line: {deferred}"
    );
    // ...and the name DISTINGUISHES this script from the others in the same document. A source name
    // shared by every inline script is not an address; the ordinal is what makes it one.
    assert!(
        deferred.contains("inline#"),
        "G_SILENT_FAIL: the reported file does not identify WHICH inline script threw. Every inline \
         `<script>` on every page compiled under the one constant `inline.js`, so a line number in a \
         page with forty of them located nothing.\n  line: {deferred}"
    );
    println!("DEFERRED: {deferred}");
}

#[test]
fn a_dead_subresource_is_never_swallowed() {
    let (cap, _g) = capture();
    let fonts = FontContext::new();

    // A stylesheet that cannot load must not be quietly treated as an empty one. The page will render
    // unstyled, which is correct and legible — but if nobody is *told*, "the site looks wrong" becomes
    // an unfalsifiable complaint, and the next person to look goes hunting in the cascade.
    let html = r#"<!doctype html><html><head>
        <link rel="stylesheet" href="http://127.0.0.1:1/never.css">
      </head><body><p id="body">text</p></body></html>"#;

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let page = rt.block_on(async {
        let mut p = manuk_page::Page::load_async(html, "https://silent.test/", &fonts, 800.0).await;
        p.finish_loading(&fonts, 800.0).await;
        p
    });

    let root = page.dom().root();
    assert!(
        !manuk_css::query_selector_all(page.dom(), root, "#body").is_empty(),
        "the document must still render when a stylesheet dies"
    );
    assert!(
        cap.saw("STYLESHEET FAILED") || cap.saw("stylesheet"),
        "G_SILENT_FAIL: a stylesheet failed to load and the engine said NOTHING. The page renders \
         unstyled and nobody knows why.\n  captured:\n  {}",
        cap.dump()
    );
    // And the failure must be COUNTABLE, not only logged: a measurement (the differential oracle)
    // has to be able to ask "did I actually style this page?" and refuse to score a UA-fallback
    // layout as the engine's own. Tick 383: crawl-load fetch starvation booked hundreds of phantom
    // divergences per site precisely because nothing machine-readable said the sheet never came.
    assert_eq!(
        page.failed_stylesheet_fetches(),
        1,
        "G_SILENT_FAIL: the dead stylesheet must be counted (got {})",
        page.failed_stylesheet_fetches()
    );
}
