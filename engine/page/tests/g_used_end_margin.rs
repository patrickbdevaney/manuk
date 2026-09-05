//! **G_USED_END_MARGIN — the margin a box actually has is the one AFTER collapsing, and the style
//! map cannot supply it.**
//!
//! `<div><div style="margin-bottom:50px">…</div></div>` — the inner child's margin collapses through
//! the auto-height wrapper, so the WRAPPER's margin box ends 50px below its border box while
//! `getComputedStyle(wrapper).marginBottom` is `0px`. Anything that compares a child against its
//! parent's margin box from the STYLE map reads the parent as smaller than it is and concludes the
//! child escapes a box it is inside. `manuk_layout::used_end_margins()` publishes the used value;
//! layout knew it all along and threw it away.
//!
//! ⭐⭐⭐ **THIS IS THE INPUT THREE BANKED GATES REFUSED A TICK WITHOUT.** t1431 measured the
//! containment fix below at **90 failing configurations → 45** on
//! `cssom-view/scrollWidthHeight-overflow-visible-margin-collapsing` and the ratchet refused it,
//! because `g_scroll_overflow_end_margin`, `g_scroll_overflow_alignment_rect` and
//! `g_scroll_extent_end_padding_containment` all went red on exactly the wrapper shape above. With
//! the used margin published, the same comparison is correct and all three stay green.
//!
//! Headless Chrome 145:
//!
//! ```text
//!                                                            chrome sh/ch   before
//!   a 20px box at `margin:20px` inside a 20px-tall FLEX ITEM,
//!   in an `overflow: auto` flex container                        60 / 60     80 / 60
//!   an auto-height wrapper whose child carries margin 50         270         270   CONTROL
//! ```
//!
//! ⚠ `overflow: auto` on that first row, not `visible`, and it is the difference between a gate and
//! a decoration: under `visible` the t1431 collapsed-margin rule already produces 60, so the row
//! passes with or without this fix. The matrix row that actually discriminates is
//! `auto/0/0/flex` — found by diffing the 140-cell matrix under the mutation rather than by
//! assuming the first fixture written would fail. *A control that cannot fail is not a control.*
//!
//! ```text
//! ```
//!
//! The first row is the containment: the inner box's BORDER box ends exactly at the flex item's edge
//! and its MARGIN box 20px past it, so judged on the border box it counts as contained and
//! contributes its own trailing margin on top of a container that has no room for it.
//!
//! ⚠ **NAMED, MEASURED, NOT GATED HERE — the vertical-writing-mode scoping.** `USED_END_MARGINS` is
//! recorded in the TRANSPOSED space an orthogonal run is laid out in, so for a box inside a
//! `writing-mode: vertical-*` run the engine's "block end" is physically HORIZONTAL and the value
//! must not be used as a physical bottom margin — the same class of mistake t1426 fixed in
//! `transform`. Priced on WPT: `css/css-overflow` reads **586 without the scoping and 588 with it**,
//! same binary both ways. The discriminating fixture
//! (`scrollable-overflow-height-with-flex-item-margin-inline-end*`) does not yet reproduce in this
//! harness — we answer 100 where Chrome answers 950 — so gating it would bank a number about a
//! DIFFERENT unimplemented rule (`margin-inline-end` in a vertical rtl flex container).

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
 body { margin:0 }
 .f { width:200px; display:flex; overflow:auto; scrollbar-width:none }
 .f div { height:20px; min-width:20px; margin:20px 10px }
 .s { width:100px; height:100px; overflow:scroll; padding:10px 5px; line-height:0 }
</style></head><body>
<div class="f" id="fx"><div><div></div></div><div></div><div></div><div></div></div>
<div class="s" id="wr"><div><div style="width:0;height:200px;margin-bottom:50px"></div></div></div>
<div id="out">-</div>
<script>var ids=["fx","wr"];
document.getElementById('out').textContent=ids.map(function(x){var e=document.getElementById(x);
return x+'='+e.scrollHeight+'/'+e.clientHeight;}).join(' ');</script>
</body></html>"##;

#[test]
fn a_box_contributes_the_margin_it_actually_has_after_collapsing() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://um.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("USED END MARGIN: {got}");

    // ── VACUITY. The published map must be non-empty, or the rows below are passing on the style
    //    fallback and this gate is about nothing.
    assert!(
        !manuk_layout::used_end_margins().is_empty(),
        "VACUOUS: `used_end_margins()` is empty, so every row below is answering from the STYLE \
         margin and the mechanism under test is not running at all"
    );

    for (claim, why) in [
        ("fx=60/60", "⭐ THE DEFECT. A 20px box at `margin: 20px` inside a 20px-tall FLEX ITEM ends its BORDER box exactly at the item's edge and its MARGIN box 20px past it. Judged on the border box it is 'contained' and contributes its own trailing margin on top — 80 against a clientHeight of 60. Judged on the margin box it is not contained and contributes only its border box, which is Chrome's 60."),
        ("wr=270/", "⭐ THE CONTROL THAT COST t1431 THE FIX. The child's `margin-bottom: 50px` collapses THROUGH the auto-height wrapper, so the wrapper's margin box ends at 260 while its STYLE margin is 0. With the style value the child looks like it escapes its parent and loses the scroller's end padding — 210 instead of 270. With the USED margin the wrapper's margin box covers it and the answer is Chrome's 270."),
    ] {
        assert!(
            got.contains(claim),
            "G_USED_END_MARGIN: expected `{claim}` (Chrome-measured) — {why}\n  got: {got}"
        );
    }
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// N1  judge containment on the child's BORDER box again (the pre-tick state)
//       -> fx reads 80; the wrapper CONTROL stays green, which identifies the mechanism as the
//          COMPARISON and not the published margin.
// N2  stop publishing `USED_END_MARGINS` (return the style margin) while keeping the margin-box
//     comparison — i.e. exactly the state the ratchet refused at t1431
//       -> wr reads 210, and this gate's VACUITY arm fires first if the map is empty outright.
// N3  publish for a collapse-through box too (drop the `rect.height > 0` guard)
//       -> `g_scroll_overflow_end_margin`'s `c5` reads 320 against Chrome's 270: a 0-height box's
//          used margin contains its own TOP margin, which is already in its position.
