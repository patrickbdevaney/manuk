//! **G_A_VERTICAL_RUN_ADVANCES_DOWN — a `writing-mode: vertical-*` box reported its scrollable
//! overflow on the WRONG AXIS, because a rotated text run's fields live in a MIXED coordinate
//! frame and the extent walk read them as if they were all physical.**
//!
//! ⭐⭐⭐ **`f.x` is ABSOLUTE, `f.baseline` and `f.line_top` are LINE-LOCAL.** Printing the two runs
//! of an identical `width=231.19` — one horizontal in a box at `y=400`, one `vertical-lr` in a box
//! at `y=200` — is the whole derivation:
//!
//! ```text
//!                        rect.y      x   baseline   line_top
//!   sideways=false        400        0     415        400      ← both carry the box's y
//!   sideways=true         200        0      15          0      ← neither does
//! ```
//!
//! The rotated run's block-axis coordinates were never translated into the box's frame while its
//! `x` was — `translate()` shifts `f.x` by `dx` and `f.baseline` by `dy` for every run alike, so
//! the slots are physical but the *values* a sideways run puts in them are not. For `sideways` the
//! three fields therefore mean: `x` = absolute physical **x**; `baseline` = an offset **along x**
//! *within* the line (the ideographic centering, ~15 for a 20px line), **not a y**; `width` = the
//! advance, which runs **down y**.
//!
//! Chrome-measured (`google-chrome --headless --dump-dom`), 100×100 `overflow:scroll` boxes,
//! `font: 16px/20px monospace`, as `scrollWidth`/`scrollHeight`:
//!
//! ```text
//!                                            Chrome    before    after
//!   v3  vertical-lr   24 chars of text       100/231   231/100   100/231
//!   v1  vertical-lr   "AB"                   100/100   100/100   100/100  ✓
//!   v2  vertical-rl   "AB"                   100/100   100/100   100/100  ✓
//!   h1  horizontal-tb "AB"          CONTROL  100/100   100/100   100/100  ✓
//!   h2  horizontal-tb 24 chars       MIRROR  231/100   231/100   231/100  ✓
//! ```
//!
//! ⭐⭐ **`v1` and `v2` are the degenerate rows and they are why this survived: a box whose overflow
//! is square cannot see a TRANSPOSITION.** Both read `100/100` before and after. Only `v3`, whose
//! content overflows on exactly one axis, discriminates — the same shape as the `width:0` fixture,
//! the symmetric `scale()` and the zero border before it. `h2` is the mirror: it must NOT move, and
//! it is what proves the fix is scoped to `sideways` rather than swapping the axes for everybody.
//!
//! ⚠⚠⚠ **THE DESCENDANT WALK MUST NOT SEE A SIDEWAYS RUN AT ALL — that half was MEASURED AND
//! REFUSED.** Applying the same transposition inside the recursive `walk` (where the run belongs to
//! a *descendant* box rather than the scroll container itself) cost `css/css-flexbox` **−22**, all
//! of it in `negative-overflow-002.html` and `negative-overflow-004-no-padding.html`; dropping just
//! the block-axis half still cost **−8**. Those two files compute every expectation from a `bias`
//! formula over writing-mode × direction × flex-direction × wrap, so their `130`/`370` is **pure box
//! geometry** — 3×110 items + 2×10 gap + 2×10 padding — and each item holds one 8px digit that must
//! contribute nothing. A descendant's run is already covered by the `non_empty` arm's
//! `b.rect.y + b.rect.height - oy`; re-measuring it under the mixed frame can only overstate. With
//! the walk left alone the same suite is **net zero** (1 fixed / 1 new, and both are one stray
//! `@import` WARN line the failure grep catches, not a subtest).
//!
//! ⚠ `css/css-writing-modes` is **completely unmoved** — 241 failing before and after, with a
//! byte-identical name list. The area named for the mechanism cannot see it; the only WPT area that
//! can is the one this fix must be careful not to break.
//!
//! Mutations that must turn this red:
//!   1. drop the `sideways` branch entirely            → v3 reads 231/100
//!   2. `f.line_top + f.baseline` for the block extent → v3 reads 100/100 (understates)
//!   3. `f.x + f.width` for the inline extent          → v3 reads 231/231
//!   4. take the branch for every run (`true`)         → h2 reads 100/231

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><meta charset=utf-8><style>body{margin:0}
.c{width:100px;height:100px;overflow:scroll;scrollbar-width:none;font:16px/20px monospace}
</style></head><body>
<div class="c" id="v1" style="writing-mode:vertical-lr">AB</div>
<div class="c" id="v2" style="writing-mode:vertical-rl">AB</div>
<div class="c" id="v3" style="writing-mode:vertical-lr">AAAAAAAAAAAAAAAAAAAAAAAA</div>
<div class="c" id="h1">AB</div>
<div class="c" id="h2">AAAAAAAAAAAAAAAAAAAAAAAA</div>
<div id="out">-</div>
<script>
function s(k){var e=document.getElementById(k);return k+'='+e.scrollWidth+'/'+e.scrollHeight;}
document.getElementById('out').textContent=['v1','v2','v3','h1','h2'].map(s).join(' ');
</script></body></html>"##;

#[test]
fn a_vertical_run_advances_down() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://verticalrun.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("VERTICAL RUN: {got}");

    // VACUITY. The horizontal MIRROR must already overflow on its own axis, or the vertical rows
    // are measuring whether text contributes to scrollable overflow at all rather than WHICH AXIS.
    assert!(
        got.contains("h2=231/100"),
        "VACUOUS: the horizontal 24-char row does not overflow to Chrome's 231 wide, so the \
         vertical rows below are not measuring the axis - got {got:?}"
    );

    // Chrome headless, all five rows.
    let want = "v1=100/100 v2=100/100 v3=100/231 h1=100/100 h2=231/100";
    assert_eq!(
        got, want,
        "\n  a rotated run's advance belongs on the BLOCK axis\n  want: {want}\n  got:  {got}"
    );
}
