//! **G_GLOBALS — a missing constructor is a THROWN EXCEPTION, and its blast radius is whatever was
//! rendering at the time.**
//!
//! This is the most expensive shape of gap this project has, and it keeps recurring:
//!
//!   * `canvas.getContext` was used by **3%** of sites and **broke 100% of them** — `ctx.fillRect(…)` on
//!     the next line is a `TypeError`, and a charting library that boots on load takes the whole bundle
//!     with it.
//!   * `WebSocket` was missing and took an entire **news front page** with it: aljazeera.com's 2,591
//!     server-rendered elements became **141**. A live-blog client constructed one at boot, React's
//!     render threw, its error boundary showed a skeleton, and the article was gone.
//!
//! Fixing that one revealed `Blob`. Fixing `Blob` revealed `FileList`. **Each was a different library's
//! first line**, and each took down whatever was rendering — because a page does not get to run its
//! fallback path if the *check* for the fallback throws.
//!
//! So the rule this gate exists to hold: **construct successfully, and answer honestly.** A blank canvas,
//! an unopened socket, an empty `Blob` are all survivable — every library on the web is written to
//! survive them, because real browsers produce exactly those behind captive portals, in private windows,
//! and with permissions denied. **A `ReferenceError` is survivable by nothing.**
//!
//! What this asserts:
//!
//!   1. Every name below **exists**. Referencing a missing one is a `ReferenceError`, not a `false`.
//!   2. The ones that must **degrade honestly** do: `WebSocket` reports that it cannot connect rather
//!      than pretending to; `canvas.getContext('webgl')` returns `null`, which is the spec's "cannot".
//!   3. **They do not lie.** A `WebSocket` that reported `OPEN` would be worse than one that throws.

use manuk_text::FontContext;

/// Every global a real bundle references. The list is long on purpose: the aljazeera wipe was found one
/// `ReferenceError` at a time, and the lesson is that they come in a long tail, not ones and twos.
const HTML: &str = r#"<!doctype html><html><body><div id="out">-</div><script>
    var NAMES = [
      // constructed at boot by real libraries
      'WebSocket','EventSource','BroadcastChannel','Worker','SharedWorker',
      'Blob','File','FileReader','FileList','DataTransfer',
      'Image','Audio','Option','DOMParser','XMLSerializer','DOMRect','DOMRectReadOnly',
      'PerformanceObserver','MutationObserver','IntersectionObserver','ResizeObserver',
      'FormData','URLSearchParams','URL','Headers','Request','Response','AbortController',
      'TextEncoder','TextDecoder','MessageChannel','CSSStyleSheet','Notification',
      // referenced by name in instanceof / type checks
      'NodeList','HTMLCollection','ShadowRoot','StyleSheet','ProgressEvent','StorageEvent',
      'CloseEvent','SubmitEvent','PointerEvent','TouchEvent','WheelEvent','DragEvent',
      'ClipboardEvent','HashChangeEvent','AnimationEvent','TransitionEvent',
      'HTMLFormElement','HTMLAnchorElement','HTMLButtonElement','HTMLIFrameElement',
      'HTMLVideoElement','HTMLTemplateElement','MediaQueryList','DOMTokenList',
      // window surface
      'getSelection','requestAnimationFrame','requestIdleCallback','matchMedia','structuredClone'
    ];
    var missing = NAMES.filter(function (n) { return typeof globalThis[n] === 'undefined'; });

    var r = [];
    r.push('missing:' + (missing.length === 0 ? 'none' : missing.join(',')));

    // window must be an EventTarget. `window.dispatchEvent(new Event('resize'))` is how a router tells
    // the app it navigated — and it was a TypeError, with a whole listener registry sitting behind it.
    r.push('winDispatch:' + (typeof window.dispatchEvent === 'function'));
    var heard = false;
    window.addEventListener('ping', function () { heard = true; });
    window.dispatchEvent(new Event('ping'));
    r.push('winRoundTrip:' + heard);

    // Document properties that a very large amount of ordinary code reads — and `undefined.split(...)`
    // is a TypeError that takes the rest of the bundle with it.
    document.title = 'set by script';
    r.push('title:' + (document.title === 'set by script'));
    r.push('referrer:' + (typeof document.referrer === 'string'));
    r.push('charset:' + (document.characterSet === 'UTF-8'));
    r.push('currentScript:' + (document.currentScript === null));   // null, NOT undefined
    r.push('vendor:' + (typeof navigator.vendor === 'string'));

    // ── Honesty. These must NOT pretend to work.
    var ws = new WebSocket('wss://nope.test/');
    r.push('wsConstructs:' + (ws.readyState === 0));         // CONNECTING, not OPEN
    r.push('wsNotOpen:' + (ws.readyState !== 1));            // a socket that claimed OPEN would be worse
    var cv = document.createElement('canvas');
    r.push('webglNull:' + (cv.getContext('webgl') === null)); // the spec's "cannot"
    r.push('canvas2d:' + (typeof cv.getContext('2d').fillRect === 'function'));

    document.getElementById('out').textContent = r.join(' ');
  </script></body></html>"#;

/// One test, on purpose — two SpiderMonkey contexts in one binary tear down messily and segfault
/// nondeterministically, and a flaky gate gets ignored. (See `g_defer`.)
#[test]
fn every_global_a_real_bundle_references_exists_and_answers_honestly() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://globals.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    assert!(
        got.contains("missing:none"),
        "G_GLOBALS: some globals a real bundle references DO NOT EXIST.\n  {got}\n\n  \
         A referenced name that does not exist is a ReferenceError, not a `false` — and its blast radius \
         is whatever was rendering at the time. WebSocket's absence took an entire news front page down: \
         2,591 server-rendered elements became 141, because a live-blog client constructed one at boot \
         and React's error boundary showed a skeleton where the article had been."
    );

    for claim in [
        "winDispatch:true",  // window is an EventTarget
        "winRoundTrip:true", // ...and dispatch actually reaches its listeners
        "title:true",        // document.title is readable AND writable
        "referrer:true",
        "charset:true",
        "currentScript:true", // null, not undefined — libraries guard against null, not undefined
        "vendor:true",
        "wsConstructs:true", // it constructs
        "wsNotOpen:true",    // ...and does NOT claim to be connected
        "webglNull:true",    // getContext('webgl') === null is the spec's "cannot"
        "canvas2d:true",     // a real 2D context, whose drawing ops are no-ops
    ] {
        assert!(
            got.contains(claim),
            "G_GLOBALS: expected {claim} in {got:?}\n  \
             Construct successfully, and answer HONESTLY. A blank canvas and an unopened socket are \
             survivable — every library on the web is written to survive them. A socket that LIED and \
             claimed OPEN would be worse than one that throws."
        );
    }
}
