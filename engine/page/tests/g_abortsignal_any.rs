//! **G_ABORTSIGNAL_ANY — `AbortSignal.any(signals)`, the compound-cancel combinator.**
//!
//! The canonical use is one request that must cancel on EITHER a user action OR a timeout:
//! `fetch(url, { signal: AbortSignal.any([userController.signal, AbortSignal.timeout(5000)]) })`.
//! `AbortSignal.timeout` was already native; `any` was missing, so that pattern threw
//! `AbortSignal.any is not a function` and the request could not be given a compound cancel.
//!
//! Teeth a stub cannot grow — the returned signal must be a REAL AbortSignal, not an inert look-alike:
//!   * `none` — with no input aborted, the combined signal is NOT aborted.
//!   * `propagates` — aborting one input aborts the combined signal AND fires its `abort` event.
//!   * `reason` — the combined signal carries the aborting input's reason.
//!   * `already` — an input that is ALREADY aborted makes the combined signal aborted immediately.
//!
//! Proven RED: gate out the shim and `present` reads `undefined` while the first call throws.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<script>
var r = [];
function push(s) { r.push(s); }
function finish() { document.getElementById('out').textContent = r.join(' '); }

try {
  push('present:' + (typeof AbortSignal.any === 'function'));

  var a = new AbortController();
  var b = new AbortController();
  var combined = AbortSignal.any([a.signal, b.signal]);
  push('none:' + (combined.aborted === false));

  var fired = false;
  combined.addEventListener('abort', function () { fired = true; });

  // abort the SECOND input — the combined signal must follow, with b's reason.
  b.abort('user-cancelled');
  push('propagates:' + (combined.aborted === true && fired === true));
  push('reason:' + (combined.reason === 'user-cancelled'));

  // an already-aborted input makes the result aborted immediately.
  var c = new AbortController();
  c.abort('was-already');
  var combined2 = AbortSignal.any([c.signal]);
  push('already:' + (combined2.aborted === true && combined2.reason === 'was-already'));

  // the canonical case: a TIMEOUT source must also propagate (proves timeout FIRES its event, not
  // merely flips `aborted`). A 0ms timeout fires on the next turn; assert from its own listener.
  var t = AbortSignal.any([AbortSignal.timeout(0)]);
  t.addEventListener('abort', function () {
    push('timeout-prop:' + (t.aborted === true && t.reason && t.reason.name === 'TimeoutError'));
    finish();
  });
  finish();
} catch (e) {
  push('THREW:' + e);
  finish();
}
</script></body></html>"##;

#[test]
fn abort_signal_any_combines_cancellation() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://abort.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("ABORT-ANY RESULT: {got}");

    for claim in [
        "present:true",
        "none:true",         // combined not aborted until an input is
        "propagates:true",   // an input aborting aborts the combined signal + fires its event
        "reason:true",       // and carries that input's reason
        "already:true",      // an already-aborted input aborts the result immediately
        "timeout-prop:true", // a timeout source propagates (timeout FIRES its abort event)
    ] {
        assert!(
            got.contains(claim),
            "G_ABORTSIGNAL_ANY: expected `{claim}`\n  got: {got}\n\n  \
             `AbortSignal.any` must return a REAL AbortSignal that aborts (firing its `abort` event, \
             with the source reason) as soon as any input aborts — immediately if one is already \
             aborted. An inert stub fails `propagates`/`already`."
        );
    }
}
