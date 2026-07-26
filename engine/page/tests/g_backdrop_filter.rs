//! **G_BACKDROP_FILTER — the frosted panel actually blurs what is behind it.**
//!
//! 34.3% of page loads (Blink use counters, surface audit #32), and **the single costliest property
//! in this engine to answer wrongly.** It is the example `G_SUPPORTS_HONESTY` was written around: a
//! page writes `@supports (backdrop-filter: blur(12px))`, is told yes, drops the opaque background
//! it shipped for engines that cannot blur, and lands its text unreadable over a photograph. It has
//! been an honest **no** since t576, through three ticks in which `filter`, `clip-path` and
//! `mix-blend-mode` all landed around it.
//!
//! ## Why it was last, and why it was still small
//!
//! Every other property in the bundle operates on the **element's own pixels**, which t592's
//! offscreen group already separates out. This one operates on the pixels the element is about to
//! *cover* — a different input, which is exactly why it could not be carried along with `filter`
//! and why its constellation row was split out rather than promoted. The implementation is
//! correspondingly the only one that needed new code rather than a new field: read the canvas region
//! back, filter that copy, write it down with `Source` (a replace, not a composite), then let the
//! normal group path paint the element on top.
//!
//! ## The claims
//!
//! A hard black/white vertical edge, with a half-transparent panel over the seam:
//!
//! ```text
//! #plain   (no backdrop-filter)  → the seam under it stays HARD: black on one side, white on the other
//! #frost   backdrop-filter:blur  → the seam under it becomes a RAMP (mid-greys appear)
//! outside  either panel          → the seam is untouched, so the blur did not escape the box
//! ```
//!
//! Claim 3 is not a formality. A backdrop filter that blurred the whole canvas would satisfy claims
//! 1 and 2 and be catastrophically wrong — the property's entire contract is that the effect stops
//! at the element's border box.
//!
//! The panel is `rgba(255,255,255,.2)` rather than opaque on purpose: an opaque panel would hide the
//! backdrop entirely and the test would be measuring the panel, not the blur.

use manuk_text::FontContext;

const W: u32 = 400;
const H: u32 = 200;

const HTML: &str = r##"<!doctype html><html><head><style>
  body { margin: 0; background: #fff }
  /* A hard vertical seam: black left half, white right half, full height. */
  #dark  { position: absolute; left: 0; top: 0; width: 200px; height: 200px; background: #000 }
  .panel { position: absolute; top: 40px; width: 120px; height: 40px;
           background: rgba(255,255,255,0.2) }
  #plain { left: 140px }
  #frost { left: 140px; top: 120px; backdrop-filter: blur(8px) }
</style></head><body>
<div id="dark"></div>
<div class="panel" id="plain"></div>
<div class="panel" id="frost"></div>
</body></html>"##;

fn at(bytes: &[u8], x: u32, y: u32) -> (u8, u8, u8) {
    let i = ((y * W + x) * 4) as usize;
    (bytes[i], bytes[i + 1], bytes[i + 2])
}
/// A pixel that is neither near-black nor near-white — i.e. the seam has been softened.
fn is_mid(p: (u8, u8, u8)) -> bool {
    p.0 > 40 && p.0 < 215
}

#[test]
fn backdrop_filter_blurs_what_is_behind_the_element_and_only_there() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://frost.test/", &fonts, W as f32);
    let canvas = page.paint(&fonts, W, H);
    let b = canvas.rgba_bytes();

    // The seam is at x=200. Sample a short span across it at three heights:
    //   y=10   — above both panels        (untouched control)
    //   y=60   — inside #plain            (panel, but NO backdrop-filter)
    //   y=140  — inside #frost            (panel WITH backdrop-filter)
    let span = |y: u32| -> Vec<(u8, u8, u8)> { (194..=206).map(|x| at(b, x, y)).collect() };
    let above = span(10);
    let plain = span(60);
    let frost = span(140);
    println!("BACKDROP above={above:?}");
    println!("BACKDROP plain={plain:?}");
    println!("BACKDROP frost={frost:?}");

    // 0. VACUITY GUARD — there must BE a hard seam to blur.
    assert!(
        above[0].0 < 40 && above[12].0 > 215,
        "the black/white seam must exist above the panels — without it there is nothing to blur \
         and every claim below is vacuous. got {above:?}"
    );
    assert!(
        !above.iter().any(|&p| is_mid(p)),
        "the untouched seam must be HARD (no mid-greys) — if it is already soft, claim 2 proves \
         nothing. got {above:?}"
    );

    // 1. Under a panel with NO backdrop-filter the seam stays hard (the panel only lightens it).
    //    This is the control: it isolates the blur from the panel's own translucency.
    let plain_mids = plain.iter().filter(|&&p| is_mid(p)).count();

    // 2. Under the frosted panel the seam becomes a RAMP.
    let frost_mids = frost.iter().filter(|&&p| is_mid(p)).count();
    assert!(
        frost_mids >= 4,
        "backdrop-filter: blur(8px) must soften the hard seam beneath the panel into a ramp of \
         mid-tones; only {frost_mids} of 13 sampled pixels are mid-grey. got {frost:?}"
    );
    assert!(
        frost_mids > plain_mids,
        "the FROSTED panel must soften the seam strictly more than the identical unfiltered panel \
         over the same seam ({frost_mids} vs {plain_mids} mid pixels) — otherwise what is being \
         measured is the panel's own alpha, not the blur"
    );

    // 3. THE CONFINEMENT CLAIM — a backdrop filter that blurred the whole canvas would pass 1 and 2
    //    and be catastrophically wrong. #frost spans x=140..260, y=120..160; sample well outside.
    let outside_below = span(190);
    assert!(
        !outside_below.iter().any(|&p| is_mid(p)),
        "the seam BELOW the frosted panel must still be hard — a backdrop-filter is confined to its \
         own border box and must not blur the page. got {outside_below:?}"
    );
}
