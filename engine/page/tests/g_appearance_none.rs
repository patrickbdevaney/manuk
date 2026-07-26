//! **G_APPEARANCE_NONE — what `appearance: none` is actually worth to THIS engine.**
//!
//! Surface audit #32 ranked `appearance` the single highest-usage unmapped capability on the board:
//! **`appearance` 49.3% of page loads, `CSSValueAppearanceNone` 60.5%** (Blink use counters,
//! 2026-07-24), and `stylo_map.rs` reads it nowhere. On the usage number alone it was the obvious
//! next tick.
//!
//! **Measured first, and the usage number does not transfer.** In a real browser `appearance: none`
//! removes *native OS widget rendering* — the drawn dropdown arrow, the checkbox glyph, the range
//! track — which CSS cannot otherwise override, and that is the entire reason authors write it. **This
//! engine has no native widget rendering.** Our form controls are drawn by ordinary UA *CSS* at
//! lowest specificity (`engine/css/src/stylo_engine.rs`: `input, textarea, select { border: 1px solid
//! #767676; background-color: #fff; padding: 1px 2px }`), so an author rule already beats them.
//!
//! ```text
//! #plain     (nothing)                          border=1  bg=#fff  padLeft=2
//! #styled    (appearance:none)                  border=1  bg=#fff  padLeft=2   ← identical: NO-OP
//! #override  (border:0; background:none; …)     border=0  bg=None  padLeft=0   ← the effect, achieved
//! ```
//!
//! So the *visual* capability authors reach for `appearance: none` to get is *already available and
//! already working here*. **A tick that "implemented `appearance`" by adding a `ComputedStyle` field
//! nobody reads would have been theatre**, and the 60.5% would have justified it.
//!
//! ## What IS missing is the JS-visible half, and it is a different (smaller, real) defect
//!
//! ```text
//! getComputedStyle(el).appearance        → undefined     (the CSSOM contract says a STRING, always)
//! getComputedStyle(el).webkitAppearance  → undefined
//! CSS.supports('appearance', 'none')     → false
//! ```
//!
//! `undefined` is the t576 `getPropertyValue` defect again: half the web writes
//! `getComputedStyle(el).appearance.indexOf(…)` in one expression, and `undefined.indexOf` kills the
//! caller's frame. That is worth fixing on its own terms — but it is a **CSSOM-completeness** tick, not
//! a rendering one, and pricing it as 60.5%-of-page-loads *rendering* work would have been wrong.
//!
//! This gate pins the measurement so it cannot rot: if native widget painting ever lands, `#styled`
//! and `#plain` will stop being identical and this gate goes red, which is exactly when the row's
//! status needs revisiting.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
  /* Distinct selectors per control — sharing one rule is what made the first attempt at this
     measurement measure nothing, because the "plain" control matched the styled rule too. */
  #styled { appearance: none; -webkit-appearance: none; }
  #override { border: 0; background: none; padding: 0; }
</style></head><body>
<select id="plain"><option>one</option></select>
<select id="styled"><option>one</option></select>
<select id="override"><option>one</option></select>
</body></html>"##;

fn boxes(page: &manuk_page::Page, sel: &str) -> (f32, bool, f32) {
    let root = page.dom().root();
    let n = manuk_css::query_selector_all(page.dom(), root, sel)[0];
    let s = page.styles_of(n).unwrap();
    (
        s.border_width.top,
        s.background_color.is_some(),
        s.padding.left.resolve(0.0, 0.0),
    )
}

#[test]
fn appearance_none_is_a_noop_here_and_author_css_already_does_its_job() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://ap.test/", &fonts, 800.0);
    let (plain, styled, over) = (
        boxes(&page, "#plain"),
        boxes(&page, "#styled"),
        boxes(&page, "#override"),
    );
    println!("APPEARANCE: plain={plain:?} styled={styled:?} override={over:?}");

    // 1. THE UA WIDGET IS REAL — without it a `<select>` is nothing at all, and if this ever became
    //    false the comparison below would be vacuous.
    assert!(
        plain.0 > 0.0 && plain.1,
        "the UA sheet must give a bare `<select>` a border and a background — without them a form \
         control is invisible, and claim 3 would be comparing two blanks"
    );

    // 2. `appearance: none` IS CURRENTLY A NO-OP, and that is the measurement being pinned.
    assert_eq!(
        styled, plain,
        "`appearance: none` changes nothing here, because there is no native widget to suppress — \
         our form controls are UA CSS at lowest specificity. RED means native widget painting has \
         landed, which is exactly when this capability's row needs re-pricing"
    );

    // 3. …AND THE EFFECT AUTHORS WANT IS ALREADY ACHIEVABLE without it.
    assert_eq!(
        over,
        (0.0, false, 0.0),
        "ordinary author CSS (`border:0; background:none; padding:0`) must fully strip the UA widget \
         — that is the visual capability `appearance: none` exists to buy, and here it is bought by \
         the cascade instead. This is why 60.5% of page loads naming the property does NOT make it \
         60.5%-of-page-loads worth of rendering work for this engine"
    );
}
