//! **G_MIN_WIDTH_FLOORS_INTRINSIC — a box's `min-width` never reached its intrinsic contribution,
//! so a block child with `min-width` and no content contributed ZERO to every shrink-to-fit
//! ancestor.**
//!
//! CSS Sizing §5.1: the min-content and max-content contributions of a box are its outer size
//! **clamped by its min and max sizes**. Ours had only the upper half — `max-width` reached the used
//! width and `min-width` reached nothing.
//!
//! ⭐ The mechanism is one branch of `content_right_extent`. That walk lays a subtree out at a 1e6
//! available width and discards a block box's own `rect.width` as an artifact of the measuring width
//! (it is ~1e6 and meaningless), recursing to the inline text that carries the real extent. **A
//! declared `min-width` is the one part of that width that was never a function of the measuring
//! width**, and it was going out with the artifact.
//!
//! Chrome-measured (`google-chrome --headless --dump-dom`), an `.k { height: 10px }` child inside
//! each shrink-to-fit context:
//!
//! ```text
//!                                                          Chrome    before    after
//!   a1  inline-block > div{min-width:20px}                    20        0        20
//!   a2  inline-block > div{min-width:20px; margin:0 10px}     40       10        40
//!   a4  float        > div{min-width:20px}                    20        0        20
//!   a5  abspos       > div{min-width:20px}                    20        0        20
//!   a6  flex item    > div{min-width:20px}                    20        0        20
//!   a7  table cell   > div{min-width:20px}                    26        6        26
//!   a9  inline-block > div{min-width:20px}"x"                 20        8        20
//!   b1  min-width:20px; box-sizing:border-box; padding:0 5px  20        5        20
//!   b2  min-width:20px; padding:0 5px  (content-box)          30        5        30
//!   b3  min-width:20px around an 8-char word     CONTROL      77.0625  77.0625  77.0625 ✓
//!   a8  inline-block > span[inline-block]{min-width}  CONTROL 20       20        20  ✓
//!   a3  inline-block > div{width:20px; max-width:5px} CONTROL  5        5         5  ✓
//! ```
//!
//! ⭐⭐ **`a3` AND `a8` ARE WHAT SAY THIS IS THE MISSING HALF OF A CLAMP RATHER THAN A MISSING
//! CLAMP.** An INLINE-level child already carried its `min-width` into the line box, and `max-width`
//! already reached the used width — so only the block child's LOWER bound was unrepresented, and it
//! was unrepresented in every context that asks for an intrinsic width at once.
//!
//! ⚠ `b3` is the row that keeps the floor a FLOOR: when the content is wider than `min-width`, the
//! content wins. Applying the floor and returning would read 20 there instead of 77.
//!
//! ⚠ `b1`/`b2` are the box-sizing pair. `content_right_extent` measures BORDER boxes, so a
//! content-box `min-width` has to gain the frame the used width would give it — and a border-box one
//! must not. Without the pair, either convention passes.
//!
//! ⚠ Only `Dim::Px`: a percentage `min-width` resolves against a containing block this measurement
//! does not have, and guessing a basis is worse than declining.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><meta charset=utf-8><style>body{margin:0}
.k{height:10px}
</style></head><body>
<div><span id="a1" style="display:inline-block"><div class=k style="min-width:20px"></div></span></div>
<div><span id="a2" style="display:inline-block"><div class=k style="min-width:20px;margin:0 10px"></div></span></div>
<div><span id="a3" style="display:inline-block"><div class=k style="max-width:5px;width:20px"></div></span></div>
<div><div id="a4" style="float:left"><div class=k style="min-width:20px"></div></div></div>
<div style="position:relative;height:20px"><div id="a5" style="position:absolute"><div class=k style="min-width:20px"></div></div></div>
<div style="display:flex"><div id="a6"><div class=k style="min-width:20px"></div></div></div>
<div><table id="a7" style="width:auto"><tr><td><div class=k style="min-width:20px"></div></td></tr></table></div>
<div><span id="a8" style="display:inline-block"><span style="display:inline-block;min-width:20px" class=k></span></span></div>
<div><span id="a9" style="display:inline-block"><div class=k style="min-width:20px">x</div></span></div>
<div><span id="b1" style="display:inline-block"><div class=k style="min-width:20px;box-sizing:border-box;padding:0 5px"></div></span></div>
<div><span id="b2" style="display:inline-block"><div class=k style="min-width:20px;padding:0 5px"></div></span></div>
<div><span id="b3" style="display:inline-block;font:16px/1 monospace"><div class=k style="min-width:20px">wwwwwwww</div></span></div>
<div id="out">-</div>
<script>
function w(k){return document.getElementById(k).getBoundingClientRect().width;}
document.getElementById('out').textContent=
 'a1='+w('a1')+' a2='+w('a2')+' a3='+w('a3')+' a4='+w('a4')+' a5='+w('a5')+' a6='+w('a6')+' a7='+w('a7')+' a8='+w('a8')+' a9='+w('a9')+' b1='+w('b1')+' b2='+w('b2')+' b3='+w('b3');
</script></body></html>"##;

#[test]
fn a_declared_min_width_floors_the_intrinsic_contribution() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://fo.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("MIN WIDTH FLOORS INTRINSIC: {got}");

    // ── VACUITY. The upper half of the clamp and the inline-level path must already be right, or
    //    these rows are measuring whether min/max sizes work at all.
    assert!(
        got.contains("a3=5") && got.contains("a8=20"),
        "VACUOUS: `max-width` or an inline-level `min-width` is not Chrome-exact, so the rows below \
         are not measuring the missing HALF of the clamp — got {got:?}"
    );

    for (claim, why) in [
        ("a1=20", "⭐ THE MECHANISM, smallest form. An empty block child declaring `min-width:20px` contributes 20 to its shrink-to-fit parent, not 0."),
        ("a2=40", "the same with margins — the contribution is the OUTER size, so the floor composes with the margin term rather than replacing it."),
        ("a4=20", "FLOAT. The same walk serves every shrink-to-fit context, which is why one branch fixed all of them."),
        ("a5=20", "ABSPOS."),
        ("a6=20", "FLEX ITEM."),
        ("a7=26", "TABLE CELL — 26 rather than 20 because the cell's own frame is added by its caller, which is what says the floor is returned as a BORDER box and not a used width."),
        ("a9=20", "content NARROWER than the floor: the 8px text loses to the 20px minimum."),
        ("b3=77.0625", "⚠ CONTROL, and it keeps the floor a FLOOR — content WIDER than the minimum wins. Applying the floor and returning would read 20 here."),
        ("b1=20", "⚠ BOX-SIZING PAIR, border-box: `min-width` already includes the padding, so nothing is added."),
        ("b2=30", "⚠ BOX-SIZING PAIR, content-box: the 10px of padding is added to reach the border box the walk measures. Without both rows, either convention passes."),
        ("a3=5", "CONTROL — `max-width` was already reaching the used width. The clamp had an upper half all along."),
        ("a8=20", "CONTROL — an INLINE-level child already carried its `min-width`, so only the BLOCK path was missing."),
    ] {
        assert!(
            got.contains(claim),
            "G_MIN_WIDTH_FLOORS_INTRINSIC: expected `{claim}` (Chrome-measured) — {why}\n  got: {got}"
        );
    }
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// S1  drop the floor from the FILLED branch (the pre-tick state)
//       -> a1, a2, a4, a5, a6, a7, a9, b1 and b2 all collapse; a3, a8 and b3 stay green, which is
//          what identifies the defect as the block path's lower bound.
// S2  return the floor as-is, without the box-sizing frame
//       -> b2 reads 20 instead of 30 while b1 stays right — only the content-box row can see it.
// S3  `return` after applying the floor instead of continuing the walk
//       -> b3 reads 20 instead of 77.0625: the floor stops being a floor and becomes the answer.
