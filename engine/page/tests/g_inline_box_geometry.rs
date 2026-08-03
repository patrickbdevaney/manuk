//! # G_INLINE_BOX_GEOMETRY — a non-replaced inline's box is its OWN content area, never its children's
//!
//! `<span class="icon"><i></i></span>` is one of the most-written lines on the web: an inline element
//! whose entire content is an *atomic* inline — a sprite `<i>`, an `<img>`, an icon-font glyph in an
//! `inline-block`. It has **no text of its own**, so before this gate it had no fragment of its own
//! either, and `LayoutBox::node_rects` fell back to the only geometry it could find: the child's box.
//!
//! That is not a rounding error. Chrome-measured on the fixture below (`16px/1.2 sans-serif`):
//!
//! ```text
//!                                              Chrome            ours (before)
//!   #a  <span><i 8x4 inline-block></i></span>  [11, 1, 8,17]     [11,11,8, 4]
//!   #c  <span><i 8x40 inline-block></i></span> [11,93, 8,17]     [11,70,8,40]
//!   #b  <span 10px><b 40px>x</b></span>        [11,48,22,11]     [11,21,22,44]
//! ```
//!
//! Two separate facts, and the second is the one that makes a naive fix wrong:
//!
//! 1. **An inline element that contributes content still has its own inline box** — its font's
//!    ascent + descent, sitting on the line's baseline. `#a` is 17 tall in Chrome because the *span's*
//!    font says so, not because anything in it is 17 tall.
//! 2. **That box is NOT unioned with its descendants vertically.** `#c`'s 40px icon and `#b`'s 44px
//!    `<b>` both overflow their parent inline box, and Chrome reports the parent at 17 and 11 — the
//!    parent's own metrics, unmoved. Horizontally the opposite holds: the inline box's width IS the
//!    advance of everything in it (`#a` is 8 wide, the icon's width).
//!
//! So the rule is **per axis** — the same shape as t849's static position — and getting it right in
//! one axis while unioning in the other is what produced a box 13px too short *and* 10px too low.
//!
//! ## Why this is ranked as I3 (actuation), not as a shape term
//!
//! `node_rects` is a shared producer:
//!
//! ```text
//!   LayoutBox::node_rects()  →  manuk_a11y::build_tree_with_rects  →  A11yNode.bbox  →  click point
//! ```
//!
//! The agent clicks the **centre of the bbox**. With the span reported as the 4px-tall icon box, the
//! click point for `#a` was computed 3.5px low in a box 13px too short — on the single most common
//! clickable-icon idiom there is. Ranked on M1 that is a sub-tolerance `shape` term the corpus cannot
//! price; ranked on I3 it is a mis-actuation surface. Hence the click-point assertion below, in the
//! same gate: a geometry fix to the producer must be shown to move the *actuation* number, which is
//! what five geometry ticks in a row had been getting for free (CONSTITUTION-CHECK #72).
//!
//! ## How each assertion goes RED
//!
//! - **Delete the `Spacer` reporter at the end of `collect_inline_node`** (the "carries no fragment of
//!   its own" branch) and `#a`/`#b`/`#c` collapse onto their children's boxes — 4, 44 and 40 tall.
//! - **Make `node_rects`'s `lift` union both axes again** (drop the `frags`-owner branch) and `#c`
//!   reads 40 and `#b` reads 44 while `#a` stays correct — which is why the tall-descendant rows are
//!   asserted at all: the common icon is *smaller* than its line, so a both-axes union passes `#a`.
//! - **Control `#s5`**: a span with ordinary text. It already had fragments and must not move.

use manuk_text::FontContext;

/// Chrome-measured, `--headless=new --dump-dom` + `getBoundingClientRect`, body `16px/1.2 sans-serif`:
///
/// ```text
///   #a  [11,  1.00,  8.00, 17]      #b  [11, 48.19, 22.25, 11]
///   #c  [11, 93.19,  8.00, 17]      #d  [11,137.38, 12.45, 17]
///   #e  [11,181.56,  8.00, 17]      #s5 [11, 39.38, 12.45, 17]
/// ```
///
/// Absolute `y` is not asserted (it depends on our own line-metric rounding, which is a different
/// question); the *heights*, the *widths* and the top-relative ORDER are, because those are the facts
/// the inline-box rule decides.
const HTML: &str = r##"<!DOCTYPE html><html><head><style>
 body{margin:0;font:16px/1.2 sans-serif}
 .ico{display:inline-block;width:8px;height:4px;background:#c00}
 .big{display:inline-block;width:8px;height:40px;background:#0c0}
 .pad{padding-left:11px}
</style></head><body>
<div class="pad"><span id="a"><i class="ico" id="ai"></i></span></div>
<div class="pad"><span id="b" style="font-size:10px"><b id="bb" style="font-size:40px">x</b></span></div>
<div class="pad"><span id="c"><i class="big"></i></span></div>
<div class="pad"><span id="s5">hi</span></div>
</body></html>"##;

fn rect_of(page: &manuk_page::Page, sel: &str) -> manuk_layout::Rect {
    let dom = page.dom();
    let n = manuk_css::query_selector_all(dom, dom.root(), sel)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("selector {sel} matched nothing"));
    *page
        .root_box
        .node_rects(dom)
        .get(&n)
        .unwrap_or_else(|| panic!("no box for {sel} — the element has no geometry at all"))
}

#[test]
fn g_inline_box_is_its_own_content_area() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://inline.test/", &fonts, 1200.0);

    let (a, ai, b, bb, c, s5) = (
        rect_of(&page, "#a"),
        rect_of(&page, "#ai"),
        rect_of(&page, "#b"),
        rect_of(&page, "#bb"),
        rect_of(&page, "#c"),
        rect_of(&page, "#s5"),
    );

    // ── THE CONTROL, and it is also the CALIBRATION. `#s5` is a span with ordinary text: it always
    // had its own fragments, so its height is this engine's content area for a 16px sans-serif face.
    // Chrome reads 17. Asserting the icon spans against `#s5` rather than against the literal 17
    // keeps the gate about the inline-box RULE and not about our font-metric rounding.
    assert!(
        (s5.height - 17.0).abs() < 1.01,
        "G_INLINE_BOX_GEOMETRY: control `#s5` (a span with plain text) is {}px tall; Chrome reads 17. \
         Every other row is measured against this one, so if this moved, the calibration moved and \
         the rest of this gate is comparing against the wrong ruler.",
        s5.height
    );

    // ── 1. AN ICON-WRAPPING SPAN IS ITS OWN LINE-HEIGHT TALL, NOT ITS ICON'S. The idiom.
    assert!(
        (a.height - s5.height).abs() < 1.01,
        "G_INLINE_BOX_GEOMETRY: `#a` — <span><i class=ico></i></span>, the icon-wrapper idiom — is \
         {:.2}px tall. It must be its OWN content area ({:.2}px, the same as the text span #s5), not \
         its 4px icon's box. Chrome: [11,1,8,17]. This is the agent's click target on every icon \
         button on the web.",
        a.height,
        s5.height
    );
    assert!(
        (a.width - 8.0).abs() < 1.01,
        "G_INLINE_BOX_GEOMETRY: `#a` is {:.2}px wide; the inline box's width IS the advance of its \
         content, so an 8px icon makes an 8px span. The rule is PER AXIS — height from the element's \
         own metrics, width from its contents — and a fix that took the element's own box in BOTH \
         axes reads 0 here.",
        a.width
    );
    // The icon must still be inside it — the vertical relation, not just the size.
    assert!(
        a.y <= ai.y + 0.51 && a.y + a.height >= ai.y + ai.height - 0.51,
        "G_INLINE_BOX_GEOMETRY: `#a` [{:.2},{:.2},{:.2},{:.2}] must CONTAIN its icon \
         [{:.2},{:.2},{:.2},{:.2}] — a 4px icon sits on the baseline, inside the 17px content area.",
        a.x, a.y, a.width, a.height, ai.x, ai.y, ai.width, ai.height
    );

    // ── 2. A TALLER DESCENDANT DOES NOT GROW THE INLINE BOX. This is the row a both-axes union
    // passes #a on and fails here, so it is what makes the fix a rule rather than a coincidence.
    assert!(
        (c.height - s5.height).abs() < 1.01,
        "G_INLINE_BOX_GEOMETRY: `#c` wraps a 40px-tall inline-block and reads {:.2}px. Chrome reads \
         17 — the parent inline box is its OWN font's content area and the icon simply OVERFLOWS it. \
         Unioning the child in vertically is the other half of the same bug.",
        c.height
    );
    assert!(
        (b.height - 11.0).abs() < 1.01,
        "G_INLINE_BOX_GEOMETRY: `#b` is a 10px-font span around a 40px-font <b> and reads {:.2}px. \
         Chrome reads 11 — the span's own metrics. Its child #bb is {:.2}px tall and must stay that \
         way; the two are separate inline boxes, not one union.",
        b.height,
        bb.height
    );
    assert!(
        bb.height > 30.0,
        "G_INLINE_BOX_GEOMETRY: the 40px-font <b> #bb is {:.2}px tall — Chrome reads 44. A fix that \
         made every inline take its PARENT's metrics would pass the row above and fail here.",
        bb.height
    );
}

/// ⚠ **I3 — THE CLICK POINT, ASSERTED IN THE SAME TICK AS THE GEOMETRY** (CONSTITUTION-CHECK #72).
///
/// `node_rects` feeds `manuk_a11y::build_tree_with_rects`, whose `bbox` centre is where the agent
/// clicks. Five consecutive geometry ticks passed I3 *because the producer is shared*, not because
/// anyone checked — so a fix to the producer itself is exactly the case where that accident stops
/// protecting us.
///
/// The assertion is a *relation*, not a coordinate: the a11y bbox for the icon-wrapping span must be
/// the same box `node_rects` reports, and its centre must land on the icon's own column. Before the
/// fix the bbox was the 4px icon box, so the click point was 3.5px low.
///
/// Goes RED with either mutation above — and also if `build_tree_with_rects` ever stops reading
/// `node_rects`, which is the coupling this gate is really pinning.
#[test]
fn g_inline_box_click_point_is_the_inline_box() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://inline.test/", &fonts, 1200.0);
    let a = rect_of(&page, "#a");
    let s5 = rect_of(&page, "#s5");

    let dom = page.dom();
    let n = manuk_css::query_selector_all(dom, dom.root(), "#a")
        .first()
        .copied()
        .expect("#a");
    fn find(nd: &manuk_a11y::A11yNode, want: manuk_dom::NodeId) -> Option<manuk_a11y::Rect> {
        if nd.node == want {
            return nd.bbox;
        }
        nd.children.iter().find_map(|c| find(c, want))
    }
    let bb = find(&page.a11y_tree(), n).unwrap_or_else(|| {
        panic!("#a has no a11y bbox — the agent has nowhere to click the icon button at all")
    });

    assert!(
        (bb.x - a.x).abs() < 0.51
            && (bb.y - a.y).abs() < 0.51
            && (bb.width - a.width).abs() < 0.51
            && (bb.height - a.height).abs() < 0.51,
        "G_INLINE_BOX_GEOMETRY/I3: the a11y bbox for `#a` is [{:.2},{:.2},{:.2},{:.2}] but \
         `node_rects` says [{:.2},{:.2},{:.2},{:.2}]. These must be the same box — the agent's click \
         point is the bbox centre, so any divergence here is a mis-actuation the render metric cannot \
         see.",
        bb.x, bb.y, bb.width, bb.height, a.x, a.y, a.width, a.height
    );
    assert!(
        (bb.height - s5.height).abs() < 1.01,
        "G_INLINE_BOX_GEOMETRY/I3: the agent's click target for the icon-wrapper `#a` is {:.2}px \
         tall, against the {:.2}px inline box it should be. A 4px-tall target puts the click point \
         3.5px below where the user's would land on every icon button on the web.",
        bb.height,
        s5.height
    );
}
