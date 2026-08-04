//! # G_BFC_SPECIFIED_WIDTH_FLOAT_BAND — a fixed-width BFC box sits BESIDE a float when it fits
//!
//! CSS 2.1 §9.5: *"the border box of … an element in the normal flow that establishes a new block
//! formatting context must not overlap the margin box of any floats in the same block formatting
//! context"*, with the *"if necessary, implementations should clear"* clause for the case where it
//! cannot fit. `bfc_float_band` built the **`width:auto`** half at t859 and wrote its own follow-up:
//!
//! > *"A SPECIFIED width is deliberately NOT handled here … Chrome shifts such a box beside the
//! > float only while it still fits (`width:300px` shifts to 100, `width:301px` stays at 0) … Today
//! > we never shift, which is Chrome-exact for the does-not-fit half and wrong for the fits half …
//! > Measured, named, and left as its own tick rather than guessed at."*
//!
//! This is that tick. Every number below is CAPTURED from
//! `google-chrome-stable --headless=new --hide-scrollbars --window-size=1200,800 --dump-dom`,
//! a 100px `float:left` in a 400px `flow-root` container:
//!
//! ```text
//!                                          Chrome    before
//!   width:300px  (fits the 300px band)      x=100     x=0      <- the boundary, INCLUSIVE
//!   width:301px  (one px too wide)          x=0       x=0      correct already
//!   width:200px                             x=100     x=0
//!   width:400px  (as wide as the container) x=0       x=0      correct already
//!   width:200px  margin-left:20px           x=100     x=20     the margin is ABSORBED
//!   width:200px  margin-left:150px          x=150     x=150    already clears — untouched
//!   width:50%                               x=100     x=0      the % resolves against `cw`
//!   float:right instead, width:200px        x=0       x=0      a right float moves no left edge
//!   both sides, width:200px                 x=100     x=0
//!   float 10px tall, box 60px tall          x=100     x=0      the band is read at the TOP
//!   overflow:hidden / display:flex          x=100     x=0
//!   box-sizing:border-box with padding      x=100     x=0
//! ```
//!
//! **Eight of the fourteen were wrong, and the six that were right are half the deliverable** — the
//! `301px` and `400px` rows are the *"if necessary, clear"* half, and a fix that shifted every box
//! unconditionally would satisfy the other eight and break these two. Both directions are asserted.
//!
//! ⚠⚠⚠ **`cw` IS RETURNED UNNARROWED, AND THAT IS THE WHOLE DIFFERENCE FROM THE `auto` ARM.** An
//! auto box takes the band as its containing block because the band is what sizes it. A specified
//! box keeps its own width, so narrowing `cw` would silently re-resolve every percentage inside it
//! against the band — which is precisely the objection the old comment raised against building this,
//! and it is answered by *not doing it* rather than by declining the shift. `width:50%` proves it
//! from the outside: Chrome resolves 50% against the 400px container, gets 200, and still shifts the
//! result to 100.
//!
//! ⚠⚠ **THIS DEFECT WAS MIS-NAMED ONE TICK EARLIER, BY ME, AND THE CORRECTION IS THE LESSON.** t905
//! reported it as *"a BFC box fails to avoid a float that ESCAPED a previous sibling"* and filed a
//! `CONSTELLATION.tsv` row saying so. That fixture set `width:400px` on its boxes **and** wrapped the
//! float in a plain `<div>` — two variables, one reading. With `width:auto` restored, all five
//! escaped-float cases are Chrome-exact and always were; the escape had nothing to do with it.
//! Third time in two ticks that a fixture of mine produced a defect that was the fixture: **a
//! differential probe is only a control if each case varies ONE thing.**
//!
use manuk_text::FontContext;

const HTML: &str = r##"<!DOCTYPE html><html><head><meta charset="utf-8"><style>
 body{margin:0;font:16px/1.5 sans-serif}
 .w{width:400px;display:flow-root;margin-bottom:4px}
 .f{float:left;width:100px;height:80px;background:#f99}
 .b{background:#cdf;height:20px;display:flow-root}
</style></head><body>
<div class="w"><div class="f"></div><div class="b" id="s1" style="width:300px">fits exactly</div></div>
<div class="w"><div class="f"></div><div class="b" id="s2" style="width:301px">one px too wide</div></div>
<div class="w"><div class="f"></div><div class="b" id="s3" style="width:200px">fits easily</div></div>
<div class="w"><div class="f"></div><div class="b" id="s4" style="width:400px">as wide as the container</div></div>
<div class="w"><div class="f"></div><div class="b" id="s5" style="width:200px;margin-left:20px">fits, margin-left 20</div></div>
<div class="w"><div class="f"></div><div class="b" id="s6" style="width:200px;margin-left:150px">already clears</div></div>
<div class="w"><div class="f" style="float:right"></div><div class="b" id="s7" style="width:200px">RIGHT float, fits</div></div>
<div class="w"><div class="f" style="float:right"></div><div class="b" id="s8" style="width:301px">RIGHT float, too wide</div></div>
<div class="w"><div class="f"></div><div class="f" style="float:right"></div><div class="b" id="s9" style="width:200px">both sides, fits</div></div>
<div class="w"><div class="f"></div><div class="b" id="s10" style="width:50%">percentage width, fits</div></div>
<div class="w"><div class="f"></div><div class="b" id="s11" style="width:200px;overflow:hidden">overflow:hidden, fits</div></div>
<div class="w"><div class="f"></div><div class="b" id="s12" style="width:200px;display:flex">flex, fits</div></div>
<div class="w"><div class="f" style="height:10px"></div><div class="b" id="s13" style="width:200px;height:60px">short float, tall box</div></div>
<div class="w"><div class="f"></div><div class="b" id="s14" style="width:200px;box-sizing:border-box;padding:0 10px">border-box padding</div></div>

</body></html>
"##;

fn rect(page: &manuk_page::Page, sel: &str) -> (f32, f32, f32) {
    let dom = page.dom();
    let n = manuk_css::query_selector_all(dom, dom.root(), sel)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("selector {sel} matched nothing"));
    let r = page
        .root_box
        .node_rects(dom)
        .get(&n)
        .copied()
        .unwrap_or_else(|| panic!("no box for {sel}"));
    (r.x, r.width, r.height)
}

fn c(page: &manuk_page::Page, sel: &str, x: f32, w: f32, h: f32) {
    let (gx, gw, gh) = rect(page, sel);
    assert!(
        (gx - x).abs() < 1.01 && (gw - w).abs() < 1.01 && (gh - h).abs() < 1.01,
        "G_BFC_SPECIFIED_WIDTH_FLOAT_BAND: `{sel}` expected x={x} w={w} h={h} (CAPTURED from \
         `google-chrome-stable --headless=new --hide-scrollbars --window-size=1200,800`), \
         got x={gx} w={gw} h={gh}"
    );
}

#[test]
fn g_bfc_specified_width_float_band() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://band.test/", &fonts, 1200.0);
    c(&page, "#s1", 100.0, 300.0, 20.0);
    c(&page, "#s2", 0.0, 301.0, 20.0);
    c(&page, "#s3", 100.0, 200.0, 20.0);
    c(&page, "#s4", 0.0, 400.0, 20.0);
    c(&page, "#s5", 100.0, 200.0, 20.0);
    c(&page, "#s6", 150.0, 200.0, 20.0);
    c(&page, "#s7", 0.0, 200.0, 20.0);
    c(&page, "#s8", 0.0, 301.0, 20.0);
    c(&page, "#s9", 100.0, 200.0, 20.0);
    c(&page, "#s10", 100.0, 200.0, 20.0);
    c(&page, "#s11", 100.0, 200.0, 20.0);
    c(&page, "#s12", 100.0, 200.0, 20.0);
    c(&page, "#s13", 100.0, 200.0, 60.0);
    c(&page, "#s14", 100.0, 200.0, 20.0);
}
