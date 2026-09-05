//! **G_FLEX_CROSS_AXIS_IS_LOGICAL — the cross-axis predicate consulted the WRITING MODE, and every
//! quantity it is used with is already in the container's LOGICAL space.**
//!
//! `Placed::slot`, and the `cw` / `solved_h` a flex container reports, are all in the container's own
//! logical space: an orthogonal subtree is laid out on SWAPPED AXES (t1347) and mapped back to
//! physical coordinates afterwards. Measured directly on a `vertical-lr` `row` flex box 80 wide and
//! 120 tall around a 300x50 item: `cw` prints **120** — the physical HEIGHT — and the slot's `x`
//! extent prints **50**, the physical height of the item.
//!
//! ⭐⭐⭐ **So `x` is the INLINE axis whatever the writing mode is, and a `row` flex's main axis is the
//! inline axis BY DEFINITION.** The predicate is `cross_is_y = (flex-direction is a row)`, full stop.
//! t1436 and t1437 wrote it as `row == writing_mode.is_vertical()` — which is the correct expression
//! for a PHYSICAL question, and is what the scroll origin uses (t1427) — and reusing it on logical
//! quantities transposed the answer for **every vertical writing mode**.
//!
//! ⚠ That was 80 of the 90 rows still failing in `cssom-view/scrollWidthHeight-negative-margin-002`
//! when t1437 landed: `css/cssom-view` 1208 → **1268**, `css/css-flexbox` 3210 → **3213**.
//!
//! Chrome-measured (`google-chrome --headless --dump-dom`), `wrap-reverse` boxes, item offset
//! relative to its container:
//!
//! ```text
//!                                                          Chrome    before    after
//!   a1  80x80  vertical-lr, row,    300x50 item            -220,0     0,0     -220,0
//!   a2  80x80  vertical-rl, row,    300x50 item             0,0       0,0      0,0    ✓
//!   a3  80x80  vertical-lr, column, 50x300 item             0,-220    0,0      0,-220
//!   a4  80x80  vertical-rl, column, 50x300 item            30,-220   30,0     30,-220
//!   a7  80x80  vertical-lr, row,    50x50 item (FITS)      30,0      30,0     30,0   ✓
//!   a5  80x80  horizontal-tb, row      CONTROL              0,-220    0,-220   0,-220 ✓
//!   a6  80x80  horizontal-tb, column   CONTROL             -220,0    -220,0   -220,0 ✓
//!   b1  80x120 horizontal-tb, row      CONTROL              0,-180    0,-180   0,-180 ✓
//!   b2  80x120 horizontal-tb, column   CONTROL             -220,0    -220,0   -220,0 ✓
//!   b3  80x120 vertical-lr, column, 50x300 item             0,-180    0,0      0,-180
//! ```
//!
//! ⭐ **`b3` is the row that says the fix is the AXIS and not a square-box coincidence.** The
//! container is 80x120, so the two candidate cross sizes differ; a predicate that picked the wrong
//! axis would read −220 there instead of −180. `a5`/`a6`/`b1`/`b2` are the horizontal controls that
//! must not move, and `a7` is a vertical case that FITS and so must stay untouched.
//!
//! ⚠⚠ **NAMED RESIDUE, MEASURED AND NOT FIXED — a NON-SQUARE orthogonal `row` container.** With the
//! axis now right, `cross_size` is still read from `solved_h`, and for an orthogonal container that
//! is the CSS `height` — a physical length pinned as if it were the logical block size:
//!
//! ```text
//!                                                          Chrome    ours
//!   80x120  vertical-lr, row,    300x50 item               -220,0    -180,0
//!   80x120  vertical-rl, row,    300x50 item                0,0      -40,0
//! ```
//!
//! `-180 = 120 - 300` where the logical block size is the container's physical WIDTH, 80. It is the
//! orthogonal-root sizing seam (t1347), not this predicate, and it is invisible on a SQUARE container
//! — which is what `cssom-view/scrollWidthHeight-negative-margin-002`'s 80x80 wrapper is, and why the
//! 80 rows flip anyway. **A square fixture cannot tell two axes apart**, so `b3` is in the gate and
//! these two rows are named here instead of asserted.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><meta charset=utf-8><style>body{margin:0}
.s{overflow:hidden;scrollbar-width:none;display:flex;flex-wrap:wrap-reverse;width:80px;height:80px}
.r{overflow:hidden;scrollbar-width:none;display:flex;flex-wrap:wrap-reverse;width:80px;height:120px}</style></head><body>
<div class="s" id="a1" style="writing-mode:vertical-lr"><div id="k1" style="width:300px;height:50px"></div></div>
<div class="s" id="a2" style="writing-mode:vertical-rl"><div id="k2" style="width:300px;height:50px"></div></div>
<div class="s" id="a3" style="writing-mode:vertical-lr;flex-direction:column"><div id="k3" style="width:50px;height:300px"></div></div>
<div class="s" id="a4" style="writing-mode:vertical-rl;flex-direction:column"><div id="k4" style="width:50px;height:300px"></div></div>
<div class="s" id="a5"><div id="k5" style="width:50px;height:300px"></div></div>
<div class="s" id="a6" style="flex-direction:column"><div id="k6" style="width:300px;height:50px"></div></div>
<div class="s" id="a7" style="writing-mode:vertical-lr"><div id="k7" style="width:50px;height:50px"></div></div>
<div class="r" id="b3" style="writing-mode:vertical-lr;flex-direction:column"><div id="m3" style="width:50px;height:300px"></div></div>
<div class="r" id="b1"><div id="m1" style="width:50px;height:300px"></div></div>
<div class="r" id="b2" style="flex-direction:column"><div id="m2" style="width:300px;height:50px"></div></div>
<div id="out">-</div>
<script>
function r(c,k){var a=document.getElementById(c),b=document.getElementById(k);
 return (b.offsetLeft-a.offsetLeft)+','+(b.offsetTop-a.offsetTop);}
document.getElementById('out').textContent=
 'a1='+r('a1','k1')+' a2='+r('a2','k2')+' a3='+r('a3','k3')+' a4='+r('a4','k4')
 +' a5='+r('a5','k5')+' a6='+r('a6','k6')+' a7='+r('a7','k7')
 +' b3='+r('b3','m3')+' b1='+r('b1','m1')+' b2='+r('b2','m2');
</script></body></html>"##;

#[test]
fn the_flex_cross_axis_is_a_logical_question_not_a_physical_one() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://fo.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("FLEX CROSS AXIS LOGICAL: {got}");

    // ── VACUITY. The horizontal rows must already be right, or the vertical ones below are
    //    measuring whether the wrap-reverse overflow shift works at all (t1437) rather than which
    //    AXIS it lands on.
    assert!(
        got.contains("a5=0,-220") && got.contains("a6=-220,0"),
        "VACUOUS: the horizontal-tb CONTROLS are not Chrome-exact, so the vertical rows below are \
         not measuring the axis question this gate is named for — got {got:?}"
    );

    for (claim, why) in [
        ("a1=-220,0", "⭐ THE MECHANISM. A `row` flex's main axis is the INLINE axis; in `vertical-lr` that is physically vertical, so the CROSS axis is physically horizontal and the overflow moves the box left. Reading `0,0` means the predicate asked a physical question of a logical quantity."),
        ("a3=0,-220", "the `column` twin in the same writing mode — the cross axis is the other one, and it must move on the other physical axis."),
        ("a2=0,0", "`vertical-rl`'s block axis runs right-to-left, and Chrome does NOT shift here. The pair a1/a2 is what stops a fix that simply flips a sign for every vertical mode."),
        ("a4=30,-220", "`vertical-rl` + `column`: the shift lands on the block axis and the 30 is the un-shifted inline placement, unchanged."),
        ("a7=30,0", "CONTROL — a vertical case whose line FITS. The shift is the negative free space, so a fitting line must be untouched in every writing mode, not just horizontal ones."),
        ("b3=0,-180", "⭐⭐ THE NON-SQUARE ROW, and the reason this gate is not a coincidence. The container is 80x120, so the two candidate cross sizes DIFFER: a predicate that picked the wrong axis reads −220 here, not −180."),
        ("b1=0,-180", "CONTROL — the same non-square container in `horizontal-tb`."),
        ("b2=-220,0", "CONTROL — and its `column` twin."),
    ] {
        assert!(
            got.contains(claim),
            "G_FLEX_CROSS_AXIS_IS_LOGICAL: expected `{claim}` (Chrome-measured) — {why}\n  got: {got}"
        );
    }
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// P1  restore `row == writing_mode.is_vertical()` (the t1437 state)
//       -> a1, a3, a4 and b3 read the un-shifted position; every horizontal CONTROL stays green,
//          which is what identifies the defect as the SPACE the predicate is evaluated in.
// P2  invert to `!row`
//       -> the horizontal controls a5/a6/b1/b2 break instead — the two failures are disjoint, so no
//          single-axis constant passes this fixture.
//
// ⚠ `container_stretches_y` is shared with t1436's stretch adoption, and NONE of that gate's rows
//   move under this change — which is exactly what `g_flex_stretch_can_shrink`'s M5 reported when it
//   said its own writing-mode row was PINNED but not DISCRIMINATED. The honest report was right, and
//   the discriminating fixture turned out to be an overflowing wrap-reverse line, one tick later.
