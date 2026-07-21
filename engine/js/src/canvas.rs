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

use tiny_skia::{
    Color, FillRule, GradientStop, LinearGradient, Paint, PathBuilder, Pixmap, Point,
    RadialGradient, Rect, Shader, SpreadMode, Stroke, SweepGradient, Transform,
};

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

/// Decode the flat `[op, args…]` command stream from JS into a tiny-skia `Path`.
///
/// The encoding is deliberately dumb — `[op, args…]` repeated — because the alternative is a native call
/// per `lineTo`, and a chart with 10,000 points would then pay 10,000 FFI crossings. ops:
/// `0 moveTo x y` · `1 lineTo x y` · `2 quadTo cx cy x y` · `3 cubicTo c1x c1y c2x c2y x y` ·
/// `4 close` · `5 rect x y w h`.
///
/// Every slice read is bounds-checked: a truncated stream (a JS bug or a hostile page) must never index
/// off the end, which is a panic, and a panic inside a JSNative is `nounwind` — it aborts the whole
/// browser rather than throwing.
fn build_path(cmds: &[f32]) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    let mut i = 0usize;
    while i < cmds.len() {
        let op = cmds[i] as i32;
        i += 1;
        let need = match op {
            0 | 1 => 2,
            2 => 4,
            3 => 6,
            4 => 0,
            5 => 4,
            _ => return None,
        };
        if i + need > cmds.len() {
            return None;
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
            _ => return None,
        }
        i += need;
    }
    pb.finish()
}

pub fn path(node: u64, cmds: &[f32], fill: bool, col: (u8, u8, u8, f32), sw: f32, m: &[f32]) {
    let Some(p) = build_path(cmds) else {
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

/// Build a tiny-skia gradient `Shader` from the flat spec JS hands across:
/// `[kind, x0, y0, r0, x1, y1, r1, off, r, g, b, a, off, r, g, b, a, …]`
/// where `kind` 0 = linear (uses the two points), 1 = radial (focal `(x0,y0)` → outer circle
/// `(x1,y1)` radius `r1`). Stops are 5-tuples: offset 0..1, then r/g/b 0..255 and a 0..1.
///
/// The gradient is authored in canvas USER space, the same space the path coordinates live in, so it
/// is built at IDENTITY: `fill_path`/`stroke_path` apply the context transform `m` to the shader (see
/// `paint.shader.transform(transform)` in tiny-skia's painter) exactly as they do to the path, keeping
/// the gradient locked to the shape under `translate`/`scale`/`rotate`. Giving it `m` here too would
/// transform it twice.
fn gradient_shader(grad: &[f32]) -> Option<Shader<'static>> {
    if grad.len() < 7 {
        return None;
    }
    let kind = grad[0] as i32;
    let (x0, y0, r0, x1, y1, r1) = (grad[1], grad[2], grad[3], grad[4], grad[5], grad[6]);
    let mut stops: Vec<GradientStop> = Vec::new();
    let mut i = 7usize;
    while i + 5 <= grad.len() {
        let a = (grad[i + 4].clamp(0.0, 1.0) * 255.0).round() as u8;
        stops.push(GradientStop::new(
            grad[i],
            Color::from_rgba8(
                grad[i + 1].clamp(0.0, 255.0) as u8,
                grad[i + 2].clamp(0.0, 255.0) as u8,
                grad[i + 3].clamp(0.0, 255.0) as u8,
                a,
            ),
        ));
        i += 5;
    }
    if stops.is_empty() {
        return None;
    }
    // `Pad` clamps to the first/last stop past the ends — the CSS/canvas default. `new` returns a
    // solid-colour shader for a single stop and `None` for a degenerate geometry, both of which are
    // the honest outcomes a real 2D context produces.
    if kind == 2 {
        // Conic (sweep): `createConicGradient(startAngle, cx, cy)` — centre `(x0,y0)`, angle swept a full
        // 360° from `startAngle`. The spec's angle is radians, clockwise from the positive x-axis; skia's
        // is degrees, same origin and direction, so the conversion is the whole mapping.
        let start_deg = r0 * 180.0 / std::f32::consts::PI;
        SweepGradient::new(
            Point::from_xy(x0, y0),
            start_deg,
            start_deg + 360.0,
            stops,
            SpreadMode::Pad,
            Transform::identity(),
        )
    } else if kind == 1 {
        // Two-point conical: canvas's inner circle (x0,y0,r0) → outer circle (x1,y1,r1). The concentric
        // r0=0 case (the overwhelmingly common one) is an ordinary radial gradient.
        RadialGradient::new(
            Point::from_xy(x0, y0),
            r0,
            Point::from_xy(x1, y1),
            r1,
            stops,
            SpreadMode::Pad,
            Transform::identity(),
        )
    } else {
        LinearGradient::new(
            Point::from_xy(x0, y0),
            Point::from_xy(x1, y1),
            stops,
            SpreadMode::Pad,
            Transform::identity(),
        )
    }
}

/// `fill`/`stroke` a path with a gradient paint instead of a flat colour — the same command stream as
/// [`path`], but the fill is a real tiny-skia gradient shader. Charts (Chart.js area/bar fills), button
/// glosses and progress bars are drawn this way, and until now the context flattened them to their last
/// stop's colour.
pub fn path_gradient(node: u64, cmds: &[f32], fill: bool, grad: &[f32], sw: f32, m: &[f32]) {
    let Some(p) = build_path(cmds) else {
        return;
    };
    let Some(shader) = gradient_shader(grad) else {
        return;
    };
    CANVASES.with(|c| {
        if let Some(px) = c.borrow_mut().get_mut(&node) {
            let mut paint = Paint {
                anti_alias: true,
                ..Default::default()
            };
            paint.shader = shader;
            // `fill_path` applies `t` to BOTH the path and the (identity-built) shader, so the gradient
            // stays locked to the user-space geometry under the context transform.
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

/// `fill`/`stroke` a path with a **pattern** paint — a source image tiled across the shape. This is how
/// hatch fills, textured backgrounds and repeating sprites are drawn (`ctx.fillStyle =
/// ctx.createPattern(img, 'repeat')`). The source pixels come from the SAME node-keyed registry
/// `drawImage` reads (a `<canvas>` backing store or a decoded `<img>`), so there is no new decode path.
///
/// `repeat` 0 = `repeat` · 1 = `repeat-x` · 2 = `repeat-y` · 3 = `no-repeat`. tiny-skia's `SpreadMode`
/// is not per-axis, so the tiling modes all use `Repeat` and `no-repeat` uses `Pad` (its edge clamps
/// rather than going transparent — the honest limit). The pattern is anchored at the canvas origin at
/// the image's natural size; `fill_path` applies the context transform to both path and pattern.
pub fn path_pattern(
    node: u64,
    cmds: &[f32],
    fill: bool,
    src: u64,
    repeat: i32,
    alpha: f32,
    sw: f32,
    m: &[f32],
) {
    let Some(p) = build_path(cmds) else {
        return;
    };
    // Copy the source pixmap out before borrowing CANVASES mutably (a canvas can pattern-fill itself).
    let src_px: Option<Pixmap> = CANVASES
        .with(|c| c.borrow().get(&src).cloned())
        .or_else(|| SOURCES.with(|s| s.borrow().get(&src).cloned()));
    let Some(src_px) = src_px else {
        return; // the source image has not decoded yet — draw nothing, as a real browser does
    };
    let spread = if repeat == 3 {
        tiny_skia::SpreadMode::Pad
    } else {
        tiny_skia::SpreadMode::Repeat
    };
    let mut paint = Paint {
        anti_alias: true,
        ..Default::default()
    };
    paint.shader = tiny_skia::Pattern::new(
        src_px.as_ref(),
        spread,
        tiny_skia::FilterQuality::Bilinear,
        alpha.clamp(0.0, 1.0),
        Transform::identity(),
    );
    CANVASES.with(|c| {
        if let Some(px) = c.borrow_mut().get_mut(&node) {
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
    SOURCES.with(|s| s.borrow_mut().clear());
}

// ─────────────────────────────────────────────────────────────────────────────────────────────
// IMAGE SOURCES — `drawImage`
//
// Every op above draws something the *script* described: a colour, a path, a string. `drawImage` is the
// first that draws something the *host* owns — the decoded pixels of an `<img>` that the network fetched
// and `engine/paint` decoded. So it needs plumbing in the direction canvas has never needed before.
//
// [`take_dirty`] is the js→host edge: finished canvases go out to the image map. This is the exact
// MIRROR, host→js: decoded images come in, keyed by the same `NodeId`. The two maps stay separate on
// purpose. `Page` drops canvases into `self.images` alongside `<img>`s, so a single shared map would
// feed a canvas's own output back in as a source — and `drawImage(myCanvas, …)` would then read a stale
// copy of itself from one script round ago instead of its live pixels. Looking CANVASES up first is what
// makes canvas→canvas compositing (the standard double-buffering idiom) read the live surface.
thread_local! {
    /// Decoded `<img>` pixels the host has published, keyed by the image element's `NodeId`.
    /// Stored premultiplied, because that is what tiny-skia composites and converting once at publish
    /// beats converting on every frame of an animation loop.
    static SOURCES: RefCell<HashMap<u64, Pixmap>> = RefCell::new(HashMap::new());
}

/// Publish a decoded image so scripts can `drawImage` it. Idempotent: re-publishing the same node at the
/// same size is a no-op, because the host has no cheap way to know whether we already have it and a
/// per-script-round re-upload of every image on the page would be a performance bug wearing a feature's
/// clothes.
///
/// `rgba` is **non-premultiplied** — the shape [`manuk_paint::DecodedImage`] holds.
pub fn publish_source(node: u64, w: u32, h: u32, rgba: &[u8]) {
    if w == 0 || h == 0 || rgba.len() < (w as usize) * (h as usize) * 4 {
        return;
    }
    SOURCES.with(|s| {
        let mut m = s.borrow_mut();
        if let Some(p) = m.get(&node) {
            if p.width() == w && p.height() == h {
                return;
            }
        }
        let Some(mut px) = Pixmap::new(w, h) else {
            return;
        };
        for (dst, src) in px.pixels_mut().iter_mut().zip(rgba.chunks_exact(4)) {
            // `from_rgba8` premultiplies. An image with alpha drawn without this is the classic
            // too-bright halo, and it looks like a blending bug rather than a colour-space one.
            *dst = tiny_skia::ColorU8::from_rgba(src[0], src[1], src[2], src[3]).premultiply();
        }
        m.insert(node, px);
    });
}

/// The intrinsic size of a publishable source, for the 3-argument `drawImage(img, dx, dy)` overload,
/// which takes its destination extent from the image itself. `None` means "not a source we know", which
/// is what lets the JS shim skip a call that would draw nothing.
pub fn source_size(node: u64) -> Option<(u32, u32)> {
    CANVASES
        .with(|c| c.borrow().get(&node).map(|p| (p.width(), p.height())))
        .or_else(|| SOURCES.with(|s| s.borrow().get(&node).map(|p| (p.width(), p.height()))))
}

/// `drawImage(src, sx, sy, sw, sh, dx, dy, dw, dh)` — all three overloads normalised to nine arguments
/// in JS, so exactly one shape crosses the FFI.
///
/// Implemented as a **pattern fill of the destination rect** rather than tiny-skia's `draw_pixmap`,
/// because `draw_pixmap` takes an integer offset and cannot express the source-rect crop, the
/// non-uniform dst/src scale, and the context transform at once. A pattern carries its own matrix, so
/// all three compose: map the source crop onto the destination rect, then let the ctx transform apply to
/// the rect itself. That is what makes a rotated, cropped, scaled sprite land correctly in one call.
#[allow(clippy::too_many_arguments)]
pub fn draw_image(
    node: u64,
    src: u64,
    sx: f32,
    sy: f32,
    sw: f32,
    sh: f32,
    dx: f32,
    dy: f32,
    dw: f32,
    dh: f32,
    alpha: f32,
    m: &[f32],
) {
    // A non-finite argument draws nothing per spec, and would otherwise produce a NaN transform that
    // silently blanks the fill.
    if ![sx, sy, sw, sh, dx, dy, dw, dh]
        .iter()
        .all(|v| v.is_finite())
    {
        return;
    }
    // NEGATIVE EXTENTS MEAN TWO DIFFERENT THINGS, and conflating them is the classic drawImage bug.
    // On the SOURCE rect a negative extent just selects the same region from the other corner. On the
    // DESTINATION rect it MIRRORS the image — `drawImage(img, x+w, y, -w, h)` is how a sprite sheet
    // draws a character facing left, and dropping it would silently render every sprite facing right.
    let (sx, sw) = if sw < 0.0 { (sx + sw, -sw) } else { (sx, sw) };
    let (sy, sh) = if sh < 0.0 { (sy + sh, -sh) } else { (sy, sh) };
    let (flip_x, flip_y) = (dw < 0.0, dh < 0.0);
    let (dx, dw) = if flip_x { (dx + dw, -dw) } else { (dx, dw) };
    let (dy, dh) = if flip_y { (dy + dh, -dh) } else { (dy, dh) };
    // A zero extent on either rect draws nothing. Per spec this is a silent no-op, not an exception —
    // and it must be caught here because it would otherwise divide by zero below.
    if sw <= 0.0 || sh <= 0.0 || dw <= 0.0 || dh <= 0.0 {
        return;
    }
    let Some(dest) = Rect::from_xywh(dx, dy, dw, dh) else {
        return;
    };

    // Copy the source out before touching CANVASES mutably: `drawImage(c, …)` with c as BOTH source and
    // destination is legal and common (scroll-by-blit), and holding a borrow of the map while mutating an
    // entry in it would panic at runtime.
    let src_px: Option<Pixmap> = CANVASES
        .with(|c| c.borrow().get(&src).cloned())
        .or_else(|| SOURCES.with(|s| s.borrow().get(&src).cloned()));
    let Some(src_px) = src_px else {
        return; // an image that has not decoded yet draws nothing, exactly as in a real browser
    };

    // Source crop → destination rect. Read right-to-left: shift the crop's top-left to the origin, scale
    // it to the destination extent, then move it to the destination position. A mirrored axis scales
    // negatively and anchors at the *far* edge, which is what turns the flip into a reflection about the
    // destination rect rather than a translation off it.
    let pat_t = Transform::from_translate(
        if flip_x { dx + dw } else { dx },
        if flip_y { dy + dh } else { dy },
    )
    .pre_scale(
        if flip_x { -(dw / sw) } else { dw / sw },
        if flip_y { -(dh / sh) } else { dh / sh },
    )
    .pre_translate(-sx, -sy);
    // **The context transform is applied by `fill_path`, to the pattern as well as the path** — tiny-skia
    // concatenates the fill transform onto the shader's own matrix, so `pat_t` is expressed purely in the
    // canvas's user space and must NOT pre-multiply `xform(m)` itself. Folding it in again type-checks,
    // and passes a one-corner test by accident: the doubly-transformed sample lands off the image and the
    // `Pad` spread clamps every pixel to the source's top-left texel, which looks correct wherever that
    // texel's colour is what you asserted. The gate's `xform` claim checks all four quadrants for exactly
    // this reason, and a RED probe confirmed the fold is wrong rather than merely redundant.

    let mut paint = Paint::default();
    paint.shader = tiny_skia::Pattern::new(
        src_px.as_ref(),
        // `Pad` rather than `Repeat`: bilinear sampling reads half a texel outside the crop at its
        // edges, and repeating would wrap the opposite edge of the image in as a one-pixel fringe.
        tiny_skia::SpreadMode::Pad,
        tiny_skia::FilterQuality::Bilinear,
        alpha.clamp(0.0, 1.0),
        pat_t,
    );
    paint.anti_alias = true;

    CANVASES.with(|c| {
        if let Some(px) = c.borrow_mut().get_mut(&node) {
            let mut pb = PathBuilder::new();
            pb.push_rect(dest);
            if let Some(path) = pb.finish() {
                px.fill_path(&path, &paint, FillRule::Winding, xform(m), None);
            }
        }
    });
    mark_dirty(node);
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
