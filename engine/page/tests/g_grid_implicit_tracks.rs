//! # G_GRID_IMPLICIT_TRACKS — the grid tracks the AUTHOR DID NOT WRITE DOWN, and the axis placement walks
//!
//! `grid-template-rows`/`-columns` size the tracks the author declared. When there are more items
//! than those tracks hold, the auto-placement algorithm invents **implicit** tracks — and three
//! properties govern them: `grid-auto-rows`, `grid-auto-columns` and `grid-auto-flow`. All three
//! exist as fields on taffy's `Style`, and **nothing ever wrote any of them.** So every grid whose
//! items outran its template put the overflow in a new ROW of content height, whatever the author
//! said.
//!
//! This is the third tick of one mechanism (t980 `justify-self`, t981 `align-content`/
//! `justify-items`, this one): a property absent at all three layers — no `ComputedStyle` field, no
//! parse arm, no `stylo_map` line, no `to_taffy_style` line — sitting directly beside a complete
//! twin that makes the family read as covered.
//!
//! Measured against headless Chrome, a 300px-wide grid of 60×40 items (`/tmp/gi.html`, every row an
//! offset from its own container):
//!
//! ```text
//!                                                      Chrome       before        after
//!   grid-auto-flow:column          3rd item           [150,  0]   [  0, 80]   [150,  0]
//!   grid-auto-flow:column
//!     + grid-auto-columns:90px     3rd item           [ 90,  0]   [  0, 80]   [ 90,  0]
//!   grid-auto-rows:80px            5th item           [  0,120]   [  0, 80]   [  0,120]
//!   grid-auto-rows:80px 20px       5th item           [  0,120]   [  0, 80]   [  0,120]
//!   grid-auto-rows:80px 20px       7th item           [  0,140]   [  0,120]   [  0,140]
//!   grid-auto-flow:row dense       back-filled item   [  0,  0]   [  0, 40]   [  0,  0]
//!  ── CONTROLS, none of which moved ──
//!   (no grid-auto-* declared)      3rd item           [  0, 40]      same     unchanged
//!   same grid, NO `dense`          2nd item           [  0, 40]      same     unchanged
//!   fixed-height container, implicit rows STRETCH     [  0,120]      same     unchanged
//!   explicit tracks only, nothing implicit            [  0, 40]      same     unchanged
//! ```
//!
//! ## Why no divergence sweep could have ranked this
//!
//! `row` is the initial `grid-auto-flow`, and an empty `grid-auto-rows` list means `auto` — which is
//! also taffy's default for both fields. An undeclared grid was therefore Chrome-exact forever, and
//! the properties were wrong **only where they were declared**. The same shape as t981's
//! `align-content`. What finds it is a battery where every row declares one value of one property
//! and a control row declares none.
//!
//! ## The `dense` row is the one that cannot be faked
//!
//! `grid-auto-flow: row dense` differs from plain `row` only in that placement may go **backwards**
//! to back-fill a hole an earlier item left. Rows G and H below are the *same markup* with and
//! without the keyword, so the pair isolates the `dense` bit specifically: dropping it from the
//! mapping (folding `RowDense → Row`) leaves every other row in this gate passing and moves exactly
//! that one item from y=0 to y=40. A gate that only tested `column` would call the flow property
//! covered while half of its value space was thrown away.
//!
//! ## The CYCLED list, and why two implicit rows are asserted and not one
//!
//! `grid-auto-rows` takes a `<track-size>+` list which is **cycled** over the implicit tracks —
//! `80px 20px` makes them 80, 20, 80, 20… A single-value fixture cannot tell cycling from "apply the
//! first value to everything", so row F declares two values and asserts the 3rd AND 4th rows.
//!
//! ⚠ **`repeat()` is legal in `grid-template-*` and FORBIDDEN here**, because an auto track list has
//! no length of its own. The two grammars therefore get two parsers rather than one shared one; a
//! shared parser would silently accept `grid-auto-rows: repeat(auto-fill, 80px)` and then be unable
//! to represent it.
//!
//! ## Which cascade this proves
//!
//! The **Stylo** one, which is the shipping cascade (`live-cascade-is-stylo-not-minimal`). The
//! minimal-cascade parse arms added in the same change are the JS-less fallback and are covered by
//! `manuk-css`'s own unit tests.
//!
//! ## NAMED, MEASURED, NOT BUILT — and it is a DIFFERENT mechanism
//!
//! A grid container's block size is taken from its items' content, not from its resolved TRACKS. A
//! grid with `grid-template-rows: 100px` holding one 40px item is 100 tall in Chrome and **40** here
//! — with no implicit track and no `grid-auto-*` declaration anywhere, so it is untouched by this
//! tick and predates it. Fixture `/tmp/gh.html` rows `k`/`l`/`m` discriminate it. The item
//! *positions* in this gate are all exact, which is why that defect hides: the tracks are laid out
//! correctly and only the container's own height is short.
//!
//! ## How this goes RED
//!
//! - **Drop `grid_auto_flow` from `to_taffy_style`** → `#b3` reads [0, 80] against Chrome's
//!   [150, 0], `#c3` reads [0, 80] against [90, 0], and `#g2` reads y=40 against 0. Every
//!   `grid-auto-rows` row still passes — the confinement.
//! - **Drop `grid_auto_rows`** → `#e5` reads y=80 against 120 and the cycled pair collapses, while
//!   every flow row holds.
//! - **Drop `grid_auto_columns`** → `#c3` reads x=150 (the auto-sized implicit column) instead of
//!   90; `#b3`, which shares the flow but declares no column size, is unmoved.
//! - **Fold `RowDense → Row`** → only `#g2` fails, at y=40.
//! - **Apply `grid-auto-rows`'s FIRST value to every implicit track** instead of cycling → `#f7`
//!   reads y=200 against 140 while `#f5` still passes.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
html,body{margin:0;padding:0}
body{font:16px/1 monospace}
.g{display:grid;width:300px;margin:0 0 10px 0}
.fix{height:200px}
.it{width:60px;height:40px}
</style></head><body>
<div class="g" id="a" style="grid-template-columns:60px 60px"><div class="it"></div><div class="it"></div><div class="it" id="a3"></div><div class="it"></div></div>
<div class="g" id="b" style="grid-template-rows:40px 40px;grid-auto-flow:column"><div class="it"></div><div class="it"></div><div class="it" id="b3"></div><div class="it"></div></div>
<div class="g" id="c" style="grid-template-rows:40px 40px;grid-auto-flow:column;grid-auto-columns:90px"><div class="it"></div><div class="it"></div><div class="it" id="c3"></div><div class="it"></div></div>
<div class="g" id="d" style="grid-template-columns:60px 60px;grid-template-rows:40px"><div class="it"></div><div class="it"></div><div class="it"></div><div class="it"></div><div class="it" id="d5"></div><div class="it"></div></div>
<div class="g" id="e" style="grid-template-columns:60px 60px;grid-template-rows:40px;grid-auto-rows:80px"><div class="it"></div><div class="it"></div><div class="it"></div><div class="it"></div><div class="it" id="e5"></div><div class="it"></div></div>
<div class="g" id="f" style="grid-template-columns:60px 60px;grid-template-rows:40px;grid-auto-rows:80px 20px"><div class="it"></div><div class="it"></div><div class="it"></div><div class="it"></div><div class="it" id="f5"></div><div class="it"></div><div class="it" id="f7"></div><div class="it"></div></div>
<div class="g" id="g" style="grid-template-columns:60px 60px;grid-auto-flow:row dense"><div class="it" style="grid-column:2"></div><div class="it" id="g2"></div><div class="it"></div></div>
<div class="g" id="h" style="grid-template-columns:60px 60px"><div class="it" style="grid-column:2"></div><div class="it" id="h2"></div><div class="it"></div></div>
<div class="g fix" id="i" style="grid-template-columns:60px 60px;grid-template-rows:40px"><div class="it"></div><div class="it"></div><div class="it"></div><div class="it"></div><div class="it" id="i5"></div><div class="it"></div></div>
<div class="g" id="j" style="grid-template-columns:60px 60px;grid-template-rows:40px 40px"><div class="it"></div><div class="it"></div><div class="it" id="j3"></div><div class="it"></div></div>
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
fn g_grid_implicit_tracks() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://gi.test/", &fonts, 1200.0);
    let r = |sel: &str| rect_of(&page, sel);
    // Every assertion is an offset from the item's OWN container, so a row going wrong above cannot
    // shift a row below into or out of agreement.
    let off = |sel: &str, w: &str| (r(sel).x - r(w).x, r(sel).y - r(w).y);
    let near = |got: (f32, f32), want: (f32, f32)| {
        (got.0 - want.0).abs() < 1.1 && (got.1 - want.1).abs() < 1.1
    };

    // ── DEFECT 1 — `grid-auto-flow: column`. Two explicit ROWS and no explicit columns, so the
    //    third item opens an IMPLICIT COLUMN. Chrome stretches the two auto columns across the
    //    300px container (150 each) and starts the item at the second one.
    assert!(
        near(off("#b3", "#b"), (150.0, 0.0)),
        "G_GRID_IMPLICIT_TRACKS: `grid-auto-flow:column` must place the 3rd item at the top of the \
         SECOND COLUMN — [150, 0], not {:?}. Reading [0, 80] means the property never reached \
         taffy and placement fell back to row flow, opening a third implicit ROW instead.",
        off("#b3", "#b")
    );

    // ── DEFECT 2 — `grid-auto-columns` sizes that implicit column. Same markup as above plus a
    //    90px auto-column, which moves the second column's start from 150 to 90. This row is what
    //    separates "the flow property works" from "the implicit TRACK SIZE works".
    assert!(
        near(off("#c3", "#c"), (90.0, 0.0)),
        "G_GRID_IMPLICIT_TRACKS: `grid-auto-columns:90px` must make the implicit columns 90 wide, \
         putting the 3rd item at [90, 0], not {:?}. x=150 would mean the flow reached taffy but the \
         track SIZE did not — the implicit columns stayed `auto` and stretched.",
        off("#c3", "#c")
    );

    // ── DEFECT 3 — `grid-auto-rows`. One explicit 40px row, two columns, six items: rows 2 and 3
    //    are implicit. The container's height is AUTO on purpose — with a fixed height the implicit
    //    `auto` rows STRETCH into the free space and happen to land where 80px rows would, so the
    //    fixture would agree with Chrome for the wrong reason. Zero free space is what makes this
    //    row discriminate at all (control D below is the same grid without the declaration).
    assert!(
        near(off("#e5", "#e"), (0.0, 120.0)),
        "G_GRID_IMPLICIT_TRACKS: `grid-auto-rows:80px` must make the two implicit rows 80 tall, so \
         the 5th item starts at y=40+80=120, not {:?}. y=80 is the content-sized `auto` row — the \
         property never reached taffy.",
        off("#e5", "#e")
    );

    // ── DEFECT 4 — the list CYCLES. `80px 20px` sizes implicit row 1 at 80 and row 2 at 20, so the
    //    two asserted items are 20px apart in a fixture where every other spacing is 40 or 80.
    assert!(
        near(off("#f5", "#f"), (0.0, 120.0)) && near(off("#f7", "#f"), (0.0, 140.0)),
        "G_GRID_IMPLICIT_TRACKS: `grid-auto-rows:80px 20px` must CYCLE over the implicit rows — the \
         5th item at y=40+80=120 and the 7th at y=40+80+20=140; got {:?} and {:?}. y=200 for the \
         7th would mean the first value was applied to every implicit track instead of cycling.",
        off("#f5", "#f"),
        off("#f7", "#f")
    );

    // ── DEFECT 5 — `dense`. The first item is pinned to column 2, leaving a hole at column 1 that
    //    sparse flow can never return to. `row dense` back-fills it.
    assert!(
        near(off("#g2", "#g"), (0.0, 0.0)),
        "G_GRID_IMPLICIT_TRACKS: `grid-auto-flow:row dense` must back-fill the 2nd item into the \
         hole the pinned first item left at [0, 0], not {:?}. y=40 is sparse flow — the `dense` bit \
         was dropped in translation, which a `column`-only test would never notice.",
        off("#g2", "#g")
    );

    // ── CONTROL A — the INITIAL flow. No `grid-auto-flow` at all: two explicit columns, the third
    //    item opens an implicit ROW. This is the row a `Column`-by-default slip would fail.
    assert!(
        near(off("#a3", "#a"), (0.0, 40.0)),
        "G_GRID_IMPLICIT_TRACKS: with NO `grid-auto-flow`, the 3rd item must start the SECOND ROW \
         at [0, 40], not {:?}. This row pins the initial value, which the property gets right by \
         accident and which every declared row is blind to.",
        off("#a3", "#a")
    );

    // ── CONTROL B — the SAME markup as the `dense` row without the keyword. This pair is the only
    //    thing in the gate that isolates the `dense` bit rather than the axis.
    assert!(
        near(off("#h2", "#h"), (0.0, 40.0)),
        "G_GRID_IMPLICIT_TRACKS: WITHOUT `dense`, sparse flow must NOT go backwards into the hole — \
         the 2nd item belongs on the second row at [0, 40], not {:?}. If this reads [0, 0] the \
         mapping made every grid dense.",
        off("#h2", "#h")
    );

    // ── CONTROL C — implicit rows with no `grid-auto-rows` in a FIXED-height container still
    //    STRETCH to share the free space (200 - 40 explicit = 160 over two rows = 80 each). This is
    //    the confound row D is built to avoid, kept as a control so a fix that hard-coded implicit
    //    rows to content height would fail here.
    assert!(
        near(off("#i5", "#i"), (0.0, 120.0)),
        "G_GRID_IMPLICIT_TRACKS: undeclared implicit rows in a 200px-tall container STRETCH to 80 \
         each, so the 5th item is at [0, 120], not {:?}.",
        off("#i5", "#i")
    );

    // ── CONTROL D — the same grid as defect 3 with NO `grid-auto-rows` and an AUTO height: the
    //    implicit rows are content-sized at 40, so the 5th item sits at 80. Together with #e5 this
    //    is the before/after pair on one property, on one fixture, one variable apart.
    assert!(
        near(off("#d5", "#d"), (0.0, 80.0)),
        "G_GRID_IMPLICIT_TRACKS: with NO `grid-auto-rows` and no free space, the implicit rows are \
         content-sized at 40, so the 5th item is at [0, 80], not {:?}.",
        off("#d5", "#d")
    );

    // ── CONTROL E — a grid with nothing implicit at all, which none of the three new lines may
    //    touch.
    assert!(
        near(off("#j3", "#j"), (0.0, 40.0)),
        "G_GRID_IMPLICIT_TRACKS: an all-explicit 2×2 grid puts its 3rd item at [0, 40], not {:?}.",
        off("#j3", "#j")
    );
}
