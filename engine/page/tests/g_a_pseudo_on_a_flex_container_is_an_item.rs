//! # G_A_PSEUDO_ON_A_FLEX_CONTAINER_IS_AN_ITEM
//!
//! ⚠⚠⚠ **A `::before`/`::after` ON A FLEX OR GRID CONTAINER IS A FLEX/GRID ITEM, AND OURS VANISHED
//! ENTIRELY.** Generated content boxes are boxes: an in-flow one whose parent is a flex or grid
//! container is an ITEM of that container (CSS Flexbox §4, Grid §6), blockified like any other. Ours
//! were materialised only into the INLINE stream (`Ctx::push_pseudo`), and a flex container never
//! builds one — so the box was never generated at all and **every real item slid into the slot the
//! pseudo should have occupied.**
//!
//! Priced before it was built: of 25 CrUX-corpus sites that a Chrome probe could measure, **6 have
//! at least one generated box on a flex or grid container** (agoda 14, otomoto 14, repubblica 13,
//! paypal 7, marktplaats 4). On `whatwg.org` it is the anchor's last four misplaced elements —
//! shape **0.892 → 1.000**.
//!
//! ⭐⭐ **AND THE ITEM'S TEXT IS TRIMMED AT BOTH EDGES, WHICH IS WHERE THE FIRST CUT REGRESSED.** A
//! flex item is a block container, so collapsible white space at the start and end of its content is
//! removed (CSS Text §4.1.1). `pseudo_content` deliberately KEEPS those outer spaces, because in the
//! inline stream they are the break opportunities either side of a `content: " | "` separator — here
//! there is no neighbour to break against. `content: " "` is **Bootstrap's clearfix**, it sits on
//! flex containers all over the real web, and billing its single space as a 5px item made
//! `marktplaats.nl`'s header nav 455px wide against Chrome's 450.
//!
//! Every number below is CAPTURED from `google-chrome --headless=new --hide-scrollbars
//! --window-size=1200,800` on this exact fixture; `body{font:16px/19px monospace}`, containers
//! 400px. The column is the first real child's x **relative to its container** — i.e. how much room
//! the generated item took.
//!
//! ```text
//!                                                              Chrome   before   after
//!   #c1  ::before{content:"XY";width:50px;display:inline-block}  50.00      0      50    KEY
//!   #c2  ::before{content:"AB"}          text sizes the item     19.27      0      19    KEY
//!   #f3  ::after on an inline-flex — the CONTAINER's width       38.55   9.64      39    KEY
//!   #c4  a GRID: ::before{content:""} consumes a whole CELL     100.00      0     100    KEY
//!   #c5  ::before{content:"Q";padding:0 5px;border-left:3px}     22.64      0      23    KEY
//!   #c6  ::before{content:"AB";order:5}  its OWN order          <0.01      0       0    KEY
//!   #c7  ::before{content:"AB";flex:1}   it GROWS              390.36      0     390    KEY
//!   #c8  ::before{content:" ";display:table}  the CLEARFIX       <0.01      0       0    KEY
//!   #c9  ::before{content:"  x  "}  edges trimmed, "x" kept       9.64      0      10    KEY
//!   #c10 no pseudo at all                                       <0.01      0       0    CTRL
//!   #c11 ::before{content:"AB";position:absolute}  NOT an item  <0.01      0       0    CTRL
//!   #c12 ::before{content:"AB";display:none}       NOT an item  <0.01      0       0    CTRL
//!   #c13 ::before{content:" ";white-space:pre}   space SURVIVES   9.64      0      10    CTRL
//!   #c14 ::before{content:" ";width:20px;display:block}          20.00      0      20    CTRL
//! ```
//!
//! ⭐ **`#c13` IS WHY THE TRIM IS NOT "DROP WHITE-SPACE-ONLY GENERATED CONTENT".** Under
//! `white-space: pre` the same single space is a rendered character and Chrome bills it — the test
//! is `pseudo_content`'s own collapsing condition, so the two halves of one rule cannot drift.
//! `#c14` is the other edge: an empty generated box with a `width` is still a box worth its width
//! (t1375), and the trim must not delete the box, only its text.
//!
//! ⭐ **`#c11` AND `#c12` ARE WHY THIS IS NOT "EVERY PSEUDO IS AN ITEM".** An out-of-flow generated
//! box takes no slot — `content:""; position:absolute` is the decorative-underline idiom every
//! design system writes — and a `display:none` one generates nothing at all.
use manuk_text::FontContext;

const HTML: &str = r##"<!DOCTYPE html><html><head><meta charset="utf-8"><style>
 body{margin:0;font:16px/19px monospace}
 .f{display:flex;width:400px;background:#eee}
 .g{display:grid;grid-template-columns:repeat(3,100px);width:400px;background:#eee}
 .f>div,.g>div{background:#cfc}
 #f1::before{content:"XY";width:50px;display:inline-block}
 #f2::before{content:"AB"}
 #f3{display:inline-flex;width:auto}
 #f3::after{content:"ZZZ"}
 #f4::before{content:""}
 #f5::before{content:"Q";padding:0 5px;border-left:3px solid #000}
 #f6::before{content:"AB";order:5}
 #f7::before{content:"AB";flex:1}
 #f8::before{content:" ";display:table}
 #f9::before{content:"  x  "}
 #f11::before{content:"AB";position:absolute}
 #f12::before{content:"AB";display:none}
 #f13::before{content:" ";white-space:pre}
 #f14::before{content:" ";width:20px;display:block}
</style></head><body>
<div class=f id=f1><div id=c1>A</div></div>
<div class=f id=f2><div id=c2>A</div></div>
<div class=f id=f3><div id=c3>A</div></div>
<div class=g id=f4><div id=c4>A</div></div>
<div class=f id=f5><div id=c5>A</div></div>
<div class=f id=f6><div id=c6>A</div></div>
<div class=f id=f7><div id=c7>A</div></div>
<div class=f id=f8><div id=c8>A</div></div>
<div class=f id=f9><div id=c9>A</div></div>
<div class=f id=f10><div id=c10>A</div></div>
<div class=f id=f11><div id=c11>A</div></div>
<div class=f id=f12><div id=c12>A</div></div>
<div class=f id=f13><div id=c13>A</div></div>
<div class=f id=f14><div id=c14>A</div></div>
</body></html>
"##;

fn pick(dom: &manuk_dom::Dom, sel: &str) -> manuk_dom::NodeId {
    manuk_css::query_selector_all(dom, dom.root(), sel)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("selector {sel} matched nothing"))
}

/// How much room the generated item took: the first real child's x **relative to its container**.
fn indent(page: &manuk_page::Page, n: u32) -> f32 {
    let dom = page.dom();
    let rects = page.root_box.node_rects(dom);
    let child = rects
        .get(&pick(dom, &format!("#c{n}")))
        .copied()
        .unwrap_or_else(|| panic!("no box for #c{n}"));
    let container = rects
        .get(&pick(dom, &format!("#f{n}")))
        .copied()
        .unwrap_or_else(|| panic!("no box for #f{n}"));
    child.x - container.x
}

fn at(page: &manuk_page::Page, n: u32, want: f32, why: &str) {
    let got = indent(page, n);
    assert!(
        (got - want).abs() < 1.01,
        "G_A_PSEUDO_ON_A_FLEX_CONTAINER_IS_AN_ITEM: `#c{n}` expected x={want} relative to \
         `#f{n}` (CAPTURED from `google-chrome --headless=new --hide-scrollbars \
         --window-size=1200,800`), got x={got} — {why}"
    );
}

#[test]
fn g_a_pseudo_on_a_flex_container_is_an_item() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://flexpseudo.test/", &fonts, 1200.0);

    at(
        &page,
        1,
        50.0,
        "a sized `::before` on a flex container is the FIRST ITEM and holds its own 50px",
    );
    at(
        &page,
        2,
        19.27,
        "a text-only `::before` is sized by its text — two monospace chars at 16px",
    );
    at(
        &page,
        4,
        100.0,
        "in a GRID an empty `::before` still consumes a whole CELL, so the first real item is in \
         column 2",
    );
    at(
        &page,
        5,
        22.64,
        "the generated item's own padding and border count: 3 + 5 + 9.64 + 5",
    );
    at(
        &page,
        6,
        0.0,
        "`order` on the pseudo is the PSEUDO's own, not the owner's — order:5 sends it last",
    );
    at(
        &page,
        7,
        390.36,
        "`flex:1` on the pseudo makes it GROW into the free space like any other item",
    );
    at(
        &page,
        8,
        0.0,
        "⭐ Bootstrap's CLEARFIX — `content:\" \"; display:table` is a block box whose single \
         collapsible space is trimmed away, so it is worth ZERO",
    );
    at(
        &page,
        9,
        9.64,
        "⭐ only the EDGES are trimmed: `content:\"  x  \"` is worth exactly its `x`",
    );
    at(
        &page,
        10,
        0.0,
        "⭐ CONTROL: no pseudo at all — nothing may appear from nowhere",
    );
    at(
        &page,
        11,
        0.0,
        "⭐ CONTROL: an OUT-OF-FLOW generated box is not an item and takes no slot",
    );
    at(
        &page,
        12,
        0.0,
        "⭐ CONTROL: `display:none` generates no box at all",
    );
    at(
        &page,
        13,
        9.64,
        "⭐ CONTROL: under `white-space:pre` the same single space is a RENDERED character — the \
         trim must not reach it",
    );
    at(
        &page,
        14,
        20.0,
        "⭐ CONTROL: an empty generated box with a `width` is still a box worth its width (t1375) — \
         the trim removes text, never the box",
    );

    // `::after` adds to the END, so it cannot be seen in a child's indent — it is the shrink-to-fit
    // CONTAINER that grows. This is the row that proves the `after` half is wired at all.
    let dom = page.dom();
    let rects = page.root_box.node_rects(dom);
    let w = rects
        .get(&pick(dom, "#f3"))
        .copied()
        .expect("no box for #f3")
        .width;
    assert!(
        (w - 38.55).abs() < 1.01,
        "G_A_PSEUDO_ON_A_FLEX_CONTAINER_IS_AN_ITEM: `#f3` (inline-flex, one 9.64px child plus an \
         `::after{{content:\"ZZZ\"}}`) expected width=38.55 (CAPTURED from Chrome), got {w} — the \
         `::after` half must be an item too"
    );
}
