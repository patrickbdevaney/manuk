//! # G_A_COLLAPSED_SPACE_DOES_NOT_OPEN_A_LINE — and a zero-width fragment made "first" lie
//!
//! ⚠⚠⚠ **A COLLAPSIBLE SPACE THAT ENDS A LINE IS REMOVED, AND WE CHARGED IT AGAIN AT THE START OF
//! THE NEXT ONE.** Every inline element that happened to OPEN a continuation line began one space
//! width — 9.64px at 16px monospace — to the right of where Chrome puts it. In ordinary prose that
//! is every other link.
//!
//! ⭐⭐ **THE TEST FOR "AM I FIRST ON THIS LINE?" WAS `cur.is_empty()`, AND AN INLINE ELEMENT'S
//! OPENING EDGE IS A ZERO-ADVANCE `Spacer`.** That edge wraps to the new line BEFORE the word it
//! belongs to, so the word arrives with `cur.len() == 1`, concludes it is not first, and pays for a
//! space that had already been collapsed away. Traced with `MANUK_INLINE_TRACE=1` on
//! `<p>xxx… <span>A</span>BBB</p>` at 600px:
//!
//! ```text
//!   ITEM adv=635.8 space=0.0 pen=0.0   cur=0  overflows=false   the long run, line 1
//!   ITEM adv=0.0   space=0.0 pen=635.8 cur=1  overflows=true    the span's EDGE — wraps first
//!   ITEM adv=9.6   space=9.6 pen=0.0   cur=1  overflows=false   "A" — and pays the 9.6
//! ```
//!
//! The predicate is now *"has anything on this line taken any ADVANCE yet?"* — a zero-width fragment
//! is an edge or a marker, not content, and it leaves the pen where it was.
//!
//! Every number below is CAPTURED from `google-chrome --headless --hide-scrollbars
//! --window-size=1200,800`, `font: 16px/19px monospace`, a 600px `<p>`; the column is the element's
//! x relative to its paragraph.
//!
//! ```text
//!                                                        Chrome    before   after
//!   #a  <span> OPENS the continuation line        KEY       0.00      10       0
//!   #b  <span> mid-line ON the continuation line  CTRL     48.17      48      48
//!   #c  <span> at the END of line 1               CTRL    635.77     636     636
//!   #d  <a> opening the continuation line         KEY       0.00      10       0
//!   #e  <b> opening the continuation line         KEY       0.00      10       0
//!   #f  a SHORT <a> opening the continuation line KEY       0.00      10       0
//! ```
//!
//! ⭐ **`#b` AND `#c` ARE WHY THIS IS NOT "DROP THE SPACE BEFORE EVERY INLINE".** An element that
//! sits mid-line on a continuation line keeps its space (48.17 = five characters plus one), and an
//! element that ENDS the previous line keeps its own position (635.77). Only the item that OPENS a
//! line is affected, and both of those rows were already Chrome-exact before this tick — the fix
//! must not move them.
//!
//! `#d`, `#e` and `#f` are the same defect through three element types and two content lengths,
//! because the mechanism is the wrapping EDGE and has nothing to do with which tag carries it.
use manuk_text::FontContext;

const HTML: &str = r##"<!DOCTYPE html><html><head><meta charset="utf-8"><style>
 body{margin:0;font:16px/19px monospace}
 p{width:600px;margin:0;background:#eee}
</style></head><body>
<p id="pa">xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx <span id="a">A</span>BBB CCC</p>
<p id="pb">xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx yyyy <span id="b">A</span></p>
<p id="pc">xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx<span id="c">A</span> BBB</p>
<p id="pd">xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx <a id="d" href="#">wraps across the line boundary here</a> tail</p>
<p id="pe">xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx <b id="e">wraps across the line boundary here</b> tail</p>
<p id="pf">xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx <a id="f" href="#">short</a> tail</p>
</body></html>
"##;

/// The element's x relative to its own paragraph — the paragraph is the line's origin, so this is
/// exactly "where on the line did it start".
fn relx(page: &manuk_page::Page, id: &str, owner: &str) -> f32 {
    let dom = page.dom();
    let pick = |sel: &str| {
        manuk_css::query_selector_all(dom, dom.root(), sel)
            .first()
            .copied()
            .unwrap_or_else(|| panic!("selector {sel} matched nothing"))
    };
    let rects = page.root_box.node_rects(dom);
    let me = rects.get(&pick(&format!("#{id}"))).copied().expect("box");
    let p = rects
        .get(&pick(&format!("#{owner}")))
        .copied()
        .expect("p box");
    me.x - p.x
}

fn at(page: &manuk_page::Page, id: &str, owner: &str, want: f32, why: &str) {
    let got = relx(page, id, owner);
    assert!(
        (got - want).abs() < 1.01,
        "G_A_COLLAPSED_SPACE_DOES_NOT_OPEN_A_LINE: `#{id}` expected x={want} relative to its \
         paragraph (CAPTURED from `google-chrome --headless --hide-scrollbars \
         --window-size=1200,800`), got x={got} — {why}"
    );
}

#[test]
fn g_a_collapsed_space_does_not_open_a_line() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://collapsedspace.test/", &fonts, 1200.0);

    at(
        &page,
        "a",
        "pa",
        0.0,
        "a <span> that OPENS a continuation line starts at the line's own edge — the space that \
         ended the previous line was collapsed away and must not be charged twice",
    );
    at(
        &page,
        "b",
        "pb",
        48.17,
        "⭐ CONTROL: an element MID-LINE on a continuation line still pays its space — the fix is \
         not `drop the space before every inline`",
    );
    at(
        &page,
        "c",
        "pc",
        635.77,
        "⭐ CONTROL: an element at the END of the first line keeps its own position",
    );
    at(&page, "d", "pd", 0.0, "…and an <a> is no different");
    at(
        &page,
        "e",
        "pe",
        0.0,
        "…nor a <b>: the mechanism is the wrapping EDGE, not the tag",
    );
    at(
        &page,
        "f",
        "pf",
        0.0,
        "…and it does not depend on how much content the element holds",
    );
}
