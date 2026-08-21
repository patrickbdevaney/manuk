//! **G_FILTER_RENDER — CSS `filter` actually changes the pixels.**
//!
//! `filter` is on **51.9% of page loads** (Blink use counters, surface audit #32) and was the board's
//! #1 unmapped capability by usage once t590 re-priced `appearance` down to a no-op. Unlike
//! `appearance`, its impact transfers: **there is no cascade-level workaround for a blur.** A page
//! that writes `@supports (backdrop-filter: blur(…))`, is told yes, and drops the opaque background
//! it shipped for engines that cannot blur, lands its text unreadably over a photograph.
//!
//! Until this tick Stylo parsed and computed `filter` correctly — it always had — and **nothing ever
//! read the computed value** (t591 measured all three routes: no `clone_*` in `stylo_map.rs`, no
//! `ComputedStyle` field, no MinimalCascade entry). t591 made the failure honest by teaching
//! `@supports` to say no. This gate is the other half: the pixels.
//!
//! ## What is asserted, and why each one can go red on its own
//!
//! ```text
//! #plain   background:#f00                    → pure red          (vacuity guard: paint works)
//! #gray    background:#f00 filter:grayscale(1) → (54,54,54)        the 0.213 luminance grey, exactly
//! #blur    background:#f00 filter:blur(6px)    → soft: ink OUTSIDE the box, ramp at the edge
//! #dim > p background:#f00 filter:brightness(0)→ the CHILD is dark too (a filter is a SUBTREE group)
//! ```
//!
//! Claim 1 is a **vacuity guard** and it is not decoration: if the paint path ever stopped filling
//! backgrounds, claims 2-4 would be comparing blank canvases and would pass for the wrong reason —
//! the exact failure mode that let a `cut:false` assertion sit green for 113 ticks on a fixture that
//! could not reach it.
//!
//! Claim 2 is an **exact** value, not a range. `grayscale(1)` has one right answer (Filter Effects 1's
//! 0.213/0.715/0.072 luminance row), and an engine that lands "approximately grey" diverges from every
//! other browser on every screenshot forever.
//!
//! Claim 4 is the one that distinguishes a filter from a per-item colour tweak: CSS applies `filter`
//! to the element **and its subtree, as a group**, so a child with no filter of its own must still
//! come out filtered.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
  body { margin: 0; background: #fff }
  div  { position: absolute; top: 20px; width: 60px; height: 60px; background: #f00 }
  #plain { left: 20px }
  #gray  { left: 120px; filter: grayscale(1) }
  #blur  { left: 220px; filter: blur(6px) }
  #dim   { left: 320px; filter: brightness(0); background: none }
  #dim p { margin: 0; width: 60px; height: 60px; background: #f00 }
  /* The scrolled + clipped case: a 40px-tall `overflow:hidden` window over a 60px filtered block. */
  #clipbox { top: 200px; left: 20px; height: 40px; overflow: hidden; background: none }
  #clipped { top: 0; left: 0; background: #00f; filter: grayscale(1) }
</style></head><body>
<div id="plain"></div>
<div id="gray"></div>
<div id="blur"></div>
<div id="dim"><p></p></div>
<div id="clipbox"><div id="clipped"></div></div>
</body></html>"##;

const W: u32 = 420;
const H: u32 = 140;

/// Straight (non-premultiplied is unnecessary here — the page background is opaque) RGBA at a pixel.
fn at(bytes: &[u8], x: u32, y: u32) -> (u8, u8, u8, u8) {
    let i = ((y * W + x) * 4) as usize;
    (bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3])
}

#[test]
fn css_filter_reaches_the_pixels() {
    // ⚠⚠ **MERGED INTO ONE `#[test]` DELIBERATELY (t1342) — DO NOT SPLIT THIS BACK OUT.**
    //
    // `libtest` spawns a thread per test, including at `--test-threads=1`, and SpiderMonkey allows
    // exactly one JS thread per process: a second one silently runs no script, or SIGSEGVs outright
    // if the first is still alive. Two `#[test]`s in a `Page`-building binary therefore means at most
    // one of them was ever really checked. See `docs/wiki/js-engine.md` and
    // `g_one_js_thread_per_process.rs`. Enforced by `G_ONE_PAGE_TEST_PER_BINARY`.
    a_filtered_group_stays_clipped_when_the_page_is_scrolled();
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://filter.test/", &fonts, W as f32);
    let canvas = page.paint(&fonts, W, H);
    let b = canvas.rgba_bytes();

    let plain = at(b, 50, 50);
    let gray = at(b, 150, 50);
    let blur_mid = at(b, 250, 50);
    let dim = at(b, 350, 50);
    println!("FILTER: plain={plain:?} gray={gray:?} blur={blur_mid:?} dim={dim:?}");

    // 1. VACUITY GUARD — the unfiltered control must be pure red. Without this, every claim below
    //    could be satisfied by a canvas on which nothing painted at all.
    assert_eq!(
        (plain.0, plain.1, plain.2),
        (255, 0, 0),
        "the UNFILTERED control box must paint pure red — if it does not, nothing below is measuring \
         a filter"
    );

    // 2. `grayscale(1)` — the exact spec luminance of #f00: 0.213 × 255 = 54.
    assert_eq!(
        (gray.0, gray.1, gray.2),
        (54, 54, 54),
        "grayscale(1) of #f00 must be the 0.213 luminance grey (54,54,54) — Filter Effects 1 uses the \
         legacy coefficients, NOT Rec.709's 0.2126"
    );

    // 3. `blur(6px)` — ink must ESCAPE the box (a blur bleeds) and the edge must become a ramp
    //    rather than a step. Sampling 4px outside the right edge of #blur (which ends at x=280).
    let outside_blur = at(b, 284, 50);
    let outside_plain = at(b, 84, 50);
    println!("FILTER blur bleed: outside_blur={outside_blur:?} outside_plain={outside_plain:?}");
    assert_eq!(
        outside_plain,
        (255, 255, 255, 255),
        "the same offset outside the UNFILTERED box must still be page white — otherwise 'ink \
         escaped' proves nothing"
    );
    assert!(
        outside_blur.0 == 255 && outside_blur.1 < 250 && outside_blur.2 < 250,
        "blur(6px) must bleed red ink 4px past the box edge; got {outside_blur:?}"
    );
    // …and the box's own interior must still be substantially red, not washed to nothing.
    assert!(
        blur_mid.0 > 200 && blur_mid.1 < 80,
        "the interior of a blurred box must stay red; got {blur_mid:?}"
    );

    // 4. A filter is a SUBTREE group: `#dim` has no background of its own, its `<p>` child does,
    //    and `brightness(0)` on the parent must black out the child.
    assert_eq!(
        (dim.0, dim.1, dim.2),
        (0, 0, 0),
        "brightness(0) on an ancestor must black out its CHILD's background — a filter applies to \
         the element AND its subtree, not just to the box that declared it"
    );
}

/// **An `overflow` clip must survive the trip through the offscreen surface, at a nonzero scroll.**
///
/// A filtered group is rasterized into its own surface whose origin is the group's ink box, so two
/// different coordinate spaces arrive at that function: the display items are in **page** space and
/// still owe the scroll, the clip has **already** been converted to device space by the caller and
/// does not. Applying the same offset to both is the obvious mistake, it double-subtracts the
/// scroll, and it slides the clip clean off the element — and it is **invisible to every gate that
/// renders at scroll 0**, which is what the rest of this file does. (It was written that way first
/// and caught on re-read, not by a test; this is the test.)
fn a_filtered_group_stays_clipped_when_the_page_is_scrolled() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://filter.test/", &fonts, W as f32);
    // `#clipbox` sits at page y=200..240 and its child paints to y=260. At scroll 150 the window is
    // device y=50..90 and the child's overflow (device y=90..110) must be gone.
    let canvas = page.paint_scrolled(&fonts, W, H, 150.0);
    let b = canvas.rgba_bytes();
    let inside = at(b, 40, 70);
    let below = at(b, 40, 100);
    println!("FILTER clip@scroll: inside={inside:?} below={below:?}");

    // grayscale(1) of #00f is the 0.072 luminance row: 0.072 × 255 = 18.
    assert_eq!(
        (inside.0, inside.1, inside.2),
        (18, 18, 18),
        "inside the scrolled clip window the filtered block must paint (18,18,18) — if it is page \
         white, the clip was shifted off the element by a doubled scroll"
    );
    assert_eq!(
        below,
        (255, 255, 255, 255),
        "the filtered block's overflow BELOW the `overflow:hidden` window must be clipped away — a \
         filter must not launder an element out of its ancestor's clip"
    );
}
