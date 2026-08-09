//! # G_RTL_OVERCONSTRAINED_MARGIN — the ignored margin is RECOMPUTED, and it may be NEGATIVE
//!
//! CSS 2.1 §10.3.3 constrains a block-level non-replaced box in normal flow to
//! `margin-left + border + padding + width + padding + border + margin-right = containing block
//! width`, and it resolves a conflict in **two ordered steps**:
//!
//! 1. *"If `width` is not `auto` and the non-auto parts are larger than the containing block, then
//!    any `auto` values for `margin-left`/`margin-right` are treated as **zero**."*
//! 2. Then, with nothing `auto` left, the equation still cannot hold, so one term is **ignored** and
//!    re-derived from the equality — `margin-right` under `ltr`, `margin-left` under `rtl`.
//!
//! Manuk ran only step 2, only for the arm where both margins were already non-`auto`, and clamped
//! its result with `.max(0.0)`. Three distinct wrong answers came out of one `match`, which is why
//! no single row exposed them. Chrome, a 100px block in a 400px RTL container:
//!
//! ```text
//!   declaration                        Chrome    was     why it was wrong
//!   margin-right: 301px                   -1       0     the clamp, one pixel past the crossing
//!   margin-right: 350px                  -50       0     the clamp
//!   margin-right: 500px                 -200       0     the clamp
//!   margin-left:auto;  margin-right:500  -200       0     auto arm ran INSTEAD of the direction clause
//!   margin-left:auto;  margin-right:auto -200       0     …and the centring arm did too (width 600)
//!   margin-left:500px; margin-right:auto  300     500     …and this arm did nothing at all
//! ```
//!
//! ⚠⚠⚠ **THE CLAMP WAS RIGHT ON ITS NEIGHBOURS FOR A DIFFERENT REASON, WHICH IS WHY IT LOOKED LIKE
//! A SHARED SAFETY RULE.** `.max(0.0)` on the two `auto` arms implements step **1** (an overflowing
//! `auto` margin is zero). The same `.max(0.0)` on the over-constrained arm was a floor on step
//! **2**, which has no floor: a box whose `margin-right` exceeds the space simply hangs off the left
//! of its containing block. One expression, two clauses, and only one of them wanted it.
//!
//! ## How it was found
//!
//! Not by reading the spec. `css/CSS2/margin-padding-clear` was at 575/102, and clustering the 102
//! failures by test family put `margin-right` at 20 — of which **19 declare `direction: rtl`**,
//! against a 2% failure rate for the 45 non-RTL tests in the same family. The suite ranked *where to
//! look*; a 23-row battery said what was there.
//!
//! ```text
//!   css/CSS2/margin-padding-clear   575 passed / 102 failed  ->  592 / 85     (+17)
//! ```
//!
//! ## How this goes RED
//!
//! - Restore `.max(0.0)` on the over-constrained arm → `neg_one`, `neg_fifty` and `neg_twohundred`
//!   report 0, while `zero_crossing` (`margin-right: 300px`, the last value whose answer IS 0) still
//!   passes. That pair is what proves the defect is the FLOOR and not the formula.
//! - Move the `(true, true)` / `(true, false)` arms back above the RTL arms → `auto_start`,
//!   `auto_both` and `auto_end` report 0 / 0 / 500 while every non-`auto` row still passes: the arm
//!   ORDER is the spec's step order, and reversing it is a different defect from the clamp.
//! - Drop the `parent_is_rtl` guard on the over-constrained arm → the control arm goes red at
//!   `ltr_neg` (reported 600 against Chrome's 0; the value a doubly-wrong build produces is not
//!   worth explaining, the point is that the LTR rows have teeth and are not passing vacuously).
//!
//! ⚠ All three of the above were RUN, not reasoned. A fourth — dropping the replaced /
//! atomic-inline exclusions — is not listed as a proof because this fixture contains no `<img>` and
//! no `inline-block`, so it would be green by inspection; those exclusions are held by the rows in
//! the RTL battery, not here, and a green mutation asserted as a red one is the error t1086 made.
//!
//! ⚠ Run with `--features stylo,spidermonkey`.
//!
//! ONE `#[test]` per file — `PageContext` is per-process (see `g_caption_paint.rs`).

use manuk_text::FontContext;

const W: f32 = 1200.0;

/// Fixed widths only, so nothing here depends on a font. Container 400, box 100: under `rtl` the
/// resolved `margin-left` is `400 − 100 − margin-right`, which crosses zero at `margin-right: 300`.
const HTML: &str = r##"<!doctype html><html><head><style>
html, body, div { margin: 0; padding: 0; border: 0 }
body { width: 1200px; font: 16px/20px monospace }
.c { width: 400px; height: 14px }
.b { width: 100px; height: 12px }
</style></head><body>

<div class="c" style="direction:rtl"><div class="b" id="zero_crossing" style="margin-right:300px"></div></div>
<div class="c" style="direction:rtl"><div class="b" id="neg_one" style="margin-right:301px"></div></div>
<div class="c" style="direction:rtl"><div class="b" id="neg_fifty" style="margin-right:350px"></div></div>
<div class="c" style="direction:rtl"><div class="b" id="neg_twohundred" style="margin-right:500px"></div></div>
<div class="c" style="direction:rtl"><div class="b" id="ignored_not_offset" style="margin-left:20px;margin-right:500px"></div></div>

<div class="c" style="direction:rtl"><div class="b" id="auto_start" style="margin-left:auto;margin-right:500px"></div></div>
<div class="c" style="direction:rtl"><div id="auto_both" style="width:600px;height:12px;margin-left:auto;margin-right:auto"></div></div>
<div class="c" style="direction:rtl"><div class="b" id="auto_end" style="margin-left:500px;margin-right:auto"></div></div>

<div class="c" style="direction:rtl;width:0"><div id="zero_container" style="width:0;height:12px;margin-right:96px"></div></div>

<div class="c" style="direction:rtl"><div class="b" id="fits_start" style="margin-left:auto;margin-right:25px"></div></div>
<div class="c" style="direction:rtl"><div class="b" id="fits_end" style="margin-left:25px;margin-right:auto"></div></div>
<div class="c" style="direction:rtl"><div class="b" id="fits_centre" style="margin-left:auto;margin-right:auto"></div></div>
<div class="c" style="direction:rtl"><div class="b" id="fits_both" style="margin-left:275px;margin-right:25px"></div></div>

<div class="c"><div class="b" id="ltr_neg" style="margin-right:500px"></div></div>
<div class="c"><div class="b" id="ltr_auto_start" style="margin-left:auto;margin-right:500px"></div></div>
<div class="c"><div id="ltr_auto_both" style="width:600px;height:12px;margin-left:auto;margin-right:auto"></div></div>
<div class="c"><div class="b" id="ltr_auto_end" style="margin-left:500px;margin-right:auto"></div></div>
<div class="c"><div class="b" id="ltr_centre" style="margin-left:auto;margin-right:auto"></div></div>

</body></html>"##;

#[test]
fn an_over_constrained_rtl_block_recomputes_its_start_margin_without_a_floor() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://example.test/", &fonts, W);
    let dom = page.dom();
    let rects = page.root_box.node_rects(dom);
    let x = |id: &str| -> f32 {
        let sel = format!("#{id}");
        let n = manuk_css::query_selector_all(dom, dom.root(), &sel)
            .first()
            .copied()
            .unwrap_or_else(|| panic!("selector {sel} matched nothing"));
        rects
            .get(&n)
            .unwrap_or_else(|| panic!("no box for {sel}"))
            .x
    };
    // Chrome's own numbers on this exact markup.
    let row = |id: &str, want: f32, why: &str| {
        assert!(
            (x(id) - want).abs() < 1.0,
            "{id}: Chrome says {want}, got {} — {why}",
            x(id)
        );
    };

    // ── THE CONTROL ARM FIRST: LTR is where the clamp was harmless, because under `ltr` the term
    //    that gets ignored is `margin-right`, and `margin-right` does not move the box. A fix that
    //    moves any of these is a trade, not a fix.
    row(
        "ltr_neg",
        0.0,
        "LTR ignores margin-right, so the box stays flush left",
    );
    row(
        "ltr_auto_start",
        0.0,
        "an overflowing auto margin-left is zero, and stays zero under LTR",
    );
    row(
        "ltr_auto_both",
        0.0,
        "…and a 600px box in a 400px container is not centred, it overflows right",
    );
    row(
        "ltr_auto_end",
        500.0,
        "a specified margin-left is honoured under LTR even when it overflows",
    );
    row(
        "ltr_centre",
        150.0,
        "the ordinary centring case must not move: (400 − 100) / 2",
    );

    // ── THE ZERO CROSSING. `zero_crossing` is the last value whose correct answer IS 0, so it
    //    passed under the clamp too — it is here to prove the next row is a floor and not a formula.
    row(
        "zero_crossing",
        0.0,
        "400 − 100 − 300 = 0, and this row passed BEFORE the fix as well",
    );
    row(
        "neg_one",
        -1.0,
        "one pixel past the crossing the resolved margin-left is NEGATIVE; `.max(0.0)` read 0",
    );
    row("neg_fifty", -50.0, "…and it scales: 400 − 100 − 350");
    row("neg_twohundred", -200.0, "…and it scales: 400 − 100 − 500");
    row(
        "ignored_not_offset",
        -200.0,
        "margin-left is IGNORED, not offset — a specified 20px must make no difference at all",
    );

    // ── STEP 1 BEFORE STEP 2: an overflowing `auto` margin becomes zero, and THEN the direction
    //    clause applies. Each of these took a different wrong arm.
    row(
        "auto_start",
        -200.0,
        "auto margin-left → 0, then margin-left is ignored: 400 − 100 − 500",
    );
    row(
        "auto_both",
        -200.0,
        "both autos → 0, then ignored: 400 − 600 − 0",
    );
    row(
        "auto_end",
        300.0,
        "auto margin-right → 0, then margin-left is ignored: 400 − 100 − 0",
    );

    // ── The CSS 2.1 suite's own shape: a zero-width box in a zero-width container, where the
    //    negative resolved margin is the whole point (it pulls an inner border over an outer one).
    row(
        "zero_container",
        -96.0,
        "0 − 0 − 96, the shape margin-right-019.xht draws with",
    );

    // ── ROWS THAT MUST NOT MOVE: RTL boxes that FIT. Without these the fix could be "always
    //    recompute" and the gate could not tell that apart from "recompute when over-constrained".
    row(
        "fits_start",
        275.0,
        "one auto margin, no overflow: it absorbs the remainder",
    );
    row(
        "fits_end",
        25.0,
        "a specified margin-left with an auto margin-right is honoured",
    );
    row(
        "fits_centre",
        150.0,
        "two auto margins with room to spare still centre",
    );
    row(
        "fits_both",
        275.0,
        "an equation that already holds must resolve to the value it holds at",
    );
}
