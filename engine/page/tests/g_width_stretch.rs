//! **G_WIDTH_STRETCH — `width: stretch` fills, for the boxes that do NOT fill on `auto`.**
//!
//! `stretch` (and its shipped aliases `-webkit-fill-available` / `-moz-available`) reached layout as
//! plain `Dim::Auto`, and on an ordinary block box that is the right answer — `auto` fills there
//! too. That equivalence is exactly what hid the gap, because **it holds for the one box shape this
//! does not matter on**. Every box that *shrink-to-fits* on `auto` diverged instead:
//!
//! * a **float** — hugs its text on `auto`, so a floated `width:stretch` card collapsed;
//! * an **inline-block** and a **form control** — hug their content;
//! * a **replaced element** (`<canvas>`, `<img>`) — takes its intrinsic size, or nothing at all;
//! * an **absolutely positioned** box — shrink-to-fits unless both insets are set.
//!
//! Each `.t` here is inside a 200px containing block and has `margin-inline: 10px/20px` plus a
//! `3px` border and `2px` padding. `stretch` sizes the **margin box** to the containing block's
//! content box, so every one of them must come out `200 - 30 = 170px` **border-box** wide — the
//! border and padding are inside that number, which is what makes this a real check of the stretch
//! definition rather than of "did something get wider".
//!
//! `#ctl` is the control: the same box with `width:auto`, which must *keep* shrink-to-fitting. A
//! change that simply made every box fill would pass all four stretch assertions and fail this one.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<style>
  .cb { width: 200px; height: 100px }
  .t  { margin-left: 10px; margin-right: 20px; border: 3px solid black; padding: 2px;
        width: stretch }
</style>
<div id="out">-</div>
<div class="cb"><div class="t" id="flt" style="float:left">x</div></div>
<div class="cb"><div class="t" id="ib" style="display:inline-block">x</div></div>
<div class="cb"><canvas class="t" id="cv" width="40" height="20"></canvas></div>
<div class="cb" style="position:relative"><div class="t" id="ap" style="position:absolute; left:0">x</div></div>
<div class="cb"><input class="t" id="inp"></div>
<div class="cb"><div class="t" id="ctl" style="float:left; width:auto">x</div></div>
<script>
  var R = [];
  var w = function (id) { return Math.round(document.getElementById(id).getBoundingClientRect().width); };
  R.push('flt:' + w('flt'));
  R.push('ib:'  + w('ib'));
  R.push('cv:'  + w('cv'));
  R.push('ap:'  + w('ap'));
  R.push('inp:' + w('inp'));
  R.push('ctl:' + (w('ctl') < 100));
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn width_stretch_fills_the_boxes_that_shrink_to_fit_on_auto() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://gws.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "flt:170",
            "a FLOAT hugs its content on `auto`, so `width:stretch` is the only way to say `fill \
             the column` — margin box fills 200, border box is 200-30 margins = 170",
        ),
        (
            "ib:170",
            "an INLINE-BLOCK likewise shrink-to-fits on `auto` and must fill on `stretch`",
        ),
        (
            "cv:170",
            "a REPLACED element takes its intrinsic 40px on `auto`; `stretch` overrides that and \
             fills the column",
        ),
        (
            "ap:170",
            "an ABSPOS box with only ONE inset shrink-to-fits — `stretch` is the other half of \
             that constraint said in one property, and fills without needing `right` too",
        ),
        (
            "inp:170",
            "a FORM CONTROL carries a UA intrinsic width (`size` characters, ~173px) — but a UA \
             default is outranked by an author declaration, and `width:stretch` is one even though \
             it computes to `Dim::Auto` and so looked absent",
        ),
        (
            "ctl:true",
            "and the control must NOT fill: the same float with `width:auto` still hugs its \
             content. Without this a change that made everything fill would look like a pass",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_WIDTH_STRETCH: expected `{claim}` — {why}.\n  got: {got}"
        );
    }
}
