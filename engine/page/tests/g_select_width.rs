//! # G_SELECT_WIDTH — a `<select>` is as wide as its WIDEST option, not its selected one
//!
//! The control renders **one** option and **reserves room for all of them** — open a dropdown and
//! every entry must fit without the box moving. We sized every `<select>` to the option it happened
//! to be displaying, so a country picker showing "Chad" was as wide as "Chad".
//!
//! **This is not a multi-select bug.** The plain dropdown was wrong by the same mechanism: on the
//! fixture below it measured **62 against Chrome's 76**, and `<select>` is on approximately every
//! form on the web.
//!
//! ```text
//!   16px sans-serif      alpha 39.16   gamma 53.36   quickbrownfox 102.28
//!
//!                                                  Chrome   before   after
//!     list box  alpha..eps          size=4          59.36     45      59
//!     list box  alpha..eps          size=10         59.36     45      59    scrolling is irrelevant
//!     list box  alpha+quickbrownfox size=2         108.28     45     108
//!     list box  alpha+quickbrownfox size=5         108.28     45     108
//!     list box  ONE option "a"      size=3          14.91     15      15    control, unmoved
//!     list box  width:300px         size=3         300.00    300     300    control, definite wins
//!     DROPDOWN  alpha+quickbrownfox                125.00     62     125    ← not a multi-select bug
//! ```
//!
//! The rule is one line: **`widest option + 6`, plus a 17px arrow strip when it is a dropdown.** The
//! `6` is the control's own border and option padding, which our UA already contributes, so the
//! engine's missing term is exactly `widest − shown`. It is **independent of the row count and of
//! whether the list scrolls** — five options in four rows measures the same 59.36 as five options in
//! ten rows, so there is no scrollbar reservation to model.
//!
//! ## Why both halves had to land together
//!
//! t958 specified dropping the arrow for a list box; t963 built it, measured it, and **reverted it**,
//! because in isolation it triples the width error (44.2px → 81.4px across six controls). The arrow
//! is real physics; it was also silently compensating for our sizing to the wrong string, so removing
//! it alone trades a right answer for a wrong reason. This gate carries both halves and the two
//! controls that fail if either one is applied without the other.
//!
//! ⚠ **The widest-option reserve survives `appearance: none`; the arrow does not.** That declaration
//! takes the native widget off the control — it does not make the control narrower than the options
//! it has to hold.
//!
//! ⚠ **NAMED RESIDUE, measured and not modelled: `<select multiple size=1>` is 95 in Chrome** where
//! every formula here predicts ~62, and we render 62. Nine of the ten controls on t963's fixture are
//! now exact and this one is not. It is a corner (`multiple` with an explicit one-row size) with no
//! real-web population worth a model, and inventing one to cover a single unexplained number is how
//! a constant gets fitted to noise. Recorded so it is not rediscovered as a regression.
//!
//! ## How this goes RED
//!
//! - **Measure the SELECTED option instead of the widest** (drop the `widest - shown` reserve) → the
//!   dropdown reads 62 against 125 and every list box reads 45 against 59.36. The original defect.
//! - **Drop the arrow for every select, not only list boxes** → `#w8` reads 108 against 125 while
//!   every list-box row still passes.
//! - **Keep the arrow for list boxes** → `#w2` reads 76 against 59.36 while `#w8` still passes.
//! - **Add the reserve without subtracting the shown text** → every row double-counts: `#w2` reads
//!   ~98 against 59.36.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
body{margin:0;font-family:sans-serif;font-size:16px}
select{margin:0;font-size:16px}
span{font-size:16px;font-family:sans-serif}
</style></head><body>
<div><span id="tA">alpha</span> <span id="tG">gamma</span> <span id="tQ">quickbrownfox</span></div>
<div><select id="w1" size="3"><option>a</option></select></div>
<div><select id="w2" size="3"><option>alpha</option><option>gamma</option></select></div>
<div><select id="w3" size="10"><option>alpha</option><option>beta</option><option>gamma</option><option>delta</option><option>eps</option></select></div>
<div><select id="w4" size="4"><option>alpha</option><option>beta</option><option>gamma</option><option>delta</option><option>eps</option></select></div>
<div><select id="w5" size="2"><option>alpha</option><option>quickbrownfox</option></select></div>
<div><select id="w6" size="5"><option>alpha</option><option>quickbrownfox</option></select></div>
<div><select id="w7" size="3" style="width:300px"><option>alpha</option></select></div>
<div><select id="w8"><option>alpha</option><option>quickbrownfox</option></select></div>
<div><select id="w9" style="appearance:none"><option>alpha</option><option>quickbrownfox</option></select></div>
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
fn g_select_width() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://selw.test/", &fonts, 1200.0);
    let w = |sel: &str| rect_of(&page, sel).width;

    // ── The fixture's own text widths, so every expectation below is stated against THIS box's font
    //    rather than against a px literal from Chrome's. Chrome measures these at 39.16 / 53.36 /
    //    102.28, and we are within a pixel of all three — which is what makes the absolute
    //    comparisons that follow meaningful.
    let alpha = w("#tA");
    let gamma = w("#tG");
    let quick = w("#tQ");
    assert!(
        (alpha - 39.16).abs() < 1.5 && (gamma - 53.36).abs() < 1.5 && (quick - 102.28).abs() < 1.5,
        "G_SELECT_WIDTH: this box's sans-serif measures alpha={alpha} gamma={gamma} \
         quickbrownfox={quick} against Chrome's 39.16/53.36/102.28. The font differs enough that \
         nothing below is a statement about <select>."
    );
    // The control's own border + option padding, which our UA already contributes.
    const CHROME: f32 = 6.0;
    const ARROW: f32 = 17.0;

    let want = |sel: &str, text: f32, arrow: bool, why: &str| {
        let got = w(sel);
        let expect = text + CHROME + if arrow { ARROW } else { 0.0 };
        assert!(
            (got - expect).abs() < 1.6,
            "G_SELECT_WIDTH: `{sel}` is {got} where the widest option gives {expect}.\n  {why}"
        );
    };

    // ── THE DEFECT: a list box sized to the option it displays, not the widest one it must hold.
    want(
        "#w2",
        gamma,
        false,
        "the widest option is `gamma`, and the control renders `alpha`. Reading ~45 means the \
         intrinsic width came from the SELECTED option — the control would clip every entry longer \
         than the one it happens to show.",
    );
    want(
        "#w5",
        quick,
        false,
        "the widest option is `quickbrownfox` and the control renders `alpha` — the biggest version \
         of the same error, 108 against 45.",
    );

    // ── THE ROW COUNT AND SCROLLING ARE IRRELEVANT, which is the discriminator against a width
    //    model that tries to reserve a scrollbar.
    assert!(
        (w("#w3") - w("#w4")).abs() < 0.6,
        "G_SELECT_WIDTH: five options in TEN rows is {} and in FOUR rows is {} — Chrome measures \
         both at 59.36. A list box reserves nothing for scrolling, so a width model that adds a \
         scrollbar term when the options overflow is fitting a constant to noise.",
        w("#w3"),
        w("#w4")
    );
    assert!(
        (w("#w5") - w("#w6")).abs() < 0.6,
        "G_SELECT_WIDTH: the same two options in 2 rows ({}) and 5 rows ({}) must measure the \
         same — the width is a function of the OPTIONS, the height of the ROWS.",
        w("#w5"),
        w("#w6")
    );

    // ── NOT A MULTI-SELECT BUG: the plain dropdown was wrong by the same mechanism, and it is on
    //    approximately every form on the web.
    want(
        "#w8",
        quick,
        true,
        "a DROPDOWN also sizes to its widest option, PLUS the 17px arrow strip. Reading 62 is the \
         original defect on the commonest control on the web; reading 108 means the arrow was \
         dropped for a control that draws one.",
    );

    // ── THE ARROW IS THE ONLY CONDITIONAL HALF. A list box does not draw one; `appearance:none`
    //    removes it from a dropdown — and neither makes the control narrower than its options.
    want(
        "#w9",
        quick,
        false,
        "`appearance:none` takes the WIDGET off the control. It does not take the OPTIONS off it: \
         the widest-option reserve must survive, or every restyled design-system select clips its \
         own entries.",
    );

    // ── CONTROLS THAT MUST NOT MOVE.
    assert!(
        (w("#w1") - 14.91).abs() < 1.6,
        "G_SELECT_WIDTH: the one-option list box is {} against Chrome's 14.91. With a single \
         option `widest` and `shown` are the same string, so the reserve is exactly zero and this \
         row must be byte-identical to what it was before the fix.",
        w("#w1")
    );
    assert!(
        (w("#w7") - 300.0).abs() < 0.6,
        "G_SELECT_WIDTH: an explicit `width:300px` gives {} — a definite width must still win over \
         any intrinsic reserve.",
        w("#w7")
    );
}
