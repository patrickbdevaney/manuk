//! # G_ABSPOS_STATIC_RTL — a static position is an INLINE-START corner, and `cx` is only that in LTR
//!
//! An absolutely-positioned box with `auto` on both inline insets sits at its STATIC POSITION —
//! CSS Position §3's *"where the box would have been if it were in the flow"*. Both places that
//! record it wrote the content box's ORIGIN, i.e. its LEFT edge, and `layout_abs` then grew the box
//! rightward from there. That is the correct corner in LTR and the wrong one in RTL, where the
//! inline axis runs right-to-left: the box starts at the content box's RIGHT edge and grows
//! LEFTWARD. Chrome puts a 5px-wide inset-less box in a 200px `direction:rtl` container at x=195;
//! ours put it at x=0, and put it there for every out-of-flow box on every RTL page. The whole
//! inline axis was mirrored except this one point.
//!
//! ## Chrome-measured (`--headless=new --dump-dom` + `getBoundingClientRect`)
//!
//! Containers are `position:relative; width:200px; height:60px; font:16px/20px monospace`; boxes
//! are `position:absolute; width:5px; height:6px` unless the row says otherwise. Rects are
//! parent-relative:
//!
//! ```text
//!   row  container                     box                        Chrome     ours (pre)
//!   s1   ltr                           (bare)              CONTROL 0,0        0,0
//!   s7   rtl                           left:0              CONTROL 0,0        0,0
//!   s2   rtl                           (bare)                      195,0      0,0
//!   s3   rtl, after a 30px block       (bare)                      195,10     0,10
//!   s4   rtl                           top:5px                     195,5      0,5
//!   s5   rtl                           width/height auto, "hi"     181,0 w19  0,0 w19
//!   s8   rtl, padding:7px              (bare)                      202,7      7,7
//!   v2   ltr, after inline text "abc"  display:inline      CONTROL 29,0       29,0
//!   v1   rtl, after inline text "abc"  display:inline              166,0      195,0
//!   v3   rtl, after inline text "abc"  display:block               195,20     195,0
//! ```
//!
//! Read the rows against the controls and the mechanism is in them:
//!
//! - **`s1` and `s7` are the controls that stop the fix from becoming "mirror the axis in RTL".**
//!   `s1` is the same markup in LTR and must not move. `s7` states a REAL inset (`left:0`) in an
//!   RTL container, and a real inset resolves against the containing block's left edge in every
//!   direction — the static position is not consulted at all, so it must stay at 0.
//! - **`s8` pins the edge to the CONTENT box, not the padding box.** The container's border box is
//!   214 wide and its content box runs 7..207, so 207-5 = 202. A padding-box reading gives 209.
//! - **`s5` pins it to the box's USED width.** With `width:auto` the box shrink-to-fits to 19px and
//!   lands at 200-19 = 181, so the correction cannot be a specified width or a constant.
//! - **`v1` and `v2` are the same declaration in the two directions**, and they are what force the
//!   INLINE ADVANCE to be measured from the correct end: after `abc` (29px wide) an LTR box starts
//!   at 29, and an RTL box starts at 200-29-5 = 166. Before this tick the refinement was SKIPPED
//!   under an RTL base direction outright, so `v1` sat at the seed, 195, as though no text preceded
//!   it. That skip was correct-ish only while the seed was wrong.
//! - **`v3` is the block-level sibling of `v1`.** A block-level out-of-flow box does not go on the
//!   line, it opens the next one, so its x stays at the inline-start edge (195) and only its y
//!   moves (20). It shares the refinement with `v1` and disagrees with it in both coordinates,
//!   which is why one row cannot carry both.
//!
//! ## How each assertion goes RED
//!
//! - **Return `(cx, false)` unconditionally from `Layout::static_inline_start`** — the pre-tick
//!   seed — and `s2`/`s3`/`s4`/`s5`/`s8` all collapse to the container's left edge.
//! - **Keep the edge but drop the `mark_static_rl` call** and every RTL row lands one box width to
//!   the RIGHT: `s2` at 200 instead of 195, `s5` at 200 instead of 181.
//! - **Restore the `if bcs.direction != Rtl` guard** around `refine_inline_static_positions` and
//!   `v1` goes back to 195 and `v3`'s y back to 0, while `v2` stays exact.
//! - **Feed the LTR far edge to `take` in the RTL branch** (`f.x + f.width` instead of `f.x`) and
//!   `v1` lands at 195 — the near edge of the text rather than its far one.
//!
//! ⚠ The `manuk-layout` unit test `an_insetless_absolute_box_starts_after_the_inline_content_before_it`
//! covers the same seam but asserts a RELATION rather than these literals: it runs on
//! `MinimalCascade`, whose `text_align` initial value is the physical `Left` and which never calls
//! `TextAlign::resolve_physical`, so an RTL line is laid out flush LEFT there and `Hello` sits at
//! x=0 where a browser puts it at 364. This gate runs the real cascade and is where the numbers
//! live.

use manuk_text::FontContext;

const HTML: &str = r##"<!DOCTYPE html><html><head><style>
  html, body { margin: 0; padding: 0; }
  div.cb { position: relative; width: 200px; height: 60px; font: 16px/20px monospace; }
  .rtl { direction: rtl; }
  i { position: absolute; width: 5px; height: 6px; display: block; }
  .flow { width: 30px; height: 10px; }
  .il { display: inline; }
</style></head><body>
<div class="cb"     id="r1"><i id="s1"></i></div>
<div class="cb rtl" id="r2"><i id="s2"></i></div>
<div class="cb rtl" id="r3"><div class="flow"></div><i id="s3"></i></div>
<div class="cb rtl" id="r4"><i id="s4" style="top:5px"></i></div>
<div class="cb rtl" id="r5"><i id="s5" style="width:auto;height:auto">hi</i></div>
<div class="cb rtl" id="r7"><i id="s7" style="left:0"></i></div>
<div class="cb rtl" id="r8" style="padding:7px"><i id="s8"></i></div>
<div class="cb"     id="n2">abc<i class="il" id="v2"></i></div>
<div class="cb rtl" id="n1">abc<i class="il" id="v1"></i></div>
<div class="cb rtl" id="n3">abc<i id="v3"></i></div>
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
        .unwrap_or_else(|| panic!("no box for {sel} — the element has no geometry at all"))
}

#[test]
fn g_abspos_static_rtl_starts_at_the_inline_start_edge() {
    // ⚠⚠ **ONE `#[test]` PER `Page`-BUILDING BINARY (t1342) — DO NOT ADD A SECOND.** SpiderMonkey
    // allows one JS thread per process; a second scripted test in this binary silently runs no
    // script or SIGSEGVs, and the symptom shows up in the OTHER test.
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://rtl-static.test/", &fonts, 1200.0);
    let r = |sel: &str| rect_of(&page, sel);

    // ── THE CONTROLS FIRST. Both are RTL-adjacent rows that must NOT move, and they are what
    //    separate this fix from "mirror the inline axis whenever `direction` is rtl".
    let (r1, s1) = (r("#r1"), r("#s1"));
    assert!(
        ((s1.x - r1.x) - 0.0).abs() < 0.51,
        "G_ABSPOS_STATIC_RTL: the LTR control #s1 is at x={:.2}; an inset-less absolute box in an \
         LTR container sits at its container's content-box LEFT edge and must stay at 0. If this \
         moved, the fix mirrored the axis unconditionally instead of reading `direction`.",
        s1.x - r1.x
    );
    let (r7, s7) = (r("#r7"), r("#s7"));
    assert!(
        ((s7.x - r7.x) - 0.0).abs() < 0.51,
        "G_ABSPOS_STATIC_RTL: #s7 states `left:0` in an RTL container and is at x={:.2}; it must be \
         at 0. A REAL inset resolves against the containing block's left edge in every direction \
         and never consults the static position at all — `position_absolutes` guards the correction \
         on `x_static` precisely so this row cannot move (Chrome: s7@0,0).",
        s7.x - r7.x
    );

    // ── 1. THE INLINE-START EDGE IS THE CONTENT BOX'S RIGHT EDGE IN RTL. Chrome: s2@195,0.
    let (r2, s2) = (r("#r2"), r("#s2"));
    assert!(
        ((s2.x - r2.x) - 195.0).abs() < 0.51 && ((s2.y - r2.y) - 0.0).abs() < 0.51,
        "G_ABSPOS_STATIC_RTL: an inset-less absolute box in a 200px `direction:rtl` container is at \
         {:.2},{:.2}; it must be at 195,0 = 200-5. Its static position is where it WOULD have been \
         in the flow, and RTL inline flow starts at the content box's RIGHT edge and runs leftward. \
         x=0 is the content-box ORIGIN, which is the inline-start corner only in LTR — the pre-tick \
         answer, and the same answer for every out-of-flow box on every RTL page (Chrome: s2@195,0).",
        s2.x - r2.x,
        s2.y - r2.y
    );

    // ── 2. THE BLOCK AXIS IS UNAFFECTED — `direction` is an INLINE-axis property. Chrome: s3@195,10
    //    (after a 30px-tall in-flow block) and s4@195,5 (a real `top`, an auto `left`/`right`).
    let (r3, s3) = (r("#r3"), r("#s3"));
    assert!(
        ((s3.x - r3.x) - 195.0).abs() < 0.51 && ((s3.y - r3.y) - 10.0).abs() < 0.51,
        "G_ABSPOS_STATIC_RTL: after a 30x10 in-flow block, #s3 is at {:.2},{:.2}; it must be at \
         195,10 — the inline-start edge on x, and still BELOW the preceding block on y. A y of 0 \
         means the inline fix ate the block-axis answer (Chrome: s3@195,10).",
        s3.x - r3.x,
        s3.y - r3.y
    );
    let (r4, s4) = (r("#r4"), r("#s4"));
    assert!(
        ((s4.x - r4.x) - 195.0).abs() < 0.51 && ((s4.y - r4.y) - 5.0).abs() < 0.51,
        "G_ABSPOS_STATIC_RTL: #s4 states `top:5px` with both inline insets auto and is at {:.2},{:.2}; \
         it must be at 195,5. The two axes are resolved independently — a real inset on one does not \
         suppress the static position on the other (Chrome: s4@195,5).",
        s4.x - r4.x,
        s4.y - r4.y
    );

    // ── 3. THE CORRECTION IS THE BOX'S *USED* WIDTH. `width:auto` shrink-to-fits `hi` to 19px, so
    //    a constant or a specified width cannot satisfy this row. Chrome: s5@181,0 w19.
    let (r5, s5) = (r("#r5"), r("#s5"));
    assert!(
        (s5.width - 19.0).abs() < 2.01,
        "G_ABSPOS_STATIC_RTL: #s5 is {:.2}px wide; `width:auto` on an out-of-flow box shrink-to-fits \
         to its content ('hi' at 16px monospace, ~19px in Chrome). The x assertion below is written \
         against this width, so a drift here means the ruler moved.",
        s5.width
    );
    assert!(
        ((s5.x - r5.x) - (200.0 - s5.width)).abs() < 0.51,
        "G_ABSPOS_STATIC_RTL: #s5 is at x={:.2} and its used width is {:.2}, so it must start at \
         {:.2}. The `-width` correction has to use the width the box ACTUALLY got — which is why it \
         is applied after `layout_abs` returns rather than where the static position is recorded \
         (Chrome: s5@181,0 w19).",
        s5.x - r5.x,
        s5.width,
        200.0 - s5.width
    );

    // ── 4. THE EDGE IS THE CONTENT BOX'S, NOT THE PADDING BOX'S. Chrome: s8@202,7 in a container
    //    whose border box is 214 wide and whose content box runs 7..207.
    let (r8, s8) = (r("#r8"), r("#s8"));
    assert!(
        (r8.width - 214.0).abs() < 1.01,
        "G_ABSPOS_STATIC_RTL: the `padding:7px` container is {:.2} wide, not 214 — the row below is \
         measured against its content box and needs this to be the border box.",
        r8.width
    );
    assert!(
        ((s8.x - r8.x) - 202.0).abs() < 0.51 && ((s8.y - r8.y) - 7.0).abs() < 0.51,
        "G_ABSPOS_STATIC_RTL: #s8 in a `padding:7px` RTL container is at {:.2},{:.2}; it must be at \
         202,7 = (7+200)-5. 209 would be the PADDING box's right edge — the static position is where \
         the box would have been IN THE FLOW, and flow starts inside the padding (Chrome: s8@202,7).",
        s8.x - r8.x,
        s8.y - r8.y
    );

    // ── 5. THE INLINE ADVANCE IS MEASURED FROM THE OTHER END. `v2`/`v1` are the same declaration
    //    in the two directions; before this tick the refinement was SKIPPED entirely under an RTL
    //    base direction, so `v1` sat at the seed as though nothing preceded it.
    let (n2, v2) = (r("#n2"), r("#v2"));
    let adv = v2.x - n2.x;
    assert!(
        (adv - 29.0).abs() < 2.01,
        "G_ABSPOS_STATIC_RTL: the LTR control #v2 is at x={adv:.2} after the text `abc`; it must be \
         one `abc` advance (~29px at 16px monospace, Chrome 29). Every RTL row below is written \
         against this measured advance rather than a font literal, so a drift here means the ruler \
         moved and not that the feature broke.",
    );
    let (n1, v1) = (r("#n1"), r("#v1"));
    assert!(
        ((v1.x - n1.x) - (200.0 - adv - 5.0)).abs() < 1.01,
        "G_ABSPOS_STATIC_RTL: the inline-level #v1 after `abc` in an RTL container is at x={:.2}; it \
         must be at {:.2} = 200 - {adv:.2} - 5, i.e. one box width left of where the text ENDS on \
         the inline axis. 195 means the refinement was skipped and the box kept the seed as though \
         no text preceded it; {:.2} means the advance was measured from the LTR end (Chrome: \
         v1@166,0 against v2@29,0).",
        v1.x - n1.x,
        200.0 - adv - 5.0,
        adv
    );

    // ── 6. A BLOCK-LEVEL OUT-OF-FLOW BOX OPENS THE NEXT LINE — its x stays at the inline-start
    //    edge and only its y moves. Same refinement as `v1`, disagreeing in BOTH coordinates.
    let (n3, v3) = (r("#n3"), r("#v3"));
    assert!(
        ((v3.x - n3.x) - 195.0).abs() < 0.51 && ((v3.y - n3.y) - 20.0).abs() < 1.51,
        "G_ABSPOS_STATIC_RTL: the block-level #v3 after `abc` in an RTL container is at {:.2},{:.2}; \
         it must be at 195,20. A block-level box does not go ON the line, it opens the NEXT one, so \
         it keeps the inline-start edge (195, unlike #v1's 166) and drops one line height. y=0 means \
         the RTL refinement never ran; x=166 means it was treated as inline-level (Chrome: v3@195,20).",
        v3.x - n3.x,
        v3.y - n3.y
    );
}
