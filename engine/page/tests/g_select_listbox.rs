//! # G_SELECT_LISTBOX — `<select multiple>` is a sized LIST BOX, and we drew a one-line dropdown
//!
//! A `<select multiple>` or `<select size=N>` is not a dropdown at all: it is a **sized scrolling
//! list box**, and its height comes from its row count rather than from its text. The engine had no
//! branch for either attribute — `form_control_text`'s `"select"` arm returned the selected option
//! and nothing anywhere computed a row count — so every one of them rendered **one line tall**.
//!
//! **A control's height displaces everything after it**, which is what makes this a burndown item
//! rather than a cosmetic one: on this fixture the content below landed **288px too high**. Filter
//! sidebars, admin forms and faceted search are where multi-selects live, and they are exactly the
//! form-heavy pages the CrUX tail is made of.
//!
//! **Every row below is measured on THIS file's fixture**, in headless Chrome and in this engine,
//! before and after — not carried over from a differently-styled probe:
//!
//! ```text
//!   height, border box                    Chrome    before    after
//!     #d0   <select>                        21.0      22       22    control, unmoved
//!     #m4   multiple                        82.8      22       85    4 rows (HTML's default size)
//!     #s4   size=4                          82.8      22       85
//!     #s2   multiple size=2                 42.4      22       44
//!     #f32  multiple size=3   32px         120.2      41      122
//!     #f10  multiple size=3   10px          41.0      15       43
//!     #s1   size=1                          21.0      22       22    control, unmoved
//!     #m1   multiple size=1                 21.0      22       22    control — a DROPDOWN
//!     #one  multiple size=3, ONE option     62.6      22       65    rows, not options
//!     #fix  multiple height:100px          100.0     100      100    control — definite wins
//!   ──────────────────────────────────────────────────────────────────
//!     y of the div BELOW all ten           601.6     313      613
//!     ...its error                             —   -288.6    +11.4
//! ```
//!
//! ## The row height is NOT the font's line box, and that is the whole point
//!
//! Chrome-measured at six font sizes and fitted:
//!
//! ```text
//!   size=3, sans-serif      9px   15.5px    16px     17px    20px     32px
//!     Chrome, border box  37.39    60.78   62.60    66.17   77.00   120.20
//!     (h - 2) / rows      11.80    19.59    20.19    21.39   25.00    39.40
//!     1.2 x size + 1      11.80    19.60    20.20    21.40   25.00    39.40
//! ```
//!
//! A 16px sans-serif **line box** is 18 in Chrome and 18 here — we agree exactly, at four sizes — and
//! a 16px list-box **row** is 20.2. Reusing the line box would look right at 16px and drift
//! everywhere else. The row metric is also **font-family independent** (16px monospace gives the same
//! 62.6) and **immune to `line-height`** (`line-height: 40px` on the select leaves it at 62.6):
//! Chrome forces its own, so a fix derived from our text metrics would be wrong for a reason no
//! single-size fixture would reveal.
//!
//! ## What this gate does NOT assert, and why
//!
//! **Absolute heights carry a ~2px residual** — ours are 85/44/122/43/65 against Chrome's
//! 82.8/42.4/120.2/41/62.6. That is our UA `<select>` chrome (border + padding) being 4px where
//! Chrome's is 2, which is *pre-existing and visible on the dropdown too* (22 vs 21). It is a
//! separate one-line UA-metrics question and is deliberately not smuggled into this tick, so the
//! tolerance here is 3px and the DIFFERENCE rows below — which cancel the chrome entirely — are
//! asserted exactly.
//!
//! **The WIDTH is not asserted here — it is `G_SELECT_WIDTH`'s, and it LANDED at t964.** This gate
//! left it alone deliberately: t958 specified dropping the 17px dropdown-arrow strip for a list box,
//! and measured in isolation that is a **regression** (total width error across the six controls
//! 44.2px → 81.4px), because our width came from the *selected* option where Chrome's comes from the
//! *widest*, so the arrow was silently compensating for the wrong measurement. t964 landed both
//! halves together — and found the plain dropdown was wrong by the same mechanism (62 against
//! Chrome's 76). ⚠ The "~6px term when the option count exceeds the row count" this file used to
//! predict **did not exist**: five options in four rows measures the same 59.36 as five in ten. It
//! was an artefact of comparing against `alpha` instead of `gamma`.
//!
//! ## How this goes RED
//!
//! All four were applied, run and reverted; the reported row is the one that actually failed, not
//! the one that seemed likeliest:
//!
//! - **Delete the `list_h` override in `layout_children`** → the four-rows-minus-two-rows difference
//!   is **0** where Chrome's is 40.4, and `#below` reads 313 against 613. The original defect.
//! - **Use the font's line box (`style.line_height`) as the row height** → that same difference
//!   reads **36** (2 x our 18px line box) against Chrome's 40.4. ⚠ This is the mutation worth
//!   dwelling on: at 16px it is only 4px out over two rows, so a fixture asserting one font size
//!   with any real tolerance would pass it.
//! - **Make `select_list_rows` return the OPTION COUNT instead of the display size** → the
//!   difference is **0** again: `#s4` and `#s2` carry the same five options, so a row model built on
//!   option count makes two differently-sized controls identical.
//! - **Treat `size=1` as a list box** (`rows > 1` weakened to "or `multiple`") → `#m1` grows from 22
//!   to **24.2**, and Chrome says 21.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
body{margin:0;font-family:sans-serif;font-size:16px}
/* ⚠ font-size EXPLICIT on every select, one variable per case. Chrome's UA gives an unstyled
   <select> its own ~13.333px font (an unstyled 4-row list box is 70, not 82.8) and we inherit the
   body's 16px — a real, separate defect that would otherwise contaminate every row of this gate
   with a font-size error and make the row-height law look wrong. Named in the journal, not fixed
   here. */
select{margin:0;font-size:16px}
</style></head><body>
<div><select id="d0"><option>alpha</option><option>beta</option><option>gamma</option><option>delta</option><option>eps</option></select></div>
<div><select id="m4" multiple><option>alpha</option><option>beta</option><option>gamma</option><option>delta</option><option>eps</option></select></div>
<div><select id="s4" size="4"><option>alpha</option><option>beta</option><option>gamma</option><option>delta</option><option>eps</option></select></div>
<div><select id="s2" multiple size="2"><option>alpha</option><option>beta</option><option>gamma</option><option>delta</option><option>eps</option></select></div>
<div><select id="f32" multiple size="3" style="font-size:32px"><option>alpha</option><option>beta</option></select></div>
<div><select id="f10" multiple size="3" style="font-size:10px"><option>alpha</option><option>beta</option></select></div>
<div><select id="s1" size="1"><option>alpha</option><option>beta</option></select></div>
<div><select id="m1" multiple size="1"><option>alpha</option><option>beta</option></select></div>
<div><select id="one" multiple size="3"><option>a</option></select></div>
<div><select id="fix" multiple style="height:100px"><option>alpha</option></select></div>
<div id="below">BELOW</div>
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

/// Chrome's row height, which is what the engine must reproduce: `1.2 x font-size + 1`.
fn row(font_size: f32) -> f32 {
    1.2 * font_size + 1.0
}

#[test]
fn g_select_listbox() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://sel.test/", &fonts, 1200.0);
    let h = |sel: &str| rect_of(&page, sel).height;

    // ── THE DIFFERENCES, asserted EXACTLY: they cancel the element's own border+padding, so they
    //    are Chrome's numbers with nothing of ours left in them.
    assert!(
        (h("#s4") - h("#s2") - 2.0 * row(16.0)).abs() < 0.6,
        "G_SELECT_LISTBOX: four rows minus two rows is {} where Chrome's is {} (2 x 20.2). The row \
         COUNT is not driving the height — this is the `size` attribute being ignored.",
        h("#s4") - h("#s2"),
        2.0 * row(16.0)
    );
    assert!(
        (h("#f32") - 3.0 * row(32.0) - (h("#f10") - 3.0 * row(10.0))).abs() < 0.6,
        "G_SELECT_LISTBOX: the row height does not scale with font-size the way Chrome's does. \
         32px gives {} and 10px gives {}; Chrome's difference is 3 x (1.2 x 32 - 1.2 x 10) = 79.2. \
         A row is 1.2 x font-size + 1 and is NOT the font's line box (18 at 16px against a 20.2 \
         row) — the two are close enough at one size to look right and drift at every other.",
        h("#f32"),
        h("#f10")
    );

    // ── THE ABSOLUTE HEIGHTS, against Chrome, with the ~2px UA-chrome residual named in the module
    //    doc. Before this gate every one of these read 22 (or 41/15 at the other font sizes).
    let abs = |sel: &str, chrome: f32, why: &str| {
        let got = h(sel);
        assert!(
            (got - chrome).abs() < 3.0,
            "G_SELECT_LISTBOX: `{sel}` is {got} where Chrome is {chrome}.\n  {why}"
        );
    };
    abs(
        "#m4",
        82.8,
        "<select multiple> with no `size` is FOUR rows — HTML's default display size.",
    );
    abs(
        "#s4",
        82.8,
        "`size=4` is four rows, with or without `multiple`.",
    );
    abs("#s2", 42.4, "`size=2` is two rows.");
    abs(
        "#f32",
        120.2,
        "three rows at 32px: 3 x (1.2 x 32 + 1) + border.",
    );
    abs(
        "#f10",
        41.0,
        "three rows at 10px: 3 x (1.2 x 10 + 1) + border.",
    );
    abs(
        "#one",
        62.6,
        "THREE ROWS, ONE OPTION — the height is the display SIZE, never the option count. A fix \
         that counts options passes every other row in this file.",
    );

    // ── THE CONTROLS THAT MUST NOT MOVE: a dropdown is still a dropdown.
    assert!(
        (h("#d0") - h("#s1")).abs() < 0.6 && h("#d0") < 30.0,
        "G_SELECT_LISTBOX: the ordinary dropdown is {} and `size=1` is {} — both must stay the \
         one-line control they always were (Chrome: 21 and 21).",
        h("#d0"),
        h("#s1")
    );
    assert!(
        (h("#m1") - h("#d0")).abs() < 0.6,
        "G_SELECT_LISTBOX: `<select multiple size=1>` is {} against the dropdown's {}. \
         Chrome-measured: it is 21, the DROPDOWN's height, not the 22.2 a one-row list box would \
         be — the list-box model applies only above one row.",
        h("#m1"),
        h("#d0")
    );
    assert!(
        (h("#fix") - 100.0).abs() < 0.6,
        "G_SELECT_LISTBOX: an explicit `height:100px` gives {} — a definite height must still win \
         over the intrinsic row count. This was ALREADY exact before the fix, which is what proved \
         the height machinery worked and only the intrinsic number was missing.",
        h("#fix")
    );

    // ── THE HEADLINE: the dy cascade. Ten controls deep, the content below was 288px too high.
    let below = rect_of(&page, "#below").y;
    assert!(
        (below - 601.6).abs() < 20.0,
        "G_SELECT_LISTBOX: the div below ten controls is at y={below} where Chrome puts it at \
         ~601.6. THIS is the burndown term — a control that is 60px short displaces every element \
         after it, and a page with a filter sidebar has several."
    );
}
