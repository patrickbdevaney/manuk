//! # G_BLOCK_REPLACED_HAS_NO_LINE_BOX — `<img style="display:block">` is not a line box
//!
//! ⚠⚠⚠ **THE BASELINE SEARCH CLASSIFIED A REPLACED ELEMENT BY ITS TAG NAME, AND `display` IS WHAT
//! DECIDES.** `kid_own_baseline` answered *"a replaced element's baseline is its bottom margin
//! edge"* for any `<img>/<canvas>/<video>/<svg>/<object>/<embed>/<iframe>` it met — and
//! `is_atomic_inline`'s own doc wrote the assumption down: *"a replaced element's computed display
//! is `inline` and it is atomic anyway."* `<img style="display:block">` falsifies it. That spelling
//! is not exotic: it is the CSS-reset idiom (`img { display: block }` ships in Tailwind's preflight
//! and every `normalize.css` descendant, precisely to kill the inline descender gap) and it is what
//! `news.ycombinator.com`'s own logo carries.
//!
//! A **block-level** replaced element produces no line box at all. CSS 2.1 §17.5.4 baseline-aligns
//! the cells of a table row, and a cell that reported its image's bottom edge as a first-line
//! baseline dragged the whole row down by the *neighbouring text cell's descender* — 5px on every
//! `<td><img></td>` beside a `<td>text</td>`, which is the masthead of every table-laid-out page
//! there is.
//!
//! ⚠⚠⚠ **AND THE TWO CONSUMERS WANT OPPOSITE ANSWERS — ONE SHARED PREDICATE IS WRONG TWICE.** The
//! §10.8.1 question an **inline-block** asks is *last* line box + synthesise, and Blink's
//! `InlineBlockBaseline` recurses into block children and takes a replaced child's bottom edge at
//! any display. The *first*-line-box question a table cell and a flex item ask must answer "no line
//! box here". Fixing the cell with one predicate broke the inline-block by exactly the same 5px in
//! the other direction, which is what `#u7` below is for.
//!
//! Every number below is CAPTURED from
//! `google-chrome --headless --hide-scrollbars --window-size=1200,800 --dump-dom`.
//!
//! ```text
//!                                                        Chrome   one-rule   before   after
//!   #u1  <td><img display:block></td> + <td>text</td>       40        40        45      40
//!   #u5  …the same image wrapped in an <a>                  40        40        45      40
//!   #u7  <span inline-block pad-bottom:12><img block>text   52        57        52      52
//!   #u2  …the image left INLINE                    CTRL     45        45        45      45
//!   #u8  …the inline-block's image left INLINE     CTRL     57        57        57      57
//!   #u3  a bare 40px <div> instead of the image    CTRL     40        40        40      40
//!   #u4  a <div> AFTER text in the cell            CTRL     60        60        60      60
//!   #u6  the block image's cell, ALONE in its row  CTRL     40        40        40      40
//!   #u9  a baseline-aligned FLEX item              CTRL     45        45        45      45
//!   news.ycombinator.com's #hnmain                        1163         —      1173    1169
//!   news.ycombinator.com's #bigbox y                        42         —        46      42
//! ```
//!
//! ⭐ **THE CONTROLS ARE WHAT IDENTIFY IT, AND `#u6` IS THE ONE THAT COULD NOT BE GUESSED.** `#u3`
//! (a bare `<div>`) was already exact, so the engine already knew a cell with no line box has no
//! baseline — the defect is confined to the box that is BOTH replaced AND block, an intersection
//! the tag-name test could not see. `#u6` is that same cell with **no text neighbour**: it was
//! already 40 and stays 40, which says the bug was never in the cell's own height and always in
//! what it contributed to its ROW. And `#u2`/`#u8` are the pair that stops the fix from becoming
//! *"a replaced element never has a baseline"* — an inline image must still grow its row by the
//! neighbour's descender (45, not 40).
//!
//! ⚠ `#u7` is the row that forced the two questions apart, and it is a control that CAN lose: with
//! the single shared predicate it reads 57 against Chrome's 52, because an inline-block's
//! `padding-bottom` sits BELOW its baseline and is only there if the block image still hands
//! §10.8.1 its bottom edge.
//!
//! ⚠ `#u9`/`#u10` are insensitive by construction and are here to say so: a flex item with no
//! baseline synthesises one from its border box, which for these items is the same 40 the image
//! already reported. They pin that this tick moved flex by nothing.
//!
//! ⚠⚠⚠ **THE TOOTH WAS RE-CUT AT t1366, BECAUSE THAT TICK TOOK IT OUT.** t1366 gave `<td>`/`<th>`
//! Chrome's UA `vertical-align: middle`, and a CENTRED cell never asks for a baseline — so every
//! `<td>` row above (`#u1`, `#u5`) became jointly satisfied and the first mutation below stopped
//! going red anywhere. A capability tick that silently blunts a banked gate is a TRADE, so t1366
//! added `#u11`–`#u13`: the same question asked of a FLEX item, which has no UA declaration to hide
//! behind. `#u11`'s item carries 12px of `padding-bottom`, which is what separates the two possible
//! answers — the item's SYNTHESISED baseline is its bottom border edge (52), the block image's own
//! bottom margin edge is 40 — and Chrome puts the row at 57 with its text at dy=37.
//!
//! ```text
//!                                                        Chrome   mut-1   after
//!   #u11 flex item pad-bottom:12 > <img block>    h         57      52      57
//!        …its text sibling's dy                            37      20      37
//!   #u12 …the image left INLINE                   CTRL      57      57      57
//!   #u13 …a bare 40px <div>                       CTRL      57      57      57
//! ```
use manuk_text::FontContext;

const HTML: &str = r##"<!DOCTYPE html><html><head><meta charset="utf-8"><style>
 body{margin:0;font:16px/20px monospace}
 table{border-collapse:separate;border-spacing:0}
 td{padding:0}
 .im{width:30px;height:40px}
</style></head><body>
<table id="u1"><tr><td id="c1"><img class="im" src="x.gif" style="display:block"></td><td id="c2">text</td></tr></table>
<table id="u2"><tr><td id="c3"><img class="im" src="x.gif"></td><td id="c4">text</td></tr></table>
<table id="u3"><tr><td id="c5"><div style="width:30px;height:40px"></div></td><td id="c6">text</td></tr></table>
<table id="u4"><tr><td id="c7">x<div style="width:30px;height:40px"></div></td><td id="c8">text</td></tr></table>
<table id="u5"><tr><td id="c9"><a href="#"><img class="im" src="x.gif" style="display:block"></a></td><td id="c10">text</td></tr></table>
<table id="u6"><tr><td id="c11"><img class="im" src="x.gif" style="display:block"></td></tr></table>
<div id="u7" style="width:300px"><span id="s1" style="display:inline-block;padding-bottom:12px"><img class="im" src="x.gif" style="display:block"></span>text</div>
<div id="u8" style="width:300px"><span id="s2" style="display:inline-block;padding-bottom:12px"><img class="im" src="x.gif"></span>text</div>
<div id="u9" style="display:flex;align-items:baseline;width:300px"><div id="f1"><img class="im" src="x.gif" style="display:block"></div><div id="f2">text</div></div>
<div id="u10" style="display:flex;align-items:baseline;width:300px"><div id="f3"><img class="im" src="x.gif"></div><div id="f4">text</div></div>
<div id="u11" style="display:flex;align-items:baseline;width:300px"><div id="f5" style="padding-bottom:12px"><img class="im" src="x.gif" style="display:block"></div><div id="f6">text</div></div>
<div id="u12" style="display:flex;align-items:baseline;width:300px"><div id="f7" style="padding-bottom:12px"><img class="im" src="x.gif"></div><div id="f8">text</div></div>
<div id="u13" style="display:flex;align-items:baseline;width:300px"><div id="f9" style="padding-bottom:12px"><div style="width:30px;height:40px"></div></div><div id="f10">text</div></div>
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

/// Height only. Every row of this gate is a HEIGHT claim — the widths never moved and asserting
/// them would print a second number for one regression.
fn h(page: &manuk_page::Page, sel: &str, want: f32) {
    let (_, _, _, got) = rect(page, sel);
    assert!(
        (got - want).abs() < 1.01,
        "G_BLOCK_REPLACED_HAS_NO_LINE_BOX: `{sel}` expected h={want} (CAPTURED from \
         `google-chrome --headless --hide-scrollbars --window-size=1200,800`), got h={got}"
    );
}

/// Where a box's own content sits relative to its container's top — the half a height cannot see.
/// A container that is the right height for the wrong reason passes every height assertion above
/// and fails this one.
fn top_offset(page: &manuk_page::Page, outer: &str, inner: &str, want: f32) {
    let (_, oy, _, _) = rect(page, outer);
    let (_, iy, _, _) = rect(page, inner);
    assert!(
        (iy - oy - want).abs() < 1.01,
        "G_BLOCK_REPLACED_HAS_NO_LINE_BOX: `{inner}` must start {want}px below `{outer}` (Chrome), \
         got {}",
        iy - oy
    );
}

#[test]
fn g_block_replaced_has_no_line_box() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://brl.test/", &fonts, 1200.0);

    // ── THE SUBJECT: a block-level replaced element contributes no first-line baseline, so the row
    //    is its image's own 40 and not 40 + the neighbour's descender.
    h(&page, "#u1", 40.0);
    h(&page, "#c1", 40.0);
    h(&page, "#c2", 40.0);
    // The same image behind an `<a>` — the shape the real web actually writes, and the one
    // `news.ycombinator.com` carries. The `<a>` is an inline blockified by its block child
    // (CSS2 §9.2.1.1), so this row also pins that the search walks THROUGH the split inline.
    h(&page, "#u5", 40.0);
    h(&page, "#c9", 40.0);

    // ── THE OTHER HALF OF THE SPLIT: §10.8.1 still takes the block image's bottom edge, so the
    //    inline-block's `padding-bottom` hangs BELOW the shared baseline. One shared predicate
    //    reads 57 here.
    h(&page, "#u7", 52.0);
    h(&page, "#s1", 52.0);

    // ── CONTROLS THAT MUST NOT MOVE.
    h(&page, "#u2", 45.0); // an INLINE image is a line box: the row keeps the descender
    h(&page, "#c3", 45.0);
    h(&page, "#u8", 57.0); // …and so does the inline-block holding one
    h(&page, "#s2", 57.0);
    h(&page, "#u3", 40.0); // a bare block <div>: already had no baseline, already exact
    h(&page, "#u4", 60.0); // text THEN a block: the cell HAS a line box and still aligns
    h(&page, "#u6", 40.0); // the same cell with no text neighbour: never wrong, still right
    h(&page, "#u9", 45.0); // a flex item synthesises from its border box either way
    h(&page, "#u10", 45.0);

    // ── WHERE THE CONTENT SITS, which is the half a height cannot see. Only the flex row is
    //    asserted, and the reason the table rows are NOT is a finding this gate refuses to bury:
    //
    // ⚠⚠⚠ **CHROME'S `<td>` COMPUTES `vertical-align: middle`, NOT `baseline`, AND OURS COMPUTES
    // `baseline`.** Blink's UA sheet declares `thead, tbody, tfoot { vertical-align: middle }` and
    // `tr`/`td`/`th` inherit it, so a default HTML table centres every cell's content and never
    // forms a baseline group at all. Measured on this very fixture, `getComputedStyle(td)
    // .verticalAlign`:
    //
    // ```text
    //                                                   Chrome    here
    //   a plain <td>                                     middle  baseline
    //   the text inside `#u1`'s text cell, from row top     10       0
    //   the text inside `#u4`'s text cell                   20       0
    //   `vertical-align:middle` written explicitly  CTRL     20      20
    //   `vertical-align:top` written explicitly     CTRL      0       0
    // ```
    //
    // The two explicit-value controls say the ALIGNMENT MODES all work; what is absent is one UA
    // declaration, exactly as `border-spacing` was in `G_TABLE_BORDER_SPACING_UA_DEFAULT`. It is a
    // separate tick and a bigger one — it moves text on every table-laid-out page — and asserting
    // today's `0` here would PIN the engine to it. So the rows above assert only what is
    // Chrome-exact today (the heights), and this comment is the receipt for what is not.
    top_offset(&page, "#u9", "#f2", 25.0); // flex: baseline-aligned, and unmoved by this tick
    top_offset(&page, "#u10", "#f4", 25.0);

    // ── ⚠⚠⚠ THE TOOTH, RE-CUT AT t1366. See this file's header: `#u1`/`#u5` stopped being able
    //    to fail once `<td>` got its UA `vertical-align: middle`, because a cell that is centred
    //    never asks for a baseline at all. These three rows ask the SAME question of a box that
    //    has no UA declaration to hide behind — a FLEX item, where `align-items: baseline` makes
    //    the first-line baseline load-bearing and 12px of `padding-bottom` separates the two
    //    possible answers: the item's synthesised baseline is its bottom BORDER edge (52), while
    //    the block image's own bottom margin edge is 40.
    h(&page, "#u11", 57.0);
    top_offset(&page, "#u11", "#f6", 37.0);
    h(&page, "#u12", 57.0); // CTRL the image left INLINE — it IS a line box, baseline 25
    top_offset(&page, "#u12", "#f8", 25.0);
    h(&page, "#u13", 57.0); // CTRL a bare <div> — already had no baseline before this tick
    top_offset(&page, "#u13", "#f10", 37.0);
}
