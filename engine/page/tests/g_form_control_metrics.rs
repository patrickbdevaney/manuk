//! # G_FORM_CONTROL_METRICS — a text field's intrinsic box, against Chrome's actual numbers
//!
//! A form control is a WIDGET: the browser decides how big it is, from `size`/`cols`/`rows` and from
//! the control's own font. Nothing on the page says how wide a search box should be, so if this
//! arithmetic is wrong the box is wrong on every form that does not override it — and a control's
//! width is a container's width one level up.
//!
//! ## The numbers are MEASURED, not recalled
//!
//! Read out of headless Chrome (`--dump-dom` + `getBoundingClientRect`), on a page whose body font is
//! `16px sans-serif`, so the control font is the UA's and not the document's:
//!
//! ```text
//!   <input size=1>    53×21      <input size=20>   205×21     (default size is 20)
//!   <input size=5>    85×21      <input size=40>   365×21
//!   <textarea>       182×36      <textarea rows=1> 182×21     (default rows is 2)
//!   <textarea rows=3>182×51      <textarea rows=2 cols=10> 102×36
//! ```
//!
//! Three separate facts fall out of that table, and each was a defect here:
//!
//! 1. **A control does not inherit the page's font.** Chrome gives every control
//!    `font: -webkit-small-control` — the ~13.3px system face, not the document's 16px. Inheriting
//!    made every control ~20% too big in both axes.
//! 2. **The input intercept is 45px border box, not ~19.** The slope is exactly 8.0px/char in both
//!    engines; the *constant* was 26px short, so every default-width text field on the web was too
//!    narrow. The comment that shipped with the old constant asserted it was "the same approximation
//!    Chrome's own default ends up at" — Chrome ends up 26px away, and nobody had put the two side
//!    by side.
//! 3. **`rows` was not read at all.** An empty `<textarea>` sized to its empty content: one line,
//!    22px, against Chrome's 36. Every comment box and contact form on the web, and the error does
//!    not stay in the control — a box a line short pulls the whole page below it upward.
//!
//! ## How each assertion goes RED
//!
//! - **Drop the UA `font-size: 13.333px` on controls** — every width fails at once, proportionally.
//! - **Restore the old `cols * 8.0 + 13.0`** — the `<input>` widths fail, the `<textarea>` widths
//!   fail by a smaller amount (the two intercepts genuinely differ: 45 vs 22 border box, because a
//!   text field reserves caret-scroll room a textarea does not).
//! - **Delete the `rows` block** — the three height assertions fail and the widths stay green, which
//!   is why the heights are asserted separately rather than as one box comparison.
//!
//! ## Residuals, stated rather than hidden
//!
//! `<input>` heights read 19px against Chrome's 21, and a `<select>`'s intrinsic width is short by
//! **exactly 17px** (142 vs 159 with a long option, 13 vs 30 with a one-character one — the same 17
//! either way, which is the dropdown arrow Chrome reserves and we do not). Both are measured, both
//! are named here, and neither is asserted as passing.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>body{margin:0;font:16px sans-serif}</style></head><body>
<input id="a1" size="1"><input id="a5" size="5"><input id="a20"><input id="a40" size="40">
<textarea id="t0"></textarea><textarea id="t1" rows="1"></textarea>
<textarea id="t3" rows="3"></textarea><textarea id="tc" rows="2" cols="10"></textarea>
</body></html>"##;

/// The BORDER-BOX `[w, h]` the live pipeline laid out — the same quantity
/// `getBoundingClientRect()` reports in Chrome, so the two tables are comparable.
fn box_of(page: &manuk_page::Page, sel: &str) -> [f32; 2] {
    let dom = page.dom();
    let n = manuk_css::query_selector_all(dom, dom.root(), sel)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("selector {sel} matched nothing"));
    let rects = page.root_box.node_rects(dom);
    let r = rects
        .get(&n)
        .unwrap_or_else(|| panic!("no box for {sel} — the control generated no box at all"));
    [r.width, r.height]
}

fn assert_w(page: &manuk_page::Page, sel: &str, want: f32, why: &str) {
    let b = box_of(page, sel);
    assert!(
        (b[0] - want).abs() < 1.01,
        "G_FORM_CONTROL_METRICS: `{sel}` width expected {want}px (MEASURED in Chrome), got {}.\n  {why}",
        b[0]
    );
}

fn assert_h(page: &manuk_page::Page, sel: &str, want: f32, why: &str) {
    let b = box_of(page, sel);
    assert!(
        (b[1] - want).abs() < 1.01,
        "G_FORM_CONTROL_METRICS: `{sel}` height expected {want}px (MEASURED in Chrome), got {}.\n  {why}",
        b[1]
    );
}

#[test]
fn g_form_control_metrics() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://forms.test/", &fonts, 1200.0);

    // ── WIDTH: slope AND intercept. Two points fix a line, so four are asserted — a wrong slope with
    // a compensating intercept would pass on any single one of them.
    assert_w(
        &page,
        "#a1",
        53.0,
        "size=1 — the intercept dominates a short field entirely",
    );
    assert_w(&page, "#a5", 85.0, "size=5");
    assert_w(
        &page,
        "#a20",
        205.0,
        "no `size` attribute — HTML's default is 20, and this is the width most of the web's text \
         fields actually get",
    );
    assert_w(
        &page,
        "#a40",
        365.0,
        "size=40 — pins the 8.0px/char slope over a long span",
    );

    // ── TEXTAREA: a DIFFERENT intercept from the text field, which is why one shared constant was
    // wrong for one of them.
    assert_w(&page, "#t0", 182.0, "default cols=20");
    assert_w(&page, "#tc", 102.0, "cols=10");

    // ── HEIGHT: `rows`, which was not read at all. Asserted at three values because a hard-coded
    // two-line box would satisfy the default case alone.
    assert_h(
        &page,
        "#t1",
        21.0,
        "rows=1 — one line box plus this sheet's padding and border",
    );
    assert_h(
        &page,
        "#t0",
        36.0,
        "no `rows` attribute — HTML's default is 2. A control that sized to its (empty) content \
         came out one line tall here and 14px short of Chrome on every comment form on the web",
    );
    assert_h(
        &page,
        "#t3",
        51.0,
        "rows=3 — pins the per-row slope, not just the default",
    );
}
