//! **G_IFRAME_LOAD_EVENT — an `<iframe>` whose document is installed must FIRE `load` on the
//! element.**
//!
//! ⚠⚠⚠ **THE FRAME LOADED AND NOTHING EVER SAID SO.** `contentDocument` has been populated and
//! readable since t512 — the frame really is there, and really is the right document — but no `load`
//! event was ever dispatched on the element. So `<iframe onload=…>`, `frame.addEventListener('load',
//! …)` and every `loadPromise` built on them simply never fired. Probed one property per file, so
//! the pass count names the failure rather than a message having to:
//!
//! ```text
//!                                          BEFORE   AFTER
//!   contentDocument non-null                PASS    PASS   <- CONTROL: the frame really is loaded
//!   contentDocument has the child's text    PASS    PASS   <- CONTROL: and is the right document
//!   INLINE onload= fired                    FAIL    PASS
//!   addEventListener('load') fired          FAIL    PASS
//!   the onload PROPERTY is a function       FAIL    FAIL   <- NOT this tick; see below
//! ```
//!
//! **The two CONTROL rows are what make this a missing EVENT rather than a missing frame.** Without
//! them the same symptom reads as "iframes don't work", which is false and would have sent the fix
//! into the loader.
//!
//! `<iframe onload>` is how an ad slot, an embed, a payment frame, an OAuth frame and every lazy
//! widget on the web announce readiness. It is also, in WPT, what gates `domparsing`'s four
//! `DOMParser-parseFromString-url*` files — 45 subtests each, all `harness=TIMEOUT` at ~120 ms
//! because their shared `loadPromise` never resolved.
//!
//! ## Measured — same-hour old-binary control
//!
//! ```text
//!   WPT dom          4004/7193  ->  6366/10503   +2362 passes, +3310 ATTEMPTED
//!   WPT html/dom    56440/59922 -> 56441/59922   +1
//!   WPT domparsing    149/1273  ->   149/1293    +20 attempted, pass FLAT (see below)
//! ```
//!
//! ⚠⚠ **THE ATTEMPTED TOTAL MOVING IS THE POINT, NOT AN ODDITY.** A testharness file emits subtests
//! as it gets through them; a file whose `loadPromise` never settles emits almost none. `dom` gained
//! 3,310 *attempted* subtests, which is the honest measure of how many tests could not previously
//! start.
//!
//! ⚠⚠ **AND `domparsing`'s PASS COUNT DID NOT MOVE, WHICH IS SAID PLAINLY BECAUSE IT WAS THE TARGET.**
//! This tick was taken to unblock that area (its −39 against the ratchet mark is what holds the whole
//! `WPT-AREAS.tsv` refresh). The four url files now RUN — +20 attempted — and then fail on their own
//! merits: they assert `doc.URL` / `doc.documentURI` / `doc.baseURI` on a `DOMParser`-created
//! document, which is a separate gap this tick did not touch and must not claim. **The area is still
//! below its mark and the refresh is still held.**
//!
//! ## NOT covered, named with its number rather than left looking handled
//!
//! - **The `onload` IDL attribute as a readable PROPERTY** (`typeof frame.onload === 'function'`
//!   after `<iframe onload="…">`). Still FAIL. That is the event-handler-IDL-attribute reflection
//!   surface — the handler *runs*, but the content attribute is not reflected into a property object
//!   — and it is a different mechanism from dispatching the event. Left, named, and asserted below in
//!   its failing state so it cannot be quietly assumed fixed.
//!
//! ## How this goes RED
//!
//! Delete either `self.fire_frame_load(node)` call in `engine/page/src/lib.rs` — the `srcdoc` one is
//! what this gate exercises (no network), the fetched one is what moved WPT.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<iframe id="a" onload="window.__inline=(window.__inline||0)+1; report();"></iframe>
<iframe id="b"></iframe>
<div id="out">-</div>
<script>
window.__inline = 0;
window.__addl = 0;
function report() {
  var a = document.getElementById('a');
  var d = a.contentDocument;
  var txt = (d && d.documentElement) ? d.documentElement.textContent : '';
  document.getElementById('out').textContent = [
    'contentDoc:' + (!!d),
    'childText:' + /child body text/.test(txt),
    'inline:' + (window.__inline | 0),
    'addl:' + (window.__addl | 0),
    'onloadProp:' + (typeof a.onload)
  ].join(' ');
}
var b = document.getElementById('b');
if (b && b.addEventListener) { b.addEventListener('load', function(){ window.__addl++; report(); }); }
</script>
</body></html>"##;

const CHILD: &str = "<!doctype html><html><body><p>child body text</p></body></html>";

#[test]
fn an_iframe_fires_load_on_its_element() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://frame.test/", &fonts, 800.0);
    let root = page.dom().root();
    let frame_a = manuk_css::query_selector_all(page.dom(), root, "#a")[0];
    let frame_b = manuk_css::query_selector_all(page.dom(), root, "#b")[0];

    // `render_iframe` is the ONE place a child document is installed — the fetched path, the
    // network-free `srcdoc`/`about:blank` path and every direct caller all arrive there, which is
    // why the event is dispatched there and why driving it directly here exercises the real wiring.
    page.render_iframe(frame_a, CHILD, "https://embed.test/a", &fonts, 0);
    page.render_iframe(frame_b, CHILD, "https://embed.test/b", &fonts, 0);

    let out = manuk_css::query_selector_all(page.dom(), page.dom().root(), "#out")[0];
    let got = page.dom().text_content(out);
    println!("G_IFRAME_LOAD_EVENT RESULT: {got}");

    // ⚠⚠ **THE "NO EVENT AT ALL" CASE HAS TO BE NAMED FIRST, OR THIS GATE MISDIAGNOSES ITS OWN
    //    SUBJECT.** `report()` is only ever called from the two handlers, so when the dispatch is
    //    severed `#out` is never written and stays `-`. The control assertion below would then fire
    //    and say *"the frame's document is not installed"* — which is FALSE, and is precisely the
    //    wrong reading that cost this defect three ticks of misattribution in the first place. Say
    //    the true thing instead.
    assert_ne!(
        got.trim(),
        "-",
        "G_IFRAME_LOAD_EVENT: `#out` was never written, so NEITHER handler ran — no `load` event was \
         dispatched on the iframe element at all. The frame's document IS installed (that is what \
         `render_iframe` just did); what is missing is the event that announces it."
    );

    // ── THE CONTROLS. If these fail, the frame did not load and nothing below is a test of an
    //    EVENT — it is a test of a loader, and the diagnosis would be wrong in exactly the way the
    //    original symptom was.
    for claim in ["contentDoc:true", "childText:true"] {
        assert!(
            got.contains(claim),
            "G_IFRAME_LOAD_EVENT: CONTROL `{claim}` failed — the frame's document is not installed, \
             so the load-event assertions below are measuring the wrong thing.\n  got: {got}"
        );
    }

    // ── THE SUBJECT. Both delivery paths, because an inline attribute handler and a registered
    //    listener are separate wiring and either could work without the other.
    assert!(
        got.contains("inline:1"),
        "G_IFRAME_LOAD_EVENT: `<iframe onload=...>` must fire exactly once when the frame's document \
         is installed. This is how every embed, ad slot, payment frame and lazy widget announces \
         readiness, and it fired ZERO times until t1167.\n  got: {got}"
    );
    assert!(
        got.contains("addl:1"),
        "G_IFRAME_LOAD_EVENT: `frame.addEventListener('load', ...)` must fire exactly once — the \
         same event, the other delivery path.\n  got: {got}"
    );

    // ── THE NAMED GAP, asserted in its FAILING state so it cannot be silently assumed fixed. When
    //    event-handler IDL attributes are reflected this flips to `function` and the assertion goes
    //    RED — which is the intended signal to update it, not a regression.
    assert!(
        got.contains("onloadProp:undefined"),
        "G_IFRAME_LOAD_EVENT: `frame.onload` is expected to still be UNDEFINED — reflecting the \
         event-handler content attribute into a property is a separate mechanism (t1167 dispatches \
         the event; it does not build the reflection). If this now reads `function`, that gap has \
         been closed elsewhere: update this assertion and the module note, do not delete them.\n  \
         got: {got}"
    );
}
