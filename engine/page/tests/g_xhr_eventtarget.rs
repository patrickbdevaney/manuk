//! **G_XHR_EVENTTARGET — `XMLHttpRequest` had the legacy `on*` half of its API and none of the
//! EventTarget half, so the idiom essentially all modern code uses threw.**
//!
//! `xhr.addEventListener`, `removeEventListener` and `dispatchEvent` were all `undefined`, and
//! `xhr instanceof EventTarget` was false. Only `xhr.onload = fn` existed. Calling a property that is
//! `undefined` is a **`TypeError` that kills the calling frame** — so a request set up the modern way
//! never even got as far as `send()`, and whatever else that frame was doing died with it.
//!
//! Measured across the 20 HEAD sites of `docs/bench/corpus-v2.tsv` (each site's HTML plus up to 12 of
//! its external bundles):
//!
//! ```text
//!   use `new XMLHttpRequest`              8 of 16 sites
//!   addEventListener within 500ch of one  4 of 16 sites
//!   XHR-specific event names              readystatechange 9 · progress 4 · loadend 3 · timeout 2
//! ```
//!
//! It is also `www.welt.de`'s second failure, the rung directly behind t612's `innerText`:
//! `TypeError: a.addEventListener is not a function`.
//!
//! **And the six open-coded dispatch sites had already drifted.** `loadend` was fired by the
//! streaming delivery path and NOT by the buffered `__deliverXhr`, so whether `onloadend` ran was a
//! function of whether the response happened to arrive in chunks. One rule, two implementations, one
//! of them wrong — so every event now goes through a single `__xhrFire`, and this gate covers both
//! paths precisely because they disagreed.

use manuk_page::FetchStreamEvent;
use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body><div id="out">-</div>
<script>
  var R = [];
  var x = new XMLHttpRequest();

  // ── The surface exists AT ALL. This is the assertion welt.de fails on.
  (function () {
    'use strict';
    try {
      x.addEventListener('load', function () {});
      R.push('ael:ok');
    } catch (e) { R.push('ael:THREW'); }
  })();
  R.push('rel:' + (typeof x.removeEventListener));
  R.push('dispatch:' + (typeof x.dispatchEvent));
  // fired by the streaming path but never declared — `'onprogress' in xhr` used to answer NO
  R.push('hasOnProgress:' + ('onprogress' in x));

  // ── Ordering, and that BOTH halves run. The `on*` handler and the listeners must both fire, and a
  // page that uses both must not have one silently win.
  var order = [];
  var y = new XMLHttpRequest();
  y.onload = function () { order.push('on'); };
  y.addEventListener('load', function () { order.push('l1'); });
  y.addEventListener('load', function () { order.push('l2'); });

  // ── Spec details that separate a real EventTarget from a callback list.
  var dupes = 0;
  function dup() { dupes++; }
  y.addEventListener('readystatechange', dup);
  y.addEventListener('readystatechange', dup);   // same callback twice → registered ONCE

  var onceCount = 0;
  y.addEventListener('loadend', function () { onceCount++; }, { once: true });

  var removedRan = false;
  function removeMe() { removedRan = true; }
  y.addEventListener('load', removeMe);
  y.removeEventListener('load', removeMe);

  // A throwing listener must not prevent the ones after it — one page's bug is not everyone's.
  var afterThrow = false;
  y.addEventListener('load', function () { throw new Error('boom'); });
  y.addEventListener('load', function () { afterThrow = true; });

  // The event object a listener receives is a real event, not undefined.
  var evType = 'none', evTarget = false;
  y.addEventListener('load', function (e) { evType = e && e.type; evTarget = (e && e.target) === y; });

  y.open('GET', '/buffered');
  y.send();

  globalThis.__report = function () {
    R.push('order:' + order.join(','));
    R.push('dupes:' + dupes);
    R.push('once:' + onceCount);
    R.push('removed:' + removedRan);
    R.push('afterThrow:' + afterThrow);
    R.push('evType:' + evType);
    R.push('evTarget:' + evTarget);
    // ⚠⚠⚠ THIS ENGINE'S INTERNAL SLOTS ARE NOT PAGE STATE (t892). `JSON.stringify(xhr)` used to
    // return `..."_ls":null,"_m":"GET","_u":"","_id":null,"_h":[],"_respHeaders":[]` where Chrome
    // returns `{}` — so any page that serialises, clones or `for...in`s an XHR (every error reporter
    // does at least one) saw our privates as its own fields. Found by the t891 rejection describer,
    // which printed sixteen rejected XHRs on beb88run.xyz with our slots inside them.
    var sx = new XMLHttpRequest();
    sx.open('GET', '/api/thing');
    R.push('privKeys:' + Object.keys(sx).filter(function (k) { return k.charAt(0) === '_'; }).length);
    R.push('slotsWork:' + (sx._m === 'GET' && sx._u === '/api/thing'));
    // ⚠ THE GUARD, and it is what keeps t891's OVER-CLAIM from being re-introduced: the METHODS are
    // on the prototype and a page's patch of them IS observed. t891's note called this "t884's
    // IndexedDB defect on another interface"; it is not, and asserting the true state here is what
    // stops a later tick from "fixing" something that already works.
    var origOpen = XMLHttpRequest.prototype.open, hits = 0;
    XMLHttpRequest.prototype.open = function () { hits++; return origOpen.apply(this, arguments); };
    try { new XMLHttpRequest().open('GET', '/y'); } catch (e) {}
    XMLHttpRequest.prototype.open = origOpen;
    R.push('protoPatch:' + hits);
    R.push('ownOpen:' + Object.prototype.hasOwnProperty.call(sx, 'open'));
    document.getElementById('out').textContent = R.join(' ');
  };
</script></body></html>"#;

/// The BUFFERED delivery path — `resolve_fetch`, the one that never fired `loadend`.
#[test]
fn xhr_is_an_event_target_on_the_buffered_path() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://x.test/", &fonts, 800.0);
    let reqs = page.take_fetches();
    assert_eq!(reqs.len(), 1, "the XHR was queued: {reqs:?}");
    page.resolve_fetch(reqs[0].0, 200, "BODY", &[], &fonts, 800.0);
    page.eval_for_test("__report()");

    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        // the surface itself — welt.de's `TypeError: a.addEventListener is not a function`
        "ael:ok",
        "rel:function",
        "dispatch:function",
        "hasOnProgress:true",
        // t892 — the internal slots are hidden, still work, and the METHODS were never the problem.
        "privKeys:0",
        "slotsWork:true",
        "protoPatch:1",
        "ownOpen:false",
        // BOTH halves run, `on*` first, listeners in registration order
        "order:on,l1,l2",
        // spec behaviour, not merely a callback array
        "dupes:1",
        "once:1",
        "removed:false",
        "afterThrow:true",
        "evType:load",
        "evTarget:true",
    ] {
        assert!(
            got.contains(claim),
            "G_XHR_EVENTTARGET: expected `{claim}`\n  got: {got}\n\n  \
             `xhr.addEventListener(...)` on an undefined method is a TypeError that kills the calling \
             frame — the request is never even sent, and so is everything else that frame was doing. \
             8 of 16 measured HEAD sites construct an XHR and 4 of 16 call addEventListener on one. \
             `once:1` and `dupes:1` are what make this an EventTarget rather than a list of callbacks; \
             `afterThrow:true` is why one page\'s broken handler must not take down the others."
        );
    }
}
