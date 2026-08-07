//! **G_BORDER_COLLAPSE — in `border-collapse: collapse` the cells SHARE their borders, and the
//! shared width is the `max` of the ones that meet, halved on each side.**
//!
//! We gave every cell its full border on all four sides and shared nothing, so a collapsed table was
//! `(n + 1) × border` too wide and **every column after the first was displaced cumulatively.** That
//! is the exact shape of the worst `reading_order` sites in the CrUX sample, which are `<td>`
//! x-divergences — the same family the RTL-column-order fix landed in (`mobile.ir`, 250+ of them).
//!
//! `border-collapse: collapse` is set by every CSS reset and by nearly every real data table.
//!
//! ## The model, and every number in it is Chrome-measured
//!
//! `google-chrome-stable --headless=new --hide-scrollbars --window-size=1200,800`, 15 tables:
//!
//! 1. Each **grid line** — `ncols + 1` vertical, `nrows + 1` horizontal — has ONE width, the `max`
//!    of every border meeting it: the cells on either side, plus the table's own border at the two
//!    outer lines.
//! 2. Each side of the line takes **half** — exact halves, no rounding (`#t15`, 1px borders, lands
//!    on 0.5 and Chrome agrees).
//! 3. The table's **`padding` is ignored** (`#t12`: `padding: 30px` is byte-identical to none).
//!
//! ```text
//!                                                          Chrome        before        after
//!   #t1  two cells, uniform 10px    a1                   [ 5, 5,20,30] [0,0,30,40]  [ 5, 5,20,30]
//!   #t2  a 4px cell beside a 20px   a2                   [ 2,50,22,40] [0,40,18,60] [ 2,50,22,40]
//!   #t3  table border 10 + cell 2   a3                   [ 5,105,16,30][10,110,14,24][5,105,16,30]
//!   #t13 middle line 20px in ROW 1, 2px in row 2   d13   [22,192,30,31]              [22,192,30,31]
//! ```
//!
//! ## Two claims that decide the shape of the rule, and both have a row
//!
//! ⚠⚠⚠ **A GRID LINE IS PER-LINE, NOT PER-SEGMENT.** In `#t13` the middle vertical line is 20px in
//! row 1 and 2px in row 2, and Chrome gives BOTH rows the 20px line — `d13` is inset 10 though its
//! own border is 2 — because a column has to be rectangular. A per-segment reading passes every
//! uniform table and gets `#t11`, `#t13` and every real table with a heavier header row wrong. This
//! is the row that discriminates, and a fixture of uniform tables cannot contain it.
//!
//! ⚠⚠⚠ **CONFLICT RESOLUTION HAS NO GEOMETRIC EFFECT, WHICH IS WHY THIS IS ONE TICK AND NOT THE
//! MULTI-TICK ALGORITHM IT WAS PRICED AS.** CSS 2.1 §17.6.2.1 resolves a collapsing conflict by
//! `hidden` → **wider** → style priority → origin. Width is consulted BEFORE style, so style can
//! only ever break a *width tie* — and two borders of equal width occupy equal space whichever wins.
//! `#t10` proves it: 2px `solid` against 6px `double` gives the 6px geometry with no style-priority
//! rule implemented at all.
//!
//! ## The one thing this does NOT do, stated rather than left to be discovered
//!
//! ⚠⚠ **`border-style: hidden` is NOT built.** It must force its line to zero even against a wider
//! neighbour — Chrome gives a 10px solid cell beside a 10px `hidden` one a width of **15, not 20**.
//! `manuk_css::BorderStyle` has no `Hidden` variant *and stores one style for all four sides*, so
//! honouring it per-side is a cascade change, not a layout one. Building the half that a uniform
//! field can express would make `border-left-style: hidden` silently wrong instead of uniformly
//! unsupported, which is the worse failure. It is the only diverging construct of the 51 rows
//! measured (48 exact, and the other 3 are all this one table).
//!
//! ## RED proof (run, not imagined)
//!
//! `let collapse = false;` in `layout_table` restores the pre-fix engine exactly — one edit, because
//! the whole mechanism hangs off that one flag. Run: **`#a1 is (x, width) (0, 30), Chrome gives
//! (5, 20)`**. The separated-model control rows `#a4`/`#b4` are asserted BEFORE it and keep passing,
//! which is what makes them a control rather than more of the same.
//!
//! ⚠ A partial revert — `None` for the cell sides only, leaving the table's own frame and the column
//! intrinsics collapsed — does NOT reproduce the defect on the inline axis: `#a1` still comes out
//! `(5, 20)` and only the table's HEIGHT goes wrong. Four places had to agree, which is why they were
//! folded onto one flag before this proof was recorded.

use manuk_text::FontContext;

/// Every table varies ONE thing. `#t4` is the SEPARATED model with identical markup: if it moves,
/// the change leaked out of `border-collapse` and into table layout generally.
const HTML: &str = r##"<!doctype html><html><head><style>
 body{margin:0;font:16px/20px monospace}
 table{border-collapse:collapse;margin-bottom:10px}
 td{padding:0;width:10px;height:14px}
 #t1 td,#t9 td{border:10px solid red}
 #t2 td{border:4px solid red} #t2 td.b{border:20px solid red}
 #t3{border:10px solid blue} #t3 td{border:2px solid red}
 /* CONTROL — the separated model, same markup */
 #t4{border-collapse:separate;border-spacing:0} #t4 td{border:2px solid red}
 /* ODD collapsed widths must halve EXACTLY, not round */
 #t15 td{border:1px solid red}
 /* `border-style: none` contributes 0 and loses the max on its own */
 #t7 td{border:10px solid red} #t7 td.b{border:none}
 /* WIDTH decides before STYLE: 6px double beats 2px solid */
 #t10 td{border:2px solid red} #t10 td.b{border:6px double blue}
 /* the SAME column edge, two different widths in two rows — per-LINE, not per-segment */
 #t13 td{border:2px solid red} #t13 td.h{border-width:20px}
 /* the table's own padding is IGNORED in the collapsing model */
 #t12{padding:30px} #t12 td{border:10px solid red}
 /* a cell's padding is KEPT */
 #t14 td{border:10px solid red;padding:7px}
</style></head><body>
<table id="t1"><tr><td id="a1">a</td><td id="b1">b</td></tr></table>
<table id="t2"><tr><td id="a2">a</td><td id="b2" class="b">b</td></tr></table>
<table id="t3"><tr><td id="a3">a</td><td id="b3">b</td></tr></table>
<table id="t4"><tr><td id="a4">a</td><td id="b4">b</td></tr></table>
<table id="t15"><tr><td id="a15">a</td><td id="b15">b</td></tr></table>
<table id="t7"><tr><td id="a7">a</td><td id="b7" class="b">b</td></tr></table>
<table id="t10"><tr><td id="a10">a</td><td id="b10" class="b">b</td></tr></table>
<table id="t13"><tr><td id="a13">a</td><td id="b13" class="h">b</td></tr><tr><td id="c13">c</td><td id="d13">d</td></tr></table>
<table id="t12"><tr><td id="a12">a</td><td id="b12">b</td></tr></table>
<table id="t14"><tr><td id="a14">a</td><td id="b14">b</td></tr></table>
<table id="t9"><tr><td id="a9">a</td><td id="b9">b</td></tr><tr><td id="c9">c</td><td id="d9">d</td></tr></table>
</body></html>"##;

/// `(x, width)` of the element with this id, rounded the way the Chrome probe rounded.
fn xw(page: &manuk_page::Page, id: &str) -> (i64, i64) {
    let r = rect(page, id);
    (r.0, r.2)
}

fn rect(page: &manuk_page::Page, id: &str) -> (i64, i64, i64, i64) {
    let dom = page.dom();
    let n = manuk_css::query_selector_all(dom, dom.root(), &format!("#{id}"))
        .first()
        .copied()
        .unwrap_or_else(|| panic!("#{id} matched nothing"));
    let b = *page
        .root_box
        .node_rects(dom)
        .get(&n)
        .unwrap_or_else(|| panic!("#{id} has no box — it was not laid out at all"));
    (
        b.x.round() as i64,
        b.y.round() as i64,
        b.width.round() as i64,
        b.height.round() as i64,
    )
}

/// `(x, y, w, h)` of `id` with `y` measured from the top of `table` — the tables are stacked, so an
/// ABSOLUTE `y` would encode the fixture's ordering and break the moment a row is added above.
fn rel(page: &manuk_page::Page, id: &str, table: &str) -> (i64, i64, i64, i64) {
    let (x, y, w, h) = rect(page, id);
    (x, y - rect(page, table).1, w, h)
}

fn check(page: &manuk_page::Page, id: &str, want: (i64, i64), why: &str) {
    let got = xw(page, id);
    assert_eq!(
        got, want,
        "G_BORDER_COLLAPSE: #{id} is (x, width) {got:?}, Chrome gives {want:?}.\n  {why}"
    );
}

#[test]
fn a_collapsed_cell_takes_half_of_the_widest_border_meeting_its_grid_line() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://collapse.test/", &fonts, 1200.0);

    // ── 0. THE CONTROL FIRST. The separated model must be untouched, or every row below is
    //    measuring a change to table layout rather than to border collapsing.
    check(
        &page,
        "a4",
        (0, 14),
        "the SEPARATED model with `border-spacing: 0` — each cell keeps its OWN 2px borders and \
         shares nothing, so 2 + 10 + 2. If this moved, the collapse path leaked into the default one",
    );
    check(&page, "b4", (14, 14), "the control's second cell");

    // ── 1. THE UNIFORM CASE. One 10px line between the cells, 5px to each.
    check(
        &page,
        "a1",
        (5, 20),
        "uniform 10px borders: the outer line is 10 (half of it OUTSIDE, belonging to the table), \
         the middle line is max(10, 10) = 10, so the cell is 5 + 10 + 5 — not the 30 it was",
    );
    check(
        &page,
        "b1",
        (25, 20),
        "and the second cell butts against it",
    );
    assert_eq!(
        rel(&page, "t1", "t1"),
        (0, 0, 50, 40),
        "the table box is the outer HALVES plus the cells: 5 + 20 + 20 + 5"
    );

    // ── 2. UNEQUAL BORDERS — the row that proves it is max-then-halve and not anything else.
    check(
        &page,
        "a2",
        (2, 22),
        "a 4px cell beside a 20px one: the middle line is max(4, 20) = 20, half 10, so cell 1 is \
         2 + 10 + 10 = 22. Equal borders cannot tell max-then-halve from any other rule",
    );
    check(&page, "b2", (24, 30), "and the 20px cell is 10 + 10 + 10");

    // ── 3. THE TABLE'S OWN BORDER IS ONE OF THE BORDERS THAT MEET.
    check(
        &page,
        "a3",
        (5, 16),
        "table border 10, cells 2: the outer line is max(10, 2) = 10 — the table takes the outer \
         half and the CELL takes the inner half, so the cell starts 5 in, not 10",
    );
    assert_eq!(
        rel(&page, "t3", "t3").2,
        42,
        "…and the table is 5 + 16 + 16 + 5, not border + cells + border"
    );

    // ── 4. EXACT HALVES. An odd line width lands on .5 and Chrome does not round it away.
    check(
        &page,
        "a15",
        (1, 11),
        "1px borders: the line is 1 and each side takes 0.5, so the cell is 0.5 + 10 + 0.5 = 11 and \
         sits at x=0.5. Rounding the half to a whole pixel gives 10 or 12 and Chrome gives 11",
    );

    // ── 5. `none` LOSES THE MAX ON ITS OWN — no special case needed, its computed width is 0.
    check(
        &page,
        "a7",
        (5, 20),
        "a 10px cell beside a `border: none` one: the middle line is max(10, 0) = 10, so the solid \
         cell is unchanged at 5 + 10 + 5",
    );
    check(
        &page,
        "b7",
        (25, 15),
        "…and the borderless cell takes the inner half of that line and NOTHING on its right, so \
         5 + 10 + 0. Its own right edge is the table's, and both are absent",
    );

    // ── 6. WIDTH DECIDES BEFORE STYLE. This is the claim that makes conflict resolution
    //    geometrically irrelevant, and it is why this landed as one tick.
    check(
        &page,
        "a10",
        (1, 14),
        "2px `solid` against 6px `double`: the WIDER one wins the line regardless of style, so the \
         line is 6 and the cell is 1 + 10 + 3. Style priority only ever breaks a width TIE, and a \
         tie is the same geometry either way",
    );
    check(&page, "b10", (15, 16), "the `double` cell, 3 + 10 + 3");

    // ── 7. PER-LINE, NOT PER-SEGMENT — the discriminating row. Row 2's cells have 2px borders and
    //    still sit on row 1's 20px line, because a column is rectangular.
    check(
        &page,
        "a13",
        (1, 21),
        "row 1: the middle line is max(2, 20, 2, 2) = 20, so this cell is 1 + 10 + 10",
    );
    check(
        &page,
        "d13",
        (22, 30),
        "ROW 2's right-hand cell has a 2px border of its own and is STILL laid out against the 20px \
         line row 1 put there. A per-segment rule gives it x=13 w=13 and passes every uniform \
         table; this is the row that refutes it",
    );
    assert_eq!(
        rel(&page, "c13", "t13"),
        (1, 50, 21, 31),
        "and the HORIZONTAL lines work the same way: the line between the rows is max(2, 20, 2, 2) \
         = 20 so row 2 starts 10 below row 1's box, while the table's bottom line is only 2"
    );

    // ── 8. THE TABLE'S PADDING IS IGNORED, and the CELL'S is not.
    assert_eq!(
        rel(&page, "t12", "t12"),
        (0, 0, 50, 40),
        "`padding: 30px` on a collapsed table changes NOTHING — byte-identical to #t1's 50×40. \
         CSS 2.1 §17.6.2, and measured rather than recalled"
    );
    check(
        &page,
        "a14",
        (5, 34),
        "a cell's own padding is kept: 5 + 7 + 10 + 7 + 5. Dropping the table's padding must not \
         drop the cells'",
    );

    // ── 9. TWO ROWS, so the block axis is asserted and not merely assumed from the inline one.
    assert_eq!(
        rel(&page, "c9", "t9"),
        (5, 35, 20, 30),
        "the second row of a uniform 10px table: the line between the rows is 10, half to each, so \
         row 2's top is 5 below row 1's bottom — the same rule on the other axis"
    );
    assert_eq!(
        rel(&page, "t9", "t9").3,
        70,
        "…and the table is 5 + 30 + 30 + 5 tall"
    );
}
