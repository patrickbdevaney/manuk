//! # G_INLINE_VERTICAL_PADDING — the padded inline BOX grows, the LINE does not
//!
//! ```html
//! <a class="btn" href="/login">Login</a>       .btn { padding: 10px 20px }
//! ```
//!
//! An inline `<a>` with padding is how every tag, badge, nav pill, chip and button-styled link on the
//! web is written. CSS 2.1 §10.6.1: on a non-replaced inline, vertical padding and border **do not
//! affect line height** — but the box still has them, so the pill *overflows* its line. That overflow
//! is the entire visual point of the idiom.
//!
//! We grew neither. The box was its text's content area, so a 37px pill reported **18** and PAINTED
//! its background at 18 — half the height the author drew.
//!
//! ```text
//!                                                Chrome     before      after
//!   <a padding:10px 20px>Login</a>            [0 -9 79x37]  [0 0 79x18]  [0 -9 79x37]  ✗→✓
//!   <span padding:10px 20px>                  [0 11 77x37]  …77x18       [0 11 77x37]  ✗→✓
//!   <span padding:10px 0>    (VERTICAL only)  [0 31 61x37]  …61x17       [0 31 61x37]  ✗→✓
//!   <span border:5px solid>                   [0 76 76x27]  …76x18       [0 76 76x27]  ✗→✓
//!   <span padding:0 20px>    (HORIZONTAL only)[0 61 110x17] …110x18      …110x18       ~ 1px, see below
//!   <span display:inline-block padding:…>    [0 100 117x40] …117x40      …117x40       ✓ always right
//!   THE CONTAINING DIV                       [0 140 600x20] …600x20      [0 140 600x20] ✓ MUST NOT MOVE
//! ```
//!
//! ## The containing div is the assertion that makes this a fix and not a trade
//!
//! `close_line` folds a synthetic reporter's `line_height` in as a **floor on the line box** — right
//! for an empty inline (Chrome gives `<span id="anchor"></span>` a line-height-tall rect and a real
//! line) and wrong here. The first working version of this change reported 37 correctly on every
//! anchor **and made the containing div 37 too**, pushing every following line down the page. So a
//! padded edge now reports a tall RECT and a ZERO line-height, and `#wrap` is asserted at Chrome's
//! **20** alongside every 37.
//!
//! ## `padding: 10px 0` needs its own arm
//!
//! Vertical padding with no horizontal edge emits no spacer at all under the old condition, so
//! nothing carries the report — `#s2` stayed at 17. It gets a zero-width edge, and that edge does
//! **not** hold a line box open, because the measurement in `collect_inline_node` says only an edge
//! occupying inline flow width does.
//!
//! ⚠ **`#s3` is 18 here against Chrome's 17 and is asserted at 18 deliberately.** That is the
//! `line-height: normal` content-height rounding residual named at t802 (we resolve 18/row where
//! Chrome uses 19 at some sizes); it predates this change, is unmoved by it, and asserting Chrome's
//! 17 would make this gate fail for a reason it does not test.
//!
//! ## How this goes RED
//!
//! - **Drop `report_ascent` from the padded edge** → `#a1` reads 18 and sits at y=0 instead of −9,
//!   while `#s5` (inline-block) and `#wrap` still pass. The inline-block row is the control: padding
//!   on an *atomic* box always worked, which is why this survived.
//! - **Feed the padded height back into `line_height`** → `#wrap` becomes 37. Every height assertion
//!   above still passes, so without `#wrap` this gate is green on a change that relaid the page.
//! - **Drop the `|| v_ascent.is_some()` arm** → only `#s2` fails, at 17.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
body{margin:0;font:16px/1.25 sans-serif}
.w{width:600px}
.p{background:#8cf;padding:10px 20px}
</style></head><body>
<div class="w"><a class="p" id="a1" href="#">Login</a></div>
<div class="w"><span class="p" id="s1">Span</span></div>
<div class="w"><span id="s2" style="background:#8cf;padding:10px 0">VertOnly</span></div>
<div class="w"><span id="s3" style="background:#8cf;padding:0 20px">HorizOnly</span></div>
<div class="w"><span id="s4" style="background:#8cf;border:5px solid red">Bordered</span></div>
<div class="w"><span id="s5" style="background:#8cf;padding:10px 20px;display:inline-block">InlineBlock</span></div>
<div class="w" id="wrap"><span id="s6" style="background:#8cf;padding:10px 20px">LineHeightUnaffected</span></div>
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

fn assert_h(page: &manuk_page::Page, sel: &str, want: f32, why: &str) {
    let got = rect_of(page, sel).height;
    assert!(
        (got - want).abs() < 1.01,
        "G_INLINE_VERTICAL_PADDING: `{sel}` expected height {want} (MEASURED in headless Chrome on \
         THIS fixture), got {got}.\n  {why}"
    );
}

#[test]
fn g_inline_vertical_padding() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://inlpad.test/", &fonts, 1200.0);

    // ── THE BUG: the box grows by the vertical padding and border.
    assert_h(
        &page,
        "#a1",
        37.0,
        "`padding:10px 20px` on an inline <a>: 17px of content plus 10 above and 10 below. This is \
         the button-styled link, and it reported 18",
    );
    // …and it starts ABOVE its own text, which a height-only assertion cannot see.
    let a1 = rect_of(&page, "#a1");
    assert!(
        (a1.y - -9.0).abs() < 1.01,
        "G_INLINE_VERTICAL_PADDING: `#a1` must start ABOVE the line's content top — Chrome puts it \
         at y=-9 (the div's content top is 1, less 10px of padding). Got y={}. A box that grew \
         DOWNWARD only would pass the height assertion and paint the pill in the wrong place",
        a1.y
    );
    assert_h(
        &page,
        "#s1",
        37.0,
        "the same on a <span> — nothing is <a>-specific",
    );
    assert_h(
        &page,
        "#s2",
        37.0,
        "`padding:10px 0` — VERTICAL ONLY, so there is no horizontal edge to hang the report on and \
         it needs its own arm. It read 17",
    );
    assert_h(
        &page,
        "#s4",
        27.0,
        "a 5px BORDER does the same thing as padding: 17 + 5 + 5. If only `padding` were read, this \
         stays at 18",
    );
    assert_h(
        &page,
        "#s6",
        37.0,
        "…and again inside the wrapper asserted below",
    );

    // ── THE ASSERTION THAT MAKES THIS A FIX AND NOT A TRADE. CSS 2.1 §10.6.1: vertical padding on a
    //    non-replaced inline does NOT affect line height. The first working version of this change
    //    got every 37 right and made this 37 too, relaying the whole page below it.
    assert_h(
        &page,
        "#wrap",
        20.0,
        "the CONTAINING BLOCK is still one 20px line tall — the padded pill OVERFLOWS its line, \
         which is the whole visual point of the idiom. At 37 this gate is green on a change that \
         pushed every following line down the page",
    );

    // ── WHAT WAS ALWAYS RIGHT, and is why this survived: padding on an ATOMIC box.
    assert_h(
        &page,
        "#s5",
        40.0,
        "`display:inline-block` with the same padding has always been correct — the atomic path owns \
         its own border box. That is the shape a test-writer reaches for, and it never failed",
    );

    // ── THE RESIDUAL, asserted at OUR number with the reason, not at Chrome's.
    assert_h(
        &page,
        "#s3",
        18.0,
        "HORIZONTAL-only padding: Chrome reads 17 and we read 18. That 1px is the `line-height: \
         normal` content-height rounding named at t802 — it predates this change and is unmoved by \
         it. Asserting Chrome's 17 would make this gate fail for a reason it does not test",
    );
}
