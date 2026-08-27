//! # G_STATIC_POS_AFTER_TEXT — preceding **bare text** advances the static position too
//!
//! CSS 2.1 §10.3.7 / §10.6.4: an out-of-flow box with `auto` insets sits at its **static
//! position** — where it would have been had it stayed in flow. `refine_inline_static_positions`
//! resolves that by walking the in-flow siblings that PRECEDE the box and taking the furthest point
//! flow reached, selecting the fragments that belong to those siblings with:
//!
//! ```rust,ignore
//!     if f.node.is_some_and(|n| before.contains(&n)) { … }
//! ```
//!
//! ⚠⚠⚠ **`TextFragment::node` is the deepest ELEMENT ancestor, which for bare text is the box's own
//! PARENT — and a parent is never one of its children's siblings.** So every fragment of every
//! bare text node was silently skipped, and the box landed at the content-box origin as though
//! nothing preceded it. Measured, one variable per row (`offsetLeft,offsetTop`, Ahem at 25px in a
//! 100px box):
//!
//! ```text
//!   preceding in-flow content                before    after / Chrome
//!   text "XX", abspos display:inline           0,0        50,0
//!   text "XX", abspos display:block            0,0        50,0
//!   text "XX", abspos is a <span>              0,0        50,0
//!   <span>XX</span>  (an ELEMENT)             50,0        50,0    <- CONTROL: already exact
//!   text "XX<br>XX"                           50,0        50,25
//!   the same inside a GRID item               25,0        50,25
//!   the same inside a FLEX item               25,0        50,25
//! ```
//!
//! **Row 4 is what named the mechanism.** Wrap the identical text in a `<span>` and the position
//! was already Chrome-exact; leave it bare and the box did not move at all. The `<br>` row is the
//! same fact in disguise — `<br>` *is* an element, so its fragment (at the end of line 1) counted
//! while the text after it did not, which is why that row was right in `x` and wrong in `y`.
//!
//! ⚠ **Block, flex and grid gave BYTE-IDENTICAL numbers**, which is what says this is one mechanism
//! in inline layout rather than three in the container algorithms. It is the whole of
//! `css/css-grid/abspos/positioned-grid-descendants-*` — 32 files, **3,200 subtests**, a flat zero
//! whose fixture is `X<br />XX` before the abspos: we reported `offsetLeft` 30 where Chrome says 55,
//! and 30 is exactly `padding-left 5 + the <br>'s x of 25`. `-001.html` is **100/100** after this.
//!
//! **The fix is a field, not a heuristic.** `TextFragment::origin` carries the text node the run
//! came from, beside the element it is attributed to. Ordering by position in the `frags` vector
//! instead is the tempting cheap version and is wrong: a container can hold bare text on **both
//! sides** of the out-of-flow box, and nothing in `frags` says which side a run is on. Row
//! `text_both_sides` below is that case, and it is why the cheap version does not ship.
//!
//! **To watch it go RED: drop `|| f_origin.is_some_and(…)` from `refine_inline_static_positions`.**
//! Every `after_text*` row collapses to 0 while `after_element` — the control — does not move.
//!
//! ONE `#[test]` per file — `PageContext` is per-process (see `g_caption_paint.rs`).

use manuk_text::FontContext;

const W: f32 = 800.0;

/// ⚠ **No hard-coded glyph widths.** Every assertion below compares an out-of-flow box's static
/// position against `#ruler`, an in-flow `<span>` holding the *same* text in the *same* font — so
/// the gate means the same thing on a host with different fonts installed, and a font change cannot
/// turn it red for a reason that has nothing to do with static positions.
const HTML: &str = r##"<!doctype html><html><head><style>
body { margin: 0; font: 25px/1 monospace }
div.cb { width: 400px; height: 90px; position: relative }
b { display: inline-block; width: 4px; height: 4px; position: absolute }
span.m { font-style: normal }
</style></head><body>
<div class="cb" id="cb_ruler"><span class="m" id="ruler">XX</span><br><span class="m" id="ruler_l2">XX</span></div>

<div class="cb" id="cb_after_text">XX<b id="after_text"></b></div>
<div class="cb" id="cb_after_text_block">XX<b id="after_text_block" style="display:block"></b></div>
<div class="cb" id="cb_after_element"><span class="m">XX</span><b id="after_element"></b></div>
<div class="cb" id="cb_after_text_line2">XX<br>XX<b id="after_text_line2"></b></div>
<div class="cb" id="cb_in_grid" style="display:grid"><div>XX<br>XX<b id="in_grid"></b></div></div>
<div class="cb" id="cb_in_flex" style="display:flex"><div>XX<br>XX<b id="in_flex"></b></div></div>
<div class="cb" id="cb_text_both_sides">XX<b id="text_both_sides"></b>XXXX</div>
<div class="cb" id="cb_nothing_before"><b id="nothing_before"></b>XX</div>
</body></html>"##;

#[test]
fn preceding_bare_text_advances_an_out_of_flow_boxs_static_position() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://example.test/", &fonts, W);
    let dom = page.dom();
    let rects = page.root_box.node_rects(dom);
    let raw = |id: &str| -> (f32, f32, f32, f32) {
        let sel = format!("#{id}");
        let n = manuk_css::query_selector_all(dom, dom.root(), &sel)
            .first()
            .copied()
            .unwrap_or_else(|| panic!("selector {sel} matched nothing"));
        let r = rects.get(&n).unwrap_or_else(|| panic!("no box for {sel}"));
        (r.x, r.y, r.width, r.height)
    };
    // ⚠ `node_rects` is in DOCUMENT coordinates and every row lives in its own 90px-tall block, so
    // the second row's `y` is 90 before anything is wrong. A static position is only meaningful
    // relative to the containing block, so state it that way.
    let rect = |id: &str| -> (f32, f32) {
        let (x, y, _, _) = raw(id);
        let (cx, cy, _, _) = raw(&format!("cb_{id}"));
        (x - cx, y - cy)
    };

    // The ruler measures BOTH quantities on this host's own face: how wide "XX" is, and how far
    // one line advances. ⚠ The line ADVANCE is not the span's rect height — with `font: 25px/1` the
    // content area is 29px tall and the line box is 25 — so it is measured as the gap between two
    // stacked rulers rather than read off one of them.
    let (_rx, ruler_y, ruler_w, _rh) = raw("ruler");
    let (_r2x, ruler2_y, _r2w, _r2h) = raw("ruler_l2");
    let two_chars = ruler_w;
    let line_h = ruler2_y - ruler_y;
    assert!(
        two_chars > 1.0 && line_h > 1.0,
        "the ruler must actually measure something — got width {two_chars}, height {line_h}. \
         Every row below is stated relative to it, so a zero-width ruler would make this gate \
         vacuous rather than failing."
    );

    let near = |got: f32, want: f32| (got - want).abs() < 1.5;

    // ── THE DEFECT. Bare text before the box advances it, exactly as an element would — for an
    //    INLINE-LEVEL box.
    {
        let (x, y) = rect("after_text");
        assert!(
            near(x, two_chars) && near(y, 0.0),
            "`after_text`: an INLINE-level out-of-flow box after bare text sits where flow had got \
             to — ({two_chars}, 0), the end of that text. Got ({x}, {y}).\n  \
             `TextFragment::node` is the deepest ELEMENT ancestor, so for bare text it is this \
             box's own PARENT, which is never in the set of its preceding SIBLINGS — every \
             bare-text fragment was skipped and the box stayed at the content-box origin."
        );
    }

    // ── ⚠⚠⚠ **AND THE BLOCK-LEVEL ROW WAS ASSERTING A VALUE CHROME DOES NOT PRODUCE** (corrected
    //    at t1358). It was in the loop above, claiming the same `({two_chars}, 0)` as its
    //    inline-level neighbour — generalised from that neighbour, never measured. Re-run on THIS
    //    fixture, `google-chrome --headless=new`, container-relative:
    //
    //    ```text
    //      <b id=after_text>                          (30.1, 0)    ← inline-block, stays on the line
    //      <b id=after_text_block style="display:block">  (0, 25)  ← a NEW LINE
    //      <div style="position:absolute">            (0, 25)      ← the same, unprefixed
    //    ```
    //
    //    CSS 2.1 §10.6.4 places an out-of-flow box where *"a hypothetical box … if its `position`
    //    property had been `static`"* would go, and a BLOCK-level hypothetical box does not go on
    //    the current line — it opens the next one. ⚠ `display` cannot distinguish these: an
    //    out-of-flow box BLOCKIFIES, so all three rows above compute `block` in Chrome and here.
    //    `ComputedStyle::display_in_flow` is the value before that.
    //
    //    ⚠ The comment table further up this file lists `text "XX", abspos display:block → 50,0`
    //    as a Chrome value. It is not one: `50` is an Ahem width and this fixture is 25px
    //    monospace, so that row was carried over from a different fixture and never re-measured.
    {
        let (x, y) = rect("after_text_block");
        assert!(
            near(x, 0.0) && near(y, line_h),
            "`after_text_block`: a BLOCK-level out-of-flow box after bare text opens a NEW LINE — \
             (0, {line_h}), not the end of the text. Got ({x}, {y}). Reading ({two_chars}, 0) is \
             the inline-level answer applied to a block-level box, which is what this gate asserted \
             until t1358 measured it."
        );
    }

    // ── CONTROL. The identical text inside a <span> was ALWAYS right, and must stay right. This
    //    row is what localises the defect to bare text rather than to the static position at large.
    let (ex, ey) = rect("after_element");
    assert!(
        near(ex, two_chars) && near(ey, 0.0),
        "CONTROL `after_element`: the same text in a <span> was already exact and must not move — \
         got ({ex}, {ey}), want ({two_chars}, 0)"
    );

    // ── The `<br>` row, in all three container types. `<br>` IS an element, so its fragment was
    //    counted and the text AFTER it was not: right in x, wrong in y. Chrome puts the box at the
    //    end of the SECOND line.
    for id in ["after_text_line2", "in_grid", "in_flex"] {
        let (x, y) = rect(id);
        assert!(
            near(x, two_chars) && near(y, line_h),
            "`{id}`: after `XX<br>XX` the box sits at the end of the SECOND line — \
             ({two_chars}, {line_h}). Got ({x}, {y}).\n  \
             Block, flex and grid must agree here: this is one mechanism in inline layout, not \
             three in the container algorithms, and the three rows agreeing is the evidence."
        );
    }

    // ── WHY THE CHEAP FIX DOES NOT SHIP. Bare text on BOTH sides: only the text BEFORE the box may
    //    advance it. Ordering fragments by their index in `frags` would put this box after all
    //    six characters; identity puts it after two.
    let (bx, _) = rect("text_both_sides");
    assert!(
        near(bx, two_chars),
        "`text_both_sides`: only the text BEFORE the box advances it — want {two_chars}, got {bx}. \
         A container can hold bare text on both sides, which is why the fragment carries the TEXT \
         NODE it came from rather than being ordered by its position in the fragment list."
    );

    // ── CONTROL. Nothing before it at all: the line's start edge, unchanged by this work
    //    (that branch is G_STATIC_POS_LINE_START's).
    let (nx, _) = rect("nothing_before");
    assert!(
        near(nx, 0.0),
        "CONTROL `nothing_before`: a box that OPENS the line still starts at the line's start \
         edge — want 0, got {nx}"
    );
}
