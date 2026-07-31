//! # G_FLOAT_CONTAINING_BLOCK — a float belongs to its own block, not to the viewport
//!
//! A float participates in its nearest **block formatting context** — that is why exclusion bands are
//! shared across nested plain blocks, and it must stay that way. But CSS 2.1 §9.5.1 rules 1 and 2 pin
//! the float to *its own containing block*: "the left outer edge of a left-floating box may not be to
//! the left of the left edge of its containing block", and the mirror for right.
//!
//! We conflated the two. Every `float: right` inside a narrow block was placed against the **viewport**
//! edge:
//!
//! ```text
//!   <div style="width:300px"><div style="float:right;width:50px"></div></div>
//!       Chrome x = 250        ours x = 1150        (a 1200px viewport)
//! ```
//!
//! 900px, on the single most common legacy layout primitive there is. A miss that size is never one
//! wrong box either: it spawns overlap and reading-order violations across everything the float was
//! meant to sit beside. `en.wikipedia.org` — floated infoboxes and thumbnails on every article —
//! went shape **53.8% → 58.8%** on this one change.
//!
//! ## Every number here is MEASURED (`--dump-dom` + `getBoundingClientRect`)
//!
//! ```text
//!   a1  float:right in a 300px block                 x = 250
//!   a2  float:left  in the same block                x =   0
//!   b1  float:right in a 150px block, margin-left 20 x = 120
//!   c1  float:right in a 300px block that is a BFC   x = 250
//!   d1  float:right 400px wide in a 300px block      x = -100   ← overflows LEFT
//!   e1  first of two right floats                    x = 250
//!   e2  second, stacking inward                      x = 200
//! ```
//!
//! ⚠ **`d1` is why "clamp the float inside its block" is the wrong rule, and it was measured, not
//! reasoned.** The first draft of this fix clamped a right float to the containing block's LEFT edge
//! too — a box should not start outside its own block, surely. Chrome puts it at **-100**: the right
//! edge stays pinned and the box overflows to the left. Clamping made that case read 0, which would
//! have traded a 900px error for a 100px one and called it a fix. **Only the hugged edge is clamped.**
//!
//! ## How this goes RED
//!
//! - **Drop the `cb_right` clamp** → `a1`, `b1`, `c1`, `e1`, `e2` all fly to the viewport edge.
//! - **Re-add the `cb_left` clamp** → only `d1` fails, at 0 instead of -100.
//! - **Clamp against the float CONTEXT's edges instead of the containing block's** → `b1` fails
//!   (150px block inside a 1200px BFC) while `a1` passes, because `a1`'s block happens to start at 0.
//!
//! ## Residue, measured in the same fixture and NOT fixed here
//!
//! A BFC root must not overlap preceding floats — Chrome moves `c1`'s `overflow:hidden` parent down
//! to `y=20`, clear of the floats above it, and we leave it at `y=0`. That is why this gate asserts
//! **x only**: the y column carries a second, independent defect, and a gate that asserted both would
//! be red for a reason it does not name.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
body{margin:0;font:16px/1.2 sans-serif}
.w{width:300px}
.n{width:150px;margin-left:20px}
.bfc{width:300px;overflow:hidden}
.fr{float:right;width:50px;height:20px}
.fl{float:left;width:50px;height:20px}
.wide{float:right;width:400px;height:10px}
</style></head><body>
<div class="w"><div class="fr" id="a1"></div><div class="fl" id="a2"></div></div>
<div class="n"><div class="fr" id="b1"></div></div>
<div class="bfc"><div class="fr" id="c1"></div></div>
<div class="w"><div class="wide" id="d1"></div></div>
<div class="w"><div class="fr" id="e1"></div><div class="fr" id="e2"></div></div>
</body></html>"##;

fn x_of(page: &manuk_page::Page, sel: &str) -> f32 {
    let dom = page.dom();
    let n = manuk_css::query_selector_all(dom, dom.root(), sel)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("selector {sel} matched nothing"));
    page.root_box
        .node_rects(dom)
        .get(&n)
        .unwrap_or_else(|| panic!("no box for {sel}"))
        .x
}

fn assert_x(page: &manuk_page::Page, sel: &str, want: f32, why: &str) {
    let got = x_of(page, sel);
    assert!(
        (got - want).abs() < 1.01,
        "G_FLOAT_CONTAINING_BLOCK: `{sel}` expected x={want} (MEASURED in Chrome), got {got}.\n  {why}"
    );
}

#[test]
fn g_float_containing_block() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://floats.test/", &fonts, 1200.0);

    assert_x(
        &page,
        "#a1",
        250.0,
        "a `float:right` hugs the right edge of ITS OWN 300px block, not the 1200px viewport. This \
         read 1150 before t792",
    );
    assert_x(
        &page,
        "#a2",
        0.0,
        "…and the left float in the same block is unmoved — the bug was one-sided, so a gate \
         asserting only right floats could not tell a fix from a rewrite",
    );
    assert_x(
        &page,
        "#b1",
        120.0,
        "a 150px block offset 20px from the left: the right edge is 170, so the float lands at 120. \
         Clamping to the float CONTEXT's edges instead of the containing block's passes #a1 and \
         fails here, which is why this case exists",
    );
    assert_x(
        &page,
        "#c1",
        250.0,
        "the same block establishing its own BFC — the clamp must not depend on which context owns \
         the exclusion bands",
    );
    assert_x(
        &page,
        "#d1",
        -100.0,
        "a 400px right float in a 300px block OVERFLOWS TO THE LEFT: its right edge stays pinned to \
         the containing block. Clamping the far edge too reads 0 here, which trades a 900px error \
         for a 100px one",
    );
    assert_x(
        &page,
        "#e1",
        250.0,
        "two right floats: the first hugs the edge",
    );
    assert_x(
        &page,
        "#e2",
        200.0,
        "…and the second stacks inward against it, so the clamp did not break the exclusion bands \
         it shares with its neighbours",
    );
}
