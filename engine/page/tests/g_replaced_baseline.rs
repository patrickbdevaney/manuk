//! # G_REPLACED_BASELINE — a replaced element's baseline is its bottom margin edge, always
//!
//! An inline `<svg>` made its line box **30px where Chrome gives 20**, and sat **14px down inside
//! it**. An `<img>` of the identical 16×16 size was correct in both engines, in the same fixture, on
//! the same line — which is what turns this from a guess into an identification.
//!
//! ```text
//!   16px sans-serif block, one 16x16 thing on the line
//!                                          Chrome   before   after
//!     <div><svg 16x16></div>                 20       30       20
//!       ...the svg's own y inside it          0       14        0
//!     <div><svg style=width/height></div>    20       30       20
//!     <div><img 16x16></div>                 20       20       20    <- THE CONTROL
//!     <div><svg display:block></div>         16       16       16
//!     <div><svg vertical-align:top></div>    18       18       18
//!     <div><svg no width/height></div>     1204     1214     1204
//!   ─────────────────────────────────────────────────────────────────
//!     y of #end, after seven rows          1316     1347     1317
//! ```
//!
//! ## The arithmetic identifies it exactly
//!
//! The svg's box reported a baseline of **0** — its own TOP — so all 16px hung *below* the baseline
//! and the line came to `strut ascent 14 + 16 = 30`, with the glyph pushed 14px down. That is not an
//! approximation; it is the measured 30 and the measured 14.
//!
//! **The rule was right and its DOMAIN was wrong.** CSS 2.1 §10.8.1 says an inline-block's baseline
//! is its *last in-flow line box's*, falling back to the bottom margin edge when there is none — and
//! the engine ran that search on **every** atomic inline. A replaced element has no in-flow line
//! boxes by definition: what it displays is not a line. Asking our own internal box structure that
//! question is asking something the spec never asks, and `<svg>` answers, because unlike `<img>` it
//! has element children to build a box out of. **`<img>` was right all along by accident** — no inner
//! content, so it always took the fallback.
//!
//! ## Why this is a burndown item and not a curiosity
//!
//! Inline `<svg>` is on **34.5% of the CrUX-trend corpus** and `<button><svg/> Label</button>` on
//! **23.4%** (`docs/loop/CORPUS-CONSTRUCTS.md`, t965) — nav bars, toolbars, chips, and every icon
//! button on the modern web. Each one was 10px too tall, and a line box that is 10px too tall drags
//! everything below it down the page.
//!
//! ## …and the same rule ONE LEVEL UP (t970)
//!
//! An inline-block that CONTAINS a replaced element was wrong in two different ways, and the `<img>`
//! row is again what proves they are one bug:
//!
//! ```text
//!                                             Chrome   before   after
//!   <span inline-block><svg 16x16></span>       20       34       20
//!   <span inline-block><img 16x16></span>       20       24       20
//!   <div><button><svg 16x16></button></div>     26       36       24
//! ```
//!
//! `layout_children` files atomics as sibling boxes, so §10.8.1's search walks them. `<svg>` has a
//! subtree, so the search descended and believed a `<rect>` fragment sitting at the box's own top —
//! baseline 0, all 20px below the line's baseline, `14.5 + 20 ≈ 34`. `<img>` has none, so the search
//! returned `None` and the caller took the *"no in-flow line boxes"* fallback — **the fallback taken
//! on a line box that exists** — giving the inline-block's own bottom edge (20) instead of the
//! image's (16), `20 + 3.5 ≈ 24`. **A replaced kid contributes its own bottom edge**, and both rows
//! land on Chrome's 20.
//!
//! ⚠ The `<button>` rows now read 24 against Chrome's 26 — a **−2** that every button in the fixture
//! shares, including the text-only one this fix never touched. That is the pre-existing UA
//! control-height difference (t963 named it on `<select>`), not a residue of this rule.
//!
//! ## How this goes RED
//!
//! - **Restore the unconditional `last_line_baseline` search** (drop the `is_replaced` guard) → the
//!   bare-svg rows read 30 against 20 and the svg sits at y=14. The original defect.
//! - **Extend the guard to every atomic, not just replaced ones** → a text-bearing `inline-block`
//!   loses its real baseline and `#ib` grows from 19 to 22, which is the t-earlier §10.8.1 defect
//!   this guard must not undo.
//! - **Restore the plain recursion in `last_line_baseline`'s `Block` arm** → `#wcsvg` reads 34.
//! - **Make a replaced kid return `None` instead of its bottom edge** (the half-fix t968 predicted)
//!   → `#wcsvg` and `#wcimg` both read **24**, strictly better than 34 and still 4px short of
//!   Chrome. Measured, not reasoned: this is why the rule contributes an edge rather than skipping.

use manuk_text::FontContext;

// A 1×1 transparent GIF: the control arm needs a replaced element that has never had inner content,
// so that "the fix" and "the fallback it always took" can be told apart on the same line.
const GIF: &str = "data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7";

fn html() -> String {
    format!(
        r##"<!doctype html><html><head><style>
body{{margin:0;font-family:sans-serif;font-size:16px}}
div{{margin:0}}
</style></head><body>
<div id="wsvg"><svg id="svg" width="16" height="16" viewBox="0 0 16 16"><rect width="16" height="16"/></svg></div>
<div id="wcss"><svg id="scss" style="width:16px;height:16px" viewBox="0 0 16 16"><rect width="16" height="16"/></svg></div>
<div id="wimg"><img id="img" width="16" height="16" src="{GIF}"></div>
<div id="wblk"><svg id="sblk" width="16" height="16" style="display:block" viewBox="0 0 16 16"><rect width="16" height="16"/></svg></div>
<div id="wtop"><svg id="stop" width="16" height="16" style="vertical-align:top" viewBox="0 0 16 16"><rect width="16" height="16"/></svg></div>
<div id="wib"><span id="ib" style="display:inline-block">Ay</span>Ay</div>
<div id="wcsvg"><span style="display:inline-block"><svg width="16" height="16" viewBox="0 0 16 16"><rect width="16" height="16"/></svg></span></div>
<div id="wcimg"><span style="display:inline-block"><img width="16" height="16" src="{GIF}"></span></div>
</body></html>"##
    )
}

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
        .unwrap_or_else(|| panic!("no box for {sel}"))
}

#[test]
fn g_replaced_baseline() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(&html(), "https://rb.test/", &fonts, 1200.0);
    let h = |sel: &str| rect_of(&page, sel).height;

    // ── THE DEFECT, and both of its halves. The line box AND the position inside it: a fix that got
    //    the height right by inflating something else would still put the glyph in the wrong place.
    assert!(
        (h("#wsvg") - 20.0).abs() < 1.1,
        "G_REPLACED_BASELINE: the line box around a 16x16 inline <svg> is {} where Chrome gives 20. \
         Reading 30 means the svg's box reported a baseline of 0 — its own TOP — so all 16px hung \
         BELOW the baseline and the line came to `strut ascent 14 + 16`.",
        h("#wsvg")
    );
    let inner = rect_of(&page, "#svg").y - rect_of(&page, "#wsvg").y;
    assert!(
        inner.abs() < 1.1,
        "G_REPLACED_BASELINE: the svg sits {inner}px below its own wrapper's top; Chrome puts it at \
         0. This is the same defect as the height assertion above and it is asserted separately \
         because a line box can be made the right size with the content still in the wrong place."
    );
    assert!(
        (h("#wcss") - 20.0).abs() < 1.1,
        "G_REPLACED_BASELINE: the same svg sized by CSS rather than by attributes is {} against \
         Chrome's 20 — the rule is about the element being REPLACED, not about where its size came \
         from.",
        h("#wcss")
    );

    // ── THE CONTROL THAT IDENTIFIES IT: an <img> of the identical size, on the same kind of line,
    //    was ALWAYS right — because it has no inner content for the search to find, so it always
    //    took the fallback this fix makes unconditional.
    assert!(
        (h("#wimg") - 20.0).abs() < 1.1,
        "G_REPLACED_BASELINE: the <img> control is {} and must be 20 both before and after. If this \
         moved, the change is not the narrow replaced-element rule it claims to be.",
        h("#wimg")
    );
    assert!(
        (h("#wimg") - h("#wsvg")).abs() < 1.1,
        "G_REPLACED_BASELINE: a 16x16 <svg> ({}) and a 16x16 <img> ({}) are the same replaced box on \
         the same line and Chrome gives both 20. Any difference between them is the bug.",
        h("#wsvg"),
        h("#wimg")
    );

    // ── THE TWO ESCAPES THAT ALREADY WORKED, so the fix is not credited for them.
    assert!(
        (h("#wblk") - 16.0).abs() < 1.1,
        "G_REPLACED_BASELINE: `display:block` takes the svg out of the line entirely — {} against \
         Chrome's 16.",
        h("#wblk")
    );
    assert!(
        (h("#wtop") - 18.0).abs() < 1.1,
        "G_REPLACED_BASELINE: `vertical-align:top` bypasses the baseline — {} against Chrome's 18.",
        h("#wtop")
    );

    // ── AND THE RULE THIS GUARD MUST NOT UNDO. A text-bearing inline-block DOES align on its last
    //    line box's baseline (§10.8.1's main clause, landed earlier and Chrome-measured at 19.19).
    //    Widening the guard from "replaced" to "every atomic" makes this row 23.
    assert!(
        (h("#wib") - 19.2).abs() < 1.6,
        "G_REPLACED_BASELINE: the line around a text-bearing `display:inline-block` is {} where \
         Chrome gives 19.19. Reading 23 means the §10.8.1 baseline search was disabled for \
         NON-replaced atomics too — the replaced-element guard is a narrowing, not a removal.",
        h("#wib")
    );

    // ── AND THE SAME RULE ONE LEVEL UP (t970): an inline-block CONTAINING a replaced element.
    //    `layout_children` files atomics as sibling boxes, so §10.8.1's baseline search walks them —
    //    and a replaced kid answered in two different wrong ways depending on whether it had a
    //    subtree. `<img>` returned None (no subtree) and the caller took the "no line boxes"
    //    fallback: the inline-block's OWN bottom edge, 24 against Chrome's 20. `<svg>` returned 0
    //    (its <rect> sits at the box's top), hanging all 20px below the baseline: 34.
    assert!(
        (h("#wcsvg") - 20.0).abs() < 1.1,
        "G_REPLACED_BASELINE: a `display:inline-block` wrapping a 16x16 <svg> gives a line of {} \
         where Chrome gives 20. Reading 34 means the baseline search DESCENDED into the svg and \
         believed a fragment sitting at the box's own top; reading 24 means it skipped the subtree \
         but contributed NOTHING, so the caller fell back to the inline-block's own bottom edge. \
         Both were measured; the rule is that a replaced kid contributes its OWN bottom edge.",
        h("#wcsvg")
    );
    assert!(
        (h("#wcimg") - 20.0).abs() < 1.1,
        "G_REPLACED_BASELINE: a `display:inline-block` wrapping a 16x16 <img> gives a line of {} \
         where Chrome gives 20. Reading 24 means the search found NOTHING and the caller fell back \
         to the inline-block's own bottom margin edge — the fallback taken on a line box that \
         EXISTS. A replaced kid contributes its own bottom edge, which is what makes this row and \
         the <svg> row above ONE bug rather than two.",
        h("#wcimg")
    );
    assert!(
        (h("#wcsvg") - h("#wcimg")).abs() < 1.1,
        "G_REPLACED_BASELINE: the <svg> wrapper ({}) and the <img> wrapper ({}) must agree — they \
         are the same replaced box at the same size, and any difference between them is the \
         subtree being searched.",
        h("#wcsvg"),
        h("#wcimg")
    );
}
