//! # G_TABLE_CAPTION — the box we rendered as NOTHING, and the width interaction that is not obvious
//!
//! `<caption>` was skipped alongside the column groups in `collect_table_rows` and never laid out at
//! all. **Three defects in one dropped child**, and they are not the same kind:
//!
//! 1. the caption's text did not appear — a MISSING_BOX, not a geometry error;
//! 2. the rows did not move down for it, so every row of the table sat where the caption belonged;
//! 3. the table did not widen for it.
//!
//! ```text
//!                                                     Chrome     before      after
//!   caption 20px tall, one-cell table  caption box    [0, h20]   ABSENT    [0, h20]
//!                                      the cell       [y=20]     [y= 0]    [y=20]
//!   caption `a very long caption`      table width    [   67]    [   10]   [   67]
//!                                      the cell       [y=72]     [y= 0]    [y=72]
//!   caption written AFTER the rows     caption box    [y= 0]     ABSENT    [y= 0]
//!                                      the cell       [y=20]     [y= 0]    [y=20]
//!  ── CONTROL ──
//!   no caption                         the cell       [y=0,w10]  unchanged
//! ```
//!
//! ## A caption WIDENS its table, and that is the part worth measuring
//!
//! The table's used width is at least the caption's **min-content** width. Chrome-measured: a
//! one-cell table whose cell holds `x` (10px wide) with the caption *"a very long caption"* comes out
//! **67** wide — the longest word — and the column takes the extra. The surplus is distributed over
//! the columns exactly as a rowspan's surplus is distributed over its rows (t990), on the other axis.
//!
//! **Min-content and not max-content, because the caption is allowed to wrap.** At 67 that caption is
//! three lines tall (h=72) and Chrome keeps it three lines rather than widening the table to fit it
//! on one. A `max_content_width` here would give a one-line caption and a table several hundred
//! pixels wide — a wrong answer of the right shape, and the row that distinguishes them is the wide
//! caption's *height*, not its width.
//!
//! ## The caption is FIRST among the table's children
//!
//! It paints above the rows and — the reason that matters here — it **reads first in the semantic
//! order the agent surface walks**. A caption is a table's accessible name; emitting it after the
//! rows would put the label after the data for every consumer of the a11y tree, which is the I3
//! surface rather than the paint one.
//!
//! ## NAMED, MEASURED, NOT BUILT
//!
//! ⚠ **`caption-side: bottom`** belongs below the row area — Chrome-measured, a 20px caption under a
//! 30px row sits at y=30 and the first cell at y=0 — and is **not built**: there is no `caption_side`
//! field on `ComputedStyle`, so this is the t985/t987 "nowhere to live" shape again and the fix is a
//! cascade addition rather than a layout one. The initial value is `top`, which is what every row in
//! this gate exercises, and what a `<caption>` written *anywhere* in the table gets — including after
//! the rows, which row 3 pins.
//!
//! ## How this goes RED
//!
//! - **Restore the `_ => {}` skip** (drop the caption collection) → the caption box vanishes and
//!   every cell reads y=0; the control passes.
//! - **Lay the caption out but do not offset the rows** → the caption boxes exist and every cell
//!   still reads y=0, which is the version that looks like it works in a screenshot of row 1.
//! - **Use `max_content_width` for the width floor** → the wide-caption row's table is far wider than
//!   67 and its caption one line tall instead of three; the narrow rows all still pass.
//! - **Emit the caption boxes AFTER the rows** → nothing in this gate fails, because the assertions
//!   are geometric. Recorded as a NON-red: the ordering claim is about the semantic tree, and the
//!   gate that could see it is an a11y-order gate this file is not.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
html,body{margin:0;padding:0}
body{font:16px/1.5 monospace}
*{box-sizing:border-box}
table{border-collapse:separate;border-spacing:0}
td{padding:0}
.w{width:400px;margin:0 0 6px 0}
</style></head><body>
<div class="w" id="c1"><table><caption id="p1" style="height:20px">cap</caption><tr><td id="a1">x</td></tr></table></div>
<div class="w" id="c2"><table><caption id="p2">a very long caption</caption><tr><td id="a2">x</td></tr></table></div>
<div class="w" id="c4"><table><tr><td id="a4">x</td></tr><caption id="p4" style="height:20px">cap</caption></table></div>
<div class="w" id="c5"><table><tr><td id="a5">x</td></tr></table></div>
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
fn g_table_caption() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://cap.test/", &fonts, 1200.0);
    let r = |sel: &str| rect_of(&page, sel);
    let dy = |sel: &str, w: &str| r(sel).y - r(w).y;
    let near = |got: f32, want: f32| (got - want).abs() < 1.6;

    // ── DEFECT 1 — the caption EXISTS. `rect_of` panics outright if it has no box, which is the
    //    honest assertion for a MISSING_BOX: there is no wrong number to compare, there is no box.
    let cap = r("#p1");
    assert!(
        near(cap.y - r("#c1").y, 0.0) && near(cap.height, 20.0) && near(cap.width, 29.0),
        "G_TABLE_CAPTION: the caption is a real box at the top of the table, 20 tall and as wide as \
         the table (29) — got y={}, h={}, w={}.",
        cap.y - r("#c1").y,
        cap.height,
        cap.width
    );

    // ── DEFECT 2 — the rows move down for it. This is the row that separates "the caption is drawn"
    //    from "the caption occupies space", and a screenshot of row 1 alone cannot tell them apart.
    assert!(
        near(dy("#a1", "#c1"), 20.0),
        "G_TABLE_CAPTION: the first cell sits BELOW the 20px caption at y=20, not {}. y=0 means the \
         caption was laid out and then not accounted for — the rows would overlap it.",
        dy("#a1", "#c1")
    );

    // ── DEFECT 3 — the caption WIDENS the table, to its MIN-content width, and then wraps inside it.
    //    Both numbers are asserted because the width alone cannot tell min-content from max-content:
    //    it is the caption's HEIGHT (three lines, 72px) that says it was allowed to wrap.
    assert!(
        near(r("#a2").width, 67.0) && near(dy("#a2", "#c2"), 72.0),
        "G_TABLE_CAPTION: `a very long caption` over a 10px cell widens the table to the caption's \
         MIN-content width (67, the longest word) and wraps to three lines (72 tall), so the cell is \
         67 wide at y=72; got w={}, y={}. A max-content floor would give one line and a table \
         several hundred px wide — right shape, wrong answer.",
        r("#a2").width,
        dy("#a2", "#c2")
    );

    // ── DEFECT 4 — a caption written AFTER the rows still renders at the top. `caption-side`'s
    //    initial value is `top` and source position does not enter into it.
    assert!(
        near(dy("#p4", "#c4"), 0.0) && near(dy("#a4", "#c4"), 20.0),
        "G_TABLE_CAPTION: a `<caption>` written after the rows is still at the top — caption y=0, \
         cell y=20; got {} and {}.",
        dy("#p4", "#c4"),
        dy("#a4", "#c4")
    );

    // ── CONTROL — a table with NO caption. Nothing about the row origin or the table's width may
    //    move for tables that do not have one, which is nearly all of them.
    assert!(
        near(dy("#a5", "#c5"), 0.0) && near(r("#a5").width, 10.0),
        "G_TABLE_CAPTION: a table without a caption keeps its cell at y=0 and its natural 10px \
         width; got y={}, w={}.",
        dy("#a5", "#c5"),
        r("#a5").width
    );
}
