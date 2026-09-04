//! **G_COLLAPSED_END_MARGIN_IS_OUTSIDE — a margin that collapsed out of the container is not inside
//! it, and counting it reports overflow that is not there.**
//!
//! The WPT file that names this says it in its own title: *"scroll{Width,Height} shouldn't account
//! for collapsed margins, in order not to report unnecessary overflow"*
//! (`cssom-view/scrollWidthHeight-overflow-visible-margin-collapsing`, 140 subtests).
//!
//! A block container with **auto block-size, no block-end padding, no block-end border and no BFC**
//! lets its last in-flow child's end margin collapse straight through its own edge — so the margin
//! is OUTSIDE the container. Headless Chrome 145, two 20px children at `margin: 20px 10px`:
//!
//! ```text
//!                                          chrome sh/ch   before
//!   display:block; overflow:visible            60 / 60     80 / 60   ← the collapsed-out margin
//!   …overflow:hidden          (a BFC)         100 / 100   100 / 100  CONTROL
//!   …padding-bottom: 2px                       82 / 82     82 / 82   CONTROL
//!   …border-bottom: 3px solid                  80 / 80     80 / 80   CONTROL
//!   …height: 50px             (definite)       60 / 50     80 / 50   ← NOT a carve-out
//! ```
//!
//! ⭐⭐ **THREE CONDITIONS, EACH ITS OWN CONTROL — AND A FOURTH THAT MEASUREMENT REFUSED.** A BFC
//! keeps the margin in; end padding keeps it in; an end border keeps it in. t1119's whole battery
//! lives in the first of those — a `padding:10px 5px; overflow:scroll` scroller — and must still read
//! 270, which is why the rule could not simply be *"never add the end margin"*. A DEFINITE block-size
//! does NOT keep it in, though CSS 2.1 §8.3.1 reads as if it should: Chrome answers 60 where the
//! carve-out answers 80.
//!
//! ⚠ **AND IT IS THE BLOCK AXIS ONLY — MARGINS DO NOT COLLAPSE IN THE INLINE AXIS.** Which physical
//! edge is block-end depends on the writing mode, so the condition is asked of that edge alone. An
//! earlier version bundled `margin-right` into the same test and withheld a margin CSS never
//! collapses.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
 body { margin:0 }
 .t { width:200px }
 .t > div { height:20px; min-width:20px; margin:20px 10px }
</style></head><body>
<div class="t" id="v" style="display:block;overflow:visible"><div></div><div></div></div>
<div class="t" id="h" style="display:block;overflow:hidden"><div></div><div></div></div>
<div class="t" id="p" style="display:block;overflow:visible;padding-bottom:2px"><div></div><div></div></div>
<div class="t" id="b" style="display:block;overflow:visible;border-bottom:3px solid"><div></div><div></div></div>
<div class="t" id="f" style="display:block;overflow:visible;height:50px"><div></div><div></div></div>
<div id="out">-</div>
<script>var ids=["v","h","p","b","f"];
document.getElementById('out').textContent=ids.map(function(x){var e=document.getElementById(x);
return x+'='+e.scrollHeight+'/'+e.clientHeight;}).join(' ');</script>
</body></html>"##;

#[test]
fn a_margin_that_collapsed_out_of_the_container_is_not_inside_it() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://cm.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("COLLAPSED END MARGIN: {got}");

    // ── VACUITY. The script must have run and the four controls must already be Chrome-exact, or
    //    the subject row below is a statement about a broken layout rather than about the margin.
    assert!(
        got.contains("v=") && got.contains("f="),
        "VACUOUS: the fixture's script did not run — got {got:?}"
    );

    for (claim, why) in [
        ("v=60/60", "⭐ THE DEFECT. Auto height, no end padding, no end border, no BFC: the last child's `margin-bottom: 20px` collapses THROUGH the container's bottom edge, so it is outside the container and there is no overflow. Ours read 80 against a clientHeight of 60 — 20px of overflow that does not exist, on the commonest block on the web."),
        ("h=100/100", "CONTROL — `overflow: hidden` establishes a BFC, so the margin cannot collapse out and stays inside. This is the row that makes the rule CONDITIONAL rather than 'never add the end margin', and t1119's whole battery lives in it."),
        ("p=82/82", "CONTROL — a 2px block-end PADDING blocks the collapse, and the margin is inside again."),
        ("b=80/80", "CONTROL — so does a 3px block-end BORDER."),
        ("f=60/50", "⭐ A DEFINITE block-size does NOT bring the margin back, which is what CSS 2.1 §8.3.1 would suggest and Chrome does not do: 60, the last child's BORDER box, against the 80 a definite-size carve-out gives. Measured at a height the content EXCEEDS — the first version of this row used `height: 200px`, where the client floor is 200 and both answers agree. A control that cannot fail is not a control."),
    ] {
        assert!(
            got.contains(claim),
            "G_COLLAPSED_END_MARGIN_IS_OUTSIDE: expected `{claim}` (Chrome-measured) — {why}\n  got: {got}"
        );
    }
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// N1  drop the collapse test (the pre-tick state: always add the end margin)
//       -> v reads 80/60; all four CONTROLS stay green, which is what identifies the mechanism as
//          the COLLAPSE and not the end-margin term itself.
// N2  drop any ONE of the three conditions
//       -> that condition's control row fails and nothing else does: `overflow` → h, padding → p,
//          border → b.
// N3  ADD a definite-block-size carve-out (the CSS 2.1 §8.3.1 reading)
//       -> f reads 80/50 against Chrome's 60/50.
