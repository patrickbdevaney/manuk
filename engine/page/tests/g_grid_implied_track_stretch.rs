//! **G_GRID_IMPLIED_TRACK_STRETCH — an `auto` grid track absorbs the container's free space.**
//!
//! `grid-template-areas: "l r"` with **no** `grid-template-columns` is the ordinary way to write a
//! two-column layout: the areas string implies the columns, and each implied track is sized by
//! `grid-auto-columns`, whose initial value is `auto`. Per CSS Grid §11.8 *Stretch auto Tracks*, an
//! `auto`-max track then **absorbs the grid container's remaining free space**. So a 600px container
//! with a 20px gap gives two 290px columns, whatever the text inside them measures.
//!
//! We produced content-sized columns instead — measured against live Chromium on
//! `tests/wpt/probes/grid-implied-tracks.html`: **88px / 133px where Chromium gives 289px / 291px**,
//! with the rest of the container left empty. The column *count* was right and every item was in the
//! right cell, which is exactly why it survived four ticks of hunting: nothing was missing, nothing
//! was misplaced, the columns were simply too narrow and the page collapsed toward the left edge.
//!
//! THE CAUSE WAS NOT IN THE GRID CODE. §11.8 runs only when the inline axis is stretch-aligned, and
//! the inline axis alignment is `justify-content`, whose initial value is **`normal`** — which means
//! *flex-start in a flex container and stretch in a grid one*. Our CSS enum had no `Normal` variant,
//! so the initial value was stored as `FlexStart` and handed to taffy as a concrete `FLEX_START`.
//! Taffy resolves an ABSENT `justify-content` per formatting context (flex → `FLEX_START`, grid →
//! `STRETCH`); we never let it, so **every grid we have ever laid out skipped the stretch step** —
//! whether or not the author wrote `justify-content` at all.
//!
//! Three things to prove, and the third is the guard rather than the feature:
//!
//! 1. implied `auto` tracks stretch to fill the container;
//! 2. an EXPLICIT `justify-content` still wins — `center` must leave the tracks content-sized and
//!    centre them, or the fix has simply replaced one hard-coded answer with another;
//! 3. flex is untouched — `normal` in a flex container is still flex-start, not stretch.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<div id="out">-</div>

<!-- 1. THE FEATURE. Areas imply two columns; no grid-template-columns anywhere. 600 - 20 gap = 580,
        split equally between two `auto` tracks => 290 each, at x=0 and x=310. Both items carry the
        same one-character label so the assertion cannot depend on text measurement. -->
<div id="g" style="display:grid;grid-template-areas:'l r';column-gap:20px;width:600px">
  <div id="l" style="grid-area:l">L</div>
  <div id="r" style="grid-area:r">R</div>
</div>

<!-- 2. THE GUARD. Same layout, but the author asked for `center`. The tracks must stay content-sized
        (far under 290px for a single character) and the pair must be pushed off the left edge. -->
<div id="gc" style="display:grid;grid-template-areas:'l r';column-gap:20px;width:600px;justify-content:center">
  <div id="cl" style="grid-area:l">L</div>
  <div id="cr" style="grid-area:r">R</div>
</div>

<!-- 3. THE FLEX GUARD. `normal` in a flex container is flex-start, so three fixed 100px items pack
        against the left edge and do NOT spread across the 600px. -->
<div id="f" style="display:flex;width:600px">
  <div id="f1" style="width:100px">1</div>
  <div id="f2" style="width:100px">2</div>
  <div id="f3" style="width:100px">3</div>
</div>

<script>
  var R = [];
  function box(id){ return document.getElementById(id).getBoundingClientRect(); }
  function px(v){ return Math.round(v); }

  // 1. The implied tracks absorb the free space.
  R.push('lx:' + px(box('l').x) + ' lw:' + px(box('l').width));
  R.push('rx:' + px(box('r').x) + ' rw:' + px(box('r').width));

  // 2. An explicit `center` still centres content-sized tracks — it must not have become stretch.
  R.push('cw:' + (box('cl').width < 200 ? 'narrow' : 'stretched'));
  R.push('cx:' + (box('cl').x > 0 ? 'centred' : 'flush'));

  // 3. Flex still packs at the start.
  R.push('fx:' + px(box('f1').x) + ',' + px(box('f2').x) + ',' + px(box('f3').x));

  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn a_grid_auto_track_absorbs_free_space_while_explicit_alignment_and_flex_are_untouched() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://grid.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "lx:0 lw:290",
            "the first implied track must absorb its half of the free space: 600px container - 20px \
             gap = 580, split equally between two `auto` tracks. Content-sizing it instead is the \
             defect — Chromium gives 289px on the probe page where we gave 88px",
        ),
        (
            "rx:310 rw:290",
            "and the second track starts after the first track plus the gap. If this reads a small \
             x with a small width, the tracks are still content-sized and the whole two-column \
             layout is huddled against the left edge with the container's right half empty",
        ),
        (
            "cw:narrow",
            "an EXPLICIT `justify-content: center` must NOT stretch the tracks. Stretch is what the \
             INITIAL value `normal` means in a grid; it is not what every value means. If this says \
             `stretched` the fix has replaced one hard-coded alignment with another",
        ),
        (
            "cx:centred",
            "…and the content-sized pair is centred in the 600px container, so the first track no \
             longer starts at x=0",
        ),
        (
            "fx:0,100,200",
            "FLEX IS UNTOUCHED. `normal` resolves per formatting context: stretch in grid, but \
             flex-start in flex. Three fixed 100px items must still pack against the left edge — if \
             they have spread out, `normal` leaked the grid meaning into the flex algorithm",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_GRID_IMPLIED_TRACK_STRETCH: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
