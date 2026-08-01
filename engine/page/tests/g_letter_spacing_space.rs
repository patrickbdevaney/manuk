//! # G_LETTER_SPACING_SPACE — the space is a character
//!
//! `letter-spacing` adds a fixed advance after every character. We added it once per character of
//! each **word** and stopped there, so an inter-word space was the one character on the line that
//! did not get it.
//!
//! That is the hardest shape a layout defect takes: **every word's own box stays exactly right while
//! its POSITION falls one `letter-spacing` behind per preceding space**, cumulatively along the line.
//! The thing you would think to measure — the word's width — is correct, so the error is only
//! visible as a drift that grows with the sentence.
//!
//! ```text
//!   letter-spacing:2px, 16px sans-serif        Chrome   before   after
//!     2nd word   (one preceding space)           39       37       39    ✗→✓
//!     4th word   (three preceding spaces)       115      109      115    ✗→✓
//!   word-spacing:5px  (the sibling property)
//!     2nd word                                   36       36       36    ✓ always right
//!     4th word                                  106      106      106    ✓ always right
//!   no spacing at all — three faces
//!     sans-serif / Arial / Georgia            31/91/…  31/91/…  31/91/…  ✓ must not move
//! ```
//!
//! The arithmetic is what identifies it rather than a fudge: at the 4th word Chrome has advanced
//! **12 characters × 2px** and we had advanced **9 × 2px** — exactly the three spaces, and nothing
//! else, missing.
//!
//! `letter-spacing` on nav bars, buttons, headings and uppercase labels is design-system standard
//! (`letter-spacing: .05em` and friends), so this rides on a large share of the chrome of the modern
//! web — and on every one of those runs, everything after the first word was in the wrong place.
//!
//! ## How this goes RED
//!
//! - **Drop `letter_spacing` from the space advance** → `#e1` reads 37 and `#e2` 109, while the
//!   `word-spacing` rows and all three no-spacing faces still pass. The split matters: `word-spacing`
//!   was ALWAYS applied to the space, so a gate built on it is green against this defect — the two
//!   properties are adjacent in the source and only one of them was wrong.
//! - **Add it to the space but drop it from the word** → `#e1`/`#e2` widths fall from 33 to 27 while
//!   their x positions still pass, which is why the widths are asserted alongside the positions.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
body{margin:0;font-family:sans-serif;font-size:16px}
.w{width:900px}
</style></head><body>
<div class="w"><p id="p1">aaa <span id="x1">bbb</span> ccc <span id="x2">ddd</span></p></div>
<div class="w" style="font-family:Georgia,serif"><p id="p3">aaa <span id="g1">bbb</span> ccc <span id="g2">ddd</span></p></div>
<div class="w" style="word-spacing:5px"><p id="p4">aaa <span id="w1">bbb</span> ccc <span id="w2">ddd</span></p></div>
<div class="w" style="letter-spacing:2px"><p id="p5">aaa <span id="e1">bbb</span> ccc <span id="e2">ddd</span></p></div>
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

fn assert_x(page: &manuk_page::Page, sel: &str, want: f32, why: &str) {
    let got = rect_of(page, sel).x;
    assert!(
        (got - want).abs() < 1.01,
        "G_LETTER_SPACING_SPACE: `{sel}` expected x {want} (MEASURED in headless Chrome on THIS \
         fixture), got {got}.\n  {why}"
    );
}

#[test]
fn g_letter_spacing_space() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://ls.test/", &fonts, 1200.0);

    // ── THE BUG: two words, because the error is CUMULATIVE and one word cannot show a slope.
    assert_x(
        &page,
        "#e1",
        39.0,
        "`letter-spacing:2px`, 2nd word: one preceding space, so 2px further along than the run \
         without it. We read 37 — the space was the one character that missed out",
    );
    assert_x(
        &page,
        "#e2",
        115.0,
        "…and the 4th word: Chrome has advanced 12 characters × 2px, we had advanced 9 × 2px. The \
         difference is exactly the three spaces, which is what makes this arithmetic and not a fudge",
    );
    // The WIDTH must not move — `letter-spacing` was already applied inside a word, and a fix that
    // moved the advance from the word to the space would pass both assertions above.
    let e1 = rect_of(&page, "#e1");
    assert!(
        (e1.width - 33.0).abs() < 1.01,
        "G_LETTER_SPACING_SPACE: `#e1` width expected 33 (Chrome) — `letter-spacing` inside the word \
         was ALREADY right and must stay so, got {}",
        e1.width
    );

    // ── THE SIBLING PROPERTY, one line away in the source, and always correct. Without this row a
    //    gate on "spacing" is green against the defect.
    assert_x(
        &page,
        "#w1",
        36.0,
        "`word-spacing:5px` has always been added to the space — this is the control that says the \
         bug was specific to `letter-spacing`, not to spacing in general",
    );
    assert_x(
        &page,
        "#w2",
        106.0,
        "…and it stays right at the 4th word too",
    );

    // ── NO SPACING AT ALL, two faces: the ordinary path must not move by a pixel.
    assert_x(&page, "#x1", 31.0, "no spacing, sans-serif — unchanged");
    assert_x(
        &page,
        "#x2",
        91.0,
        "no spacing, sans-serif, 4th word — unchanged",
    );
    assert_x(
        &page,
        "#g1",
        25.0,
        "no spacing, Georgia — a different face, because a change to the space advance would show \
         up differently on a different font's space width",
    );
    assert_x(&page, "#g2", 79.0, "…and its 4th word");
}
