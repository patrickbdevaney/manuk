//! **G_BLOCK_EXTENT_IS_THE_LOGICAL_ONE — a flex/grid container's BLOCK extent is its CSS `width` in
//! a vertical writing mode, and this read `height` unconditionally.**
//!
//! Everything `solve_subtree` is handed is in the container's own LOGICAL space (t1438): `cw` is
//! already the logical INLINE size — the physical HEIGHT for a vertical container — and the block
//! size beside it was still the physical one. So an orthogonal grid distributed its rows down 300px
//! of physical height while Chrome distributed them across 400px of physical width.
//!
//! ⭐ **One of the pair was mapped and the other was not**, which is this engine's most-repeated
//! shape: the `justify-content` control below is unmoved because the INLINE size was already being
//! transposed correctly.
//!
//! Chrome-measured (`google-chrome --headless --dump-dom`), `width:400px; height:300px;
//! grid-auto-rows:40px`, single-column rows; the pair is the LAST item's offset from the container:
//!
//! ```text
//!                                                        Chrome    before    after
//!   a1  vertical-lr   align-content: space-between        360,0     260,0     360,0
//!   a2  vertical-lr   align-content: center               200,0     150,0     200,0
//!   a3  vertical-lr   align-content: end                  360,0     260,0     360,0
//!   a5  vertical-rl   align-content: space-between          0,0     100,0       0,0
//!   a7  vertical-lr   FLEX column, justify-content: s-b     360,0     260,0     360,0
//!   a4  horizontal-tb align-content   CONTROL              0,260     0,260     0,260  ✓
//!   a6  vertical-lr   JUSTIFY-content CONTROL              80,0      80,0      80,0   ✓
//!   a8  vertical-lr   width:auto      CONTROL              40,0      40,0      40,0   ✓
//! ```
//!
//! ⭐⭐ **`260 = 300 − 40` and `360 = 400 − 40`.** The free space was measured against the wrong
//! physical extent, so every distribution value was short by exactly the difference between the two —
//! which is why `center` was wrong by half of it and `end` by all of it. A single arithmetic tell
//! across three different alignment values.
//!
//! ⚠ `a6` is the row that says the INLINE half was already right, so this is a missing HALF and not a
//! missing transposition. ⚠ `a8` is the auto-width row: an indefinite block size must stay
//! indefinite — reading `width: auto` as a definite zero would collapse the container instead of
//! letting it size to content.
//!
//! ⚠ `a7` is FLEX rather than grid — `flex-direction: column` in `vertical-lr` puts the MAIN axis on
//! the block axis, so `justify-content` distributes over the same 400px width. It is why the number
//! moved in two areas at once: `css/css-grid` **−137** and `css/css-flexbox` **−137** failing
//! subtests, from one predicate.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><meta charset=utf-8><style>body{margin:0}
.g{display:grid;position:relative;width:400px;height:300px;grid-auto-columns:20px;grid-auto-rows:40px;
   grid-template-columns:20px;background:#eee}
.i{background:green}
</style></head><body>
<div class="g" id="a1" style="writing-mode:vertical-lr;align-content:space-between"><div class=i></div><div class=i></div><div class=i id="k1"></div></div>
<div class="g" id="a2" style="writing-mode:vertical-lr;align-content:center"><div class=i></div><div class=i id="k2"></div></div>
<div class="g" id="a3" style="writing-mode:vertical-lr;align-content:end"><div class=i></div><div class=i id="k3"></div></div>
<div class="g" id="a4" style="align-content:space-between"><div class=i></div><div class=i></div><div class=i id="k4"></div></div>
<div class="g" id="a5" style="writing-mode:vertical-rl;align-content:space-between"><div class=i></div><div class=i></div><div class=i id="k5"></div></div>
<div class="g" id="a6" style="writing-mode:vertical-lr;justify-content:space-between"><div class=i></div><div class=i></div><div class=i id="k6"></div></div>
<div class="g" id="a7" style="display:flex;writing-mode:vertical-lr;flex-direction:column;justify-content:space-between"><div class=i style="width:40px;height:20px"></div><div class=i id="k7" style="width:40px;height:20px"></div></div>
<div class="g" id="a8" style="writing-mode:vertical-lr;width:auto;align-content:space-between"><div class=i></div><div class=i id="k8"></div></div>
<div id="out">-</div>
<script>
function o(c,k){var a=document.getElementById(c).getBoundingClientRect(),b=document.getElementById(k).getBoundingClientRect();
 return Math.round(b.left-a.left)+','+Math.round(b.top-a.top);}
document.getElementById('out').textContent=
 'a1='+o('a1','k1')+' a2='+o('a2','k2')+' a3='+o('a3','k3')+' a4='+o('a4','k4')+' a5='+o('a5','k5')+' a6='+o('a6','k6')+' a7='+o('a7','k7')+' a8='+o('a8','k8');
</script></body></html>"##;

#[test]
fn a_containers_block_extent_is_its_logical_one() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://blockextent.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("BLOCK EXTENT: {got}");

    // ── VACUITY. The horizontal case must already be right, or these rows are measuring whether
    //    `align-content` distributes at all rather than which EXTENT it distributes over.
    assert!(
        got.contains("a4=0,260"),
        "VACUOUS: `align-content: space-between` is not Chrome-exact in a horizontal writing mode, \
         so the vertical rows below are not measuring the extent — got {got:?}"
    );

    for (claim, why) in [
        ("a1=360,0", "⭐ THE MECHANISM. In `vertical-lr` the BLOCK axis is physically horizontal, so three 40px rows distribute across the container's 400px WIDTH: the last one starts at 360. Reading 260 is `300 − 40` — the free space measured against the physical height."),
        ("a2=200,0", "`center` is wrong by HALF the difference between the two extents, which is what says the error is in the EXTENT and not in the alignment value."),
        ("a3=360,0", "`end` is wrong by all of it — the third arithmetic witness for one cause."),
        ("a5=0,0", "`vertical-rl` runs its block axis right-to-left, so `space-between` puts the last row at the LEFT edge. The pair a1/a5 is what stops a fix that adds a constant instead of using the right extent."),
        ("a7=360,0", "⭐⭐ A FLEX container, not a grid: `flex-direction: column` in `vertical-lr` puts the MAIN axis on the block axis, so `justify-content` distributes over the same 400px width. One predicate, two formatting contexts, and the reason this moved `css/css-grid` and `css/css-flexbox` by the same 137 subtests."),
        ("a4=0,260", "CONTROL — the horizontal case, which was always right."),
        ("a6=80,0", "⚠ CONTROL — `justify-content` in the SAME vertical container is unmoved: the INLINE size was already being transposed correctly. This is a missing HALF, not a missing transposition."),
        ("a8=40,0", "⚠ CONTROL — `width: auto` on a vertical container. An indefinite block size must stay indefinite; reading it as a definite zero would collapse the container rather than let it size to its content."),
    ] {
        assert!(
            got.contains(claim),
            "G_BLOCK_EXTENT_IS_THE_LOGICAL_ONE: expected `{claim}` (Chrome-measured) — {why}\n  got: {got}"
        );
    }
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// X1  read `height` unconditionally (the pre-tick state)
//       -> a1, a2, a3, a5, a7 and a8 all distribute over the physical height; a4 and a6 stay green —
//          the horizontal case and the INLINE axis of the same vertical container.
// X2  read `width` unconditionally, in every writing mode
//       -> a4 alone fails — the horizontal control, distributing over 400 instead of 300.
// X3  key on `writing_mode.is_rl()` instead of `is_vertical()`
//       -> every `vertical-lr` row fails while `vertical-rl` (a5) passes: `vertical-lr` IS vertical
//          and is NOT rl, which is precisely the pair that separates the two predicates.
