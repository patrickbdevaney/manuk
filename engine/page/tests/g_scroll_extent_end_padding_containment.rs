//! **G_SCROLL_EXTENT_END_PADDING_CONTAINMENT — the scroll container's end padding belongs to the
//! content it CONTAINS, and it was being handed to everything.**
//!
//! t1417 fixed the negative-margin half of the scrollable-overflow walk and named this as the rule
//! underneath it: *"a grandchild that already overflows gets the container's END PADDING added on
//! top, and Chrome does not add it."* This is that rule.
//!
//! Headless Chrome 145.0.7632.116, `100x100; padding:10px; overflow:scroll` around a 100px filler,
//! at three depths — plus a fixed-height wrapper the filler overflows:
//!
//! ```text
//!                                                          chrome   before
//!   the filler is the DIRECT CHILD                           120      120     10 + 100 + 10  ✓
//!   the filler is a GRANDCHILD of a 10px-tall wrapper        110      120     ← the +10 again
//!   the filler is a GREAT-GRANDCHILD, same shape             110      120     ← and again
//!   height:30px; padding:10px > 10px wrapper > 60px box       70       80
//!   the same with padding:0                                   60       60     CONTROL
//!   the same with a 5px box that FITS                         50       50     CONTROL
//! ```
//!
//! ⚠⚠⚠ **AND THE RULE IS NOT DEPTH — THAT WAS TRIED FIRST AND AN EXISTING GATE REFUSED IT.**
//! `g_scroll_overflow_end_margin` holds a Chrome-measured counterexample that was already in the
//! tree: *"the realistic nested shape — an auto-height wrapper whose inner child carries the
//! margin"* expects **270**, and a depth rule gives 260. **A grandchild sometimes DOES get the
//! padding.**
//!
//! ⭐⭐⭐ **THE DISCRIMINATOR IS CONTAINMENT.** In that gate's fixture the wrapper is AUTO-HEIGHT: it
//! grows to contain its child, so the child is part of the scroller's in-flow content and the end
//! padding applies. In the rows above the wrapper has a FIXED height and its child overflows it — the
//! overflowing part is not the scroller's in-flow content and gets no padding. **Depth was a proxy
//! that fitted three fixtures; containment fits all of them, and the gate that refused the proxy is
//! the reason the right rule was found at all.**

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
 .s { width:100px; height:100px; overflow:scroll; padding:10px; }
 #d > .f { width:1px; height:100px; }
 #e > div { height:10px; }
 #e > div > .f { width:1px; height:100px; }
 #g > div { height:10px; }
 #g > div > div > .f { width:1px; height:100px; }
 .t { width:40px; height:30px; overflow:hidden; padding:10px; }
 .t > div { height:10px; }
 .t > div > div { height:60px; width:10px; }
 #k > div > div { height:5px; }
 #n { padding:0; }
 #w { width:100px; overflow:scroll; padding:10px; height:60px; }
 #w > div { }
 #w > div > div { height:100px; margin-bottom:30px; }
 #h { width:40px; height:40px; overflow:scroll; padding:10px; }
 #h > div { width:10px; height:10px; }
 #h > div > div { width:80px; height:5px; }
</style></head><body>
<div class="s" id="d"><div class="f"></div></div>
<div class="s" id="e"><div><div class="f"></div></div></div>
<div class="s" id="g"><div><div><div class="f"></div></div></div></div>
<div class="t" id="j"><div><div></div></div></div>
<div class="t" id="k"><div><div></div></div></div>
<div class="t" id="n"><div><div></div></div></div>
<div id="w"><div><div></div></div></div>
<div id="h"><div><div></div></div></div>
<div id="out">-</div>
<script>
function q(x){var e=document.getElementById(x);return x+'='+e.scrollHeight+'/'+e.clientHeight;}
var h=document.getElementById('h');
document.getElementById('out').textContent=['d','e','g','j','k','n','w'].map(q).join(' ')
  +' h='+h.scrollWidth+'/'+h.clientWidth;
</script></body></html>"##;

#[test]
fn the_end_padding_goes_only_to_content_the_container_contains() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://ep.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("END-PADDING CONTAINMENT: {got}");

    for (claim, why) in [
        // The rule t1258 established, and it is still right: a DIRECT child in flow gets the padding.
        ("d=120/105", "a direct child in flow: 10 + 100 + 10. This is t1258's rule and it must not move."),
        // …and a descendant that OVERFLOWS a fixed-height ancestor does not.
        ("e=110/105", "a grandchild overflowing a 10px-tall wrapper gets NO end padding (Chrome)."),
        ("g=110/105", "and one level deeper behaves identically — it is containment, not depth."),
        ("j=70/50", "the small-scroller form of the same divergence t1417 had to leave unfixed."),
        // The two controls that keep the fix from being 'never add the padding'.
        ("k=50/50", "CONTROL: content that FITS reports no overflow at all."),
        ("n=60/30", "CONTROL: with padding:0 there is no padding to add or withhold — 60 either way."),
        // ⭐ THE CONTAINMENT CONTROL. An AUTO-height wrapper CONTAINS its child, so the padding DOES
        // apply through it — the shape a depth rule got wrong and `g_scroll_overflow_end_margin`
        // refused. Without this row the fix could be "only direct children" and pass everything above.
        ("w=150/65", "⭐ an AUTO-height wrapper CONTAINS its child, so the end padding DOES reach it: \
                      10 + 100 + 30 margin + 10 padding = 150. This is the row that makes the rule \
                      CONTAINMENT rather than DEPTH. (The 65 is the client box after the scrollbar — \
                      MEASURED, after a first version guessed 40 and was refuted by Chrome.)"),
        // ⭐ THE HORIZONTAL AXIS, ADDED BECAUSE A MUTATION THAT DROPPED IT CAME BACK GREEN. Every
        // row above is vertical, so removing the `x` term from the containment test changed nothing.
        // Chrome, a `40x40; padding:10px; overflow:scroll` scroller > a 10px-wide wrapper > an 80px
        // box: `h=90/45` — 10 + 80, and NO end padding, exactly as the block axis behaves.
        ("h=90/45", "the containment test is TWO-AXIS. A mutation dropping the horizontal term passed                      every vertical row."),
    ] {
        assert!(
            got.contains(claim),
            "G_SCROLL_EXTENT_END_PADDING_CONTAINMENT: expected `{claim}` — {why}\n  got: {got}"
        );
    }
}
