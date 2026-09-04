//! **G_SCROLL_AREA_OF_EVERY_ELEMENT — `scrollWidth`/`scrollHeight` are properties of every element,
//! and a plain `<div>` was answering with its BORDER box, which is the "nothing ever overflows" lie.**
//!
//! CSSOM-View defines both as *"the width/height of the element's scrolling area"* with no
//! scrollability precondition. `scroll_geometry_of` mapped only `overflow: auto|scroll|hidden`, so
//! the majority of the DOM fell through to the binding's fallback — the element's own border box.
//! `scrollHeight - clientHeight` is the *"is this overflowing?"* test every clamped-text widget,
//! tooltip placer and read-more toggle runs, and on an ordinary element it read a constant zero.
//!
//! Headless Chrome 145, one `100x50; padding:10px; border:3px` box, overflow the ONLY variable:
//!
//! ```text
//!                                          chrome sh/sw    before
//!   overflow:visible, a 20px child that FITS   70 / 120    76 / 126   ← the BORDER box
//!   overflow:visible, a 300x200 child         210 / 310    76 / 126   ← the overflow, unseen
//!   overflow:clip,    the same                210 / 310    76 / 126
//!   overflow:hidden,  the same                220 / 320   220 / 320   CONTROL, already right
//! ```
//!
//! ⭐⭐⭐ **AND `hidden` SITS WITH `scroll`, NOT WITH `clip` — 220 against 210.** The end-padding
//! inflation of CSS Overflow 3 §3.1 belongs to SCROLL CONTAINERS, and `overflow: hidden` is one
//! (programmatically scrollable) while `clip` is not. Ten pixels, and it is the whole distinction
//! between the two halves of "not visibly scrollable".
//!
//! ⚠⚠⚠ **A MAP LOOKUP DOES NOT FORCE THE REFLOW THAT A RECT READ DOES.** `layout_rect` calls
//! `force_reflow_if_stale()`; `SCROLL_GEOM` is a published snapshot. While only scroll containers
//! were mapped this was a rare staleness — the moment every element is mapped it is the COMMON path,
//! and a loop that writes a style then reads `scrollHeight` reads the pre-write layout every time.
//! The `restyle` row is that loop, in one step.
//!
//! ⭐⭐ **AND THAT REFLOW IS WHY THIS TICK IS THREE TICKS LATE.** t1425 measured this exact change at
//! **−12** on `css/css-overflow` and the ratchet refused it. The 40 rows it broke were passing on
//! STALE READS over two real defects — a transform that rode the writing-mode axis swap (t1426) and
//! an unreachable overflow region pinned to the top-left (t1427). With both fixed, the same change
//! measures **+41**. *A staleness that flatters is not a smaller bug than one that breaks.*

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
 body { margin:0 }
 .b { width:100px; height:50px; padding:10px; border:3px solid; }
</style></head><body>
<div class="b" id="v_fit" style="overflow:visible"><div style="height:20px"></div></div>
<div class="b" id="v_over" style="overflow:visible"><div style="height:200px;width:300px"></div></div>
<div class="b" id="c_over" style="overflow:clip"><div style="height:200px;width:300px"></div></div>
<div class="b" id="h_over" style="overflow:hidden"><div style="height:200px;width:300px"></div></div>
<div class="b" id="r_over" style="overflow:visible"><div id="rk" style="height:20px;width:20px"></div></div>
<span id="sp" style="border:2px solid">x</span>
<div id="out">-</div>
<script>
var ids=["v_fit","v_over","c_over","h_over","sp"];
var parts=ids.map(function(x){var e=document.getElementById(x);
  return x+'='+e.scrollHeight+'/'+e.clientHeight+'/'+e.scrollWidth+'/'+e.clientWidth;});
var r=document.getElementById('r_over');
var before=r.scrollHeight;
document.getElementById('rk').style.height='400px';
parts.push('restyle='+before+'->'+r.scrollHeight);
document.getElementById('out').textContent=parts.join(' ');
</script></body></html>"##;

#[test]
fn every_element_reports_its_scrolling_area_not_its_border_box() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://sa.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("SCROLL AREA OF EVERY ELEMENT: {got}");

    // ── VACUITY. The script has to have run at all; without this every `contains` below is a
    //    statement about the string "-".
    assert!(
        got.contains("v_fit=") && got.contains("restyle="),
        "VACUOUS: the fixture's script did not run — got {got:?}"
    );

    for (claim, why) in [
        ("v_fit=70/70/120/120", "an overflow:visible box whose child FITS reports its PADDING box, not its border box (76/126 before). scrollHeight == clientHeight, so `sh > ch` is false — the answer every overflow test wants when nothing overflows."),
        ("v_over=210/70/310/120", "⭐ THE DEFECT. The same box with a 300x200 child reports the OVERFLOW — 10 + 200 and 10 + 300. Before, it reported 76/126 and every `is this clamped?` check on the web read `false`."),
        ("c_over=210/70/310/120", "overflow:clip behaves as visible does: it is not a scroll container, so it gets no end-padding inflation."),
        ("h_over=220/70/320/120", "⭐ CONTROL, AND THE DISCRIMINATOR. overflow:hidden IS a scroll container, so the end padding DOES inflate it: 220, not 210. A fix that treats `hidden` like `clip` reads 210 here."),
        ("sp=0/0/0/0", "CONTROL — a non-replaced INLINE box has no padding box and reports 0 for BOTH pairs (Chrome; its offsetWidth/Height is 8/21). A border-box fallback answers 12/21 and defeats every `if (!el.scrollHeight)` layout guard."),
        ("restyle=70->410", "⭐ THE REFLOW. Writing a style and then reading scrollHeight must see the WRITE: 70 before, 10 + 400 after. Reading the published SCROLL_GEOM snapshot without forcing the pending reflow answers 70 twice."),
    ] {
        assert!(
            got.contains(claim),
            "G_SCROLL_AREA_OF_EVERY_ELEMENT: expected `{claim}` (Chrome-measured) — {why}\n  got: {got}"
        );
    }
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// N1  map only `auto|scroll|hidden` again (the pre-tick state)
//       -> v_fit 76/126, v_over 76/126, c_over 76/126 — the border box. h_over stays green, which
//          identifies the mechanism as the UNMAPPED elements and not the extent arithmetic.
// N2  give non-scrollable boxes the end padding too
//       -> v_over 220/320 against Chrome's 210/310, and h_over stays green.
// N3  answer the border box for a non-replaced inline
//       -> sp 21/0/12/0.
// N4  drop the `force_reflow_if_stale()` in the scroll getters
//       -> restyle=70->70.
