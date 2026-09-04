//! **G_TRANSFORM_IS_PHYSICAL_IN_VERTICAL_MODES — a `transform` is physical in every writing mode,
//! and ours rode the axis swap.**
//!
//! An orthogonal (vertical) run is laid out in a TRANSPOSED style space and mapped back to page
//! coordinates by `writing_mode::map_subtree`. `transpose_in_place` transposed the `transform-origin`
//! and **not the transform itself**, so a transform applied inside that space swapped with it.
//!
//! Headless Chrome 145, a `100x200` child of a `100x200` container, rect relative to the container:
//!
//! ```text
//!                                        chrome              before
//!   horizontal-tb  translate(-3px,-6px)  [-3,-6,97,194]   [-3,-6,97,194]   ✓ CONTROL
//!   vertical-lr    the same              [-3,-6,97,194]   [-6,-3,94,197]   ← x and y swapped
//!   vertical-rl    the same              [-3,-6,97,194]   [ 6,-3,106,197]  ← swapped AND mirrored
//!   horizontal-tb  scale(1.10)           [-5,-10,105,210] [-5,-10,105,210] ✓ CONTROL
//!   vertical-lr    scale(1.10)           [-5,-10,105,210] [-5,-10,105,210] ✓ and THIS is why it hid
//! ```
//!
//! ⭐⭐⭐ **A SYMMETRIC FUNCTION CANNOT SEE A SWAP.** Every `scale()` row was already exact, in both
//! writing modes, because a uniform scale is invariant under transposition — so any fixture built
//! from `scale` (or from `rotate` about the centre, or from a square box) reports a clean bill of
//! health for an axis map that is broken. The discriminating input is an **asymmetric** one.
//!
//! ⭐ The conjugation DISTRIBUTES over the function list (`J⁻¹ABCJ = (J⁻¹AJ)(J⁻¹BJ)(J⁻¹CJ)`), so
//! each function is transposed on its own. The map is `x_phys = f(ey)`, `y_phys = by + ex`: a swap
//! for `vertical-lr` (a reflection, determinant −1, which REVERSES a rotation) and a swap-plus-mirror
//! for `vertical-rl` (a quarter turn, determinant +1, which preserves it).
//!
//! ⚠ NAMED RESIDUE, not fixed here: `css-overflow/scrollable-overflow-transform-unreachable-region`
//! still fails its `scrollToLeft`/`scrollToTop` rows (`sw` 87 against 108, `sh` 189 against 216) —
//! that is the **unreachable scrollable overflow region**, a second mechanism, and this fix took the
//! same file's `ltr/vertical-lr` rows from wrong to exact.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
 body { margin:0 }
 .w { width:100px; height:200px; overflow:hidden; display:flow-root }
 .k { width:100px; height:200px; transform:translate(-3px,-6px) }
</style></head><body>
<div class="w" id="h" style="writing-mode:horizontal-tb"><div class="k"></div></div>
<div class="w" id="vlr" style="writing-mode:vertical-lr"><div class="k"></div></div>
<div class="w" id="vrl" style="writing-mode:vertical-rl"><div class="k"></div></div>
<div class="w" id="hs" style="writing-mode:horizontal-tb"><div class="k" style="transform:scale(1.10)"></div></div>
<div class="w" id="vlrs" style="writing-mode:vertical-lr"><div class="k" style="transform:scale(1.10)"></div></div>
<div id="out">-</div>
<script>var ids=["h","vlr","vrl","hs","vlrs"];
document.getElementById('out').textContent=ids.map(function(x){var e=document.getElementById(x);
var r=e.getBoundingClientRect(),k=e.firstElementChild.getBoundingClientRect();
return x+'=['+Math.round(k.left-r.left)+','+Math.round(k.top-r.top)+','+Math.round(k.right-r.left)+','+Math.round(k.bottom-r.top)+']';}).join(' ');</script>
</body></html>"##;

#[test]
fn a_transform_is_physical_in_every_writing_mode() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://tx.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("TRANSFORM IN VERTICAL MODES: {got}");

    // ── VACUITY. The script must have run and the CONTROL must already be right, or every row
    //    below is a statement about the string "-" or about a broken horizontal layout.
    assert!(
        got.contains("h=[-3,-6,97,194]"),
        "VACUOUS: the horizontal-tb CONTROL is not Chrome-exact, so this gate's subject (the \
         WRITING MODE) is not what is being measured — got {got:?}"
    );

    for (claim, why) in [
        ("vlr=[-3,-6,97,194]", "⭐ THE DEFECT. `translate(-3px,-6px)` is PHYSICAL: in vertical-lr it must move the box left 3 and up 6, exactly as in horizontal-tb. Ours read [-6,-3,94,197] — the run's own axis swap, applied to the transform as well as to the box."),
        ("vrl=[-3,-6,97,194]", "and vertical-rl, where the map is a quarter turn rather than a reflection: ours read [6,-3,106,197], swapped AND mirrored, so the box moved the wrong way along x."),
        ("hs=[-5,-10,105,210]", "CONTROL — a uniform scale in horizontal-tb."),
        ("vlrs=[-5,-10,105,210]", "⭐ THE CONTROL THAT EXPLAINS THE HIDING. A uniform scale is INVARIANT under transposition, so it was exact before and after. Any fixture built from `scale` gives a broken axis map a clean bill of health; only an asymmetric function discriminates."),
    ] {
        assert!(
            got.contains(claim),
            "G_TRANSFORM_IS_PHYSICAL_IN_VERTICAL_MODES: expected `{claim}` (Chrome-measured) — {why}\n  got: {got}"
        );
    }
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// N1  drop the `transpose_transform` call (the pre-tick state)
//       -> vlr [-6,-3,94,197] and vrl [6,-3,106,197]; both CONTROLS stay green, which is what
//          identifies the mechanism as the AXIS MAP and not the transform arithmetic.
// N2  transpose translate but not its SIGN under vertical-rl (`T::Translate(ty, tx)` for both)
//       -> vrl [3,-6,103,194] while vlr stays green — the mirror half of the quarter turn.
// N3  transpose the sign but not the axes (`T::Translate(tx, neg(ty))`)
//       -> vlr and vrl both wrong, controls green.
