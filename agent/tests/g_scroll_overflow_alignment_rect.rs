//! **G_SCROLL_OVERFLOW_ALIGNMENT_RECT — a relatively-positioned box contributes BOTH positions to a
//! scroll container's overflow region, and only the in-flow one is padded.**
//!
//! t1381 named this as the other half of `css/css-overflow`'s 117 wrong `scrollHeight`/`scrollWidth`
//! subtests, measured it, and left it: **88 of those 117 are two files about relative offsets**
//! (`scrollable-overflow-padding.html`, `scrollable-overflow-transform-unreachable-region.html`).
//!
//! ## THE RULE, AND IT IS TWO RULES THAT ONLY LOOK LIKE ONE
//!
//! 1. A relatively-positioned box contributes its **alignment rectangle** — the position it occupies
//!    in the FLOW, before the offset — as well as the position it was painted at. That is why
//!    `top: -1000px` does not shrink the scroller the box lives in.
//! 2. **Only the in-flow rectangle is inflated by the container's own END PADDING.** The offset
//!    rectangle is added raw.
//!
//! Chrome-measured (CDP-free, `--hide-scrollbars`, a `width:100px; height:100px; padding:10px 5px;
//! overflow:scroll` container around a `10px × 200px` child):
//!
//! ```text
//!                                       chrome   before   after
//!   no offset                 CONTROL     220      220      220     10 + 200 + 10
//!   top:   50px                           260      270      260     10 +  50 + 200 + 0
//!   top: 1000px                          1210     1220     1210     10 +1000 + 200 + 0
//!   top: -1000px                          220      105      220     the IN-FLOW rect, padded
//! ```
//!
//! ⭐⭐⭐ **THE `+10` IN ROW 1 AND ITS ABSENCE IN ROWS 2 AND 3 ARE THE SAME PADDING.** Before this
//! tick the container's end padding was added ONCE, to the finished extent — so it was applied to
//! whichever rectangle happened to win. Rows 2 and 3 are the ones that say it belongs to a
//! *contribution*, not to the total, and they were wrong by exactly `padding-bottom` in a way that
//! looks like a rounding error until the offset is made large.
//!
//! ⭐ **Row 4 is the 88-subtest family**, and it needs the in-flow rectangle: the painted rectangle
//! is 790px above the scroll origin and contributes nothing at all, so without the alignment
//! rectangle the container reports its own padding box and the 200px child may as well not exist.
//!
//! ## WHY THE IN-FLOW POSITION HAD TO BE RECORDED
//!
//! `layout_block` applies the offset with `boxx.translate(dx, dy)`, which **overwrites** the in-flow
//! position — nothing in the fragment tree remembers it. `manuk_layout::relative_offsets()` is a
//! thread-local written at the two `Position::Relative` sites (the block path and the float path),
//! published wholesale at the end of `layout_document` exactly as `grid_tracks()` is, so a box that
//! stops being relatively positioned stops having an offset. It is skipped inside an
//! `intrinsic_probe`, which is the t1120 rule this file's neighbours already carry.
//!
//! ## THE BATTERY
//!
//! ```text
//!                                                          chrome   before   after
//!   g1  WPT's own shape: top:-1000px + margin-bottom:50      270      105      270
//!   g2  …the inline axis: left:-1000px + margin-right:50     260       95      260   (scrollW)
//!   h1  top: 1000px                                         1210     1220     1210
//!   h2  top: -1000px                                         220      105      220
//!   h3  left: 1000px                                        1205     1210     1205   (scrollW)
//!   h4  top: 50px                                            260      270      260
//!   c1  no offset                                CONTROL     220      220      220
//!   c2  no offset, margin-bottom: 50px           CONTROL     270      270      270
//!   c3  no offset, margin-right: 50px            CONTROL     260      260      260   (scrollW)
//!   d7  no offset, margin-bottom: -30px          CONTROL     190      190      190
//!   e5  no offset, two margined children         CONTROL     340      340      340
//!   f1  no offset, nested margin                 CONTROL     270      270      270
//! ```
//!
//! ⚠ `g1`/`g2` are the shape WPT's `scrollable-overflow-padding.html` uses, down to the `width: 0`.
//! They are kept exactly as the suite writes them so this gate and those 30 subtests are testing the
//! same thing; `h1`–`h4` use a `10px` width because a zero-width box is a degenerate case in Blink's
//! propagation (measured and named in `docs/wiki/scrollable-overflow-end-margin.md`) and the
//! positive-offset rows must not be measured through it.
//!
//! ⚠ The six CONTROL rows are t1381's whole battery re-asserted here: this tick moved the end
//! padding from the finished extent into each contribution, which is exactly the kind of change that
//! is right on the new rows and off-by-a-padding on the old ones.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
.s { display:block; width:100px; height:100px; overflow:scroll; padding:10px 5px; line-height:0; }
</style></head><body>
<div class="s" id="g1"><div style="position:relative;width:0;height:200px;margin-bottom:50px;top:-1000px"></div></div>
<div class="s" id="g2"><div style="position:relative;width:200px;height:0;margin-right:50px;left:-1000px"></div></div>
<div class="s" id="h1"><div style="position:relative;width:10px;height:200px;top:1000px"></div></div>
<div class="s" id="h2"><div style="position:relative;width:10px;height:200px;top:-1000px"></div></div>
<div class="s" id="h3"><div style="position:relative;width:200px;height:10px;left:1000px"></div></div>
<div class="s" id="h4"><div style="width:10px;height:200px;position:relative;top:50px"></div></div>
<div class="s" id="c1"><div style="width:0;height:200px"></div></div>
<div class="s" id="c2"><div style="width:0;height:200px;margin-bottom:50px"></div></div>
<div class="s" id="c3"><div style="width:200px;height:0;margin-right:50px"></div></div>
<div class="s" id="d7"><div style="width:0;height:200px;margin-bottom:-30px"></div></div>
<div class="s" id="e5"><div style="width:0;height:100px;margin-bottom:50px"></div><div style="width:0;height:100px;margin-bottom:70px"></div></div>
<div class="s" id="f1"><div><div style="width:0;height:200px;margin-bottom:50px"></div></div></div>
</body></html>"##;

#[test]
fn a_relative_box_contributes_its_in_flow_position_too() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://align.test/", &fonts, 1200.0);
    let dom = page.dom();
    let node = |id: &str| {
        dom.get_element_by_id(dom.root(), id)
            .unwrap_or_else(|| panic!("VACUOUS: the fixture has no #{id}"))
    };
    let geom = |id: &str| -> [f32; 6] {
        page.scroll_geometry(node(id))
            .unwrap_or_else(|| panic!("VACUOUS: #{id} is not a scroll container at all"))
    };
    let near = |g: f32, w: f32| (g - w).abs() < 0.6;

    // ── VACUITY. The offsets must actually have been APPLIED and RECORDED, or every row below is a
    //    statement about ordinary in-flow boxes wearing a `position:relative` declaration.
    let rel = manuk_layout::relative_offsets();
    let child_of = |id: &str| {
        dom.descendants(node(id))
            .find(|&n| dom.tag_name(n) == Some("div"))
            .unwrap_or_else(|| panic!("VACUOUS: #{id} has no child div"))
    };
    assert_eq!(
        rel.get(&child_of("h2")).copied(),
        Some((0.0, -1000.0)),
        "VACUOUS: #h2's child has no recorded relative offset, so its row cannot be about the \
         alignment rectangle — recorded offsets: {} entries",
        rel.len()
    );
    assert!(
        rel.get(&child_of("c1")).is_none(),
        "VACUOUS: an UNOFFSET child has a recorded offset, so the CONTROL rows are not controls"
    );

    // (id, axis (2 = scrollHeight, 3 = scrollWidth), Chrome's number, what the row decides)
    let rows: &[(&str, usize, f32, &str)] = &[
        ("g1", 2, 270.0, "THE 88-SUBTEST FAMILY, in WPT's own spelling — the painted rect is 790px above the scroll origin and contributes nothing, so only the IN-FLOW rectangle keeps the 200px child in the region"),
        ("g2", 3, 260.0, "the same on the inline axis: left:-1000px with a margin-right"),
        ("h1", 2, 1210.0, "a POSITIVE offset contributes its painted rect — and RAW: 10 + 1000 + 200, with no bottom padding"),
        ("h2", 2, 220.0, "the negative twin: the in-flow rect, and this one IS padded — 10 + 200 + 10"),
        ("h3", 3, 1205.0, "the inline axis of the positive case, unpadded: 5 + 1000 + 200"),
        ("h4", 2, 260.0, "a SMALL positive offset, where the missing padding looks like a rounding error until the offset is made large"),
        ("c1", 2, 220.0, "CONTROL — no offset at all, the row the end padding belongs to"),
        ("c2", 2, 270.0, "CONTROL — t1381's end-margin rule, which the padding move must not disturb"),
        ("c3", 3, 260.0, "CONTROL — and its inline-axis twin"),
        ("d7", 2, 190.0, "CONTROL — the negative end margin still pulls the region in"),
        ("e5", 2, 340.0, "CONTROL — two margined children still accumulate"),
        ("f1", 2, 270.0, "CONTROL — the nested auto-height wrapper is still Chrome-exact"),
    ];
    for (id, axis, want, why) in rows {
        let got = geom(id)[*axis];
        assert!(
            near(got, *want),
            "G_SCROLL_OVERFLOW_ALIGNMENT_RECT #{id} {}: Chrome reports {want}, got {got}.\n  {why}",
            if *axis == 2 {
                "scrollHeight"
            } else {
                "scrollWidth"
            }
        );
    }
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// N1  `relative_offsets()` returns an empty map (the pre-tick state — nothing recorded it)
//       -> the VACUITY assert fires first, honestly: the recorded offset and the rule share a
//          source, so N1 proves the fixture reaches the recorder and N2 is the mutation that
//          proves the RULE.
// N2  drop the in-flow arm, keeping only the painted rect
//       -> g1, g2 and h2 read 105 / 95 / 105. The three negative-offset rows, and nothing else.
// N3  pad BOTH rectangles (the pre-tick "add the padding once, at the end" behaviour)
//       -> h1, h3 and h4 read 1220 / 1210 / 270, one end-padding too large; every negative-offset
//          row and every control stays green, which is what makes the padding a per-contribution
//          rule rather than a term in the total.
