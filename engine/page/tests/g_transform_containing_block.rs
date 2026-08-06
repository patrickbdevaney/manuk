//! # G_TRANSFORM_CONTAINING_BLOCK — a transformed ancestor is a containing block, and nothing knew it
//!
//! CSS Transforms §3: an element with a `transform` becomes the containing block for its
//! `position: absolute` **and** `position: fixed` descendants — whatever its own `position` is.
//! `filter` and `backdrop-filter` carry the same rule. `abs_containing_block` tested only
//! `position != Static`, and `position: fixed` was handed the viewport unconditionally, so an
//! out-of-flow box inside a transformed wrapper **escaped past it entirely**.
//!
//! Measured against headless Chrome, each offset from the wrapper that should own it:
//!
//! ```text
//!                                                          Chrome      before       after
//!   fixed    inside transform:translateX(10px)            [ 20, 20]  [ 10,-1328]  [ 20, 20]
//!   absolute inside transform, ancestor NOT positioned    [ 20, 20]  [ 10,-1200]  [ 20, 20]
//!   fixed    inside filter:blur(0px)                      [ 20, 20]  [ 20,-1072]  [ 20, 20]
//!   fixed    inside a transformed GRANDparent             [ 20, 20]  [ 10, -816]  [ 20, 20]
//!  ── CONTROLS ──
//!   absolute inside a plain position:relative ancestor    [ 20, 20]  [ 20, 20]  unchanged
//!   a transformed box with an IN-FLOW child               [  0,  0]  [  0,  0]  unchanged
//! ```
//!
//! **This is not a rounding error — it is a box on a different part of the page**, which is what
//! makes it an I3/jarring-class defect rather than a shape one. And it is not rare: `transform` is
//! on **34.5% of the corpus** — every animated card, carousel slide, `translateZ(0)` compositing
//! hint and CSS-transitioned panel — and the out-of-flow children inside them are the badges, close
//! buttons, dropdowns and tooltips that a user actually clicks.
//!
//! ## Why one predicate and not a `transform` special case
//!
//! `filter: blur(0px)` — a *no-op* blur — is enough to create the containing block, measured. Three
//! properties, one rule; writing it as `transform`-only would leave two silent holes of the same
//! shape, and the filter row is here to keep the predicate honest.
//!
//! ## The `absolute` row is the one that shows the old test was the wrong test
//!
//! Its wrapper is `position: static`. Under `position != Static` alone it was invisible as a
//! containing block, so the box escaped to the viewport — the ancestor was *right there* and failed
//! a test that had nothing to do with the rule being applied.
//!
//! ⚠ **NAMED, MEASURED, NOT BUILT — and it is the t985 shape one level up.** `will-change`,
//! `contain` and `perspective` obey this rule too. They are not unhandled values: they have **no
//! `ComputedStyle` field at all**, so there is nowhere for the information to live and the fix is a
//! cascade addition rather than a layout one. `will-change: transform` — the commonest of the three
//! by far, since it is the standard compositing hint — was measured Chrome-exact at `[20, 20]` and
//! reads `[20, -364]` here. Fixture `/tmp/tcb.html` row `a4` discriminates it.
//!
//! ## How this goes RED
//!
//! - **Drop `|| Self::establishes_out_of_flow_cb(s)` from `abs_containing_block`** → the `absolute`
//!   row escapes to the viewport; every `fixed` row still passes, because those go through the other
//!   walk. The two halves are separately provable, which is why both are asserted.
//! - **Restore `position == Fixed => viewport`** → the three `fixed` rows fail and the `absolute`
//!   row passes.
//! - **Narrow the predicate to `!s.transform.is_empty()`** → only the `filter` row fails.
//! - **Return the ancestor's BORDER box instead of its padding box** → all rows still pass on this
//!   fixture (no borders); the `padding_box_of` helper is shared with the `absolute` walk precisely
//!   so the two cannot drift, and `g_abs_padding_box`-style bordered cases cover it.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
html,body{margin:0;padding:0}
body{font:16px/1 monospace}
*{box-sizing:border-box}
.w{width:300px;height:120px;margin:0 0 8px 0}
.b{width:50px;height:30px}
</style></head><body>
<div class="w" id="c1" style="transform:translateX(10px)"><div class="b" id="a1" style="position:fixed;left:20px;top:20px"></div></div>
<div class="w" id="c2" style="transform:translateX(10px)"><div class="b" id="a2" style="position:absolute;left:20px;top:20px"></div></div>
<div class="w" id="c3" style="filter:blur(0px)"><div class="b" id="a3" style="position:fixed;left:20px;top:20px"></div></div>
<div class="w" id="c5" style="transform:translateX(10px)"><div style="margin-left:40px"><div class="b" id="a5" style="position:fixed;left:20px;top:20px"></div></div></div>
<div class="w" id="c6" style="position:relative"><div class="b" id="a6" style="position:absolute;left:20px;top:20px"></div></div>
<div class="w" id="c7" style="transform:translateX(10px)"><div class="b" id="a7"></div></div>
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
fn g_transform_containing_block() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://tcb.test/", &fonts, 1200.0);
    let off = |sel: &str, w: &str| {
        let (a, c) = (rect_of(&page, sel), rect_of(&page, w));
        (a.x - c.x, a.y - c.y)
    };
    let near = |got: (f32, f32)| (got.0 - 20.0).abs() < 1.1 && (got.1 - 20.0).abs() < 1.1;

    // ── DEFECT — four ways an out-of-flow box escaped a wrapper that owns it. Every one of these
    //    read a y of several hundred to over a thousand pixels off: a box on a different part of
    //    the page, not a mis-sized one.
    for (sel, w, why) in [
        (
            "#a1",
            "#c1",
            "`position:fixed` inside `transform:translateX(10px)` — CSS Transforms §3 makes the \
             transformed ancestor the containing block, which is why a fixed box inside a \
             transformed wrapper SCROLLS WITH THE PAGE in every browser instead of staying pinned. \
             It was handed the viewport unconditionally",
        ),
        (
            "#a2",
            "#c2",
            "`position:absolute` inside a transformed ancestor that is `position: static`. This is \
             the row that shows the old test was the WRONG TEST: `position != Static` alone made \
             the wrapper invisible as a containing block, so the box escaped past an ancestor that \
             was right there",
        ),
        (
            "#a3",
            "#c3",
            "`filter: blur(0px)` — a NO-OP blur is enough. Three properties carry this rule and \
             writing it as a `transform` special case would leave two holes of the same shape",
        ),
        (
            "#a5",
            "#c5",
            "the transformed ancestor is the GRANDparent, with an untransformed block between — the \
             walk must keep going up rather than give up at the first non-qualifying ancestor",
        ),
    ] {
        assert!(
            near(off(sel, w)),
            "G_TRANSFORM_CONTAINING_BLOCK: {sel} must sit at [20, 20] from {w} — {why}; got {:?}.",
            off(sel, w)
        );
    }

    // ── CONTROL A — an ordinary `position:relative` ancestor, which always worked. A predicate that
    //    fired on the wrong ancestors, or a walk that stopped in the wrong place, breaks this.
    assert!(
        near(off("#a6", "#c6")),
        "G_TRANSFORM_CONTAINING_BLOCK: `absolute` inside a plain `position:relative` ancestor is \
         still [20, 20], not {:?} — the pre-existing rule must survive the new one.",
        off("#a6", "#c6")
    );

    // ── CONTROL B — a transformed box whose child is IN FLOW. The transform still moves the child
    //    with its parent and must not additionally offset it: this is the row that fails if the
    //    ancestor's transform were applied a second time on the way out.
    let inflow = off("#a7", "#c7");
    assert!(
        inflow.0.abs() < 1.1 && inflow.1.abs() < 1.1,
        "G_TRANSFORM_CONTAINING_BLOCK: an IN-FLOW child of a transformed box sits at [0, 0] \
         relative to it, not {inflow:?} — the transform is already baked into the subtree's \
         coordinates and must not be counted twice."
    );
}
