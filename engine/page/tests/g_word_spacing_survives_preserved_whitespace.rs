//! # G_WORD_SPACING_SURVIVES_PRESERVED_WHITESPACE — the one place `pre` has for a space is INSIDE
//! the run
//!
//! ⚠⚠⚠ **`word-spacing` WORKED EVERYWHERE EXCEPT THE CONTENT THAT PRESERVES ITS SPACING ON
//! PURPOSE.** In the wrapping path an inline run never contains a space: the inter-word space is
//! its own item and `space_before` pays both spacings for it. Under `white-space: pre` there is no
//! such split — the preserved spaces travel INSIDE the run's text, so that arm never runs and
//! `word-spacing` was dropped for code blocks, ASCII tables, terminal transcripts and
//! `<pre>`-formatted logs: precisely the content whose spacing is load-bearing.
//!
//! ⭐ **`letter-spacing` WAS NEVER DROPPED THERE, AND THAT IS WHY THIS READ AS A `word-spacing`
//! BUG.** The run's width already pays `letter_spacing` once per CHARACTER, and a space is a
//! character, so the `pre` path looked half-correct — one of the two spacings survived it. The
//! defect is the PATH, not the property.
//!
//! Every number below is CAPTURED from `google-chrome --headless --hide-scrollbars
//! --window-size=1200,800`, `font: 20px/1.2 monospace`, the string `a b c d` in an `inline-block`.
//!
//! ```text
//!                                                  Chrome    before    after
//!   #n    word-spacing: normal                       84.30     84       84    CTRL
//!   #w    word-spacing: 10px                        114.30    114      114    CTRL — the path
//!                                                                             that already worked
//!   #pn   white-space: pre, normal                   84.30     84       84    CTRL
//!   #pw   white-space: pre, 10px                    114.30     84      114    KEY
//!   #pch  white-space: pre, 1ch                     120.42     84      120    KEY
//!   #pnb  white-space: pre, 10px, NBSP separators   114.30     84      114    KEY
//!   #pem  white-space: pre, 10px, EM-SPACE seps      84.30     84       84    KEY (negative)
//!   #pls  white-space: pre, 10px + letter-spacing 2px 128.30   104     128    KEY
//! ```
//!
//! ⭐⭐ **`#pem` IS THE ROW THAT REFUSES THE OBVIOUS IMPLEMENTATION.** "Charge the spacing for each
//! whitespace character in the run" passes every other row here and fails this one: **U+2003 EM
//! SPACE is not a word separator** and Chrome leaves it at 84.30, while U+0020 and U+00A0 each take
//! the full 10px (`#pw`, `#pnb`). The separator set is measured, not read off the spec — CSS Text 3
//! lists more separators than Chrome charges, and counting "whitespace" would widen every em space
//! and every tab on every `<pre>` on the web.
//!
//! `#pls` is the interaction row: under `pre` the two spacings are paid by two different terms —
//! `letter_spacing` per character (which already included the spaces) and `word_spacing` per
//! separator — so 84.30 + 3×10 + 7×2 = 128.30. An implementation that pays `word_spacing` inside
//! the per-character term instead would land on a different number.
//!
//! `#w` is the control that keeps this honest: the wrapping path was already Chrome-exact, and the
//! fix must not double-pay it. It cannot — a wrapping run holds no separator, so the new term is
//! zero there and the width is byte-identical.
//!
//! ⚠ CORRECTING THE RECORD: t1371's journal and commit message state that "`word-spacing` is inert
//! in LAYOUT". **That is wrong and this file is the measurement that corrects it.** Its fixture set
//! `white-space: pre` on every row (to keep the advance measurable), so it only ever exercised the
//! one path where the property was dropped. `word-spacing: 10px` on ordinary wrapping text was
//! already 114.30 then, exactly as Chrome renders it.
use manuk_text::FontContext;

const HTML: &str = r##"<!DOCTYPE html><html><head><meta charset="utf-8"><style>
 body{margin:0}
 span{font:20px/1.2 monospace;display:inline-block}
 #n{word-spacing:normal}
 #w{word-spacing:10px}
 #pn{white-space:pre;word-spacing:normal}
 #pw{white-space:pre;word-spacing:10px}
 #pch{white-space:pre;word-spacing:1ch}
 #pnb{white-space:pre;word-spacing:10px}
 #pem{white-space:pre;word-spacing:10px}
 #pls{white-space:pre;word-spacing:10px;letter-spacing:2px}
</style></head><body>
<div><span id="n">a b c d</span></div>
<div><span id="w">a b c d</span></div>
<div><span id="pn">a b c d</span></div>
<div><span id="pw">a b c d</span></div>
<div><span id="pch">a b c d</span></div>
<div><span id="pnb">a&#160;b&#160;c&#160;d</span></div>
<div><span id="pem">a&#8195;b&#8195;c&#8195;d</span></div>
<div><span id="pls">a b c d</span></div>
</body></html>
"##;

fn w(page: &manuk_page::Page, sel: &str, want: f32, why: &str) {
    let dom = page.dom();
    let n = manuk_css::query_selector_all(dom, dom.root(), sel)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("selector {sel} matched nothing"));
    let got = page
        .root_box
        .node_rects(dom)
        .get(&n)
        .copied()
        .unwrap_or_else(|| panic!("no box for {sel}"))
        .width;
    assert!(
        (got - want).abs() < 1.01,
        "G_WORD_SPACING_SURVIVES_PRESERVED_WHITESPACE: `{sel}` expected w={want} (CAPTURED from \
         `google-chrome --headless --hide-scrollbars --window-size=1200,800`), got w={got} — {why}"
    );
}

#[test]
fn g_word_spacing_survives_preserved_whitespace() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://wordspacing.test/", &fonts, 1200.0);

    // ── CONTROLS: the wrapping path was already right and must stay byte-identical.
    w(
        &page,
        "#n",
        84.30,
        "CONTROL: `normal` is zero spacing — the number every DROPPED case produced",
    );
    w(
        &page,
        "#w",
        114.30,
        "CONTROL: ordinary wrapping text already paid word-spacing, and the fix must not \
         double-pay it",
    );
    w(
        &page,
        "#pn",
        84.30,
        "CONTROL: `pre` alone changes no advance",
    );

    // ── THE SUBJECT: the preserved-whitespace path.
    w(
        &page,
        "#pw",
        114.30,
        "a preserved space is still a word separator — this is the same 114.30 as #w, reached by \
         the other path",
    );
    w(
        &page,
        "#pch",
        120.42,
        "…and the value's UNIT resolves there too (`1ch` is 12.041px in this face)",
    );
    w(
        &page,
        "#pnb",
        114.30,
        "U+00A0 NO-BREAK SPACE is a word separator and takes the full spacing",
    );
    w(
        &page,
        "#pem",
        84.30,
        "⭐ U+2003 EM SPACE is NOT — the row that fails the obvious \"charge every whitespace \
         character\" implementation while every other row still passes",
    );
    w(
        &page,
        "#pls",
        128.30,
        "the two spacings are paid by two different terms under `pre`: letter-spacing per \
         character (spaces included) plus word-spacing per separator — 84.30 + 3x10 + 7x2",
    );
}
