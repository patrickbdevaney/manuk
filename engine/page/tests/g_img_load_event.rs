//! **G_IMG_LOAD_EVENT — `<img>` fired no `load` and no `error`, ever.**
//!
//! Measured before anything was built: a markup `<img src="data:…" onload="…">` reports
//! `complete === true` and `naturalWidth === 1` — so the bitmap is genuinely decoded and t1398's
//! accessors report it correctly — **while the handler never runs.** The engine did the work and
//! never told the page, which is the same shape as the `select` event (t1394) and the dynamic-script
//! path (t1397): three ticks in one arc where a completion signal simply did not exist.
//!
//! **Nothing fails loudly when this is missing; things WAIT.** Every lazy loader, every gallery and
//! carousel, every `loadImage()` promise and every "swap the placeholder when the real image is
//! ready" component is parked on this event.
//!
//! ```text
//!                                              chrome    before    after
//!   markup <img onload="…">                    load      NONE      load
//!   img.onload = … assigned by script          load      NONE      load
//!   img.addEventListener('load', …)            load      NONE      load
//!   a <div> listening for 'load'               none      none      none
//!   how many times it fires                    1         0         1
//!   img.complete inside the handler            true      —         true
//! ```
//!
//! ⭐ **`event.target` read INSIDE the handler and read AFTER dispatch are two different questions.**
//! A first draft stored the event and compared `e.target` in a later timeout: Chrome answered
//! `false` there and this engine `true`, which reads like a target bug and is not one — Chrome clears
//! the event's target once dispatch finishes. The row that means something is the one taken while the
//! handler is running, and on that both agree.
//!
//! ⚠ **Fired AFTER the script pass**, so a handler the page assigned in that very pass is already in
//! place; a markup `onload="…"` was registered by the parser, so firing late satisfies both orders.
//!
//! ⚠ Known gap, named rather than discovered later: a **script-set `src` does not re-fetch** in this
//! engine (the image worklist runs once per navigation and walks the DOM), so
//! `img.src = img.dataset.src` and the `new Image()` preload idiom still get nothing. That is a
//! fetch-worklist question, not an event one, and it is the next tick in this vein.
//!
//! ⚠ ONE `#[test]` in this binary: two SpiderMonkey contexts in one manuk-page binary tear each other
//! down and the second test reads the first one's empty output.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<img id="a" src="data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7" onload="window.inlineFired='load'">
<img id="b" src="data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7">
<img id="c" src="data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7">
<div id="d">not an image</div>
<div id="scroller" style="width:100px;height:100px;overflow:scroll">
  <div style="width:130px;height:10000vh"></div>
  <img id="far" src="data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7" loading="lazy" onload="window.farFired=1">
</div>
<img id="near" src="data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7" loading="lazy" onload="window.nearFired=1">
<div id="clipped" style="width:100px;height:100px;overflow:hidden">
  <img id="clip" src="data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7" loading="lazy" style="margin-top:-9000px" onload="window.clipFired=1">
</div>
<div id="out">-</div><script>
window.inlineFired = window.inlineFired || 'none';
window.farFired = window.farFired || 0;
window.nearFired = window.nearFired || 0;
window.clipFired = window.clipFired || 0;
var L=[];function k(n,v){L.push(n+':'+JSON.stringify(v));}
var bFired='none', bCount=0;
document.getElementById('b').onload=function(e){ bFired='load'; bCount++; window.bEvt=e;
  window.bTargetAtHandler = (e.target === document.getElementById('b'));
  window.bTypeAtHandler = e.type; };
var cFired='none';
document.getElementById('c').addEventListener('load', function(){ cFired='load'; });
var dFired='none';
document.getElementById('d').addEventListener('load', function(){ dFired='load'; });
setTimeout(function(){
  k('a_inlineAttrHandler', window.inlineFired);
  k('b_scriptAssignedOnload', bFired);
  k('c_addEventListener', cFired);
  k('d_nonImageGetsNothing', dFired);
  k('e_firesOnce', bCount);
  k('f_eventType', window.bTypeAtHandler === undefined ? 'none' : window.bTypeAtHandler);
  k('g_targetAtHandlerTime', window.bTargetAtHandler === undefined ? 'none' : window.bTargetAtHandler);
  k('h_completeAtHandler', document.getElementById('b').complete);
  k('i_lazyFarStillFires_dataUrl', window.farFired);
  k('j_lazyNearFires', window.nearFired);
  k('k_lazyClippedStillFires_dataUrl', window.clipFired);
  document.getElementById('out').textContent=L.join(' ');
}, 500);
</script></body></html>
"##;

/// One test in the binary — see the module note.
#[test]
fn an_img_tells_the_page_when_its_pixels_arrive() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://img-load.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("IMG-LOAD-EVENT RESULT: {got}");

    for claim in [
        // every registration form reaches the event
        "a_inlineAttrHandler:\"load\"",
        "b_scriptAssignedOnload:\"load\"",
        "c_addEventListener:\"load\"",
        // …and a non-image gets nothing (the queue also carries <canvas> sources)
        "d_nonImageGetsNothing:\"none\"",
        // once, not once per script round — the queue drains, it does not re-announce
        "e_firesOnce:1",
        "f_eventType:\"load\"",
        // the meaningful target question: asked while the handler is running
        "g_targetAtHandlerTime:true",
        // and the state the handler is there to observe is already true
        "h_completeAtHandler:true",
        // ⭐ the lazy rows assert that `loading="lazy"` does NOT defer a `data:` URL — there is no
        // fetch to defer, and Chrome agrees on all three. ⚠ The DISTANCE logic itself (both axes and
        // every clipping ancestor) cannot be exercised by a serverless fixture, because a data: image
        // is never deferred; its gate is WPT's `image-loading-lazy-in-scroller{,-horizontal}-far` and
        // the negative-margin case, all three of which go red without it (see the module note).
        "i_lazyFarStillFires_dataUrl:1",
        "j_lazyNearFires:1",
        "k_lazyClippedStillFires_dataUrl:1",
    ] {
        assert!(
            got.contains(claim),
            "G_IMG_LOAD_EVENT: expected `{claim}`\n  got: {got}\n\n  \
             An <img> whose pixels arrive must fire `load` at the element — through an inline \
             attribute handler, a script-assigned `onload`, and `addEventListener` alike — exactly \
             ONCE, with `complete` already true. A <div> must get nothing. Every row is \
             headless-Chrome-measured."
        );
    }
}
