//! # G_TEXT_INDENT_EDGES — the indent moves the line's START edge; it was charged as a leading fragment
//!
//! `text-indent` shifts the first line box's inline-**start** edge. Its **end** edge is the
//! container's, so the line is `indent` px narrower *and* begins `indent` px further in. The line
//! layout reduced `line_avail` by the indent **and** started the first fragment's `x` at
//! `text_indent`, while `line_left` never moved — so the wrap test
//! `pen + space + advance > line_avail`, with `pen` already carrying the indent, **charged it
//! twice**, and alignment was computed against a band whose left edge was in the wrong place.
//!
//! ## Two symptoms, one arithmetic error, and neither is visible in a block's height
//!
//! ```text
//!                                                          Chrome     before      after
//!   text-indent:20px in an 80px box, marker on LINE 2       [  0]      [ 29]      [  0]
//!   text-indent:20px + text-align:center, 400px box         [196]      [186]      [196]
//!   text-indent:20px + text-align:right,  400px box         [371]      [351]      [371]
//!  ── CONTROLS, none of which moved ──
//!   text-indent:20px, marker on LINE 1                      [ 20]      [ 20]  unchanged
//!   text-indent:30px with a nested marker                   [ 30]      [ 30]  unchanged
//!   text-indent:10% of a 400px container                    [ 40]      [ 40]  unchanged
//!   text-indent:-9999px (image replacement)               [-9999]    [-9999]  unchanged
//!   text-align:center / right with NO indent           [186] / [371]         unchanged
//! ```
//!
//! **The break-point symptom is invisible to every obvious instrument.** With `text-indent: 20px` in
//! an 80px box, Chrome breaks `aa bb / cc` and we broke `aa / bb cc`: **two lines either way**, so
//! the block's height matches, the container's width matches, and the text is all present. Only the
//! x of a marker on the *second* line reveals it — which is why the twenty-row text battery that
//! found this could see it only as a 20px width difference on an inline box's union, and a
//! dedicated probe was needed to say what had actually happened.
//!
//! ## The alignment rows are the evidence that the MODEL is right, not a bonus
//!
//! Both fell out of the same one-line change, and they discriminate between the two models that
//! explain the break-point symptom equally well:
//!
//! ```text
//!   "the indent is a leading space"          -> centre at (400−29)/2 = 186        WRONG
//!   "the start edge moves in, the end edge does not"
//!                                            -> centre at 20 + (380−29)/2 = 196   Chrome
//!                                            -> right  at 20 + (380−29)  = 371   Chrome
//! ```
//!
//! A fix aimed only at the break point could have been written either way and passed. **Two models
//! that agree on the symptom you noticed are separated by the property you did not think to
//! combine it with.**
//!
//! ⚠ A **negative** indent still widens the line and moves it off-screen, because both terms flip
//! sign together — `text-indent: -9999px`, the image-replacement idiom, is a control here for
//! exactly that reason.
//!
//! ## How this goes RED
//!
//! Each recipe below was applied, built, and read off the WHOLE fixture — not off the gate's first
//! failing assertion — so the confinement is measured rather than assumed:
//!
//! - **Restore `line_left = l` and the first fragment's `x = text_indent`** (the original) →
//!   `#a1` 29, `#a2` 186, `#a3` 351. **All six controls pass.**
//! - **Move `line_left` but leave `line_avail = w`** → `#a1` 78, `#a2` 206, `#a3` 391. All six
//!   controls pass. ⚠ I predicted this one would fail the alignment rows and *pass* the break-point
//!   row; the alignment numbers were exactly right and the break-point row fails too, because an
//!   un-narrowed 80px band fits all of `aa bb cc` on the indented line.
//! - **Clamp a negative indent at zero "to be safe"** → **only `#a7` fails**, at 0 against -9999.
//!
//! ⚠ **A fourth recipe — applying the indent to EVERY line rather than the first — does NOT fire,
//! and the reason is structural rather than a fixture gap.** Wrapped lines never reach the
//! `cur.is_empty()` block that reads `first_line`: the break branch sets `line_left = l` and
//! `line_avail = w` directly. So the "first line only" behaviour is enforced by the *break path*,
//! and the `first_line` test in the open-band block only ever governs the block's first line.
//! Recorded rather than dropped — a "how to break it" step that cannot break it is worse than a
//! shorter list.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
html,body{margin:0;padding:0}
body{font:16px/1.5 monospace}
*{box-sizing:border-box}
.w{width:400px;margin:0 0 6px 0}
</style></head><body>
<div class="w" id="c1" style="width:80px;text-indent:20px">aa bb <span id="a1">cc</span></div>
<div class="w" id="c2" style="text-indent:20px;text-align:center"><span id="a2">abc</span></div>
<div class="w" id="c3" style="text-indent:20px;text-align:right"><span id="a3">abc</span></div>
<div class="w" id="c4" style="width:80px;text-indent:20px"><span id="a4">aa</span> bb cc</div>
<div class="w" id="c5" style="text-indent:30px"><span id="a5">x</span>yyy</div>
<div class="w" id="c6" style="text-indent:10%"><span id="a6">abc</span></div>
<div class="w" id="c7" style="text-indent:-9999px"><span id="a7">abc</span></div>
<div class="w" id="c8" style="text-align:center"><span id="a8">abc</span></div>
<div class="w" id="c9" style="text-align:right"><span id="a9">abc</span></div>
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
fn g_text_indent_edges() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://ti.test/", &fonts, 1200.0);
    let dx = |sel: &str, w: &str| rect_of(&page, sel).x - rect_of(&page, w).x;
    let near = |got: f32, want: f32| (got - want).abs() < 1.6;

    // ── DEFECT 1 — the BREAK POINT. The indent was charged to the wrap test twice, so the first
    //    line ended one word early. Two lines either way, so nothing about the block's SIZE moves.
    assert!(
        near(dx("#a1", "#c1"), 0.0),
        "G_TEXT_INDENT_EDGES: with `text-indent:20px` in an 80px box, Chrome fits `aa bb` on the \
         indented first line and starts line 2 with `cc` at x=0; got {}. 29 means we broke after \
         `aa` — the indent was subtracted from `line_avail` AND added to the pen, so the wrap test \
         paid for it twice. The block is two lines tall either way, which is why no height or width \
         reading can see this.",
        dx("#a1", "#c1")
    );

    // ── DEFECT 2 — ALIGNMENT, and these two rows are what separate the right model from a fix that
    //    merely cures defect 1. The indent moves the start edge in and leaves the end edge alone.
    assert!(
        near(dx("#a2", "#c2"), 196.0),
        "G_TEXT_INDENT_EDGES: `text-indent:20px; text-align:center` in a 400px box centres a 29px \
         word at 20 + (380-29)/2 = 196, not {}. 186 is centring in the FULL width — the indent \
         narrowed the band but never moved its left edge.",
        dx("#a2", "#c2")
    );
    assert!(
        near(dx("#a3", "#c3"), 371.0),
        "G_TEXT_INDENT_EDGES: `text-indent:20px; text-align:right` puts the word at \
         20 + (380-29) = 371 — flush with the container's right edge, because the indent does NOT \
         move the END edge; got {}. 351 is the band narrowed at the wrong end.",
        dx("#a3", "#c3")
    );

    // ── CONTROL A — the FIRST line's own start, which was always right and is what a fix that
    //    dropped the indent entirely would break.
    assert!(
        near(dx("#a4", "#c4"), 20.0),
        "G_TEXT_INDENT_EDGES: the first line still begins at the indent — a marker at its start is \
         at x=20, not {}.",
        dx("#a4", "#c4")
    );
    assert!(
        near(dx("#a5", "#c5"), 30.0),
        "G_TEXT_INDENT_EDGES: `text-indent:30px` with a nested marker is at x=30, not {}.",
        dx("#a5", "#c5")
    );

    // ── CONTROL B — a PERCENTAGE indent resolves against the containing block's width.
    assert!(
        near(dx("#a6", "#c6"), 40.0),
        "G_TEXT_INDENT_EDGES: `text-indent:10%` of a 400px container is 40px, not {}.",
        dx("#a6", "#c6")
    );

    // ── CONTROL C — a NEGATIVE indent must still widen the line and carry it off-screen. Both terms
    //    flip sign together, so the image-replacement idiom is unchanged; a fix that clamped the
    //    indent at zero "to be safe" fails here and nowhere else.
    assert!(
        near(dx("#a7", "#c7"), -9999.0),
        "G_TEXT_INDENT_EDGES: `text-indent:-9999px` — the image-replacement idiom — puts the text \
         at -9999, not {}.",
        dx("#a7", "#c7")
    );

    // ── CONTROL D — alignment with NO indent at all. These are the rows the alignment fix must not
    //    move, and they are the same two properties on the same content.
    assert!(
        near(dx("#a8", "#c8"), 186.0) && near(dx("#a9", "#c9"), 371.0),
        "G_TEXT_INDENT_EDGES: with no indent, centre is (400-29)/2 = 186 and right is 371; got {} \
         and {}.",
        dx("#a8", "#c8"),
        dx("#a9", "#c9")
    );
}
