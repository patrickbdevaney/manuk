//! **G_GRID_AREA_SHORTHAND — `grid-column` and `grid-row` were parsed and `grid-area` was not.**
//!
//! `grid-area: <row-start> / <col-start> / <row-end> / <col-end>` places an item on both axes at
//! once, and it is the spelling authors reach for. `MinimalCascade` parsed the two per-axis
//! shorthands and not the combined one, so a rule that placed an item with `grid-area` placed
//! nothing and the item fell back to **auto-placement** — which lands in the right cell often enough
//! to hide the bug. It is declared, with `grid-template-areas`, **127 times** across 14 sampled CrUX
//! stylesheets (surface audit #79).
//!
//! ⚠ This is the weaker of the two cascades: the shipping (Stylo) path places both forms correctly,
//! measured. The value here is the `--no-default-features` build **and** instrument fidelity —
//! `engine/layout`'s 191 unit tests run on `MinimalCascade`, and one of them
//! (`a_grid_generated_containing_block_is_the_grid_area_for_children_and_descendants_alike`) writes
//! `style="grid-area:1/1"` on an intermediate box. That declaration did nothing; the box landed in
//! cell 1/1 by auto-placement instead, so the gate was green **for a different reason than it
//! states**.
//!
//! ## ⭐ The order is row / column / row / column
//!
//! Not the row-then-column PAIRS that `grid-row: a / b` and `grid-column: a / b` use. Reading it as
//! two pairs puts the item in the transposed cell — **invisible on a symmetric fixture**, which is
//! why the `grid-area: 2` row below is asymmetric: a transposed read lands at `[100 0]` where Chrome
//! says `[0 50]`.
//!
//! ## Chrome-measured, a 2×2 grid of 100×50 cells (item rect relative to the container)
//!
//! ```text
//!   grid-area: 2 / 2 / 3 / 3      [100 50 100x50]
//!   grid-area: 2 / 2              [100 50 100x50]   the omitted ends are auto — one cell
//!   grid-area: 2                  [  0 50 100x50]   row-start only; the column is auto
//!   grid-area: 1 / 1 / 3 / 3      [  0  0 200x100]  spanning both cells on both axes
//!   grid-area: span 2 / span 2    [  0  0 200x100]  `span` is a grid line like any other
//! ```
//!
//! ⚠ **The NAMED form (`grid-area: header`) is deliberately NOT parsed.** `GridLine` is
//! `Auto`/`Line`/`Span` with no ident, and the shipping path resolves names against
//! `grid-template-areas` before this type is reached. A name silently becoming `Auto` would place
//! the item in a different cell — worse than leaving the declaration alone — so the parser detects
//! an ident and does nothing. That is asserted below as a NEGATIVE row.
//!
//! PROVEN RED by two mutations — see the module tail.

use manuk_dom::NodeId;
use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
html,body{margin:0;padding:0} body{font-family:monospace;font-size:16px}
.g{display:grid;grid-template-columns:100px 100px;grid-template-rows:50px 50px;width:200px}
#a{grid-area:2 / 2 / 3 / 3}
#b{grid-area:2 / 2}
#c{grid-area:2}
#d{grid-area:1 / 1 / 3 / 3}
#e{grid-area:span 2 / span 2}
#f{grid-area:someName}
</style></head><body>
<div class="g" id="p1"><div id="a">A</div></div>
<div class="g" id="p2"><div id="b">B</div></div>
<div class="g" id="p3"><div id="c">C</div></div>
<div class="g" id="p4"><div id="d">D</div></div>
<div class="g" id="p5"><div id="e">E</div></div>
<div class="g" id="p6"><div id="f">F</div></div>
</body></html>"##;

fn by_id(page: &manuk_page::Page, id: &str) -> NodeId {
    let dom = page.dom();
    dom.descendants(dom.root())
        .find(|&n| {
            dom.element(n)
                .and_then(|e| e.attr("id"))
                .is_some_and(|v| v == id)
        })
        .unwrap_or_else(|| panic!("VACUOUS: no element with id={id:?}"))
}

fn rect(page: &manuk_page::Page, id: &str) -> manuk_layout::Rect {
    *page
        .root_box
        .node_rects(page.dom())
        .get(&by_id(page, id))
        .unwrap_or_else(|| panic!("VACUOUS: no box for id={id:?}"))
}

#[test]
fn the_grid_area_shorthand_places_on_both_axes() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://ga.test/", &fonts, 1200.0);
    let near = |g: f32, w: f32| (g - w).abs() < 1.5;

    // ── VACUITY. The containers must be real 200x100 grids, or every "cell" below is a coincidence
    // of auto layout.
    for p in 1..=6 {
        let r = rect(&page, &format!("p{p}"));
        assert!(
            near(r.width, 200.0) && near(r.height, 100.0),
            "VACUOUS: container p{p} is {r:?}, not a 200x100 grid — the cells below are not cells"
        );
    }

    // (id, parent, chrome dx, dy, w, h, what the row is for)
    let rows: &[(&str, &str, f32, f32, f32, f32, &str)] = &[
        ("a", "p1", 100.0, 50.0, 100.0, 50.0, "all four values"),
        ("b", "p2", 100.0, 50.0, 100.0, 50.0, "two values — the omitted ENDS are auto, so one cell"),
        ("c", "p3", 0.0, 50.0, 100.0, 50.0, "one value: it is the ROW start. A transposed read puts this at [100 0] — the row this fixture is asymmetric for"),
        ("d", "p4", 0.0, 0.0, 200.0, 100.0, "spanning both cells on both axes"),
        ("e", "p5", 0.0, 0.0, 200.0, 100.0, "`span N` is a grid line like any other"),
    ];
    for (id, p, dx, dy, w, h, why) in rows {
        let (e, q) = (rect(&page, id), rect(&page, p));
        let got = (e.x - q.x, e.y - q.y, e.width, e.height);
        assert!(
            near(got.0, *dx) && near(got.1, *dy) && near(got.2, *w) && near(got.3, *h),
            "G_GRID_AREA_SHORTHAND #{id}: Chrome places this at [{dx} {dy} {w}x{h}], got \
             [{:.0} {:.0} {:.0}x{:.0}].\n  {why}",
            got.0,
            got.1,
            got.2,
            got.3
        );
    }

    // ── PINNED NEGATIVE — the NAMED form must be left alone, not mis-parsed. `GridLine` has no
    //    ident, so the only alternatives are "ignore it" and "silently turn it into Auto at some
    //    other cell". With one item and no `grid-template-areas`, auto-placement puts it in cell
    //    1/1; the row asserts that, which is what an UNPARSED declaration produces and what a
    //    mis-parse into `Line(0)` or a span would NOT.
    let (f, q) = (rect(&page, "f"), rect(&page, "p6"));
    assert!(
        near(f.x - q.x, 0.0)
            && near(f.y - q.y, 0.0)
            && near(f.width, 100.0)
            && near(f.height, 50.0),
        "G_GRID_AREA_SHORTHAND #f: `grid-area: someName` is not representable and must be left \
         UNPARSED, so the item auto-places into cell 1/1 at [0 0 100x50]; got \
         [{:.0} {:.0} {:.0}x{:.0}]. A name silently becoming a line number places the item \
         somewhere the author never asked for.",
        f.x - q.x,
        f.y - q.y,
        f.width,
        f.height
    );
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// N1  drop the `"grid-area"` arm (the pre-tick behaviour)
//       -> every item auto-places into cell 1/1: `a`, `b` and `c` read [0 0 100x50], `d` and `e`
//          read [0 0 100x50] instead of spanning. The NEGATIVE row stays green, because an unparsed
//          named form is what it asserts — which is why it cannot carry this gate alone.
// N2  read the values as row/row/column/column pairs instead of row/col/row/col
//       -> `a` fires first, at [200 50 10x50]: with four values the mis-grouping is visible on the
//          full form too. ⚠ The ledger's first draft predicted `c` alone, on the reasoning that the
//          other rows are symmetric — true of the ONE- and TWO-value rows, false of the four-value
//          ones. `c` remains the row that catches the transposition when only one value is given,
//          which no other row can.
