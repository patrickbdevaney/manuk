//! # G_FLEX_ITEM_SLOT_IS_FINAL — taffy's slot is a finished answer, not an input
//!
//! A flex/grid item's box is decided by taffy: it resolves the item's `width`, applies its
//! `min-width`/`max-width` clamp **against the real containing block**, and positions the slot with
//! the item's margins already taken out of the line. `layout_block` then ran over the top of that
//! answer and did two of those three again, using the SLOT as the containing block:
//!
//! * the min/max-width clamp was re-applied — so `max-width: <pct>` resolved against the width it
//!   had itself just produced, and the used width came out the percentage **squared**;
//! * the margins were added to the slot position — so every margined flex item moved **twice** its
//!   margin, `px` and `%` alike.
//!
//! Tick ~700 already fixed the third (`width: <pct>` re-resolving against the slot) and left the
//! guard it introduced, `taffy_item_width`, covering only that one property.
//!
//! Chrome-measured on a 1200px `display:flex; flex-wrap:wrap` row and an `800px 400px` grid
//! (`[x y width]` of the item; the fixture below is the one that was measured, and no other):
//!
//! ```text
//!                                                  Chrome           before      after
//!   flex:0 0 90%; max-width:50%                  [   0 600]      300 wide     600  ✗→✓
//!   width:90%;    max-width:50%                  [   0 600]      300 wide     600  ✗→✓
//!   flex:0 0 66.666667%; max-width:66.666667%    [   0 800]      533 wide     800  ✗→✓  ← Bootstrap 4
//!   flex:0 0 33.333333%; max-width:33.333333%    [ 800 400]      133 wide     400  ✗→✓  ← Bootstrap 4
//!   flex:0 0 50%; margin-left:100px              [ 100 600]      x = 200      100  ✗→✓
//!   flex:0 0 50%; margin-left:10%                [ 120 600]      x = 180      120  ✗→✓
//!   grid item, 800px track, max-width:50%        [   0 400]      200 wide     400  ✗→✓
//!   grid item, 400px track, margin-left:10%      [ 840 360]      x = 876      840  ✗→✓
//!   flex:0 0 10%; min-width:300px                [   0 300]      300          300   ✓   ← guard
//!   flex:0 0 90%; max-width:300px                [   0 300]      300          300   ✓   ← guard
//!   flex:0 0 50%; padding-left:100px             [   0 700]      700          700   ✓   ← guard
//!   flex:0 0 50%; padding-left:10%               [   0 720]      720          720   ✓   ← guard
//!   plain block,  max-width:50%                  [   0 600]      600          600   ✓   ← control
//!   plain block,  margin-left:10%; width:50%     [ 120 600]      120          120   ✓   ← control
//! ```
//!
//! ## ⚠ Why the `px` rows are the whole reason this survived
//!
//! **A percentage clamp re-applied to the slot always binds again; a pixel one never does.**
//! `max-width: 300px` against an already-300px slot is a no-op, and `min-width: <pct>` of a slot can
//! never exceed that slot — so of the four min/max combinations, **the only one that is observably
//! wrong is `max-width: <pct>`**, and it is wrong by the percentage squared (50% of the 50% answer is
//! 25% of the container). The three quiet ones are asserted here precisely because they are what a
//! reader would check first and be reassured by.
//!
//! ## The reach, which is why this is a gate and not a note
//!
//! Bootstrap 4's grid column is literally `flex: 0 0 66.666667%; max-width: 66.666667%` — the
//! `max-width` is the column's *point*, capping a grown item at its share — and it rendered **533px
//! against Chrome's 800**. t817 and t819 both chased that 533 (through flex line-breaking, then
//! through `flex-basis`); t819 measured the basis on its own, found it correct, and named `max-width`
//! as the remaining suspect. This is the measurement that convicts it. The margin half is wider
//! still: it needs no percentage and no framework — one `margin-left` on one flex item was doubled.
//!
//! ## How this goes RED
//!
//! - **Delete the `!taffy_item` guard on the min/max clamp** in `layout_block` → `#x2` reads 300 and
//!   `#a1` reads 533. (Verified, not assumed.)
//! - **Restore `border_x = x + ml`** → `#m1` reads `x = 200`, `#q2` reads `x = 876`. (Verified.)
//!
//! ## The bound, stated rather than glossed
//!
//! ⚠ The clamp is skipped **wholesale** for a taffy item rather than re-resolved against the true
//! containing block. That is correct because taffy already resolved it against the correct reference
//! and a second pass can only be a no-op or wrong — but it does mean this path now *trusts* taffy's
//! min/max handling. The two `px` guard rows above are what hold that trust honest: if taffy ever
//! stopped applying `max-width`, `#x1` would go red here rather than silently pass because our own
//! second clamp was covering for it.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
body{margin:0;font:16px/1.25 sans-serif}
.r{display:flex;flex-wrap:wrap;width:1200px}
</style></head><body>
<div class="r"><div id="a1" style="flex:0 0 66.666667%;max-width:66.666667%">x</div><div id="a2" style="flex:0 0 33.333333%;max-width:33.333333%">x</div></div>
<div class="r"><div id="x2" style="flex:0 0 90%;max-width:50%">x</div></div>
<div class="r"><div id="x3" style="width:90%;max-width:50%">x</div></div>
<div class="r"><div id="x1" style="flex:0 0 90%;max-width:300px">x</div></div>
<div class="r"><div id="n1" style="flex:0 0 10%;min-width:300px">x</div></div>
<div class="r"><div id="m1" style="flex:0 0 50%;margin-left:100px">x</div></div>
<div class="r"><div id="m2" style="flex:0 0 50%;margin-left:10%">x</div></div>
<div class="r"><div id="p1" style="flex:0 0 50%;padding-left:100px">x</div></div>
<div class="r"><div id="p2" style="flex:0 0 50%;padding-left:10%">x</div></div>
<div style="display:grid;grid-template-columns:800px 400px;width:1200px"><div id="q1" style="max-width:50%">x</div><div id="q2" style="margin-left:10%">x</div></div>
<div style="width:1200px"><div id="z1" style="max-width:50%">x</div><div id="z2" style="margin-left:10%;width:50%">x</div></div>
</body></html>"##;

fn rect_of(page: &manuk_page::Page, sel: &str) -> [f32; 3] {
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
    [r.x, r.width, r.y]
}

/// Assert the item's inline-axis geometry — `x` and `width`, both Chrome-measured on THIS fixture.
/// The block axis is deliberately NOT asserted: our `sans-serif` resolves to a different face than
/// the reference Chrome's, so every row's `y` differs by an accumulating 2px of line height. That is
/// a font-metric residue, not this mechanism, and asserting it here would make the gate go red for a
/// reason it does not name.
fn assert_inline(page: &manuk_page::Page, sel: &str, x: f32, w: f32, why: &str) {
    let g = rect_of(page, sel);
    assert!(
        (g[0] - x).abs() < 1.01 && (g[1] - w).abs() < 1.01,
        "G_FLEX_ITEM_SLOT_IS_FINAL: `{sel}` expected x={x} width={w} (MEASURED in headless Chrome on \
         THIS fixture), got x={} width={}.\n  {why}",
        g[0],
        g[1]
    );
}

#[test]
fn g_flex_item_slot_is_final() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://flx.test/", &fonts, 1400.0);

    // ── THE CLAMP, APPLIED TWICE. A percentage of the slot the percentage produced.
    assert_inline(
        &page,
        "#x2",
        0.0,
        600.0,
        "`max-width:50%` on a flex item resolves against the FLEX CONTAINER's 1200px content box, not \
         against the 600px slot taffy already clamped it into. Re-clamping gave 50% of 50% = 300px — \
         the percentage squared",
    );
    assert_inline(
        &page,
        "#x3",
        0.0,
        600.0,
        "the same defect reached through `width:<pct>` instead of `flex-basis`, asserted separately so \
         a change to the flex-basis path cannot silently take both rows",
    );
    assert_inline(
        &page,
        "#a1",
        0.0,
        800.0,
        "Bootstrap 4's `.col-8`: `flex: 0 0 66.666667%; max-width: 66.666667%`. It came out 533px \
         (800 x 0.666667) on every Bootstrap-4 page on the web. t817 and t819 both chased this number \
         into the wrong organ before it was measured with the max-width removed",
    );
    assert_inline(
        &page,
        "#a2",
        800.0,
        400.0,
        "its `.col-4` sibling — 133px before. The x was ALREADY right (t819 snapped the flex-basis to \
         Chrome's 1/64px grid so the pair stopped stacking), which is what left the width defect \
         looking like it had been fixed",
    );
    assert_inline(
        &page,
        "#q1",
        0.0,
        400.0,
        "a GRID item's containing block is its grid AREA, so `max-width:50%` of an 800px track is 400 \
         — Chrome-measured. Ours re-clamped the 400px slot to 200. Grid is in this gate because the \
         mechanism is the slot, not the formatting context",
    );

    // ── THE MARGIN, APPLIED TWICE. No percentage and no framework needed.
    assert_inline(
        &page,
        "#m1",
        100.0,
        600.0,
        "taffy positions the slot with the item's margin already taken out of the line; adding `ml` to \
         that position again put the item at 200. A PIXEL margin, so this half was never about \
         percentage resolution at all",
    );
    assert_inline(
        &page,
        "#m2",
        120.0,
        600.0,
        "the percentage twin: 10% of the 1200px container is 120, and 120 + 10%-of-the-600px-slot was \
         180",
    );
    assert_inline(
        &page,
        "#q2",
        840.0,
        360.0,
        "the same doubling on a grid item — 800px track offset + 10% of the 400px track = 840, and we \
         added another 36",
    );

    // ── THE GUARDS. These were green BEFORE and must stay green: they are what says the fix removed a
    //    double application rather than removing the constraint.
    assert_inline(
        &page,
        "#x1",
        0.0,
        300.0,
        "`max-width:300px` still clamps a 90% (1080px) basis down to 300. A PIXEL clamp re-applied to \
         the slot was always a no-op, so this row passed through the whole defect — and it is now the \
         assertion that taffy is really doing the clamping we stopped duplicating",
    );
    assert_inline(
        &page,
        "#n1",
        0.0,
        300.0,
        "`min-width:300px` still pushes a 10% (120px) basis up to 300. `min-width:<pct>` could never be \
         observed as wrong (a percentage of the slot cannot exceed the slot), which is why max-width \
         was the only one of the four combinations that showed",
    );
    assert_inline(
        &page,
        "#p1",
        0.0,
        700.0,
        "PADDING is not doubled and must not become so: taffy's slot is a BORDER box, so `layout_block` \
         subtracting the item's own padding from it is correct and stays",
    );
    assert_inline(
        &page,
        "#p2",
        0.0,
        720.0,
        "the percentage twin — `padding-left:10%` is 120px of the 1200px container, and the 600px \
         content box plus that padding is a 720px border box. Correct before and after",
    );

    // ── THE CONTROLS. Ordinary blocks share every line of code this touched and must not move.
    assert_inline(
        &page,
        "#z1",
        0.0,
        600.0,
        "a plain block's `max-width:50%` resolves against its containing block and IS clamped here — \
         this is the path the `taffy_item` guard must leave alone",
    );
    assert_inline(
        &page,
        "#z2",
        120.0,
        600.0,
        "a plain block's `margin-left:10%` is applied HERE and nowhere else, so it must still move the \
         box. If this reads 0 the guard has been widened past flex/grid items",
    );
}
