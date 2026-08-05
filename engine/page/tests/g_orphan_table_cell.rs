//! # G_ORPHAN_TABLE_CELL — a table-internal box without its table is ATOMIC, not a run of text
//!
//! `display: table-cell` used **without** a `table`/`table-row` wrapper is the legacy
//! vertical-centring and equal-height-column idiom, and it is everywhere in the CrUX tail. CSS 2.1
//! §17.2.1 wraps such a box in **anonymous table objects**; the box that results is *atomic* — laid
//! out as a block and flowed like a word.
//!
//! We had it in neither of the two places that decide that. The inline collector's atomic list was
//! `InlineBlock | Flex | Grid | InlineFlex | InlineGrid`, so an orphan cell **fell through to the
//! text recursion and was laid out as a plain non-replaced inline**; and the `width: auto`
//! shrink-to-fit arm had the same omission, so once it *was* atomic it filled its container.
//!
//! Chrome-measured on this fixture, `16px/1.25 sans-serif`:
//!
//! ```text
//!                                             Chrome         before        after
//!   #ib  display:inline-block               [0   0  85x20]  [0  0 85x20]  unchanged ✓
//!   #c1  table-cell, no table               [0  30  85x20]  [0 31 85x17]  [0  30  85x20] ✗→✓
//!   #c2a two sibling cells share one row    [0  60  21x20]  [0 61 21x17]  [0  60  21x20] ✗→✓
//!   #c2b   …so they sit SIDE BY SIDE        [21 60  31x20]  [21 61 31x17] [21 60  31x20] ✗→✓
//!   #c4c cell inside an orphan table-row    [0 180  87x20]  [0 181 87x17] [0 180  87x20] ✗→✓
//!   #c5a a cell, then a real block, then    [0 210  45x20]  [0 211 45x17] [0 210  45x20] ✗→✓
//!   #c5b   …a cell — the block SPLITS the   [0 230 600x20]  [0 230 600x20] unchanged ✓
//!   #c5c   anonymous table run              [0 250  32x20]  [0 251 32x17] [0 250  32x20] ✗→✓
//!   #c6  a proper table/row/cell            [0 280  46x20]  [0 280 46x20] unchanged ✓
//! ```
//!
//! ## What proves the diagnosis
//!
//! ⚠ **`#ib` — an `inline-block` with BYTE-IDENTICAL content — was already Chrome-exact at 85x20.**
//! That is the control that turns "our boxes are 3px short" into a statement about *one code path*:
//! an inline box is sized to its **glyph box** (17), an atomic one to its **line box** (20), and the
//! leftover half-leading is also what pushed `y` down by 1. Without `#ib` this fixture would be
//! equally consistent with a general line-height error, which it is not.
//!
//! `#c6` and `#c5b` are the other half of that argument: a properly structured table and a real
//! block were always right and must stay right. The table formatter is not the defect here.
//!
//! ## How this goes RED
//!
//! - **Drop `TableCell` from the inline collector's atomic list** → `#c1` reads `85x17` at `y=31`.
//!   The height *and* the offset both come back.
//! - **Drop `TableCell` from the `width:auto` shrink-to-fit arm** → `#c1` reads `600x20`: the right
//!   height at the full container width, and `#c2a`/`#c2b` STACK instead of sitting side by side.
//! - ⚠ **But the shrink-to-fit arm ADDED ON ITS OWN would have been a NO-OP, and the mutations
//!   prove it rather than assert it.** Mutation 1 above reverts the atomic half while LEAVING the
//!   shrink-to-fit arm in place, and it reproduces the untouched baseline exactly — `85x17` at
//!   `y=31`, to the pixel. With the box still a text-recursion inline, `layout_block` (the only
//!   caller of that arm) is never reached for it, so the arm cannot fire. The two edits are ONE
//!   behaviour: the second is measurable only once the first has landed, which is why they land
//!   together and why neither is a separable tick.
//!
//! ## The residue that was named rather than blamed — and is now CLOSED (t932 + t933)
//!
//! ⚠⚠ A `table-cell` inside a table with an **explicit height** must STRETCH to fill it. This row was
//! pinned at OUR number *specifically so that a future fix would have to come and change it
//! deliberately*, and t814 named the two things it would need. **Both landed, one per tick, and the
//! row is now asserted entirely at Chrome's numbers:**
//!
//! ```text
//!                    Chrome          at t814        at t932        at t933
//!   #c3          [0 90 300x80]   [0 92  67x20]  [0 90 300x20]  [0 90 300x80]
//! ```
//!
//! * **t932 — anonymous-row generation INSIDE a real table.** `collect_table_rows` recognised only
//!   `table-row`/`table-row-group`, so a bare cell child of a `table` was dropped entirely. Moved x,
//!   y and width onto Chrome.
//! * **t933 — cell stretching.** t908 had taught the table BOX to grow to its declared height and
//!   nothing inside it moved, so the height became empty space at the bottom: the box was right and
//!   every row was wrong. t933 distributes that surplus over the rows, proportionally to natural
//!   height and excluding rows that specified one. Moved the height.
//!
//! **Four gates named this one algorithm before anything was built** — this file, t908's
//! `g_table_height_is_a_minimum#t10`, t925, and t932's `g_anonymous_table_row#mid` — and all four
//! now assert Chrome. A residue pinned at our own number with its mechanism written beside it is what
//! made that possible; the practice is the point, not this instance of it.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
body{margin:0;font:16px/1.25 sans-serif}
.w{width:600px;margin-bottom:10px}
</style></head><body>
<div class="w"><span id="ib" style="display:inline-block;background:#8cf">cell no table</span></div>
<div class="w"><div id="c1" style="display:table-cell;background:#8cf">cell no table</div></div>
<div class="w"><div id="c2a" style="display:table-cell;background:#8cf">left</div><div id="c2b" style="display:table-cell;background:#fc8">right</div></div>
<div class="w"><div style="display:table;height:80px;width:300px" id="c3t"><div id="c3" style="display:table-cell;vertical-align:middle;background:#8cf">v-centred</div></div></div>
<div class="w"><div id="c4" style="display:table-row;background:#8cf"><div id="c4c" style="display:table-cell">row no table</div></div></div>
<div class="w"><div id="c5a" style="display:table-cell;background:#8cf">before</div><div id="c5b" style="background:#ddd">a real block</div><div id="c5c" style="display:table-cell;background:#fc8">after</div></div>
<div class="w"><div style="display:table" id="c6t"><div style="display:table-row"><div id="c6" style="display:table-cell;background:#8cf">proper</div></div></div></div>
</body></html>"##;

fn rect_of(page: &manuk_page::Page, sel: &str) -> [f32; 4] {
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
    [r.x, r.y, r.width, r.height]
}

fn assert_box(page: &manuk_page::Page, sel: &str, want: [f32; 4], why: &str) {
    let g = rect_of(page, sel);
    let ok = (0..4).all(|i| (g[i] - want[i]).abs() < 1.01);
    assert!(
        ok,
        "G_ORPHAN_TABLE_CELL: `{sel}` expected [{} {} {}x{}] (MEASURED in headless Chrome on THIS \
         fixture), got [{} {} {}x{}].\n  {why}",
        want[0], want[1], want[2], want[3], g[0], g[1], g[2], g[3]
    );
}

#[test]
fn g_orphan_table_cell() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://tc.test/", &fonts, 1200.0);

    // ── THE CONTROL THAT NAMES THE PATH. Identical content, identical font, already exact. Without
    //    this row, every assertion below is also consistent with a general line-height bug.
    assert_box(
        &page,
        "#ib",
        [0.0, 0.0, 85.0, 20.0],
        "an `inline-block` with byte-identical content was ALWAYS Chrome-exact. This is what proves \
         the orphan-cell error is one code path and not a global metric error — if this row ever \
         fails, the diagnosis in this file is wrong and the fix is aimed at the wrong organ",
    );

    // ── THE BUG: an orphan cell was a run of inline text — glyph height, baseline offset.
    assert_box(
        &page,
        "#c1",
        [0.0, 30.0, 85.0, 20.0],
        "`display:table-cell` with no table gets an anonymous table (CSS 2.1 §17.2.1) and is ATOMIC: \
         a LINE box (20), not a glyph box (17). It read 85x17 at y+1",
    );
    assert_box(
        &page,
        "#c2a",
        [0.0, 60.0, 21.0, 20.0],
        "two sibling orphan cells share ONE anonymous row",
    );
    assert_box(
        &page,
        "#c2b",
        [21.0, 60.0, 31.0, 20.0],
        "…so the second sits SIDE BY SIDE with the first at x=21. If this stacks, the shrink-to-fit \
         half of the fix is missing and the cell filled its container",
    );
    assert_box(
        &page,
        "#c4c",
        [0.0, 180.0, 87.0, 20.0],
        "a cell inside an ORPHAN `table-row` takes the same path",
    );

    // ── The anonymous table run is SPLIT by a real block, and all three keep their own boxes.
    assert_box(
        &page,
        "#c5a",
        [0.0, 210.0, 45.0, 20.0],
        "cell before a block",
    );
    assert_box(
        &page,
        "#c5b",
        [0.0, 230.0, 600.0, 20.0],
        "a real in-flow block between two orphan cells was ALWAYS right and must stay full-width",
    );
    assert_box(
        &page,
        "#c5c",
        [0.0, 250.0, 32.0, 20.0],
        "cell after the block",
    );

    // ── WHAT MUST NOT MOVE: a properly structured table keeps the table formatter.
    assert_box(
        &page,
        "#c6",
        [0.0, 280.0, 46.0, 20.0],
        "a real table/table-row/table-cell was ALWAYS Chrome-exact and must stay so — the table \
         formatter is not the defect and must keep running",
    );

    // ── THE NAMED RESIDUE, HALVED AT t932. Chrome stretches this cell to [0 90 300x80]; we now
    //    match it on x, y and width, and not on height.
    assert_box(
        &page,
        "#c3",
        [0.0, 90.0, 300.0, 80.0],
        "a cell in a table with an EXPLICIT HEIGHT must stretch to fill it — Chrome says \
         [0 90 300x80]. x, y and WIDTH are Chrome's here; only the HEIGHT is ours. t814 pinned this \
         at [0 92 67x20] on purpose, naming anonymous-row generation INSIDE a real table as one of \
         the two things it needed; t932 did that, and it moved three of the four coordinates onto \
         Chrome. What is left is exactly one quantity — cell stretching, the height-distribution \
         algorithm t908/t925 name from the real-<table> side. The next fix must change this line \
         again, and it has only one number to move",
    );
}
