//! # G_PERCENTAGE_GAP — `column-gap: 10%` had nowhere to be stored, so it became zero
//!
//! Not a missing arm: a **field that could not represent the value**. `ComputedStyle.row_gap` and
//! `.column_gap` were bare `f32` px, and every producer funnelled into them accordingly —
//! `parse_length_px` in the minimal cascade, and in `stylo_map` a `lp_to_dim` whose result was
//! immediately narrowed by `Dim::Px(p) => p, _ => 0.0`. So a percentage **arrived from Stylo intact
//! and was thrown away one line later**, which is the same "arrived and dropped" shape t981 found in
//! the `place-*` shorthands, one type down.
//!
//! The fix is the widening: `row_gap`/`column_gap` become `Dim`, and taffy's `gap` is a
//! `LengthPercentage` too, so the percentage crosses intact and is resolved by the participant that
//! knows the basis.
//!
//! ## Which basis, measured rather than assumed
//!
//! A gap percentage resolves against the **container's content box on that axis**. Three rows pin it:
//!
//! ```text
//!                                                        Chrome     before      after
//!   column-gap:10%, 300px grid                    2nd x     90         60         90    (gap 30)
//!   column-gap:10%, 300px grid + padding:0 50px   2nd x    130        110        130    (gap 20)
//!   row-gap:10%, grid with height:200px           2nd y     60         40         60    (gap 20)
//!   row-gap:10%, AUTO-height grid                 2nd y     48         40         48    (gap  8)
//!   gap:10% 20% shorthand (row THEN column)       3rd y     48         40         48
//!   column-gap:10% on a FLEX container            2nd x     90         60         90
//!  ── CONTROLS ──
//!   column-gap:30px                               2nd x     90         90     unchanged
//!   no gap declared                               2nd x     60         60     unchanged
//! ```
//!
//! The padded row is the one that separates *content box* from *border box*: 10% of 300 is 30 and
//! 10% of the 200px content box is 20, and only one of those is Chrome's.
//!
//! ⚠ **The auto-height rows are the interesting ones and they were NOT predicted.** A `row-gap`
//! percentage on a container whose block size is indefinite has a circular basis — the height
//! depends on the gap which depends on the height. Chrome resolves it against the height computed
//! **with the percentage treated as zero**: rows 40+40 = 80, gap = 8, second row at 48. Taffy does
//! the same thing, so these rows came out exact with no extra work; they are asserted because that
//! agreement is a fact about taffy that could change under it, not a consequence of this fix.
//!
//! ## The CSSOM half, which a geometry-only gate would have missed
//!
//! `getComputedStyle(el).columnGap` on `column-gap: 10%` returns **`"10%"`** in Chrome — the
//! percentage, not a used pixel length. Both readers of these fields were formatting `{}px` around
//! a float, so widening the type without touching them would have printed `10px` for a 10% gap: a
//! *plausible* wrong answer of the right type, which is the class this project has caught before.
//! Both now route through `dim_css`, the shared serialiser every other `Dim`-valued property uses.
//!
//! ⚠ **STILL OPEN, and named rather than built:** an UNDECLARED gap reads back as `0px` where Chrome
//! says `normal`. That is a pre-existing CSSOM divergence with nothing to do with percentages —
//! `Dim` has no `normal` and the initial value is stored as `Px(0.0)` — and it is not worth
//! inventing a variant for inside a tick about percentages. The `normal` gap *behaves* as zero in
//! both engines, so only the serialisation differs.
//!
//! ## How this goes RED
//!
//! - **Narrow `gap_dim` back to `Dim::Px(p) => p, _ => Dim::Px(0.0)`** in `stylo_map` → every
//!   percentage row reads its un-gapped position; both controls hold.
//! - **Map the gap with `length(..)` on a resolved px instead of `lp(..)`** → same rows fail, and
//!   this is the version that looks like it works because the px control still passes.
//! - **Serialise with `format!("{}px", ..)` instead of `dim_css`** → the geometry rows ALL still
//!   pass and only `cssom:10%` fails, reading `10px`. That is the row a geometry-only gate cannot
//!   have.
//! ⚠ **A fourth recipe — swapping the two halves of the `gap` shorthand — CANNOT fire here, and
//! that is recorded rather than dropped.** This gate loads a page, so it exercises the **Stylo**
//! cascade, and Stylo expands the shorthand into its two longhands before we ever see it: the
//! minimal cascade's `"gap"` arm is unreachable from any `Page::load` test. Tried, measured green,
//! and moved to where it can fail —
//! `gap_carries_percentages_and_the_shorthand_sets_row_first` in `manuk-css`, where swapping the
//! halves reads `Percent(20)` against `Percent(10)`. That test also pins the axis order, which
//! matters because `gap: <row> <column>` puts the BLOCK axis first — the opposite of the
//! `margin`-style analogy most people reach for.
//!
//! This is the same trap as t976 and as the non-firing recipe in `G_GRID_CONTAINER_HEIGHT`: **a RED
//! proof aimed at the wrong cascade proves nothing, and looks like a pass.**

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
html,body{margin:0;padding:0}
body{font:16px/1 monospace}
*{box-sizing:border-box}
.g{display:grid;background:#eee;margin:0 0 6px 0}
.f{display:flex;background:#fed;margin:0 0 6px 0}
.it{width:60px;height:40px}
</style></head><body>
<div class="g" id="c1" style="width:300px;grid-template-columns:60px 60px;column-gap:10%"><div class="it"></div><div class="it" id="a1"></div></div>
<div class="g" id="c2" style="width:300px;height:200px;grid-template-columns:60px;grid-template-rows:40px 40px;row-gap:10%"><div class="it"></div><div class="it" id="a2"></div></div>
<div class="g" id="c3" style="width:300px;grid-template-columns:60px;grid-template-rows:40px 40px;row-gap:10%"><div class="it"></div><div class="it" id="a3"></div></div>
<div class="g" id="c4" style="width:300px;grid-template-columns:60px 60px;gap:10% 20%"><div class="it"></div><div class="it"></div><div class="it" id="a4"></div></div>
<div class="f" id="c5" style="width:300px;column-gap:10%"><div class="it"></div><div class="it" id="a5"></div></div>
<div class="g" id="c6" style="width:300px;padding:0 50px;grid-template-columns:60px 60px;column-gap:10%"><div class="it"></div><div class="it" id="a6"></div></div>
<div class="g" id="c7" style="width:300px;grid-template-columns:60px 60px;column-gap:30px"><div class="it"></div><div class="it" id="a7"></div></div>
<div class="g" id="c8" style="width:300px;grid-template-columns:60px 60px"><div class="it"></div><div class="it" id="a8"></div></div>
<pre id="out"></pre>
<script>
  var R = [], s = getComputedStyle(document.getElementById('c1'));
  R.push('cssom:' + s.columnGap);
  R.push('gpv:' + (s.getPropertyValue('column-gap') === s.columnGap));
  R.push('px:' + getComputedStyle(document.getElementById('c7')).columnGap);
  document.getElementById('out').textContent = R.join(' ');
</script>
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
fn g_percentage_gap() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://pg.test/", &fonts, 1200.0);
    let off = |sel: &str, w: &str| {
        let (a, c) = (rect_of(&page, sel), rect_of(&page, w));
        (a.x - c.x, a.y - c.y)
    };
    let near = |got: (f32, f32), want: (f32, f32)| {
        (got.0 - want.0).abs() < 1.1 && (got.1 - want.1).abs() < 1.1
    };

    // ── DEFECT — each row is one thing the basis could have been, and only one of them is Chrome's.
    for (sel, w, want, why) in [
        (
            "#a1",
            "#c1",
            (90.0, 0.0),
            "`column-gap:10%` of a 300px grid is 30px, so the second 60px column starts at 90. x=60 \
             is a gap of ZERO — the percentage had nowhere to be stored and became `0.0`",
        ),
        (
            "#a6",
            "#c6",
            (130.0, 0.0),
            "the same 10% on a grid with `padding: 0 50px` is 20px, not 30 — the basis is the \
             CONTENT box (200px), and this row is the only one that separates the two",
        ),
        (
            "#a2",
            "#c2",
            (0.0, 60.0),
            "`row-gap:10%` on a `height:200px` grid is 20px — the BLOCK axis resolves against the \
             block size, not against the width",
        ),
        (
            "#a3",
            "#c3",
            (0.0, 48.0),
            "`row-gap:10%` on an AUTO-height grid: the basis is circular, and Chrome resolves it \
             against the height computed with the percentage treated as ZERO — 40+40=80, gap 8",
        ),
        (
            "#a4",
            "#c4",
            (0.0, 48.0),
            "`gap: 10% 20%` sets ROW first then COLUMN, so the third item's row offset uses the 10% \
             — swapping the halves gives 16 here",
        ),
        (
            "#a5",
            "#c5",
            (90.0, 0.0),
            "the same percentage on a FLEX container, which shares the mapping",
        ),
    ] {
        assert!(
            near(off(sel, w), want),
            "G_PERCENTAGE_GAP: {sel} must sit at {want:?} — {why}; got {:?}. A gap percentage \
             resolves against the container's CONTENT BOX on that axis, which is a basis only \
             layout knows: it has to survive the cascade as a percentage.",
            off(sel, w)
        );
    }

    // ── CONTROLS — a px gap and no gap at all. A widening that resolved percentages correctly but
    //    dropped plain lengths on the way through would pass every row above and fail these.
    assert!(
        near(off("#a7", "#c7"), (90.0, 0.0)),
        "G_PERCENTAGE_GAP: a plain `column-gap:30px` still puts the second column at 90, not {:?}.",
        off("#a7", "#c7")
    );
    assert!(
        near(off("#a8", "#c8"), (60.0, 0.0)),
        "G_PERCENTAGE_GAP: with NO gap declared the columns are flush at 60, not {:?} — the initial \
         value must stay zero through the type change.",
        off("#a8", "#c8")
    );

    // ── THE CSSOM HALF. A geometry-only gate cannot see this: both readers formatted `{}px` around
    //    a float, so a widening that fixed layout and left them alone would report `10px` for a 10%
    //    gap — a plausible wrong answer of the right type.
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("PERCENTAGE GAP CSSOM: {got}");
    for (claim, why) in [
        (
            "cssom:10%",
            "`getComputedStyle(el).columnGap` on `column-gap:10%` returns the PERCENTAGE in Chrome, \
             not a used pixel length. `10px` is the reading that means the serialiser was left \
             behind by the type change",
        ),
        (
            "gpv:true",
            "`getPropertyValue('column-gap')` and `.columnGap` are two doors onto one value",
        ),
        (
            "px:30px",
            "a real length still serialises as `30px` — the control for the serialiser",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_PERCENTAGE_GAP: expected `{claim}` in the CSSOM readout — {why}. Got: {got}"
        );
    }
}
