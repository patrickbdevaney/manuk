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
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DisplayList {
    pub items: Vec<DisplayItem>,
}

impl DisplayList {
    /// Whether this display list differs from `prev` — the invalidation check a compositor
    /// uses to skip re-rasterizing / re-uploading an idle frame whose content is unchanged.
    pub fn changed_since(&self, prev: &DisplayList) -> bool {
        self.items != prev.items
    }

    /// A coarse damage rectangle covering everything that changed vs `prev`: the union of
    /// the bounding rects of items present in one list but not the other (compared by index,
    /// a safe over-approximation). `None` if unchanged. Rect-anchored items contribute their
    /// rect; text/other items contribute a rect around their origin. The compositor repaints
    /// (and re-uploads) only this region instead of the whole viewport.
    pub fn damage_since(&self, prev: &DisplayList) -> Option<Rect> {
        if self.items == prev.items {
            return None;
        }
        let mut dmg: Option<Rect> = None;
        let mut add = |r: Rect| {
            dmg = Some(match dmg {
                Some(d) => d.union(&r),
                None => r,
            });
        };
        let item_rect = |it: &DisplayItem| -> Rect {
            match it {
                DisplayItem::Rect { rect, .. }
                | DisplayItem::Image { rect, .. }
                | DisplayItem::MaskedRect { rect, .. }
                | DisplayItem::Gradient { rect, .. }
                | DisplayItem::BackgroundImage { rect, .. }
                | DisplayItem::RoundRect { rect, .. } => *rect,
                DisplayItem::TextLine { x, y, width, thickness, .. } => Rect {
                    x: *x,
                    y: *y,
                    width: *width,
                    height: *thickness,
                },
                // A shadow bleeds `blur` px past its rect — grow the damage box so it repaints.
                DisplayItem::Shadow { rect, blur, .. } => Rect {
                    x: rect.x - blur,
                    y: rect.y - blur,
                    width: rect.width + blur * 2.0,
                    height: rect.height + blur * 2.0,
                },
                DisplayItem::Text { x, baseline, style, .. } => Rect {
                    x: *x,
                    y: baseline - style.line_height,
                    // Text has no stored width; a generous box keeps the damage a superset.
                    width: 4096.0,
                    height: style.line_height * 2.0,
                },
            }
        };
        let n = self.items.len().max(prev.items.len());
        for i in 0..n {
            let a = self.items.get(i);
            let b = prev.items.get(i);
            if a != b {
                if let Some(it) = a {
                    add(item_rect(it));
                }
                if let Some(it) = b {
                    add(item_rect(it));
                }
            }
        }
        dmg
    }
}

/// A decoded raster image: non-premultiplied RGBA8, row-major.
#[derive(Clone, Debug, PartialEq)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// One paint operation.
#[derive(Clone, Debug, PartialEq)]
pub enum DisplayItem {
    /// A solid-color rectangle (backgrounds, borders).
    Rect { rect: Rect, color: Rgba },
    /// A solid-color rectangle with rounded corners (`border-radius`). `radius` is uniform and
    /// already clamped to half the shorter side.
    RoundRect {
        rect: Rect,
        color: Rgba,
        radius: f32,
    },
    /// An outer `box-shadow`: a (rounded) rect offset by the shadow, softened over `blur` px.
    /// Painted *beneath* the box's own background.
    Shadow {
        rect: Rect,
        color: Rgba,
        radius: f32,
        blur: f32,
    },
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
    /// A `background-image: url(...)` layer. **Not** an `<img>`: a background is painted at its
    /// natural size and TILED by default — it is not stretched to fill its box. Treating it like a
    /// replaced image blew a subreddit's banner up to the size of the page and painted the content
    /// underneath it.
    BackgroundImage {
        rect: Rect,
        image: std::rc::Rc<DecodedImage>,
        size: manuk_css::BackgroundSize,
        repeat: manuk_css::BackgroundRepeat,
        radius: f32,
    },
    /// A **gradient** filling `rect`. `angle_deg` uses CSS's convention (0° points up, clockwise);
    /// a radial gradient ignores it and runs from the centre outwards.
    Gradient {
        rect: Rect,
        stops: Vec<manuk_css::ColorStop>,
        angle_deg: f32,
        radial: bool,
        radius: f32,
    },
    /// A **line under / over / through** a text run: `text-decoration`. Emitted as its own item
    /// because the line spans the run, not the glyphs, and must not be re-shaped.
    TextLine {
        x: f32,
        y: f32,
        width: f32,
        thickness: f32,
        color: Rgba,
    },
    /// `color` painted THROUGH a mask's alpha channel — how the modern web draws an **icon**:
    /// an empty element whose `background-color` is shaped by `mask-image`. Painting the
    /// background without the mask yields a solid block where the glyph should be.
    MaskedRect {
        rect: Rect,
        color: Rgba,
        mask: std::rc::Rc<DecodedImage>,
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
        let groups = Self::layered_groups(root, images, z_index, &std::collections::HashMap::new());
        DisplayList {
            items: groups.into_iter().flat_map(|(_, _, it)| it).collect(),
        }
    }

    /// The paint groups in stacking order: `(z, clip, items)` per box, stably sorted by `z`.
    /// `clip` is the intersection of any `overflow`-clipping ancestors' boxes (from
    /// `clip_map`), applied to this box's items at paint time.
    #[allow(clippy::type_complexity)]
    fn layered_groups(
        root: &LayoutBox,
        images: &std::collections::HashMap<manuk_dom::NodeId, std::rc::Rc<DecodedImage>>,
        z_index: &std::collections::HashMap<manuk_dom::NodeId, i32>,
        clip_map: &std::collections::HashMap<manuk_dom::NodeId, Rect>,
    ) -> Vec<(i32, Option<Rect>, Vec<DisplayItem>)> {
        // One group of paint items per box, tagged with its layer (effective z).
        let mut groups: Vec<(i32, Option<Rect>, Vec<DisplayItem>)> = Vec::new();
        root.walk(&mut |b| {
            let mut items = Vec::new();
            // `visibility: hidden` / `opacity: 0` — the box still occupies its space (layout already
            // accounted for it) but paints NOTHING. Without this, every dropdown, modal and tooltip
            // the modern web hides this way renders on top of the page.
            if b.hidden || b.opacity <= 0.01 {
                return;
            }
            // A radius can never exceed half the shorter side (CSS clamps overlapping corners).
            let radius = b.radius.min(b.rect.width / 2.0).min(b.rect.height / 2.0).max(0.0);
            // Partial opacity: scale every colour's alpha. (A true CSS opacity group would composite
            // the subtree off-screen; per-item alpha is a close, cheap approximation and is exact
            // for the overwhelmingly common non-overlapping case.)
            let fade = |c: Rgba| -> Rgba {
                if b.opacity >= 0.999 {
                    c
                } else {
                    Rgba { a: ((c.a as f32) * b.opacity).round().clamp(0.0, 255.0) as u8, ..c }
                }
            };
            // `box-shadow` paints *beneath* the background.
            if let Some(sh) = b.shadow {
                let sh = manuk_css::BoxShadow { color: fade(sh.color), ..sh };
                if sh.color.a > 0 {
                    items.push(DisplayItem::Shadow {
                        rect: Rect {
                            x: b.rect.x + sh.dx,
                            y: b.rect.y + sh.dy,
                            width: b.rect.width,
                            height: b.rect.height,
                        },
                        color: sh.color,
                        radius,
                        blur: sh.blur.max(0.0),
                    });
                }
            }
            // An element with `mask-image` whose mask decoded: paint its background through the
            // mask instead of as a rectangle. (Fetched into the same per-node bitmap map — a
            // masked element is empty by construction, so it is never also a replaced `<img>`.)
            let mask = match (&b.mask_image, b.node) {
                (Some(_), Some(n)) => images.get(&n).cloned(),
                _ => None,
            };
            // `background-image` sits ON TOP of `background-color` (CSS backgrounds paint
            // colour first, then each image layer). A gradient paints directly; a `url()` is
            // resolved to a decoded bitmap by the page layer and blitted into the box.
            let bg_img = b.background_image.clone();
            if let Some(bg) = b.background.map(fade) {
                if bg.a > 0 {
                    if let Some(m) = &mask {
                        items.push(DisplayItem::MaskedRect {
                            rect: b.rect,
                            color: bg,
                            mask: m.clone(),
                        });
                    } else if radius > 0.0 {
                        items.push(DisplayItem::RoundRect {
                            rect: b.rect,
                            color: bg,
                            radius,
                        });
                    } else {
                        items.push(DisplayItem::Rect {
                            rect: b.rect,
                            color: bg,
                        });
                    }
                }
            }
            if let Some(img) = &bg_img {
                match img {
                    manuk_css::BackgroundImage::Linear { angle_deg, stops } => {
                        items.push(DisplayItem::Gradient {
                            rect: b.rect,
                            stops: stops.iter().map(|s| manuk_css::ColorStop { color: fade(s.color), at: s.at }).collect(),
                            angle_deg: *angle_deg,
                            radial: false,
                            radius,
                        });
                    }
                    manuk_css::BackgroundImage::Radial { stops } => {
                        items.push(DisplayItem::Gradient {
                            rect: b.rect,
                            stops: stops.iter().map(|s| manuk_css::ColorStop { color: fade(s.color), at: s.at }).collect(),
                            angle_deg: 0.0,
                            radial: true,
                            radius,
                        });
                    }
                    // A `url()` background is keyed by node in the same bitmap map as `<img>` —
                    // the page layer fetches and decodes it there. It is painted as a BACKGROUND
                    // (natural size, tiled, honouring `background-size`/`-repeat`), not blitted to
                    // fill the box like a replaced image.
                    manuk_css::BackgroundImage::Url(_) => {
                        if let Some(node) = b.node {
                            if let Some(bmp) = images.get(&node) {
                                items.push(DisplayItem::BackgroundImage {
                                    rect: b.rect,
                                    image: bmp.clone(),
                                    size: b.background_size,
                                    repeat: b.background_repeat,
                                    radius,
                                });
                            }
                        }
                    }
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
            // **This blit is for REPLACED elements, and only for them.**
            //
            // It stretches the bitmap to fill the box, which is exactly right for an `<img>` and
            // exactly wrong for a `background-image: url()` — and a `url()` background's bitmap is
            // stored in the SAME `images` map, keyed by the same node. So every element with a CSS
            // background image got its correctly-tiled `BackgroundImage` item painted first, and
            // then this one stretched over the top of it. Every sprite, texture, pattern and icon
            // on the web was scaled to the size of its element; old.reddit.com's small header art
            // became a page-sized blob covering the content.
            //
            // A `url()` background on the box is the signal that this node's bitmap belongs to the
            // background layer, which already painted it properly.
            let bg_is_url = matches!(b.background_image, Some(manuk_css::BackgroundImage::Url(_)));
            if let Some(node) = b.node.filter(|_| mask.is_none() && !bg_is_url) {
                if let Some(img) = images.get(&node) {
                    items.push(DisplayItem::Image {
                        rect: b.rect,
                        image: img.clone(),
                    });
                }
            }
            // The list marker — generated content, so it rides on the box, not the tree.
            if let Some(m) = &b.marker {
                items.push(DisplayItem::Text {
                    x: m.x,
                    baseline: m.baseline,
                    text: m.text.clone(),
                    style: m.style,
                });
            }
            if let BoxContent::Inline(frags) = &b.content {
                for f in frags {
                    items.push(DisplayItem::Text {
                        x: f.x,
                        baseline: f.baseline,
                        text: f.text.clone(),
                        style: f.style,
                    });
                    // `text-decoration`: a line ACROSS the run, not part of the glyphs.
                    let d = f.style.decoration;
                    if d.any() && f.width > 0.0 {
                        let thickness = (f.style.font_size / 14.0).max(1.0);
                        let mut line = |y: f32| {
                            items.push(DisplayItem::TextLine {
                                x: f.x,
                                y,
                                width: f.width,
                                thickness,
                                color: fade(f.style.color),
                            });
                        };
                        if d.underline {
                            line(f.baseline + (f.style.font_size * 0.12).max(1.0));
                        }
                        if d.overline {
                            line(f.baseline - f.style.font_size * 0.9);
                        }
                        if d.line_through {
                            line(f.baseline - f.style.font_size * 0.30);
                        }
                    }
                }
            }
            // `outline` paints OUTSIDE the border box and never affects layout — which is exactly
            // what makes it usable as a focus ring.
            if let Some((ow, oc)) = b.outline {
                let oc = fade(oc);
                if ow > 0.0 && oc.a > 0 {
                    let r = b.rect;
                    let mut edge = |x: f32, y: f32, w: f32, h: f32| {
                        items.push(DisplayItem::Rect { rect: Rect { x, y, width: w, height: h }, color: oc });
                    };
                    edge(r.x - ow, r.y - ow, r.width + ow * 2.0, ow);
                    edge(r.x - ow, r.y + r.height, r.width + ow * 2.0, ow);
                    edge(r.x - ow, r.y, ow, r.height);
                    edge(r.x + r.width, r.y, ow, r.height);
                }
            }
            if !items.is_empty() {
                let z = b.node.and_then(|n| z_index.get(&n)).copied().unwrap_or(0);
                let clip = b.node.and_then(|n| clip_map.get(&n)).copied();
                groups.push((z, clip, items));
            }
        });
        // Stable sort keeps tree (document) order within each layer.
        groups.sort_by_key(|(z, _, _)| *z);
        groups
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
            let Some(bitmap) = fonts.rasterize(g.glyph_id, g.face, style.font_size, pen_x) else {
                continue;
            };
            if bitmap.width == 0 || bitmap.height == 0 {
                continue;
            }
            let left = pen_x.floor() as i32 + bitmap.left;
            let top = baseline.round() as i32 - bitmap.top;
            blit_glyph(&mut self.pixmap, &bitmap, left, top, style.color, None);
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

type ClipMap<'a> = std::collections::HashMap<manuk_dom::NodeId, Rect>;

pub struct CpuPainter<'a> {
    fonts: &'a FontContext,
    images: Option<&'a NodeImages<'a>>,
    z_index: Option<&'a ZIndexMap<'a>>,
    clip: Option<&'a ClipMap<'a>>,
}

impl<'a> CpuPainter<'a> {
    pub fn new(fonts: &'a FontContext) -> Self {
        CpuPainter {
            fonts,
            images: None,
            z_index: None,
            clip: None,
        }
    }

    /// A painter that also blits decoded images for replaced `<img>` nodes.
    pub fn with_images(fonts: &'a FontContext, images: &'a NodeImages<'a>) -> Self {
        CpuPainter {
            fonts,
            images: Some(images),
            z_index: None,
            clip: None,
        }
    }

    /// A painter that blits images, paints in stacking order (z-index), and clips content
    /// to `overflow`-clipping ancestors (`clip`).
    pub fn with_layers(
        fonts: &'a FontContext,
        images: &'a NodeImages<'a>,
        z_index: &'a ZIndexMap<'a>,
        clip: &'a ClipMap<'a>,
    ) -> Self {
        CpuPainter {
            fonts,
            images: Some(images),
            z_index: Some(z_index),
            clip: Some(clip),
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
        let empty_c = std::collections::HashMap::new();
        let groups = DisplayList::layered_groups(
            root,
            self.images.unwrap_or(&empty),
            self.z_index.unwrap_or(&empty_z),
            self.clip.unwrap_or(&empty_c),
        );
        for (_z, clip, items) in &groups {
            // A group's clip is an `overflow` ancestor's box; shift it by the scroll.
            let clip = clip.map(|c| Rect {
                x: c.x,
                y: c.y - scroll_y,
                width: c.width,
                height: c.height,
            });
            for item in items {
                match item {
                    DisplayItem::Rect { rect, color } => {
                        let mut r = *rect;
                        r.y -= scroll_y;
                        if let Some(cl) = clip {
                            r = r.intersect(&cl);
                        }
                        fill_rect(&mut pixmap, r, *color);
                    }
                    DisplayItem::RoundRect { rect, color, radius } => {
                        let mut r = *rect;
                        r.y -= scroll_y;
                        fill_round_rect(&mut pixmap, r, *color, *radius, clip);
                    }
                    DisplayItem::Shadow { rect, color, radius, blur } => {
                        let mut r = *rect;
                        r.y -= scroll_y;
                        fill_shadow(&mut pixmap, r, *color, *radius, *blur, clip);
                    }
                    DisplayItem::Text {
                        x,
                        baseline,
                        text,
                        style,
                    } => self.draw_text(&mut pixmap, *x, *baseline - scroll_y, text, style, clip),
                    DisplayItem::Image { rect, image } => {
                        let mut r = *rect;
                        r.y -= scroll_y;
                        blit_image(&mut pixmap, image, r, clip);
                    }
                    DisplayItem::MaskedRect { rect, color, mask } => {
                        let mut r = *rect;
                        r.y -= scroll_y;
                        blit_masked(&mut pixmap, mask, *color, r, clip);
                    }
                    DisplayItem::BackgroundImage { rect, image, size, repeat, radius } => {
                        let mut r = *rect;
                        r.y -= scroll_y;
                        blit_background(&mut pixmap, image, r, *size, *repeat, *radius, clip);
                    }
                    DisplayItem::Gradient { rect, stops, angle_deg, radial, radius } => {
                        let mut r = *rect;
                        r.y -= scroll_y;
                        fill_gradient(&mut pixmap, r, stops, *angle_deg, *radial, *radius, clip);
                    }
                    DisplayItem::TextLine { x, y, width, thickness, color } => {
                        let mut r = Rect { x: *x, y: *y - scroll_y, width: *width, height: *thickness };
                        if let Some(cl) = clip {
                            r = r.intersect(&cl);
                        }
                        fill_rect(&mut pixmap, r, *color);
                    }
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
        clip: Option<Rect>,
    ) {
        let run = self.fonts.shape(text, style.font_key, style.font_size);
        for g in &run.glyphs {
            let pen_x = origin_x + g.x;
            // swash rasterizes at the fractional pen position for crisp subpixel placement.
            let Some(bitmap) = self
                .fonts
                .rasterize(g.glyph_id, g.face, style.font_size, pen_x)
            else {
                continue;
            };
            if bitmap.width == 0 || bitmap.height == 0 {
                continue; // whitespace and zero-area glyphs
            }
            // swash placement: `left` = pen→bitmap-left, `top` = baseline→bitmap-top (up).
            let left = pen_x.floor() as i32 + bitmap.left;
            let top = baseline.round() as i32 - bitmap.top;
            blit_glyph(pixmap, &bitmap, left, top, style.color, clip);
        }
    }
}

/// Blit a rasterized glyph: an alpha coverage bitmap tinted with `color`, or a color/emoji
/// bitmap composited as-is (source-over), clipped to `clip`.
fn blit_glyph(
    pixmap: &mut tiny_skia::Pixmap,
    bmp: &manuk_text::GlyphBitmap,
    left: i32,
    top: i32,
    color: Rgba,
    clip: Option<Rect>,
) {
    if bmp.is_color {
        blit_color_glyph(
            pixmap,
            &bmp.coverage,
            bmp.width as usize,
            bmp.height as usize,
            left,
            top,
            clip,
        );
    } else {
        blit_coverage(
            pixmap,
            &bmp.coverage,
            bmp.width as usize,
            bmp.height as usize,
            left,
            top,
            color,
            clip,
        );
    }
}

/// Source-over composite a straight-alpha RGBA glyph bitmap onto the (opaque) pixmap.
#[allow(clippy::too_many_arguments)]
fn blit_color_glyph(
    pixmap: &mut tiny_skia::Pixmap,
    rgba: &[u8],
    gw: usize,
    gh: usize,
    left: i32,
    top: i32,
    clip: Option<Rect>,
) {
    let pw = pixmap.width() as i32;
    let ph = pixmap.height() as i32;
    let (cx0, cy0, cx1, cy1) = match clip {
        Some(c) => (
            c.x.floor() as i32,
            c.y.floor() as i32,
            c.right().ceil() as i32,
            c.bottom().ceil() as i32,
        ),
        None => (i32::MIN, i32::MIN, i32::MAX, i32::MAX),
    };
    let data = pixmap.data_mut();
    for row in 0..gh as i32 {
        let py = top + row;
        if py < 0 || py >= ph || py < cy0 || py >= cy1 {
            continue;
        }
        for col in 0..gw as i32 {
            let px = left + col;
            if px < 0 || px >= pw || px < cx0 || px >= cx1 {
                continue;
            }
            let s = ((row as usize) * gw + col as usize) * 4;
            let (sr, sg, sb, sa) = (rgba[s], rgba[s + 1], rgba[s + 2], rgba[s + 3]);
            if sa == 0 {
                continue;
            }
            let a = sa as f32 / 255.0;
            let d = ((py * pw + px) as usize) * 4;
            for (k, sc) in [sr, sg, sb].into_iter().enumerate() {
                data[d + k] = (sc as f32 * a + data[d + k] as f32 * (1.0 - a)).round() as u8;
            }
            data[d + 3] = 255;
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

/// A rounded-rectangle path (uniform corner radius), clamped so the corners never overlap.
fn round_rect_path(rect: Rect, radius: f32) -> Option<tiny_skia::Path> {
    let (x, y, w, h) = (rect.x, rect.y, rect.width, rect.height);
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let r = radius.min(w / 2.0).min(h / 2.0).max(0.0);
    let mut pb = tiny_skia::PathBuilder::new();
    if r <= 0.0 {
        pb.push_rect(tiny_skia::Rect::from_xywh(x, y, w, h)?);
        return pb.finish();
    }
    // `k` is the circle-approximating cubic constant: a quarter circle of radius r is closely
    // approximated by a Bézier whose control points sit k*r along the tangents.
    const K: f32 = 0.552_284_75;
    let c = r * K;
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.cubic_to(x + w - r + c, y, x + w, y + r - c, x + w, y + r); // top-right
    pb.line_to(x + w, y + h - r);
    pb.cubic_to(x + w, y + h - r + c, x + w - r + c, y + h, x + w - r, y + h); // bottom-right
    pb.line_to(x + r, y + h);
    pb.cubic_to(x + r - c, y + h, x, y + h - r + c, x, y + h - r); // bottom-left
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + r - c, x + r - c, y, x + r, y); // top-left
    pb.close();
    pb.finish()
}

/// Fill a rounded rect (`border-radius`), optionally clipped to an ancestor's overflow box.
fn fill_round_rect(
    pixmap: &mut tiny_skia::Pixmap,
    rect: Rect,
    color: Rgba,
    radius: f32,
    clip: Option<Rect>,
) {
    let Some(path) = round_rect_path(rect, radius) else {
        return;
    };
    let mask = clip.and_then(|cl| rect_mask(pixmap.width(), pixmap.height(), cl));
    let mut paint = tiny_skia::Paint::default();
    paint.set_color_rgba8(color.r, color.g, color.b, color.a);
    paint.anti_alias = true;
    pixmap.fill_path(
        &path,
        &paint,
        tiny_skia::FillRule::Winding,
        tiny_skia::Transform::identity(),
        mask.as_ref(),
    );
}

/// Paint an outer `box-shadow`. tiny-skia has no Gaussian blur, so the soft edge is approximated
/// by stacking concentric rounded rects: the shadow's rect grown by 0..blur px, each at a low
/// alpha, so the accumulated coverage falls off toward the outside — visually a soft drop shadow.
/// A `blur` of 0 is just a hard offset rect.
fn fill_shadow(
    pixmap: &mut tiny_skia::Pixmap,
    rect: Rect,
    color: Rgba,
    radius: f32,
    blur: f32,
    clip: Option<Rect>,
) {
    if rect.width <= 0.0 || rect.height <= 0.0 || color.a == 0 {
        return;
    }
    if blur <= 0.5 {
        fill_round_rect(pixmap, rect, color, radius, clip);
        return;
    }
    let mask = clip.and_then(|cl| rect_mask(pixmap.width(), pixmap.height(), cl));
    // One ring per px of blur (capped — a huge blur doesn't need hundreds of passes).
    let steps = (blur.ceil() as u32).clamp(1, 24);
    for i in (0..steps).rev() {
        // t: 0 at the outermost ring → 1 at the core.
        let t = (i as f32 + 1.0) / steps as f32;
        let grow = blur * (1.0 - t);
        let grown = Rect {
            x: rect.x - grow,
            y: rect.y - grow,
            width: rect.width + grow * 2.0,
            height: rect.height + grow * 2.0,
        };
        // Quadratic falloff reads closer to a Gaussian than a linear ramp.
        let a = (color.a as f32) * (t * t) / steps as f32 * 2.0;
        let alpha = a.clamp(0.0, 255.0) as u8;
        if alpha == 0 {
            continue;
        }
        let Some(path) = round_rect_path(grown, radius + grow) else {
            continue;
        };
        let mut paint = tiny_skia::Paint::default();
        paint.set_color_rgba8(color.r, color.g, color.b, alpha);
        paint.anti_alias = true;
        pixmap.fill_path(
            &path,
            &paint,
            tiny_skia::FillRule::Winding,
            tiny_skia::Transform::identity(),
            mask.as_ref(),
        );
    }
}

/// Scale a decoded (straight-alpha) RGBA image into `rect` and blit it onto the pixmap
/// with bilinear filtering.
fn blit_image(pixmap: &mut tiny_skia::Pixmap, image: &DecodedImage, rect: Rect, clip: Option<Rect>) {
    if rect.width <= 0.0 || rect.height <= 0.0 || image.width == 0 || image.height == 0 {
        return;
    }
    // Build a rectangular clip mask when the image is inside an overflow-clipping box.
    let mask = clip.and_then(|cl| rect_mask(pixmap.width(), pixmap.height(), cl));
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
    pixmap.draw_pixmap(0, 0, src.as_ref(), &paint, transform, mask.as_ref());
}

/// A full-canvas alpha mask that is opaque inside `clip` — used to bound image draws to an
/// overflow-clipping ancestor's box.
fn rect_mask(pw: u32, ph: u32, clip: Rect) -> Option<tiny_skia::Mask> {
    let mut mask = tiny_skia::Mask::new(pw, ph)?;
    let rect = tiny_skia::Rect::from_xywh(clip.x, clip.y, clip.width.max(0.0), clip.height.max(0.0))?;
    let path = tiny_skia::PathBuilder::from_rect(rect);
    mask.fill_path(
        &path,
        tiny_skia::FillRule::Winding,
        true,
        tiny_skia::Transform::identity(),
    );
    Some(mask)
}

/// Alpha-blit an 8-bit coverage bitmap in `color` onto the (opaque) pixmap.
///
/// The canvas starts fully opaque, so premultiplied == straight alpha here and we
/// can blend in-place without un/re-premultiplying.
#[allow(clippy::too_many_arguments)]
fn blit_coverage(
    pixmap: &mut tiny_skia::Pixmap,
    coverage: &[u8],
    gw: usize,
    gh: usize,
    left: i32,
    top: i32,
    color: Rgba,
    clip: Option<Rect>,
) {
    let pw = pixmap.width() as i32;
    let ph = pixmap.height() as i32;
    // Integer clip bounds (glyph pixels outside an overflow box are skipped).
    let (cx0, cy0, cx1, cy1) = match clip {
        Some(c) => (
            c.x.floor() as i32,
            c.y.floor() as i32,
            c.right().ceil() as i32,
            c.bottom().ceil() as i32,
        ),
        None => (i32::MIN, i32::MIN, i32::MAX, i32::MAX),
    };
    let data = pixmap.data_mut();
    for row in 0..gh as i32 {
        let py = top + row;
        if py < 0 || py >= ph || py < cy0 || py >= cy1 {
            continue;
        }
        for col in 0..gw as i32 {
            let px = left + col;
            if px < 0 || px >= pw || px < cx0 || px >= cx1 {
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
mod bg_tests {
    use super::*;
    use manuk_css::{MinimalCascade, StyleEngine, Stylesheet};

    /// Regression: **a `background-image: url()` must not ALSO be blitted as a replaced image.**
    ///
    /// A `url()` background's decoded bitmap lives in the same `images` map, keyed by the same node,
    /// as an `<img>`'s does. The replaced-element blit — which stretches the bitmap to fill the box,
    /// and is exactly right for an `<img>` — therefore fired for backgrounds too, painting a
    /// stretched copy on top of the correctly-tiled background beneath it. Every sprite, texture,
    /// pattern and icon on the web was scaled up to the size of its element; old.reddit.com's small
    /// header art became a page-sized blob over the content.
    #[test]
    fn a_url_background_is_not_also_painted_as_a_replaced_image() {
        let dom = manuk_html::parse(r#"<div id="d">x</div>"#);
        let styles = MinimalCascade.cascade(
            &dom,
            &[Stylesheet::parse("#d{width:300px;height:120px;background-image:url(t.png)}")],
        );
        let fonts = FontContext::new();
        let root = manuk_layout::layout_document(&dom, &styles, &fonts, 400.0);

        // Stand in for the decoded bitmap the page layer would have fetched.
        let node = dom
            .descendants(dom.root())
            .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some("d"))
            .expect("the div");
        let mut images = std::collections::HashMap::new();
        images.insert(
            node,
            std::rc::Rc::new(DecodedImage { width: 40, height: 30, rgba: vec![255; 40 * 30 * 4] }),
        );

        let items = DisplayList::build_with_images(&root, &images).items;
        let backgrounds = items
            .iter()
            .filter(|i| matches!(i, DisplayItem::BackgroundImage { .. }))
            .count();
        let replaced = items.iter().filter(|i| matches!(i, DisplayItem::Image { .. })).count();

        assert_eq!(
            backgrounds, 1,
            "the background layer must paint the bitmap — tiled, at its natural size, honouring \
             background-size/-repeat"
        );
        assert_eq!(
            replaced, 0,
            "and the REPLACED-element blit must NOT also fire: it stretches the bitmap to fill the \
             box, painting a scaled copy straight over the tiled background"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use manuk_css::{MinimalCascade, StyleEngine, Stylesheet};

    #[test]
    fn display_list_change_detection_and_damage() {
        let red = DisplayItem::Rect {
            rect: Rect { x: 0.0, y: 0.0, width: 10.0, height: 10.0 },
            color: Rgba::new(255, 0, 0, 255),
        };
        let blue = DisplayItem::Rect {
            rect: Rect { x: 100.0, y: 100.0, width: 20.0, height: 20.0 },
            color: Rgba::new(0, 0, 255, 255),
        };
        let a = DisplayList { items: vec![red.clone(), blue.clone()] };
        let b = DisplayList { items: vec![red.clone(), blue.clone()] };
        // Identical lists → no change, no damage (idle frame skips re-upload).
        assert!(!a.changed_since(&b));
        assert_eq!(a.damage_since(&b), None);

        // Change the second item's color → changed, and the damage covers its rect.
        let blue2 = DisplayItem::Rect {
            rect: Rect { x: 100.0, y: 100.0, width: 20.0, height: 20.0 },
            color: Rgba::new(0, 200, 0, 255),
        };
        let c = DisplayList { items: vec![red, blue2] };
        assert!(c.changed_since(&a));
        let dmg = c.damage_since(&a).expect("some damage");
        // Damage must contain the changed rect (100,100 20x20).
        assert!(dmg.x <= 100.0 && dmg.y <= 100.0 && dmg.right() >= 120.0 && dmg.bottom() >= 120.0);
    }

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

    /// `border-radius` actually cuts the corners: the centre of a rounded rect is filled while
    /// its extreme corner pixel is not. (Verified visually too — see the render screenshots.)
    #[test]
    fn rounded_rect_cuts_the_corners() {
        let mut pm = tiny_skia::Pixmap::new(50, 50).expect("pixmap");
        let red = Rgba { r: 255, g: 0, b: 0, a: 255 };
        fill_round_rect(
            &mut pm,
            Rect { x: 0.0, y: 0.0, width: 50.0, height: 50.0 },
            red,
            20.0,
            None,
        );
        let alpha = |x: u32, y: u32| pm.data()[((y * 50 + x) * 4 + 3) as usize];
        assert_eq!(alpha(25, 25), 255, "the centre is filled");
        assert_eq!(alpha(0, 0), 0, "the corner is cut away by the 20px radius");
        assert_eq!(alpha(49, 0), 0, "…on every corner");
        assert_eq!(alpha(25, 0), 255, "but the straight top edge is still filled");
    }

    /// An outer `box-shadow` paints *outside* the box (softened over `blur`), and nothing at all
    /// when the shadow colour is transparent.
    #[test]
    fn box_shadow_paints_outside_the_box() {
        let mut pm = tiny_skia::Pixmap::new(60, 60).expect("pixmap");
        let black = Rgba { r: 0, g: 0, b: 0, a: 200 };
        // A 20x20 box at (20,20), shadow blurred 8px: pixels just outside it get some alpha.
        fill_shadow(
            &mut pm,
            Rect { x: 20.0, y: 20.0, width: 20.0, height: 20.0 },
            black,
            0.0,
            8.0,
            None,
        );
        let alpha = |x: u32, y: u32| pm.data()[((y * 60 + x) * 4 + 3) as usize];
        assert!(alpha(30, 30) > 0, "the shadow core is painted");
        assert!(alpha(30, 15) > 0, "it bleeds above the box (blur)");
        assert_eq!(alpha(0, 0), 0, "but not across the whole canvas");
    }
}

/// Paint `color` through `mask`'s **alpha channel**, scaled to fill `rect`.
///
/// This is how the modern web draws icons: an empty element with a `background-color` and a
/// `mask-image` holding the glyph's shape. tiny-skia has no mask-composite op, so this is a direct
/// source-over blend — for every destination pixel, sample the mask, multiply its alpha into the
/// fill colour, and composite. Nearest sampling is deliberate: icons are small and crisp, and
/// smoothing a 20×20 glyph scaled to 16px only muddies it.
fn blit_masked(
    pixmap: &mut tiny_skia::Pixmap,
    mask: &DecodedImage,
    color: Rgba,
    rect: Rect,
    clip: Option<Rect>,
) {
    if rect.width <= 0.0 || rect.height <= 0.0 || mask.width == 0 || mask.height == 0 {
        return;
    }
    let (pw, ph) = (pixmap.width() as i32, pixmap.height() as i32);
    let x0 = rect.x.floor().max(0.0) as i32;
    let y0 = rect.y.floor().max(0.0) as i32;
    let x1 = (rect.x + rect.width).ceil().min(pw as f32) as i32;
    let y1 = (rect.y + rect.height).ceil().min(ph as f32) as i32;
    // Intersect with any overflow clip.
    let (cx0, cy0, cx1, cy1) = match clip {
        Some(c) => (
            x0.max(c.x.floor() as i32),
            y0.max(c.y.floor() as i32),
            x1.min((c.x + c.width).ceil() as i32),
            y1.min((c.y + c.height).ceil() as i32),
        ),
        None => (x0, y0, x1, y1),
    };
    let data = pixmap.pixels_mut();
    for py in cy0..cy1 {
        for px in cx0..cx1 {
            // Map the destination pixel back into mask space.
            let u = ((px as f32 - rect.x) / rect.width * mask.width as f32) as i32;
            let v = ((py as f32 - rect.y) / rect.height * mask.height as f32) as i32;
            if u < 0 || v < 0 || u >= mask.width as i32 || v >= mask.height as i32 {
                continue;
            }
            let mi = ((v as u32 * mask.width + u as u32) * 4) as usize;
            let Some(&ma) = mask.rgba.get(mi + 3) else { continue };
            if ma == 0 {
                continue;
            }
            let a = (ma as f32 / 255.0) * (color.a as f32 / 255.0);
            if a <= 0.002 {
                continue;
            }
            let di = (py * pw + px) as usize;
            let Some(dst) = data.get_mut(di) else { continue };
            // Source-over, on premultiplied storage.
            let inv = 1.0 - a;
            let blend = |s: u8, d: u8| -> u8 {
                ((s as f32 * a) + (d as f32 * inv)).round().clamp(0.0, 255.0) as u8
            };
            let (r, g, b) = (
                blend(color.r, dst.red()),
                blend(color.g, dst.green()),
                blend(color.b, dst.blue()),
            );
            let na = ((a * 255.0) + (dst.alpha() as f32 * inv)).round().clamp(0.0, 255.0) as u8;
            if let Some(p) = tiny_skia::PremultipliedColorU8::from_rgba(
                r.min(na),
                g.min(na),
                b.min(na),
                na,
            ) {
                *dst = p;
            }
        }
    }
}


/// Fill `rect` with a **gradient** — the modern web's most common background.
///
/// tiny-skia has real gradient shaders, but they need `GradientStop`s and a transform; a direct
/// per-pixel evaluation is simpler, exact for our stop model, and lets the radial case share the
/// same code. `angle_deg` follows CSS: **0° points up**, angles increase clockwise — which is not
/// the maths convention and is the usual place to get this wrong.
#[allow(clippy::too_many_arguments)]
fn fill_gradient(
    pixmap: &mut tiny_skia::Pixmap,
    rect: Rect,
    stops: &[manuk_css::ColorStop],
    angle_deg: f32,
    radial: bool,
    radius: f32,
    clip: Option<Rect>,
) {
    if rect.width <= 0.0 || rect.height <= 0.0 || stops.is_empty() {
        return;
    }
    let (pw, ph) = (pixmap.width() as i32, pixmap.height() as i32);
    let mut r = rect;
    if let Some(cl) = clip {
        r = r.intersect(&cl);
    }
    let x0 = r.x.floor().max(0.0) as i32;
    let y0 = r.y.floor().max(0.0) as i32;
    let x1 = (r.x + r.width).ceil().min(pw as f32) as i32;
    let y1 = (r.y + r.height).ceil().min(ph as f32) as i32;
    if x1 <= x0 || y1 <= y0 {
        return;
    }

    // The gradient LINE, per CSS Images 3: it passes through the centre at `angle_deg`, and its
    // length is the projection of the box onto it, so the first and last stops land exactly on the
    // corners.
    let a = angle_deg.to_radians();
    let (dx, dy) = (a.sin(), -a.cos()); // 0° = up
    let (cx, cy) = (rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
    let len = (rect.width * dx.abs() + rect.height * dy.abs()).max(1.0);
    let rmax = ((rect.width * rect.width + rect.height * rect.height).sqrt() / 2.0).max(1.0);

    let sample = |t: f32| -> Rgba {
        let t = t.clamp(0.0, 1.0);
        if t <= stops[0].at {
            return stops[0].color;
        }
        let last = stops[stops.len() - 1];
        if t >= last.at {
            return last.color;
        }
        for w in stops.windows(2) {
            let (a, b) = (w[0], w[1]);
            if t >= a.at && t <= b.at {
                let span = (b.at - a.at).max(1e-6);
                let f = (t - a.at) / span;
                let lerp = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * f).round().clamp(0.0, 255.0) as u8;
                return Rgba {
                    r: lerp(a.color.r, b.color.r),
                    g: lerp(a.color.g, b.color.g),
                    b: lerp(a.color.b, b.color.b),
                    a: lerp(a.color.a, b.color.a),
                };
            }
        }
        last.color
    };

    let rad = radius.min(rect.width / 2.0).min(rect.height / 2.0).max(0.0);
    let data = pixmap.pixels_mut();
    for py in y0..y1 {
        for px in x0..x1 {
            let (fx, fy) = (px as f32 + 0.5, py as f32 + 0.5);
            // Respect a border-radius: a gradient in a rounded card must not spill its corners.
            if rad > 0.0 && !inside_round_rect(fx, fy, &rect, rad) {
                continue;
            }
            let t = if radial {
                (((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt()) / rmax
            } else {
                ((fx - cx) * dx + (fy - cy) * dy) / len + 0.5
            };
            let c = sample(t);
            if c.a == 0 {
                continue;
            }
            let al = c.a as f32 / 255.0;
            let di = (py * pw + px) as usize;
            let Some(dst) = data.get_mut(di) else { continue };
            let inv = 1.0 - al;
            let blend = |s: u8, d: u8| ((s as f32 * al) + (d as f32 * inv)).round().clamp(0.0, 255.0) as u8;
            let na = ((al * 255.0) + (dst.alpha() as f32 * inv)).round().clamp(0.0, 255.0) as u8;
            let (rr, gg, bb) = (blend(c.r, dst.red()), blend(c.g, dst.green()), blend(c.b, dst.blue()));
            if let Some(p) = tiny_skia::PremultipliedColorU8::from_rgba(rr.min(na), gg.min(na), bb.min(na), na) {
                *dst = p;
            }
        }
    }
}

/// Is `(x, y)` inside a rounded rectangle? (Corner circles, straight edges.)
fn inside_round_rect(x: f32, y: f32, r: &Rect, rad: f32) -> bool {
    let (l, t, rt, b) = (r.x, r.y, r.x + r.width, r.y + r.height);
    if x < l || x > rt || y < t || y > b {
        return false;
    }
    let corner = |cx: f32, cy: f32| (x - cx).powi(2) + (y - cy).powi(2) <= rad * rad;
    if x < l + rad && y < t + rad {
        return corner(l + rad, t + rad);
    }
    if x > rt - rad && y < t + rad {
        return corner(rt - rad, t + rad);
    }
    if x < l + rad && y > b - rad {
        return corner(l + rad, b - rad);
    }
    if x > rt - rad && y > b - rad {
        return corner(rt - rad, b - rad);
    }
    true
}


/// Paint a `background-image` into `rect`: at its **natural size** by default, **tiled** by default,
/// clipped to the box, honouring `background-size` and `background-repeat`.
///
/// The distinction from `blit_image` is the whole point. An `<img>` is a *replaced element*: the
/// bitmap IS the box, so it scales to fill it. A background is a *decoration*: it keeps its own
/// size and repeats. Painting a background the first way stretched a subreddit's banner across the
/// entire page and buried the content beneath it.
#[allow(clippy::too_many_arguments)]
fn blit_background(
    pixmap: &mut tiny_skia::Pixmap,
    img: &DecodedImage,
    rect: Rect,
    size: manuk_css::BackgroundSize,
    repeat: manuk_css::BackgroundRepeat,
    radius: f32,
    clip: Option<Rect>,
) {
    use manuk_css::{BackgroundRepeat as R, BackgroundSize as S};
    if rect.width <= 0.0 || rect.height <= 0.0 || img.width == 0 || img.height == 0 {
        return;
    }
    let (iw, ih) = (img.width as f32, img.height as f32);
    let (tw, th) = match size {
        S::Auto => (iw, ih),
        S::Px(w, h) => (w.max(1.0), h.max(1.0)),
        S::Cover => {
            let k = (rect.width / iw).max(rect.height / ih);
            (iw * k, ih * k)
        }
        S::Contain => {
            let k = (rect.width / iw).min(rect.height / ih);
            (iw * k, ih * k)
        }
    };
    if tw < 0.5 || th < 0.5 {
        return;
    }

    let mut r = rect;
    if let Some(cl) = clip {
        r = r.intersect(&cl);
    }
    let (pw, ph) = (pixmap.width() as i32, pixmap.height() as i32);
    let x0 = r.x.floor().max(0.0) as i32;
    let y0 = r.y.floor().max(0.0) as i32;
    let x1 = (r.x + r.width).ceil().min(pw as f32) as i32;
    let y1 = (r.y + r.height).ceil().min(ph as f32) as i32;
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let rad = radius.min(rect.width / 2.0).min(rect.height / 2.0).max(0.0);
    let tile = matches!(repeat, R::Repeat);
    let data = pixmap.pixels_mut();
    for py in y0..y1 {
        for px in x0..x1 {
            let (fx, fy) = (px as f32 + 0.5, py as f32 + 0.5);
            if rad > 0.0 && !inside_round_rect(fx, fy, &rect, rad) {
                continue;
            }
            // Position within the tile, measured from the box's origin.
            let mut lx = fx - rect.x;
            let mut ly = fy - rect.y;
            if tile {
                lx = lx.rem_euclid(tw);
                ly = ly.rem_euclid(th);
            } else if lx < 0.0 || lx >= tw || ly < 0.0 || ly >= th {
                continue; // no-repeat: outside the single tile, paint nothing
            }
            let u = ((lx / tw) * iw) as i32;
            let v = ((ly / th) * ih) as i32;
            if u < 0 || v < 0 || u >= img.width as i32 || v >= img.height as i32 {
                continue;
            }
            let si = ((v as u32 * img.width + u as u32) * 4) as usize;
            let Some(px4) = img.rgba.get(si..si + 4) else { continue };
            let a = px4[3] as f32 / 255.0;
            if a <= 0.002 {
                continue;
            }
            let di = (py * pw + px) as usize;
            let Some(dst) = data.get_mut(di) else { continue };
            let inv = 1.0 - a;
            let blend = |s: u8, d: u8| ((s as f32 * a) + (d as f32 * inv)).round().clamp(0.0, 255.0) as u8;
            let na = ((a * 255.0) + (dst.alpha() as f32 * inv)).round().clamp(0.0, 255.0) as u8;
            let (rr, gg, bb) = (blend(px4[0], dst.red()), blend(px4[1], dst.green()), blend(px4[2], dst.blue()));
            if let Some(p) = tiny_skia::PremultipliedColorU8::from_rgba(rr.min(na), gg.min(na), bb.min(na), na) {
                *dst = p;
            }
        }
    }
}
