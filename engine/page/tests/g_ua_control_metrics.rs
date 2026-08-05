//! # G_UA_CONTROL_METRICS — an unstyled form control's box, against Chrome's UA sheet
//!
//! **A `<button>`'s UA border is `2px`, not `1px`**, and we declared 1px — so every button on every
//! page was **2px short in both axes**. `<button>` is on **55.6% of the burndown corpus**
//! (`docs/loop/CORPUS-CONSTRUCTS.md`, t965), the single most common construct measured, which makes
//! this one declaration against more of the corpus than any other number outstanding.
//!
//! Measured on an unstyled fixture — no author `font-size`, so every control uses the UA font that
//! Chrome computes as `13.3333px Arial`:
//!
//! ```text
//!                                          Chrome        before        after
//!     <button>Search</button>             58.2 x 21     56 x 19      58 x 21
//!     <input type=button value=Go>        33.8 x 21     32 x 19      34 x 21
//!   ── rows that were ALREADY exact, pinned here so a UA edit cannot silently move them ──
//!     <input value=Search>                 205 x 21     205 x 19     205 x 19
//!     <select><option>alpha</option>        55 x 19      56 x 19      56 x 19
//!     <textarea>x</textarea>               182 x 36     182 x 36     182 x 36
//!     <select multiple> (4 rows)          38.6 x 70     39 x 72      39 x 72
//! ```
//!
//! ## The claim this gate exists to stop being re-made
//!
//! Surface audit #37 recorded that *"Chrome's UA gives an unstyled `<select>` its own ~13.333px font
//! and we inherit the body's 16px"*. **That was wrong.** The UA rule
//! (`input, select, textarea, button { font-family: Arial, sans-serif; font-size: 13.333px }`) has
//! been in `stylo_engine.rs`'s `UA_CSS` all along, and the proof is in the table: our unstyled
//! `<select>` is **19**, exactly Chrome's, and our `<textarea>` is **182 × 36**, exactly Chrome's.
//! A font-size error could not leave two controls exact. The audit inferred the cause from a height
//! difference that had a different origin, and the rows above are here so the next reader measures
//! instead of inferring.
//!
//! ## What is deliberately NOT fixed, with its numbers
//!
//! - **A text `<input>` is 19 tall against Chrome's 21** — Chrome gives it `2px inset` as well. Its
//!   **width is already exactly 205**, because a text field's intrinsic width is a `size`-driven
//!   formula with its own intercept; widening the border without retuning that formula trades an
//!   exact width for a corrected height. That is a trade, not a fix.
//! - **A `<select multiple>` is 72 against Chrome's 70** — our select carries 1px of vertical
//!   padding where Chrome's computes `0`, but removing it would take the single-line `<select>` from
//!   an exact 19 to 17, because our dropdown's content height is 15 where Chrome's is 17. Both halves
//!   have to move together and the dropdown's own metric does not follow the list-box row law
//!   (Chrome: 21 at a 16px font, where `1 × (1.2 × 16 + 1) + 2` would be 22.2).
//! - **`outset` vs our `solid`** is the bevel, i.e. paint, not geometry.
//!
//! ## How this goes RED
//!
//! - **Put the button border back to `1px`** → `#b` reads 56 × 19 against 58.2 × 21, and `#ib`
//!   32 × 19 against 33.8 × 21. The original defect.
//! - **Give the border `3px`** → 60 × 23; the assertion is two-sided, not a floor.
//! - **Widen the shared `input, textarea, select` border to 2px** (the trade this gate refuses) →
//!   the first row to fail is `#s`, at **57.6 × 21 against Chrome's 55 × 19** — that rule carries
//!   `<select>` and `<textarea>` too, which is itself the reason the trade is not a one-line change.
//! - **Drop the UA `font-size: 13.333px`** → every row moves at once, `<textarea>` worst (182 × 36 →
//!   the 16px metrics), which is what makes the two already-exact rows a real pin.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
body{margin:0;font-family:sans-serif;font-size:16px}
</style></head><body>
<div><button id="b">Search</button></div>
<div><input id="i" value="Search"></div>
<div><select id="s"><option>alpha</option></select></div>
<div><textarea id="t">x</textarea></div>
<div><input id="ib" type="button" value="Go"></div>
<div><select id="sm" multiple><option>alpha</option><option>beta</option></select></div>
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
fn g_ua_control_metrics() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://ua.test/", &fonts, 1200.0);
    let r = |sel: &str| rect_of(&page, sel);

    let want = |sel: &str, w: f32, h: f32, why: &str| {
        let g = r(sel);
        assert!(
            (g.width - w).abs() < 1.1 && (g.height - h).abs() < 1.1,
            "G_UA_CONTROL_METRICS: `{sel}` is {:.1} x {:.1} where Chrome gives {w} x {h}.\n  {why}",
            g.width,
            g.height
        );
    };

    // ── THE DEFECT: a button's UA border is 2px on all four sides.
    want(
        "#b",
        58.2,
        21.0,
        "Chrome's UA computes `border: 2px outset` on a <button>; we declared 1px, so every button \
         on every page was 2px short in BOTH axes. Reading 56 x 19 is the original defect; reading \
         60 x 23 means the border went to 3px.",
    );
    want(
        "#ib",
        33.8,
        21.0,
        "`<input type=button>` takes the same UA rule as <button> — if this one moved without the \
         <button> row moving, the selector list lost a member.",
    );

    // ── THE ROWS THAT WERE ALREADY EXACT. Pinned because surface audit #37 claimed we inherit the
    //    body's font-size for controls, and these two rows are the measurement that refutes it: a
    //    font-size error could not leave a <select> and a <textarea> exactly Chrome's.
    want(
        "#s",
        55.0,
        19.0,
        "an unstyled <select> is 19 tall in BOTH engines. This is the row that proves the UA font \
         rule (13.333px Arial) is applied — audit #37 inferred that we inherit the body's 16px, and \
         a 16px control could not land on Chrome's height.",
    );
    want(
        "#t",
        182.0,
        36.0,
        "an unstyled <textarea> is 182 x 36 in BOTH engines — the second refutation of the same \
         claim, and the most sensitive one, since a textarea's height is rows x line-height.",
    );

    // ── AND THE TWO KNOWN RESIDUES, asserted AT THEIR CURRENT WRONG VALUES so that a future fix has
    //    to come here and say what it changed. A residue nobody pinned is a residue nobody notices
    //    moving.
    let i = r("#i");
    assert!(
        (i.width - 205.0).abs() < 1.1,
        "G_UA_CONTROL_METRICS: a text <input> is {:.1} wide and Chrome gives exactly 205. Reading \
         207 means the 2px border was applied to text fields as well — which corrects their height \
         (19 -> 21) and BREAKS this width, because a text field's intrinsic width is a size-driven \
         formula with its own intercept. Both have to move together or neither does.",
        i.width
    );
    assert!(
        (i.height - 19.0).abs() < 1.1,
        "G_UA_CONTROL_METRICS: a text <input> is {:.1} tall against Chrome's 21 — a KNOWN, NAMED \
         residue pinned at its wrong value. If this reads 21, the trade above was taken; check the \
         width assertion first.",
        i.height
    );
    let sm = r("#sm");
    assert!(
        (sm.height - 72.0).abs() < 1.1,
        "G_UA_CONTROL_METRICS: a 4-row <select multiple> is {:.1} against Chrome's 70 — a KNOWN, \
         NAMED residue (our select carries 1px of vertical padding where Chrome computes 0). \
         Removing that padding alone takes the single-line <select> from an exact 19 to 17, so the \
         dropdown's content height has to move with it.",
        sm.height
    );
}
