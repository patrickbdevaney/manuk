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

const HTML: &str = r##"<!doctype html><html><head><style>body{margin:0;font:16px sans-serif}
#z{appearance:none;-webkit-appearance:none}</style></head><body>
<input id="a1" size="1"><input id="a5" size="5"><input id="a20"><input id="a40" size="40">
<textarea id="t0"></textarea><textarea id="t1" rows="1"></textarea>
<textarea id="t3" rows="3"></textarea><textarea id="tc" rows="2" cols="10"></textarea>
<select id="s1"><option>English (United States)</option></select>
<select id="s2"><option>a</option></select>
<select id="z"><option>English (United States)</option></select>
</body></html>"##;

/// ⚠⚠ **THE SECOND FIXTURE, AND ITS ONE VARIABLE IS `line-height` ON THE BODY** (t802). The fixture
/// above deliberately sets `font: 16px sans-serif` with no `line-height`, so the document's value is
/// `normal` and identical to the control's — which made it structurally incapable of seeing that we
/// were inheriting the document's. `body { line-height: 1.7 }` is one of the most common typographic
/// rules on the web, and with it every control multiplied out:
///
/// ```text
///                                                  Chrome   before   after
///   <textarea rows=5>  (UA font)                   182x81   182x119  182x81   ✗→✓
///   <textarea>         (UA font, rows=2 default)   182x36   182x 51  182x36   ✗→✓
///   <input>            (UA font)                   205x21   205x 27  205x19   ✗→~
///   <button>                                        47x21    45x 27   45x19   ✗→~
///   <select>                                        30x19    30x 27   30x19   ✗→✓
///   <textarea rows=3 line-height:2>  (AUTHOR)      182x86   182x 86  182x86   ✓ must not move
///   <div> plain block                             1200x27  1200x 27 1200x27   ✓ must not move
///
/// Every `before`/`after` column here is READ OFF THIS FIXTURE (t797's rule: a measured number is
/// only measured for the fixture it was measured in), the `before` column by running the mutation
/// below. `~` marks the two rows that improve but do not land on Chrome's number — a single-line
/// control is 19 here against Chrome's 21, which is the `line-height: normal` metric residual named
/// at the bottom of this comment and is NOT asserted.
/// ```
///
/// `line-height` is the third property of Chrome's `font: -webkit-small-control` shorthand, which
/// resets it to `normal`; a UA *declared* value beats inheritance. We set the family and the size and
/// stopped, so the page's value walked back in through the door the shorthand closes.
///
/// ⚠ `#lo` and `#ld` are the two constraints, and they are not decoration. `#lo` proves the rule is
/// still UA-origin (an author's own `line-height` on the control wins, 86 before and after), and
/// `#ld` proves the fix did not disable `line-height` inheritance generally — a plain block still
/// gets the body's 1.7. A fix that reset line-height everywhere passes every row above and fails
/// `#ld`.
///
/// ⚠ NOT asserted, and named rather than left looking covered: at an AUTHOR font-size of 16px the
/// textarea is 96 here against Chrome's 101 and the input 245 against 238. That residual is
/// `line-height: normal` resolving to 18/row where Chrome uses 19, plus a character-width difference
/// — a font-metric question, independent of this rule and unchanged by it. Asserting Chrome's number
/// there would make this gate fail for a reason it does not test.
const LH_HTML: &str = r##"<!doctype html><html><head><style>body{margin:0;font-family:sans-serif;font-size:16px;line-height:1.7}
.big{font-size:16px}
#lo{line-height:2}</style></head><body>
<input id="li"><textarea id="lt"></textarea><textarea id="lt5" rows="5"></textarea><button id="lb">Send</button><select id="ls"><option>a</option></select>
<div id="ld">plain block</div>
<input id="bi" class="big"><textarea id="bt5" class="big" rows="5"></textarea>
<textarea id="lo" rows="3"></textarea>
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

    // ── SELECT: the text plus the WIDGET. A select sizes to its selected option and then every
    // engine adds room for the arrow it draws beside it. Two options of very different lengths,
    // because a constant and a proportion are only distinguishable across a span: Chrome is 17px
    // wider than our text measurement on BOTH, which is what says "reserved widget" rather than
    // "our glyphs are narrow".
    assert_w(&page, "#s1", 159.0, "a long option — text plus the arrow");
    assert_w(
        &page,
        "#s2",
        30.0,
        "a ONE-CHARACTER option — here the arrow is most of the box, and a proportional error \
         would show up as a different miss than on #s1",
    );

    // ── …and `appearance: none` must stand the reservation DOWN. This is the assertion that makes
    // the fix a fix rather than a trade: reserving unconditionally would correct the classic select
    // and newly break every restyled one, which is most of the modern web's design systems.
    //
    // Asserted as a RELATION, not against a number of our own: Chrome reads 139 here against our
    // 142, a 3px residual that predates this work and belongs to select text measurement, not to
    // the arrow. Pinning 142 would freeze our own answer as if it were Chrome's.
    let (s1, z) = (box_of(&page, "#s1")[0], box_of(&page, "#z")[0]);
    assert!(
        z < s1 - 15.0,
        "G_FORM_CONTROL_METRICS: `appearance: none` must remove the reserved arrow — #s1 is {s1}px \
         with the widget and #z is {z}px without it, a difference of {:.1}px where the arrow is 17. \
         If these are equal the property is not being read, and every restyled <select> on the web \
         is now 17px too wide.",
        s1 - z
    );
}

/// ⚠⚠ **A CONTROL DOES NOT INHERIT THE PAGE'S `line-height` EITHER** (t802) — the third property of
/// the shorthand `font: -webkit-small-control`, left out when the family and the size were added at
/// t787. The fixture above cannot see this: it sets no `line-height`, so the document's value and the
/// control's are both `normal` and agree by accident. Given `body { line-height: 1.7 }` — one of the
/// most common typographic rules on the web — every control multiplied out, and a `<textarea>`'s
/// height is *rows × line-height*, so the error grew with the control.
///
/// Found on `255md.com` (in-scope CrUX, jarring-clean, n=43): a `<textarea>` 138 tall against
/// Chrome's 97 dragged the `<form>`/`<p>`/`<div>`/`<body>` chain above it with it. The site crossed
/// the M1 bar on this fix, 0.721 → 0.767.
///
/// ## How this goes RED
///
/// Delete `line-height: normal` from the control rule in `stylo_engine.rs` and `#lt5` reads 119.33
/// against Chrome's 81 — the body's 1.7 walking back in — while `#lo` and `#ld` still pass. That
/// split is what makes the pair of constraints worth asserting: a gate that only checked the
/// textarea could not tell "we reset it on controls" from "we disabled line-height".
#[test]
fn g_control_line_height_is_normal() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(LH_HTML, "https://forms.test/", &fonts, 1200.0);

    // ── THE BUG: rows × line-height, so the error is proportional to the control.
    assert_h(
        &page,
        "#lt5",
        81.0,
        "a 5-row <textarea> under `body{line-height:1.7}`: the control's line-height is `normal`, \
         not the document's. At 1.7 this reads 119",
    );
    assert_h(
        &page,
        "#lt",
        36.0,
        "the default 2-row textarea — the same rule at a different row count, so a fix that got the \
         intercept right and the slope wrong fails here",
    );
    assert_h(
        &page,
        "#ls",
        19.0,
        "a <select> is one line tall and is measured by the same rule",
    );
    assert_w(
        &page,
        "#lt5",
        182.0,
        "and the WIDTH must not move — `line-height` is a block-axis property, so a fix that \
         disturbed the cols arithmetic would show up here",
    );

    // ── THE TWO CONSTRAINTS. Neither is decoration; each kills a different plausible wrong fix.
    assert_h(
        &page,
        "#lo",
        86.0,
        "an AUTHOR `line-height:2` on the textarea itself still WINS — this rule is UA-origin. \
         Chrome reads 86 with or without the fix; if this changed, the new declaration is being \
         applied at the wrong origin and every styled control on the web just lost its typography",
    );
    assert_h(
        &page,
        "#ld",
        27.0,
        "a PLAIN BLOCK still inherits the body's 1.7 (16 × 1.7 ≈ 27). A fix that reset line-height \
         globally, rather than on controls, passes every assertion above and fails this one",
    );
}
