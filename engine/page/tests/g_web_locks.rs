//! **G_WEB_LOCKS — `navigator.locks` (the Web Locks API).**
//!
//! A page coordinates EXCLUSIVE access to a named resource: `navigator.locks.request(name, cb)` runs
//! `cb` only while it holds the lock, and a second request for the SAME name waits until the first
//! callback settles. Auth SDKs (AWS/GCP/Firebase) use exactly this so two concurrent requests do not
//! both refresh a token. It was ABSENT, so `navigator.locks.request(...)` threw on `undefined`.
//!
//! The teeth prove REAL serialisation, which an inert stub (that runs both callbacks at once) cannot:
//!   * `serialised` — the second holder does NOT start until the first has ended (order is
//!     `a-start, a-end, b-start`, never interleaved).
//!   * `value` — `request` resolves with the callback's return value.
//!   * `if-available` — with `{ifAvailable:true}` on a held lock, the callback runs with a `null` grant
//!     instead of waiting.
//!
//! Proven RED: delete the `navigator.locks` block and `present` reads `undefined` while the first call
//! throws.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<script>
var r = [];
function push(s) { r.push(s); }
function finish() { document.getElementById('out').textContent = r.join(' '); }

try {
  push('present:' + (typeof navigator.locks === 'object' && navigator.locks !== null &&
                     typeof navigator.locks.request === 'function'));

  var order = [];
  // `d` lets the FIRST holder hang onto the lock until we deliberately release it.
  var d = Promise.withResolvers();

  var p1 = navigator.locks.request('res', function () {
    order.push('a-start');
    return d.promise.then(function () { order.push('a-end'); return 42; });
  });

  var p2 = navigator.locks.request('res', function () {
    order.push('b-start');   // must NOT run until a-end
    return 'second';
  });

  // ifAvailable on the (currently held) lock → callback gets a null grant immediately, no wait.
  var p3 = navigator.locks.request('res', { ifAvailable: true }, function (lock) {
    push('if-available:' + (lock === null));
  });

  // At this microtask point only 'a-start' should have run; b is queued behind the held lock.
  Promise.resolve().then(function () {
    push('queued:' + (order.join(',') === 'a-start'));
    d.resolve();  // release the first lock → a-end, then b-start
  });

  Promise.all([p1, p2, p3]).then(function (vals) {
    push('serialised:' + (order.join(',') === 'a-start,a-end,b-start'));
    push('value:' + (vals[0] === 42 && vals[1] === 'second'));
    finish();
  }, function (e) { push('ALL-THREW:' + e); finish(); });
} catch (e) {
  push('THREW:' + e);
  finish();
}
</script></body></html>"##;

#[test]
fn navigator_locks_serialises_named_access() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://locks.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("WEB-LOCKS RESULT: {got}");

    for claim in [
        "present:true",
        "if-available:true", // held lock + ifAvailable -> null grant, no wait
        "queued:true",       // only a-start ran while the lock was held
        "serialised:true",   // b-start ran strictly after a-end
        "value:true",        // request resolves with the callback's return value
    ] {
        assert!(
            got.contains(claim),
            "G_WEB_LOCKS: expected `{claim}`\n  got: {got}\n\n  \
             `navigator.locks.request` must SERIALISE callbacks for the same name (the second waits \
             for the first to settle) and resolve with the callback's value; `ifAvailable` skips the \
             wait with a null grant. A stub that runs both at once fails `serialised`."
        );
    }
}
