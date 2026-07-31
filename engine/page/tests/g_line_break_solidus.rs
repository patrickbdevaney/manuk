//! # G_LINE_BREAK_SOLIDUS — Chrome does not break a line after `/`, and URLs are where it shows
//!
//! UAX #14 offers a break opportunity after a solidus (class SY, whose only member is U+002F), and
//! `unicode-linebreak` reports it faithfully. **Blink tailors it away**: a long URL overflows its box
//! in Chrome rather than wrapping at its path separators. We took the opportunity, so every URL,
//! file path and breadcrumb in body text produced a different set of line boxes — and a different
//! line count is a different height for the container and a `dy` for everything below it.
//!
//! ## Measured, at a 120px width, heights in px (`--dump-dom` + `getBoundingClientRect`)
//!
//! ```text
//!                                                  Chrome   ours (before)
//!   aaaa/bbbb/cccc/dddd                              19          38
//!   https://example.com/very/long/path/here          19          77
//!   one/two three/four five/six seven/eight          77          58
//!   the URL again, with `overflow-wrap: break-word`    58          58
//! ```
//!
//! **The third row is the one that proves this is not "Chrome wraps less".** Chrome takes MORE lines
//! there: refusing the `/` opportunity means a whole token has to move down. The error is not a bias
//! in one direction — it is a different set of line boxes, which is why a "we overflow slightly less
//! often" reading of the first two rows would have been the wrong lesson.
//!
//! ## What must NOT change, and is asserted here
//!
//! * **Every other separator already agreed** — `- . _ ? = & , : +`, numeric dates, CJK, soft
//!   hyphens, U+200B — so this is a one-character tailoring, not a quarrel with the crate. The
//!   hyphen and the zero-width space are asserted so a broader "stop breaking inside words" fix
//!   cannot pass this gate.
//! * **`overflow-wrap: break-word` is a different path** (`InlineItem::break_word`). A page that
//!   asks for the URL to be broken must still get it broken — that is the half which would make
//!   this a regression rather than a fix, and it has its own assertion.
//!
//! ## How this goes RED
//!
//! - **Remove the solidus guard from `break_segments`** → `#s1`, `#d4` and `#d5` all fail at once.
//! - **Suppress the break-word path too** → `#w1` fails while the others pass.
//! - **Suppress hyphen breaks as well** (the over-broad version of this fix) → `#s2` fails.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
body{margin:0;font:16px/1.2 sans-serif}
.b{width:120px}
#w1{width:120px;overflow-wrap:break-word}
</style></head><body>
<div class="b" id="s1">aaaa/bbbb/cccc/dddd</div>
<div class="b" id="s2">aaaa-bbbb-cccc-dddd</div>
<div class="b" id="d4">https://example.com/very/long/path/here</div>
<div class="b" id="d5">one/two three/four five/six seven/eight</div>
<div class="b" id="d6">alpha&#8203;beta&#8203;gamma&#8203;delta&#8203;epsilon</div>
<div id="w1">https://example.com/very/long/path/here</div>
</body></html>"##;

fn height_of(page: &manuk_page::Page, sel: &str) -> f32 {
    let dom = page.dom();
    let n = manuk_css::query_selector_all(dom, dom.root(), sel)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("selector {sel} matched nothing"));
    page.root_box
        .node_rects(dom)
        .get(&n)
        .unwrap_or_else(|| panic!("no box for {sel}"))
        .height
}

fn assert_h(page: &manuk_page::Page, sel: &str, want: f32, why: &str) {
    let got = height_of(page, sel);
    assert!(
        (got - want).abs() < 1.01,
        "G_LINE_BREAK_SOLIDUS: `{sel}` expected {want}px tall (MEASURED in Chrome), got {got}.\n  {why}"
    );
}

#[test]
fn g_line_break_solidus() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://break.test/", &fonts, 1200.0);

    assert_h(
        &page,
        "#s1",
        19.0,
        "`aaaa/bbbb/cccc/dddd` stays on ONE line and overflows its 120px box — Chrome does not take \
         the UAX #14 break after a solidus",
    );
    assert_h(
        &page,
        "#d4",
        19.0,
        "a real URL, the case this is actually about: one line, overflowing",
    );
    assert_h(
        &page,
        "#d5",
        77.0,
        "FOUR lines — MORE than the three we used to produce. Refusing the `/` opportunity moves a \
         whole token down, which is what makes this a different set of line boxes rather than \
         'Chrome wraps less'",
    );

    // ── The guards. Each of these passes today and must keep passing, or the fix has been widened
    // into a different bug.
    assert_h(
        &page,
        "#s2",
        38.0,
        "a HYPHEN is still a break opportunity — `aaaa-bbbb-cccc-dddd` wraps to two lines. An \
         over-broad 'stop breaking inside words' fix reads 19 here",
    );
    assert_h(
        &page,
        "#d6",
        58.0,
        "U+200B ZERO WIDTH SPACE still breaks — it exists for nothing else",
    );
    assert_h(
        &page,
        "#w1",
        58.0,
        "`overflow-wrap: break-word` is a DIFFERENT path and still breaks the same URL — Chrome \
         reads 58 here, THREE lines, not the four an ordinary break-at-every-slash would give. A \
         page that asks for the break must get it, or this fix is a regression wearing a parity \
         number",
    );
}
