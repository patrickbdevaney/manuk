//! # G_CONTAINER_ALIGNMENT — `justify-content`/`align-items` reached taffy and `align-content`/`justify-items` did not
//!
//! One level up from [`g_self_alignment`](../g_self_alignment.rs) (t980) and the same shape, in both
//! directions at once. Of the four container-level Box-Alignment longhands, **two existed and two did
//! not** — and the two that did are precisely the ones that make the family read as covered:
//!
//! ```text
//!                    INLINE axis                BLOCK / CROSS axis
//!   distribution     justify-content  ✓ built   align-content    ✗ ABSENT
//!   item default     justify-items    ✗ ABSENT  align-items      ✓ built
//! ```
//!
//! Measured against headless Chrome on a 300×200 container holding 60×40 items (`/tmp/ac.html`,
//! every row an offset from its own container):
//!
//! ```text
//!                                                     Chrome        before        after
//!    flex-wrap, align-content:flex-start   last line [  0,  40]   [  0, 100]   [  0,  40]
//!    flex-wrap, align-content:center      first line [  0,  60]   [  0,   0]   [  0,  60]
//!    flex-wrap, align-content:flex-end    first line [  0, 120]   [  0,   0]   [  0, 120]
//!    flex-wrap, align-content:space-between last     [  0, 160]   [  0, 100]   [  0, 160]
//!    grid,      align-content:center       first row [  0,  60]   [  0,   0]   [  0,  60]
//!    grid,      align-content:end          first row [  0, 120]   [  0,   0]   [  0, 120]
//!    grid,      align-content:space-between last row [  0, 160]   [  0,  40]   [  0, 160]
//!    grid,      justify-items:end                    [140,   0]   [  0,   0]   [140,   0]
//!    grid,      justify-items:center                 [ 70,   0]   [  0,   0]   [ 70,   0]
//!    grid,      place-content:center                 [120,  60]   [120,   0]   [120,  60]
//!    grid,      place-items:end end                  [140,  60]   [  0,  60]   [140,  60]
//!   ── the twins that were already right, and are the CONTROLS ──
//!    flex-wrap, no align-content (stretch) last line [  0, 100]      same      unchanged
//!    flex-wrap, justify-content:center       last x  [120, 100]      same      unchanged
//!    grid,      justify-content:center               [120,   0]      same      unchanged
//!    grid,      align-items:end                      [  0,  60]      same      unchanged
//! ```
//!
//! **The `place-*` rows are the sharpest evidence that this was an absence and not a mis-mapping.**
//! `place-content: center` and `place-items: end end` each set both axes in one declaration, and
//! before this change each of them landed **exactly half**: the shorthand was expanded correctly by
//! Stylo, one longhand was consumed and the other was dropped on the floor because there was no
//! field to put it in. A property that arrives and is discarded is indistinguishable from a property
//! that never parsed — until something measures the axis that vanished.
//!
//! ## Where it lived
//!
//! Absent at all three layers, exactly as `justify-self` was: no field on `ComputedStyle`, no parse
//! arm in the minimal cascade, no `stylo_map` line, and in `taffy_tree.rs` two lines with no
//! partners. The `Normal → None` mapping is why the *initial* value looked right the whole time —
//! taffy's own default on the block axis is stretch, which is what `normal` means there — so only
//! **declared** values were ever wrong.
//!
//! ## Which cascade this proves
//!
//! The **Stylo** one, which is the shipping cascade (`live-cascade-is-stylo-not-minimal`). The
//! minimal-cascade arms added in the same change are the JS-less/headless fallback and are covered
//! by `manuk-css`'s own unit tests, not by this gate — recorded here because t976 found a RED proof
//! that could not fire for exactly this reason.
//!
//! ## How this goes RED
//!
//! - **Drop the `align_content` line from `taffy_tree.rs`** → every `align-content` row collapses to
//!   the container's start (`#a4f` reads 0 against Chrome's 120) while all four control rows hold.
//! - **Drop the `justify_items` line** → `#c2i` reads x=0 against Chrome's 140.
//! - **Map `align-content: normal` to `FlexStart` instead of `Normal`** → `#a1l` (no declaration at
//!   all) moves from 100 to 40, because taffy stops stretching the lines. That row exists to pin the
//!   initial value, which is the one this property gets right by accident.
//! - **Swap the two halves of `place-content`** (justify first) → `#e1f` reads [0, 60] instead of
//!   [120, 60]; the shorthand rows are asserted on BOTH axes so an order slip cannot hide.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
html,body{margin:0;padding:0}
body{font:16px/1 monospace}
.box{width:300px;height:200px;margin:0 0 10px 0}
.it{width:60px;height:40px}
.f{display:flex;flex-wrap:wrap}
.g{display:grid}
</style></head><body>
<div class="box f" id="a1"><div class="it" id="a1f"></div><div class="it"></div><div class="it"></div><div class="it"></div><div class="it"></div><div class="it" id="a1l"></div></div>
<div class="box f" id="a2" style="align-content:flex-start"><div class="it" id="a2f"></div><div class="it"></div><div class="it"></div><div class="it"></div><div class="it"></div><div class="it" id="a2l"></div></div>
<div class="box f" id="a3" style="align-content:center"><div class="it" id="a3f"></div><div class="it"></div><div class="it"></div><div class="it"></div><div class="it"></div><div class="it" id="a3l"></div></div>
<div class="box f" id="a4" style="align-content:flex-end"><div class="it" id="a4f"></div><div class="it"></div><div class="it"></div><div class="it"></div><div class="it"></div><div class="it" id="a4l"></div></div>
<div class="box f" id="a5" style="align-content:space-between"><div class="it" id="a5f"></div><div class="it"></div><div class="it"></div><div class="it"></div><div class="it"></div><div class="it" id="a5l"></div></div>
<div class="box f" id="a6" style="justify-content:center"><div class="it" id="a6f"></div><div class="it"></div><div class="it"></div><div class="it"></div><div class="it"></div><div class="it" id="a6l"></div></div>
<div class="box g" id="b1" style="grid-template-rows:40px 40px;grid-template-columns:60px"><div class="it" id="b1f"></div><div class="it" id="b1l"></div></div>
<div class="box g" id="b2" style="grid-template-rows:40px 40px;grid-template-columns:60px;align-content:center"><div class="it" id="b2f"></div><div class="it" id="b2l"></div></div>
<div class="box g" id="b3" style="grid-template-rows:40px 40px;grid-template-columns:60px;align-content:end"><div class="it" id="b3f"></div><div class="it" id="b3l"></div></div>
<div class="box g" id="b4" style="grid-template-rows:40px 40px;grid-template-columns:60px;align-content:space-between"><div class="it" id="b4f"></div><div class="it" id="b4l"></div></div>
<div class="box g" id="b5" style="grid-template-rows:40px 40px;grid-template-columns:60px;justify-content:center"><div class="it" id="b5f"></div><div class="it" id="b5l"></div></div>
<div class="box g" id="c1" style="grid-template-rows:100px;grid-template-columns:200px"><div class="it" id="c1i"></div></div>
<div class="box g" id="c2" style="grid-template-rows:100px;grid-template-columns:200px;justify-items:end"><div class="it" id="c2i"></div></div>
<div class="box g" id="c3" style="grid-template-rows:100px;grid-template-columns:200px;justify-items:center"><div class="it" id="c3i"></div></div>
<div class="box g" id="c4" style="grid-template-rows:100px;grid-template-columns:200px;align-items:end"><div class="it" id="c4i"></div></div>
<div class="box g" id="e1" style="grid-template-rows:40px 40px;grid-template-columns:60px;place-content:center"><div class="it" id="e1f"></div><div class="it" id="e1l"></div></div>
<div class="box g" id="e2" style="grid-template-rows:100px;grid-template-columns:200px;place-items:end end"><div class="it" id="e2i"></div></div>
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
fn g_container_alignment() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://ca.test/", &fonts, 1200.0);
    let r = |sel: &str| rect_of(&page, sel);
    // Every row is its own container, and every assertion is the item's offset from THAT container,
    // so a row going wrong above cannot shift a row below into or out of agreement.
    let dy = |sel: &str, w: &str| r(sel).y - r(w).y;
    let dx = |sel: &str, w: &str| r(sel).x - r(w).x;
    let near = |got: f32, want: f32| (got - want).abs() < 1.1;

    // ── DEFECT 1 — `align-content` in a WRAPPED FLEX container. Six 60×40 items in a 300×200 box
    //    wrap to two lines; the lines are 80px of content in 200px of container, so the property has
    //    120px of free space to place and every value puts it somewhere different.
    for (first, last, fy, ly, val) in [
        ("#a2f", "#a2l", 0.0, 40.0, "flex-start"),
        ("#a3f", "#a3l", 60.0, 100.0, "center"),
        ("#a4f", "#a4l", 120.0, 160.0, "flex-end"),
        ("#a5f", "#a5l", 0.0, 160.0, "space-between"),
    ] {
        let w = &first[..3];
        assert!(
            near(dy(first, w), fy) && near(dy(last, w), ly),
            "G_CONTAINER_ALIGNMENT: `align-content:{val}` on a wrapped flex container must put its \
             two lines at y={fy} and y={ly}; got {} and {}. Both lines are asserted because the \
             FIRST line alone cannot separate `stretch` from `center` — they agree on the second.",
            dy(first, w),
            dy(last, w)
        );
    }

    // ── DEFECT 2 — `align-content` in a GRID, where it distributes the ROWS in the block axis.
    for (first, last, fy, ly, val) in [
        ("#b2f", "#b2l", 60.0, 100.0, "center"),
        ("#b3f", "#b3l", 120.0, 160.0, "end"),
        ("#b4f", "#b4l", 0.0, 160.0, "space-between"),
    ] {
        let w = &first[..3];
        assert!(
            near(dy(first, w), fy) && near(dy(last, w), ly),
            "G_CONTAINER_ALIGNMENT: `align-content:{val}` on a grid must put its two 40px rows at \
             y={fy} and y={ly} inside a 200px container; got {} and {}.",
            dy(first, w),
            dy(last, w)
        );
    }

    // ── DEFECT 3 — `justify-items`: the grid container's INLINE-axis default for its items.
    assert!(
        near(dx("#c2i", "#c2"), 140.0),
        "G_CONTAINER_ALIGNMENT: `justify-items:end` must put a 60px item at x=140 in a 200px track, \
         not {}. Reading 0 means the property never reached taffy — the same absence as t980's \
         `justify-self`, one level up at the container.",
        dx("#c2i", "#c2")
    );
    assert!(
        near(dx("#c3i", "#c3"), 70.0),
        "G_CONTAINER_ALIGNMENT: `justify-items:center` must put a 60px item at x=70, not {}.",
        dx("#c3i", "#c3")
    );

    // ── DEFECT 4 — the `place-*` shorthands, asserted on BOTH axes. Each of these landed exactly
    //    half before the change, which is the evidence that the longhand was arriving and being
    //    discarded rather than never parsing.
    assert!(
        near(dx("#e1f", "#e1"), 120.0) && near(dy("#e1f", "#e1"), 60.0),
        "G_CONTAINER_ALIGNMENT: `place-content:center` must centre on BOTH axes — [120, 60], not \
         [{}, {}]. Before this change the justify half landed and the align half was dropped.",
        dx("#e1f", "#e1"),
        dy("#e1f", "#e1")
    );
    assert!(
        near(dx("#e2i", "#e2"), 140.0) && near(dy("#e2i", "#e2"), 60.0),
        "G_CONTAINER_ALIGNMENT: `place-items:end end` must end-align on BOTH axes — [140, 60], not \
         [{}, {}]. Before this change the align half landed and the justify half was dropped.",
        dx("#e2i", "#e2"),
        dy("#e2i", "#e2")
    );

    // ── CONTROL A — the INITIAL value, which this property has always got right and which a
    //    `Normal → FlexStart` slip would silently break: with no declaration the two flex lines
    //    STRETCH to 100px each, so the second line starts at 100 and not at 40.
    assert!(
        near(dy("#a1f", "#a1"), 0.0) && near(dy("#a1l", "#a1"), 100.0),
        "G_CONTAINER_ALIGNMENT: with NO `align-content` declared, a wrapped flex container stretches \
         its lines — second line at y=100, not {}. This row pins the initial value; folding `normal` \
         into `flex-start` is the mistake that would pass every declared row and fail this one.",
        dy("#a1l", "#a1")
    );
    assert!(
        near(dy("#b1f", "#b1"), 0.0) && near(dy("#b1l", "#b1"), 40.0),
        "G_CONTAINER_ALIGNMENT: an undeclared grid keeps its two fixed 40px rows at 0 and 40, not \
         {} — fixed tracks have nothing for `stretch` to absorb.",
        dy("#b1l", "#b1")
    );

    // ── CONTROL B — the two twins that were already correct, on rows that share `map_justify` /
    //    `map_align` with the new lines, so an edit to either helper cannot move them silently.
    assert!(
        near(dx("#a6l", "#a6"), 120.0),
        "G_CONTAINER_ALIGNMENT: `justify-content:center` must centre the flex line's leftover item \
         at x=120, not {} — the inline-axis twin, unchanged by this tick.",
        dx("#a6l", "#a6")
    );
    assert!(
        near(dx("#b5f", "#b5"), 120.0),
        "G_CONTAINER_ALIGNMENT: `justify-content:center` on a grid must centre its 60px column at \
         x=120, not {}.",
        dx("#b5f", "#b5")
    );
    assert!(
        near(dy("#c4i", "#c4"), 60.0) && near(dx("#c4i", "#c4"), 0.0),
        "G_CONTAINER_ALIGNMENT: `align-items:end` must put the item at [0, 60], not [{}, {}] — the \
         block-axis twin of `justify-items`, and it must NOT have acquired an inline-axis offset \
         from the new `justify_items` line.",
        dx("#c4i", "#c4"),
        dy("#c4i", "#c4")
    );
    assert!(
        near(dx("#c1i", "#c1"), 0.0) && near(dy("#c1i", "#c1"), 0.0),
        "G_CONTAINER_ALIGNMENT: an undeclared grid item sits at [0, 0] in its 200×100 track, not \
         [{}, {}] — the initial `justify-items` must behave as stretch, which for a 60px-wide item \
         means start.",
        dx("#c1i", "#c1"),
        dy("#c1i", "#c1")
    );
}
