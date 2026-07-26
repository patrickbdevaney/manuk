//! CSS `filter` — the pixel pipeline that runs over a group's offscreen surface.
//!
//! **Why this exists at all.** `filter` is on **51.9% of page loads** (Blink use counters, surface
//! audit #32) and it is the one visual-effects property with no cascade-level fallback: an author
//! who asks for `blur(12px)` behind a nav bar and gets a sharp image has not lost polish, they have
//! lost the contrast their text depends on. Until tick 592 Stylo parsed and computed the property
//! correctly and *nothing ever read it* — which is also why `@supports (filter: blur(4px))` was
//! answering **yes** (fixed t591, honestly, to no).
//!
//! **The surface is premultiplied RGBA8**, which is what `tiny-skia` hands us and what these passes
//! must hand back. Blur is linear, so it runs directly on premultiplied samples (that is the whole
//! point of premultiplication — averaging straight-alpha colour bleeds the transparent pixels'
//! nominal colour into the edge). The colour filters are defined on *straight* colour, so those
//! round-trip through `demultiply()`/`premultiply()` per pixel.
//!
//! Every coefficient here is from Filter Effects 1 §. They are not tuned by eye, and they should
//! not be: `grayscale(1)` has one right answer and a browser that guesses it renders every
//! screenshot-diffing test a shade off forever.

use manuk_css::{FilterOp, Rgba};
use tiny_skia::Pixmap;

/// Run the function list over `px`, **in order**. The order is the author's and it is load-bearing:
/// `grayscale(1) sepia(1)` is a sepia photo, `sepia(1) grayscale(1)` is a grey one.
pub(crate) fn apply_filters(px: &mut Pixmap, ops: &[FilterOp]) {
    for op in ops {
        match *op {
            FilterOp::Blur(sigma) => blur(px, sigma),
            FilterOp::DropShadow {
                dx,
                dy,
                blur: b,
                color,
            } => drop_shadow(px, dx, dy, b, color),
            FilterOp::Opacity(a) => scale_alpha(px, a),
            other => {
                if let Some(m) = color_matrix(other) {
                    apply_color_matrix(px, &m);
                }
            }
        }
    }
}

/// The sRGB luminance coefficients the filter matrices are built from (Filter Effects 1 uses the
/// legacy 0.213/0.715/0.072 triple, **not** Rec.709's 0.2126/0.7152/0.0722 — a browser that
/// "corrects" this diverges from every other browser).
const LR: f32 = 0.213;
const LG: f32 = 0.715;
const LB: f32 = 0.072;

/// A 3×4 colour matrix: three rows of `[r, g, b, offset]`, applied to straight-alpha channels in
/// 0..1. Alpha is untouched by all of these (`opacity()` is handled separately because it is the
/// only one that is *not* a colour matrix in this sense).
type ColorMatrix = [[f32; 4]; 3];

/// The matrix for one colour filter, or `None` for the ops handled elsewhere.
fn color_matrix(op: FilterOp) -> Option<ColorMatrix> {
    Some(match op {
        // `brightness` scales, `contrast` scales about the 0.5 midpoint, `invert` reflects.
        FilterOp::Brightness(k) => [[k, 0.0, 0.0, 0.0], [0.0, k, 0.0, 0.0], [0.0, 0.0, k, 0.0]],
        FilterOp::Contrast(k) => {
            let o = 0.5 - 0.5 * k;
            [[k, 0.0, 0.0, o], [0.0, k, 0.0, o], [0.0, 0.0, k, o]]
        }
        FilterOp::Invert(a) => {
            let k = 1.0 - 2.0 * a;
            [[k, 0.0, 0.0, a], [0.0, k, 0.0, a], [0.0, 0.0, k, a]]
        }
        // `grayscale(a)` IS `saturate(1 - a)` — the spec defines it that way, so deriving it here
        // keeps the two from drifting apart.
        FilterOp::Grayscale(a) => return color_matrix(FilterOp::Saturate(1.0 - a)),
        FilterOp::Saturate(s) => [
            [LR + (1.0 - LR) * s, LG - LG * s, LB - LB * s, 0.0],
            [LR - LR * s, LG + (1.0 - LG) * s, LB - LB * s, 0.0],
            [LR - LR * s, LG - LG * s, LB + (1.0 - LB) * s, 0.0],
        ],
        FilterOp::Sepia(a) => {
            // Lerp identity → the spec's sepia matrix by `a`.
            const S: [[f32; 3]; 3] = [
                [0.393, 0.769, 0.189],
                [0.349, 0.686, 0.168],
                [0.272, 0.534, 0.131],
            ];
            let mut m: ColorMatrix = [[0.0; 4]; 3];
            for (i, row) in m.iter_mut().enumerate() {
                for (j, cell) in row.iter_mut().take(3).enumerate() {
                    let ident = if i == j { 1.0 } else { 0.0 };
                    *cell = ident * (1.0 - a) + S[i][j] * a;
                }
            }
            m
        }
        FilterOp::HueRotate(deg) => {
            let (s, c) = deg.to_radians().sin_cos();
            [
                [
                    LR + c * (1.0 - LR) - s * LR,
                    LG - c * LG - s * LG,
                    LB - c * LB + s * (1.0 - LB),
                    0.0,
                ],
                [
                    LR - c * LR + s * 0.143,
                    LG + c * (1.0 - LG) + s * 0.140,
                    LB - c * LB - s * 0.283,
                    0.0,
                ],
                [
                    LR - c * LR - s * (1.0 - LR),
                    LG - c * LG + s * LG,
                    LB + c * (1.0 - LB) + s * LB,
                    0.0,
                ],
            ]
        }
        _ => return None,
    })
}

/// Apply a colour matrix per pixel. Fully transparent pixels are skipped — their colour is
/// unobservable and demultiplying them is a divide by zero.
fn apply_color_matrix(px: &mut Pixmap, m: &ColorMatrix) {
    for p in px.pixels_mut() {
        if p.alpha() == 0 {
            continue;
        }
        let c = p.demultiply();
        let (r, g, b) = (
            c.red() as f32 / 255.0,
            c.green() as f32 / 255.0,
            c.blue() as f32 / 255.0,
        );
        let ch = |row: &[f32; 4]| {
            ((row[0] * r + row[1] * g + row[2] * b + row[3]).clamp(0.0, 1.0) * 255.0).round() as u8
        };
        // `premultiply()` re-establishes the invariant tiny-skia relies on (r,g,b <= a).
        *p =
            tiny_skia::ColorU8::from_rgba(ch(&m[0]), ch(&m[1]), ch(&m[2]), c.alpha()).premultiply();
    }
}

/// `opacity(k)` — scale alpha. On premultiplied data every channel scales together, which is both
/// correct and cheaper than a round-trip.
fn scale_alpha(px: &mut Pixmap, k: f32) {
    let k = k.clamp(0.0, 1.0);
    if k >= 1.0 {
        return;
    }
    for b in px.data_mut() {
        *b = ((*b as f32) * k).round() as u8;
    }
}

/// The box-blur radius that approximates a Gaussian of standard deviation `sigma`, per the SVG
/// filter spec's own three-box recipe (`d = floor(s * 3 * sqrt(2π) / 4 + 0.5)`).
fn box_radius(sigma: f32) -> usize {
    let d = (sigma * 3.0 * (2.0 * std::f32::consts::PI).sqrt() / 4.0 + 0.5).floor();
    // Cap it: `blur(9999px)` is a page that would otherwise stall the frame, and beyond a few
    // hundred px of standard deviation the result is a flat wash either way.
    ((d / 2.0).max(1.0) as usize).min(256)
}

/// `blur(sigma)` — three box blurs, which is the standard Gaussian approximation and the one the
/// SVG spec prescribes. Runs on premultiplied samples (blur is linear, so this is exact).
pub(crate) fn blur(px: &mut Pixmap, sigma: f32) {
    if !(sigma > 0.0) {
        return;
    }
    let (w, h) = (px.width() as usize, px.height() as usize);
    if w == 0 || h == 0 {
        return;
    }
    let r = box_radius(sigma);
    let mut a = px.data().to_vec();
    let mut b = vec![0u8; a.len()];
    for _ in 0..3 {
        box_blur_axis(&a, &mut b, w, h, r, true);
        box_blur_axis(&b, &mut a, w, h, r, false);
    }
    px.data_mut().copy_from_slice(&a);
}

/// One box-blur pass along one axis with a sliding window, so the cost is O(pixels) regardless of
/// the radius. Samples outside the surface clamp to the edge pixel (SVG's `duplicate` edge mode).
fn box_blur_axis(src: &[u8], dst: &mut [u8], w: usize, h: usize, r: usize, horizontal: bool) {
    let (lanes, len, stride, lane_stride) = if horizontal {
        (h, w, 4usize, w * 4)
    } else {
        (w, h, w * 4, 4usize)
    };
    let win = (2 * r + 1) as u32;
    for lane in 0..lanes {
        let base = lane * lane_stride;
        let at = |i: isize| base + (i.clamp(0, len as isize - 1) as usize) * stride;
        let mut acc = [0u32; 4];
        for k in -(r as isize)..=(r as isize) {
            let p = at(k);
            for (i, s) in acc.iter_mut().enumerate() {
                *s += src[p + i] as u32;
            }
        }
        for x in 0..len {
            let o = base + x * stride;
            for (i, s) in acc.iter().enumerate() {
                // **Round, do not truncate.** Six integer passes each biased downward by up to
                // 1/window darkens a large flat region visibly — `blur()` must soften an area, not
                // dim it. Rounding makes the error unbiased instead of cumulative.
                dst[o + i] = ((s + win / 2) / win) as u8;
            }
            let out = at(x as isize - r as isize);
            let inn = at(x as isize + r as isize + 1);
            for (i, s) in acc.iter_mut().enumerate() {
                *s = *s + src[inn + i] as u32 - src[out + i] as u32;
            }
        }
    }
}

/// `drop-shadow(dx dy blur color)` — a shadow of the surface's **alpha silhouette**, painted behind
/// it. That silhouette is the entire difference from `box-shadow`: a cut-out PNG or an icon glyph
/// casts the shape of its ink, not the shape of its box.
fn drop_shadow(px: &mut Pixmap, dx: f32, dy: f32, blur_px: f32, color: Rgba) {
    let Some(mut shadow) = Pixmap::new(px.width(), px.height()) else {
        return;
    };
    // Tint the source's alpha with the shadow colour, straight into premultiplied form.
    for (dstp, srcp) in shadow.pixels_mut().iter_mut().zip(px.pixels()) {
        let a = ((srcp.alpha() as u32) * (color.a as u32) / 255) as u8;
        *dstp = tiny_skia::ColorU8::from_rgba(color.r, color.g, color.b, a).premultiply();
    }
    blur(&mut shadow, blur_px);

    // Shadow first, then the original over it.
    let Some(mut out) = Pixmap::new(px.width(), px.height()) else {
        return;
    };
    let paint = tiny_skia::PixmapPaint::default();
    out.draw_pixmap(
        dx.round() as i32,
        dy.round() as i32,
        shadow.as_ref(),
        &paint,
        tiny_skia::Transform::identity(),
        None,
    );
    out.draw_pixmap(
        0,
        0,
        px.as_ref(),
        &paint,
        tiny_skia::Transform::identity(),
        None,
    );
    px.data_mut().copy_from_slice(out.data());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fill a 1-pixel-tall surface with one opaque colour and read back the filtered pixel.
    fn one_px(color: Rgba, ops: &[FilterOp]) -> (u8, u8, u8, u8) {
        let mut px = Pixmap::new(1, 1).unwrap();
        px.pixels_mut()[0] =
            tiny_skia::ColorU8::from_rgba(color.r, color.g, color.b, color.a).premultiply();
        apply_filters(&mut px, ops);
        let p = px.pixels()[0].demultiply();
        (p.red(), p.green(), p.blue(), p.alpha())
    }

    /// The colour matrices are spec constants, so they have exact expected answers. A filter that
    /// is merely "in the right direction" is the kind of approximation that never gets corrected.
    #[test]
    fn colour_filters_hit_their_spec_values() {
        let red = Rgba {
            r: 255,
            g: 0,
            b: 0,
            a: 255,
        };
        // grayscale(1) of pure red = the luminance coefficient, 0.213 * 255 = 54.3 → 54.
        let g = one_px(red, &[FilterOp::Grayscale(1.0)]);
        assert_eq!(
            (g.0, g.1, g.2),
            (54, 54, 54),
            "grayscale(1) of #f00 must be the 0.213 luminance grey; got {g:?}"
        );
        // grayscale(0) is the identity — an amount of zero must not touch the pixel.
        assert_eq!(one_px(red, &[FilterOp::Grayscale(0.0)]), (255, 0, 0, 255));
        // invert(1) of red is cyan.
        assert_eq!(one_px(red, &[FilterOp::Invert(1.0)]), (0, 255, 255, 255));
        // brightness(0) is black, and alpha survives it (a common way to dim a thumbnail).
        assert_eq!(one_px(red, &[FilterOp::Brightness(0.0)]), (0, 0, 0, 255));
        // opacity(0.5) halves alpha and nothing else.
        let o = one_px(red, &[FilterOp::Opacity(0.5)]);
        assert!(
            o.3 == 128 || o.3 == 127,
            "opacity(.5) must halve alpha; got {o:?}"
        );
        // sepia(1) of white is the row sums of the sepia matrix, clamped.
        let sep = one_px(
            Rgba {
                r: 255,
                g: 255,
                b: 255,
                a: 255,
            },
            &[FilterOp::Sepia(1.0)],
        );
        // Rows 1 and 2 sum past 1.0 and clamp; row 3 sums to 0.937 → 239.
        assert_eq!(
            (sep.0, sep.1, sep.2),
            (255, 255, 239),
            "sepia(1) of white must be the matrix row sums; got {sep:?}"
        );
    }

    /// **Order is not decorative.** The list is a pipeline, so a renderer that sorted or
    /// commuted it would silently produce a different picture on real pages.
    #[test]
    fn filter_order_changes_the_result() {
        let red = Rgba {
            r: 255,
            g: 0,
            b: 0,
            a: 255,
        };
        let a = one_px(red, &[FilterOp::Grayscale(1.0), FilterOp::Sepia(1.0)]);
        let b = one_px(red, &[FilterOp::Sepia(1.0), FilterOp::Grayscale(1.0)]);
        assert_ne!(
            a, b,
            "grayscale→sepia and sepia→grayscale must differ; both gave {a:?}"
        );
    }

    /// A blur must **soften an edge outward while leaving the interior alone**, and it must not
    /// dim what it blurs. Those are three separate failure modes and this pins all three.
    ///
    /// The subject is a filled block, not a single lit pixel — deliberately. An 8-bit surface
    /// *cannot* represent a delta spread over a few hundred pixels, so a single-pixel test would
    /// have measured the format's rounding rather than the blur, and no correct implementation
    /// could pass it. (It was written that way first, and failed for exactly that reason.)
    #[test]
    fn blur_softens_the_edge_without_dimming_the_interior() {
        const W: usize = 64;
        let mut px = Pixmap::new(W as u32, W as u32).unwrap();
        for y in 20..44 {
            for x in 20..44 {
                px.pixels_mut()[y * W + x] =
                    tiny_skia::ColorU8::from_rgba(255, 255, 255, 255).premultiply();
            }
        }
        let before: u32 = px.pixels().iter().map(|p| p.alpha() as u32).sum();
        let outside_before = px.pixels()[32 * W + 46].alpha();
        blur(&mut px, 3.0);
        let centre = px.pixels()[32 * W + 32].alpha();
        let outside = px.pixels()[32 * W + 46].alpha();
        let edge = px.pixels()[32 * W + 44].alpha();
        assert_eq!(outside_before, 0, "the probe point must start transparent");
        assert!(
            centre >= 250,
            "the interior of a blurred block must stay opaque; got {centre}"
        );
        assert!(
            outside > 0,
            "ink must spread 2px past the block's edge; got {outside}"
        );
        assert!(
            edge > 0 && edge < 250,
            "the edge itself must become a ramp, not a step; got {edge}"
        );
        let after: u32 = px.pixels().iter().map(|p| p.alpha() as u32).sum();
        // Blur conserves energy; rounded integer passes lose a fraction of a percent, not a fifth.
        assert!(
            after * 100 > before * 95 && after < before * 2,
            "blur must conserve alpha: {before} -> {after}"
        );
    }

    /// `blur(0)` and an empty list are both the identity — a no-op filter must not cost fidelity.
    #[test]
    fn zero_blur_is_the_identity() {
        let mut px = Pixmap::new(8, 8).unwrap();
        px.pixels_mut()[10] = tiny_skia::ColorU8::from_rgba(10, 20, 30, 255).premultiply();
        let before = px.data().to_vec();
        apply_filters(&mut px, &[FilterOp::Blur(0.0)]);
        assert_eq!(px.data(), &before[..], "blur(0) must change nothing");
    }

    /// `drop-shadow` casts the ALPHA SILHOUETTE, offset — so a pixel that was transparent at the
    /// offset position becomes shadow-coloured, and the source pixel survives on top.
    #[test]
    fn drop_shadow_casts_the_alpha_silhouette_offset() {
        let mut px = Pixmap::new(16, 16).unwrap();
        px.pixels_mut()[4 * 16 + 4] = tiny_skia::ColorU8::from_rgba(255, 0, 0, 255).premultiply();
        apply_filters(
            &mut px,
            &[FilterOp::DropShadow {
                dx: 3.0,
                dy: 0.0,
                blur: 0.0,
                color: Rgba {
                    r: 0,
                    g: 0,
                    b: 255,
                    a: 255,
                },
            }],
        );
        let src = px.pixels()[4 * 16 + 4].demultiply();
        let shadow = px.pixels()[4 * 16 + 7].demultiply();
        assert_eq!(
            (src.red(), src.green(), src.blue()),
            (255, 0, 0),
            "the source pixel must still be on top of its own shadow"
        );
        assert_eq!(
            (shadow.red(), shadow.green(), shadow.blue(), shadow.alpha()),
            (0, 0, 255, 255),
            "a blue drop-shadow offset 3px must land at +3px"
        );
    }
}
