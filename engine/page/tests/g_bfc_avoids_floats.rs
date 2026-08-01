//! # G_BFC_AVOIDS_FLOATS — a BFC root sits BESIDE a float, or below it
//!
//! CSS 2.1 §9.5: *"the border box of a table, a block-level replaced element, or an element in the
//! normal flow that establishes a new block formatting context must not overlap the margin box of any
//! floats in the same block formatting context."*
//!
//! We did neither half. A BFC root sat straight on top of the float.
//!
//! ```text
//!   float:left 80x40, then a BFC root in a 300px column
//!                                        Chrome        before        after
//!     overflow:hidden                [80   0 220x20]  [0 … 300x20]  [80   0 220x20]  ✗→✓
//!     display:flow-root              [80  50 220x20]  [0 … 300x20]  [80  50 220x20]  ✗→✓
//!     display:flex                   [80 100 220x20]  [0 … 300x20]  [80 100 220x20]  ✗→✓
//!     display:grid                   [80 150 220x20]  [0 … 300x20]  [80 150 220x20]  ✗→✓
//!     float:RIGHT + overflow:hidden  [0  320 220x20]  [0 … 300x20]  [0  320 220x20]  ✗→✓
//!     overflow:hidden, width:280px   [0  290 280x20]  [0 … 280x20]  [0  290 280x20]  ✓ DROPS to clear
//!     a PLAIN block (not a BFC root) [0  200 300x20]  [0 … 300x20]  [0  200 300x20]  ✓ must not move
//!
//!   (y values are THIS gate's fixture; the characterisation fixture in the journal has an extra
//!    `display:table` row and every y below it differs by 50 — see `#p7` below.)
//! ```
//!
//! **The reach is the MEDIA OBJECT** — a floated avatar or thumbnail with an `overflow:hidden` /
//! `flow-root` / flex content block beside it. That is every comment thread, every card list, every
//! article with a pull-quote, and the standard pre-flexbox two-column idiom.
//!
//! ## The plain block is the rule's boundary, not an oversight
//!
//! A non-BFC block's border box legitimately **does** overlap floats — only its *line boxes* avoid
//! them, which `open_band` already does. `#p6` is asserted at the full 300px width at x=0 for exactly
//! that reason: a fix that moved every block beside a float would pass all five rows above and be
//! badly wrong on the commonest layout on the web.
//!
//! ## Two halves, and the second one is easy to miss
//!
//! An `auto` width shrinks to the band and always fits. An **explicit** width that does not fit is
//! moved DOWN past the floats instead of overlapping them — `#p7` at `width:280px` in a band of 220.
//! Chrome puts it at y=290, below the float, not at 220 wide.
//!
//! ## How this goes RED
//!
//! - **Drop the `establishes_bfc` branch** → the five shifted rows all read x=0 w=300, while `#p6`
//!   (plain block) and `#p7` (dropped) still pass.
//! - **Key it on every block instead of on `establishes_bfc`** → `#p6` moves to 80 and narrows. That
//!   is the single assertion separating this fix from a page-wide regression.
//! - **Use `left_offset`/`right_offset` instead of `left_float_edge`/`right_float_edge`** → those
//!   fall back to the CONTEXT's edges rather than reporting the float-derived edge alone, so blocks
//!   with no float near them start moving. The `Option` form is the t797 distinction, reused.
//!
//! ⚠ **`display:table` is NOT asserted, and the reason is written down.** Chrome puts it at
//! `[80 200 35x20]`; we produce a **0-wide** box, from a table intrinsic-width defect that predates
//! this change and is unmoved by it (`layout_table` documents its own bounded scope). Shifting a box
//! that is already the wrong width would be cosmetic, and asserting Chrome's 35 would make this gate
//! fail for something it does not test.

use manuk_text::FontContext;

/// Each row is its own BFC (`.w { overflow:hidden }`) so its float cannot leak into the next one.
/// The first version of this fixture let them share a context, four floats stacked up across the
/// rows, and every reading below the third was a measurement of the stack rather than of the rule.
const HTML: &str = r##"<!doctype html><html><head><style>
body{margin:0;font:16px/1.25 sans-serif}
.w{width:300px;overflow:hidden;margin-bottom:10px}
.f{float:left;width:80px;height:40px;background:#fc8}
.b{background:#8cf;height:20px}
</style></head><body>
<div class="w"><div class="f"></div><div class="b" id="p1" style="overflow:hidden">ovh</div></div>
<div class="w"><div class="f"></div><div class="b" id="p2" style="display:flow-root">fr</div></div>
<div class="w"><div class="f"></div><div class="b" id="p3" style="display:flex">flex</div></div>
<div class="w"><div class="f"></div><div class="b" id="p4" style="display:grid">grid</div></div>
<div class="w"><div class="f"></div><div class="b" id="p6">plain</div></div>
<div class="w"><div class="f"></div><div class="b" id="p7" style="overflow:hidden;width:280px">wide</div></div>
<div class="w"><div class="f" style="float:right"></div><div class="b" id="p8" style="overflow:hidden">rightfloat</div></div>
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
        .unwrap_or_else(|| panic!("no box for {sel}"))
}

fn assert_xw(page: &manuk_page::Page, sel: &str, x: f32, w: f32, why: &str) {
    let r = rect_of(page, sel);
    assert!(
        (r.x - x).abs() < 1.01 && (r.width - w).abs() < 1.01,
        "G_BFC_AVOIDS_FLOATS: `{sel}` expected x={x} w={w} (MEASURED in headless Chrome on THIS \
         fixture), got x={} w={}.\n  {why}",
        r.x,
        r.width
    );
}

#[test]
fn g_bfc_avoids_floats() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://bfc.test/", &fonts, 1200.0);

    // ── THE BUG: shifted to the band's edge AND narrowed to it. Four display types, because the
    //    rule is about establishing a BFC and not about any one of them.
    assert_xw(
        &page,
        "#p1",
        80.0,
        220.0,
        "`overflow:hidden` beside a float:left 80 wide — the media object, and the oldest way to \
         write it. It sat on top of the float at x=0 w=300",
    );
    assert_xw(
        &page,
        "#p2",
        80.0,
        220.0,
        "`display:flow-root` — the modern spelling of the same thing",
    );
    assert_xw(
        &page,
        "#p3",
        80.0,
        220.0,
        "a FLEX container is a BFC root too",
    );
    assert_xw(&page, "#p4", 80.0, 220.0, "…and a GRID container");
    assert_xw(
        &page,
        "#p8",
        0.0,
        220.0,
        "a float:RIGHT narrows the band without shifting the box — if the fix only ever moved boxes \
         right, this reads x=80 or w=300",
    );

    // ── THE SECOND HALF: an explicit width that will not fit is moved DOWN, not squeezed.
    // ⚠ y=290 is measured on THIS fixture. I first wrote 340, ported from the wider characterisation
    //    fixture that also carried a `display:table` row — and dropping that row moved everything
    //    below it up by 50. The gate caught it on its first run. t797's rule, for the second time in
    //    this session: **a measured number is only measured for the fixture it was measured in.**
    let p7 = rect_of(&page, "#p7");
    assert!(
        (p7.width - 280.0).abs() < 1.01 && (p7.y - 290.0).abs() < 1.01,
        "G_BFC_AVOIDS_FLOATS: `#p7` is `width:280px` in a 220px band, so it must DROP below the \
         float and keep its width (Chrome, THIS fixture: y=290, w=280). Got y={} w={}. Squeezing it \
         to 220 would satisfy 'must not overlap' and be the wrong answer",
        p7.y,
        p7.width
    );

    // ── THE BOUNDARY, and it is the assertion that separates this from a page-wide regression.
    assert_xw(
        &page,
        "#p6",
        0.0,
        300.0,
        "a PLAIN block is NOT a BFC root: its border box legitimately overlaps the float and only \
         its LINE boxes avoid it. A fix keyed on every block instead of on `establishes_bfc` passes \
         every row above and is badly wrong on the commonest layout on the web",
    );
}
