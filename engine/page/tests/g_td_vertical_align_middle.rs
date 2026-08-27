//! # G_TD_VERTICAL_ALIGN_MIDDLE — a default `<td>` CENTRES its content, it does not baseline it
//!
//! ⚠⚠⚠ **CHROME'S `<td>` COMPUTES `vertical-align: middle` AND OURS COMPUTED `baseline`, SO EVERY
//! CELL ON EVERY DEFAULT TABLE PUT ITS TEXT AT THE TOP.** Blink's UA sheet is two lines:
//!
//! ```css
//!   thead, tbody, tfoot { vertical-align: middle }
//!   tr, td, th          { vertical-align: inherit }
//! ```
//!
//! and the SECOND line is the one that is easy to get wrong. `vertical-align` is **not** an
//! inherited property, so `inherit` is the only way the row group's value reaches a cell — writing
//! `td, th { vertical-align: middle }` instead produces the same answer on a default table and the
//! WRONG one the moment an author sets the alignment on the `<tbody>` or the `<tr>`, which is the
//! ordinary way HTML has expressed table alignment since 1997.
//!
//! The visible defect is a whole class of page, not a corner: a data table whose cells differ in
//! height — a masthead with a logo beside its nav, a form label beside a taller input, a row with
//! an image in one cell — put every neighbouring cell's text flush against the top of the row
//! instead of centred in it. `news.ycombinator.com`'s own masthead is the case: its nav text sits
//! at y=12 in Chrome and sat at y=10 here.
//!
//! Every number below is CAPTURED from
//! `google-chrome --headless --hide-scrollbars --window-size=1200,800`, 16px/20px monospace.
//!
//! ```text
//!                                                   computed va    Chrome dy   before   after
//!   #u1  a plain <td> beside a 40px block             middle          10          0      10
//!   #u2  …the same in a 60px row                      middle          20          0      20
//!   #u8  a <th>                                       middle          10          0      10
//!   #z1  a cell holding an INLINE 40px image          middle        12.5          0    12.5
//!   #u6  <tbody style="vertical-align:top">    KEY    top              0          0       0
//!   #u7  <tr style="vertical-align:bottom">    KEY    bottom          20          0      20
//!   #u3  vertical-align:middle written on the td CTRL middle          10         10      10
//!   #u4  vertical-align:top written on the td   CTRL  top              0          0       0
//!   #u5  …a `bottom` on the OTHER cell          CTRL  middle          10          0      10
//!   #z2  a <div style="display:table-cell">     CTRL  baseline        25         25      25
//!   #u9  a 32px/40px cell beside a 40px block   CTRL  middle           —         40      40
//! ```
//!
//! ⭐ **`#u6` AND `#u7` ARE THE ROWS THAT SAY `inherit`, AND `#u6` IS THE ONE THAT LOOKS LIKE A
//! DECORATION AND IS NOT.** `#u6` read dy=0 before this tick and reads dy=0 after — a row that did
//! not move. It is here because it is the only row that goes RED under the *plausible wrong fix*:
//! spell the UA rule `td, th { vertical-align: middle }` and `#u6` jumps to 10 while every other
//! row above stays exactly right. `#u7` is its mirror on the `<tr>`.
//!
//! ⚠⚠⚠ **`#z2` IS THE OTHER HALF, AND IT IS WHY THE RULE IS KEYED ON THE TAG.** Chrome's rule is a
//! UA *declaration* on `td`/`th`, not a property of the computed display — so a
//! `div { display: table-cell }` keeps the initial `baseline` and its neighbour's text sits at
//! dy=25, not 12.5. Matching on `Display::TableCell` would have moved that div too. We were already
//! Chrome-exact here before the tick and must still be after it.
//!
//! ⚠ A `Display::TableCell` test written *where the fix lives* cannot go wrong: at that point in
//! `cascade_node` no author rule has been applied yet, so a `<div>`'s display is still `Block` and
//! the mistake is INERT. The mutation that catches it has to be written where such a rule would
//! naturally be put — a post-cascade pass, on the final display, which is exactly where t1364 found
//! `cellpadding`'s second implementation living. Run there, `#z2` reads 12.5.
//!
//! MUTATIONS, each applied and read:
//!
//! * **the rule deleted** → `#m1` dy=0, not 10
//! * **`td, th { vertical-align: middle }`**, the plausible wrong spelling → `#m6` dy=10, not 0
//! * **keyed on `Display::TableCell` post-cascade** → `#m6` dy=10 and `#k2` dy=12.5, not 25
//!
//! ⚠ **MEASURED, NOT FIXED, AND DELIBERATELY NOT ASSERTED HERE.** A baseline-aligned cell with NO
//! in-flow line box must report its BOTTOM MARGIN EDGE as its baseline (CSS 2.1 §17.5.4); we drop
//! it out of the baseline group instead. `<div style="display:table-cell"><div 40px></div></div>`
//! beside a text cell is 45 in Chrome and 40 here. It is invisible on a real `<td>` — which this
//! tick just made `middle` — and it is the next tick.
use manuk_text::FontContext;

const HTML: &str = r##"<!DOCTYPE html><html><head><meta charset="utf-8"><style>
 body{margin:0;font:16px/20px monospace}
 table{border-collapse:separate;border-spacing:0}
 td,th{padding:0}
 .tall{width:30px;height:40px}
</style></head><body>
<table id="u1"><tr><td><div class="tall"></div></td><td><span id="m1">t</span></td></tr></table>
<table id="u2"><tr><td><div style="width:30px;height:60px"></div></td><td><span id="m2">t</span></td></tr></table>
<table id="u3"><tr><td style="vertical-align:middle"><div class="tall"></div></td><td style="vertical-align:middle"><span id="m3">t</span></td></tr></table>
<table id="u4"><tr><td style="vertical-align:top"><div class="tall"></div></td><td style="vertical-align:top"><span id="m4">t</span></td></tr></table>
<table id="u5"><tr><td style="vertical-align:bottom"><div class="tall"></div></td><td><span id="m5">t</span></td></tr></table>
<table id="u6"><tbody style="vertical-align:top"><tr><td><div class="tall"></div></td><td><span id="m6">t</span></td></tr></tbody></table>
<table id="u7"><tr style="vertical-align:bottom"><td><div class="tall"></div></td><td><span id="m7">t</span></td></tr></table>
<table id="u8"><tr><td><div class="tall"></div></td><th><span id="m8">t</span></th></tr></table>
<table id="u9"><tr><td><div class="tall"></div></td><td style="font:32px/40px monospace"><span id="m9">t</span></td></tr></table>
<table id="z1"><tr><td><img src="x.gif" style="width:30px;height:40px"></td><td><span id="k1">t</span></td></tr></table>
<div id="z2" style="display:table"><div style="display:table-row"><div style="display:table-cell"><img src="x.gif" style="width:30px;height:40px"></div><div style="display:table-cell"><span id="k2">t</span></div></div></div>
</body></html>
"##;

fn rect(page: &manuk_page::Page, sel: &str) -> (f32, f32, f32, f32) {
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
    (r.x, r.y, r.width, r.height)
}

/// Height. The rows here are mostly about WHERE the content sits, but a row whose height moved is
/// a different bug wearing this one's clothes, so each subject pins both.
fn h(page: &manuk_page::Page, sel: &str, want: f32) {
    let (_, _, _, got) = rect(page, sel);
    assert!(
        (got - want).abs() < 1.01,
        "G_TD_VERTICAL_ALIGN_MIDDLE: `{sel}` expected h={want} (CAPTURED from `google-chrome \
         --headless --hide-scrollbars --window-size=1200,800`), got h={got}"
    );
}

/// **The whole tick, in one number: how far the cell's TEXT sits from the top of its table.** A
/// table that is the right height with its text jammed against the top passes every height
/// assertion ever written and fails this one.
fn dy(page: &manuk_page::Page, outer: &str, inner: &str, want: f32) {
    let (_, oy, _, _) = rect(page, outer);
    let (_, iy, _, _) = rect(page, inner);
    let got = iy - oy;
    assert!(
        (got - want).abs() < 1.01,
        "G_TD_VERTICAL_ALIGN_MIDDLE: `{inner}` inside `{outer}` expected dy={want} (CAPTURED from \
         `google-chrome --headless --hide-scrollbars --window-size=1200,800`), got dy={got}"
    );
}

#[test]
fn g_td_vertical_align_middle() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://tdva.test/", &fonts, 1200.0);

    // ── THE SUBJECT: a default cell centres its content.
    h(&page, "#u1", 40.0);
    dy(&page, "#u1", "#m1", 10.0);
    h(&page, "#u2", 60.0);
    dy(&page, "#u2", "#m2", 20.0);
    h(&page, "#u8", 40.0);
    dy(&page, "#u8", "#m8", 10.0); // a <th> is the same rule
    h(&page, "#z1", 45.0);
    dy(&page, "#z1", "#k1", 12.5); // an INLINE image makes the cell 45; half of the 25 gap

    // ── `inherit`, NOT `middle` — the two rows that tell the right rule from the plausible one.
    h(&page, "#u6", 40.0);
    dy(&page, "#u6", "#m6", 0.0); // <tbody vertical-align:top> reaches the cell
    h(&page, "#u7", 40.0);
    dy(&page, "#u7", "#m7", 20.0); // …and so does <tr vertical-align:bottom>

    // ── CONTROLS. The explicit-value rows were ALREADY Chrome-exact before this tick — they are
    //    what said the alignment MODES all work and only the UA declaration was missing.
    h(&page, "#u3", 40.0);
    dy(&page, "#u3", "#m3", 10.0);
    h(&page, "#u4", 40.0);
    dy(&page, "#u4", "#m4", 0.0);
    h(&page, "#u5", 40.0);
    dy(&page, "#u5", "#m5", 10.0); // a `bottom` on the OTHER cell leaves this one centred
    h(&page, "#u9", 40.0); // a 32px/40px cell does not grow a 40px row

    // ── THE ROW THAT KEEPS THE RULE ON THE TAG: `display: table-cell` is NOT `<td>`, so it keeps
    //    the initial `baseline` and its text sits a full 25 down. Already exact before the tick.
    h(&page, "#z2", 45.0);
    dy(&page, "#z2", "#k2", 25.0);
}
