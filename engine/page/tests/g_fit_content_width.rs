//! # G_FIT_CONTENT_WIDTH — `width: fit-content` reached the block path and was given up on inside flex and grid
//!
//! `fit-content` parses, maps out of Stylo, and lives on `ComputedStyle.width_keyword` — and the
//! block path consumes it in **six** places via `shrink_to_fit`. The taffy path had one line for it:
//!
//! ```text
//!    IntrinsicSize::FitContent => return None,      // taffy_tree.rs
//! ```
//!
//! so inside a flex or grid container the keyword was dropped and the box kept `width: auto` — which
//! for a grid item means **stretch to the track**, the exact opposite of what the declaration asks
//! for. Measured: `<div style="width:fit-content">abc</div>` in a 200px track is **29px** in Chrome
//! and was **200** here.
//!
//! This is the half-installed shape rather than the absent one: two of the three intrinsic keywords
//! (`min-content`, `max-content`) resolve to a length by asking the measure closure, and the third
//! *cannot*, so the arm that could not return a number returned none and did nothing.
//!
//! ## Why it cannot be resolved to a length there, and what it becomes instead
//!
//! ```text
//!   fit-content = min(max-content, max(min-content, stretch))
//! ```
//!
//! The `stretch` term is *the space the formatting context is about to hand this box* — not known
//! inside the style-conversion pass, and not askable without re-entering the measure it sits in. So
//! the keyword is not resolved; it is expressed as the **bounds it is defined by**, leaving
//! `size.width` at `auto` so that taffy's own offer supplies the middle term. Clamping that offer
//! between the two content bounds is `clamp(min-content, available, max-content)` — `fit-content`,
//! computed by the one participant that knows the available width.
//!
//! ## The row that a one-case fixture gets wrong
//!
//! The `min-content` term lives **inside** `fit-content`; `max-width` clamps the **result**. Taffy
//! resolves min-over-max, so a first implementation that pushed `min-content` in as a floor and the
//! author's `max-width` in as a ceiling made the floor outrank the ceiling: `width:fit-content;
//! max-width:20px` around a 29px unbreakable word read **29** against Chrome's **20**. Two rows here
//! pin the two directions of that order — `max-width` must beat the synthetic floor, and the
//! author's own `min-width` must beat everything (CSS 2.1 §10.4).
//!
//! ```text
//!                                                       Chrome     before      after
//!   fit-content, 200px track, "abc"                        29        200         29
//!   fit-content, 20px track (narrower than the word)       29         20         29
//!   fit-content, 40px track, wrappable "aa bbbbbb c"       58         40         58
//!   fit-content + max-width:20px                           20         20 †       20
//!   fit-content + min-width:120px                         120        200        120
//!   fit-content on a FLEX item                             29         29 ‡       29
//!  ── CONTROLS ──
//!   width:max-content                                     106        106     unchanged
//!   width:min-content                                      58         58     unchanged
//!   no keyword — a grid item stretches to its track       200        200     unchanged
//!   fit-content on the BLOCK path (no flex/grid)           29         29     unchanged
//! ```
//!
//! **† this row was RIGHT BEFORE THE FIX, BY ACCIDENT, AND THE FIRST VERSION OF THE FIX BROKE IT.**
//! The box stretched to its 200px track and the author's `max-width: 20px` clamped it to 20 — which
//! is the correct answer, reached by a route that has nothing to do with `fit-content`. Then the
//! first implementation pushed `min-content` in as a floor, taffy resolved min-over-max, and the row
//! that had been right for the wrong reason became **29**. A row that already passes is not a row
//! that can be left out: this one is the only thing in the gate that catches the ordering.
//!
//! **‡ the flex row was genuinely already right**, because a flex item's base size comes from its
//! content anyway. It stays because the same style conversion feeds both formatting contexts, so a
//! fix aimed at grid must be shown not to disturb flex.
//!
//! ## How this goes RED
//!
//! Each recipe below was applied, built, and read off the WHOLE fixture — not off the gate's first
//! failing assertion — so the confinement is measured rather than assumed:
//!
//! - **Delete the `fit-content` block** (leaving the arm giving up, as before) → `#a1` 200, `#a2` 20,
//!   `#a3` 40, `#a5` 200. All four controls pass, and so do `#a4` and `#a6` — which is exactly the
//!   partial agreement that let this survive.
//! - **Set `size.width = length(max_content)` instead of the bounds** → **only `#a3`** fails, at 106
//!   against 58: with `stretch` thrown away, a wrappable string stops being allowed to wrap into the
//!   space it was offered.
//! - **Do not clamp the synthetic floor by the ceiling** (`floor = min_c`, the first version of this
//!   fix) → **only `#a4`** fails, at 29 against 20.
//! - **Drop the author's `min-width` composition** → **only `#a5`** fails, at 29 against 120.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
html,body{margin:0;padding:0}
body{font:16px/1 monospace}
*{box-sizing:border-box}
.g{display:grid;background:#eee;margin:0 0 6px 0}
.f{display:flex;background:#fed;margin:0 0 6px 0}
.b{height:40px}
</style></head><body>
<div class="g" id="c1" style="width:300px;grid-template-columns:200px"><div class="b" id="a1" style="width:fit-content">abc</div></div>
<div class="g" id="c2" style="width:300px;grid-template-columns:20px"><div class="b" id="a2" style="width:fit-content">abc</div></div>
<div class="g" id="c3" style="width:300px;grid-template-columns:40px"><div class="b" id="a3" style="width:fit-content">aa bbbbbb c</div></div>
<div class="g" id="c4" style="width:300px;grid-template-columns:200px"><div class="b" id="a4" style="width:fit-content;max-width:20px">abc</div></div>
<div class="g" id="c5" style="width:300px;grid-template-columns:200px"><div class="b" id="a5" style="width:fit-content;min-width:120px">abc</div></div>
<div class="f" id="c6" style="width:300px"><div class="b" id="a6" style="width:fit-content">abc</div><div style="flex:1">y</div></div>
<div class="g" id="c7" style="width:300px;grid-template-columns:200px"><div class="b" id="a7" style="width:max-content">aa bbbbbb c</div></div>
<div class="g" id="c8" style="width:300px;grid-template-columns:200px"><div class="b" id="a8" style="width:min-content">aa bbbbbb c</div></div>
<div class="g" id="c9" style="width:300px;grid-template-columns:200px"><div class="b" id="a9">abc</div></div>
<div id="c10" style="width:300px"><div class="b" id="a10" style="width:fit-content">abc</div></div>
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
fn g_fit_content_width() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://fc.test/", &fonts, 1200.0);
    let w = |sel: &str| rect_of(&page, sel).width;
    let near = |got: f32, want: f32| (got - want).abs() < 1.6;

    // ── DEFECT — `fit-content` inside a grid or flex container. Each row is one term of the
    //    definition doing its job.
    for (sel, want, why) in [
        (
            "#a1",
            29.0,
            "a 200px track around a 29px word — the MAX-CONTENT term wins and the box must NOT \
             stretch to its track. Reading 200 is the keyword being dropped entirely",
        ),
        (
            "#a3",
            58.0,
            "a 40px track around wrappable `aa bbbbbb c` — the STRETCH term is what lets it wrap, \
             and the MIN-CONTENT floor (the longest word) is what stops it at 58 rather than 40",
        ),
        (
            "#a4",
            20.0,
            "`max-width:20px` clamps the RESULT, and must beat the synthetic min-content floor of \
             29 — the floor lives INSIDE fit-content, not outside it",
        ),
        (
            "#a5",
            120.0,
            "`min-width:120px` is the author's own bound and wins over everything (CSS 2.1 §10.4)",
        ),
        (
            "#a6",
            29.0,
            "the same keyword on a FLEX item, which routes through the same style conversion",
        ),
    ] {
        assert!(
            near(w(sel), want),
            "G_FIT_CONTENT_WIDTH: {sel} must be {want}px wide — {why}; got {}. `fit-content` is \
             `min(max-content, max(min-content, stretch))`, and inside flex/grid it is expressed as \
             the BOUNDS it is defined by so that taffy's own offer supplies the stretch term.",
            w(sel)
        );
    }

    // ── The OVERFLOW direction: fit-content is floored at min-content, so in a track narrower than
    //    an unbreakable word the box overflows its track rather than clamping to it.
    assert!(
        near(w("#a2"), 29.0),
        "G_FIT_CONTENT_WIDTH: a 20px track around a 29px unbreakable word is 29 — fit-content \
         OVERFLOWS its track rather than clamping to it; got {}. 20 is the track, which is what a \
         dropped keyword reads.",
        w("#a2")
    );

    // ── CONTROL B — the two intrinsic keywords that always worked, on the same fixture and the same
    //    string, so a change to the shared measure probe cannot move them silently.
    assert!(
        near(w("#a7"), 106.0) && near(w("#a8"), 58.0),
        "G_FIT_CONTENT_WIDTH: `max-content` is 106 and `min-content` is 58 on `aa bbbbbb c`; got {} \
         and {}. These two resolve to a LENGTH from the same probe fit-content borrows.",
        w("#a7"),
        w("#a8")
    );

    // ── CONTROL C — no keyword at all: a grid item stretches to its track. This is the row that
    //    fails if the new bounds were applied to boxes that never asked for them.
    assert!(
        near(w("#a9"), 200.0),
        "G_FIT_CONTENT_WIDTH: an undeclared grid item still stretches to its 200px track, not {} — \
         the new bounds must touch ONLY boxes whose `width` keyword is `fit-content`.",
        w("#a9")
    );

    // ── CONTROL D — the BLOCK path, which handled this keyword correctly the whole time through
    //    `shrink_to_fit`. It is the reason the defect read as "implemented" from the outside: the
    //    same declaration on the same content is right or wrong depending on whether an ancestor
    //    happens to be `display:flex`.
    assert!(
        near(w("#a10"), 29.0),
        "G_FIT_CONTENT_WIDTH: `fit-content` outside any flex/grid is 29 and always was, not {}.",
        w("#a10")
    );
}
