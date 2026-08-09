//! # G_DIR_ATTR_LOGICAL — the `dir` attribute is a CASCADE INPUT, and the shipping sheet lacked it
//!
//! Every CSS **logical** property (`margin-inline-start`, `inset-inline-end`, the `margin-inline`
//! shorthand, …) is resolved to a physical one *inside Stylo's cascade*, against Stylo's own
//! `WritingMode` — which is computed from the `direction` and `writing-mode` declarations it saw.
//! Manuk implemented `dir="rtl"` only in `MinimalCascade`'s presentational hints and then
//! **recovered** the value onto the computed style *after* Stylo had finished
//! (`cs.direction = m.direction` in `stylo_engine.rs`). That is enough for everything **we** resolve
//! from `direction` — the bidi inline reorder, `text-align: start/end` — and it is not enough for
//! the one thing **Stylo** resolves from it. With no `direction` declaration in any sheet, Stylo's
//! writing mode was LTR on every element of every page, so `margin-inline-start` on an RTL page
//! became `margin-LEFT`.
//!
//! ## The measurement that separates the two halves
//!
//! 28 rows against headless Chrome, the same eight logical properties declared twice — once with
//! the `dir` **attribute** and once with a `direction: rtl` **declaration** (`/tmp/bat-rtl2.html`):
//!
//! ```text
//!   row                              dir="rtl" ATTRIBUTE        direction:rtl STYLESHEET
//!   margin-inline-start:25px       Chrome 275  ours 300  ✗      Chrome 275  ours 275  ✓
//!   margin-inline-end:25px         Chrome 300  ours 275  ✗      Chrome 300  ours 300  ✓
//!   inset-inline-start:25px (abs)  Chrome 275  ours  25  ✗      Chrome 275  ours 275  ✓
//!   margin-inline:25px 60px        Chrome 275  ours 240  ✗      Chrome 275  ours 275  ✓
//! ```
//!
//! ⚠⚠⚠ **THE STYLESHEET COLUMN WAS ALREADY PERFECT.** That is the whole finding: Stylo's logical
//! resolution works and always did — it was never told the direction. A battery that had only
//! tested `direction: rtl` (the spelling a CSS test writes) would have reported the area **clean**,
//! and the spelling the RTL web actually uses is the attribute: `<html dir="rtl">` is how
//! essentially every Arabic, Hebrew, Persian and Urdu site declares itself.
//!
//! ⚠⚠ It is also why the defect survived a *correct* RTL implementation. A 61-row RTL battery run
//! the same hour scored **58/61** — inline reorder, alignment, wrapping, Arabic text, mirrored
//! margins and nested `dir` islands were all Chrome-exact — because every one of those reads
//! `ComputedStyle::direction`, which the recovery had already fixed. Only the properties that
//! Stylo itself must map were wrong.
//!
//! ## How this goes RED
//!
//! - Delete `[dir="rtl" i] { direction: rtl }` from `UA_CSS` → the four `attr_*` rows report the
//!   LTR answers (300 / 275 / 25 / 240) while every `sheet_*` row still passes. That pair is what
//!   distinguishes "logical properties are broken" from "the attribute is invisible".
//! - Delete `[dir="ltr" i]` instead → `child_ltr` alone reports 0 against Chrome's 25, because a
//!   `dir="ltr"` island can no longer undo an RTL ancestor. ⚠ `attr_then_style_ltr` does **not**
//!   move: its `style="direction:ltr"` is an author declaration and carries the row on its own.
//!   Both rules are therefore load-bearing, and each is proven by a different row.
//! - Drop the ` i` flag → **the gate stays GREEN, and that is a reading, not a gap.** `dir` is on
//!   HTML's list of attributes whose values selectors match ASCII-case-insensitively, and Stylo
//!   implements the list, so `[dir="rtl"]` already matches `dir="RTL"`. Verified from both sides on
//!   the same build: a NON-listed attribute stays case-sensitive (`[data-x="abc"]` does not match
//!   `data-x="ABC"` — Chrome agrees), so the engine is not blanket-insensitive. The flag is kept as
//!   documentation of the intent; `attr_uppercase` is a real Chrome-checked row but it is **not**
//!   what makes the flag necessary, because nothing does.
//! - Give the rules the author origin → `style_wins` fails, because a UA rule must lose to the
//!   page's own `direction`.
//!
//! ⚠ Run with `--features stylo,spidermonkey`. Without them this loads the MinimalCascade path,
//! which resolves logical properties elsewhere and cannot see the subject at all.
//!
//! ONE `#[test]` per file — `PageContext` is per-process (see `g_caption_paint.rs`).

use manuk_text::FontContext;

const W: f32 = 800.0;

/// Every box is a fixed-width block in a fixed-width container, so no number here depends on a
/// font: a 100px block in a 400px container sits at 0 under LTR and at 300 under RTL, and each
/// logical property moves it by exactly its own 25px.
const HTML: &str = r##"<!doctype html><html><head><style>
body { margin: 0; font: 16px/20px monospace }
.c { width: 400px; height: 26px }
.b { width: 100px; height: 12px }
.rtl { direction: rtl }
</style></head><body>

<div class="c"><div class="b" id="ltr_mis" style="margin-inline-start:25px"></div></div>
<div class="c"><div class="b" id="ltr_mie" style="margin-inline-end:25px"></div></div>
<div class="c" style="position:relative"><div class="b" id="ltr_iis" style="position:absolute;inset-inline-start:25px"></div></div>
<div class="c"><div class="b" id="ltr_mi" style="margin-inline:25px 60px"></div></div>

<div class="c rtl"><div class="b" id="sheet_mis" style="margin-inline-start:25px"></div></div>
<div class="c rtl"><div class="b" id="sheet_mie" style="margin-inline-end:25px"></div></div>
<div class="c rtl" style="position:relative"><div class="b" id="sheet_iis" style="position:absolute;inset-inline-start:25px"></div></div>
<div class="c rtl"><div class="b" id="sheet_mi" style="margin-inline:25px 60px"></div></div>

<div class="c" dir="rtl"><div class="b" id="attr_mis" style="margin-inline-start:25px"></div></div>
<div class="c" dir="rtl"><div class="b" id="attr_mie" style="margin-inline-end:25px"></div></div>
<div class="c" dir="rtl" style="position:relative"><div class="b" id="attr_iis" style="position:absolute;inset-inline-start:25px"></div></div>
<div class="c" dir="rtl"><div class="b" id="attr_mi" style="margin-inline:25px 60px"></div></div>

<div class="c" dir="RTL"><div class="b" id="attr_uppercase" style="margin-inline-start:25px"></div></div>
<div class="c rtl" style="direction:ltr"><div class="b" id="style_wins" style="margin-inline-start:25px"></div></div>
<div class="c" dir="rtl" style="direction:ltr"><div class="b" id="attr_then_style_ltr" style="margin-inline-start:25px"></div></div>
<div class="c" dir="rtl"><div dir="ltr" style="height:12px"><div class="b" id="child_ltr" style="margin-inline-start:25px"></div></div></div>

</body></html>"##;

#[test]
fn the_dir_attribute_reaches_stylos_writing_mode() {
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
    // Chrome's own numbers, so a row that disagrees names a divergence and not a preference.
    let row = |id: &str, want: f32, why: &str| {
        assert!(
            (x(id) - want).abs() < 1.0,
            "{id}: Chrome says {want}, got {} — {why}",
            x(id)
        );
    };

    // ── NEGATIVE ROWS FIRST. LTR is the direction the broken writing mode already assumed, so
    //    every one of these was passing before the fix and a fix that moves one is a trade.
    row(
        "ltr_mis",
        25.0,
        "margin-inline-start is margin-left under LTR",
    );
    row(
        "ltr_mie",
        0.0,
        "margin-inline-end is margin-right: the box does not move",
    );
    row("ltr_iis", 25.0, "inset-inline-start is left");
    row(
        "ltr_mi",
        25.0,
        "the shorthand's FIRST value is the start side",
    );

    // ── THE CONTROL ARM. `direction: rtl` as a DECLARATION was already Chrome-exact, and saying so
    //    here is what makes the four rows below an attribute defect rather than a property defect.
    row(
        "sheet_mis",
        275.0,
        "start is the RIGHT edge: 400 − 100 − 25",
    );
    row(
        "sheet_mie",
        300.0,
        "end is the left side, so the box stays at the right edge",
    );
    row(
        "sheet_iis",
        275.0,
        "inset-inline-start is `right: 25px` under RTL",
    );
    row(
        "sheet_mi",
        275.0,
        "the shorthand's first value is still the start side",
    );

    // ── THE DEFECT. Identical rows, direction declared with the attribute the RTL web uses.
    row(
        "attr_mis",
        275.0,
        "`dir=\"rtl\"` must reach Stylo's writing mode; it read 300 because the attribute was \
         invisible to the shipping cascade and every logical property resolved LTR",
    );
    row(
        "attr_mie",
        300.0,
        "…and the end side, which was inverted to 275",
    );
    row(
        "attr_iis",
        275.0,
        "…and an absolutely positioned box's logical inset, which was resolved as `left: 25px`",
    );
    row(
        "attr_mi",
        275.0,
        "…and the shorthand, which was resolving BOTH values to the wrong side",
    );

    // ── HTML matches the attribute value ASCII-case-insensitively.
    //    ⚠ It passes WITHOUT the ` i` flag — `dir` is on HTML's case-insensitive-matching list and
    //    Stylo honours it. Kept because the row is Chrome-checked, not because the flag needs it.
    row(
        "attr_uppercase",
        275.0,
        "`dir=\"RTL\"` is the same attribute",
    );

    // ── The UA origin, proven from the losing side: the page's own `direction` must still win, and
    //    a `dir=\"ltr\"` island must undo an RTL ancestor. Without these three the fix could be a
    //    blunt \"treat every dir attribute as final\" and the gate could not tell.
    row(
        "style_wins",
        25.0,
        "an author `direction:ltr` outranks a UA `[dir]` rule",
    );
    row(
        "attr_then_style_ltr",
        25.0,
        "…including against the element's own dir attribute",
    );
    row(
        "child_ltr",
        25.0,
        "a dir=\"ltr\" child re-establishes LTR inside an RTL parent",
    );
}
