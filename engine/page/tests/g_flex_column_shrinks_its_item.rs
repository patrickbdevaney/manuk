//! **G_FLEX_COLUMN_SHRINKS_ITS_ITEM — `flex-direction: column` never shrank an item, because the
//! block axis recorded taffy's verdict only for a PERCENTAGE height.**
//!
//! `extract_placed` records taffy's used border-box **width** for every flex/grid item
//! unconditionally — that is what makes `flex-shrink` work in a `row` container. The **height** was
//! recorded only when the item's own `height` was a percentage, on the reasoning that a percentage
//! is the one case taffy has *resolved* rather than *stretched*. That reasoning is right about
//! `auto` and wrong about a LENGTH: a `height: 300px` item in a `height: 80px` COLUMN container has
//! been shrunk by taffy (`flex-shrink` is 1 by default and negative free space is exactly what it is
//! for), and dropping that verdict re-resolved the item at its own 300px.
//!
//! ⭐ **The main axis is the WIDTH in a `row` container and the HEIGHT in a `column` one — so one
//! word in one condition discarded exactly one direction's `flex-shrink` and nothing else.** Every
//! `row` fixture in the suite passed before this tick and passes after it, which is what kept a
//! defect this size invisible for as long as the field has existed.
//!
//! Chrome-measured (`google-chrome --headless --dump-dom`), `.b { width:80px; height:80px }`:
//!
//! ```text
//!                                                        Chrome      before      after
//!   col   column, child height:300px                     50x80       50x300      50x80
//!   colm  column, child height:300px; margin:-100px 0    50x280      50x300      50x280
//!   row   row,    child width:300px          CONTROL     80x50       80x50       80x50
//!   rowm  row,    child width:300px; margin:0 -100px     280x50      280x50      280x50
//!   pct   column, child height:50%           REGRESSION  50x40       50x40       50x40
//!   aut   column, child height:auto (30px)   REGRESSION  50x30       50x30       50x30
//!   grd   GRID,   child height:300px in an 80px track    50x300      50x300      50x300
//! ```
//!
//! ⚠ **`aut` and `pct` are the two arms that say what the rule is NOT.** `auto` stays out, and the
//! percentage arm — the one that already existed — must survive: `pct` reads 40, and resolving the
//! percentage a second time against the slot it produced squares it to 20.
//!
//! ⚠⚠ **AND THIS GATE CANNOT CATCH THE `auto` ARM BEING WIDENED — SAID PLAINLY RATHER THAN
//! PRETENDED.** Adding `Dim::Auto` to the condition leaves all seven rows above unchanged, because
//! in a COLUMN container the height is the MAIN axis and an auto-height item's main size is already
//! its content size. The reason `auto` is nonetheless left out here is that this tick's measurement
//! says the `auto` case is a DIFFERENT, still-open defect rather than a hazard to defend against:
//!
//! ```text
//!   Chrome-measured, same 80x80 box                        Chrome    ours (both before and after)
//!   s1  row flex, item height:auto, 200px content            80          200      ← align-items:stretch
//!   s6  grid, 80px row track, item auto, 200px content       80          200
//!   s4  COLUMN flex, item auto, 200px content   CONTROL     200          200  ✓
//!   s5  row flex, align-items:flex-start, 200px  CONTROL    200          200  ✓
//! ```
//!
//! `stretch` sets an auto cross size to the LINE's cross size and lets the content overflow; we keep
//! the content height instead, because the adoption further down `extract_placed` is written
//! `slot > box` and so can only ever grow a box. That is the next tick, it is one axis and one
//! comparison, and s1/s4/s5/s6 are the fixture for it. It is deliberately NOT taken here: it is a
//! different mechanism from the one this gate is named for.
//!
//! ⭐⭐ **`grd` IS THE MEASUREMENT THAT KEPT A GUARD OUT OF THE TREE.** The obvious worry is that a
//! GRID item with a definite height should overflow a shorter track rather than be clamped to it, so
//! the first version of this fix carried a `parent_is_flex` scope. Taffy's slot height for that item
//! was measured directly and it is **300, not the 80px track** — the scope was inert, and
//! `css/css-grid`'s failing count was 4245 with it and 4245 without it across 643 files. One rule,
//! both formatting contexts; the guard is not shipped and this row is why.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><meta charset=utf-8><style>
body { margin:0; font:16px/20px monospace }
.b { width:80px; height:80px; overflow:hidden; scrollbar-width:none }
</style></head><body>
<div class="b" id="col"  style="display:flex;flex-direction:column"><div id="kcol" style="height:300px;width:50px"></div></div>
<div class="b" id="colm" style="display:flex;flex-direction:column"><div id="kcolm" style="height:300px;width:50px;margin:-100px 0"></div></div>
<div class="b" id="row"  style="display:flex"><div id="krow" style="width:300px;height:50px"></div></div>
<div class="b" id="rowm" style="display:flex"><div id="krowm" style="width:300px;height:50px;margin:0 -100px"></div></div>
<div class="b" id="pct"  style="display:flex;flex-direction:column"><div id="kpct" style="height:50%;width:50px"></div></div>
<div class="b" id="aut"  style="display:flex;flex-direction:column"><div id="kaut" style="width:50px"><div style="height:30px"></div></div></div>
<div class="b" id="grd"  style="display:grid;grid-template-rows:80px;grid-template-columns:80px"><div id="kgrd" style="height:300px;width:50px"></div></div>
<div id="out">-</div>
<script>
document.getElementById('out').textContent=[["col","kcol"],["colm","kcolm"],["row","krow"],["rowm","krowm"],["pct","kpct"],["aut","kaut"],["grd","kgrd"]].map(function(p){
  var e=document.getElementById(p[0]), k=document.getElementById(p[1]);
  return p[0]+'='+k.offsetWidth+'x'+k.offsetHeight+'@'+(k.offsetLeft-e.offsetLeft)+','+(k.offsetTop-e.offsetTop);
}).join(' ');
</script>
</body></html>"##;

#[test]
fn a_column_flex_container_shrinks_an_over_tall_item() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://fo.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("FLEX COLUMN SHRINK: {got}");

    // ── VACUITY. The ROW axis must already be exact, or these rows are about flex sizing in
    //    general rather than about the axis that was dropped.
    assert!(
        got.contains("row=80x50@0,0") && got.contains("rowm=280x50@-100,0"),
        "VACUOUS: the ROW controls are not Chrome-exact, so the COLUMN rows below are not measuring \
         the axis asymmetry this gate is named for — got {got:?}"
    );

    for (claim, why) in [
        (
            "col=50x80@0,0",
            "⭐ THE MECHANISM. `flex-shrink` is 1 by default, the container's main (block) size is a \
             definite 80px, and the item's 300px hypothetical main size leaves −220px of free space \
             — so the used height is 80. Reading 300 means taffy's verdict was computed and then \
             discarded on the way out.",
        ),
        (
            "colm=50x280@0,-100",
            "⭐ THE SAME RULE WITH A NEGATIVE MARGIN, which is what makes the number 280 rather than \
             80: the item's OUTER main size is 300 − 100 − 100 = 100, so the free space is −20 and \
             the shrink is 20. The position (−100) is unaffected either way, which is what \
             separates a sizing defect from a placement one.",
        ),
        (
            "pct=50x40@0,0",
            "REGRESSION ARM — the percentage case is the one that already worked and it must \
             survive. `height: 50%` of the 80px container is 40; resolving the percentage a second \
             time against the slot it produced squares it (t14's defect, on the other axis).",
        ),
        (
            "aut=50x30@0,0",
            "REGRESSION ARM — `auto` MUST STAY OUT. For an auto-height item taffy's slot is a \
             STRETCH verdict, not a resolution; adopting it freezes the item at its line's height \
             and this row would read 80 instead of the 30px its content asks for.",
        ),
        (
            "grd=50x300@0,0",
            "⭐⭐ THE ROW THAT KEPT A GUARD OUT OF THE TREE. A GRID item with a definite height \
             overflows a shorter track rather than being clamped to it — and taffy's slot height \
             here was measured directly at 300, not the 80px track, so the `parent_is_flex` scope \
             the first version of this fix carried was inert. One rule, both formatting contexts.",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_FLEX_COLUMN_SHRINKS_ITS_ITEM: expected `{claim}` (Chrome-measured) — {why}\n  got: {got}"
        );
    }
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// N1  the pre-tick condition (`Dim::Percent(_) | Dim::Calc { .. }` alone)
//       -> col=50x300 and colm=50x300; every control stays green, which is what identifies the
//          defect as the BLOCK axis of the condition rather than flex sizing at large.
// N2  widen the condition to include `Dim::Auto`
//       -> GREEN, and that is REPORTED rather than papered over. All seven rows are unchanged: in a
//          COLUMN container the height is the main axis, where an auto item's size is already its
//          content size. The `auto` case is the named residue in the header (s1/s6), not a hazard
//          this gate defends against — see `docs/wiki/flex-column-shrink.md`.
// N3  record the slot height for `Dim::Px(_)` ONLY (drop the percentage arm)
//       -> pct collapses, because the percentage is then resolved a second time against the slot.
// N4  scope the new arm to GRID parents instead of flex (`!parent_is_flex`)
//       -> col and colm read 50x300 again; grd is unmoved, which is the direct evidence that the
//          scope was inert in the direction it was written for.
