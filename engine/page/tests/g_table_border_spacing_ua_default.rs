//! # G_TABLE_BORDER_SPACING_UA_DEFAULT — `table { border-spacing: 2px }` is in Chrome's UA sheet
//!
//! ⚠⚠⚠ **THE PROPERTY WAS BUILT, APPLIED AND CHROME-EXACT. ONLY THE DEFAULT WAS ABSENT (t908).**
//! `border-spacing: 10px` matched Chrome to the pixel and always had; `border-spacing: 0` matched;
//! `border-collapse: collapse` matched. What was missing was one declaration in the UA stylesheet,
//! so **a plain `<table>` with no author CSS at all** — most of the data tables on the web — had
//! every cell 4px too wide, flush against the table edge, and the table 4px too short per row.
//!
//! ```text
//!   a 200px table, one `padding:0` cell        Chrome            before
//!     <td>                                     x=2  w=196       x=0  w=200
//!     <table>                                  h=28             h=24
//!     two cells                                100 / 94         103 / 97
//!     two rows                                 h=54             h=48
//! ```
//!
//! > **A capability that is correct whenever anyone asks for it, and wrong when nobody does, is
//! > invisible to every test that sets the property.** Every fixture this engine had for
//! > `border-spacing` declared it. The defect lived exactly where no test looked: the default.
//!
//! Found by a probe aimed at something else — t907's `<table>` rows, which were measured, named as
//! *"the table ALGORITHM rather than the box's own height rule"* and deliberately left for a later
//! tick. Two of those three rows were this one-line UA gap, not an algorithm at all.
//!
//! Every number below is CAPTURED from
//! `google-chrome-stable --headless=new --hide-scrollbars --window-size=1200,800 --dump-dom`.
//!
//! ⚠ **THE GUARDS MATTER AS MUCH AS THE DEFAULT**, because a UA declaration is the easiest kind of
//! change to over-apply: `border-spacing: 0` must still collapse to zero, an author's `10px` must
//! still win, and `border-collapse: collapse` must still ignore spacing entirely. All three are
//! asserted beside the rows that moved.
//!
//! ⚠ **TWO ROWS ARE MEASURED, NAMED AND NOT FIXED.** (1) The two-value form
//! `border-spacing: 10px 20px` drops its VERTICAL component — `ComputedStyle::border_spacing` is a
//! single `f32` fed from `clone_border_spacing().horizontal()`, so the table comes out 44 tall where
//! Chrome says 64. (2) A row/cell does not STRETCH to fill a table given a taller `height`: Chrome
//! gives a single cell in a `height:60px` table 56, and two cells 27 each; ours stay at their content
//! height of 24. That second one is the genuine table height-distribution algorithm.
//!
use manuk_text::FontContext;

const HTML: &str = r##"<!DOCTYPE html><html><head><meta charset="utf-8"><style>
 body{margin:0;font:16px/1.5 sans-serif}
 table{width:200px;background:#cdf}
 td{background:#fda;padding:0}
 .sp8{border-spacing:8px}
</style></head><body>
<table id="u1"><tr><td id="c1">one</td></tr></table>
<table id="u2"><tr><td id="c2">one</td><td id="c3">two</td></tr></table>
<table id="u3" style="border-spacing:0"><tr><td id="c4">one</td></tr></table>
<table id="u4" style="border-spacing:10px"><tr><td id="c5">one</td></tr></table>
<table id="u5" style="border-spacing:10px 20px"><tr><td id="c6">one</td></tr></table>
<table id="u6" style="border-collapse:collapse"><tr><td id="c7">one</td></tr></table>
<table id="u7"><tr><td id="c8">one</td></tr><tr><td id="c9">two</td></tr></table>
<table id="u8" style="height:60px"><tr><td id="c10">one</td></tr></table>
<table id="u9" style="height:60px"><tr><td id="c11">a</td></tr><tr><td id="c12">b</td></tr></table>
<table id="u10"><tr><td id="c13" style="height:50px">tall cell</td></tr></table>
<table id="u11" cellspacing="0"><tr><td id="c14">one</td></tr></table>
<table id="u12" cellspacing="0"><tr><td id="c15">one</td></tr><tr><td id="c16">two</td></tr></table>
<table id="u13" cellspacing="5"><tr><td id="c17">one</td></tr><tr><td id="c18">two</td></tr></table>
<table id="u14" cellspacing="0"><tr><td id="c19">one</td><td id="c20">two</td></tr></table>
<table id="u15"><tr><td id="c21">one</td></tr><tr><td id="c22">two</td></tr></table>
<table id="u16" cellspacing="0" style="border-spacing:6px"><tr><td id="c23">one</td></tr><tr><td id="c24">two</td></tr></table>
<table id="u17" class="sp8" cellspacing="0"><tr><td id="c25">one</td></tr><tr><td id="c26">two</td></tr></table>
<table id="u18" cellspacing="0" cellpadding="4"><tr><td id="c27">one</td></tr></table>
<table id="u19" cellspacing="0" cellpadding="4"><tr><th id="c28">one</th></tr></table>
<table id="u20" cellspacing="0" cellpadding="4"><tr><td id="c29" style="padding:1px">one</td></tr></table>

</body></html>
"##;

fn rect(page: &manuk_page::Page, sel: &str) -> (f32, f32, f32) {
    let dom = page.dom();
    let n = manuk_css::query_selector_all(dom, dom.root(), sel)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("selector {sel} matched nothing"));
    let r = page
        .root_box
        .node_rects(dom)
        .get(&n)
        .copied()
        .unwrap_or_else(|| panic!("no box for {sel}"));
    (r.x, r.width, r.height)
}

/// `x` is asserted RELATIVE to the table, not absolutely: the fixture stacks ten tables, so an
/// absolute `y` would make every claim depend on every claim above it and one real regression would
/// print as twenty-three.
fn c(page: &manuk_page::Page, sel: &str, w: f32, h: f32) {
    let (_, gw, gh) = rect(page, sel);
    assert!(
        (gw - w).abs() < 1.01 && (gh - h).abs() < 1.01,
        "G_TABLE_BORDER_SPACING_UA_DEFAULT: `{sel}` expected w={w} h={h} (CAPTURED from \
         `google-chrome-stable --headless=new --hide-scrollbars --window-size=1200,800`), \
         got w={gw} h={gh}"
    );
}

/// The inset itself, which is the whole point and is invisible in a width: the first cell of a
/// default table starts 2px in from the table's own left edge.
fn inset(page: &manuk_page::Page, table: &str, cell: &str, want: f32) {
    let (tx, _, _) = rect(page, table);
    let (cx, _, _) = rect(page, cell);
    assert!(
        (cx - tx - want).abs() < 1.01,
        "G_TABLE_BORDER_SPACING_UA_DEFAULT: `{cell}` must start {want}px inside `{table}` \
         (Chrome), got {}",
        cx - tx
    );
}

#[test]
fn g_table_border_spacing_ua_default() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://bs.test/", &fonts, 1200.0);
    c(&page, "#c1", 196.0, 24.0);
    c(&page, "#c2", 100.0, 24.0);
    c(&page, "#c3", 94.0, 24.0);
    c(&page, "#c4", 200.0, 24.0);
    c(&page, "#c5", 180.0, 24.0);
    c(&page, "#c6", 180.0, 24.0);
    c(&page, "#c7", 200.0, 24.0);
    c(&page, "#c8", 196.0, 24.0);
    c(&page, "#c9", 196.0, 24.0);
    c(&page, "#c13", 196.0, 50.0);
    c(&page, "#u1", 200.0, 28.0);
    c(&page, "#u2", 200.0, 28.0);
    c(&page, "#u3", 200.0, 24.0);
    c(&page, "#u4", 200.0, 44.0);
    c(&page, "#u6", 200.0, 24.0);
    c(&page, "#u7", 200.0, 54.0);
    c(&page, "#u8", 200.0, 60.0);
    c(&page, "#u9", 200.0, 60.0);
    c(&page, "#u10", 200.0, 54.0);

    // ── t925: the TWO-VALUE form. `border-spacing: 10px 20px` puts 10 between COLUMNS and 20
    // between ROWS, and `ComputedStyle` carried one `f32` fed from `clone_border_spacing()
    // .horizontal()` — so the row inset silently took the column value and the table came out 44
    // where Chrome says 64. The single-value rows above are what make this assertable: they pin
    // that one length still sets both.
    c(&page, "#u5", 200.0, 64.0);

    // ── t1364: `cellspacing` IS THE SHORTHAND AND IT WAS SETTING ONE AXIS.
    //
    // The presentational hint wrote `border_spacing` and not `border_spacing_v`, so the UA sheet's
    // 2px survived on the VERTICAL axis of every `cellspacing="0"` table — which is nearly every
    // legacy table-based page there is. The horizontal axis it did set was exactly right, which is
    // precisely why this read as "cellspacing is supported": half of it was.
    //
    // ```text
    //                                              Chrome   before
    //   #u12  cellspacing="0", TWO ROWS              48       54     ← 2px × 3 gaps survived
    //   #u14  cellspacing="0", TWO CELLS ONE ROW    103/97   103/97  ← CONTROL: the axis that WAS set
    //   #u13  cellspacing="5", two rows              63       53     ← 5×3 + 24×2; a NON-ZERO value
    //                                                                  must set the vertical axis too,
    //                                                                  so the fix cannot be "force 0"
    //   #u15  no attribute at all                    54       54     ← CONTROL: the UA default survives
    //   #u16  cellspacing="0" + style="…:6px"        66       66     ← CONTROL: INLINE style beats a hint
    //   #u17  cellspacing="0" + a .sp8 STYLESHEET rule 72       48     ← CONTROL: so does a plain rule
    // ```
    //
    // ⭐ `#u14` and `#u13` are the pair that names the mechanism. `#u14` says the horizontal axis
    // was already correct, so this is a FORGOTTEN AXIS and not a dead code path; `#u13` says the fix
    // is to propagate the attribute's value, not to special-case zero.
    //
    // ⚠⚠⚠ `#u16` AND `#u17` ARE BOTH NEEDED, AND WRITING ONLY `#u16` IS THE MISTAKE THIS GATE
    // ALMOST SHIPPED. A hint must lose to author CSS, and `#u16` states that with an INLINE
    // `style=` — which is applied last no matter where the hint sits, so it survives the very
    // mutation it was written to catch (hint pushed ABOVE all author rules: `#u16` still passes).
    // `#u17` uses an ordinary stylesheet rule, which is the only form that can actually rank
    // against the hint. **A control has to be able to LOSE**, and the way to find out whether it
    // can is to run the mutation it names and watch it stay green.
    c(&page, "#u11", 200.0, 24.0);
    c(&page, "#c14", 200.0, 24.0);
    c(&page, "#u12", 200.0, 48.0);
    c(&page, "#c15", 200.0, 24.0);
    c(&page, "#u13", 200.0, 63.0);
    c(&page, "#c17", 190.0, 24.0);
    c(&page, "#u14", 200.0, 24.0);
    c(&page, "#c19", 103.0, 24.0);
    c(&page, "#c20", 97.0, 24.0);
    c(&page, "#u15", 200.0, 54.0);
    c(&page, "#u16", 200.0, 66.0);
    c(&page, "#c23", 188.0, 24.0);
    c(&page, "#u17", 200.0, 72.0);
    c(&page, "#c25", 184.0, 24.0);

    // ── THE INSET, stated as a relationship rather than a coordinate.
    inset(&page, "#u1", "#c1", 2.0); // the UA default — the row this tick landed
    inset(&page, "#u3", "#c4", 0.0); // `border-spacing: 0` still collapses
    inset(&page, "#u4", "#c5", 10.0); // an author's value still wins
    inset(&page, "#u6", "#c7", 0.0); // `border-collapse: collapse` ignores spacing entirely
    inset(&page, "#u11", "#c14", 0.0); // `cellspacing="0"` collapses the horizontal axis…
    inset(&page, "#u13", "#c17", 5.0); // …and a non-zero attribute sets it
    inset(&page, "#u15", "#c21", 2.0); // no attribute: the UA default is untouched
    inset(&page, "#u16", "#c23", 6.0); // an inline style beats the attribute on BOTH axes
    inset(&page, "#u17", "#c25", 8.0); // …and so does an ordinary stylesheet rule

    // ── t1364, THE SECOND IMPLEMENTATION: `cellpadding` had the identical defect one function
    // away, and beat author CSS in BOTH of its forms rather than just one.
    //
    // ```text
    //                                                   Chrome   before
    //   #u18  cellpadding="4", `td{padding:0}` RULE       24       32     ← the rule must win
    //   #u19  cellpadding="4" on a <th>                   32       32     ← CONTROL: `td{}` does not
    //                                                                        match a <th>, so the hint
    //                                                                        DOES apply — this is the
    //                                                                        row that keeps the fix
    //                                                                        from being "ignore it"
    //   #u20  cellpadding="4" + inline `padding:1px`      26       32     ← inline must win
    // ```
    //
    // ⭐ `#u19` exists because `#u18` and `#u20` alone are satisfied by deleting `cellpadding`
    // support outright. A pair of "the author wins" rows cannot tell a correctly-ranked hint from
    // an absent one; it takes a row where nothing else declares the property.
    c(&page, "#u18", 200.0, 24.0);
    c(&page, "#u19", 200.0, 32.0);
    c(&page, "#u20", 200.0, 26.0);
}
