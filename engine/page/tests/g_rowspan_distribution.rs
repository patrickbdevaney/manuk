//! # G_ROWSPAN_DISTRIBUTION — a rowspan cell's excess is SHARED by the rows it spans, not dumped on the last
//!
//! A `rowspan` cell taller than the rows it covers has excess height to place. That excess was added
//! entirely to the **last** spanned row:
//!
//! ```rust
//! if *bh > spanned { row_h[last] += *bh - spanned; }
//! ```
//!
//! Chrome shares it across every spanned row, **proportionally to their natural heights**. This is
//! the defect `CONSTITUTION.MD` VI.2 has carried as *"t933 row-height distribution"* since check
//! #82, and the sixteen-row table battery of t989 is the first fixture to put numbers on it.
//!
//! ```text
//!                                                         Chrome     before     after
//!   rowspan=2 60px over two 24px rows          row 1       [30]       [24]      [30]
//!                                              row 2       [30]       [36]      [30]
//!   rowspan=2 100px over rows of 40 and 24     row 1       [63]       [40]      [63]
//!                                              row 2       [38]       [84]      [38]
//!   rowspan=3 90px over three 24px rows        each        [30]    [24/24/42]   [30]
//!  ── CONTROLS, none of which moved ──
//!   rowspan=2 30px over rows of 40 and 24 (cell SHORTER)  [40]/[24]        unchanged
//!   the same two rows with NO rowspan at all              [40]/[24]        unchanged
//! ```
//!
//! ## Proportional, not even — and only one row in the fixture can tell
//!
//! The excess of 36 over rows of 40 and 24 splits **22.5 / 13.5**, giving 63 / 38. Even distribution
//! would give 58 / 42. Every other row here has equal natural heights, where proportional and even
//! **degenerate to the same answer** — so the unequal-rows row is the entire discriminator between
//! the two rules, and a fixture of equal rows would have shipped either one.
//!
//! Even distribution survives as the fallback for rows that are all zero-height, where
//! "proportional" has no meaning and dividing by the total would divide by zero.
//!
//! ## Why this is not a one-cell error
//!
//! Every spanned row grows, so **everything inside the other cells of those rows moves too**. A
//! rowspan in a real table — an invoice's line-item block, a schedule's merged slot, a spec table's
//! grouped column — displaced its whole neighbourhood, and the further down the table it sat the
//! more it displaced.
//!
//! ## How this goes RED
//!
//! - **Restore `row_h[last] += deficit`** (the original) → the unequal row reads 40 / 84 and both
//!   equal-row cases read 24 / 36; both controls pass.
//! - **Distribute EVENLY instead of proportionally** → **only the unequal-rows case fails**, at
//!   58 / 42 against 63 / 38. That row is the whole reason the fixture has unequal rows.
//! - **Drop the `deficit > 0.0` guard** → the SHORTER-cell control fails, because a negative deficit
//!   would shrink rows that a rowspan cell is not entitled to shrink.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
html,body{margin:0;padding:0}
body{font:16px/1.5 monospace}
*{box-sizing:border-box}
table{border-collapse:separate;border-spacing:0}
td{padding:0}
.w{width:400px;margin:0 0 6px 0}
</style></head><body>
<div class="w" id="c1"><table><tr><td rowspan="2" style="height:60px">a</td><td id="a1">b</td></tr><tr><td id="a1b">c</td></tr></table></div>
<div class="w" id="c2"><table><tr><td rowspan="2" style="height:100px">a</td><td id="a2" style="height:40px">b</td></tr><tr><td id="a2b" style="height:20px">c</td></tr></table></div>
<div class="w" id="c3"><table><tr><td rowspan="2" style="height:30px">a</td><td id="a3" style="height:40px">b</td></tr><tr><td id="a3b" style="height:20px">c</td></tr></table></div>
<div class="w" id="c4"><table><tr><td rowspan="3" style="height:90px">a</td><td id="a4">b</td></tr><tr><td id="a4b">c</td></tr><tr><td id="a4c">d</td></tr></table></div>
<div class="w" id="c5"><table><tr><td id="a5" style="height:40px">b</td></tr><tr><td id="a5b" style="height:20px">c</td></tr></table></div>
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
fn g_rowspan_distribution() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://rs.test/", &fonts, 1200.0);
    let yh = |sel: &str, w: &str| {
        let (a, c) = (rect_of(&page, sel), rect_of(&page, w));
        (a.y - c.y, a.height)
    };
    let near = |got: (f32, f32), want: (f32, f32)| {
        (got.0 - want.0).abs() < 1.1 && (got.1 - want.1).abs() < 1.1
    };

    // ── DEFECT 1 — the equal-rows case. A 60px rowspan over two 24px rows: 12px of excess, 6 to
    //    each. Reading 24/36 means the whole excess landed on the last row.
    assert!(
        near(yh("#a1", "#c1"), (0.0, 30.0)) && near(yh("#a1b", "#c1"), (30.0, 30.0)),
        "G_ROWSPAN_DISTRIBUTION: a `rowspan=2` cell 60 tall over two 24px rows makes them 30 and 30; \
         got {:?} and {:?}. 24 and 36 is the whole excess dumped on the LAST spanned row.",
        yh("#a1", "#c1"),
        yh("#a1b", "#c1")
    );

    // ── DEFECT 2 — THE DISCRIMINATOR. Unequal natural heights (40 and 24) with 36px of excess:
    //    proportional gives 22.5/13.5 -> 63/38, EVEN would give 18/18 -> 58/42. Every other row in
    //    this fixture has equal rows, where the two rules give the same answer.
    assert!(
        near(yh("#a2", "#c2"), (0.0, 63.0)) && near(yh("#a2b", "#c2"), (63.0, 38.0)),
        "G_ROWSPAN_DISTRIBUTION: 36px of excess over rows of natural height 40 and 24 splits \
         PROPORTIONALLY — 22.5 and 13.5, so the rows are 63 and 38; got {:?} and {:?}. 58 and 42 is \
         an EVEN split, which every other row in this gate is blind to.",
        yh("#a2", "#c2"),
        yh("#a2b", "#c2")
    );

    // ── DEFECT 3 — three spanned rows, to pin that the loop covers the whole span and not just its
    //    ends.
    assert!(
        near(yh("#a4", "#c4"), (0.0, 30.0))
            && near(yh("#a4b", "#c4"), (30.0, 30.0))
            && near(yh("#a4c", "#c4"), (60.0, 30.0)),
        "G_ROWSPAN_DISTRIBUTION: a `rowspan=3` cell 90 tall over three 24px rows makes them all 30; \
         got {:?}, {:?}, {:?}.",
        yh("#a4", "#c4"),
        yh("#a4b", "#c4"),
        yh("#a4c", "#c4")
    );

    // ── CONTROL A — a rowspan cell SHORTER than the rows it spans distributes nothing. This is the
    //    row that fails if the `deficit > 0` guard is dropped: a negative deficit would SHRINK rows
    //    a rowspan cell has no right to shrink.
    assert!(
        near(yh("#a3", "#c3"), (0.0, 40.0)) && near(yh("#a3b", "#c3"), (40.0, 24.0)),
        "G_ROWSPAN_DISTRIBUTION: a `rowspan=2` cell only 30 tall over rows of 40 and 24 changes \
         nothing — the rows stay 40 and 24; got {:?} and {:?}.",
        yh("#a3", "#c3"),
        yh("#a3b", "#c3")
    );

    // ── CONTROL B — the same two rows with NO rowspan anywhere. The distribution must be reachable
    //    only from a spanning cell.
    assert!(
        near(yh("#a5", "#c5"), (0.0, 40.0)) && near(yh("#a5b", "#c5"), (40.0, 24.0)),
        "G_ROWSPAN_DISTRIBUTION: without any rowspan the rows are 40 and 24; got {:?} and {:?}. \
         `height:20px` on the second is a MINIMUM — its 24px line wins.",
        yh("#a5", "#c5"),
        yh("#a5b", "#c5")
    );
}
