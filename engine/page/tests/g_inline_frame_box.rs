//! # G_INLINE_FRAME_BOX — a HORIZONTAL-only frame left the inline's box on the LINE, not on its content
//!
//! t851 established that a non-replaced inline's box is its own **content area**, resolved per axis,
//! and that holds for a bare inline and for one with an all-sides border. It did **not** hold for a
//! frame on the inline axis only — which is the overwhelmingly common inline decoration: `<code>`,
//! `<kbd>`, a padded `<a>` chip or pill, a badge span, a syntax-highlighted token.
//!
//! ```text
//!                                          Chrome            before            after
//!   <span>y</span>                     [10,  2, 10, 19]  [10,  2, 10, 19]   unchanged
//!   <span background>y</span>          [10,  2, 10, 19]  [10,  2, 10, 19]   unchanged
//!   <span border-left:12px>y</span>    [10,  2, 22, 19]  [10,  0, 22, 21]  [10, 2, 22, 19]
//!   <span padding-left:12px>y</span>   [10,  2, 22, 19]  [10,  0, 22, 21]  [10, 2, 22, 19]
//!   <span border:12px>y</span>         [10,-10, 34, 43]  [10,-10, 34, 43]   unchanged
//! ```
//!
//! ## The inline axis was right in every row — only the block axis was wrong
//!
//! The frame advanced the pen correctly throughout (`width` is 22 before and after). What went wrong
//! is the *vertical* report, and only when the vertical frame was zero: `collect_inline_node`
//! computed the inline's vertical extent behind `if pad_t > 0.0 || pad_b > 0.0`, so a
//! horizontal-only frame emitted an edge spacer carrying **no vertical report at all**. That spacer
//! fell back to the LINE BOX, and an inline's box is the union of its fragments — so the union of a
//! line-box spacer (0..21) with the element's own word (2..21) came out **0..21**: two pixels of
//! half-leading too high, and two too tall, on a box whose background is painted.
//!
//! > **A conditional that guards a computation by the axis it happens to be about is how a per-axis
//! > rule loses one axis.** The vertical report was written for vertical padding, so it was gated on
//! > vertical padding — and the box it produces is the right answer for *any* framed inline, because
//! > with zero vertical padding the two terms simply add nothing.
//!
//! ## Why the all-sides case was already right, which is what made this narrow
//!
//! `border: 12px` has a non-zero `pad_t`, so it took the same branch and got a correct report. Only
//! the *horizontal-only* shape fell through — so a fixture built from `padding: 10px 20px` (the row
//! that motivated the original code) cannot see this, and the bare-inline row cannot either.
//!
//! ## How this goes RED
//!
//! - **Restore `if pad_t > 0.0 || pad_b > 0.0`** → the two horizontal-only rows read `[10, 0, …, 21]`
//!   and the other three pass. That confinement is the point: three of the five rows here are
//!   controls for behaviour this change must not touch.
//! - **Drop `pad_r` from the new condition** → nothing fails on this fixture, because every framed
//!   row here has a LEFT edge. Recorded as a NON-red: `pad_r` is in the condition because
//!   `padding-right` alone is as much a frame as `padding-left`, and the row that would prove it is
//!   not written — the honest statement is that this half is reasoned, not measured.
//! - **Use the line height instead of `ascent + descent` for the report** → all three framed rows
//!   read height 24 and the two bare rows still pass.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
html,body{margin:0;padding:0}
body{font:16px/1.5 monospace}
*{box-sizing:border-box}
.w{width:400px;margin:0 0 6px 0}
</style></head><body>
<div class="w" id="c1">x<span id="a1" style="border-left:12px solid">y</span></div>
<div class="w" id="c2">x<span id="a2">y</span></div>
<div class="w" id="c3">x<span id="a3" style="padding-left:12px">y</span></div>
<div class="w" id="c4">x<span id="a4" style="border:12px solid">y</span></div>
<div class="w" id="c5">x<span id="a5" style="background:#39c">y</span></div>
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

#[test]
fn g_inline_frame_box() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://ifb.test/", &fonts, 1200.0);
    let off = |sel: &str, w: &str| {
        let (a, c) = (rect_of(&page, sel), rect_of(&page, w));
        (a.x - c.x, a.y - c.y, a.width, a.height)
    };
    let near = |g: (f32, f32, f32, f32), w: (f32, f32, f32, f32)| {
        (g.0 - w.0).abs() < 1.1
            && (g.1 - w.1).abs() < 1.1
            && (g.2 - w.2).abs() < 1.1
            && (g.3 - w.3).abs() < 1.1
    };

    // ── DEFECT — a frame on the INLINE AXIS ONLY. Both spellings, because `border-left` and
    //    `padding-left` reach the same edge sum and a fix that handled one would be an accident.
    for (sel, w, decl) in [
        ("#a1", "#c1", "border-left:12px"),
        ("#a3", "#c3", "padding-left:12px"),
    ] {
        assert!(
            near(off(sel, w), (10.0, 2.0, 22.0, 19.0)),
            "G_INLINE_FRAME_BOX: `{decl}` on an inline gives a box of [10, 2, 22, 19] — the frame \
             advances the pen (22 wide) and the BLOCK axis stays the element's own content area; \
             got {:?}. [10, 0, 22, 21] is the LINE BOX: 2px of half-leading too high and 2px too \
             tall, on a box whose background is painted.",
            off(sel, w)
        );
    }

    // ── CONTROL A — a bare inline and a background-only inline. Neither emits an edge spacer, and
    //    both were already exact (t851). A fix that reported the content area for every inline
    //    rather than for framed ones would have to pass through here.
    assert!(
        near(off("#a2", "#c2"), (10.0, 2.0, 10.0, 19.0))
            && near(off("#a5", "#c5"), (10.0, 2.0, 10.0, 19.0)),
        "G_INLINE_FRAME_BOX: a bare inline and a background-only inline are [10, 2, 10, 19]; got \
         {:?} and {:?}.",
        off("#a2", "#c2"),
        off("#a5", "#c5")
    );

    // ── CONTROL B — an ALL-SIDES border, which was already right and is why this defect was narrow:
    //    its non-zero vertical padding took the branch the horizontal-only case fell through. The
    //    vertical frame must still extend the box beyond the content area, 12px each way.
    assert!(
        near(off("#a4", "#c4"), (10.0, -10.0, 34.0, 43.0)),
        "G_INLINE_FRAME_BOX: `border:12px` extends the box 12px above and below its content area — \
         [10, -10, 34, 43], not {:?}. A vertical frame is NOT clamped to the line.",
        off("#a4", "#c4")
    );
}
