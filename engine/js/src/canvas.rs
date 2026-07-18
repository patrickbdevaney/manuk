//! **`<canvas>` 2D — a real rasterizer, not a stub that accepts every call and draws nothing.**
//!
//! Until tick 66 `getContext('2d')` returned a context object whose every drawing operation was a
//! `noop`. That was a *deliberate* trade and an honest one for its time — the alternative was
//! `getContext` being `undefined`, which made `ctx.fillRect(...)` on the next line a `TypeError` that
//! took the whole bundle down with it. **A blank chart on a working page beats an exception**, and it
//! even warned in the console.
//!
//! But it is the worst *shape* of failure that still counts as working: a page feature-detects canvas,
//! is told **yes**, draws its chart, and nothing appears. `G_CAPABILITY` measured it exactly — fill the
//! canvas red, read the pixel back, get `0,0,0,0`.
//!
//! So it rasterizes now. The pixels are real, and `getImageData` can prove it.
//!
//! ## How it reaches the screen
//!
//! With no new machinery at all, which is why this is one tick and not five. The painter already scales
//! a [`manuk_paint::DecodedImage`] into a replaced element's content box, keyed by `NodeId` — that is how
//! `<img>` works. **A canvas is simply an image the page draws into.** So each `<canvas>` owns a
//! `tiny_skia::Pixmap`, the JS context draws into it, and `Page` drains the finished pixmaps into the
//! very same image map an `<img>` would land in.
//!
//! ## Where the state lives, and why it is split
//!
//! The *state machine* — `fillStyle`, `strokeStyle`, `lineWidth`, `globalAlpha`, the transform stack,
//! the current path — stays in JavaScript, because that is where it is cheap and where the spec's
//! ergonomics live (colour strings, `save()`/`restore()`, method chaining). Only the **rasterization**
//! crosses into Rust: a resolved colour, a resolved transform, and a flat array of path commands. One
//! native call per drawing operation, not one per `lineTo`.

use std::cell::RefCell;
use std::collections::HashMap;

use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};

/// One canvas's backing store, keyed by the `<canvas>` element's `NodeId`.
thread_local! {
    static CANVASES: RefCell<HashMap<u64, Pixmap>> = RefCell::new(HashMap::new());
    /// Which canvases have been drawn into since the host last collected them. A canvas nobody touched
    /// must not be re-uploaded on every script round.
    static DIRTY: RefCell<Vec<u64>> = const { RefCell::new(Vec::new()) };
}

/// Create or resize a canvas's backing store.
///
/// Per spec, setting `width`/`height` **clears** the canvas — it is the idiomatic way to erase one, and
/// a chart library that resizes and expects a clean surface gets one.
pub fn init(node: u64, w: u32, h: u32) {
    // A canvas can legally be enormous; refuse the absurd rather than OOM the browser on a typo.
    // 8192² × 4 bytes is 256MB, which is already far past anything a page has a right to expect.
    let (w, h) = (w.clamp(1, 8192), h.clamp(1, 8192));
    CANVASES.with(|c| {
        let mut m = c.borrow_mut();
        match m.get(&node) {
            Some(p) if p.width() == w && p.height() == h => {
                // Same size: still a clear, because that is what the spec says an assignment does.
                if let Some(p) = m.get_mut(&node) {
                    p.fill(tiny_skia::Color::TRANSPARENT);
                }
            }
            _ => {
                if let Some(p) = Pixmap::new(w, h) {
                    m.insert(node, p);
                }
            }
        }
    });
    mark_dirty(node);
}

fn mark_dirty(node: u64) {
    DIRTY.with(|d| {
        let mut d = d.borrow_mut();
        if !d.contains(&node) {
            d.push(node);
        }
    });
}

fn paint_for(r: u8, g: u8, b: u8, a: f32) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(r, g, b, (a.clamp(0.0, 1.0) * 255.0).round() as u8);
    p.anti_alias = true;
    p
}

fn xform(m: &[f32]) -> Transform {
    // The canvas 2D matrix is [a b c d e f]; tiny-skia's is (sx, ky, kx, sy, tx, ty) in the same order.
    if m.len() == 6 {
        Transform::from_row(m[0], m[1], m[2], m[3], m[4], m[5])
    } else {
        Transform::identity()
    }
}

/// `fillRect` / `strokeRect`. `stroke_width <= 0` means fill.
pub fn rect(node: u64, x: f32, y: f32, w: f32, h: f32, col: (u8, u8, u8, f32), sw: f32, m: &[f32]) {
    CANVASES.with(|c| {
        if let Some(px) = c.borrow_mut().get_mut(&node) {
            // A zero-or-negative-extent rect is a no-op, not an error — and `Rect::from_xywh` returns
            // `None` for it, which would otherwise silently swallow the *whole* call including valid ones.
            let Some(r) = Rect::from_xywh(x, y, w, h) else {
                return;
            };
            let paint = paint_for(col.0, col.1, col.2, col.3);
            let t = xform(m);
            if sw > 0.0 {
                let mut pb = PathBuilder::new();
                pb.push_rect(r);
                if let Some(path) = pb.finish() {
                    let mut stroke = Stroke::default();
                    stroke.width = sw;
                    px.stroke_path(&path, &paint, &stroke, t, None);
                }
            } else if t.is_identity() {
                px.fill_rect(r, &paint, t, None);
            } else {
                // `fill_rect` ignores a non-identity transform's skew in some builds; go through a path,
                // which always honours it.
                let mut pb = PathBuilder::new();
                pb.push_rect(r);
                if let Some(path) = pb.finish() {
                    px.fill_path(&path, &paint, FillRule::Winding, t, None);
                }
            }
        }
    });
    mark_dirty(node);
}

/// `clearRect` — erase to transparent. Not a white fill: a canvas is transparent, and a page that
/// composites one over a background would see the difference immediately.
pub fn clear_rect(node: u64, x: f32, y: f32, w: f32, h: f32, m: &[f32]) {
    CANVASES.with(|c| {
        if let Some(px) = c.borrow_mut().get_mut(&node) {
            let Some(r) = Rect::from_xywh(x, y, w, h) else {
                return;
            };
            let mut paint = Paint::default();
            paint.set_color_rgba8(0, 0, 0, 0);
            paint.blend_mode = tiny_skia::BlendMode::Clear;
            let mut pb = PathBuilder::new();
            pb.push_rect(r);
            if let Some(path) = pb.finish() {
                px.fill_path(&path, &paint, FillRule::Winding, xform(m), None);
            }
        }
    });
    mark_dirty(node);
}

/// A path, encoded as a flat command stream from JS.
///
/// The encoding is deliberately dumb — `[op, args…]` repeated — because the alternative is a native call
/// per `lineTo`, and a chart with 10,000 points would then pay 10,000 FFI crossings.
///
/// ops: `0 moveTo x y` · `1 lineTo x y` · `2 quadTo cx cy x y` · `3 cubicTo c1x c1y c2x c2y x y` ·
/// `4 close` · `5 rect x y w h`
pub fn path(node: u64, cmds: &[f32], fill: bool, col: (u8, u8, u8, f32), sw: f32, m: &[f32]) {
    let mut pb = PathBuilder::new();
    let mut i = 0usize;
    while i < cmds.len() {
        let op = cmds[i] as i32;
        i += 1;
        // Bounds-check EVERY read. A truncated command stream (a JS bug, or a hostile page) must not
        // index off the end of the slice — that is a panic, and a panic inside a JSNative is `nounwind`,
        // which means it aborts the whole browser rather than throwing.
        let need = match op {
            0 | 1 => 2,
            2 => 4,
            3 => 6,
            4 => 0,
            5 => 4,
            _ => return,
        };
        if i + need > cmds.len() {
            return;
        }
        match op {
            0 => pb.move_to(cmds[i], cmds[i + 1]),
            1 => pb.line_to(cmds[i], cmds[i + 1]),
            2 => pb.quad_to(cmds[i], cmds[i + 1], cmds[i + 2], cmds[i + 3]),
            3 => pb.cubic_to(
                cmds[i],
                cmds[i + 1],
                cmds[i + 2],
                cmds[i + 3],
                cmds[i + 4],
                cmds[i + 5],
            ),
            4 => pb.close(),
            5 => {
                if let Some(r) = Rect::from_xywh(cmds[i], cmds[i + 1], cmds[i + 2], cmds[i + 3]) {
                    pb.push_rect(r);
                }
            }
            _ => return,
        }
        i += need;
    }
    let Some(p) = pb.finish() else {
        return; // an empty or degenerate path is a no-op, exactly as in a real browser
    };

    CANVASES.with(|c| {
        if let Some(px) = c.borrow_mut().get_mut(&node) {
            let paint = paint_for(col.0, col.1, col.2, col.3);
            let t = xform(m);
            if fill {
                px.fill_path(&p, &paint, FillRule::Winding, t, None);
            } else {
                let mut stroke = Stroke::default();
                stroke.width = if sw > 0.0 { sw } else { 1.0 };
                px.stroke_path(&p, &paint, &stroke, t, None);
            }
        }
    });
    mark_dirty(node);
}

/// `getImageData` — **non-premultiplied RGBA8**, which is what the spec hands JavaScript.
///
/// tiny-skia stores premultiplied. Handing back the raw bytes would give a page subtly wrong colours
/// wherever alpha < 255, and it would look like a rounding bug rather than a colour-space bug.
pub fn get_image_data(node: u64, x: i32, y: i32, w: u32, h: u32) -> Vec<u8> {
    let mut out = vec![0u8; (w as usize) * (h as usize) * 4];
    CANVASES.with(|c| {
        if let Some(px) = c.borrow().get(&node) {
            let (pw, ph) = (px.width() as i32, px.height() as i32);
            let src = px.pixels();
            for row in 0..h as i32 {
                for col in 0..w as i32 {
                    let (sx, sy) = (x + col, y + row);
                    if sx < 0 || sy < 0 || sx >= pw || sy >= ph {
                        continue; // outside the surface reads as transparent black, per spec
                    }
                    let p = src[(sy * pw + sx) as usize];
                    let o = ((row * w as i32 + col) * 4) as usize;
                    out[o] = p.demultiply().red();
                    out[o + 1] = p.demultiply().green();
                    out[o + 2] = p.demultiply().blue();
                    out[o + 3] = p.alpha();
                }
            }
        }
    });
    out
}

/// PNG bytes for `toDataURL`.
pub fn to_png(node: u64) -> Option<Vec<u8>> {
    CANVASES.with(|c| c.borrow().get(&node).and_then(|p| p.encode_png().ok()))
}

/// Hand the host every canvas that has been drawn into since the last call, as non-premultiplied RGBA8
/// — the exact shape [`manuk_paint::DecodedImage`] wants, so `Page` can drop them straight into the map
/// an `<img>` lands in and the painter needs no idea a canvas exists.
pub fn take_dirty() -> Vec<(u64, u32, u32, Vec<u8>)> {
    let ids: Vec<u64> = DIRTY.with(|d| std::mem::take(&mut *d.borrow_mut()));
    CANVASES.with(|c| {
        let m = c.borrow();
        ids.iter()
            .filter_map(|id| {
                let px = m.get(id)?;
                let (w, h) = (px.width(), px.height());
                let mut rgba = Vec::with_capacity((w * h * 4) as usize);
                for p in px.pixels() {
                    let d = p.demultiply();
                    rgba.extend_from_slice(&[d.red(), d.green(), d.blue(), p.alpha()]);
                }
                Some((*id, w, h, rgba))
            })
            .collect()
    })
}

/// Forget every canvas. Called when a document goes away — a `Pixmap` is easily megabytes, and one per
/// tab per navigation is a leak with a straight face.
pub fn reset() {
    CANVASES.with(|c| c.borrow_mut().clear());
    DIRTY.with(|d| d.borrow_mut().clear());
}

// ─────────────────────────────────────────────────────────────────────────────────────────────
// TEXT — `fillText` / `strokeText` / `measureText`
//
// Every drawing op above produces geometry. Text is the one that produces *glyphs*, and the engine
// already has a shaper-and-rasterizer for those: `engine/text`, swash-backed, with the bidi
// reordering, per-glyph font fallback and complex-script shaping that ticks 214/215 put there. So
// this is a WIRING job, not a new renderer. The alternative — a second text stack living inside the
// canvas — would drift from the DOM's within one tick and would have to re-learn every one of those
// lessons separately.
//
// What that buys, concretely: a canvas draws Arabic joined, Devanagari with conjuncts, CJK and emoji
// through the fallback chain, and it hits the same glyph raster cache as the paragraph next to it.
// ─────────────────────────────────────────────────────────────────────────────────────────────

thread_local! {
    /// One font context per thread, built on first use.
    ///
    /// Lazily, because constructing it scans the system font database — a page that never touches a
    /// canvas (or touches one but only draws rects) must not pay for that. `FontContext` is `Rc`-based
    /// and single-threaded by design, which is exactly the shape a `thread_local` wants, and it
    /// matches how `CANVASES` above already stores per-thread canvas state.
    static FONTS: RefCell<Option<manuk_text::FontContext>> = const { RefCell::new(None) };
}

/// Resolve a parsed canvas font into the shaper's key.
///
/// The JS side parses the `ctx.font` CSS shorthand (that is where the string ergonomics belong) and
/// hands down a resolved size, a comma-split family list, and the two style bits.
fn font_key(
    fonts: &manuk_text::FontContext,
    families: &str,
    bold: bool,
    italic: bool,
) -> manuk_text::FontKey {
    let names: Vec<String> = families
        .split(',')
        .map(|f| f.trim().trim_matches(['"', '\'']).to_string())
        .filter(|f| !f.is_empty())
        .collect();
    manuk_text::FontKey {
        family: fonts.resolve_family(&names),
        bold,
        italic,
    }
}

fn with_fonts<R>(f: impl FnOnce(&manuk_text::FontContext) -> R) -> R {
    FONTS.with(|c| {
        let mut slot = c.borrow_mut();
        f(slot.get_or_insert_with(manuk_text::FontContext::new))
    })
}

/// The transform's effect on text, reduced to a scale and a mapped origin.
///
/// **This is the documented limit of canvas text under a transform.** Glyphs are rasterized from
/// outlines at a size, so a uniform scale is exact — `ctx.scale(2,2)` really does render at twice the
/// size rather than magnifying a bitmap. Rotation and skew are NOT applied to the glyph raster: the
/// text lands at the correctly transformed origin, at the correctly scaled size, but upright. That is
/// wrong for rotated axis labels and right for everything else, and it is a bounded, honest gap
/// rather than a silent one (see the module residue note in `docs/wiki/`).
fn text_transform(m: &[f32], x: f32, y: f32) -> (f32, f32, f32) {
    if m.len() != 6 {
        return (1.0, x, y);
    }
    let sx = (m[0] * m[0] + m[1] * m[1]).sqrt();
    let sy = (m[2] * m[2] + m[3] * m[3]).sqrt();
    let scale = ((sx + sy) * 0.5).clamp(0.01, 100.0);
    (
        scale,
        m[0] * x + m[2] * y + m[4],
        m[1] * x + m[3] * y + m[5],
    )
}

/// Source-over composite one glyph into the canvas.
///
/// **Not shareable with `manuk_paint`'s blit, and the difference is the whole point.** That one
/// writes `alpha = 255` because it composites onto an opaque page background. A canvas is
/// transparent-backed — that is what makes it compose over the page — so alpha has to *accumulate*
/// (`a_out = a_src + a_dst(1 - a_src)`), in premultiplied space, which is what `Pixmap` stores.
/// Reusing the opaque blit here would fill every glyph's bounding box with opaque black fringing.
fn blit_glyph(
    px: &mut Pixmap,
    bmp: &manuk_text::GlyphBitmap,
    left: i32,
    top: i32,
    col: (u8, u8, u8, f32),
) {
    let (pw, ph) = (px.width() as i32, px.height() as i32);
    let alpha = col.3.clamp(0.0, 1.0);
    if alpha <= 0.0 {
        return;
    }
    let data = px.data_mut();
    for row in 0..bmp.height as i32 {
        let py = top + row;
        if py < 0 || py >= ph {
            continue;
        }
        for c in 0..bmp.width as i32 {
            let pxx = left + c;
            if pxx < 0 || pxx >= pw {
                continue;
            }
            let i = (row as usize) * bmp.width as usize + c as usize;
            // A color glyph (emoji) carries straight-alpha RGBA; a normal one carries 8-bit coverage
            // to be tinted with the fill colour.
            let (sr, sg, sb, sa) = if bmp.is_color {
                let p = i * 4;
                if p + 3 >= bmp.coverage.len() {
                    continue;
                }
                (
                    bmp.coverage[p],
                    bmp.coverage[p + 1],
                    bmp.coverage[p + 2],
                    (bmp.coverage[p + 3] as f32 / 255.0) * alpha,
                )
            } else {
                match bmp.coverage.get(i) {
                    Some(0) | None => continue,
                    Some(cov) => (col.0, col.1, col.2, (*cov as f32 / 255.0) * alpha),
                }
            };
            if sa <= 0.0 {
                continue;
            }
            let idx = ((py * pw + pxx) as usize) * 4;
            let inv = 1.0 - sa;
            for (k, s) in [sr, sg, sb].into_iter().enumerate() {
                let src = s as f32 * sa; // premultiply
                data[idx + k] = (src + data[idx + k] as f32 * inv).round().clamp(0.0, 255.0) as u8;
            }
            let da = data[idx + 3] as f32 / 255.0;
            data[idx + 3] = ((sa + da * inv) * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
}

/// `fillText` / `strokeText`.
///
/// `max_width <= 0` means the argument was omitted.
#[allow(clippy::too_many_arguments)]
pub fn fill_text(
    node: u64,
    text: &str,
    x: f32,
    y: f32,
    col: (u8, u8, u8, f32),
    size: f32,
    families: &str,
    bold: bool,
    italic: bool,
    rtl: bool,
    max_width: f32,
    m: &[f32],
) {
    if text.is_empty() || !size.is_finite() || !x.is_finite() || !y.is_finite() {
        return;
    }
    let (scale, ox, oy) = text_transform(m, x, y);
    // The same guard `manuk_paint::draw_text` uses: a zero-or-absurd size is a page bug, and the
    // rasterizer must not be asked to allocate for it.
    let mut render_size = (size * scale).clamp(0.0, 1024.0);
    if render_size < 0.5 {
        return;
    }

    with_fonts(|fonts| {
        let key = font_key(fonts, families, bold, italic);
        let mut run = fonts.shape_bidi(text, key, render_size, rtl);

        // `maxWidth` — the spec condenses the text horizontally. We re-shape smaller instead, which
        // is an approximation (it loses height as well as width) but keeps the label INSIDE the box
        // the author reserved for it. Overflowing would be the worse failure: chart axis labels pass
        // maxWidth precisely because they know the column is narrow.
        if max_width > 0.0 && run.width > max_width && run.width > 0.0 {
            render_size = (render_size * (max_width / run.width)).max(0.5);
            run = fonts.shape_bidi(text, key, render_size, rtl);
        }

        CANVASES.with(|c| {
            if let Some(px) = c.borrow_mut().get_mut(&node) {
                for g in &run.glyphs {
                    let pen = ox + g.x;
                    if let Some(bmp) = fonts.rasterize(g.glyph_id, g.face, render_size, pen) {
                        blit_glyph(
                            px,
                            &bmp,
                            pen.floor() as i32 + bmp.left,
                            oy.round() as i32 - bmp.top,
                            col,
                        );
                    }
                }
            }
        });
    });
    mark_dirty(node);
}

/// `measureText` → `(width, font_ascent, font_descent)`.
///
/// Real shaped advances, not a character count. The old stub returned `len * 7`, which is not merely
/// imprecise: every layout a page derives from it — centring, wrapping, column fitting, hit-testing a
/// terminal cell — is computed from a number that has no relationship to the glyphs drawn.
pub fn measure_text(
    text: &str,
    size: f32,
    families: &str,
    bold: bool,
    italic: bool,
) -> (f32, f32, f32) {
    if !size.is_finite() || size < 0.5 {
        return (0.0, 0.0, 0.0);
    }
    with_fonts(|fonts| {
        let key = font_key(fonts, families, bold, italic);
        let lm = fonts.line_metrics(key, size);
        let w = if text.is_empty() {
            0.0
        } else {
            fonts.measure(text, key, size)
        };
        (w, lm.ascent, lm.descent.abs())
    })
}
