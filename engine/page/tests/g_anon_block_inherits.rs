//! # G_ANON_BLOCK_INHERITS — an anonymous block box inherits from the container that made it
//!
//! CSS 2.1 §9.2.1.1: when a block container holds *both* inline and block-level children, the runs
//! of inline content are wrapped in **anonymous block boxes**, and those boxes inherit every
//! inheritable property from the block container that generated them.
//!
//! `flush_inline_run` — the function that builds them — passed `layout_inline` the literals
//! `TextAlign::Left, 0.0, …, None` where the pure-IFC branch of the same file passes
//! `bcs.text_align, text_indent, …, Some(&bcs)`. **The anonymous twin was constructed with no
//! inherited context at all**, and two symptoms fell out of the one omission:
//!
//! ```text
//!                                                      Chrome   before   after
//!   inline-block in text-align:center, INLINE-ONLY       350      350      350   ✓ always right
//!   …the same, with ONE block-level sibling             350        0      350   ✗→✓
//!   …the inline run AFTER the block child               350        0      350   ✗→✓
//!   …between two block children                         350        0      350   ✗→✓
//!   text-align:right, mixed                             700        0      700   ✗→✓
//!   <center> with a block child (the linkmake.in shape) 350        0      350   ✗→✓
//!   align inherited from a GRANDPARENT                  350        0      350   ✗→✓
//!   a plain TEXT run, centred, mixed                    344        0      344   ✗→✓
//!   default (left) with a block child                     0        0        0   ✓ must not move
//!
//!   the anonymous line box's HEIGHT (20px inline-block)   24       20       24   ✗→✓  (the STRUT)
//! ```
//!
//! **The strut half.** With `strut_style: None` the line box carries a zero strut, so a line whose
//! only content is an atomic inline-block was exactly the inline-block's height — Chrome adds the
//! containing block's font DESCENT below the baseline the inline-block sits on. A *text* run was
//! already right, because each fragment's own inherited `line-height` covers it; only the atomic
//! case exposed the gap, which is why this survived so long.
//!
//! ⚠ **A test in `layout/src/lib.rs` had frozen the missing strut as ground truth** —
//! `inline_block_boxes_flow_horizontally_then_a_block_drops_below` asserted the following block
//! lands at `y=30` and its comment claimed the number was *"verified numerically against Chrome by
//! the parity harness"*. Headless Chrome on that exact markup says **34**. The assertion was the
//! defect's last line of defence; it is now the Chrome number, with the story attached.
//!
//! ## The real-site evidence
//!
//! `linkmake.in` (in-scope CrUX corpus, coverage 1.00) is `<center><b>Paste Link URLs</b><br>…
//! <textarea class="form-control">…</center>` — inline copy and a block-level control in one
//! container. Its whole centred column rendered flush left: `<b>` at x=170 against Chrome's 537,
//! `<font>` at 170 against 273, `<small>` at 170 against 491. **shape 0.622 → 0.703.**
//!
//! ## How this goes RED
//!
//! - **Restore `TextAlign::Left`** → the eight centred/right cases collapse to x=0, `#a7` (the
//!   left-aligned control) still passes, and so does `#a1` — which is the point: `#a1` is the same
//!   markup MINUS the block child and it was never broken. A gate without it cannot tell "we lost
//!   text-align" from "text-align never worked".
//! - **Restore `strut_style: None`** → every mixed container is 4px short (`#q2` 44 → 40) and the
//!   x assertions all still pass. The two halves are independently falsifiable.
//!
//! ## What is NOT claimed, and is asserted as-is rather than papered over
//!
//! `#a8` — a `float:left` sibling AFTER an inline run — is **370 in Chrome and 350 here**, because
//! we flush the pending run *before* registering the float, so the float drops to the next line
//! instead of sharing this one and the run centres in the full width rather than the float-narrowed
//! band. Pre-existing, independent of this fix, and untouched by it. The case stays in the fixture
//! because it is the only cover for the FLOAT call site of `flush_inline_run` (one of three), and
//! it asserts what this tick actually delivers there — the run is CENTRED, not at x=0 — with the
//! residual named. **A gate must not assert a number it knows is wrong; it may assert the weaker
//! property it can prove.**

use manuk_text::FontContext;

/// Widths are carried by fixed-size `inline-block`s, not by text, so every assertion below is
/// **font-independent**: `(800 − 100) / 2 = 350` holds whatever face the box happens to resolve.
/// `#t10` is the one text case and is asserted as a band for exactly that reason.
const HTML: &str = r##"<!doctype html><html><head><style>
body{margin:0;font:16px/1.2 sans-serif}
.w{width:800px}
.b{display:block;width:100px;height:20px;background:#ddd}
.ib{display:inline-block;width:100px;height:20px;background:#8cf}
.f{float:left;width:60px;height:20px;background:#fc8}
</style></head><body>
<div class="w" style="text-align:center" id="q1"><span class="ib" id="a1"></span></div>
<div class="w" style="text-align:center" id="q2"><span class="ib" id="a2"></span><div class="b" id="k2"></div></div>
<div class="w" style="text-align:center" id="q3"><div class="b" id="k3"></div><span class="ib" id="a3"></span></div>
<div class="w" style="text-align:center" id="q4"><div class="b" id="k4a"></div><span class="ib" id="a4"></span><div class="b" id="k4b"></div></div>
<div class="w" style="text-align:right" id="q5"><span class="ib" id="a5"></span><div class="b" id="k5"></div></div>
<div class="w" id="q6"><center id="c6"><span class="ib" id="a6"></span><div class="b" id="k6"></div></center></div>
<div class="w" id="q7"><span class="ib" id="a7"></span><div class="b" id="k7"></div></div>
<div class="w" style="text-align:center" id="q9"><div id="n9"><span class="ib" id="a9"></span><div class="b" id="k9"></div></div></div>
<div class="w" style="text-align:center" id="q10"><span id="t10">centred text run</span><div class="b" id="k10"></div></div>
<div class="w" style="text-align:center" id="q8"><span class="ib" id="a8"></span><div class="f" id="fl8"></div></div>
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
        "G_ANON_BLOCK_INHERITS: `{sel}` expected x {want} (MEASURED in headless Chrome on THIS \
         fixture), got {got}.\n  {why}"
    );
}

fn assert_h(page: &manuk_page::Page, sel: &str, want: f32, why: &str) {
    let got = rect_of(page, sel).height;
    assert!(
        (got - want).abs() < 1.01,
        "G_ANON_BLOCK_INHERITS: `{sel}` expected height {want} (MEASURED in headless Chrome on \
         THIS fixture), got {got}.\n  {why}"
    );
}

#[test]
fn g_anon_block_inherits() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://anon.test/", &fonts, 1200.0);

    // ── THE CONTROL, and it is the half that makes the rest mean anything. Same markup as `#q2`
    //    with the block child removed, so it takes the pure-IFC path and was ALWAYS correct.
    assert_x(
        &page,
        "#a1",
        350.0,
        "inline-only container: text-align:center has always worked here, and must keep working — \
         without this the gate cannot distinguish a lost inheritance from an unimplemented property",
    );

    // ── THE BUG: one block-level sibling is the entire difference.
    assert_x(
        &page,
        "#a2",
        350.0,
        "one block-level sibling forces the inline run into an anonymous block; it still inherits \
         text-align:center",
    );
    assert_x(
        &page,
        "#a3",
        350.0,
        "the run AFTER the block child — the trailing flush, a different call site",
    );
    assert_x(
        &page,
        "#a4",
        350.0,
        "a run BETWEEN two block children — the mid-loop flush",
    );
    assert_x(
        &page,
        "#a5",
        700.0,
        "text-align:right inherits the same way: 800 − 100. If only `center` were special-cased \
         this would read 350 or 0",
    );
    assert_x(
        &page,
        "#a6",
        350.0,
        "`<center>` + a block child — the UA sheet's `center{text-align:center}` reaching an \
         anonymous block. This is literally linkmake.in's markup",
    );
    assert_x(
        &page,
        "#a9",
        350.0,
        "declared on the GRANDPARENT: the value arrives by inheritance, not by matching a rule on \
         the generating container",
    );

    // ── A real TEXT run, not an inline-block. Asserted as a band because its width is the font's,
    //    and the gate must not be a font-metrics test wearing an alignment test's name. Chrome
    //    reads 344 for this string; anywhere near the left edge is the defect.
    let t10 = rect_of(&page, "#t10").x;
    assert!(
        t10 > 250.0,
        "G_ANON_BLOCK_INHERITS: a centred TEXT run in a mixed container sits near the middle of \
         800px (Chrome: 344), got x={t10}. At ~0 the anonymous block lost text-align."
    );

    // ── WHAT MUST NOT MOVE: the default is still left.
    assert_x(
        &page,
        "#a7",
        0.0,
        "no text-align declared: the anonymous block inherits `left` and stays at the left edge. \
         A fix that centred everything would pass every assertion above and fail this one",
    );
    assert_x(
        &page,
        "#k2",
        0.0,
        "the BLOCK child is not affected by text-align at all — it is not inline content. Chrome \
         puts it at 0 (`<center>`'s `-webkit-center` would not, and we do not implement it)",
    );

    // ── THE STRUT HALF: the anonymous line box carries the container's font descent.
    assert_h(
        &page,
        "#q1",
        24.0,
        "the pure-IFC control: a 20px inline-block on a 16px/1.2 line is a 24px line box",
    );
    assert_h(
        &page,
        "#q2",
        44.0,
        "…and the anonymous twin must agree: 24 for the line + 20 for the block child. It read 40 \
         while `strut_style` was None",
    );
    assert_h(
        &page,
        "#q4",
        64.0,
        "20 + 24 + 20 — the strut applies to a run wrapped between two blocks too",
    );
    assert_h(
        &page,
        "#c6",
        44.0,
        "the `<center>` case measures the same, so the fix is not keyed on the alignment value",
    );

    // ── THE FLOAT CALL SITE — the third and last caller. Asserted as the property this tick
    //    proves (the run is centred) and NOT as Chrome's 380, which we do not yet produce for an
    //    unrelated, named reason: we flush the run before registering the float, so the float
    //    drops a line instead of narrowing this one.
    let a8 = rect_of(&page, "#a8").x;
    assert!(
        a8 > 250.0,
        "G_ANON_BLOCK_INHERITS: a run flushed by a FLOAT sibling must still inherit text-align \
         (Chrome 380 in the float-narrowed band; 350 here in the full width — see the module doc), \
         got x={a8}. At 0 the float call site is still passing the literal."
    );
}
