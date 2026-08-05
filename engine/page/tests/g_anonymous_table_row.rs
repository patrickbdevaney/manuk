//! # G_ANONYMOUS_TABLE_ROW — a bare `table-cell` gets an ANONYMOUS ROW, or it disappears
//!
//! ```css
//! .outer { display: table; height: 100px }              /* the pre-flexbox centring idiom */
//! .inner { display: table-cell; vertical-align: middle } /* …with NO table-row in between */
//! ```
//!
//! CSS 2.1 §17.2.1 generates an **anonymous table-row** around a `table-cell` whose parent is a
//! `table`. `collect_table_rows` recognised only `table-row` and `table-row-group`, so a bare cell
//! matched no arm and **was dropped on the floor**. The table then had no rows at all, took the
//! rowless shrink-to-fit path, and collapsed onto its own text.
//!
//! **This is not a geometry near-miss — it is a 392px container-width error and, in one arrangement,
//! a MISSING_BOX.** Which makes it the burndown's family #1 (`docs/loop/PHASE0-RENDER-BURNDOWN.md`
//! §3.1: container-WIDTH errors launder into wrap/line-count → dy) in its grossest available form: a
//! container that should be 400px wide is 8px, so every line of prose inside it re-wraps and the
//! whole subtree's height is wrong.
//!
//! It looks exotic and is not. `display:table` + `display:table-cell` with no row between them is
//! the pre-flexbox **vertical-centring** idiom and the **equal-height-columns** idiom, and both are
//! everywhere in the legacy/CMS markup that makes up the CrUX tail this corpus is moving toward.
//!
//! ## Chrome-measured on THIS fixture
//!
//! `google-chrome --headless --hide-scrollbars --window-size=1200,800`, `16px/1.5 sans-serif`, a
//! 400px `display:table`. Each row pins a *different clause* of the rule:
//!
//! ```text
//!                                                     Chrome        before        after
//!   two bare cells                              200@0 · 200@200    8@0 · 8@8   200@0 · 200@200
//!   bare · real row · bare  (three rows)        400 · 400 · 400   GONE·400·GONE  400 · 400 · 400
//!   display:table;height:100px + one bare cell     400 wide          8 wide       400 wide
//!   a bare cell's `width:50%` child                  200               4           200
//!   explicit table-row              (CONTROL)        400              400          400
//!   real <table><tr><td>            (CONTROL)        394              394          394
//! ```
//!
//! ⚠⚠ **Two bare cells make ONE anonymous row, not two.** They come out side by side at x=0 and
//! x=200 — the run of consecutive cells is wrapped *together*. The x coordinates are asserted
//! separately from the widths because side-by-side vs stacked is a distinct claim; **measured, the
//! one-row-per-cell mistake is caught by the WIDTH assertion first** (each cell becomes the only
//! one in its row, so it takes the whole 400 rather than 200), which is what the RED proof below
//! actually printed rather than what writing the fixture would have led me to predict.
//!
//! ⚠⚠ **A real `table-row` BREAKS the run.** `bare · row · bare` is three rows in document order,
//! not one anonymous row of two cells plus a row. That is why the accumulator is flushed when a real
//! row or row-group is seen, and the middle row of that fixture is what measures it.
//!
//! ⚠ **The anonymous row carries `None` for its node**, which is what an anonymous box is: no style
//! lookup, so no background of its own, and no node on the emitted `LayoutBox`. The consumer already
//! took an `Option<NodeId>` — it was written for the `<tr>`-has-real-geometry fix — so the anonymous
//! case slots into the shape that was already there.
//!
//! ## How this goes RED
//!
//! - **Restore the `_ => {}` arm for `Display::TableCell`** (i.e. stop accumulating bare cells) →
//!   every ✗→✓ row snaps back to 8 / 4 / GONE while both CONTROLs stay green. Verified.
//! - **Push each bare cell as its own row** (`rows.push((None, vec![child]))` instead of
//!   accumulating) → the two bare cells report **400 and 400** instead of 200 and 200, because each
//!   is now the only cell in its row. Verified. This is the plausible wrong fix — it satisfies
//!   "a bare cell is no longer dropped" completely and gets the arrangement wrong.
//! - **Drop the flush before a real row** → `bare · row · bare` becomes two rows, the leading and
//!   trailing cells share one anonymous row, and `#run_a` reports **200** instead of 400. Verified.
//!
//! ## NOT covered, named with its number
//!
//! **A cell does not STRETCH to fill a taller table.** `display:table; height:100px` with one bare
//! cell is 400×**24** here against Chrome's 400×**100**: the width is now exact and the height is
//! not. That is the height-distribution residue t908 and t925 both named on real `<table>` markup
//! (`a <td> does not stretch to fill a taller table`) — the same missing algorithm reached by a
//! different door, not a new defect and not something this fix was ever going to close. The
//! `vertical-align: middle` half of the centring idiom therefore still does not centre; the box is
//! merely the right width now instead of 2% of it.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
html,body{margin:0;padding:0;font:16px/1.5 sans-serif}
.cb{width:400px}
</style></head><body>
<div class="cb" style="display:table"><div style="display:table-cell" id="two_a">x</div><div style="display:table-cell" id="two_b">y</div></div>

<div class="cb" style="display:table"><div style="display:table-cell" id="run_a">x</div><div style="display:table-row"><div style="display:table-cell" id="run_b">y</div></div><div style="display:table-cell" id="run_c">z</div></div>

<div class="cb" style="display:table;height:100px"><div style="display:table-cell;vertical-align:middle" id="mid">x</div></div>

<div class="cb" style="display:table"><div style="display:table-cell"><div id="pct" style="width:50%">x</div></div></div>

<div class="cb" style="display:table"><div style="display:table-row"><div style="display:table-cell" id="ctl_row">x</div></div></div>
<table class="cb"><tr><td><div id="ctl_tbl">x</div></td></tr></table>
</body></html>"##;

fn rect_of(page: &manuk_page::Page, sel: &str) -> manuk_layout::Rect {
    let dom = page.dom();
    let n = manuk_css::query_selector_all(dom, dom.root(), sel)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("selector {sel} matched nothing"));
    *page.root_box.node_rects(dom).get(&n).unwrap_or_else(|| {
        panic!(
            "G_ANONYMOUS_TABLE_ROW: no box for {sel} — the element generated NONE AT ALL, \
                    which is the before-state for a bare cell that follows a real row"
        )
    })
}

#[test]
fn g_anonymous_table_row() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://atr.test/", &fonts, 1200.0);

    // ── TWO BARE CELLS SHARE ONE ANONYMOUS ROW. The x coordinates are the claim, not the widths: a
    //    fix that gave each cell its own row produces the right widths and stacks them.
    let a = rect_of(&page, "#two_a");
    let b = rect_of(&page, "#two_b");
    assert!(
        (a.width - 200.0).abs() < 1.01 && (b.width - 200.0).abs() < 1.01,
        "G_ANONYMOUS_TABLE_ROW: two bare `table-cell` children of a 400px `display:table` split the \
         table evenly (Chrome: 200 each). Got {} and {} — before the fix both were 8, the width of \
         the letter they contain, because the cells were dropped and the table shrink-to-fit.",
        a.width,
        b.width
    );
    assert!(
        a.x < 1.01 && (b.x - 200.0).abs() < 1.01,
        "G_ANONYMOUS_TABLE_ROW: consecutive bare cells belong to ONE anonymous row and sit SIDE BY \
         SIDE (Chrome: x=0 and x=200). Got x={} and x={}. Equal x means each cell was wrapped in \
         its own row — the plausible wrong fix, which every width assertion above still passes.",
        a.x,
        b.x
    );

    // ── A REAL ROW BREAKS THE RUN: bare · row · bare is THREE stacked rows in document order.
    let ra = rect_of(&page, "#run_a");
    let rb = rect_of(&page, "#run_b");
    let rc = rect_of(&page, "#run_c");
    for (sel, r) in [("#run_a", ra), ("#run_b", rb), ("#run_c", rc)] {
        assert!(
            (r.width - 400.0).abs() < 1.01,
            "G_ANONYMOUS_TABLE_ROW: `{sel}` is the only cell in its row and spans the 400px table. \
             Got {}.",
            r.width
        );
    }
    assert!(
        ra.y < rb.y && rb.y < rc.y,
        "G_ANONYMOUS_TABLE_ROW: `bare cell · real table-row · bare cell` is THREE rows in document \
         order (Chrome y = 24 / 48 / 72). Got y = {} / {} / {}. If the accumulator is not flushed \
         when a real row is seen, the trailing bare cell joins the leading one and the order breaks.",
        ra.y,
        rb.y,
        rc.y
    );

    // ── The centring idiom, and the percentage child that first exposed this.
    let mid = rect_of(&page, "#mid");
    assert!(
        (mid.width - 400.0).abs() < 1.01,
        "G_ANONYMOUS_TABLE_ROW: the `display:table` + `display:table-cell; vertical-align:middle` \
         centring idiom — the cell spans the 400px table (Chrome). Got {}. NOTE its HEIGHT is still \
         24 against Chrome's 100: a cell does not stretch to fill a taller table, which is the \
         t908/t925 height-distribution residue and deliberately NOT asserted here.",
        mid.width
    );
    let pct = rect_of(&page, "#pct");
    assert!(
        (pct.width - 200.0).abs() < 1.01,
        "G_ANONYMOUS_TABLE_ROW: a `width:50%` child of a bare cell resolves against the cell's 400px \
         (Chrome: 200). Got {}. This is the symptom that found the defect — it was 4px, because the \
         percentage resolved against a container that had collapsed to its own text. A 392px \
         container-width error re-wraps every line inside it: burndown family #1 in its grossest form.",
        pct.width
    );

    // ── WHAT MUST NOT MOVE. The explicit-row and real-`<table>` paths were always correct and are
    //    what a fix that mishandles the flush breaks.
    let cr = rect_of(&page, "#ctl_row");
    assert!(
        (cr.width - 400.0).abs() < 1.01,
        "G_ANONYMOUS_TABLE_ROW: an EXPLICIT `display:table-row` was always correct and must stay so \
         — got {}. This is the discriminator that identified the mechanism.",
        cr.width
    );
    let ct = rect_of(&page, "#ctl_tbl");
    assert!(
        (ct.width - 394.0).abs() < 1.01,
        "G_ANONYMOUS_TABLE_ROW: real `<table><tr><td>` markup is untouched — Chrome-measured 394 on \
         THIS fixture (400 less the UA cell padding and border), got {}.",
        ct.width
    );
}
