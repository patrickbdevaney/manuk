//! # G_INLINE_BOX_LEADING — every inline box contributes its leading, text or no text
//!
//! ```html
//! <span class="h1"><span class="txt">Heading</span></span>   <!-- the typographic wrapper -->
//! <a><span>label</span></a>                                   <!-- across font sizes -->
//! <span class="icon"><i></i></span>                           <!-- the icon wrapper -->
//! ```
//!
//! CSS 2.1 §10.8 is unconditional: **every** inline box contributes its `line-height` to the line
//! box, whether or not it *directly* contains text. Ours contributed only through the fragments its
//! text produced — the line box is folded over `LineFrag`s, `LineFrag`s come from `InlineItem`s, and
//! `collect_inline_node` emitted an item only for text, atomics, spacers and breaks. **An element
//! that merely wraps another inline element emitted nothing**, so it never reached the fold and was
//! invisible to the line box.
//!
//! This is a **`dy`** error, which is the term `PHASE0-RENDER-BURNDOWN.md` says dominates: one wrong
//! line box displaces every element below it, on every page that has a typographic wrapper.
//!
//! ## Chrome-measured on THIS fixture
//!
//! `google-chrome --headless --hide-scrollbars --window-size=1200,800`, a `font:16px/1.5 sans-serif`
//! div (strut 24), 400px wide. The measured quantity is the DIV's height — the line box:
//!
//! ```text
//!                                                        Chrome   before   after
//!   <span 24px>outer24</span>                 (CONTROL)    36       36      36
//!   <span 24px><span 12px>nested</span></span>             36       24      36   ✗→✓
//!   <span 12px>small only</span>              (CONTROL)    24       24      24
//!   <span 24px>big</span> and 16px            (CONTROL)    36       36      36
//!   plain 16                                  (CONTROL)    24       24      24
//!   line-height:normal + the nested pair                   28       18      28   ✗→✓
//!   <span 24px;line-height:1.5><span 12px>…</span></span>  36       24      36   ✗→✓
//!   <span 24px><span 12px>n</span>x</span>    (CONTROL)    36       36      36
//! ```
//!
//! ⚠⚠⚠ **THE LAST CONTROL IS THE DISCRIMINATOR AND IT IS WHAT IDENTIFIES THE MECHANISM.** Give the
//! outer span **one character** of its own text and we were already exact at 36. A metrics, font or
//! rounding error would have moved the other four CONTROLs too; only a *structural* one — a box that
//! produced no fragment was never in the fold — leaves text-bearing elements untouched and breaks
//! exactly the text-less ones.
//!
//! ⚠⚠ **`line-height: normal` is a SEPARATE row on purpose.** The `1.5` rows all resolve through
//! arithmetic on the font-size; the `normal` row resolves through the FACE
//! (`round(ascent + descent + lineGap)` = 28 for a 24px Liberation Sans). A fix that read
//! `ComputedStyle::line_height` directly passes every `1.5` row and fails this one, because that
//! field holds the raw `1.2 × font-size` fallback (28.8) rather than the face's answer — **which is
//! exactly the bug the first draft of this fix had**, caught by `manuk-layout`'s button-centring test
//! reading 19.2 where a plain line is 18. One rule, two implementations; the fix now routes through
//! `text_style`, the same function every other item on the line uses.
//!
//! ## How this goes RED
//!
//! - **Delete the per-element `Spacer`** in `collect_inline_node` → the three ✗→✓ rows fall back to
//!   24 / 18 / 24 while all five CONTROLs stay green. Verified.
//! - **Read `ComputedStyle::line_height` instead of `text_style(...).line_height`** → the
//!   `line-height:normal` row reads **28.800003** against Chrome's 28, and every `1.5` row still
//!   passes. This is the plausible wrong fix, and it is why the `normal` row exists — **and why that
//!   row alone carries a 0.5 tolerance.** At the 1.01 the other rows use, the first run of this
//!   proof came back GREEN and the claim written here would have been false; the tolerance was
//!   tightened until the proof actually bit, rather than the claim being kept and the proof waved
//!   through.
//! - **Drop the wrapper's `metrics`** (the t935 state) → the nested text returns to **6px** below
//!   the line top while every line-box HEIGHT above stays green. Verified. The two are separable and
//!   the gate asserts both.
//! - **Give the spacer `holds_line: true`** → `g_empty_inline_rect` fails with `#s1` at
//!   `[0 3 0x17]` against Chrome's `[0 0 0x0]`: an empty inline ALONE brings no line box into
//!   existence (CSS2 §9.4.2, t760) and would get a phantom 17px box back. Verified — and it is a
//!   DIFFERENT gate that catches it, which is the point of running the whole suite rather than the
//!   one file this tick wrote.
//!
//! ## THE RESIDUE THAT WAS PINNED AT OUR NUMBER — CLOSED AT t939, ONE TICK LATER
//!
//! ⚠⚠ t935 landed the line box's HEIGHT and left the inner text at the wrong BASELINE inside it:
//! Chrome puts the 12px text **15px** below the line box top and we put it **6px** below. Pinned at
//! our number on purpose, and changed deliberately here.
//!
//! **The cause was one branch.** `close_line` places a fragment as a real inline box *about the
//! baseline* (`above = ascent + half_leading`) only when it has metrics; without them it falls to
//! `min_h_down`, **a floor that grows the line DOWNWARD** — correct for a padding edge and a
//! `<br>`'s reporter, which hold a line open and have no baseline of their own, and wrong for a
//! text-less wrapper, whose leading must be half-led around the baseline exactly like the text it
//! stands in for. t935 gave the wrapper its `leading` and no `(ascent, descent)`, so **the entire
//! new leading landed below the baseline**: the line box was the right height and everything on it
//! sat 9px too high. t939 gives it metrics, and it takes the branch the engine already had.
//!
//! **Ranked, not stumbled on.** The t936 sweep says `reading_order` blocks **9 of the 10 cheapest
//! M1 crossings** — sites already over the shape bar and failing only jarring — and the standing
//! finding is that a reading-order symptom is a geometry error upstream. `<a><i></i><span>label</span></a>`
//! is the shape this fix moves.
//!
//! This tick contributes the element's `line_height` (a floor on the line box's total height) and
//! **not** its ascent/descent, deliberately: those are the metrics `vertical-align: middle /
//! text-top / text-bottom / sub / super` are defined against — *the parent's font, never the aligned
//! box's own* — and the fold at `layout/lib.rs:7549` filters atomics out for exactly that reason.
//! Feeding a nested span's ascent in without re-deriving that rule would trade a `dy` cascade for a
//! `vertical-align` regression, which is the trade the ratchet refuses.
//!
//! **It is worth separating because the two errors have different blast radii.** The height error
//! CASCADED — every element below the wrapper was displaced, and in the 22-case sweep that built
//! this gate the drift reached 12.8px by the eighth div. The baseline error is contained to the one
//! line: with this fix, **every div in this fixture matches Chrome on both height and y**
//! (36/36/24/36/24/28/36/36 at 0/36/72/96/132/156/184/220), and in the 22-case sweep that found the
//! defect the seven elements below the wrapper went from 12.8px adrift to exact. Asserted below at
//! our `6` so the next fix has one number to move and cannot land silently.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
html,body{margin:0;padding:0;font:16px/1.5 sans-serif}
.w{width:400px}
</style></head><body>
<div class="w" id="d1"><span style="font-size:24px">outer24</span></div>
<div class="w" id="d2"><span style="font-size:24px"><span style="font-size:12px" id="inner">nested</span></span></div>
<div class="w" id="d3"><span style="font-size:12px">small only</span></div>
<div class="w" id="d4"><span style="font-size:24px">big</span> and 16px</div>
<div class="w" id="d5">plain 16</div>
<div class="w" id="d6" style="line-height:normal"><span style="font-size:24px"><span style="font-size:12px">nested</span></span></div>
<div class="w" id="d7"><span style="font-size:24px;line-height:1.5"><span style="font-size:12px">nested</span></span></div>
<div class="w" id="d8"><span style="font-size:24px"><span style="font-size:12px">n</span>x</span></div>
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

#[test]
fn g_inline_box_leading() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://ibl.test/", &fonts, 1200.0);

    let check = |sel: &str, want: f32, why: &str| {
        let got = rect_of(&page, sel).height;
        assert!(
            (got - want).abs() < 1.01,
            "G_INLINE_BOX_LEADING: `{sel}` expected line box = {want} (MEASURED in headless Chrome \
             on THIS fixture), got {got}.\n  {why}"
        );
    };

    // ── THE CONTROLS. Four of the five were already exact, and they are what turn this from "our
    //    line boxes are short" into a statement about ONE structural path.
    check(
        "#d1",
        36.0,
        "a 24px span WITH its own text has always raised the line — it contributes through its own \
         word fragments. If this row ever fails, the diagnosis in this file is wrong",
    );
    check(
        "#d3",
        24.0,
        "a 12px span cannot LOWER the line below the div's own 24px strut — the fold is a max, and a \
         fix that replaced rather than maxed would break this row",
    );
    check(
        "#d4",
        36.0,
        "a big span beside plain text: both contribute, the larger wins",
    );
    check("#d5", 24.0, "no inline element at all — the bare strut");
    check(
        "#d8",
        36.0,
        "THE DISCRIMINATOR: the same nesting as #d2, plus ONE character of the outer span's own \
         text. This was already 36 before the fix, which is what proves the defect was structural — \
         a box that produced no fragment was never in the fold — and not a metrics, font or rounding \
         error, all of which would have moved the four CONTROLs above as well",
    );

    // ── THE BUG: a wrapper with no text of its own was invisible to the line box.
    check(
        "#d2",
        36.0,
        "`<span 24px><span 12px>nested</span></span>` — CSS 2.1 §10.8 is unconditional, so the outer \
         span contributes its 36px leading even though every character belongs to its child. It read \
         24, the div's own strut, because the outer span emitted no InlineItem at all",
    );
    // ⚠ TIGHT TOLERANCE ON PURPOSE — see the note on this row in the header. The wrong-field fix
    //   produces 28.8 here, which the 1.01 tolerance every other row uses would wave through.
    {
        let got = rect_of(&page, "#d6").height;
        assert!(
            (got - 28.0).abs() < 0.5,
            "G_INLINE_BOX_LEADING: `#d6` expected line box = 28 (MEASURED in headless Chrome on \
             THIS fixture), got {got}. `line-height:normal` resolves through the FACE \
             (round(ascent+descent+lineGap) = 28 for 24px Liberation Sans), not through arithmetic. \
             A fix reading `ComputedStyle::line_height` gets the raw 1.2x fallback of 28.8 — which \
             is the bug the first draft of this fix had, and the reason this row's tolerance is \
             0.5 rather than the 1.01 the others use: at 1.01 the wrong value passes."
        );
    }
    let _unused_d6 = check(
        "#d5",
        24.0,
        "re-asserted here beside the tightened #d6 row above",
    );
    check(
        "#d7",
        36.0,
        "an EXPLICIT `line-height:1.5` on the text-less wrapper is honoured for the same reason",
    );

    // ── THE RESIDUE, pinned at OUR number. Chrome puts this at y=457.3; we put it at 448, because
    //    the new leading all lands below the baseline instead of being half-led around it.
    let inner = rect_of(&page, "#inner");
    let d2 = rect_of(&page, "#d2");
    let offset = inner.y - d2.y;
    assert!(
        (offset - 15.0).abs() < 1.01,
        "G_INLINE_BOX_LEADING: the nested 12px text must sit 15px below its line box top — CHROME'S \
         NUMBER, closed at t939. It was 6 at t935: the wrapper carried its `leading` but no metrics, \
         so `close_line` routed it to `min_h_down`, a floor that grows the line DOWNWARD, and the \
         whole of the new leading landed BELOW the baseline. Giving the wrapper its own \
         (ascent, descent) puts it through the branch that places an inline box ABOUT the baseline \
         (`above = ascent + half_leading`), which is the model the engine already implemented for \
         text. NOTE the old text of this assertion, kept because it was wrong in an instructive way: This tick contributes the wrapper's \
         line-height (a floor on the line box) and NOT its ascent/descent, because those are the \
         metrics `vertical-align: middle/text-top/text-bottom/sub/super` are defined against — the \
         PARENT's font, never the aligned box's own — and feeding a nested span's ascent in without \
         re-deriving that rule trades a dy cascade for a vertical-align regression. The height error \
         CASCADED (12.8px by the eighth div in the sweep that found this); this one is contained to \
         the single line, and every element BELOW is now Chrome-exact. A future fix must come and \
         change this line deliberately"
    );
}
