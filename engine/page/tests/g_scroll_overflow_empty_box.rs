//! **G_SCROLL_OVERFLOW_EMPTY_BOX — a negative end margin does not shrink a box that has AREA, and
//! the whole battery that said otherwise was built out of `width:0` boxes.**
//!
//! t1420 through t1423 produced FIVE candidate rules for the scrollable-overflow end-margin term,
//! each refuted by exactly one fixture the others did not contain, and reverted three
//! implementations. The refuting fixture was `d7` in `g_scroll_overflow_end_margin` — Chrome 190 for
//! a `height:200px; margin-bottom:-30px` child in a `100x100; padding:10px 5px; overflow:scroll`
//! container, where a border-box floor gives 210.
//!
//! ⭐⭐⭐ **`d7`'s CHILD IS `width:0`, AND THAT IS THE ENTIRE DIFFERENCE.** Re-measured in headless
//! Chrome 145, one variable — the child's width — and nothing else:
//!
//! ```text
//!                                                     chrome   ours(before)
//!   width:0    height:200  margin-bottom:-30px          190        190   ← d7, and it is a CORNER
//!   width:1px  same                                     210        190   ← the rule for every real box
//!   width:50px same                                     210        190
//!   width:50px same, padding:0                          200        170
//! ```
//!
//! An empty box (zero area) contributes no border box to the union — Blink unions rects and an empty
//! rect is a no-op — but its MARGIN box, expanded by the scroll container's end padding, still
//! counts. So the region is `max(border box if non-empty, margin box + end padding)`, and `d7`'s 190
//! is the *second* term winning because the *first* one was never there.
//!
//! ⚠⚠⚠ **EVERY ROW OF THE t1119 BATTERY THAT ARBITRATED THIS RULE USES `width:0`** — c1, c2, d7, f1,
//! all of them — because a zero-width box is the tidy way to write "a 200px-tall thing". The battery
//! selected a code path Chrome treats specially, and then five ticks of rules were fitted to it.
//! *A fixture that zeroes a dimension to keep itself simple is not a simpler case of the general
//! one; it is a different case.*
//!
//! ## AND THE SUBTREE CLAMP IS SUPERSEDED
//!
//! t1417 clamped a subtree to its parent's margin box when that end margin is negative. t1418 then
//! made the end padding conditional on CONTAINMENT (measured against the parent's MARGIN box), and
//! containment already does the clamp's job on every fixture that motivated it. Measured, a
//! `width:60px; padding:10px; overflow:hidden` container around a `height:5px; margin-bottom:-5px`
//! wrapper whose 100px child overflows it:
//!
//! ```text
//!                                          chrome   ours(before)
//!   the escaping grandchild                  110        20   ← clamped to a box it is not inside
//!   an AUTO-height wrapper, margin -5px      115       115   CONTROL — containment, not the clamp
//!   the same wrapper with margin +5px        125       125   CONTROL
//! ```
//!
//! A subtree that has already escaped its parent's box is not bounded by that box's margin, and the
//! clamp said it was.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
 .s { display:block; width:100px; height:100px; overflow:scroll; padding:10px 5px; }
 .t { display:block; width:60px; overflow:hidden; padding:10px; }
</style></head><body>
<div class="s" id="z0"><div style="width:0;height:200px;margin-bottom:-30px"></div></div>
<div class="s" id="z1"><div style="width:1px;height:200px;margin-bottom:-30px"></div></div>
<div class="s" id="zw"><div style="width:50px;height:200px;margin-bottom:-30px"></div></div>
<div class="s" id="zp0" style="padding:0"><div style="width:50px;height:200px;margin-bottom:-30px"></div></div>
<div class="s" id="zp0w0" style="padding:0"><div style="width:0;height:200px;margin-bottom:-30px"></div></div>
<div class="s" id="zpos"><div style="width:50px;height:200px;margin-bottom:50px"></div></div>
<div class="t" id="esc"><div style="height:5px;margin-bottom:-5px"><div style="height:100px"></div></div></div>
<div class="t" id="con"><div style="margin-bottom:-5px"><div style="height:100px"></div></div></div>
<div class="t" id="conp"><div style="margin-bottom:5px"><div style="height:100px"></div></div></div>
</body></html>"##;

#[test]
fn a_negative_end_margin_does_not_shrink_a_box_that_has_area() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://empty.test/", &fonts, 1200.0);
    let dom = page.dom();
    let geom = |id: &str| -> [f32; 6] {
        let n = dom
            .get_element_by_id(dom.root(), id)
            .unwrap_or_else(|| panic!("VACUOUS: the fixture has no #{id}"));
        page.scroll_geometry(n)
            .unwrap_or_else(|| panic!("VACUOUS: #{id} is not a scroll container at all"))
    };
    let near = |g: f32, w: f32| (g - w).abs() < 0.6;

    // ── VACUITY. Every row must be a real scroll container with a real client box, or the extents
    //    below are measurements of nothing. `zpos` is additionally the pre-existing rule (t1119) and
    //    has to be right before any row about the NEGATIVE case decides anything.
    for id in ["z0", "z1", "esc"] {
        let g = geom(id);
        assert!(
            g[4] > 0.0 && g[5] > 0.0,
            "VACUOUS: #{id} reports a zero client box {g:?}"
        );
    }

    // (id, axis (2 = scrollHeight), Chrome 145's number, what the row decides)
    let rows: &[(&str, usize, f32, &str)] = &[
        ("z1", 2, 210.0, "⭐ THE DEFECT. One pixel of width, and the border box is in the union: a negative end margin cannot pull the region in past the box's own border box. Ours said 190 — d7's answer, for a box d7 is not about"),
        ("zw", 2, 210.0, "the same at a realistic width — this is the shape every card deck with a negative margin actually has"),
        ("zp0", 2, 200.0, "⭐ and with NO container padding, which removes the `+ end padding` term entirely and leaves the border box alone against the margin box: 200, not 170"),
        ("z0", 2, 190.0, "⭐ THE CORNER, PRESERVED. `width:0` — an empty box contributes no border box, so the margin box wins and the answer is d7's 190. Delete the emptiness test and this row reads 210"),
        ("zp0w0", 2, 170.0, "the empty box with no padding either: purely the margin box, 10 + 200 - 30 with a 0 padding, and nothing floors it"),
        ("zpos", 2, 270.0, "CONTROL — t1119's rule. A POSITIVE end margin still EXTENDS the region; a border-box floor must not turn the inflation into a max that ignores it"),
        ("esc", 2, 110.0, "⭐ THE SUPERSEDED CLAMP. A grandchild that has already escaped its parent's 5px box is not bounded by that parent's -5px margin. Ours clamped it to 20"),
        ("con", 2, 115.0, "CONTROL — the AUTO-height wrapper the clamp was written for still answers 115, because CONTAINMENT (t1418) already withholds the end padding from the child that overflows the wrapper's margin box"),
        ("conp", 2, 125.0, "CONTROL — the positive-margin twin of `con`, which a clamp widened to every margin would break"),
    ];
    for (id, axis, want, why) in rows {
        let got = geom(id)[*axis];
        assert!(
            near(got, *want),
            "G_SCROLL_OVERFLOW_EMPTY_BOX #{id} scrollHeight: Chrome reports {want}, got {got}.\n  {why}"
        );
        let _ = axis;
    }
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// N1  drop the border-box term entirely (the pre-tick state)
//       -> z1 190, zw 190, zp0 170 — the three rows this tick is about; every CONTROL stays green,
//          which is what identifies the mechanism as the border box and not the margin.
// N2  drop the EMPTINESS test (contribute the border box unconditionally)
//       -> z0 210 and zp0w0 200 against Chrome's 190/170 — and this is the mutation that re-breaks
//          `g_scroll_overflow_end_margin`'s d7, which is why that gate is the second half of this one.
// N3  restore t1417's negative-margin subtree clamp
//       -> esc 20 against Chrome's 110.
// N4  clamp EVERY margin rather than only negative ones
//       -> conp 120 against 125.
