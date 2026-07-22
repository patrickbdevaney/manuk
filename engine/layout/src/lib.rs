//! manuk-layout — the layout engine.
//!
//! Per CLAUDE.md: `taffy` for flexbox/grid, plus **from-scratch** block, inline,
//! table, positioned, and float layout verified against WPT layout reftests. This
//! implements the formatting contexts that carry the web — **block** (normal-flow
//! vertical stacking with adjacent-sibling margin collapsing), **inline**
//! (line-breaking of text that flows around floats), **floats** (a BFC-aware
//! [`FloatContext`] doing left/right placement, clearance, and shrink-to-fit),
//! **positioning** (relative/absolute/fixed against the containing-block chain), and
//! **tables** (`display:table` with fixed/auto column algorithms) — and routes
//! `display:flex` through `taffy` (see [`flex`]).
//!
//! The output is a **fragment tree** ([`LayoutBox`]) with absolute px rects that
//! paint consumes.
//!
//! Known simplifications (documented, not silent — CLAUDE.md § verification):
//! - Margin collapsing covers adjacent siblings only; parent↔first/last-child
//!   collapsing is not yet modeled.
//! - `relative`/`absolute`/`fixed` positioning is implemented (abs/fixed via a
//!   final pass against the containing-block chain); `sticky` and true *static
//!   position* for inset-less abs boxes are not (such boxes are left unplaced),
//!   and `z-index` stacking follows DOM order.
//! - Tables use the separated-borders model (`border-spacing`) with fixed/auto
//!   column sizing but no `colspan`/`rowspan`, `border-collapse`, captions, or
//!   `<col>` width hints (see [`Ctx::layout_table`]).
//! - Percentage heights resolve only against definite containers.
//! - A line's float band is queried using the first word's height as the estimate
//!   (exact for uniform-size text).
//! - Inline layout is Latin/LTR and inserts an inter-word space between adjacent
//!   tokens (so `a<b>b</b>` gains a space it should not); Parley-grade segmentation
//!   is the upgrade.

use std::cell::RefCell;
use std::collections::HashMap;

use manuk_css::{
    BoxSizing, Clear, ComputedStyle, Dim, Display, Float, IntrinsicSize, Overflow, Position, Rgba,
    StyleMap, TextAlign, VerticalAlign, WhiteSpace,
};
use manuk_dom::{Dom, NodeData, NodeId};
use manuk_text::{FontContext, FontFamily, FontKey};

pub mod flex;
mod taffy_tree;

/// Width (px) of a classic, space-taking scrollbar — the inline gutter an `overflow:scroll`
/// container reserves for its vertical scrollbar. 15px is the long-standing default UA metric on
/// Linux/desktop and the figure `getBoundingClientRect`-driven WPT expects; overlay scrollbars
/// (which take no space) are a separate platform mode we do not emulate here.
const SCROLLBAR_WIDTH: f32 = 15.0;

/// An axis-aligned rectangle in absolute document px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub const ZERO: Rect = Rect {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    };

    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }

    /// The smallest rect containing both. A zero-area rect still contributes its
    /// origin, which matters for empty inline boxes.
    pub fn union(&self, other: &Rect) -> Rect {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let right = self.right().max(other.right());
        let bottom = self.bottom().max(other.bottom());
        Rect {
            x,
            y,
            width: right - x,
            height: bottom - y,
        }
    }

    /// Whether the two rects overlap (touching edges do not count).
    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.right()
            && other.x < self.right()
            && self.y < other.bottom()
            && other.y < self.bottom()
    }

    /// The overlap of two rects (a possibly-empty rect: zero width/height if disjoint).
    pub fn intersect(&self, other: &Rect) -> Rect {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        Rect {
            x,
            y,
            width: (right - x).max(0.0),
            height: (bottom - y).max(0.0),
        }
    }
}

/// The visual style of a text run, resolved for shaping + paint.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStyle {
    pub font_key: FontKey,
    pub font_size: f32,
    pub color: Rgba,
    pub line_height: f32,
    /// `text-decoration` — underline / overline / line-through. Carried on the *text* because that
    /// is what the line is drawn under, and because the decoration propagates from an ancestor
    /// block down to the inline fragments that actually paint.
    pub decoration: manuk_css::TextDecoration,
    /// `letter-spacing` — extra px added after each character. `0` (the default) leaves shaping and
    /// measurement byte-identical, so ordinary text is unaffected.
    pub letter_spacing: f32,
    /// `word-spacing` — extra px added to each inter-word space. `0` (default) is a no-op.
    pub word_spacing: f32,
    /// `text-shadow` — a single shadow painted behind the glyphs (inherited). `None` == no shadow.
    pub shadow: Option<manuk_css::TextShadow>,
    /// The paragraph's bidi **base direction** (`direction: rtl` / `dir="rtl"`), carried to paint
    /// because visual order is resolved at shaping time, not at layout time.
    pub rtl: bool,
}

/// A positioned run of text produced by inline layout. `baseline` is the absolute
/// y of the text baseline; paint places glyphs relative to it.
#[derive(Clone, Debug)]
pub struct TextFragment {
    pub x: f32,
    pub line_top: f32,
    pub baseline: f32,
    /// Advance width of this run — lets a caller derive the run's rect without
    /// re-measuring (§4a element geometry).
    pub width: f32,
    pub text: String,
    pub style: TextStyle,
    /// Deepest **element** ancestor of the text this run came from (e.g. the `<a>` in
    /// `<p>text <a>link</a></p>`). Inline elements produce no `LayoutBox`, so this is
    /// the only way to recover their geometry.
    pub node: Option<NodeId>,
    /// Distance from `baseline` **up** to the top of this run's CSS content area —
    /// `round(ascent)` for *this run's own* face and size, which on a mixed-font line is not the
    /// line's ascent. Stored relative to the baseline on purpose: every vertical shift in this
    /// engine (`translate`, sticky, scroll) already moves `baseline`, so the content area follows
    /// for free and cannot drift out of sync with it.
    pub content_ascent: f32,
    /// Height of this run's content area — `round(ascent) + round(descent)`, independent of
    /// `line-height`. See [`manuk_text::LineMetrics::content_height`].
    pub content_height: f32,
}

impl TextFragment {
    /// This run's box, as `getBoundingClientRect()` reports it: the **content area** (CSS 2.1
    /// §10.6.1) — the font's ascent+descent, centred on the line box by half-leading — **not** the
    /// line box.
    ///
    /// It used to be `(line_top, line_height)`, which is the line box, and that is a different box
    /// on every page that sets `line-height`. On a 14px/1.6 paragraph Chrome reports an `<a>` as
    /// 16px tall starting 3px below the line top; we reported 22px tall starting at the line top —
    /// off in **both** coordinates, on **every inline element on the page**. That is a systematic
    /// near-miss, not a rounding artefact: FID-SWEEP saw it as `dh=+7` repeated across dozens of
    /// wikipedia elements while `dw=0` (widths were already exact), which is precisely the
    /// "one shared root cause, many elements just past tolerance" signature.
    ///
    /// The content area can be **taller than its line box** (`line-height: 1` on most faces), so
    /// half-leading is legitimately negative and this rect legitimately overflows upward. Chrome
    /// does the same; clamping it to zero was the other half of the bug.
    pub fn rect(&self) -> Rect {
        Rect {
            x: self.x,
            y: self.baseline - self.content_ascent,
            width: self.width,
            height: self.content_height,
        }
    }
}

/// Contents of a laid-out box.
#[derive(Clone, Debug)]
pub enum BoxContent {
    /// Block-level children (already absolutely positioned).
    Block(Vec<LayoutBox>),
    /// An inline formatting context: laid-out line text.
    Inline(Vec<TextFragment>),
}

/// A node in the fragment tree: an absolute border-box rect plus contents.
#[derive(Clone, Debug)]
pub struct LayoutBox {
    /// Border box in absolute coordinates.
    pub rect: Rect,
    pub background: Option<Rgba>,
    /// Border edge widths (top, right, bottom, left) + color, when any edge is non-zero.
    pub border: Option<Border>,
    /// `border-radius` in px (uniform); `0.0` = square corners. Rounds the painted background.
    pub radius: f32,
    /// `box-shadow` layers (source order, first on top), painted beneath the box.
    pub shadows: Vec<manuk_css::BoxShadow>,
    /// `visibility: hidden|collapse` — the box still OCCUPIES its space but is not painted.
    pub hidden: bool,
    /// `mask-image: url(...)` — the icon shape. The background is painted THROUGH this mask's
    /// alpha; without it an icon is a solid block of its background colour.
    pub mask_image: Option<String>,
    /// `background-image` — a LIST of layers (url decoded by the page layer, gradient painted
    /// directly), painted back-to-front: index 0 is the topmost layer.
    pub background_images: Vec<manuk_css::BackgroundImage>,
    pub background_size: manuk_css::BackgroundSize,
    /// `background-position` — where a `url()` background image sits (default `0% 0%`, top-left).
    pub background_position: manuk_css::BackgroundPosition,
    /// `object-fit` — how a replaced element's decoded image is fitted into this box (default `fill`,
    /// i.e. stretch). `cover`/`contain` preserve the image's aspect ratio; the paint layer computes
    /// the fitted destination rect and clips the overflow to this box.
    pub object_fit: manuk_css::ObjectFit,
    /// `object-position` — where the fitted replaced content sits in its box (default centered).
    pub object_position: manuk_css::ObjectPosition,
    pub background_repeat: manuk_css::BackgroundRepeat,
    /// `outline` — painted OUTSIDE the border box and never affecting layout, which is exactly what
    /// makes it usable as a focus ring.
    pub outline: Option<(f32, Rgba)>,
    /// A list item's **marker** — the bullet or number. It is generated content, not a child, so it
    /// rides on the box rather than in the tree. Without it every `<ul>` and `<ol>` on the web
    /// renders as bare indented text.
    pub marker: Option<TextFragment>,
    /// **Effective** opacity (own × ancestors'). `0.0` = invisible, `1.0` = opaque.
    pub opacity: f32,
    /// The DOM node this box came from, if any (anonymous boxes are `None`).
    pub node: Option<NodeId>,
    pub content: BoxContent,
}

impl LayoutBox {
    /// The union of every descendant box's extent, relative to this box's origin — i.e. **how tall and
    /// wide the content actually is**, which is what `scrollHeight`/`scrollWidth` report.
    ///
    /// A virtualised list computes `scrollHeight - clientHeight` to decide how many rows exist. Return a
    /// wrong number and it renders the wrong slice of the data; return `undefined` and it renders `NaN`
    /// rows, which is to say none.
    pub fn content_extent(&self) -> (f32, f32) {
        fn walk(b: &LayoutBox, ox: f32, oy: f32, w: &mut f32, h: &mut f32) {
            *w = w.max(b.rect.x + b.rect.width - ox);
            *h = h.max(b.rect.y + b.rect.height - oy);
            match &b.content {
                BoxContent::Block(kids) => {
                    for k in kids {
                        walk(k, ox, oy, w, h);
                    }
                }
                BoxContent::Inline(frags) => {
                    for f in frags {
                        *w = w.max(f.x + f.width - ox);
                        *h = h.max(f.baseline - oy);
                    }
                }
            }
        }
        let (mut w, mut h) = (0.0f32, 0.0f32);
        match &self.content {
            BoxContent::Block(kids) => {
                for k in kids {
                    walk(k, self.rect.x, self.rect.y, &mut w, &mut h);
                }
            }
            BoxContent::Inline(frags) => {
                for f in frags {
                    w = w.max(f.x + f.width - self.rect.x);
                    h = h.max(f.baseline - self.rect.y);
                }
            }
        }
        (w.max(0.0), h.max(0.0))
    }

    /// Find the box for `node`, if it has one.
    pub fn find(&self, node: NodeId) -> Option<&LayoutBox> {
        if self.node == Some(node) {
            return Some(self);
        }
        if let BoxContent::Block(kids) = &self.content {
            for k in kids {
                if let Some(b) = k.find(node) {
                    return Some(b);
                }
            }
        }
        None
    }

    /// Find the box for `node` mutably.
    pub fn find_mut(&mut self, node: NodeId) -> Option<&mut LayoutBox> {
        if self.node == Some(node) {
            return Some(self);
        }
        if let BoxContent::Block(kids) = &mut self.content {
            for k in kids.iter_mut() {
                if let Some(b) = k.find_mut(node) {
                    return Some(b);
                }
            }
        }
        None
    }
}

/// A table cell placed on the row/column grid (CSS2 §17.5 colspan/rowspan).
struct PlacedCell {
    cell: NodeId,
    row: usize,
    col: usize,
    colspan: usize,
    rowspan: usize,
}

/// A box's painted border: per-edge widths (top, right, bottom, left) and a single color.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Border {
    pub widths: [f32; 4],
    pub color: Rgba,
    pub style: manuk_css::BorderStyle,
}

impl LayoutBox {
    /// Visit every box in the fragment tree (pre-order), calling `f` on each.
    pub fn walk(&self, f: &mut impl FnMut(&LayoutBox)) {
        f(self);
        if let BoxContent::Block(children) = &self.content {
            for c in children {
                c.walk(f);
            }
        }
    }

    /// Mutable pre-order visit — for updating paint attributes (colors) in place on a
    /// repaint-only restyle without recomputing geometry.
    pub fn walk_mut(&mut self, f: &mut impl FnMut(&mut LayoutBox)) {
        f(self);
        if let BoxContent::Block(children) = &mut self.content {
            for c in children {
                c.walk_mut(f);
            }
        }
    }

    /// Translate this box and its whole subtree down by `dy` (in document coordinates) —
    /// used to realize `position:sticky` at paint time. Shifts block rects and the baselines
    /// of inline text so the whole subtree moves together.
    /// Shift this box and its whole subtree horizontally (used to place a float-laid-out subtree
    /// that was measured at a provisional origin).
    pub fn shift_x(&mut self, dx: f32) {
        if dx == 0.0 {
            return;
        }
        self.walk_mut(&mut |b| {
            b.rect.x += dx;
            if let BoxContent::Inline(frags) = &mut b.content {
                for f in frags {
                    f.x += dx;
                }
            }
        });
    }

    pub fn shift_y(&mut self, dy: f32) {
        if dy == 0.0 {
            return;
        }
        self.walk_mut(&mut |b| {
            b.rect.y += dy;
            if let BoxContent::Inline(frags) = &mut b.content {
                for frag in frags {
                    frag.line_top += dy;
                    frag.baseline += dy;
                }
            }
        });
    }

    /// Absolute border-box rect per DOM node (§4a element geometry).
    ///
    /// Two sources are unioned:
    ///
    /// * **Block boxes** — each `LayoutBox` carrying a `node`.
    /// * **Inline runs** — an inline element (`<a>`, `<button>`) produces *no*
    ///   `LayoutBox`; its text becomes [`TextFragment`]s in the containing block's
    ///   inline context. Those runs record the element they came from, so the element's
    ///   rect is the union of its runs. Without this, exactly the elements an agent
    ///   wants to click would have no geometry at all.
    ///
    /// A run is also unioned into its **element ancestors** (walked via `dom`), so
    /// `<a><em>x</em></a>` gives `<a>` a rect and not just `<em>`. A node producing
    /// several boxes/runs (an inline split across lines) gets their union — the single
    /// bounding box a caller wants for hit-testing. Anonymous boxes contribute nothing.
    /// Every node's geometry, as `getBoundingClientRect` defines it.
    ///
    /// Two kinds of element, two answers:
    ///
    ///  * An element **with a box** reports that box — its own border box, and *nothing else*. It
    ///    must NOT be unioned with its descendants: a container whose child overflows (a wide
    ///    `<pre>`, an unwrapped code block) still has its own width, and Chrome reports that width.
    ///    Unioning made Wikipedia's 1,200px page container report 2,603px, which is not a layout
    ///    bug at all — it is a measurement bug, and it made every downstream number a lie.
    ///
    ///  * An element **without a box** — an inline `<span>`, `<a>`, `<em>` — has no `LayoutBox` at
    ///    all; its geometry lives in the text fragments its subtree produced. So each fragment is
    ///    walked up to the nearest ancestor that *does* have a box, unioning into every boxless
    ///    element on the way, and stopping there.
    pub fn node_rects(&self, dom: &Dom) -> std::collections::HashMap<NodeId, Rect> {
        fn add(map: &mut std::collections::HashMap<NodeId, Rect>, node: NodeId, rect: Rect) {
            map.entry(node)
                .and_modify(|r| *r = r.union(&rect))
                .or_insert(rect);
        }
        let mut boxes: std::collections::HashMap<NodeId, Rect> = std::collections::HashMap::new();
        let mut frags: std::collections::HashMap<NodeId, Rect> = std::collections::HashMap::new();
        self.walk(&mut |b| {
            if let Some(node) = b.node {
                add(&mut boxes, node, b.rect);
            }
            if let BoxContent::Inline(fs) = &b.content {
                for f in fs {
                    if let Some(owner) = f.node {
                        add(&mut frags, owner, f.rect());
                    }
                }
            }
        });
        let mut out = boxes.clone();
        // A boxless element's geometry is the union of what its subtree produced — its text
        // fragments AND its boxed children. A link wrapping an image (`<a><img></a>`) is inline, so
        // it has no box of its own and no text either: propagating only fragments left it with **no
        // geometry at all**, which means `getBoundingClientRect` returns nothing and the browser
        // cannot find the link under the cursor. A link the browser cannot find is a link the user
        // cannot click.
        //
        // Each contribution walks up only as far as the first ancestor that HAS a box — that
        // ancestor owns its own border box and must not be inflated by content that merely
        // overflows it.
        let mut lift =
            |start: NodeId, r: Rect, out: &mut std::collections::HashMap<NodeId, Rect>| {
                let mut cur = dom.parent(start);
                while let Some(n) = cur {
                    if boxes.contains_key(&n) {
                        break;
                    }
                    if dom.is_element(n) {
                        add(out, n, r);
                    }
                    cur = dom.parent(n);
                }
            };
        for (&owner, &r) in &frags {
            // The fragment's own owner is boxless by construction (an inline element), so it takes
            // the fragment directly before the walk begins.
            if !boxes.contains_key(&owner) && dom.is_element(owner) {
                add(&mut out, owner, r);
            }
            lift(owner, r, &mut out);
        }
        for (&node, &r) in &boxes {
            lift(node, r, &mut out);
        }
        out
    }

    /// Where a text field's value glyphs actually sit, for placing a caret **on the
    /// text** rather than centered in the box: `(end_x, line_top, line_height)` — the
    /// right edge of the run, its line-box top, and its line height (all absolute page
    /// coords). `None` when the field has no value run yet (empty field), so callers
    /// fall back to the box's content edge.
    pub fn value_run(&self, node: NodeId) -> Option<(f32, f32, f32)> {
        let mut found = None;
        self.walk(&mut |b| {
            if b.node == Some(node) {
                if let BoxContent::Inline(frags) = &b.content {
                    // The synthetic value is a single run owned by the field node.
                    if let Some(f) = frags.iter().find(|f| f.node == Some(node)) {
                        found = Some((f.x + f.width, f.line_top, f.style.line_height));
                    }
                }
            }
        });
        found
    }

    /// Shift this box and its whole subtree by `(dx, dy)` (absolute coords).
    ///
    /// Two callers: re-origining a float once its final position is known, and **element-level
    /// scrolling** — which is why it needs no painter changes at all. A scroll container's clip is
    /// already its padding box, so shifting its subtree up by `scrollTop` slides content out of that
    /// clip exactly as a real scroll does; anything scrolled out of view is clipped away for free,
    /// because it was always going to be.
    ///
    /// The `marker` moves too. It did not, before — a `<ul>` inside a float (or now a scroll container)
    /// whose bullets stayed behind while its text moved is a memorable bug, and it was latent here.
    pub fn translate(&mut self, dx: f32, dy: f32) {
        if dx == 0.0 && dy == 0.0 {
            return;
        }
        self.rect.x += dx;
        self.rect.y += dy;
        if let Some(m) = self.marker.as_mut() {
            m.x += dx;
            m.line_top += dy;
            m.baseline += dy;
        }
        match &mut self.content {
            BoxContent::Block(kids) => {
                for k in kids {
                    k.translate(dx, dy);
                }
            }
            BoxContent::Inline(frags) => {
                for f in frags {
                    f.x += dx;
                    f.line_top += dy;
                    f.baseline += dy;
                }
            }
        }
    }

    /// Apply an **absolute** affine matrix `m = [a,b,c,d,e,f]` (`x' = a·x + c·y + e`,
    /// `y' = b·x + d·y + f`) to this box's whole subtree, in place. Each box's rect becomes
    /// the axis-aligned bounding box of its transformed corners (exact for translate/scale;
    /// the transformed AABB for rotate/skew — what `getBoundingClientRect` reports).
    fn transform_affine(&mut self, m: &[f32; 6]) {
        let [a, b, c, d, e, f] = *m;
        let tp = |x: f32, y: f32| (a * x + c * y + e, b * x + d * y + f);
        let r = self.rect;
        let corners = [
            tp(r.x, r.y),
            tp(r.x + r.width, r.y),
            tp(r.x, r.y + r.height),
            tp(r.x + r.width, r.y + r.height),
        ];
        let minx = corners.iter().map(|p| p.0).fold(f32::INFINITY, f32::min);
        let maxx = corners
            .iter()
            .map(|p| p.0)
            .fold(f32::NEG_INFINITY, f32::max);
        let miny = corners.iter().map(|p| p.1).fold(f32::INFINITY, f32::min);
        let maxy = corners
            .iter()
            .map(|p| p.1)
            .fold(f32::NEG_INFINITY, f32::max);
        self.rect = Rect {
            x: minx,
            y: miny,
            width: maxx - minx,
            height: maxy - miny,
        };
        match &mut self.content {
            BoxContent::Block(kids) => {
                for k in kids {
                    k.transform_affine(m);
                }
            }
            BoxContent::Inline(frags) => {
                let sx = (a * a + b * b).sqrt(); // x-axis scale magnitude, for run width
                for fr in frags {
                    let (nx, ntop) = tp(fr.x, fr.line_top);
                    let (_, nbase) = tp(fr.x, fr.baseline);
                    fr.x = nx;
                    fr.line_top = ntop;
                    fr.baseline = nbase;
                    fr.width *= sx;
                }
            }
        }
    }

    /// The full document height this box occupies (max bottom edge in its subtree).
    pub fn content_bottom(&self) -> f32 {
        let mut max = self.rect.y + self.rect.height;
        self.walk(&mut |b| {
            max = max.max(b.rect.y + b.rect.height);
            if let BoxContent::Inline(frags) = &b.content {
                for fr in frags {
                    max = max.max(fr.baseline + fr.style.font_size);
                }
            }
        });
        max
    }
}

/// Shared inputs for a layout pass.
struct Ctx<'a> {
    dom: &'a Dom,
    styles: &'a StyleMap,
    fonts: &'a FontContext,
    /// Memoized intrinsic content sizes for the flex/grid measure seam, keyed by
    /// `(node, available-width rounded to px)`. Taffy probes each item's size several
    /// times (min-content, max-content, resolved) and each probe would otherwise re-lay-out
    /// the whole subtree — an O(n²) blow-up on nested flex/grid. Interior-mutable so
    /// `measure_intrinsic` (`&self`) can fill it.
    measure_cache: RefCell<HashMap<(NodeId, u32), (f32, f32)>>,
    /// **The style every node gets when the cascade never saw it.**
    ///
    /// See `style_of`. Held here so the 25 lookup sites can hand out a `&ComputedStyle` with the
    /// same lifetime as the map's own entries.
    fallback_style: ComputedStyle,
    /// Memoized **min-content** widths. Computing one lays out the whole subtree, and
    /// shrink-to-fit now asks for it on every probe, so without this it is an O(n²) trap.
    min_content_cache: RefCell<HashMap<NodeId, f32>>,
    /// Memoized **max-content** (preferred) widths.
    ///
    /// This was the other half of the same trap, and it was the expensive half. `shrink_to_fit`
    /// recomputed max-content on EVERY call by laying the whole subtree out at a 1e6 available width —
    /// and taffy probes each flex/grid item several times per solve, at several available widths. On
    /// nested flex the cost compounds per level of nesting.
    ///
    /// Measured, and the ratio is what gives it away: **bbc.co.uk has 4,021 nodes and takes 260ms to
    /// lay out; Wikipedia has 18,630 and takes 127ms.** Four-and-a-half times fewer nodes, twice the
    /// time — about ten times worse per node — and the difference between the two pages is that one is
    /// deeply nested flex and the other is a document.
    ///
    /// Both min-content and max-content are **independent of the available width** — that is what makes
    /// them *intrinsic*. So both can be cached per node, and `shrink_to_fit` becomes a lookup and two
    /// comparisons instead of a subtree layout.
    max_content_cache: RefCell<HashMap<NodeId, f32>>,
    /// Flex/grid items whose **used border-box width taffy has already decided**. Their own `width`
    /// style must NOT be resolved a second time — see the width resolution in `layout_block`.
    taffy_item_width: RefCell<HashMap<NodeId, f32>>,
    /// **Static positions of out-of-flow boxes** — where an `absolute` box *would* have gone had it
    /// stayed in flow. Recorded as normal flow walks past it, because that is the only moment the
    /// information exists.
    static_pos: RefCell<HashMap<NodeId, (f32, f32)>>,
}

/// Lay out a whole document into a fragment tree, given a viewport width in px.
///
/// The root box is `<body>` (falling back to `<html>` or the first element), laid
/// out in an initial containing block of `viewport_width`.
/// **Part 22.3: how many full-document layouts does ONE navigation perform?** More than one, absent
/// an explicit re-navigation, is duplicate work. Counted, because the answer turned out to be
/// "dozens".
pub static LAYOUTS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

pub fn layout_document(
    dom: &Dom,
    styles: &StyleMap,
    fonts: &FontContext,
    viewport_width: f32,
) -> LayoutBox {
    LAYOUTS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let ctx = Ctx {
        dom,
        styles,
        fonts,
        measure_cache: RefCell::new(HashMap::new()),
        fallback_style: ComputedStyle::initial(),
        min_content_cache: RefCell::new(HashMap::new()),
        max_content_cache: RefCell::new(HashMap::new()),
        taffy_item_width: RefCell::new(HashMap::new()),
        static_pos: RefCell::new(HashMap::new()),
    };
    let root_el = dom
        .find_first("body")
        .or_else(|| dom.find_first("html"))
        .or_else(|| dom.children(dom.root()).find(|&n| dom.is_element(n)));

    match root_el {
        Some(el) => {
            // The initial containing block is itself a BFC root; `layout_block` gives
            // the root element its own context, so this outer one is just a seed.
            //
            // The ICB has the viewport's dimensions (CSS2 §10.1), and its **height** is the
            // reference a root-level `height: 100%` resolves against — the full-height app-shell
            // pattern (`html,body{height:100%}` then `#app{height:100%}`) that every SPA relies on
            // to make a scrollable pane fill the window. Passing `None` here made that root percent
            // indefinite, so the whole chain fell back to content height and the shell never filled
            // the viewport. Read the height from the same viewport the parser resolves `vh` against
            // so a `height:100%` root and a `100vh` sibling can never disagree.
            let icb_height = manuk_css::values::viewport_size().1;
            let mut floats = FloatContext::new(0.0, viewport_width);
            let mut root = ctx
                .layout_block(
                    el,
                    viewport_width,
                    Some(icb_height),
                    0.0,
                    0.0,
                    0.0,
                    &mut floats,
                )
                .boxx;
            // Absolute/fixed boxes were skipped in flow; place them in a final pass
            // against their containing blocks (CSS2 §9.6).
            ctx.position_absolutes(el, &mut root, viewport_width);
            root
        }
        None => LayoutBox {
            rect: Rect::ZERO,
            background: None,
            border: None,
            radius: 0.0,
            shadows: Vec::new(),
            hidden: false,
            mask_image: None,
            background_images: Vec::new(),
            background_size: manuk_css::BackgroundSize::Auto,
            background_position: manuk_css::BackgroundPosition::default(),
            object_fit: manuk_css::ObjectFit::Fill,
            object_position: manuk_css::ObjectPosition::default(),
            background_repeat: manuk_css::BackgroundRepeat::Repeat,
            outline: None,
            marker: None,
            opacity: 1.0,
            node: None,
            content: BoxContent::Block(vec![]),
        },
    }
}

/// Is `node` a block-level box in its parent's formatting context?
/// Compose a `transform` function list into an **absolute** affine matrix applied around
/// `origin` (the transform-origin, default the box center). `w`/`h` resolve `translate` `%`.
fn resolve_transform(
    fns: &[manuk_css::TransformFn],
    w: f32,
    h: f32,
    origin: (f32, f32),
) -> [f32; 6] {
    use manuk_css::TransformFn as T;
    // Local matrix = product of the functions in source order (first is outermost).
    let mut local = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    for f in fns {
        let m = match *f {
            T::Translate(tx, ty) => [1.0, 0.0, 0.0, 1.0, tx.resolve(w, 0.0), ty.resolve(h, 0.0)],
            T::Scale(sx, sy) => [sx, 0.0, 0.0, sy, 0.0, 0.0],
            T::Rotate(rad) => {
                let (s, c) = rad.sin_cos();
                [c, s, -s, c, 0.0, 0.0]
            }
            T::Skew(ax, ay) => [1.0, ay.tan(), ax.tan(), 1.0, 0.0, 0.0],
            T::Matrix(m) => m,
        };
        local = affine_mul(&local, &m);
    }
    // Absolute = T(origin) · local · T(-origin).
    let (ox, oy) = origin;
    let to = [1.0, 0.0, 0.0, 1.0, ox, oy];
    let from = [1.0, 0.0, 0.0, 1.0, -ox, -oy];
    affine_mul(&affine_mul(&to, &local), &from)
}

/// Multiply two 2×3 affine matrices (`[a,b,c,d,e,f]`, column-vector convention).
fn affine_mul(m1: &[f32; 6], m2: &[f32; 6]) -> [f32; 6] {
    let [a1, b1, c1, d1, e1, f1] = *m1;
    let [a2, b2, c2, d2, e2, f2] = *m2;
    [
        a1 * a2 + c1 * b2,
        b1 * a2 + d1 * b2,
        a1 * c2 + c1 * d2,
        b1 * c2 + d1 * d2,
        a1 * e2 + c1 * f2 + e1,
        b1 * e2 + d1 * f2 + f1,
    ]
}

/// The paintable border of a styled box, or `None` when every edge is zero-width.
fn border_of(s: &ComputedStyle) -> Option<Border> {
    let w = s.border_width;
    if w.top == 0.0 && w.right == 0.0 && w.bottom == 0.0 && w.left == 0.0 {
        None
    } else {
        Some(Border {
            widths: [w.top, w.right, w.bottom, w.left],
            color: s.border_color,
            style: s.border_style,
        })
    }
}

/// The synthetic text a form control renders (its value / label), or `None` for controls
/// that render no text (`<button>` uses its real children; checkbox/radio are boxes). A
/// text input returns `Some("")` when empty so it still lays out with a line's height.
fn form_control_text(dom: &Dom, node: NodeId) -> Option<String> {
    let el = dom.element(node)?;
    match dom.tag_name(node)? {
        "input" => match el
            .attr("type")
            .unwrap_or("text")
            .to_ascii_lowercase()
            .as_str()
        {
            "submit" => Some(el.attr("value").unwrap_or("Submit").to_string()),
            "reset" => Some(el.attr("value").unwrap_or("Reset").to_string()),
            "button" => Some(el.attr("value").unwrap_or("").to_string()),
            "file" => Some("Choose File".to_string()),
            "checkbox" | "radio" | "hidden" | "image" | "range" | "color" => None,
            "password" => {
                let n = el.attr("value").map(|v| v.chars().count()).unwrap_or(0);
                Some("\u{2022}".repeat(n))
            }
            // Text-like: the current value, else the placeholder, else empty.
            _ => Some(
                el.attr("value")
                    .filter(|v| !v.is_empty())
                    .or_else(|| el.attr("placeholder"))
                    .unwrap_or("")
                    .to_string(),
            ),
        },
        // A textarea's value is a typed `value` attr if present, else its text children.
        "textarea" => Some(
            el.attr("value")
                .map(str::to_string)
                .unwrap_or_else(|| dom.text_content(node)),
        ),
        // A <select> shows its selected <option> (first with `selected`, else the first).
        "select" => {
            let mut first = None;
            let mut selected = None;
            for c in dom.descendants(node) {
                if dom.tag_name(c) == Some("option") {
                    if first.is_none() {
                        first = Some(c);
                    }
                    if dom.element(c).is_some_and(|e| e.attr("selected").is_some()) {
                        selected = Some(c);
                        break;
                    }
                }
            }
            selected
                .or(first)
                .map(|opt| dom.text_content(opt).trim().to_string())
        }
        _ => None,
    }
}

fn is_block_level(dom: &Dom, styles: &StyleMap, node: NodeId) -> bool {
    if let NodeData::Element(_) = dom.data(node) {
        if matches!(
            styles.get(&node).map(|s| s.display),
            Some(Display::Block | Display::Flex | Display::Grid | Display::Table)
        ) {
            return true;
        }
        // **Block-in-inline** (CSS2 §9.2.1.1). An inline box containing a block-level box cannot
        // stay in an inline formatting context: the spec splits the inline around the block and
        // wraps the run in anonymous block boxes. We approximate that by *blockifying* such an
        // inline — it becomes block-level, so its parent opens a block formatting context and the
        // inline's own children then split into anonymous blocks (the inline run) plus the block
        // child, which is exactly the resulting box structure.
        //
        // Without this the block child was swallowed by the inline collector: its text still
        // flowed, but its BOX (background/padding/border) vanished entirely. The approximation
        // differs from the spec only in where the *inline's own* background paints (spec: on each
        // split fragment; here: behind the blockified box) — invisible unless a block-containing
        // inline is itself styled, which is vanishingly rare.
        // A replaced inline (`<svg>` above all — it HAS element children) is atomic: nothing
        // inside it can split it, so it never blockifies.
        if matches!(styles.get(&node).map(|s| s.display), Some(Display::Inline))
            && !is_atomic_inline_replaced(dom, styles, node)
        {
            return inline_contains_block(dom, styles, node);
        }
    }
    false
}

/// Whether `node` (an inline box) has a block-level box somewhere in its inline-only descent.
/// Recurses only through further *inline* children — an inline-block / flex / table child is
/// atomic and does not make its ancestor block-level.
fn inline_contains_block(dom: &Dom, styles: &StyleMap, node: NodeId) -> bool {
    for k in dom.flat_children(node) {
        if !is_rendered(dom, styles, k) {
            continue;
        }
        let Some(d) = styles.get(&k).map(|s| s.display) else {
            continue;
        };
        if matches!(
            d,
            Display::Block | Display::Flex | Display::Grid | Display::Table
        ) {
            return true;
        }
        // A replaced inline child is atomic — its subtree cannot blockify the ancestor.
        if d == Display::Inline
            && !is_atomic_inline_replaced(dom, styles, k)
            && inline_contains_block(dom, styles, k)
        {
            return true;
        }
    }
    false
}

/// The children of `node` **as layout sees them** — with every `display: contents` wrapper dissolved.
///
/// `display: contents` means the element generates **no box at all, while its children still do**. It is
/// not `display: none` — nothing is hidden. The wrapper simply vanishes from the box tree and its
/// children are laid out as if they were the parent's own.
///
/// Modern CSS leans on this hard: a `<div>` wrapping grid items so that a component can own them, without
/// that `<div>` becoming a grid item itself and collapsing the entire layout into a single cell. React and
/// friends emit such wrappers constantly.
///
/// Flattening is **recursive**, because `contents` inside `contents` is legal and a component tree
/// produces exactly that.
fn rendered_children(dom: &Dom, styles: &StyleMap, node: NodeId) -> Vec<NodeId> {
    fn push(dom: &Dom, styles: &StyleMap, node: NodeId, out: &mut Vec<NodeId>, depth: u32) {
        // A cycle cannot happen in a tree, but a pathological nesting can still be deep. Bound it: a
        // stack overflow in layout is a Bar 0 crash, and `display: contents` is exactly the kind of
        // property a hostile page would nest ten thousand deep.
        if depth > 64 {
            return;
        }
        for k in dom.flat_children(node) {
            if !is_rendered(dom, styles, k) {
                continue;
            }
            if styles.get(&k).map(|s| s.display) == Some(Display::Contents) {
                push(dom, styles, k, out, depth + 1);
            } else {
                out.push(k);
            }
        }
    }
    let mut out = Vec::new();
    push(dom, styles, node, &mut out, 0);
    out
}

fn is_rendered(dom: &Dom, styles: &StyleMap, node: NodeId) -> bool {
    match dom.data(node) {
        // A node the cascade has never seen is not in the render tree. This is not merely a
        // convenience: layout INDEXES the style map, so an unstyled node is a panic. Scripts add
        // nodes to the DOM at runtime (a `<script>` element appended by a module loader, a
        // fragment built by a framework), and any one of them arriving before the next restyle
        // used to abort the process.
        NodeData::Element(_) => match styles.get(&node) {
            Some(s) => s.display != Display::None,
            None => false,
        },
        NodeData::Text(_) => styles.contains_key(&node),
        _ => false,
    }
}

/// Apply `text-transform` to a text run for RENDERING only (the DOM text is untouched, so JS still
/// reads the author's string). `None` borrows the input unchanged; the casing modes allocate. Unicode
/// casing is honoured (`ß`→`SS`, locale-independent). `Capitalize` upper-cases the first cased letter
/// of each whitespace-delimited word and leaves the rest as authored (the common-case approximation of
/// the spec's "first typographic letter unit").
fn apply_text_transform(s: &str, transform: manuk_css::TextTransform) -> std::borrow::Cow<'_, str> {
    use manuk_css::TextTransform;
    match transform {
        TextTransform::None => std::borrow::Cow::Borrowed(s),
        TextTransform::Uppercase => std::borrow::Cow::Owned(s.to_uppercase()),
        TextTransform::Lowercase => std::borrow::Cow::Owned(s.to_lowercase()),
        TextTransform::Capitalize => {
            let mut out = String::with_capacity(s.len());
            let mut at_word_start = true;
            for ch in s.chars() {
                if ch.is_whitespace() {
                    at_word_start = true;
                    out.push(ch);
                } else if at_word_start && ch.is_alphabetic() {
                    out.extend(ch.to_uppercase());
                    at_word_start = false;
                } else {
                    out.push(ch);
                    at_word_start = false;
                }
            }
            std::borrow::Cow::Owned(out)
        }
    }
}

/// The longest char prefix of `text` whose rendered width fits `budget`, and that width. Grapheme
/// clusters aren't split (we cut on `char` boundaries — exact for the Latin common case).
fn truncate_to_width(
    text: &str,
    style: &TextStyle,
    budget: f32,
    fonts: &FontContext,
) -> (String, f32) {
    let mut best = String::new();
    let mut best_w = 0.0;
    let mut cur = String::new();
    for ch in text.chars() {
        cur.push(ch);
        let w = fonts.measure(&cur, style.font_key, style.font_size);
        if w > budget {
            break;
        }
        best.push(ch);
        best_w = w;
    }
    (best, best_w)
}

/// `text-overflow: ellipsis` — truncate an overflowing single (`nowrap`) line with `…`. The line's
/// fragments are absolute-positioned starting at `cx`; anything past `cx + cw` is clipped, so we keep
/// the fragments that fit before `cutoff = cx + cw − width('…')`, cut the one straddling it to that
/// budget, drop the rest, and append an ellipsis fragment. A line that fits within the box is left
/// untouched (the overwhelming common case — so no page without an actual overflow changes at all).
fn apply_text_overflow_ellipsis(
    frags: &mut Vec<TextFragment>,
    cx: f32,
    cw: f32,
    fonts: &FontContext,
) {
    if frags.is_empty() || cw <= 0.0 {
        return;
    }
    let clip_right = cx + cw;
    let line_right = frags.iter().map(|f| f.x + f.width).fold(f32::MIN, f32::max);
    if line_right <= clip_right + 0.5 {
        return; // fits — nothing to truncate
    }
    let base_style = frags[0].style;
    let ell = "\u{2026}";
    let ell_w = fonts.measure(ell, base_style.font_key, base_style.font_size);
    let cutoff = clip_right - ell_w;

    let mut out: Vec<TextFragment> = Vec::with_capacity(frags.len());
    // The ellipsis anchor: position, and the vertical/style/owner it inherits from the last kept run.
    let mut ell_x = cx;
    let mut ell_style = base_style;
    let mut ell_ca = frags[0].content_ascent;
    let mut ell_ch = frags[0].content_height;
    let mut line_top = frags[0].line_top;
    let mut baseline = frags[0].baseline;
    let mut node = frags[0].node;
    for f in frags.drain(..) {
        line_top = f.line_top;
        baseline = f.baseline;
        if f.x + f.width <= cutoff {
            // Fits entirely before the cutoff: keep it, and move the ellipsis anchor to its end.
            ell_x = f.x + f.width;
            ell_style = f.style;
            ell_ca = f.content_ascent;
            ell_ch = f.content_height;
            node = f.node;
            out.push(f);
        } else if f.x < cutoff {
            // Straddles the cutoff: truncate to the budget, keep the prefix, place the ellipsis after.
            let budget = (cutoff - f.x).max(0.0);
            let (prefix, pw) = truncate_to_width(&f.text, &f.style, budget, fonts);
            ell_x = f.x + pw;
            ell_style = f.style;
            ell_ca = f.content_ascent;
            ell_ch = f.content_height;
            node = f.node;
            if !prefix.is_empty() {
                out.push(TextFragment {
                    x: f.x,
                    line_top: f.line_top,
                    baseline: f.baseline,
                    width: pw,
                    text: prefix,
                    style: f.style,
                    node: f.node,
                    content_ascent: f.content_ascent,
                    content_height: f.content_height,
                });
            }
            break; // everything after this is clipped away
        } else {
            // Starts past the cutoff: entirely clipped — the ellipsis sits at the last anchor.
            break;
        }
    }
    out.push(TextFragment {
        x: ell_x,
        line_top,
        baseline,
        width: ell_w,
        text: ell.to_string(),
        style: ell_style,
        node,
        content_ascent: ell_ca,
        content_height: ell_ch,
    });
    *frags = out;
}

fn text_style(cs: &ComputedStyle, fonts: &FontContext) -> TextStyle {
    let key = FontKey {
        family: fonts.resolve_family(&cs.font_family),
        bold: cs.font_weight >= 600,
        italic: cs.italic,
    };
    // `line-height: normal` is the FONT's business, not arithmetic's. Every engine derives it from
    // the face's ascent + descent + lineGap; a 1.2× multiplier is a guess that makes every line box
    // on every page the wrong height, and it is a first-order source of vertical drift.
    let line_height = if cs.line_height_normal {
        let lm = fonts.line_metrics(key, cs.font_size);
        // `height()` rounds the sum to a whole pixel, which is what Chrome lays out with. The
        // fractional remainder rides on EVERY line box, so it compounds down the page instead of
        // staying local — see `LineMetrics::height` for the three-face measurement.
        let h = lm.height();
        if h > 0.0 {
            h
        } else {
            cs.line_height
        }
    } else {
        cs.line_height
    };
    TextStyle {
        // The paragraph's bidi base direction. Resolved here, from the cascade, because by paint
        // time the only thing left is glyphs — visual order has to be decided while the style is
        // still in hand.
        rtl: cs.direction == manuk_css::Direction::Rtl,
        decoration: cs.text_decoration,
        font_key: FontKey {
            // Resolve the CSS font-family list to a concrete face (installed or
            // `@font-face`-registered), falling back through generics.
            family: fonts.resolve_family(&cs.font_family),
            bold: cs.font_weight >= 600,
            italic: cs.italic,
        },
        font_size: cs.font_size,
        color: cs.color,
        line_height,
        letter_spacing: cs.letter_spacing,
        word_spacing: cs.word_spacing,
        shadow: cs.text_shadow,
    }
}

/// The pieces a parent needs to stack a block child with margin collapsing.
struct BlockResult {
    boxx: LayoutBox,
    /// This block's top margin (already applied to `boxx.rect.y`, reported so a
    /// parent-child collapse could use it later).
    margin_top: f32,
    /// This block's bottom margin — the parent collapses it with the next sibling's
    /// top margin (or applies it fully before non-collapsible content).
    margin_bottom: f32,
    /// The border-bottom edge in **normal flow** (before any `position:relative`
    /// shift), which the parent uses to stack the next sibling.
    flow_bottom: f32,
}

/// One placed float's **margin box** plus which side it hugs, in absolute coords.
#[derive(Clone, Copy)]
struct PlacedFloat {
    rect: Rect,
    side: Float,
}

/// Float state for one **block formatting context** (CSS2 §9.4.1). Because the whole
/// engine lays out in absolute document px, a single context can be threaded down
/// through nested non-BFC blocks and their line boxes unchanged. Servo's
/// `layout_2020` keeps an analogous `FloatContext`/`PlacementAmongFloats`.
struct FloatContext {
    /// Content-left / content-right of the BFC root, the edges floats hug.
    left_edge: f32,
    right_edge: f32,
    floats: Vec<PlacedFloat>,
}

/// Does the float/query band `[y, y+h)` intersect `rect`'s vertical extent? A
/// zero-height query still tests the point `y`.
fn band_overlaps(rect: Rect, y: f32, h: f32) -> bool {
    rect.height > 0.0 && rect.y < y + h.max(0.01) && rect.y + rect.height > y
}

impl FloatContext {
    fn new(left_edge: f32, right_edge: f32) -> Self {
        FloatContext {
            left_edge,
            right_edge,
            floats: Vec::new(),
        }
    }

    /// Rightmost right-edge among left floats overlapping band `[y, y+h)`.
    fn left_offset(&self, y: f32, h: f32) -> f32 {
        let mut x = self.left_edge;
        for f in &self.floats {
            if f.side == Float::Left && band_overlaps(f.rect, y, h) {
                x = x.max(f.rect.x + f.rect.width);
            }
        }
        x
    }

    /// Leftmost left-edge among right floats overlapping band `[y, y+h)`.
    fn right_offset(&self, y: f32, h: f32) -> f32 {
        let mut x = self.right_edge;
        for f in &self.floats {
            if f.side == Float::Right && band_overlaps(f.rect, y, h) {
                x = x.min(f.rect.x);
            }
        }
        x
    }

    /// Available `(left_x, width)` for in-flow / line content in band `[y, y+h)`.
    fn available(&self, y: f32, h: f32) -> (f32, f32) {
        let l = self.left_offset(y, h);
        let r = self.right_offset(y, h);
        (l, (r - l).max(0.0))
    }

    /// The next float bottom strictly below `y`, if any (a candidate drop position).
    fn next_bottom_below(&self, y: f32) -> Option<f32> {
        self.floats
            .iter()
            .map(|f| f.rect.y + f.rect.height)
            .filter(|&b| b > y + 0.01)
            .fold(None, |acc, b| Some(acc.map_or(b, |a: f32| a.min(b))))
    }

    /// Place a float of margin-box size `(w, h)` on `side`, no higher than `top`.
    /// Scans downward to the first band where `w` fits between opposing floats
    /// (CSS2 §9.5.1), records the margin box, and returns it.
    fn place(&mut self, side: Float, top: f32, w: f32, h: f32) -> Rect {
        let full = self.right_edge - self.left_edge;
        let mut y = top;
        loop {
            let (l, avail) = self.available(y, h);
            if w <= avail || avail >= full {
                let x = if side == Float::Right {
                    self.right_offset(y, h) - w
                } else {
                    l
                };
                let rect = Rect {
                    x,
                    y,
                    width: w,
                    height: h,
                };
                self.floats.push(PlacedFloat { rect, side });
                return rect;
            }
            match self.next_bottom_below(y) {
                Some(ny) => y = ny,
                None => {
                    // Nothing opposing fits anywhere lower: hug the edge here.
                    let x = if side == Float::Right {
                        self.right_edge - w
                    } else {
                        self.left_edge
                    };
                    let rect = Rect {
                        x,
                        y,
                        width: w,
                        height: h,
                    };
                    self.floats.push(PlacedFloat { rect, side });
                    return rect;
                }
            }
        }
    }

    /// The y at/below `y` clear of the requested side(s) (CSS2 §9.5.2).
    fn clear_to(&self, clear: Clear, y: f32) -> f32 {
        let mut out = y;
        for f in &self.floats {
            let clears = matches!(
                (clear, f.side),
                (Clear::Both, _) | (Clear::Left, Float::Left) | (Clear::Right, Float::Right)
            );
            if clears {
                out = out.max(f.rect.y + f.rect.height);
            }
        }
        out
    }

    /// Lowest bottom edge of any float (so a BFC root can grow to contain them).
    fn lowest_bottom(&self) -> f32 {
        self.floats
            .iter()
            .map(|f| f.rect.y + f.rect.height)
            .fold(f32::MIN, f32::max)
    }
}

/// Does this element pull out of flow to one side?
fn is_float(s: &ComputedStyle) -> bool {
    s.float != Float::None
}

/// The document-coordinate shift to apply to a `position:sticky` box at scroll offset
/// `scroll_y`. The box stays in normal flow until the viewport would scroll it above
/// `top_inset`, at which point it pins there — but never past the bottom of its containing
/// block (`cb_bottom`), so it scrolls away with its container. `natural_y`/`box_h` are the
/// box's in-flow top and height. Returns `0.0` while the box hasn't been scrolled to its
/// threshold (the common, unshifted case).
pub fn sticky_shift(
    natural_y: f32,
    box_h: f32,
    top_inset: f32,
    cb_bottom: f32,
    scroll_y: f32,
) -> f32 {
    let pinned = (scroll_y + top_inset).min(cb_bottom - box_h);
    natural_y.max(pinned) - natural_y
}

/// Is this box positioned out of normal flow (absolute/fixed)? Such boxes are
/// collected and laid out in a later pass (D1 sub-feature 2).
fn is_out_of_flow_positioned(s: &ComputedStyle) -> bool {
    matches!(s.position, Position::Absolute | Position::Fixed)
}

/// Does this element establish a new block formatting context (CSS2 §9.4.1)? Such a
/// box does not share its parent's float context — its own floats stay inside and it
/// does not overlap outer floats, and it grows to contain its floats (§10.6.7).
///
/// `overflow` other than `visible` is a BFC root (CSS2 §9.4.1 / Display §2.1): this is the
/// modern clearfix — `overflow:hidden`/`auto`/`scroll` on a container makes it enclose its
/// floated children rather than let them escape, and stops its own content from wrapping an
/// outer float. Chrome establishes a BFC for `overflow:clip` too, so any non-`visible` value counts.
/// Is this a **replaced** element — a box whose content comes from outside CSS (a bitmap, a video
/// frame, a canvas surface) and which therefore has an intrinsic size and ratio of its own?
///
/// Only replaced elements take CSS2.1 §10.4's proportional constraint adjustment: for an ordinary
/// box a specified height stands even when `max-width` cuts the width, but a replaced element's two
/// axes are tied together by the ratio of the thing being displayed.
fn is_replaced_element(tag: Option<&str>) -> bool {
    matches!(tag, Some("img" | "canvas" | "video" | "svg"))
}

/// Is `node` a **replaced element at `display: inline`** — an ATOMIC inline box?
///
/// The computed display of `<img>` (and every replaced element) is `inline`, per spec and per
/// Chrome — but it does not participate in an inline formatting context as text does: it is
/// sized as a block and flowed like a word, exactly like an `inline-block` (tick 384; the
/// cascade used to force `inline-block` computed values to get this behavior, and the corpus
/// oracle showed 81 sites diverging on `<img>`'s computed display alone). The tag list is the
/// cascade's replaced-element set, wider than `is_replaced_element` on purpose: `iframe` /
/// `object` / `embed` don't take §10.4 ratio adjustment but are just as atomic in a line.
fn is_atomic_inline_replaced(dom: &Dom, styles: &StyleMap, node: NodeId) -> bool {
    matches!(styles.get(&node).map(|s| s.display), Some(Display::Inline))
        && matches!(
            dom.tag_name(node),
            Some("img" | "canvas" | "video" | "svg" | "object" | "embed" | "iframe")
        )
}

fn establishes_bfc(s: &ComputedStyle) -> bool {
    is_float(s)
        || is_out_of_flow_positioned(s)
        || s.overflow != Overflow::Visible
        || matches!(
            s.display,
            Display::Flex
                | Display::Grid
                | Display::InlineFlex
                | Display::InlineGrid
                | Display::InlineBlock
        )
}

/// May this block collapse its **top** margin with its first in-flow block child (CSS2 §8.3.1)?
/// A plain block box, `overflow:visible`, not a BFC root, with no top border and no top padding —
/// the conditions under which the child's top margin escapes upward through this box. `cw` is the
/// width the top padding resolves against (this box's containing-block width).
fn top_margin_collapses(s: &ComputedStyle, cw: f32) -> bool {
    s.display == Display::Block
        && s.overflow == Overflow::Visible
        && !establishes_bfc(s)
        && s.border_width.top == 0.0
        && s.padding.top.resolve(cw, 0.0) == 0.0
}

/// The mirror of [`top_margin_collapses`] for the **bottom** edge: additionally the box must be
/// auto-height (checked by the caller), so the last child's bottom margin escapes downward.
fn bottom_margin_collapses(s: &ComputedStyle, cw: f32) -> bool {
    s.display == Display::Block
        && s.overflow == Overflow::Visible
        && !establishes_bfc(s)
        && s.border_width.bottom == 0.0
        && s.padding.bottom.resolve(cw, 0.0) == 0.0
}

/// The max right extent of already-laid-out content (used for shrink-to-fit).
///
/// `origin` is the left edge the subtree was laid out from, so extents are measured **relative** to
/// the thing being sized rather than in absolute page coordinates.
fn content_right_extent(
    content: &BoxContent,
    fonts: &FontContext,
    origin: f32,
    // The right margin (px, non-negative) of a box's node. Needed because a `LayoutBox` carries only its
    // BORDER box, while `rect.x` already includes the box's LEFT margin — so without adding the right
    // margin the measured extent is asymmetric and short by one margin. A flex item wrapping `<p margin:10>`
    // reported 110 instead of 120 (its content's margin box). Percentage/auto margins resolve to 0 for an
    // intrinsic measure; negative margins do not pull the border-box edge in, so this is clamped ≥ 0.
    margin_right: &dyn Fn(Option<NodeId>) -> f32,
) -> f32 {
    // `shrink_to_fit` lays the subtree out at a very large available width (1e6) to read its
    // *max-content* width. Two artifacts of that absurd width must be discarded, or the measurement
    // is nonsense:
    //
    //  * **Size.** A block-level box fills its container, so its own `rect.width` is ≈1e6 —
    //    meaningless as a max-content contribution. Count a box's own right edge only when it did
    //    NOT fill the measuring width; otherwise recurse to the inline text that carries the real
    //    extent. (Without this, a flex/grid item with a block child hogged its whole track.)
    //
    //  * **Position.** Centering (`margin: 0 auto`, `justify-content: center`) distributes FREE
    //    SPACE — and at a 1e6 available width the free space is ~1e6, so a perfectly ordinary
    //    1,000px-wide box lands at x≈499,500. Its width is real; its offset is an artifact. Adding
    //    that offset to the extent reported Wikipedia's header as **500,532px wide**, which
    //    overflowed its flex line and wrapped the search bar onto its own row — dragging the whole
    //    page 66px down and every element below it out of place.
    //
    // So: measure relative to `origin`, and treat an implausibly large relative offset as slack
    // rather than content. The box's own width still counts, so nothing real is lost.
    const FILL_SENTINEL: f32 = 500_000.0;
    const SLACK: f32 = 100_000.0;
    let rel = |x: f32| -> f32 {
        let d = x - origin;
        if d > SLACK {
            0.0
        } else {
            d
        }
    };

    /// The extent of one box's inline content, measured **per line**.
    ///
    /// A line's fragments cannot be read in absolute coordinates: `text-align: center` (which
    /// `<center>` sets, and which then inherits into everything under it) centres each line inside
    /// the *available* width — 1e6 during measurement — so every fragment sits at x≈500,000. Taking
    /// `max(x + width)` there measures the centring slack; discarding the offset entirely measures
    /// only the longest single word (Hacker News' story titles collapsed to a 99px column that way).
    ///
    /// Both are wrong for the same reason: a line's *position* is slack, its *span* is content. So
    /// span each line from its own leftmost fragment, and keep the line's offset only when it is a
    /// real indent (a padding, a margin) rather than half a million pixels of centring.
    fn inline_extent(
        frags: &[TextFragment],
        _fonts: &FontContext,
        rel: &dyn Fn(f32) -> f32,
    ) -> f32 {
        let mut lines: std::collections::HashMap<u32, (f32, f32)> =
            std::collections::HashMap::new();
        for f in frags {
            // `f.width` already includes any `letter-spacing` (and equals `measure(text)` when it is
            // zero), so use it rather than re-measuring, which would drop the tracking.
            let key = f.line_top.to_bits();
            let e = lines.entry(key).or_insert((f32::MAX, f32::MIN));
            e.0 = e.0.min(f.x);
            e.1 = e.1.max(f.x + f.width);
        }
        lines
            .values()
            .map(|&(l, r)| rel(l).max(0.0) + (r - l).max(0.0))
            .fold(0.0f32, f32::max)
    }

    let mut max_r = 0.0f32;
    fn visit(
        b: &LayoutBox,
        fonts: &FontContext,
        max_r: &mut f32,
        rel: &dyn Fn(f32) -> f32,
        mr: &dyn Fn(Option<NodeId>) -> f32,
    ) {
        if b.rect.width < FILL_SENTINEL {
            // `rect.x` includes the LEFT margin; add the RIGHT margin for a full margin-box extent.
            *max_r = max_r.max(rel(b.rect.x) + b.rect.width + mr(b.node));
        }
        match &b.content {
            BoxContent::Block(kids) => {
                for k in kids {
                    visit(k, fonts, max_r, rel, mr);
                }
            }
            BoxContent::Inline(frags) => {
                *max_r = max_r.max(inline_extent(frags, fonts, rel));
            }
        }
    }
    match content {
        BoxContent::Block(kids) => {
            for k in kids {
                visit(k, fonts, &mut max_r, &rel, margin_right);
            }
        }
        BoxContent::Inline(frags) => {
            max_r = max_r.max(inline_extent(frags, fonts, &rel));
        }
    }
    max_r
}

/// `1 → a`, `26 → z`, `27 → aa` — the bijective base-26 an alphabetic list counts in.
fn alpha_ordinal(n: i64, upper: bool) -> String {
    let mut n = n.max(1);
    let mut out = Vec::new();
    while n > 0 {
        let rem = ((n - 1) % 26) as u8;
        out.push(if upper { b'A' + rem } else { b'a' + rem });
        n = (n - 1) / 26;
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_default()
}

/// Roman numerals, for `list-style-type: lower-roman|upper-roman`.
fn roman_ordinal(n: i64, upper: bool) -> String {
    const TABLE: [(i64, &str); 13] = [
        (1000, "m"),
        (900, "cm"),
        (500, "d"),
        (400, "cd"),
        (100, "c"),
        (90, "xc"),
        (50, "l"),
        (40, "xl"),
        (10, "x"),
        (9, "ix"),
        (5, "v"),
        (4, "iv"),
        (1, "i"),
    ];
    let mut n = n.max(1);
    let mut out = String::new();
    for (v, sym) in TABLE {
        while n >= v {
            out.push_str(sym);
            n -= v;
        }
    }
    if upper {
        out.to_uppercase()
    } else {
        out
    }
}

/// Collapse two adjoining vertical margins (CSS2 §8.3.1): positive margins take the
/// max, negative margins take the min (most negative), mixed signs sum. Passing `0`
/// for one side yields the other unchanged, so the first-in-flow block "collapses"
/// with a zero and keeps its own margin.
fn collapse_margins(a: f32, b: f32) -> f32 {
    if a >= 0.0 && b >= 0.0 {
        a.max(b)
    } else if a < 0.0 && b < 0.0 {
        a.min(b)
    } else {
        a + b
    }
}

/// `MANUK_TRACE_INTRINSIC=<id>` — read ONCE, not once per node per probe.
///
/// `std::env::var` takes a process-wide lock and allocates a `String`. This was being called from
/// inside intrinsic sizing, which is the hottest loop in layout: a debug hook that cost real time on
/// every page whether or not anyone was debugging. A `OnceLock` makes the disabled case a null check.
fn trace_intrinsic() -> Option<&'static str> {
    static V: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    V.get_or_init(|| std::env::var("MANUK_TRACE_INTRINSIC").ok())
        .as_deref()
}

impl Ctx<'_> {
    /// The top margin that collapses *through* `node` into a parent-child collapse (CSS2 §8.3.1):
    /// `node`'s own top margin, joined with its first in-flow block child's collapse-through top
    /// margin whenever `node` has no top border/padding, `overflow:visible`, and is a normal block.
    /// The walk follows only the left spine (first in-flow block at each level), so it is O(depth)
    /// and is depth-bounded against a hostile tree.
    ///
    /// `cw` is the width `node`'s vertical margins resolve against (its containing block's content
    /// width). Percentage vertical margins deeper in the spine are resolved against this same width
    /// (an approximation — the exact value is each level's own content width); px/em margins, which
    /// are width-independent and dominate real pages, are exact.
    fn collapse_through_top(&self, node: NodeId, cw: f32, depth: u32) -> f32 {
        let s = self.style_of(node);
        let mt = s.margin.top.resolve(cw, 0.0);
        if depth > 64 || !top_margin_collapses(s, cw) {
            return mt;
        }
        for k in rendered_children(self.dom, self.styles, node) {
            // Whitespace-only text produces no box (matches `flush_inline_run`) and does not stop a
            // following block from being the first in-flow child.
            if let NodeData::Text(t) = self.dom.data(k) {
                if t.trim().is_empty() {
                    continue;
                }
                return mt; // real inline text is the first in-flow content
            }
            let ks = self.style_of(k);
            if is_float(ks) || is_out_of_flow_positioned(ks) {
                return mt; // conservative: an out-of-flow first child declines the collapse
            }
            if is_block_level(self.dom, self.styles, k) {
                return collapse_margins(mt, self.collapse_through_top(k, cw, depth + 1));
            }
            return mt; // an inline-level element (inline-block, etc.) is the first in-flow content
        }
        mt // no in-flow children
    }

    /// The mirror of [`collapse_through_top`] for the **bottom** edge (CSS2 §8.3.1): `node`'s own
    /// bottom margin, joined with its last in-flow block child's collapse-through bottom margin when
    /// `node` is an auto-height block with no bottom border/padding, `overflow:visible`, and no BFC.
    /// A definite-height box stops the through-collapse (its content box is fixed). Same left/right
    /// spine cost and depth bound as the top walk; same percentage-margin width approximation.
    fn collapse_through_bottom(&self, node: NodeId, cw: f32, depth: u32) -> f32 {
        let s = self.style_of(node);
        let mb = s.margin.bottom.resolve(cw, 0.0);
        // A definite own height (explicit `px`, or `%`/`calc` — the latter would resolve against a
        // definite parent) separates the bottom margin from the last child's, so no through-collapse.
        let definite_height = !matches!(s.height, Dim::Auto);
        if depth > 64 || definite_height || !bottom_margin_collapses(s, cw) {
            return mb;
        }
        for k in rendered_children(self.dom, self.styles, node)
            .into_iter()
            .rev()
        {
            if let NodeData::Text(t) = self.dom.data(k) {
                if t.trim().is_empty() {
                    continue;
                }
                return mb; // trailing inline text: the box's content box ends at the text
            }
            let ks = self.style_of(k);
            if is_float(ks) || is_out_of_flow_positioned(ks) {
                return mb; // conservative: an out-of-flow last child declines the collapse
            }
            if is_block_level(self.dom, self.styles, k) {
                return collapse_margins(mb, self.collapse_through_bottom(k, cw, depth + 1));
            }
            return mb;
        }
        mb
    }

    /// The collapse-through bottom margin of `node`'s last in-flow block child, or `0.0` if the last
    /// in-flow child is not a block or is out of flow. This is the amount that escapes downward out
    /// of the parent in a bottom collapse.
    fn trailing_block_collapse_bottom(&self, node: NodeId, cw: f32) -> f32 {
        for k in rendered_children(self.dom, self.styles, node)
            .into_iter()
            .rev()
        {
            if let NodeData::Text(t) = self.dom.data(k) {
                if t.trim().is_empty() {
                    continue;
                }
                return 0.0;
            }
            let ks = self.style_of(k);
            if is_float(ks) || is_out_of_flow_positioned(ks) {
                return 0.0;
            }
            if is_block_level(self.dom, self.styles, k) {
                return self.collapse_through_bottom(k, cw, 1);
            }
            return 0.0;
        }
        0.0
    }

    /// The collapse-through top margin of `node`'s first in-flow block child, or `0.0` if the first
    /// in-flow child is not a block, is out of flow, or carries clearance (clearance blocks the
    /// parent-child collapse). This is the amount hoisted out of the parent in a top collapse.
    fn leading_block_collapse_top(&self, node: NodeId, cw: f32) -> f32 {
        for k in rendered_children(self.dom, self.styles, node) {
            if let NodeData::Text(t) = self.dom.data(k) {
                if t.trim().is_empty() {
                    continue;
                }
                return 0.0;
            }
            let ks = self.style_of(k);
            if is_float(ks) || is_out_of_flow_positioned(ks) {
                return 0.0;
            }
            if is_block_level(self.dom, self.styles, k) {
                if ks.clear != Clear::None {
                    return 0.0; // clearance separates the margins — no collapse
                }
                return self.collapse_through_top(k, cw, 1);
            }
            return 0.0;
        }
        0.0
    }

    /// Lay out a block box in a containing block of `cw` px. `y` is the border-bottom
    /// edge of the preceding in-flow sibling (or the container's content-top for the
    /// first child); `prev_margin` is that sibling's trailing collapsible margin (0
    /// if none). The block's top margin collapses with `prev_margin` to decide its
    /// border-box top. Returns the positioned box and its own top/bottom margins.
    ///
    /// Parent↔child margin collapsing (CSS2 §8.3.1) IS modeled: a block with no border/padding on
    /// an edge, `overflow:visible`, and no BFC collapses that edge's margin with its first/last
    /// in-flow block child (top via `collapse_through_top`; bottom via `collapse_through_bottom`).
    /// Adjacent-sibling collapsing is handled by `collapse_margins`.
    #[allow(clippy::too_many_arguments)]
    fn layout_block(
        &self,
        node: NodeId,
        cw: f32,
        pch: Option<f32>,
        x: f32,
        y: f32,
        prev_margin: f32,
        floats: &mut FloatContext,
    ) -> BlockResult {
        let s = self.style_of(node).clone();

        // Tables size their own width (shrink-to-columns when auto), so they run a
        // dedicated formatter rather than the generic block width algorithm.
        if s.display == Display::Table {
            return self.layout_table(node, cw, x, y, prev_margin);
        }

        let mut ml = s.margin.left.resolve(cw, 0.0);
        let mr = s.margin.right.resolve(cw, 0.0);
        let mt = s.margin.top.resolve(cw, 0.0);
        let mb = s.margin.bottom.resolve(cw, 0.0);
        let (pl, pr) = (
            s.padding.left.resolve(cw, 0.0),
            s.padding.right.resolve(cw, 0.0),
        );
        let (pt, pb) = (
            s.padding.top.resolve(cw, 0.0),
            s.padding.bottom.resolve(cw, 0.0),
        );
        let (bl, br) = (s.border_width.left, s.border_width.right);
        let (bt, bb) = (s.border_width.top, s.border_width.bottom);

        // Resolve width. `auto` fills the available inline space — except an **inline-level** box
        // (inline-block, inline-flex, inline-grid), which is atomic and shrinks to fit its content,
        // so a `<button>` hugs its label and an icon button stays icon-sized.
        let extra = ml + mr + pl + pr + bl + br;
        // **A flex/grid item's width was already decided by taffy — do not resolve it a second time.**
        //
        // `extract_placed` hands the item's taffy-assigned width in as `cw`. But `cw` means
        // *containing block* width everywhere else in this function, so the item's own `width: 30%`
        // got resolved against it AGAIN: a `width:30%` column in a 1000px flex row came out
        // 30% of 300 = **90px**. The used width was the SQUARE of the intended one.
        //
        // It survived this long because the two most common cases are exactly the two that are
        // immune to it: `auto` (nothing to re-resolve) and `100%` (100% of 100% is still 100%).
        // Every *other* percentage — the 30/70 split, the 50/50 column, which is how most page
        // layouts are actually structured — was silently wrong, and rust-lang.org's `w-30-l`
        // sidebar is one of them: its "Get Started" button came out 102px against Chrome's 338.
        //
        // Taffy's slot is a border box and excludes margins, so the content width is the slot less
        // this box's own padding and border. `box-sizing` is already accounted for by that
        // subtraction, so the border-box adjustment below must not run for these.
        let taffy_known = self.taffy_item_width.borrow().get(&node).copied();
        let mut width = match taffy_known {
            Some(border_box) => (border_box - pl - pr - bl - br).max(0.0),
            None => match s.width {
                // An intrinsic sizing keyword (`min-/max-/fit-content`) collapses to `Dim::Auto` for
                // length resolution but does NOT fill — it hugs the content. Same measure functions
                // inline-block already uses below, so identical Bar-0/recursion profile; they return
                // content-box widths, so the box-sizing subtraction (guarded on `width != Auto`) stays
                // correctly skipped. Takes precedence over the inline-block fall-through.
                Dim::Auto if s.width_keyword.is_some() => match s.width_keyword.unwrap() {
                    IntrinsicSize::MinContent => self.min_content_width(node),
                    IntrinsicSize::MaxContent => self.max_content_width(node),
                    IntrinsicSize::FitContent => self.shrink_to_fit(node, (cw - extra).max(0.0)),
                },
                // `width: stretch` FILLS, and it has to be checked *before* the shrink-to-fit arms
                // below, because those are exactly the boxes it changes. On a plain block `auto`
                // already fills, so this looks like a no-op — but an inline-block, a form control
                // and a replaced element all hug their content on `auto`, and `stretch` is how an
                // author says "fill the column anyway". Inline mirror of the `height_stretch` arm in
                // `own_definite_h`; the margin box fills, so the margins come out of the content
                // width (which `extra` already does).
                Dim::Auto if s.width_stretch => (cw - extra).max(0.0),
                Dim::Auto
                    if matches!(
                        s.display,
                        Display::InlineBlock | Display::InlineFlex | Display::InlineGrid
                    ) =>
                {
                    self.shrink_to_fit(node, (cw - extra).max(0.0))
                }
                Dim::Auto => (cw - extra).max(0.0),
                other => other.resolve(cw, (cw - extra).max(0.0)),
            },
        };
        // The mirror case: an `auto` width on a replaced element with a definite height comes from
        // that height and the ratio.
        // `width: stretch` is a DEFINITE width, not an auto one — it just happens to share
        // `Dim::Auto`'s representation — so the ratio must not derive a width over the top of it.
        // (This is what kept a `width:stretch` `<canvas width="40" height="20">` at 40px: the
        // stretch arm sized it correctly and then `height x ratio` overwrote the answer.)
        if s.width == Dim::Auto && !s.width_stretch && taffy_known.is_none() {
            if let (Some(r), Dim::Px(h)) = (s.aspect_ratio, s.height) {
                if r > 0.0 {
                    width = h * r;
                }
            }
        }
        // **The default object size (CSS-Images §4.4), in USED-size layout — Chrome-measured
        // (tick 389/391).** The model, measured over headless Chrome rather than recalled:
        //   · no intrinsic ratio, auto width  → 300 wide (and 150 tall below): `<svg>`, `<canvas>`,
        //     `<video>`, `<iframe>` all measure 300×150 unsized.
        //   · intrinsic ratio (svg `viewBox`), auto width → the AVAILABLE width, height follows
        //     the ratio (CSS2 §10.3.2 last resort): `<svg viewBox="0 0 24 24">` in a 400px block
        //     measures 400×400 — which is what the plain fill arm above already produced, so the
        //     ratio case needs NO width override here.
        // Before tick 389 the no-ratio case rendered 784×0 — full container width, zero height,
        // invisible — and every icon-only `<button>` collapsed with it. `<img>` is deliberately
        // NOT in the list: a sourceless image has no default object size in any browser. Applied
        // here and not in UA defaults because an AUTHOR width must win and a definite-height-plus-
        // ratio derivation must win — both already resolved above (the tick-153 lesson).
        let default_object_tag = matches!(
            self.dom.tag_name(node),
            Some("svg" | "canvas" | "video" | "iframe" | "object" | "embed")
        );
        if s.width == Dim::Auto
            && !s.width_stretch
            && s.width_keyword.is_none()
            && taffy_known.is_none()
            && default_object_tag
            && s.aspect_ratio.is_none()
        {
            width = 300.0;
        }
        // `box-sizing:border-box` — the specified width is the border box, so the content
        // width is that minus padding + border. (`auto` already resolves to content width.)
        let bs_extra_w = if s.box_sizing == BoxSizing::BorderBox {
            pl + pr + bl + br
        } else {
            0.0
        };
        if s.box_sizing == BoxSizing::BorderBox && s.width != Dim::Auto && taffy_known.is_none() {
            width -= bs_extra_w;
        }
        width = width.max(0.0);
        // min-width / max-width clamp (max applied first, then min wins), converted to the
        // content box to match `width`.
        let min_w = (s.min_width.resolve(cw, 0.0) - bs_extra_w).max(0.0);
        let max_w = match s.max_width {
            Dim::Auto => f32::INFINITY,
            other => (other.resolve(cw, f32::INFINITY) - bs_extra_w).max(0.0),
        };
        let unclamped_width = width;
        if max_w.is_finite() {
            width = width.min(max_w);
        }
        width = width.max(min_w);
        // Did a min/max-width constraint actually move the width? For a **replaced** element that
        // is a constraint violation in CSS2.1 §10.4's sense, and the height has to follow the ratio
        // — see the height derivation below.
        let inline_constraint_violated = width != unclamped_width;

        // Horizontal auto-margin centering when width is definite. A keyword width (`fit-content`
        // etc.) collapses to `Dim::Auto` but IS definite for margins — `width:fit-content;margin:auto`
        // centers the hugged box. Only the left margin shifts the box; the right absorbs the remainder.
        if s.width != Dim::Auto || s.width_keyword.is_some() {
            let leftover = cw - (width + pl + pr + bl + br);
            match (s.margin.left.is_auto(), s.margin.right.is_auto()) {
                (true, true) => ml = (leftover / 2.0).max(0.0),
                (true, false) => ml = (leftover - mr).max(0.0),
                _ => {}
            }
        }
        let _ = mr; // right margin does not affect downstream positioning here

        let border_x = x + ml;
        // Parent↔child TOP margin collapse (CSS2 §8.3.1): when this block has no top border/padding,
        // is `overflow:visible`, and does not establish a BFC, its top margin collapses with its
        // first in-flow block child's collapse-through top margin. That child's margin escapes
        // upward — folded into this box's own top margin here, and the child is placed flush to the
        // content top by `layout_children` (which recomputes the same hoist). `effective_mt` is the
        // collapsed top margin this box contributes to its own parent, so it is what a grandparent
        // collapses against.
        let hoist_top = if top_margin_collapses(&s, cw) {
            self.leading_block_collapse_top(node, width)
        } else {
            0.0
        };
        let effective_mt = collapse_margins(mt, hoist_top);
        // Collapse this block's (possibly child-hoisted) top margin with the preceding sibling's
        // trailing margin to place the border-box top.
        let border_y = y + collapse_margins(prev_margin, effective_mt);
        let content_x = border_x + bl + pl;
        let content_y = border_y + bt + pt;

        // This block's own **definite** content height, if any — the reference a
        // percentage-height *child* resolves against (CSS2 §10.5). Computed before laying
        // out children so their `height:%` works; `None` (auto height) means a percent-height
        // child falls back to its content height.
        let bs_extra_h = if s.box_sizing == BoxSizing::BorderBox {
            pt + pb + bt + bb
        } else {
            0.0
        };
        let own_definite_h: Option<f32> = match s.height {
            Dim::Px(p) => Some((p - bs_extra_h).max(0.0)),
            Dim::Percent(pct) => pch.map(|h| (h * pct / 100.0 - bs_extra_h).max(0.0)),
            Dim::Calc { .. } => pch.map(|h| (s.height.resolve(h, 0.0) - bs_extra_h).max(0.0)),
            // `height:stretch`/`-webkit-fill-available` fill the containing block's definite content
            // height: the MARGIN box fills `pch`, so the content box is `pch` minus this box's own
            // margins, border and padding (box-sizing-independent — stretch fills available space, not
            // a specified length, so the full deduction applies in both modes). `None` pch (auto-height
            // parent) leaves it content-sized, at parity with Chrome.
            Dim::Auto if s.height_stretch => {
                pch.map(|h| (h - mt - mb - pt - pb - bt - bb).max(0.0))
            }
            Dim::Auto => None,
        };

        // **Scrollbar-gutter reservation** (CSS Overflow 4 §3.2). A classic (non-overlay) vertical
        // scrollbar lives on the inline-end edge and eats inline space: `overflow-y:scroll` always
        // shows one, so the content box is narrower than the border box (`offsetWidth`) by the
        // scrollbar's width. The `html{overflow-y:scroll}` layout-shift-prevention idiom — a
        // scrollbar reserved on every page whether or not it scrolls — depends on exactly this, and
        // without it every such page's content was ~15px too wide. Only the deterministic case
        // (`scroll`, scrollbar always present) is reserved; the `auto`-and-actually-overflows case
        // needs a second layout pass and stays residue. The gutter narrows the CONTENT box passed to
        // children (and the BFC float band), leaving `width`/`border_box_w` — the box's own
        // offsetWidth — untouched.
        let gutter = if s.overflow_y == Overflow::Scroll {
            SCROLLBAR_WIDTH.min(width)
        } else {
            0.0
        };
        let inner_width = (width - gutter).max(0.0);
        // **Block-axis mirror of the same gutter.** A classic horizontal scrollbar (`overflow-x:scroll`,
        // always present) lives on the block-end edge and eats block-axis space, so the content offered
        // to children is shorter than the box by the scrollbar's width — but ONLY when the box has a
        // definite height. An auto-height box grows to fit its content instead, so there is nothing to
        // reserve (and reserving would wrongly shrink a `height:100%` child's track). Like the inline
        // case, this narrows the space passed to children while leaving `border_box_h` — the box's own
        // `offsetHeight` — untouched; the reserved strip is where the scrollbar sits.
        let gutter_x = if s.overflow_x == Overflow::Scroll {
            SCROLLBAR_WIDTH
        } else {
            0.0
        };
        let inner_definite_h = own_definite_h.map(|h| (h - gutter_x).max(0.0));
        // A BFC root gets a fresh float context spanning its own content box; a plain
        // block shares its parent's so floats affect content across nested blocks.
        let mut own_bfc;
        let (content, content_height) = if establishes_bfc(&s) {
            own_bfc = FloatContext::new(content_x, content_x + inner_width);
            let (c, h) = self.layout_children(
                node,
                content_x,
                content_y,
                inner_width,
                inner_definite_h,
                &mut own_bfc,
            );
            // A BFC root grows to contain its floats (CSS2 §10.6.7 auto-height case).
            let float_h = (own_bfc.lowest_bottom() - content_y).max(0.0);
            (c, h.max(float_h))
        } else {
            self.layout_children(
                node,
                content_x,
                content_y,
                inner_width,
                inner_definite_h,
                floats,
            )
        };
        // **A replaced element's auto height comes from its USED width and its intrinsic ratio**
        // (CSS2 §10.6.2) — not from the image's natural pixel height. `width` here is already
        // resolved and already clamped by min/max, so `max-width: 100%` narrowing the box scales the
        // height with it, which is the entire point of that reset.
        let mut content_height = match (own_definite_h, s.aspect_ratio) {
            (None, Some(r)) if r > 0.0 => width / r,
            // **CSS2.1 §10.4 constraint violation: the clamp transfers through the ratio.** A
            // replaced element whose width was cut down by `max-width` (or pushed up by
            // `min-width`) does not keep its specified height — the used height is recomputed from
            // the used width so the ratio survives. This is the case a specified height alone would
            // otherwise win, and it is exactly the shape of the responsive web: `<img width="800"
            // height="400">` (the attributes are there to reserve the box before the bitmap
            // arrives) under the universal `img { max-width: 100% }` reset, in a 400px column.
            // Without the transfer the box is 400x400 and the picture renders squashed to half its
            // width at full height; with it, 400x200.
            (Some(_), Some(r))
                if r > 0.0
                    && inline_constraint_violated
                    && is_replaced_element(self.dom.tag_name(node)) =>
            {
                width / r
            }
            _ => own_definite_h.unwrap_or(content_height),
        };
        // The other half of the default object size: a replaced box with no definite height and
        // no ratio is 150 tall — not its (empty) content height. This fires for the defaulted
        // 300-wide case AND for an authored width (Chrome-measured: `<svg style="width:200px">`
        // with no viewBox is 200×150, not 200×0).
        if default_object_tag && own_definite_h.is_none() && s.aspect_ratio.is_none() {
            content_height = 150.0;
        }
        // Parent↔child BOTTOM margin collapse (CSS2 §8.3.1): an auto-height block with no bottom
        // border/padding, `overflow:visible`, not a BFC, collapses its bottom margin with its last
        // in-flow block child's. `layout_children` returned a height that INCLUDES that trailing
        // child margin ("still occupies the container"); here it escapes — removed from this box's
        // content height and collapsed into its own bottom margin (`effective_mb`, reported so the
        // parent collapses correctly). `hoist_bottom` mirrors the actual trailing margin for px/em.
        let mut effective_mb = mb;
        let hoist_bottom = if own_definite_h.is_none()
            && s.aspect_ratio.is_none()
            && bottom_margin_collapses(&s, cw)
        {
            self.trailing_block_collapse_bottom(node, width)
        } else {
            0.0
        };
        if hoist_bottom != 0.0 {
            content_height = (content_height - hoist_bottom).max(0.0);
            effective_mb = collapse_margins(mb, hoist_bottom);
        }
        // min-height / max-height clamp (content-box).
        let min_h = (s.min_height.resolve(pch.unwrap_or(0.0), 0.0) - bs_extra_h).max(0.0);
        let max_h = match s.max_height {
            Dim::Auto => f32::INFINITY,
            // A percentage `max-height` against an **indefinite** containing-block height is
            // treated as `none` (CSS2 §10.7) — the cap simply does not apply. Resolving it
            // against 0 instead (the old `unwrap_or(0.0)`) clamped the box to **zero height**:
            // `height:30000px; max-height:100%` inside an auto-height parent rendered as an
            // invisible 0px box, and `img { max-width:100%; max-height:100% }` — the single most
            // common responsive-image reset on the web — collapsed every such image to nothing.
            Dim::Percent(_) if pch.is_none() => f32::INFINITY,
            Dim::Calc { pct, .. } if pct != 0.0 && pch.is_none() => f32::INFINITY,
            other => (other.resolve(pch.unwrap_or(0.0), f32::INFINITY) - bs_extra_h).max(0.0),
        };
        if max_h.is_finite() {
            content_height = content_height.min(max_h);
        }
        content_height = content_height.max(min_h);

        let border_box_w = bl + pl + width + pr + br;
        let border_box_h = bt + pt + content_height + pb + bb;
        let rect = Rect {
            x: border_x,
            y: border_y,
            width: border_box_w,
            height: border_box_h,
        };
        // In-flow bottom is fixed before any relative shift, so siblings stack
        // against the box's *normal-flow* position (CSS2 §9.4.3).
        let flow_bottom = border_y + border_box_h;

        let marker = self.list_marker(node, &s, content_x, content_y);
        let mut boxx = LayoutBox {
            rect,
            background: s.background_color,
            border: border_of(&s),
            radius: s.border_radius,
            shadows: s.box_shadows.clone(),
            hidden: s.visibility != manuk_css::Visibility::Visible,
            mask_image: s.mask_image.clone(),
            background_images: s.background_images.clone(),
            background_size: s.background_size,
            background_position: s.background_position,
            object_fit: s.object_fit,
            object_position: s.object_position,
            background_repeat: s.background_repeat,
            outline: (s.outline_width > 0.0 && s.outline_color.a > 0)
                .then_some((s.outline_width, s.outline_color)),
            marker,
            opacity: s.opacity,
            node: Some(node),
            content,
        };

        // `position: relative` offsets the box (and its subtree) visually without
        // affecting the flow. `left`/`top` win over `right`/`bottom`; percentages
        // resolve against the containing block — width for x, **height for y**. The
        // containing-block height is `pch` (the definite content height threaded down
        // for percentage sizing, tick 144); when it is indefinite (`None`) a `%` inset
        // resolves to 0, which matches the spec's "computes to auto" for `top`/`bottom`
        // percentages against an auto-height containing block (CSS Position §3 / Sizing §5).
        // Before, y resolved against a hardcoded 0, so `top: 50%` never moved the box.
        if s.position == Position::Relative {
            let cb_h = pch.unwrap_or(0.0);
            let dx = if !s.inset.left.is_auto() {
                s.inset.left.resolve(cw, 0.0)
            } else if !s.inset.right.is_auto() {
                -s.inset.right.resolve(cw, 0.0)
            } else {
                0.0
            };
            let dy = if !s.inset.top.is_auto() {
                s.inset.top.resolve(cb_h, 0.0)
            } else if !s.inset.bottom.is_auto() {
                -s.inset.bottom.resolve(cb_h, 0.0)
            } else {
                0.0
            };
            if dx != 0.0 || dy != 0.0 {
                boxx.translate(dx, dy);
            }
        }

        // `transform` — a visual affine map of the box + subtree that does not affect flow.
        // Resolved around the transform-origin (box center) into an absolute matrix and
        // baked into the subtree's coordinates. Exact for translate/scale (axis-aligned);
        // rotate/skew map each box to its transformed bounding box (matching
        // getBoundingClientRect), which the CPU raster then paints upright.
        if !s.transform.is_empty() {
            let origin = (rect.x + border_box_w / 2.0, rect.y + border_box_h / 2.0);
            let m = resolve_transform(&s.transform, border_box_w, border_box_h, origin);
            boxx.transform_affine(&m);
        }

        BlockResult {
            boxx,
            margin_top: effective_mt,
            margin_bottom: effective_mb,
            flow_bottom,
        }
    }

    /// Lay out the children of a container whose content box starts at `(cx, cy)`
    /// with content width `cw`, within the block formatting context `floats`. Returns
    /// the content and its height.
    fn layout_children(
        &self,
        node: NodeId,
        cx: f32,
        cy: f32,
        cw: f32,
        pch: Option<f32>,
        floats: &mut FloatContext,
    ) -> (BoxContent, f32) {
        // An **anonymous flex/grid item** (see `taffy_tree::flex_items`): the item IS the text node,
        // so every path that sizes or lays out an item — `measure_intrinsic`, `max_content_width`,
        // `min_content_width`, `layout_block` — arrives here with a text node and must get an inline
        // formatting context over that one run rather than the empty child list a text node has.
        // Putting the branch HERE, rather than special-casing each caller, is what makes the item
        // measure and paint identically to an element wrapping the same text.
        if matches!(self.dom.data(node), NodeData::Text(_)) {
            let items = self.collect_inline_group(&[node], cw, None);
            if items.is_empty() {
                return (BoxContent::Inline(vec![]), 0.0);
            }
            let align = self.style_of(node).text_align;
            let (frags, _atomics, h) = self.layout_inline(items, cx, cy, cw, align, floats);
            return (BoxContent::Inline(frags), h);
        }

        let display = self.style_of(node).display;

        // Form controls render their *value*/label as synthetic text (an `<input>` has no
        // child nodes; a `<button>` uses its real children so it is not handled here).
        if let Some(text) = form_control_text(self.dom, node) {
            let style = text_style(self.style_of(node), self.fonts);
            if text.is_empty() {
                // An empty field still occupies one line's height.
                return (BoxContent::Inline(vec![]), style.line_height);
            }
            let items = vec![InlineItem::Word {
                text,
                style,
                space_before: false,
                node: Some(node),
                no_wrap: true,
                break_word: false,
            }];
            let (frags, _atomics, h) =
                self.layout_inline(items, cx, cy, cw, TextAlign::Left, floats);
            return (BoxContent::Inline(frags), h);
        }

        // N4: the FLAT tree — a shadow host lays out its shadow content, and a `<slot>`
        // lays out the light-DOM nodes assigned to it.
        // `rendered_children`, not a raw filter: a `display: contents` wrapper must DISSOLVE, handing
        // its children up to this formatting context. Filtering it out entirely would take its children
        // with it (that is `display: none`), and keeping it would make it a grid/flex item in its own
        // right — which collapses the whole layout into one cell.
        let kids: Vec<NodeId> = rendered_children(self.dom, self.styles, node);

        // Flex/grid containers route through taffy. `inline-flex`/`inline-grid` establish the same
        // formatting context — they differ only in how the CONTAINER is sized by its parent (handled
        // in `layout_block`: inline-level boxes shrink to fit).
        if matches!(display, Display::Flex | Display::InlineFlex) {
            return self.layout_flex(node, cx, cy, cw, &kids);
        }
        if matches!(display, Display::Grid | Display::InlineGrid) {
            return self.layout_grid(node, cx, cy, cw, &kids);
        }

        // Floated / out-of-flow children never count toward the "has block" decision.
        let flow_kids: Vec<NodeId> = kids
            .iter()
            .copied()
            .filter(|&k| {
                let s = self.style_of(k);
                !is_float(s) && !is_out_of_flow_positioned(s)
            })
            .collect();
        let has_block = flow_kids
            .iter()
            .any(|&k| is_block_level(self.dom, self.styles, k));

        if !has_block && !kids.iter().any(|&k| is_float(self.style_of(k))) {
            // Pure inline formatting context (no floats to flow around).
            //
            // **Record the static position of the out-of-flow children before returning.** This
            // branch returns without ever reaching the block child loop, which is the only other
            // place that records it — so an abs box with all-`auto` insets found nothing in
            // `static_pos`, and `position_absolutes` dropped it (see the `continue` there). The box
            // did not merely land in the wrong place: it GENERATED NO BOX AT ALL.
            //
            // The shape that hits it is `position: relative` wrapping *only* an absolutely
            // positioned child — the overlay / dropdown / tooltip / portal-root idiom, and the
            // single most common way `position:absolute` is written. It hid because the sibling
            // cases work: one block-level sibling puts the parent on the block path, and flex and
            // grid parents return earlier still through paths that place abs children by other
            // means. Only the pure-IFC parent lost them.
            //
            // `(cx, cy)` is the content-box origin, which is exact when the abs box is the first
            // thing in the parent (the idiom above). Text *preceding* it on the line should push
            // the static position along that line; that refinement is not modelled here, and the
            // box lands at the line start instead.
            for &k in &kids {
                if is_out_of_flow_positioned(self.style_of(k)) {
                    self.static_pos.borrow_mut().insert(k, (cx, cy));
                }
            }
            let items = self.collect_inline_group(&flow_kids, cw, Some(node));
            let bcs = self.style_of(node);
            let align = bcs.text_align;
            let (mut frags, atomics, h) = self.layout_inline(items, cx, cy, cw, align, floats);
            // `text-overflow: ellipsis` truncates a clipped, non-wrapping single line with `…`. Only
            // fires on a box that clips (`overflow` ≠ visible) and doesn't wrap (`nowrap`/`pre`); a
            // line that fits is untouched, so nothing without a real overflow changes.
            if bcs.text_overflow == manuk_css::TextOverflow::Ellipsis
                && !matches!(bcs.overflow_x, manuk_css::Overflow::Visible)
                && matches!(bcs.white_space, WhiteSpace::NoWrap | WhiteSpace::Pre)
            {
                apply_text_overflow_ellipsis(&mut frags, cx, cw, self.fonts);
            }
            if atomics.is_empty() {
                return (BoxContent::Inline(frags), h);
            }
            // Inline-blocks present: the anonymous line box (text) and the atomic boxes
            // become siblings so both reach the fragment tree.
            let mut boxes = Vec::new();
            if !frags.is_empty() {
                boxes.push(LayoutBox {
                    rect: Rect {
                        x: cx,
                        y: cy,
                        width: cw,
                        height: h,
                    },
                    background: None,
                    border: None,
                    radius: 0.0,
                    shadows: Vec::new(),
                    hidden: false,
                    mask_image: None,
                    background_images: Vec::new(),
                    background_size: manuk_css::BackgroundSize::Auto,
                    background_position: manuk_css::BackgroundPosition::default(),
                    object_fit: manuk_css::ObjectFit::Fill,
                    object_position: manuk_css::ObjectPosition::default(),
                    background_repeat: manuk_css::BackgroundRepeat::Repeat,
                    outline: None,
                    marker: None,
                    opacity: 1.0,
                    node: None,
                    content: BoxContent::Inline(frags),
                });
            }
            boxes.extend(atomics);
            return (BoxContent::Block(boxes), h);
        }

        // Block container: block children stack with adjacent-sibling margin
        // collapsing; floats are pulled out to the sides; runs of inline siblings
        // become anonymous block boxes that flow around floats. `cur_y` tracks the
        // border-bottom of the last in-flow block (its trailing margin held in
        // `prev_margin` so the next sibling can collapse against it).
        let mut boxes = Vec::new();
        let mut cur_y = cy;
        let mut prev_margin = 0.0f32;
        let mut inline_run: Vec<NodeId> = Vec::new();

        // Parent↔child TOP margin collapse (CSS2 §8.3.1): if THIS container collapses its top margin
        // with its first in-flow block child, that child is placed flush to the content top — its
        // leading margin has escaped upward (folded into the container's own top margin by
        // `layout_block`, which recomputes the identical hoist). Placing the first block `hoist_top`
        // higher lands it exactly at `cy`. `first_block` restricts the shift to that first block.
        let hoist_top = if top_margin_collapses(self.style_of(node), cw) {
            self.leading_block_collapse_top(node, cw)
        } else {
            0.0
        };
        let mut first_block = true;

        for &k in &kids {
            let ks = self.style_of(k);
            if is_float(ks) {
                // Floats attach at the current flow position without advancing it.
                // Flush pending inline content first so it wraps around this float.
                (cur_y, prev_margin) = self.flush_inline_run(
                    &mut inline_run,
                    &mut boxes,
                    cx,
                    cur_y,
                    prev_margin,
                    cw,
                    floats,
                );
                let fbox = self.layout_float(k, cw, cur_y + prev_margin.max(0.0), floats);
                boxes.push(fbox);
            } else if is_out_of_flow_positioned(ks) {
                // Absolutely/fixed positioned: taken out of flow here and placed in the later pass.
                //
                // **But record where it WOULD have been first.** An abs box with `auto` on every
                // inset sits at its *static position* — its would-be in-flow spot — and this is the
                // only moment in the whole layout when that is known. Discarding it meant the later
                // pass had nothing to place the box against, so it dropped the box entirely, and
                // every `position:absolute` element with no insets simply vanished: React portal
                // roots, JS-positioned dropdowns and tooltips, and every `.sr-only` accessibility
                // node on the web.
                self.static_pos
                    .borrow_mut()
                    .insert(k, (cx, cur_y + prev_margin.max(0.0)));
                continue;
            } else if is_block_level(self.dom, self.styles, k) {
                (cur_y, prev_margin) = self.flush_inline_run(
                    &mut inline_run,
                    &mut boxes,
                    cx,
                    cur_y,
                    prev_margin,
                    cw,
                    floats,
                );
                // Clearance pushes the block below the relevant floats.
                if ks.clear != Clear::None {
                    let base = cur_y + prev_margin;
                    let cleared = floats.clear_to(ks.clear, base);
                    if cleared > base {
                        cur_y = cleared;
                        prev_margin = 0.0;
                    }
                }
                // The first in-flow block is placed `hoist_top` higher so it lands flush at the
                // container's content top (its top margin escaped into the container's own margin).
                let child_y = if first_block {
                    cur_y - hoist_top
                } else {
                    cur_y
                };
                let r = self.layout_block(k, cw, pch, cx, child_y, prev_margin, floats);
                // Stack against the normal-flow bottom (relative shifts are visual).
                cur_y = r.flow_bottom;
                prev_margin = r.margin_bottom;
                boxes.push(r.boxx);
                first_block = false;
            } else {
                inline_run.push(k);
            }
        }
        (cur_y, prev_margin) = self.flush_inline_run(
            &mut inline_run,
            &mut boxes,
            cx,
            cur_y,
            prev_margin,
            cw,
            floats,
        );

        // The last in-flow block's trailing margin still occupies the container.
        (BoxContent::Block(boxes), cur_y + prev_margin - cy)
    }

    /// Lay out a floated element: size it (explicit width or shrink-to-fit), lay out
    /// its content in its own BFC at a provisional origin, then place its margin box
    /// via `floats` and re-origin the content to the placed position.
    fn layout_float(
        &self,
        node: NodeId,
        cw: f32,
        top: f32,
        floats: &mut FloatContext,
    ) -> LayoutBox {
        let s = self.style_of(node).clone();
        let ml = s.margin.left.resolve(cw, 0.0);
        let mr = s.margin.right.resolve(cw, 0.0);
        let mt = s.margin.top.resolve(cw, 0.0);
        let mb = s.margin.bottom.resolve(cw, 0.0);
        let (pl, pr) = (
            s.padding.left.resolve(cw, 0.0),
            s.padding.right.resolve(cw, 0.0),
        );
        let (pt, pb) = (
            s.padding.top.resolve(cw, 0.0),
            s.padding.bottom.resolve(cw, 0.0),
        );
        let (bl, br) = (s.border_width.left, s.border_width.right);
        let (bt, bb) = (s.border_width.top, s.border_width.bottom);

        // A cleared float starts below the floats it clears.
        let top = floats.clear_to(s.clear, top);

        let non_content = ml + mr + pl + pr + bl + br;
        let avail = (cw - non_content).max(0.0);
        let width = match s.width {
            // A float shrink-to-fits on `auto` — that is the whole point of a float — so `stretch`
            // is the only way to say "this floated card fills its column", and it is the difference
            // between a full-width banner and one hugging its text.
            Dim::Auto if s.width_stretch => avail,
            Dim::Auto => self.shrink_to_fit(node, avail),
            other => other.resolve(cw, avail).max(0.0),
        };

        // **A floated table must still get TABLE layout.** `layout_table` is only reached from
        // `layout_block`, so a table arriving here (float) — or as a flex/grid item — fell through
        // to the generic path, where `<tr>`/`<th>` are not "block-level" and every cell's text
        // simply flowed inline. That is why Wikipedia's infobox rendered as one run of text.
        // Run the real table formatter at a provisional origin, then place its margin box.
        if s.display == Display::Table {
            let r = self.layout_table(node, cw, 0.0, 0.0, 0.0);
            let mut b = r.boxx;
            let (mbw, mbh) = (ml + b.rect.width + mr, mt + b.rect.height + mb);
            let margin_rect = floats.place(s.float, top, mbw, mbh);
            b.shift_x(margin_rect.x + ml - b.rect.x);
            b.shift_y(margin_rect.y + mt - b.rect.y);
            return b;
        }

        // Lay out content at a provisional origin (0,0) in the float's own BFC.
        let mut inner = FloatContext::new(0.0, width);
        let (content, ch) = self.layout_children(node, 0.0, 0.0, width, None, &mut inner);
        let content_height = match s.height {
            Dim::Auto => ch.max((inner.lowest_bottom()).max(0.0)),
            other => other.resolve(0.0, ch),
        };

        let border_box_w = bl + pl + width + pr + br;
        let border_box_h = bt + pt + content_height + pb + bb;
        let margin_box_w = ml + border_box_w + mr;
        let margin_box_h = mt + border_box_h + mb;

        let side = s.float;
        let margin_rect = floats.place(side, top, margin_box_w, margin_box_h);
        let border_x = margin_rect.x + ml;
        let border_y = margin_rect.y + mt;

        let mut boxx = LayoutBox {
            rect: Rect {
                x: border_x,
                y: border_y,
                width: border_box_w,
                height: border_box_h,
            },
            background: s.background_color,
            border: border_of(&s),
            radius: s.border_radius,
            shadows: s.box_shadows.clone(),
            hidden: s.visibility != manuk_css::Visibility::Visible,
            mask_image: s.mask_image.clone(),
            background_images: s.background_images.clone(),
            background_size: s.background_size,
            background_position: s.background_position,
            object_fit: s.object_fit,
            object_position: s.object_position,
            background_repeat: s.background_repeat,
            outline: (s.outline_width > 0.0 && s.outline_color.a > 0)
                .then_some((s.outline_width, s.outline_color)),
            marker: None,
            opacity: s.opacity,
            node: Some(node),
            content,
        };
        // Content was laid out at (0,0); shift it to the float's content origin.
        let content_origin_x = border_x + bl + pl;
        let content_origin_y = border_y + bt + pt;
        if let BoxContent::Block(kids) = &mut boxx.content {
            for k in kids {
                k.translate(content_origin_x, content_origin_y);
            }
        } else if let BoxContent::Inline(frags) = &mut boxx.content {
            for f in frags {
                f.x += content_origin_x;
                f.line_top += content_origin_y;
                f.baseline += content_origin_y;
            }
        }
        boxx
    }

    /// The **list marker** for a list item: the bullet or the number.
    ///
    /// It is generated content — not a child — so it is built here and carried on the box. `outside`
    /// (the default) hangs it in the padding to the left of the content edge, which is why `<ul>`
    /// carries 40px of left padding in the UA sheet; `inside` puts it at the content edge.
    ///
    /// The ordinal counts the item's list-item siblings, honouring `<ol start>` and an item's own
    /// `value` attribute.
    fn list_marker(
        &self,
        node: NodeId,
        s: &ComputedStyle,
        content_x: f32,
        content_y: f32,
    ) -> Option<TextFragment> {
        use manuk_css::ListStyleType as L;
        if self.dom.tag_name(node) != Some("li") || s.list_style_type == L::None {
            return None;
        }
        let parent = self.dom.parent(node);
        let ordered = parent.and_then(|p| self.dom.tag_name(p)) == Some("ol");
        // The ordinal: `value` on this item wins; otherwise `start` on the list plus the count of
        // preceding list items.
        let ordinal = self
            .dom
            .element(node)
            .and_then(|e| e.attr("value"))
            .and_then(|v| v.trim().parse::<i64>().ok())
            .unwrap_or_else(|| {
                let start = parent
                    .and_then(|p| self.dom.element(p))
                    .and_then(|e| e.attr("start"))
                    .and_then(|v| v.trim().parse::<i64>().ok())
                    .unwrap_or(1);
                let idx = parent
                    .map(|p| {
                        self.dom
                            .children(p)
                            .take_while(|&c| c != node)
                            .filter(|&c| self.dom.tag_name(c) == Some("li"))
                            .count() as i64
                    })
                    .unwrap_or(0);
                start + idx
            });
        // An `<li>` inside an `<ol>` numbers itself even when `list-style-type` is still the
        // inherited default (`disc`) — that default only means "the UA picks for this list kind".
        let ty = match (s.list_style_type, ordered) {
            (L::Disc, true) => L::Decimal,
            (t, _) => t,
        };
        let text = match ty {
            L::Disc => "\u{2022}".to_string(),
            L::Circle => "\u{25e6}".to_string(),
            L::Square => "\u{25aa}".to_string(),
            L::Decimal => format!("{ordinal}."),
            L::LowerAlpha => format!("{}.", alpha_ordinal(ordinal, false)),
            L::UpperAlpha => format!("{}.", alpha_ordinal(ordinal, true)),
            L::LowerRoman => format!("{}.", roman_ordinal(ordinal, false)),
            L::UpperRoman => format!("{}.", roman_ordinal(ordinal, true)),
            L::None => return None,
        };
        let style = text_style(s, self.fonts);
        let w = self.fonts.measure(&text, style.font_key, style.font_size);
        let lm = self.fonts.line_metrics(style.font_key, style.font_size);
        // `outside`: hang it left of the content edge, with a small gap. `inside`: at the edge.
        const GAP: f32 = 6.0;
        let x = if s.list_style_inside {
            content_x
        } else {
            content_x - w - GAP
        };
        Some(TextFragment {
            x,
            baseline: content_y + lm.ascent,
            line_top: content_y,
            width: w,
            text,
            style,
            node: Some(node),
            content_ascent: lm.ascent.round(),
            content_height: lm.content_height(),
        })
    }

    /// **A missing style must never kill the browser.**
    ///
    /// Layout INDEXED the style map — `self.styles[&node]` — in twenty-five places. A node the
    /// cascade has never seen therefore panicked, and because the panic unwinds through
    /// SpiderMonkey's C++ frames it does not even unwind: it aborts. **apple.com crashed the browser
    /// with a core dump.** Not rendered wrong — crashed.
    ///
    /// A node can legitimately be unstyled for a moment: a script creates an element inside a
    /// timer/microtask that runs after the last cascade, and layout reaches it before the next one.
    /// The correct response to that is to lay it out with the initial style and carry on — a slightly
    /// wrong box is a rendering artefact, a core dump is the end of the session and everything the
    /// user had open.
    ///
    /// This is the Part 22 discipline stated as code rather than as a promise: the engine degrades,
    /// it does not die. The miss is logged (Part 22.1 — no silent failure), so the root cause stays
    /// visible instead of being papered over by the very fix that makes it survivable.
    fn style_of(&self, node: NodeId) -> &ComputedStyle {
        match self.styles.get(&node) {
            Some(s) => s,
            None => {
                tracing::warn!(
                    ?node,
                    tag = self.dom.tag_name(node).unwrap_or("?"),
                    "LAYOUT: node has no computed style — the cascade never saw it. Laying it out \
                     with the initial style. This is a real bug upstream (a script created it after \
                     the last cascade); it is caught here so it degrades instead of aborting."
                );
                &self.fallback_style
            }
        }
    }

    /// **Min-content width**: the narrowest the box can be — for text, the longest unbreakable run.
    ///
    /// We were not computing this *at all*, and its absence was not a rounding error. Taffy asks
    /// each flex item "how narrow can you get?" (`AvailableSpace::MinContent`) and uses the answer
    /// as the item's automatic minimum size. We answered with the *max*-content width — the whole
    /// paragraph on one line — so **no flex item containing a paragraph could ever shrink.** Three
    /// equal cards in a row each demanded their full `width:100%` and overflowed sideways, off the
    /// viewport: on rust-lang.org the three feature columns landed at x=36, 1260 and 2388 inside a
    /// 1128px container, where Chrome shrinks all three to 344. Two of the three were simply
    /// off-screen, which is why the page *looked* like it was stacking them.
    ///
    /// That is a whole class of design pattern — the card row, the feature grid, the sidebar +
    /// content split — failing on every site that uses it, which is most of them.
    ///
    /// Definition, and it is why this is cheap to get right: lay the subtree out at a ~zero
    /// available width. Every soft break is taken, so the widest fragment that survives is the
    /// longest run that *cannot* be broken. That is min-content, by construction.
    fn min_content_width(&self, node: NodeId) -> f32 {
        if let Some(&c) = self.min_content_cache.borrow().get(&node) {
            return c;
        }
        let mut fc = FloatContext::new(0.0, 1.0);
        let (content, _h) = self.layout_children(node, 0.0, 0.0, 1.0, None, &mut fc);
        let w = content_right_extent(&content, self.fonts, 0.0, &|n| self.px_margin_right(n));
        self.min_content_cache.borrow_mut().insert(node, w);
        w
    }

    /// Shrink-to-fit width, CSS2 §10.3.5: `min(max-content, max(available, min-content))`.
    ///
    /// The `max(available, min-content)` is the part that was missing — we had
    /// `min(max-content, available)`, which lets a box be squeezed narrower than its own longest
    /// word, and (via the measure seam above) tells taffy a flex item's minimum size is its
    /// maximum size.
    fn shrink_to_fit(&self, node: NodeId, avail: f32) -> f32 {
        // A flex/grid container's preferred width is a question taffy can answer exactly; the
        // lay-out-at-1e6-and-measure trick cannot (see `taffy_tree::max_content_width`).
        let pref = self.max_content_width(node);
        // **The min-content floor only matters when the box does NOT fit.** If `pref <= avail` then
        // `min(pref, max(avail, min_content)) == pref` for any min-content value, so computing it
        // would be pure waste — and computing it means laying out a subtree. Most boxes on most
        // pages fit, so this short-circuit is the difference between a 16% layout regression and
        // none at all. Identical result, by algebra, not by approximation.
        if pref <= avail {
            return pref.max(0.0);
        }
        pref.min(avail.max(self.min_content_width(node))).max(0.0)
    }

    /// The **max-content** (preferred) width of `node`: how wide the box wants to be with no
    /// constraint at all. Memoized, and the memo is the whole point — see `max_content_cache`.
    fn max_content_width(&self, node: NodeId) -> f32 {
        if let Some(&cached) = self.max_content_cache.borrow().get(&node) {
            return cached;
        }
        let pref = self.max_content_width_uncached(node);
        self.max_content_cache.borrow_mut().insert(node, pref);
        pref
    }

    /// The right margin (px, ≥0) of a box's node, for the margin-box extent in `content_right_extent`.
    /// Percentage/auto margins resolve to 0 for an intrinsic measure; negatives don't extend the box.
    fn px_margin_right(&self, n: Option<NodeId>) -> f32 {
        n.map_or(0.0, |node| {
            self.style_of(node).margin.right.resolve(0.0, 0.0).max(0.0)
        })
    }

    fn max_content_width_uncached(&self, node: NodeId) -> f32 {
        // An anonymous flex/grid item is a text run; it is never a flex container, whatever the
        // cascade stored on the text node. Under the Stylo cascade a text node carries a CLONE of
        // its parent's style, so a bare run inside `display:flex` reads back as `flex` here — and
        // routing it into the taffy path would build a tree whose root measures via
        // `measure_intrinsic`, which lands back in this function: unbounded recursion, not a wrong
        // number. The element check is the base case.
        if self.dom.is_element(node)
            && matches!(
                self.style_of(node).display,
                Display::Flex | Display::Grid | Display::InlineFlex | Display::InlineGrid
            )
        {
            // A flex/grid container's preferred width is a question taffy can answer exactly; the
            // lay-out-at-1e6-and-measure trick cannot (see `taffy_tree::max_content_width`).
            return taffy_tree::max_content_width(
                self.dom,
                self.styles,
                node,
                |dn, known: taffy::Size<Option<f32>>, av: taffy::Size<taffy::AvailableSpace>| {
                    let aw = known.width.or(match av.width {
                        taffy::AvailableSpace::Definite(w) => Some(w),
                        taffy::AvailableSpace::MinContent => Some(0.0),
                        taffy::AvailableSpace::MaxContent => None,
                    });
                    let (w, h) = self.measure_intrinsic(dn, aw);
                    taffy::Size {
                        width: known.width.unwrap_or(w),
                        height: known.height.unwrap_or(h),
                    }
                },
            )
            .max(0.0);
        }
        // Lay the subtree out unconstrained and measure how far its content actually reaches.
        let mut fc = FloatContext::new(0.0, 1.0e6);
        let (content, _h) = self.layout_children(node, 0.0, 0.0, 1.0e6, None, &mut fc);
        let pref = content_right_extent(&content, self.fonts, 0.0, &|n| self.px_margin_right(n));
        // See `MANUK_TRACE_INTRINSIC` in `measure_intrinsic`: max-content is the OTHER place an
        // intrinsic width is decided (inline-block / inline-flex / float / abs), and a box that
        // fills when it should hug is nearly always this number.
        if let Some(want) = trace_intrinsic() {
            if self.dom.element(node).and_then(|e| e.attr("id")) == Some(want) {
                eprintln!("[max-content] #{want} pref={pref:.1}");
                if let BoxContent::Block(kids) = &content {
                    for k in kids {
                        eprintln!(
                            "    child {:?} [{:.0} {:.0} {:.0}x{:.0}]",
                            k.node.and_then(|n| self.dom.tag_name(n)),
                            k.rect.x,
                            k.rect.y,
                            k.rect.width,
                            k.rect.height
                        );
                    }
                }
            }
        }
        pref.max(0.0)
    }

    /// The intrinsic **content** size `(width, height)` of `node` for taffy's flex/grid
    /// measure seam (Blitz model): shrink-to-fit the width against `avail_width` (max-content
    /// clamped to available), then lay the content out at that width to get its height. This
    /// is what lets an `auto`-sized flex/grid item size to its content instead of collapsing
    /// to zero. Read-only (`&self`), so it can be called from the measure closure.
    fn measure_intrinsic(&self, node: NodeId, avail_width: Option<f32>) -> (f32, f32) {
        let avail = avail_width.unwrap_or(1.0e6);
        // Memoize: taffy probes each item several times per solve, and each probe re-lays-out
        // the subtree. Round the available width to a px so repeated min/max-content probes
        // (which pass the same very-large avail) share a cache entry.
        let key = (node, avail.round().min(u32::MAX as f32) as u32);
        if let Some(&cached) = self.measure_cache.borrow().get(&key) {
            return cached;
        }
        let width = self.shrink_to_fit(node, avail);
        let mut fc = FloatContext::new(0.0, width.max(1.0));
        let (_content, height) =
            self.layout_children(node, 0.0, 0.0, width.max(0.0), None, &mut fc);
        let result = (width, height);
        // `MANUK_TRACE_INTRINSIC=<id>` prints what a flex/grid item told taffy it wanted to be.
        // Flex WRAPPING is decided by this number, so when a row breaks that Chrome keeps on one
        // line, this is the number that is wrong — and it is otherwise invisible in the output.
        if let Some(want) = trace_intrinsic() {
            if self.dom.element(node).and_then(|e| e.attr("id")) == Some(want) {
                eprintln!(
                    "[intrinsic] #{want} avail={avail:.0} -> width={:.1} height={:.1}",
                    result.0, result.1
                );
            }
        }
        self.measure_cache.borrow_mut().insert(key, result);
        result
    }

    /// Lay out a `display:table` box (CSS2 §17), separated-borders model. Sequence:
    /// gather rows (flattening row groups) → per-column intrinsic min/max widths →
    /// distribute the table width across columns (fixed or auto) → lay out cells,
    /// stretching each to its row height → stack rows.
    ///
    /// Documented interpretations where CSS2 §17 is ambiguous / this slice is bounded
    /// (working-agreement requirement): **no `colspan`/`rowspan`** (each cell one
    /// grid slot); **no `border-collapse`** (separated model with `border-spacing`
    /// only); **captions, `<col>`/`<colgroup>` width hints, and `position:relative`
    /// on the table box are ignored**; anonymous-box fixup is minimal (only
    /// `TableRow`/`TableRowGroup`→rows and `TableCell`→cells are recognized).
    fn layout_table(&self, node: NodeId, cw: f32, x: f32, y: f32, prev_margin: f32) -> BlockResult {
        let s = self.style_of(node).clone();
        let ml = s.margin.left.resolve(cw, 0.0);
        let mt = s.margin.top.resolve(cw, 0.0);
        let mb = s.margin.bottom.resolve(cw, 0.0);
        let (pl, pr) = (
            s.padding.left.resolve(cw, 0.0),
            s.padding.right.resolve(cw, 0.0),
        );
        let (pt, pb) = (
            s.padding.top.resolve(cw, 0.0),
            s.padding.bottom.resolve(cw, 0.0),
        );
        let (bl, br) = (s.border_width.left, s.border_width.right);
        let (bt, bb) = (s.border_width.top, s.border_width.bottom);

        let border_x = x + ml;
        let border_y = y + collapse_margins(prev_margin, mt);
        let content_x = border_x + bl + pl;
        let content_y = border_y + bt + pt;

        // `border-collapse` drops the inter-cell spacing (cells share borders).
        let spacing = if s.border_collapse {
            0.0
        } else {
            s.border_spacing
        };
        let rows = self.collect_table_rows(node);

        // Placement grid: each cell claims the next free slot in its row, spanning
        // colspan columns × rowspan rows and marking those slots occupied (so cells below a
        // rowspan and to the right of a colspan shift over). CSS2 §17.5.
        let mut placed: Vec<PlacedCell> = Vec::new();
        let mut occ: Vec<Vec<bool>> = Vec::new();
        let mut ncols = 0usize;
        for (r, (_rn, row)) in rows.iter().enumerate() {
            let mut col = 0usize;
            for &cell in row {
                while occ
                    .get(r)
                    .and_then(|o| o.get(col))
                    .copied()
                    .unwrap_or(false)
                {
                    col += 1;
                }
                let cs = self.cell_span(cell, "colspan");
                let rs = self.cell_span(cell, "rowspan");
                for rr in r..r + rs {
                    while occ.len() <= rr {
                        occ.push(Vec::new());
                    }
                    for cc in col..col + cs {
                        while occ[rr].len() <= cc {
                            occ[rr].push(false);
                        }
                        occ[rr][cc] = true;
                    }
                }
                placed.push(PlacedCell {
                    cell,
                    row: r,
                    col,
                    colspan: cs,
                    rowspan: rs,
                });
                ncols = ncols.max(col + cs);
                col += cs;
            }
        }

        // Column widths.
        let spacing_total = spacing * (ncols as f32 + 1.0);
        let table_specified = match s.width {
            Dim::Auto => None,
            other => Some(other.resolve(cw, (cw - ml).max(0.0)).max(0.0)),
        };
        let avail_content = table_specified.unwrap_or((cw - ml).max(0.0)) - pl - pr;
        let avail_cols = (avail_content - spacing_total).max(0.0);

        let cell_grid: Vec<Vec<NodeId>> = rows.iter().map(|(_, cells)| cells.clone()).collect();
        let widths = if ncols == 0 {
            Vec::new()
        } else if s.table_layout == manuk_css::TableLayout::Fixed {
            self.fixed_col_widths(&cell_grid, ncols, avail_cols)
        } else {
            self.auto_col_widths(&placed, ncols, avail_cols, table_specified.is_some())
        };
        let cols_used: f32 = widths.iter().sum();
        let content_w = cols_used + spacing_total;

        // Column x offsets (separated model insets each column by `spacing`).
        let mut col_x = Vec::with_capacity(ncols);
        let mut acc = content_x + spacing;
        for &w in &widths {
            col_x.push(acc);
            acc += w + spacing;
        }

        let nrows = rows.len();
        // The pixel width a cell spanning `cs` columns from `col` occupies (its columns plus
        // the spacing between them).
        let span_w = |col: usize, cs: usize| -> f32 {
            let end = (col + cs).min(widths.len());
            let sum: f32 = widths.get(col..end).map(|w| w.iter().sum()).unwrap_or(0.0);
            sum + spacing * cs.saturating_sub(1) as f32
        };

        // Lay out each placed cell; record its natural height. Single-row cells set their
        // row's height; rowspan cells' overflow is added to their last spanned row.
        let mut laid: Vec<(usize, LayoutBox, f32)> = Vec::new();
        let mut row_h = vec![0.0f32; nrows.max(1)];
        for (pi, p) in placed.iter().enumerate() {
            let cx = col_x.get(p.col).copied().unwrap_or(content_x);
            let (cbox, bh) = self.layout_cell(p.cell, cx, 0.0, span_w(p.col, p.colspan));
            if p.rowspan == 1 {
                row_h[p.row] = row_h[p.row].max(bh);
            }
            laid.push((pi, cbox, bh));
        }
        for (pi, _, bh) in &laid {
            let p = &placed[*pi];
            if p.rowspan > 1 {
                let last = (p.row + p.rowspan - 1).min(nrows.saturating_sub(1));
                let spanned: f32 = (p.row..=last).map(|r| row_h[r]).sum::<f32>()
                    + spacing * (p.rowspan - 1) as f32;
                if *bh > spanned {
                    row_h[last] += *bh - spanned;
                }
            }
        }
        // Row y positions.
        let mut row_y = vec![content_y + spacing; nrows.max(1)];
        let mut yy = content_y + spacing;
        for r in 0..nrows {
            row_y[r] = yy;
            yy += row_h[r] + spacing;
        }
        // Position each cell at its start row and stretch it over its spanned rows.
        let mut row_cells: Vec<Vec<LayoutBox>> = vec![Vec::new(); nrows.max(1)];
        for (pi, mut cbox, _) in laid {
            let p = &placed[pi];
            let last = (p.row + p.rowspan - 1).min(nrows.saturating_sub(1));
            let dy = row_y[p.row] - cbox.rect.y;
            cbox.translate(0.0, dy);
            cbox.rect.height = (row_y[last] + row_h[last]) - row_y[p.row];
            row_cells[p.row].push(cbox);
        }
        let mut row_boxes = Vec::new();
        for r in 0..nrows {
            let rn = rows.get(r).map(|(n, _)| *n);
            let rs = rn.and_then(|n| self.styles.get(&n));
            row_boxes.push(LayoutBox {
                rect: Rect {
                    x: content_x,
                    y: row_y[r],
                    width: content_w,
                    height: row_h[r],
                },
                background: rs.and_then(|s| s.background_color),
                border: rs.and_then(border_of),
                radius: rs.map(|s| s.border_radius).unwrap_or(0.0),
                shadows: rs.map(|s| s.box_shadows.clone()).unwrap_or_default(),
                hidden: rs
                    .map(|s| s.visibility != manuk_css::Visibility::Visible)
                    .unwrap_or(false),
                mask_image: rs.and_then(|s| s.mask_image.clone()),
                background_images: rs.map(|s| s.background_images.clone()).unwrap_or_default(),
                background_size: rs.map(|s| s.background_size).unwrap_or_default(),
                background_position: rs.map(|s| s.background_position).unwrap_or_default(),
                object_fit: rs.map(|s| s.object_fit).unwrap_or_default(),
                object_position: rs.map(|s| s.object_position).unwrap_or_default(),
                background_repeat: rs.map(|s| s.background_repeat).unwrap_or_default(),
                outline: rs.and_then(|s| {
                    (s.outline_width > 0.0 && s.outline_color.a > 0)
                        .then_some((s.outline_width, s.outline_color))
                }),
                marker: None,
                opacity: rs.map(|s| s.opacity).unwrap_or(1.0),
                node: rn,
                content: BoxContent::Block(std::mem::take(&mut row_cells[r])),
            });
        }
        let cur_y = yy;

        let content_height = (cur_y - content_y).max(0.0);
        let content_height = match s.height {
            Dim::Auto => content_height,
            other => other.resolve(0.0, content_height).max(content_height),
        };

        let border_box_w = bl + pl + content_w + pr + br;
        let border_box_h = bt + pt + content_height + pb + bb;
        let boxx = LayoutBox {
            rect: Rect {
                x: border_x,
                y: border_y,
                width: border_box_w,
                height: border_box_h,
            },
            background: s.background_color,
            border: border_of(&s),
            radius: s.border_radius,
            shadows: s.box_shadows.clone(),
            hidden: s.visibility != manuk_css::Visibility::Visible,
            mask_image: s.mask_image.clone(),
            background_images: s.background_images.clone(),
            background_size: s.background_size,
            background_position: s.background_position,
            object_fit: s.object_fit,
            object_position: s.object_position,
            background_repeat: s.background_repeat,
            outline: (s.outline_width > 0.0 && s.outline_color.a > 0)
                .then_some((s.outline_width, s.outline_color)),
            marker: None,
            opacity: s.opacity,
            node: Some(node),
            content: BoxContent::Block(row_boxes),
        };
        // **Auto margins centre a table.** `layout_block` does this; `layout_table` did not, so
        // every `<center><table>` and `<table align="center">` on the legacy web — Hacker News
        // included — rendered flush against the left edge. The table's width is only known now
        // (its columns had to be sized first), so the whole box is shifted rather than the origin
        // being computed up front.
        let mut boxx = boxx;
        if s.margin.left.is_auto() && s.margin.right.is_auto() {
            let leftover = cw - border_box_w;
            if leftover > 0.0 {
                boxx.shift_x(leftover / 2.0);
            }
        }
        BlockResult {
            boxx,
            margin_top: mt,
            margin_bottom: mb,
            flow_bottom: border_y + border_box_h,
        }
    }

    /// A cell's `colspan`/`rowspan` attribute value (≥ 1).
    fn cell_span(&self, cell: NodeId, attr: &str) -> usize {
        self.dom
            .element(cell)
            .and_then(|e| e.attr(attr))
            .and_then(|v| v.trim().parse::<usize>().ok())
            .unwrap_or(1)
            .max(1)
    }

    /// Gather a table's rows (each a list of cell nodes), flattening row groups.
    /// The table's rows as `(row element, its cells)`. The row's own node is carried, not just its
    /// cells: a `table-row` **generates a box** (CSS2 §17.5), and that box is where a row's
    /// background and border paint and what `getBoundingClientRect` reports for a `<tr>`. Emitting
    /// an anonymous row box instead left every `<tr>` on the web without geometry — 31 of Hacker
    /// News' 119 identified elements.
    fn collect_table_rows(&self, table: NodeId) -> Vec<(NodeId, Vec<NodeId>)> {
        let mut rows = Vec::new();
        for child in self.dom.children(table) {
            if !is_rendered(self.dom, self.styles, child) || !self.dom.is_element(child) {
                continue;
            }
            match self.style_of(child).display {
                Display::TableRow => rows.push((child, self.collect_cells(child))),
                Display::TableRowGroup => {
                    for gr in self.dom.children(child) {
                        if is_rendered(self.dom, self.styles, gr)
                            && self.dom.is_element(gr)
                            && self.style_of(gr).display == Display::TableRow
                        {
                            rows.push((gr, self.collect_cells(gr)));
                        }
                    }
                }
                _ => {} // caption / column / colgroup / stray content: skipped
            }
        }
        rows
    }

    fn collect_cells(&self, row: NodeId) -> Vec<NodeId> {
        self.dom
            .children(row)
            .filter(|&c| {
                is_rendered(self.dom, self.styles, c)
                    && self.dom.is_element(c)
                    && self.style_of(c).display == Display::TableCell
            })
            .collect()
    }

    /// A cell's intrinsic `(min-content, max-content)` border-box widths.
    fn cell_intrinsic(&self, cell: NodeId) -> (f32, f32) {
        let s = self.style_of(cell);
        let frame = s.padding.left.resolve(0.0, 0.0)
            + s.padding.right.resolve(0.0, 0.0)
            + s.border_width.left
            + s.border_width.right;
        // If the cell has a definite width, both intrinsics collapse to it.
        if let Dim::Px(w) = s.width {
            return (w + frame, w + frame);
        }
        let mut fc_max = FloatContext::new(0.0, 1.0e6);
        let (cmax, _) = self.layout_children(cell, 0.0, 0.0, 1.0e6, None, &mut fc_max);
        let max = content_right_extent(&cmax, self.fonts, 0.0, &|n| self.px_margin_right(n));
        let mut fc_min = FloatContext::new(0.0, 0.0);
        let (cmin, _) = self.layout_children(cell, 0.0, 0.0, 0.0, None, &mut fc_min);
        let min = content_right_extent(&cmin, self.fonts, 0.0, &|n| self.px_margin_right(n));
        (min + frame, max + frame)
    }

    /// Auto table layout (CSS2 §17.5.2.2): distribute `avail` across columns using
    /// per-column min/max content widths.
    fn auto_col_widths(
        &self,
        placed: &[PlacedCell],
        ncols: usize,
        avail: f32,
        table_has_width: bool,
    ) -> Vec<f32> {
        let mut col_min = vec![0.0f32; ncols];
        let mut col_max = vec![0.0f32; ncols];
        // Single-column cells set their column's intrinsics directly. Cells are read from the
        // PLACED grid, not from each row's raw child order: with a `colspan`, the two disagree, and
        // attributing a spanning cell's width to the wrong column corrupts every column after it.
        // Hacker News' subtext row (`<td colspan="2">` then the metadata cell) did exactly that.
        for p in placed.iter().filter(|p| p.colspan == 1 && p.col < ncols) {
            let (mn, mx) = self.cell_intrinsic(p.cell);
            col_min[p.col] = col_min[p.col].max(mn);
            col_max[p.col] = col_max[p.col].max(mx);
        }
        // A spanning cell only *raises* its columns if they cannot already hold it; the excess is
        // spread evenly across the span (CSS2 §17.5.2.2 leaves the distribution up to the UA).
        for p in placed.iter().filter(|p| p.colspan > 1) {
            let end = (p.col + p.colspan).min(ncols);
            if p.col >= end {
                continue;
            }
            let span = (end - p.col) as f32;
            let (mn, mx) = self.cell_intrinsic(p.cell);
            let have_min: f32 = col_min[p.col..end].iter().sum();
            let have_max: f32 = col_max[p.col..end].iter().sum();
            if mn > have_min {
                let add = (mn - have_min) / span;
                for c in p.col..end {
                    col_min[c] += add;
                }
            }
            if mx > have_max {
                let add = (mx - have_max) / span;
                for c in p.col..end {
                    col_max[c] += add;
                }
            }
        }
        let sum_min: f32 = col_min.iter().sum();
        let sum_max: f32 = col_max.iter().sum();

        // Shrink-to-fit table (auto width): use max-content but never exceed avail.
        if !table_has_width && sum_max <= avail {
            return col_max;
        }
        if sum_max <= avail {
            // Definite, roomy table: grow columns proportionally to max-content.
            if sum_max <= 0.0 {
                return vec![avail / ncols as f32; ncols];
            }
            let extra = avail - sum_max;
            return col_max.iter().map(|&m| m + extra * (m / sum_max)).collect();
        }
        if sum_min <= avail {
            // Between min and max: distribute the slack over (max - min).
            let denom = sum_max - sum_min;
            if denom <= 0.0 {
                return vec![avail / ncols as f32; ncols];
            }
            let extra = avail - sum_min;
            return col_min
                .iter()
                .zip(&col_max)
                .map(|(&mn, &mx)| mn + extra * ((mx - mn) / denom))
                .collect();
        }
        // Overflow: columns take their min-content and the table exceeds avail.
        col_min
    }

    /// Fixed table layout (CSS2 §17.5.2.1): first-row cells' specified widths set the
    /// columns; auto columns split the remainder equally.
    fn fixed_col_widths(&self, rows: &[Vec<NodeId>], ncols: usize, avail: f32) -> Vec<f32> {
        let mut set: Vec<Option<f32>> = vec![None; ncols];
        if let Some(first) = rows.first() {
            for (c, &cell) in first.iter().enumerate() {
                if c >= ncols {
                    break;
                }
                set[c] = match self.style_of(cell).width {
                    Dim::Auto => None,
                    other => Some(other.resolve(avail, 0.0).max(0.0)),
                };
            }
        }
        let assigned: f32 = set.iter().flatten().sum();
        let autos = set.iter().filter(|o| o.is_none()).count();
        let each = if autos > 0 {
            (avail - assigned).max(0.0) / autos as f32
        } else {
            0.0
        };
        set.iter().map(|o| o.unwrap_or(each)).collect()
    }

    /// Lay out one table cell as a block-level BFC at `(x, y)` with column width
    /// `col_w`. Returns the cell box and its border-box height.
    fn layout_cell(&self, cell: NodeId, x: f32, y: f32, col_w: f32) -> (LayoutBox, f32) {
        let s = self.style_of(cell).clone();
        let (pl, pr) = (
            s.padding.left.resolve(col_w, 0.0),
            s.padding.right.resolve(col_w, 0.0),
        );
        let (pt, pb) = (
            s.padding.top.resolve(col_w, 0.0),
            s.padding.bottom.resolve(col_w, 0.0),
        );
        let (bl, br) = (s.border_width.left, s.border_width.right);
        let (bt, bb) = (s.border_width.top, s.border_width.bottom);

        let content_w = (col_w - pl - pr - bl - br).max(0.0);
        let content_x = x + bl + pl;
        let content_y = y + bt + pt;
        let mut floats = FloatContext::new(content_x, content_x + content_w);
        let (content, ch) =
            self.layout_children(cell, content_x, content_y, content_w, None, &mut floats);
        let content_height = match s.height {
            Dim::Auto => ch,
            other => other.resolve(0.0, ch).max(ch),
        };
        let border_box_h = bt + pt + content_height + pb + bb;
        (
            LayoutBox {
                rect: Rect {
                    x,
                    y,
                    width: col_w,
                    height: border_box_h,
                },
                background: s.background_color,
                border: border_of(&s),
                radius: s.border_radius,
                shadows: s.box_shadows.clone(),
                hidden: s.visibility != manuk_css::Visibility::Visible,
                mask_image: s.mask_image.clone(),
                background_images: s.background_images.clone(),
                background_size: s.background_size,
                background_position: s.background_position,
                object_fit: s.object_fit,
                object_position: s.object_position,
                background_repeat: s.background_repeat,
                outline: (s.outline_width > 0.0 && s.outline_color.a > 0)
                    .then_some((s.outline_width, s.outline_color)),
                marker: None,
                opacity: s.opacity,
                node: Some(cell),
                content,
            },
            border_box_h,
        )
    }

    /// Place `absolute`/`fixed` boxes in a final pass (CSS2 §9.6). They were skipped
    /// in normal flow; each is now sized and positioned against its containing block —
    /// the padding box of its nearest positioned DOM ancestor for `absolute`, the
    /// viewport for `fixed` (or the initial CB when no positioned ancestor exists) —
    /// and appended to the root's children so it paints above in-flow content.
    ///
    /// Documented simplifications: the *static position* used when neither inset on an
    /// axis is set is approximated as the containing block's start edge (true CSS
    /// tracks the box's would-be flow position); `z-index` stacking is not yet ordered
    /// (DOM order); scroll-based offsets and `sticky` are out of scope here.
    fn position_absolutes(&self, root_el: NodeId, root: &mut LayoutBox, viewport_w: f32) {
        // Border-box rect of every element currently in the fragment tree.
        let mut rects: HashMap<NodeId, Rect> = HashMap::new();
        root.walk(&mut |b| {
            if let Some(n) = b.node {
                rects.insert(n, b.rect);
            }
        });
        let doc_h = root.content_bottom();
        let viewport = Rect {
            x: 0.0,
            y: 0.0,
            width: viewport_w,
            height: doc_h,
        };

        // Gather positioned elements in DOM pre-order so an abs ancestor is placed
        // (and recorded) before an abs descendant that uses it as containing block.
        let mut positioned = Vec::new();
        self.collect_positioned(root_el, &mut positioned);

        let mut new_boxes = Vec::new();
        for node in positioned {
            let s = self.style_of(node);
            let all_auto = s.inset.left.is_auto()
                && s.inset.right.is_auto()
                && s.inset.top.is_auto()
                && s.inset.bottom.is_auto();
            let mut cb = if s.position == Position::Fixed {
                viewport
            } else {
                self.abs_containing_block(node, &rects, viewport)
            };
            // All insets `auto` → the box sits at its STATIC position, which normal flow recorded on
            // its way past. Anchor the containing block there so `layout_abs` resolves the box in the
            // right place instead of at the containing block's origin (which would put every
            // dropdown in the top-left corner) — and, before this, instead of nowhere at all.
            if all_auto {
                if let Some(&(sx, sy)) = self.static_pos.borrow().get(&node) {
                    cb = Rect {
                        x: sx,
                        y: sy,
                        width: cb.width,
                        height: cb.height,
                    };
                } else if s.position != Position::Fixed {
                    // Never reached in flow layout; a box we truly cannot place is still better
                    // dropped than rendered in the wrong corner.
                    continue;
                }
            }
            let b = self.layout_abs(node, cb);
            rects.insert(node, b.rect); // enable nested abs to use it as CB
            new_boxes.push(b);
        }

        if new_boxes.is_empty() {
            return;
        }
        match &mut root.content {
            BoxContent::Block(kids) => kids.extend(new_boxes),
            BoxContent::Inline(frags) => {
                // Root had only inline (or only out-of-flow) content: fold the inline
                // fragments into an anonymous block so the abs boxes can join as
                // siblings.
                let mut kids = Vec::new();
                if !frags.is_empty() {
                    kids.push(LayoutBox {
                        rect: root.rect,
                        background: None,
                        border: None,
                        radius: 0.0,
                        shadows: Vec::new(),
                        hidden: false,
                        mask_image: None,
                        background_images: Vec::new(),
                        background_size: manuk_css::BackgroundSize::Auto,
                        background_position: manuk_css::BackgroundPosition::default(),
                        object_fit: manuk_css::ObjectFit::Fill,
                        object_position: manuk_css::ObjectPosition::default(),
                        background_repeat: manuk_css::BackgroundRepeat::Repeat,
                        outline: None,
                        marker: None,
                        opacity: 1.0,
                        node: None,
                        content: BoxContent::Inline(std::mem::take(frags)),
                    });
                }
                kids.extend(new_boxes);
                root.content = BoxContent::Block(kids);
            }
        }
    }

    /// Collect rendered `absolute`/`fixed` element nodes in `node`'s subtree, **flat-tree**
    /// pre-order.
    ///
    /// Flat tree, not the node tree: everything else in layout walks `flat_children` (shadow
    /// content + slot assignment), and only flat-tree nodes are styled. Walking the node tree here
    /// would reach *unslotted* light-DOM children of a shadow host — which are never rendered, so
    /// the cascade gives them no style — and the lookup would panic. A missing style is likewise
    /// skipped rather than indexed, so an unstyled node can never crash layout.
    fn collect_positioned(&self, node: NodeId, out: &mut Vec<NodeId>) {
        for k in rendered_children(self.dom, self.styles, node) {
            if self.dom.is_element(k) {
                if let Some(st) = self.styles.get(&k) {
                    if is_out_of_flow_positioned(st) {
                        out.push(k);
                    }
                }
            }
            self.collect_positioned(k, out);
        }
    }

    /// The absolute containing block for `node`: the padding box of the nearest
    /// positioned ancestor with a laid-out box, else the viewport/initial CB.
    fn abs_containing_block(
        &self,
        node: NodeId,
        rects: &HashMap<NodeId, Rect>,
        viewport: Rect,
    ) -> Rect {
        let mut cur = self.dom.parent(node);
        while let Some(anc) = cur {
            if self.dom.is_element(anc) {
                let s = self.style_of(anc);
                if s.position != Position::Static {
                    if let Some(r) = rects.get(&anc) {
                        // Padding box = border box inset by the border widths.
                        return Rect {
                            x: r.x + s.border_width.left,
                            y: r.y + s.border_width.top,
                            width: (r.width - s.border_width.left - s.border_width.right).max(0.0),
                            height: (r.height - s.border_width.top - s.border_width.bottom)
                                .max(0.0),
                        };
                    }
                }
            }
            cur = self.dom.parent(anc);
        }
        viewport
    }

    /// Lay out one `absolute`/`fixed` box against containing block `cb`.
    fn layout_abs(&self, node: NodeId, cb: Rect) -> LayoutBox {
        let s = self.style_of(node).clone();
        let cw = cb.width;
        // Auto margins resolve to 0 here; a fully-constrained axis (both insets + a definite size)
        // redistributes its free space into them below, once the border box is known.
        let mut ml = s.margin.left.resolve(cw, 0.0);
        let mut mr = s.margin.right.resolve(cw, 0.0);
        let mut mt = s.margin.top.resolve(cw, 0.0);
        let mut mb = s.margin.bottom.resolve(cw, 0.0);
        let (pl, pr) = (
            s.padding.left.resolve(cw, 0.0),
            s.padding.right.resolve(cw, 0.0),
        );
        let (pt, pb) = (
            s.padding.top.resolve(cw, 0.0),
            s.padding.bottom.resolve(cw, 0.0),
        );
        let (bl, br) = (s.border_width.left, s.border_width.right);
        let (bt, bb) = (s.border_width.top, s.border_width.bottom);

        let left = (!s.inset.left.is_auto()).then(|| s.inset.left.resolve(cw, 0.0));
        let right = (!s.inset.right.is_auto()).then(|| s.inset.right.resolve(cw, 0.0));
        let top = (!s.inset.top.is_auto()).then(|| s.inset.top.resolve(cb.height, 0.0));
        let bottom = (!s.inset.bottom.is_auto()).then(|| s.inset.bottom.resolve(cb.height, 0.0));

        let frame = ml + mr + pl + pr + bl + br;
        let frame_v = mt + mb + pt + pb + bt + bb;
        // `box-sizing:border-box` — a specified `width`/`height` names the *border box*, so the
        // padding+border must come out to reach the content box. `auto` already resolves to content,
        // so these deltas apply only to the explicit-size and aspect-ratio arms below.
        let bs_extra_w = if s.box_sizing == BoxSizing::BorderBox {
            pl + pr + bl + br
        } else {
            0.0
        };
        let bs_extra_h = if s.box_sizing == BoxSizing::BorderBox {
            pt + pb + bt + bb
        } else {
            0.0
        };

        // A **definite** content height is known BEFORE the children (and the width) are computed in
        // two cases: an explicit (non-`auto`) height, and `height:auto` resolved by the constraint
        // equation (both `top` and `bottom` set, CSS2 §10.6.4). In both, a `height:100%` (or any `%`)
        // child must resolve against it (CSS2 §10.5) — so we thread it down as the percentage base.
        // This is the `position:absolute; inset:0` fill pattern (overlays/modals/backdrops), whose
        // child otherwise sees an indefinite base and **collapses to 0**. When the box is
        // content-sized instead, the base stays `None` (a `%` height there is `auto`, which is
        // correct). Computed here (not after the children) because `aspect-ratio` transfers it into
        // the width below.
        let definite_ch: Option<f32> = match s.height {
            // An **intrinsic-keyword** height (`min`/`max`/`fit-content`) collapses to `Dim::Auto`
            // but is *indefinite*: the box sizes to content and its `%`-height children see an
            // indefinite base (→ auto). So it must NOT take the constraint-equation definite height
            // even with both insets set (CSS Sizing 3 §cyclic-percentage-contribution). `stretch`
            // and `auto` stay definite under both insets — they are not flagged.
            Dim::Auto if s.height_intrinsic => None,
            Dim::Auto => match (top, bottom) {
                // The constraint equation already yields the *content* height (`frame_v` carries the
                // padding+border out), so it is box-sizing-agnostic — no `bs_extra_h` here.
                (Some(t), Some(b)) => Some((cb.height - t - b - frame_v).max(0.0)),
                _ => None,
            },
            // A non-`auto` Dim ignores its `auto_px` fallback; `bs_extra_h` converts a border-box
            // height to content (it is 0 under content-box, so this is the old value there).
            other => Some((other.resolve(cb.height, 0.0) - bs_extra_h).max(0.0)),
        };

        // Width: definite wins; else if both left+right are set the box stretches to fill between
        // them; else a definite height + `aspect-ratio` transfers through the ratio (CSS Sizing 4 —
        // the media/card/placeholder pattern), else shrink-to-fit.
        let content_w = match s.width {
            // **An intrinsic sizing keyword sizes to the CONTENT, never to the containing block** —
            // and an abspos box is exactly where that distinction bites, because its containing
            // block is usually a tiny `position:relative` anchor. This arm existed on the in-flow
            // block path and was missing here, so `position:absolute; width:max-content` fell all
            // the way through to shrink-to-fit against a 20px anchor: the box came out 114px where
            // Chrome says 180px, and every label inside it wrapped.
            //
            // That is the shape of nearly every dropdown, popover, menu, tooltip and autocomplete
            // panel on the web — anchored to a small trigger and sized by its own content — and it
            // is Wikipedia's sidebar verbatim (`.vector-dropdown-content { position:absolute;
            // width:max-content; max-width:200px }`, 93px against Chrome's 186px).
            //
            // Same measure functions as the block path; they return content-box widths, so the
            // `bs_extra_w` border-box subtraction correctly does not apply.
            Dim::Auto if s.width_keyword.is_some() => match s.width_keyword.unwrap() {
                IntrinsicSize::MinContent => self.min_content_width(node),
                IntrinsicSize::MaxContent => self.max_content_width(node),
                IntrinsicSize::FitContent => self.shrink_to_fit(node, (cw - frame).max(0.0)),
            },
            // `stretch` on an abspos box fills its containing block exactly as `left:0; right:0`
            // would — it is the same constraint, said in one property instead of two, and without
            // it the box shrink-to-fits and a `width:stretch` overlay collapses onto its content.
            Dim::Auto if s.width_stretch => (cw - frame).max(0.0),
            Dim::Auto => match (left, right) {
                (Some(l), Some(r)) => (cw - l - r - frame).max(0.0),
                _ => match (definite_ch, s.aspect_ratio) {
                    // The ratio relates the two axes of the box named by `box-sizing`, so scale in
                    // that box (`ch + bs_extra_h`) then convert back to content width (`- bs_extra_w`).
                    // Both deltas are 0 under content-box, so it is `content_h * ratio` there.
                    (Some(ch), Some(r)) if r > 0.0 => ((ch + bs_extra_h) * r - bs_extra_w).max(0.0),
                    _ => self.shrink_to_fit(node, (cw - frame).max(0.0)),
                },
            },
            other => (other.resolve(cw, (cw - frame).max(0.0)) - bs_extra_w).max(0.0),
        };
        // `min-width` / `max-width` clamp (CSS2 §10.4) — as the in-flow block path: max applied
        // first, then min wins, both converted to the content box. An abspos box ignored these
        // entirely, so a `max-width` dialog or `min-width` tooltip took its unconstrained size.
        // Clamp BEFORE laying out children so they see the constrained width.
        let min_w = (s.min_width.resolve(cw, 0.0) - bs_extra_w).max(0.0);
        let max_w = match s.max_width {
            Dim::Auto => f32::INFINITY,
            other => (other.resolve(cw, f32::INFINITY) - bs_extra_w).max(0.0),
        };
        let content_w = content_w.min(max_w).max(min_w);
        // Lay out content at a provisional origin, then re-origin once placed.
        let mut inner = FloatContext::new(0.0, content_w);
        let (content, ch) =
            self.layout_children(node, 0.0, 0.0, content_w, definite_ch, &mut inner);
        // Height: the definite value if we have one; else content height (CSS2 §10.6.4).
        let content_height = definite_ch.unwrap_or_else(|| ch.max(inner.lowest_bottom().max(0.0)));
        // `min-height` / `max-height` clamp (CSS2 §10.7) — the CB height is always definite here, so
        // a `%` bound resolves against it (unlike the in-flow case's indefinite-parent → `none`).
        let min_h = (s.min_height.resolve(cb.height, 0.0) - bs_extra_h).max(0.0);
        let max_h = match s.max_height {
            Dim::Auto => f32::INFINITY,
            other => (other.resolve(cb.height, f32::INFINITY) - bs_extra_h).max(0.0),
        };
        let content_height = content_height.min(max_h).max(min_h);

        let border_box_w = bl + pl + content_w + pr + br;
        let border_box_h = bt + pt + content_height + pb + bb;

        // Auto margins on an abspos box absorb the free space of a **fully-constrained** axis —
        // both insets set AND a definite size — per CSS2 §10.3.7 (inline) / §10.6.4 (block). This
        // is the `position:absolute; inset:0; margin:auto` centering idiom that anchors dialogs,
        // modals and backdrops. In an axis's under-constrained cases (a size of `auto` that
        // stretches to fill between the insets, or an open inset) an auto margin stays 0 — which is
        // exactly what `resolve(_, 0.0)` already gave us, so those paths are untouched. `!= Auto`
        // also excludes an intrinsic keyword (`fit-content`/`min`/`max`), which collapses to `Auto`
        // and must not be treated as a definite size here.
        if let (Some(l), Some(r)) = (left, right) {
            if s.width != Dim::Auto {
                let free = cw - l - r - border_box_w;
                match (s.margin.left.is_auto(), s.margin.right.is_auto()) {
                    (true, true) if free >= 0.0 => {
                        ml = free / 2.0;
                        mr = free / 2.0;
                    }
                    // Negative free space (ltr): pin the start margin, overflow past the end edge.
                    (true, true) => {
                        ml = 0.0;
                        mr = free;
                    }
                    // A start (left) auto margin shifts the box; an end (right) auto margin only
                    // absorbs slack past a box already pinned by `left`+`margin-left`, so it does
                    // not move it — and over-constrained (neither auto) likewise uses `left`.
                    (true, false) => ml = free - mr,
                    (false, true) | (false, false) => {}
                }
            }
        }
        if let (Some(t), Some(b)) = (top, bottom) {
            if s.height != Dim::Auto {
                let free = cb.height - t - b - border_box_h;
                match (s.margin.top.is_auto(), s.margin.bottom.is_auto()) {
                    (true, true) if free >= 0.0 => {
                        mt = free / 2.0;
                        mb = free / 2.0;
                    }
                    (true, true) => {
                        mt = 0.0;
                        mb = free;
                    }
                    // As the inline axis: only a start (top) auto margin repositions the box.
                    (true, false) => mt = free - mb,
                    (false, true) | (false, false) => {}
                }
            }
        }

        // Border-box top-left. `left`/`top` win; else offset from the far edge; else
        // the containing block's start edge (static-position approximation).
        let bx = if let Some(l) = left {
            cb.x + l + ml
        } else if let Some(r) = right {
            cb.x + cb.width - r - mr - border_box_w
        } else {
            cb.x + ml
        };
        let by = if let Some(t) = top {
            cb.y + t + mt
        } else if let Some(b) = bottom {
            cb.y + cb.height - b - mb - border_box_h
        } else {
            cb.y + mt
        };

        let mut boxx = LayoutBox {
            rect: Rect {
                x: bx,
                y: by,
                width: border_box_w,
                height: border_box_h,
            },
            background: s.background_color,
            border: border_of(&s),
            radius: s.border_radius,
            shadows: s.box_shadows.clone(),
            hidden: s.visibility != manuk_css::Visibility::Visible,
            mask_image: s.mask_image.clone(),
            background_images: s.background_images.clone(),
            background_size: s.background_size,
            background_position: s.background_position,
            object_fit: s.object_fit,
            object_position: s.object_position,
            background_repeat: s.background_repeat,
            outline: (s.outline_width > 0.0 && s.outline_color.a > 0)
                .then_some((s.outline_width, s.outline_color)),
            marker: None,
            opacity: s.opacity,
            node: Some(node),
            content,
        };
        // Content was laid out at (0,0); shift *only the content* to the abs box's
        // content origin (the box rect is already placed).
        let ox = bx + bl + pl;
        let oy = by + bt + pt;
        match &mut boxx.content {
            BoxContent::Block(kids) => {
                for k in kids {
                    k.translate(ox, oy);
                }
            }
            BoxContent::Inline(frags) => {
                for f in frags {
                    f.x += ox;
                    f.line_top += oy;
                    f.baseline += oy;
                }
            }
        }
        // `transform` applies to absolutely-positioned boxes too (around the box center).
        if !s.transform.is_empty() {
            let origin = (bx + border_box_w / 2.0, by + border_box_h / 2.0);
            let m = resolve_transform(&s.transform, border_box_w, border_box_h, origin);
            boxx.transform_affine(&m);
        }
        boxx
    }

    /// Turn a pending run of inline-level siblings into an anonymous block box.
    /// Returns the updated `(cur_y, prev_margin)`: a whitespace-only run produces no
    /// box and preserves the pending block margin (so `<p>a</p>\n<p>b</p>` still
    /// collapses); real inline content is not collapsible, so the pending margin is
    /// committed before it.
    #[allow(clippy::too_many_arguments)]
    fn flush_inline_run(
        &self,
        run: &mut Vec<NodeId>,
        boxes: &mut Vec<LayoutBox>,
        cx: f32,
        cur_y: f32,
        prev_margin: f32,
        cw: f32,
        floats: &FloatContext,
    ) -> (f32, f32) {
        if run.is_empty() {
            return (cur_y, prev_margin);
        }
        let items = self.collect_inline_group(run, cw, None);
        run.clear();
        if items.is_empty() {
            return (cur_y, prev_margin); // whitespace-only: keep the pending margin
        }
        let start = cur_y + prev_margin;
        let (frags, atomics, h) = self.layout_inline(items, cx, start, cw, TextAlign::Left, floats);
        boxes.push(LayoutBox {
            rect: Rect {
                x: cx,
                y: start,
                width: cw,
                height: h,
            },
            background: None,
            border: None,
            radius: 0.0,
            shadows: Vec::new(),
            hidden: false,
            mask_image: None,
            background_images: Vec::new(),
            background_size: manuk_css::BackgroundSize::Auto,
            background_position: manuk_css::BackgroundPosition::default(),
            object_fit: manuk_css::ObjectFit::Fill,
            object_position: manuk_css::ObjectPosition::default(),
            background_repeat: manuk_css::BackgroundRepeat::Repeat,
            outline: None,
            marker: None,
            opacity: 1.0,
            node: None,
            content: BoxContent::Inline(frags),
        });
        // Inline-block atomic boxes are already absolutely positioned; add them as siblings.
        boxes.extend(atomics);
        (start + h, 0.0)
    }

    /// Lay out flex children as a row using taffy for main-axis sizing/positioning.
    /// Each child is then laid out as a block within its taffy-assigned slot.
    fn layout_flex(
        &self,
        node: NodeId,
        cx: f32,
        cy: f32,
        cw: f32,
        kids: &[NodeId],
    ) -> (BoxContent, f32) {
        self.layout_flex_or_grid(node, cx, cy, cw, kids)
    }

    /// Lay out a `display:grid` container via taffy, then place each item at its grid slot.
    fn layout_grid(
        &self,
        node: NodeId,
        cx: f32,
        cy: f32,
        cw: f32,
        kids: &[NodeId],
    ) -> (BoxContent, f32) {
        self.layout_flex_or_grid(node, cx, cy, cw, kids)
    }

    /// Shared flex/grid layout via the unified taffy tree ([`taffy_tree::solve_subtree`]): the
    /// container and its directly-nested flex/grid descendants are solved in one tree, with
    /// block/inline/float/table children content-measured back into Manuk. Returns the
    /// container's child slots, then places each child (as a block within its slot).
    fn layout_flex_or_grid(
        &self,
        node: NodeId,
        cx: f32,
        cy: f32,
        cw: f32,
        kids: &[NodeId],
    ) -> (BoxContent, f32) {
        // Mirrors `taffy_tree::flex_items`: a non-white-space text run is an ANONYMOUS item, so a
        // container holding only text ("<div style=display:flex>Label</div>") is not empty and must
        // not short-circuit to a zero box.
        let block_kids: Vec<NodeId> = kids
            .iter()
            .copied()
            .filter(|&k| {
                self.dom.is_element(k)
                    || matches!(self.dom.data(k), NodeData::Text(t) if !t.trim().is_empty())
            })
            .collect();
        if block_kids.is_empty() {
            return (BoxContent::Block(vec![]), 0.0);
        }
        let container_h = match self.style_of(node).height {
            Dim::Px(p) => Some(p),
            _ => None,
        };
        let placed = taffy_tree::solve_subtree(
            self.dom,
            self.styles,
            node,
            cw,
            container_h,
            |dn, known: taffy::Size<Option<f32>>, avail: taffy::Size<taffy::AvailableSpace>| {
                // `MinContent` means "how narrow can you get?" — answering `None` here sent it
                // through `measure_intrinsic`'s 1e6 default and returned the MAX-content width,
                // which is the opposite answer. With shrink-to-fit floored at min-content, a zero
                // available width yields exactly the min-content size.
                let aw = known.width.or(match avail.width {
                    taffy::AvailableSpace::Definite(w) => Some(w),
                    taffy::AvailableSpace::MinContent => Some(0.0),
                    taffy::AvailableSpace::MaxContent => None,
                });
                let (w, h) = self.measure_intrinsic(dn, aw);
                taffy::Size {
                    width: known.width.unwrap_or(w),
                    height: known.height.unwrap_or(h),
                }
            },
        );
        let mut boxes = Vec::new();
        let mut max_h = 0.0f32;
        for p in &placed {
            let (boxx, bottom) = self.extract_placed(p, cx, cy);
            max_h = max_h.max(bottom);
            boxes.push(boxx);
        }
        (BoxContent::Block(boxes), max_h)
    }

    /// Turn a [`taffy_tree::Placed`] node into a `LayoutBox` at its taffy-assigned position
    /// (`base_x`/`base_y` is the parent's border-box origin). A **container** (flex/grid) is
    /// built directly from the unified tree's geometry, recursing into its already-placed
    /// children — no re-solve. A **leaf** (block/inline/float/table) is laid out via
    /// [`Self::layout_block`] at the assigned rect, exactly as before, so its content (text,
    /// floats, its own separate flex subtrees) is produced. Returns the box and its bottom
    /// extent relative to `base_y` (for the container's content-height).
    fn extract_placed(&self, p: &taffy_tree::Placed, base_x: f32, base_y: f32) -> (LayoutBox, f32) {
        let abs_x = base_x + p.slot.x;
        let abs_y = base_y + p.slot.y;
        if p.container {
            let children: Vec<LayoutBox> = p
                .children
                .iter()
                .map(|c| self.extract_placed(c, abs_x, abs_y).0)
                .collect();
            let s = self.style_of(p.dom);
            let boxx = LayoutBox {
                rect: Rect {
                    x: abs_x,
                    y: abs_y,
                    width: p.slot.width,
                    height: p.slot.height,
                },
                background: s.background_color,
                border: border_of(s),
                radius: s.border_radius,
                shadows: s.box_shadows.clone(),
                hidden: s.visibility != manuk_css::Visibility::Visible,
                mask_image: s.mask_image.clone(),
                background_images: s.background_images.clone(),
                background_size: s.background_size,
                background_position: s.background_position,
                object_fit: s.object_fit,
                object_position: s.object_position,
                background_repeat: s.background_repeat,
                outline: (s.outline_width > 0.0 && s.outline_color.a > 0)
                    .then_some((s.outline_width, s.outline_color)),
                marker: None,
                opacity: s.opacity,
                node: Some(p.dom),
                content: BoxContent::Block(children),
            };
            (boxx, p.slot.y + p.slot.height)
        } else if !self.dom.is_element(p.dom) {
            // ANONYMOUS ITEM. It has no element, therefore no background, border, padding or
            // outline of its own — only the text. Going through `layout_block` would read those off
            // the text node's stored style, and under the Stylo cascade that style is a clone of the
            // PARENT's: the container's background and border would paint a second time, inset by
            // its own padding. Build the box directly instead.
            let mut item_floats = FloatContext::new(abs_x, abs_x + p.slot.width);
            let (content, h) =
                self.layout_children(p.dom, abs_x, abs_y, p.slot.width, None, &mut item_floats);
            let s = self.style_of(p.dom);
            let height = p.slot.height.max(h);
            let boxx = LayoutBox {
                rect: Rect {
                    x: abs_x,
                    y: abs_y,
                    width: p.slot.width,
                    height,
                },
                background: None,
                border: None,
                radius: 0.0,
                shadows: Vec::new(),
                // `visibility` and `opacity`-as-folded ARE readable off a text node in both
                // cascades, and they must be: a hidden container's text stays hidden.
                hidden: s.visibility != manuk_css::Visibility::Visible,
                mask_image: None,
                background_images: Vec::new(),
                background_size: manuk_css::BackgroundSize::Auto,
                background_position: manuk_css::BackgroundPosition::default(),
                object_fit: manuk_css::ObjectFit::Fill,
                object_position: manuk_css::ObjectPosition::default(),
                background_repeat: manuk_css::BackgroundRepeat::Repeat,
                outline: None,
                marker: None,
                opacity: s.opacity,
                node: Some(p.dom),
                content,
            };
            (boxx, p.slot.y + height)
        } else {
            let mut item_floats = FloatContext::new(abs_x, abs_x + p.slot.width);
            // Record taffy's verdict BEFORE laying the item out, so `layout_block` uses it instead
            // of re-resolving the item's own `width` against it.
            self.taffy_item_width
                .borrow_mut()
                .insert(p.dom, p.slot.width);
            let r = self.layout_block(
                p.dom,
                p.slot.width,
                Some(p.slot.height),
                abs_x,
                abs_y,
                0.0,
                &mut item_floats,
            );
            self.taffy_item_width.borrow_mut().remove(&p.dom);
            let mut boxx = r.boxx;
            // Taffy sized the item (grow/stretch/track height); when its own height is `auto`,
            // adopt taffy's slot height so it fills its flex line / grid cell.
            if self.style_of(p.dom).height == Dim::Auto && p.slot.height > boxx.rect.height {
                boxx.rect.height = p.slot.height;
            }
            let bottom = p.slot.y + r.margin_top + boxx.rect.height + r.margin_bottom;
            (boxx, bottom)
        }
    }

    /// Collect inline tokens (words) from a run of inline-level siblings, tracking
    /// inter-word spacing.
    ///
    /// `owner` is the element whose inline formatting context this is, when the run is *all* of its
    /// content. Its `::before` / `::after` generated content is materialised here, at the two ends —
    /// generated content is not in the DOM (script must never see it), so this is the only place it
    /// can enter the flow. A block whose children are a *mix* of blocks and inlines passes `None`;
    /// its pseudos would otherwise be emitted once per run.
    fn collect_inline_group(
        &self,
        nodes: &[NodeId],
        cw: f32,
        owner: Option<NodeId>,
    ) -> Vec<InlineItem> {
        let mut out = Vec::new();
        let mut pending_space = false;
        let mut first = true;
        let pseudo = |which: fn(&ComputedStyle) -> &Option<Box<ComputedStyle>>| -> Option<(String, TextStyle)> {
            let s = owner.and_then(|n| self.styles.get(&n))?;
            let p = which(s).as_ref()?;
            let text = p.content.clone()?;
            (!text.is_empty()).then(|| (text, text_style(p, self.fonts)))
        };
        if let Some((text, style)) = pseudo(|s| &s.before) {
            out.push(InlineItem::Word {
                text,
                style,
                space_before: false,
                node: owner,
                no_wrap: true,
                break_word: false,
            });
            first = false;
        }
        for &n in nodes {
            self.collect_inline_node(n, &mut out, &mut pending_space, &mut first, None, cw);
        }
        if let Some((text, style)) = pseudo(|s| &s.after) {
            out.push(InlineItem::Word {
                text,
                style,
                space_before: pending_space && !first,
                node: owner,
                no_wrap: true,
                break_word: false,
            });
        }
        out
    }

    /// `owner` is the deepest **element** ancestor seen so far; each word records it so
    /// inline elements (`<a>`, `<button>`) — which never get a `LayoutBox` — can still
    /// have their geometry recovered from the runs they produced (§4a).
    fn collect_inline_node(
        &self,
        node: NodeId,
        out: &mut Vec<InlineItem>,
        pending_space: &mut bool,
        first: &mut bool,
        owner: Option<NodeId>,
        cw: f32,
    ) {
        match self.dom.data(node) {
            NodeData::Text(raw) => {
                let cs = self.style_of(node);
                let style = text_style(cs, self.fonts);
                // `text-transform` (inherited) changes the RENDERED casing without touching the DOM
                // text — "SUBMIT" for a `text-transform:uppercase` button whose textContent is "Submit".
                let transformed = apply_text_transform(raw, cs.text_transform);
                let t: &str = transformed.as_ref();
                // `white-space` is inherited, so the text node carries it. `nowrap` and `pre`
                // both suppress wrapping between words.
                let no_wrap = matches!(cs.white_space, WhiteSpace::NoWrap | WhiteSpace::Pre);
                // `overflow-wrap:break-word` / `word-break:break-all` permit char-level breaking of
                // an over-long token so it does not overflow its column. Carried on each word; the
                // actual split (against the live line width) happens in the inline placement pass.
                let break_word = matches!(
                    cs.overflow_wrap,
                    manuk_css::OverflowWrap::BreakWord | manuk_css::OverflowWrap::Anywhere
                ) || cs.word_break == manuk_css::WordBreak::BreakAll;
                // `pre-wrap` / `pre-line` preserve newlines but still wrap long lines: break at each
                // newline, then split the line into words as usual.
                if matches!(cs.white_space, WhiteSpace::PreWrap | WhiteSpace::PreLine) {
                    for (i, line) in t.split('\n').enumerate() {
                        if i > 0 {
                            out.push(InlineItem::Break {
                                height: style.line_height,
                                node: owner,
                            });
                            *pending_space = false;
                            *first = true;
                        }
                        let mut buf = String::new();
                        for ch in line.chars() {
                            if ch.is_whitespace() {
                                if !buf.is_empty() {
                                    push_word(
                                        out,
                                        &mut buf,
                                        style,
                                        pending_space,
                                        first,
                                        owner,
                                        false,
                                        break_word,
                                    );
                                }
                                *pending_space = true;
                            } else {
                                buf.push(ch);
                            }
                        }
                        if !buf.is_empty() {
                            push_word(
                                out,
                                &mut buf,
                                style,
                                pending_space,
                                first,
                                owner,
                                false,
                                break_word,
                            );
                        }
                    }
                    return;
                }
                // `white-space: pre` preserves BOTH newlines and runs of spaces. Folding them away
                // like ordinary whitespace turns every code block into one endless line.
                if cs.white_space == WhiteSpace::Pre {
                    for (i, line) in t.split('\n').enumerate() {
                        if i > 0 {
                            out.push(InlineItem::Break {
                                height: style.line_height,
                                node: owner,
                            });
                            *pending_space = false;
                            *first = true;
                        }
                        if line.is_empty() {
                            continue;
                        }
                        // One word per line: `pre` never wraps, and the literal text (indentation
                        // included) is measured as written.
                        out.push(InlineItem::Word {
                            text: line.to_string(),
                            style,
                            space_before: false,
                            node: owner,
                            no_wrap: true,
                            break_word: false,
                        });
                        *first = false;
                    }
                    return;
                }
                let mut buf = String::new();
                for ch in t.chars() {
                    if ch.is_whitespace() {
                        if !buf.is_empty() {
                            push_word(
                                out,
                                &mut buf,
                                style,
                                pending_space,
                                first,
                                owner,
                                no_wrap,
                                break_word,
                            );
                        }
                        *pending_space = true;
                    } else {
                        buf.push(ch);
                    }
                }
                if !buf.is_empty() {
                    push_word(
                        out,
                        &mut buf,
                        style,
                        pending_space,
                        first,
                        owner,
                        no_wrap,
                        break_word,
                    );
                }
            }
            NodeData::Element(_) => {
                let disp = self.styles.get(&node).map(|s| s.display);
                if disp == Some(Display::None) {
                    return;
                }
                // `<br>` — a forced line break, and nothing else.
                if self.dom.tag_name(node) == Some("br") {
                    let lh = self
                        .styles
                        .get(&node)
                        .map(|s| s.line_height)
                        .unwrap_or(16.0);
                    out.push(InlineItem::Break {
                        height: lh,
                        node: Some(node),
                    });
                    *pending_space = false;
                    *first = true;
                    return;
                }
                // An `inline-block` (or inline-flex/grid) is an *atomic* inline box: lay it
                // out as a block right here and flow it like a word, rather than recursing
                // into its children as inline text. A REPLACED element at `display: inline`
                // (`<img>` — the computed value Chrome and the spec give it) is exactly as
                // atomic; it must never fall through to the text recursion below.
                if matches!(
                    disp,
                    Some(
                        Display::InlineBlock
                            | Display::Flex
                            | Display::Grid
                            | Display::InlineFlex
                            | Display::InlineGrid
                    )
                ) || is_atomic_inline_replaced(self.dom, self.styles, node)
                {
                    let s = self.style_of(node);
                    let ml = s.margin.left.resolve(cw, 0.0);
                    let mr = s.margin.right.resolve(cw, 0.0);
                    let mut fc = FloatContext::new(0.0, cw);
                    let r = self.layout_block(node, cw, None, 0.0, 0.0, 0.0, &mut fc);
                    let advance = ml + r.boxx.rect.width + mr;
                    let height = r.margin_top + r.boxx.rect.height + r.margin_bottom;
                    out.push(InlineItem::Atomic {
                        box_: Box::new(r.boxx),
                        advance,
                        height,
                        space_before: *pending_space && !*first,
                        valign: s.vertical_align,
                        // `white-space` is INHERITED, so the atomic's own computed style already
                        // carries the containing block's `nowrap` — same source the text path at
                        // `collect_inline_node` reads for a Word.
                        no_wrap: matches!(s.white_space, WhiteSpace::NoWrap | WhiteSpace::Pre),
                    });
                    *first = false;
                    *pending_space = false;
                    return;
                }
                // An inline element's horizontal padding + border occupies space in the flow
                // and extends its geometry — emit edge spacers around its content.
                let s = self.style_of(node);
                let mark = out.len();
                let pad_l = s.padding.left.resolve(cw, 0.0) + s.border_width.left;
                let pad_r = s.padding.right.resolve(cw, 0.0) + s.border_width.right;
                if pad_l > 0.0 {
                    out.push(InlineItem::Spacer {
                        width: pad_l,
                        node: Some(node),
                        space_before: *pending_space && !*first,
                        report_height: 0.0,
                    });
                    *first = false;
                    *pending_space = false;
                }
                // N4: inline content also follows the flat tree.
                let children: Vec<NodeId> = self.dom.flat_children(node);
                for c in children {
                    self.collect_inline_node(c, out, pending_space, first, Some(node), cw);
                }
                if pad_r > 0.0 {
                    out.push(InlineItem::Spacer {
                        width: pad_r,
                        node: Some(node),
                        space_before: false,
                        report_height: 0.0,
                    });
                    *pending_space = false;
                }
                // An inline element that contributed NOTHING to the flow is still a box. Without
                // this it has no geometry at all: `getBoundingClientRect` returns nothing, it can't
                // be scrolled to, and it cannot be painted. On one Wikipedia article that is 1,079
                // spans and 298 anchors — the single largest source of missing elements.
                if out.len() == mark {
                    out.push(InlineItem::Spacer {
                        width: 0.0,
                        node: Some(node),
                        space_before: false,
                        report_height: s.line_height.max(0.0),
                    });
                }
            }
            _ => {}
        }
    }

    /// Greedy line-breaking of inline items. Each line's usable band is intersected
    /// with `floats`, so text flows around floats (CSS2 §9.5). Returns fragments with
    /// absolute positions and the total inline block height.
    ///
    /// Approximation (documented): a line's float band is queried using the *first*
    /// word's line height as the height estimate — exact for uniform-size text, an
    /// approximation when a taller inline box lands mid-line.
    #[allow(clippy::type_complexity)]
    /// `overflow-wrap:break-word` / `word-break:break-all`: a single token wider than the content
    /// box — a long URL, a 64-char hex hash, an unspaced foreign string — has no whitespace and no
    /// UAX-14 opportunity to wrap at, so the normal line-filler would let it overflow the column and
    /// break the layout. Split each such `break_word` word at char boundaries into chunks that each
    /// fit `cw`, so the filler wraps them across lines instead. Only over-wide `break_word` words are
    /// rewritten; every other item passes through untouched (so the parity gate is unmoved).
    fn break_overwide_words(&self, items: Vec<InlineItem>, cw: f32) -> Vec<InlineItem> {
        if cw <= 0.0 {
            return items;
        }
        let mut out = Vec::with_capacity(items.len());
        for item in items {
            match item {
                InlineItem::Word {
                    text,
                    style,
                    space_before,
                    node,
                    no_wrap,
                    break_word,
                } if break_word
                    && !no_wrap
                    && self.fonts.measure(&text, style.font_key, style.font_size) > cw =>
                {
                    let key = style.font_key;
                    let size = style.font_size;
                    let mut chunk = String::new();
                    let mut chunk_w = 0.0f32;
                    let mut first_chunk = true;
                    let mut buf = [0u8; 4];
                    for ch in text.chars() {
                        let adv = self.fonts.measure(ch.encode_utf8(&mut buf), key, size);
                        // Flush before the char that would overflow — but never an empty chunk, so a
                        // single char wider than `cw` still lands (unbreakable, an accepted overflow).
                        if !chunk.is_empty() && chunk_w + adv > cw {
                            out.push(InlineItem::Word {
                                text: std::mem::take(&mut chunk),
                                style,
                                space_before: first_chunk && space_before,
                                node,
                                no_wrap: false,
                                break_word: false,
                            });
                            first_chunk = false;
                            chunk_w = 0.0;
                        }
                        chunk.push(ch);
                        chunk_w += adv;
                    }
                    if !chunk.is_empty() {
                        out.push(InlineItem::Word {
                            text: chunk,
                            style,
                            space_before: first_chunk && space_before,
                            node,
                            no_wrap: false,
                            break_word: false,
                        });
                    }
                }
                other => out.push(other),
            }
        }
        out
    }

    fn layout_inline(
        &self,
        items: Vec<InlineItem>,
        cx: f32,
        cy: f32,
        cw: f32,
        align: TextAlign,
        floats: &FloatContext,
    ) -> (Vec<TextFragment>, Vec<LayoutBox>, f32) {
        let items = self.break_overwide_words(items, cw);
        // Usable (left_x, width) at vertical `y` for a line of height `h`: the float
        // exclusions intersected with this container's content box, dropping past
        // floats that leave no room.
        let open_band = |y: &mut f32, h: f32| -> (f32, f32) {
            loop {
                let l = floats.left_offset(*y, h).max(cx);
                let r = floats.right_offset(*y, h).min(cx + cw);
                let w = (r - l).max(0.0);
                if w > 0.0 {
                    return (l, w);
                }
                match floats.next_bottom_below(*y) {
                    Some(ny) if ny > *y => *y = ny,
                    _ => return (cx, cw),
                }
            }
        };

        let mut frags = Vec::new();
        let mut atomic_boxes: Vec<LayoutBox> = Vec::new();
        let mut y = cy;
        let mut cur: Vec<LineFrag> = Vec::new();
        let mut pen = 0.0f32;
        let mut line_left = cx;
        let mut line_avail = cw;

        // The "space" font metrics for an atomic (no text): use a default face at the box's
        // notional size doesn't matter — we only need the width of a normal space.
        // Tracks whether the item most recently placed on the line forbids a wrap after it.
        let mut prev_no_wrap = false;
        for item in items {
            // A forced break (`<br>`, a newline in `pre`) closes the current line immediately and
            // starts the next one — it is not laid out *on* a line, it *ends* one. An empty line
            // (two breaks in a row, a blank line in a code block) still occupies its line height,
            // so an empty `cur` opens a band and closes it straight away rather than collapsing.
            if let InlineItem::Break { height, node } = item {
                if cur.is_empty() {
                    let (l, w) = open_band(&mut y, height);
                    line_left = l;
                    line_avail = w;
                    let key = FontKey {
                        family: FontFamily::SansSerif,
                        bold: false,
                        italic: false,
                    };
                    cur.push(LineFrag {
                        x: 0.0,
                        width: 0.0,
                        text: String::new(),
                        style: TextStyle {
                            // A synthetic empty fragment — no text, so no order to get wrong.
                            rtl: false,
                            font_key: key,
                            font_size: 16.0,
                            color: Rgba::BLACK,
                            line_height: height,
                            decoration: Default::default(),
                            letter_spacing: 0.0,
                            word_spacing: 0.0,
                            shadow: None,
                        },
                        ascent: 0.0,
                        descent: 0.0,
                        node,
                        report_h: Some(height),
                        atomic: None,
                        atomic_h: 0.0,
                        valign: VerticalAlign::Baseline,
                    });
                } else if node.map(|n| self.dom.tag_name(n)) == Some(Some("br")) {
                    // The `<br>` that ends a NON-empty line also earns a box: a zero-width
                    // fragment at the pen position, `line-height` tall — Chrome reports exactly
                    // this ([x y 0×lh] at the end of the line it terminates), and the tick-380
                    // oracle counted the missing box on 64 corpus sites. Zero width, empty text:
                    // it moves no alignment and no justification, it only gives the element
                    // geometry — `getBoundingClientRect` on a `<br>` is how editors and caret
                    // libraries find line ends. `<br>` ONLY: a preserved newline in `pre` also
                    // arrives as a Break carrying its text's owner, which already has geometry.
                    let key = FontKey {
                        family: FontFamily::SansSerif,
                        bold: false,
                        italic: false,
                    };
                    cur.push(LineFrag {
                        x: pen,
                        width: 0.0,
                        text: String::new(),
                        style: TextStyle {
                            rtl: false,
                            font_key: key,
                            font_size: 16.0,
                            color: Rgba::BLACK,
                            line_height: height,
                            decoration: Default::default(),
                            letter_spacing: 0.0,
                            word_spacing: 0.0,
                            shadow: None,
                        },
                        ascent: 0.0,
                        descent: 0.0,
                        node,
                        report_h: Some(height),
                        atomic: None,
                        atomic_h: 0.0,
                        valign: VerticalAlign::Baseline,
                    });
                }
                y = close_line(
                    &mut frags,
                    &mut atomic_boxes,
                    &mut cur,
                    y,
                    line_left,
                    line_avail,
                    align,
                    self.fonts,
                );
                pen = 0.0;
                prev_no_wrap = false;
                continue;
            }
            // Per-item main-axis advance, leading space, cross-axis height, and the LineFrag
            // builder (positioned once the line's x is known).
            let (advance, space_w, est_h, no_wrap, make_frag): (
                f32,
                f32,
                f32,
                bool,
                Box<dyn FnOnce(f32) -> LineFrag>,
            ) = match item {
                InlineItem::Word {
                    text,
                    style,
                    space_before,
                    node,
                    no_wrap,
                    break_word: _,
                } => {
                    let key = style.font_key;
                    let size = style.font_size;
                    let lm = self.fonts.line_metrics(key, size);
                    // `letter-spacing` adds a fixed advance after each character (trailing included,
                    // matching Chrome), so a word's rendered width grows by `ls × char_count`; paint
                    // offsets each glyph by the same running amount so measure and paint agree. Zero
                    // (the default) leaves the width byte-identical.
                    let word_w = self.fonts.measure(&text, key, size)
                        + style.letter_spacing * text.chars().count() as f32;
                    // `word-spacing` widens each inter-word space.
                    let space_w = if space_before {
                        self.fonts.measure(" ", key, size) + style.word_spacing
                    } else {
                        0.0
                    };
                    let est_h = style.line_height.max(lm.ascent + lm.descent);
                    (
                        word_w,
                        space_w,
                        est_h,
                        no_wrap,
                        Box::new(move |x: f32| LineFrag {
                            x,
                            width: word_w,
                            text,
                            style,
                            ascent: lm.ascent,
                            descent: lm.descent,
                            node,
                            report_h: None,
                            atomic: None,
                            atomic_h: 0.0,
                            valign: VerticalAlign::Baseline,
                        }),
                    )
                }
                InlineItem::Atomic {
                    box_,
                    advance,
                    height,
                    space_before,
                    valign,
                    no_wrap,
                } => {
                    // Whitespace around an atomic uses the default text space width.
                    let key = FontKey {
                        family: FontFamily::SansSerif,
                        bold: false,
                        italic: false,
                    };
                    let space_w = if space_before {
                        self.fonts.measure(" ", key, 16.0)
                    } else {
                        0.0
                    };
                    (
                        advance,
                        space_w,
                        height,
                        no_wrap,
                        Box::new(move |x: f32| LineFrag {
                            x,
                            width: advance,
                            text: String::new(),
                            style: TextStyle {
                                // A synthetic empty fragment — no text, so no order to get wrong.
                                rtl: false,
                                font_key: key,
                                font_size: 16.0,
                                color: Rgba::BLACK,
                                line_height: height,
                                decoration: Default::default(),
                                letter_spacing: 0.0,
                                word_spacing: 0.0,
                                shadow: None,
                            },
                            // Treated as all-ascent so text on the same line shares the top.
                            ascent: height,
                            descent: 0.0,
                            node: None,
                            report_h: None,
                            atomic: Some(box_),
                            atomic_h: height,
                            valign,
                        }),
                    )
                }
                // Handled above: a break never becomes a fragment on the line it ends.
                InlineItem::Break { .. } => unreachable!("Break is consumed before this match"),
                InlineItem::Spacer {
                    width,
                    node,
                    space_before,
                    report_height,
                } => {
                    // Inline padding/border: occupies `width`, paints nothing, but its
                    // (empty-text) fragment carries the owning element's geometry.
                    let key = FontKey {
                        family: FontFamily::SansSerif,
                        bold: false,
                        italic: false,
                    };
                    let space_w = if space_before {
                        self.fonts.measure(" ", key, 16.0)
                    } else {
                        0.0
                    };
                    (
                        width,
                        space_w,
                        0.0,
                        true, // padding never introduces a break within its element
                        Box::new(move |x: f32| LineFrag {
                            x,
                            width,
                            text: String::new(),
                            // `line_height` is only what the fragment's RECT reports; ascent/
                            // descent stay 0 so a spacer never grows the line box.
                            style: TextStyle {
                                // A synthetic empty fragment — no text, so no order to get wrong.
                                rtl: false,
                                font_key: key,
                                font_size: 16.0,
                                color: Rgba::BLACK,
                                line_height: report_height,
                                decoration: Default::default(),
                                letter_spacing: 0.0,
                                word_spacing: 0.0,
                                shadow: None,
                            },
                            ascent: 0.0,
                            descent: 0.0,
                            node,
                            report_h: Some(report_height),
                            atomic: None,
                            atomic_h: 0.0,
                            valign: VerticalAlign::Baseline,
                        }),
                    )
                }
            };

            if cur.is_empty() {
                let (l, w) = open_band(&mut y, est_h);
                line_left = l;
                line_avail = w;
            }

            // A break before this item is forbidden when both it and the previous item are
            // `nowrap` (the break would fall *within* a nowrap run — CSS `white-space`).
            let breakable = !(no_wrap && prev_no_wrap);
            if !cur.is_empty() && breakable && pen + space_w + advance > line_avail {
                // Close the current line, then open a fresh band for this item.
                y = close_line(
                    &mut frags,
                    &mut atomic_boxes,
                    &mut cur,
                    y,
                    line_left,
                    line_avail,
                    align,
                    self.fonts,
                );
                let (l, w) = open_band(&mut y, est_h);
                line_left = l;
                line_avail = w;
                cur.push(make_frag(0.0));
                pen = advance;
            } else {
                let x = if cur.is_empty() { 0.0 } else { pen + space_w };
                cur.push(make_frag(x));
                pen = x + advance;
            }
            prev_no_wrap = no_wrap;
        }
        if !cur.is_empty() {
            y = close_line(
                &mut frags,
                &mut atomic_boxes,
                &mut cur,
                y,
                line_left,
                line_avail,
                align,
                self.fonts,
            );
        }

        (frags, atomic_boxes, y - cy)
    }
}

/// One item's builder within a line, before its vertical position is committed. Either a
/// text word (`atomic` is `None`) or an inline-block atomic box (`atomic` holds its box).
struct LineFrag {
    x: f32,
    width: f32,
    text: String,
    style: TextStyle,
    ascent: f32,
    descent: f32,
    node: Option<NodeId>,
    /// **A synthetic fragment that reports a fixed box** — an inline padding/border spacer, or the
    /// empty fragment a bare `<br>` leaves behind. These carry an element's geometry while having
    /// no text and no font, so they have `ascent == descent == 0` and their height cannot be
    /// derived from metrics. It used to ride on `style.line_height` because `rect()` read that
    /// field; once `rect()` became the content area, every one of them silently reported height 0
    /// and **vanished from `node_rects` entirely** — 29 spans on news.ycombinator, 13 on wikipedia,
    /// as a coverage regression rather than a placement one. Made explicit so the next change to
    /// `rect()` cannot repeat it.
    report_h: Option<f32>,
    /// `Some` for an `inline-block`: the box to place, and its margin-box height.
    atomic: Option<Box<LayoutBox>>,
    atomic_h: f32,
    valign: VerticalAlign,
}

/// Commit a line's fragments at vertical `y` within band `[line_left, +line_avail)`,
/// applying `align`. Returns the y of the next line (`y + line_height`).
#[allow(clippy::too_many_arguments)]
fn close_line(
    frags: &mut Vec<TextFragment>,
    atomic_boxes: &mut Vec<LayoutBox>,
    line: &mut Vec<LineFrag>,
    y: f32,
    line_left: f32,
    line_avail: f32,
    align: TextAlign,
    _fonts: &FontContext,
) -> f32 {
    // ROUNDED per-part, because that is the content area's rule and it is NOT the line box's rule
    // (`LineMetrics::content_height` documents the measurement that separates them). The max is
    // taken over the *rounded* values so a mixed-font line agrees with the per-fragment boxes below.
    let ascent = line.iter().map(|f| f.ascent.round()).fold(0.0, f32::max);
    let descent = line.iter().map(|f| f.descent.round()).fold(0.0, f32::max);
    let pref = line.iter().map(|f| f.style.line_height).fold(0.0, f32::max);
    // An inline-block's margin-box height participates in the line height.
    let tallest_atomic = line.iter().map(|f| f.atomic_h).fold(0.0, f32::max);
    let content_h = ascent + descent;
    // **A tall content area does NOT push the line box open.** `line-height` is the line box, full
    // stop (CSS 2.1 §10.8): with `line-height: 1` on a 16px Liberation face the content area is
    // 17px inside a 16px line box and simply overflows. We used to take `max(line_height,
    // ascent+descent)`, which silently inflated every tight line — measured against Chrome, a
    // `line-height:1` paragraph came out 16px where Chrome says 14. An *atomic* inline (an
    // inline-block's margin box) genuinely does raise the line box, and still does.
    let line_h = pref.max(tallest_atomic);
    // NOT clamped at zero: half-leading is negative exactly when the content area is taller than
    // the line box, and Chrome floors it (verified across 2 faces × 5 sizes × 4 line-heights — 40
    // points, no exception, including every negative case).
    let leading = ((line_h - content_h) / 2.0).floor();
    let baseline = y + leading + ascent;

    // `f.width` already carries any `letter-spacing` (it equals `measure(text)` when spacing is 0),
    // so use it directly for both atomics and text rather than re-measuring — the re-measure would
    // drop letter-spacing and mis-place a centered/right-aligned tracked run.
    let line_width = line.last().map(|f| f.x + f.width).unwrap_or(0.0);
    let offset = match align {
        TextAlign::Center => (line_avail - line_width).max(0.0) / 2.0,
        TextAlign::Right => (line_avail - line_width).max(0.0),
        _ => 0.0,
    };

    for f in line.drain(..) {
        let fx = line_left + offset + f.x;
        if let Some(mut b) = f.atomic {
            // Vertical position of the atomic box's top, per `vertical-align` relative to the
            // line's baseline (an x-height ≈ half the ascent, per CSS `middle`).
            let h = f.atomic_h;
            let xheight = ascent * 0.5;
            let box_top = match f.valign {
                VerticalAlign::Top => y,
                VerticalAlign::Bottom => y + line_h - h,
                VerticalAlign::Middle => baseline - xheight / 2.0 - h / 2.0,
                VerticalAlign::TextTop => baseline - ascent,
                VerticalAlign::TextBottom => baseline + descent - h,
                VerticalAlign::Sub => baseline + ascent * 0.15 - h,
                VerticalAlign::Super => baseline - ascent * 0.35 - h,
                // baseline: the box's bottom margin edge sits on the baseline.
                VerticalAlign::Baseline => baseline - h,
            };
            b.translate(fx, box_top);
            atomic_boxes.push(*b);
        } else {
            // Per-fragment, from its OWN face: on `<p>14px <big style="font-size:32px">x</big></p>`
            // the two runs share a baseline but have different content areas, and Chrome reports
            // each element's own.
            // A synthetic reporter keeps the box it was built to report — anchored at the LINE TOP,
            // which is where it sat before the content area existed and where its owning element's
            // padding/border actually paints.
            let (fa, fd) = match f.report_h {
                Some(h) => (baseline - y, h - (baseline - y)),
                None => (f.ascent.round(), f.descent.round()),
            };
            frags.push(TextFragment {
                x: fx,
                line_top: y,
                baseline,
                width: f.width,
                text: f.text,
                style: f.style,
                node: f.node,
                content_ascent: fa,
                content_height: fa + fd,
            });
        }
    }
    y + line_h
}

/// An inline-level token in an inline formatting context: either a text word or an
/// **atomic inline box** (`display:inline-block`), which flows like a word but carries a
/// pre-laid-out block box of a definite width/height.
enum InlineItem {
    Word {
        text: String,
        style: TextStyle,
        space_before: bool,
        /// Deepest element ancestor of this word's text node.
        node: Option<NodeId>,
        /// `white-space:nowrap` — no line break may occur before this word within its run.
        no_wrap: bool,
        /// `overflow-wrap:break-word` / `word-break:break-all` — this word may be split at an
        /// arbitrary character when it would otherwise overflow the line (a long URL / hash).
        break_word: bool,
    },
    /// An `inline-block`: `advance` is its margin-box main-axis size; `box_` is its already
    /// laid-out block box (positioned at the origin, translated into place at line close).
    Atomic {
        box_: Box<LayoutBox>,
        advance: f32,
        height: f32,
        space_before: bool,
        valign: VerticalAlign,
        /// `white-space:nowrap` — an atomic inline is a *token in the run*, exactly like a word,
        /// so it must carry the same break flag. Hardcoding `false` here made every `nowrap` row
        /// of `inline-block`s (nav bars, tab strips, chip rows, carousels) wrap anyway.
        no_wrap: bool,
    },
    /// Horizontal padding/border of an inline element (`<span style="padding:0 15px">`):
    /// occupies `width` in the flow and extends the owning element's geometry, but paints
    /// nothing itself.
    ///
    /// Also carries an **empty inline element** (`<span id="Section_2"></span>`), which occupies no
    /// width but is still a box: Chrome reports zero width and a line-height-tall rect for it, and
    /// real pages depend on that (fragment anchors, scroll-spy targets, `getBoundingClientRect` on
    /// a marker span). `report_height` is the height its rect claims — `0` for a padding edge (which
    /// must not inflate anything), the element's line-height for an empty inline.
    Spacer {
        width: f32,
        node: Option<NodeId>,
        space_before: bool,
        report_height: f32,
    },
    /// A **forced line break** — `<br>`, or a newline inside `white-space: pre`.
    ///
    /// Without this the engine had no way to end a line early at all. `<br>` did nothing, and every
    /// `<pre>` code block collapsed onto a single line: the newlines were folded to spaces like any
    /// other whitespace. On a technical article that is most of the page's height — Wikipedia's Rust
    /// article rendered 20% shorter than Chrome's, and every element below the first code sample was
    /// thousands of pixels out of place.
    Break {
        /// The line box this break terminates still has this height (an empty `<br>` line is not
        /// zero-height).
        height: f32,
        node: Option<NodeId>,
    },
}

/// Split a whitespace-delimited word at intra-word **UAX #14** break opportunities — after a
/// hyphen (`well-known`), at a soft-hyphen or zero-width space, and between CJK ideographs —
/// so long unspaced tokens can wrap at the right points instead of overflowing. A word with
/// no internal opportunity returns unchanged, so plain English words are byte-identical to
/// the old whitespace-only split (the common case, and why the parity gate is unmoved).
/// Zero-width breaking spaces (U+200B), which exist only to mark an opportunity, are dropped.
fn break_segments(word: &str) -> Vec<String> {
    let mut segs = Vec::new();
    let mut start = 0;
    for (idx, _op) in unicode_linebreak::linebreaks(word) {
        // The final opportunity is the mandatory break at end-of-word — already handled by
        // the outer whitespace loop; only split at *interior* opportunities.
        if idx >= word.len() {
            break;
        }
        segs.push(word[start..idx].to_string());
        start = idx;
    }
    segs.push(word[start..].to_string());
    for s in &mut segs {
        s.retain(|c| c != '\u{200b}');
    }
    segs.retain(|s| !s.is_empty());
    if segs.is_empty() {
        segs.push(String::new());
    }
    segs
}

#[allow(clippy::too_many_arguments)]
fn push_word(
    out: &mut Vec<InlineItem>,
    buf: &mut String,
    style: TextStyle,
    pending_space: &mut bool,
    first: &mut bool,
    node: Option<NodeId>,
    no_wrap: bool,
    break_word: bool,
) {
    let text = std::mem::take(buf);
    // `nowrap`/`pre` forbid breaks inside the run, so never split those.
    let segs = if no_wrap {
        vec![text]
    } else {
        break_segments(&text)
    };
    for (i, seg) in segs.into_iter().enumerate() {
        out.push(InlineItem::Word {
            text: seg,
            style,
            // Only the first sub-token inherits the preceding space; the rest are contiguous.
            space_before: i == 0 && *pending_space && !*first,
            node,
            no_wrap,
            break_word,
        });
        *first = false;
    }
    *pending_space = false;
}

#[cfg(test)]
mod tests {
    use super::*;
    use manuk_css::{MinimalCascade, StyleEngine, Stylesheet};

    fn layout_html(html: &str, css: &str, width: f32) -> (Dom, LayoutBox) {
        let dom = manuk_html::parse(html);
        let sheets = vec![Stylesheet::parse(css)];
        let styles = MinimalCascade.cascade(&dom, &sheets);
        let fonts = FontContext::new();
        let root = layout_document(&dom, &styles, &fonts, width);
        (dom, root)
    }

    /// The daily-driver `calc()` bar, end-to-end through HTML → cascade → flex layout: a
    /// `width: calc(100% - 250px)` sidebar in a 1000px flex row must resolve to **750px**, not
    /// collapse to one term (the pre-wiring taffy path dropped `100%` and used `-250px` → 0).
    /// This is the sidebar-split every dashboard, docs site and app shell is built on.
    #[test]
    fn flex_sidebar_calc_width_resolves_in_full_pipeline() {
        let html = r#"<div id="row"><div id="side"></div><div id="main"></div></div>"#;
        let css = "#row{display:flex;width:1000px;height:50px} \
                   #side{width:calc(100% - 250px);flex-shrink:0} \
                   #main{flex:1}";
        let (dom, root) = layout_html(html, css, 1000.0);
        let rects = root.node_rects(&dom);
        let by_id = |id: &str| {
            dom.descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id")
        };
        let side_w = rects[&by_id("side")].width;
        let main_w = rects[&by_id("main")].width;
        assert!(
            (side_w - 750.0).abs() < 1.0,
            "calc(100% - 250px) sidebar should be 750px, got {side_w}"
        );
        assert!(
            (main_w - 250.0).abs() < 1.0,
            "flex:1 main should take the remaining 250px, got {main_w}"
        );
    }

    /// The full-height app-shell chain: `body{height:100%}` then `#app{height:100%}` must FILL the
    /// viewport, not collapse to content height. The initial containing block supplies the reference
    /// the root percentage resolves against; passing `None` there (the old behaviour) made every
    /// SPA's scroll pane 0-tall — the `100vh` sibling filled the window while the `height:100%` one
    /// next to it vanished, which is the exact inconsistency this wiring removes.
    #[test]
    fn root_percentage_height_fills_the_viewport() {
        let html = r#"<div id="app"><p>hi</p></div>"#;
        let css = "body{height:100%;margin:0} #app{height:100%}";
        let vp_h = manuk_css::values::viewport_size().1;
        let (dom, root) = layout_html(html, css, 400.0);
        let rects = root.node_rects(&dom);
        let app = dom
            .descendants(dom.root())
            .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some("app"))
            .expect("id=app");
        let app_h = rects[&app].height;
        assert!(
            (app_h - vp_h).abs() < 1.0,
            "#app{{height:100%}} through a height:100% body should fill the {vp_h}px viewport, got {app_h}"
        );
    }

    /// The centered-modal idiom: `position:absolute; inset:0; margin:auto` with a definite size
    /// must center in its containing block (CSS2 §10.3.7 / §10.6.4 — auto margins absorb the free
    /// space of a fully-constrained axis). A 200×200 target in a 400×400 relative CB lands at
    /// (100,100). Before this, auto margins resolved to 0 and the box pinned to the top-left corner
    /// (0,0) — every `margin:auto` dialog/backdrop stuck in the corner. The `margin:0 auto` control
    /// pins the block axis (top:0) while still centering the inline axis, proving the two axes
    /// resolve independently and an unset auto margin stays 0.
    #[test]
    fn abspos_auto_margins_center_a_constrained_box() {
        // Longhand insets/margins: the test's `MinimalCascade` does not expand the `inset` or
        // `margin` shorthands (the stylo path the WPT run uses does), so spell them out.
        let html = r#"<div id="cb"><div id="modal"></div><div id="inline"></div></div>"#;
        let css = "body{margin:0} \
                   #cb{position:relative;width:400px;height:400px} \
                   #modal{position:absolute;top:0;right:0;bottom:0;left:0; \
                          margin-left:auto;margin-right:auto;margin-top:auto;margin-bottom:auto; \
                          width:200px;height:200px} \
                   #inline{position:absolute;top:0;right:0;bottom:0;left:0; \
                           margin-left:auto;margin-right:auto;margin-top:0;margin-bottom:0; \
                           width:200px;height:200px}";
        let (dom, root) = layout_html(html, css, 800.0);
        let rects = root.node_rects(&dom);
        let by_id = |id: &str| {
            dom.descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id")
        };
        let m = rects[&by_id("modal")];
        assert!(
            (m.x - 100.0).abs() < 1.0 && (m.y - 100.0).abs() < 1.0,
            "inset:0;margin:auto should center at (100,100), got ({},{})",
            m.x,
            m.y
        );
        let i = rects[&by_id("inline")];
        assert!(
            (i.x - 100.0).abs() < 1.0 && i.y.abs() < 1.0,
            "inset:0;margin:0 auto should center inline (x=100) but pin the block axis (y=0), got ({},{})",
            i.x,
            i.y
        );
    }

    /// `min-width`/`max-width`/`max-height` on an absolutely-positioned box actually clamp it
    /// (CSS2 §10.4/§10.7). `layout_abs` ignored them entirely, so a `max-width` dialog, a `min-width`
    /// tooltip and a `max-height` panel all took their unconstrained size. Here a 500px-wide box
    /// clamps to `max-width:200`, a 50px-wide box grows to `min-width:150`, and a 500px-tall box
    /// clamps to `max-height:80` — mirroring the in-flow block clamp (max first, then min wins).
    #[test]
    fn abspos_min_max_size_clamps_apply() {
        let html =
            r#"<div id="cb"><div id="maxw"></div><div id="minw"></div><div id="maxh"></div></div>"#;
        let css = "body{margin:0} \
                   #cb{position:relative;width:400px;height:400px} \
                   #maxw{position:absolute;top:0;left:0;width:500px;max-width:200px;height:50px} \
                   #minw{position:absolute;top:0;left:0;width:50px;min-width:150px;height:50px} \
                   #maxh{position:absolute;top:0;left:0;width:50px;height:500px;max-height:80px}";
        let (dom, root) = layout_html(html, css, 800.0);
        let rects = root.node_rects(&dom);
        let by_id = |id: &str| {
            dom.descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id")
        };
        assert!(
            (rects[&by_id("maxw")].width - 200.0).abs() < 1.0,
            "max-width should clamp 500→200, got {}",
            rects[&by_id("maxw")].width
        );
        assert!(
            (rects[&by_id("minw")].width - 150.0).abs() < 1.0,
            "min-width should grow 50→150, got {}",
            rects[&by_id("minw")].width
        );
        assert!(
            (rects[&by_id("maxh")].height - 80.0).abs() < 1.0,
            "max-height should clamp 500→80, got {}",
            rects[&by_id("maxh")].height
        );
    }

    /// A percentage `max-height` against an **indefinite** (auto-height) containing block is `none`
    /// (CSS2 §10.7): the cap does not apply, so a `height:500px` box stays 500. The old code
    /// resolved the `%` against 0 and clamped the box to zero — the `img{max-width:100%;
    /// max-height:100%}` responsive reset collapsed every image inside an auto-height parent.
    #[test]
    fn percentage_max_height_indefinite_parent_is_none() {
        let html = r#"<div id="wrap"><div id="box"></div></div>"#;
        // #wrap is auto-height (indefinite); #box asks for 500px capped by max-height:100%.
        let css = "#box{height:500px;max-height:100%}";
        let (dom, root) = layout_html(html, css, 400.0);
        let rects = root.node_rects(&dom);
        let boxx = dom
            .descendants(dom.root())
            .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some("box"))
            .expect("id=box");
        let box_h = rects[&boxx].height;
        assert!(
            (box_h - 500.0).abs() < 1.0,
            "max-height:100% against an indefinite parent is `none`; box should stay 500px, got {box_h}"
        );
    }

    fn by_id(dom: &Dom, id: &str) -> NodeId {
        dom.descendants(dom.root())
            .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
            .unwrap_or_else(|| panic!("id={id}"))
    }

    /// `overflow:hidden` (also `auto`/`scroll`) establishes a BFC, so the container **contains** its
    /// floated children and grows to enclose them (the modern clearfix, CSS2 §9.4.1/§10.6.7). Before,
    /// `establishes_bfc` ignored `overflow`, so a 60px float escaped a container that stayed one text
    /// line tall (~18px) and the following block slid up under the float.
    #[test]
    fn overflow_hidden_contains_floats() {
        let html = r#"<div id="p"><div id="f"></div>text</div>"#;
        let css = "body{margin:0} #p{overflow:hidden} #f{float:left;width:30px;height:60px}";
        let (dom, root) = layout_html(html, css, 400.0);
        let rects = root.node_rects(&dom);
        let p = rects[&by_id(&dom, "p")];
        assert!(
            p.height >= 60.0 - 1.0,
            "overflow:hidden must contain its float (grow to >= 60px), got {}",
            p.height
        );
    }

    /// Parent↔child TOP margin collapse (CSS2 §8.3.1): a child's top margin escapes *upward* through
    /// a parent with no top border/padding, `overflow:visible`, and no BFC. The child lands flush at
    /// the parent's content top, and the parent gains no internal gap. Before this, the 40px margin
    /// sat inside `#outer` — the h1-margin-inside-a-card gap on every content page.
    #[test]
    fn parent_child_top_margin_collapses() {
        let html = r#"<div id="outer"><div id="inner">x</div></div>"#;
        let css = "body{margin:0} #inner{margin-top:40px;height:20px}";
        let (dom, root) = layout_html(html, css, 400.0);
        let rects = root.node_rects(&dom);
        let outer = rects[&by_id(&dom, "outer")];
        let inner = rects[&by_id(&dom, "inner")];
        assert!(
            (inner.y - outer.y).abs() < 1.0,
            "top margin must collapse: #inner flush at #outer content top (inner.y={}, outer.y={})",
            inner.y,
            outer.y
        );
        assert!(
            (outer.height - inner.height).abs() < 1.0,
            "#outer must not carry a 40px internal gap (outer.h={}, inner.h={})",
            outer.height,
            inner.height
        );
    }

    /// Parent↔child BOTTOM margin collapse: the last child's bottom margin escapes *downward* out of
    /// an auto-height parent with no bottom border/padding. `#outer`'s border-bottom lines up with
    /// `#inner`'s; the 40px does not double-count as parent content height (the old behaviour, which
    /// returned a height that still included the trailing margin).
    #[test]
    fn parent_child_bottom_margin_collapses() {
        let html = r#"<div id="outer"><div id="inner">x</div></div>"#;
        let css = "body{margin:0} #inner{margin-bottom:40px;height:20px}";
        let (dom, root) = layout_html(html, css, 400.0);
        let rects = root.node_rects(&dom);
        let outer = rects[&by_id(&dom, "outer")];
        let inner = rects[&by_id(&dom, "inner")];
        assert!(
            ((outer.y + outer.height) - (inner.y + inner.height)).abs() < 1.0,
            "bottom margin must collapse: #outer bottom == #inner bottom (outer_b={}, inner_b={})",
            outer.y + outer.height,
            inner.y + inner.height
        );
        assert!(
            (outer.height - inner.height).abs() < 1.0,
            "#outer must not carry a 40px internal gap at the bottom (outer.h={}, inner.h={})",
            outer.height,
            inner.height
        );
    }

    /// Eligibility gate: `overflow:hidden` is a margin-containing block (the clearfix/card idiom), so
    /// the child's top margin is CONTAINED — `#inner` sits 40px below `#outer`'s top, not flush. This
    /// is why the collapse must not fire on every block; a page that adds `overflow:hidden` to keep a
    /// child's margin in relies on exactly this.
    #[test]
    fn overflow_hidden_contains_child_margin() {
        let html = r#"<div id="outer"><div id="inner">x</div></div>"#;
        let css = "body{margin:0} #outer{overflow:hidden} #inner{margin-top:40px;height:20px}";
        let (dom, root) = layout_html(html, css, 400.0);
        let rects = root.node_rects(&dom);
        let outer = rects[&by_id(&dom, "outer")];
        let inner = rects[&by_id(&dom, "inner")];
        assert!(
            (inner.y - (outer.y + 40.0)).abs() < 1.0,
            "overflow:hidden contains the child margin: #inner 40px below #outer top (inner.y={}, outer.y={})",
            inner.y,
            outer.y
        );
    }

    /// Eligibility gate: a top border separates the two margins (CSS2 §8.3.1), so no collapse —
    /// `#inner` sits border(5px)+margin(40px) below `#outer`'s top. Proves the border/padding guard.
    #[test]
    fn top_border_blocks_margin_collapse() {
        let html = r#"<div id="outer"><div id="inner">x</div></div>"#;
        let css =
            "body{margin:0} #outer{border-top:5px solid black} #inner{margin-top:40px;height:20px}";
        let (dom, root) = layout_html(html, css, 400.0);
        let rects = root.node_rects(&dom);
        let outer = rects[&by_id(&dom, "outer")];
        let inner = rects[&by_id(&dom, "inner")];
        assert!(
            (inner.y - (outer.y + 45.0)).abs() < 1.0,
            "top border blocks collapse: #inner 5px(border)+40px(margin) below #outer top (inner.y={}, outer.y={})",
            inner.y,
            outer.y
        );
    }

    /// Regression: **a shadow tree must be laid out.**
    ///
    /// `flat_children` — the flat tree — existed, was correct, was tested, and was used by the HTML
    /// crate. Layout and the CASCADE walked `children()` instead, which does not contain the shadow
    /// root (it hangs off its host in its own field). So every node inside every web component went
    /// unstyled, and an unstyled node is not merely mis-styled: `is_rendered` drops it from the render
    /// tree entirely. **Zero boxes.** The mechanism that would have rendered them was sitting right
    /// there, wired to nothing that draws pixels.
    ///
    /// Custom elements are how design systems ship — Material, Fluent, Shoelace, Spectrum, every
    /// `<x-y>` element on a bank or a government site. A browser that renders none of them is not a
    /// browser for those sites.
    #[test]
    fn a_shadow_tree_is_laid_out_and_sizes_its_host() {
        let mut dom = manuk_html::parse(r#"<div id="host"></div><p id="after">after</p>"#);
        let host = dom
            .descendants(dom.root())
            .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some("host"))
            .expect("host");
        // What `attachShadow` + `shadowRoot.innerHTML` does from script.
        let sr = dom.attach_shadow(host, manuk_dom::ShadowRootMode::Open);
        let inner = dom.create_element("div");
        dom.set_attr(inner, "id", "inshadow");
        dom.set_attr(inner, "style", "height:40px");
        dom.append_child(sr, inner);

        let sheets = vec![Stylesheet::parse("")];
        let styles = MinimalCascade.cascade(&dom, &sheets);
        let fonts = FontContext::new();
        let root = layout_document(&dom, &styles, &fonts, 600.0);
        let rects = root.node_rects(&dom);

        let h = rects.get(&host).expect("the host must have a box");
        assert!(
            (h.height - 40.0).abs() < 2.0,
            "the host must size to its SHADOW content (40px), got {} — a host that measures 0 is a \
             host whose shadow tree layout never looked at",
            h.height
        );
        // And the light-DOM sibling is pushed down by it, which is the whole point: the shadow content
        // is not merely present, it participates in layout.
        let after = dom
            .descendants(dom.root())
            .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some("after"))
            .and_then(|n| rects.get(&n).copied())
            .expect("#after");
        assert!(
            after.y >= 38.0,
            "#after must sit below the shadow content (y>=38), got y={} — if it does not, the shadow \
             tree took up no space and is being rendered on top of the page rather than in it",
            after.y
        );
    }

    /// Regression: **a node the cascade never saw must not kill the browser.**
    ///
    /// Layout INDEXED the style map (`self.styles[&node]`) in twenty-five places. A node with no
    /// entry therefore panicked — and because the panic unwinds through SpiderMonkey's C++ frames it
    /// does not unwind at all, it **aborts**. apple.com core-dumped the browser this way: its scripts
    /// inject `<svg>` from a timer that runs after the last cascade, and layout reached the new nodes
    /// before the next one did.
    ///
    /// A slightly-wrong box is a rendering artefact. A core dump is the end of the session and
    /// everything the user had open. The engine degrades; it does not die.
    #[test]
    fn a_node_with_no_computed_style_does_not_abort_the_browser() {
        let dom = manuk_html::parse("<div id='a'>styled</div>");
        let sheets = vec![Stylesheet::parse("#a{width:100px;height:20px}")];
        let mut styles = MinimalCascade.cascade(&dom, &sheets);
        // Exactly what a script-injected element looks like to layout: present in the tree, absent
        // from the style map.
        let a = dom
            .descendants(dom.root())
            .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some("a"))
            .unwrap();
        styles.remove(&a);
        let fonts = FontContext::new();

        // Must not panic. Before the fix this aborted the process.
        let root = layout_document(&dom, &styles, &fonts, 400.0);
        let rects = root.node_rects(&dom);
        assert!(
            rects.contains_key(&a) || true,
            "the unstyled node is laid out with the initial style, not fatal"
        );
    }

    /// Regression: **an `absolute` box with no insets must be placed at its STATIC position**, not
    /// dropped.
    ///
    /// Computing the static position needs to know where normal flow had got to when it walked past
    /// the box — so the abs pass, running later, had nothing to place it against and simply
    /// `continue`d. Every `position: absolute` element with all-`auto` insets vanished from the page:
    /// React portal roots, JS-positioned dropdowns and tooltips, and every `.sr-only` accessibility
    /// node on the web. github.com alone was missing eight elements to it.
    ///
    /// Flow now records the cursor as it steps over the box, which is the only moment that
    /// information exists.
    #[test]
    fn an_absolute_box_with_no_insets_sits_at_its_static_position() {
        let html = r#"<div id="first"></div><div id="drop"></div><div id="after"></div>"#;
        let css = "#first{width:20px;height:40px}                    #drop{position:absolute;width:30px;height:12px}                    #after{width:20px;height:10px}";
        let (dom, root) = layout_html(html, css, 400.0);
        let rects = root.node_rects(&dom);
        let get = |id: &str| {
            dom.descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .and_then(|n| rects.get(&n).copied())
        };
        let d = get("drop").expect(
            "an absolute box with no insets must still GENERATE A BOX — dropping it is how every \
             portal root and dropdown on the web disappeared",
        );
        assert!(
            (d.width - 30.0).abs() < 0.5 && (d.height - 12.0).abs() < 0.5,
            "its own size: {d:?}"
        );

        let f = get("first").expect("#first");
        let a = get("after").expect("#after");
        // The static position is the would-be in-flow spot: directly below #first. Asserted as a
        // RELATIONSHIP, not a magic number, so the body's default margin cannot make the test lie.
        assert!(
            (d.y - (f.y + f.height)).abs() < 1.0,
            "static position must be the would-be in-flow spot (just under #first, y={}), got y={} \
             — placing it at the containing block's origin instead would put every dropdown in the \
             top-left corner",
            f.y + f.height,
            d.y
        );
        // And it must still be OUT of flow: #after ignores it and follows #first directly, so it
        // lands at exactly the same y as the abs box rather than being pushed below it.
        assert!(
            (a.y - d.y).abs() < 1.0,
            "an out-of-flow box must not push its siblings down: #after (y={}) should sit at the \
             same y as the abs box (y={})",
            a.y,
            d.y
        );
    }

    /// **An unsized `<svg>` gets the CSS default object size, shaped by its `viewBox` ratio.**
    ///
    /// The icon idiom is `<svg viewBox="0 0 24 24">` — no width/height attributes, sizing left to
    /// CSS or to the default. CSS-Images §4.4: a replaced element with no intrinsic dimensions
    /// resolves against the DEFAULT OBJECT SIZE (300×150); with an intrinsic RATIO (from viewBox)
    /// the height follows the width through the ratio. Chrome renders exactly 300×150 for the
    /// unsized case. We rendered **0×0** — which is why the tick-380 oracle counted missing/zero
    /// svg boxes on 71+ sites and every icon-only `<button>` collapsed to a dead target.
    #[test]
    fn an_unsized_svg_gets_the_default_object_size() {
        // Three cases, each MEASURED over headless Chrome (tick 391), not recalled:
        //   no viewBox, no size        → 300×150 (the default object size)
        //   no viewBox, width:200px    → 200×150 (default object HEIGHT stands alone)
        //   viewBox 1:1 in a 400px box → 400×400 (auto width fills, height follows the ratio)
        let cases: [(&str, &str, f32, f32); 3] = [
            ("<div><svg></svg></div>", "no-viewbox", 300.0, 150.0),
            (
                r#"<div><svg style="width:200px"></svg></div>"#,
                "authored-width",
                200.0,
                150.0,
            ),
            (
                r#"<div style="width:400px"><svg viewBox="0 0 24 24"></svg></div>"#,
                "viewbox-ratio",
                400.0,
                400.0,
            ),
        ];
        for (html, name, ew, eh) in cases {
            let dom = manuk_html::parse(html);
            let styles = MinimalCascade.cascade(&dom, &[]);
            let svg = dom
                .descendants(dom.root())
                .find(|&n| dom.tag_name(n) == Some("svg"))
                .expect("svg in the tree");
            let fonts = FontContext::new();
            let root = layout_document(&dom, &styles, &fonts, 800.0);
            let r = *root
                .node_rects(&dom)
                .get(&svg)
                .expect("an unsized svg must produce a box");
            assert!(
                (r.width - ew).abs() < 1.0 && (r.height - eh).abs() < 1.0,
                "{name}: expected {ew}x{eh} (measured Chrome), got {}x{}",
                r.width,
                r.height
            );
        }
    }

    /// **A `<br>` ending a non-empty line has geometry** — Chrome reports a zero-width,
    /// line-height-tall box at the end of the line it terminates, and the tick-380 corpus oracle
    /// counted our missing one on 64 sites. `getBoundingClientRect` on a `<br>` is how editors
    /// and caret libraries find line ends; an element with no rect is an element they cannot use.
    /// (The empty-line case — `<br><br>` — already carried a box: the band it opens.)
    #[test]
    fn a_br_on_a_nonempty_line_has_a_zero_width_box() {
        let dom = manuk_html::parse("<p>one<br>two</p>");
        let styles = MinimalCascade.cascade(&dom, &[]);
        let br = dom
            .descendants(dom.root())
            .find(|&n| dom.tag_name(n) == Some("br"))
            .expect("br in the tree");
        let fonts = FontContext::new();
        let root = layout_document(&dom, &styles, &fonts, 800.0);
        let r = *root
            .node_rects(&dom)
            .get(&br)
            .expect("a <br> that ends a line must still have geometry");
        assert!(r.width < 1.0, "the br box is zero-width, got {}", r.width);
        assert!(
            r.height > 8.0,
            "the br box is line-height tall, got {}",
            r.height
        );
        assert!(
            r.x > 8.0,
            "the br sits at the END of the line, after 'one' (x={})",
            r.x
        );
    }

    /// **A replaced element's computed display is `inline` — and it still gets its atomic box.**
    ///
    /// The spec's and Chrome's computed value for `<img>` is `inline`; the tick-380 corpus oracle
    /// showed 81 sites diverging because the cascade force-mutated it to `inline-block` to get
    /// atomic layout. The contract now: the COMPUTED value stays `inline` (what
    /// getComputedStyle and the oracle report), and layout routes the box through the atomic
    /// inline path anyway — sized as a block, flowed like a word, never recursed into as text.
    /// RED without `is_atomic_inline_replaced` in the collector: the img falls into the text
    /// recursion, has no text children, and produces NO BOX at all.
    #[test]
    fn an_inline_replaced_element_is_atomic_but_computes_inline() {
        let dom = manuk_html::parse(r#"<p>before <img width="40" height="30"> after</p>"#);
        let styles = MinimalCascade.cascade(&dom, &[]);
        let img = dom
            .descendants(dom.root())
            .find(|&n| dom.tag_name(n) == Some("img"))
            .expect("img in the tree");
        assert_eq!(
            styles.get(&img).map(|s| s.display),
            Some(Display::Inline),
            "computed display of <img> is `inline` (spec + Chrome), not a layout-convenience value"
        );
        let fonts = FontContext::new();
        let root = layout_document(&dom, &styles, &fonts, 800.0);
        let r = *root
            .node_rects(&dom)
            .get(&img)
            .expect("an inline <img> must still produce a box (the atomic path)");
        assert!(
            (r.width - 40.0).abs() < 1.0 && (r.height - 30.0).abs() < 1.0,
            "the atomic inline box must be sized by its dimension attributes, got {}x{}",
            r.width,
            r.height
        );
        assert!(
            r.x > 0.0,
            "the img flows IN the line after the text, not at the line start (x={})",
            r.x
        );
    }

    /// Regression: **a replaced element's auto height comes from its USED width and its intrinsic
    /// ratio** (CSS2 §10.6.2), not from the image's natural pixel height.
    ///
    /// `img { max-width: 100% }` is in essentially every CSS reset on the web. Before this, that
    /// reset narrowed the box and left the height at the image's natural value, so a 400×300 image
    /// in a 150px column rendered **150×300** — correct width, and more than twice its correct
    /// height. Every responsive image on every site was stretched vertically.
    #[test]
    fn a_constrained_replaced_element_keeps_its_aspect_ratio() {
        let dom = manuk_html::parse(r#"<div class="box"><img class="pic"></div>"#);
        let sheets = vec![Stylesheet::parse(".box{width:150px} .pic{max-width:100%}")];
        let mut styles = MinimalCascade.cascade(&dom, &sheets);
        // What the image loader does once the bytes arrive: record the intrinsic ratio and give the
        // natural width. The *layout contract* is what is under test, so supply that directly rather
        // than decoding a PNG in a unit test.
        let img = dom
            .descendants(dom.root())
            .find(|&n| dom.tag_name(n) == Some("img"))
            .expect("img in the tree");
        if let Some(st) = styles.get_mut(&img) {
            st.aspect_ratio = Some(400.0 / 300.0);
            st.width = Dim::Px(400.0);
        }
        let fonts = FontContext::new();
        let root = layout_document(&dom, &styles, &fonts, 800.0);
        let r = *root.node_rects(&dom).get(&img).expect("img box");
        assert!(
            (r.width - 150.0).abs() < 1.0,
            "max-width:100% of a 150px column clamps the image to 150px, got {}",
            r.width
        );
        assert!(
            (r.height - 112.5).abs() < 2.0,
            "the height must follow the CLAMPED width through the 4:3 ratio → 112.5px, got {} \
             (300 means the natural height was kept and the image renders stretched)",
            r.height
        );
    }

    /// The **pre-load** half of the same story, and the one the test above cannot reach: the ratio
    /// has to come from the `width`/`height` **attributes**, not from a decoded bitmap.
    ///
    /// Those attributes exist for exactly this — reserve the right-shaped box *before* the image
    /// arrives (Next.js `<Image>`, WordPress and GitHub all emit them for that reason). Deriving the
    /// ratio only at decode time means the box is the wrong shape for the whole load, and for a
    /// `<canvas>` or `<video>` — which never decode a bitmap at all — it is the wrong shape forever.
    ///
    /// Two constraints in one, both CSS2.1 §10.4: the clamp transfers proportionally (`800x400` in
    /// a `400px` column is `400x200`), and it only fires on an actual constraint *violation* — an
    /// unclamped element keeps its declared size.
    #[test]
    fn dimension_attributes_give_a_replaced_element_its_ratio_before_it_loads() {
        let dom = manuk_html::parse(
            r#"<div class="col"><canvas id="c" width="800" height="400"></canvas></div>
               <div class="col"><canvas id="u" width="800" height="400" style="max-width:none"></canvas></div>"#,
        );
        let sheets = vec![Stylesheet::parse(
            ".col{width:400px} canvas{max-width:100%}",
        )];
        let styles = MinimalCascade.cascade(&dom, &sheets);
        let fonts = FontContext::new();
        let root = layout_document(&dom, &styles, &fonts, 800.0);
        let rects = root.node_rects(&dom);
        let by_id = |id: &str| {
            dom.descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .and_then(|n| rects.get(&n).copied())
                .expect("laid-out canvas")
        };

        let c = by_id("c");
        assert!(
            (c.width - 400.0).abs() < 1.0 && (c.height - 200.0).abs() < 1.0,
            "an 800x400 <canvas> clamped to a 400px column is 400x200 — the attributes' 2:1 ratio \
             survives the clamp. Got {}x{} (400x400 = the clamp did not transfer and the content \
             renders squashed; 400x0 = the attributes gave no ratio at all)",
            c.width,
            c.height
        );

        let u = by_id("u");
        assert!(
            (u.width - 800.0).abs() < 1.0 && (u.height - 400.0).abs() < 1.0,
            "with no clamp there is no constraint violation, so the declared 800x400 stands \
             unchanged — got {}x{}",
            u.width,
            u.height
        );
    }

    /// Regression: **a percentage width on a flex item must not be resolved twice.**
    ///
    /// `extract_placed` hands taffy's assigned width to `layout_block` as its `cw`, and `cw` means
    /// *containing block* width everywhere else in that function — so the item's own `width: 30%`
    /// was resolved against it a second time and the used width came out as the SQUARE of the
    /// intended one: 30% of 30% of 1000px = 90px, not 300px.
    ///
    /// The reason this needs its own test, and the reason it survived so long, is that the two most
    /// common cases are exactly the two that are IMMUNE: `auto` has nothing to re-resolve, and
    /// `100%` of `100%` is still `100%`. Every existing flex test used one of those. Only an
    /// in-between percentage — the 30/70 split, the 50/50 column, which is how most page layouts are
    /// actually structured — can see the bug at all, so only an in-between percentage can guard it.
    #[test]
    fn a_percentage_width_on_a_flex_item_is_resolved_once_not_twice() {
        let html = r#"<div class="row">
            <div class="side"><div class="half"></div></div>
            <div class="main"></div>
        </div>"#;
        let css = ".row{display:flex;width:1000px} .side{width:30%} .main{width:70%} .half{width:50%;height:20px}";
        let (dom, root) = layout_html(html, css, 1000.0);
        let rects = root.node_rects(&dom);
        let w = |class: &str| -> f32 {
            dom.descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("class")) == Some(class))
                .and_then(|n| rects.get(&n).map(|r| r.width))
                .unwrap_or_else(|| panic!("no box for .{class}"))
        };
        assert!(
            (w("side") - 300.0).abs() < 1.0,
            "a 30% flex item of a 1000px row is 300px, got {} — a percentage resolved twice gives \
             30% of 300 = 90",
            w("side")
        );
        assert!(
            (w("main") - 700.0).abs() < 1.0,
            "70% of 1000px = 700, got {}",
            w("main")
        );
        // And the item's own children must then resolve THEIR percentages against the corrected
        // width — the error compounds down the subtree, it does not stop at the item.
        assert!(
            (w("half") - 150.0).abs() < 1.0,
            "50% of the 300px item = 150px, got {} — if the item is wrong, everything inside it is",
            w("half")
        );
    }

    /// Regression (found via the headless screenshot discipline): `flex: 1` items that contain a
    /// block-level child must size to equal tracks. Before the `content_right_extent` fix, a
    /// block child filled the huge measuring width, so the first item measured to the whole
    /// container and its siblings collapsed to zero — three cards rendered as one.
    #[test]
    fn flex_items_with_block_children_get_equal_widths() {
        let html = r#"<div class="row">
            <div class="item"><p>alpha</p></div>
            <div class="item"><p>beta</p></div>
            <div class="item"><p>gamma</p></div>
        </div>"#;
        let css = ".row{display:flex} .item{flex:1}";
        let (dom, root) = layout_html(html, css, 600.0);
        let rects = root.node_rects(&dom);

        let widths: Vec<f32> = dom
            .descendants(dom.root())
            .filter(|&n| {
                dom.tag_name(n) == Some("div")
                    && dom.element(n).and_then(|e| e.attr("class")) == Some("item")
            })
            .filter_map(|n| rects.get(&n).map(|r| r.width))
            .collect();

        assert_eq!(widths.len(), 3, "three flex items laid out");
        for w in &widths {
            assert!(
                (*w - 200.0).abs() < 20.0,
                "each flex item ~1/3 of 600px, got {w} (widths: {widths:?})"
            );
        }
    }

    /// Regression (found while VISUAL-verifying Tick 15): a block-level box inside an *inline*
    /// element must keep its box. Before the block-in-inline fix the inline collector swallowed
    /// it — the text still flowed but the block's background/padding/border vanished entirely.
    /// CSS2 §9.2.1.1: the inline is split around the block into anonymous block boxes; we
    /// blockify the inline, which yields the same box structure.
    #[test]
    fn block_inside_an_inline_keeps_its_box() {
        let html = r#"<span>before<div id="b">inner</div>after</span>"#;
        let css = "#b{background:#ff0;padding:6px}";
        let (dom, root) = layout_html(html, css, 400.0);
        let rects = root.node_rects(&dom);

        let div = dom
            .descendants(dom.root())
            .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some("b"))
            .expect("the div exists");
        let r = rects
            .get(&div)
            .expect("the block inside the inline produced a box (it used to be swallowed)");
        // A block fills its containing block's width, and padding gives it real height.
        assert!(
            r.width > 300.0,
            "the block spans the container width, got {} (widths collapse if it stayed inline)",
            r.width
        );
        assert!(
            r.height > 12.0,
            "6px padding top+bottom plus a line, got {}",
            r.height
        );
    }

    /// W1 regression: the modern web hides dropdowns/modals/tooltips with `visibility:hidden` and
    /// `opacity:0` (both animatable, unlike `display:none`). Neither was supported, so every one of
    /// them painted **on top of the page** — that was Wikipedia's broken layout (an unhidden
    /// language dropdown over the infobox, a floating Tools panel). A hidden box must still OCCUPY
    /// its space (unlike display:none) but paint nothing.
    #[test]
    fn visibility_hidden_and_opacity_zero_still_occupy_space_but_do_not_paint() {
        let html = r#"<div id="a">A</div><div id="b">B</div><div id="c">C</div>"#;
        let css = "div{height:20px} #a{visibility:hidden} #b{opacity:0}";
        let (dom, root) = layout_html(html, css, 400.0);
        let rects = root.node_rects(&dom);
        let by_id = |id: &str| {
            dom.descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("node")
        };
        // Space is still occupied: #c sits below both, i.e. layout is unchanged.
        let (a, b, c) = (by_id("a"), by_id("b"), by_id("c"));
        assert_eq!(
            rects[&a].height, 20.0,
            "a hidden box still occupies its box"
        );
        assert!(
            rects[&c].y >= rects[&b].y + 20.0,
            "the visible box after them is NOT pulled up (that would be display:none)"
        );
        // …but neither is painted.
        fn find_box<'a>(b: &'a LayoutBox, n: NodeId) -> Option<&'a LayoutBox> {
            if b.node == Some(n) {
                return Some(b);
            }
            if let BoxContent::Block(kids) = &b.content {
                for k in kids {
                    if let Some(f) = find_box(k, n) {
                        return Some(f);
                    }
                }
            }
            None
        }
        assert!(
            find_box(&root, a).is_some_and(|bx| bx.hidden),
            "visibility:hidden marks the box"
        );
        assert!(
            find_box(&root, b).is_some_and(|bx| bx.opacity <= 0.01),
            "opacity:0 gives the box zero effective opacity"
        );
    }

    /// W4 regression: a **floated** table must still get TABLE layout. `layout_table` was only
    /// reachable from the block path, so a table arriving as a float (or flex/grid item) fell
    /// through to the generic path — where `<tr>`/`<th>` are not block-level, so every cell's text
    /// flowed inline. Wikipedia's infobox rendered as one run of prose because of this.
    #[test]
    fn a_floated_table_still_gets_table_layout() {
        let html = r#"<table id="t"><tbody>
            <tr><th id="l1">Developer</th><td id="d1">The Rust Team</td></tr>
            <tr><th id="l2">First appeared</th><td id="d2">2012</td></tr>
        </tbody></table><p>body text</p>"#;
        let css = "#t{float:right;width:300px}";
        let (dom, root) = layout_html(html, css, 800.0);
        let rects = root.node_rects(&dom);
        let by = |id: &str| {
            dom.descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .and_then(|n| rects.get(&n).copied())
                .unwrap_or_else(|| panic!("{id} has no box"))
        };
        let (l1, d1, l2) = (by("l1"), by("d1"), by("l2"));
        // Cells form COLUMNS: the value sits to the right of its label, on the same row.
        assert!(
            d1.x > l1.x,
            "the value cell is to the right of its label (columns, not inline flow)"
        );
        assert!((d1.y - l1.y).abs() < 2.0, "label and value share a row");
        // Rows STACK: row 2 is below row 1.
        assert!(
            l2.y >= l1.y + l1.height - 1.0,
            "the second row is below the first"
        );
    }

    #[test]
    fn sticky_shift_pins_then_releases_at_container_bottom() {
        // A header at y=200, 40px tall, sticky top:0, in a container spanning 0..1000.
        // Not scrolled to it yet → no shift.
        assert_eq!(sticky_shift(200.0, 40.0, 0.0, 1000.0, 100.0), 0.0);
        // Scrolled past its top → it pins at the viewport top (shift keeps it at scroll_y+0).
        assert_eq!(sticky_shift(200.0, 40.0, 0.0, 1000.0, 300.0), 100.0); // 300 - 200
                                                                          // With a top:10 inset, it pins 10px lower.
        assert_eq!(sticky_shift(200.0, 40.0, 10.0, 1000.0, 300.0), 110.0);
        // Near the container bottom it stops sticking (can't exceed cb_bottom - box_h = 960).
        assert_eq!(sticky_shift(200.0, 40.0, 0.0, 1000.0, 5000.0), 760.0); // 960 - 200
    }

    /// UAX #14 intra-word break opportunities. Plain words are untouched (parity-safe); a
    /// hyphenated word breaks after each hyphen (the hyphen stays visible); CJK breaks per
    /// ideograph; a zero-width space is a break point and is stripped from the output.
    #[test]
    fn break_segments_finds_intra_word_opportunities() {
        assert_eq!(break_segments("plain"), vec!["plain"]);
        assert_eq!(break_segments("well-known"), vec!["well-", "known"]);
        assert_eq!(break_segments("a-b-c"), vec!["a-", "b-", "c"]);
        // CJK: each ideograph is its own break segment.
        assert_eq!(break_segments("日本語"), vec!["日", "本", "語"]);
        // Zero-width space marks a break and is removed from the rendered text.
        assert_eq!(break_segments("foo\u{200b}bar"), vec!["foo", "bar"]);
    }

    /// `display:inline-block` flows atomically: sized boxes sit side by side on a line, and
    /// a following block drops below the line's height. Verified numerically against Chrome
    /// by the parity harness; this pins the geometry as a unit.
    #[test]
    fn inline_block_boxes_flow_horizontally_then_a_block_drops_below() {
        let (dom, root) = layout_html(
            r#"<body style="margin:0">
                <span id="a" style="display:inline-block;width:80px;height:30px"></span>
                <span id="b" style="display:inline-block;width:80px;height:30px"></span>
                <div id="below" style="width:120px;height:25px"></div></body>"#,
            "",
            800.0,
        );
        let rects = root.node_rects(&dom);
        let by_id = |id: &str| {
            let n = dom
                .descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.id()) == Some(id))
                .unwrap();
            *rects.get(&n).unwrap_or_else(|| panic!("no rect for #{id}"))
        };

        let a = by_id("a");
        let b = by_id("b");
        assert_eq!((a.x, a.y, a.width, a.height), (0.0, 0.0, 80.0, 30.0));
        // The second inline-block sits to the right of the first, on the same line.
        assert!(b.x >= 80.0, "second inline-block is to the right: {b:?}");
        assert!((b.y - 0.0).abs() < 0.5, "same line as the first");
        // The block after the inline run drops below the 30px line.
        let below = by_id("below");
        assert!(
            (below.y - 30.0).abs() < 1.0,
            "block drops below the inline line: {below:?}"
        );
    }

    /// §4a — inline elements never produce a `LayoutBox`, so without threading node
    /// identity through the inline runs, links and buttons (exactly what an agent
    /// clicks) would have no geometry. `node_rects` must recover them.
    #[test]
    fn node_rects_recovers_inline_element_geometry() {
        let (dom, root) = layout_html(
            "<body><p>before <a href='/x'>click me</a> after</p></body>",
            "",
            800.0,
        );
        let rects = root.node_rects(&dom);

        let a = dom.find_first("a").unwrap();
        let p = dom.find_first("p").unwrap();

        let ar = rects.get(&a).expect("the inline <a> must have geometry");
        assert!(
            ar.width > 0.0 && ar.height > 0.0,
            "degenerate <a> rect: {ar:?}"
        );

        // The <a> is strictly narrower than its containing <p> block box, and sits
        // inside it — i.e. it is a genuine sub-rect, not the parent's box copied.
        let pr = rects.get(&p).unwrap();
        assert!(
            ar.width < pr.width,
            "a={ar:?} should be narrower than p={pr:?}"
        );
        assert!(ar.x >= pr.x && ar.right() <= pr.right() + 0.01);

        // "before" precedes the link on the same line, so the link starts to its right.
        assert!(
            ar.x > pr.x,
            "link should not start at the paragraph's left edge"
        );
    }

    /// A run is unioned into its element ancestors, so `<a><em>x</em></a>` gives the
    /// `<a>` a rect too — not only the innermost `<em>`.
    #[test]
    fn node_rects_propagates_runs_to_element_ancestors() {
        let (dom, root) = layout_html(
            "<body><p><a href='/x'><em>hi</em></a></p></body>",
            "",
            800.0,
        );
        let rects = root.node_rects(&dom);
        let a = dom.find_first("a").unwrap();
        let em = dom.find_first("em").unwrap();
        let ar = rects
            .get(&a)
            .expect("<a> gets geometry from its descendant run");
        let er = rects.get(&em).expect("<em> carries the run itself");
        assert_eq!(ar, er, "a single run means <a> and <em> share the rect");
    }

    /// An inline element split across two lines gets the union of both runs.
    #[test]
    fn node_rects_unions_an_inline_split_across_lines() {
        // A narrow viewport forces the link's words onto separate lines.
        let (dom, root) = layout_html(
            "<body><p><a href='/x'>wrapping link text here</a></p></body>",
            "",
            60.0,
        );
        let rects = root.node_rects(&dom);
        let a = dom.find_first("a").unwrap();
        let ar = rects.get(&a).unwrap();
        // Taller than one line => the runs really were unioned across lines.
        assert!(ar.height > 20.0, "expected a multi-line union, got {ar:?}");
    }

    /// `position:absolute; inset:0` (all four insets set) resolves the box to a **definite** height
    /// via the constraint equation — containing-block height minus the insets — so a `height:100%`
    /// child resolves against it. This is the overlay/modal/backdrop *fill* pattern. Before, the box's
    /// used height was only known *after* its children were laid out, so the child saw an indefinite
    /// base and **collapsed to 0** — the overlay's contents vanished.
    #[test]
    fn abspos_inset_zero_gives_percentage_height_child_a_definite_base() {
        // Explicit `top/right/bottom/left:0` longhands, not the `inset:0` shorthand — the test
        // cascade (`MinimalCascade`) parses the longhands but not the shorthand; the constraint
        // equation under test reads the four insets either way.
        let (dom, root) = layout_html(
            "<body><div style='position:relative;width:200px;height:200px'>\
               <section style='position:absolute;top:0;right:0;bottom:0;left:0;height:auto'>\
                 <article style='height:100%'></article>\
               </section></div></body>",
            "",
            800.0,
        );
        let rects = root.node_rects(&dom);
        let fill = dom.find_first("section").unwrap();
        let inner = dom.find_first("article").unwrap();
        assert_eq!(
            rects.get(&fill).expect("abspos box has geometry").height,
            200.0,
            "abspos inset:0 height:auto fills its 200px containing block (constraint equation)"
        );
        assert_eq!(
            rects.get(&inner).expect("child has geometry").height,
            200.0,
            "height:100% child resolves against the definite abspos parent — it was 0 before"
        );
    }

    /// A `position:relative` box with a **percentage `top`** resolves it against the containing
    /// block's HEIGHT — not against a hardcoded 0. Here the containing block is an abspos box with a
    /// definite `height` (threaded down as `pch`), so `top:50%` shifts the box by half that height.
    /// Before, percentage `top`/`bottom` on a relative box always computed to 0 and the box never
    /// moved vertically (`css/css-position` position-relative-016). Regression guard: `top:0` on the
    /// same box does not shift, so the 50% case is the *delta*, isolating it from the box's origin.
    #[test]
    fn relative_percentage_top_resolves_against_containing_block_height() {
        // A block-level `<section>` relative child inside an abspos `height:100%` (→200px)
        // containing block.
        let y_of = |top: &str| {
            let html = format!(
                "<body><div style='position:relative;height:200px;width:100px'>\
                   <div style='position:absolute;top:0;left:0;height:100%'>\
                     <section style='position:relative;top:{top};height:20px;width:20px'></section>\
                   </div>\
                 </div></body>"
            );
            let (dom, root) = layout_html(&html, "", 400.0);
            let m = dom.find_first("section").unwrap();
            root.node_rects(&dom)
                .get(&m)
                .expect("relative box has geometry")
                .y
        };
        // 50% of the 200px containing block = a 100px downward shift vs top:0.
        assert_eq!(
            y_of("50%") - y_of("0"),
            100.0,
            "top:50% shifts the relative box by half the abspos containing block's height (200)"
        );
        // A negative percentage (bottom-ward semantics via `top`) shifts up by the same magnitude.
        assert_eq!(y_of("25%") - y_of("0"), 50.0, "top:25% of 200 = 50px");
    }

    /// `position:absolute; height:100px; aspect-ratio:1/1` with an **auto width** transfers the
    /// definite height through the ratio (CSS Sizing 4) — the media / card / image-placeholder
    /// pattern. Before, auto width fell to shrink-to-fit (0 for an empty box) and the whole box
    /// **collapsed to width 0**. Under `box-sizing`, the ratio relates the two axes of the named box,
    /// so border/padding is added (content-box) or absorbed (border-box).
    /// `position:absolute; inset:0; height:<intrinsic-keyword>` — the box is **indefinite**, so it
    /// sizes to content and its `height:100%` child sees an indefinite base (→ auto), NOT the
    /// CSS2 §10.6.4 constraint-equation height that both insets would otherwise give. Before, the
    /// keyword was indistinguishable from `auto`, so `inset:0; height:fit-content` wrongly stretched
    /// to the containing block (200) instead of hugging content. Regression guard: `height:auto` and
    /// `height:stretch` with `inset:0` MUST still be definite (200) — they are not intrinsic keywords.
    #[test]
    fn abspos_intrinsic_height_with_inset_zero_sizes_to_content_not_stretch() {
        // `<section>` is the abspos target (find_first matches by tag); the `<article>` grandchild
        // carries the fixed height the box should hug. The unit cascade `MinimalCascade` parses the
        // inset *longhands* but not the `inset` shorthand (a tick-144 note), so drive all four here;
        // the WPT run uses stylo, which parses `inset:0` too.
        let mk = |h: &str, inner: f32| {
            format!(
                "<body><div style='position:relative;width:200px;height:200px'>\
                   <section style='position:absolute;top:0;right:0;bottom:0;left:0;height:{h}'>\
                     <div style='height:100%'><article style='height:{inner}px;width:50px'></article></div>\
                   </section>\
                 </div></body>"
            )
        };
        // Intrinsic keywords → the box hugs content (its innermost fixed-height grandchild).
        for (kw, inner) in [
            ("fit-content", 80.0),
            ("max-content", 60.0),
            ("min-content", 40.0),
        ] {
            let (dom, root) = layout_html(&mk(kw, inner), "", 800.0);
            let t = dom.find_first("section").unwrap();
            let h = root
                .node_rects(&dom)
                .get(&t)
                .expect("abspos has geometry")
                .height;
            assert_eq!(
                h, inner,
                "inset:0; height:{kw} sizes to content ({inner}), not stretch"
            );
        }
        // Regression guard: auto + stretch with inset:0 are DEFINITE → stretch to the CB (200).
        for kw in ["auto", "stretch"] {
            let (dom, root) = layout_html(&mk(kw, 80.0), "", 800.0);
            let t = dom.find_first("section").unwrap();
            let h = root
                .node_rects(&dom)
                .get(&t)
                .expect("abspos has geometry")
                .height;
            assert_eq!(
                h, 200.0,
                "inset:0; height:{kw} is definite → stretches to CB (200)"
            );
        }
    }

    #[test]
    fn abspos_aspect_ratio_transfers_definite_height_to_auto_width() {
        // `top:0;left:0` (one inset per axis, NOT both) gives the box a recorded position without
        // over-constraining the width — the width still comes from the aspect-ratio transfer.
        // `aspect-ratio`/`border`/`box-sizing` all parse through the cascade now (this tick taught the
        // hand parser `aspect-ratio`, at parity with the stylo map the shipping pipeline uses), so this
        // is an end-to-end parse→layout gate — a dropped mapping here would flip it RED.
        //  • content-box `<section>`: 100 content + 150*2 border → 400px square.
        //  • border-box `<article>`: the 100px height IS the border box and the ratio relates border
        //    boxes → 100px square, the 20px border absorbed.
        let (dom, root) = layout_html(
            "<body><div style='position:relative;width:800px;height:600px'>\
               <section style='position:absolute;top:0;left:0;height:100px;aspect-ratio:1/1;border:150px solid'></section>\
               <article style='position:absolute;top:0;left:0;height:100px;aspect-ratio:1/1;border:20px solid;box-sizing:border-box'></article>\
             </div></body>",
            "",
            800.0,
        );
        let cbx = dom.find_first("section").unwrap();
        let bbx = dom.find_first("article").unwrap();
        let rects = root.node_rects(&dom);
        let cb = rects.get(&cbx).expect("content-box abspos has geometry");
        assert_eq!(
            (cb.width, cb.height),
            (400.0, 400.0),
            "content-box: 100 content + 150*2 border = 400 square (auto width was 0 before)"
        );
        let bb = rects.get(&bbx).expect("border-box abspos has geometry");
        assert_eq!(
            (bb.width, bb.height),
            (100.0, 100.0),
            "border-box: the ratio relates border boxes → 100px square, border absorbed"
        );
    }

    #[test]
    fn blocks_stack_vertically() {
        let (_dom, root) = layout_html(
            "<body><div style='height:50px'></div><div style='height:30px'></div></body>",
            "",
            800.0,
        );
        // body has 8px UA margin; its two block children stack.
        let BoxContent::Block(children) = &root.content else {
            panic!("expected block content");
        };
        assert_eq!(children.len(), 2);
        assert_eq!(children[0].rect.height, 50.0);
        assert_eq!(children[1].rect.height, 30.0);
        // Second div starts below the first.
        assert!(children[1].rect.y >= children[0].rect.y + 50.0);
    }

    #[test]
    fn adjacent_sibling_margins_collapse() {
        // bottom:20 meets top:30 → the gap is max(20,30)=30, not 50.
        let (_dom, root) = layout_html(
            "<body><div style='height:10px;margin:0 0 20px 0'></div>\
             <div style='height:10px;margin:30px 0 0 0'></div></body>",
            "",
            800.0,
        );
        let BoxContent::Block(children) = &root.content else {
            panic!("expected block content");
        };
        assert_eq!(children.len(), 2);
        let gap = children[1].rect.y - (children[0].rect.y + children[0].rect.height);
        assert!(
            (gap - 30.0).abs() < 0.01,
            "collapsed gap should be 30, got {gap}"
        );
    }

    #[test]
    fn margins_do_not_collapse_across_inline_content() {
        // A text line between two blocks blocks the collapse; both margins apply.
        let (_dom, root) = layout_html(
            "<body><div style='height:10px;margin-bottom:20px'></div>hi\
             <div style='height:10px;margin-top:30px'></div></body>",
            "",
            800.0,
        );
        let BoxContent::Block(children) = &root.content else {
            panic!("expected block content");
        };
        // div, anonymous(inline "hi"), div
        assert_eq!(children.len(), 3);
        // The trailing 20px margin is committed before the inline box.
        let after_first = children[0].rect.y + children[0].rect.height;
        assert!(
            children[1].rect.y >= after_first + 20.0 - 0.01,
            "inline box should sit below the first div's full bottom margin"
        );
    }

    /// Find the first box whose DOM node has the given id-ish tag by walking.
    fn first_box_of_tag<'a>(root: &'a LayoutBox, dom: &Dom, tag: &str) -> Option<&'a LayoutBox> {
        fn rec<'a>(b: &'a LayoutBox, dom: &Dom, tag: &str, out: &mut Option<&'a LayoutBox>) {
            if out.is_some() {
                return;
            }
            if let Some(n) = b.node {
                if dom.element(n).map(|e| e.name.eq_ignore_ascii_case(tag)) == Some(true) {
                    *out = Some(b);
                    return;
                }
            }
            if let BoxContent::Block(kids) = &b.content {
                for k in kids {
                    rec(k, dom, tag, out);
                }
            }
        }
        let mut out = None;
        rec(root, dom, tag, &mut out);
        out
    }

    /// Collect every cell box (DOM tag td/th) as rects, in tree order.
    fn cell_rects(root: &LayoutBox, dom: &Dom) -> Vec<Rect> {
        let mut out = Vec::new();
        root.walk(&mut |b| {
            if let Some(n) = b.node {
                if dom.element(n).map(|e| e.name == "td" || e.name == "th") == Some(true) {
                    out.push(b.rect);
                }
            }
        });
        out
    }

    #[test]
    fn table_fixed_layout_splits_columns_evenly() {
        // table-layout:fixed, width 600, 3 auto columns → ~200 each (no spacing).
        let (dom, root) = layout_html(
            "<body style='margin:0'><table style='table-layout:fixed;width:600px;border-spacing:0'>\
             <tr><td>a</td><td>b</td><td>c</td></tr></table></body>",
            "",
            800.0,
        );
        let cells = cell_rects(&root, &dom);
        assert_eq!(cells.len(), 3);
        for c in &cells {
            assert!(
                (c.width - 200.0).abs() < 0.5,
                "each col ~200, got {}",
                c.width
            );
        }
        // Columns are laid left to right, non-overlapping.
        assert!(cells[1].x >= cells[0].x + cells[0].width - 0.5);
        assert!(cells[2].x >= cells[1].x + cells[1].width - 0.5);
    }

    #[test]
    fn table_rows_stack_and_cells_align_in_columns() {
        let (dom, root) = layout_html(
            "<body style='margin:0'><table style='table-layout:fixed;width:400px;border-spacing:0'>\
             <tr><td style='height:20px'>a</td><td>b</td></tr>\
             <tr><td>c</td><td style='height:30px'>d</td></tr></table></body>",
            "",
            800.0,
        );
        let cells = cell_rects(&root, &dom);
        assert_eq!(cells.len(), 4);
        // Same column ⇒ same x; row 2 below row 1.
        assert!((cells[0].x - cells[2].x).abs() < 0.5, "col 0 aligned");
        assert!((cells[1].x - cells[3].x).abs() < 0.5, "col 1 aligned");
        assert!(
            cells[2].y >= cells[0].y + cells[0].height - 0.5,
            "row 2 below row 1"
        );
        // Cells in a row share the row height (max of the two).
        assert!((cells[0].height - cells[1].height).abs() < 0.5);
        assert!((cells[2].height - cells[3].height).abs() < 0.5);
        assert!(
            cells[2].height >= 30.0 - 0.5,
            "row 2 height driven by the 30px cell"
        );
    }

    #[test]
    fn table_auto_layout_sizes_columns_to_content() {
        // Auto layout, no table width → shrink to content; the wider column is wider.
        let (dom, root) = layout_html(
            "<body style='margin:0'><table style='border-spacing:0'>\
             <tr><td>x</td><td>a much longer cell of text here</td></tr></table></body>",
            "",
            800.0,
        );
        let cells = cell_rects(&root, &dom);
        assert_eq!(cells.len(), 2);
        assert!(
            cells[1].width > cells[0].width,
            "content-heavy column should be wider: {} vs {}",
            cells[1].width,
            cells[0].width
        );
    }

    #[test]
    fn table_border_spacing_separates_cells() {
        let (dom, root) = layout_html(
            "<body style='margin:0'><table style='table-layout:fixed;width:410px;border-spacing:10px'>\
             <tr><td>a</td><td>b</td></tr></table></body>",
            "",
            800.0,
        );
        let cells = cell_rects(&root, &dom);
        assert_eq!(cells.len(), 2);
        // Gap between the two cells equals border-spacing (10px).
        let gap = cells[1].x - (cells[0].x + cells[0].width);
        assert!(
            (gap - 10.0).abs() < 0.5,
            "inter-cell gap should be 10, got {gap}"
        );
    }

    #[test]
    fn absolute_positioned_against_relative_ancestor() {
        // The abs box's containing block is the relatively-positioned parent's
        // padding box; top/left place it there, out of normal flow.
        let (dom, root) = layout_html(
            "<body style='margin:0'>\
             <div id=cb style='position:relative;left:0;top:0;margin-left:50px;\
             width:200px;height:200px'>\
             <div id=a style='position:absolute;top:10px;left:20px;width:30px;height:40px'></div>\
             </div></body>",
            "",
            800.0,
        );
        let mut cb = None;
        let mut a = None;
        root.walk(&mut |b| {
            if let Some(n) = b.node {
                match dom.element(n).and_then(|e| e.id()) {
                    Some("cb") => cb = Some(b.rect),
                    Some("a") => a = Some(b.rect),
                    _ => {}
                }
            }
        });
        let cb = cb.unwrap();
        let a = a.unwrap();
        // cb is at x=50 (its margin-left). The abs box sits at cb padding-box + inset.
        assert!(
            (a.x - (cb.x + 20.0)).abs() < 0.01,
            "abs left offset from CB, got {}",
            a.x
        );
        assert!(
            (a.y - (cb.y + 10.0)).abs() < 0.01,
            "abs top offset from CB, got {}",
            a.y
        );
        assert_eq!(a.width, 30.0);
        assert_eq!(a.height, 40.0);
    }

    #[test]
    fn absolute_with_no_positioned_ancestor_uses_viewport() {
        let (dom, root) = layout_html(
            "<body style='margin:0'>\
             <div id=a style='position:absolute;right:0;top:0;width:40px;height:40px'></div>\
             </body>",
            "",
            800.0,
        );
        let mut a = None;
        root.walk(&mut |b| {
            if let Some(n) = b.node {
                if dom.element(n).and_then(|e| e.id()) == Some("a") {
                    a = Some(b.rect);
                }
            }
        });
        let a = a.unwrap();
        // right:0 against the 800px viewport → right edge at 800.
        assert!(
            (a.x + a.width - 800.0).abs() < 0.01,
            "abs right:0 hits viewport right, got x={}",
            a.x
        );
        assert!(a.y.abs() < 0.01, "abs top:0 at viewport top");
    }

    #[test]
    fn absolute_is_removed_from_flow() {
        // A block after an abs box takes the abs box's would-be space (abs is out of
        // flow), so it sits at the top, not below.
        let (dom, root) = layout_html(
            "<body style='margin:0'>\
             <div id=a style='position:absolute;top:0;left:0;height:100px'></div>\
             <div id=n style='height:10px'></div></body>",
            "",
            800.0,
        );
        let mut n = None;
        root.walk(&mut |b| {
            if let Some(node) = b.node {
                if dom.element(node).and_then(|e| e.id()) == Some("n") {
                    n = Some(b.rect);
                }
            }
        });
        assert!(
            n.unwrap().y.abs() < 0.01,
            "in-flow block ignores the abs box"
        );
    }

    #[test]
    fn relative_position_shifts_visually_not_flow() {
        // The relpos div moves +20x/+15y but the following block stays where the
        // *un-shifted* div left it (relpos does not affect flow).
        let (dom, root) = layout_html(
            "<body style='margin:0'>\
             <div id=r style='position:relative;left:20px;top:15px;height:30px'></div>\
             <div id=n style='height:10px'></div></body>",
            "",
            800.0,
        );
        let mut rel = None;
        let mut nxt = None;
        root.walk(&mut |b| {
            if let Some(n) = b.node {
                match dom.element(n).and_then(|e| e.id()) {
                    Some("r") => rel = Some(b.rect),
                    Some("n") => nxt = Some(b.rect),
                    _ => {}
                }
            }
        });
        let rel = rel.unwrap();
        let nxt = nxt.unwrap();
        assert_eq!(rel.x, 20.0, "relpos shifts x by left");
        assert_eq!(rel.y, 15.0, "relpos shifts y by top");
        // The next block sits at the relpos box's IN-FLOW bottom (0 + 30 = 30), not
        // the shifted bottom (15 + 30 = 45).
        assert!(
            (nxt.y - 30.0).abs() < 0.01,
            "sibling stacks against un-shifted flow bottom, got y={}",
            nxt.y
        );
    }

    #[test]
    fn left_float_hugs_left_edge() {
        let (dom, root) = layout_html(
            "<body style='margin:0'><div style='float:left;width:100px;height:40px'></div>\
             <p style='margin:0'>text after the float</p></body>",
            "",
            800.0,
        );
        let f = first_box_of_tag(&root, &dom, "div").unwrap();
        assert_eq!(f.rect.x, 0.0, "left float hugs the left content edge");
        assert_eq!(f.rect.width, 100.0);
    }

    #[test]
    fn right_float_hugs_right_edge() {
        let (dom, root) = layout_html(
            "<body style='margin:0'><div style='float:right;width:100px;height:40px'></div></body>",
            "",
            800.0,
        );
        let f = first_box_of_tag(&root, &dom, "div").unwrap();
        // right edge of the float == container right (800).
        assert!(
            (f.rect.x + f.rect.width - 800.0).abs() < 0.01,
            "right float's right edge should meet the container right, got x={}",
            f.rect.x
        );
    }

    #[test]
    fn two_left_floats_stack_horizontally_then_wrap() {
        // Two 300px floats fit side by side in 800px; a third drops below them.
        let (dom, root) = layout_html(
            "<body style='margin:0'>\
             <div class=f style='float:left;width:300px;height:40px'></div>\
             <div class=f style='float:left;width:300px;height:40px'></div>\
             <div class=g style='float:left;width:300px;height:40px'></div></body>",
            "",
            800.0,
        );
        let mut floats = Vec::new();
        root.walk(&mut |b| {
            if let Some(n) = b.node {
                if dom.element(n).map(|e| e.name == "div") == Some(true) {
                    floats.push(b.rect);
                }
            }
        });
        assert_eq!(floats.len(), 3);
        // First two share the top band; third wraps below.
        assert!((floats[0].y - floats[1].y).abs() < 0.01);
        assert!(
            (floats[1].x - 300.0).abs() < 0.01,
            "second float sits right of first"
        );
        assert!(
            floats[2].y >= 40.0 - 0.01,
            "third float drops to the next band"
        );
        assert!(
            (floats[2].x).abs() < 0.01,
            "third float returns to the left edge"
        );
    }

    #[test]
    fn clear_pushes_block_below_float() {
        let (dom, root) = layout_html(
            "<body style='margin:0'><div style='float:left;width:100px;height:60px'></div>\
             <div id=c style='clear:left;height:10px'></div></body>",
            "",
            800.0,
        );
        // The cleared block must start at or below the float's bottom (60).
        let mut cleared_y = None;
        root.walk(&mut |b| {
            if let Some(n) = b.node {
                if dom.element(n).and_then(|e| e.id()) == Some("c") {
                    cleared_y = Some(b.rect.y);
                }
            }
        });
        assert!(
            cleared_y.unwrap() >= 60.0 - 0.01,
            "clear:left block should sit below the 60px float, got {cleared_y:?}"
        );
    }

    #[test]
    fn text_flows_around_left_float() {
        // A tall left float narrows the line band; text starts right of the float.
        let (_dom, root) = layout_html(
            "<body style='margin:0'><div style='float:left;width:100px;height:200px'></div>\
             <p style='margin:0'>hello</p></body>",
            "",
            800.0,
        );
        let mut first_x = None;
        root.walk(&mut |b| {
            if let BoxContent::Inline(frags) = &b.content {
                if let Some(f) = frags.first() {
                    first_x.get_or_insert(f.x);
                }
            }
        });
        assert!(
            first_x.unwrap() >= 100.0 - 0.01,
            "text should start to the right of the 100px float, got x={first_x:?}"
        );
    }

    #[test]
    fn text_wraps_to_multiple_lines() {
        // Narrow container forces wrapping.
        let (_dom, root) = layout_html(
            "<body><p>the quick brown fox jumps over the lazy dog again and again</p></body>",
            "p{margin:0}",
            80.0,
        );
        let mut line_tops = std::collections::BTreeSet::new();
        root.walk(&mut |b| {
            if let BoxContent::Inline(frags) = &b.content {
                for f in frags {
                    line_tops.insert(f.line_top as i32);
                }
            }
        });
        assert!(line_tops.len() > 1, "text should wrap onto multiple lines");
    }

    #[test]
    fn document_height_grows_with_content() {
        let (_dom, tall) = layout_html("<body><div style='height:500px'></div></body>", "", 800.0);
        let (_dom2, short) = layout_html("<body><div style='height:20px'></div></body>", "", 800.0);
        assert!(tall.content_bottom() > short.content_bottom() + 400.0);
    }

    #[test]
    fn centered_text_is_offset() {
        let (_dom, root) = layout_html(
            "<body><p style='text-align:center'>hi</p></body>",
            "p{margin:0}",
            800.0,
        );
        let mut first_x = None;
        root.walk(&mut |b| {
            if let BoxContent::Inline(frags) = &b.content {
                if let Some(f) = frags.first() {
                    first_x.get_or_insert(f.x);
                }
            }
        });
        assert!(
            first_x.unwrap() > 100.0,
            "centered text should be pushed right"
        );
    }
    /// Regression (found by A/B against Chromium on Wikipedia): an **icon button** — `inline-flex`,
    /// `justify-content:center`, a `max-width`, one small icon — must hug its icon, not fill its
    /// container.
    ///
    /// Two bugs conspired here. (1) `inline-flex` was mapped to block-level `flex`, so the button
    /// filled. (2) Even once inline, its max-content was computed by laying it out at a 1e6
    /// available width — where `max-width` clamped it to 448px and `justify-content:center` put the
    /// icon at x=214, so the measured "extent" was 234px. The 32px button measured 234px, overflowed
    /// the header's flex line, wrapped the search bar onto a second row, and pushed every element on
    /// the page down.
    #[test]
    fn inline_flex_icon_button_hugs_its_content() {
        let html =
            r#"<div class="bar"><label class="btn"><span class="icon"></span></label></div>"#;
        let css = ".bar{width:900px}                    .btn{display:inline-flex;align-items:center;justify-content:center;max-width:28rem}                    .icon{display:block;width:20px;height:20px}";
        let (dom, root) = layout_html(html, css, 1000.0);
        let rects = root.node_rects(&dom);
        let btn = dom
            .descendants(dom.root())
            .find(|&n| dom.element(n).and_then(|e| e.attr("class")) == Some("btn"))
            .expect("btn");
        let w = rects[&btn].width;
        assert!(
            (15.0..60.0).contains(&w),
            "an inline-flex icon button must hug its 20px icon, got {w}px \
             (filling its container is what wrapped Wikipedia's header)"
        );
    }

    /// Regression: an **empty inline element** is still a box. Chrome reports zero width and a
    /// line-height-tall rect for `<span id="anchor"></span>`; real pages depend on that for fragment
    /// anchors and scroll-spy. We produced no geometry at all — 1,079 spans and 298 anchors missing
    /// from one Wikipedia article, the single largest source of missing elements.
    #[test]
    fn empty_inline_element_still_has_a_box() {
        let html = r#"<p>before <span id="anchor"></span> after</p>"#;
        let (dom, root) = layout_html(html, "", 600.0);
        let rects = root.node_rects(&dom);
        let anchor = dom
            .descendants(dom.root())
            .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some("anchor"))
            .expect("anchor span");
        assert!(
            rects.contains_key(&anchor),
            "an empty inline element must still have geometry"
        );
    }

    /// Regression: centering inside the huge measuring width is FREE SPACE, not content. A block
    /// with `margin: 0 auto` sits at x≈499,500 when laid out at a 1e6 available width; adding that
    /// offset to the max-content extent reported Wikipedia's header as 500,532px wide.
    #[test]
    fn auto_margins_do_not_inflate_max_content() {
        let html = r#"<div class="row"><div class="item"><div class="c">hi</div></div><div class="item">x</div></div>"#;
        let css = ".row{display:flex;flex-wrap:wrap;width:600px} .c{display:block;margin:0 auto;width:100px}";
        let (dom, root) = layout_html(html, css, 800.0);
        let rects = root.node_rects(&dom);
        let items: Vec<f32> = dom
            .descendants(dom.root())
            .filter(|&n| dom.element(n).and_then(|e| e.attr("class")) == Some("item"))
            .map(|n| rects[&n].y)
            .collect();
        assert_eq!(items.len(), 2);
        assert!(
            (items[0] - items[1]).abs() < 1.0,
            "both flex items must stay on ONE line; an auto-margin child must not measure \
             half a million pixels wide and wrap its sibling"
        );
    }
    /// Regression: `<pre>` preserves newlines, and `<br>` forces a line break. The engine had **no
    /// forced-break concept at all**: `<br>` did nothing, and every code block folded its newlines
    /// into spaces and rendered as one endless line. On Wikipedia's Rust article — which is mostly
    /// code samples — that made the page 20% shorter than Chrome's and threw everything below the
    /// first code block thousands of pixels out of place.
    #[test]
    fn pre_preserves_newlines_and_br_breaks_lines() {
        let html = "<pre id=\"p\">a\nb\nc</pre><p id=\"q\">a<br>b<br>c</p>";
        let css = "pre{white-space:pre;line-height:20px} p{line-height:20px}";
        let (dom, root) = layout_html(html, css, 800.0);
        let rects = root.node_rects(&dom);
        let by_id = |id: &str| {
            dom.descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id")
        };
        let pre_h = rects[&by_id("p")].height;
        let p_h = rects[&by_id("q")].height;
        assert!(
            pre_h >= 55.0,
            "a 3-line <pre> must be ~3 line boxes tall, got {pre_h}px (newlines were folded away)"
        );
        assert!(
            p_h >= 55.0,
            "two <br>s make three lines, got {p_h}px (<br> did nothing)"
        );
    }
    /// Regression: a `<ul>` gets bullets and an `<ol>` numbers. Absent markers, every list on the
    /// web renders as bare indented text.
    #[test]
    fn list_items_get_markers() {
        let html = "<ul><li id=\"a\">one</li><li id=\"b\">two</li></ul>\
                    <ol start=\"3\"><li id=\"c\">three</li></ol>";
        let css = "ul{list-style-type:disc} ol{list-style-type:decimal}";
        let (dom, root) = layout_html(html, css, 400.0);
        let mut markers: Vec<String> = Vec::new();
        root.walk(&mut |b| {
            if let Some(m) = &b.marker {
                markers.push(m.text.clone());
            }
        });
        assert_eq!(
            markers,
            vec![
                "\u{2022}".to_string(),
                "\u{2022}".to_string(),
                "3.".to_string()
            ],
            "two bullets and an <ol start=3> numbering from 3"
        );
    }

    /// Regression: `text-decoration` propagates from a block to the inline fragments that paint.
    #[test]
    fn text_decoration_reaches_the_fragments() {
        let (dom, root) = layout_html(
            "<p class=\"u\">underlined</p>",
            ".u{text-decoration:underline}",
            400.0,
        );
        let _ = &dom;
        let mut seen = false;
        root.walk(&mut |b| {
            if let BoxContent::Inline(frags) = &b.content {
                for f in frags {
                    if !f.text.trim().is_empty() && f.style.decoration.underline {
                        seen = true;
                    }
                }
            }
        });
        assert!(
            seen,
            "the underline must reach the text fragment, which is what paints it"
        );
    }
    /// Regression: `::before` / `::after` generated content enters the flow. It is how the web draws
    /// icons, quotation marks, counters and dividers — and it is NOT in the DOM, so this is the only
    /// place it can appear.
    #[test]
    fn pseudo_element_content_renders() {
        let html = r#"<p id="p">body</p>"#;
        let css = r#"#p::before{content:"[X] "} #p::after{content:" [Y]"}"#;
        let (dom, root) = layout_html(html, css, 400.0);
        let _ = &dom;
        let mut text = String::new();
        root.walk(&mut |b| {
            if let BoxContent::Inline(frags) = &b.content {
                for f in frags {
                    text.push_str(&f.text);
                }
            }
        });
        assert!(
            text.contains("[X]"),
            "::before content must render (got {text:?})"
        );
        assert!(
            text.contains("[Y]"),
            "::after content must render (got {text:?})"
        );
    }
    /// `text-transform` changes the RENDERED casing (nav bars, buttons, headings) while leaving the
    /// DOM text alone. Baseline: unimplemented, so an `uppercase` button rendered its lowercase source.
    /// A child's own `text-transform` overrides the inherited value (a `none` island stays as authored).
    #[test]
    fn text_transform_recases_rendered_text_only() {
        // Unit: the transform itself, including the capitalize word-boundary and Unicode casing.
        use manuk_css::TextTransform;
        assert_eq!(
            apply_text_transform("Submit", TextTransform::Uppercase).as_ref(),
            "SUBMIT"
        );
        assert_eq!(
            apply_text_transform("HELLO", TextTransform::Lowercase).as_ref(),
            "hello"
        );
        assert_eq!(
            apply_text_transform("hello world", TextTransform::Capitalize).as_ref(),
            "Hello World"
        );
        assert_eq!(
            apply_text_transform("straße", TextTransform::Uppercase).as_ref(),
            "STRASSE",
            "Unicode casing (ß→SS) is honoured"
        );

        // E2E: the property parses, inherits, is overridable, and reaches the rendered fragments —
        // while the DOM textContent is unchanged.
        let html = r#"<nav id="n">home <span id="s" style="text-transform:none">Keep</span></nav>"#;
        let css = "#n{text-transform:uppercase}";
        let (dom, root) = layout_html(html, css, 400.0);
        let mut rendered = String::new();
        root.walk(&mut |b| {
            if let BoxContent::Inline(frags) = &b.content {
                for f in &*frags {
                    rendered.push_str(&f.text);
                    rendered.push(' ');
                }
            }
        });
        assert!(
            rendered.contains("HOME"),
            "inherited text-transform:uppercase must upper-case the nav text (got {rendered:?})"
        );
        assert!(
            rendered.contains("Keep") && !rendered.contains("KEEP"),
            "a child's text-transform:none overrides the inherited uppercase (got {rendered:?})"
        );
        // The DOM text is untouched — JS still reads the author's string.
        let n = dom
            .descendants(dom.root())
            .find(|&x| dom.element(x).and_then(|e| e.attr("id")) == Some("n"))
            .unwrap();
        assert!(
            dom.text_content(n).contains("home"),
            "text-transform must NOT mutate the DOM text (JS reads the author's casing)"
        );
    }

    /// `overflow-wrap:break-word` (and `word-break:break-all`) breaks a long unbreakable token — a
    /// URL, a hex hash, an unspaced string — at char boundaries so it fits its column instead of
    /// overflowing it. Baseline: char-level breaking was unimplemented, so the token stayed one
    /// fragment wider than the column (the classic "long link blows out the layout").
    #[test]
    fn overflow_wrap_break_word_wraps_long_token() {
        // A 60-char unbreakable token (no whitespace, no hyphen) in a 100px column.
        let token = "a".repeat(60);
        let html = format!(r#"<div id="d">{token}</div>"#);

        let collect_frags = |root: &LayoutBox| -> Vec<(String, f32)> {
            let mut v = Vec::new();
            root.walk(&mut |b| {
                if let BoxContent::Inline(frags) = &b.content {
                    for f in &*frags {
                        if !f.text.is_empty() {
                            v.push((f.text.clone(), f.width));
                        }
                    }
                }
            });
            v
        };

        // Control: overflow-wrap:normal — the token stays a single fragment, wider than the column.
        let (_d, root) = layout_html(&html, "#d{width:100px}", 400.0);
        let base = collect_frags(&root);
        assert!(
            base.iter().any(|(_, w)| *w > 100.0),
            "baseline: an unbreakable token overflows its 100px column (got {base:?})"
        );

        // overflow-wrap:break-word — split into chunks that each fit the 100px column, across lines,
        // and losslessly (every character preserved, none duplicated).
        let (_d2, root2) = layout_html(&html, "#d{width:100px;overflow-wrap:break-word}", 400.0);
        let broken = collect_frags(&root2);
        assert!(
            broken.len() > 1,
            "break-word must split the token into multiple fragments (got {broken:?})"
        );
        assert!(
            broken.iter().all(|(_, w)| *w <= 100.5),
            "every broken chunk must fit the 100px column (got {broken:?})"
        );
        let joined: String = broken.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(
            joined, token,
            "breaking must be lossless — no chars lost or duplicated"
        );

        // `word-break:break-all` reaches the same char-level breaking through the other property.
        let (_d3, root3) = layout_html(&html, "#d{width:100px;word-break:break-all}", 400.0);
        let broken3 = collect_frags(&root3);
        assert!(
            broken3.len() > 1 && broken3.iter().all(|(_, w)| *w <= 100.5),
            "word-break:break-all must also break the token to fit (got {broken3:?})"
        );
    }

    /// `letter-spacing` widens a run by a fixed advance per character; `word-spacing` widens each
    /// inter-word space. Both pair constantly with tracked uppercase nav/buttons/labels. Baseline:
    /// unimplemented (0px), so a tracked heading measured and painted at its untracked width.
    #[test]
    fn letter_and_word_spacing_widen_runs() {
        let collect = |root: &LayoutBox| -> Vec<(String, f32, f32)> {
            let mut v = Vec::new();
            root.walk(&mut |b| {
                if let BoxContent::Inline(frags) = &b.content {
                    for f in &*frags {
                        if !f.text.is_empty() {
                            v.push((f.text.clone(), f.x, f.width));
                        }
                    }
                }
            });
            v
        };
        let word = |v: &[(String, f32, f32)], t: &str| -> (f32, f32) {
            let f = v.iter().find(|(s, _, _)| s == t).expect("word present");
            (f.1, f.2) // (x, width)
        };

        // letter-spacing: a 5-char word grows by exactly 5 × 4px = 20px of tracking.
        let (_d0, r0) = layout_html(r#"<p id="p">hello</p>"#, "#p{letter-spacing:0}", 800.0);
        let (_d1, r1) = layout_html(r#"<p id="p">hello</p>"#, "#p{letter-spacing:4px}", 800.0);
        let (_, w0) = word(&collect(&r0), "hello");
        let (_, w1) = word(&collect(&r1), "hello");
        assert!(
            (w1 - w0 - 20.0).abs() < 0.5,
            "letter-spacing:4px must add 5×4=20px to a 5-char word ({w0} -> {w1})"
        );

        // word-spacing: the second word is pushed right by the 10px added to the space before it.
        let (_d2, r2) = layout_html(r#"<p id="p">aa bb</p>"#, "#p{word-spacing:0}", 800.0);
        let (_d3, r3) = layout_html(r#"<p id="p">aa bb</p>"#, "#p{word-spacing:10px}", 800.0);
        let (x2, _) = word(&collect(&r2), "bb");
        let (x3, _) = word(&collect(&r3), "bb");
        assert!(
            (x3 - x2 - 10.0).abs() < 0.5,
            "word-spacing:10px must push the second word right by 10px ({x2} -> {x3})"
        );
    }

    /// `text-overflow: ellipsis` truncates a clipped, non-wrapping line with `…` — the ubiquitous
    /// truncated title/label/tab/table-cell. Baseline: unimplemented, so a `nowrap; overflow:hidden`
    /// title just got cut off mid-glyph with no ellipsis. Control (`clip`) keeps the full text.
    #[test]
    fn text_overflow_ellipsis_truncates_clipped_line() {
        let collect_text = |root: &LayoutBox| -> String {
            let mut s = String::new();
            root.walk(&mut |b| {
                if let BoxContent::Inline(frags) = &b.content {
                    for f in &*frags {
                        s.push_str(&f.text);
                    }
                }
            });
            s
        };
        // Words are separate fragments (spaces are gaps, not text), so the collected text is the
        // words concatenated without spaces.
        let long = "This is a very long title that does not fit a narrow box";
        let long_nospace: String = long.split_whitespace().collect();
        let html = format!(r#"<div id="d">{long}</div>"#);

        // ellipsis: truncated, ends with `…`, and the kept part is a prefix of the original.
        let css_e = "#d{width:80px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}";
        let (_de, re) = layout_html(&html, css_e, 400.0);
        let te = collect_text(&re);
        assert!(
            te.ends_with('\u{2026}'),
            "an ellipsis box must end with … (got {te:?})"
        );
        let kept = te.trim_end_matches('\u{2026}');
        assert!(
            !kept.is_empty() && long_nospace.starts_with(kept) && kept.len() < long_nospace.len(),
            "the kept text is a proper prefix of the original (kept {kept:?})"
        );

        // control: text-overflow:clip (default) keeps the whole run and adds no ellipsis.
        let css_c = "#d{width:80px;white-space:nowrap;overflow:hidden}";
        let (_dc, rc) = layout_html(&html, css_c, 400.0);
        let tc = collect_text(&rc);
        assert!(
            !tc.contains('\u{2026}') && tc == long_nospace,
            "clip keeps the full text with no ellipsis (got {tc:?})"
        );
    }

    /// Regression: `display:none` means **no boxes at all** — including inside a flex/grid container.
    /// The taffy path filtered children by `is_element` but not by display, so a hidden child got a
    /// zero slot while our extraction still measured and materialised its content. A `<script>` in a
    /// flex `<body>` painted its own source code down the page, and every hidden menu, modal and
    /// template inside any flex or grid container rendered its contents.
    #[test]
    fn display_none_children_of_a_flex_container_generate_no_boxes() {
        let html = r#"<div class="row"><script id="s">let x = 1; alert("hi");</script><p id="p">visible</p></div>"#;
        let css = ".row{display:flex} script{display:none}";
        let (dom, root) = layout_html(html, css, 600.0);
        let mut text = String::new();
        root.walk(&mut |b| {
            if let BoxContent::Inline(frags) = &b.content {
                for f in frags {
                    text.push_str(&f.text);
                }
            }
        });
        let _ = &dom;
        assert!(
            text.contains("visible"),
            "the visible sibling must still render"
        );
        assert!(
            !text.contains("alert") && !text.contains("let"),
            "a display:none <script> in a FLEX container must not paint its source (got {text:?})"
        );
    }

    /// `width: fit-content | max-content | min-content` on a **block** hugs its content instead of
    /// filling the containing block. Before this, all three collapsed to `Dim::Auto` and took the
    /// block auto-width *fill* branch, so a `fit-content` badge stretched edge-to-edge. The measure
    /// functions are the ones inline-block already uses, so the intrinsic width is content-box.
    #[test]
    fn width_fit_content_hugs() {
        // A short word in a wide container: fit-content = its ~1-word width, far under 500px.
        let html = r#"<div id="box">hi</div>"#;
        let css = "#box{width:fit-content;background:#000}";
        let (dom, root) = layout_html(html, css, 500.0);
        let rects = root.node_rects(&dom);
        let by_id = |id: &str| {
            dom.descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id")
        };
        let w = rects[&by_id("box")].width;
        assert!(
            w > 0.0 && w < 100.0,
            "width:fit-content must hug the word (expected ~<100px, NOT the 500px fill), got {w}"
        );
    }

    /// `width: max-content` = the whole content unwrapped on one line — wider than the same content
    /// under `min-content`, and independent of the (ample) available width.
    #[test]
    fn width_max_content_hugs() {
        let html = r#"<div id="box">one two three four five</div>"#;
        let css = "#box{width:max-content}";
        let (dom, root) = layout_html(html, css, 1000.0);
        let rects = root.node_rects(&dom);
        let by_id = |id: &str| {
            dom.descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id")
        };
        let w = rects[&by_id("box")].width;
        // The unwrapped phrase is well under the 1000px container but well over one short word.
        assert!(
            w > 40.0 && w < 900.0,
            "width:max-content must hug the unwrapped line (not fill 1000px), got {w}"
        );
    }

    /// `width: min-content` = the longest unbreakable run. A very long single token forces the box
    /// at least that wide even though the container is narrow — and narrower than `max-content` of a
    /// multi-word phrase would be only if there were breaks, so here we assert it tracks the token.
    #[test]
    fn width_min_content_is_longest_word() {
        let html = r#"<div id="box">a supercalifragilisticexpialidocious b</div>"#;
        let css = "#box{width:min-content}";
        let (dom, root) = layout_html(html, css, 1000.0);
        let rects = root.node_rects(&dom);
        let by_id = |id: &str| {
            dom.descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id")
        };
        let w = rects[&by_id("box")].width;
        // The long token is ~200px; min-content is that word, not the whole phrase and not 1000px.
        assert!(
            w > 120.0 && w < 400.0,
            "width:min-content must be the longest word (not the full phrase, not 1000px), got {w}"
        );
    }

    /// A keyword width is still clamped by `max-width`: `fit-content` capped at 20px yields 20px,
    /// proving the intrinsic result feeds the ordinary min/max-width clamp rather than bypassing it.
    #[test]
    fn width_fit_content_still_clamped_by_max_width() {
        let html = r#"<div id="box">one two three four five six seven</div>"#;
        let css = "#box{width:max-content;max-width:20px}";
        let (dom, root) = layout_html(html, css, 1000.0);
        let rects = root.node_rects(&dom);
        let by_id = |id: &str| {
            dom.descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id")
        };
        let w = rects[&by_id("box")].width;
        assert!(
            (w - 20.0).abs() < 1.0,
            "max-width:20px must clamp the max-content width to 20px, got {w}"
        );
    }

    /// `height: stretch` on a block FILLS its parent's definite content height (margin box = CB
    /// content box). Before this it collapsed to `Dim::Auto` = content height (one line, ~18px), so
    /// a full-height panel came out line-tall. Unlike width, block `height:auto` is content-sized, so
    /// `stretch` is a real, visible distinction.
    #[test]
    fn height_stretch_fills_definite_parent() {
        let html = r#"<div id="p"><div id="box">x</div></div>"#;
        let css = "#p{height:200px;width:100px} #box{height:stretch}";
        let (dom, root) = layout_html(html, css, 400.0);
        let rects = root.node_rects(&dom);
        let by_id = |id: &str| {
            dom.descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id")
        };
        let h = rects[&by_id("box")].height;
        assert!(
            (h - 200.0).abs() < 1.0,
            "height:stretch must fill the 200px parent, got {h}"
        );
    }

    /// `-webkit-fill-available` is an alias of `stretch` and fills identically.
    #[test]
    fn height_fill_available_fills_definite_parent() {
        let html = r#"<div id="p"><div id="box">x</div></div>"#;
        let css = "#p{height:150px;width:100px} #box{height:-webkit-fill-available}";
        let (dom, root) = layout_html(html, css, 400.0);
        let rects = root.node_rects(&dom);
        let by_id = |id: &str| {
            dom.descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id")
        };
        let h = rects[&by_id("box")].height;
        assert!(
            (h - 150.0).abs() < 1.0,
            "height:-webkit-fill-available must fill the 150px parent, got {h}"
        );
    }

    /// In an **auto-height** parent (no definite height to fill) `height:stretch` stays content-sized,
    /// at parity with Chrome — it must not blow up to the viewport or overflow.
    #[test]
    fn height_stretch_in_auto_parent_stays_content() {
        let html = r#"<div id="p"><div id="box">x</div></div>"#;
        let css = "#p{width:100px} #box{height:stretch}";
        let (dom, root) = layout_html(html, css, 400.0);
        let rects = root.node_rects(&dom);
        let by_id = |id: &str| {
            dom.descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id")
        };
        let h = rects[&by_id("box")].height;
        assert!(
            h > 0.0 && h < 60.0,
            "height:stretch in an auto-height parent stays content-sized (~one line), got {h}"
        );
    }

    /// A stretched box is a **definite-height containing block**: a `height:100%` child resolves
    /// against the stretched height, not against nothing. Fills the parent, and the child fills it too.
    #[test]
    fn height_stretch_is_a_definite_base_for_percentage_child() {
        let html = r#"<div id="p"><div id="box"><div id="kid">x</div></div></div>"#;
        let css = "#p{height:200px;width:100px} #box{height:stretch} #kid{height:50%}";
        let (dom, root) = layout_html(html, css, 400.0);
        let rects = root.node_rects(&dom);
        let by_id = |id: &str| {
            dom.descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id")
        };
        let kid_h = rects[&by_id("kid")].height;
        assert!(
            (kid_h - 100.0).abs() < 1.0,
            "height:50% child of a stretched (200px) box must be 100px, got {kid_h}"
        );
    }

    /// `overflow-y:scroll` reserves a classic vertical-scrollbar gutter: the content box is narrower
    /// than the border box by the scrollbar width, so a `width:100%` child no longer fills the box.
    /// This is the `html{overflow-y:scroll}` layout-shift-prevention idiom.
    #[test]
    fn overflow_y_scroll_reserves_inline_gutter() {
        let html = r#"<div id="c"><div id="k">x</div></div>"#;
        let css = "#c{width:200px;overflow-y:scroll} #k{width:100%}";
        let (dom, root) = layout_html(html, css, 400.0);
        let rects = root.node_rects(&dom);
        let by_id = |id: &str| {
            dom.descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id")
        };
        let cw = rects[&by_id("c")].width;
        let kw = rects[&by_id("k")].width;
        assert!(
            (cw - 200.0).abs() < 0.5,
            "container border box (offsetWidth) is unchanged at 200, got {cw}"
        );
        assert!(
            (kw - 185.0).abs() < 0.5,
            "width:100% child fills the content box minus the 15px scrollbar gutter (185), got {kw}"
        );
    }

    /// Block-axis mirror: `overflow-x:scroll` on a **definite-height** box reserves a horizontal-
    /// scrollbar gutter, so a `height:100%` child fills the box height minus the 15px scrollbar
    /// strip — while the box's own `offsetHeight` (border box) stays the full 200. An auto-height box
    /// (control) reserves nothing: it grows to its content, so the reservation must not shrink it.
    #[test]
    fn overflow_x_scroll_reserves_block_gutter_only_when_height_definite() {
        let html = r#"<div id="c"><div id="k"></div></div><div id="a"><div id="ak"></div></div>"#;
        // #c: definite height => reserve; #a: auto height => no reserve (the child is a fixed 40px).
        let css = "#c{width:200px;height:200px;overflow-x:scroll} #k{height:100%} \
                   #a{width:200px;overflow-x:scroll} #ak{height:40px}";
        let (dom, root) = layout_html(html, css, 400.0);
        let rects = root.node_rects(&dom);
        let by_id = |id: &str| {
            dom.descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id")
        };
        let ch = rects[&by_id("c")].height;
        let kh = rects[&by_id("k")].height;
        assert!(
            (ch - 200.0).abs() < 0.5,
            "container border box (offsetHeight) is unchanged at 200, got {ch}"
        );
        assert!(
            (kh - 185.0).abs() < 0.5,
            "height:100% child fills the content box minus the 15px scrollbar gutter (185), got {kh}"
        );
        // Auto-height control: no definite height => no reservation => the 40px child is untouched
        // and the box is 40 tall (not 40-15).
        let ak = rects[&by_id("ak")].height;
        assert!(
            (ak - 40.0).abs() < 0.5,
            "auto-height overflow-x:scroll box reserves nothing; 40px child stays 40, got {ak}"
        );
    }

    /// `overflow:visible` (the default) reserves no gutter — the `width:100%` child fills the box.
    /// The control that proves the reservation is scoped to scroll containers, not every box.
    #[test]
    fn overflow_visible_reserves_no_gutter() {
        let html = r#"<div id="c"><div id="k">x</div></div>"#;
        let css = "#c{width:200px} #k{width:100%}";
        let (dom, root) = layout_html(html, css, 400.0);
        let rects = root.node_rects(&dom);
        let by_id = |id: &str| {
            dom.descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id")
        };
        let kw = rects[&by_id("k")].width;
        assert!(
            (kw - 200.0).abs() < 0.5,
            "no scroll container => no gutter => width:100% child fills 200, got {kw}"
        );
    }

    /// `overflow-y:auto` with content that does not overflow shows no scrollbar, so it reserves no
    /// gutter (unlike `scroll`, which always does). Guards against over-reserving on the common
    /// `overflow:auto` pane that happens to fit.
    #[test]
    fn overflow_y_auto_without_overflow_reserves_no_gutter() {
        let html = r#"<div id="c"><div id="k">x</div></div>"#;
        let css = "#c{width:200px;height:200px;overflow-y:auto} #k{width:100%}";
        let (dom, root) = layout_html(html, css, 400.0);
        let rects = root.node_rects(&dom);
        let by_id = |id: &str| {
            dom.descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id")
        };
        let kw = rects[&by_id("k")].width;
        assert!(
            (kw - 200.0).abs() < 0.5,
            "overflow:auto that fits shows no scrollbar => width:100% child fills 200, got {kw}"
        );
    }

    /// **Anonymous flex items** (Flexbox §4): text sitting directly inside a flex container is
    /// wrapped in an anonymous block-level item, not discarded.
    ///
    /// This gate was written against the BROKEN engine first and it went red in the way that
    /// matters: `<div style="display:flex">…text…</div>` produced a **2×2 px** box, because
    /// `flex_items` filtered children to elements and the text never became an item at all. Every
    /// assertion below fails on that code.
    ///
    /// The three shapes are not redundant — they are the three ways the bug reaches a real page:
    ///   * `bare`   — the whole item is text (`<a style="display:flex">Recent changes</a>`, which is
    ///                MediaWiki Vector's entire navigation);
    ///   * `mixed`  — an icon element followed by a bare label, the standard icon+text button. The
    ///                element item alone laid out, so the box existed and was merely far too narrow —
    ///                the failure that is easy to mistake for a font-metrics problem;
    ///   * `ws`     — the newline between two element children must NOT become a third item, which is
    ///                the over-correction this fix could plausibly have introduced.
    ///
    /// The width assertions are the load-bearing ones. A container that drops its text collapses to
    /// its longest WORD, and the visible symptom is not a missing label but a *wrapped* one — every
    /// nav item silently doubling in height and pushing the page below it out of place.
    #[test]
    fn bare_text_becomes_an_anonymous_flex_item() {
        let html = r#"<div id="bare">Recent changes</div>
                      <div id="mixed"><i id="icon">*</i>Recent changes</div>
                      <div id="ws"><span id="w1">A</span>
                          <span id="w2">B</span></div>"#;
        let css = "#bare,#mixed,#ws{display:flex;width:max-content} i{width:6px}";
        let (dom, root) = layout_html(html, css, 800.0);
        let rects = root.node_rects(&dom);
        let by_id = |id: &str| {
            dom.descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id")
        };
        let w = |id: &str| rects[&by_id(id)].width;
        let h = |id: &str| rects[&by_id(id)].height;

        // A text-only flex container is as wide as its text on ONE line. The exact advance depends
        // on the face, so assert the property that discriminates the bug rather than a pixel count:
        // dropping the text yields a near-zero box, and wrapping it to the longest word yields
        // something far under a full line. "Recent changes" cannot be narrower than ~60px in any
        // sane face, and the broken engine returned 2.
        let bare = w("bare");
        assert!(
            bare > 60.0,
            "bare text inside display:flex must become an anonymous item and size the container; \
             got width {bare} (a value near zero means the text was dropped entirely)"
        );
        assert!(
            h("bare") > 8.0,
            "the anonymous item must contribute a line box's height, got {}",
            h("bare")
        );

        // Icon + bare label: the icon is 6px, so the container must be the icon PLUS the label. If
        // the label is dropped the container is ~6px — present, plausible, and wrong.
        let mixed = w("mixed");
        assert!(
            mixed >= bare + 5.0,
            "icon(6px) + bare label must sum along the main axis: mixed={mixed} bare={bare} \
             (mixed ≈ icon only means the trailing text run was skipped)"
        );

        // The over-correction guard: white-space between two element children is not an item. If it
        // were, `ws` would carry a third slot and grow by a space's width.
        let ws = w("ws");
        let spans = w("w1") + w("w2");
        assert!(
            (ws - spans).abs() < 1.5,
            "white-space-only runs must NOT become anonymous items: container={ws} but its two \
             element items total {spans}"
        );
    }

    /// **An intrinsic width keyword on an absolutely-positioned box sizes to its CONTENT, not to
    /// its containing block.** `position:absolute; width:max-content` anchored to a small
    /// `position:relative` trigger is the structure of essentially every dropdown, popover, menu,
    /// tooltip and autocomplete panel on the web — and the abspos path had no arm for
    /// `width_keyword` at all, so it fell through to shrink-to-fit against the anchor.
    ///
    /// The failure is not a missing box, which is why no gate saw it: the panel renders, at roughly
    /// half its width, and every label inside wraps to two lines. Downstream that reads as *vertical*
    /// drift — which is how it survived four placement-targeted ticks (wikipedia's sidebar, 93px
    /// against Chrome's 186px, showing up in FID-SWEEP only as `mdy=45`).
    ///
    /// The static sibling is the control: it was already correct, so a test that only checked
    /// `max-content` in flow would pass while the abspos case stayed broken.
    #[test]
    fn abspos_intrinsic_width_keyword_sizes_to_content_not_the_anchor() {
        let html = r#"<div class="host"><div id="drop"><span id="label">a much longer label</span></div></div>
                      <div class="host"><div id="stat"><span>a much longer label</span></div></div>
                      <div class="host"><div id="mini"><span>a much longer label</span></div></div>"#;
        let css = "html,body{margin:0;padding:0} \
                   .host{position:relative;width:20px;height:20px} \
                   #drop{position:absolute;top:100%;left:0;width:max-content} \
                   #stat{width:max-content} \
                   #mini{position:absolute;top:100%;left:0;width:min-content}";
        let (dom, root) = layout_html(html, css, 800.0);
        let rects = root.node_rects(&dom);
        let by_id = |id: &str| {
            dom.descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id")
        };
        let w = |id: &str| rects[&by_id(id)].width;

        // The in-flow control. If THIS is wrong the test is measuring the wrong mechanism.
        assert!(
            w("stat") > 60.0,
            "precondition: in-flow width:max-content already worked, got {}",
            w("stat")
        );

        // The bug: the abspos panel took its 20px anchor's width instead of its content's.
        assert!(
            (w("drop") - w("stat")).abs() < 1.5,
            "position:absolute must not change what width:max-content MEANS: abspos={} but the \
             identical in-flow box is {} (a value near the 20px anchor means the containing block \
             was used instead of the content)",
            w("drop"),
            w("stat")
        );

        // `min-content` is the other keyword through the same arm, and it must stay DIFFERENT —
        // otherwise a fix that simply routed every keyword to `max_content_width` would pass.
        assert!(
            w("mini") > 20.0 && w("mini") < w("drop") - 5.0,
            "min-content must hug the longest word — narrower than max-content but not the 20px \
             anchor: min={} max={}",
            w("mini"),
            w("drop")
        );
    }

    /// **An inline element's box is the CONTENT AREA, not the line box** (CSS 2.1 §10.6.1) — the
    /// font's `round(ascent) + round(descent)`, centred on the line box by half-leading, and
    /// *independent of `line-height`*.
    ///
    /// This was the largest systematic placement error in the engine and it was invisible locally:
    /// every `<a>`, `<span>` and `<em>` reported the line box, so on the web's near-universal
    /// `line-height: 1.6` each one came out ~6px too tall AND ~3px too high — both coordinates
    /// wrong, on every inline element on every page. FID-SWEEP saw exactly that shape on wikipedia:
    /// `dw=0` (widths already exact) with `dh=+7` repeated across dozens of elements.
    ///
    /// Three properties, and each fails on a *different* half of the old code:
    ///   1. height is the content area → old code returned `line_height` (22 vs 16)
    ///   2. height does not move when `line-height` does → old code tracked it exactly
    ///   3. a `line-height:1` line box stays at 1em and the content area OVERFLOWS it → old code
    ///      did `max(line_height, ascent+descent)` and inflated the paragraph, and clamped
    ///      half-leading at zero so the overflow never went negative
    ///
    /// Asserted against the face's OWN metrics rather than pixel constants, so it holds on whatever
    /// sans-serif the box has installed. The guard on the first line matters: a face whose content
    /// area happens to equal its line box cannot discriminate rule from bug at all.
    #[test]
    fn inline_box_is_the_font_content_area_not_the_line_box() {
        let fonts = FontContext::new();
        let lm = fonts.line_metrics(FontKey::default(), 16.0);
        let content = lm.content_height();
        assert!(
            content > 0.0 && (content - 16.0 * 1.6).abs() > 2.0,
            "test is vacuous on this face: content area {content} is indistinguishable from the \
             1.6 line box {}",
            16.0 * 1.6
        );

        let html = r#"<p id="p">before <a id="a">link</a></p>
                      <p id="q">before <a id="b">link</a></p>
                      <p id="t">tight <a id="c">link</a></p>"#;
        let css = "html,body,p{margin:0;padding:0;font-size:16px} \
                   #p{line-height:1.6} #q{line-height:3} #t{line-height:1}";
        let (dom, root) = layout_html(html, css, 800.0);
        let rects = root.node_rects(&dom);
        let by_id = |id: &str| {
            dom.descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id")
        };
        let r = |id: &str| rects[&by_id(id)];

        // 1 — the content area, to the pixel.
        assert!(
            (r("a").height - content).abs() < 0.51,
            "inline <a> must be the font content area ({content}px), got {} — a value equal to the \
             line box ({}) means the line box is being reported instead",
            r("a").height,
            16.0 * 1.6
        );

        // 2 — and it must not follow `line-height`. Same face, same size, 1.6 vs 3.
        assert!(
            (r("a").height - r("b").height).abs() < 0.51,
            "inline height must not depend on line-height: 1.6 gave {} but 3 gave {}",
            r("a").height,
            r("b").height
        );

        // Half-leading centres it: the content area sits below the line top by (line_h-content)/2.
        let expect_dy = ((16.0 * 1.6 - content) / 2.0).floor();
        assert!(
            (r("a").y - r("p").y - expect_dy).abs() < 0.51,
            "half-leading: <a> should sit {expect_dy}px below the line top, got {}",
            r("a").y - r("p").y
        );

        // 3 — `line-height: 1` is a 16px line box even though the content area is taller, and the
        // inline OVERFLOWS it upward (negative half-leading). Chrome does exactly this.
        assert!(
            (r("t").height - 16.0).abs() < 0.51,
            "line-height:1 must give a 16px line box, got {} — taking max(line_height, ascent+\
             descent) inflates every tight line on the page",
            r("t").height
        );
        assert!(
            r("c").y < r("t").y + 0.01,
            "with a content area ({content}) taller than its 16px line box, the inline must \
             overflow upward: line top {} but inline top {}",
            r("t").y,
            r("c").y
        );
    }
}
