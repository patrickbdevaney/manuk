//! **G_FLEX_STRETCH_CAN_SHRINK — the adoption of taffy's slot height could only ever GROW a box,
//! and `align-items: stretch` is a rule that sometimes shrinks one.**
//!
//! `extract_placed` takes taffy's slot height for an `auto`-height item only when it is LARGER than
//! the height the item measured for itself. `stretch` — the initial value of `align-items`, so the
//! case on nearly every page — sets an `auto` CROSS size to the **line's** cross size and lets the
//! content overflow. When the content is taller than the line, taffy's slot is SMALLER and the `>`
//! declined it.
//!
//! ⭐ **This is the same asymmetry t1435 fixed one property earlier in the same function**: the
//! inline axis takes taffy's verdict unconditionally and the block axis took it only when it agreed.
//!
//! ⚠⚠⚠ **BUT THE VERDICT IS ONLY A VERDICT ON THE CROSS AXIS.** Along the MAIN axis an `auto` item's
//! size is its content size floored by `min-height: auto`, and a slot smaller than that is taffy
//! failing to MEASURE rather than taffy ALIGNING. `s9` is the reduced shape of
//! `css-flexbox/flex-basis-013.html`, where taffy's slot comes back **0** on a `column` container
//! whose flex base size depends on the cross size — adopting it unconditionally turned ten green
//! rows red with `height expected 50 but got 0`.
//!
//! Chrome-measured (`google-chrome --headless --dump-dom`), 80x80 boxes, item `width:50px`:
//!
//! ```text
//!                                                       Chrome    before    after
//!   s1  row flex, item auto, 200px content                 80       200       80    ← MECHANISM
//!   s6  GRID, 80px row track, item auto, 200px content      80       200       80    ← MECHANISM
//!   s2  row flex, item auto, 30px content                   80        80       80    the GROW half
//!   s4  COLUMN flex, item auto, 200px content   CONTROL    200       200      200
//!   s5  row flex, align-items:flex-start        CONTROL    200       200      200
//!   s7  vertical-lr row flex, 200px content     CONTROL    200       200      200
//!   s8  row flex, item height:stretch, 200px content        80        80       80
//!   s9  column inline-flex, canvas width:100%   REGRESSION  50        50       50
//!   s11 REPLACED canvas 480x474 in a flex row              148.4     148.4    148.4
//!   s12 the same canvas in a BLOCK              CONTROL    148.4     148.4    148.4
//! ```
//!
//! ⭐ `s4`, `s5` and `s7` are what make the rule safe rather than merely smaller: in a COLUMN
//! container the height is the MAIN axis, `align-items: flex-start` does not stretch at all, and a
//! `row` flex in a VERTICAL writing mode has its main axis on the physical y — taffy returns 200 for
//! all three, so the rows that move are only the ones where a stretch verdict was being ignored.
//!
//! ⚠⚠⚠ **AND A REPLACED ELEMENT IS NOT STRETCHED — THE WALL CAUGHT THAT AND THIS FIXTURE COULD
//! NOT.** `align-items: stretch` on a `<canvas>`/`<img>` with an intrinsic ratio does not hand it the
//! line's cross size; the RATIO decides. Adopting taffy's slot published taffy's own ratio
//! arithmetic (148.1) over ours (148.4), and
//! `manuk_layout::tests::replaced_constraint_violation_table_per_formatting_context` went red on two
//! cells (`j/flex`, `j/grid`) that had been green. ⭐ The 0.3px is not the point: **a replaced box's
//! cross size is a TRANSFER, not an alignment, so an alignment verdict does not apply to it.** `s11`
//! and `s12` are the pair — the same canvas in a flex row and in a block must agree.
//!
//! ⚠ **`s7` PINS the writing-mode term but does not DISCRIMINATE it** — replacing
//! `row == writing_mode.is_vertical()` with `!row` leaves every row above unchanged, because taffy's
//! slot happens to agree there. It is kept because it is the correct expression (the same one the
//! scroll origin uses, t1427), not because this gate defends it. Said rather than implied.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><meta charset=utf-8><style>body{margin:0}
.b{width:80px;height:80px;overflow:hidden;scrollbar-width:none}</style></head><body>
<div class="b" id="s1" style="display:flex"><div id="k1" style="width:50px"><div style="height:200px"></div></div></div>
<div class="b" id="s2" style="display:flex"><div id="k2" style="width:50px"><div style="height:30px"></div></div></div>
<div class="b" id="s4" style="display:flex;flex-direction:column"><div id="k4" style="width:50px"><div style="height:200px"></div></div></div>
<div class="b" id="s5" style="display:flex;align-items:flex-start"><div id="k5" style="width:50px"><div style="height:200px"></div></div></div>
<div class="b" id="s6" style="display:grid;grid-template-rows:80px;grid-template-columns:80px"><div id="k6" style="width:50px"><div style="height:200px"></div></div></div>
<div class="b" id="s7" style="display:flex;writing-mode:vertical-lr"><div id="k7" style="width:50px"><div style="height:200px"></div></div></div>
<div class="b" id="s8" style="display:flex"><div id="k8" style="width:50px;height:stretch"><div style="height:200px"></div></div></div>
<div id="s9" style="display:inline-flex;flex-direction:column;width:50px;height:50px;overflow:hidden"><div id="k9" style="min-width:0;min-height:0"><canvas width="5" height="5" style="display:block;width:100%"></canvas></div></div>
<div style="width:230px;display:flex"><canvas id="k11" width="480" height="474" style="box-sizing:border-box;padding:10px;max-width:150px"></canvas></div>
<div style="width:230px"><canvas id="k12" width="480" height="474" style="box-sizing:border-box;padding:10px;max-width:150px"></canvas></div>
<div id="out">-</div>
<script>
document.getElementById('out').textContent=[["s1","k1"],["s2","k2"],["s4","k4"],["s5","k5"],["s6","k6"],["s7","k7"],["s8","k8"],["s9","k9"]].map(function(p){
  var k=document.getElementById(p[1]); return p[0]+'='+k.offsetWidth+'x'+k.offsetHeight;}).join(' ')+' s11='+document.getElementById('k11').getBoundingClientRect().width.toFixed(1)+'x'+document.getElementById('k11').getBoundingClientRect().height.toFixed(1)+' s12='+document.getElementById('k12').getBoundingClientRect().width.toFixed(1)+'x'+document.getElementById('k12').getBoundingClientRect().height.toFixed(1);
</script></body></html>"##;

#[test]
fn a_stretched_item_takes_its_lines_cross_size_even_when_that_shrinks_it() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://fo.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("FLEX STRETCH SHRINK: {got}");

    // ── VACUITY. The GROW half must already work, or `s1`/`s6` below are measuring whether the
    //    adoption runs at all rather than which DIRECTION it runs in.
    assert!(
        got.contains("s2=50x80"),
        "VACUOUS: the grow half of the adoption is not working, so the shrink rows below are not \
         measuring the direction this gate is named for — got {got:?}"
    );

    for (claim, why) in [
        (
            "s1=50x80",
            "⭐ THE MECHANISM. `align-items` defaults to `stretch`, the single flex line's cross \
             size is the 80px container, and a stretched item takes that even though its own \
             content is 200px tall — the content overflows, the box does not grow.",
        ),
        (
            "s6=50x80",
            "⭐ THE SAME RULE IN THE OTHER FORMATTING CONTEXT. A grid item stretches to its ROW \
             TRACK; an 80px track holding 200px of content is an 80px item with overflow.",
        ),
        (
            "s4=50x200",
            "CONTROL — in a COLUMN container the height is the MAIN axis, where `min-height: auto` \
             floors an item at its content size. Nothing stretches it, and taffy returns 200.",
        ),
        (
            "s5=50x200",
            "CONTROL — `align-items: flex-start` does not stretch, so the slot is the item's own \
             hypothetical cross size and the adoption must be a no-op.",
        ),
        (
            "s7=50x200",
            "CONTROL — a `row` flex in `writing-mode: vertical-lr` has its MAIN axis on the physical \
             y, so the physical height is not the stretched axis here either. This is why the \
             predicate is `row == is_vertical()` and not `flex-direction is a row`.",
        ),
        (
            "s11=150.0x148.4",
            "⭐⭐ THE ROW THE WALL HAD TO TEACH THIS FIXTURE. A replaced element with an intrinsic \
             ratio is NOT stretched — its cross size is a transfer through the ratio, and `s12` is \
             the same canvas in a BLOCK, which must give the identical answer.",
        ),
        (
            "s12=150.0x148.4",
            "CONTROL for `s11` — if these two ever disagree, the flex context is applying an \
             alignment to a box whose size the ratio already decided.",
        ),
        (
            "s9=50x50",
            "⚠⚠⚠ REGRESSION ARM, and the reduced shape of `css-flexbox/flex-basis-013.html`. A \
             COLUMN container whose flex base size depends on the cross size gets a slot height of \
             **0** from taffy. On the main axis that is a MISSING MEASUREMENT, not an alignment \
             verdict — publishing it reads 0 where Chrome reads 50, and it cost ten rows the first \
             time this fix was written without the axis test.",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_FLEX_STRETCH_CAN_SHRINK: expected `{claim}` (Chrome-measured) — {why}\n  got: {got}"
        );
    }
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// M1  the pre-tick rule (`if slot.height > box.height` alone)
//       -> s1=50x200 and s6=50x200; every control stays green, which identifies the defect as the
//          DIRECTION of the adoption rather than the adoption itself.
// M2  drop the axis test and adopt for every flex/grid parent
//       -> s9=50x0 — taffy's un-measured main-axis zero published straight through, which is the
//          ten-row regression in `flex-basis-013.html` reproduced in one row.
// M4  make the GRID arm of `container_stretches_y` return false
//       -> s6=50x200; the flex rows are unmoved, so the two formatting contexts are separable here.
// M6  drop the `!replaced` term
//       -> s11=150.0x148.1 while s12 stays 148.4 — the flex row disagreeing with the block for the
//          same canvas, which is the shape the wall reported as `j/flex` and `j/grid`.
// M5  replace `row == writing_mode.is_vertical()` with `!row`
//       -> GREEN, and reported as such. Taffy's slot agrees on `s7` either way, so no row in this
//          fixture discriminates the writing-mode term. It is kept for correctness, not coverage.
//
// ⚠ NAMED RESIDUE, measured this tick and NOT fixed: the `s9` shape in a VERTICAL writing mode
//   (`writing-mode: vertical-lr`, a `<canvas>` at `height:100%`) reads Chrome **5** and ours **0**.
//   It is pre-existing — the branch this tick adds is inactive there — and it is the same
//   un-measured main axis, one writing mode over.
