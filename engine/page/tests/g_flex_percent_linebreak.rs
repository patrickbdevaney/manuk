//! # G_FLEX_PERCENT_LINEBREAK — a sub-pixel float excess must not break a flex line
//!
//! taffy collects flex items into lines with a bare `>` and no tolerance
//! (`taffy-0.12.1/src/compute/flexbox.rs:930`):
//!
//! ```text
//! line_length += child.hypothetical_outer_size.main(constants.dir) + gap_contribution;
//! line_length > main_axis_available_space && idx != 0
//! ```
//!
//! `width: 66.66666667%` is not representable in binary. As `f32` it resolves against a 1200px row
//! to `800.00004`, its `33.33333333%` sibling to `400.00002`, and the pair sums to a hair over
//! 1200 — which is enough. **The second column started a new flex line and the two columns
//! stacked.** Chrome never sees it: Blink quantises every resolved length to `LayoutUnit` (1/64 px)
//! first, so the same pair is exactly `800 + 400 = 1200` and fits.
//!
//! Those are Bootstrap's literal column widths — `.col-8` ships as `width: 66.66666667%` and
//! `.col-4` as `33.33333333%` — so **a two-column Bootstrap 5 row stacked instead of sitting side
//! by side**, on every page that uses one.
//!
//! Chrome-measured on a 1200px `flex-wrap: wrap` row; `[x y]` of the SECOND item:
//!
//! ```text
//!   width pair                      Chrome x    before        after
//!   50% + 50%                          600     600           unchanged ✓  (exact in binary)
//!   75% + 25%                          900     900           unchanged ✓  (exact in binary)
//!   66.6667% + 33.3333%                800     800           unchanged ✓  (sums UNDER 100)
//!   66.66% + 33.33%                    800     800           unchanged ✓  (sums under 100)
//!   66.66666667% + 33.33333333%        800     0, WRAPPED    800  ✗→✓   ← Bootstrap 5
//!   66.666667% + 33.333333%            800     0, WRAPPED    800  ✗→✓
//!   33.33333333% × 3  (3rd item)       800     0, WRAPPED    800  ✗→✓
//!   70% + 40%  (genuinely too wide)      0, WRAPPED — in BOTH states, and that is asserted
//! ```
//!
//! (x of the SECOND item; "WRAPPED" means it started a new flex line. The y values live in the
//! assertions, measured on the fixture below and on no other.)
//!
//! ## Why the passing rows are load-bearing
//!
//! ⚠ The four `unchanged ✓` rows are not padding — they are what says this fix **did not simply
//! widen the tolerance until everything fits on one line**. `50/50` and `75/25` are exactly
//! representable and were always correct; `66.6667/33.3333` and `66.66/33.33` sum to strictly under
//! 100% and were also always correct. A change that made lines stop breaking in general would keep
//! these green while being wrong, so they cannot catch it alone — but paired with `#wrapb` below,
//! which asserts that a genuinely over-wide pair STILL wraps, they bound the fix from both sides.
//!
//! ## How this goes RED
//!
//! - **Delete the `snap_row_item_percent_widths` call in `solve_subtree`** → `#b8dp` reads
//!   `[0 100]`: the Bootstrap pair stacks onto its own line again. (Verified, not assumed.)
//! - ⚠ **`LAYOUT_UNIT = 1.0` instead of `64.0` does NOT go red — RUN, not assumed.** Whole-pixel
//!   snapping still passes every row here, because this fixture's percentages of 1200px all land on
//!   integers. **The 1/64 grid is chosen because it is Chrome's actual quantum, not because this
//!   gate proves it**, and a gate that implied a red it cannot produce would be worse than one that
//!   says so. A fixture that WOULD separate them needs a container width whose percentage lands
//!   between two 1/64 steps; it is not built here, and this line is the note that it is missing.
//!
//! ## The bound, stated rather than glossed
//!
//! ⚠ This snaps the percentage main-axis widths of a flex row's **direct children**, because that
//! is the only place the containing-block width is known before taffy runs. A flex container nested
//! *inside* a flex item has a content width that taffy itself decides, so its children keep raw
//! `f32` resolution and can still lose a line-break by a sub-pixel. Fixing that needs the
//! quantisation inside taffy's resolver, which is not ours to patch.
//!
//! ⚠ Separately measured and NOT fixed here: Bootstrap **4**'s column shape
//! (`flex: 0 0 66.666667%; max-width: 66.666667%`) comes out `533x20` / `133x20` against Chrome's
//! `800` / `400` — the percentage is applied twice. That is a flex-BASIS defect, a different
//! mechanism in a different place, and it is deliberately left alone by this tick.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
body{margin:0;font:16px/1.25 sans-serif}
.r{display:flex;flex-wrap:wrap;width:1200px}
.h{background:#8cf}.i{background:#fc8}
</style></head><body>
<div class="r"><div class="h" id="h50" style="width:50%">x</div><div class="i" id="b50" style="width:50%">x</div></div>
<div class="r"><div class="h" id="h75" style="width:75%">x</div><div class="i" id="b75" style="width:25%">x</div></div>
<div class="r"><div class="h" id="h4dp" style="width:66.6667%">x</div><div class="i" id="b4dp" style="width:33.3333%">x</div></div>
<div class="r"><div class="h" id="h2dp" style="width:66.66%">x</div><div class="i" id="b2dp" style="width:33.33%">x</div></div>
<div class="r"><div class="h" id="h8dp" style="width:66.66666667%">x</div><div class="i" id="b8dp" style="width:33.33333333%">x</div></div>
<div class="r"><div class="h" id="h6dp" style="width:66.666667%">x</div><div class="i" id="b6dp" style="width:33.333333%">x</div></div>
<div class="r"><div class="h" id="t1" style="width:33.33333333%">x</div><div class="i" id="t2" style="width:33.33333333%">x</div><div class="h" id="t3" style="width:33.33333333%">x</div></div>
<div class="r"><div class="h" id="wrapa" style="width:70%">x</div><div class="i" id="wrapb" style="width:40%">x</div></div>
</body></html>"##;

fn pos_of(page: &manuk_page::Page, sel: &str) -> [f32; 2] {
    let dom = page.dom();
    let n = manuk_css::query_selector_all(dom, dom.root(), sel)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("selector {sel} matched nothing"));
    let r = page
        .root_box
        .node_rects(dom)
        .get(&n)
        .copied()
        .unwrap_or_else(|| panic!("no box for {sel}"));
    [r.x, r.y]
}

fn assert_pos(page: &manuk_page::Page, sel: &str, x: f32, y: f32, why: &str) {
    let p = pos_of(page, sel);
    assert!(
        (p[0] - x).abs() < 1.01 && (p[1] - y).abs() < 1.01,
        "G_FLEX_PERCENT_LINEBREAK: `{sel}` expected [{x} {y}] (MEASURED in headless Chrome on THIS \
         fixture), got [{} {}].\n  {why}",
        p[0],
        p[1]
    );
}

#[test]
fn g_flex_percent_linebreak() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://flx.test/", &fonts, 1400.0);

    // ── THE BUG. Bootstrap's own column widths: the pair sums to 100% but not in binary.
    assert_pos(
        &page,
        "#b8dp",
        800.0,
        80.0,
        "`66.66666667% + 33.33333333%` is Bootstrap 5's `.col-8`/`.col-4`. In f32 the pair sums to a \
         hair OVER the 1200px row, and taffy breaks a flex line on a bare `>` — so this column \
         stacked onto its own line at [0 100]. Chrome quantises to LayoutUnit (1/64px) first, so it \
         is exactly 800+400 and fits",
    );
    assert_pos(
        &page,
        "#b6dp",
        800.0,
        100.0,
        "the same defect at six decimal places — it is the binary representability, not the digit \
         count",
    );
    assert_pos(
        &page,
        "#t3",
        800.0,
        120.0,
        "three `33.33333333%` thirds sum to UNDER 100% in decimal and still overflowed in f32 \
         (each rounds UP), so the third one wrapped",
    );

    // ── WHAT MUST NOT MOVE, and it is what says this is not just a widened tolerance. These pairs
    //    were ALWAYS correct — two exactly representable in binary, two summing to under 100%.
    assert_pos(
        &page,
        "#b50",
        600.0,
        0.0,
        "50%+50% is exact in binary and was always right",
    );
    assert_pos(
        &page,
        "#b75",
        900.0,
        20.0,
        "75%+25% is exact in binary and was always right",
    );
    assert_pos(
        &page,
        "#b4dp",
        800.0,
        40.0,
        "66.6667%+33.3333% sums to strictly under 100% and was always right",
    );
    assert_pos(
        &page,
        "#b2dp",
        800.0,
        60.0,
        "66.66%+33.33% likewise — a fix that stopped breaking lines in general would keep these \
         green, which is why #wrapb below is the one that bounds it from the other side",
    );

    // ── THE OTHER BOUND: a genuinely over-wide pair MUST still wrap. 70% + 40% = 110% of the row,
    //    an overflow of 120px — nowhere near a rounding artefact. If this stops wrapping, the fix
    //    has become a blanket tolerance and flex-wrap is broken.
    assert_pos(
        &page,
        "#wrapb",
        0.0,
        160.0,
        "70%+40% = 110% genuinely does not fit and MUST still break to the next line. This is the \
         assertion that stops the fix from degenerating into 'never wrap'",
    );
}
