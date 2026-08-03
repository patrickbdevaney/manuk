//! **G_EMPTY_INLINE_RECT — an empty inline SHARING a line reports its own content area, not the
//! line height.**
//!
//! ⚠⚠⚠ **The comment this replaces was half-measured, and its half was the one that does not
//! matter.** It read: *"an EMPTY inline keeps the old line-top anchoring: Chrome reports a
//! line-height-tall rect for `<span id="anchor"></span>`, and that is measured behaviour this must
//! not disturb."* Re-measured across five contexts with `chromium --headless --dump-dom`
//! (`16px/1.5 sans-serif`):
//!
//! ```text
//!                                          Chrome        before
//!   <div><span></span></div>               [0, 0,0, 0]   [0, 0,0, 0]   agree
//!   <div><span></span><span></span></div>  [0,48,0, 0]   [0,48,0, 0]   agree
//!   <div><span></span>text</div>           [0, 3,0,17]   [0, 0,0,24]   <-
//!   <div>text<span></span></div>          [26,27,0,17]  [26,24,0,24]   <-
//!   <div style="line-height:3">…</div>     [0,63,0,17]   [0,48,0,48]   <- the error SCALES
//! ```
//!
//! The `0x0` rows — the ones the old comment was looking at — are right for a reason that has
//! nothing to do with the reported height: an empty inline **alone** brings no line box into
//! existence (CSS2 §9.4.2, `holds_line: false`), so there is no line for a height to be reported
//! against. The moment it shares a line with content, Chrome reports the element's **own content
//! area on the line's baseline** — the identical rule the no-fragment branch beside it already
//! implements. Two branches, one rule, and they now say the same thing.
//!
//! **Why it is worth a gate.** `<a><i class="icon"></i><span>Label</span></a>` is on every
//! navigation bar on the web, and an empty `<i>` reported 3px too high and 7px too tall next to its
//! label is what flips a `reading-order` comparison. In the t867 sweep, **13 sites are already over
//! the M1 shape bar and fail ONLY on a jarring dimension**, and `possssno.sbs`'s single
//! reading-order inversion is exactly this markup.
//!
//! **Both halves are asserted**, because each alone is a way to get this wrong: report the content
//! area (or the `0x0`-when-alone case regresses into a phantom 17px box that never existed), AND
//! keep the line boxes byte-identical (the containing block's height must not move — an empty inline
//! still holds no line open).

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
  body { margin:0; font: 16px/1.5 sans-serif }
</style></head><body>
<div id="d1"><span id="s1"></span></div>
<div id="d2"><span id="s2"></span>text</div>
<div id="d3">text<span id="s3"></span></div>
<div id="d4"><span id="s4"></span><span id="s5"></span></div>
<div id="d5" style="line-height:3"><span id="s6"></span>text</div>
</body></html>"##;

/// `(selector, x, y, w, h)` — every number from `chromium --headless --dump-dom` on this exact
/// fixture. Transcribed, never derived: the previous version of this rule was derived and was wrong.
const CHROME: &[(&str, i32, i32, i32, i32)] = &[
    // ALONE: no line box exists, so there is no rect. This pair is the case the old comment saw.
    ("#s1", 0, 0, 0, 0),
    ("#s4", 0, 48, 0, 0),
    ("#s5", 0, 48, 0, 0),
    // SHARING a line: the element's own content area, on the line's baseline.
    ("#s2", 0, 3, 0, 17),
    ("#s3", 26, 27, 0, 17),
    // …and it does NOT grow with `line-height`. This row is the one that fails loudest when the
    // reported height is the line box: 48 against 17.
    ("#s6", 0, 63, 0, 17),
];

/// The containing blocks must not move — an empty inline holds no line open either way.
const BLOCK_HEIGHTS: &[(&str, i32)] = &[("#d1", 0), ("#d2", 24), ("#d5", 48)];

#[test]
fn an_empty_inline_sharing_a_line_reports_its_content_area_not_the_line_height() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://inline.test/", &fonts, 1200.0);
    let dom = page.dom();
    let rects = page.root_box.node_rects(dom);

    let find = |sel: &str| -> [f32; 4] {
        let n = manuk_css::query_selector_all(dom, dom.root(), sel);
        assert!(!n.is_empty(), "G_EMPTY_INLINE_RECT: `{sel}` did not match");
        let r = rects
            .get(&n[0])
            .unwrap_or_else(|| panic!("G_EMPTY_INLINE_RECT: `{sel}` has NO rect at all"));
        [r.x, r.y, r.width, r.height]
    };

    for &(sel, x, y, w, h) in CHROME {
        let r = find(sel);
        let got = [
            r[0].round() as i32,
            r[1].round() as i32,
            r[2].round() as i32,
            r[3].round() as i32,
        ];
        assert_eq!(
            got,
            [x, y, w, h],
            "G_EMPTY_INLINE_RECT: {sel} is [{} {} {}x{}], Chrome says [{x} {y} {w}x{h}]\n\n  \
             An empty inline that SHARES a line with content reports its own CONTENT AREA on the \
             line's baseline — not the line box's height anchored to the line's top. Reporting the \
             line height makes the error grow with `line-height` (48 against 17 at `line-height:3`) \
             and is what flips a reading-order comparison on \
             `<a><i class=\"icon\"></i><span>Label</span></a>`, which is every nav bar on the web. \
             An empty inline ALONE still reports 0x0, because it brings no line box into existence \
             (CSS2 §9.4.2) — that half must not regress into a phantom 17px box.",
            got[0],
            got[1],
            got[2],
            got[3],
        );
    }

    for &(sel, h) in BLOCK_HEIGHTS {
        let r = find(sel);
        assert_eq!(
            r[3].round() as i32,
            h,
            "G_EMPTY_INLINE_RECT: {sel} height is {}, Chrome says {h}\n\n  \
             The LINE BOXES must be byte-identical: this rule changes what an empty inline REPORTS, \
             never what it contributes to the flow. An empty inline holds no line open, so the \
             containing block's height cannot move — if it did, the reporter grew a `holds_line`.",
            r[3].round() as i32
        );
    }
}
