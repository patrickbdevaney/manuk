//! # G_VERTICAL_ALIGN_ON_TEXT — `<sup>`/`<sub>` are TEXT, and text never read `vertical-align`
//!
//! ⚠⚠⚠ **THE FRAGMENT WAS CONSTRUCTED WITH `valign: VerticalAlign::Baseline` HARD-CODED**, so the
//! eight `vertical-align` match arms in `line_metrics` were unreachable for text and only ever ran
//! for atomic inlines (images, inline-blocks). t913 measured the symptom — thirteen cases, every one
//! of them 24px where Chrome grows the line — and located the branch; the root cause is one level
//! further in: the word's own `vertical-align` was never read at the point where the line fragment
//! is built, because that builder is a `move` closure and nobody had captured it.
//!
//! **And `<sup>`/`<sub>` had no UA rule at all**, so a footnote marker, a citation, a ™, an ordinal
//! and every chemical formula on the web rendered as plain baseline text at full size. Chrome's UA
//! sheet is `sup { vertical-align: super; font-size: smaller }`.
//!
//! ## What is asserted here is only what is CHROME-EXACT
//!
//! ```text
//!                                     Chrome   before   after
//!   <sup>XX</sup> own box              18x15    21x17    18x15   <- the UA shrink, EXACT
//!   plain <span>XX</span> own box      21x17    21x17    21x17   <- the control
//!   plain baseline line (CONTROL)        24       24       24
//!   vertical-align: top   (CONTROL)      24       24       24
//!   super on a font-size:10px span       24       24       24    <- fits the strut: must NOT grow
//!   super on a 10px <img>  (CONTROL)     24       24       24
//! ```
//!
//! ⚠⚠⚠ **THE RESIDUAL IS NAMED AND DELIBERATELY NOT ASSERTED, because asserting it would bank an
//! approximation as if it were measured.** The keyword offsets reuse the constants the ATOMIC arms
//! already use (`ascent * 0.35` for `super`, `ascent * 0.15` for `sub`) — shared on purpose so the
//! two implementations of `vertical-align` cannot drift — and they are approximations of what Chrome
//! derives from the font's own `OS/2` superscript/subscript offsets:
//!
//! ```text
//!                                     Chrome   before   after
//!   vertical-align: super                30       24       29    <- 1px short
//!   vertical-align: sub                  28       24       26    <- 2px short
//!   vertical-align: middle               25       24       26    <- 1px over
//!   <sup> / <sub>                        27       24       24    <- shrunk enough to fit the strut
//!   vertical-align: text-top             27       24       24
//!   vertical-align: text-bottom          28       24       24
//!   vertical-align: 10px / -10px / 50%   34/34/36 24       24    <- NO enum variant exists
//! ```
//!
//! The last row is a different job: `VerticalAlign` has eight keyword variants and no length or
//! percentage, so `vertical-align: 10px` parses to `Baseline` and cannot be represented at all.
//! Adding that variant touches the css crate and every match site, and it is its own tick.
//!
//! **So this gate asserts the MECHANISM and the CONTROLS, and states the calibration as an open
//! number.** The direction claims below are what make it a ratchet tooth: a line carrying a raised
//! inline must GROW, and the four controls must NOT — and the second half is the one that stops a
//! future "fix" from simply inflating every line box.
//!
//! ⚠ **BOTH HALVES SHIPPED IN ONE CHANGE, which is the ratchet and not a preference.** Growing the
//! line box without moving the glyphs would make every `<sup>` line taller with its text still on
//! the baseline — a metric win bought with a visible regression. `valign_text_shift` is called from
//! `line_metrics` (to size the line) and from the placement loop (to move the baseline), and the
//! raise is asserted below as a relationship between a `<sup>` and a plain `<span>`.
//!
use manuk_text::FontContext;

const HTML: &str = r##"<!DOCTYPE html><html><head><meta charset="utf-8"><style>
 body{margin:0;font:16px/1.5 sans-serif}
 .f{width:300px;background:#eee;margin-bottom:4px}
</style></head><body>
<div class="f" id="v0">plain baseline</div>
<div class="f" id="v1"><span style="vertical-align:super">s</span>base</div>
<div class="f" id="v2"><span style="vertical-align:sub">s</span>base</div>
<div class="f" id="v3"><sup>s</sup>base</div>
<div class="f" id="v4"><sub>s</sub>base</div>
<div class="f" id="v5"><span style="vertical-align:top">s</span>base</div>
<div class="f" id="v6"><span style="vertical-align:middle">s</span>base</div>
<div class="f" id="v7"><span style="vertical-align:10px">s</span>base</div>
<div class="f" id="v8"><span style="vertical-align:-10px">s</span>base</div>
<div class="f" id="v9"><span style="vertical-align:50%">s</span>base</div>
<div class="f" id="v10"><span style="vertical-align:text-top">s</span>base</div>
<div class="f" id="v11"><span style="vertical-align:text-bottom">s</span>base</div>
<div class="f" id="v12"><span style="vertical-align:super;font-size:10px">s</span>base</div>
<div class="f" id="v13"><img style="vertical-align:super" width="10" height="10" src="data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7">base</div>

</body></html>
"##;

const CAL_HTML: &str = r##"<!DOCTYPE html><html><head><meta charset="utf-8"><style>body{margin:0;font-family:sans-serif}
 .f{width:400px;background:#eee;margin-bottom:2px}</style></head><body>
<div class="f" id="c16" style="font-size:16px;line-height:1.5">x<span style="vertical-align:super">s</span></div>
<div class="f" id="b16" style="font-size:16px;line-height:1.5">x</div>
<div class="f" id="c24" style="font-size:24px;line-height:1.5">x<span style="vertical-align:super">s</span></div>
<div class="f" id="b24" style="font-size:24px;line-height:1.5">x</div>
<div class="f" id="c32" style="font-size:32px;line-height:1.5">x<span style="vertical-align:super">s</span></div>
<div class="f" id="b32" style="font-size:32px;line-height:1.5">x</div>
<div class="f" id="d16" style="font-size:16px;line-height:1.5">x<span style="vertical-align:sub">s</span></div>
<div class="f" id="d24" style="font-size:24px;line-height:1.5">x<span style="vertical-align:sub">s</span></div>
<div class="f" id="e16" style="font-size:16px;line-height:3">x<span style="vertical-align:super">s</span></div>
<div class="f" id="f16" style="font-size:16px;line-height:3">x</div>

</body></html>
"##;

const SUP_HTML: &str = r##"<!DOCTYPE html><html><head><meta charset="utf-8"><style>body{margin:0;font:16px/1.5 sans-serif}</style></head><body>
<div id="d1"><sup id="s1">XX</sup></div>
<div id="d2"><span id="s2">XX</span></div>
</body></html>
"##;

fn rect(page: &manuk_page::Page, sel: &str) -> (f32, f32, f32, f32) {
    let dom = page.dom();
    let n = manuk_css::query_selector_all(dom, dom.root(), sel)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("selector {sel} matched nothing"));
    let r = page
        .root_box
        .node_rects(dom)
        .get(&n)
        .copied()
        .unwrap_or_else(|| panic!("no box for {sel}"));
    (r.x, r.y, r.width, r.height)
}

fn h(page: &manuk_page::Page, sel: &str) -> f32 {
    rect(page, sel).3
}

fn c_cal(page: &manuk_page::Page, sel: &str, want: f32) {
    let got = h(page, sel);
    assert!(
        (got - want).abs() < 1.01,
        "G_VERTICAL_ALIGN_ON_TEXT calibration: `{sel}` expected {want} (CAPTURED from \
         `google-chrome-stable --headless=new --hide-scrollbars --window-size=1200,800`), got {got}"
    );
}

#[test]
fn g_vertical_align_on_text() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://va.test/", &fonts, 1200.0);

    // ── THE CONTROLS. Every one is Chrome-exact, and together they are what stops this fix from
    // becoming "make every line taller".
    for (sel, why) in [
        ("#v0", "a plain baseline line is the 24px control everything else is measured against"),
        ("#v5", "`vertical-align: top` aligns to the line box and cannot grow it"),
        (
            "#v12",
            "a `super` span at font-size:10px is RAISED but still fits inside the strut, so the              line must NOT grow — this is the row that proves the rule is CSS 2.1 §10.8's UNION              and not an addition",
        ),
        ("#v13", "…and the same is true of a 10px <img> raised by `super`"),
    ] {
        let got = h(&page, sel);
        assert!(
            (got - 24.0).abs() < 1.01,
            "G_VERTICAL_ALIGN_ON_TEXT: `{sel}` must stay 24 (Chrome), got {got} — {why}"
        );
    }

    // ── THE MECHANISM, and t915 turned two of these three from a DIRECTION into a NUMBER: the
    // offsets are now measured against Chrome rather than borrowed from the atomic arms.
    for (sel, want, why) in [
        (
            "#v1",
            30.0,
            "`vertical-align: super` on TEXT — parent-font x 0.375",
        ),
        (
            "#v2",
            28.0,
            "`vertical-align: sub` on TEXT — parent-font x 0.25",
        ),
    ] {
        let got = h(&page, sel);
        assert!(
            (got - want).abs() < 1.01,
            "G_VERTICAL_ALIGN_ON_TEXT: `{sel}` expected {want} (Chrome) — {why}; got {got}. Before \
             t914 it read exactly 24, because the line fragment was built with `valign: Baseline` \
             hard-coded; before t915 it read one to two px short, because the offset was the \
             atomic arms' approximation."
        );
    }
    // ── t916: `text-top` / `text-bottom`, now EXACT. The old formula was `strut_ascent - a`, which
    // is ZERO whenever the fragment and the strut share a font — so these did nothing at all on the
    // overwhelming majority of real markup. CSS 2.1 §10.8.1 aligns the top of the aligned subtree's
    // INLINE BOX (which includes its half-leading) with the top of the parent's CONTENT AREA (which
    // does not), and at `line-height: 1.5` that is a ~2.5px downward shift.
    for (sel, want) in [("#v10", 27.0), ("#v11", 28.0)] {
        let got = h(&page, sel);
        assert!(
            (got - want).abs() < 1.01,
            "G_VERTICAL_ALIGN_ON_TEXT: `{sel}` expected {want} (Chrome) — the inline box carries \
             its half-leading and the content area does not; got {got}"
        );
    }

    // ── t922: the LENGTH and PERCENTAGE forms, which were UNREPRESENTABLE until this tick — the
    // enum had eight keyword variants and no length, so `vertical-align: -2px` (the standard idiom
    // for nudging an inline icon against its label) parsed to `baseline` and vanished. A length is
    // the raise itself; a percentage is of THIS element's own `line-height`, not the strut's and not
    // the font size — which `#v9` pins, being 50% of a 24px line and not of a 16px font.
    for (sel, want) in [("#v7", 34.0), ("#v8", 34.0), ("#v9", 36.0)] {
        let got = h(&page, sel);
        assert!(
            (got - want).abs() < 1.01,
            "G_VERTICAL_ALIGN_ON_TEXT: `{sel}` expected {want} (Chrome) — a length raises by itself \
             and a percentage by that fraction of the element's own line-height; got {got}"
        );
    }

    // `middle` is still an open number (Chrome 25, ours 26) and is asserted as DIRECTION only —
    // banking 26 would freeze an approximation as though it had been measured.
    assert!(
        h(&page, "#v6") > 24.5,
        "`vertical-align: middle` on TEXT must still GROW the line (Chrome 25); got {}",
        h(&page, "#v6")
    );

    // ── THE CALIBRATION, which is what makes the two numbers above a RULE rather than a fit to one
    // font size. Chrome-measured at 16/24/32px and at two line-heights: the raise is exactly
    // `parent-font-size x 0.375` and the drop `x 0.25`, and BOTH are independent of `line-height`.
    // The `line-height: 3` pair is the row that proves the second half — an offset derived from the
    // line box would move there and Chrome's does not.
    let cal = manuk_page::Page::load(CAL_HTML, "https://va.test/", &fonts, 1200.0);
    c_cal(&cal, "#b16", 24.0);
    c_cal(&cal, "#b24", 36.0);
    c_cal(&cal, "#b32", 48.0);
    c_cal(&cal, "#c16", 30.0);
    c_cal(&cal, "#c24", 45.0);
    c_cal(&cal, "#c32", 60.0);
    c_cal(&cal, "#d16", 28.0);
    c_cal(&cal, "#d24", 42.0);
    c_cal(&cal, "#e16", 54.0);
    c_cal(&cal, "#f16", 48.0);

    // ── THE UA RULE, and this half IS Chrome-exact: `sup { font-size: smaller }` shrinks the box.
    let sup = manuk_page::Page::load(SUP_HTML, "https://va.test/", &fonts, 1200.0);
    let (_, sy, sw, sh) = rect(&sup, "#s1");
    let (_, py, pw, ph) = rect(&sup, "#s2");
    assert!(
        (sw - 18.0).abs() < 1.01 && (sh - 15.0).abs() < 1.01,
        "a <sup>'s own box is 18x15 in Chrome (`font-size: smaller`), got {sw}x{sh}"
    );
    assert!(
        (pw - 21.0).abs() < 1.01 && (ph - 17.0).abs() < 1.01,
        "…and the plain <span> control is 21x17 in Chrome, got {pw}x{ph}"
    );
    // ── THE RAISE, as a RELATIONSHIP rather than a coordinate: the two live in different divs, so
    // an absolute `y` would encode the first div's height into a claim about superscripts.
    assert!(
        sy < py - 10.0,
        "a <sup> must be RAISED well above where a plain <span> sits on the baseline — this is the          half that must ship with the line-growth or the glyphs stay put while the line gets          taller. sup y={sy}, span y={py}"
    );
}
