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
//! ## ⚠⚠⚠ t1346 — THIS GATE COUNTED DOCUMENT INSTALLS AND CALLED THE SECOND ONE A DOUBLE-FIRE
//!
//! It went red reading `inline:2 addl:2`, and the obvious diagnosis — *"`load` fires twice"* — was
//! wrong. The test body installs each frame's document **twice**: `Page::load` now gives a bare
//! `<iframe>` its initial `about:blank` document (which Chrome does, and which fires `load`), and
//! then the body calls `render_iframe` and installs a second one. Two installs, two events, each
//! correct. Measured by reading `#out` between the two calls:
//!
//! ```text
//!   after Page::load ONLY        contentDoc:true childText:false inline:1 addl:1
//!   after render_iframe(a)       contentDoc:true childText:true  inline:2 addl:1
//!   after render_iframe(b)       contentDoc:true childText:true  inline:2 addl:2
//! ```
//!
//! ⭐ **A COUNT IS NOT AN IDENTIFICATION.** `2` where `1` was expected reads as "fired twice" and
//! was in fact "loaded twice"; the reading that distinguishes them is one probe between the calls.
//! The gate is now staged over the two installs rather than asserting the total.
//!
//! Headless Chrome on the same shape, and it disagrees with us in exactly one place:
//!
//! ```text
//!                                          CHROME    ours
//!   <iframe onload> with NO src, fires       1         1     ✓
//!   …and it fires BEFORE the next script     yes       NO    ✗  named residue below
//!   a listener added AFTER that script       0         1     ✗  the same fact, observed
//!   typeof frame.onload                   function  function  ✓  CLOSED at t1346
//! ```
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

    let read = |page: &manuk_page::Page| -> String {
        let out = manuk_css::query_selector_all(page.dom(), page.dom().root(), "#out")[0];
        page.dom().text_content(out)
    };

    // ── STAGE 1 — the INITIAL `about:blank`, installed by `Page::load` itself for a bare `<iframe>`.
    //    Reading here rather than at the end is the whole repair: the totals at the end cannot tell
    //    "fired twice for one load" from "loaded twice".
    let stage1 = read(&page);
    println!("G_IFRAME_LOAD_EVENT STAGE1: {stage1}");

    // ⚠⚠ **THE "NO EVENT AT ALL" CASE HAS TO BE NAMED FIRST, OR THIS GATE MISDIAGNOSES ITS OWN
    //    SUBJECT.** `report()` is only ever called from the two handlers, so when the dispatch is
    //    severed `#out` is never written and stays `-`. The control assertion below would then fire
    //    and say *"the frame's document is not installed"* — which is FALSE, and is precisely the
    //    wrong reading that cost this defect three ticks of misattribution in the first place.
    assert_ne!(
        stage1.trim(),
        "-",
        "G_IFRAME_LOAD_EVENT: `#out` was never written, so NEITHER handler ran — no `load` event was \
         dispatched on the iframe element at all. The frame's document IS installed (that is what \
         `Page::load` just did); what is missing is the event that announces it."
    );
    assert!(
        stage1.contains("contentDoc:true"),
        "G_IFRAME_LOAD_EVENT: CONTROL `contentDoc:true` failed after `Page::load` — a bare \
         `<iframe>` must already hold its initial `about:blank` document, which is what Chrome \
         reports (`contentDocB:true`, measured) and what makes the event below an EVENT test.\n  \
         got: {stage1}"
    );
    assert!(
        stage1.contains("inline:1"),
        "G_IFRAME_LOAD_EVENT: `<iframe onload=...>` must fire EXACTLY ONCE for the initial \
         `about:blank` — headless-Chrome-measured at 1 on this exact shape. This is how every embed, \
         ad slot, payment frame and lazy widget announces readiness, and it fired ZERO times until \
         t1167. Reading 2 here would be a real double-fire; reading 2 at the END of this test is \
         not, and that confusion is what this staging exists to prevent.\n  got: {stage1}"
    );
    // ── ⚠ THE NAMED TIMING RESIDUE, pinned at OUR value with Chrome's beside it.
    assert!(
        stage1.contains("addl:1"),
        "G_IFRAME_LOAD_EVENT: `addl:1` is a KNOWN DIVERGENCE pinned at its current value. Chrome \
         gives 0: it fires the initial `about:blank` load DURING PARSING, before the trailing \
         `<script>` runs, so a listener registered by that script never sees it (measured — \
         `inlineSoFar:1` is already 1 when the script executes). We fire it after the script, so \
         the late listener does see it. If this reads 0, the ordering has been corrected — update \
         this assertion and the header table, do not delete them.\n  got: {stage1}"
    );

    // ── STAGE 2 — a SECOND document install on the same element, which is a re-navigation and MUST
    //    fire `load` again. `render_iframe` is the ONE place a child document is installed — the
    //    fetched path, the network-free `srcdoc`/`about:blank` path and every direct caller all
    //    arrive there, which is why driving it here exercises the real wiring.
    page.render_iframe(frame_a, CHILD, "https://embed.test/a", &fonts, 0);
    let stage2 = read(&page);
    println!("G_IFRAME_LOAD_EVENT STAGE2: {stage2}");
    assert!(
        stage2.contains("childText:true"),
        "G_IFRAME_LOAD_EVENT: CONTROL `childText:true` failed — the second install did not replace \
         the frame's document, so nothing below is a test of a re-navigation.\n  got: {stage2}"
    );
    assert!(
        stage2.contains("inline:2"),
        "G_IFRAME_LOAD_EVENT: a SECOND document install on the same `<iframe>` fires `load` again — \
         that is a re-navigation, and 2 here is correct rather than a double-fire. Reading 1 means \
         the event is being suppressed on re-navigation, which breaks every frame that changes its \
         `src` (an ad rotation, a wizard step, an OAuth hand-off).\n  got: {stage2}"
    );
    assert!(
        stage2.contains("addl:1"),
        "G_IFRAME_LOAD_EVENT: frame `b` has NOT been re-installed yet, so its listener must still \
         read 1. This is the row that proves the two frames' events are independent — a fix that \
         fired `load` on every frame whenever any frame loaded passes both `inline` rows and fails \
         this one.\n  got: {stage2}"
    );

    // ── STAGE 3 — the other delivery path, driven the same way. An inline attribute handler and a
    //    registered listener are separate wiring and either could work without the other.
    page.render_iframe(frame_b, CHILD, "https://embed.test/b", &fonts, 0);
    let stage3 = read(&page);
    println!("G_IFRAME_LOAD_EVENT STAGE3: {stage3}");
    assert!(
        stage3.contains("addl:2"),
        "G_IFRAME_LOAD_EVENT: `frame.addEventListener('load', ...)` must fire on a re-navigation — \
         the same event, the other delivery path.\n  got: {stage3}"
    );
    assert!(
        stage3.contains("inline:2"),
        "G_IFRAME_LOAD_EVENT: and frame `a` must NOT have fired a third time when `b` loaded — the \
         independence control, one axis over.\n  got: {stage3}"
    );

    // ── ⭐ CLOSED AT t1346: the event-handler CONTENT ATTRIBUTE is now the event-handler IDL
    //    PROPERTY. `<iframe onload="…">` makes `typeof frame.onload === "function"`, as Chrome
    //    reports. The handler used to be registered as an anonymous listener, which fired correctly
    //    and left the property `undefined` — so `var prev = el.onerror; el.onerror = wrap(prev)`,
    //    the chaining idiom every error-reporting snippet is built from, silently dropped the page's
    //    own handler.
    assert!(
        stage3.contains("onloadProp:function"),
        "G_IFRAME_LOAD_EVENT: `frame.onload` must reflect the content attribute as a FUNCTION — \
         Chrome-measured. `undefined` means the handler is being registered as an anonymous \
         listener instead of assigned to the IDL property: it fires, and it is unreadable and \
         unreplaceable, which is the half-installed shape that reads as working.\n  got: {stage3}"
    );
}
