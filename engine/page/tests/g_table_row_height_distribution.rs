//! # G_TABLE_ROW_HEIGHT_DISTRIBUTION — a table's surplus height goes INTO the rows
//!
//! CSS 2.1 §17.5.3 says a table taller than its content distributes the extra over its rows, and
//! then declines to say how: *"the distribution of the remaining space is
//! implementation-dependent"*. So the spec cannot settle this and **the only honest source is a
//! measurement of Chrome**. Every number in this file is one.
//!
//! t908 taught the table BOX to grow to its declared height (`G_TABLE_HEIGHT_IS_A_MINIMUM`) and
//! nothing inside it moved: the rows kept their natural heights and the declared height became empty
//! space at the bottom. **The box was right and every row was wrong** — a `<td>` that should be 56
//! tall was 26 — which is why four separate gates ended up naming this one algorithm before any of
//! them could close: this file's ancestor `g_table_height_is_a_minimum#t10` (t908), t925,
//! `g_orphan_table_cell#c3` (t814) and `g_anonymous_table_row#mid` (t932). All four now assert
//! Chrome.
//!
//! ## Chrome-measured on THIS fixture
//!
//! `google-chrome --headless --hide-scrollbars --window-size=1200,800`, `16px/1.5 sans-serif`, a
//! 200px table. `<table>` carries the UA `border-spacing: 2px`, so a table of N rows spends
//! `2 × (N+1)` on gutters and the rows share what is left:
//!
//! ```text
//!                                                        Chrome     before      after
//!   height:60,  one row (natural 26)                       56         26         56
//!   height:200, two rows (natural 26 + 26)               97 · 97    26 · 26    97 · 97
//!   height:200, two rows (natural 26 + 74)          50.4 · 143.6    26 · 74   50.4 · 143.6
//!   height:200, row1 height:100px, row2 natural 26     100 · 94     26 · 26    100 · 94
//!   height:100, one row of two cells (valign)            96 each    26 each    96 each
//!   height:100, cell with height:10px                      96         26         96
//!   display:table (spacing 0), height:100, two rows       50 · 50    24 · 24    50 · 50
//!   height:10  (content 26)             (CONTROL)          26         26         26
//!   no height                           (CONTROL)          26         26         26
//! ```
//!
//! ## The two clauses, and the row that discriminates each
//!
//! ⚠⚠⚠ **The surplus is shared PROPORTIONALLY to natural height, not in equal shares.** With two
//! rows of natural 26 and 74 in a 200px table, Chrome gives **50.4 and 143.6** — exactly
//! `194 × 26/100` and `194 × 74/100`. Equal shares would give 73 and 121. The two models agree on
//! every *equal-natural* row and disagree only here, which is why the fixture carries a row whose
//! natural heights differ by a factor of three. Without it this gate would pass against the wrong
//! algorithm.
//!
//! ⚠⚠⚠ **A row that SPECIFIES a height is EXCLUDED from the surplus, not merely counted in it.**
//! `row1 { height: 100px }` beside a natural-26 row in a 200px table gives Chrome **100 and 94** —
//! row1 keeps exactly what it asked for and row2 absorbs all 70px of surplus. Proportional
//! distribution over *both* rows would give 154 and 40. This is also the only row that exercises
//! the first half of the fix — **a row's own `height` is a minimum on its natural height** — which
//! is unobservable on its own and is why the two edits are one behaviour and one tick.
//!
//! ⚠ **The two CONTROLs are t908's rule, and they are what a fix phrased as "make rows fill the
//! table" would break.** A declared height SHORTER than the content is a *minimum*: the table grows
//! to 30 and the row stays 26. Nothing here may shrink a row, ever.
//!
//! ## How this goes RED
//!
//! - **Delete the distribution block** → every ✗→✓ row snaps back to its natural height (56→26,
//!   97·97→26·26, 100·94→26·26) while both CONTROLs stay green. Verified.
//! - **Distribute in EQUAL shares** (`extra / n` per flexible row instead of `extra × hᵣ / Σh`) →
//!   the unequal-natural pair reads **73 · 121** instead of 50.4 · 143.6, and every other row still
//!   passes. Verified — this is the plausible wrong algorithm.
//! - **Include specified-height rows in the distribution** (drop the `row_min_specified` filter) →
//!   the `height:100px` pair reads **154 · 40** instead of 100 · 94. Verified.
//!
//! ## NOT covered, named rather than left looking handled
//!
//! **When EVERY row specifies a height, the surplus is left as space at the bottom — and that is a
//! deliberate non-answer, not a rule.** Chrome's behaviour in that case is unmeasured, so the code
//! keeps the pre-existing behaviour rather than inventing a distribution this fixture cannot defend.
//! Named here with the reason so the next person measures it instead of assuming it was decided.
//!
//! **A row BOX is not inset by the horizontal border-spacing.** Chrome reports a `<tr>` as 196 wide
//! in a 200px table (the cells' span), we report 200. Measured in passing on this fixture and left
//! alone: it is a different quantity from row *height*, it moves no cell, and folding it in would
//! make this gate's RED proofs ambiguous.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
html,body{margin:0;padding:0;font:16px/1.5 sans-serif}
table,.t{width:200px}
</style></head><body>
<table id="one" style="height:60px"><tr><td id="one_c">one</td></tr></table>

<table id="eq" style="height:200px"><tr><td id="eq_a">one</td></tr><tr><td id="eq_b">two</td></tr></table>

<table id="uneq" style="height:200px"><tr><td id="uneq_a">one</td></tr><tr><td id="uneq_b">a<br>b<br>c</td></tr></table>

<table id="spec" style="height:200px"><tr style="height:100px"><td id="spec_a">one</td></tr><tr><td id="spec_b">two</td></tr></table>

<table id="va" style="height:100px"><tr><td id="va_top" style="vertical-align:top">top</td><td id="va_bot" style="vertical-align:bottom">bot</td></tr></table>

<table id="small" style="height:100px"><tr><td id="small_c" style="height:10px">one</td></tr></table>

<div class="t" id="dt" style="display:table;height:100px"><div style="display:table-row"><div style="display:table-cell" id="dt_a">one</div></div><div style="display:table-row"><div style="display:table-cell" id="dt_b">two</div></div></div>

<table id="ctl_short" style="height:10px"><tr><td id="ctl_short_c">one</td></tr></table>
<table id="ctl_none"><tr><td id="ctl_none_c">one</td></tr></table>
</body></html>"##;

fn h_of(page: &manuk_page::Page, sel: &str) -> f32 {
    let dom = page.dom();
    let n = manuk_css::query_selector_all(dom, dom.root(), sel)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("selector {sel} matched nothing"));
    page.root_box
        .node_rects(dom)
        .get(&n)
        .unwrap_or_else(|| panic!("no box for {sel}"))
        .height
}

#[test]
fn g_table_row_height_distribution() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://trh.test/", &fonts, 1200.0);

    let check = |sel: &str, want: f32, why: &str| {
        let got = h_of(&page, sel);
        assert!(
            (got - want).abs() < 1.01,
            "G_TABLE_ROW_HEIGHT_DISTRIBUTION: `{sel}` expected height={want} (MEASURED in headless \
             Chrome on THIS fixture), got {got}.\n  {why}"
        );
    };

    // ── THE CONTROLS FIRST, because they are t908's rule and nothing below may violate it.
    check(
        "#ctl_short_c",
        26.0,
        "a declared height SHORTER than the content is a MINIMUM (CSS 2.1 §17.5.3, gated by \
         G_TABLE_HEIGHT_IS_A_MINIMUM): the table grows to 30 and the row stays at its natural 26 (24 of line box + the UA cell padding). A \
         fix phrased as `make rows fill the table` rather than `distribute the SURPLUS` shrinks this \
         row and passes everything else",
    );
    check(
        "#ctl_none_c",
        26.0,
        "a table with no declared height has no surplus and every row is natural",
    );

    // ── THE SIMPLE CASE: one row takes the whole surplus.
    check(
        "#one_c",
        56.0,
        "a 60px table with one row: 2px of border-spacing above and below leaves 56 for the row. \
         This is the cell that was 24 tall while its table was correctly 60 — the box right and \
         everything in it wrong, which is what made four separate gates name this algorithm",
    );

    // ── CLAUSE 1: PROPORTIONAL, not equal shares. The unequal pair is the discriminator.
    check("#eq_a", 97.0, "two equal-natural rows split 194 evenly");
    check("#eq_b", 97.0, "…and so does the second");
    check(
        "#uneq_a",
        50.4,
        "natural 24 beside a natural 72, in 194 of usable height: Chrome gives 194 x 26/100 = 50.4. \
         EQUAL SHARES would give 73. This row and its partner are the only ones in the fixture that \
         tell the two algorithms apart",
    );
    check(
        "#uneq_b",
        143.6,
        "…and 194 x 74/100 = 143.6, where equal shares would give 121",
    );

    // ── CLAUSE 2: a specified-height row is EXCLUDED from the surplus.
    check(
        "#spec_a",
        100.0,
        "a row that declared `height:100px` keeps exactly that and takes NO share of the surplus. \
         Distributing proportionally over both rows would give it 154. This row is also the only \
         thing that makes the first half of the fix observable — a row's own height as a minimum on \
         its natural height does nothing measurable without the distribution beside it",
    );
    check(
        "#spec_b",
        94.0,
        "…so the natural-24 row absorbs ALL 70px of surplus: 194 - 100 = 94. Equal or proportional \
         sharing across both rows gives 40 here",
    );

    // ── The stretch reaches every cell in the row, whatever its own height or alignment says.
    check(
        "#va_top",
        96.0,
        "`vertical-align` positions a cell's CONTENT inside its box; it does not stop the box \
         filling the row (Chrome: 96 for both cells)",
    );
    check("#va_bot", 96.0, "…and the same for `vertical-align:bottom`");
    check(
        "#small_c",
        96.0,
        "a cell's own `height:10px` loses to the row it sits in — the cell fills the row",
    );

    // ── `display:table` has NO UA border-spacing, so the arithmetic differs and must still hold.
    check(
        "#dt_a",
        50.0,
        "a `display:table` div gets no UA `border-spacing`, so its two rows split the full 100 \
         rather than 100 minus gutters. If the spacing term were hard-coded rather than read, this \
         row reads 47",
    );
    check("#dt_b", 50.0, "…and so does the second");
}
