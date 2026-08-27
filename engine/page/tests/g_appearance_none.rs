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
<select id="wplain"><option>alpha</option></select>
<select id="wnone" style="appearance:none"><option>alpha</option></select>
<select id="wwk" style="-webkit-appearance:none"><option>alpha</option></select>
<button id="btn">b</button><div id="dv">d</div><input id="inp">
<div id="dauto" style="appearance:auto">x</div>
<div id="out">-</div>
<script>
  var R = [], g = getComputedStyle, $ = function (i) { return document.getElementById(i); };
  // ── ⭐ THE COMPUTED VALUE (t1355). It was `undefined`, and that ONE absence failed 300 subtests
  //    in `css/css-ui/appearance-cssom-001` without ever being about appearance's behaviour: the
  //    test reads `initial_appearance = getComputedStyle(button).appearance` once and compares
  //    every invalid-value row against it, so an `undefined` reference made every row
  //    `assert_in_array("", [undefined])`.
  R.push('cBtn:' + g($('btn')).appearance);
  R.push('cDiv:' + g($('dv')).appearance);
  R.push('cInput:' + g($('inp')).appearance);
  R.push('cNone:' + g($('wnone')).appearance);
  R.push('cAuto:' + g($('dauto')).appearance);
  // Both camel spellings and both getPropertyValue spellings — Chrome exposes all four.
  R.push('cWk:' + g($('btn')).WebkitAppearance + '/' + g($('btn')).webkitAppearance);
  R.push('cPV:' + g($('dv')).getPropertyValue('appearance') + '/' + g($('dv')).getPropertyValue('-webkit-appearance'));
  // ── ⭐⭐ THE CSSOM PATH MUST *RENDER*, which is the whole bug: the markup path honoured
  //    `appearance:none` and `el.style.appearance = 'none'` was validated away to nothing.
  var mk = document.createElement('select');
  mk.innerHTML = '<option>alpha</option>';
  document.body.appendChild(mk);
  var wBefore = Math.round(mk.getBoundingClientRect().width);
  mk.style.appearance = 'none';
  R.push('jsSet:[' + mk.style.appearance + ']');
  R.push('jsNarrowed:' + (Math.round(mk.getBoundingClientRect().width) < wBefore));
  R.push('supports:' + CSS.supports('appearance', 'none') + '/' + CSS.supports('-webkit-appearance', 'none'));
  // ⚠ THE INLINE DECLARATION'S TWO CAMEL SPELLINGS. `dash()` turns a capital into `-` + lowercase,
  //   so `WebkitAppearance` supplies its own leading dash and `webkitAppearance` does NOT — the
  //   lowercase form mapped to `webkit-appearance`, a property that does not exist, and read `''`.
  var wk = document.createElement('div').style;
  wk.setProperty('-webkit-appearance', 'none');
  R.push('inlineWk:' + wk.WebkitAppearance + '/' + wk.webkitAppearance + '/' + wk.getPropertyValue('-webkit-appearance'));
  // An invalid keyword is DROPPED; a valid non-`none` one is echoed as SPECIFIED and computes to auto.
  var d2 = $('dv').style;
  d2.setProperty('appearance', 'bogus-button');
  R.push('invalid:[' + d2.getPropertyValue('appearance') + ']');
  d2.setProperty('appearance', 'textfield');
  R.push('special:[' + d2.getPropertyValue('appearance') + ']/' + g($('dv')).appearance);
  $('out').textContent = R.join(' ');
</script>
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

    // ── ⚠⚠ 4. AND THE HEADLINE ABOVE IS HALF-WRONG, WHICH THIS ROW CORRECTS RATHER THAN HIDES.
    //    `appearance: none` is a no-op for BORDER, BACKGROUND and PADDING — the three things claims
    //    1-3 measure — and it is NOT a no-op for WIDTH: a `<select>` under it stops reserving the
    //    dropdown arrow. Measured on one `alpha` option: 56px plain, 39px with `appearance:none`,
    //    39px with the `-webkit-` spelling. A gate that measures three properties and concludes
    //    "no-op" has said something about three properties.
    let w = |sel: &str| boxes(&page, sel);
    let (wp, wn, ww) = (
        rect_width(&page, "#wplain"),
        rect_width(&page, "#wnone"),
        rect_width(&page, "#wwk"),
    );
    let _ = w;
    assert!(
        wn < wp - 1.0 && (ww - wn).abs() < 1.0,
        "`appearance: none` on a <select> must drop the dropdown-arrow reserve — plain {wp:.0}, \
         none {wn:.0}, -webkit-none {ww:.0}. Equal widths mean the property stopped being read at \
         all; unequal `none` and `-webkit-none` mean the alias was lost."
    );

    // ── ⭐ 5. THE CSSOM SURFACE (t1355), every row Chrome-measured.
    let out = {
        let n = manuk_css::query_selector_all(page.dom(), page.dom().root(), "#out")[0];
        page.dom().text_content(n)
    };
    println!("APPEARANCE-CSSOM: {out}");
    for (claim, why) in [
        (
            "cBtn:auto",
            "⭐ a form control computes `auto`. This ONE value being `undefined` failed 300 subtests \
             in `appearance-cssom-001` — the test caches it as its reference and every invalid-value \
             row then compares against `undefined`",
        ),
        (
            "cDiv:none",
            "⚠ and a plain <div> computes `none`. THE ASYMMETRY IS THE POINT: `auto` is a UA rule \
             keyed on the tag, not the property's initial value, so a fix that returned one constant \
             passes the row above and fails this one",
        ),
        ("cInput:auto", "…and every native control, not just <button>"),
        ("cNone:none", "an author `appearance:none` computes `none` on a control"),
        (
            "cAuto:auto",
            "…and an author `appearance:auto` computes `auto` on a NON-control — the mirror, which \
             is what proves the value is cascaded rather than derived from the tag alone",
        ),
        (
            "cWk:auto/auto",
            "both camel spellings of the prefixed property resolve on a computed style. \
             `alias_pairs_js` generates only `webkitAppearance`; `WebkitAppearance` is an explicit \
             pair, and the pair is [TARGET, SOURCE] — writing it the other way round reads an \
             `undefined` and defines nothing",
        ),
        (
            "cPV:none/none",
            "…and both dashed spellings through `getPropertyValue`, which is a different lookup path \
             from the IDL attribute",
        ),
        (
            "jsSet:[none]",
            "⭐⭐ THE BUG. `el.style.appearance = 'none'` was VALIDATED AWAY: `appearance` is \
             `engine=\"gecko\"` in Stylo's servo build, so `CSS.supports` answered no and the setter \
             dropped a declaration the MARKUP path honours. Reading `[]` here is that state",
        ),
        (
            "jsNarrowed:true",
            "…and it must RENDER, not merely read back. The <select> narrows because the \
             dropdown-arrow reserve is dropped. This is the row that separates a CSSOM value from a \
             CSSOM value that does something",
        ),
        (
            "supports:true/true",
            "`CSS.supports` must describe THIS engine (t1180) — an honest NO for a capability we \
             HAVE is the mirror of the false YES that rule was written about, and it is what made \
             the setter a no-op",
        ),
        (
            "inlineWk:none/none/none",
            "⚠ BOTH camel spellings on an INLINE declaration, and the lowercase one is the arm that \
             was broken: `dash()` maps a capital to `-` + lowercase, so `WebkitAppearance` supplies \
             its own leading dash and `webkitAppearance` produced `webkit-appearance` — a property \
             that does not exist, so the read was `''` and a write went nowhere. ⚠ The fix is scoped \
             to STYLE and not to `dataset`, where `dataset.webkitFoo` really is `data-webkit-foo`",
        ),
        (
            "invalid:[]",
            "an invalid keyword is DROPPED, not coerced — CSS ignores a declaration it cannot parse, \
             and `appearance-cssom-001` asserts exactly that for 40+ legacy widget names",
        ),
        (
            "special:[textfield]/auto",
            "⚠ THE TWO SURFACES DISAGREE ON PURPOSE: the SPECIFIED value echoes what the author \
             wrote (`textfield`), and the COMPUTED value says what this engine will DO (`auto`, \
             because it draws one native control or none). Collapsing them either way is a lie in \
             one direction or the other",
        ),
    ] {
        assert!(
            out.contains(claim),
            "G_APPEARANCE_NONE: expected `{claim}`\n  got: {out}\n\n  {why}."
        );
    }
}

/// The border-box width of `sel`, for the arrow-reserve rows.
fn rect_width(page: &manuk_page::Page, sel: &str) -> f32 {
    let dom = page.dom();
    let n = manuk_css::query_selector_all(dom, dom.root(), sel)[0];
    page.root_box
        .node_rects(dom)
        .get(&n)
        .map_or(0.0, |r| r.width)
}
