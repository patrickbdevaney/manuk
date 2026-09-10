//! **G_WINDOW_HANDLER_THROWS_ARE_REPORTED — the fourth implementation, and the only silent one.**
//!
//! `__fireWindowEvent` — the dispatcher behind every `window.addEventListener(…)` and every
//! `window.onX` property handler — invoked handlers inside a **bare `catch (e) {}`**. A throw from
//! any of them was discarded whole: no `window.onerror`, no `error` event, no `__errors` entry, no
//! log line. The sibling dispatcher for element events (`__dispatchEvent`) has routed both of its
//! equivalents through `__reportError` for ticks; this one never did.
//!
//! ⚠ **THE CLASS IS NOT NARROW.** It covers `load`, `resize`, `popstate`, `message` (i.e. every
//! `postMessage` receiver), `unhandledrejection` and `hashchange` — which is where a page's boot
//! code and its cross-origin plumbing actually live. Every error-reporting SDK on the web (Sentry,
//! Bugsnag, Rollbar) and every app's own fallback UI installs exactly `window.onerror`, and for this
//! whole class of throw they saw a page that was working fine.
//!
//! Found while gating `Page::boot_errors`: a throwing `setTimeout` callback and a throwing
//! `document.addEventListener` handler both reached the harvest and a throwing
//! `window.addEventListener('load')` handler did not.
//!
//! ## Arbitrated against headless Chrome, on this exact fixture
//!
//! ```text
//!   Chrome   onerror:Uncaught TypeError: winload-th | errevt:… | second-listener-ran
//!   before   (nothing at all — the throw was swallowed)
//!   after    errevt:winload-threw | onerror:winload-threw | second-listener-ran
//! ```
//!
//! Two deliberate, named divergences remain, and neither is what this gate is about:
//!
//! ⚠ **THE `"Uncaught "` PREFIX IS CHROME'S, NOT THE SPEC'S** — Firefox reports the bare message.
//! Pinning Chrome's exact wording would assert a thing the standard does not say, so this gate
//! asserts the message CONTAINS the thrown text and nothing about its framing.
//!
//! ⚠ **RESIDUE: ORDER.** Chrome runs the `onerror` PROPERTY handler before the
//! `addEventListener('error')` one, because an event-handler attribute is a listener registered at
//! the position it was assigned; `__fireWindowEvent` runs its listener list first and the property
//! handler last, for every window event. Real but separate, and fixing it means giving the two
//! lists one order.
//!
//! ## What this gate pins
//!
//! 1. **A THROWING WINDOW LISTENER IS REPORTED** — `onerror` and the `error` event both see it.
//! 2. **THE MESSAGE IS A STRING, NOT THE EVENT.** `window.onerror` is an `ErrorEventHandler`
//!    (HTML §8.1.7.2.1): `(message, source, lineno, colno, error)`. Handing it the `ErrorEvent`
//!    makes every handler that does the ordinary `String(message)` report `[object Object]` —
//!    which is what this engine did, *on top of* firing the handler a second time.
//! 3. **EXACTLY ONCE.** `__reportError` called `globalThis.onerror` directly AND dispatched an
//!    `error` event whose dispatcher calls it again. Chrome fires it once; a double fire makes
//!    every SDK on the page double-count.
//! 4. **A THROWING `onerror` DOES NOT RECURSE.** Reporting dispatches, dispatch can throw, and as
//!    of this tick that throw comes back here — HTML's *"in error reporting mode"* flag is what
//!    keeps that finite. Chrome-measured: `onerror-calls=1`.
//! 5. **THE LISTENERS AFTER THE THROWING ONE STILL RUN.** The rule `run_one_script` protects one
//!    layer down, at the dispatch layer.
//!
//! Mutations that must turn this red:
//!   1. restore the bare `catch (e) {}` in either arm     -> `calls=0`, nothing reported
//!   2. pass `ev` instead of the unpacked args to `onerror` -> `msg=[object Object]`
//!   3. restore the direct `globalThis.onerror(...)` call  -> `calls=2`
//!   4. drop the `__inErrorReport` flag                    -> the recursion row hangs or blows the stack
//!   5. `break` out of the listener loop on a throw        -> `after=absent`

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><meta charset=utf-8></head><body><div id="out">-</div>
<script>
  var calls = 0, msg = '(never)', typ = '(never)', evt = 0, after = 'absent';
  var recur = 0;
  window.onerror = function (m, src, line, col, err) {
    calls++; msg = String(m); typ = typeof m;
    // (4) THE RECURSION ARM. Reporting this throw dispatches an `error` event, whose handler is
    //     this function — without HTML's error-reporting-mode flag that is unbounded.
    if (recur === 0) { recur = 1; throw new Error('onerror-itself-threw'); }
    return false;
  };
  window.addEventListener('error', function (e) { evt++; });
  window.addEventListener('load', function () { throw new TypeError('winload-threw'); });
  window.addEventListener('load', function () { after = 'ran'; });
  window.addEventListener('load', function () {
    document.getElementById('out').textContent =
      'calls=' + calls + ' typ=' + typ + ' evt=' + evt + ' after=' + after +
      ' msg=' + (msg.indexOf('winload-threw') >= 0 ? 'has-text' : msg);
  });
</script></body></html>"##;

#[test]
fn a_throwing_window_handler_is_reported_exactly_once_and_does_not_recurse() {
    let fonts = FontContext::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let page = rt.block_on(async {
        let mut p = manuk_page::Page::load_async(HTML, "https://winerr.test/", &fonts, 800.0).await;
        p.finish_loading(&fonts, 800.0).await;
        p
    });
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("WINDOW ERROR REPORTING: {got}");

    // ── VACUITY. The reporting listener has to have RUN, or every field below is its initial value
    //    and the gate passes on a page where `load` never fired at all.
    assert!(
        got.contains("after=ran"),
        "VACUOUS: the listeners after the throwing one did not run, so this gate is reading a page \
         whose dispatch stopped — got {got:?}"
    );

    // Chrome headless, every field this engine can honestly claim.
    let want = "calls=1 typ=string evt=1 after=ran msg=has-text";
    assert_eq!(
        got, want,
        "\n  a throwing window handler must be reported ONCE, with a STRING message, to both \
         `onerror` and the `error` event, without recursing\n  want: {want}\n  got:  {got}"
    );

    // …and the same throw reaches the harvest, which is the consumer this whole path exists for.
    let harvest: Vec<String> = page
        .boot_errors()
        .iter()
        .map(|e| format!("{}/{}", e.phase, e.message.lines().next().unwrap_or("")))
        .collect();
    assert!(
        harvest.iter().any(|h| h.contains("winload-threw")),
        "a reported window-handler throw must also reach `Page::boot_errors` — reporting it to the \
         page and not to the host is the same silence one consumer over: {harvest:?}"
    );
}
