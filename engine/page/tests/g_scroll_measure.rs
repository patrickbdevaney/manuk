//! **G_SCROLL_MEASURE — a geometry read after a SCROLL sees the scrolled layout.**
//!
//! `G_FORCED_REFLOW` holds the half of this that the DOM's mutation counter can see:
//! `measure -> mutate -> measure` in one task lays out before it answers. This gate holds the half
//! it could NOT see, and the gap was invisible for exactly that reason:
//!
//! ```text
//!   scroller.scrollTop = 100;                    // mutates NO DOM
//!   row.getBoundingClientRect().top;             // ...so nothing was ever re-laid-out
//! ```
//!
//! `forced_reflow`'s staleness test was `dom.mutation_seq() == laid_out_at` and nothing else, so a
//! scroll assignment left the published rect snapshot untouched and the read answered from the
//! **pre-scroll** layout. That is the shape of every virtualised list (react-window, react-virtuoso,
//! any data grid: scroll, then measure which rows are now in view), of every "scroll it into place
//! then measure" carousel, and of the whole `css/css-position/sticky` suite.
//!
//! ⭐ **The general defect, which is why this gate is named for the read and not for sticky: a
//! snapshot invalidated by ONE kind of change is blind to every other kind.** Layout-affecting state
//! that is not the DOM — scroll offsets, sticky state, viewport size, media-query state — needs its
//! own term in the guard, or the read is stale in precisely the case the code was written for.
//!
//! **To watch it go RED, three ways, each isolating one arm:**
//!
//! 1. drop `scroll_seq == c.scrolled_at` from `forced_reflow`'s guard → every row below reports its
//!    pre-scroll position, because no reflow happens at all;
//! 2. keep the guard but delete the offset-application loop → the reflow runs and rebuilds an
//!    **unscrolled** tree, which is the worse failure: a confident wrong answer of the right type;
//! 3. delete the `apply_sticky` call in `forced_reflow` → the plain rows still pass and only
//!    `sticky:` fails, which is what makes the sticky row a separate claim rather than a restatement.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<div style="height:400px">spacer — the pane is NOT at the document origin</div>
<div id="pane" style="overflow:auto;height:200px">
  <div id="sec" style="height:1200px">
    <div id="hdr" style="position:sticky;top:0;height:30px">Header</div>
    <div id="row" style="height:50px">row</div>
    <div style="height:1120px">filler</div>
  </div>
</div>
<div id="out">-</div>
<script>
  var R = [];
  var pane = document.getElementById('pane');
  var row  = document.getElementById('row');
  var hdr  = document.getElementById('hdr');

  // Baseline. The pane starts 400px down the document; the sticky header is at its top and the row
  // sits directly under it. Stated first so a later number cannot be luck.
  R.push('pane0:' + Math.round(pane.getBoundingClientRect().top));   // 400
  R.push('row0:'  + Math.round(row.getBoundingClientRect().top));    // 430

  // THE READ THIS GATE EXISTS FOR — one task, no DOM mutation between the write and the read.
  pane.scrollTop = 100;
  R.push('row1:' + Math.round(row.getBoundingClientRect().top));     // 330 — moved up by 100
  // The script must also read back its own scroll write on the very next line.
  R.push('top1:' + Math.round(pane.scrollTop));                      // 100

  // ...and the sticky header must NOT have moved with it: it is pinned to the PANE's top edge,
  // which is a different scrollport from the document's.
  R.push('sticky:' + Math.round(hdr.getBoundingClientRect().top));   // 400

  // A second read with nothing further scrolled returns the same answer — the added staleness term
  // must not make the reflow fire forever.
  R.push('row2:' + Math.round(row.getBoundingClientRect().top));     // 330

  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

// ⚠ ONE `#[test]` fn per binary that runs JS — a live `PageContext` parked on one thread plus a
// second `Page::load` on another faults SpiderMonkey. Sequential loads inside one fn are fine.
#[test]
fn a_geometry_read_after_a_scroll_sees_the_scrolled_layout() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://scrollmeasure.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "pane0:400",
            "the pane really does start 400px down — without that the document scrollport and the \
             pane's own coincide, and the sticky row below would pass for the wrong reason",
        ),
        (
            "row0:430",
            "the baseline position of the measured row, under the 30px header. A later 330 is only \
             meaningful against a stated 430",
        ),
        (
            "row1:330",
            "THE GATE. `pane.scrollTop = 100` mutates no DOM, so with only the mutation-counter \
             staleness term no reflow fires and this reads the pre-scroll 430 — which is how a \
             virtualised list decides the wrong rows are on screen",
        ),
        (
            "top1:100",
            "the script reads back its own scroll write on the next line — the JS-side mirror, \
             which is a different mechanism from the rect snapshot and must not be confused with it",
        ),
        (
            "sticky:400",
            "the sticky header is pinned to the PANE's top edge, not carried down with the content \
             and not resolved against the document viewport. Reads 330 if the forced reflow skips \
             the sticky pass, and 430 if it skips the scroll",
        ),
        (
            "row2:330",
            "idempotent — a second read with nothing newly scrolled gives the same answer, so the \
             new staleness term settles instead of re-laying-out on every read",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_SCROLL_MEASURE: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
