//! # G_LETTER_SPACING_RESOLVES_ITS_UNIT — a unit it does not know is not `normal`
//!
//! ⚠⚠⚠ **`letter-spacing` DROPPED EVERY FONT-RELATIVE UNIT EXCEPT `em`, AND A DROPPED VALUE HERE
//! IS INDISTINGUISHABLE FROM `normal`.** The property was one of the handful `stylo_engine.rs`
//! recovers from `MinimalCascade` after Stylo has run, and `MinimalCascade` resolves the length
//! through `values::dimension_to_px` — which maps **`"em" | "rem"` to the SAME arm** and returns
//! `None` for anything else. `None` becomes `0.0` here, so `letter-spacing: .15ch` rendered exactly
//! like `letter-spacing: normal`: no error, no fallback, no trace.
//!
//! Every number below is CAPTURED from `google-chrome --headless --hide-scrollbars
//! --window-size=1200,800`, `font-size: 20px` monospace, the string `Hamburgefonstiv 0123` in an
//! `inline-block` with `white-space: pre`, root font-size 16px.
//!
//! ```text
//!                                    Chrome used   Chrome box   before   after
//!   #n    normal                        —             240.83     241      241   CTRL
//!   #px   2px                           2px           280.83     281      281   CTRL
//!   #neg  -1px                         -1px           220.83     221      221   CTRL
//!   #em   .1em  (font-size longhand)    2px           280.83     281      281
//!   #emf  .1em  (after `font:` shorthand) 2px         280.83     273      281   KEY
//!   #ch   .15ch                         1.80615px     276.95     241      277   KEY
//!   #rem  .1rem (root 16px)             1.6px         272.83     281      273   KEY
//!   #big  .1em at font-size 40px        4px           561.64     562      562
//! ```
//!
//! ⭐ **THE THREE `KEY` ROWS ARE THREE DIFFERENT WAYS TO GET THE BASIS WRONG, AND ONE FIX ANSWERS
//! ALL OF THEM.** `#ch` is a unit the resolver had never heard of. `#rem` is a unit it had heard of
//! and aliased to `em`, so it used the ELEMENT's 20px where the root's 16px was meant — the one row
//! whose old value was too LARGE, which is why "we drop spacing" was not the whole story. `#emf` is
//! the right unit against the wrong number: the `font:` shorthand had not established the basis in
//! that cascade, so `.1em` resolved against the INHERITED 16px and produced 1.6px where Chrome
//! produces 2px. A resolver that owns the unit but not the font context gets this class wrong
//! forever.
//!
//! The fix is to stop resolving it twice: `stylo_map` now takes Stylo's own computed
//! `LetterSpacing`, which is a `LengthPercentage` Stylo has already resolved against the correct
//! bases — the same machinery whose `width: 40ch` and `max-width: 50ch` are Chrome-exact today.
//!
//! ⚠ **THIS IS THE FOURTH PROPERTY CAUGHT BEING RECOVERED FROM A CASCADE THAT COMPUTES IT IN A
//! DIFFERENT CONTEXT** — t923 (`sup`'s `vertical-align`), t1366 (`<td>`'s), t1368 (`align`'s), and
//! now this. **A recovery is a second implementation, and a second implementation of a UNIT is a
//! second answer.** `#emf` and `#rem` are the rows that catch a re-introduction.
//!
//! MEASURED, NOT FIXED, AND DELIBERATELY NOT ASSERTED HERE:
//!
//! * **`ex` is applied but on the wrong basis.** `letter-spacing: .2ex` at 20px monospace is 2.2px
//!   in Chrome (the face's real x-height) and 2.0px here, because Stylo's servo build resolves `ex`
//!   as a flat `0.5em` with no font metrics wired. Before this tick it was DROPPED (241); it is now
//!   281 against Chrome's 284.83 — strictly closer, still wrong, and pinning 281 as correct is
//!   exactly the wrong-answer-of-the-right-type this file refuses elsewhere.
//! * **`word-spacing` is inert in LAYOUT, and that is a separate defect.** Its value now comes from
//!   Stylo alongside `letter-spacing`, but `word-spacing: 10px` on `a b c d` measures 114.30 in
//!   Chrome and 84 here — the same 84 as `normal`. `manuk-layout` reads `style.word_spacing`
//!   (lib.rs ~13414) yet the advance never changes, so the gap is downstream of the cascade and is
//!   not what this tick fixed. Named rather than quietly bundled.
use manuk_text::FontContext;

const HTML: &str = r##"<!DOCTYPE html><html><head><meta charset="utf-8"><style>
 html{font-size:16px}
 body{margin:0;font-family:monospace}
 span{display:inline-block;white-space:pre;font-family:monospace}
 #n{font-size:20px;letter-spacing:normal}
 #px{font-size:20px;letter-spacing:2px}
 #neg{font-size:20px;letter-spacing:-1px}
 #em{font-size:20px;letter-spacing:.1em}
 #emf{font:20px/1.2 monospace;letter-spacing:.1em}
 #ch{font-size:20px;letter-spacing:.15ch}
 #rem{font-size:20px;letter-spacing:.1rem}
 #big{font-size:40px;letter-spacing:.1em}
</style></head><body>
<div><span id="n">Hamburgefonstiv 0123</span></div>
<div><span id="px">Hamburgefonstiv 0123</span></div>
<div><span id="neg">Hamburgefonstiv 0123</span></div>
<div><span id="em">Hamburgefonstiv 0123</span></div>
<div><span id="emf">Hamburgefonstiv 0123</span></div>
<div><span id="ch">Hamburgefonstiv 0123</span></div>
<div><span id="rem">Hamburgefonstiv 0123</span></div>
<div><span id="big">Hamburgefonstiv 0123</span></div>
</body></html>
"##;

fn width(page: &manuk_page::Page, sel: &str) -> f32 {
    let dom = page.dom();
    let n = manuk_css::query_selector_all(dom, dom.root(), sel)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("selector {sel} matched nothing"));
    page.root_box
        .node_rects(dom)
        .get(&n)
        .copied()
        .unwrap_or_else(|| panic!("no box for {sel}"))
        .width
}

/// The whole tick is an ADVANCE, so the measured width of one fixed string is what every row
/// asserts. The string is 20 characters, so a 1px error in the resolved spacing is a 20px error
/// here — the tolerance cannot hide a wrong basis.
fn w(page: &manuk_page::Page, sel: &str, want: f32, why: &str) {
    let got = width(page, sel);
    assert!(
        (got - want).abs() < 1.01,
        "G_LETTER_SPACING_RESOLVES_ITS_UNIT: `{sel}` expected w={want} (CAPTURED from \
         `google-chrome --headless --hide-scrollbars --window-size=1200,800`), got w={got} — {why}"
    );
}

#[test]
fn g_letter_spacing_resolves_its_unit() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://letterspacing.test/", &fonts, 1200.0);

    // ── THE CONTROLS: the units that always worked must keep working, including a NEGATIVE one.
    w(
        &page,
        "#n",
        240.83,
        "CONTROL: `normal` is zero spacing — this is the number every DROPPED unit used to produce, \
         which is why a dropped unit was invisible",
    );
    w(&page, "#px", 280.83, "CONTROL: an absolute px length");
    w(
        &page,
        "#neg",
        220.83,
        "CONTROL: spacing is signed — a negative value must still tighten",
    );

    // ── THE SUBJECT: three ways to get the basis wrong.
    w(
        &page,
        "#em",
        280.83,
        "`.1em` against a longhand `font-size:20px` is 2px",
    );
    w(
        &page,
        "#emf",
        280.83,
        "⭐ the same `.1em` after a `font:` SHORTHAND — the shorthand must establish the basis, and \
         resolving against the inherited 16px gives 273",
    );
    w(
        &page,
        "#ch",
        276.95,
        "⭐ `ch` is the advance of `0` in this element's font, not an unknown unit worth zero",
    );
    w(
        &page,
        "#rem",
        272.83,
        "⭐ `rem` is the ROOT's 16px, not this element's 20px — the row whose old value was too \
         LARGE, so `we drop spacing` was never the whole story",
    );
    w(
        &page,
        "#big",
        561.64,
        "and the basis tracks the element's own font-size: `.1em` at 40px is 4px",
    );
}
