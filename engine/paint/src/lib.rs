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

mod filters;
use filters::apply_filters;

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
        let n = self.items.len().max(prev.items.len());
        for i in 0..n {
            let a = self.items.get(i);
            let b = prev.items.get(i);
            if a != b {
                if let Some(it) = a {
                    add(item_bounds(it));
                }
                if let Some(it) = b {
                    add(item_bounds(it));
                }
            }
        }
        dmg
    }
}

/// The rect an item can ink, in page coordinates — a deliberate **over**-approximation.
///
/// Two callers depend on that direction and neither tolerates the other one: damage tracking must
/// cover every pixel that changed or the compositor leaves stale content on screen, and a filter's
/// offscreen group must cover every pixel the group paints or the filtered element is cropped. So
/// there is one definition, and it is allowed to be loose but never tight — including the 4096px
/// text box, which is a superset stand-in for a width the display list does not carry.
fn item_bounds(it: &DisplayItem) -> Rect {
    match it {
        DisplayItem::Rect { rect, .. }
        | DisplayItem::Image { rect, .. }
        | DisplayItem::MaskedRect { rect, .. }
        | DisplayItem::Gradient { rect, .. }
        | DisplayItem::BackgroundImage { rect, .. }
        | DisplayItem::RoundRect { rect, .. } => *rect,
        DisplayItem::TextLine {
            x,
            y,
            width,
            thickness,
            ..
        } => Rect {
            x: *x,
            y: *y,
            width: *width,
            height: *thickness,
        },
        // A shadow bleeds `blur` px past its rect — grow the box so it repaints.
        DisplayItem::Shadow { rect, blur, .. } => Rect {
            x: rect.x - blur,
            y: rect.y - blur,
            width: rect.width + blur * 2.0,
            height: rect.height + blur * 2.0,
        },
        DisplayItem::Text {
            x, baseline, style, ..
        } => Rect {
            x: *x,
            y: baseline - style.line_height,
            width: 4096.0,
            height: style.line_height * 2.0,
        },
    }
}

/// A decoded raster image: non-premultiplied RGBA8, row-major.
#[derive(Clone, Debug, PartialEq)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// A cue the **UA itself** must paint over a media element's box.
///
/// Ticks 255-260 built the whole caption pipeline — parse, hold, time, fire `cuechange`, preserve
/// placement — and stopped one step short: every one of those steps hands cues to *a renderer*, and
/// on a plain `<video>` with `<track default>` there is no renderer, because a page without a player
/// library never draws a caption itself. The browser is supposed to. Until this item existed, a
/// correctly parsed, correctly timed, correctly placed cue reached the viewer as nothing at all.
///
/// The settings arrive in **the spec's own vocabulary**, exactly as tick 260 left them, and are
/// resolved to pixels here — this is the code the `CueSettings` doc comment was deferring to, the
/// renderer that finally knows the video box. `line`/`position` are `None` for `auto`, and `auto` is
/// not `0`: `line: 0` is the TOP of the frame and `auto` is the bottom.
#[derive(Clone, Debug, PartialEq)]
pub struct CaptionCue {
    /// Cue text. Newlines are hard breaks the author wrote, and each becomes its own painted line.
    pub text: String,
    /// `line` — `None` is `auto` (stack up from the bottom).
    pub line: Option<f64>,
    /// Whether `line` is a percentage of the box height rather than a **line count**. A bare
    /// `line:-1` means the LAST line, which as a percentage would be nonsense.
    pub line_is_percent: bool,
    /// `position` — `None` is `auto` (centred in the writing direction).
    pub position: Option<f64>,
    /// `size` — the cue box width, as a percentage of the video box. Default 100.
    pub size: f64,
    /// `align` — `start` | `center` | `end` | `left` | `right`.
    pub align: String,
    /// `vertical` — `""` (horizontal) | `"rl"` | `"lr"`. **Vertical writing is not painted
    /// vertically yet**; a vertical cue is laid out horizontally, which is wrong but legible.
    /// Recorded rather than dropped so the gap stays visible.
    pub vertical: String,
}

/// Cues to paint over each media element's box, keyed by node — the same NodeId-keyed side-map
/// channel `images` and `z_index` use, because the painter never sees the DOM.
pub type CaptionMap = std::collections::HashMap<manuk_dom::NodeId, Vec<CaptionCue>>;

/// The UA caption stylesheet, in one place: white text on a translucent black box, sized relative
/// to the video (browsers scale captions with the picture — a fixed px size is unreadable on a
/// thumbnail and comical full-screen).
const CAPTION_FONT_FRACTION: f32 = 0.06;
const CAPTION_FONT_MIN: f32 = 10.0;
const CAPTION_FONT_MAX: f32 = 48.0;
const CAPTION_LINE_HEIGHT: f32 = 1.25;
/// Breathing room between the bottom-most caption line and the bottom edge of the video.
const CAPTION_BOTTOM_PAD: f32 = 0.04;

/// Resolve `cues` against the video box `r` into paint items: a translucent backing rect per line,
/// then the line's text.
///
/// Returns items in back-to-front order, ready to append after the video's own blit.
pub fn caption_items(r: Rect, cues: &[CaptionCue]) -> Vec<DisplayItem> {
    if r.width <= 0.0 || r.height <= 0.0 {
        return Vec::new();
    }
    let font_size = (r.height * CAPTION_FONT_FRACTION).clamp(CAPTION_FONT_MIN, CAPTION_FONT_MAX);
    let line_height = font_size * CAPTION_LINE_HEIGHT;
    let style = manuk_layout::TextStyle {
        font_key: manuk_text::FontKey::default(),
        font_size,
        color: Rgba {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        },
        line_height,
        decoration: manuk_css::TextDecoration::default(),
        letter_spacing: 0.0,
        word_spacing: 0.0,
        shadow: None,
        rtl: false,
        // A video caption is drawn on the frame, never in a page's writing mode.
        sideways: false,
    };

    // Every hard-wrapped line of every cue, tagged with the cue it came from. A two-line cue
    // occupies two lines of the frame, so `auto` stacking has to count LINES, not cues — otherwise
    // a multi-line cue overlaps whatever sits above it.
    let mut lines: Vec<(&CaptionCue, &str)> = Vec::new();
    for c in cues {
        for l in c.text.split('\n') {
            lines.push((c, l));
        }
    }

    // `auto`-line cues stack UP from the bottom, oldest at the top of the stack — the browser
    // behaviour a viewer reads as "the new line pushes the old one up".
    let auto_total = lines
        .iter()
        .filter(|(c, _)| c.line.is_none())
        .count()
        .max(1);
    let mut auto_seen = 0usize;
    let bottom = r.y + r.height - r.height * CAPTION_BOTTOM_PAD;

    let mut items = Vec::new();
    for (cue, text) in &lines {
        // --- vertical placement -------------------------------------------------------------
        let top = match cue.line {
            None => {
                let from_end = auto_total - auto_seen - 1;
                auto_seen += 1;
                bottom - line_height * (from_end as f32 + 1.0)
            }
            Some(v) if cue.line_is_percent => r.y + r.height * (v as f32 / 100.0),
            // A LINE COUNT: non-negative counts down from the top, negative counts back from the
            // bottom with `-1` naming the last line.
            Some(v) if v >= 0.0 => r.y + line_height * v as f32,
            Some(v) => bottom + line_height * v as f32,
        };
        // --- horizontal placement -----------------------------------------------------------
        let box_w = r.width * (cue.size as f32 / 100.0).clamp(0.0, 1.0);
        // `align` decides which edge of the cue box `position` names, so `auto` position lands the
        // box where the alignment implies rather than always centring it.
        let anchor = match cue.align.as_str() {
            "start" | "left" => 0.0,
            "end" | "right" => 1.0,
            _ => 0.5,
        };
        let pos_frac = match cue.position {
            Some(p) => (p as f32 / 100.0).clamp(0.0, 1.0),
            None => anchor,
        };
        let box_x = r.x + r.width * pos_frac - box_w * anchor;

        // The text is placed inside the cue box by the same alignment. Width is estimated from the
        // font size rather than shaped here — shaping happens at raster time (`CpuPainter`), and
        // the display list deliberately carries plain strings.
        let est_w = (text.chars().count() as f32 * font_size * 0.5).min(box_w);
        let text_x = box_x + (box_w - est_w) * anchor;

        let backing = Rect {
            x: text_x,
            y: top,
            width: est_w,
            height: line_height,
        };
        items.push(DisplayItem::Rect {
            rect: backing,
            color: Rgba {
                r: 0,
                g: 0,
                b: 0,
                a: 204,
            },
        });
        items.push(DisplayItem::Text {
            x: text_x,
            // Sit the baseline inside the line box, leaving room for descenders.
            baseline: top + font_size,
            text: (*text).to_string(),
            style,
        });
    }
    items
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
    /// A decoded image drawn into `rect`. For the default `object-fit: fill`, `rect` is the box and
    /// the bitmap stretches to it. For `cover`/`contain`/`none`/`scale-down`, `rect` is the
    /// **aspect-ratio-preserved destination** the full bitmap is scaled into (which for `cover`/`none`
    /// may exceed the box), and `content_clip` is the box the overflow is cropped to.
    Image {
        rect: Rect,
        image: std::rc::Rc<DecodedImage>,
        /// `object-fit` crop box (the used content box). `None` = fill/contain, which never overflow.
        content_clip: Option<Rect>,
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
        position: manuk_css::BackgroundPosition,
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
        Self::build_captioned(root, images, z_index, &CaptionMap::new())
    }

    /// Like [`build_layered`], but also paints the UA's own caption overlay over any media box that
    /// has active cues. Split from `build_layered` rather than added to it so the four existing
    /// callers keep compiling unchanged — the overlay is opt-in at the one call site that can know
    /// whether a `<track>` is showing.
    pub fn build_captioned(
        root: &LayoutBox,
        images: &std::collections::HashMap<manuk_dom::NodeId, std::rc::Rc<DecodedImage>>,
        z_index: &std::collections::HashMap<manuk_dom::NodeId, i32>,
        captions: &CaptionMap,
    ) -> DisplayList {
        let groups = Self::layered_groups(
            root,
            images,
            z_index,
            &std::collections::HashMap::new(),
            captions,
        );
        DisplayList {
            items: groups.into_iter().flat_map(|g| g.items).collect(),
        }
    }

    /// The paint groups in stacking order: `(z, clip, filters, items)` per box, stably sorted by `z`.
    /// `clip` is the intersection of any `overflow`-clipping ancestors' boxes (from
    /// `clip_map`), applied to this box's items at paint time. `filters` is the composed `filter`
    /// chain from the root down to this box (see [`PaintGroup`]).
    #[allow(clippy::type_complexity)]
    pub(crate) fn layered_groups(
        root: &LayoutBox,
        images: &std::collections::HashMap<manuk_dom::NodeId, std::rc::Rc<DecodedImage>>,
        z_index: &std::collections::HashMap<manuk_dom::NodeId, i32>,
        clip_map: &std::collections::HashMap<manuk_dom::NodeId, Rect>,
        captions: &CaptionMap,
    ) -> Vec<PaintGroup> {
        // One group of paint items per box, tagged with its layer (effective z).
        //
        // **An anonymous box has no node, and therefore was falling out of its own subtree's layer.**
        // `z` and `clip` are looked up by NodeId; a box the layout engine synthesised (the inline
        // formatting context inside a block, for instance) has none, so it got `z = 0` and no clip
        // no matter what stacking context it was actually inside.
        //
        // That is not a corner case — it is where the TEXT lives. A `z-index`'d ancestor put its own
        // background in layer 1 while the anonymous box holding its text stayed in layer 0, so the
        // background sorted *after* the text and painted straight over it. old.reddit.com's post
        // titles were laid out at the right place, in the right colour, at full alpha, present in the
        // display list — and buried under their own ancestor's background. The identical hole in the
        // clip lookup means an anonymous box also escaped every `overflow: hidden` above it.
        //
        // So z and clip are INHERITED down the tree, and a node's own entry (when it has one) wins.
        let mut groups: Vec<PaintGroup> = Vec::new();
        fn visit(
            b: &LayoutBox,
            inherited_z: i32,
            inherited_clip: Option<Rect>,
            inherited_filters: &[manuk_css::FilterOp],
            inherited_shapes: &[(manuk_css::ClipShape, Rect)],
            inherited_blend: manuk_css::BlendMode,
            z_index: &std::collections::HashMap<manuk_dom::NodeId, i32>,
            clip_map: &std::collections::HashMap<manuk_dom::NodeId, Rect>,
            emit: &mut impl FnMut(
                &LayoutBox,
                i32,
                Option<Rect>,
                &[manuk_css::FilterOp],
                &[(manuk_css::ClipShape, Rect)],
                manuk_css::BlendMode,
            ),
        ) {
            let z = b
                .node
                .and_then(|n| z_index.get(&n))
                .copied()
                .unwrap_or(inherited_z);
            let clip = b
                .node
                .and_then(|n| clip_map.get(&n))
                .copied()
                .or(inherited_clip);
            // `filter` applies to the element AND its subtree, so it accumulates downwards the way
            // `clip` does — but it CONCATENATES rather than overriding: a blurred card inside a
            // grayscale section is both.
            let filters: Vec<manuk_css::FilterOp> = if b.filters.is_empty() {
                inherited_filters.to_vec()
            } else {
                inherited_filters
                    .iter()
                    .copied()
                    .chain(b.filters.iter().copied())
                    .collect()
            };
            // `clip-path` accumulates the same way, and it must carry the REFERENCE BOX with it:
            // percentages in the shape resolve against the box that DECLARED the clip, not against
            // the descendant being painted. Nested clips INTERSECT, which is what applying each
            // mask in turn does (`apply_mask` multiplies alpha).
            let shapes: Vec<(manuk_css::ClipShape, Rect)> = match &b.clip_path {
                None => inherited_shapes.to_vec(),
                Some(cp) => inherited_shapes
                    .iter()
                    .cloned()
                    .chain(std::iter::once((cp.clone(), b.rect)))
                    .collect(),
            };
            // `mix-blend-mode` inherits down the subtree in the same PAINT sense the other two do
            // — the element and its contents blend as one group — but a descendant that declares its
            // OWN mode overrides rather than composing: blending is not a pipeline, there is one
            // backdrop and one formula.
            let blend = if b.blend.is_blending() {
                b.blend
            } else {
                inherited_blend
            };
            emit(b, z, clip, &filters, &shapes, blend);
            if let BoxContent::Block(children) = &b.content {
                for c in children {
                    visit(
                        c, z, clip, &filters, &shapes, blend, z_index, clip_map, emit,
                    );
                }
            }
        }
        let mut push_group = |b: &LayoutBox,
                              z: i32,
                              clip: Option<Rect>,
                              filters: &[manuk_css::FilterOp],
                              shapes: &[(manuk_css::ClipShape, Rect)],
                              blend: manuk_css::BlendMode| {
            let mut items = Vec::new();
            // `visibility: hidden` / `opacity: 0` — the box still occupies its space (layout already
            // accounted for it) but paints NOTHING. Without this, every dropdown, modal and tooltip
            // the modern web hides this way renders on top of the page.
            if b.hidden || b.opacity <= 0.01 {
                return;
            }
            // A radius can never exceed half the shorter side (CSS clamps overlapping corners).
            let radius = b
                .radius
                .min(b.rect.width / 2.0)
                .min(b.rect.height / 2.0)
                .max(0.0);
            // Partial opacity: scale every colour's alpha. (A true CSS opacity group would composite
            // the subtree off-screen; per-item alpha is a close, cheap approximation and is exact
            // for the overwhelmingly common non-overlapping case.)
            let fade = |c: Rgba| -> Rgba {
                if b.opacity >= 0.999 {
                    c
                } else {
                    Rgba {
                        a: ((c.a as f32) * b.opacity).round().clamp(0.0, 255.0) as u8,
                        ..c
                    }
                }
            };
            // `box-shadow` paints *beneath* the background. A comma list stacks layers (Tailwind's
            // `shadow-md` is two); the FIRST layer paints on top, so push in reverse. `inset` layers
            // are honestly skipped (inner painting is not built yet — same as before).
            for sh in b.shadows.iter().rev() {
                if sh.inset {
                    continue;
                }
                let color = fade(sh.color);
                if color.a == 0 {
                    continue;
                }
                // `spread` inflates the shadow rect before offset/blur (negative shrinks it).
                items.push(DisplayItem::Shadow {
                    rect: Rect {
                        x: b.rect.x + sh.dx - sh.spread,
                        y: b.rect.y + sh.dy - sh.spread,
                        width: (b.rect.width + 2.0 * sh.spread).max(0.0),
                        height: (b.rect.height + 2.0 * sh.spread).max(0.0),
                    },
                    color,
                    radius,
                    blur: sh.blur.max(0.0),
                });
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
            // `background-image` is a LIST of layers; the FIRST is on top, and CSS paints
            // back-to-front, so iterate in REVERSE (last layer painted first = bottom).
            for img in b.background_images.iter().rev() {
                match img {
                    manuk_css::BackgroundImage::Linear { angle_deg, stops } => {
                        items.push(DisplayItem::Gradient {
                            rect: b.rect,
                            stops: stops
                                .iter()
                                .map(|s| manuk_css::ColorStop {
                                    color: fade(s.color),
                                    at: s.at,
                                })
                                .collect(),
                            angle_deg: *angle_deg,
                            radial: false,
                            radius,
                        });
                    }
                    manuk_css::BackgroundImage::Radial { stops } => {
                        items.push(DisplayItem::Gradient {
                            rect: b.rect,
                            stops: stops
                                .iter()
                                .map(|s| manuk_css::ColorStop {
                                    color: fade(s.color),
                                    at: s.at,
                                })
                                .collect(),
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
                                    position: b.background_position,
                                    radius,
                                });
                            }
                        }
                    }
                }
            }
            if let Some(border) = &b.border {
                use manuk_css::BorderStyle as BS;
                let r = b.rect;
                let [t, rr, bb, l] = border.widths;
                let mut rect = |x: f32, y: f32, w: f32, h: f32, c: Rgba| {
                    if w > 0.0 && h > 0.0 {
                        items.push(DisplayItem::Rect {
                            rect: Rect {
                                x,
                                y,
                                width: w,
                                height: h,
                            },
                            color: c,
                        });
                    }
                };
                // One edge strip (`horizontal` = a top/bottom bar, else a left/right bar). `thick` is
                // the strip's short dimension; `len` its long one. Solid emits one rect (byte-identical
                // to before); dashed/dotted emit segments along `len`; double splits `thick` into two
                // lines with a gap.
                // ⚠ `style` and `c` are now PARAMETERS, not captures. They used to be read once,
                // from the top edge, and applied to all four — see `ComputedStyle::border_color`.
                let mut edge =
                    |x: f32, y: f32, w: f32, h: f32, horizontal: bool, c: Rgba, style: BS| {
                        if w <= 0.0 || h <= 0.0 {
                            return;
                        }
                        let (thick, len) = if horizontal { (h, w) } else { (w, h) };
                        match style {
                            BS::Solid => rect(x, y, w, h, c),
                            BS::Dashed | BS::Dotted => {
                                let (dash, gap) = if matches!(style, BS::Dashed) {
                                    (3.0 * thick, 3.0 * thick)
                                } else {
                                    (thick, thick) // dotted: square dots, one-thickness gap
                                };
                                let period = (dash + gap).max(0.5);
                                let mut pos = 0.0;
                                while pos < len {
                                    let seg = dash.min(len - pos);
                                    if horizontal {
                                        rect(x + pos, y, seg, h, c);
                                    } else {
                                        rect(x, y + pos, w, seg, c);
                                    }
                                    pos += period;
                                }
                            }
                            BS::Double => {
                                // Two lines each ~1/3 of the thickness, at the outer edges. Below 3px the
                                // thirds collapse and it reads as solid — the honest degradation.
                                let unit = (thick / 3.0).floor().max(1.0);
                                if horizontal {
                                    rect(x, y, w, unit, c);
                                    rect(x, y + h - unit, w, unit, c);
                                } else {
                                    rect(x, y, unit, h, c);
                                    rect(x + w - unit, y, unit, h, c);
                                }
                            }
                        }
                    };
                let [ct, cr, cb, cl] = border.colors;
                let [st, sr, sb, sl] = border.styles;
                edge(r.x, r.y, r.width, t, true, ct, st); // top
                edge(r.x, r.y + r.height - bb, r.width, bb, true, cb, sb); // bottom
                edge(r.x, r.y, l, r.height, false, cl, sl); // left
                edge(r.x + r.width - rr, r.y, rr, r.height, false, cr, sr); // right
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
            let bg_is_url = b
                .background_images
                .iter()
                .any(|i| matches!(i, manuk_css::BackgroundImage::Url(_)));
            if let Some(node) = b.node.filter(|_| mask.is_none() && !bg_is_url) {
                if let Some(img) = images.get(&node) {
                    let (rect, content_clip) = object_fit_geometry(
                        b.object_fit,
                        b.object_position,
                        b.rect,
                        img.width,
                        img.height,
                    );
                    items.push(DisplayItem::Image {
                        rect,
                        image: img.clone(),
                        content_clip,
                    });
                }
            }
            // Captions paint OVER the video's own bitmap, and therefore after it — a cue behind the
            // frame is a cue nobody can read, which is the state this whole arc was stuck in.
            if let Some(node) = b.node {
                if let Some(cues) = captions.get(&node) {
                    items.extend(caption_items(b.rect, cues));
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
                    // ⚠ A run in a VERTICAL writing mode keeps logical fields on the fragment (see
                    // `TextFragment::vertical`), so the two numbers the display list wants are asked
                    // for by name: the pen's starting **y** and the baseline's **x**. They arrive in
                    // the item's `x`/`baseline` slots because that is exactly what a ninety-degree
                    // rotation makes of those two words, and `style.sideways` says which reading
                    // applies. A horizontal run is untouched — `vertical_pen` is `None`.
                    let (item_x, item_baseline) = match f.vertical_pen() {
                        Some((baseline_x, pen_y)) => (pen_y, baseline_x),
                        None => (f.x, f.baseline),
                    };
                    items.push(DisplayItem::Text {
                        x: item_x,
                        baseline: item_baseline,
                        text: f.text.clone(),
                        style: f.style,
                    });
                    // `text-decoration`: a line ACROSS the run, not part of the glyphs.
                    //
                    // ⚠ Skipped for a SIDEWAYS run and named rather than approximated: a
                    // `TextLine` is a horizontal strip by construction, so emitting one for a
                    // rotated run draws a rule across the page through unrelated content. A missing
                    // underline is a smaller wrong than a stripe over the article, and the fix is a
                    // rotated line primitive rather than different numbers here.
                    let d = f.style.decoration;
                    if d.any() && f.width > 0.0 && !f.style.sideways {
                        // `text-decoration-thickness` defaults to `auto` (font-derived); a length
                        // overrides it (Tailwind `decoration-2`, thick brand underlines).
                        let thickness = d
                            .thickness
                            .filter(|t| *t > 0.0)
                            .unwrap_or((f.style.font_size / 14.0).max(1.0));
                        // `text-decoration-color` defaults to `currentColor` — the text color —
                        // but a colored underline (hover states, brand links) sets its own.
                        let line_color = fade(d.color.unwrap_or(f.style.color));
                        let mut line = |y: f32| {
                            items.push(DisplayItem::TextLine {
                                x: f.x,
                                y,
                                width: f.width,
                                thickness,
                                color: line_color,
                            });
                        };
                        if d.underline {
                            // `text-underline-offset` pushes the underline further below the text.
                            line(
                                f.baseline
                                    + (f.style.font_size * 0.12).max(1.0)
                                    + d.underline_offset,
                            );
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
                        items.push(DisplayItem::Rect {
                            rect: Rect {
                                x,
                                y,
                                width: w,
                                height: h,
                            },
                            color: oc,
                        });
                    };
                    edge(r.x - ow, r.y - ow, r.width + ow * 2.0, ow);
                    edge(r.x - ow, r.y + r.height, r.width + ow * 2.0, ow);
                    edge(r.x - ow, r.y, ow, r.height);
                    edge(r.x + r.width, r.y, ow, r.height);
                }
            }
            if !items.is_empty() {
                groups.push(PaintGroup {
                    z,
                    clip,
                    filters: filters.to_vec(),
                    shapes: shapes.to_vec(),
                    blend,
                    backdrop: b.backdrop.clone(),
                    bounds: b.rect,
                    items,
                });
            }
        };
        visit(
            root,
            0,
            None,
            &[],
            &[],
            manuk_css::BlendMode::Normal,
            z_index,
            clip_map,
            &mut push_group,
        );
        // Stable sort keeps tree (document) order within each layer.
        groups.sort_by_key(|g| g.z);
        groups
    }
}

/// One box's paint items, tagged with everything the rasterizer needs that is not in the items
/// themselves: its stacking layer, its inherited `overflow` clip, and its composed `filter` chain.
///
/// **A group is the unit a `filter` is applied to, and that is an approximation with a name.** CSS
/// says the filter applies to the element and its subtree *composited as one group*; here each box
/// in that subtree is filtered separately, because the display list is flat and z-sorted and a real
/// group would have to survive that sort. For the colour filters the two are identical wherever the
/// group's own pixels do not overlap each other — a colour transform is per-pixel — and for `blur`
/// they differ only across an internal edge. The case it gets right is the one that matters: the
/// element and everything in it is blurred, rather than nothing being blurred at all.
pub(crate) struct PaintGroup {
    pub z: i32,
    pub clip: Option<Rect>,
    pub filters: Vec<manuk_css::FilterOp>,
    /// The `clip-path` chain from the root down, each paired with the border box of the element
    /// that declared it — the shape's reference box, which a descendant's own box is not.
    pub shapes: Vec<(manuk_css::ClipShape, Rect)>,
    /// `mix-blend-mode` for this group. Anything but `Normal` forces the offscreen path, because a
    /// blend needs the group's own pixels SEPARATE from the backdrop it is blending against.
    pub blend: manuk_css::BlendMode,
    /// `backdrop-filter` for this box — its OWN value, never inherited (see `LayoutBox::backdrop`).
    pub backdrop: Vec<manuk_css::FilterOp>,
    /// This box's own border box, in page coordinates. `backdrop-filter` is confined to it, and it
    /// is the one thing a display list of loose items cannot reconstruct.
    pub bounds: Rect,
    pub items: Vec<DisplayItem>,
}

/// An owned RGBA raster surface backed by a `tiny-skia` pixmap.
pub struct Canvas {
    pixmap: tiny_skia::Pixmap,
}

impl Canvas {
    /// A blank canvas filled with `background` — for a page-less view (new tab) that still
    /// needs browser chrome drawn on it.
    pub fn new(width: u32, height: u32, background: Rgba) -> Self {
        let mut pixmap =
            tiny_skia::Pixmap::new(width.max(1), height.max(1)).expect("valid pixmap dimensions");
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
        self.pixmap.stroke_path(
            &path,
            &paint,
            &stroke,
            tiny_skia::Transform::identity(),
            None,
        );
    }

    /// Fill an opaque rect (used for browser chrome bands drawn over the page).
    pub fn fill_rect(&mut self, x: f32, y: f32, width: f32, height: f32, color: Rgba) {
        fill_rect(
            &mut self.pixmap,
            Rect {
                x,
                y,
                width,
                height,
            },
            color,
        );
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
        let run = fonts.shape_bidi(text, style.font_key, style.font_size, style.rtl);
        // `letter-spacing` pushes each successive glyph out by a running multiple of the tracking, so
        // glyph `i` sits at `i × letter_spacing` past its shaped pen position. This mirrors layout's
        // width bump (`letter_spacing × char_count`), so a tracked run measures and paints in step.
        // Zero (the default) leaves every pen position exactly as shaped.
        let ls = style.letter_spacing;
        for (i, g) in run.glyphs.iter().enumerate() {
            let pen_x = origin_x + g.x + ls * i as f32;
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
    captions: Option<&'a CaptionMap>,
}

impl<'a> CpuPainter<'a> {
    pub fn new(fonts: &'a FontContext) -> Self {
        CpuPainter {
            fonts,
            images: None,
            z_index: None,
            clip: None,
            captions: None,
        }
    }

    /// Paint the UA's own caption overlay over media boxes with active cues.
    ///
    /// A builder method rather than another `with_*` constructor argument: the existing painter
    /// call sites (shell, demo, the WPT runner) have no captions to supply and should not have to
    /// pass an empty map to say so.
    pub fn with_captions(mut self, captions: &'a CaptionMap) -> Self {
        self.captions = Some(captions);
        self
    }

    /// A painter that also blits decoded images for replaced `<img>` nodes.
    pub fn with_images(fonts: &'a FontContext, images: &'a NodeImages<'a>) -> Self {
        CpuPainter {
            fonts,
            images: Some(images),
            z_index: None,
            clip: None,
            captions: None,
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
            captions: None,
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
        let empty_cap = CaptionMap::new();
        let groups = DisplayList::layered_groups(
            root,
            self.images.unwrap_or(&empty),
            self.z_index.unwrap_or(&empty_z),
            self.clip.unwrap_or(&empty_c),
            self.captions.unwrap_or(&empty_cap),
        );
        for g in &groups {
            // A group's clip is an `overflow` ancestor's box; shift it by the scroll.
            let clip = g.clip.map(|c| Rect {
                x: c.x,
                y: c.y - scroll_y,
                width: c.width,
                height: c.height,
            });
            // `backdrop-filter` runs BEFORE the element's own content: it filters what is already
            // on the canvas, in place, and the element then paints over the result.
            if !g.backdrop.is_empty() {
                self.filter_the_backdrop(&mut pixmap, g, clip, scroll_y);
            }
            if g.filters.is_empty() && g.shapes.is_empty() && !g.blend.is_blending() {
                for item in &g.items {
                    self.draw_item(&mut pixmap, item, clip, 0.0, -scroll_y);
                }
                continue;
            }
            self.draw_filtered_group(&mut pixmap, g, clip, scroll_y);
        }

        Canvas { pixmap }
    }

    /// Paint one `filter`ed group: rasterize its items into a transparent offscreen surface, run the
    /// filter pipeline over that surface, then composite the result back.
    ///
    /// The surface is sized to the group's own ink box (grown for blur bleed and clamped to the
    /// canvas), **not** to the viewport — a page with fifty drop-shadowed icons must not pay fifty
    /// full-screen buffers. Everything is translated so the box's top-left is the surface origin,
    /// which is the only reason `draw_item` takes an offset pair instead of just the scroll.
    ///
    /// `clip-path` rides the same path — it needs the identical offscreen surface, and giving it
    /// its own would mean two round-trips for an element that has both.
    fn draw_filtered_group(
        &self,
        pixmap: &mut tiny_skia::Pixmap,
        g: &PaintGroup,
        clip: Option<Rect>,
        scroll_y: f32,
    ) {
        // The blur radii in the chain decide how far ink escapes the item bounds. Sum them: two
        // blurs in series spread further than either alone, and a drop-shadow also offsets.
        let mut bleed = 0.0f32;
        for f in &g.filters {
            match f {
                manuk_css::FilterOp::Blur(r) => bleed += r * 3.0,
                manuk_css::FilterOp::DropShadow { dx, dy, blur, .. } => {
                    bleed += dx.abs().max(dy.abs()) + blur * 3.0
                }
                _ => {}
            }
        }
        let Some(ink) = g.items.iter().map(item_bounds).reduce(|a, b| a.union(&b)) else {
            return;
        };
        // Page → device coordinates, grown by the bleed, then clamped to the canvas.
        let x0 = (ink.x - bleed).floor().max(0.0);
        let y0 = (ink.y - scroll_y - bleed).floor().max(0.0);
        let x1 = (ink.x + ink.width + bleed)
            .ceil()
            .min(pixmap.width() as f32);
        let y1 = (ink.y + ink.height - scroll_y + bleed)
            .ceil()
            .min(pixmap.height() as f32);
        if x1 <= x0 || y1 <= y0 {
            return; // entirely off-screen
        }
        let (sw, sh) = ((x1 - x0) as u32, (y1 - y0) as u32);
        let Some(mut scratch) = tiny_skia::Pixmap::new(sw.max(1), sh.max(1)) else {
            return; // absurd extent — drop the filter rather than abort the frame
        };
        // **Two different coordinate spaces arrive here and only one of them still owes the
        // scroll.** The ITEMS are in page space, so their offset carries both the scroll and the
        // shift that puts the group's top-left at the scratch origin. The CLIP was already
        // converted to device space by the caller, so it owes only the shift — adding the scroll a
        // second time would slide every `overflow: hidden` clip off a filtered element the moment
        // the page is scrolled, which is invisible in any gate that renders at scroll 0.
        let (dx, dy) = (-x0, -scroll_y - y0);
        let clip = clip.map(|c| Rect {
            x: c.x - x0,
            y: c.y - y0,
            width: c.width,
            height: c.height,
        });
        for item in &g.items {
            self.draw_item(&mut scratch, item, clip, dx, dy);
        }
        apply_filters(&mut scratch, &g.filters);
        // **`clip-path` runs AFTER `filter`, and the order is not arbitrary** (CSS Masking §:
        // filter, then clip, then mask, then opacity). Clipping first would let the blur smear
        // colour back across the edge the clip had just cut, which is the visible difference
        // between a hard-edged shape and a fuzzy one.
        for (shape, reference) in &g.shapes {
            // The reference box is in page space like the items, so it takes the same offset.
            let rb = Rect {
                x: reference.x + dx,
                y: reference.y + dy,
                width: reference.width,
                height: reference.height,
            };
            filters::apply_clip_shape(&mut scratch, shape, rb);
        }
        // **The composite back is where `mix-blend-mode` lives, and that answers the question t593
        // left open.** The backdrop a blend needs is exactly what is already on `pixmap` under this
        // group's box, and the group's own pixels are exactly what the offscreen surface holds. One
        // mechanism, built for `filter`, and the blend is a field on the paint.
        pixmap.draw_pixmap(
            x0 as i32,
            y0 as i32,
            scratch.as_ref(),
            &tiny_skia::PixmapPaint {
                blend_mode: blend_mode(g.blend),
                ..Default::default()
            },
            tiny_skia::Transform::identity(),
            None,
        );
    }

    /// `backdrop-filter` — filter what is ALREADY PAINTED behind this box, in place.
    ///
    /// This is the one member of the visual-effects bundle that needed genuinely new code rather
    /// than a new field, and the reason is worth stating: every other property here operates on the
    /// element's own pixels, which the offscreen group already separates out. This one operates on
    /// the pixels the element is about to cover. So it reads the canvas region back, filters that
    /// copy, and writes it down again with `Source` (a replace, not a blend) — after which the
    /// normal group path paints the element on top exactly as before.
    ///
    /// It is confined to the box's own border box, which is why `PaintGroup` carries `bounds`: the
    /// blur must stop at the frosted panel's edge, not bleed across the page.
    fn filter_the_backdrop(
        &self,
        pixmap: &mut tiny_skia::Pixmap,
        g: &PaintGroup,
        clip: Option<Rect>,
        scroll_y: f32,
    ) {
        let mut r = Rect {
            x: g.bounds.x,
            y: g.bounds.y - scroll_y,
            width: g.bounds.width,
            height: g.bounds.height,
        };
        if let Some(cl) = clip {
            r = r.intersect(&cl);
        }
        let x0 = r.x.floor().max(0.0);
        let y0 = r.y.floor().max(0.0);
        let x1 = (r.x + r.width).ceil().min(pixmap.width() as f32);
        let y1 = (r.y + r.height).ceil().min(pixmap.height() as f32);
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let Some(irect) =
            tiny_skia::IntRect::from_xywh(x0 as i32, y0 as i32, (x1 - x0) as u32, (y1 - y0) as u32)
        else {
            return;
        };
        let Some(mut back) = pixmap.clone_rect(irect) else {
            return;
        };
        apply_filters(&mut back, &g.backdrop);
        pixmap.draw_pixmap(
            x0 as i32,
            y0 as i32,
            back.as_ref(),
            &tiny_skia::PixmapPaint {
                // REPLACE, not composite: the filtered copy IS the backdrop now. Compositing it
                // source-over its own unfiltered original would leave the sharp version showing
                // through wherever the filter reduced alpha.
                blend_mode: tiny_skia::BlendMode::Source,
                ..Default::default()
            },
            tiny_skia::Transform::identity(),
            None,
        );
    }

    /// Rasterize one display item into `pixmap`, translated by `(dx, dy)` device px. `clip` is
    /// already in the destination surface's coordinates.
    fn draw_item(
        &self,
        pixmap: &mut tiny_skia::Pixmap,
        item: &DisplayItem,
        clip: Option<Rect>,
        dx: f32,
        dy: f32,
    ) {
        let shift = |rect: &Rect| Rect {
            x: rect.x + dx,
            y: rect.y + dy,
            width: rect.width,
            height: rect.height,
        };
        match item {
            DisplayItem::Rect { rect, color } => {
                let mut r = shift(rect);
                if let Some(cl) = clip {
                    r = r.intersect(&cl);
                }
                fill_rect(pixmap, r, *color);
            }
            DisplayItem::RoundRect {
                rect,
                color,
                radius,
            } => fill_round_rect(pixmap, shift(rect), *color, *radius, clip),
            DisplayItem::Shadow {
                rect,
                color,
                radius,
                blur,
            } => fill_shadow(pixmap, shift(rect), *color, *radius, *blur, clip),
            DisplayItem::Text {
                x,
                baseline,
                text,
                style,
            } => self.draw_text(pixmap, *x + dx, *baseline + dy, text, style, clip),
            DisplayItem::Image {
                rect,
                image,
                content_clip,
            } => {
                // `object-fit: cover`/`none` may paint the bitmap larger than its box; crop
                // the overflow to the content box, intersected with any ancestor overflow clip.
                let eff_clip = match content_clip {
                    Some(cc) => {
                        let cc = shift(cc);
                        Some(clip.map_or(cc, |a| a.intersect(&cc)))
                    }
                    None => clip,
                };
                blit_image(pixmap, image, shift(rect), eff_clip);
            }
            DisplayItem::MaskedRect { rect, color, mask } => {
                blit_masked(pixmap, mask, *color, shift(rect), clip)
            }
            DisplayItem::BackgroundImage {
                rect,
                image,
                size,
                repeat,
                position,
                radius,
            } => blit_background(
                pixmap,
                image,
                shift(rect),
                *size,
                *repeat,
                *position,
                *radius,
                clip,
            ),
            DisplayItem::Gradient {
                rect,
                stops,
                angle_deg,
                radial,
                radius,
            } => fill_gradient(
                pixmap,
                shift(rect),
                stops,
                *angle_deg,
                *radial,
                *radius,
                clip,
            ),
            DisplayItem::TextLine {
                x,
                y,
                width,
                thickness,
                color,
            } => {
                let mut r = Rect {
                    x: *x + dx,
                    y: *y + dy,
                    width: *width,
                    height: *thickness,
                };
                if let Some(cl) = clip {
                    r = r.intersect(&cl);
                }
                fill_rect(pixmap, r, *color);
            }
        }
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
        // **`font-size: 0` renders NOTHING.** Not "a very small glyph" — nothing.
        //
        // Asked to rasterize at 0px, swash falls back to the face's *unscaled* outline, in font
        // units, and hands back a bitmap of 1,000-1,500px per glyph. `blit_glyph` then floods every
        // one of those pixels with the run's text colour. A single `font-size: 0` word painted a
        // page-sized blob of flat colour over the content — on old.reddit.com, ~27,000px of #888888
        // squarely on top of the post titles.
        //
        // And `font-size: 0` is not exotic. It is one of the most common tricks on the web: killing
        // the whitespace gap between `inline-block`s, and image-replacement
        // (`text-indent: -9999px; font-size: 0`) for logos and icon buttons. Every site that uses it
        // was getting glyph-shaped continents painted across the viewport.
        if style.font_size < 0.5 {
            return;
        }
        let run = self
            .fonts
            .shape_bidi(text, style.font_key, style.font_size, style.rtl);
        // Blit every glyph of the run at (origin + offset) in `color`. Called once for the
        // `text-shadow` pass (offset, shadow colour) and once for the text itself.
        let mut paint_run =
            |pixmap: &mut tiny_skia::Pixmap, off_x: f32, off_y: f32, color: Rgba| {
                for g in &run.glyphs {
                    let pen_x = origin_x + off_x + g.x;
                    // swash rasterizes at the fractional pen position for crisp subpixel placement.
                    let Some(bitmap) =
                        self.fonts
                            .rasterize(g.glyph_id, g.face, style.font_size, pen_x)
                    else {
                        continue;
                    };
                    if bitmap.width == 0 || bitmap.height == 0 {
                        continue; // whitespace and zero-area glyphs
                    }
                    // ⚠⚠ **A SIDEWAYS RUN: THE PEN RUNS DOWN THE PAGE AND EACH GLYPH LIES ON ITS
                    // SIDE.** `origin_x` is the pen's starting **y** and `baseline` is the
                    // baseline's **x** (see `TextStyle::sideways`), so the same two swash offsets
                    // are applied to the axes a ninety-degree clockwise turn maps them onto:
                    // `left` (pen→bitmap-left) walks DOWN, and `top` (baseline→bitmap-top, upward)
                    // becomes a distance to the RIGHT — which is why the ascent ends up on the +x
                    // side, exactly as Chrome renders both `vertical-rl` and `vertical-lr`.
                    if style.sideways {
                        let rotated = rotate_glyph_cw(&bitmap);
                        let left =
                            (baseline + off_y).round() as i32 + bitmap.top - rotated.width as i32;
                        let top = pen_x.floor() as i32 + bitmap.left;
                        blit_glyph(pixmap, &rotated, left, top, color, clip);
                        continue;
                    }
                    // swash placement: `left` = pen→bitmap-left, `top` = baseline→bitmap-top (up).
                    let left = pen_x.floor() as i32 + bitmap.left;
                    let top = (baseline + off_y).round() as i32 - bitmap.top;
                    blit_glyph(pixmap, &bitmap, left, top, color, clip);
                }
            };
        // `text-shadow` paints a second, offset copy of the glyphs BEHIND the text. Blur is residue —
        // a hard-edged offset copy is the honest first approximation and already restores readability
        // of hero/heading text over a busy background.
        if let Some(sh) = style.shadow {
            paint_run(pixmap, sh.dx, sh.dy, sh.color);
        }
        paint_run(pixmap, 0.0, 0.0, style.color);
    }
}

/// **Turn a rasterized glyph ninety degrees CLOCKWISE**, for a run in a vertical writing mode.
///
/// `text-orientation: mixed` — the initial value, and the one every `writing-mode: vertical-*` page
/// gets unless it says otherwise — lays a *sideways* (non-ideographic) glyph on its side. Measured
/// in Chrome: `ab` in `vertical-rl` and in `vertical-lr` are rendered identically, both with the
/// glyph tops pointing RIGHT, so the turn is clockwise in both and only `sideways-lr` differs.
///
/// Rotating the coverage bitmap rather than the destination is deliberate: `blit_coverage` and
/// `blit_color_glyph` already handle clipping, tinting and the two pixel formats, and a second
/// blitter that walks the source transposed would be a second implementation of both. The cost is
/// one `width*height` copy per glyph, on the vanishingly small fraction of runs that are vertical.
///
/// `dst(dx, dy) = src(dy, h - 1 - dx)` — the standard CW map, applied per pixel for coverage and
/// per RGBA quad for a colour glyph.
fn rotate_glyph_cw(bmp: &manuk_text::GlyphBitmap) -> manuk_text::GlyphBitmap {
    let (w, h) = (bmp.width as usize, bmp.height as usize);
    let px = if bmp.is_color { 4 } else { 1 };
    let mut out = vec![0u8; w * h * px];
    // The rotated bitmap is `h` wide and `w` tall.
    for dx in 0..h {
        for dy in 0..w {
            let sx = dy;
            let sy = h - 1 - dx;
            let si = (sy * w + sx) * px;
            let di = (dy * h + dx) * px;
            out[di..di + px].copy_from_slice(&bmp.coverage[si..si + px]);
        }
    }
    manuk_text::GlyphBitmap {
        left: bmp.left,
        top: bmp.top,
        width: bmp.height,
        height: bmp.width,
        coverage: out,
        is_color: bmp.is_color,
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
pub(crate) fn round_rect_path(rect: Rect, radius: f32) -> Option<tiny_skia::Path> {
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
/// Compute where a decoded `img_w`×`img_h` bitmap is drawn inside `box_rect` under `object-fit`,
/// plus the crop box for any overflow. Returns `(destination_rect, content_clip)`: for `fill` the
/// image stretches to the box and nothing is cropped; the aspect-ratio-preserving modes center the
/// scaled image (`object-position: 50% 50%`, the default) and `cover`/`none` return the box as a
/// crop rect because the image can exceed it. Explicit `object-position` is not yet parsed.
fn object_fit_geometry(
    fit: manuk_css::ObjectFit,
    pos: manuk_css::ObjectPosition,
    box_rect: Rect,
    img_w: u32,
    img_h: u32,
) -> (Rect, Option<Rect>) {
    use manuk_css::ObjectFit;
    let (bw, bh) = (box_rect.width, box_rect.height);
    let (iw, ih) = (img_w as f32, img_h as f32);
    if iw <= 0.0 || ih <= 0.0 || bw <= 0.0 || bh <= 0.0 {
        return (box_rect, None);
    }
    let scale = match fit {
        ObjectFit::Fill => return (box_rect, None), // stretch to the box, ignore aspect ratio
        ObjectFit::Contain => (bw / iw).min(bh / ih),
        ObjectFit::Cover => (bw / iw).max(bh / ih),
        ObjectFit::None => 1.0,
        ObjectFit::ScaleDown => (bw / iw).min(bh / ih).min(1.0),
    };
    let (dw, dh) = (iw * scale, ih * scale);
    // `object-position` distributes the free space (which is negative — an overflow — for `cover`/
    // `none`) by the per-axis fraction: 0.5 centers (the default), 0 pins the start edge, 1 the end.
    let dest = Rect {
        x: box_rect.x + (bw - dw) * pos.x,
        y: box_rect.y + (bh - dh) * pos.y,
        width: dw,
        height: dh,
    };
    // cover / none can overflow the box → crop to it; contain / scale-down never do.
    let clip = if dw > bw + 0.5 || dh > bh + 0.5 {
        Some(box_rect)
    } else {
        None
    };
    (dest, clip)
}

fn blit_image(
    pixmap: &mut tiny_skia::Pixmap,
    image: &DecodedImage,
    rect: Rect,
    clip: Option<Rect>,
) {
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
    let rect =
        tiny_skia::Rect::from_xywh(clip.x, clip.y, clip.width.max(0.0), clip.height.max(0.0))?;
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

    /// Regression: **`font-size: 0` renders NOTHING** — not a tiny glyph, nothing.
    ///
    /// Asked to rasterize at 0px, swash falls back to the face's *unscaled* outline, in font units,
    /// and returns a bitmap of 1,000-1,500px **per glyph**. `blit_glyph` then floods every one of
    /// those pixels with the run's text colour. One `font-size: 0` word painted a page-sized
    /// continent of flat colour over the content — ~27,000px of #888888 sitting squarely on top of
    /// old.reddit.com's post titles.
    ///
    /// `font-size: 0` is not exotic. It is one of the most common tricks on the web: killing the
    /// whitespace gap between `inline-block`s, and image-replacement (`text-indent: -9999px;
    /// font-size: 0`) for logos and icon buttons.
    #[test]
    fn font_size_zero_paints_nothing_at_all() {
        let dom = manuk_html::parse(r#"<div id="hidden">Submit</div><p id="ok">visible</p>"#);
        let styles = MinimalCascade.cascade(
            &dom,
            &[Stylesheet::parse(
                "#hidden{font-size:0;color:#888888} #ok{font-size:16px}",
            )],
        );
        let fonts = FontContext::new();
        let root = manuk_layout::layout_document(&dom, &styles, &fonts, 400.0);
        let canvas = CpuPainter::new(&fonts).render(&root, 400, 200, Rgba::WHITE);

        // Nothing anywhere on the canvas may be the zero-sized run's colour. A single stray glyph
        // at unscaled em units is over a million pixels; there is no "a few is fine" here.
        let grey = canvas
            .rgba_bytes()
            .chunks_exact(4)
            .filter(|p| p[0] == 0x88 && p[1] == 0x88 && p[2] == 0x88)
            .count();
        assert_eq!(
            grey, 0,
            "a `font-size: 0` run painted {grey} pixels — swash rasterizes a 0px glyph from the \
             UNSCALED outline, so each one is a 1000px+ bitmap flood-filled with the text colour"
        );
        // And the guard must not have silenced ordinary text along with it.
        let inked = canvas
            .rgba_bytes()
            .chunks_exact(4)
            .filter(|p| p[0] < 250 || p[1] < 250 || p[2] < 250)
            .count();
        assert!(
            inked > 40,
            "the 16px paragraph must still paint; only {inked}px inked"
        );
    }

    /// `box-shadow` is a comma-separated LIST, and each layer carries a `spread`. Tailwind's
    /// elevation utilities (`shadow`, `shadow-md`, `shadow-lg`) all stack TWO layers, the second
    /// with a negative spread — so a single-shadow, spread-less model painted every one of them
    /// wrong: one layer, at the wrong size. An `inset`-only shadow honestly paints nothing (inner
    /// shadows are captured in the list but not yet rendered).
    #[test]
    fn box_shadow_is_a_list_with_spread() {
        let build = |css: &str| -> Vec<(Rect, f32)> {
            let dom = manuk_html::parse(r#"<div id="c">x</div>"#);
            let styles = MinimalCascade.cascade(&dom, &[Stylesheet::parse(css)]);
            let fonts = FontContext::new();
            let root = manuk_layout::layout_document(&dom, &styles, &fonts, 400.0);
            DisplayList::build(&root)
                .items
                .iter()
                .filter_map(|it| match it {
                    DisplayItem::Shadow { rect, blur, .. } => Some((*rect, *blur)),
                    _ => None,
                })
                .collect()
        };

        // Two comma-separated layers → TWO Shadow items. The old single-shadow model emitted one.
        let two = build(
            "#c{width:100px;height:40px;box-shadow:0 4px 6px -1px #000, 0 2px 4px -2px #000}",
        );
        assert_eq!(
            two.len(),
            2,
            "a two-layer box-shadow must emit two Shadow items; got {two:?}"
        );

        // `spread` inflates the shadow rect: +10px on a 100×40 box → 120×60 (before offset/blur).
        let spread = build("#c{width:100px;height:40px;box-shadow:0 0 5px 10px #000}");
        assert_eq!(spread.len(), 1, "one shadow expected; got {spread:?}");
        assert!(
            (spread[0].0.width - 120.0).abs() < 0.01 && (spread[0].0.height - 60.0).abs() < 0.01,
            "box-shadow spread:10px must inflate the 100×40 shadow rect to 120×60; got {:?}",
            spread[0].0
        );

        // An `inset`-only shadow paints no outer Shadow item (honest — inner painting not built).
        let inset = build("#c{width:100px;height:40px;box-shadow:inset 0 2px 4px #000}");
        assert_eq!(
            inset.len(),
            0,
            "an inset-only box-shadow must paint no outer Shadow item; got {inset:?}"
        );
    }

    /// Regression: **an anonymous box must inherit its ancestors' stacking layer and clip.**
    ///
    /// `z` and `clip` are looked up by NodeId, and a box the layout engine synthesised has no node —
    /// so it got `z = 0` and no clip regardless of what stacking context it was actually inside. That
    /// is not a corner case: the anonymous box is where the TEXT lives. A `z-index`'d ancestor put
    /// its own background in layer 1 while the anonymous box holding its text stayed in layer 0, so
    /// the background sorted AFTER the text and painted straight over it.
    ///
    /// old.reddit.com's post titles were laid out at the right place, in the right colour, at full
    /// alpha, and present in the display list — and buried under their own ancestor's background.
    /// Every geometry probe called it perfect.
    #[test]
    fn an_anonymous_box_inherits_its_ancestors_stacking_layer() {
        // **MIXED inline + block content** is what makes the layout engine synthesise an anonymous
        // box (`flush_inline_run`) to hold the inline run. A block whose children are all inline
        // does not, and a test written that way cannot fail — I wrote two of those first, and both
        // passed with the bug deliberately reintroduced.
        let dom = manuk_html::parse(
            r##"<div id="card">Title<a id="lnk" href="#x"> link</a><div id="blk">block</div></div>"##,
        );
        let styles = MinimalCascade.cascade(
            &dom,
            &[Stylesheet::parse(
                "#card{position:relative;z-index:1;background:#fff;width:200px}",
            )],
        );
        let fonts = FontContext::new();
        let root = manuk_layout::layout_document(&dom, &styles, &fonts, 400.0);

        let node = |id: &str| {
            dom.descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .unwrap()
        };
        let mut z = std::collections::HashMap::new();
        // What the page layer computes: the z-index'd element applies its layer to its whole subtree.
        z.insert(node("card"), 1);
        z.insert(node("lnk"), 1);
        z.insert(node("blk"), 1);

        let groups = DisplayList::layered_groups(
            &root,
            &std::collections::HashMap::new(),
            &z,
            &std::collections::HashMap::new(),
            &CaptionMap::new(),
        );
        // Find the layer of the group that actually carries the text.
        let text_layer = groups
            .iter()
            .find(|g| {
                g.items
                    .iter()
                    .any(|i| matches!(i, DisplayItem::Text { text, .. } if text.contains("Title")))
            })
            .map(|g| g.z)
            .expect("the text must be somewhere in the display list");
        let card_layer = groups
            .iter()
            .find(|g| {
                g.items
                    .iter()
                    .any(|i| matches!(i, DisplayItem::Rect { .. }))
            })
            .map(|g| g.z)
            .expect("the card background");

        assert_eq!(
            text_layer, 1,
            "the anonymous box holding the text must be in its ancestor's layer (1), not stranded in \
             layer 0 — in layer 0 it sorts BEFORE the card's background and gets painted over"
        );
        assert!(
            text_layer >= card_layer,
            "text (layer {text_layer}) must not sort below the background of its own ancestor \
             (layer {card_layer})"
        );
    }

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
            &[Stylesheet::parse(
                "#d{width:300px;height:120px;background-image:url(t.png)}",
            )],
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
            std::rc::Rc::new(DecodedImage {
                width: 40,
                height: 30,
                rgba: vec![255; 40 * 30 * 4],
            }),
        );

        let items = DisplayList::build_with_images(&root, &images).items;
        let backgrounds = items
            .iter()
            .filter(|i| matches!(i, DisplayItem::BackgroundImage { .. }))
            .count();
        let replaced = items
            .iter()
            .filter(|i| matches!(i, DisplayItem::Image { .. }))
            .count();

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

    /// `text-shadow` paints a second, offset copy of the glyphs behind the text — the readability
    /// treatment on hero/heading text over a busy or light background. Baseline: text-shadow was
    /// unimplemented, so a white heading with `text-shadow:...black` painted only the (invisible on a
    /// light page) white text. Falsifiable: white text on a white canvas with a BLACK shadow must
    /// produce dark pixels that the same text without a shadow does not.
    #[test]
    fn text_shadow_paints_behind_the_glyphs() {
        use manuk_css::{MinimalCascade, Stylesheet};
        let dark_px = |extra: &str| -> usize {
            let dom = manuk_html::parse(r#"<p id="t">Hi</p>"#);
            let css = format!("#t{{color:white;font-size:48px;{extra}}}");
            let styles = MinimalCascade.cascade(&dom, &[Stylesheet::parse(&css)]);
            let fonts = FontContext::new();
            let root = manuk_layout::layout_document(&dom, &styles, &fonts, 400.0);
            let canvas = CpuPainter::new(&fonts).render(&root, 400, 120, Rgba::WHITE);
            canvas
                .rgba_bytes()
                .chunks_exact(4)
                .filter(|p| p[0] < 90 && p[1] < 90 && p[2] < 90)
                .count()
        };
        let none = dark_px("");
        let shadowed = dark_px("text-shadow: 4px 4px 0 black");
        assert!(
            none < 10,
            "white text on a white canvas should paint ~no dark pixels ({none})"
        );
        assert!(
            shadowed > 60,
            "a black text-shadow must paint the glyph outline in dark pixels ({shadowed}) — was {none} without it"
        );
    }

    /// `border-style` — dashed/dotted/double borders must render as broken/paired lines, not solid.
    /// Baseline: the painter drew every edge as one solid Rect regardless of style, so a drop-zone's
    /// dashed outline, a ticket card's perforation and a `double` frame all came out solid. A plain
    /// bordered `<div>` (no background) emits exactly one Rect per non-zero edge, so counting Rects
    /// distinguishes the styles: solid=4 (one/edge), double=8 (two/edge), dashed/dotted≫4 (segments).
    #[test]
    fn border_style_breaks_the_line() {
        use manuk_css::{MinimalCascade, Stylesheet};
        let rects = |style: &str| -> usize {
            let dom = manuk_html::parse(r#"<div id="d"></div>"#);
            let css = format!("#d{{width:120px;height:60px;border:9px {style} #333}}");
            let styles = MinimalCascade.cascade(&dom, &[Stylesheet::parse(&css)]);
            let fonts = FontContext::new();
            let root = manuk_layout::layout_document(&dom, &styles, &fonts, 400.0);
            let images = std::collections::HashMap::new();
            DisplayList::build_with_images(&root, &images)
                .items
                .iter()
                .filter(|i| matches!(i, DisplayItem::Rect { .. }))
                .count()
        };
        assert_eq!(rects("solid"), 4, "a solid border is one Rect per edge (4)");
        assert_eq!(
            rects("double"),
            8,
            "a double border is TWO lines per edge (8) — was 4 solid"
        );
        assert!(
            rects("dashed") > 8,
            "a dashed border breaks each edge into multiple segments — was 4 solid ({} rects)",
            rects("dashed")
        );
        assert!(
            rects("dotted") > 8,
            "a dotted border is a run of dots per edge — was 4 solid ({} rects)",
            rects("dotted")
        );
    }

    /// `background-position` places a `no-repeat` image where the design put it, instead of always at
    /// the box's top-left corner — the icon/logo/sprite idiom (`url(sprite.png) no-repeat;
    /// background-position: -16px -48px` / `center` / `right bottom`). Baseline: the fixed-origin blit
    /// only ever painted the default `0% 0%`, so a positioned image showed the wrong slice / sat jammed
    /// in the corner.
    #[test]
    fn background_position_places_the_image() {
        use manuk_css::{BackgroundPosition, BackgroundRepeat, BackgroundSize, BgPos};
        // A 20×20 fully-opaque red tile.
        let mut rgba = Vec::with_capacity(20 * 20 * 4);
        for _ in 0..(20 * 20) {
            rgba.extend_from_slice(&[255, 0, 0, 255]);
        }
        let img = DecodedImage {
            width: 20,
            height: 20,
            rgba,
        };
        let paint = |pos: BackgroundPosition| -> tiny_skia::Pixmap {
            let mut pm = tiny_skia::Pixmap::new(100, 100).unwrap();
            blit_background(
                &mut pm,
                &img,
                Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 100.0,
                    height: 100.0,
                },
                BackgroundSize::Auto,
                BackgroundRepeat::NoRepeat,
                pos,
                0.0,
                None,
            );
            pm
        };
        let is_red = |pm: &tiny_skia::Pixmap, x: u32, y: u32| {
            let p = pm.pixel(x, y).unwrap();
            p.alpha() > 200 && p.red() > 200 && p.green() < 50
        };

        // Default `0% 0%`: the historic top-left blit — byte-for-byte the old behaviour.
        let tl = paint(BackgroundPosition::default());
        assert!(is_red(&tl, 5, 5), "default `0% 0%` must paint the top-left");
        assert!(
            !is_red(&tl, 95, 95),
            "default `0% 0%` must leave the bottom-right corner empty (20×20 tile in a 100×100 box)"
        );

        // `right bottom` (`Pct(1,1)`): free space 80 on each axis, so the tile occupies [80,100).
        let br = paint(BackgroundPosition {
            x: BgPos::Pct(1.0),
            y: BgPos::Pct(1.0),
        });
        assert!(
            is_red(&br, 95, 95),
            "`right bottom` must place the image in the bottom-right corner"
        );
        assert!(
            !is_red(&br, 5, 5),
            "`right bottom` must move the image OFF the top-left origin"
        );

        // A `<length>` is an absolute offset: `50px 50px` places the tile at [50,70).
        let px = paint(BackgroundPosition {
            x: BgPos::Px(50.0),
            y: BgPos::Px(50.0),
        });
        assert!(is_red(&px, 60, 60), "a px offset places the sprite slice");
        assert!(
            !is_red(&px, 5, 5),
            "a px offset moves the image off the origin"
        );
    }

    /// `object-fit` — a 2:1 photo in a 1:1 tile must NOT distort. This is the near-universal thumbnail
    /// idiom (`img { width:100%; height:100%; object-fit:cover }` in a card grid). Baseline: the
    /// replaced blit stretched the bitmap to the box, so a 200×100 photo in a 100×100 tile came out
    /// squashed to 1:1. Now `cover` scales the bitmap preserving aspect ratio to COVER the box (dest
    /// 200×100, cropped to the 100×100 box); `contain` scales it to FIT inside (dest 100×50,
    /// letterboxed, no crop).
    #[test]
    fn object_fit_preserves_aspect_ratio() {
        // A helper: the single DisplayItem::Image produced for `<img id=p>` under `fit`.
        fn image_item(fit: &str) -> (Rect, Option<Rect>) {
            let dom = manuk_html::parse(r#"<img id="p">"#);
            let css = format!("#p{{width:100px;height:100px;object-fit:{fit}}}");
            let styles = MinimalCascade.cascade(&dom, &[Stylesheet::parse(&css)]);
            let fonts = FontContext::new();
            let root = manuk_layout::layout_document(&dom, &styles, &fonts, 400.0);
            let node = dom
                .descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some("p"))
                .expect("the img");
            let mut images = std::collections::HashMap::new();
            images.insert(
                node,
                std::rc::Rc::new(DecodedImage {
                    width: 200, // a 2:1 photo
                    height: 100,
                    rgba: vec![255; 200 * 100 * 4],
                }),
            );
            let items = DisplayList::build_with_images(&root, &images).items;
            items
                .iter()
                .find_map(|i| match i {
                    DisplayItem::Image {
                        rect, content_clip, ..
                    } => Some((*rect, *content_clip)),
                    _ => None,
                })
                .expect("a replaced-image display item")
        }

        // fill (default): the bitmap stretches to the box — the historical behaviour, unchanged.
        let (fill, fill_clip) = image_item("fill");
        assert!(
            (fill.width - 100.0).abs() < 0.5 && (fill.height - 100.0).abs() < 0.5,
            "object-fit:fill stretches to the box (100×100), got {}×{}",
            fill.width,
            fill.height
        );
        assert!(fill_clip.is_none(), "fill never overflows, so no crop");

        // cover: aspect ratio preserved, scaled to cover the box → 200×100, cropped to the tile.
        let (cover, cover_clip) = image_item("cover");
        assert!(
            (cover.width - 200.0).abs() < 0.5 && (cover.height - 100.0).abs() < 0.5,
            "object-fit:cover keeps 2:1 and covers the box (dest 200×100), got {}×{} — a stretched \
             baseline would report 100×100",
            cover.width,
            cover.height
        );
        let cc = cover_clip.expect("cover crops the overflow to the box");
        assert!(
            (cc.width - 100.0).abs() < 0.5 && (cc.height - 100.0).abs() < 0.5,
            "the crop box is the 100×100 tile, got {}×{}",
            cc.width,
            cc.height
        );

        // contain: aspect ratio preserved, scaled to fit inside → 100×50, letterboxed, no crop.
        let (contain, contain_clip) = image_item("contain");
        assert!(
            (contain.width - 100.0).abs() < 0.5 && (contain.height - 50.0).abs() < 0.5,
            "object-fit:contain keeps 2:1 and fits inside the box (dest 100×50), got {}×{}",
            contain.width,
            contain.height
        );
        assert!(
            contain_clip.is_none(),
            "contain fits inside the box, so nothing is cropped"
        );
    }

    /// `object-position` slides the fitted image within its box. For `object-fit:cover` a 2:1 photo in
    /// a square tile overflows horizontally by 100px, and `object-position` picks which 100px slice
    /// shows: `left` pins the image's left edge to the box, `right` the right edge, the default
    /// centers. Baseline (tick 181): the position was hardcoded to center, so a subject at the top/side
    /// of a cropped hero was always cut off.
    #[test]
    fn object_position_places_cropped_image() {
        fn dest_x(pos: &str) -> f32 {
            let dom = manuk_html::parse(r#"<img id="p">"#);
            let css =
                format!("#p{{width:100px;height:100px;object-fit:cover;object-position:{pos}}}");
            let styles = MinimalCascade.cascade(&dom, &[Stylesheet::parse(&css)]);
            let fonts = FontContext::new();
            let root = manuk_layout::layout_document(&dom, &styles, &fonts, 400.0);
            let node = dom
                .descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some("p"))
                .expect("the img");
            let mut images = std::collections::HashMap::new();
            images.insert(
                node,
                std::rc::Rc::new(DecodedImage {
                    width: 200,
                    height: 100,
                    rgba: vec![255; 200 * 100 * 4],
                }),
            );
            let items = DisplayList::build_with_images(&root, &images).items;
            items
                .iter()
                .find_map(|i| match i {
                    DisplayItem::Image { rect, .. } => Some(rect.x),
                    _ => None,
                })
                .expect("a replaced-image display item")
        }
        // The 200px image overflows the 100px box by 100px: `left` pins at box.x, `center` is 50px
        // further left, `right` 100px further left. Measured relative so box.x need not be known.
        let left = dest_x("left");
        let center = dest_x("50% 50%");
        let right = dest_x("right");
        assert!(
            (center - (left - 50.0)).abs() < 0.5,
            "centered sits 50px left of the left-pinned image ({left} vs {center})"
        );
        assert!(
            (right - (left - 100.0)).abs() < 0.5,
            "object-position:right pins the right edge (100px left of the left-pin) ({left} vs {right})"
        );
        assert!(
            (dest_x("0% 50%") - left).abs() < 0.5,
            "the keyword `left` and `0%` resolve to the same edge"
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
            rect: Rect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
            color: Rgba::new(255, 0, 0, 255),
        };
        let blue = DisplayItem::Rect {
            rect: Rect {
                x: 100.0,
                y: 100.0,
                width: 20.0,
                height: 20.0,
            },
            color: Rgba::new(0, 0, 255, 255),
        };
        let a = DisplayList {
            items: vec![red.clone(), blue.clone()],
        };
        let b = DisplayList {
            items: vec![red.clone(), blue.clone()],
        };
        // Identical lists → no change, no damage (idle frame skips re-upload).
        assert!(!a.changed_since(&b));
        assert_eq!(a.damage_since(&b), None);

        // Change the second item's color → changed, and the damage covers its rect.
        let blue2 = DisplayItem::Rect {
            rect: Rect {
                x: 100.0,
                y: 100.0,
                width: 20.0,
                height: 20.0,
            },
            color: Rgba::new(0, 200, 0, 255),
        };
        let c = DisplayList {
            items: vec![red, blue2],
        };
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

    /// `text-decoration-color` paints the line in its own color, not the text color.
    ///
    /// A colored underline (brand links, hover states, a strikethrough price in a distinct hue) is
    /// everywhere in modern design. Before this, every decoration line was drawn in `f.style.color`,
    /// so `text-decoration-color:red` on blue text drew a *blue* underline — the wrong color on any
    /// link whose underline was meant to contrast with its text.
    #[test]
    fn text_decoration_color_overrides_text_color() {
        let dom = manuk_html::parse(r#"<p class="l">link</p>"#);
        let fonts = FontContext::new();

        // A red underline under blue text: the TextLine must be red, and no TextLine may be blue.
        let styles = MinimalCascade.cascade(
            &dom,
            &[Stylesheet::parse(
                ".l{color:#0000ff;text-decoration:underline;text-decoration-color:#ff0000}",
            )],
        );
        let root = manuk_layout::layout_document(&dom, &styles, &fonts, 400.0);
        let lines: Vec<Rgba> = DisplayList::build(&root)
            .items
            .iter()
            .filter_map(|it| match it {
                DisplayItem::TextLine { color, .. } => Some(*color),
                _ => None,
            })
            .collect();
        assert!(
            !lines.is_empty(),
            "the underline must reach the display list as a TextLine"
        );
        assert!(
            lines.iter().any(|c| *c == Rgba::new(255, 0, 0, 255)),
            "text-decoration-color:red must paint a RED underline; got {lines:?}"
        );
        assert!(
            lines.iter().all(|c| *c != Rgba::new(0, 0, 255, 255)),
            "no underline may be painted in the blue TEXT color once a decoration color is set"
        );

        // Control: with no text-decoration-color, the line follows the text color (blue).
        let styles2 = MinimalCascade.cascade(
            &dom,
            &[Stylesheet::parse(
                ".l{color:#0000ff;text-decoration:underline}",
            )],
        );
        let root2 = manuk_layout::layout_document(&dom, &styles2, &fonts, 400.0);
        let has_blue = DisplayList::build(&root2).items.iter().any(|it| {
            matches!(it, DisplayItem::TextLine { color, .. } if *color == Rgba::new(0, 0, 255, 255))
        });
        assert!(
            has_blue,
            "without a decoration color, the underline must default to the blue currentColor"
        );
    }

    /// `text-decoration-thickness` sets the line thickness and `text-underline-offset` pushes the
    /// underline down — both are Tailwind staples (`decoration-2`, `underline-offset-4`).
    ///
    /// Before this, thickness was hardcoded to `font_size / 14` and the underline sat at a fixed
    /// offset, so `decoration-2` on a small font drew a hairline and `underline-offset-*` did
    /// nothing — the underline crowded the text on every design that asked for breathing room.
    #[test]
    fn text_decoration_thickness_and_offset_shape_the_underline() {
        let dom = manuk_html::parse(r#"<p class="l">link</p>"#);
        let fonts = FontContext::new();

        let extract = |css: &str| -> Vec<(f32, f32)> {
            let styles = MinimalCascade.cascade(&dom, &[Stylesheet::parse(css)]);
            let root = manuk_layout::layout_document(&dom, &styles, &fonts, 400.0);
            DisplayList::build(&root)
                .items
                .iter()
                .filter_map(|it| match it {
                    DisplayItem::TextLine { y, thickness, .. } => Some((*y, *thickness)),
                    _ => None,
                })
                .collect()
        };

        // Baseline: a plain underline at font-size 14 → auto thickness = 14/14 = 1px.
        let base = extract(".l{font-size:14px;text-decoration:underline}");
        assert_eq!(base.len(), 1, "one underline expected; got {base:?}");
        let (base_y, base_thick) = base[0];
        assert!(
            (base_thick - 1.0).abs() < 0.01,
            "auto thickness at 14px must be ~1px; got {base_thick}"
        );

        // text-decoration-thickness:6px → a 6px line, regardless of the tiny font.
        let thick =
            extract(".l{font-size:14px;text-decoration:underline;text-decoration-thickness:6px}");
        assert!(
            thick.iter().any(|(_, t)| (*t - 6.0).abs() < 0.01),
            "text-decoration-thickness:6px must paint a 6px line; got {thick:?}"
        );

        // text-underline-offset:8px → the SAME thickness, but the line sits 8px lower.
        let offset =
            extract(".l{font-size:14px;text-decoration:underline;text-underline-offset:8px}");
        assert_eq!(offset.len(), 1, "one underline expected; got {offset:?}");
        let (off_y, _) = offset[0];
        assert!(
            (off_y - (base_y + 8.0)).abs() < 0.01,
            "text-underline-offset:8px must push the underline 8px below the default \
             (base y {base_y}, offset y {off_y})"
        );
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
        let red = Rgba {
            r: 255,
            g: 0,
            b: 0,
            a: 255,
        };
        fill_round_rect(
            &mut pm,
            Rect {
                x: 0.0,
                y: 0.0,
                width: 50.0,
                height: 50.0,
            },
            red,
            20.0,
            None,
        );
        let alpha = |x: u32, y: u32| pm.data()[((y * 50 + x) * 4 + 3) as usize];
        assert_eq!(alpha(25, 25), 255, "the centre is filled");
        assert_eq!(alpha(0, 0), 0, "the corner is cut away by the 20px radius");
        assert_eq!(alpha(49, 0), 0, "…on every corner");
        assert_eq!(
            alpha(25, 0),
            255,
            "but the straight top edge is still filled"
        );
    }

    /// An outer `box-shadow` paints *outside* the box (softened over `blur`), and nothing at all
    /// when the shadow colour is transparent.
    #[test]
    fn box_shadow_paints_outside_the_box() {
        let mut pm = tiny_skia::Pixmap::new(60, 60).expect("pixmap");
        let black = Rgba {
            r: 0,
            g: 0,
            b: 0,
            a: 200,
        };
        // A 20x20 box at (20,20), shadow blurred 8px: pixels just outside it get some alpha.
        fill_shadow(
            &mut pm,
            Rect {
                x: 20.0,
                y: 20.0,
                width: 20.0,
                height: 20.0,
            },
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
            let Some(&ma) = mask.rgba.get(mi + 3) else {
                continue;
            };
            if ma == 0 {
                continue;
            }
            let a = (ma as f32 / 255.0) * (color.a as f32 / 255.0);
            if a <= 0.002 {
                continue;
            }
            let di = (py * pw + px) as usize;
            let Some(dst) = data.get_mut(di) else {
                continue;
            };
            // Source-over, on premultiplied storage.
            let inv = 1.0 - a;
            let blend = |s: u8, d: u8| -> u8 {
                ((s as f32 * a) + (d as f32 * inv))
                    .round()
                    .clamp(0.0, 255.0) as u8
            };
            let (r, g, b) = (
                blend(color.r, dst.red()),
                blend(color.g, dst.green()),
                blend(color.b, dst.blue()),
            );
            let na = ((a * 255.0) + (dst.alpha() as f32 * inv))
                .round()
                .clamp(0.0, 255.0) as u8;
            if let Some(p) =
                tiny_skia::PremultipliedColorU8::from_rgba(r.min(na), g.min(na), b.min(na), na)
            {
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
                let lerp = |x: u8, y: u8| {
                    (x as f32 + (y as f32 - x as f32) * f)
                        .round()
                        .clamp(0.0, 255.0) as u8
                };
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
            let Some(dst) = data.get_mut(di) else {
                continue;
            };
            let inv = 1.0 - al;
            let blend = |s: u8, d: u8| {
                ((s as f32 * al) + (d as f32 * inv))
                    .round()
                    .clamp(0.0, 255.0) as u8
            };
            let na = ((al * 255.0) + (dst.alpha() as f32 * inv))
                .round()
                .clamp(0.0, 255.0) as u8;
            let (rr, gg, bb) = (
                blend(c.r, dst.red()),
                blend(c.g, dst.green()),
                blend(c.b, dst.blue()),
            );
            if let Some(p) =
                tiny_skia::PremultipliedColorU8::from_rgba(rr.min(na), gg.min(na), bb.min(na), na)
            {
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
#[allow(clippy::too_many_arguments)]
fn blit_background(
    pixmap: &mut tiny_skia::Pixmap,
    img: &DecodedImage,
    rect: Rect,
    size: manuk_css::BackgroundSize,
    repeat: manuk_css::BackgroundRepeat,
    position: manuk_css::BackgroundPosition,
    radius: f32,
    clip: Option<Rect>,
) {
    use manuk_css::{BackgroundRepeat as R, BackgroundSize as S, BgPos};
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
    // `background-position`: a percentage/keyword aligns the p-point of the image with the p-point of
    // the box (an offset over the FREE space, `box − tile`); a length is an absolute offset. Default
    // `0% 0%` gives offset 0 on both axes — the historic top-left blit, byte-for-byte.
    let resolve = |p: BgPos, free: f32| match p {
        BgPos::Pct(f) => f * free,
        BgPos::Px(px) => px,
    };
    let off_x = resolve(position.x, rect.width - tw);
    let off_y = resolve(position.y, rect.height - th);

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
            // Position within the tile, measured from the box's origin, shifted by
            // `background-position` (so the image starts at `origin + offset`).
            let mut lx = fx - rect.x - off_x;
            let mut ly = fy - rect.y - off_y;
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
            let Some(px4) = img.rgba.get(si..si + 4) else {
                continue;
            };
            let a = px4[3] as f32 / 255.0;
            if a <= 0.002 {
                continue;
            }
            let di = (py * pw + px) as usize;
            let Some(dst) = data.get_mut(di) else {
                continue;
            };
            let inv = 1.0 - a;
            let blend = |s: u8, d: u8| {
                ((s as f32 * a) + (d as f32 * inv))
                    .round()
                    .clamp(0.0, 255.0) as u8
            };
            let na = ((a * 255.0) + (dst.alpha() as f32 * inv))
                .round()
                .clamp(0.0, 255.0) as u8;
            let (rr, gg, bb) = (
                blend(px4[0], dst.red()),
                blend(px4[1], dst.green()),
                blend(px4[2], dst.blue()),
            );
            if let Some(p) =
                tiny_skia::PremultipliedColorU8::from_rgba(rr.min(na), gg.min(na), bb.min(na), na)
            {
                *dst = p;
            }
        }
    }
}

/// CSS `mix-blend-mode` → `tiny-skia`'s blend mode. Every separable AND non-separable CSS mode has a
/// counterpart, so nothing is approximated here; `Normal` never reaches this function in practice (a
/// normal group does not go offscreen at all).
fn blend_mode(m: manuk_css::BlendMode) -> tiny_skia::BlendMode {
    use manuk_css::BlendMode as B;
    use tiny_skia::BlendMode as T;
    match m {
        B::Normal => T::SourceOver,
        B::Multiply => T::Multiply,
        B::Screen => T::Screen,
        B::Overlay => T::Overlay,
        B::Darken => T::Darken,
        B::Lighten => T::Lighten,
        B::ColorDodge => T::ColorDodge,
        B::ColorBurn => T::ColorBurn,
        B::HardLight => T::HardLight,
        B::SoftLight => T::SoftLight,
        B::Difference => T::Difference,
        B::Exclusion => T::Exclusion,
        B::Hue => T::Hue,
        B::Saturation => T::Saturation,
        B::Color => T::Color,
        B::Luminosity => T::Luminosity,
    }
}
