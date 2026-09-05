//! **G_FLEX_WRAP_REVERSE_RTL_ORDER — the `wrap-reverse` overflow shift has to run BEFORE the RTL
//! mirror, and t1437 ran it after.**
//!
//! The shift t1437 added (`Ctx::shift_wrap_reverse_overflow`) is expressed in **taffy's own
//! un-mirrored logical space**, where a reversed cross axis always overflows toward NEGATIVE. The RTL
//! mirror (`mirror_rtl_inline`, which exists because taffy has no `direction`) then carries it to the
//! correct physical side for free. Running the shift *after* the mirror subtracts on an axis that has
//! already been flipped.
//!
//! ⭐ **It only goes wrong when BOTH reversals are present**, which is why it survived t1437's
//! ten-row fixture and why it is the last ten rows of
//! `cssom-view/scrollWidthHeight-negative-margin-002`. `direction: rtl` puts a COLUMN flex's cross
//! start at the RIGHT edge and `wrap-reverse` flips it back to the LEFT, so the two together are the
//! un-reversed case and the box must not move at all — `80 − (−220) − 300 = 0` is the mirror doing
//! exactly that arithmetic once the shift is on the side of it that speaks taffy's coordinates.
//!
//! Chrome-measured (`google-chrome --headless --dump-dom`), 80x80 boxes, item offset relative to its
//! container:
//!
//! ```text
//!                                                            Chrome    before    after
//!   e6  column, rtl, wrap-reverse, 300px-wide item             0,0     -220,0     0,0
//!   e5  column, rtl, `wrap`, 300px-wide item     CONTROL      -220,0   -220,0    -220,0  ✓
//!   e9  column, LTR, wrap-reverse, 300px item    CONTROL      -220,0   -220,0    -220,0  ✓
//!   ea  row,    rtl, wrap-reverse, 300px-tall    CONTROL      60,-220  60,-220   60,-220 ✓
//!   e2  column, rtl, 50px item (FITS)            CONTROL      30,0     30,0      30,0    ✓
//!   e3  column, rtl, wrap-reverse, 50px (FITS)   CONTROL      0,0      0,0       0,0     ✓
//!   e4  column, LTR, wrap-reverse, 50px (FITS)   CONTROL      30,0     30,0      30,0    ✓
//!   e1  column, LTR, 50px item                   CONTROL      0,0      0,0       0,0     ✓
//!   e7  column, rtl, wrap, TWO 30px lines        CONTROL      50,0/10,0  same    same    ✓
//!   e8  row,    rtl, 20px item                   CONTROL      60,0     60,0      60,0    ✓
//! ```
//!
//! ⭐⭐ **`e5`/`e9`/`ea` are the three controls that make the ordering the ONLY explanation.** Each
//! has exactly one of the two reversals — `rtl` without `wrap-reverse`, `wrap-reverse` without `rtl`,
//! and `wrap-reverse` + `rtl` on a `row` container whose cross axis the mirror does not touch — and
//! all three are Chrome-exact before and after. Only the row with BOTH moves.
//!
//! ⚠ `e2`/`e3`/`e4` are the FITTING twins of the same four combinations: the shift is the negative
//! free space, so a line that fits must be untouched no matter how many reversals are stacked on it.
//! They were Chrome-exact before this tick too — the RTL cross-start rule itself was already right,
//! and only its interaction with the overflow shift was not.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><meta charset=utf-8><style>body{margin:0}
.b{overflow:hidden;scrollbar-width:none;display:flex;width:80px;height:80px}</style></head><body>
<!-- column flex: cross axis is the INLINE axis; rtl should start it at the RIGHT -->
<div class="b" id="e1" style="flex-direction:column"><div id="x1" style="width:50px;height:20px"></div></div>
<div class="b" id="e2" style="flex-direction:column;direction:rtl"><div id="x2" style="width:50px;height:20px"></div></div>
<div class="b" id="e3" style="flex-direction:column;direction:rtl;flex-wrap:wrap-reverse"><div id="x3" style="width:50px;height:20px"></div></div>
<div class="b" id="e4" style="flex-direction:column;flex-wrap:wrap-reverse"><div id="x4" style="width:50px;height:20px"></div></div>
<!-- overflowing cross axis -->
<div class="b" id="e5" style="flex-direction:column;direction:rtl"><div id="x5" style="width:300px;height:20px"></div></div>
<div class="b" id="e6" style="flex-direction:column;direction:rtl;flex-wrap:wrap-reverse"><div id="x6" style="width:300px;height:20px"></div></div>
<!-- two lines, column + rtl -->
<div class="b" id="e7" style="flex-direction:column;direction:rtl;flex-wrap:wrap"><div id="x7" style="width:30px;height:60px"></div><div id="y7" style="width:30px;height:60px"></div></div>
<!-- row flex controls: cross is the BLOCK axis, rtl must not move it -->
<div class="b" id="e8" style="direction:rtl"><div id="x8" style="width:20px;height:50px"></div></div>
<div class="b" id="e9" style="flex-direction:column;flex-wrap:wrap-reverse"><div id="x9" style="width:300px;height:20px"></div></div>
<div class="b" id="ea" style="direction:rtl;flex-wrap:wrap-reverse"><div id="xa" style="width:20px;height:300px"></div></div>
<div id="out">-</div>
<script>
function r(c,k){var a=document.getElementById(c),b=document.getElementById(k);
 return (b.offsetLeft-a.offsetLeft)+','+(b.offsetTop-a.offsetTop);}
document.getElementById('out').textContent=
 'e1='+r('e1','x1')+' e2='+r('e2','x2')+' e3='+r('e3','x3')+' e4='+r('e4','x4')
 +' e5='+r('e5','x5')+' e6='+r('e6','x6')+' e7='+r('e7','x7')+'/'+r('e7','y7')+' e8='+r('e8','x8')+' e9='+r('e9','x9')+' ea='+r('ea','xa');
</script></body></html>"##;

#[test]
fn the_wrap_reverse_shift_runs_in_taffys_coordinates_not_the_mirrored_ones() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://fo.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("WRAP REVERSE RTL ORDER: {got}");

    // ── VACUITY. Each reversal ON ITS OWN must already be right, or `e6` below is measuring one of
    //    them rather than their COMPOSITION.
    assert!(
        got.contains("e5=-220,0") && got.contains("e9=-220,0"),
        "VACUOUS: `rtl` alone or `wrap-reverse` alone is not Chrome-exact, so the both-reversals row \
         is not measuring the ordering this gate is named for — got {got:?}"
    );

    for (claim, why) in [
        ("e6=0,0", "⭐ THE MECHANISM. `direction: rtl` puts a COLUMN flex's cross start at the RIGHT edge and `wrap-reverse` flips it back to the LEFT — the two together are the un-reversed case, so an overflowing item must not move at all. Reading −220 is the shift being applied on an axis the mirror had already flipped."),
        ("e5=-220,0", "CONTROL — `rtl` WITHOUT `wrap-reverse`: the cross start really is the right edge and the item really does overflow left."),
        ("e9=-220,0", "CONTROL — `wrap-reverse` WITHOUT `rtl`: the shift alone, unchanged."),
        ("ea=60,-220", "CONTROL — both reversals on a ROW container, whose cross axis is the block axis and which the inline mirror therefore does not touch. This is what separates 'the ordering' from 'rtl and wrap-reverse cancel'."),
        ("e2=30,0", "CONTROL — the FITTING twin of e5. A line that fits is untouched however many reversals are stacked on it."),
        ("e3=0,0", "CONTROL — the fitting twin of e6."),
        ("e4=30,0", "CONTROL — the fitting twin of e9."),
        ("e1=0,0", "CONTROL — neither reversal."),
        ("e7=50,0/10,0", "CONTROL — TWO lines under `rtl` + `wrap`: the RTL cross-start rule for a column flex was ALREADY right, and this row is what says so."),
        ("e8=60,0", "CONTROL — a ROW flex under `rtl`: the cross axis is the block axis, so `direction` must not move it at all."),
    ] {
        assert!(
            got.contains(claim),
            "G_FLEX_WRAP_REVERSE_RTL_ORDER: expected `{claim}` (Chrome-measured) — {why}\n  got: {got}"
        );
    }
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// Q1  run the shift AFTER the RTL mirror again (the t1437 order)
//       -> e6 alone reads -220,0. All nine controls stay green, which is what makes this an ORDERING
//          defect rather than a rule defect: nothing about either reversal changes, only their
//          composition.
// Q2  delete the shift entirely
//       -> e9 and ea lose it, and e6 reads -220,0 as well — the RTL mirror alone puts it there. So
//          the correct answer for e6 is NOT "no shift": it is a shift that the mirror then undoes,
//          and only running the two in the right order produces it. Q1 and Q2 disagree about
//          everything except e6, which is the row both get wrong for opposite reasons.
