//! # G_LINE_BOX_ROUNDING — a line box is a whole number of pixels, because Chrome's is
//!
//! The FID-SWEEP's NEAR-MISS population is the most tractable part of the Phase-0 placement gap
//! (coverage 85.9%, placement 4.5%): old.reddit `mdy=12`, airbnb `mdy=20`, wikipedia `mdy=45`,
//! usa.gov `mdy=82` — always with **`mdx=0`**. Horizontal exact, vertical drifting, and the drift
//! **grows with the page's text density**. Tick 268 tested the "missing UA margin constant" branch
//! of that hypothesis: it was a real Chrome-verified correctness bug, and fixing it moved wikipedia
//! **not at all** (7.2 → 7.2, `mdy` 45 → 45). This is the branch that was actually load-bearing.
//!
//! ## The measurement
//!
//! One 600px-wide 6-line paragraph at `font: 16px sans-serif`:
//!
//! ```text
//!   Chrome: 108px      Manuk (before): 110.39px      →  0.4px per line box
//! ```
//!
//! Both engines pick the *same face* — our metrics are Liberation Sans to four decimals, and
//! Chrome's `sans-serif` measures 18px per line where DejaVu gives 19 and Noto 22, which is
//! Liberation. Nothing was wrong with font selection, shaping or advance widths. The line box was
//! simply **fractional**: 18.398px against Chrome's 18. On one line that is invisible; over the
//! ~110 line boxes of a dense article it is 45px, and it lands on every element below the text.
//!
//! ## The rule, and the wrong rule that looks just like it
//!
//! `line-height: normal` = `round(ascent + descent + lineGap)`. Verified on three faces, because
//! **one face cannot tell this rule apart from rounding each term separately**:
//!
//! ```text
//!                   ascent  descent    gap     sum   round(sum)  Chrome   round-each
//! Liberation Sans   14.484    3.391  0.523  18.398          18       18       17  ✗
//! DejaVu Sans       14.854    3.773  0      18.627          19       19       19  =
//! Noto Sans         17.104    4.688  0      21.792          22       22       22  =
//! ```
//!
//! The round-each rule agrees on two of the three faces and is wrong on the one we actually ship.
//! It was implemented here first, with a confident comment citing Skia, and was caught only by
//! **re-running the probe after the edit** rather than trusting the reasoning that motivated it.
//! Hence three faces in the table above, and hence the assertions below check a face whose gap is
//! non-zero — a `gap: 0` face cannot distinguish the two rules at all.
//!
//! ## How each assertion here can go RED
//!
//! - **Drop the `.round()` in `LineMetrics::height`.** The paragraph returns to 110.39px and the
//!   6-line assertion fails by 2.39px — the whole bug, restored.
//! - **Round the parts instead of the sum** (`ascent.round() + descent.round()`). The sans-serif
//!   assertion fails at 102px (17 per line). This is the plausible-but-wrong rule, and it gets its
//!   own assertion precisely because it survives a single-face test.
//! - **Drop `line_gap` from the sum.** Liberation loses its 0.523 and rounds to 18 anyway — so the
//!   *height* assertion still passes, and only the explicit metric assertion catches it. A term
//!   that is invisible at one size is not a term that can be dropped.

use manuk_text::{FontContext, FontFamily, FontKey};

/// Six lines at 16px/600px. Six on purpose: a one-line probe cannot see a per-line error, which is
/// the entire mechanism under test.
const HTML: &str = r#"<!doctype html><html><body style="margin:0">
<div id="box" style="width:600px;font:16px sans-serif"><p id="p1">Lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor incididunt ut labore et dolore magna aliqua ut enim ad minim veniam quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur excepteur sint occaecat cupidatat non proident sunt in culpa qui officia deserunt mollit anim id est laborum.</p></div>
</body></html>"#;

#[test]
fn g_line_box_rounding() {
    let fonts = FontContext::new();

    // ── The RULE, stated on the metrics themselves. `sans-serif` resolves to a face with a
    //    NON-ZERO line gap, which is the only kind that can tell `round(sum)` from
    //    `sum(round)` — on a zero-gap face the two rules agree and the test proves nothing.
    let key = FontKey {
        family: FontFamily::SansSerif,
        bold: false,
        italic: false,
    };
    let lm = fonts.line_metrics(key, 16.0);
    assert!(
        lm.line_gap > 0.0,
        "this gate needs a face with a NON-ZERO line gap to be able to fail — got gap={}. \
         On a zero-gap face `round(a+d+g)` and `round(a)+round(d)` coincide and the assertions \
         below stop discriminating between the right rule and the wrong one",
        lm.line_gap
    );
    let raw = lm.ascent + lm.descent + lm.line_gap;
    assert_eq!(
        lm.height(),
        raw.round(),
        "line-height:normal is round(ascent+descent+lineGap) — the SUM rounded, not the parts"
    );
    assert_ne!(
        lm.height(),
        lm.ascent.round() + lm.descent.round(),
        "…and it must differ from the round-EACH rule on this face ({} vs {}), or this gate would \
         pass under the wrong implementation too",
        lm.height(),
        lm.ascent.round() + lm.descent.round()
    );
    assert_eq!(
        lm.height(),
        18.0,
        "MEASURED in Chrome: a 16px sans-serif line box is exactly 18px (DejaVu would be 19, \
         Noto 22 — this is Liberation, the same face Chrome picks)"
    );

    // ── The CONSEQUENCE, on a real paragraph. This is the assertion that would have caught the
    //    original bug, and the one whose failure is measured in accumulated page drift.
    let page = manuk_page::Page::load(HTML, "https://lb.test/", &fonts, 1280.0);
    let root = page.dom().root();
    let p = manuk_css::query_selector_all(page.dom(), root, "#p1")[0];
    let h = page
        .node_rects()
        .get(&p)
        .map(|r| r.height)
        .expect("#p1 has a box");
    assert_eq!(
        h, 108.0,
        "MEASURED in Chrome: this exact paragraph is 108px (6 lines x 18). Got {h}. \
         A fractional line box reads 110.39 here — 0.4px per line, which is invisible on one \
         line and becomes wikipedia's 45px of accumulated drift over a full article"
    );
    // The height must be a whole number of whole lines: a fractional total is the bug's signature
    // even when the absolute number happens to look close.
    assert_eq!(
        h % 18.0,
        0.0,
        "a stack of line boxes must be an exact multiple of the line box ({h})"
    );
}
