//! **G_TABLE_STRUCTURE_GEOMETRY — three behaviours that were BUILT and never BANKED, and whose own
//! gate still called them missing.**
//!
//! t933 measured a sixteen-row table battery, fixed one mechanism, and left three named:
//!
//! > ```text
//! >   ROWSPAN row-height distribution   a 60px rowspan=2 cell must give 30/30 to its two rows;
//! >                                     we give 24/36 — the overflow all lands on the LAST row
//! >   CAPTION                           `<caption>` reserves no space and does not widen the
//! >                                     table: the first cell belongs at y=20 and 29 wide,
//! >                                     reads y=0 and 10 wide
//! >   THEAD ORDERING                    a `<thead>` written AFTER a `<tbody>` must still render
//! >                                     FIRST; we render in source order (y=24, Chrome y=0)
//! > ```
//!
//! ⭐⭐⭐ **RE-MEASURED AGAINST HEADLESS CHROME, ALL THREE ARE CORRECT — AND NONE OF THEM HAS A
//! GATE.** They were built somewhere in the ~427 ticks since, the list that called them missing was
//! never re-run, and `CONSTITUTION.MD` VI.2 has carried *"t933 row-height distribution"* as an open
//! item since check #82. A backlog entry is a claim about the present tense, and this one had been
//! false for a long time.
//!
//! **Banking them is the point of this gate, not the retraction.** Three real behaviours were one
//! edit away from silently regressing, in a subsystem that has now been shown twice in three ticks
//! to regress silently for weeks: t1360 found `g_table_cell_valign` red for twenty-three days
//! because it is not in the wall's launch list, and t1361 found `manuk-css`'s entire Stylo test
//! module cfg'd out of the wall. Behaviour that is implemented but ungated is not banked, and this
//! project's own history is the evidence.
//!
//! ## Every number is headless-Chrome-measured, `16px/24px monospace`, `border-collapse: separate`
//!
//! ```text
//!  A  ROWSPAN     <td rowspan=2 height:60px> + two rows
//!                   the spanning cell  [0  0 10x60]      the two rows get 30 and 30, NOT 24/36
//!                   row-1 neighbour    [10 0 10x30]
//!                   row-2 neighbour    [10 30 10x30]     table 60
//!  B  CAPTION     <caption>Cap</caption> + one <td>cell</td>
//!                   the caption        [0 0 39x24]       it RESERVES a band above the rows
//!                   the cell           [0 24 39x24]      pushed down by exactly the caption
//!                   table width        39                sized by the WIDER of caption and cells
//!  C  THEAD       <tbody> written BEFORE <thead> in source
//!                   the tbody cell     [0 24 39x24]      source order is NOT render order
//!                   the thead cell     [0  0 39x24]      the head renders FIRST
//! ```
//!
//! ⭐ **Arm B is two claims in one fixture and they fail separately.** A caption that reserved its
//! band but did not participate in the table's width would put the cell at y=24 and leave the table
//! 10 wide (the cell's own content); a caption that widened the table but reserved no band would get
//! the width right and put the cell at y=0. Both halves are asserted, which is what t933's note
//! ("reserves no space AND does not widen") described as one bug and is really two.
//!
//! ⚠ **Arm C's fixture puts `<tbody>` FIRST on purpose.** With `<thead>` written first, source
//! order and render order agree and the arm is vacuous — it would pass against an engine that had
//! never heard of `<thead>`. The inverted spelling is the only one that separates them, and it is
//! also the one real pages produce, because a template that appends rows to a `<tbody>` and then
//! prepends a header emits exactly this.
//!
//! ⚠ This gate lives in `agent/tests/` for the reason t1360 documented at length: the wall's crate
//! list is `manuk-css manuk-layout manuk-paint manuk-dom manuk-net manuk-agent manuk-shell`, and a
//! gate under `engine/page/tests/` runs only when `scripts/verify.sh` names it in an explicit
//! `_launch` line. `scripts/` is observer-owned, so the gate is placed where the wall already looks.

use manuk_dom::NodeId;
use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
html,body{margin:0;padding:0}
body{font-family:monospace;font-size:16px;line-height:24px}
*{box-sizing:border-box}
table{border-collapse:separate;border-spacing:0}
td,th{padding:0;border:0}
.w{width:400px;margin:0 0 8px 0}
</style></head><body>
<div class="w" id="cA"><table>
  <tr><td rowspan="2" style="height:60px" id="a1">R</td><td id="a2">x</td></tr>
  <tr><td id="a3">y</td></tr>
</table></div>
<div class="w" id="cB"><table><caption id="b1">Cap</caption><tr><td id="b2">cell</td></tr></table></div>
<div class="w" id="cC"><table>
  <tbody><tr><td id="c1">body</td></tr></tbody>
  <thead><tr><td id="c2">head</td></tr></thead>
</table></div>
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

fn table_in(page: &manuk_page::Page, wrapper: &str) -> manuk_layout::Rect {
    let dom = page.dom();
    let t = dom
        .descendants(by_id(page, wrapper))
        .find(|&n| dom.tag_name(n) == Some("table"))
        .unwrap_or_else(|| panic!("VACUOUS: no <table> under #{wrapper}"));
    *page
        .root_box
        .node_rects(dom)
        .get(&t)
        .unwrap_or_else(|| panic!("VACUOUS: no box for the table under #{wrapper}"))
}

#[test]
fn rowspan_caption_and_thead_ordering_match_chrome() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://tsg.test/", &fonts, 1200.0);
    let near = |got: f32, want: f32| (got - want).abs() < 1.6;
    // `(dx, dy, w, h)` of `id` relative to wrapper `w`.
    let d = |id: &str, w: &str| {
        let (a, x) = (rect(&page, id), rect(&page, w));
        (a.x - x.x, a.y - x.y, a.width, a.height)
    };
    let same = |got: (f32, f32, f32, f32), want: (f32, f32, f32, f32)| {
        near(got.0, want.0) && near(got.1, want.1) && near(got.2, want.2) && near(got.3, want.3)
    };

    // ── VACUITY. Three laid-out tables with real boxes; a page that laid none of this out would
    // sail through on zeros.
    for w in ["cA", "cB", "cC"] {
        let t = table_in(&page, w);
        assert!(
            t.height > 0.0 && t.width > 0.0,
            "VACUOUS: the table under #{w} has no box ({t:?})"
        );
    }

    // ── ARM A · ROWSPAN ROW-HEIGHT DISTRIBUTION. A 60px cell spanning two auto rows gives each of
    // them 30, not 24/36 — the excess is shared in proportion to the rows' natural heights and does
    // not all land on the last one.
    assert!(
        same(d("a1", "cA"), (0.0, 0.0, 10.0, 60.0)),
        "ARM A: the spanning cell is [0 0 10x60] in Chrome, got {:?}",
        d("a1", "cA")
    );
    assert!(
        same(d("a2", "cA"), (10.0, 0.0, 10.0, 30.0)),
        "ARM A: the FIRST row's neighbour is [10 0 10x30] in Chrome, got {:?}. Reading 10x24 is the \
         t933 signature: the rowspan's excess all dumped on the LAST row.",
        d("a2", "cA")
    );
    assert!(
        same(d("a3", "cA"), (10.0, 30.0, 10.0, 30.0)),
        "ARM A: the SECOND row's neighbour is [10 30 10x30] in Chrome, got {:?}",
        d("a3", "cA")
    );

    // ── ARM B · CAPTION. Two claims that fail separately: it reserves a band ABOVE the rows, and it
    // participates in the table's WIDTH.
    assert!(
        same(d("b1", "cB"), (0.0, 0.0, 39.0, 24.0)),
        "ARM B: the caption is [0 0 39x24] in Chrome, got {:?}",
        d("b1", "cB")
    );
    assert!(
        same(d("b2", "cB"), (0.0, 24.0, 39.0, 24.0)),
        "ARM B: the first cell sits BELOW the caption at [0 24 39x24] in Chrome, got {:?}. y=0 means \
         the caption reserved no space; a width of 10 means it did not participate in the table's \
         width. The two halves fail independently.",
        d("b2", "cB")
    );
    assert!(
        near(table_in(&page, "cB").width, 39.0),
        "ARM B: the table is sized by the WIDER of its caption and its columns — 39 in Chrome, got \
         {}",
        table_in(&page, "cB").width
    );

    // ── ARM C · THEAD ORDERING. The fixture writes <tbody> FIRST on purpose: with <thead> first,
    // source order and render order agree and this arm proves nothing.
    assert!(
        same(d("c2", "cC"), (0.0, 0.0, 39.0, 24.0)),
        "ARM C: a <thead> written AFTER the <tbody> still renders FIRST — [0 0 39x24] in Chrome, \
         got {:?}. y=24 is source order, which is the t933 signature.",
        d("c2", "cC")
    );
    assert!(
        same(d("c1", "cC"), (0.0, 24.0, 39.0, 24.0)),
        "ARM C: the <tbody> row follows the head at [0 24 39x24] in Chrome, got {:?}",
        d("c1", "cC")
    );
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// These are REGRESSION gates on behaviour that is already correct, so the mutations are the
// pre-t933 implementations each arm describes:
//   A  give a rowspan cell's excess entirely to its LAST row  -> a2 reads 10x24, a3 reads 10x36
//   B  lay <caption> out as an ordinary row / skip it in the width -> b2 reads y=0, or table w=10
//   C  render row groups in source order                      -> c2 reads y=24 and c1 y=0
