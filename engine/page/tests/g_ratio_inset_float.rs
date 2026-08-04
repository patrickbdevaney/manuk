//! # G_RATIO_INSET_FLOAT — 53 Chrome-captured claims under the burndown's top two causes
//!
//! ⚠⚠⚠ **THIS GATE EXISTS BECAUSE THREE HYPOTHESES DIED AND NOTHING GUARDED WHAT SURVIVED (t905).**
//! The t904 sweep's mechanism oracle ranks corpus-wide causes by DISTINCT SITES, and its top two are
//! `missing box: <div>` (36 sites) and `geometry/mis-sized: height ~256px` (29 sites, median 364px).
//! Both have an obvious suspect, and neither suspect is guilty:
//!
//! * **The 32px width band** (23 sites, 1154 hits, and its median is *exactly* 32 where every other
//!   band's median sits well above its label) looks like a dropped 16px-per-side horizontal inset.
//!   **Thirty claims say otherwise** — `padding`, `padding-inline`, `padding-inline-start/end`,
//!   `margin`, `margin-inline`, `border-box` with padding, left/right borders, flex and grid
//!   containers, `rem`, `em`, percentage padding, RTL, and the physical longhands are all
//!   Chrome-exact, parent box and child box alike.
//! * **The zero-height box** — `[2400 194 1200x360]` against ours `[2400 203 1200x0]` on ebay,
//!   `[389 5445 739x456]` against `[389 5384 739x0]` on ikea, x and width EXACT and the height gone —
//!   looks like unimplemented `aspect-ratio` or the `padding-top:56.25%` ratio hack. **Twelve claims
//!   say otherwise**: `aspect-ratio` is correct as a bare ratio, with a positioned child, under
//!   flex/grid/`min-height`/`max-height`, and so is the percentage-padding hack, and so is float
//!   containment by `overflow:hidden` and `display:flow-root`.
//!
//! **None of this was gated, and `aspect-ratio` was not even in `CONSTELLATION.tsv`** — neither
//! claimed nor denied, which is the shape STATUS.md's platform map has already been caught in four
//! times: *an absent measurement is not a negative measurement.* So the surviving behaviour is
//! banked here rather than left for the next tick that guesses the same suspect.
//!
//! ⚠⚠⚠ **TWO OF THE THREE "DEFECTS" THIS PROBE FOUND WERE THE PROBE.** Both are recorded because the
//! project's Lesson 4 — *every number has a harness, and the harness is part of the number* — fired
//! twice inside one tick:
//!
//! 1. `padding-top:56.25%` read **667 in Chrome against our 675**, a clean 15px. My Chrome
//!    invocation omitted **`--hide-scrollbars`**, which the fidelity harness always passes; the
//!    percentage resolves against the containing block, and Chrome's was 1185 rather than 1200. With
//!    the flag Chrome answers 675, to the pixel.
//! 2. A `display:flow-root` box appeared to sit 120px left of Chrome's. My fixture put every case in
//!    a plain `<div>`, so each row's float **escaped into the next row** and the x values were
//!    cumulative. Isolating each case in its own BFC removed it — t780-783's *"the probe's own
//!    sentinel widened its subject"*, one fixture later.
//!
//! ⚠⚠⚠ **AND THE THIRD WAS THE PROBE TOO — CORRECTED AT t906, ONE TICK LATER.** This header
//! originally reported *"a BFC box fails to avoid a float that ESCAPED a previous sibling"* and left
//! `#a9` unasserted on that basis. The fixture set `width:400px` on its boxes **and** wrapped the
//! float in a plain `<div>` — two variables, one reading. With `width:auto` restored, all five
//! escaped-float cases are Chrome-exact and always were. The real defect was the one
//! `bfc_float_band` had already named and declined to build: **a SPECIFIED width never shifted
//! beside a float, even when it fits.** Fixed at t906 and gated by
//! `g_bfc_specified_width_float_band`; `#a9` is asserted below because it now passes.
//!
//! Three defects, three fixtures, and **all three were the fixture** until one of them was isolated
//! properly. The rule earned twice over: *a differential probe is only a control if each case varies
//! ONE thing.*
//!
//! ⚠ **`display:table` remains open and unasserted**: Chrome applies its `height` as a MINIMUM, so a
//! 16px/1.5 line in a `height:20px` table is 24 there and 20 here.
//!
use manuk_text::FontContext;

const HTML: &str = r##"<!DOCTYPE html><html><head><meta charset="utf-8"><style>

  * { box-sizing: content-box; }
  body { margin: 0; font: 16px/1.5 sans-serif; }
  .frame { width: 400px; background: #eee; margin-bottom: 4px; }
  .kid { background: #cdf; height: 20px; }
  #p1  { padding: 0 16px; }
  #p2  { padding-inline: 16px; }
  #p3  { padding-inline-start: 16px; padding-inline-end: 16px; }
  #k4  { margin: 0 16px; }
  #k5  { margin-inline: 16px; }
  #p6  { width: 400px; box-sizing: border-box; padding: 0 16px; }
  #p7  { border-left: 16px solid #999; border-right: 16px solid #999; }
  #p8  { display: flex; padding: 0 16px; }
  #k8  { flex: 1; }
  #p9  { padding: 0 1rem; }
  #p10 { padding: 0 4%; }
  #p11 { display: flex; }
  #k11 { width: 100%; margin: 0 16px; }
  #p12 { padding: 0 16px; direction: rtl; }
  #p13 { display: grid; padding: 0 16px; }
  #p14 { padding: 0 1em; }
  #p15 { padding-left: 16px; padding-right: 16px; }

.frame2{width:400px;background:#eee;margin-bottom:6px}

  
  
  .fill { position:absolute; inset:0; background:#cdf; }
  #a1 { aspect-ratio: 16 / 9; }
  #a2 { aspect-ratio: 16 / 9; position:relative; }
  #a3 { padding-top: 56.25%; height:0; position:relative; }
  #a4 { aspect-ratio: 2; }
  #a5 { aspect-ratio: 16/9; display:flex; }
  #a6 { aspect-ratio: 16/9; height:auto; }
  #a7 { overflow:hidden; }
  #a7 > .f { float:left; width:50px; height:40px; background:#cdf; }
  #a8 { }
  #a8 > .f { float:left; width:50px; height:40px; background:#cdf; }
  #a9 { display:flow-root; }
  #a9 > .f { float:left; width:50px; height:40px; background:#cdf; }
  #a10 { aspect-ratio: 16/9; min-height: 0; }
  #a11 { aspect-ratio: 16/9; max-height: 500px; }
  #a12 { display:grid; aspect-ratio: 16/9; }


 
 .f{float:left;width:60px;height:80px;background:#f99}
 .b{background:#cdf;height:20px}
 .w{width:400px;display:flow-root}

</style></head><body>

<div class="frame" id="p1"><div class="kid" id="k1">a</div></div>
<div class="frame" id="p2"><div class="kid" id="k2">a</div></div>
<div class="frame" id="p3"><div class="kid" id="k3">a</div></div>
<div class="frame" id="p4"><div class="kid" id="k4">a</div></div>
<div class="frame" id="p5"><div class="kid" id="k5">a</div></div>
<div class="frame" id="p6"><div class="kid" id="k6">a</div></div>
<div class="frame" id="p7"><div class="kid" id="k7">a</div></div>
<div class="frame" id="p8"><div class="kid" id="k8">a</div></div>
<div class="frame" id="p9"><div class="kid" id="k9">a</div></div>
<div class="frame" id="p10"><div class="kid" id="k10">a</div></div>
<div class="frame" id="p11"><div class="kid" id="k11">a</div></div>
<div class="frame" id="p12"><div class="kid" id="k12">a</div></div>
<div class="frame" id="p13"><div class="kid" id="k13">a</div></div>
<div class="frame" id="p14"><div class="kid" id="k14">a</div></div>
<div class="frame" id="p15"><div class="kid" id="k15">a</div></div>

<div class="frame2" id="a1"></div>
<div class="frame2" id="a2"><div class="fill"></div></div>
<div class="frame2" id="a3"><div class="fill"></div></div>
<div class="frame2" id="a4"></div>
<div class="frame2" id="a5"></div>
<div class="frame2" id="a6"></div>
<div class="frame2" id="a7"><div class="f"></div></div>
<div class="frame2" id="a8"><div class="f"></div></div>
<div class="frame2" id="a9"><div class="f"></div></div>
<div class="frame2" id="a10"></div>
<div class="frame2" id="a11"></div>
<div class="frame2" id="a12"></div>

<div class="w"><div class="f"></div><div class="b" id="b1">plain block overlaps the float</div></div>
<div class="w"><div class="f"></div><div class="b" id="b2" style="display:flow-root">flow-root</div></div>
<div class="w"><div class="f"></div><div class="b" id="b3" style="overflow:hidden">overflow hidden</div></div>
<div class="w"><div class="f"></div><div class="b" id="b4" style="overflow:auto">overflow auto</div></div>
<div class="w"><div class="f"></div><div class="b" id="b5" style="display:flex">flex</div></div>
<div class="w"><div class="f"></div><div class="b" id="b6" style="display:grid">grid</div></div>
<div class="w"><div class="f"></div><div class="b" id="b7" style="display:table">table</div></div>
<div class="w"><div class="f"></div><div class="b" id="b8" style="display:inline-block">inline-block</div></div>
<div class="w"><div class="f"></div><div class="b" id="b9" style="clear:left">clear left</div></div>
<div class="w"><div class="f" style="float:right"></div><div class="b" id="b10" style="display:flow-root">flow-root vs RIGHT float</div></div>
<div class="w"><div class="f"></div><div class="f"></div><div class="b" id="b11" style="display:flow-root">two floats</div></div>
<div class="w"><div class="f" style="height:10px"></div><div class="b" id="b12" style="display:flow-root">short float</div></div>
</body></html>"##;

const ESCAPED_FLOAT_HTML: &str = r##"<!DOCTYPE html><html><head><meta charset="utf-8"><style>
 body{margin:0;font:16px/1.5 sans-serif}
 .f{float:left;width:60px;height:80px;background:#f99}
 .box{background:#cdf;height:20px;width:400px}
</style></head><body>
<div id="host"><div class="f"></div></div>
<div class="box" id="e1" style="display:flow-root">BFC after an ESCAPED float</div>
<div class="box" id="e2">plain block after an escaped float</div>
<div class="box" id="e3" style="overflow:hidden">overflow:hidden after an escaped float</div>
<div class="box" id="e4" style="display:flex">flex after an escaped float</div>
<div class="box" id="e5" style="clear:left">clear:left after an escaped float</div>

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

/// Every claim is a triple, because a mechanism that gets the width right and the position wrong is
/// exactly what this gate's cohort is made of — asserting one axis alone would let it through.
fn c(page: &manuk_page::Page, sel: &str, x: f32, w: f32, h: f32) {
    let (gx, gw, gh) = rect(page, sel);
    assert!(
        (gx - x).abs() < 1.01 && (gw - w).abs() < 1.01 && (gh - h).abs() < 1.01,
        "G_RATIO_INSET_FLOAT: `{sel}` expected x={x} w={w} h={h} (CAPTURED from \
         `google-chrome-stable --headless=new --hide-scrollbars --window-size=1200,800`), \
         got x={gx} w={gw} h={gh}"
    );
}

#[test]
fn g_ratio_inset_float() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://ratio.test/", &fonts, 1200.0);
    c(&page, "#a1", 0.0, 400.0, 225.0);
    c(&page, "#a2", 0.0, 400.0, 225.0);
    c(&page, "#a3", 0.0, 400.0, 675.0);
    c(&page, "#a4", 0.0, 400.0, 200.0);
    c(&page, "#a5", 0.0, 400.0, 225.0);
    c(&page, "#a6", 0.0, 400.0, 225.0);
    c(&page, "#a7", 0.0, 400.0, 40.0);
    c(&page, "#a8", 0.0, 400.0, 0.0);
    c(&page, "#a9", 50.0, 400.0, 40.0);
    c(&page, "#a10", 0.0, 400.0, 225.0);
    c(&page, "#a11", 0.0, 400.0, 225.0);
    c(&page, "#a12", 0.0, 400.0, 225.0);
    c(&page, "#b1", 0.0, 400.0, 20.0);
    c(&page, "#b2", 60.0, 340.0, 20.0);
    c(&page, "#b3", 60.0, 340.0, 20.0);
    c(&page, "#b4", 60.0, 340.0, 20.0);
    c(&page, "#b5", 60.0, 340.0, 20.0);
    c(&page, "#b6", 60.0, 340.0, 20.0);
    c(&page, "#b8", 60.0, 80.0, 20.0);
    c(&page, "#b9", 0.0, 400.0, 20.0);
    c(&page, "#b10", 0.0, 340.0, 20.0);
    c(&page, "#b11", 120.0, 280.0, 20.0);
    c(&page, "#b12", 60.0, 340.0, 20.0);
    c(&page, "#k1", 16.0, 400.0, 20.0);
    c(&page, "#k2", 16.0, 400.0, 20.0);
    c(&page, "#k3", 16.0, 400.0, 20.0);
    c(&page, "#k4", 16.0, 368.0, 20.0);
    c(&page, "#k5", 16.0, 368.0, 20.0);
    c(&page, "#k6", 16.0, 368.0, 20.0);
    c(&page, "#k7", 16.0, 400.0, 20.0);
    c(&page, "#k8", 16.0, 400.0, 20.0);
    c(&page, "#k9", 16.0, 400.0, 20.0);
    c(&page, "#k10", 48.0, 400.0, 20.0);
    c(&page, "#k11", 16.0, 368.0, 20.0);
    c(&page, "#k12", 16.0, 400.0, 20.0);
    c(&page, "#k13", 16.0, 400.0, 20.0);
    c(&page, "#k14", 16.0, 400.0, 20.0);
    c(&page, "#k15", 16.0, 400.0, 20.0);
    c(&page, "#p1", 0.0, 432.0, 20.0);
    c(&page, "#p2", 0.0, 432.0, 20.0);
    c(&page, "#p3", 0.0, 432.0, 20.0);
    c(&page, "#p4", 0.0, 400.0, 20.0);
    c(&page, "#p5", 0.0, 400.0, 20.0);
    c(&page, "#p6", 0.0, 400.0, 20.0);
    c(&page, "#p7", 0.0, 432.0, 20.0);
    c(&page, "#p8", 0.0, 432.0, 20.0);
    c(&page, "#p9", 0.0, 432.0, 20.0);
    c(&page, "#p10", 0.0, 496.0, 20.0);
    c(&page, "#p11", 0.0, 400.0, 20.0);
    c(&page, "#p12", 0.0, 432.0, 20.0);
    c(&page, "#p13", 0.0, 432.0, 20.0);
    c(&page, "#p14", 0.0, 432.0, 20.0);
    c(&page, "#p15", 0.0, 432.0, 20.0);

    // ── THE GUARD, and it is the half that makes the block above mean anything: a plain block MUST
    // overlap a float, and `clear` MUST NOT shift a box horizontally. A change that made every box
    // avoid every float would satisfy every claim above and be badly wrong.
    let esc = manuk_page::Page::load(ESCAPED_FLOAT_HTML, "https://ratio.test/", &fonts, 1200.0);
    let (x2, w2, _) = rect(&esc, "#e2");
    assert!(
        x2 == 0.0 && (w2 - 400.0).abs() < 1.01,
        "a PLAIN block overlaps a float — only its line boxes shorten (Chrome x=0 w=400), got x={x2} w={w2}"
    );
    let (x5, _, _) = rect(&esc, "#e5");
    assert!(
        x5 == 0.0,
        "`clear:left` moves a box DOWN, never right (Chrome x=0), got x={x5}"
    );
    let (_, _, hh) = rect(&esc, "#host");
    assert!(
        hh == 0.0,
        "the float ESCAPES its non-BFC parent, so the parent is zero-height (Chrome h=0) — this is \
         the precondition of the open defect in this file's header, and if it ever stops holding, \
         those numbers are measuring something else"
    );
}
