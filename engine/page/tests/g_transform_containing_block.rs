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
//! ## The other three, and the negative half that is the whole difficulty (t987)
//!
//! `will-change`, `contain` and `perspective` obey this rule too, and t986 left them out because
//! they had **no `ComputedStyle` field at all**. They now reach layout as one `bool`, because one
//! bit is all layout needs — and the interesting part is not which values set it but **which do
//! not**:
//!
//! ```text
//!   will-change: transform / filter / perspective     containing block      [ 20,  20]
//!   will-change: top, transform  (one qualifying)     containing block      [ 20,  20]
//!   contain: layout / paint / strict / content        containing block      [ 20,  20]
//!   perspective: 100px                                containing block      [ 20,  20]
//!   contain: layout, with an ABSOLUTE child           containing block      [ 20,  20]
//!  ── NEGATIVE, and each one is a trap ──
//!   will-change: opacity                              NOT                   [ 20,-364]
//!   contain: style                                    NOT                   [ 20,-1132]
//!   contain: size                                     NOT                   [ 20,-1260]
//!   nothing at all                                    NOT                   [ 20,-1644]
//! ```
//!
//! **A predicate written as "any `will-change`" or "any `contain`" passes every positive row above
//! and is wrong about all three negatives.** `will-change: opacity` creates a *stacking context* —
//! which is a different thing that the same property also does — and `contain: style` / `contain:
//! size` are containment of other kinds entirely. All four negative rows are Chrome-measured, not
//! reasoned from the grammar.
//!
//! On the Stylo path none of that list is re-derived: `WillChangeBits::FIXPOS_CB_NON_SVG` is
//! literally *"a property that creates a containing block for fixed-position descendants will
//! change"*, so the engine that already computed the answer is asked for it. Re-deriving the keyword
//! list by hand is exactly how the `opacity` case gets shipped wrong.
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
//! - **Write the t987 predicate as `!will_change.is_empty() || !contain.is_empty()`** — the obvious
//!   version — → **all ten positive rows pass** and `will-change: opacity` reads [20, 20] against a
//!   viewport-relative answer. That is the single most useful RED in this file: it is what the fix
//!   looks like if you write it from the property NAMES instead of from their VALUES.

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
<div class="w" id="w1" style="will-change:transform"><div class="b" id="b1" style="position:fixed;left:20px;top:20px"></div></div>
<div class="w" id="w2" style="will-change:filter"><div class="b" id="b2" style="position:fixed;left:20px;top:20px"></div></div>
<div class="w" id="w3" style="will-change:perspective"><div class="b" id="b3" style="position:fixed;left:20px;top:20px"></div></div>
<div class="w" id="w4" style="will-change:top,transform"><div class="b" id="b4" style="position:fixed;left:20px;top:20px"></div></div>
<div class="w" id="w5" style="contain:layout"><div class="b" id="b5" style="position:fixed;left:20px;top:20px"></div></div>
<div class="w" id="w6" style="contain:paint"><div class="b" id="b6" style="position:fixed;left:20px;top:20px"></div></div>
<div class="w" id="w7" style="contain:strict"><div class="b" id="b7" style="position:fixed;left:20px;top:20px"></div></div>
<div class="w" id="w8" style="contain:content"><div class="b" id="b8" style="position:fixed;left:20px;top:20px"></div></div>
<div class="w" id="w9" style="perspective:100px"><div class="b" id="b9" style="position:fixed;left:20px;top:20px"></div></div>
<div class="w" id="w10" style="contain:layout"><div class="b" id="b10" style="position:absolute;left:20px;top:20px"></div></div>
<div class="w" id="n1" style="will-change:opacity"><div class="b" id="m1" style="position:fixed;left:20px;top:20px"></div></div>
<div class="w" id="n2" style="contain:style"><div class="b" id="m2" style="position:fixed;left:20px;top:20px"></div></div>
<div class="w" id="n3" style="contain:size"><div class="b" id="m3" style="position:fixed;left:20px;top:20px"></div></div>
<div class="w" id="n4"><div class="b" id="m4" style="position:fixed;left:20px;top:20px"></div></div>
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

    // ── THE OTHER THREE PROPERTIES (t987): `will-change`, `contain`, `perspective`. Same rule,
    //    reaching layout as one `bool` because one bit is all layout needs.
    for (i, decl) in [
        "will-change:transform",
        "will-change:filter",
        "will-change:perspective",
        "will-change:top,transform (one qualifying feature in a list)",
        "contain:layout",
        "contain:paint",
        "contain:strict",
        "contain:content",
        "perspective:100px",
        "contain:layout, with an ABSOLUTE child rather than a fixed one",
    ]
    .iter()
    .enumerate()
    {
        let (b, w) = (format!("#b{}", i + 1), format!("#w{}", i + 1));
        assert!(
            near(off(&b, &w)),
            "G_TRANSFORM_CONTAINING_BLOCK: `{decl}` makes the ancestor the containing block, so the \
             out-of-flow child belongs at [20, 20] from it; got {:?}. These three properties reach \
             layout as one bit on `ComputedStyle`; a reading of [20, -something] means the bit \
             never arrived.",
            off(&b, &w)
        );
    }

    // ── THE NEGATIVE HALF, which is the whole difficulty. A predicate written as "any
    //    `will-change`" or "any `contain`" passes all ten rows above and is wrong about all four of
    //    these. Every one is Chrome-measured, not reasoned from the grammar.
    for (i, (decl, why)) in [
        (
            "will-change: opacity",
            "creates a STACKING CONTEXT — a different thing the same property also does — and NOT a              containing block",
        ),
        ("contain: style", "is style containment, an unrelated kind"),
        ("contain: size", "is size containment, likewise"),
        (
            "no such property at all",
            "is the base case, and the row that fails if the bit defaulted to true",
        ),
    ]
    .iter()
    .enumerate()
    {
        let (m, n) = (format!("#m{}", i + 1), format!("#n{}", i + 1));
        assert!(
            !near(off(&m, &n)),
            "G_TRANSFORM_CONTAINING_BLOCK: `{decl}` {why}, so the fixed child must fall through to \
             the VIEWPORT and NOT sit at [20, 20] from its wrapper; got {:?}.",
            off(&m, &n)
        );
    }
}
