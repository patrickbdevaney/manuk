//! # G_TABLE_CELL_VALIGN — a cell is STRETCHED to its row, and stretching a box does not move what is in it
//!
//! A table cell is laid out at its own content height and then stretched to the row's height. That
//! stretch is a single assignment to `rect.height`, and it does not move the cell's children — so
//! **every cell was top-aligned**, whatever `vertical-align` said. On a table cell that property is
//! the pre-flexbox vertical-centring idiom and is still everywhere: toolbars, data grids, icon+label
//! rows, and the whole `display: table` / `display: table-cell` centring pattern.
//!
//! ```text
//!                                                        Chrome     before      after
//!   vertical-align:middle, 19px word in a 60px cell        [20]       [ 2]       [20]
//!   vertical-align:bottom, same cell                       [38]       [ 2]       [38]
//!  ── CONTROLS, none of which moved ──
//!   vertical-align not declared (UA middle, t1360)         [20]       [ 2]       [20]
//!   vertical-align:baseline, single line (t1360)           [ 2]        --        [ 2]
//!   the CELL BOX of a row sized by its tallest cell        [ 0]       [ 0]   unchanged
//!   two `middle` cells in one row agree with each other    [20]       [ 2]       [20]
//! ```
//!
//! ## The trap, and it cost a build to find
//!
//! The obvious implementation reads the free space as `row_height − cell_box_height`. That is **zero
//! for exactly the cells that have the most free space**: a cell with `height: 60px` around a 24px
//! line reports a *border-box* height of 60, because the explicit height was already applied when
//! the cell was laid out. The free space has to be measured against the cell's **natural content
//! height** — the height its children actually came out — which `layout_cell` now returns alongside
//! the border-box height for that reason.
//!
//! > A first version of this fix compiled, ran, and moved nothing, because it asked the box how tall
//! > it was instead of asking the content. **`height` on a box you have already sized tells you what
//! > you asked for, not what is inside it.**
//!
//! ## The shift is applied to the CONTENT, not to the cell
//!
//! The subtree is translated and the box's own origin is then restored, so the cell's background,
//! borders and hit rect keep the row's geometry while its children move within it. A cell that
//! moved as a whole would take its background off the row.
//!
//! ## What `baseline` does here, stated rather than assumed
//!
//! `baseline` is the CSS initial value for a cell and aligns the first lines of a row's cells with
//! each other. It is approximated as `top`, which is what it degrades to for a single-line row and
//! what this code already did — so the control row below is unchanged, and the approximation is
//! named rather than silently shipped as an implementation.
//!
//! ## NAMED, MEASURED, NOT BUILT — three more table defects from the same battery
//!
//! The sixteen-row table battery that found this found **five** divergences in **four** mechanisms.
//! The other three are each their own tick and are recorded here so the next one starts from a
//! measurement rather than a hunt (fixture `/tmp/tbl.html`):
//!
//! ```text
//!   ROWSPAN row-height distribution   a 60px rowspan=2 cell must give 30/30 to its two rows;
//!                                     we give 24/36 — the overflow all lands on the LAST row
//!                                     (Chrome [10,0,10,30] + [10,30,10,30]; ours 24 + 36)
//!   CAPTION                           `<caption>` reserves no space and does not widen the
//!                                     table: the first cell belongs at y=20 and 29 wide,
//!                                     reads y=0 and 10 wide
//!   THEAD ORDERING                    a `<thead>` written AFTER a `<tbody>` must still render
//!                                     FIRST; we render in source order (y=24, Chrome y=0)
//! ```
//!
//! The rowspan one is what `CONSTITUTION.MD` VI.2 has carried as *"t933 row-height distribution"*
//! since check #82.
//!
//! ## How this goes RED
//!
//! - **Delete the `shift` block** → both defect rows read 2; every control passes.
//! - **Measure the free space as `cell_h − cbox.rect.height`** (the first version of this fix) →
//!   both defect rows read 2 again, and the code looks correct. That is the recipe worth keeping.
//! - **Apply the shift to the cell instead of its content** (drop the `rect.y` restore) → only the
//!   CELL-BOX control fails, reading y=18. ⚠ The first version of this gate did **not** catch this:
//!   every other row measures a `<span>` inside the cell, which moves under either rule, and the
//!   recipe came back green. The control that separates them had to be added after the RED refused
//!   to fire.
//! - **Give `Middle` the whole free space rather than half** → only the `middle` row fails, at 38.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
html,body{margin:0;padding:0}
body{font:16px/1.5 monospace}
*{box-sizing:border-box}
table{border-collapse:separate;border-spacing:0}
td{padding:0}
.w{width:400px;margin:0 0 6px 0}
</style></head><body>
<div class="w" id="c1"><table><tr><td style="height:60px;vertical-align:middle"><span id="a1">x</span></td></tr></table></div>
<div class="w" id="c2"><table><tr><td style="height:60px;vertical-align:bottom"><span id="a2">x</span></td></tr></table></div>
<div class="w" id="c3"><table><tr><td style="height:60px"><span id="a3">x</span></td></tr></table></div>
<div class="w" id="c4"><table><tr><td style="height:50px">a</td><td id="a4">b</td></tr></table></div>
<div class="w" id="c5"><table><tr><td style="height:60px;vertical-align:middle"><span>x</span></td><td style="height:60px;vertical-align:middle"><span id="a5">y</span></td></tr></table></div>
<div class="w" id="c6"><table><tr><td style="height:60px;vertical-align:middle" id="a6"><span>x</span></td></tr></table></div>
<div class="w" id="c7"><table><tr><td style="height:60px;vertical-align:baseline"><span id="a7">x</span></td></tr></table></div>
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
fn g_table_cell_valign() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://tv.test/", &fonts, 1200.0);
    let r = |sel: &str| rect_of(&page, sel);
    let dy = |sel: &str, w: &str| r(sel).y - r(w).y;
    let near = |got: f32, want: f32| (got - want).abs() < 1.6;

    // ── DEFECT — `vertical-align` on a table cell. A 19px word in a 60px cell: the 24px line box
    //    has 36px of free space, so `middle` puts it 18 down (plus 2px of half-leading) and
    //    `bottom` 36 down.
    assert!(
        near(dy("#a1", "#c1"), 20.0),
        "G_TABLE_CELL_VALIGN: `vertical-align:middle` in a 60px cell puts a 19px word at y=20, not \
         {}. y=2 is the top of the cell — the cell was STRETCHED to the row and stretching a box \
         does not move what is inside it.",
        dy("#a1", "#c1")
    );
    assert!(
        near(dy("#a2", "#c2"), 38.0),
        "G_TABLE_CELL_VALIGN: `vertical-align:bottom` puts it at y=38, not {}. This row is also what \
         separates a correct `middle` from one that consumes the whole free space.",
        dy("#a2", "#c2")
    );

    // ── ⚠⚠⚠ CONTROL A WAS WRONG, AND IT WAS WRONG IN THE DIRECTION THAT PINS A BUG. It asserted
    //    y=2 on the reasoning that *"the CSS initial value for a cell is `baseline`, approximated
    //    here as `top`"*. The CSS initial value is not what a `<td>` computes: Chrome's UA sheet is
    //    `tbody { vertical-align: middle }` + `tr, td { vertical-align: inherit }`, so a plain cell
    //    computes **middle**. Measured in headless Chrome on this exact fixture:
    //
    //    ```text
    //      #a3   dy = 20        getComputedStyle(td).verticalAlign = "middle"
    //            tbody = middle    tr = middle
    //    ```
    //
    //    This row is a prose-derived value that was never measured, and because
    //    `g_table_cell_valign` is not in `scripts/verify.sh`'s launch list it went red silently the
    //    moment the engine became correct and stayed red behind green walls. The row now asserts
    //    Chrome's answer.
    assert!(
        near(dy("#a3", "#c3"), 20.0),
        "G_TABLE_CELL_VALIGN: a `<td>` with no `vertical-align` computes `middle` from the UA sheet \
         (tbody:middle + tr/td:inherit), so the content is CENTRED at y=20, not {}.",
        dy("#a3", "#c3")
    );

    // ── CONTROL A2 — the row CONTROL A was reaching for, spelled so it means it. An EXPLICIT
    //    `vertical-align: baseline` on a single-line cell degrades to `top`, and Chrome measures
    //    y=2. This is what fails if a fix ever centres unconditionally — which is the check
    //    CONTROL A was written to be and could not perform, because the value it asserted was the
    //    same one an unconditional `top` produces.
    assert!(
        near(dy("#a7", "#c7"), 2.0),
        "G_TABLE_CELL_VALIGN: an explicit `vertical-align:baseline` on a single-line cell aligns \
         with the top — y=2 (half-leading), not {}.",
        dy("#a7", "#c7")
    );

    // ── CONTROL B — a row whose height comes from its TALLEST cell, with no explicit height on the
    //    cell being measured. The free space here is real but the alignment is the default, and this
    //    is the row that fails if the shift were applied to the CELL instead of to its content.
    assert!(
        near(dy("#a4", "#c4"), 0.0),
        "G_TABLE_CELL_VALIGN: a cell in a 50px row keeps its own box at the row's top — y=0, not \
         {}. The shift moves a cell's CONTENT; the cell itself must keep the row's geometry or its \
         background leaves the row.",
        dy("#a4", "#c4")
    );

    // ── CONTROL C — the content of two middle-aligned cells in one row lands at the SAME y. A
    //    per-cell computation that used the wrong height for one of them would split them.
    //    ⚠ The marker is a `<span>` INSIDE the second cell, not the cell: the cell box itself
    //    correctly stays at the row's top, which is what control B pins.
    assert!(
        near(dy("#a5", "#c5"), 20.0),
        "G_TABLE_CELL_VALIGN: two `middle` cells in one row align with each other at y=20, not {}.",
        dy("#a5", "#c5")
    );

    // ── CONTROL D — the CELL BOX of a cell that DOES shift. This is the only row that separates
    //    "move the content" from "move the cell", and the first version of this gate did not have
    //    it: every other row measures a `<span>` INSIDE the cell, which moves under either rule.
    //    Chrome-measured: the cell keeps the row's origin and the row's full height.
    let cell = r("#a6");
    let wrap = r("#c6");
    assert!(
        near(cell.y - wrap.y, 0.0) && near(cell.height, 60.0),
        "G_TABLE_CELL_VALIGN: a `middle`-aligned cell's own BOX stays at the row's top and keeps the \
         row's height — [0, 60], not [{}, {}]. If the cell moved instead of its content, its \
         background, borders and hit rect would leave the row while looking correct on every row \
         that measures the text.",
        cell.y - wrap.y,
        cell.height
    );
}
