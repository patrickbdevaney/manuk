//! **G_LOAD_WAITS_FOR_SUBRESOURCES — `load` fired before the images, so every `window.onload`
//! handler on the web measured a document that was not finished.**
//!
//! HTML's *"the end"* steps run the load event only once the document's list of in-flight fetches is
//! empty, and IMAGES are on that list. `Page::load_async` fired it before the subresource phases, and
//! said so in a comment: *"the subresource phases have not run yet, but the document and its frames
//! are ready, which is what `load` waits for."* That is not what `load` waits for.
//!
//! ⭐ **The symptom is an undecoded image, and it has a signature: one axis wholly right and the
//! other wholly wrong.** An `<img>` whose natural size has not arrived has `naturalWidth === 0`, so
//! its intrinsic ratio is unavailable and any height derived from a declared width comes out ZERO.
//! `css-grid/grid-items/grid-minimum-size-grid-items-021` scored **exactly half** its subtests for
//! that reason — every declared WIDTH passed, every ratio-derived HEIGHT failed. **72/144 → 126/144.**
//!
//! ⚠ The budget is unchanged and is what keeps this safe: the enhancement phase already runs under
//! `load_budget()`, so a page with a dead image still fires `load` on schedule with whatever arrived.
//!
//! ## And the repair the reordering demanded
//!
//! Firing `load` late made `cssom-view/elementsFromPoint.html` fail one row it had been passing —
//! **for the wrong reason.** The test samples the CENTRE of a squiggle whose `<path>` has
//! `fill="none"`, and the path had simply had no layout box yet at the old firing time. With the box
//! present, our hit test returned it: four elements where Chrome returns three.
//!
//! `pointer-events` defaults to `visiblePainted`, and for an SVG shape "painted" means the FILL
//! region when `fill` is not `none` and the STROKE region when `stroke` is not `none` — never the
//! bounding box. So a `fill:none` shape is a curve with a hole the size of its own bbox.
//!
//! ⚠⚠ **BOUND, STATED: this declines the bbox hit, it does not implement the stroke.** A point that
//! really is ON the stroke of a `fill:none` path IS hit in Chrome and is not hit here. That needs
//! path geometry this seam does not have; the gap is one-directional (we under-hit, never over-hit)
//! and is recorded rather than approximated.
//!
//! Chrome-measured (`google-chrome --headless --dump-dom`):
//!
//! ```text
//!                                                     Chrome              before          after
//!   hit      centre of <path fill=none>               3: sq,body,html     4 (path too)    3  ✓
//!   hitfill  centre of <rect fill=#00f>               4: rf,sf,body,html  see below       4  ✓
//!   hitnone  centre of <svg fill=none><rect fill=#0f0> 4: rn,sn,body,html  4              4  ✓
//! ```
//!
//! ⭐⭐ **`hitfill` IS THE ROW THAT SEES THE REORDERING**, and it is the only one that can from a
//! `data:` URL fixture: with `load` fired before the enhancement phase the SVG's own boxes do not
//! exist yet, so the handler finds **3** elements instead of 4. A `data:` image is decoded during
//! parse, so `naturalWidth` cannot discriminate here — the WPT file above is what measures that half,
//! and it is named rather than claimed.
//!
//! ⚠ `hitnone` is the row that keeps the tag list honest: `fill="none"` on the `<svg>` CONTAINER (or
//! on any non-shape element) must NOT stop it being hit — only the SHAPE elements paint a fill.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><meta charset=utf-8><style>body{margin:0}
.g{display:inline-grid;grid-template-rows:auto;grid-template-columns:auto}
svg{display:block}
</style></head><body>
<div class="g" id="g1"><img id="i1" style="width:200px" src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAADIAAAAyCAIAAACRXR/mAAAARElEQVR4nO3OMQ0AMAwDsPBHNlgj0D85LBmAk5dF/YGWlpaWltaG/kBLS0tLS2tDf6ClpaWlpbWhP9DS0tLS0trQH1w+rEehih7s10EAAAAASUVORK5CYII="></div>
<svg id="sq" xmlns="http://www.w3.org/2000/svg" height="98" width="500" viewBox="0 0 581 98">
  <path id="pa" d="M2 2 L579 2 L579 96" stroke="#000" stroke-width="4" fill="none"></path>
</svg>
<svg id="sf" xmlns="http://www.w3.org/2000/svg" height="40" width="80">
  <rect id="rf" x="0" y="0" width="80" height="40" fill="#00f"></rect>
</svg>
<svg id="sn" fill="none" xmlns="http://www.w3.org/2000/svg" height="40" width="80">
  <rect id="rn" x="0" y="0" width="80" height="40" fill="#0f0"></rect>
</svg>
<div id="out">-</div>
<script>
window.addEventListener('load', function(){
  var G=getComputedStyle(document.getElementById('g1')), I=document.getElementById('i1');
  var s=document.getElementById('sq'), r=s.getBoundingClientRect();
  var hit=document.elementsFromPoint(r.left+Math.round(r.width/2), r.top+Math.round(r.height/2));
  var s2=document.getElementById('sf'), r2=s2.getBoundingClientRect();
  var hit2=document.elementsFromPoint(r2.left+Math.round(r2.width/2), r2.top+Math.round(r2.height/2));
  document.getElementById('out').textContent=
    'atload_nat='+I.naturalWidth+'x'+I.naturalHeight+' atload_gridh='+G.height
    +' hit='+hit.length+':'+Array.prototype.map.call(hit,function(e){return e.id||e.tagName.toLowerCase()}).join(',')
    +' hitfill='+hit2.length+':'+Array.prototype.map.call(hit2,function(e){return e.id||e.tagName.toLowerCase()}).join(',')+' hitnone='+(function(){var a=document.getElementById('sn'),b=a.getBoundingClientRect();var h=document.elementsFromPoint(b.left+Math.round(b.width/2), b.top+Math.round(b.height/2));return h.length+':'+Array.prototype.map.call(h,function(e){return e.id||e.tagName.toLowerCase()}).join(',')})();
});
</script></body></html>"##;

#[test]
fn the_load_event_waits_for_subresources_and_an_unpainted_shape_is_not_hit() {
    let fonts = FontContext::new();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let page = rt.block_on(manuk_page::Page::load_async(
        HTML,
        "https://loadorder.test/",
        &fonts,
        800.0,
    ));
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("LOAD WAITS FOR SUBRESOURCES: {got}");

    // ── VACUITY. The handler must have run at all, and the image must have decoded, or every row
    //    below is measuring an empty document rather than an ordering.
    assert!(
        got.contains("atload_nat=50x50"),
        "VACUOUS: the load handler did not run, or the image never decoded — the rows below are not \
         measuring an ORDERING — got {got:?}"
    );

    for (claim, why) in [
        (
            "hitfill=4:rf,sf,body,html",
            "⭐⭐ THE REORDERING. A filled `<rect>` inside an `<svg>` is hit, along with its `<svg>`, \
             the body and the html — FOUR elements. With `load` fired before the enhancement phase \
             the SVG's boxes do not exist yet and the handler finds three. This is the only row a \
             `data:`-URL fixture can see the ordering with, because a `data:` image decodes during \
             parse.",
        ),
        (
            "hit=3:sq,body,html",
            "⭐ THE REPAIR. `pointer-events: visiblePainted` means an SVG shape is hit where it is \
             PAINTED; a `<path fill=\"none\">` paints nothing at the centre of its own bounding \
             box, so the path must not be in the list.",
        ),
        (
            "hitnone=4:rn,sn,body,html",
            "CONTROL — `fill=\"none\"` on the `<svg>` CONTAINER must not stop it being hit. Only \
             the SHAPE elements paint a fill, and without this row the rule could be written as \
             `any element with fill=none` and still pass.",
        ),
        (
            "atload_gridh=200px",
            "the image's intrinsic ratio is available to the handler: a 50x50 bitmap declared \
             `width:200px` is 200 tall, and the `auto` grid row that holds it is 200px. This is the \
             shape that scored exactly half of `grid-minimum-size-grid-items-021`.",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_LOAD_WAITS_FOR_SUBRESOURCES: expected `{claim}` (Chrome-measured) — {why}\n  got: {got}"
        );
    }
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// T1  fire `load` before the images+masks phase again (the pre-tick order)
//       -> hitfill reads `3:sf,body,html` — the filled rect has no box when the handler runs.
// T2  make `svg_shape_is_unpainted` always false
//       -> hit reads `4:pa,sq,body,html`, the four-element answer the WPT file rejected.
// T3  drop the SHAPE-tag list, so any element with `fill="none"` is skipped
//       -> hitnone reads `3:rn,body,html` — the `<svg>` container disappears from its own hit list.
//
// ⚠ Each mutation moves a DIFFERENT row and leaves the others green, which is what says the two
//   mechanisms in this tick are separable even though one of them was only reachable because of the
//   other.
