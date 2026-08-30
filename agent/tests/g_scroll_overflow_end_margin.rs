//! **G_SCROLL_OVERFLOW_END_MARGIN — a trailing margin is part of the scrollable overflow region.**
//!
//! `scrollHeight` was short by the last child's `margin-bottom`, on every scroll container on the
//! web. `scrollTop + clientHeight >= scrollHeight` is the *"am I at the bottom"* test every infinite
//! scroller, lazy-image loader and virtualised list runs, and a short `scrollHeight` makes it true
//! too early.
//!
//! ## THE MECHANISM
//!
//! `content_extent` unions the BORDER BOXES of the descendants. A trailing margin with nothing after
//! it is invisible to that union: every box that follows a margin has already accounted for it by
//! sitting lower down, and nothing follows the last one. A scroll container establishes a BFC, so
//! that last margin does not collapse out — it is real space inside the container.
//!
//! ## FOUND BY SURVEYING THE AREA, NOT BY GRINDING IT
//!
//! `css/css-overflow` is the board's lowest-pass ★ CSS-LAYOUT row (49.9%). Surveyed rather than
//! ground, its 457 failing subtests decompose as:
//!
//! ```text
//!   scrollHeight / scrollWidth wrong          117   ← this family
//!   `scroll-marker*` / `scroll-buttons` / `scroll-target-group` / `scroll-axis-lock`
//!   / `line-clamp` (the CSS Overflow 4 shorthand) / `max-lines` / `continue`
//!   — properties of an UNSHIPPED spec level    ~150   SPEC FRONTIER, refused
//!   `overflow-clip-margin`, `overflow-block/inline`, `block-ellipsis` parsing  ~30
//!   the rest: promise rejections, querySelector throws, serialization           ~160
//! ```
//!
//! *An area percentage is not a work item.* The one implementable mechanism with mass is the first
//! row, and this gate is the half of it that is a rule rather than a corner (see the residual below).
//!
//! ## THE BATTERY — headless Chrome (`--hide-scrollbars`), one
//! `width:100px; height:100px; padding:10px 5px; overflow:scroll` container per row
//!
//! ```text
//!                                                         chrome   before   after
//!   c1  one 200px-tall child                    CONTROL     220      220      220
//!   c2  …with margin-bottom: 50px                           270      220      270
//!   c3  a 200px-WIDE child, margin-right: 50px  (scrollW)   260      210      260
//!   c5  a 0-height sibling AFTER it              CONTROL    270      270      270
//!   c6  a 200px-wide child, no margin  (scrollW) CONTROL    210      210      210
//!   d3  a child with margin-TOP: 50px            CONTROL    270      270      270
//!   d4  a child with margin-LEFT: 50px (scrollW) CONTROL    260      260      260
//!   d6  a FLOATED child with margin-bottom: 50px            270      220      270
//!   d7  …with margin-bottom: -30px                          190      220      190
//!   d9  a 0-height child after the margined one  CONTROL    270      270      270
//!   e3  a RELATIVE child (no offset), margin 50             270      220      270
//!   e5  two children, margins 50 and 70                     340      270      340
//!   e6  an inline-block child, margin-bottom 50             270      220      270
//! ```
//!
//! ⭐⭐ **`d7` — the NEGATIVE margin — is what makes this an INFLATION and not a `max`.** Chrome
//! reports **190**, not 220: a negative end margin pulls the region IN. A `.max(bottom)` guard would
//! keep the larger wrong answer on every negative-margin card deck, and it is the mutation a reader
//! adds to "be safe".
//!
//! ⭐⭐ **`d3` / `d4` — the START margins — are the control that says this is an END rule.** A start
//! margin already moved the box along the flow, so it is in its border box's POSITION; adding it
//! again double-counts it. Both rows read the same number in both engines precisely because nothing
//! was added.
//!
//! ⭐ **`c5` / `d9` are the control that says the union was not simply broken.** A margin with a
//! sibling AFTER it was always counted — the following box sits lower down and the union sees that.
//! Only the LAST one was lost, which is why this was invisible to any fixture with a footer.
//!
//! ⚠ `scrollWidth` rows are chosen so the content is WIDER than the client box. Our engine reserves
//! a scrollbar gutter and Chrome was measured with `--hide-scrollbars`, so `clientWidth` is 95 here
//! and 110 there; a row whose content fits inside the client box reports that floor and would be
//! comparing two different scrollbar policies, not two overflow regions.
//!
//! ## ⚠ NAMED, MEASURED, NOT BUILT — the other half of the 117, and it is a different mechanism
//!
//! ```text
//!                                                             chrome   ours
//!   a child at `position:relative; top:-1000px`, margin 50       270    105
//!     (css/css-overflow/scrollable-overflow-padding.html, 30 subtests, and
//!      scrollable-overflow-transform-unreachable-region.html, 58 more)
//!
//!   NESTED PROPAGATION, measured in the same pass:
//!   a 200px child inside a `width:0;height:0` wrapper           120    220
//!   …inside a `width:10px;height:0` wrapper                     210    220
//!   …inside a `width:10px;height:20px` wrapper                  210    220
//!   …inside a `width:0;height:0; overflow:hidden` wrapper       120    220
//!   an auto-height wrapper, inner margin-bottom: 50  CONTROL    270    270  ✓
//! ```
//!
//! The first is the **alignment rectangle** (Blink's *"inflow-bounds"*): a relatively-positioned box
//! contributes its ORIGINAL in-flow position to the region as well as its offset one, so moving it
//! to `top:-1000px` does not shrink the scroller. That needs layout to record a pre-offset rect —
//! a different mechanism from this one, with 88 WPT subtests attached, and it is the ranked next
//! tick rather than something to bolt on here.
//!
//! ⚠⚠ **AND THE HONEST HALF: two of those rows MOVED, from 220 to 270, against Chrome's 120.** They
//! are the ones with a zero-width intermediate wrapper — Chrome propagates no scrollable overflow
//! through one, we propagate all of it, and the margin this tick adds rides along on a contribution
//! that should not have been there. Both rows were already wrong before this tick (220 vs 120); it
//! is the pre-existing nested-propagation gap, unmasked further, and it is named here with its
//! numbers rather than left for someone to find. The realistic nested shape — an AUTO-height wrapper
//! whose inner child carries the margin — is the last row above, and it is Chrome-exact.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
.s { display:block; width:100px; height:100px; overflow:scroll; padding:10px 5px; line-height:0; }
</style></head><body>
<div class="s" id="c1"><div style="width:0;height:200px"></div></div>
<div class="s" id="c2"><div style="width:0;height:200px;margin-bottom:50px"></div></div>
<div class="s" id="c3"><div style="width:200px;height:0;margin-right:50px"></div></div>
<div class="s" id="c5"><div style="width:0;height:200px"></div><div style="width:0;height:0;margin-top:50px"></div></div>
<div class="s" id="c6"><div style="width:200px;height:0"></div></div>
<div class="s" id="d3"><div style="width:0;height:200px;margin-top:50px"></div></div>
<div class="s" id="d4"><div style="width:200px;height:0;margin-left:50px"></div></div>
<div class="s" id="d6"><div style="float:left;width:0;height:200px;margin-bottom:50px"></div></div>
<div class="s" id="d7"><div style="width:0;height:200px;margin-bottom:-30px"></div></div>
<div class="s" id="d9"><div style="width:0;height:200px;margin-bottom:50px"></div><div style="width:0;height:0"></div></div>
<div class="s" id="e3"><div style="width:0;height:200px;margin-bottom:50px;position:relative"></div></div>
<div class="s" id="e5"><div style="width:0;height:100px;margin-bottom:50px"></div><div style="width:0;height:100px;margin-bottom:70px"></div></div>
<div class="s" id="e6"><div style="width:0;height:200px;margin-bottom:50px;display:inline-block"></div></div>
<div class="s" id="f1"><div><div style="width:0;height:200px;margin-bottom:50px"></div></div></div>
</body></html>"##;

#[test]
fn a_trailing_margin_is_inside_the_scrollable_overflow_region() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://scroll.test/", &fonts, 1200.0);
    let dom = page.dom();
    let geom = |id: &str| -> [f32; 6] {
        let n = dom
            .get_element_by_id(dom.root(), id)
            .unwrap_or_else(|| panic!("VACUOUS: the fixture has no #{id}"));
        page.scroll_geometry(n)
            .unwrap_or_else(|| panic!("VACUOUS: #{id} is not a scroll container at all"))
    };
    let near = |g: f32, w: f32| (g - w).abs() < 0.6;

    // ── VACUITY. Every row is a scroll container with a real client box; without this an engine
    //    that returned zeros for all six fields would satisfy nothing below and say so unclearly.
    for id in ["c1", "c2", "c3"] {
        let g = geom(id);
        assert!(
            g[4] > 0.0 && g[5] > 0.0,
            "VACUOUS: #{id} reports a zero client box {g:?}, so the extents below are not \
             measurements of anything"
        );
    }
    // …and the CONTROL row must already be right, or the fixture is not measuring an overflow
    // region at all — `c1` is the same container with the margin removed.
    assert!(
        near(geom("c1")[2], 220.0),
        "VACUOUS: the no-margin control #c1 reports scrollHeight {} where Chrome says 220 — this \
         gate's subject is the MARGIN, and the container's own padding rule has to be right first",
        geom("c1")[2]
    );

    // (id, axis, the number Chrome reports, what the row decides)
    //   axis 2 = scrollHeight, 3 = scrollWidth
    let rows: &[(&str, usize, f32, &str)] = &[
        ("c1", 2, 220.0, "CONTROL — 10 + 200 + 10, the container's own end padding, which was already right"),
        ("c2", 2, 270.0, "THE DEFECT — the last child's margin-bottom is real space inside a BFC and the union of border boxes could not see it"),
        ("c3", 3, 260.0, "the same rule on the INLINE axis: margin-right, not just margin-bottom"),
        ("c5", 2, 270.0, "CONTROL — a margin with a sibling AFTER it was always counted, because the following box sits lower down"),
        ("c6", 3, 210.0, "CONTROL — a wide child with NO margin is unchanged; the inflation must be zero when the margin is"),
        ("d3", 2, 270.0, "CONTROL — a margin-TOP is already in the box's POSITION. Adding it here would double-count it"),
        ("d4", 3, 260.0, "CONTROL — and the same for margin-LEFT on the inline axis"),
        ("d6", 2, 270.0, "a FLOAT contributes to the scrollable overflow region, end margin and all"),
        ("d7", 2, 190.0, "A NEGATIVE end margin pulls the region IN — 190, not 220. This is an inflation, not a `max`"),
        ("d9", 2, 270.0, "CONTROL — the margined child is no longer last, and the answer is the same by a different route"),
        ("e3", 2, 270.0, "a `position:relative` child with NO offset is an ordinary in-flow box here"),
        ("e5", 2, 340.0, "two margined children: 10 + 100 + 50 + 100 + 70 + 10. The rule applies per box, not once"),
        ("e6", 2, 270.0, "an inline-block child — an atomic inline is still a box with margins"),
        ("f1", 2, 270.0, "THE REALISTIC NESTED SHAPE — an auto-height wrapper whose inner child carries the margin, which is what a card in a scroller actually looks like"),
    ];
    for (id, axis, want, why) in rows {
        let got = geom(id)[*axis];
        assert!(
            near(got, *want),
            "G_SCROLL_OVERFLOW_END_MARGIN #{id} {}: Chrome reports {want}, got {got}.\n  {why}",
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
// N1  `content_extent_with_end_margins` ignores its closure (the pre-tick `content_extent`)
//       -> c2, c3, d6, e3, e5, e6 and f1 all read the pre-tick number; every CONTROL stays green,
//          which is what identifies the mechanism as the END margin and not the union.
// N2  clamp the inflation to non-negative (`mb.max(0.0)`) — the "be safe" mutation
//       -> only d7 fails, at 220 against Chrome's 190.
// N3  inflate by the START margins as well (margin-left / margin-top)
//       -> c5 reads 320 against Chrome's 270 — its second child's margin-TOP gets counted twice,
//          once in the box's position and once here. The start margin is already in the position.
