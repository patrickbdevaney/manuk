//! **G_CLIP_PATH — `clip-path` basic shapes actually cut the pixels.**
//!
//! 43.8% of page loads (Blink use counters, surface audit #32). Like `filter` before tick 592, Stylo
//! parsed and computed it natively and nothing ever read the result, so every `clip-path` on the web
//! rendered as **the full rectangle**.
//!
//! **MEASURED FIRST, per §VI.3 — and the failure is not uniformly cosmetic.** A round avatar
//! rendering square is a blemish. These are not:
//!
//! - `inset(50%)` is Bootstrap 5's `.visually-hidden` and the modern replacement for
//!   `clip: rect(0,0,0,0)`. Ignore it and **screen-reader-only text renders on the page** — the same
//!   class of defect as rendering an `opacity: 0` fade-in's base rule, with the sign flipped.
//! - `polygon()` diagonal section dividers render as full-bleed rectangles that **cover the content
//!   beneath them**, because the part the author clipped away is exactly the part that overlaps.
//!
//! So the shape must be able to produce an EMPTY region, and that is the one case where "clip
//! nothing on failure" is the wrong default — hence the explicit empty-inset branch in
//! `apply_clip_shape`, and claim 4 below.
//!
//! ## The claims
//!
//! ```text
//! #plain    (no clip)                        → the whole 60×60 square is red   (vacuity guard)
//! #circle   circle(50%)                      → centre red, CORNERS cut away
//! #poly     polygon(0 0, 100% 0, 100% 100%)  → upper-right triangle kept, lower-LEFT cut
//! #hidden   inset(50%)                       → NOTHING paints (the .visually-hidden idiom)
//! ```
//!
//! Claim 1 is a **vacuity guard**: with no red anywhere, every "this pixel is white" assertion
//! below passes for the wrong reason. That is the failure mode that kept a `cut:false` assertion
//! green for 113 ticks on a fixture that could not reach it.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
  body { margin: 0; background: #fff }
  div  { position: absolute; top: 20px; width: 60px; height: 60px; background: #f00 }
  #plain  { left: 20px }
  #circle { left: 120px; clip-path: circle(50%) }
  #poly   { left: 220px; clip-path: polygon(0 0, 100% 0, 100% 100%) }
  #hidden { left: 320px; clip-path: inset(50%) }
</style></head><body>
<div id="plain"></div>
<div id="circle"></div>
<div id="poly"></div>
<div id="hidden"></div>
</body></html>"##;

const W: u32 = 420;
const H: u32 = 140;

fn at(bytes: &[u8], x: u32, y: u32) -> (u8, u8, u8, u8) {
    let i = ((y * W + x) * 4) as usize;
    (bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3])
}
fn is_red(p: (u8, u8, u8, u8)) -> bool {
    p.0 > 200 && p.1 < 80 && p.2 < 80
}
fn is_white(p: (u8, u8, u8, u8)) -> bool {
    p.0 > 245 && p.1 > 245 && p.2 > 245
}

#[test]
fn clip_path_basic_shapes_cut_the_pixels() {
    // ⚠⚠ **MERGED INTO ONE `#[test]` DELIBERATELY (t1342) — DO NOT SPLIT THIS BACK OUT.**
    //
    // `libtest` spawns a thread per test, including at `--test-threads=1`, and SpiderMonkey allows
    // exactly one JS thread per process: a second one silently runs no script, or SIGSEGVs outright
    // if the first is still alive. Two `#[test]`s in a `Page`-building binary therefore means at most
    // one of them was ever really checked. See `docs/wiki/js-engine.md` and
    // `g_one_js_thread_per_process.rs`. Enforced by `G_ONE_PAGE_TEST_PER_BINARY`.
    a_clip_path_applies_to_the_subtree_against_the_declaring_box();
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://clip.test/", &fonts, W as f32);
    let canvas = page.paint(&fonts, W, H);
    let b = canvas.rgba_bytes();

    // 1. VACUITY GUARD — the unclipped control fills its whole box, corners included.
    let plain_centre = at(b, 50, 50);
    let plain_corner = at(b, 22, 22);
    println!("CLIP plain: centre={plain_centre:?} corner={plain_corner:?}");
    assert!(
        is_red(plain_centre) && is_red(plain_corner),
        "the UNCLIPPED control must fill its box CORNERS INCLUDED — otherwise every 'this pixel is \
         white' claim below passes for the wrong reason. centre={plain_centre:?} \
         corner={plain_corner:?}"
    );

    // 2. `circle(50%)` — the inscribed circle. Centre kept, corners cut.
    //    Box is x=120..180, y=20..80; centre (150,50), radius 30.
    let c_centre = at(b, 150, 50);
    let c_corner = at(b, 122, 22);
    println!("CLIP circle: centre={c_centre:?} corner={c_corner:?}");
    assert!(
        is_red(c_centre),
        "circle(50%) must KEEP the centre of the box; got {c_centre:?}"
    );
    assert!(
        is_white(c_corner),
        "circle(50%) must CUT the box corner — a corner is outside the inscribed circle by \
         (30-30/√2)≈9px. got {c_corner:?}"
    );

    // 3. `polygon(0 0, 100% 0, 100% 100%)` — the upper-right triangle. Box x=220..280, y=20..80.
    //    The top-right corner is inside the triangle; the bottom-left corner is not.
    let p_keep = at(b, 276, 24);
    let p_cut = at(b, 224, 76);
    println!("CLIP poly: keep={p_keep:?} cut={p_cut:?}");
    assert!(
        is_red(p_keep),
        "the polygon's own corner (top-right) must be kept; got {p_keep:?}"
    );
    assert!(
        is_white(p_cut),
        "the half the polygon excludes (bottom-left) must be cut — this is the diagonal section \
         divider that otherwise renders as a full-bleed rectangle over the content below it. got \
         {p_cut:?}"
    );

    // 4. `inset(50%)` — THE `.visually-hidden` IDIOM. Overlapping insets are an EMPTY region, and
    //    an implementation that clamps them to a non-negative rect renders the screen-reader-only
    //    text it exists to remove.
    for (x, y) in [(322, 22), (350, 50), (378, 78)] {
        let p = at(b, x, y);
        assert!(
            is_white(p),
            "inset(50%) is an EMPTY clip — Bootstrap's `.visually-hidden`. Nothing may paint \
             anywhere in the box; ({x},{y}) is {p:?}"
        );
    }
}

/// **A `clip-path` clips the element's whole SUBTREE, and the shape resolves against the box that
/// DECLARED it — not against the descendant being painted.**
///
/// Those are two separate mistakes with the same symptom (content in the wrong place), and a
/// single-element gate cannot tell them apart.
///
/// ⚠ **RESIDUE, found by this test and named rather than papered over.** The child here is
/// **in-flow**. An `position: absolute` child is hoisted out of its ancestor's box subtree by
/// `position_absolutes`, so it is no longer a descendant at paint time and this clip does not reach
/// it — exactly as an `overflow: hidden` ancestor's clip does not. That is a box-tree limitation
/// shared by both, not a `clip-path` one, and it is the same shape in both cases: **a paint-time
/// tree walk cannot see a box the layout pass re-parented.**
fn a_clip_path_applies_to_the_subtree_against_the_declaring_box() {
    const NESTED: &str = r##"<!doctype html><html><head><style>
      body { margin: 0; background: #fff }
      #outer { position: absolute; left: 20px; top: 20px; width: 100px; height: 100px;
               background: none; clip-path: circle(50%) }
      #inner { width: 100px; height: 100px; background: #00f; margin: 0 }
    </style></head><body><div id="outer"><div id="inner"></div></div></body></html>"##;
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(NESTED, "https://clip.test/", &fonts, W as f32);
    let b = page.paint(&fonts, W, H);
    let b = b.rgba_bytes();
    // #outer's box is x=20..120, y=20..120 → circle centred (70,70) r=50.
    let centre = at(b, 70, 70);
    let corner = at(b, 24, 24);
    println!("CLIP subtree: centre={centre:?} corner={corner:?}");
    assert!(
        centre.2 > 200 && centre.0 < 80,
        "the CHILD's background must paint inside the ancestor's circle — a clip-path applies to \
         the element AND its subtree, and the child declares no clip of its own; got {centre:?}"
    );
    assert!(
        is_white(corner),
        "the child must be cut at the ANCESTOR's circle. The child fills the same box here, so a \
         corner that survives means the clip never reached the subtree; got {corner:?}"
    );
}
