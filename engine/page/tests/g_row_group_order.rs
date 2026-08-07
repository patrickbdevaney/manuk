//! # G_ROW_GROUP_ORDER — `<tfoot>` written first rendered first, and the UA sheet is where we lost the distinction
//!
//! CSS Tables lays row groups out **header → body → footer, regardless of source order**. Our UA
//! sheet said:
//!
//! ```css
//! thead, tbody, tfoot { display: table-row-group; }
//! ```
//!
//! — one value for three groups, which **discards the only thing that distinguishes them**. So
//! `<tfoot>` written before `<tbody>` rendered at the *top* of the table.
//!
//! That idiom is not exotic. Putting `<tfoot>` before `<tbody>` is the classic HTML4 pattern — it
//! exists so a long table's footer reaches the parser before its thousand body rows — and it is
//! still everywhere in legacy markup, invoice templates and report generators. **A totals row at the
//! top of an invoice is not a geometry error, it is a reading-order one**, which puts it in the I3
//! class rather than the shape class: the numbers are all present, all correctly sized, and mean
//! something different.
//!
//! ```text
//!                                                       Chrome     before      after
//!   <tfoot> before <tbody>              foot row         [24]       [ 0]       [24]
//!                                       body row         [ 0]       [24]       [ 0]
//!   <thead> after <tbody>               head row         [ 0]       [24]       [ 0]
//!                                       body row         [24]       [ 0]       [24]
//!   all three scrambled (foot,body,head)  h/b/f      [0/24/48]  [48/24/0]  [0/24/48]
//!  ── CONTROLS, neither of which moved ──
//!   the usual thead/tbody/tfoot order            [0/24/48]         unchanged
//!   TWO <tbody>s keep their source order           [0/24]          unchanged
//! ```
//!
//! ## Where the fix had to go, which is not where the symptom was
//!
//! The layout code was walking the DOM in order, and adding a rank there is the obvious fix — but it
//! could not work, because **every group arrived carrying the same `display` value**. The
//! distinction is made in the UA sheet, and Chrome makes it there too. Two new `Display` variants
//! (`TableHeaderGroup`, `TableFooterGroup`) exist so that the value can *survive* the cascade, and
//! the layout rank reads them.
//!
//! > The symptom was in layout. The lost information was three layers up, in a stylesheet, in a rule
//! > that looked like a tidy abbreviation of three identical declarations. **A fold that discards a
//! > distinction reads as a simplification right up until something needs the distinction.**
//!
//! ## Why a STABLE sort, and the row that proves it matters
//!
//! Groups of the same kind keep their source order relative to each other: two `<tbody>`s are
//! ordered by the document. A stable sort on the rank preserves that; an unstable one, or a
//! three-bucket rebuild, may not. The two-`<tbody>` control is the only row that can tell.
//!
//! ## How this goes RED
//!
//! - **Restore `thead, tbody, tfoot { display: table-row-group }` in the UA sheet** → all three
//!   scrambled rows render in source order; both controls pass. The `Display` variants and the rank
//!   remain in place and do nothing, which is what makes this the interesting recipe: *the layout
//!   fix alone is inert.*
//! - **Drop the `sort_by_key`** → same three rows fail, from the other end of the same pipe.
//! - **Give `TableFooterGroup` rank 0 and `TableHeaderGroup` rank 2** → the scrambled rows invert
//!   and both controls still pass.
//! - **Sort unstably** (`sort_unstable_by_key`) → nothing fails here today; recorded as a NON-red
//!   because with two elements the two sorts cannot differ, and the control exists to catch a future
//!   three-bucket rewrite rather than this line.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
html,body{margin:0;padding:0}
body{font:16px/1.5 monospace}
*{box-sizing:border-box}
table{border-collapse:separate;border-spacing:0}
td{padding:0;height:20px}
.w{width:400px;margin:0 0 6px 0}
</style></head><body>
<div class="w" id="c1"><table><tfoot><tr><td id="a1">foot</td></tr></tfoot><tbody><tr><td id="a1b">body</td></tr></tbody></table></div>
<div class="w" id="c2"><table><tbody><tr><td id="a2b">body</td></tr></tbody><thead><tr><td id="a2">head</td></tr></thead></table></div>
<div class="w" id="c3"><table><tfoot><tr><td id="a3c">f</td></tr></tfoot><tbody><tr><td id="a3b">b</td></tr></tbody><thead><tr><td id="a3">h</td></tr></thead></table></div>
<div class="w" id="c4"><table><thead><tr><td id="a4">h</td></tr></thead><tbody><tr><td id="a4b">b</td></tr></tbody><tfoot><tr><td id="a4c">f</td></tr></tfoot></table></div>
<div class="w" id="c5"><table><tbody><tr><td id="a5">one</td></tr></tbody><tbody><tr><td id="a5b">two</td></tr></tbody></table></div>
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
fn g_row_group_order() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://rg.test/", &fonts, 1200.0);
    let dy = |sel: &str, w: &str| rect_of(&page, sel).y - rect_of(&page, w).y;
    let near = |got: f32, want: f32| (got - want).abs() < 1.1;

    // ── DEFECT 1 — `<tfoot>` before `<tbody>`, the classic HTML4 idiom. The footer belongs LAST.
    assert!(
        near(dy("#a1", "#c1"), 24.0) && near(dy("#a1b", "#c1"), 0.0),
        "G_ROW_GROUP_ORDER: a `<tfoot>` written BEFORE its `<tbody>` still renders LAST — foot at \
         y=24, body at y=0; got {} and {}. Source order puts a totals row at the TOP of an invoice: \
         every number present, correctly sized, and meaning something else.",
        dy("#a1", "#c1"),
        dy("#a1b", "#c1")
    );

    // ── DEFECT 2 — `<thead>` after `<tbody>`, the mirror case, which proves the rank has two ends.
    assert!(
        near(dy("#a2", "#c2"), 0.0) && near(dy("#a2b", "#c2"), 24.0),
        "G_ROW_GROUP_ORDER: a `<thead>` written AFTER its `<tbody>` still renders FIRST — head at \
         y=0, body at y=24; got {} and {}.",
        dy("#a2", "#c2"),
        dy("#a2b", "#c2")
    );

    // ── DEFECT 3 — all three, fully scrambled (foot, body, head in source). One row for the whole
    //    ordering rather than two halves of it.
    assert!(
        near(dy("#a3", "#c3"), 0.0)
            && near(dy("#a3b", "#c3"), 24.0)
            && near(dy("#a3c", "#c3"), 48.0),
        "G_ROW_GROUP_ORDER: source order foot/body/head must render head/body/foot at 0/24/48; got \
         {}/{}/{}.",
        dy("#a3", "#c3"),
        dy("#a3b", "#c3"),
        dy("#a3c", "#c3")
    );

    // ── CONTROL A — the ordinary order, which was always right and which an inverted rank breaks.
    assert!(
        near(dy("#a4", "#c4"), 0.0)
            && near(dy("#a4b", "#c4"), 24.0)
            && near(dy("#a4c", "#c4"), 48.0),
        "G_ROW_GROUP_ORDER: thead/tbody/tfoot in the usual order stay at 0/24/48; got {}/{}/{}.",
        dy("#a4", "#c4"),
        dy("#a4b", "#c4"),
        dy("#a4c", "#c4")
    );

    // ── CONTROL B — two `<tbody>`s. Groups of the SAME kind keep their document order, which is why
    //    the sort must be stable. This is the only row that can see that.
    assert!(
        near(dy("#a5", "#c5"), 0.0) && near(dy("#a5b", "#c5"), 24.0),
        "G_ROW_GROUP_ORDER: two `<tbody>`s keep their source order — 0 then 24, not {} and {}. \
         Equal ranks must not be reordered.",
        dy("#a5", "#c5"),
        dy("#a5b", "#c5")
    );
}
