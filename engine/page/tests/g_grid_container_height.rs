//! # G_GRID_CONTAINER_HEIGHT — a grid container's height is its TRACKS, not its children's bottom edge
//!
//! `layout_flex_or_grid` returned the container's height as `max_h` — how far down the lowest child
//! reached — and threw away the height taffy had already resolved for the container itself. For
//! **flex** those two are the same number, and that is why it survived: a flex line's cross size
//! *is* its tallest item. For a **grid** they are different questions. A grid container's block size
//! is the sum of its resolved ROW TRACKS plus the row gaps, and **a track has a size whether or not
//! anything fills it**.
//!
//! Measured against headless Chrome (300px-wide grid, 60×40 items):
//!
//! ```text
//!                                                   Chrome     before      after
//!   grid-template-rows:100px, one 40px item           100         40        100
//!   grid-template-rows:20px,  one 40px item            20         40         20
//!   grid-template-rows:40px 100px, two items          140         80        140
//!   grid-template-rows:40px 70px, ONE item            110         40        110
//!   grid-template-rows:100px + padding:10px           120         60        120
//!   grid-template-rows:40px 40px; row-gap:30px        110        110      unchanged
//!  ── FLEX CONTROLS, which is the half that was always right ──
//!   flex row, tallest item 70px                        70         70      unchanged
//!   flex column, two 40px items                        80         80      unchanged
//!   flex, height:30px around a 40px item               30         30      unchanged
//! ```
//!
//! ## The row that decides the SHAPE of the fix
//!
//! `grid-template-rows: 20px` around a 40px item is **20** in Chrome: the container is *shorter than
//! its own content* and the item overflows. So this can never be `max(child_extent, tracks)` — a
//! combination that only ever grows would keep that case wrong in the direction that looks safe. The
//! answer has to be the formatting context's own, and taffy had computed it and been ignored.
//!
//! ## The row-gap row is the one that shows why this hid
//!
//! `row-gap:30px` between two 40px rows was **already correct at 110**, because the gap sits between
//! the children and the lowest child's bottom edge therefore includes it. Every grid whose tracks
//! are exactly as tall as their content — which is every grid that sizes its rows `auto`, the common
//! case — agreed with Chrome for the wrong reason. Only a grid with a track *bigger or smaller than
//! what fills it* can tell the two models apart, and a trailing EMPTY track (row Q) is the sharpest
//! version: there is no child down there at all, so the child-extent model cannot even see it.
//!
//! ## Where it was found
//!
//! Not by aiming at it. `G_GRID_IMPLICIT_TRACKS` (t982) got all eleven of its ITEM offsets exact and
//! two of its CONTAINERS stayed 40px short — an item-only readout would have declared that family
//! finished. **Measure the container after the items agree.**
//!
//! ## Blast radius, and what was checked
//!
//! Every flex and grid container on every page now takes its height from taffy rather than from its
//! children. The flex controls below are three of the four ways a flex container's height is
//! decided (row cross-size, column main-size, explicit height clamping an overflowing item). An
//! old-binary A/B on four anchor sites in the same hour moved mean shape 87.3% → 87.4% with all four
//! jarring invariants byte-identical — no regression, and no measurable gain either: this is
//! Chrome-exact on the fixture and invisible on those four pages.
//!
//! ## How this goes RED
//!
//! - **Return `max_h` instead of `solved_h`** (the original code) → `#k` reads 40 against 100, `#q`
//!   reads 40 against 110, and every flex control still passes — which is exactly the confinement
//!   that let this survive.
//! - **Return `max_h.max(solved_h)`** → every row above passes EXCEPT `#n`, which reads 40 against
//!   Chrome's 20. `#k` passes under this mistake, so `#n` is the only row in the gate that catches
//!   it, and it exists for that.
//!
//! ⚠ **A third recipe was tried and it does NOT go red, which is worth recording rather than
//! quietly dropping.** Swapping `content_box_height()` for taffy's border-box `size.height` changes
//! nothing — `TaffyDom::build` zeroes the ROOT's margin/padding/border/inset (Manuk applies the
//! container's own frame around the content origin it passes in), so the two are equal on the root
//! by construction, on every fixture. `content_box_height()` is kept as the defensive form: it stays
//! correct if that zeroing is ever removed, where `size.height` would silently double-count the
//! frame. The claim it makes is *"this is the content box"*, and that claim will still be true.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
html,body{margin:0;padding:0}
body{font:16px/1 monospace}
*{box-sizing:border-box}
.g{display:grid;width:300px;margin:0 0 6px 0}
.fx{display:flex;width:300px;margin:0 0 6px 0}
.it{width:60px;height:40px}
</style></head><body>
<div class="g" id="k" style="grid-template-columns:60px;grid-template-rows:100px"><div class="it"></div></div>
<div class="g" id="n" style="grid-template-columns:60px;grid-template-rows:20px"><div class="it"></div></div>
<div class="g" id="l" style="grid-template-columns:60px;grid-template-rows:40px 100px"><div class="it"></div><div class="it"></div></div>
<div class="g" id="o" style="grid-template-columns:60px;grid-template-rows:40px 40px;row-gap:30px"><div class="it"></div><div class="it"></div></div>
<div class="g" id="p" style="grid-template-columns:60px;grid-template-rows:100px;padding:10px"><div class="it"></div></div>
<div class="g" id="q" style="grid-template-columns:60px;grid-template-rows:40px 70px"><div class="it"></div></div>
<div class="fx" id="r"><div class="it"></div><div class="it" style="height:70px"></div></div>
<div class="fx" id="s" style="flex-direction:column"><div class="it"></div><div class="it"></div></div>
<div class="fx" id="t" style="height:30px"><div class="it"></div></div>
</body></html>"##;

fn rect_of(page: &manuk_page::Page, sel: &str) -> manuk_layout::Rect {
    let dom = page.dom();
    let n = manuk_css::query_selector_all(dom, dom.root(), sel)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("selector {sel} matched nothing"));
    *page
        .root_box
        .node_rects(dom)
        .get(&n)
        .unwrap_or_else(|| panic!("no box for {sel}"))
}

#[test]
fn g_grid_container_height() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://gch.test/", &fonts, 1200.0);
    let h = |sel: &str| rect_of(&page, sel).height;
    let near = |got: f32, want: f32| (got - want).abs() < 1.1;

    // ── DEFECT — the container takes the TRACKS. Each row is one way a track can differ from what
    //    fills it: bigger, smaller, a second bigger one, a padded frame, and an EMPTY trailing one.
    for (sel, want, why) in [
        (
            "#k",
            100.0,
            "a 100px row holding a 40px item — the track is TALLER than its content",
        ),
        (
            "#n",
            20.0,
            "a 20px row holding a 40px item — the track is SHORTER, and the container must SHRINK \
             below its own content rather than grow to contain it. This is the row that rules out \
             `max(child_extent, tracks)`",
        ),
        (
            "#l",
            140.0,
            "40px + 100px rows — the sum of the tracks, not the lowest child's bottom (80)",
        ),
        (
            "#p",
            120.0,
            "a 100px row inside 10px of padding — 120 border-box. 140 would mean the frame was \
             added to a height that already contained it",
        ),
        (
            "#q",
            110.0,
            "40px + 70px rows holding ONE item — the trailing track is EMPTY, so a model built on \
             where the children reach cannot see it at all",
        ),
    ] {
        assert!(
            near(h(sel), want),
            "G_GRID_CONTAINER_HEIGHT: {sel} must be {want} tall — {why}; got {}. A grid container's \
             block size is the sum of its resolved ROW TRACKS plus the row gaps, and a track has a \
             size whether or not anything fills it.",
            h(sel)
        );
    }

    // ── CONTROL A — the grid row that was ALREADY right, and for the wrong reason: the row-gap sits
    //    between the children, so the lowest child's bottom edge happens to include it. Every grid
    //    with `auto` rows agreed with Chrome this way, which is why the defect above survived.
    assert!(
        near(h("#o"), 110.0),
        "G_GRID_CONTAINER_HEIGHT: two 40px rows with a 30px row-gap is 110 tall, not {} — this row \
         was correct BEFORE the fix and must stay correct after it.",
        h("#o")
    );

    // ── CONTROL B — FLEX, which is the half that was always right and the half this change could
    //    break: the same code path now feeds every flex container's height too. Three of the four
    //    ways a flex container's height is decided.
    assert!(
        near(h("#r"), 70.0),
        "G_GRID_CONTAINER_HEIGHT: a flex ROW is as tall as its tallest item (70), not {} — the \
         cross size of its single line.",
        h("#r")
    );
    assert!(
        near(h("#s"), 80.0),
        "G_GRID_CONTAINER_HEIGHT: a flex COLUMN of two 40px items is 80, not {} — here the block \
         axis is the MAIN axis and the height is the sum, not the max.",
        h("#s")
    );
    assert!(
        near(h("#t"), 30.0),
        "G_GRID_CONTAINER_HEIGHT: an explicit `height:30px` wins over a 40px item that overflows it \
         — 30, not {}. The author's height is not a minimum.",
        h("#t")
    );
}
