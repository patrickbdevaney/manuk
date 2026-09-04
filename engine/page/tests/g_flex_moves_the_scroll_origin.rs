//! **G_FLEX_MOVES_THE_SCROLL_ORIGIN — `flex-direction: *-reverse` and `flex-wrap: wrap-reverse` each
//! move the scroll origin, and which physical axis they move it on depends on the writing mode.**
//!
//! t1427 keyed the unreachable scrollable overflow region on `direction` and `writing-mode`. A flex
//! container moves its own origin on top of that: the first item still starts AT the origin, so
//! reversing where the first item goes moves the origin with it. `*-reverse` reverses the MAIN axis
//! and `wrap-reverse` the CROSS one — **opposite axes for the same `main_is_vertical`**, which is
//! what a fix that flips one axis for both gets wrong.
//!
//! ```text
//!   main_is_vertical = (flex-direction is a ROW) == (writing mode is VERTICAL)
//!   *-reverse     flips  y_at_end if main_is_vertical else x_at_end
//!   wrap-reverse  flips  x_at_end if main_is_vertical else y_at_end
//! ```
//!
//! Headless Chrome 145, `cssom-view/scrollWidthHeight-negative-margin-002`'s wrapper (four different
//! borders, four different paddings) with `display:flex` and a `300x300` child at `margin:-100px`:
//!
//! ```text
//!                                   chrome sw / sh    before
//!   row          nowrap                196 / 201     196 / 201   CONTROL — no reversal
//!   row-reverse  nowrap                184 / 201     196 / 201   ← the MAIN axis, physically x
//!   column-reverse nowrap              216 / 188     216 / 201   ← the MAIN axis, physically y
//! ```
//!
//! ⚠⚠ **THE `wrap-reverse` HALF IS MEASURED AND DELIBERATELY NOT GATED HERE, WITH ITS RECEIPT.** It
//! is implemented and it is worth **+16 `css/css-overflow` subtests** (587 with it, 571 without,
//! same binary both ways), but our flex layout does not yet place a `wrap-reverse` line at the far
//! CROSS end at all — Chrome puts the item at `k=[-50,-150]` where we put it at `[-50,-50]` — so
//! every Chrome number for those rows is a statement about the layout gap, not about the origin.
//! Gating them would bank a number that the flex layout fix will have to move. *A gate that names
//! what it cannot catch beats one that pretends* (t1417).

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
 body { margin:0 }
 .b { width:80px; height:80px; border:1px solid; padding:1px 4px 8px 16px;
      border-width:1px 2px 3px 4px; border-right-width:50px; border-bottom-width:40px;
      display:flex; overflow:scroll; scrollbar-width:none }
 .bk { margin:-100px; height:300px; width:300px }
</style></head><body>
<div class="b" id="r" style="flex-direction:row;flex-wrap:nowrap"><div class="bk"></div></div>
<div class="b" id="rr" style="flex-direction:row-reverse;flex-wrap:nowrap"><div class="bk"></div></div>
<div class="b" id="cr" style="flex-direction:column-reverse;flex-wrap:nowrap"><div class="bk"></div></div>
<div id="out">-</div>
<script>var ids=["r","rr","cr"];
document.getElementById('out').textContent=ids.map(function(x){var e=document.getElementById(x);
return x+'='+e.scrollWidth+'/'+e.scrollHeight+'/'+e.clientWidth+'/'+e.clientHeight;}).join(' ');</script>
</body></html>"##;

#[test]
fn a_reversed_flex_axis_moves_the_scroll_origin_with_it() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://fo.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("FLEX SCROLL ORIGIN: {got}");

    // ── VACUITY. The un-reversed row must already be Chrome-exact, or these rows are about the
    //    negative-margin extent rather than about the origin.
    assert!(
        got.contains("r=196/201/100/89"),
        "VACUOUS: the un-reversed CONTROL is not Chrome-exact, so the reversals below are not what \
         is being measured — got {got:?}"
    );

    for (claim, why) in [
        ("rr=184/201/100/89", "⭐ `flex-direction: row-reverse` in a horizontal writing mode reverses the MAIN axis, which is physically x: the origin moves to the right edge and the start overflow becomes reachable — 184, not the 196 an un-reversed row gives. scrollHeight must NOT move."),
        ("cr=216/188/100/89", "⭐ `column-reverse` reverses the same MAIN axis, which is now physically y: 188, not 201 — and scrollWidth must NOT move. The pair is what makes this `main_is_vertical` rather than a fixed axis."),
    ] {
        assert!(
            got.contains(claim),
            "G_FLEX_MOVES_THE_SCROLL_ORIGIN: expected `{claim}` (Chrome-measured) — {why}\n  got: {got}"
        );
    }
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// N1  drop the `*-reverse` flip (the pre-tick state)
//       -> rr 196/201 and cr 216/201; the CONTROL stays green, which identifies the mechanism as the
//          REVERSAL and not the extent.
// N2  flip x for `*-reverse` regardless of `main_is_vertical`
//       -> cr reads 204/201 — the flip lands on the wrong physical axis, and only the column row
//          can see it.
