//! manuk-paint — display list construction + rasterization tiers.
//!
//! CLAUDE.md's paint target is **Vello** (GPU-compute via `wgpu`) for the focused
//! tab, with Vello CPU / Hybrid as lighter tiers for background/hibernated tabs.
//! Vello is alpha upstream, so this first pass implements the **CPU tier for real**
//! with `tiny-skia` (rects) + `fontdue` glyph blitting, behind the [`Painter`]
//! trait. That gives a headless-verifiable `render-to-PNG` path today; a
//! `VelloGpuPainter` drops in behind the same trait for the focused tab without
//! layout/compositor changes.
//!
//! The intermediate [`DisplayList`] is the hand-off the compositor also consumes,
//! so the GPU tier and damage tracking share one representation.

use anyhow::Result;
use manuk_css::Rgba;
use manuk_layout::{BoxContent, LayoutBox, Rect, TextStyle};
use manuk_text::FontContext;

/// A flat, back-to-front list of paint operations derived from a fragment tree.
#[derive(Clone, Debug, Default)]
pub struct DisplayList {
    pub items: Vec<DisplayItem>,
}

/// A decoded raster image: non-premultiplied RGBA8, row-major.
#[derive(Clone, Debug)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// One paint operation.
#[derive(Clone, Debug)]
pub enum DisplayItem {
    /// A solid-color rectangle (backgrounds, borders).
    Rect { rect: Rect, color: Rgba },
    /// A run of text drawn along a baseline.
    Text {
        x: f32,
        baseline: f32,
        text: String,
        style: TextStyle,
    },
    /// A decoded image scaled into `rect` (a replaced `<img>`'s content box).
    Image {
        rect: Rect,
        image: std::rc::Rc<DecodedImage>,
    },
}

impl DisplayList {
    /// Flatten a laid-out fragment tree into a display list (backgrounds first,
    /// then text, in document order — a correct back-to-front order for normal
    /// flow without z-index).
    pub fn build(root: &LayoutBox) -> DisplayList {
        Self::build_with_images(root, &std::collections::HashMap::new())
    }

    /// Like [`build`], but emits an [`DisplayItem::Image`] for any box whose DOM node has a
    /// decoded image in `images` (a replaced `<img>`), painted over its box after its
    /// background so the bitmap fills the element.
    pub fn build_with_images(
        root: &LayoutBox,
        images: &std::collections::HashMap<manuk_dom::NodeId, std::rc::Rc<DecodedImage>>,
    ) -> DisplayList {
        Self::build_layered(root, images, &std::collections::HashMap::new())
    }

    /// Like [`build_with_images`], but paints in **stacking order**: each box's items are
    /// grouped and the groups are stably sorted by the box's effective z-index (`z_index`,
    /// keyed by node — negative behind, positive in front, tree order within a layer). A
    /// positioned element with an explicit z-index applies its layer to its whole subtree
    /// (an approximation of CSS stacking contexts), so overlays/modals paint on top.
    pub fn build_layered(
        root: &LayoutBox,
        images: &std::collections::HashMap<manuk_dom::NodeId, std::rc::Rc<DecodedImage>>,
        z_index: &std::collections::HashMap<manuk_dom::NodeId, i32>,
    ) -> DisplayList {
        // One group of paint items per box, tagged with its layer (effective z).
        let mut groups: Vec<(i32, Vec<DisplayItem>)> = Vec::new();
        root.walk(&mut |b| {
            let mut items = Vec::new();
            if let Some(bg) = b.background {
                if bg.a > 0 {
                    items.push(DisplayItem::Rect {
                        rect: b.rect,
                        color: bg,
                    });
                }
            }
            if let Some(border) = &b.border {
                let r = b.rect;
                let [t, rr, bb, l] = border.widths;
                let c = border.color;
                let mut edge = |x: f32, y: f32, w: f32, h: f32| {
                    if w > 0.0 && h > 0.0 {
                        items.push(DisplayItem::Rect {
                            rect: Rect { x, y, width: w, height: h },
                            color: c,
                        });
                    }
                };
                edge(r.x, r.y, r.width, t); // top
                edge(r.x, r.y + r.height - bb, r.width, bb); // bottom
                edge(r.x, r.y, l, r.height); // left
                edge(r.x + r.width - rr, r.y, rr, r.height); // right
            }
            if let Some(node) = b.node {
                if let Some(img) = images.get(&node) {
                    items.push(DisplayItem::Image {
                        rect: b.rect,
                        image: img.clone(),
                    });
                }
            }
            if let BoxContent::Inline(frags) = &b.content {
                for f in frags {
                    items.push(DisplayItem::Text {
                        x: f.x,
                        baseline: f.baseline,
                        text: f.text.clone(),
                        style: f.style,
                    });
                }
            }
            if !items.is_empty() {
                let z = b.node.and_then(|n| z_index.get(&n)).copied().unwrap_or(0);
                groups.push((z, items));
            }
        });
        // Stable sort keeps tree (document) order within each layer.
        groups.sort_by_key(|(z, _)| *z);
        DisplayList {
            items: groups.into_iter().flat_map(|(_, it)| it).collect(),
        }
    }
}

/// An owned RGBA raster surface backed by a `tiny-skia` pixmap.
pub struct Canvas {
    pixmap: tiny_skia::Pixmap,
}

impl Canvas {
    /// A blank canvas filled with `background` — for a page-less view (new tab) that still
    /// needs browser chrome drawn on it.
    pub fn new(width: u32, height: u32, background: Rgba) -> Self {
        let mut pixmap = tiny_skia::Pixmap::new(width.max(1), height.max(1))
            .expect("valid pixmap dimensions");
        pixmap.fill(tiny_skia::Color::from_rgba8(
            background.r,
            background.g,
            background.b,
            background.a,
        ));
        Canvas { pixmap }
    }

    pub fn width(&self) -> u32 {
        self.pixmap.width()
    }
    pub fn height(&self) -> u32 {
        self.pixmap.height()
    }
    /// Premultiplied RGBA8 bytes, row-major — ready for a GPU texture upload.
    pub fn rgba_bytes(&self) -> &[u8] {
        self.pixmap.data()
    }
    /// Encode the canvas as PNG.
    pub fn encode_png(&self) -> Result<Vec<u8>> {
        Ok(self.pixmap.encode_png()?)
    }
    /// Encode and write the canvas to `path` as a PNG.
    pub fn save_png(&self, path: impl AsRef<std::path::Path>) -> Result<()> {
        std::fs::write(path, self.encode_png()?)?;
        Ok(())
    }

    /// E1 — composite a translucent rect **on top** of the already-rendered page.
    ///
    /// This is the find-in-page highlight primitive. It is deliberately an overlay
    /// applied after paint: highlighting must never mutate the DOM or trigger a
    /// relayout. Coordinates are viewport pixels (the caller subtracts the scroll).
    /// Rects outside the canvas are clipped, not an error.
    pub fn fill_rect_blended(&mut self, x: f32, y: f32, width: f32, height: f32, color: Rgba) {
        let Some(rect) = tiny_skia::Rect::from_xywh(x, y, width, height) else {
            return; // non-finite or non-positive extent
        };
        let mut paint = tiny_skia::Paint::default();
        paint.set_color_rgba8(color.r, color.g, color.b, color.a);
        paint.anti_alias = false;
        // `SourceOver` = alpha-composite over what is already drawn.
        paint.blend_mode = tiny_skia::BlendMode::SourceOver;
        self.pixmap
            .fill_rect(rect, &paint, tiny_skia::Transform::identity(), None);
    }

    /// Stroke a rect outline (used to mark the *active* find match).
    pub fn stroke_rect(&mut self, x: f32, y: f32, width: f32, height: f32, color: Rgba, w: f32) {
        let Some(rect) = tiny_skia::Rect::from_xywh(x, y, width, height) else {
            return;
        };
        let path = tiny_skia::PathBuilder::from_rect(rect);
        let mut paint = tiny_skia::Paint::default();
        paint.set_color_rgba8(color.r, color.g, color.b, color.a);
        paint.anti_alias = true;
        let stroke = tiny_skia::Stroke {
            width: w,
            ..Default::default()
        };
        self.pixmap
            .stroke_path(&path, &paint, &stroke, tiny_skia::Transform::identity(), None);
    }

    /// Fill an opaque rect (used for browser chrome bands drawn over the page).
    pub fn fill_rect(&mut self, x: f32, y: f32, width: f32, height: f32, color: Rgba) {
        fill_rect(&mut self.pixmap, Rect { x, y, width, height }, color);
    }

    /// Draw a text string with its baseline at `baseline`, left edge at `origin_x`. Shapes
    /// and rasterizes via `fonts`. Used for browser chrome (address bar, buttons) — the
    /// page's own text goes through the layout/paint pipeline, not this.
    pub fn draw_text(
        &mut self,
        fonts: &FontContext,
        origin_x: f32,
        baseline: f32,
        text: &str,
        style: &TextStyle,
    ) {
        let run = fonts.shape(text, style.font_key, style.font_size);
        for g in &run.glyphs {
            let pen_x = origin_x + g.x;
            let Some(bitmap) = fonts.rasterize(g.glyph_id, style.font_key, style.font_size, pen_x) else {
                continue;
            };
            if bitmap.width == 0 || bitmap.height == 0 {
                continue;
            }
            let left = pen_x.floor() as i32 + bitmap.left;
            let top = baseline.round() as i32 - bitmap.top;
            blit_coverage(
                &mut self.pixmap,
                &bitmap.coverage,
                bitmap.width as usize,
                bitmap.height as usize,
                left,
                top,
                style.color,
            );
        }
    }
}

/// A rasterization backend. The CPU tier is [`CpuPainter`]; a Vello GPU tier will
/// implement the same trait for the focused tab.
pub trait Painter {
    fn render(&self, root: &LayoutBox, width: u32, height: u32, background: Rgba) -> Canvas;
}

/// The CPU rasterization tier: `tiny-skia` for fills, `fontdue` glyph coverage
/// blitting for text. Deterministic and headless — no GPU/display required.
type NodeImages<'a> = std::collections::HashMap<manuk_dom::NodeId, std::rc::Rc<DecodedImage>>;
type ZIndexMap<'a> = std::collections::HashMap<manuk_dom::NodeId, i32>;

pub struct CpuPainter<'a> {
    fonts: &'a FontContext,
    images: Option<&'a NodeImages<'a>>,
    z_index: Option<&'a ZIndexMap<'a>>,
}

impl<'a> CpuPainter<'a> {
    pub fn new(fonts: &'a FontContext) -> Self {
        CpuPainter {
            fonts,
            images: None,
            z_index: None,
        }
    }

    /// A painter that also blits decoded images for replaced `<img>` nodes.
    pub fn with_images(fonts: &'a FontContext, images: &'a NodeImages<'a>) -> Self {
        CpuPainter {
            fonts,
            images: Some(images),
            z_index: None,
        }
    }

    /// A painter that blits images **and** paints in stacking order per the effective
    /// z-index of each node.
    pub fn with_images_and_z(
        fonts: &'a FontContext,
        images: &'a NodeImages<'a>,
        z_index: &'a ZIndexMap<'a>,
    ) -> Self {
        CpuPainter {
            fonts,
            images: Some(images),
            z_index: Some(z_index),
        }
    }
}

impl Painter for CpuPainter<'_> {
    fn render(&self, root: &LayoutBox, width: u32, height: u32, background: Rgba) -> Canvas {
        self.render_scrolled(root, width, height, background, 0.0)
    }
}

impl CpuPainter<'_> {
    /// Render into a `width × height` canvas with the page content shifted up by
    /// `scroll_y` px — i.e. paint only the visible viewport of a scrolled page.
    pub fn render_scrolled(
        &self,
        root: &LayoutBox,
        width: u32,
        height: u32,
        background: Rgba,
        scroll_y: f32,
    ) -> Canvas {
        let w = width.max(1);
        let h = height.max(1);
        let mut pixmap = tiny_skia::Pixmap::new(w, h).expect("valid pixmap dimensions");
        pixmap.fill(tiny_skia::Color::from_rgba8(
            background.r,
            background.g,
            background.b,
            background.a,
        ));

        let empty = std::collections::HashMap::new();
        let empty_z = std::collections::HashMap::new();
        let list = DisplayList::build_layered(
            root,
            self.images.unwrap_or(&empty),
            self.z_index.unwrap_or(&empty_z),
        );
        for item in &list.items {
            match item {
                DisplayItem::Rect { rect, color } => {
                    let mut r = *rect;
                    r.y -= scroll_y;
                    fill_rect(&mut pixmap, r, *color);
                }
                DisplayItem::Text {
                    x,
                    baseline,
                    text,
                    style,
                } => self.draw_text(&mut pixmap, *x, *baseline - scroll_y, text, style),
                DisplayItem::Image { rect, image } => {
                    let mut r = *rect;
                    r.y -= scroll_y;
                    blit_image(&mut pixmap, image, r);
                }
            }
        }

        Canvas { pixmap }
    }
}

impl CpuPainter<'_> {
    fn draw_text(
        &self,
        pixmap: &mut tiny_skia::Pixmap,
        origin_x: f32,
        baseline: f32,
        text: &str,
        style: &TextStyle,
    ) {
        let run = self.fonts.shape(text, style.font_key, style.font_size);
        for g in &run.glyphs {
            let pen_x = origin_x + g.x;
            // swash rasterizes at the fractional pen position for crisp subpixel placement.
            let Some(bitmap) = self
                .fonts
                .rasterize(g.glyph_id, style.font_key, style.font_size, pen_x)
            else {
                continue;
            };
            if bitmap.width == 0 || bitmap.height == 0 {
                continue; // whitespace and zero-area glyphs
            }
            // swash placement: `left` = pen→bitmap-left, `top` = baseline→bitmap-top (up).
            let left = pen_x.floor() as i32 + bitmap.left;
            let top = baseline.round() as i32 - bitmap.top;
            blit_coverage(
                pixmap,
                &bitmap.coverage,
                bitmap.width as usize,
                bitmap.height as usize,
                left,
                top,
                style.color,
            );
        }
    }
}

fn fill_rect(pixmap: &mut tiny_skia::Pixmap, rect: Rect, color: Rgba) {
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return;
    }
    let Some(r) = tiny_skia::Rect::from_xywh(rect.x, rect.y, rect.width, rect.height) else {
        return;
    };
    let mut paint = tiny_skia::Paint::default();
    paint.set_color_rgba8(color.r, color.g, color.b, color.a);
    paint.anti_alias = true;
    pixmap.fill_rect(r, &paint, tiny_skia::Transform::identity(), None);
}

/// Scale a decoded (straight-alpha) RGBA image into `rect` and blit it onto the pixmap
/// with bilinear filtering.
fn blit_image(pixmap: &mut tiny_skia::Pixmap, image: &DecodedImage, rect: Rect) {
    if rect.width <= 0.0 || rect.height <= 0.0 || image.width == 0 || image.height == 0 {
        return;
    }
    // Build a source pixmap, premultiplying each pixel (tiny-skia stores premultiplied).
    let Some(mut src) = tiny_skia::Pixmap::new(image.width, image.height) else {
        return;
    };
    let dst_px = src.pixels_mut();
    for (i, px) in dst_px.iter_mut().enumerate() {
        let o = i * 4;
        let (r, g, b, a) = (
            image.rgba[o],
            image.rgba[o + 1],
            image.rgba[o + 2],
            image.rgba[o + 3],
        );
        *px = tiny_skia::ColorU8::from_rgba(r, g, b, a).premultiply();
    }
    let sx = rect.width / image.width as f32;
    let sy = rect.height / image.height as f32;
    let transform = tiny_skia::Transform::from_row(sx, 0.0, 0.0, sy, rect.x, rect.y);
    let paint = tiny_skia::PixmapPaint {
        quality: tiny_skia::FilterQuality::Bilinear,
        ..Default::default()
    };
    pixmap.draw_pixmap(0, 0, src.as_ref(), &paint, transform, None);
}

/// Alpha-blit an 8-bit coverage bitmap in `color` onto the (opaque) pixmap.
///
/// The canvas starts fully opaque, so premultiplied == straight alpha here and we
/// can blend in-place without un/re-premultiplying.
fn blit_coverage(
    pixmap: &mut tiny_skia::Pixmap,
    coverage: &[u8],
    gw: usize,
    gh: usize,
    left: i32,
    top: i32,
    color: Rgba,
) {
    let pw = pixmap.width() as i32;
    let ph = pixmap.height() as i32;
    let data = pixmap.data_mut();
    for row in 0..gh as i32 {
        let py = top + row;
        if py < 0 || py >= ph {
            continue;
        }
        for col in 0..gw as i32 {
            let px = left + col;
            if px < 0 || px >= pw {
                continue;
            }
            let cov = coverage[(row as usize) * gw + (col as usize)];
            if cov == 0 {
                continue;
            }
            let a = (cov as f32 / 255.0) * (color.a as f32 / 255.0);
            let idx = ((py * pw + px) as usize) * 4;
            data[idx] = lerp(data[idx], color.r, a);
            data[idx + 1] = lerp(data[idx + 1], color.g, a);
            data[idx + 2] = lerp(data[idx + 2], color.b, a);
            data[idx + 3] = 255;
        }
    }
}

#[inline]
fn lerp(dst: u8, src: u8, a: f32) -> u8 {
    (src as f32 * a + dst as f32 * (1.0 - a))
        .round()
        .clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use manuk_css::{MinimalCascade, StyleEngine, Stylesheet};

    fn render_html(html: &str, css: &str, w: u32, h: u32) -> Canvas {
        let dom = manuk_html::parse(html);
        let styles = MinimalCascade.cascade(&dom, &[Stylesheet::parse(css)]);
        let fonts = FontContext::new();
        let root = manuk_layout::layout_document(&dom, &styles, &fonts, w as f32);
        CpuPainter::new(&fonts).render(&root, w, h, Rgba::WHITE)
    }

    fn count_non_white(canvas: &Canvas) -> usize {
        canvas
            .rgba_bytes()
            .chunks_exact(4)
            .filter(|p| p[0] < 250 || p[1] < 250 || p[2] < 250)
            .count()
    }

    #[test]
    fn renders_background_rect() {
        let canvas = render_html(
            "<body style='margin:0'><div style='width:100px;height:50px;background:red'></div></body>",
            "",
            200,
            100,
        );
        // A solid red block should paint ~100*50 non-white pixels.
        assert!(count_non_white(&canvas) > 4000, "background not painted");
    }

    #[test]
    fn renders_text_pixels() {
        let canvas = render_html(
            "<body style='margin:0'><p>Hello world</p></body>",
            "",
            300,
            80,
        );
        let fonts = FontContext::new();
        if fonts.face_count() == 0 {
            eprintln!("no system fonts; skipping text-pixel assertion");
            return;
        }
        assert!(count_non_white(&canvas) > 50, "text glyphs not painted");
    }

    #[test]
    fn png_round_trips() {
        let canvas = render_html("<body><p>hi</p></body>", "", 64, 32);
        let png = canvas.encode_png().unwrap();
        // PNG magic number.
        assert_eq!(
            &png[..8],
            &[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n']
        );
    }
}
