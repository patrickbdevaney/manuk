//! **G_UNHANDLED_REJECTION — a page must be able to HEAR its own rejected promises.**
//!
//! HTML §8.1.7.5 (*"notify about rejected promises"*): when a promise is rejected with nothing
//! listening, the browser fires a **cancelable** `unhandledrejection` event at the global carrying
//! `reason` and `promise`, and only reports to the console if nobody called `preventDefault()`.
//!
//! We had the first half and none of the second. The native rejection tracker (`G_SILENT_FAIL`, which
//! stays green throughout — it asserts the *engine* says something) logged the rejection and fired
//! **nothing**, so `PromiseRejectionEvent` was `undefined` and neither `window.onunhandledrejection`
//! nor `addEventListener('unhandledrejection', ...)` ever ran. **That is where an application's error
//! reporting lives** — Sentry, Rollbar, Bugsnag, Datadog RUM and every hand-rolled
//! `window.onunhandledrejection = report` install exactly this listener, and on this engine all of
//! them were silently deaf. A page whose async boot fails had no way to tell anyone: not the user, not
//! its own telemetry, and not us.
//!
//! **And it is an instrument, which is why it was worth a tick.** Chasing cluster `C3833`
//! (`MISSING BOX: <div>`, the top cluster by hits) to its worst site, the entire application subtree
//! is deleted by a `SITE_CONTAINER.innerHTML = ""` and never rebuilt — 4917 elements — while the only
//! thing the engine emitted was an anonymous `Error: couldn't get user details` with no stack. *An
//! error reported but not attributable is a status, not a finding.* With the stack lifted it reads
//! `isLoggedInUser@https://wix.com/ inline#102:94:15`, which is an address.
//!
//! What this asserts, and each one is separately falsifiable:
//!
//!   1. `addEventListener('unhandledrejection', ...)` **fires**, with the real `reason` object —
//!      not a string, since a handler reads `e.reason.stack`.
//!   2. The `onunhandledrejection` **property** form fires too. It is the form the error reporters
//!      actually use, and the property and listener forms have drifted apart in this engine before.
//!   3. `preventDefault()` **suppresses the browser's own report**. Cancelable is the whole point:
//!      an app that owns the failure should not also get a console entry it did not ask for.
//!   4. The report the engine does emit carries the **STACK**, not just the message.

use std::sync::{Arc, Mutex};

use manuk_text::FontContext;
use tracing::subscriber::DefaultGuard;
use tracing_subscriber::layer::SubscriberExt;

/// Collects every log line the engine emits, so the gate can assert what a developer would see.
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
        self.0
            .lock()
            .unwrap()
            .push(format!("{} {}", event.metadata().level(), v.0));
    }
}

impl Capture {
    fn saw(&self, needle: &str) -> bool {
        self.0.lock().unwrap().iter().any(|l| l.contains(needle))
    }
    fn dump(&self) -> String {
        self.0.lock().unwrap().join("\n  ")
    }
}

/// ONE `#[test]` per JS gate: two SpiderMonkey runtimes in one process is a SIGSEGV, so every
/// assertion below shares a single page load and reports its result through the DOM.
#[test]
fn a_page_can_hear_its_own_rejected_promises() {
    let cap = Capture::default();
    let guard: DefaultGuard =
        tracing::subscriber::set_default(tracing_subscriber::registry().with(cap.clone()));
    let fonts = FontContext::new();

    // Two rejections, deliberately distinguishable:
    //   * `HEARD_*` — nobody cancels, so the listener AND the property handler must both see it and
    //     the engine must still report it (with a stack).
    //   * `CANCELLED_*` — the handler calls `preventDefault()`, so the engine must say NOTHING.
    // The `reason` is read as an OBJECT (`.message`), never stringified, because that is the
    // difference between a handler that can report a stack and one that cannot.
    let html = r#"<!doctype html><html><body>
        <div id="listener">-</div>
        <div id="property">-</div>
        <div id="ctor">-</div>
        <script>
          window.addEventListener('unhandledrejection', function (e) {
            if (!e || !e.reason || String(e.reason.message).indexOf('HEARD') < 0) { return; }
            document.getElementById('listener').textContent =
              'reason=' + e.reason.message +
              ' promise=' + (e.reason && e.promise !== undefined ? 'yes' : 'no') +
              ' cancelable=' + (e.cancelable ? 'yes' : 'no') +
              ' stack=' + (e.reason.stack ? 'yes' : 'no');
          });
          window.onunhandledrejection = function (e) {
            if (!e || !e.reason || String(e.reason.message).indexOf('HEARD') < 0) { return; }
            document.getElementById('property').textContent = 'fired=' + e.reason.message;
          };
          window.addEventListener('unhandledrejection', function (e) {
            if (!e || !e.reason || String(e.reason.message).indexOf('CANCELLED') < 0) { return; }
            e.preventDefault();
          });
          document.getElementById('ctor').textContent = 'type=' + (typeof PromiseRejectionEvent);
          function deepThrow() { return Promise.reject(new Error('HEARD_BOOM')); }
          deepThrow();
          Promise.reject(new Error('CANCELLED_BOOM'));
        </script>
      </body></html>"#;

    let page = manuk_page::Page::load(html, "https://rejection.test/", &fonts, 800.0);
    let dom = page.dom();
    let root = dom.root();
    let text = |sel: &str| -> String {
        let n = manuk_css::query_selector_all(dom, root, sel);
        assert!(!n.is_empty(), "the gate's own fixture lost {sel}");
        dom.text_content(n[0])
    };

    // (1) The listener form fires, and the event is the real shape — a `reason` OBJECT (so the
    //     handler can read `.stack`), a `promise`, and `cancelable: true`.
    assert_eq!(
        text("#listener"),
        "reason=HEARD_BOOM promise=yes cancelable=yes stack=yes",
        "G_UNHANDLED_REJECTION: addEventListener('unhandledrejection') did not fire with a real \
         PromiseRejectionEvent. This is the listener Sentry/Rollbar/Bugsnag install; without it a \
         page whose async boot fails cannot tell ANYONE.\n  captured:\n  {}",
        cap.dump()
    );

    // (2) The PROPERTY form. `window.onunhandledrejection = report` is what most reporters actually
    //     write, and in this engine the property and listener paths have drifted apart before.
    assert_eq!(
        text("#property"),
        "fired=HEARD_BOOM",
        "G_UNHANDLED_REJECTION: window.onunhandledrejection did not fire, though the \
         addEventListener form did — the two handler forms have drifted apart."
    );

    // (3) The type exists. A feature-detect (`typeof PromiseRejectionEvent === 'function'`) is how a
    //     reporter decides whether to install the handler at all.
    assert_eq!(text("#ctor"), "type=function");

    // (4) The engine's own report carries the ADDRESS, not just the message. `couldn't get user
    //     details` is a status; `isLoggedInUser@https://wix.com/ inline#102:94:15` is a finding.
    assert!(
        cap.saw("HEARD_BOOM") && cap.saw("deepThrow@"),
        "G_UNHANDLED_REJECTION: the rejection was reported WITHOUT its stack. An error reported but \
         not attributable is a status, not a finding.\n  captured:\n  {}",
        cap.dump()
    );

    // (5) `preventDefault()` means the browser does NOT also report it. Cancelable that cancels
    //     nothing is decoration — and this is the assertion that proves the event is wired to the
    //     report rather than fired beside it.
    assert!(
        !cap.saw("CANCELLED_BOOM"),
        "G_UNHANDLED_REJECTION: a handler called preventDefault() and the engine reported the \
         rejection anyway. Per HTML §8.1.7.5 the event is cancelable precisely so an app that owns \
         the failure does not also get a console entry it did not ask for.\n  captured:\n  {}",
        cap.dump()
    );

    drop(guard);
}
