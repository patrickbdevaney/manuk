//! # G_LEGACY_ALIGN_IS_NOT_TEXT_ALIGN_CENTER — `<center>` computes `-webkit-center`, and a
//! `<table>` RESETS it
//!
//! ⚠⚠⚠ **HTML'S LEGACY ALIGNMENT AND CSS'S `text-align` SHARE A NAME AND ARE NOT THE SAME
//! PROPERTY, AND WE HAD ONLY THE CSS ONE.** `<center>`, `<div align=center>` and `<td align=right>`
//! do not compute to `center`/`right`; they compute to `-webkit-center` / `-webkit-right` (Stylo
//! spells them `-moz-*`), and those keywords differ from their CSS twins in exactly two ways:
//!
//! * **a `<table>` resets them to `start`**, so the cells of a table inside a `<center>` are
//!   LEFT-aligned — while a real `text-align: center` inherits straight into them; and
//! * **they align BLOCK-LEVEL children**, which is the only reason `<center>` centres a table at
//!   all.
//!
//! Our UA sheet said `center { text-align: center }` and `stylo_map` folded `MozCenter` into
//! `Center`, so both halves were lost. The first is the visible one: **`news.ycombinator.com` is a
//! `<table>` inside a `<center>`, so every story title rendered centred in its cell.**
//!
//! ```text
//!   news.ycombinator.com, story #1              Chrome    before    after
//!     the title text          "507 Mechanical…"  129.3      295      129
//!     the subtext score span  #score_49465169    129.3      306      129
//!     the rank                span.rank          104.2      101      104   ← `<td align=right>`
//! ```
//!
//! The rank moved too, and by a different half of the same mechanism: HTML's `align` attribute had
//! **no mapping to `text-align` at all** here, on any element.
//!
//! ⭐ **THE MAPPING IS THREE FAMILIES, NOT ONE, AND THE GATE'S KEY ROW IS THE ONE THAT SEPARATES
//! THEM.** Measured in headless Chrome via `getComputedStyle().textAlign`, not reasoned from prose:
//!
//! ```text
//!   <div align=center>    -webkit-center     <h1 align=center>        center     ← generic
//!   <p  align=center>     -webkit-center     <section align=center>   center
//!   <td align=center>     -webkit-center     <li align=right>         right
//!   <tr align=center>     -webkit-center     <fieldset align=right>   right
//!   <tbody align=right>   -webkit-right      <blockquote align=right> right
//!   <col align=right>     -webkit-right      <legend align=right>     right
//!   <div align=justify>   justify            <img align=right>        start   (float:right)
//!   <div align=middle>    -webkit-center     <table align=right>      start   (float:right)
//!                                            <hr|input|marquee>       start
//! ```
//!
//! So `div`, `p` and the table parts get the LEGACY keywords; `img`/`object`/`embed`/`iframe`/
//! `table`/`hr`/`input`/`marquee` get no `text-align` at all (their `align` means FLOAT); and every
//! other element gets the literal value with `middle` folded to `center`. Blink spells this as three
//! `CollectStyleForPresentationAttribute` overrides — `HTMLDivElement`, `HTMLParagraphElement`,
//! `HTMLTablePartElement` — over the `HTMLElement` base.
//!
//! Every number below is CAPTURED from
//! `google-chrome --headless --hide-scrollbars --window-size=1200,800`, 16px/20px monospace. The
//! last column is the MUTATION that turns the row red — a "before" column is not written here
//! because no single earlier state produces all of them (the defect was three separate holes), and
//! the measured before/after numbers that DO exist are on the real page, above.
//!
//! ```text
//!                                                          Chrome x   goes red under
//!   #m1  <center> → a table CELL's text            KEY        300      M1 (295) M2 (0) M4 (0) M5 (0)
//!   #m2  <center> → a 50px BLOCK child             KEY        575      M4
//!   #m3t <div align=center> → the table                       300      M4
//!   #m3  …its cell's text                                     300      M2
//!   #m4  <div align=right> → a 50px BLOCK child              1150      M4
//!   #m5  <td align=center> → its text                      295.17      the align hint removed
//!   #m6  <td align=justify> → its text             CTRL           0      —
//!   #m7  <h1 align=center> → its text                      595.17      the align hint removed
//!   #m8  <div align=center><h1 align=center>               595.17      the align hint removed
//!   #m9  <h1 align=center> → a BLOCK child         KEY           0      M3 (575)
//!   #j1  text-align:center → a table CELL's text   CTRL     295.17      M2b (0)
//!   #j2  text-align:center → a BLOCK child         CTRL           0      —
//!   #j4  <table align=center> → its cell's text    CTRL         300      —
//!   #m10t a table nested inside the centred one                 300      M4
//!   #m10 …its cell's text                                       300      M2
//!   #m11 <center><div style="text-align:center">    CTRL     595.17      —
//! ```
//!
//! ⭐ **`#m9` IS THE ROW THAT LOOKS LIKE A DECORATION AND IS THE ONLY ONE THAT TELLS THE THREE
//! FAMILIES APART.** It reads 0 before this tick and 0 after — a row that did not move. Put `h1` in
//! the legacy list (the plausible *"all `align=center` is `-webkit-center`"* simplification) and
//! **every other row in the gate still passes** while `#m9` jumps to 575, because only the legacy
//! keywords move a block child. `#m7` is its mirror: the same `<h1 align=center>` must still centre
//! its *inline* text, so a fix that simply dropped `h1` from the mapping fails there instead.
//!
//! ⭐ **`#j1` IS THE ROW THAT REFUSES THE PLAUSIBLE WRONG FIX, AND IT WAS RUN.** Spelling the reset
//! as a UA rule `table { text-align: start }` — which is how one would naturally *"stop tables
//! inheriting the centring"* — leaves every subject row above passing and puts `#j1` at **0**
//! (measured, mutation M2b): a declaration beats inheritance always, so a real author
//! `text-align: center` would stop reaching table cells too. The reset has to be conditional on the
//! VALUE being a legacy keyword, which is exactly what Stylo's own `adjust_for_table_text_align`
//! does — already compiled into our build, and firing for the first time now that it is fed
//! `MozCenter` instead of `Center`.
//!
//! MUTATIONS, each applied to the engine, rebuilt, and read back:
//!
//! * **M1 — `center { text-align: center }`**, the original UA spelling → `#m1` 295.18, not 300
//! * **M2 — M1 plus a UA `table { text-align: start }`**, the plausible whole fix → `#m1` 0
//! * **M2b — a UA `table { text-align: start }` on top of the CORRECT fix** → `#j1` 0, not 295,
//!   with every subject row still green. This is the mutation the gate exists for.
//! * **M3 — `h1` added to the LEGACY list** → `#m9` 575, not 0
//! * **M4 — `apply_legacy_block_align` not called** → `#m1` 0
//! * **M5 — `map_text_align` folding `MozCenter` back into `Center`** → `#m1` 0 (the fold is what
//!   made both halves invisible in the first place)
//!
//! ⚠ **MEASURED, NOT FIXED, AND DELIBERATELY NOT ASSERTED HERE.** The OTHER half of the `align`
//! attribute is FLOAT: `<img align=right>` maps to `float: right; vertical-align: top` and
//! `<table align=left|right>` to `float: left|right`. We implement neither, so
//! `<div><img align=right><span>t</span></div>` puts the text at x=10 where Chrome puts it at 0.
//! That is a second, separate mapping (attribute → `float`/`vertical-align`, not → `text-align`)
//! and it is the next tick; it is left out of this gate rather than pinned at our wrong value.
use manuk_text::FontContext;

const HTML: &str = r##"<!DOCTYPE html><html><head><meta charset="utf-8"><style>
 body{margin:0;font:16px/20px monospace}
 table{border-collapse:separate;border-spacing:0}
 td,th{padding:0}
 .blk{width:50px;height:10px;background:#ccc}
 h1{font:16px/20px monospace;margin:0}
</style></head><body>
<center id="u1"><table width="600"><tr><td><span id="m1">t</span></td></tr></table></center>
<center id="u2"><div class="blk" id="m2"></div></center>
<div align="center" id="u3"><table width="600" id="m3t"><tr><td><span id="m3">t</span></td></tr></table></div>
<div align="right" id="u4"><div class="blk" id="m4"></div></div>
<table id="u5" width="600"><tr><td align="center"><span id="m5">t</span></td></tr></table>
<table id="u6" width="600"><tr><td align="justify"><span id="m6">t</span></td></tr></table>
<h1 align="center" id="u7"><span id="m7">t</span></h1>
<div align="center" id="u8"><h1 align="center" id="u8h"><span id="m8">t</span></h1></div>
<h1 align="center" id="u9"><div class="blk" id="m9"></div></h1>
<div style="text-align:center" id="k1"><table width="600"><tr><td><span id="j1">t</span></td></tr></table></div>
<div style="text-align:center" id="k2"><div class="blk" id="j2"></div></div>
<table align="center" width="600" id="k4"><tr><td><span id="j4">t</span></td></tr></table>
<center id="u10"><table width="600"><tr><td><table width="300" id="m10t"><tr><td><span id="m10">t</span></td></tr></table></td></tr></table></center>
<center id="u11"><div style="text-align:center" id="u11i"><span id="m11">t</span></div></center>
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

/// The whole tick is a HORIZONTAL position, so x is what every row asserts.
fn x(page: &manuk_page::Page, sel: &str, want: f32, why: &str) {
    let (got, _, _, _) = rect(page, sel);
    assert!(
        (got - want).abs() < 1.01,
        "G_LEGACY_ALIGN_IS_NOT_TEXT_ALIGN_CENTER: `{sel}` expected x={want} (CAPTURED from \
         `google-chrome --headless --hide-scrollbars --window-size=1200,800`), got x={got} — {why}"
    );
}

/// Width, on the two rows where a box that moved for the WRONG reason (a changed width rather than
/// a changed alignment) would otherwise read as a pass.
fn w(page: &manuk_page::Page, sel: &str, want: f32) {
    let (_, _, got, _) = rect(page, sel);
    assert!(
        (got - want).abs() < 1.01,
        "G_LEGACY_ALIGN_IS_NOT_TEXT_ALIGN_CENTER: `{sel}` expected w={want}, got w={got} — a box \
         at the right x for the wrong reason is not a pass"
    );
}

#[test]
fn g_legacy_align_is_not_text_align_center() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://legacyalign.test/", &fonts, 1200.0);

    // ── THE SUBJECT, HALF ONE: a <table> RESETS the legacy keyword, so its cells are left-aligned.
    //    This is the Hacker News defect: the cell text sits at the table's own left edge.
    w(&page, "#m3t", 600.0);
    x(
        &page,
        "#m1",
        300.0,
        "a cell inside <center> is LEFT-aligned: the table resets -webkit-center to start",
    );
    x(
        &page,
        "#m3",
        300.0,
        "…and <div align=center> is the same keyword by another spelling",
    );
    x(
        &page,
        "#m10",
        300.0,
        "…and the reset reaches a table NESTED inside the centred one",
    );
    w(&page, "#m10t", 300.0);

    // ── THE SUBJECT, HALF TWO: the legacy keywords align BLOCK-LEVEL children.
    x(
        &page,
        "#m2",
        575.0,
        "<center> centres a 50px BLOCK child, which plain `center` never does",
    );
    x(
        &page,
        "#m3t",
        300.0,
        "…and that is the whole reason <div align=center> centres a table",
    );
    x(
        &page,
        "#m4",
        1150.0,
        "-webkit-right pushes a block child to the END edge",
    );

    // ── THE SUBJECT, HALF THREE: the `align` attribute had NO text-align mapping at all.
    x(
        &page,
        "#m5",
        295.0,
        "<td align=center> centres its text (the table-part family)",
    );
    x(
        &page,
        "#m7",
        595.0,
        "<h1 align=center> centres its text (the generic family)",
    );
    x(
        &page,
        "#m8",
        595.0,
        "…and nested inside a <div align=center> it still does",
    );

    // ── THE ROW THAT SEPARATES THE GENERIC FAMILY FROM THE LEGACY ONE. Did not move this tick;
    //    goes red the moment `h1` is treated like `div`.
    x(
        &page,
        "#m9",
        0.0,
        "<h1 align=center> computes plain `center`, which does NOT move a block child — put h1 in \
         the legacy list and this jumps to 575 while every subject row above stays right",
    );

    // ── CONTROLS: what a REAL `text-align` must keep doing. #j1 is the row that refuses
    //    `table { text-align: start }`.
    x(
        &page,
        "#j1",
        295.0,
        "CSS `text-align: center` DOES inherit into a table cell — a UA `table{text-align:start}` \
         would put this at 0 and pass every subject row above",
    );
    x(
        &page,
        "#j2",
        0.0,
        "…and CSS `center` does NOT move a block child",
    );
    x(
        &page,
        "#j4",
        300.0,
        "<table align=center> still centres the table (margin-inline, not text-align)",
    );
    x(
        &page,
        "#m11",
        595.0,
        "an author's own text-align:center inside <center> still wins",
    );
    x(
        &page,
        "#m6",
        0.0,
        "align=justify falls to the literal value on a table part too",
    );
}
