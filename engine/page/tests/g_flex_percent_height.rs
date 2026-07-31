//! # G_FLEX_PERCENT_HEIGHT — a percentage height on a flex/grid item is resolved ONCE
//!
//! `layout_flex` hands each item its taffy slot as the parent's definite height, and `own_definite_h`
//! then resolved the item's own `height: 50%` against **that** — so the percentage was applied twice
//! and the used height came out squared.
//!
//! ```text
//!   height:50% item in a height:200px flex ROW      Chrome 100   ours 50    (0.5² × 200)
//!   height:50% item in a height:300px flex ROW      Chrome 150   ours 75
//!   height:25% item in a height:200px flex ROW      Chrome  50   ours 13    (0.25² × 200)
//!   height:50% item in a height:200px flex COLUMN   Chrome 100   ours 50
//!   height:50% child of a height:200px BLOCK        Chrome 100   ours 100   ✓ always right
//! ```
//!
//! ⚠ **This is the same defect the WIDTH axis had and fixed at tick 14** — *"a percentage width on a
//! flex item resolved twice; used width came out squared"*. `taffy_item_width` exists for exactly
//! that, with a comment explaining it. One axis was corrected and the mirror was left standing, which
//! is this project's most-repeated shape: **the forgotten copy is never the main path, it is the
//! other axis.**
//!
//! ## What must NOT change, and is asserted here
//!
//! An `auto`-height item still ADOPTS its slot (the flex line stretches it) — `#s1`: a `height:auto` item
//! in a 200px row is 200 tall, not its content's height. And an auto item whose content OVERFLOWS its
//! container keeps the content height (`#ov`, 77 in a 60px column). Both are Chrome-measured and both
//! pass with or without the `pct_h` guard — see the RED list.
//!
//! ## How this goes RED
//!
//! - **Stop recording the slot height** → the four percentage cases square again.
//! - **Record it for `auto` items too** → ⚠ **NOTHING HERE FAILS, and that is stated rather than
//!   papered over.** Every case in this fixture — including `#s1`, `#s2` and `#ov`, which were added
//!   specifically to try to break it — passes with the guard removed, because the post-layout
//!   `height == Dim::Auto` adoption already takes `max(slot, content)`. So the `pct_h` guard in
//!   `layout_flex` is a CONSERVATISM, not a proven necessity: it keeps taffy's verdict where it is a
//!   resolution and declines it where it is a stretch decision, which is the narrower claim. If a
//!   case is ever found that distinguishes them, it belongs here.
//! - **Subtract nothing for padding/border** → `#p1` (a padded percentage item) is 20px too tall.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
body{margin:0;font:16px/1.2 sans-serif}
.row{display:flex;width:400px}
#r1{height:200px}#r2{height:300px}#r3{height:200px}#r4{height:200px}
#x1{height:50%;width:50px}
#x2{height:50%;width:50px}
#x3{height:25%;width:80px}
#s1{height:auto;width:50px}
#col{display:flex;flex-direction:column;height:200px;width:100px}
#short{display:flex;flex-direction:column;height:60px;width:200px}
#ov{height:auto;flex:0 0 auto}
#y1{height:50%}
#s2{height:auto}
#blk{height:200px;width:400px}
#z1{height:50%;width:50px}
#p1{height:50%;width:50px;padding:10px;box-sizing:border-box}
</style></head><body>
<div class="row" id="r1"><div id="x1"></div><div id="s1"></div></div>
<div class="row" id="r2"><div id="x2"></div></div>
<div class="row" id="r3"><div id="x3"></div></div>
<div class="row" id="r4"><div id="p1"></div></div>
<div id="col"><div id="y1"></div><div id="s2">a</div></div>
<div id="short"><div id="ov">one<br>two<br>three<br>four</div></div>
<div id="blk"><div id="z1"></div></div>
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

fn assert_h(page: &manuk_page::Page, sel: &str, want: f32, why: &str) {
    let got = h_of(page, sel);
    assert!(
        (got - want).abs() < 1.01,
        "G_FLEX_PERCENT_HEIGHT: `{sel}` expected height {want} (MEASURED in Chrome), got {got}.\n  {why}"
    );
}

#[test]
fn g_flex_percent_height() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://pct.test/", &fonts, 1200.0);

    // ── THE BUG: one resolution, not two. Three ratios and two container heights, because a squared
    // percentage and a correct one agree at 100% and nowhere else.
    assert_h(
        &page,
        "#x1",
        100.0,
        "50% of a 200px flex row — not 0.5² × 200 = 50",
    );
    assert_h(
        &page,
        "#x2",
        150.0,
        "…the same ratio against a 300px row, so the basis is the row",
    );
    assert_h(
        &page,
        "#x3",
        50.0,
        "25% of 200 is 50; squared it is 13, which is what this read before",
    );
    assert_h(
        &page,
        "#y1",
        100.0,
        "a COLUMN container resolves the same way",
    );
    assert_h(
        &page,
        "#z1",
        100.0,
        "the plain BLOCK case, which was always right and must stay so",
    );
    assert_h(
        &page,
        "#p1",
        100.0,
        "`box-sizing:border-box` with padding: taffy's slot is a border box, so the content height \
         is the slot less this box's own padding and border — not the slot with the box-sizing \
         adjustment applied a second time",
    );

    // ── THE GUARD: an `auto`-height item still adopts its slot. Taffy's verdict is a RESOLUTION only
    // where the item asked for a percentage; for `auto` it is a stretch decision.
    assert_h(
        &page,
        "#s1",
        200.0,
        "a `height:auto` item still STRETCHES to its flex line — recording taffy's slot for auto \
         items too would look like a simplification and would freeze this at its content height",
    );
    assert!(
        h_of(&page, "#s2") > 0.0,
        "G_FLEX_PERCENT_HEIGHT: an auto-height item in a COLUMN container must still size to its \
         content — got {}",
        h_of(&page, "#s2")
    );
    // ⚠ An auto item whose CONTENT is taller than its container keeps its content height and
    // overflows — measured in Chrome, 77px of text inside a 60px column container. This was added to
    // try to make the `pct_h` guard fail when removed, and it does NOT: the post-layout adoption
    // takes `max(slot, content)` either way. It stays because it pins real Chrome behaviour that
    // nothing else here covers.
    assert_h(
        &page,
        "#ov",
        77.0,
        "an auto-height item whose content is TALLER than its column container keeps its content \
         height and overflows — clamping it to the slot is what 'record the slot for every item' \
         would do, and nothing else in this gate can tell the two apart",
    );
}
