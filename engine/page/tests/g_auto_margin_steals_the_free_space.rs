//! # G_AUTO_MARGIN_STEALS_THE_FREE_SPACE — and `justify-content` has none left to distribute
//!
//! ⚠⚠⚠ **BOTH MECHANISMS SPENT THE SAME FREE SPACE, SO EVERY ITEM AFTER AN `auto` MARGIN WAS
//! DISPLACED BY ONE WHOLE FREE-SPACE WIDTH.** CSS Flexbox §8.1: *"If free space is distributed to
//! auto margins, the alignment properties will have no effect in that dimension because the margins
//! will have stolen all the free space."*
//! `taffy::compute::flexbox::distribute_remaining_free_space` (taffy 0.12.1) hands the free space to
//! the auto margins and then calls `compute_alignment_offset(free_space, …)` with **the same,
//! undiminished** `free_space`.
//!
//! On `whatwg.org` — a named burndown anchor — every `#links-with-explanations a` is
//! `display:flex; justify-content:space-between` with `a > strong { margin-right: auto }`, so each
//! link's description ran off the right edge of its own 884px box. Fixing it moved the anchor's
//! **shape from 78.4% to 89.2%** and its misplaced count from 8 to 4.
//!
//! Every number below is CAPTURED from `google-chrome --headless --hide-scrollbars
//! --window-size=1200,800`: a `display:flex` row, `font: 16px monospace`, two items of 39px
//! (`<strong>AAAA</strong><p>BBBB</p>`), `strong { margin-right: auto }`. The left column is a
//! 600px row (free space **+522**); the right column is a 60px row (free space **−18**). Each cell
//! is the two items' x, relative to the row.
//!
//! ```text
//!                          600px row  (free space > 0)      60px row  (free space < 0)
//!    justify-content       Chrome        before   after     Chrome      before   after
//!      flex-start          [0,561]      [0,561]  [0,561]    [0,39]     [0,39]   [0,39]   CTRL
//!      flex-end            [0,561]    [523,1084] [0,561]    [-17,21]   [-17,21] [-17,21] CTRL
//!      center              [0,561]    [261,823]  [0,561]    [-9,30]    [-9,30]  [-9,30]  CTRL
//!      space-between       [0,561]      [0,1084] [0,561]    [0,39]     [0,39]   [0,39]   CTRL
//!      space-around        [0,561]    [131,954]  [0,561]    [0,39]     [0,39]   [0,39]   CTRL
//!      space-evenly        [0,561]    [174,910]  [0,561]    [0,39]     [0,39]   [0,39]   CTRL
//!
//!    #x0  center + `margin-top:auto` (CROSS axis), 600px row   Chrome [261,300]   CTRL
//! ```
//!
//! ⭐⭐ **THE RIGHT-HAND COLUMN IS THE WHOLE GATE, AND IT IS SIX ROWS THAT DO NOT MOVE.** The
//! obvious fix — *"drop `justify-content` whenever an item has an auto margin"* — passes all six
//! rows on the left and **breaks `flex-end` and `center` on the right**, where a NEGATIVE free space
//! means the auto margins resolve to zero and the alignment does apply. Those two cells
//! (`[-17,21]`, `[-9,30]`) were already Chrome-exact before this tick, so the naive fix trades two
//! correct answers for five. **The predicate is the SIGN of the free space, not the presence of an
//! auto margin** — and only a completed layout knows the sign, which is why the correction is a
//! second solve rather than a style tweak.
//!
//! The correction sets `justify_content: None` (= `FLEX_START`), whose alignment offset is 0 for
//! any free space — arithmetically identical to handing the justify step the zero free space the
//! auto margins left it.
//!
//! ⚠ **COST, MEASURED.** The second solve runs only for a subtree that actually contains such a
//! container; the detection is one O(nodes) scan. `docs/bench/mid.html` and `large.html` contain
//! **zero** auto-margin declarations, so the floors never enter the second pass, and a same-hour
//! old-binary control put the new binary at mid 47.82ms / large 313.83ms against the old 55.47 /
//! 367.71 — inside the run-to-run spread, with the pass never firing.
//!
//! ⚠ MEASURED, NOT FIXED, AND DELIBERATELY NOT ASSERTED HERE: **a `::before` with `content:""` is
//! not generated as a FLEX ITEM.** `whatwg.org`'s links are `<a>` flex rows whose icon is
//! `a::before { content:""; width:30px; margin:12px 20px 12px 8px }` — 58px of flex item — and its
//! four remaining misplaced elements are exactly the four `<strong>`s at Chrome's 68 against our 10,
//! a 58px offset. That is a pseudo-element/box-generation defect, not a distribution one, and it is
//! the next tick.
use manuk_text::FontContext;

const HTML: &str = r##"<!DOCTYPE html><html><head><meta charset="utf-8"><style>
 *{box-sizing:border-box}
 body{margin:0;font:16px monospace}
 .row{display:flex;background:#eee;height:30px}
 .row > strong{margin-right:auto}
</style></head><body>
<div class="row" id="w0" style="justify-content:flex-start;width:600px"><strong id="w0s">AAAA</strong><p id="w0p">BBBB</p></div>
<div class="row" id="n0" style="justify-content:flex-start;width:60px"><strong id="n0s">AAAA</strong><p id="n0p">BBBB</p></div>
<div class="row" id="w1" style="justify-content:flex-end;width:600px"><strong id="w1s">AAAA</strong><p id="w1p">BBBB</p></div>
<div class="row" id="n1" style="justify-content:flex-end;width:60px"><strong id="n1s">AAAA</strong><p id="n1p">BBBB</p></div>
<div class="row" id="w2" style="justify-content:center;width:600px"><strong id="w2s">AAAA</strong><p id="w2p">BBBB</p></div>
<div class="row" id="n2" style="justify-content:center;width:60px"><strong id="n2s">AAAA</strong><p id="n2p">BBBB</p></div>
<div class="row" id="w3" style="justify-content:space-between;width:600px"><strong id="w3s">AAAA</strong><p id="w3p">BBBB</p></div>
<div class="row" id="n3" style="justify-content:space-between;width:60px"><strong id="n3s">AAAA</strong><p id="n3p">BBBB</p></div>
<div class="row" id="w4" style="justify-content:space-around;width:600px"><strong id="w4s">AAAA</strong><p id="w4p">BBBB</p></div>
<div class="row" id="n4" style="justify-content:space-around;width:60px"><strong id="n4s">AAAA</strong><p id="n4p">BBBB</p></div>
<div class="row" id="w5" style="justify-content:space-evenly;width:600px"><strong id="w5s">AAAA</strong><p id="w5p">BBBB</p></div>
<div class="row" id="n5" style="justify-content:space-evenly;width:60px"><strong id="n5s">AAAA</strong><p id="n5p">BBBB</p></div>
<div class="row" id="x0" style="justify-content:center;width:600px;height:60px"><strong id="x0s" style="margin-right:0;margin-top:auto">AAAA</strong><p id="x0p">BBBB</p></div>
</body></html>
"##;

fn relx(page: &manuk_page::Page, sel: &str) -> f32 {
    let dom = page.dom();
    let n = manuk_css::query_selector_all(dom, dom.root(), sel)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("selector {sel} matched nothing"));
    let rects = page.root_box.node_rects(dom);
    let me = rects
        .get(&n)
        .copied()
        .unwrap_or_else(|| panic!("no box for {sel}"));
    let parent = dom.parent(n).expect("row parent");
    let row = rects.get(&parent).copied().expect("no box for the row");
    me.x - row.x
}

/// One row: both items' x relative to the row, which is what the whole tick is.
fn row(page: &manuk_page::Page, id: &str, want: (f32, f32), why: &str) {
    let got = (
        relx(page, &format!("#{id}s")),
        relx(page, &format!("#{id}p")),
    );
    assert!(
        (got.0 - want.0).abs() < 1.01 && (got.1 - want.1).abs() < 1.01,
        "G_AUTO_MARGIN_STEALS_THE_FREE_SPACE: `#{id}` expected [{}, {}] (CAPTURED from \
         `google-chrome --headless --hide-scrollbars --window-size=1200,800`), got [{}, {}] — {}",
        want.0,
        want.1,
        got.0,
        got.1,
        why
    );
}

#[test]
fn g_auto_margin_steals_the_free_space() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://automargin.test/", &fonts, 1200.0);

    // ── POSITIVE FREE SPACE: the auto margin takes all of it, so every alignment is FLEX_START.
    row(
        &page,
        "w0",
        (0.0, 561.0),
        "flex-start: the auto margin absorbs the free space, which is also the answer alignment \
         would give — the row that cannot tell the two mechanisms apart",
    );
    row(
        &page,
        "w1",
        (0.0, 561.0),
        "flex-end must have NO effect once an auto margin has taken the free space",
    );
    row(&page, "w2", (0.0, 561.0), "…nor center");
    row(
        &page,
        "w3",
        (0.0, 561.0),
        "…nor space-between — whatwg.org's own declaration",
    );
    row(&page, "w4", (0.0, 561.0), "…nor space-around");
    row(&page, "w5", (0.0, 561.0), "…nor space-evenly");

    // ── ⭐⭐ NEGATIVE FREE SPACE: the auto margins resolve to ZERO and the alignment DOES apply.
    //    These six were already correct before the tick, and the naive fix breaks two of them.
    row(
        &page,
        "n0",
        (0.0, 39.0),
        "CONTROL: an overflowing row has no free space to steal",
    );
    row(
        &page,
        "n1",
        (-17.0, 21.0),
        "⭐ CONTROL: flex-end still applies when free space is NEGATIVE — dropping justify-content \
         whenever an auto margin exists puts this back at [0,39]",
    );
    row(
        &page,
        "n2",
        (-9.0, 30.0),
        "⭐ CONTROL: and so does center — the second row the unconditional fix breaks",
    );
    row(
        &page,
        "n3",
        (0.0, 39.0),
        "CONTROL: space-between already falls back to flex-start below zero free space",
    );
    row(&page, "n4", (0.0, 39.0), "CONTROL: space-around likewise");
    row(&page, "n5", (0.0, 39.0), "CONTROL: space-evenly likewise");

    // ── ⭐ THE AXIS CONTROL. Only a MAIN-axis auto margin can steal MAIN-axis free space; a
    //    cross-axis one is resolved by taffy's `resolve_cross_axis_auto_margins` and never touches
    //    this distribution. `margin-top: auto` in a ROW container is cross-axis, so `center` must
    //    still apply — without this row the "any auto margin on any side" mutation is inert,
    //    because every other row's auto margin is already on the main axis.
    row(
        &page,
        "x0",
        (261.0, 300.0),
        "⭐ a CROSS-axis `margin-top:auto` in a row container steals nothing on the main axis, so \
         justify-content:center still centres",
    );
}
