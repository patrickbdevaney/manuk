//! **G_NEGATIVE_MARGIN_SCROLL_EXTENT — a grandchild sailed straight past its parent's negative
//! margin, and the scroller reported overflow that is not there.**
//!
//! Located by the t1416 concentration survey: `css/cssom-view`'s 1,480 failing subtests are not 1,480
//! defects — 769 of them are three `scrollWidthHeight-*` files, and the WPT file that names the rule
//! says it in its own title: *"scroll{Width,Height} shouldn't account for collapsed margins, in order
//! not to report unnecessary overflow."*
//!
//! Headless Chrome 145.0.7632.116, four `margin:-5px -7px` children each wrapping a 20px box, inside
//! a `width:20px; padding:10px 20px; overflow:hidden` scroller:
//!
//! ```text
//!                            chrome              before
//!   the children's rects     [13,33][28,48][43,63][58,78]   IDENTICAL — layout was never wrong
//!   clientHeight             75                  75         IDENTICAL
//!   scrollHeight             75                  80         ← 5px of overflow that does not exist
//! ```
//!
//! ⭐⭐⭐ **THE LAYOUT WAS RIGHT AND THE WALK WAS FLAT.** Every descendant was measured directly
//! against the scroll container, so the inner 20px box contributed its own bottom (78) and its
//! parent's `margin-bottom: -5px` never applied to it. The parent's margin box ends at 73 — exactly
//! the content-box bottom — so Chrome reports no overflow at all.
//!
//! ⭐ **Scoped to NEGATIVE margins on purpose.** A positive end margin genuinely extends the
//! scrollable region (t1119's rule, and `g_scroll_overflow_end_margin` holds it); widening this to
//! every margin would trade one wrong answer for another. A negative margin pulls the margin box IN,
//! and a subtree cannot report more overflow than the box containing it claims to occupy.
//!
//! **Measured, same binary both ways:** `css/cssom-view` 563 → **602** (+39). `css/css-position`
//! 1174 → 1174 (its apparent +8 against the stored row is that row drifting, not this change) and
//! `css/css-overflow` unchanged.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
 #t { width:20px; overflow:hidden; padding:10px 20px; }
 #t > div { background:green; margin:-5px -7px; }
 #t > div > div { height:20px; width:20px; }
 #p { width:20px; height:30px; overflow:hidden; padding:10px 20px; }
 #p > div { margin-bottom:5px; }
 #p > div > div { height:40px; width:20px; }
 #q { width:40px; height:30px; overflow:hidden; padding:10px; }
 #q > div { height:10px; }
 #q > div > div { height:60px; width:10px; }
</style></head><body>
<div id="t"><div><div></div></div><div><div></div></div><div><div></div></div><div><div></div></div></div>
<div id="p"><div><div></div></div></div>
<div id="q"><div><div></div></div></div>
<div id="out">-</div>
<script>
var t=document.getElementById('t'), p=document.getElementById('p'), q=document.getElementById('q');
document.getElementById('out').textContent =
  'sh='+t.scrollHeight+' ch='+t.clientHeight+
  ' kids='+Array.prototype.map.call(t.children,function(k){var r=k.getBoundingClientRect();return '['+Math.round(r.top)+','+Math.round(r.bottom)+']';}).join('')+
  ' | pos_sh='+p.scrollHeight+' pos_ch='+p.clientHeight+
  ' | gc_sh='+q.scrollHeight+' gc_ch='+q.clientHeight;
</script></body></html>"##;

#[test]
fn a_negative_end_margin_clamps_its_subtrees_scroll_contribution() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://nm.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("NEGATIVE-MARGIN SCROLL EXTENT: {got}");

    // ── 1. THE DEFECT. `scrollHeight` must equal `clientHeight`: the last child's margin box ends
    // exactly at the content-box bottom, so there is no overflow to report.
    assert!(
        got.contains("sh=75") && got.contains("ch=75"),
        "G_NEGATIVE_MARGIN_SCROLL_EXTENT: expected `sh=75 ch=75` (Chrome-measured) — got {got:?}. \
         The inner 20px box was contributing its own bottom (78) straight to the scroll container, \
         past its parent's `margin-bottom: -5px`, so the scroller claimed 5px of overflow that does \
         not exist. Every infinite scroller and every `is this overflowing?` check reads this pair."
    );

    // ── 2. THE LAYOUT CONTROL, AND IT IS WHY THIS IS AN OVERFLOW BUG AND NOT A LAYOUT ONE. The child
    // rects already matched Chrome exactly before the fix; if this arm ever fails, the defect moved
    // into layout and the fix above is measuring something else.
    assert!(
        got.contains("kids=[13,33][28,48][43,63][58,78]"),
        "CONTROL: the children's rects are Chrome-exact and were BEFORE this fix too — the layout was \
         never wrong (got {got:?}). This arm keeps the scroll fix from being credited for, or hiding, \
         a layout change."
    );

    // ── 3. THE POSITIVE-MARGIN CONTROL, AND ITS NUMBERS WERE MEASURED AFTER A WRONG GUESS. A
    // FIXED-HEIGHT scroller is required: on an auto-height one both engines answer `45/45` and the
    // arm cannot discriminate, which is what the first version of it did. Chrome, `height:30px;
    // padding:10px` around a 40px child with `margin-bottom:5px`: **`pos_sh=65 pos_ch=50`** — the
    // positive margin IS in the region. Without this arm the fix could clamp EVERY margin, pass arm
    // 1, and silently destroy the rule t1119 established (`g_scroll_overflow_end_margin`).
    assert!(
        got.contains("pos_sh=65") && got.contains("pos_ch=50"),
        "CONTROL: a POSITIVE end margin still extends the region (Chrome: `pos_sh=65 pos_ch=50`) — \
         got {got:?}. The clamp is scoped to NEGATIVE margins precisely so this stays true; widening \
         it would trade one wrong answer for another."
    );

    // ⚠⚠⚠ **THREE THINGS WRITTEN, MEASURED, AND MOVED OUT RATHER THAN SHIPPED RED — INCLUDING THE
    // ARM THIS GATE MOST WANTS.**
    //
    // 1. **A grandchild overflowing its parent, no margins at all.** Chrome: `sh=70 ch=50` for a
    //    `height:30px; padding:10px` scroller > a 10px child > a 60px box. We say **80** — we add the
    //    container's END PADDING to a descendant that already overflows, and Chrome does not. A
    //    SEPARATE pre-existing rule, and it is the next tick.
    // 2. The same shape with a POSITIVE `margin-bottom: 5px` on the parent: Chrome `70`, we `80`.
    //    Same root cause as (1).
    // 3. Because of (1), **the mutation "clamp EVERY end margin, not only negative ones" cannot be
    //    caught by this gate.** The only shape that distinguishes it is the grandchild overflow —
    //    and we are already wrong there, so an arm asserting Chrome's answer would be red for a
    //    reason this tick does not own (t1004), while an arm asserting OUR answer would pin the
    //    engine to a bug. Stated plainly instead of papered over: **this gate is red under 2 of 4
    //    mutations, and fixing (1) is what makes the other two catchable.**
}
