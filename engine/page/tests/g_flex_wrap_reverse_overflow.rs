//! **G_FLEX_WRAP_REVERSE_OVERFLOW — `flex-wrap: wrap-reverse` packs OVERFLOWING lines from the
//! wrong edge, and only when `align-content` is the default.**
//!
//! With `wrap-reverse` the cross axis is reversed, so the lines' start edge is the container's
//! PHYSICAL end. When the lines together are larger than the container, CSS Box Alignment §5.3 makes
//! `stretch` (and `space-between`) behave as `flex-start` — which, reversed, anchors the lines at
//! that physical end and lets them overflow **backwards**. Taffy packs them forwards from the
//! physical start.
//!
//! ⭐⭐ **A GENERAL MIRROR WOULD HAVE BROKEN EVERYTHING THAT WAS ALREADY RIGHT.** Taffy's
//! `wrap-reverse` gets line ORDER, the fitting cases and every explicit `align-content` exactly
//! right, including multi-line. Chrome-measured, 80x80 boxes, item offset relative to the container:
//!
//! ```text
//!                                                  Chrome     before      after
//!   c1  one line, 40px item (FITS)                   0,40       0,40       0,40   ✓ untouched
//!   c2  one line, 80px item (EXACT)                  0,0        0,0        0,0    ✓ untouched
//!   c3  one line, 100px item                         0,-20      0,0        0,-20
//!   c4  one line, 300px item                         0,-220     0,0        0,-220
//!   c5  TWO lines, 60+60 in 80                    0,20/0,-40  0,60/0,0  0,20/0,-40
//!   c7  align-content:flex-start        CONTROL      0,-220     0,-220     0,-220 ✓
//!   d2  align-content:center            CONTROL      0,-110     0,-110     0,-110 ✓
//!   d3  align-content:flex-end          CONTROL      0,0        0,0        0,0    ✓
//!   d5  align-content:space-around      CONTROL      0,0        0,0        0,0    ✓
//!   d1  align-content:space-between               0,20/0,-40  0,60/0,0  0,20/0,-40
//!   c8  COLUMN wrap-reverse (cross is x)            -220,0      0,0       -220,0
//!   w3  TWO lines that FIT             CONTROL     0,60/0,15  0,60/0,15  0,60/0,15 ✓
//!   w4  the same two lines, `wrap`     CONTROL     0,0/0,35   0,0/0,35   0,0/0,35  ✓
//!   m1  300px item with margin:-100px 0             0,-120     0,-100     0,-120
//!   ap  in-flow 300px + an ABSPOS child           -220 / 70   0 / 70    -220 / 70
//! ```
//!
//! ⚠⚠⚠ **`m1` IS THE ROW THAT COST 80 SUBTESTS.** The shift is the negative free space, and free
//! space is measured on **MARGIN boxes** — flex lines are packed by margin box. Every hand-written
//! row above has zero cross-axis margins, so border box and margin box agreed in all of them; the
//! first version measured `slot + size`, was exact on every one, and read **80 subtests WORSE** on
//! `cssom-view/scrollWidthHeight-negative-margin-002`, whose item carries `margin: -100px`. *A
//! fixture with a zero in a term cannot see that term.*
//!
//! ⚠⚠ **`ap` IS THE OTHER ONE.** An out-of-flow child is not in a flex line (Flexbox §4.1 takes it
//! out of the formatting context and leaves taffy's slot as its STATIC POSITION only), so the
//! line-packing overflow must not move it — Chrome keeps it at 70 while the in-flow item goes to
//! −220. Shifting it too cost three subtests across the `flex-abspos-staticpos-align-self-*` family.
//!
//! ⚠ Taffy is a sanctioned dependency and is never patched (CONSTITUTION I2), so this lands on the
//! placed slots on the way OUT — the same seam as `mirror_rtl_inline`, which exists because taffy has
//! no `direction` either.
//!
//! ⚠ NAMED RESIDUE, measured and NOT fixed: the same overflow in a VERTICAL writing mode
//! (`writing-mode: vertical-lr`, a `row` flex whose cross axis is physical x) reads Chrome **−220**
//! and ours **0**, before this tick and after it. The predicate resolves the axis correctly; the
//! slots in an orthogonal subtree are not in the space this correction assumes.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><meta charset=utf-8><style>body{margin:0}
.b{width:80px;height:80px;overflow:hidden;scrollbar-width:none;display:flex;flex-wrap:wrap-reverse}</style></head><body>
<div class="b" id="c1"><div id="i1" style="width:50px;height:40px"></div></div>
<div class="b" id="c2"><div id="i2" style="width:50px;height:80px"></div></div>
<div class="b" id="c3"><div id="i3" style="width:50px;height:100px"></div></div>
<div class="b" id="c4"><div id="i4" style="width:50px;height:300px"></div></div>
<div class="b" id="c5"><div id="i5" style="width:60px;height:60px"></div><div id="j5" style="width:60px;height:60px"></div></div>
<div class="b" id="c7" style="align-content:flex-start"><div id="i7" style="width:50px;height:300px"></div></div>
<div class="b" id="c8" style="flex-direction:column"><div id="i8" style="height:50px;width:300px"></div></div>
<div class="b" id="d1" style="align-content:space-between"><div id="p1" style="width:60px;height:60px"></div><div id="q1" style="width:60px;height:60px"></div></div>
<div class="b" id="d2" style="align-content:center"><div id="p2" style="width:50px;height:300px"></div></div>
<div class="b" id="d3" style="align-content:flex-end"><div id="p3" style="width:50px;height:300px"></div></div>
<div class="b" id="d5" style="align-content:space-around"><div id="p5" style="width:50px;height:300px"></div></div>
<div class="b" id="m1"><div id="n1" style="width:50px;height:300px;margin:-100px 0"></div></div>
<div class="b" id="w4" style="flex-wrap:wrap"><div id="v4" style="width:60px;height:20px"></div><div id="u4" style="width:60px;height:30px"></div></div>
<div class="b" id="w3"><div id="v3" style="width:60px;height:20px"></div><div id="u3" style="width:60px;height:30px"></div></div>
<div class="b" id="ap"><div id="apk" style="width:50px;height:300px"></div><div id="apa" style="position:absolute;width:10px;height:10px"></div></div>
<div id="out">-</div>
<script>
function r(c,k){var a=document.getElementById(c),b=document.getElementById(k);
 return (b.offsetLeft-a.offsetLeft)+','+(b.offsetTop-a.offsetTop);}
document.getElementById('out').textContent=
 'c1='+r('c1','i1')+' c2='+r('c2','i2')+' c3='+r('c3','i3')+' c4='+r('c4','i4')
 +' c5='+r('c5','i5')+'/'+r('c5','j5')+' c7='+r('c7','i7')+' c8='+r('c8','i8')
 +' d1='+r('d1','p1')+'/'+r('d1','q1')+' d2='+r('d2','p2')+' d3='+r('d3','p3')+' d5='+r('d5','p5')
 +' m1='+r('m1','n1')+' w3='+r('w3','v3')+'/'+r('w3','u3')+' w4='+r('w4','v4')+'/'+r('w4','u4')+' ap='+r('ap','apk')+'/'+(document.getElementById('apa').getBoundingClientRect().top-document.getElementById('ap').getBoundingClientRect().top);
</script></body></html>"##;

#[test]
fn wrap_reverse_anchors_overflowing_lines_at_the_reversed_start() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://fo.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("WRAP REVERSE OVERFLOW: {got}");

    // ── VACUITY. Taffy's own `wrap-reverse` must already be right for the cases that FIT, or the
    //    overflow rows below are measuring whether wrap-reverse works at all.
    assert!(
        got.contains("w3=0,60/0,15") && got.contains("w4=0,0/0,35"),
        "VACUOUS: taffy's multi-line `wrap-reverse` is not Chrome-exact for lines that FIT, so the \
         overflow rows below are not measuring the overflow fallback — got {got:?}"
    );

    for (claim, why) in [
        ("c3=0,-20", "⭐ THE MECHANISM, smallest form: one line 20px taller than the container overflows BACKWARDS off the reversed start edge."),
        ("c4=0,-220", "the same at 220px of overflow — the shift is the negative free space, not a constant."),
        ("c5=0,20/0,-40", "TWO lines: both move together, and their ORDER (taffy's, already correct) is unchanged."),
        ("d1=0,20/0,-40", "`space-between` has the same overflow fallback as `stretch` — §5.3 sends both to flex-start."),
        ("c8=-220,0", "⭐ A `column` container's cross axis is physical X. Shifting y here would move nothing and leave the box wrong; this is why the axis is computed, not assumed."),
        ("m1=0,-120", "⚠⚠⚠ THE MARGIN-BOX ROW. `margin:-100px 0` makes the item's outer cross size 100, not 300 — free space is measured on MARGIN boxes. Measuring border boxes over-shifts and cost 80 subtests in cssom-view."),
        ("ap=0,-220/70", "⚠⚠ THE OUT-OF-FLOW ROW. The in-flow item shifts to −220; the abspos child's STATIC POSITION stays at 70, because an out-of-flow box is not in a flex line."),
        ("c1=0,40", "CONTROL — a line that FITS is untouched: the stretched line already ends at the container edge, so the arithmetic yields a zero shift with no second predicate."),
        ("c2=0,0", "CONTROL — an EXACT fit, the boundary of the previous row."),
        ("c7=0,-220", "CONTROL — explicit `align-content: flex-start` was ALREADY Chrome-exact. This row is why the fix is scoped to the default rather than written as a general mirror."),
        ("d2=0,-110", "CONTROL — `center` overflows symmetrically and was already right."),
        ("d3=0,0", "CONTROL — `flex-end`, already right."),
        ("d5=0,0", "CONTROL — `space-around`, already right."),
    ] {
        assert!(
            got.contains(claim),
            "G_FLEX_WRAP_REVERSE_OVERFLOW: expected `{claim}` (Chrome-measured) — {why}\n  got: {got}"
        );
    }
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// N1  remove the shift entirely (the pre-tick state)
//       -> c3, c4, c5, c8, d1, m1 and ap all read the un-shifted position; every CONTROL stays
//          green, which is what identifies the defect as the overflow fallback and not wrap-reverse.
// N2  measure the free space on BORDER boxes (drop the margin term)
//       -> m1 only. The other thirteen rows are unmoved, which is exactly why the first version
//          shipped green on a fixture and lost 80 subtests on the real file.
// N3  shift the out-of-flow children too
//       -> ap's second value moves off 70.
// N4  drop the `Normal | SpaceBetween` filter and shift for every align-content
//       -> d2, d3 and d5 — the three that taffy already gets right — go wrong.
// N5  force the cross axis to y
//       -> c8 reads 0,0: a column container shifted on the axis it does not overflow on.
