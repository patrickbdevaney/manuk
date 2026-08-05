//! # G_ROWLESS_TABLE — a `display:table` with no rows is a shrink-to-fit BLOCK
//!
//! `collect_table_rows` keeps only `table-row` / `table-row-group` **elements**. A `display:table`
//! box whose content is bare text — or any non-table content — therefore yields zero rows, and
//! `layout_table` produced an **empty box**. Not narrow: absent.
//!
//! ```text
//!                                              Chrome        before      after
//!   display:table, bare text "short"        [0   0  36x20]   0x0      [0   0  36x20]  ✗→✓
//!   display:table, a longer run of text     [0  20 213x20]   0x0      [0  20 213x20]  ✗→✓
//!   display:table, width:200px, bare text   [0  86 200x20]   0x0      [0  86 200x20]  ✗→✓
//!   display:inline-table, bare text         [0 106  72x20]   0x0      [0 106  72x20]  ✗→✓
//!   display:table + table-row + table-cell  [0  40 109x20]  109x20    [0  40 109x20]  ✓ always right
//!   a real <table><tr><td>                  [0  60  97x26]   91x22       91x22        ~ see below
//! ```
//!
//! **An explicit `width:200px` did not save it either**, which is what rules out sizing and names
//! the cause: the box was never built.
//!
//! ## Why a shrink-to-fit block is the right answer, not a patch
//!
//! CSS 2.1 §17.2.1 wraps non-table content in an anonymous table-cell inside an anonymous table-row.
//! A table with ONE anonymous cell is, in both axes, exactly a shrink-to-fit block over the same
//! content. So instead of synthesising boxes the row collector has no node to return, the style
//! *clone* in `layout_block` is given `width: fit-content` and the generic block path runs. An
//! author's explicit width is guarded and left alone — which is `#t5`.
//!
//! **The reach is the pre-flexbox layout vocabulary**: `display:table; margin:0 auto` to shrink-wrap
//! and centre, and `display:inline-table`. Still everywhere in the CrUX tail this corpus samples.
//!
//! ⚠ **Third time in one session a bare text node fell through a structural filter** — t799 (an
//! anonymous block inherited nothing), t803 (a text node cloned its parent's `position:absolute` and
//! filtered itself out of the box it WAS), and this. The recurring shape is **a filter written for
//! elements, applied to a child list that contains text**.
//!
//! ## How this goes RED
//!
//! - **Route a rowless table back into `layout_table`** → `#t1`, `#t2`, `#t5` and `#t6` all read
//!   0×0, while `#t3` (a real row/cell structure) still passes. That split is the point: the table
//!   formatter is correct and must keep running for anything that actually has rows.
//! - ⚠ **Dropping the `width == Dim::Auto` guard does NOT go red, and that is stated rather than
//!   implied.** `#t5` still reads 200: `layout_block` only consults `width_keyword` when `s.width`
//!   is `Auto`, so setting it on a box with a definite width is already inert. The guard is
//!   therefore a **statement of intent, not a proven necessity** — it is kept because it makes the
//!   condition read as the spec rule it is, and the gate says so instead of claiming a red it cannot
//!   produce. `#t5` remains asserted, because it pins the BEHAVIOUR (an author's width survives)
//!   whether or not this particular expression is what delivers it.
//!
//! ⚠⚠ **`#t4` WAS asserted at OUR 91×22 rather than Chrome's 97×26, and re-pinned to Chrome at
//! t931 — because the engine moved and the pin did not.** A real `<table><tr><td>` takes the table
//! formatter in both states, so this row was originally pinned to our own number to prove the
//! rowless fix did not disturb the real-table path, with the 6×4 gap recorded as a pre-existing
//! cell-metric difference. That gap has since closed: re-measured in headless Chrome on this exact
//! fixture at t931, `#t4` is **97.17×26** — our number to the pixel, and every other row of the
//! fixture (`35.58 · 213.45 · 108.52 · 200 · 72.06`, all ×20) is Chrome-exact too.
//!
//! So the gate had gone RED for the one reason a gate must never go red: **the engine improved.**
//! It is not in the wall's 19-of-104 subset, which is why it stayed red unnoticed; t930 found it
//! while sweeping and named it rather than folding it in. Re-pinning to the reference makes the
//! assertion STRICTER (it is now a claim about Chrome, not about us), and it is attributable on its
//! own — the failure reproduced byte-identically on the pre-t931 tree in the same hour.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
body{margin:0;font:16px/1.25 sans-serif}
.w{width:600px}
.t{display:table;background:#8cf}
</style></head><body>
<div class="w"><div class="t" id="t1">short</div></div>
<div class="w"><div class="t" id="t2">a much longer run of text here</div></div>
<div class="w"><div class="t" id="t3"><div style="display:table-row"><div style="display:table-cell">cell one</div><div style="display:table-cell">cell two</div></div></div></div>
<div class="w"><table id="t4"><tr><td>td one</td><td>td two</td></tr></table></div>
<div class="w"><div class="t" id="t5" style="width:200px">explicit</div></div>
<div class="w"><div id="t6" style="display:inline-table;background:#8cf">inlinetable</div></div>
</body></html>"##;

fn size_of(page: &manuk_page::Page, sel: &str) -> [f32; 2] {
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
        .unwrap_or_else(|| panic!("no box for {sel} — the element generated none at all"));
    [r.width, r.height]
}

fn assert_size(page: &manuk_page::Page, sel: &str, w: f32, h: f32, why: &str) {
    let s = size_of(page, sel);
    assert!(
        (s[0] - w).abs() < 1.01 && (s[1] - h).abs() < 1.01,
        "G_ROWLESS_TABLE: `{sel}` expected {w}x{h} (MEASURED in headless Chrome on THIS fixture), \
         got {}x{}.\n  {why}",
        s[0],
        s[1]
    );
}

#[test]
fn g_rowless_table() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://tbl.test/", &fonts, 1200.0);

    // ── THE BUG: the box did not exist. Two content lengths, because a shrink-to-fit answer has to
    //    track the content and a constant would satisfy either one alone.
    assert_size(
        &page,
        "#t1",
        36.0,
        20.0,
        "`display:table` around bare text — it shrink-wraps the text. This produced NO box at all",
    );
    assert_size(
        &page,
        "#t2",
        213.0,
        20.0,
        "…and a longer run gives a wider box: the width tracks the content, so this is shrink-to-fit \
         and not a constant",
    );
    assert_size(
        &page,
        "#t6",
        72.0,
        20.0,
        "`display:inline-table` takes the same path and was equally absent",
    );

    // ── THE GUARD: an author's explicit width survives.
    assert_size(
        &page,
        "#t5",
        200.0,
        20.0,
        "`width:200px` on a rowless table is honoured, not shrink-wrapped to 36. An explicit width \
         did NOT save this box before, which is what proved the cause was not sizing",
    );

    // ── WHAT MUST NOT MOVE: anything that really has rows keeps the table formatter.
    assert_size(
        &page,
        "#t3",
        109.0,
        20.0,
        "a real `table-row` + `table-cell` structure was ALWAYS correct and must stay so — the table \
         formatter is not the defect and must keep running",
    );
    assert_size(
        &page,
        "#t4",
        97.0,
        26.0,
        "a real `<table><tr><td>`, re-measured in headless Chrome on THIS fixture at t931: 97.17x26. \
         This row was pinned at OUR 91x22 while a 6x4 cell-metric gap was open; the gap has since \
         closed, so the pin was asserting a number the engine had correctly stopped producing. It is \
         now a claim about Chrome rather than about us, which is strictly stronger",
    );
}
