//! # G_MAX_WIDTH_AUTO_MARGIN — a `max-width` clamp re-runs the auto-margin split
//!
//! ```css
//! .container { max-width: 1200px; margin: 0 auto; }
//! ```
//!
//! That is the centred-content rule of the entire modern web — every Bootstrap `.container`, every
//! Tailwind `mx-auto max-w-*`, every blog theme's article column, and the reason a 1200px-wide page
//! has margins at all. **It rendered flush left.**
//!
//! CSS 2.1 §10.4: when the used width violates `max-width`, the §10.3.3 rules are *applied again*
//! with the constraint as the computed width — and §10.3.3 is precisely where a pair of `auto`
//! margins splits the remainder. We did the first half (the clamp) and skipped the second: the
//! auto-margin block was guarded on `s.width != Dim::Auto || s.width_keyword.is_some()`, i.e. on
//! whether the AUTHOR wrote a `width`. For `max-width: 1200px; margin: 0 auto` they did not, so the
//! box became definite and the margins never learned about it.
//!
//! ```text
//!                                                    Chrome   before   after
//!   max-width:400px; margin:auto        in 800px      200        0      200   ✗→✓
//!   max-width:400px; margin:0 auto      in 800px      200        0      200   ✗→✓
//!   max-width:400px; margin-left:auto   in 800px      400        0      400   ✗→✓
//!   …inside a 48px-padded parent                      200       48      200   ✗→✓
//!   width:400px;     margin:0 auto      in 800px      200      200      200   ✓ always right
//!   max-width:400px; NO auto margin                     0        0        0   ✓ must not move
//!   max-width:1000px (NOT binding); margin:0 auto       0        0        0   ✓ must not move
//!   min-width:600px; width:100px; margin:0 auto       100      100      100   ✓ always right
//! ```
//!
//! ⚠ **The `min-width` half of the same spec sentence looked fine, and the reason is instructive:**
//! a clamp *upward* is only observable when there is an explicit `width` (a `width:auto` box already
//! fills its container, so `min-width` never binds), and an explicit width always took the guard's
//! first term. **One rule, two constraints, and only the constraint that needed no help worked** —
//! so no test and no site could distinguish "we implement §10.4's re-run" from "we don't".
//!
//! ## How this goes RED
//!
//! - **Drop `inline_constraint_violated` from the guard** → `#m1`, `#m2`, `#m6` and `#m7` snap back
//!   to the left edge (0, 0, 0, 48) while `#m3` and `#m8` — the explicit-width cases — still pass.
//!   That split is the whole point: a gate built only from `width:Npx; margin:0 auto` is green
//!   against the defect.
//! - **Widen it to run the split unconditionally** → nothing here fails, and that is stated rather
//!   than implied: for a `width:auto` box that was NOT clamped, `leftover` is already 0, so the
//!   centring is a no-op. The narrow guard is a statement of INTENT (only a real constraint
//!   violation re-runs §10.3.3), not a behaviour this fixture can prove necessary. `#m4` and `#m5`
//!   pin the cases that would break if the *clamp* stopped being required.

use manuk_text::FontContext;

/// `line-height:1.7` on the body is deliberate — it is 255md.com's own value, and it must not reach
/// any of these boxes' widths. Heights are fixed so the fixture is font-independent.
const HTML: &str = r##"<!doctype html><html><head><style>
body{margin:0;font:16px/1.7 sans-serif}
.outer{width:800px}
</style></head><body>
<div class="outer"><div id="m1" style="max-width:400px;margin:auto;height:10px"></div></div>
<div class="outer"><div id="m2" style="max-width:400px;margin:0 auto;height:10px"></div></div>
<div class="outer"><div id="m3" style="width:400px;margin:0 auto;height:10px"></div></div>
<div class="outer"><div id="m4" style="max-width:400px;height:10px"></div></div>
<div class="outer"><div id="m5" style="max-width:1000px;margin:0 auto;height:10px"></div></div>
<div class="outer"><div id="m6" style="max-width:400px;margin-left:auto;height:10px"></div></div>
<div class="outer" style="padding:0 48px;box-sizing:border-box"><div id="m7" style="max-width:400px;margin:auto;height:10px"></div></div>
<div class="outer"><div id="m8" style="min-width:600px;width:100px;margin:0 auto;height:10px"></div></div>
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

fn assert_box(page: &manuk_page::Page, sel: &str, x: f32, w: f32, why: &str) {
    let r = rect_of(page, sel);
    assert!(
        (r.x - x).abs() < 1.01 && (r.width - w).abs() < 1.01,
        "G_MAX_WIDTH_AUTO_MARGIN: `{sel}` expected x={x} w={w} (MEASURED in headless Chrome on THIS \
         fixture), got x={} w={}.\n  {why}",
        r.x,
        r.width
    );
}

#[test]
fn g_max_width_auto_margin() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://mw.test/", &fonts, 1200.0);

    // ── THE BUG: a max-width clamp makes the width definite, and §10.4 says redo §10.3.3.
    assert_box(
        &page,
        "#m1",
        200.0,
        400.0,
        "`max-width:400px; margin:auto` in 800px — (800−400)/2. This is `.container` on most of the \
         web and it was rendering at x=0",
    );
    assert_box(
        &page,
        "#m2",
        200.0,
        400.0,
        "the `margin:0 auto` spelling, which is the one authors actually write",
    );
    assert_box(
        &page,
        "#m6",
        400.0,
        400.0,
        "ONE auto margin pushes the box fully right (800−400), it does not centre — so the fix is \
         the §10.3.3 split, not a `centre when clamped` special case",
    );
    assert_box(
        &page,
        "#m7",
        200.0,
        400.0,
        "the containing block's PADDING is the frame: 48 + (704−400)/2. This is 255md.com's \
         `.contact-form` inside its 48px-padded card, which sat flush at 48",
    );

    // ── WHAT WAS ALWAYS RIGHT, and must stay so — the cases a narrower gate would have covered
    //    alone, leaving the defect green.
    assert_box(
        &page,
        "#m3",
        200.0,
        400.0,
        "an EXPLICIT width has always centred; it takes the guard's first term",
    );
    assert_box(
        &page,
        "#m8",
        100.0,
        600.0,
        "`min-width` clamping UP with an explicit width: (800−600)/2. The other half of §10.4's \
         sentence, and the half that never looked broken",
    );

    // ── WHAT MUST NOT MOVE: a clamp alone does not centre anything.
    assert_box(
        &page,
        "#m4",
        0.0,
        400.0,
        "`max-width` with NO auto margin stays at the left edge — a fix that centred every clamped \
         box would pass everything above and fail here",
    );
    assert_box(
        &page,
        "#m5",
        0.0,
        800.0,
        "a `max-width` that does NOT bind leaves the box full-width at 0: no constraint violation, \
         so no re-run, so no centring",
    );
}
