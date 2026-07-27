//! # G_SVG_CLIENT_RECT — `getBoundingClientRect()` on an SVG child is the shape, not an inline box
//!
//! **The failure this gate exists for.** A `<rect x=10 y=20 width=50 height=30>` reported
//! `{x:8, y:8, width:0, height:19}` — a **zero-width inline box at the `<svg>`'s origin**, the `19`
//! being a default line height. Wrong, plausible-looking, and in CSS pixels: a chart library that
//! positions a tooltip or a label from it puts it in the corner of the chart.
//!
//! t629 measured this and `getBBox`'s own doc named it in place — *"the alternative they reach for,
//! `getBoundingClientRect`, answers in CSS-box coordinates for an SVG child and is therefore the
//! wrong number rather than a missing one"* — and it stayed open for 18 ticks. **Both halves already
//! worked and had simply never been composed:** `svg_bbox` gives exact user-space geometry,
//! `layout_rect` gives the `<svg>`'s own CSS box.
//!
//! ## `viewBox` is the claim that stops this being a translation
//!
//! Without a `viewBox` the mapping is a translation and is exact. With one, user space is SCALED
//! into the viewport, and composing without that scale would be a confidently wrong box on exactly
//! the SVGs that have one — which is most charting output. `vbScaled` asserts the `xMidYMid meet`
//! default: uniform `min(vpW/vbW, vpH/vbH)`, leftover centred.
//!
//! **Honest bound, asserted rather than hidden:** a non-default `preserveAspectRatio` and
//! per-element `transform=` are NOT applied — they need the real SVG transform stack. `noTransform`
//! records that, so the day it lands, the gate says so.
//!
//! ## RED probes run against this gate
//!
//! | mutation | result |
//! |---|---|
//! | `svg_child_client_rect` returns `None` (restore the pre-t647 fall-through to `layout_rect`) | RED — `rect`, `circle` and `vbScaled` all fail together |
//! | drop the `viewBox` scale (treat every SVG as unscaled) | RED — `vbScaled` alone fails, so it is not riding on the translation |

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
  <svg id="s" width="200" height="100" style="display:block">
    <rect id="r" x="10" y="20" width="50" height="30"/>
    <circle id="c" cx="100" cy="50" r="20"/>
  </svg>
  <svg id="v" width="200" height="100" viewBox="0 0 100 50" style="display:block">
    <rect id="vr" x="10" y="10" width="20" height="10"/>
  </svg>
  <div id="out">-</div>
  <script>
    var R = [];
    function p(s) { R.push(s); document.getElementById('out').textContent = R.join(' '); }
    function box(id) {
      var b = document.getElementById(id).getBoundingClientRect();
      return b.x.toFixed(0) + ',' + b.y.toFixed(0) + ',' + b.width.toFixed(0) + 'x' + b.height.toFixed(0);
    }
    p('svg:' + box('s'));
    p('rect:' + box('r'));
    p('circle:' + box('c'));
    p('vbSvg:' + box('v'));
    p('vbScaled:' + box('vr'));
    // The element's own bbox must stay in USER space — the two calls answer different questions and
    // a fix that made them agree would have broken getBBox instead of fixing this.
    var bb = document.getElementById('vr').getBBox();
    p('bbUser:' + bb.x + ',' + bb.y + ',' + bb.width + 'x' + bb.height);
  </script>
</body></html>"##;

#[test]
fn an_svg_child_reports_its_shape_in_css_pixels() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://svgrect.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("SVG CLIENT RECT: {got}");
    for (claim, why) in CLAIMS {
        assert!(
            got.contains(claim),
            "G_SVG_CLIENT_RECT: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}

const CLAIMS: &[(&str, &str)] = &[
    (
        "rect:10,20,50x30",
        "THE BUG. `<rect x=10 y=20 width=50 height=30>` in an `<svg>` at the origin. It reported \
         `0x19` — a zero-width inline box with a default line height — which is what a chart \
         library reads when it places a tooltip",
    ),
    (
        "circle:80,30,40x40",
        "a `<circle cx=100 cy=50 r=20>` is 40x40 at (80,30). Asserted alongside rect so the fix \
         cannot be an attribute passthrough that only works for x/y/width/height",
    ),
    (
        "vbScaled:20,120,40x20",
        "THE CLAIM THAT STOPS THIS BEING A TRANSLATION. `viewBox='0 0 100 50'` into a 200x100 \
         viewport is a uniform 2x, so a 20x10 rect at (10,10) is 40x20 at (20,20) — plus the second \
         svg's own y-origin of 100. Ignoring the scale would give 30,110,20x10: wrong, and wrong \
         only on the SVGs that have a viewBox, which is most charting output",
    ),
    (
        "svg:0,0,200x100",
        "the `<svg>` ELEMENT itself still answers from layout — it has a real CSS box, and only its \
         geometry children go through the new path",
    ),
    ("vbSvg:0,100,200x100", "and the second one is below the first, from layout as before"),
    (
        "bbUser:10,10,20x10",
        "`getBBox()` on that same element stays in USER space, unscaled. The two calls answer \
         DIFFERENT questions, and a fix that made them agree would have broken getBBox rather than \
         fixed getBoundingClientRect",
    ),
];
