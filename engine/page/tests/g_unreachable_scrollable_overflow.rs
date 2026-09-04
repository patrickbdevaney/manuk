//! **G_UNREACHABLE_SCROLLABLE_OVERFLOW — a scroll container can only be scrolled AWAY from its
//! scroll origin, and which side that is was hard-coded to `horizontal-tb` + `ltr`.**
//!
//! CSS Overflow 3 §*unreachable scrollable overflow region*: overflow on the scroll-origin side
//! cannot be reached and is not in `scrollWidth`/`scrollHeight`. Our extent expressed that as
//! `.max(0.0)` on the maxima — which is the origin at the TOP-LEFT, and nothing else.
//!
//! Headless Chrome 145, `100x200; overflow:scroll; scrollbar-width:none` around a `100x200` child at
//! `transform: translate(-3px,-6px) scale(1.10)`. ⭐ **The child's rect is IDENTICAL in all six
//! rows** (`[-8,-16,102,204]`, verified in the fixture), so nothing about layout varies and only the
//! ORIGIN moves:
//!
//! ```text
//!                          chrome sw / sh   before      origin
//!   ltr  horizontal-tb        102 / 204    102 / 204    top-left      CONTROL
//!   ltr  vertical-lr          102 / 204    102 / 204    top-left      CONTROL
//!   ltr  vertical-rl          108 / 204    102 / 204    top-RIGHT
//!   rtl  horizontal-tb        108 / 204    102 / 204    top-RIGHT
//!   rtl  vertical-lr          102 / 216    102 / 204    BOTTOM-left
//!   rtl  vertical-rl          108 / 216    102 / 204    BOTTOM-RIGHT
//! ```
//!
//! `108 = 100 + |−8|` and `216 = 200 + |−16|`: with the origin at the far edge the START overflow
//! becomes reachable and is added to the padding box, and the END overflow stops counting. The x
//! origin is at the end edge when the INLINE axis runs that way (`rtl` in a horizontal mode) or when
//! the BLOCK axis does (`vertical-rl`); the y origin is at the end edge when the inline axis runs
//! bottom-to-top, which is `rtl` in a vertical mode.
//!
//! ⭐⭐ **THE TWO `ltr` CONTROLS ARE WHAT MAKE THIS A RULE ABOUT THE ORIGIN.** A fix that simply
//! added the start overflow to every container would pass the four flipped rows and turn both
//! controls into 110 / 220.
//!
//! ⚠ `scrollbar-width: none` on the fixture on purpose: this engine reserves a scrollbar gutter and
//! Chrome was measured with `--hide-scrollbars`, so a fixture with a gutter would be comparing two
//! scrollbar policies rather than two scrolling areas.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
 body { margin:0 }
 .w { width:100px; height:200px; overflow:scroll; scrollbar-width:none; display:flow-root }
 .k { width:100px; height:200px; transform:translate(-3px,-6px) scale(1.10) }
 /* ⭐ THE BORDERED ROWS. Every `.w` above has a ZERO border, so the border-box→padding-box
    conversion of the START edges is multiplied by nothing and its SIGN is invisible. This wrapper is
    `cssom-view/scrollWidthHeight-negative-margin-002`'s, whose borders and padding are asymmetric on
    every side on purpose. */
 .b { width:80px; height:80px; border:1px solid; padding:1px 4px 8px 16px;
      border-width:1px 2px 3px 4px; border-right-width:50px; border-bottom-width:40px;
      display:flow-root; overflow:scroll; scrollbar-width:none }
 .bk { margin:-100px; height:300px; width:300px }
</style></head><body>
<div class="w" id="a" style="direction:ltr;writing-mode:horizontal-tb"><div class="k"></div></div>
<div class="w" id="b" style="direction:ltr;writing-mode:vertical-lr"><div class="k"></div></div>
<div class="w" id="c" style="direction:ltr;writing-mode:vertical-rl"><div class="k"></div></div>
<div class="w" id="d" style="direction:rtl;writing-mode:horizontal-tb"><div class="k"></div></div>
<div class="w" id="e" style="direction:rtl;writing-mode:vertical-lr"><div class="k"></div></div>
<div class="w" id="f" style="direction:rtl;writing-mode:vertical-rl"><div class="k"></div></div>
<div class="b" id="g" style="direction:ltr"><div class="bk"></div></div>
<div class="b" id="h" style="direction:rtl"><div class="bk"></div></div>
<div class="b" id="i" style="direction:ltr;writing-mode:vertical-rl"><div class="bk"></div></div>
<div class="b" id="j" style="direction:rtl"><div style="width:20px;height:20px"></div></div>
<div class="b" id="k" style="direction:ltr;writing-mode:vertical-rl"><div style="width:20px;height:20px"></div></div>
<div id="out">-</div>
<script>var ids=["a","b","c","d","e","f","g","h","i","j","k"];
document.getElementById('out').textContent=ids.map(function(x){var e=document.getElementById(x);
var r=e.getBoundingClientRect(),k=e.firstElementChild.getBoundingClientRect();
return x+'='+e.scrollWidth+'/'+e.scrollHeight+'/'+e.clientWidth+'/'+e.clientHeight
 +'/k['+Math.round(k.left-r.left)+','+Math.round(k.top-r.top)+','+Math.round(k.right-r.left)+','+Math.round(k.bottom-r.top)+']';}).join(' ');</script>
</body></html>"##;

#[test]
fn the_unreachable_side_of_the_scrolling_area_follows_the_scroll_origin() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://ur.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("UNREACHABLE SCROLLABLE OVERFLOW: {got}");

    // ── VACUITY, AND IT IS THE WHOLE POINT OF THIS GATE. The transformed child must land on the
    //    SAME physical rect in all six rows; if it does not, the numbers below are a statement about
    //    layout and not about the scroll origin. (t1426 is what made this true — before it, the
    //    vertical rows put the child somewhere else entirely.)
    for id in ["a", "b", "c", "d", "e", "f"] {
        assert!(
            got.contains(&format!("/k[-8,-16,102,204]")) && got.contains(&format!("{id}=")),
            "VACUOUS: #{id} is missing, or a row's child rect is not the Chrome-exact \
             [-8,-16,102,204] that every row must share — got {got:?}"
        );
    }

    for (claim, why) in [
        ("a=102/204/100/200", "CONTROL — ltr horizontal-tb, the origin at the top-left: the START overflow (-8, -16) is UNREACHABLE and only the end shows. This is the row the old `.max(0.0)` hard-coded."),
        ("b=102/204/100/200", "CONTROL — ltr vertical-lr also has its origin at the top-left, so a fix keyed on 'is this vertical?' rather than on the DIRECTIONS breaks here."),
        ("c=108/204/100/200", "⭐ vertical-rl puts the BLOCK axis right-to-left, so the x origin is the RIGHT edge: 100 + |−8| = 108, and the +2 of end overflow stops counting."),
        ("d=108/204/100/200", "⭐ rtl in a horizontal mode moves the same origin by the INLINE axis instead — the same 108 by a different route, which is why the rule is about the ORIGIN and not about `writing-mode`."),
        ("e=102/216/100/200", "⭐ rtl in a VERTICAL mode runs the inline axis bottom-to-top, so it is the Y origin that moves: 200 + |−16| = 216, while x stays at 102."),
        ("f=108/216/100/200", "⭐ both at once — the only row where both axes flip, and the one that catches a fix that flips them together."),
        // ── ⭐⭐⭐ THE BORDERED ROWS, AND THEY EXIST BECAUSE THIS GATE MISSED A SIGN.
        //    Every row above has a ZERO border, so converting the START edges from border-box to
        //    padding-box coordinates multiplies by nothing and `+ bw.left` and `- bw.left` agree.
        //    They do not agree here: `#h` reads 196 with the `+` this gate shipped at t1427 and 204
        //    with the `-` Chrome measures. *A fixture with a zero in the term cannot see the term's
        //    sign* — the third time this session a fixture certified a bug (`width:0` at t1424, a
        //    symmetric `scale()` at t1426, a zero border here).
        ("g=216/201/100/89", "CONTROL — ltr, origin at the left: the child's right edge at 216 wins over the 100px padding box, and the −104 of start overflow is unreachable."),
        ("h=204/201/100/89", "⭐ THE SIGN. rtl moves the x origin to the right, so the START overflow becomes reachable: 100 + |−104| = 204. With the border added instead of subtracted this reads 196."),
        ("i=204/201/100/89", "⭐ and vertical-rl reaches the same 204 by the BLOCK axis instead of the inline one, on a wrapper whose four borders and four paddings are all different."),
        // ── ⭐⭐ THE ZERO ROWS: A FLIPPED ORIGIN, A BORDER, AND NOTHING OVERFLOWING BACKWARDS.
        //    Every flipped row above HAS start overflow, so a start accumulator seeded at the
        //    BORDER-box origin instead of the padding-box one produces exactly `border-left` pixels
        //    of overflow that is not there and no row notices. These two notice: Chrome reports the
        //    client box, and a zero-seeded accumulator reports client + 4. It is
        //    `css-overflow/overflow-outside-padding`'s assertion in one line — *"blocks wholly
        //    outside padding edges should not contribute to overflow"* — on a container whose left
        //    border is not zero.
        ("j=100/89/100/89", "⭐ rtl, a 20px child that fits: the origin is flipped but there IS no start overflow, so scrollWidth is exactly the client box."),
        ("k=100/89/100/89", "⭐ and the vertical-rl twin — the same zero by the block axis."),
    ] {
        assert!(
            got.contains(claim),
            "G_UNREACHABLE_SCROLLABLE_OVERFLOW: expected `{claim}` (Chrome-measured) — {why}\n  got: {got}"
        );
    }
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// N1  never flip (the pre-tick state: `.max(0.0)` for both axes always)
//       -> c, d, e and f all read 102/204; both CONTROLS stay green.
// N2  flip x whenever the mode is VERTICAL (rather than on `is_rl()`)
//       -> b reads 108 against Chrome's 102 — the control that costs a "vertical means mirrored"
//          shortcut its pass.
// N3  flip both axes together whenever either flips
//       -> c and d read 108/216 against 108/204, and e reads 108/216 against 102/216.
// N4  add the start overflow WITHOUT dropping the end (`client + |min| + max`)
//       -> c reads 110 against 108.
