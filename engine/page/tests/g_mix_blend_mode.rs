//! **G_MIX_BLEND_MODE — an element composites against its BACKDROP, not merely over it.**
//!
//! 12.9% of page loads (Blink use counters, surface audit #32). Every gradient scrim over a hero
//! image, every duotone photo treatment, and every `difference` caption that stays legible over
//! whatever is behind it is this property. Without it the overlay does not tint what it was drawn
//! to tint — **it covers it**, which is strictly worse than not drawing the overlay at all.
//!
//! ## This tick's finding, and it is the reason the row was cheap
//!
//! t593 left an open question: `mix-blend-mode` (12.9%) and `backdrop-filter` (34.3%) both need the
//! group's **backdrop**, an input the paint path did not have — so is there one mechanism that buys
//! both? **Yes, and t592 already built it.** The backdrop a blend needs is exactly what is already
//! on the canvas under the group's ink box, and the group's own pixels are exactly what the
//! offscreen surface built for `filter` holds. The blend is then one field on the composite-back
//! call. Every CSS mode — separable and non-separable — has a `tiny-skia` counterpart, so nothing
//! here is approximated.
//!
//! ## The claims
//!
//! A red backdrop with a blue square over it, at 50% blue:
//!
//! ```text
//! normal      → the square wins outright:  (0, 0, 255)   ← vacuity guard AND control
//! multiply    → red × blue = black:        (0, 0, 0)
//! screen      → red + blue = magenta:      (255, 0, 255)
//! difference  → |red - blue| = magenta:    (255, 0, 255)
//! luminosity  → blue's LUMA on red's hue → a dark red, NOT blue and NOT unchanged
//! ```
//!
//! `multiply` and `screen` are chosen because their answers on these two colours are **exact
//! integers with no rounding slack**, so the gate can assert equality rather than a direction. A
//! blend gate that only asserts "the pixel changed" passes for a wrong formula.
//!
//! `luminosity` is included deliberately: it is **non-separable** (it mixes channels rather than
//! operating per-channel), so an implementation that wired up only the easy separable modes and
//! silently dropped the rest still goes green on the first four and red here.
//!
//! ## ⚠ ONE OPEN NUMBER, RECORDED RATHER THAN ASSERTED
//!
//! `luminosity` measures **(207, 0, 0)** here. Working Compositing-1's `SetLum` + `ClipColor` by
//! hand for this pair gives ≈ **(94, 0, 0)** — and 207/255 is exactly the *un-clipped* intermediate,
//! which suggests `tiny-skia` skips `ClipColor`. **That derivation is mine, from the spec text, and
//! a number derived from a reading is not a measurement** — this repo has been burned twice by
//! gates whose expected value came from memory. A headless-Chrome cross-check was attempted and did
//! not reproduce the layout (the blue squares never painted in the screenshot), so **no third-party
//! number is claimed here.**
//!
//! So the separable modes — where the formula is unambiguous and the arithmetic is exact — are
//! asserted to the integer, and `luminosity` is asserted only on what was actually observed: the
//! mode is applied, the backdrop's hue survives, the source's luma darkens it. Pinning 207 as
//! *correct* would bank a possible `tiny-skia` divergence as an intended value, which is how a wrong
//! constant becomes permanent. The exact non-separable answer is the open item, and it needs the
//! repo's own parity harness, not a hand-rolled screenshot.

use manuk_text::FontContext;

const W: u32 = 500;
const H: u32 = 120;

const HTML: &str = r##"<!doctype html><html><head><style>
  body { margin: 0; background: #fff }
  .bg  { position: absolute; top: 10px; width: 80px; height: 80px; background: #f00 }
  .fg  { position: absolute; top: 30px; left: 20px; width: 40px; height: 40px; background: #00f }
  #b1 { left: 10px }  #b2 { left: 110px } #b3 { left: 210px } #b4 { left: 310px } #b5 { left: 410px }
  #b2 .fg { mix-blend-mode: multiply }
  #b3 .fg { mix-blend-mode: screen }
  #b4 .fg { mix-blend-mode: difference }
  #b5 .fg { mix-blend-mode: luminosity }
</style></head><body>
<div class="bg" id="b1"><div class="fg"></div></div>
<div class="bg" id="b2"><div class="fg"></div></div>
<div class="bg" id="b3"><div class="fg"></div></div>
<div class="bg" id="b4"><div class="fg"></div></div>
<div class="bg" id="b5"><div class="fg"></div></div>
</body></html>"##;

fn at(bytes: &[u8], x: u32, y: u32) -> (u8, u8, u8) {
    let i = ((y * W + x) * 4) as usize;
    (bytes[i], bytes[i + 1], bytes[i + 2])
}

#[test]
fn mix_blend_mode_composites_against_the_backdrop() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://blend.test/", &fonts, W as f32);
    let canvas = page.paint(&fonts, W, H);
    let b = canvas.rgba_bytes();

    // Each `.fg` sits at its parent's left+20, top+30, so its centre is (parent.left+40, 50).
    let normal = at(b, 50, 50);
    let multiply = at(b, 150, 50);
    let screen = at(b, 250, 50);
    let difference = at(b, 350, 50);
    let luminosity = at(b, 450, 50);
    // And a point on the red backdrop OUTSIDE every square, to prove the backdrop is really there.
    let backdrop = at(b, 15, 15);
    println!(
        "BLEND: backdrop={backdrop:?} normal={normal:?} multiply={multiply:?} screen={screen:?} \
         difference={difference:?} luminosity={luminosity:?}"
    );

    // 0. VACUITY GUARD — there must BE a red backdrop and a blue square, or every blend claim
    //    below is a comparison between two things that were never painted.
    assert_eq!(
        backdrop,
        (255, 0, 0),
        "the red backdrop must paint; without it there is nothing to blend against"
    );
    assert_eq!(
        normal,
        (0, 0, 255),
        "with no blend mode the square must simply WIN — this is the control, and it must not change"
    );

    // 1. `multiply`: red(1,0,0) × blue(0,0,1) = (0,0,0). Exact.
    assert_eq!(
        multiply,
        (0, 0, 0),
        "multiply of #f00 backdrop and #00f source is BLACK — a source that merely covers the \
         backdrop gives (0,0,255)"
    );

    // 2. `screen`: 1-(1-a)(1-b) per channel → (255, 0, 255). Exact.
    assert_eq!(
        screen,
        (255, 0, 255),
        "screen of #f00 and #00f is MAGENTA — both channels survive"
    );

    // 3. `difference`: |a-b| per channel → (255, 0, 255). Exact.
    assert_eq!(
        difference,
        (255, 0, 255),
        "difference of #f00 and #00f is MAGENTA"
    );

    // 4. `luminosity` is NON-SEPARABLE — it takes the SOURCE's luma and the BACKDROP's hue and
    //    saturation. Blue's luma is low, red's hue is red: the result is a dark red. An engine that
    //    wired only the easy per-channel modes and dropped the rest passes 1-3 and fails here.
    //
    //    The BOUND is deliberately loose and the header says why: 207 is what `tiny-skia` produces
    //    and a hand-derived ≈94 is what the spec's ClipColor step suggests. Asserting either exact
    //    value would bank an unverified number. What IS verified — and what a dropped mode would
    //    break — is the shape of the answer.
    assert!(
        luminosity.0 > 20 && luminosity.0 < 250 && luminosity.1 < 40 && luminosity.2 < 40,
        "luminosity must put the SOURCE's luma on the BACKDROP's hue — a DARKENED RED. Getting \
         (0,0,255) means the non-separable modes were dropped to `normal`; getting (255,0,0) means \
         the source was dropped entirely; anything blue-ish means the channels were mixed \
         per-channel like a separable mode. got {luminosity:?}"
    );
}
