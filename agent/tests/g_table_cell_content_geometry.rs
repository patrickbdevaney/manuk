//! **G_TABLE_CELL_CONTENT_GEOMETRY — a cell's height and its baseline both have to account for
//! content that produces NO LINE BOX, and neither did.**
//!
//! `vertical-align: baseline` is the initial value for a table cell, and `td { vertical-align:
//! baseline }` sits in the reset sheet of four of thirty-nine sampled CrUX sites, so a row's cells
//! align their first-line baselines with each other constantly. t933 built that alignment — for
//! cells that *have* a first line box. A cell whose content is a **block** (an icon `<div>`, a
//! spacer, a `display:block` image) produces none, and the code said so out loud:
//!
//! > *"A cell with no line box at all (an empty cell) has no baseline to contribute and does not
//! > join the max."*
//!
//! ⭐⭐⭐ **"NO LINE BOX" AND "NOTHING IN THE CELL" ARE TWO DIFFERENT ANSWERS, AND THAT COMMENT
//! RETURNED THE SECOND ONE FOR BOTH.** Chrome joins a block-content cell to its row's baseline
//! group, synthesizing the cell's baseline from the bottom edge of its content (CSS 2.1 §17.5.4).
//! The empty cell really does stay out — but it is a *different* case, and collapsing them left the
//! commonest table row on the legacy web (an icon cell beside a label cell) aligned to the wrong
//! thing and the row several pixels short, which then moves every box below the table.
//!
//! The same blind spot has a second, larger face one function away:
//!
//! ⭐⭐⭐ **A TABLE CELL IS A BFC ROOT, SO IT CONTAINS ITS FLOATS — AND `layout_cell` BUILT A FLOAT
//! CONTEXT AND NEVER QUERIED IT.** A cell whose only content is a float collapsed to **zero**:
//! `<td><div style="float:left;height:50px"></div></td>` is a 50px cell in a 50px table in Chrome
//! and was a 0px cell in a 0px table here — the whole row gone. `layout_block` has answered this
//! for every other BFC root since floats were built (`own_bfc.lowest_bottom() - content_y`); this
//! is that same line, at the one BFC root that constructed the context and never asked it.
//!
//! ## Every row below is headless-Chrome-measured (`16px/24px monospace`, 400px wrapper)
//!
//! ```text
//!                                                   b dy   label dy   table h
//!   1  50px BLOCK child  |  text label                  0        35        57   <- was 2 / 50
//!   2  EMPTY cell h=50   |  text label                  0         2        50      NEGATIVE
//!   3  50px <img> child  |  text label                  0        35        57      CONTROL
//!   4  BLOCK + overflow:hidden on the cell              0        35        57   <- was 2 / 50
//!   5  40px/60px text    |  text label                  7        29        60      CONTROL
//!   6  BLOCK, cell padding:10 border:3                 13        48        76   <- was 2
//!   7  SHORT 6px block   |  text label                 11         2        24   <- was 0
//!   8  BLOCK 50, cell forced height:200                 0        35       200   <- was 2
//!   9  TWO blocks 20+30  |  text label                  0        35        57   <- was 2 / 50
//!  10  BLOCK 50 + margin-bottom:25                      0        60        82   <- was 2 / 50
//!  11  FLOAT-only cell h=60  |  text label              0        35        60   <- was 2
//!  12  FLOAT-only cell, no height (cell / table)             50 / 50            <- was 0 / 0
//! ```
//!
//! ## The three distinctions the rows exist to pin, none of which is guessable
//!
//! 1. **It is the NATURAL content height, not the used one** (row 8). A cell forced to
//!    `height: 200px` around a 50px block still has its baseline at 50 — the neighbour's label
//!    lands at 35, not 185. This is the same distinction t933's free-space calculation already had
//!    to make, and reading the box height back gets it wrong for exactly the cells with the most
//!    free space.
//! 2. **It is the bottom MARGIN edge of the content** (row 10). A block with `margin-bottom: 25px`
//!    puts the row's baseline at 75, not 50 — which falls out of asking the cell's own BFC height
//!    rather than the last child's border box.
//! 3. **The alignment moves BOTH ways** (row 7). When the text cell has the deeper baseline it is
//!    the BLOCK that must come down — to y=11, not stay at 0. A fix that only pushes text down
//!    passes rows 1/4/6/8/9/10 and fails this one.
//!
//! ⚠ Row 2 is a PINNED NEGATIVE and it is why the synthesis is conditional. Giving an empty cell a
//! baseline of 0 makes it demand *its own height plus the row's whole baseline shift* and grows the
//! row by that much; Chrome keeps that row at its declared 50. An empty cell has no baseline to
//! contribute and nothing to align — a different statement from "a cell with no line box", and
//! telling the two apart is the whole tick.
//!
//! ⚠ Rows 3 and 5 are CONTROLS on the path that already worked: an `<img>` is an atomic *inline*,
//! so it makes a line box and reaches the baseline through `first_line_baseline` rather than
//! through the synthesis. They must not move, or the fix has replaced the working path instead of
//! adding to it.
//!
//! ⚠ Row 5 carries an explicit `line-height: 60px` beside its `font-size: 40px`, and that is not
//! decoration. Left to inherit the 24px line-height, 40px text overflows its own line box upward —
//! Chrome puts it at **-11** and this engine at **+1**, a real divergence in line-box overflow that
//! has nothing to do with table cells. NAMED, MEASURED, NOT BUILT (Chrome -11 / 11 / 33; ours
//! +1 / … ). A control that can be reddened by an unrelated mechanism is not a control.
//!
//! ⚠⚠ **THE FIXTURE STATES `font-family`/`font-size`/`line-height` AS LONGHANDS ON PURPOSE.** It
//! was written with the `font: 16px/24px monospace` shorthand and this gate read a **54px** row
//! where Chrome and the shipping (Stylo) pipeline both read 57 — `manuk-agent` takes `manuk-page`
//! with DEFAULT features, so a gate in this crate runs on `MinimalCascade`, and the two cascades do
//! not agree about the shorthand's `line-height`. That disagreement is real and is recorded as its
//! own finding; it is not what this gate is about, so the confound is removed rather than measured
//! here. A table gate that can be reddened by a font shorthand is not a table gate.
//!
//! ## ⚠ Why a TABLE gate lives in `agent/tests/`
//!
//! It needs the real `manuk_page::Page` pipeline (`engine/layout`'s own tests run on
//! `MinimalCascade` and this crate's font stack, so the Chrome numbers above would not transfer),
//! and the wall's crate list — `manuk-css manuk-layout manuk-paint manuk-dom manuk-net manuk-agent
//! manuk-shell` — does **not** include `manuk-page`: a gate under `engine/page/tests/` runs only if
//! `scripts/verify.sh` names it in an explicit `_launch` line. That is not hypothetical. The
//! sibling gate `g_table_cell_valign` has no such line, and this tick found it RED and drifting for
//! twenty-three days behind green walls. `scripts/` is observer-owned, so the gate is placed where
//! the wall already looks rather than the wall being changed to look here.
//!
//! PROVEN RED by four mutations, each with a different message — see the module tail.

use manuk_dom::NodeId;
use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
html,body{margin:0;padding:0}
body{font-family:monospace;font-size:16px;line-height:24px}
*{box-sizing:border-box}
table{border-collapse:separate;border-spacing:0}
td{padding:0;vertical-align:baseline}
.w{width:400px;margin:0 0 6px 0}
.blk{width:20px;height:50px;background:#333}
</style></head><body>
<div class="w" id="c1"><table><tr><td><div class="blk" id="b1"></div></td><td><span id="a1">label</span></td></tr></table></div>
<div class="w" id="c2"><table><tr><td style="height:50px" id="b2"></td><td><span id="a2">label</span></td></tr></table></div>
<div class="w" id="c3"><table><tr><td><img id="b3" src="data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7" style="width:20px;height:50px"></td><td><span id="a3">label</span></td></tr></table></div>
<div class="w" id="c4"><table><tr><td style="overflow:hidden"><div class="blk" id="b4"></div></td><td><span id="a4">label</span></td></tr></table></div>
<div class="w" id="c5"><table><tr><td style="font-size:40px;line-height:60px"><span id="b5">A</span></td><td><span id="a5">label</span></td></tr></table></div>
<div class="w" id="c6"><table><tr><td style="padding:10px;border:3px solid"><div class="blk" id="b6"></div></td><td><span id="a6">label</span></td></tr></table></div>
<div class="w" id="c7"><table><tr><td><div style="width:20px;height:6px;background:#333" id="b7"></div></td><td><span id="a7">label</span></td></tr></table></div>
<div class="w" id="c8"><table><tr><td style="height:200px"><div class="blk" id="b8"></div></td><td><span id="a8">label</span></td></tr></table></div>
<div class="w" id="c9"><table><tr><td><div style="height:20px;background:#333" id="b9"></div><div style="height:30px;background:#999"></div></td><td><span id="a9">label</span></td></tr></table></div>
<div class="w" id="c10"><table><tr><td><div style="height:50px;margin-bottom:25px;background:#333" id="b10"></div></td><td><span id="a10">label</span></td></tr></table></div>
<div class="w" id="c11"><table><tr><td style="height:60px"><div style="float:left;width:20px;height:50px;background:#333" id="b11"></div></td><td><span id="a11">label</span></td></tr></table></div>
<div class="w" id="c12"><table><tr><td id="b12"><div style="float:left;width:20px;height:50px;background:#333"></div></td></tr></table></div>
</body></html>"##;

/// The node carrying `id`. A plain DOM walk rather than the selector engine, so this stays a
/// LAYOUT gate: a change in selector matching cannot turn a row of it green or red.
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
    let n = by_id(page, id);
    *page
        .root_box
        .node_rects(page.dom())
        .get(&n)
        .unwrap_or_else(|| panic!("VACUOUS: no box for id={id:?}"))
}

/// The `<table>` inside wrapper `#c<n>` — the row's height, which is the half of the defect a
/// per-element `dy` cannot see.
fn table_in(page: &manuk_page::Page, wrapper: &str) -> manuk_layout::Rect {
    let dom = page.dom();
    let w = by_id(page, wrapper);
    let t = dom
        .descendants(w)
        .find(|&n| dom.tag_name(n) == Some("table"))
        .unwrap_or_else(|| panic!("VACUOUS: no <table> under #{wrapper}"));
    *page
        .root_box
        .node_rects(dom)
        .get(&t)
        .unwrap_or_else(|| panic!("VACUOUS: no box for the table under #{wrapper}"))
}

#[test]
fn a_cell_without_a_line_box_still_has_a_height_and_a_baseline() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://tcg.test/", &fonts, 1200.0);
    let near = |got: f32, want: f32| (got - want).abs() < 1.6;

    // ── VACUITY. Twelve wrappers, each holding a laid-out table with a real box. A page that laid
    // none of this out would sail through every row below on `0.0 == 0.0`.
    for n in 1..=12 {
        let t = table_in(&page, &format!("c{n}"));
        assert!(
            t.height > 0.0 && t.width > 0.0,
            "VACUOUS: table {n} has no box ({t:?})"
        );
    }

    // (row, first cell's content dy, label dy, table height) — every number Chrome-measured.
    let rows: &[(usize, f32, f32, f32)] = &[
        (1, 0.0, 35.0, 57.0),
        (2, 0.0, 2.0, 50.0),
        (3, 0.0, 35.0, 57.0),
        (4, 0.0, 35.0, 57.0),
        (5, 7.0, 29.0, 60.0),
        (6, 13.0, 48.0, 76.0),
        (7, 11.0, 2.0, 24.0),
        (8, 0.0, 35.0, 200.0),
        (9, 0.0, 35.0, 57.0),
        (10, 0.0, 60.0, 82.0),
        (11, 0.0, 35.0, 60.0),
    ];
    for (n, want_b, want_a, want_h) in rows {
        let wrap = rect(&page, &format!("c{n}")).y;
        let b = rect(&page, &format!("b{n}")).y - wrap;
        let a = rect(&page, &format!("a{n}")).y - wrap;
        let h = table_in(&page, &format!("c{n}")).height;
        assert!(
            near(b, *want_b),
            "G_TABLE_CELL_CONTENT_GEOMETRY row {n}: the FIRST cell's content belongs at y={want_b} \
             (Chrome), not {b}. Row 7 is the one that reads 0 when the alignment only ever pushes \
             the TEXT down and never brings a block cell down to meet a deeper baseline."
        );
        assert!(
            near(a, *want_a),
            "G_TABLE_CELL_CONTENT_GEOMETRY row {n}: the LABEL belongs at y={want_a} (Chrome), not \
             {a}. y=2 is the top of its cell — the block cell contributed no baseline, so the row \
             aligned to the text alone."
        );
        assert!(
            near(h, *want_h),
            "G_TABLE_CELL_CONTENT_GEOMETRY row {n}: the table is {want_h} tall in Chrome, not {h}. \
             A row aligned to a deeper baseline must GROW by the shift, or every box below the \
             table moves up."
        );
    }

    // ── ROW 12 · THE FLOAT-ONLY CELL — the BFC half rather than the baseline half. Its own row
    // because the failure is total: the cell and the table both measured ZERO.
    let cell = rect(&page, "b12");
    let table = table_in(&page, "c12");
    assert!(
        near(cell.height, 50.0) && near(table.height, 50.0),
        "G_TABLE_CELL_CONTENT_GEOMETRY row 12: a cell whose only content is a 50px float is 50 tall \
         in a 50-tall table (Chrome); got cell {} / table {}. A table cell establishes a BFC and so \
         CONTAINS its floats — `layout_cell` builds the float context and must also ask it how far \
         down the floats went.",
        cell.height,
        table.height
    );
}

// ── HOW THIS GOES RED (each mutation gives a different message) ────────────────────────────────
//
// N1  Drop the synthesis (`first_line_baseline(...)` with no `.or_else`) — the pre-tick code.
//     → rows 1/4/6/8/9/10/11 read label y=2, and 1/4/9/10 also read a short table.
// N2  Synthesize unconditionally (`Some(frame_top + natural_ch)`, no `> 0.0` guard).
//     → row 2, the empty cell, grows its table from 50 to 67.
// N3  Synthesize from the USED height (the cell's border box) rather than the natural content
//     height. → row 6 fires first, putting the label at 61 instead of 48 (the cell's border box is
//     76 where its content ends at 63); row 8, the cell forced to 200px, fails the same way.
// N4  Drop `ch.max(floats.lowest_bottom() - content_y)` in `layout_cell`.
//     → row 12 collapses to 0/0 and row 11's label falls back to y=2.
