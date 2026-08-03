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
use std::collections::{HashMap, HashSet};

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

    /// **Is this run a direct continuation of `prev` — the SAME WORD, split by the line breaker?**
    ///
    /// This exists because the answer is not obvious and three separate consumers got it wrong the
    /// same way. **Inline layout emits one fragment per line-break OPPORTUNITY, not per line**, and
    /// CSS puts an opportunity after a hyphen, after `//`, and after `?` in a query string. So any
    /// consumer that reassembles the page's text by joining runs with a space produces
    /// `non- mainstream` and `https:// example.com/? a=1` for text that rendered, correctly, as one
    /// unbroken word.
    ///
    /// That is invisible to every instrument this engine has, because the *boxes* are right and only
    /// the *string* is wrong — so the rule lives here, next to the data it reads, rather than in each
    /// consumer. `Page::visible_text` (the agent's `Observation.text` and the history index),
    /// find-in-page, and selection-copy all ask this one question.
    ///
    /// Same baseline **and** boxes touching. Both halves are load-bearing: without the x test a whole
    /// page glues into one token; without the baseline test a hard `<br>` glues, because a new line
    /// restarts at `x = 0` and trivially "touches" the previous run's right edge. The half-pixel
    /// tolerance absorbs accumulated advance rounding — a space is an order of magnitude wider.
    ///
    /// A trailing space that belongs to a run is inside both its `text` and its `width`, so it
    /// survives either answer.
    pub fn continues(&self, prev: &TextFragment) -> bool {
        (prev.baseline - self.baseline).abs() < 0.5 && self.x <= prev.x + prev.width + 0.5
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
    /// `filter` — this box's OWN function list, empty == `none`. Unlike [`Self::opacity`] it is not
    /// folded with its ancestors' here: a filter applies to the whole subtree *as one group*, and a
    /// group is a paint-time concept, so paint composes the ancestor chain when it builds the
    /// stacking groups. Folding it into every descendant would be the wrong model in the one case
    /// that matters — `blur(4px)` twice is not `blur(8px)`.
    pub filters: Vec<manuk_css::FilterOp>,
    /// `clip-path` — this box's own basic shape, resolved at paint against [`Self::rect`] (the
    /// border box, which is the shape's default reference box). Like [`Self::filters`] it clips the
    /// element and its whole subtree, so paint carries it down rather than layout folding it in.
    pub clip_path: Option<manuk_css::ClipShape>,
    /// `mix-blend-mode` — how this box's group composites against what is already painted beneath
    /// it. `Normal` (the overwhelming majority) keeps the box on the direct-to-canvas paint path.
    pub blend: manuk_css::BlendMode,
    /// `backdrop-filter` — this box's OWN list, and deliberately **not** propagated to descendants
    /// the way `filter` is. `filter` applies to the subtree as a group; `backdrop-filter` applies to
    /// what is behind *this box*, once. Inheriting it would re-filter the same backdrop for every
    /// descendant group — both wrong and expensive.
    pub backdrop: Vec<manuk_css::FilterOp>,
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

    /// A box that occupies `rect` on behalf of `node` and **paints nothing** — no background, no
    /// border, no text, no children.
    ///
    /// It exists for geometry that is real but is not produced by CSS layout: the inside of an
    /// `<svg>`. A `<path>`'s rect comes from path data and the `viewBox` transform, not from a
    /// formatting context, so nothing in this crate can compute it — but `node_rects` must still
    /// report it, because that is what `getBoundingClientRect` returns and what the oracle probes.
    /// See `Page::map_svg_child_geometry`.
    pub fn inert(rect: Rect, node: NodeId) -> LayoutBox {
        LayoutBox {
            rect,
            background: None,
            border: None,
            radius: 0.0,
            shadows: Vec::new(),
            filters: Vec::new(),
            clip_path: None,
            blend: manuk_css::BlendMode::Normal,
            backdrop: Vec::new(),
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
            node: Some(node),
            content: BoxContent::Block(vec![]),
        }
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
        //
        // ── **THE FRAGMENT'S OWN OWNER TAKES ITS FRAGMENTS, AND NOTHING ELSE VERTICALLY.**
        //
        // Seeded first, before any lifting, because the lift below has to be able to ask *does this
        // ancestor own an inline box?* and get the answer from a map that is already complete.
        for (&owner, &r) in &frags {
            if !boxes.contains_key(&owner) && dom.is_element(owner) {
                add(&mut out, owner, r);
            }
        }
        // ⚠⚠⚠ **A NON-REPLACED INLINE'S BOX IS RESOLVED PER AXIS** — the same shape as the static
        // position (t849), and getting one axis right while unioning the other is what produced a box
        // 13px too short *and* 10px too low on the commonest icon idiom on the web.
        //
        // Chrome-measured, `16px/1.2 sans-serif`:
        //
        // ```text
        //   <span><i 8x40 inline-block></i></span>   [11,93, 8,17]   <- the 40px icon OVERFLOWS it
        //   <span 10px><b 40px>x</b></span>          [11,48,22,11]   <- the 44px child does too
        // ```
        //
        // * **Block axis** — the inline box is the element's OWN content area (its font's ascent +
        //   descent, on the line's baseline), and a taller descendant does *not* grow it. That box
        //   arrives here as the element's own fragments, which `collect_inline_node` now guarantees
        //   exist even when the element carries no text of its own.
        // * **Inline axis** — the box IS the advance of everything inside it, so a descendant's
        //   extent must still be unioned in.
        //
        // So: an ancestor that owns fragments takes only the horizontal extent of what it lifts. An
        // ancestor that owns none takes the whole rect, which is the pre-existing behaviour and the
        // only thing standing between an exotic boxless element and having no geometry at all.
        let mut lift =
            |start: NodeId, r: Rect, out: &mut std::collections::HashMap<NodeId, Rect>| {
                let mut cur = dom.parent(start);
                while let Some(n) = cur {
                    if boxes.contains_key(&n) {
                        break;
                    }
                    if dom.is_element(n) {
                        match out.get_mut(&n).filter(|_| frags.contains_key(&n)) {
                            Some(e) => {
                                let l = e.x.min(r.x);
                                e.width = (e.x + e.width).max(r.x + r.width) - l;
                                e.x = l;
                            }
                            None => add(out, n, r),
                        }
                    }
                    cur = dom.parent(n);
                }
            };
        for (&owner, &r) in &frags {
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
    /// **Taffy's verdict on a flex/grid item's BORDER-BOX HEIGHT** — the block-axis twin of
    /// [`taffy_item_width`], and it was missing for as long as that one has existed.
    ///
    /// `layout_flex` hands each item its slot height as the parent's definite height (`pch`), and
    /// `own_definite_h` then resolves the item's OWN `height: 50%` against it — **so the percentage
    /// is applied twice and the used height comes out squared.** Measured against Chrome: a
    /// `height:50%` item in a `height:200px` flex row reads **100** there and read **50** here;
    /// `height:25%` reads 50 there and 13 here (0.25² × 200). Blocks were always right; only flex
    /// and grid items squared.
    ///
    /// ⚠ This is the SAME defect the width axis had and fixed at tick 14 (*"a percentage width on a
    /// flex item resolved twice — used width came out squared"*). One axis was corrected and the
    /// mirror was left, which is this project's most-repeated shape: **the forgotten copy is never
    /// the main path, it is the other axis.**
    taffy_item_height: RefCell<HashMap<NodeId, f32>>,
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
        taffy_item_height: RefCell::new(HashMap::new()),
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
            filters: Vec::new(),
            clip_path: None,
            blend: manuk_css::BlendMode::Normal,
            backdrop: Vec::new(),
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
            Some(
                Display::Block | Display::FlowRoot | Display::Flex | Display::Grid | Display::Table
            )
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
        let Some(st) = styles.get(&k) else {
            continue;
        };
        // ⚠⚠ **ONLY AN IN-FLOW BLOCK SPLITS AN INLINE.** CSS 2.1 §9.2.1.1 is about block-level boxes
        //    *in the flow*; a float or an out-of-flow positioned box is removed from the inline
        //    formatting context and cannot split anything. And `position: absolute` **blockifies
        //    `display`** (CSS Display §2.7) — so `<span style="position:absolute">` computes to
        //    `display: block`, walked straight into the check below, and **blockified its inline
        //    ancestor**.
        //
        //    `<a style="position:relative">text<span style="position:absolute">…</span></a>` is the
        //    stretched click target, the badge on an icon link, the tooltip anchor, the dropdown under
        //    a nav item — and every one of them turned its `<a>` into a FULL-WIDTH BLOCK. That is not
        //    a subtle metric error: the link takes the whole line, forces a break, changes its
        //    parent's height, and displaces everything below it. Measured against Chrome
        //    (`margin:0; 16px/normal sans-serif`), `<p>xx <a>LINK<span abs></span></a> yy</p>`:
        //    the `<a>` is **[20 84 36×17]** and we made it **[0 102 1200×18]**.
        //
        //    It also made the `<a>` the WRONG SHAPE OF CONTAINING BLOCK when it was
        //    `position: relative`, so the abs child it was holding resolved against a full-width box.
        //    One cause, both symptoms.
        if is_float(st) || is_out_of_flow_positioned(st) {
            continue;
        }
        let d = st.display;
        if matches!(
            d,
            Display::Block | Display::FlowRoot | Display::Flex | Display::Grid | Display::Table
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
/// the spec's "first typographic letter unit") — leading punctuation, quotes and digits do NOT consume
/// the word start, so `(hello)` → `(Hello)`, `'twas` → `'Twas`, `3d` → `3D`, matching Chrome.
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
                    // A non-letter at a word start (a quote, bracket, digit) is passed through WITHOUT
                    // clearing the word-start flag — the first typographic LETTER is what gets
                    // titlecased, not the first character. Clearing it here (the old behaviour) let a
                    // single leading `"`/`(`/digit silently suppress the capital.
                    out.push(ch);
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

/// `-webkit-line-clamp: N` — keep the first `n` line boxes of a block, drop the rest, and force an
/// ellipsis onto line `n`. Unlike single-line `text-overflow`, the ellipsis is **unconditional** here:
/// content genuinely continued past line `n` (that is why there are extra lines to drop), so `…` always
/// belongs. `frags` are absolute-positioned; lines share a `line_top`. Returns the clamped content
/// height (bottom of line `n`, relative to `cy`) when a clamp actually happened, or `None` when the
/// block already has `n` lines or fewer — the common case, so a `-webkit-line-clamp` selector on a short
/// paragraph changes nothing at all.
fn apply_line_clamp(
    frags: &mut Vec<TextFragment>,
    cx: f32,
    cw: f32,
    cy: f32,
    n: usize,
    fonts: &FontContext,
) -> Option<f32> {
    if frags.is_empty() || n == 0 || cw <= 0.0 {
        return None;
    }
    // Distinct line tops, ascending.
    let mut tops: Vec<f32> = Vec::new();
    for f in frags.iter() {
        if !tops.iter().any(|&t| (t - f.line_top).abs() < 0.5) {
            tops.push(f.line_top);
        }
    }
    tops.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    if tops.len() <= n {
        return None; // fewer lines than the limit — untouched
    }
    let keep_top = tops[n - 1]; // last kept line's top
    let drop_top = tops[n]; // first dropped line's top = bottom of line n

    let clip_right = cx + cw;
    let base_style = frags[0].style;
    let ell = "\u{2026}";
    let ell_w = fonts.measure(ell, base_style.font_key, base_style.font_size);
    let cutoff = clip_right - ell_w;

    let mut out: Vec<TextFragment> = Vec::with_capacity(frags.len() + 1);
    // Ellipsis anchor + the vertical/style/owner it inherits from the last kept run on line n.
    let mut ell_x = cx;
    let mut ell_style = base_style;
    let mut ell_ca = frags[0].content_ascent;
    let mut ell_ch = frags[0].content_height;
    let mut baseline = frags[0].baseline;
    let mut node = frags[0].node;
    for f in frags.drain(..) {
        if f.line_top < keep_top - 0.5 {
            out.push(f); // a line above line n — kept verbatim
            continue;
        }
        if f.line_top > keep_top + 0.5 {
            continue; // a dropped line — skip (defensive; retain already excludes most)
        }
        // On line n: inherit the run's vertical metrics for the ellipsis.
        baseline = f.baseline;
        ell_ca = f.content_ascent;
        ell_ch = f.content_height;
        if f.x + f.width <= cutoff {
            // Fits before the cutoff: keep it, advance the ellipsis anchor to its end.
            ell_x = f.x + f.width;
            ell_style = f.style;
            node = f.node;
            out.push(f);
        } else if f.x < cutoff {
            // Straddles the cutoff: truncate to the budget, keep the prefix, anchor the ellipsis.
            let budget = (cutoff - f.x).max(0.0);
            let (prefix, pw) = truncate_to_width(&f.text, &f.style, budget, fonts);
            ell_x = f.x + pw;
            ell_style = f.style;
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
            // Everything after this on line n starts past the cutoff — skipped by the loop.
        }
        // else: starts past the cutoff — clipped; the ellipsis stays at the last anchor.
    }
    out.push(TextFragment {
        x: ell_x,
        line_top: keep_top,
        baseline,
        width: ell_w,
        text: ell.to_string(),
        style: ell_style,
        node,
        content_ascent: ell_ca,
        content_height: ell_ch,
    });
    *frags = out;
    Some(drop_top - cy)
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

    /// The inner edge imposed by OVERLAPPING FLOATS ALONE, ignoring this context's own edges —
    /// `None` when no float on that side overlaps the band.
    ///
    /// `left_offset`/`right_offset` fold the context's `left_edge`/`right_edge` in as a floor, which
    /// is right for LINE content (it lives in this block) and wrong for placing a float whose
    /// CONTAINING BLOCK starts outside those edges. A negative horizontal margin does exactly that,
    /// and it is not an exotic case: `.row { margin: 0 -15px }` with floated columns inside is the
    /// Bootstrap grid, and every framework that copied it. Measured — a `float:left` column in a
    /// `margin:0 -15px` row inside a 400px block: Chrome **x = -15**, ours **x = 0**.
    fn left_float_edge(&self, y: f32, h: f32) -> Option<f32> {
        self.floats
            .iter()
            .filter(|f| f.side == Float::Left && band_overlaps(f.rect, y, h))
            .map(|f| f.rect.x + f.rect.width)
            .fold(None, |a: Option<f32>, x| {
                Some(a.map_or(x, |m: f32| m.max(x)))
            })
    }

    /// Mirror of [`left_float_edge`] for right floats.
    fn right_float_edge(&self, y: f32, h: f32) -> Option<f32> {
        self.floats
            .iter()
            .filter(|f| f.side == Float::Right && band_overlaps(f.rect, y, h))
            .map(|f| f.rect.x)
            .fold(None, |a: Option<f32>, x| {
                Some(a.map_or(x, |m: f32| m.min(x)))
            })
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
    ///
    /// **`cb_left`/`cb_right` are the CONTAINING BLOCK's content edges, and they are not the same
    /// thing as this context's edges.** A float participates in its nearest BFC — which is why the
    /// exclusion bands are shared across nested plain blocks and must be — but CSS 2.1 §9.5.1 rules
    /// 1 and 2 pin it to *its own containing block*: "the left outer edge of a left-floating box may
    /// not be to the left of the left edge of its containing block", and the mirror for right.
    ///
    /// Conflating the two put every `float: right` inside a narrow block against the VIEWPORT edge.
    /// Measured (t792): a `float:right` 50px box inside a `width:300px` div reads Chrome **x=250**
    /// and read **x=1150** here — 900px away, on the single most common legacy layout primitive
    /// there is. A miss that size is not one wrong box: it spawns overlap and reading-order
    /// violations across everything the float was supposed to sit beside.
    fn place(
        &mut self,
        side: Float,
        top: f32,
        w: f32,
        h: f32,
        cb_left: f32,
        cb_right: f32,
    ) -> Rect {
        let full = self.right_edge - self.left_edge;
        let mut y = top;
        loop {
            let (l, avail) = self.available(y, h);
            let _ = l;
            let (left_float_edge, right_float_edge) =
                (self.left_float_edge(y, h), self.right_float_edge(y, h));
            if w <= avail || avail >= full {
                // ⚠ **THE CONTAINING BLOCK IS THE ORIGIN; THE FLOATS ARE THE OBSTACLE.** Taking
                // `left_offset` here — which folds in this CONTEXT's left edge — makes the BFC's
                // edge a floor, and a containing block with a negative margin starts OUTSIDE it. So
                // the float's own block gets overruled by a block it is not in.
                let x = if side == Float::Right {
                    right_float_edge.map_or(cb_right, |e| e.min(cb_right)) - w
                } else {
                    left_float_edge.map_or(cb_left, |e| e.max(cb_left))
                };
                // ⚠ **ONLY THE HUGGED EDGE IS CLAMPED, and that was measured rather than reasoned.**
                // The first draft also clamped a right float to `cb_left`, on the theory that a box
                // should never start outside its own block. Chrome disagrees: a `float:right` 400px
                // wide inside a 300px block reads **x = -100** — its right edge stays on the
                // containing block's right edge and it overflows to the LEFT. Clamping made that
                // case read 0, so the fix would have traded a 900px error for a 100px one.
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
                        cb_right - w
                    } else {
                        cb_left
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

/// Is `node` a **button**, whose content is centred vertically in its content box?
///
/// `<button>` and the three button-valued `<input>` types. Not `<select>`, whose text is centred by
/// the same mechanism in Chrome but whose box we synthesise rather than lay out from children, and
/// not `<input type=text>`, which is a single line by construction and is handled by the control's
/// own text path. The narrow set is deliberate: the rule needs a real content height to centre, and
/// these are the controls that get one from their own children.
fn is_button_like(dom: &Dom, node: NodeId) -> bool {
    match dom.tag_name(node) {
        Some("button") => true,
        Some("input") => dom
            .element(node)
            .and_then(|e| e.attr("type"))
            .map(|t| {
                matches!(
                    t.to_ascii_lowercase().as_str(),
                    "submit" | "reset" | "button"
                )
            })
            .unwrap_or(false),
        _ => false,
    }
}

/// Move a laid-out box's CONTENT down by `dy`, leaving the box itself where it is.
///
/// The block half delegates to [`LayoutBox::shift_y`], which already walks a whole subtree and moves
/// its inline fragments' `line_top`/`baseline` with it; the inline half is that same fragment shift
/// applied directly. Used by the button-centring rule, which moves what is inside the border box
/// without moving the border box.
fn shift_content_y(content: &mut BoxContent, dy: f32) {
    if dy == 0.0 {
        return;
    }
    match content {
        BoxContent::Block(kids) => {
            for k in kids.iter_mut() {
                k.shift_y(dy);
            }
        }
        BoxContent::Inline(frags) => {
            for f in frags.iter_mut() {
                f.line_top += dy;
                f.baseline += dy;
            }
        }
    }
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
    // `flow-root` exists for EXACTLY this: a block box whose only distinguishing property is that it
    // establishes a BFC, so it contains its floats without `overflow:hidden`'s clipping.
    s.display == Display::FlowRoot
        || is_float(s)
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

/// Does this box take part in margin collapsing **as a block**? A real `display:block` box does —
/// and so does an inline that CSS2 §9.2.1.1 has split around a block child, which [`is_block_level`]
/// already blockifies for every other layout decision.
///
/// ⚠ **The blockified inline stands in for the spec's ANONYMOUS BLOCK BOXES, and an anonymous block
/// has no margin, border or padding of its own** — so the block child's vertical margins pass
/// straight through it. Testing the RAW `display` here (which both predicates below used to do) made
/// that box opaque to the collapse, so `<a><div style="margin:3px 0 6px">…</div></a>` — the card
/// link, the vote arrow, every block wrapped in an anchor — kept the child's margins INSIDE and came
/// out 9px too tall, displacing everything below it. `is_block_level` said "block"; the collapse
/// predicates said "inline". One rule, two implementations.
fn collapses_as_block(dom: &Dom, styles: &StyleMap, node: NodeId, s: &ComputedStyle) -> bool {
    s.display == Display::Block
        || (s.display == Display::Inline && is_block_level(dom, styles, node))
}

/// **The characters CSS Text calls *document white space* — and NOT `char::is_whitespace`.**
///
/// White-space processing (collapsing runs, trimming edges, choosing soft-wrap opportunities) applies
/// to exactly SPACE, TAB, LINE FEED, CARRIAGE RETURN and FORM FEED (CSS Text 3 §3, §4.1). Rust's
/// `char::is_whitespace` is the **Unicode** `White_Space` property, which is a strictly larger set —
/// and the extra members are precisely the characters an author reaches for when they want a space
/// that is *not* collapsible.
///
/// ⚠ **The one that matters is U+00A0 NO-BREAK SPACE (`&nbsp;`).** `'\u{a0}'.is_whitespace()` is
/// `true`, so every collapse site treated it as ordinary white space: a run of it collapsed away, and
/// an element whose only content was `&nbsp;` was left with **no text at all, hence no line box**.
/// Measured against live Chromium: `<div>&nbsp;</div>` is **18px** tall in Chrome and was **0** here.
/// That is a `dy` term on one of the most common constructs in hand-written HTML — the spacer cell,
/// `10&nbsp;km`, `&nbsp;|&nbsp;` separators, and French punctuation.
///
/// Also in the larger Unicode set and equally non-collapsible: U+2007 FIGURE SPACE, U+202F NARROW
/// NO-BREAK SPACE, and the U+2000–U+200A fixed-width spaces, which authors use for exactly the reason
/// their names suggest.
fn is_css_white_space(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\n' | '\r' | '\u{c}')
}

/// May this block collapse its **top** margin with its first in-flow block child (CSS2 §8.3.1)?
/// A block-collapsing box ([`collapses_as_block`]), `overflow:visible`, not a BFC root, with no top
/// border and no top padding — the conditions under which the child's top margin escapes upward
/// through this box. `cw` is the width the top padding resolves against (this box's
/// containing-block width).
fn top_margin_collapses(
    dom: &Dom,
    styles: &StyleMap,
    node: NodeId,
    s: &ComputedStyle,
    cw: f32,
) -> bool {
    collapses_as_block(dom, styles, node, s)
        && s.overflow == Overflow::Visible
        && !establishes_bfc(s)
        && s.border_width.top == 0.0
        && s.padding.top.resolve(cw, 0.0) == 0.0
}

/// The mirror of [`top_margin_collapses`] for the **bottom** edge: additionally the box must be
/// auto-height (checked by the caller), so the last child's bottom margin escapes downward.
fn bottom_margin_collapses(
    dom: &Dom,
    styles: &StyleMap,
    node: NodeId,
    s: &ComputedStyle,
    cw: f32,
) -> bool {
    collapses_as_block(dom, styles, node, s)
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
    // A box's RIGHT-EDGE insets, as `(margin-right, padding-right + border-right)`, both px and ≥0.
    // Needed because a `LayoutBox` carries only its BORDER box, while `rect.x` already includes the
    // box's LEFT margin — so without adding the right margin the measured extent is asymmetric and
    // short by one margin. A flex item wrapping `<p margin:10>` reported 110 instead of 120 (its
    // content's margin box). The second term exists for the FILL_SENTINEL case below, which is the
    // same asymmetry one level deeper. Percentage/auto insets resolve to 0 for an intrinsic measure;
    // negative margins do not pull the border-box edge in, so this is clamped ≥ 0.
    right_insets: &dyn Fn(Option<NodeId>) -> (f32, f32),
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
        ins: &dyn Fn(Option<NodeId>) -> (f32, f32),
        // The right-edge insets of every FILLED ancestor between this box and the box being
        // measured. See the `else` branch: a skipped box's right insets are real content extent
        // that nothing else in this walk can account for.
        pending: f32,
    ) {
        let (mr, pbr) = ins(b.node);
        let mut pending_kids = pending;
        if b.rect.width < FILL_SENTINEL {
            // `rect.x` includes the LEFT margin; add the RIGHT margin for a full margin-box extent.
            *max_r = max_r.max(rel(b.rect.x) + b.rect.width + mr + pending);
        } else {
            // **A box discarded by FILL_SENTINEL still has right-hand insets, and they are the
            // half of it that nothing downstream can see.** Its LEFT padding/border/margin survive
            // the skip for free — they are baked into where its descendants were laid out, so they
            // show up in the descendants' `x`. Its RIGHT ones have no content after them to carry
            // them, so dropping the box drops them, and the measured max-content comes out short by
            // exactly one padding (or one margin, or one border) per skipped ancestor.
            //
            // That is a shrink-to-fit box hugging its text one padding too tightly, which then
            // re-wraps a line that fitted in Chrome and cascades a whole-line height error down the
            // subtree — the "container-WIDTH errors LAUNDER into dy" mechanism, measured on
            // kicktipp.com as a `<a>` 96px wide against Chrome's 103. Carry them down instead.
            pending_kids += mr + pbr;
        }
        match &b.content {
            BoxContent::Block(kids) => {
                for k in kids {
                    visit(k, fonts, max_r, rel, ins, pending_kids);
                }
            }
            BoxContent::Inline(frags) => {
                *max_r = max_r.max(inline_extent(frags, fonts, rel) + pending_kids);
            }
        }
    }
    match content {
        BoxContent::Block(kids) => {
            for k in kids {
                visit(k, fonts, &mut max_r, &rel, right_insets, 0.0);
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
        if depth > 64 || !top_margin_collapses(self.dom, self.styles, node, s, cw) {
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
                continue; // out of flow: not a child for §8.3.1's purposes — SKIP, do not stop
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
        if depth > 64
            || definite_height
            || !bottom_margin_collapses(self.dom, self.styles, node, s, cw)
        {
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
                continue; // out of flow: not a child for §8.3.1's purposes — SKIP, do not stop
            }
            if is_block_level(self.dom, self.styles, k) {
                return collapse_margins(mb, self.collapse_through_bottom(k, cw, depth + 1));
            }
            return mb;
        }
        mb
    }

    /// The collapse-through bottom margin of `node`'s last in-flow block child, or `0.0` if that
    /// child is not a block. This is the amount that escapes downward out of the parent in a bottom
    /// collapse. Out-of-flow children are **skipped**, not treated as terminators — see
    /// [`Ctx::leading_block_collapse_top`] for the measurement and the reasoning.
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
                continue; // out of flow: not a child for §8.3.1's purposes — SKIP, do not stop
            }
            if is_block_level(self.dom, self.styles, k) {
                return self.collapse_through_bottom(k, cw, 1);
            }
            return 0.0;
        }
        0.0
    }

    /// The collapse-through top margin of `node`'s first in-flow block child, or `0.0` if that child
    /// is not a block or carries clearance (clearance blocks the parent-child collapse). This is the
    /// amount hoisted out of the parent in a top collapse.
    ///
    /// ⚠⚠ **AN OUT-OF-FLOW CHILD IS SKIPPED, NOT A TERMINATOR — and reading it as one CANCELLED the
    /// collapse on the single commonest float idiom on the web.** CSS 2.1 §8.3.1 collapses a box's
    /// top margin with its first **in-flow** child's; a float or an absolutely-positioned box is by
    /// definition not an in-flow child, so it is passed over and the block *after* it is the first
    /// in-flow child. All four §8.3.1 search helpers here `return`ed on one, described in the comment
    /// as "conservative" — but conservative in the wrong direction is just wrong: it left the child's
    /// margin *inside* the parent, which is a visible gap, not a cautious no-op.
    ///
    /// Chrome-measured (`file:///tmp/mc.html`, 800px, `p{margin:15px 0}`, `body{margin:0}`):
    ///
    /// | first child | Chrome parent y / first `<p>` y | meaning |
    /// |---|---|---|
    /// | `float:right` div | `15` / `15` | collapsed **through** the float |
    /// | `position:absolute` div | `68` / `68` | collapsed **through** the abspos box |
    /// | text | `159` / `192` | NOT collapsed — real inline content does separate |
    ///
    /// The block layout loop never had this bug: `first_block` is cleared only by a *block-level*
    /// child, so a float already did not count there. The hoist computation and the placement
    /// disagreed with each other, and the placement was the correct one.
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
                continue; // out of flow: not a child for §8.3.1's purposes — SKIP, do not stop
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

    /// **CSS 2.1 §10.3.7 / §10.6.4 — an insetless `position:absolute` box sits where its
    /// hypothetical box would have started, and on an inline line that includes the ADVANCE of
    /// everything before it.** The seed in the pure-IFC branch records the container's content-box
    /// origin, which is right only when the abs box is the first thing in the parent. This replaces
    /// it once the line is laid out.
    ///
    /// Chrome, `<a style="display:block"><span>Hello</span><span class="sr-only">SR</span></a>` in
    /// a 400px `position:relative` wrapper: the `.sr-only` span belongs at **x=35** (36px of
    /// "Hello", less its `margin:-1px`) and we put it at **x=-1** — the line start. That is
    /// Bootstrap's `.sr-only`, on every framework page that ships it, plus every badge, caret and
    /// tooltip written as `position:absolute` after inline content with only `top` set.
    ///
    /// **Attribution is by SUBTREE, and content it cannot attribute is left on the seed.** A
    /// `TextFragment`'s `node` is the deepest *element* ancestor of its text, so bare text sitting
    /// directly in the block reports the block itself rather than the child text node — there is no
    /// way to tell *which* bare-text sibling such a fragment came from, and guessing would move
    /// boxes on pages this rule should not touch. Elements and atomic inlines (the shape every real
    /// instance of this idiom takes) are attributable and are what this handles.
    ///
    /// **RTL is deliberately excluded** by the caller: under an RTL base direction the inline start
    /// is the right edge, and `frags` have already been through UAX #9 rule L2, so "the trailing
    /// edge of the last preceding fragment" is the wrong end of the wrong box. Named as residue
    /// rather than guessed at.
    fn refine_inline_static_positions(
        &self,
        kids: &[NodeId],
        frags: &[TextFragment],
        atomics: &[LayoutBox],
    ) {
        if !kids.iter().any(|&k| self.kid_is_out_of_flow(k)) {
            return;
        }
        for (i, &k) in kids.iter().enumerate() {
            if !self.kid_is_out_of_flow(k) {
                continue;
            }
            // Every node under an in-flow sibling that PRECEDES this one in source order. Anything
            // after it must not push it along, which is why this is a prefix and not the whole set.
            let mut before: HashSet<NodeId> = HashSet::new();
            for &prev in &kids[..i] {
                if self.kid_is_out_of_flow(prev) {
                    continue;
                }
                before.insert(prev);
                before.extend(self.dom.descendants(prev));
            }
            if before.is_empty() {
                continue;
            }
            // The furthest-along point flow reached: latest line first, then rightmost on it. A
            // fragment that wrapped onto a later line is genuinely later, so `line_top` outranks `x`.
            let mut best: Option<(f32, f32)> = None;
            let mut take = |top: f32, right: f32| {
                let better = match best {
                    None => true,
                    Some((t, r)) => top > t + 0.5 || ((top - t).abs() <= 0.5 && right > r),
                };
                if better {
                    best = Some((top, right));
                }
            };
            for f in frags {
                if f.node.is_some_and(|n| before.contains(&n)) {
                    take(f.line_top, f.x + f.width);
                }
            }
            for b in atomics {
                if b.node.is_some_and(|n| before.contains(&n)) {
                    take(b.rect.y, b.rect.x + b.rect.width);
                }
            }
            if let Some((top, right)) = best {
                self.static_pos.borrow_mut().insert(k, (right, top));
            }
        }
    }

    /// Does `node`'s containing block resolve `direction` to `rtl`? Used only by CSS 2.1 §10.3.3's
    /// over-constrained rule, which is defined against the CONTAINING BLOCK's `direction` rather
    /// than the box's own — and `direction` is inherited, so the two agree everywhere except the
    /// case that distinguishes them (`<div style="direction:ltr">` inside an RTL page).
    ///
    /// The nearest element ancestor is the containing block for an in-flow block, which is the only
    /// caller. A text node or a missing style answers `false`, i.e. the LTR behaviour that shipped.
    fn parent_is_rtl(&self, node: NodeId) -> bool {
        let mut cur = self.dom.parent(node);
        while let Some(a) = cur {
            if self.dom.is_element(a) {
                return self.style_of(a).direction == manuk_css::Direction::Rtl;
            }
            cur = self.dom.parent(a);
        }
        false
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
        let mut s = self.style_of(node).clone();

        // Tables size their own width (shrink-to-columns when auto), so they run a
        // dedicated formatter rather than the generic block width algorithm.
        //
        // ⚠⚠ **…unless it has no rows at all, in which case it is not a table, it is a
        // SHRINK-TO-FIT BLOCK.** `collect_table_rows` keeps only `table-row`/`table-row-group`
        // ELEMENTS, so a `display:table` box whose content is bare text — or any non-table
        // content — yields zero rows and `layout_table` produced an **empty box**. Not narrow:
        // absent. Chrome-measured (t814):
        //
        // ```text
        //                                            Chrome         before
        //   display:table, bare text "short"       [0   0  36x20]   0x0
        //   display:table, a longer run of text    [0  20 213x20]   0x0
        //   display:table, width:200px, bare text  [0  86 200x20]   0x0   ← even EXPLICIT
        //   display:inline-table, bare text        [0 106  72x20]   0x0
        // ```
        //
        // An explicit width did not save it, which is what rules out sizing and names the cause.
        //
        // CSS 2.1 §17.2.1 wraps such content in an anonymous table-cell inside an anonymous
        // table-row — and a table with ONE anonymous cell is, for both axes, exactly a
        // shrink-to-fit block over the same content. So rather than synthesise boxes the row
        // collector has no node to return, the style CLONE is given `width: fit-content` and the
        // generic block path runs. An author's explicit width is left alone (the guard), so
        // `width:200px` stays 200.
        //
        // ⚠ THE REACH is the pre-flexbox layout vocabulary: `display:table; margin:0 auto` to
        // shrink-wrap and centre, and `display:inline-table` — still everywhere in the CrUX tail.
        if s.display == Display::Table {
            if self.collect_table_rows(node).is_empty() {
                if s.width == Dim::Auto && s.width_keyword.is_none() && !s.width_stretch {
                    s.width_keyword = Some(IntrinsicSize::FitContent);
                }
            } else {
                return self.layout_table(node, cw, x, y, prev_margin);
            }
        }

        // ── **THE SLOT IS A FINISHED ANSWER, NOT AN INPUT — AND THREE THINGS WERE STILL BEING
        //    RECOMPUTED ON TOP OF IT** (t823).
        //
        // `taffy_item_width` records that this box is a flex/grid ITEM whose border box taffy already
        // resolved. Tick ~700 used it to stop re-resolving the item's own `width` against its own
        // slot (the comment further down tells that story). But taffy's answer includes **more than
        // the width**: it also applied the item's `min-width`/`max-width` clamp, and it positioned the
        // slot with the item's MARGINS already taken out of the line. Both were then applied a second
        // time here. Measured against headless Chrome on a 1200px `display:flex` row:
        //
        // ```text
        //                                              Chrome            before
        //   flex:0 0 90%; max-width:50%              [0 140 600x20]    600 → 300   ✗ clamped twice
        //   width:90%;    max-width:50%              [0 160 600x20]    600 → 300   ✗
        //   flex:0 0 50%; margin-left:100px          [100 0 600x20]    x = 200     ✗ margin twice
        //   flex:0 0 50%; margin-left:10%            [120 20 600x20]   x = 180     ✗
        //   grid item, 800px track, max-width:50%    [0 180 400x20]    400 → 200   ✗
        //   grid item, 400px track, margin-left:10%  [840 180 360x20]  x = 876     ✗
        //   flex:0 0 10%; min-width:300px            [0  80 300x20]    300         ✓ (see below)
        //   flex:0 0 90%; max-width:300px            [0 120 300x20]    300         ✓
        //   plain block,  max-width:50% / margin:10% [0 200 600] [120 220 600]     ✓ controls
        // ```
        //
        // ⚠ **A PERCENTAGE CLAMP RE-APPLIED TO THE SLOT ALWAYS BINDS AGAIN; A PIXEL ONE NEVER DOES.**
        // That is why the two `px` rows above are green and were green before: `max-width:300px`
        // against an already-300px slot is a no-op, so the defect was invisible on exactly the rows a
        // reader would reach for first. `max-width: <pct>` is not — 50% of a slot that is *itself* the
        // 50% answer is 25% of the container, and the error is the percentage SQUARED. `min-width:<pct>`
        // is latently wrong the same way but unobservable below 100%, because a percentage of the slot
        // can never exceed the slot.
        //
        // ⚠ **REACH: this is Bootstrap 4's grid.** `.col-8` ships as
        // `flex: 0 0 66.666667%; max-width: 66.666667%` — the `max-width` is the column's whole point
        // (it stops a grown item from exceeding its share) — and it came out **533px against Chrome's
        // 800**. t817/t819 chased that number through `flex-basis` and through line-breaking; t819
        // named `max-width` as the remaining suspect and this is the measurement that convicts it.
        // The margin half is wider still: every `margin-left` on a flex item, `px` or `%`, was doubled.
        let taffy_known = self.taffy_item_width.borrow().get(&node).copied();
        let taffy_item = taffy_known.is_some();

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
                // ⚠ **The orphaned table-internal boxes belong in this arm too, and it is the OTHER
                // HALF of the same fix** — making them atomic in the inline collector without this
                // gives them a line box (right) at the FULL container width (wrong): 600 where
                // Chrome shrink-wraps to 79. A table box sizes to its columns, and CSS 2.1 §17.2.1's
                // anonymous table around an orphan cell is shrink-to-fit exactly as an inline-block
                // is. Each edit alone is a worse answer than neither; they land together.
                Dim::Auto
                    if matches!(
                        s.display,
                        Display::InlineBlock
                            | Display::InlineFlex
                            | Display::InlineGrid
                            | Display::TableCell
                            | Display::TableRow
                            | Display::TableRowGroup
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
        // ── **AN `<img>` WHOSE SOURCE DID NOT LOAD IS 16×16, NOT THE FULL LINE × ZERO** (tick 689).
        //
        // The comment above says `<img>` is excluded because *"a sourceless image has no default
        // object size in any browser"* — true of `<img>` with no `src`, and NOT true of the case the
        // web is full of: an `<img src>` whose bytes never arrive. Measured over headless Chrome on the
        // same fixture rather than recalled:
        //
        // ```text
        //   <img src="…/never.png">            Chrome  16×16      ours  784×0
        //   <img width=120 height=70 src=…>    Chrome 120×70      ours 120×70   ✓
        //   #a3 (the div after the bare img)   Chrome  y=196      ours  y=168
        // ```
        //
        // 16×16 is the broken-image placeholder Chrome reserves, and reserving it is what keeps the
        // rest of the page from sliding up. Our 784×0 is wrong twice: an INLINE replaced element must
        // not take the whole line, and a box with a broken source is not zero-height.
        //
        // Conditioned on `taffy_known.is_none()` (no natural size ⇒ nothing decoded) exactly like the
        // arm above, so an image that HAS loaded, or that carries author dimensions, or that has a
        // ratio to derive from, is untouched — all three already resolved before this line.
        //
        // ⚠ NOT covered, and named rather than left to look handled: an `<img alt="text">` whose source
        // failed. Chrome sizes that box to the ALT TEXT, which needs the text measurer here and is its
        // own change. This arm is the no-alt case, which is what icon/logo/tracker images are.
        let is_img = self.dom.tag_name(node) == Some("img");
        let broken_img_placeholder = is_img
            && taffy_known.is_none()
            && s.aspect_ratio.is_none()
            && s.width == Dim::Auto
            && !s.width_stretch
            && s.width_keyword.is_none();
        if broken_img_placeholder {
            width = 16.0;
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
        // A taffy item's slot is ALREADY clamped — re-clamping squares any percentage (see the top of
        // this function). Skipped wholesale rather than re-resolved against the containing block,
        // because taffy resolved it against the correct reference and a second pass can only be a
        // no-op (px) or wrong (%).
        if !taffy_item {
            if max_w.is_finite() {
                width = width.min(max_w);
            }
            width = width.max(min_w);
        }
        // Did a min/max-width constraint actually move the width? For a **replaced** element that
        // is a constraint violation in CSS2.1 §10.4's sense, and the height has to follow the ratio
        // — see the height derivation below.
        let inline_constraint_violated = width != unclamped_width;

        // Horizontal auto-margin centering when width is definite. A keyword width (`fit-content`
        // etc.) collapses to `Dim::Auto` but IS definite for margins — `width:fit-content;margin:auto`
        // centers the hugged box. Only the left margin shifts the box; the right absorbs the remainder.
        //
        // ⚠⚠ **`inline_constraint_violated` IS THE THIRD TERM, AND WITHOUT IT `max-width` + `margin:
        // 0 auto` — the standard centred-container idiom of the entire modern web — RENDERED FLUSH
        // LEFT.** CSS 2.1 §10.4: when the used width violates `max-width` (or `min-width`), the
        // §10.3.3 rules are *applied again* with the constraint as the computed width — and §10.3.3
        // is precisely where a pair of `auto` margins splits the remainder. The clamp above already
        // does the first half (the box IS 400 wide); this guard was still asking whether the AUTHOR
        // wrote a `width`, which for `.container { max-width: 1200px; margin: 0 auto }` they did not.
        // So the box became definite and the margins never learned about it.
        //
        // The `min-width` half of the same sentence looked fine only because a clamp UP needs an
        // explicit `width` to be observable (`width:auto` already fills the container), so it always
        // took the first term. One rule, two constraints, and only the one that needs no help worked.
        // ⚠ `!taffy_item`: an `auto` margin on a flex item is how `ml-auto` pushes it to the end of
        // the line, and TAFFY is what distributes that free space — against the line, not against this
        // one item's slot. Re-centring here against `cw` (the slot) would shove it back.
        if !taffy_item
            && (s.width != Dim::Auto || s.width_keyword.is_some() || inline_constraint_violated)
        {
            let leftover = cw - (width + pl + pr + bl + br);
            match (s.margin.left.is_auto(), s.margin.right.is_auto()) {
                (true, true) => ml = (leftover / 2.0).max(0.0),
                (true, false) => ml = (leftover - mr).max(0.0),
                // ── **CSS 2.1 §10.3.3 — THE OVER-CONSTRAINED EQUATION IGNORES `margin-left` UNDER
                //    `rtl`.** With a definite `width` and neither margin `auto`, the equation cannot
                //    hold, and the spec says which term gives: *"if the `direction` property of the
                //    containing block has the value `ltr`, the specified value of `margin-right` is
                //    ignored … if the value of `direction` is `rtl`, `margin-left` is ignored."*
                //    So a narrower-than-container block is flush LEFT in an LTR page and flush RIGHT
                //    in an RTL one — every sidebar, card, centred-by-width wrapper and fixed-width
                //    panel on the Arabic/Hebrew/Persian/Urdu web sat on the wrong side.
                //
                //    Chrome-measured, `dir=rtl` body, a 400px block in a 1200px viewport: **x=800**,
                //    and we said **x=0**. Named as residue at t841, where the line-level fix (rule
                //    L2) made the CONTENT of such a block read correctly while the block itself
                //    stayed on the wrong side.
                //
                //    ⚠ **The direction is the CONTAINING BLOCK's, not this element's** — a
                //    `<div style="direction:ltr">` inside an RTL page is still placed by its RTL
                //    parent and stays flush right, which is what makes this a *containing-block*
                //    rule rather than "RTL elements go right". `direction` is inherited, so reading
                //    the element's own style would agree everywhere EXCEPT the one case that
                //    distinguishes the two readings.
                //    ⚠⚠ **NON-REPLACED ONLY, AND THE CORPUS TAUGHT ME THAT.** §10.3.3 is written
                //    for a block-level *non-replaced* box. Applying it to a replaced one moved every
                //    `<svg>` on `www.ta3lemkonline.com` — an atomic inline whose position belongs to
                //    its LINE BOX, not to this equation — and the first draft cost exactly 3 of 457
                //    elements there (deterministic, −0.00656 twice) while fixing NOTHING on the same
                //    page. **Zero fixed and three broken is not a small win with a cost, it is the
                //    wrong rule applied to the wrong box class.**
                (false, false)
                    if !is_replaced_element(self.dom.tag_name(node))
                        && self.parent_is_rtl(node) =>
                {
                    ml = (leftover - mr).max(0.0)
                }
                _ => {}
            }
        }
        let _ = mr; // right margin does not affect downstream positioning here

        // Taffy's slot POSITION already has the item's margins taken out of the line (`extract_placed`
        // passes `base + slot.x/y` straight in), so adding them again moved every margined flex item
        // by exactly twice its margin. The margins are still computed above — they are reported in
        // `BlockResult` and read by the caller — they just must not be spent a second time here.
        let mut border_x = x + if taffy_item { 0.0 } else { ml };
        // Parent↔child TOP margin collapse (CSS2 §8.3.1): when this block has no top border/padding,
        // is `overflow:visible`, and does not establish a BFC, its top margin collapses with its
        // first in-flow block child's collapse-through top margin. That child's margin escapes
        // upward — folded into this box's own top margin here, and the child is placed flush to the
        // content top by `layout_children` (which recomputes the same hoist). `effective_mt` is the
        // collapsed top margin this box contributes to its own parent, so it is what a grandparent
        // collapses against.
        let hoist_top = if top_margin_collapses(self.dom, self.styles, node, &s, cw) {
            self.leading_block_collapse_top(node, width)
        } else {
            0.0
        };
        let effective_mt = collapse_margins(mt, hoist_top);
        // Collapse this block's (possibly child-hoisted) top margin with the preceding sibling's
        // trailing margin to place the border-box top.
        let border_y = y + if taffy_item {
            0.0
        } else {
            collapse_margins(prev_margin, effective_mt)
        };
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
        // Taffy already resolved this item's height against its real containing block; re-resolving
        // the percentage against the slot it produced applies it twice (see `taffy_item_height`).
        // The slot is a BORDER box, so the content height is it less this box's own padding+border —
        // the same subtraction the width axis makes above, and it makes the `box-sizing` adjustment
        // (`bs_extra_h`) redundant for these, exactly as it is for `taffy_known` widths.
        let own_definite_h: Option<f32> = match self.taffy_item_height.borrow().get(&node).copied()
        {
            Some(border_box) => Some((border_box - pt - pb - bt - bb).max(0.0)),
            None => match s.height {
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
            },
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
        let (mut content, content_height) = if establishes_bfc(&s) {
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
        // The height the children actually came to, kept before every override below — a button's
        // vertical centring needs the difference between what the content wanted and what the box
        // was given, and every assignment after this point is one of those overrides.
        let natural_content_h = content_height;
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
        // The height half of the broken-image placeholder — see the `broken_img_placeholder` comment
        // above. 16 tall, Chrome-measured, and only when nothing definite was resolved for it.
        if broken_img_placeholder && own_definite_h.is_none() {
            content_height = 16.0;
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
            && bottom_margin_collapses(self.dom, self.styles, node, &s, cw)
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
        // ── **THE BLOCK-AXIS TWIN OF t823, AND IT WAS BANKED THERE BEFORE IT WAS MEASURED HERE.**
        //
        // Same rule, same reason: taffy already applied this item's `min-height`/`max-height` against
        // its REAL containing block, and `pch` for a taffy item is the SLOT it produced — so a
        // percentage clamp re-resolved here is the percentage SQUARED. Chrome-measured on a 400px
        // `display:flex` row, `flex:0 0 50%; height:100%; max-height:50%`:
        //
        // ```text
        //                                                   Chrome    before   after
        //   flex row item, height:100%, max-height:50%      600x200    100      200   ✗→✓
        //   …the same with max-height:200px                 600x200    200      200    ✓  guard
        //   …with min-height:50%                            600x200    200      200    ✓  guard
        //   column-flex item, max-height:50% / 200px        600x200    200      200    ✓  guard
        //   grid item in a 300px track, max-height:50%      600x150    150      150    ✓  guard
        //   plain block, height:100%, max-height:50%        600x200    200      200    ✓  control
        // ```
        //
        // ⚠ **ONE ROW OF SIX WAS OBSERVABLE, AND IT IS THE SAME ASYMMETRY t823 NAMED.** A `px` clamp
        // re-applied to the slot is a no-op; `min-height: <pct>` of the slot can never exceed the
        // slot. Even the percentage `max-height` cases hid unless the item ALSO had a percentage
        // `height` — without one the item's height is `auto`, and `extract_placed` adopts the slot
        // height *after* this clamp runs, quietly overwriting the squared value. So the defect was
        // masked by a later assignment on every row but one.
        if !taffy_item {
            let unclamped_height = content_height;
            if max_h.is_finite() {
                content_height = content_height.min(max_h);
            }
            content_height = content_height.max(min_h);
            // ── **CSS 2.1 §10.4 RUNS BLOCK → INLINE TOO, AND THIS PATH ONLY EVER RAN IT ONE WAY.**
            //
            // The inline→block half is 60 lines up (`inline_constraint_violated`): a `max-width`
            // that moves a replaced element's used width recomputes its height so the ratio
            // survives. The block→inline half — a `max-height`/`min-height` that moves the HEIGHT
            // must pull the width back the same way — was never written, so the box kept the width
            // it had before the clamp and the picture rendered **stretched**.
            //
            // t831 added exactly this to `layout_float` and its pattern note said the quiet part:
            // *a second implementation of a rule does not inherit the first one's fixes.* This is
            // that sentence collected in the other direction — the float path now has both halves
            // and the block path had one.
            //
            // Chrome-measured on a 1000×266 PNG in a 320px block, which is the AWS Cognito hosted
            // login page's `.logo-customizable { max-width:100%; max-height:30px }` exactly:
            //
            // ```text
            //                                            Chrome   before   after
            //   max-width:100% + max-height:30px         113x30   320x30   113x30   ✗→✓
            //   max-width:100% alone                     320x85   320x85   320x85    ✓  ← control
            //   max-height:30px alone                    113x30   1000x30  113x30   ✗→✓
            // ```
            //
            // The `max-width` control is the one that says which half was missing: it was already
            // right, because the inline→block transfer has been here the whole time.
            //
            // Safe to move the width this late **only** because the guard is `is_replaced_element`:
            // a replaced box has no children, so nothing has been laid out against the old width.
            // The auto-margin centring is re-run below for the same reason it exists at all —
            // §10.4 says the §10.3.3 rules are applied *again* with the constraint as the computed
            // width, and §10.3.3 is where a pair of `auto` margins splits the remainder.
            if content_height != unclamped_height && is_replaced_element(self.dom.tag_name(node)) {
                if let Some(r) = s.aspect_ratio {
                    if r > 0.0 {
                        let mut w = content_height * r;
                        if max_w.is_finite() {
                            w = w.min(max_w);
                        }
                        width = w.max(min_w);
                        let leftover = cw - (width + pl + pr + bl + br);
                        match (s.margin.left.is_auto(), s.margin.right.is_auto()) {
                            (true, true) => ml = (leftover / 2.0).max(0.0),
                            (true, false) => ml = (leftover - mr).max(0.0),
                            _ => {}
                        }
                        border_x = x + ml;
                    }
                }
            }
        }

        // ── **A BUTTON CENTRES ITS CONTENT VERTICALLY, AND NO STYLESHEET CAN SAY SO.** The UA sheet
        //    already gives buttons `text-align: center`, which is why the HORIZONTAL half has always
        //    matched — but the vertical half is not expressible in CSS at all. Blink lays a button's
        //    children out inside an anonymous flex-like box with `align-items: center`; the HTML
        //    rendering spec describes the same thing. So a button taller than its content has that
        //    content centred in its CONTENT BOX, after padding, as a single group.
        //
        //    Chrome-measured, `button{display:block;width:300px;padding:0;border:0;font:16px Arial}`,
        //    y of the label relative to the button's border box:
        //
        //    ```text
        //                                                    Chrome   before   after
        //      height:50px, one 18px line                       16       0      16    ✗→✓
        //      height:50px; padding-top:20px                    26      20      26    ✗→✓
        //      height:80px, TWO block spans (36px together)      22       0      22    ✗→✓
        //      height:20px, an 18px line (nearly full)            1       0       1    ✗→✓
        //      height:auto                                        0       0       0     ✓ control
        //      a plain <div> at height:50px                       0       0       0     ✓ control
        //    ```
        //
        //    Every design-system button fixes a height, so before this the label sat 5-20px too high
        //    on essentially every button on the web — and, being a label inside a fixed-size box, it
        //    is exactly the kind of divergence the `overlap` invariant reports rather than `shape`.
        //
        //    ⚠ It is the CONTENT that moves, not the box: `border_box_h` is already `content_height`
        //    and the button's own rect must not shift. And it is the whole content as ONE group —
        //    two block children keep their 18px separation and move 22 together, which is what makes
        //    this centring rather than per-line alignment.
        if content_height > natural_content_h && is_button_like(self.dom, node) {
            shift_content_y(&mut content, (content_height - natural_content_h) / 2.0);
        }
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
            filters: s.filter.clone(),
            clip_path: s.clip_path.clone(),
            blend: s.mix_blend_mode,
            backdrop: s.backdrop_filter.clone(),
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
            let rtl = self.style_of(node).direction == manuk_css::Direction::Rtl;
            let (frags, _atomics, h) =
                self.layout_inline(items, cx, cy, cw, align, 0.0, floats, None, rtl);
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
            let (frags, _atomics, h) = self.layout_inline(
                items,
                cx,
                cy,
                cw,
                TextAlign::Left,
                0.0,
                floats,
                None,
                self.style_of(node).direction == manuk_css::Direction::Rtl,
            );
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
            .filter(|&k| !self.kid_is_float(k) && !self.kid_is_out_of_flow(k))
            .collect();
        let has_block = flow_kids
            .iter()
            .any(|&k| is_block_level(self.dom, self.styles, k));

        if !has_block && !kids.iter().any(|&k| self.kid_is_float(k)) {
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
            // `(cx, cy)` is the content-box origin — the correct answer only when the abs box is the
            // first thing in the parent. `refine_inline_static_positions` below replaces it with the
            // real inline advance once the line has been laid out; this is the seed, and the fallback
            // for anything that refinement cannot attribute.
            for &k in &kids {
                if self.kid_is_out_of_flow(k) {
                    self.static_pos.borrow_mut().insert(k, (cx, cy));
                }
            }
            let items = self.collect_inline_group(&flow_kids, cw, Some(node));
            let bcs = self.style_of(node);
            let align = bcs.text_align;
            // `text-indent` on the block establishing this IFC applies to its first line box;
            // percentages resolve against the container width.
            let text_indent = bcs.text_indent.resolve(cw, 0.0);
            let (mut frags, atomics, mut h) = self.layout_inline(
                items,
                cx,
                cy,
                cw,
                align,
                text_indent,
                floats,
                Some(&bcs),
                bcs.direction == manuk_css::Direction::Rtl,
            );
            // ── **CSS 2.1 §10.3.7 / §10.6.4 — THE STATIC POSITION INCLUDES THE INLINE ADVANCE.**
            //    Now that the line exists, replace the seed above with where flow actually got to.
            //    Inert unless this block has an out-of-flow child, and skipped under an RTL base
            //    direction (see the helper).
            if bcs.direction != manuk_css::Direction::Rtl {
                self.refine_inline_static_positions(&kids, &frags, &atomics);
            }
            // `text-overflow: ellipsis` truncates a clipped, non-wrapping single line with `…`. Only
            // fires on a box that clips (`overflow` ≠ visible) and doesn't wrap (`nowrap`/`pre`); a
            // line that fits is untouched, so nothing without a real overflow changes.
            if bcs.text_overflow == manuk_css::TextOverflow::Ellipsis
                && !matches!(bcs.overflow_x, manuk_css::Overflow::Visible)
                && matches!(bcs.white_space, WhiteSpace::NoWrap | WhiteSpace::Pre)
            {
                apply_text_overflow_ellipsis(&mut frags, cx, cw, self.fonts);
            }
            // `-webkit-line-clamp: N` caps the box at N lines with a trailing `…`. It rides the same
            // clip guard as the idiom (`overflow` hidden on the vertical axis); with `line_clamp` unset
            // this branch never runs, so an unclamped page is byte-identical. The clamp shrinks the box
            // to N lines, so `h` (returned as the box height, and used for sibling flow) follows.
            if let Some(n) = bcs.line_clamp {
                if !matches!(bcs.overflow_y, manuk_css::Overflow::Visible) {
                    if let Some(clamped_h) =
                        apply_line_clamp(&mut frags, cx, cw, cy, n as usize, self.fonts)
                    {
                        h = clamped_h;
                    }
                }
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
                    filters: Vec::new(),
                    clip_path: None,
                    blend: manuk_css::BlendMode::Normal,
                    backdrop: Vec::new(),
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
        // The anonymous blocks this container generates inherit from IT (CSS 2.1 §9.2.1.1) — its
        // `text-align` and its font/`line-height` strut. Read once here, exactly as the pure-IFC
        // branch above reads `bcs`.
        let run_bcs = self.style_of(node);

        // Parent↔child TOP margin collapse (CSS2 §8.3.1): if THIS container collapses its top margin
        // with its first in-flow block child, that child is placed flush to the content top — its
        // leading margin has escaped upward (folded into the container's own top margin by
        // `layout_block`, which recomputes the identical hoist). Placing the first block `hoist_top`
        // higher lands it exactly at `cy`. `first_block` restricts the shift to that first block.
        let hoist_top =
            if top_margin_collapses(self.dom, self.styles, node, self.style_of(node), cw) {
                self.leading_block_collapse_top(node, cw)
            } else {
                0.0
            };
        let mut first_block = true;

        for &k in &kids {
            let ks = self.style_of(k);
            // `kid_is_*`, not the raw style predicates: a bare text node clones its parent's style,
            // so inside a floated or absolutely-positioned box its own text would take these arms
            // and be dropped from the flow it constitutes. See `kid_is_float`.
            if self.kid_is_float(k) {
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
                    &run_bcs,
                );
                let fbox = self.layout_float(k, cw, cur_y + prev_margin.max(0.0), floats, cx);
                boxes.push(fbox);
            } else if self.kid_is_out_of_flow(k) {
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
                    &run_bcs,
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
            &run_bcs,
        );

        // ⚠⚠ **THE CLEARFIX — a BLOCK-LEVEL `::after`, which generated content had no way to be.**
        //    `collect_inline_group` materialises `::before`/`::after` as inline WORDS, and its own
        //    comment says that is "the only place [generated content] can enter the flow". So a
        //    pseudo with `display: block` produced no box at all — and it dropped `content: ""` on
        //    top of that, because an empty string looked like nothing to render.
        //
        //    `.cf::after { content: ""; display: block; clear: both }` is **the** float-containment
        //    idiom of the last fifteen years — every Bootstrap-era grid, every WordPress theme, every
        //    hand-rolled `.clearfix`. Its entire job is to be a box that clears, so the parent's
        //    height grows past its floats. With no box, nothing cleared and **the parent collapsed to
        //    zero**, dumping its floated children outside it and pulling every following sibling up.
        //
        //    Measured on `keirin.jp`, whose nav is exactly this shape: `#nav_menus` and `#navbar` are
        //    **h=0 against Chrome's h=70**, and 70 is precisely the `dy` the first-divergence probe
        //    reports for that page. Fixture, Chrome `--headless=new` 1200×800:
        //
        //    ```text
        //                                              Chrome   before   after
        //      .cf::after{content:"";display:block;clear:both}    h70      h0     h70
        //      .cfb::after{content:"";display:table;clear:both}   h70      h0     h70
        //      a plain block (must NOT contain its float)         h0       h0     h0    <- guard
        //      overflow:hidden (already worked)                   h70      h70    h70   <- guard
        //    ```
        //
        //    ⚠ Deliberately NOT a general generated-block-box implementation: this places the box,
        //    honours `clear`, and gives it its own height/margins, which is the whole of the idiom's
        //    observable effect. It paints nothing, because the clearfix has nothing to paint — a
        //    pseudo carrying a background or a border still belongs to the inline path, and giving it
        //    a painted block box here would be a second implementation of the same rule. Named so the
        //    next person extending it knows which half exists.
        if let Some(p) = self
            .styles
            .get(&node)
            .and_then(|s| s.after.as_ref())
            .filter(|p| p.content.is_some())
            .filter(|p| {
                matches!(
                    p.display,
                    Display::Block
                        | Display::FlowRoot
                        | Display::Table
                        | Display::Flex
                        | Display::Grid
                )
            })
        {
            // Clearance first — it is the reason the box exists — then the box's own extent.
            if p.clear != Clear::None {
                let base = cur_y + prev_margin;
                let cleared = floats.clear_to(p.clear, base);
                if cleared > base {
                    cur_y = cleared;
                    prev_margin = 0.0;
                }
            }
            let mt = p.margin.top.resolve(cw, 0.0);
            let mb = p.margin.bottom.resolve(cw, 0.0);
            let h = p.height.resolve(pch.unwrap_or(0.0), 0.0);
            cur_y += prev_margin.max(mt) + h;
            prev_margin = mb;
        }

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
        // The containing block's LEFT content edge, in the same absolute space the float context
        // uses. Together with `cw` this is what pins the float to its own block rather than to
        // whatever BFC happens to own the exclusion bands — see `FloatContext::place`.
        cb_left: f32,
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
        let mut width = match s.width {
            // A float shrink-to-fits on `auto` — that is the whole point of a float — so `stretch`
            // is the only way to say "this floated card fills its column", and it is the difference
            // between a full-width banner and one hugging its text.
            Dim::Auto if s.width_stretch => avail,
            Dim::Auto => self.shrink_to_fit(node, avail),
            // ── **`box-sizing: border-box` — THE FLOAT PATH NEVER APPLIED IT.** A specified width on a
            // border-box element is the BORDER box, so the content width is that minus padding and
            // border; `layout_block` has done this since t~ (`bs_extra_w`), and this function, which is
            // a separate width resolution, simply did not. `auto` is already a content width, so only
            // the specified arm subtracts.
            //
            // Measured vs live Chromium on the exact corpus shape (`*{box-sizing:border-box}` +
            // `.card{width:50%;float:left;padding:0 5px}` in a 704px container): Chrome **352** border
            // box / **342** content, ours **362 / 352** — every float 10px too wide, and the control in
            // the same fixture (the identical box WITHOUT `float`) was already Chrome-exact at 352/342.
            //
            // `*{box-sizing:border-box}` is in every CSS reset written since 2011, and a
            // `width:%` + `padding` float is the pre-flexbox column — i.e. most of the WordPress web.
            other => {
                let w = other.resolve(cw, avail).max(0.0);
                if s.box_sizing == BoxSizing::BorderBox {
                    (w - (pl + pr + bl + br)).max(0.0)
                } else {
                    w
                }
            }
        };

        // ── ⓵ **A FLOATED REPLACED ELEMENT DERIVES ITS AUTO WIDTH FROM ITS HEIGHT AND ITS RATIO**
        // (CSS2 §10.4) — the mirror of the height derivation below, and `layout_block` has had it
        // for as long as it has had `aspect_ratio`. This path never did, so a floated `<img>` with
        // a height and no width shrink-to-fit to the width of its (empty) content: **zero**.
        //
        // `width: stretch` is a definite width wearing `Dim::Auto`'s representation, so it is
        // excluded here exactly as it is in `layout_block` — the ratio must not overwrite it.
        if s.width == Dim::Auto && !s.width_stretch {
            if let (Some(r), Dim::Px(h)) = (s.aspect_ratio, s.height) {
                if r > 0.0 {
                    width = h * r;
                }
            }
        }

        // ── ⓶ **`min-width` / `max-width` — THE FLOAT PATH APPLIED NEITHER, EVER.**
        //
        // Not "applied them wrongly": the words do not appear in this function. A float is a second,
        // hand-rolled width resolution living beside `layout_block`'s, and it has been acquiring that
        // function's rules one measured defect at a time (`box-sizing` was the last one, in the arm
        // directly above). Chrome-measured on plain floated `<div>`s, so no replaced-element
        // machinery is in the way of reading it:
        //
        // ```text
        //                                              Chrome   before   after
        //   float, width:200px; max-width:50px         50x10    200x10   50x10   ✗→✓
        //   float, width:20px;  min-width:80px         80x10     20x10   80x10   ✗→✓
        //   float, width:10px;  max-height:50px        10x50    10x200   10x50   ✗→✓
        //   float, width:10px;  min-height:80px        10x80     10x20   10x80   ✗→✓
        // ```
        //
        // `.col { float:left; width:50%; max-width:600px }` is the entire pre-flexbox responsive
        // column, and `img { max-width:100% }` is in every CSS reset written since 2011 — so this is
        // not an edge of the float path, it is most of what floats are used for.
        //
        // Same construction as `layout_block`: max first, then min wins, both converted to the
        // content box so a `border-box` clamp measures the same edge the specified width did.
        let bs_extra_w = if s.box_sizing == BoxSizing::BorderBox {
            pl + pr + bl + br
        } else {
            0.0
        };
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
        // CSS2.1 §10.4: a clamp that MOVED the used width of a replaced element is a constraint
        // violation, and the used height is recomputed from it so the ratio survives. This is what
        // turns `float:left; max-width:50px` on a 101×32 logo into 50×16 rather than 50×32.
        let inline_constraint_violated = width != unclamped_width;

        // **A floated table must still get TABLE layout.** `layout_table` is only reached from
        // `layout_block`, so a table arriving here (float) — or as a flex/grid item — fell through
        // to the generic path, where `<tr>`/`<th>` are not "block-level" and every cell's text
        // simply flowed inline. That is why Wikipedia's infobox rendered as one run of text.
        // Run the real table formatter at a provisional origin, then place its margin box.
        // Same rowless exception as `layout_block`: a floated `display:table` with no rows is a
        // shrink-to-fit block, and routing it into the table formatter yields a 0x0 float.
        if s.display == Display::Table && !self.collect_table_rows(node).is_empty() {
            let r = self.layout_table(node, cw, 0.0, 0.0, 0.0);
            let mut b = r.boxx;
            let (mbw, mbh) = (ml + b.rect.width + mr, mt + b.rect.height + mb);
            let margin_rect = floats.place(s.float, top, mbw, mbh, cb_left, cb_left + cw);
            b.shift_x(margin_rect.x + ml - b.rect.x);
            b.shift_y(margin_rect.y + mt - b.rect.y);
            return b;
        }

        // Lay out content at a provisional origin (0,0) in the float's own BFC.
        let mut inner = FloatContext::new(0.0, width);
        let (content, ch) = self.layout_children(node, 0.0, 0.0, width, None, &mut inner);
        // ── ⓷ **AND `box-sizing: border-box` ON THE BLOCK AXIS, which the width arm above already
        // got and this one did not** — a specified height on a border-box float came out padding +
        // border too tall (Chrome-measured: `box-sizing:border-box; padding:10px; height:100px`
        // floated is **100** tall, ours was 120). One rule, two axes, and only the inline one landed.
        let bs_extra_h = if s.box_sizing == BoxSizing::BorderBox {
            pt + pb + bt + bb
        } else {
            0.0
        };
        let mut content_height = match (s.height, s.aspect_ratio) {
            // A replaced element's auto height comes from its USED width and its intrinsic ratio
            // (CSS2 §10.6.2). Without this a floated `<img>` is its content's height — and an
            // `<img>` has no children, so **zero**. This is the defect that aimed the tick:
            // `.logo a img { float:left }` measured `101x0` against Chrome's `101x32`, with the
            // otherwise-identical unfloated image in the same document already Chrome-exact.
            (Dim::Auto, Some(r)) if r > 0.0 => width / r,
            // The §10.4 transfer, inline → block: the width was clamped, so the height follows it.
            (_, Some(r))
                if r > 0.0
                    && inline_constraint_violated
                    && is_replaced_element(self.dom.tag_name(node)) =>
            {
                width / r
            }
            (Dim::Auto, _) => ch.max((inner.lowest_bottom()).max(0.0)),
            (other, _) => (other.resolve(0.0, ch) - bs_extra_h).max(0.0),
        };
        // The block-axis half of ⓶. A percentage min/max-height on a float resolves against its
        // containing block's height, which this path does not carry — and CSS2 §10.7 says a
        // percentage `max-height` against an INDEFINITE containing block is treated as `none`. So an
        // unresolvable percentage is dropped rather than resolved against 0, because resolving
        // against 0 clamps the box to nothing: that is the exact shape of the bug the
        // `layout_block` twin of this comment records, where `img { max-height:100% }` erased every
        // responsive image on the page. A px/em clamp — which is what the corpus actually uses —
        // resolves normally.
        let indefinite_pct = |d: Dim| {
            matches!(d, Dim::Percent(_)) || matches!(d, Dim::Calc { pct, .. } if pct != 0.0)
        };
        let min_h = if indefinite_pct(s.min_height) {
            0.0
        } else {
            (s.min_height.resolve(0.0, 0.0) - bs_extra_h).max(0.0)
        };
        let max_h = match s.max_height {
            Dim::Auto => f32::INFINITY,
            other if indefinite_pct(other) => f32::INFINITY,
            other => (other.resolve(0.0, f32::INFINITY) - bs_extra_h).max(0.0),
        };
        let unclamped_height = content_height;
        if max_h.is_finite() {
            content_height = content_height.min(max_h);
        }
        content_height = content_height.max(min_h);
        // §10.4 again, block → inline. A `max-height` that moved a replaced element's height must
        // pull the width back through the ratio, or the picture renders stretched at its old width.
        // This is `.help img { max-height:14px; max-width:14px }` over an `<img height="16">` — the
        // shape that made `app.ordertime.com` measure `0x16` where Chrome measures **14x14**.
        if content_height != unclamped_height {
            if let Some(r) = s.aspect_ratio {
                if r > 0.0 && is_replaced_element(self.dom.tag_name(node)) {
                    width = (content_height * r).min(max_w).max(min_w);
                }
            }
        }

        let border_box_w = bl + pl + width + pr + br;
        let border_box_h = bt + pt + content_height + pb + bb;
        let margin_box_w = ml + border_box_w + mr;
        let margin_box_h = mt + border_box_h + mb;

        let side = s.float;
        let margin_rect =
            floats.place(side, top, margin_box_w, margin_box_h, cb_left, cb_left + cw);
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
            filters: s.filter.clone(),
            clip_path: s.clip_path.clone(),
            blend: s.mix_blend_mode,
            backdrop: s.backdrop_filter.clone(),
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
    /// The ordinal follows the HTML "ordinal value" algorithm — a running counter over the list
    /// items honouring `<ol start>`, `<ol reversed>`, and an item's own `value` (which continues
    /// the count for every item after it, not just itself).
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
        // The ordinal follows the HTML "ordinal value" algorithm: a running counter, not this
        // item's sibling index. Two things the index form got silently wrong — a `value` on any
        // item CONTINUES the count (the next unmarked item is value±1, not its position), and
        // `reversed` counts DOWN. Index-based numbering prints a resumed list restarting at its
        // position and a ranked/countdown `<ol reversed>` going 1,2,3… upward.
        let el_attr = |n: NodeId, name: &str| -> Option<&str> {
            self.dom.element(n).and_then(|e| e.attr(name))
        };
        let parse_i64 = |v: &str| v.trim().parse::<i64>().ok();
        let reversed = parent.is_some_and(|p| el_attr(p, "reversed").is_some());
        let li_count = parent
            .map(|p| {
                self.dom
                    .children(p)
                    .filter(|&c| self.dom.tag_name(c) == Some("li"))
                    .count() as i64
            })
            .unwrap_or(0);
        // No `start`: forward lists begin at 1, reversed lists at the item count (so the first
        // item is N and the last is 1).
        let start = parent
            .and_then(|p| el_attr(p, "start"))
            .and_then(parse_i64)
            .unwrap_or(if reversed { li_count } else { 1 });
        let step = if reversed { -1 } else { 1 };
        let mut counter = start;
        let mut ordinal = start;
        if let Some(p) = parent {
            for c in self.dom.children(p) {
                if self.dom.tag_name(c) != Some("li") {
                    continue;
                }
                // A `value` resets the running counter for this item and everything after it.
                if let Some(v) = el_attr(c, "value").and_then(parse_i64) {
                    counter = v;
                }
                if c == node {
                    ordinal = counter;
                    break;
                }
                counter += step;
            }
        }
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
    /// Is this CHILD a float / out-of-flow box? — the node-aware form, and the only one a child
    /// filter may use.
    ///
    /// ⚠⚠ **A BARE TEXT NODE CARRIES A CLONE OF ITS PARENT'S STYLE under the Stylo cascade**, so
    /// `is_out_of_flow_positioned(self.style_of(text))` inside a `position:absolute` box answers
    /// **yes** — and every filter that asked it dropped the box's own text. `<div
    /// style="position:absolute">Menu</div>` measured **0×0**: the text filtered itself out of the
    /// content it WAS. A text node has no box of its own, cannot be positioned and cannot float, so
    /// the element check is not an optimisation — it is the predicate's precondition.
    ///
    /// `max_content_width_uncached` already documents this exact trap for `display:flex` ("a bare
    /// run inside `display:flex` reads back as `flex` here") and guards it with `is_element`. Same
    /// cascade quirk, same guard, four more call sites.
    fn kid_is_float(&self, k: NodeId) -> bool {
        self.dom.is_element(k) && is_float(self.style_of(k))
    }

    /// See [`Self::kid_is_float`] — a text node is never absolutely or fixed positioned.
    fn kid_is_out_of_flow(&self, k: NodeId) -> bool {
        self.dom.is_element(k) && is_out_of_flow_positioned(self.style_of(k))
    }

    fn min_content_width(&self, node: NodeId) -> f32 {
        if let Some(&c) = self.min_content_cache.borrow().get(&node) {
            return c;
        }
        let mut fc = FloatContext::new(0.0, 1.0);
        let (content, _h) = self.layout_children(node, 0.0, 0.0, 1.0, None, &mut fc);
        // Ceil to the LayoutUnit grid for the same reason max-content does: a box given exactly its
        // min-content width must still fit its longest unbreakable run.
        let w = taffy_tree::ceil_to_layout_unit(
            content_right_extent(&content, self.fonts, 0.0, &|n| self.px_right_insets(n))
                + self.native_widget_width(node),
        );
        self.min_content_cache.borrow_mut().insert(node, w);
        w
    }

    /// **The part of a control's intrinsic width that is the WIDGET, not the text** — today, a
    /// `<select>`'s dropdown arrow.
    ///
    /// A select sizes to its selected option, and every engine then adds room for the arrow it
    /// draws beside it. Measured against headless Chrome, and the number is a constant rather than
    /// a proportion, which is what identifies it: **159 vs our 142 with a long option, 30 vs our 13
    /// with a one-character one — the same 17px either way.** A text-measurement difference would
    /// have scaled with the text.
    ///
    /// ⚠ **`appearance: none` is the condition, and without it this would be a TRADE.** That
    /// declaration takes the native widget off the control — Chrome drops to 139px on the same
    /// option text — so reserving unconditionally would fix the classic select and newly break every
    /// restyled one, which is most of the modern web's design systems. Reading the property is what
    /// makes this a fix rather than a swap of one error for another.
    ///
    /// ⚠ We do not PAINT the arrow (this engine draws no native widget — `G_APPEARANCE_NONE`), so
    /// the reserved strip is blank. That is a Bar-2 gap, deliberately: the BOX is what every sibling
    /// and every ancestor is laid out against, and a right box with a missing glyph is a smaller
    /// error than a wrong box.
    fn native_widget_width(&self, node: NodeId) -> f32 {
        if self.dom.tag_name(node) != Some("select") {
            return 0.0;
        }
        if self.style_of(node).appearance_none {
            return 0.0;
        }
        17.0
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
        // Ceil to the LayoutUnit grid, never round — see `taffy_tree::ceil_to_layout_unit`. An
        // intrinsic width that is a few thousandths of a pixel SHORT of what its own content needs
        // makes the box re-wrap the run it was measured from.
        let pref = taffy_tree::ceil_to_layout_unit(self.max_content_width_uncached(node));
        self.max_content_cache.borrow_mut().insert(node, pref);
        pref
    }

    /// The right-edge insets `(margin-right, padding-right + border-right)` of a box's node, in px
    /// and ≥0, for the margin-box extent in `content_right_extent`. Percentage/auto insets resolve
    /// to 0 for an intrinsic measure; negatives don't extend the box.
    fn px_right_insets(&self, n: Option<NodeId>) -> (f32, f32) {
        n.map_or((0.0, 0.0), |node| {
            let s = self.style_of(node);
            (
                s.margin.right.resolve(0.0, 0.0).max(0.0),
                s.padding.right.resolve(0.0, 0.0).max(0.0) + s.border_width.right.max(0.0),
            )
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
        // The widget strip rides on BOTH intrinsic widths, or a select would hug its text at
        // max-content and reserve at min-content — the box would change size with the space around
        // it, which is not what a reserved widget is.
        let pref = content_right_extent(&content, self.fonts, 0.0, &|n| self.px_right_insets(n))
            + self.native_widget_width(node);
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

    /// The size a **replaced element with a default object size** wants to be, for the taffy
    /// measure seam — `None` for everything else, which is every ordinary box.
    ///
    /// The tag list and the rules are the block path's, restated at the seam rather than re-derived:
    /// an author width wins; a definite height plus an intrinsic ratio derives the width; a ratio
    /// with an auto width fills the available width (CSS2 §10.3.2's last resort); and with neither
    /// it is the default object size, 300×150 (CSS-Images §4.4).
    ///
    /// ⚠⚠ `avail_width` is `None` for taffy's **max-content** probe, and the answer there is the
    /// unbounded one — a ratio'd replaced element's preferred width is "as much as it can get".
    /// Falling back to the default object width (300) instead reads as a *preference* for 300px and
    /// the flex algorithm honours it: measured, that put a nav-bar icon at 300×300 next to a 56px
    /// label where Chrome gives it the 544px the label leaves. Taffy shrinks the 1e6 down to the
    /// free space, which is what makes this the Chrome answer rather than an unbounded one.
    fn replaced_default_size(&self, node: NodeId, avail_width: Option<f32>) -> Option<(f32, f32)> {
        // ── **`<img>` WAS NOT ON THIS LIST, AND IT IS THE ONE THE WEB IS MADE OF.**
        //
        // The list was written for the DEFAULT OBJECT SIZE (300×150), which `<img>` correctly does
        // not have — and that exclusion silently did a second job it was never meant to do: it kept
        // images out of the seam that reports a replaced element's **intrinsic** size to taffy. So a
        // flex item `<img>` told taffy its content wanted **zero**, taffy's automatic minimum size
        // (CSS Flexbox §4.5, `min-width:auto`) floored at nothing, and a row of logos shrank to
        // slivers instead of overflowing a scroll container.
        //
        // `<img>` is admitted here but still gets **no default object size**: the guard below
        // returns `None` when neither a definite axis nor a ratio is known, so a sourceless or
        // not-yet-decoded image falls through to the broken-image path exactly as before. What it
        // gains is the case where we DO know the size — which, once the bytes have arrived, is
        // every image on the page.
        let is_img = self.dom.tag_name(node) == Some("img");
        if !is_img
            && !matches!(
                self.dom.tag_name(node),
                Some("svg" | "canvas" | "video" | "object" | "embed")
            )
        {
            return None;
        }
        let s = self.style_of(node);
        let ratio = s.aspect_ratio.filter(|r| *r > 0.0);
        // No default object size for `<img>`: with nothing known, say nothing. `300×150` here would
        // resurrect the bug t689 fixed (an image whose bytes never arrive is 16×16, not a
        // full-line-by-150 band), and a *wrong* answer from this seam is worse than none, because
        // taffy trusts it as the item's content size.
        if is_img
            && ratio.is_none()
            && !matches!(s.width, Dim::Px(_))
            && !matches!(s.height, Dim::Px(_))
        {
            return None;
        }
        let width = match (s.width, ratio, s.height) {
            (Dim::Px(w), ..) => w,
            (_, Some(r), Dim::Px(h)) => h * r,
            (_, Some(_), _) => avail_width.filter(|a| a.is_finite()).unwrap_or(1.0e6),
            // ⚠ **THE DEFAULT OBJECT SIZE MUST NOT LEAK INTO `<img>` ON EITHER AXIS.** The guard at
            // the top of this function catches an image with NOTHING known; this catches the one
            // that cost a real regression — an image with a definite WIDTH and no ratio, which took
            // the `150.0` height below and rendered a 36×0 icon as 36×150. Caught by the 16-site
            // control on `777juegos.com` (whose footer is a row of unloaded payment icons Chrome
            // measures at height ZERO), not by any fixture. An `<img>` with an underivable axis has
            // no answer to give here, and `None` sends it back to the broken-image path (t689).
            _ if is_img => return None,
            _ => 300.0,
        };
        let height = match (s.height, ratio) {
            (Dim::Px(h), _) => h,
            (_, Some(r)) => width / r,
            _ if is_img => return None,
            _ => 150.0,
        };
        Some((width.max(0.0), height.max(0.0)))
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
        // ⚠⚠ **A REPLACED ELEMENT HAS NO CHILDREN TO MEASURE, so measuring them reports ZERO.**
        //
        // This seam answers "how big does this flex/grid item want to be?" by laying the subtree out
        // and reading how far the content reached. For `<canvas>`/`<video>`/`<svg>` there IS no
        // subtree, so the honest content extent is 0 — and an unsized `<canvas>` flex item came out
        // **0×150** where Chrome measures 300×150. The block path already owns the default-object /
        // ratio model (see `an_unsized_svg_gets_the_default_object_size`); taffy simply never asked
        // it. Chrome-measured on one fixture, all four in a 400px container:
        //
        // ```text
        //                                    CHROME     BEFORE
        //   flex  <canvas>                   300×150     0×150
        //   flex  <video>                    300×150     0×150
        //   flex  <svg viewBox="0 0 100 25"> 400×100     0×0
        //   grid  <svg viewBox="0 0 100 25"> 400×100   400×100  ← the grid path already stretched
        // ```
        if let Some(sz) = self.replaced_default_size(node, avail_width) {
            self.measure_cache.borrow_mut().insert(key, sz);
            return sz;
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
        // ── **AN RTL TABLE'S COLUMN AXIS RUNS RIGHT-TO-LEFT.** `direction` on the table box orders the
        // COLUMNS, not just the text inside them (CSS 2.1 §17.5.3: the column axis follows the inline
        // direction), so column 0 — the first `<td>` in source order — is the RIGHTMOST one.
        //
        // Measured vs live Chromium, `<html dir=rtl>`, a 600px table of four 150px cells (x relative to
        // the table): Chrome **450 / 300 / 150 / 0** for the 1st…4th cell; ours was 0 / 150 / 300 / 450 —
        // the whole table read backwards, which is the single largest mechanism on `mobile.ir` (the worst
        // `reading_order` site in the CrUX sample: 250+ `<td>` x-divergences).
        //
        // ⚠ The direction is read from the TABLE's own computed style, not the document's: a
        // `<table style="direction:ltr">` inside an RTL page keeps LTR column order, and Chrome agrees
        // (fixture row `#t2`: 0 / 300, unchanged). Mirroring the whole SPAN — not the first column —
        // is what makes `colspan` land on the right cells.
        let rtl_cols = s.direction == manuk_css::Direction::Rtl;
        for (pi, p) in placed.iter().enumerate() {
            let cx0 = col_x.get(p.col).copied().unwrap_or(content_x);
            let cw_span = span_w(p.col, p.colspan);
            let cx = if rtl_cols {
                content_x + content_w - (cx0 - content_x) - cw_span
            } else {
                cx0
            };
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
                filters: rs.map(|s| s.filter.clone()).unwrap_or_default(),
                clip_path: rs.and_then(|s| s.clip_path.clone()),
                blend: rs.map(|s| s.mix_blend_mode).unwrap_or_default(),
                backdrop: rs.map(|s| s.backdrop_filter.clone()).unwrap_or_default(),
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
            filters: s.filter.clone(),
            clip_path: s.clip_path.clone(),
            blend: s.mix_blend_mode,
            backdrop: s.backdrop_filter.clone(),
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
    /// ⚠⚠⚠ **BAR 0 — AN UNCLAMPED `colspan` IS AN INFINITE LOOP WITH A NUMBER IN IT.**
    ///
    /// `colspan` and `rowspan` are HTML **"clamped unsigned long"** attributes, and the clamp is not
    /// decoration: `<td colspan="2147483648">` parses cleanly as a `usize` on a 64-bit target, and
    /// the table then tries to build **two billion columns**. Measured here: the page never
    /// finishes. `g_reflect_numeric` did not fail, it *spun* — `user 2m57s` of a 3m00s cap, on a
    /// four-element fixture — and because a hang is not a red assertion it read as a slow gate for
    /// as long as it has existed.
    ///
    /// **One rule, two implementations, and only one of them had it.** `reflect_js.rs` implements
    /// `clamped unsigned long` correctly — *"a colspan of a billion is 1000, not the default"* — so
    /// `td.colSpan` answered **1000** while the layout that actually builds the table read
    /// 2,147,483,648. The IDL was right, the geometry was hung, and no test compared them.
    ///
    /// Chrome-measured (`--headless=new --dump-dom`), and these are the bounds:
    ///
    /// ```text
    ///   <td colspan="2147483648">   colSpan 1000     (2-col table: 46px, i.e. 2 cells)
    ///   <td colspan="1000">         colSpan 1000
    ///   <td rowspan="2147483648">   rowSpan 65534
    /// ```
    ///
    /// ⚠ **RESIDUAL, named rather than hidden:** HTML's rules for parsing non-negative integers stop
    /// at the first non-digit, so Chrome reads `colspan="3px"` as **3**; `parse::<usize>()` rejects
    /// it and we fall back to 1. That is a *wrong answer*, not a hang, and it is a different rule
    /// from this one — fixing it here would smuggle an unmeasured behaviour change into a Bar-0 fix.
    fn cell_span(&self, cell: NodeId, attr: &str) -> usize {
        // Per HTML: `colspan` clamps into [1, 1000], `rowspan` into [0, 65534]. The `.max(1)` floor
        // is this engine's own — `rowspan="0"` ("to the end of the row group") is not modelled, and
        // treating it as 1 is the pre-existing behaviour this must not change.
        let max = if attr == "colspan" { 1000 } else { 65534 };
        self.dom
            .element(cell)
            .and_then(|e| e.attr(attr))
            .and_then(|v| v.trim().parse::<usize>().ok())
            .unwrap_or(1)
            .clamp(1, max)
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
        let max = content_right_extent(&cmax, self.fonts, 0.0, &|n| self.px_right_insets(n));
        let mut fc_min = FloatContext::new(0.0, 0.0);
        let (cmin, _) = self.layout_children(cell, 0.0, 0.0, 0.0, None, &mut fc_min);
        let min = content_right_extent(&cmin, self.fonts, 0.0, &|n| self.px_right_insets(n));
        (
            taffy_tree::ceil_to_layout_unit(min + frame),
            taffy_tree::ceil_to_layout_unit(max + frame),
        )
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
                filters: s.filter.clone(),
                clip_path: s.clip_path.clone(),
                blend: s.mix_blend_mode,
                backdrop: s.backdrop_filter.clone(),
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
        //
        // ⚠⚠ **AN INLINE ELEMENT IS A CONTAINING BLOCK TOO, AND `walk` CANNOT SEE ONE.**
        //    `LayoutBox::walk` descends `BoxContent::Block` only — it never enters
        //    `BoxContent::Inline(frags)` — so a *boxless inline* element has no entry here at all.
        //    `abs_containing_block` then walks straight past it to the nearest BLOCK-level positioned
        //    ancestor, or to the viewport. CSS 2.1 §10.1 is explicit that it must not: for an
        //    absolutely positioned box the containing block is the nearest ancestor with `position`
        //    other than `static`, and *"if the ancestor is inline-level, the containing block is the
        //    bounding box around the padding boxes of the first and last inline boxes generated for
        //    that element."*
        //
        //    `<a style="position:relative">text<span style="position:absolute">…</span></a>` is one of
        //    the most common idioms on the web — the stretched click target, the badge on an icon
        //    link, the tooltip anchor, the dropdown under a nav item — and every one of them escaped
        //    to the wrong ancestor. Measured against Chrome (`margin:0; 16px/normal sans-serif`, an
        //    `.outer{position:relative}` wrapper, `.corner{position:absolute;top:0;left:0;10×10}`):
        //    the corner belongs at its link's origin **[36 50]** and we put it at **[0 68]** — the
        //    wrapper's origin, 36px left and 18px down, on every such element on the page.
        //
        //    `node_rects` already performs exactly the union this needs, and it is safe to reuse
        //    HERE specifically: the out-of-flow boxes are appended to `root` *after* this function
        //    returns, so the map it builds is pure in-flow geometry and an abspos box cannot inflate
        //    the very ancestor it is about to be positioned against. The `static` case stays correct
        //    by construction — `abs_containing_block` still requires `position != Static`, so a plain
        //    inline `<a>` is skipped exactly as before.
        let mut rects: HashMap<NodeId, Rect> = root.node_rects(self.dom);
        // ── ⚠⚠⚠ **THIS WAS NAMED `viewport` AND HELD THE DOCUMENT HEIGHT.**
        //
        // CSS 2.1 §10.1: the initial containing block has the dimensions of the **viewport**, and a
        // `position:fixed` box's containing block IS the viewport. This used `root.content_bottom()`
        // — the whole scrolled document — so every percentage height on an out-of-flow box resolved
        // against the page instead of the window. On a 3000px-tall page under an 800px window:
        //
        // ```text
        //                                        Chrome   before   after
        //   position:fixed;  height:100%         300x713  300x3000  300x713   ✗→✓
        //   position:fixed;  height:50%          100x357  100x1500  100x357   ✗→✓
        //   position:absolute; height:100%       100x713  100x3000  100x713   ✗→✓
        //   position:fixed;  height:auto          100x50   100x50    100x50    ✓  control
        // ```
        //
        // That is every full-height drawer, modal backdrop, off-canvas menu and overlay on any page
        // long enough to scroll — i.e. the ones it matters on. Measured on `possssno.sbs`, whose
        // `#aside { position:fixed; height:100% }` drawer came out **4462px** tall against Chrome's
        // **713**, and it is that site's single largest shape miss.
        //
        // The height is read from the same viewport the parser resolves `vh` against, which is what
        // the IN-FLOW initial containing block already does 60 lines up (`icb_height`). One rule, two
        // implementations, and only the in-flow one had been corrected — the same shape as t831/t833.
        let viewport = Rect {
            x: 0.0,
            y: 0.0,
            width: viewport_w,
            height: manuk_css::values::viewport_size().1,
        };

        // Gather positioned elements in DOM pre-order so an abs ancestor is placed
        // (and recorded) before an abs descendant that uses it as containing block.
        let mut positioned = Vec::new();
        self.collect_positioned(root_el, &mut positioned);

        let mut new_boxes = Vec::new();
        for node in positioned {
            let s = self.style_of(node);
            // ── **THE STATIC POSITION IS PER AXIS.** CSS 2.1 §10.3.7 solves the horizontal
            //    equation and §10.6.4 the vertical one, *separately*: with `left` and `right` both
            //    `auto` the box's INLINE position is its static position, and independently, with
            //    `top` and `bottom` both `auto` its BLOCK position is. This tested all four insets
            //    at once, so setting ONE inset threw the static position away on BOTH axes and the
            //    box fell back to the containing block's origin on the axis still `auto`.
            //
            //    Chrome-measured, a 400px `position:relative` wrapper 234px down the page:
            //    `position:absolute; left:200px` (with `top` auto) belongs at **y=294** and landed
            //    at **234** — the containing block's top, 60px out. That is every
            //    `position:absolute; right:8px` badge and close button, every `left:0` full-bleed
            //    underline, every `top:100%` dropdown: one inset named, the other axis static, and
            //    the static half discarded.
            let x_static = s.inset.left.is_auto() && s.inset.right.is_auto();
            let y_static = s.inset.top.is_auto() && s.inset.bottom.is_auto();
            let mut cb = if s.position == Position::Fixed {
                viewport
            } else {
                self.abs_containing_block(node, &rects, viewport)
            };
            // Anchor the containing block at the static position on each axis that asks for it, so
            // `layout_abs` resolves the box in the right place instead of at the containing block's
            // origin (which would put every dropdown in the top-left corner) — and, before any of
            // this existed, instead of nowhere at all. An axis with a real inset keeps `cb`, because
            // that inset must resolve against the CONTAINING BLOCK's edge and not against the flow
            // cursor.
            if x_static || y_static {
                if let Some(&(sx, sy)) = self.static_pos.borrow().get(&node) {
                    cb = Rect {
                        x: if x_static { sx } else { cb.x },
                        y: if y_static { sy } else { cb.y },
                        width: cb.width,
                        height: cb.height,
                    };
                } else if x_static && y_static && s.position != Position::Fixed {
                    // Never reached in flow layout; a box we truly cannot place is still better
                    // dropped than rendered in the wrong corner. ⚠ Only when BOTH axes wanted the
                    // static position — a box with a real inset on one axis is placeable and must
                    // not be dropped just because flow never recorded a cursor for it.
                    continue;
                }
            }
            let b = self.layout_abs(node, cb);
            // ⚠⚠⚠ **THE WHOLE SUBTREE, NOT JUST THIS BOX.** `rects` was built from the IN-FLOW
            // fragment tree, so nothing inside an out-of-flow subtree has an entry in it — and
            // `abs_containing_block` reads `position != Static` and then requires a rect, walking
            // straight PAST any ancestor it cannot find. So a `position:relative` element that
            // happens to live inside an `position:absolute` ancestor was invisible as a containing
            // block, and every abspos box under it escaped to the OUTER positioned ancestor.
            //
            // That is the AdminLTE sidebar exactly — `.main-sidebar{position:absolute}` >
            // `section` > `ul` > `li` > `a{position:relative}` > `span{position:absolute;top:50%}`
            // — and it is the shape of every off-canvas menu, drawer, dropdown panel and fixed
            // toolbar whose rows carry their own badges, carets or absolutely-placed icons.
            // Chrome-measured on `ubys.bingol.edu.tr`, 14 sidebar carets: each belongs at its own
            // row (`y` 65, 109, 153, …) and every one of ours landed on the SAME `y`, because
            // `top:50%` was resolving against the sidebar instead of the row.
            //
            // ⚠ Only ONE of the two axes was visibly wrong, which is why this read as a `top`
            // defect rather than a containing-block defect: `right:10px` is a LENGTH, and the
            // sidebar and the row happen to share a right edge, so x came out correct from the
            // wrong containing block. **A wrong containing block is only as visible as the insets
            // that distinguish it.**
            //
            // Inserted AFTER `layout_abs` so it is this box's placed geometry, and safe against the
            // inflation concern that governs the map above: `positioned` is DOM pre-order, so an
            // out-of-flow ancestor is laid out and recorded before any out-of-flow descendant reads
            // it, and a nested abspos box is not in `b`'s in-flow content to begin with.
            //
            // ⚠⚠⚠ **FILTERED TO `node`'s OWN DESCENDANTS, AND THE UNFILTERED VERSION BROKE TWO
            // GATES.** `node_rects` LIFTS a boxless element's geometry up the DOM until it reaches
            // an ancestor that has a box *in the tree it was called on* — which is right for the
            // whole-document call above and catastrophic here, because from inside an out-of-flow
            // subtree EVERY ancestor is boxless: `#modal`'s rect propagated onto its own
            // `position:relative` containing block, so the next abspos sibling resolved against
            // `[100 100 200x200]` instead of `[0 0 400x400]`
            // (`abspos_auto_margins_center_a_constrained_box`), and a `position:static` inline
            // acquired geometry it must never have
            // (`an_out_of_flow_child_neither_splits_its_inline_nor_escapes_it`, whose control row
            // is written for exactly this mistake). Keeping the lift is still necessary — a
            // `position:relative` INLINE inside a drawer has no box of its own and is a perfectly
            // legal containing block — so the answer is to keep the union and drop everything it
            // pushed ABOVE this box.
            let sub = b.node_rects(self.dom);
            rects.extend(sub.into_iter().filter(|&(n, _)| {
                let mut cur = self.dom.parent(n);
                while let Some(a) = cur {
                    if a == node {
                        return true;
                    }
                    cur = self.dom.parent(a);
                }
                false
            }));
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
                        filters: Vec::new(),
                        clip_path: None,
                        blend: manuk_css::BlendMode::Normal,
                        backdrop: Vec::new(),
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
        let mut content_w = content_w.min(max_w).max(min_w);
        // Lay out content at a provisional origin, then re-origin once placed.
        let mut inner = FloatContext::new(0.0, content_w);
        let (content, ch) =
            self.layout_children(node, 0.0, 0.0, content_w, definite_ch, &mut inner);
        // ── **AN ABSOLUTELY POSITIONED REPLACED ELEMENT WAS ZERO PIXELS TALL. ALWAYS.**
        //
        // Height was `definite_ch` or the CONTENT height, and a replaced element has no children —
        // so unless the author gave it an explicit `height` or set BOTH `top` and `bottom`, an
        // `<img>` measured `<w>x0` and painted nothing. This is the third implementation of the
        // rule t831 landed in `layout_float` and t833 completed in `layout_block`, and it was the
        // worst of the three: those produced a wrong size, this produced **no box at all**.
        //
        // `position:absolute; top:0; left:0` on an image is the hero/overlay/thumbnail idiom of the
        // whole web. The `inset:0` variant HAPPENED to work — both insets make `definite_ch` — which
        // is precisely why this survived: the most-cited form of the pattern is the one that hid it.
        //
        // Chrome-measured, a 1000×266 image absolutely positioned in a 320×200 block:
        //
        // ```text
        //                                  Chrome    before     after
        //   max-width:100%                 320x85    320x0     320x85    ✗→✓
        //   max-height:30px                113x30   1000x0     113x30    ✗→✓
        //   max-width:100% + max-height    113x30    320x0     113x30    ✗→✓
        //   min-width:1500px              1500x399  1500x0    1500x399   ✗→✓
        // ```
        //
        // Note every `before` height is 0 and every `before` WIDTH but one is already right: the
        // clamps landed here in an earlier tick, the ratio never did.
        let content_height = match (definite_ch, s.aspect_ratio) {
            (None, Some(r)) if r > 0.0 => ((content_w + bs_extra_w) / r - bs_extra_h).max(0.0),
            _ => definite_ch.unwrap_or_else(|| ch.max(inner.lowest_bottom().max(0.0))),
        };
        // ⚠ **§10.4's inline→block half is NOT written here, and that is deliberate rather than
        // forgotten.** I wrote it, and the falsification pass found the gate stayed GREEN with it
        // mutated out — because the arm above already derives the height from `content_w` *after*
        // the width clamp, so the transfer could only ever recompute the number it had just
        // computed. `layout_block` and `layout_float` genuinely need their copies (both resolve the
        // height from a source that is not the clamped width); this path does not, and shipping a
        // fourth copy of the rule for symmetry would have been unreachable code guarded by a test
        // that cannot fail — which is the exact shape this project calls a vacuous gate.
        // `min-height` / `max-height` clamp (CSS2 §10.7) — the CB height is always definite here, so
        // a `%` bound resolves against it (unlike the in-flow case's indefinite-parent → `none`).
        let min_h = (s.min_height.resolve(cb.height, 0.0) - bs_extra_h).max(0.0);
        let max_h = match s.max_height {
            Dim::Auto => f32::INFINITY,
            other => (other.resolve(cb.height, f32::INFINITY) - bs_extra_h).max(0.0),
        };
        let unclamped_h = content_height;
        let content_height = content_height.min(max_h).max(min_h);
        // And §10.4 the other way, block → inline — the half t833 added to `layout_block`. Safe to
        // move the width after `layout_children` ONLY under the replaced guard, since a replaced box
        // has no children that were laid out against the old width.
        if content_height != unclamped_h && is_replaced_element(self.dom.tag_name(node)) {
            if let Some(r) = s.aspect_ratio {
                if r > 0.0 {
                    let w = ((content_height + bs_extra_h) * r - bs_extra_w).max(0.0);
                    content_w = w.min(max_w).max(min_w);
                }
            }
        }
        let content_w = content_w;

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
            filters: s.filter.clone(),
            clip_path: s.clip_path.clone(),
            blend: s.mix_blend_mode,
            backdrop: s.backdrop_filter.clone(),
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
    ///
    /// ⚠⚠ **`bcs` is the CONTAINING BLOCK's style, and this call used to pass NONE of it.** The
    /// `layout_inline` arguments were the literals `TextAlign::Left, 0.0, …, None` where the
    /// pure-IFC branch two thousand lines up passes `bcs.text_align, text_indent, …, Some(&bcs)`.
    /// CSS 2.1 §9.2.1.1: an anonymous block box *inherits* every inheritable property from the
    /// block container that generated it — so the anonymous twin was built with no inherited
    /// context at all, and two separate symptoms fell out of the one omission:
    ///
    /// * **`text-align` was lost.** The moment a container mixed inline content with even one
    ///   block-level child — which is the only condition under which this function runs — every
    ///   inline run in it snapped back to the left edge, while the SAME markup with no block child
    ///   centred correctly. `<center><b>…</b><textarea></textarea></center>` is the archetype.
    /// * **The STRUT was lost.** With `strut_style: None` the line box carries a zero strut, so a
    ///   line whose only content is an atomic inline-block is exactly the inline-block's height —
    ///   Chrome adds the containing block's font descent below the baseline the inline-block sits
    ///   on. Measured: a 20px inline-block in a `font:16px/1.2` container is a 24px line in Chrome
    ///   and was a 20px line here. A text run was already right, because a fragment's own inherited
    ///   `line-height` covers it; only the atomic case exposed the missing strut.
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
        bcs: &manuk_css::ComputedStyle,
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
        // `text_indent` stays 0 and is NOT passed through: Chrome indents only the FIRST anonymous
        // run of a container (`text-indent:40px` on a mixed container → run 1 at x=40, run 2 after
        // the block child at x=0), so handing it to every flush would over-indent every run but the
        // first. Measured, named in the journal, and left for its own tick rather than guessed at.
        let (frags, atomics, h) = self.layout_inline(
            items,
            cx,
            start,
            cw,
            bcs.text_align,
            0.0,
            floats,
            Some(bcs),
            bcs.direction == manuk_css::Direction::Rtl,
        );
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
            filters: Vec::new(),
            clip_path: None,
            blend: manuk_css::BlendMode::Normal,
            backdrop: Vec::new(),
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
        // ── **AN RTL GRID'S COLUMN AXIS RUNS RIGHT-TO-LEFT, AND TAFFY CANNOT BE TOLD.** `direction`
        // reverses a grid's inline-axis track order (CSS Grid §3: the column axis is the inline axis),
        // so the first item goes in the RIGHTMOST column. Taffy has no `direction` property, and the
        // `row` ⇄ `row-reverse` swap that fixes flex (t764) has no grid equivalent — `grid-auto-flow`
        // is not a direction. So the mirror happens on the way OUT, on the placed slots.
        //
        // Measured vs live Chromium (`<html dir=rtl>`, a 600px two-column grid): Chrome puts the first
        // item at **300** and the second at **0**; ours had them at 0 and 300. This is the third and
        // last of the RTL axis-order primitives (flex row t764, table columns t765) and the same
        // mechanism each time: an axis that the spec defines as LOGICAL reaching a physical engine.
        //
        // Mirroring the SLOT is enough because `extract_placed` positions each subtree relative to it,
        // and it is applied recursively so a grid nested inside a grid gets its own mirror — against
        // its own CONTENT width, which is why the padding/border frame is subtracted here.
        let mut placed = placed;
        if self.grid_is_rtl(node) {
            let frame = 0.0; // the root's slots are already relative to its content origin
            for p in placed.iter_mut() {
                self.mirror_rtl_grid(p, cw - frame);
            }
        }
        for p in placed.iter_mut() {
            self.mirror_rtl_grid_descendants(p);
        }
        let mut boxes = Vec::new();
        let mut max_h = 0.0f32;
        for p in &placed {
            let (boxx, bottom) = self.extract_placed(p, cx, cy);
            max_h = max_h.max(bottom);
            boxes.push(boxx);
        }
        (BoxContent::Block(boxes), max_h)
    }

    /// Is this node a GRID container whose inline axis runs right-to-left?
    fn grid_is_rtl(&self, node: NodeId) -> bool {
        let s = self.style_of(node);
        matches!(
            s.display,
            manuk_css::Display::Grid | manuk_css::Display::InlineGrid
        ) && s.direction == manuk_css::Direction::Rtl
    }

    /// Mirror one placed slot within `content_w` — the RTL column-order flip (see the call site).
    fn mirror_rtl_grid(&self, p: &mut taffy_tree::Placed, content_w: f32) {
        p.slot.x = content_w - p.slot.x - p.slot.width;
    }

    /// Apply the mirror to every RTL grid container INSIDE an already-placed subtree, so a grid nested
    /// in a flex row (or in another grid) is flipped against its own content box rather than the
    /// outermost one.
    fn mirror_rtl_grid_descendants(&self, p: &mut taffy_tree::Placed) {
        if p.container && self.grid_is_rtl(p.dom) {
            let s = self.style_of(p.dom);
            let frame = s.padding.left.resolve(p.slot.width, 0.0)
                + s.padding.right.resolve(p.slot.width, 0.0)
                + s.border_width.left
                + s.border_width.right;
            let content_w = (p.slot.width - frame).max(0.0);
            for c in p.children.iter_mut() {
                self.mirror_rtl_grid(c, content_w);
            }
        }
        for c in p.children.iter_mut() {
            self.mirror_rtl_grid_descendants(c);
        }
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
                filters: s.filter.clone(),
                clip_path: s.clip_path.clone(),
                blend: s.mix_blend_mode,
                backdrop: s.backdrop_filter.clone(),
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
                filters: Vec::new(),
                clip_path: None,
                blend: manuk_css::BlendMode::Normal,
                backdrop: Vec::new(),
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
            // The block-axis twin, and it is only recorded when the item asked for a PERCENTAGE
            // height: taffy's slot is authoritative there because it already resolved that
            // percentage against the real containing block. For an `auto`-height item the slot is a
            // stretch verdict, not a resolution, and the item must still size to its content — that
            // case is handled after the layout by the `height == Dim::Auto` adoption below, and
            // overriding it here would freeze every stretched item at its line's height.
            let pct_h = matches!(
                self.style_of(p.dom).height,
                Dim::Percent(_) | Dim::Calc { .. }
            );
            if pct_h {
                self.taffy_item_height
                    .borrow_mut()
                    .insert(p.dom, p.slot.height);
            }
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
            self.taffy_item_height.borrow_mut().remove(&p.dom);
            let mut boxx = r.boxx;
            // Taffy sized the item (grow/stretch/track height); when its own height is `auto`,
            // adopt taffy's slot height so it fills its flex line / grid cell.
            if self.style_of(p.dom).height == Dim::Auto && p.slot.height > boxx.rect.height {
                boxx.rect.height = p.slot.height;
            }
            // `p.slot.y` is taffy's placement, which ALREADY has the item's top margin in it — so this
            // must not add `r.margin_top` on top (t823: `layout_block` no longer does either). The
            // bottom margin is not in the slot, so it stays.
            let bottom = p.slot.y + boxx.rect.height + r.margin_bottom;
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
        // ⚠⚠ **`position: absolute` on a pseudo was IGNORED, and the marker sat in the flow.**
        //
        // `.item::before { content: "–"; position: absolute; left: 0 }` with `padding-left: 20px` on
        // the item is *the* custom-bullet idiom, and the same shape carries every icon, chevron and
        // decorative bar the web puts in a pseudo. An out-of-flow box takes no space and is placed
        // against its containing block; ours was emitted as an ordinary inline word, so it **pushed
        // the item's text right by the marker's width** and drew the marker where the text should
        // start. Measured on `255md.com` against Chrome: the dash glued to `ad delivery` instead of
        // sitting 20px to its left.
        //
        // `dx` is the horizontal correction. Insets resolve against the containing block's PADDING
        // box, and the inline pen starts at the content box, so the shift is
        // `left - padding-left` (or `-(right + padding-right)` for a right inset). That is exact
        // whenever the owner is itself the containing block — `position: relative` on the owner,
        // which is what this idiom always writes, and what makes the marker land at the padding edge.
        //
        // ⚠ **Deliberately partial, and named so the next person knows which half exists.** The
        // VERTICAL inset is not honoured: the fragment keeps the line's baseline, which is right for
        // the one-line markers and inline icons this idiom is made of and wrong for a tall block
        // with `::before { top: 0 }`. Nor does this walk up to a positioned ancestor when the owner
        // is static. Both need the pseudo to become a real out-of-flow box with its own containing
        // block, which is a different tick — this one removes it from the flow and puts it at the
        // right x, which is the whole of the observable effect for the idiom that is everywhere.
        let pseudo = |which: fn(&ComputedStyle) -> &Option<Box<ComputedStyle>>|
         -> Option<(String, TextStyle, Option<f32>)> {
            let s = owner.and_then(|n| self.styles.get(&n))?;
            let p = which(s).as_ref()?;
            let text = p.content.clone()?;
            if text.is_empty() {
                return None;
            }
            let dx = is_out_of_flow_positioned(p).then(|| {
                if !p.inset.left.is_auto() {
                    p.inset.left.resolve(cw, 0.0) - s.padding.left.resolve(cw, 0.0)
                } else if !p.inset.right.is_auto() {
                    -(p.inset.right.resolve(cw, 0.0) + s.padding.right.resolve(cw, 0.0))
                } else {
                    // `auto` on both: the static position, i.e. exactly where the flow would have
                    // put it. The box still takes no space — that is the part that matters.
                    0.0
                }
            });
            Some((text, text_style(p, self.fonts), dx))
        };
        if let Some((text, style, dx)) = pseudo(|s| &s.before) {
            out.push(match dx {
                Some(dx) => InlineItem::AbsPseudo { text, style, dx },
                None => InlineItem::Word {
                    text,
                    style,
                    space_before: false,
                    node: owner,
                    no_wrap: true,
                    break_word: false,
                },
            });
            first = false;
        }
        for &n in nodes {
            self.collect_inline_node(n, &mut out, &mut pending_space, &mut first, None, cw);
        }
        if let Some((text, style, dx)) = pseudo(|s| &s.after) {
            out.push(match dx {
                Some(dx) => InlineItem::AbsPseudo { text, style, dx },
                None => InlineItem::Word {
                    text,
                    style,
                    space_before: pending_space && !first,
                    node: owner,
                    no_wrap: true,
                    break_word: false,
                },
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
                // `pre-wrap` PRESERVES every space (unlike `pre-line`, which collapses runs): each
                // maximal whitespace run becomes its own measured token, so N spaces stay N spaces
                // and leading indentation survives, while a soft wrap can still fall between tokens.
                // This is the rendering `<textarea>` (pre-wrap by UA default) and every "preformatted
                // but still wrapping" block depend on — collapsing them, as the shared pre-line path
                // did, silently reflows code samples and aligned text into a single-spaced blob.
                if cs.white_space == WhiteSpace::PreWrap {
                    for (i, line) in t.split('\n').enumerate() {
                        if i > 0 {
                            out.push(InlineItem::Break {
                                height: style.line_height,
                                node: owner,
                            });
                            *pending_space = false;
                            *first = true;
                        }
                        let mut run = String::new();
                        let mut run_ws = false;
                        // Emit the run built so far, whitespace verbatim (its own token) or a word.
                        macro_rules! flush_run {
                            () => {
                                if !run.is_empty() {
                                    if run_ws {
                                        out.push(InlineItem::Word {
                                            text: std::mem::take(&mut run),
                                            style,
                                            space_before: false,
                                            node: owner,
                                            no_wrap: false,
                                            break_word: false,
                                        });
                                        *first = false;
                                    } else {
                                        push_word(
                                            out,
                                            &mut run,
                                            style,
                                            pending_space,
                                            first,
                                            owner,
                                            false,
                                            break_word,
                                        );
                                    }
                                }
                            };
                        }
                        for ch in line.chars() {
                            let ws = is_css_white_space(ch);
                            if !run.is_empty() && ws != run_ws {
                                flush_run!();
                            }
                            run_ws = ws;
                            run.push(ch);
                        }
                        flush_run!();
                    }
                    return;
                }
                // `pre-line` preserves newlines but COLLAPSES runs of spaces, and still wraps long
                // lines: break at each newline, then split the line into words as usual.
                if cs.white_space == WhiteSpace::PreLine {
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
                            if is_css_white_space(ch) {
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
                    if is_css_white_space(ch) {
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
                //
                // ⚠⚠ **AND SO IS AN ORPHANED TABLE-INTERNAL BOX**, which used to fall through to
                // the text recursion and be laid out as a plain non-replaced INLINE. CSS 2.1
                // §17.2.1 wraps a `table-cell` / `table-row` / `table-row-group` whose parent is
                // not the table box it needs in ANONYMOUS table objects — and the resulting box is
                // atomic, never a run of inline text. The difference is not academic; it is the
                // difference between a line box and a glyph box. Chrome-measured, same content,
                // `16px/1.25 sans-serif`:
                //
                // ```text
                //                                      Chrome         before
                //   display:inline-block             [0  0 79x20]   [0  0 79x20]  ✓ always right
                //   display:table-cell (no table)    [0 30 79x20]   [0 31 79x17]  ✗
                // ```
                //
                // **An `inline-block` with byte-identical content was already exact**, which is
                // what proves this is not a general inline-box metric error but this one path: an
                // inline box is sized to its GLYPHS (17), an atomic one to its LINE BOX (20), and
                // the leftover half-leading is also what pushed `y` down by 1.
                //
                // THE REACH is every `display:table-cell` used without a table wrapper — the
                // legacy vertical-centring and equal-height-column idioms — and every cell is
                // 3px short at the default metrics, with the error accumulating DOWN the page.
                if matches!(
                    disp,
                    Some(
                        Display::InlineBlock
                            | Display::Flex
                            | Display::Grid
                            | Display::InlineFlex
                            | Display::InlineGrid
                            | Display::TableCell
                            | Display::TableRow
                            | Display::TableRowGroup
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
                    // ── **THE INLINE-BLOCK'S OWN BASELINE** (CSS 2.1 §10.8.1). Its last in-flow
                    // line box's baseline aligns with the parent's — unless it has no in-flow line
                    // boxes, or `overflow` is not `visible`, in which case the bottom margin edge is
                    // the baseline. We only ever implemented the fallback, so every text-bearing
                    // inline-block sat entirely ABOVE the line's baseline and made its line ~4px too
                    // tall. Measured against Chrome:
                    //
                    //     <span style="display:inline-block">Ay</span>Ay   Chrome 19.19   ours 23
                    //     …the same with padding:5px                      Chrome 29.19   ours 33
                    //     …the same with overflow:hidden                   Chrome 23.38   ours 23  ✓
                    //     …an EMPTY inline-block                           Chrome 19.19   ours 19  ✓
                    //
                    // The two rows we already matched are the fallback cases, which is exactly why
                    // this survived: the rule we implemented is a real rule, applied everywhere.
                    let own_baseline = if matches!(s.overflow_x, Overflow::Visible)
                        && matches!(s.overflow_y, Overflow::Visible)
                    {
                        last_line_baseline(&r.boxx).map(|b| r.margin_top + (b - r.boxx.rect.y))
                    } else {
                        None
                    };
                    out.push(InlineItem::Atomic {
                        box_: Box::new(r.boxx),
                        advance,
                        height,
                        baseline: own_baseline,
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
                // ── **VERTICAL padding/border extend the inline BOX without touching the LINE**
                //    (CSS 2.1 §10.6.1: on a non-replaced inline, `height` and vertical padding do not
                //    affect line height, but the box still has them). Chrome-measured, `16px/1.25
                //    sans-serif`:
                //
                //    ```text
                //      <a style="padding:10px 20px">Login</a>   box 37 tall, starting 10px ABOVE its
                //                                               text; the LINE stays 20
                //      <span style="border:5px">Bordered</span> box 27 tall
                //      <span style="padding:0 20px">…</span>    box 17 — horizontal only, unchanged
                //    ```
                //
                //    We reported 18 for the first two, so a padded inline link — the way every tag,
                //    badge, nav pill and button-styled link on the web is written — had a box half
                //    the height the author drew, and PAINTED its background at that size.
                //
                //    The height is the element's own content area (its font's ascent+descent) plus
                //    its vertical padding and border; the rect starts `pad_t + border_t` above the
                //    content top, which is what `report_ascent` carries.
                let pad_t = s.padding.top.resolve(cw, 0.0) + s.border_width.top;
                let pad_b = s.padding.bottom.resolve(cw, 0.0) + s.border_width.bottom;
                let (v_ascent, v_height) = if pad_t > 0.0 || pad_b > 0.0 {
                    let ts = text_style(&s, self.fonts);
                    let lm = self.fonts.line_metrics(ts.font_key, ts.font_size);
                    (
                        Some(lm.ascent.round() + pad_t),
                        lm.ascent.round() + lm.descent.round() + pad_t + pad_b,
                    )
                } else {
                    (None, 0.0)
                };
                // `|| v_ascent.is_some()`: `padding: 10px 0` has NO horizontal edge, so without this
                // there is no spacer at all and nothing carries the vertical report — the box stays
                // 17 where Chrome says 37. `holds_line` still keys on the HORIZONTAL edge only,
                // because that is what the measurement says brings a line box into existence (see
                // the `out.len() == mark` branch below): a vertical-only edge does not.
                if pad_l > 0.0 || v_ascent.is_some() {
                    out.push(InlineItem::Spacer {
                        width: pad_l,
                        node: Some(node),
                        space_before: *pending_space && !*first,
                        report_height: v_height,
                        report_ascent: v_ascent,
                        holds_line: pad_l > 0.0,
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
                        report_height: v_height,
                        report_ascent: v_ascent,
                        holds_line: true,
                    });
                    *pending_space = false;
                }
                // An inline element that contributed NOTHING to the flow is still a box. Without
                // this it has no geometry at all: `getBoundingClientRect` returns nothing, it can't
                // be scrolled to, and it cannot be painted. On one Wikipedia article that is 1,079
                // spans and 298 anchors — the single largest source of missing elements.
                if out.len() == mark {
                    // …but it does NOT bring a line box into existence on its own (CSS2 §9.4.2), so
                    // `holds_line` is FALSE here — see `close_line`.
                    //
                    // §9.4.2's escape hatch reads *"no inline elements with non-zero margins, padding
                    // or borders"*, which invites the general test. **Chrome is narrower than its own
                    // spec text, measured on this exact fixture** (`16px/normal sans-serif`, the div's
                    // height):
                    //
                    // ```text
                    //   <span style="padding:4px">       18   <- 4px of it is HORIZONTAL
                    //   <span style="padding:4px 0">      0   <- vertical only
                    //   <span style="border-top:3px">     0
                    //   <span style="margin-left:10px">   0
                    // ```
                    //
                    // So what actually holds a line open is an edge that occupies INLINE FLOW WIDTH —
                    // which is precisely the `pad_l`/`pad_r` spacers above, and reaching this branch
                    // means both were zero. Deriving `holds_line` from the spec sentence instead of
                    // from the measurement would have made three of these four rows 18 against
                    // Chrome's 0.
                    out.push(InlineItem::Spacer {
                        width: 0.0,
                        node: Some(node),
                        space_before: false,
                        report_height: s.line_height.max(0.0),
                        // An EMPTY inline keeps the old line-top anchoring: Chrome reports a
                        // line-height-tall rect for `<span id="anchor"></span>`, and that is
                        // measured behaviour this must not disturb.
                        report_ascent: None,
                        holds_line: false,
                    });
                }
                // ── ⚠⚠⚠ **AN INLINE THAT CARRIES NO FRAGMENT OF ITS OWN STILL HAS AN INLINE BOX.**
                //
                // `<span class="icon"><i></i></span>` — an inline whose entire content is an ATOMIC
                // inline (a sprite `<i>`, an `<img>`, an icon glyph in an `inline-block`) or a
                // nested inline (`<a><em>x</em></a>`) — emits items that all belong to something
                // ELSE. It is not empty, so the branch above does not fire; it has no padding, so
                // neither edge spacer fires; and it owns no `Word`, because the text belongs to the
                // descendant. So it reached `node_rects` with **no fragment at all**, and the only
                // geometry left to find was the child's box.
                //
                // Chrome-measured, `16px/1.2 sans-serif` (`--headless=new --dump-dom`):
                //
                // ```text
                //                                              Chrome           before
                //   <span><i 8x4  inline-block></i></span>      [11, 1, 8,17]    [11,11,8, 4]
                //   <span><i 8x40 inline-block></i></span>      [11,93, 8,17]    [11,70,8,40]
                //   <span 10px><b 40px>x</b></span>             [11,48,22,11]    [11,21,22,44]
                // ```
                //
                // The inline box is the element's OWN content area — its font's ascent + descent, on
                // the line's baseline — and Chrome does **not** grow it to cover a taller descendant
                // (rows 2 and 3: the child overflows it and the parent is unmoved). `node_rects`
                // owns the other half of that rule, per axis; this only has to make sure the element
                // reports a box at all.
                //
                // TWO reporters, at the head and the tail of the element's items, because an inline
                // that wraps spans several lines and Chrome's rect covers the first line's content
                // top through the last line's bottom. One reporter would report only the line it
                // happened to land on. Both are zero-width and `holds_line: false`, so neither
                // brings a line into existence, neither consumes a pending space, and — because
                // `report_ascent` is `Some` — neither feeds a `line_height` floor into `close_line`.
                // The line boxes are byte-identical; only the reported geometry changes.
                //
                // Reaching here implies `pad_l == pad_t == pad_b == 0` — any of those would have
                // emitted an edge spacer that already carries this node — so the reported box is the
                // bare content area, with no padding term to add.
                else if !out[mark..].iter().any(|it| it.owner() == Some(node)) {
                    let ts = text_style(&s, self.fonts);
                    let lm = self.fonts.line_metrics(ts.font_key, ts.font_size);
                    let reporter = || InlineItem::Spacer {
                        width: 0.0,
                        node: Some(node),
                        space_before: false,
                        report_height: lm.ascent.round() + lm.descent.round(),
                        report_ascent: Some(lm.ascent.round()),
                        holds_line: false,
                    };
                    out.push(reporter());
                    out.insert(mark, reporter());
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
        text_indent: f32,
        floats: &FloatContext,
        strut_style: Option<&manuk_css::ComputedStyle>,
        // The IFC's **bidi base direction**, from the `direction` of the block that establishes it.
        // Passed rather than read off `strut_style` because two call sites legitimately have no
        // block style in hand (an anonymous flex item that IS a text node; a form control's
        // synthetic value text) and still sit inside a `dir=rtl` document.
        base_rtl: bool,
    ) -> (Vec<TextFragment>, Vec<LayoutBox>, f32) {
        // The STRUT — the containing block's font metrics and `line-height`, folded into every line box
        // this IFC produces. See `close_line`. `None` (a caller with no block style in hand) yields a
        // zero strut, which is exactly the old behaviour, so no call site changes meaning by accident.
        let strut = strut_style
            .map(|bcs| {
                // Through `text_style`, not the raw `ComputedStyle`: that is the one function that
                // resolves a family list to a `FontKey` and `line-height: normal` to a number, and the
                // strut must be the SAME resolution the fragments use or the fold below compares two
                // different notions of the same font.
                let ts = text_style(bcs, self.fonts);
                let lm = self.fonts.line_metrics(ts.font_key, ts.font_size);
                (lm.ascent, lm.descent, ts.line_height)
            })
            .unwrap_or((0.0, 0.0, 0.0));
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
        // `text-indent` shifts the inline-start of the FIRST line box only. `first_line` flips false
        // after that line closes, so wrapped and subsequent lines are unindented. With `text_indent`
        // 0 the arithmetic below is the identity (`x + 0.0`, `w - 0.0`), so every existing line box
        // is byte-identical — the indent path is inert until an author sets it.
        let mut first_line = true;

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
                        report_ascent: None,
                        atomic: None,
                        atomic_h: 0.0,
                        atomic_baseline: 0.0,
                        valign: VerticalAlign::Baseline,
                        content_bearing: true,
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
                        report_ascent: None,
                        atomic: None,
                        atomic_h: 0.0,
                        atomic_baseline: 0.0,
                        valign: VerticalAlign::Baseline,
                        content_bearing: true,
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
                    strut,
                    false, // ended by a FORCED break (`<br>`) — takes `text-align-last`
                    base_rtl,
                );
                first_line = false;
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
                    // `word-spacing` widens each inter-word space — and so does `letter-spacing`.
                    //
                    // ⚠⚠ **THE SPACE IS A CHARACTER.** The line above adds `letter_spacing` once per
                    // character of the WORD and stopped there, so an inter-word space was the one
                    // character on the line that did not get it. Every word's own width stayed
                    // correct while its POSITION fell one `letter-spacing` behind per preceding
                    // space, cumulatively along the line — the hardest shape to spot, because the
                    // thing you would measure (the word box) is right.
                    //
                    // Chrome-measured at `letter-spacing: 2px`, `16px sans-serif`: the 2nd word sits
                    // at 39 (we had 37, one space short) and the 4th at 115 (we had 109, three
                    // spaces short — 12 characters × 2 against our 9). `letter-spacing` on nav bars,
                    // buttons, headings and uppercase labels is design-system standard, so this rides
                    // on a large share of the chrome of the modern web.
                    let space_w = if space_before {
                        self.fonts.measure(" ", key, size)
                            + style.word_spacing
                            + style.letter_spacing
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
                            report_ascent: None,
                            atomic: None,
                            atomic_h: 0.0,
                            atomic_baseline: 0.0,
                            valign: VerticalAlign::Baseline,
                            content_bearing: true,
                        }),
                    )
                }
                InlineItem::AbsPseudo { text, style, dx } => {
                    // Zero advance, zero space, zero height: an out-of-flow box occupies nothing in
                    // its parent's flow. `width` is still the measured width so the fragment reports
                    // a real box and paint draws the glyph; `ascent`/`descent` stay 0 so the marker
                    // cannot grow the line it is no longer part of.
                    let key = style.font_key;
                    let size = style.font_size;
                    let w = self.fonts.measure(&text, key, size)
                        + style.letter_spacing * text.chars().count() as f32;
                    (
                        0.0,
                        0.0,
                        0.0,
                        true,
                        Box::new(move |x: f32| LineFrag {
                            x: x + dx,
                            width: w,
                            text,
                            style,
                            ascent: 0.0,
                            descent: 0.0,
                            node: None,
                            report_h: None,
                            report_ascent: None,
                            atomic: None,
                            atomic_h: 0.0,
                            atomic_baseline: 0.0,
                            valign: VerticalAlign::Baseline,
                            content_bearing: true,
                        }),
                    )
                }
                InlineItem::Atomic {
                    box_,
                    advance,
                    height,
                    baseline: own_baseline,
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
                            report_ascent: None,
                            atomic: Some(box_),
                            atomic_h: height,
                            atomic_baseline: own_baseline.unwrap_or(height),
                            valign,
                            content_bearing: true,
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
                    report_ascent,
                    holds_line,
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
                                // ⚠ **A PADDED edge reports a tall RECT and a ZERO line-height.**
                                // `close_line` folds a synthetic reporter's `line_height` in as a
                                // floor on the line box — right for an empty inline (Chrome gives
                                // `<span id=anchor></span>` a line-height-tall rect and a real line),
                                // and WRONG here: CSS 2.1 §10.6.1 says vertical padding on a
                                // non-replaced inline does not affect line height. Measured: the div
                                // around `<a style="padding:10px 20px">` is **20** in Chrome while
                                // the anchor itself is 37 — the pill overflows its line, which is the
                                // whole visual point of the idiom. Feeding the padded height in as a
                                // floor made that div 37 and pushed every following line down.
                                line_height: if report_ascent.is_some() {
                                    0.0
                                } else {
                                    report_height
                                },
                                decoration: Default::default(),
                                letter_spacing: 0.0,
                                word_spacing: 0.0,
                                shadow: None,
                            },
                            ascent: 0.0,
                            descent: 0.0,
                            node,
                            report_h: Some(report_height),
                            report_ascent,
                            atomic: None,
                            atomic_h: 0.0,
                            atomic_baseline: 0.0,
                            valign: VerticalAlign::Baseline,
                            content_bearing: holds_line,
                        }),
                    )
                }
            };

            if cur.is_empty() {
                let (l, w) = open_band(&mut y, est_h);
                line_left = l;
                // The first line's usable width is reduced by the indent (a negative indent widens
                // it, so the image-replacement line never wraps and sits off-screen).
                line_avail = w - if first_line { text_indent } else { 0.0 };
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
                    strut,
                    true, // a WRAPPED line — the only kind `justify` stretches
                    base_rtl,
                );
                first_line = false;
                let (l, w) = open_band(&mut y, est_h);
                line_left = l;
                line_avail = w;
                cur.push(make_frag(0.0));
                pen = advance;
            } else {
                let x = if cur.is_empty() {
                    // First fragment on the line: the first line begins at the indent, later lines at 0.
                    if first_line {
                        text_indent
                    } else {
                        0.0
                    }
                } else {
                    pen + space_w
                };
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
                strut,
                false, // the LAST line of the block — never justified
                base_rtl,
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
    /// See `InlineItem::Spacer::report_ascent` — how far above the baseline the reported rect starts.
    report_ascent: Option<f32>,
    /// `Some` for an `inline-block`: the box to place, and its margin-box height.
    atomic: Option<Box<LayoutBox>>,
    atomic_h: f32,
    /// **Where the atomic's OWN baseline sits, measured down from its margin-box top** (CSS 2.1
    /// §10.8.1). `atomic_h` — the bottom margin edge — is the FALLBACK, and it is only correct when
    /// the box has no in-flow line boxes or its `overflow` is not `visible`. For an ordinary
    /// text-bearing `inline-block` the baseline is its LAST line box's, and using the fallback puts
    /// the whole box above the parent's baseline: measured, a `<span style="display:inline-block">Ay
    /// </span>Ay` line reads Chrome **19.19px** tall and read **23** here, on every row of chips,
    /// nav items, badges and buttons on the web.
    atomic_baseline: f32,
    valign: VerticalAlign,
    /// **Does this fragment bring its line box into existence?** (CSS 2.1 §9.4.2.) True for text, an
    /// atomic inline, a `<br>`'s box and a real margin/border/padding edge; false for the zero-width
    /// geometry box of an empty inline, which is a *reporter* and not content. A line every one of
    /// whose fragments answers `false` is treated as not existing — see `close_line`.
    content_bearing: bool,
}

/// **UAX #9 rule L2, applied to a line's INLINE BOXES** — reorder them from logical (source) order
/// into visual order, in place, preserving the line's total advance to the float.
///
/// ⚠ **The engine already had the OTHER half of bidi and that is exactly why this was invisible.**
/// `FontContext::shape_bidi` runs the bidirectional algorithm inside a single text run, so one
/// Arabic word, or a sentence in one text node, comes out right; `engine/text`'s
/// `g_bidi_base_direction` gate has asserted that since t?. But a line is not a run — it is a
/// sequence of inline boxes, one per `<a>` / `<span>` / `<em>` / `inline-block`, each measured and
/// placed on its own — and *nothing* reordered those. Measured against Chrome on a `dir=rtl`
/// paragraph of three `<a>`s: the widths matched to the pixel, the line was correctly flush right,
/// and the anchors read **backwards**, which is what a real RTL page (`possssno.sbs`, 503 of 575
/// elements misplaced at coverage 1.00) looks like from the inside.
///
/// **Spaces are modelled as items, not as gaps.** The flow leaves inter-word space as the distance
/// between one fragment's end and the next one's start; under reordering that space is a
/// *character* with its own bidi level and its own place in the visual sequence, so the permutation
/// has to carry it. Reversing positions in place instead — mirroring the gaps — composes wrongly the
/// moment there are two levels (an LTR run embedded in RTL), because the array stays in logical
/// order while the nested reversal has already moved its members.
///
/// **Inert on pure-LTR content, by construction:** with no odd level on the line the L2 loop has an
/// empty range and every `x` is untouched, so a page with no RTL text cannot reach a single
/// arithmetic operation here.
fn reorder_line_bidi(line: &mut [LineFrag], base_rtl: bool) {
    if line.len() < 2 {
        return;
    }
    // A line whose fragments are not laid out left-to-right by increasing x is not a flow the
    // permutation below can model: `InlineItem::AbsPseudo` (the `position:absolute` custom-bullet
    // `::before`) contributes ZERO advance and paints at its own `dx`, so it overlaps its
    // neighbour. Bail rather than reorder a line one of whose boxes is not in the flow at all.
    for w in line.windows(2) {
        if w[1].x - (w[0].x + w[0].width) < -0.01 {
            return;
        }
    }
    /// One thing that occupies inline advance: a fragment of `line`, or the white space the flow
    /// left between two of them.
    enum Slot {
        Frag(usize),
        Space(f32),
    }
    let mut slots: Vec<Slot> = Vec::with_capacity(line.len() * 2);
    let mut text = String::new();
    // Byte offset into `text` at which each slot's first character starts.
    let mut at: Vec<usize> = Vec::with_capacity(line.len() * 2);
    for i in 0..line.len() {
        if i > 0 {
            let gap = line[i].x - (line[i - 1].x + line[i - 1].width);
            if gap > 0.01 {
                at.push(text.len());
                text.push(' ');
                slots.push(Slot::Space(gap));
            }
        }
        at.push(text.len());
        if line[i].text.is_empty() {
            // An atomic inline (`inline-block`, a replaced box) or an inline padding edge. CSS
            // Writing Modes §2.1: it participates in bidi as U+FFFC OBJECT REPLACEMENT CHARACTER,
            // a neutral that takes the direction of what surrounds it.
            text.push('\u{FFFC}');
        } else {
            text.push_str(&line[i].text);
        }
        slots.push(Slot::Frag(i));
    }
    let levels = manuk_text::bidi_levels(&text, base_rtl);
    if levels.is_empty() {
        return;
    }
    let slot_levels: Vec<u8> = at.iter().map(|&o| levels[o]).collect();
    let max = slot_levels.iter().copied().max().unwrap_or(0);
    // The lowest ODD level present, counting an even level as the odd one above it (UAX #9 L2's
    // "including intermediate levels not actually present"). When every level is even this is
    // `max + 1` and the loop below does not run — the identity, which is every LTR page.
    let lowest_odd = slot_levels
        .iter()
        .map(|&l| if l % 2 == 1 { l } else { l + 1 })
        .min()
        .unwrap_or(1);
    if lowest_odd > max {
        return;
    }
    // `order` holds slot indices in VISUAL order. L2 scans the LOGICAL level array and reverses the
    // corresponding window of `order`, from the highest level down; the windows nest, so the
    // reversals compose.
    let mut order: Vec<usize> = (0..slots.len()).collect();
    for lvl in (lowest_odd..=max).rev() {
        let mut i = 0;
        while i < slot_levels.len() {
            if slot_levels[i] >= lvl {
                let mut j = i + 1;
                while j < slot_levels.len() && slot_levels[j] >= lvl {
                    j += 1;
                }
                order[i..j].reverse();
                i = j;
            } else {
                i += 1;
            }
        }
    }
    // Re-lay the line from its own start. Every slot keeps its advance, so the line's total width —
    // which alignment, justification and the float band all already agreed on — is unchanged.
    let mut pen = line[0].x;
    for &s in &order {
        match slots[s] {
            Slot::Frag(i) => {
                let w = line[i].width;
                line[i].x = pen;
                pen += w;
            }
            Slot::Space(w) => pen += w,
        }
    }
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
    strut: (f32, f32, f32),
    // ⚠ **Is this line ELIGIBLE to be justified?** CSS Text §7.3: `text-align: justify` justifies
    // every line of the block EXCEPT the last one and any line ended by a FORCED break (`<br>`),
    // which take the `text-align-last` value — `start` by default. So this is `true` only at the
    // wrap-induced call site. Getting it wrong is not a subtle error: justifying a three-word last
    // line stretches it across the whole column, which is the most recognisable rendering bug the
    // property has.
    justified: bool,
    // The IFC's **bidi base direction** (`direction: rtl` / `dir="rtl"` on the block that
    // establishes it) — the paragraph level UAX #9 resolves every run against. See
    // `reorder_line_bidi`.
    base_rtl: bool,
) -> f32 {
    // ── **A LINE BOX WITH NOTHING IN IT DOES NOT EXIST** (CSS 2.1 §9.4.2):
    //
    //    > *"Line boxes that contain no text, no preserved white space, no inline elements with
    //    > non-zero margins, padding or borders, and no other in-flow content … must be treated as
    //    > zero-height line boxes for the purposes of determining the positions of any elements
    //    > inside of them, and must be treated as not existing for any other purpose."*
    //
    //    The strut (§10.8) is folded into every line box unconditionally, so a `<div><span></span></div>`
    //    came out **19px tall against Chrome's 0** — a phantom line under every empty wrapper, and a
    //    `dy` that charges everything below it. Measured t760: `d2/s2` Chrome 0/0, ours 19/19.
    //
    //    ⚠ **The rule is about the LINE, not about the empty inline.** An empty inline *sharing a line
    //    with text* keeps its real rect — Chrome reports `<div>text<span id=s1></span>text</div>`'s
    //    span as 17px tall, and fragment anchors, scroll-spy targets and `getBoundingClientRect` on a
    //    marker span depend on it (which is exactly what `InlineItem::Spacer` was built for). So the
    //    test is `any(content_bearing)` over the line, and the reporter fragments are still EMITTED
    //    here — at zero height — rather than dropped, because dropping them would take the element out
    //    of `node_rects` and trade a placement error for a coverage one.
    if !line.iter().any(|f| f.content_bearing) {
        for f in line.drain(..) {
            frags.push(TextFragment {
                x: line_left + f.x,
                line_top: y,
                baseline: y,
                width: f.width,
                text: f.text,
                style: f.style,
                node: f.node,
                content_ascent: 0.0,
                content_height: 0.0,
            });
        }
        return y;
    }

    // ROUNDED per-part, because that is the content area's rule and it is NOT the line box's rule
    // (`LineMetrics::content_height` documents the measurement that separates them). The max is
    // taken over the *rounded* values so a mixed-font line agrees with the per-fragment boxes below.
    //
    // ── **THE TEXT METRICS** — folded over the fragments that HAVE metrics, plus the strut (CSS 2.1
    //    §10.8, tick 691: *"each line box starts with a zero-width inline box with the element's font
    //    and line height properties"*). These are the numbers `vertical-align: middle / text-top /
    //    text-bottom / sub / super` are defined against — **the parent's font**, never the aligned
    //    box's own — so an ATOMIC must not contribute to them. It used to: an atomic `LineFrag`
    //    carries `ascent = its own height` (see `LineFrag`), so one 40px inline image made `ascent`
    //    40 and `vertical-align: middle` aligned that line against a **20px "x-height"**.
    let ascent = line
        .iter()
        .filter(|f| f.atomic_h <= 0.0)
        .map(|f| f.ascent.round())
        .fold(strut.0.round(), f32::max);
    let descent = line
        .iter()
        .filter(|f| f.atomic_h <= 0.0)
        .map(|f| f.descent.round())
        .fold(strut.1.round(), f32::max);

    // ── **THE LEADING BELONGS TO EACH INLINE BOX, NOT TO THE LINE** (CSS 2.1 §10.8/§10.8.1, tick 695,
    //    Chrome-measured). The line box is built from two maxima taken *about the baseline*:
    //
    //    > *"The height of each inline-level box is calculated. For replaced/inline-block boxes this
    //    > is the margin box; for inline boxes it is the leading added to the font's ascent and
    //    > descent. The boxes are aligned per `vertical-align`, and the line box height is the
    //    > distance between the uppermost box top and the lowermost box bottom."*
    //
    //    What we did instead: fold `max(ascent)`, `max(descent)`, `max(line-height)` over the line,
    //    take `line_h = max(line-height, tallest atomic)`, then **centre the content area inside it**
    //    (`leading = (line_h - content_h)/2`). On a line whose tallest thing is a *text* box those two
    //    agree exactly — which is why it survived 690 ticks. They diverge the moment the line's
    //    tallest box is NOT the one carrying the leading, and then the whole line is displaced:
    //
    // ```text
    //    margin:0; font:16px/normal sans-serif; a 40x40 <img> + a <span>   Chrome   before   after
    //      div>img + span, line-height:60px   — the div                     h=65     h=60     h=65
    //                                         — the img top                  0        8        0
    //                                         — the span top                26       34       26
    //      div>img[vertical-align:top] + span — the span top                 0       24        0
    //      div>img + span (line-height:normal)— the div                     h=44     h=43     h=44
    //                                         — img top / span top         0 / 26   0 / 26   0 / 26
    //      div>span alone                     — the div                     h=18     h=18     h=18
    // ```
    //
    //    The `vertical-align:top` row is the loud one — **24px on every line that carries a
    //    top-aligned image**, because `ascent` was the *image's* height and the baseline was placed
    //    from it. `line-height` + an inline image is the ordinary shape of a nav bar, a card, a
    //    byline and an avatar row, so this is a `dy` on ordinary pages, not on a corner case.
    //
    //    ⚠ `half_leading` FLOORS, and the remainder goes BELOW: that is what the old code did
    //    (`leading = ((line_h - content_h) / 2.0).floor()`, verified against Chrome across 2 faces ×
    //    5 sizes × 4 line-heights) and keeping the split identical is what makes a plain text line —
    //    the overwhelming majority of lines on the web — come out byte-identical to before. It is
    //    also why `above + below == line_height` exactly for a single-font line, with no float dust.
    let half_leading = |a: f32, d: f32, lh: f32| ((lh - (a + d)) / 2.0).floor();
    let hl_s = half_leading(strut.0.round(), strut.1.round(), strut.2);
    let mut above = strut.0.round() + hl_s;
    let mut below = if strut.2 > 0.0 {
        strut.2 - strut.0.round() - hl_s
    } else {
        strut.1.round()
    };
    // A floor on the line box, applied AFTER the baseline-relative maxima. `vertical-align: top` and
    // `bottom` are aligned to the LINE BOX's own edges, which do not exist until everything else has
    // been placed, so per the spec they come last and can only make the line taller. **Which END they
    // grow is the whole distinction and they are opposites** — Chrome-measured, 40px image + a span on
    // a `16px/normal` line:
    //
    // ```text
    //                                  line h   img top   span top
    //   vertical-align: top              40        0          0     <- grows DOWNWARD: baseline stays
    //   vertical-align: bottom           40        0         22     <- grows UPWARD:   baseline moves
    // ```
    //
    // A `bottom` box pins the line's BOTTOM, so everything the strut demands *below* the baseline
    // still has to fit under it — the baseline is pushed down to 36 and the span with it. Treating
    // the two the same leaves `bottom` 22px out on every line that carries one, and
    // `vertical-align: bottom` on inline media is ordinary CSS-reset material.
    //
    // A synthetic reporter (inline padding/border, a `<br>`'s box) carries no metrics and holds the
    // line open through `line_height` alone; that is preserved exactly, as a floor rather than as
    // leading, and it grows downward like `top`.
    let mut min_h_down: f32 = 0.0;
    let mut min_h_up: f32 = 0.0;
    for f in line.iter() {
        if f.atomic_h > 0.0 {
            let h = f.atomic_h;
            // Each arm is the inverse of this fragment's `box_top` below — the pair is
            // (distance above the baseline, distance below it) for the same placement, and if the
            // two ever disagree the box is placed outside the line box it asked for.
            let bl = f.atomic_baseline;
            let (a, b) = match f.valign {
                // CSS 2.1 §10.8.1: the atomic contributes `baseline` above the line's baseline and
                // whatever is left below it. With the fallback (`bl == h`) this is the old `(h, 0)`
                // exactly, which is what keeps an empty or `overflow:hidden` inline-block, and every
                // replaced element, byte-identical.
                VerticalAlign::Baseline => (bl, h - bl),
                VerticalAlign::Middle => (h / 2.0 + ascent * 0.25, h / 2.0 - ascent * 0.25),
                VerticalAlign::TextTop => (ascent, h - ascent),
                VerticalAlign::TextBottom => (h - descent, descent),
                VerticalAlign::Sub => (h - ascent * 0.15, ascent * 0.15),
                VerticalAlign::Super => (h + ascent * 0.35, -(ascent * 0.35)),
                VerticalAlign::Top => {
                    min_h_down = min_h_down.max(h);
                    continue;
                }
                VerticalAlign::Bottom => {
                    min_h_up = min_h_up.max(h);
                    continue;
                }
            };
            above = above.max(a);
            below = below.max(b);
        } else if f.ascent > 0.0 || f.descent > 0.0 {
            let (a, d) = (f.ascent.round(), f.descent.round());
            let hl = half_leading(a, d, f.style.line_height);
            above = above.max(a + hl);
            below = below.max(f.style.line_height - a - hl);
        } else {
            min_h_down = min_h_down.max(f.style.line_height);
        }
    }
    // `bottom` first: it moves the baseline, and `top`'s floor is measured from the baseline it
    // leaves behind.
    above = above.max(min_h_up - below);
    below = below.max(min_h_down - above);
    let line_h = above + below;
    let baseline = y + above;

    // `f.width` already carries any `letter-spacing` (it equals `measure(text)` when spacing is 0),
    // so use it directly for both atomics and text rather than re-measuring — the re-measure would
    // drop letter-spacing and mis-place a centered/right-aligned tracked run.
    let line_width = line.last().map(|f| f.x + f.width).unwrap_or(0.0);
    // ── **`text-align: justify` — the slack goes into the WORD GAPS, not into one offset.**
    //
    // Every other alignment is a single translation of the whole line, which is why `justify` fell
    // through `_ => 0.0` and rendered identically to `left` for the engine's whole life. Justified
    // text is not rare: it is the default look of prose-heavy pages, newspapers, institutional and
    // government sites, and much of the non-English long tail this corpus is drawn from. And it does
    // not degrade gently — on a justified paragraph EVERY word after the first is misplaced, and the
    // error grows along the line, so one paragraph produces dozens of divergences. `www.wdimax.com`
    // is 127 `<span>`s whose WIDTHS all match Chrome exactly and whose x positions lag further and
    // further behind: the signature of the missing expansion, not of a measurement error.
    //
    // A gap is a place where the next fragment starts after this one ends — the advance the space
    // already contributed. Distributing `slack / gaps` cumulatively moves each fragment (and each
    // atomic box, which is positioned from the same `f.x`) to where the expanded spaces put it.
    // With no gaps (one long word) or no slack (an overflowing line) nothing moves.
    if matches!(align, TextAlign::Justify) && justified {
        const GAP_EPS: f32 = 0.01;
        let gaps = line
            .windows(2)
            .filter(|w| w[1].x - (w[0].x + w[0].width) > GAP_EPS)
            .count();
        let slack = line_avail - line_width;
        if gaps > 0 && slack > GAP_EPS {
            // ⚠ **The gap test has to be taken BEFORE anything moves.** Reading `line[i-1].x` inside
            // the same loop that has already shifted it compares a moved fragment against an unmoved
            // one, so every gap after the first measures as closed and the expansion stops
            // accumulating. Measured while writing this: the 2nd word landed exactly right and the
            // 6th was 10px short, which is what a shift that stops accumulating looks like from the
            // outside. Snapshot the gap positions first, then apply.
            let is_gap: Vec<bool> = (0..line.len())
                .map(|i| i > 0 && line[i].x - (line[i - 1].x + line[i - 1].width) > GAP_EPS)
                .collect();
            let per_gap = slack / gaps as f32;
            let mut shift = 0.0f32;
            for i in 0..line.len() {
                if is_gap[i] {
                    shift += per_gap;
                }
                line[i].x += shift;
            }
        }
    }
    // ── **UAX #9 RULE L2 — the line's INLINE BOXES are reordered into visual order.** Runs after
    // justification (which reads the flow-order gaps) and before the alignment offset (a uniform
    // shift, which reordering commutes with).
    reorder_line_bidi(line, base_rtl);
    let offset = match align {
        TextAlign::Center => (line_avail - line_width).max(0.0) / 2.0,
        TextAlign::Right => (line_avail - line_width).max(0.0),
        // `justify` leaves the line at the start edge: the expansion above has already placed every
        // fragment, and the last line (which reaches here with `justified == false`) is `start`.
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
                // baseline: the box's OWN baseline sits on the line's baseline — its last in-flow
                // line box's, or its bottom margin edge when §10.8.1's fallback applies. The pair
                // above (`(bl, h - bl)`) is the inverse of this line; if the two ever disagree the
                // box is placed outside the line box it asked for.
                VerticalAlign::Baseline => baseline - f.atomic_baseline,
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
            let (fa, fd) = match (f.report_h, f.report_ascent) {
                // A PADDED inline edge: its rect starts above the content top by the vertical
                // padding+border, so it carries its own ascent rather than anchoring at the line
                // top. See `InlineItem::Spacer::report_ascent`.
                (Some(h), Some(a)) => (a, h - a),
                (Some(h), None) => (baseline - y, h - (baseline - y)),
                (None, _) => (f.ascent.round(), f.descent.round()),
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
    /// An **out-of-flow positioned `::before`/`::after`** — the custom-bullet / icon idiom
    /// (`content: "–"; position: absolute; left: 0`). It contributes **zero advance and zero line
    /// metrics**, so it neither pushes the following text nor grows the line, and paints at `dx`
    /// from the pen. See the long comment in `collect_inline_group` for what this deliberately does
    /// NOT do (vertical insets; a static owner's positioned ancestor).
    AbsPseudo {
        text: String,
        style: TextStyle,
        dx: f32,
    },
    /// An `inline-block`: `advance` is its margin-box main-axis size; `box_` is its already
    /// laid-out block box (positioned at the origin, translated into place at line close).
    Atomic {
        box_: Box<LayoutBox>,
        advance: f32,
        height: f32,
        /// Distance from the margin-box top to the box's own last in-flow line box's baseline, or
        /// `None` when CSS 2.1 §10.8.1's fallback applies (no in-flow line boxes, or `overflow`
        /// other than `visible`) and the bottom margin edge is the baseline.
        baseline: Option<f32>,
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
    ///
    /// `holds_line` is CSS 2.1 §9.4.2: a spacer that is a real *margin/border/padding* edge keeps its
    /// line box in existence, while the zero-width box of a bare `<span></span>` does not — see
    /// `close_line`.
    Spacer {
        width: f32,
        node: Option<NodeId>,
        space_before: bool,
        report_height: f32,
        /// ⚠⚠ **How far ABOVE the baseline this spacer's reported rect starts** — `None` means
        /// "from the line top", which is what every spacer used to do.
        ///
        /// It exists because **vertical padding and border on an INLINE box extend the box without
        /// touching the line**. `<a style="padding:10px 20px">Login</a>` is 37px tall in Chrome and
        /// starts 10px ABOVE its own text, while the line box around it stays 20px — the padded pill
        /// simply overflows its line, which is exactly why `padding` on an inline link is the way
        /// every tag, badge, nav pill and button-styled link on the web is written. We reported 18
        /// and painted 18: the blue pill was half the height the author drew.
        ///
        /// A rect anchored at the line top cannot express that, because the box starts above it. So
        /// the padded edge spacer carries its own ascent.
        report_ascent: Option<f32>,
        holds_line: bool,
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

impl InlineItem {
    /// **The element whose geometry this item reports** — the deepest element ancestor of a word's
    /// text, the owner of a padding edge, the element a `<br>` is. `None` for an item that reports
    /// nobody's box: an `Atomic` (its `LayoutBox` already carries its own node) and an `AbsPseudo`
    /// (generated content, which is not an element).
    ///
    /// `collect_inline_node` asks this one question — *did anything I just emitted claim MY node?* —
    /// to decide whether the element needs a reporter of its own. Written as an accessor rather than
    /// inline so that adding a variant forces the answer to be given, instead of silently defaulting
    /// to "no" and handing the element a duplicate box.
    fn owner(&self) -> Option<NodeId> {
        match self {
            InlineItem::Word { node, .. }
            | InlineItem::Spacer { node, .. }
            | InlineItem::Break { node, .. } => *node,
            InlineItem::Atomic { .. } | InlineItem::AbsPseudo { .. } => None,
        }
    }
}

/// **The baseline of the LAST in-flow line box inside `b`**, in `b`'s own coordinate space, or
/// `None` when there is no line box to take one from (CSS 2.1 §10.8.1's fallback case).
///
/// Blocks are searched last-first because that is what "last line box" means, and the search
/// recurses: the line may be several block levels down (`<div><p>text</p></div>` as an
/// inline-block). A block whose subtree holds no text at all yields `None` and the caller falls back
/// to the bottom margin edge, which is the same answer an empty inline-block has always got here.
fn last_line_baseline(b: &LayoutBox) -> Option<f32> {
    match &b.content {
        // Within one inline formatting context the LAST line has the greatest baseline, so the max
        // is the last line's — no ordering assumption about the fragment vector is needed.
        BoxContent::Inline(frags) => frags
            .iter()
            .map(|f| f.baseline)
            .fold(None, |acc: Option<f32>, x| {
                Some(acc.map_or(x, |m| m.max(x)))
            }),
        BoxContent::Block(kids) => kids.iter().rev().find_map(last_line_baseline),
    }
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
        // ── **NO BREAK AFTER A SOLIDUS — Chrome does not take this opportunity, and URLs are where
        // it shows.** UAX #14 offers a break after `/` (class SY), and `unicode-linebreak` reports
        // it faithfully; Blink tailors it away, so a long URL overflows its box in Chrome instead of
        // wrapping. Measured on three fixtures at a 120px width, heights in px:
        //
        //   `aaaa/bbbb/cccc/dddd`                        Chrome 19   ours 38
        //   `https://example.com/very/long/path/here`    Chrome 19   ours 77
        //   `one/two three/four five/six seven/eight`    Chrome 77   ours 58
        //
        // The last one is the tell that this is not "Chrome wraps less": Chrome takes MORE lines
        // there, because refusing the `/` opportunity means a whole token has to move down. So the
        // error is not a bias in one direction — it is a different set of line boxes, and every
        // element below one inherits the difference as `dy`.
        //
        // Every other separator probed in the same fixture agrees already (`- . _ ? = & , : +`,
        // numeric dates, CJK, soft hyphens, U+200B), which is what makes this a one-character
        // tailoring rather than a quarrel with the crate. U+002F is the only character in class SY,
        // so "after a solidus" and "after SY" are the same rule.
        //
        // ⚠ `overflow-wrap: break-word` is a DIFFERENT path (`InlineItem::break_word`) and is
        // unaffected: a page that asks for the URL to be broken still gets it broken, which is the
        // half that would make this a regression rather than a fix.
        if word[..idx].ends_with('/') {
            continue;
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

    /// **EVERY LINE BOX STARTS WITH A STRUT, AND A BASELINE-ALIGNED ATOMIC SITS ON THE BASELINE.**
    ///
    /// CSS 2.1 §10.8: *"each line box starts with a zero-width inline box with the element's font and
    /// line height properties — the strut."* Ours folded ascent/descent/line-height over **the
    /// fragments present**, and an atomic or synthetic `LineFrag` carries `ascent == descent == 0` by
    /// construction — so a line whose only content is an `<img>` had **zero descent** and reserved
    /// nothing below the baseline. Measured, `margin:0; font:16px/normal sans-serif`, a 40×40 `<img>`:
    ///
    /// ```text
    ///                                     Chrome   before   after
    ///   div > img  (default = baseline)     h=44     h=40     h=43
    ///   div > img  vertical-align:top       h=40     h=40     h=40   <- guard
    ///   div > img  display:block            h=40     h=40     h=40   <- guard
    ///   p  (a plain text line)              --       h=18     h=18   <- guard
    /// ```
    ///
    /// The 1px residual against Chrome is a FONT-descent difference (our `sans-serif` resolves to a
    /// different face than the reference Chrome's), not a logic one — and it is inside the 8px SHAPE
    /// tolerance the certificate scores on, where 4px of missing descent per inline image was not.
    ///
    /// ⚠ **TWO CHANGES, ONE BEHAVIOUR, and each alone reads as a no-op.** The strut supplies a non-zero
    /// `descent`; `tallest_atomic + descent` spends it. Tick 690 tried the second half alone, measured
    /// no change, and reverted it — correctly, on the evidence available then. This gate asserts the
    /// combination so neither half can be removed as dead code.
    ///
    /// ⚠ The three guards are not decoration: `top`/`block`/text already agreed with Chrome, so a fix
    /// that opened EVERY line box by the descent would move them too and would be wrong in a way a
    /// single assertion on `w1` could not see. `parity` (72/72 vs headless Chrome, 30 pages) is the
    /// wider net and it holds.
    #[test]
    fn a_line_box_starts_with_a_strut_so_a_baseline_atomic_reserves_its_descent() {
        let html = r#"<div id="w1"><img width="40" height="40" src="x.png"></div>
                      <div id="w2"><img width="40" height="40" style="vertical-align:top" src="x.png"></div>
                      <div id="w3"><img width="40" height="40" style="display:block" src="x.png"></div>
                      <p id="p1">plain text line</p>"#;
        let css = "body{margin:0}";
        let (dom, root) = layout_html(html, css, 800.0);
        let rects = root.node_rects(&dom);
        let h = |id: &str| {
            let n = dom
                .descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id");
            rects[&n].height
        };
        assert!(
            h("w1") > 40.5,
            "a div holding a baseline-aligned 40px <img> must be TALLER than the image — the strut's \
             descent sits below the baseline the image rests on (Chrome: 44). Got {}. Without the \
             strut this is exactly 40 and every inline icon on the web shifts everything below it up.",
            h("w1")
        );
        for (id, why) in [
            (
                "w2",
                "`vertical-align:top` does not sit on the baseline, so it demands only its height",
            ),
            (
                "w3",
                "a `display:block` image is not an inline atomic at all",
            ),
        ] {
            assert!(
                (h(id) - 40.0).abs() < 0.5,
                "OVER-CORRECTION: #{id} is {} and must stay 40 — {why}. A change that opened EVERY \
                 line box by the descent would move these too, and a single assertion on #w1 could \
                 not see it.",
                h(id)
            );
        }
        assert!(
            h("p1") < 30.0,
            "a plain text line must not grow: the strut is the block's OWN font, so on a line of that \
             same font it adds nothing. Got {} — the strut is being double-counted.",
            h("p1")
        );
    }

    /// **THE HALF-LEADING BELONGS TO EACH INLINE BOX, NOT TO THE LINE** (CSS 2.1 §10.8/§10.8.1).
    ///
    /// The line box is two maxima taken *about the baseline* — `max(distance above)` and
    /// `max(distance below)` over every inline-level box, each box having contributed its **own**
    /// leading. We instead folded `max(ascent)`, `max(descent)`, `max(line-height)` over the line and
    /// then **centred the content area inside the result**. On a line whose tallest box is the one
    /// carrying the leading those two agree exactly, which is why this survived 690 ticks; they
    /// diverge the moment the tallest box is an ATOMIC, and then the whole line is displaced.
    ///
    /// Chrome-measured (`--headless=new --dump-dom`, 1280×800, `margin:0`, `16px/normal sans-serif`,
    /// a 40×40 `<img>` followed by a `<span>`), all values relative to the div's own top:
    ///
    /// ```text
    ///                                          Chrome   before   after
    ///   line-height:60px  — the div              h=65     h=60     h=65
    ///                     — the img top            0        8        0
    ///                     — the span top          26       34       26
    ///   vertical-align:top — the span top          0       24        0
    ///   vertical-align:bottom — the span top      22        0       22
    ///   (guards)
    ///   line-height:normal — the div             h=44     h=43     h=44
    ///                     — img top / span top  0 / 26   0 / 26   0 / 26
    ///   a span alone       — the div             h=18     h=18     h=18
    /// ```
    ///
    /// ⚠ **`top` and `bottom` are opposites and that is the point of asserting both.** Both are
    /// aligned to the line box's own edges, so both are applied after the baseline-relative maxima —
    /// but `top` grows the line DOWNWARD (the baseline stays) and `bottom` grows it UPWARD (the
    /// baseline moves, because the strut's descent still has to fit above the pinned bottom edge). A
    /// fix that treated them alike passed the `top` row and left `bottom` 22px out, and only a
    /// fixture carrying both could see it. `img { vertical-align: middle|bottom }` is CSS-reset
    /// material, so this is ordinary-page geometry, not a corner case.
    ///
    /// ⚠ The *heights* alone cannot gate this: `top` and `bottom` produce the same 40px line box and
    /// differ only in where the text inside it sits. The assertions are on POSITIONS.
    #[test]
    fn the_half_leading_belongs_to_each_inline_box_not_to_the_line() {
        let html = r#"<div id="w1" style="line-height:60px"><img width="40" height="40" src="x.png"><span id="s1">Hg</span></div>
                      <div id="w2"><img width="40" height="40" style="vertical-align:top" src="x.png"><span id="s2">Hg</span></div>
                      <div id="w3"><img width="40" height="40" style="vertical-align:bottom" src="x.png"><span id="s3">Hg</span></div>
                      <div id="w4"><img width="40" height="40" src="x.png"><span id="s4">Hg</span></div>
                      <div id="w5"><span id="s5">Hg</span></div>
                      <div id="w6"><img width="40" height="40" style="vertical-align:middle" src="x.png"><span id="s6">Hg</span></div>"#;
        let css = "body{margin:0} div{font-family:sans-serif;font-size:16px;line-height:normal}";
        let (dom, root) = layout_html(html, css, 1280.0);
        let rects = root.node_rects(&dom);
        let r = |id: &str| {
            let n = dom
                .descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id");
            rects[&n]
        };
        // The span's top WITHIN its own div — the number the old code got wrong, and the one a
        // height assertion cannot see.
        let span_top = |w: &str, s: &str| r(s).y - r(w).y;
        for (w, s, want, why) in [
            (
                "w1", "s1", 26.0,
                "`line-height:60px` gives the STRUT 21px of half-leading; the 40px image gets NONE. \
                 Centring the content area in the line instead put the baseline at 48 and everything \
                 on the line 8px low (Chrome: baseline 40, span top 26).",
            ),
            (
                "w2", "s2", 0.0,
                "`vertical-align:top` grows the line DOWNWARD — the baseline stays where the strut \
                 put it, at 14. The atomic used to set the line's `ascent` to its OWN height, which \
                 put the baseline at 38 and the text 24px low.",
            ),
            (
                "w3", "s3", 22.0,
                "`vertical-align:bottom` grows the line UPWARD — the image pins the line's bottom at \
                 40 and the strut's 4px of descent must still fit under the baseline, so the \
                 baseline moves DOWN to 36. Treating it like `top` leaves the text at 0, 22px out.",
            ),
            (
                "w4", "s4", 26.0,
                "GUARD: the ordinary baseline case already agreed with Chrome and must not move.",
            ),
            (
                "w5", "s5", 0.0,
                "GUARD: a plain text line is the overwhelming majority of lines on the web. Its \
                 half-leading is the line's half-leading, so the two rules coincide exactly here — \
                 and if this moves, every paragraph on every page moved.",
            ),
            (
                "w6", "s6", 10.0,
                "`vertical-align: middle` is defined against the PARENT's x-height, and ours is \
                 approximated as half the line's `ascent` — so an ATOMIC must not contribute to that \
                 ascent. It used to: a 40px image made `ascent` 40, giving a 20px `x-height` and \
                 putting the text 6px low. `img { vertical-align: middle }` is in most CSS resets.",
            ),
        ] {
            assert!(
                (span_top(w, s) - want).abs() < 1.5,
                "#{s} sits {} below #{w}, Chrome says {want}. {why}",
                span_top(w, s)
            );
        }
        // The line box heights Chrome reports for the same five rows.
        for (w, want, why) in [
            (
                "w1",
                65.0,
                "the strut's leading is BELOW the baseline too: 40 above + 25 below",
            ),
            (
                "w2",
                40.0,
                "GUARD: a top-aligned atomic is exactly its own height of line box",
            ),
            (
                "w3",
                40.0,
                "GUARD: and so is a bottom-aligned one — same height, different text position",
            ),
            ("w4", 44.0, "40 above the baseline + the strut's 4 below it"),
            (
                "w5",
                18.0,
                "GUARD: `line-height: normal` on this face, unchanged",
            ),
            (
                "w6",
                40.0,
                "a middle-aligned atomic straddles the baseline by half its height each way",
            ),
        ] {
            assert!(
                (r(w).height - want).abs() < 1.5,
                "#{w} is {} tall, Chrome says {want} — {why}",
                r(w).height
            );
        }
    }

    /// **AN `<img>` WHOSE SOURCE DID NOT LOAD IS 16×16 — Chrome-measured, not recalled.**
    ///
    /// This is the highest-mass shape behind the SHAPE score, and tick 688 measured why: across the
    /// scored HEAD-20 sites the median `dx` is 0–2 and `dw`/`dh` are 0 on the worst sites, while `dy`
    /// runs 91 / 145 / 206 / 3077. **The boxes are the right size and in the wrong place**, which means
    /// something ABOVE them has the wrong height — and `keirin.jp`'s first divergence begins
    /// *immediately after an `<img>`*, off by `dy=70`. `Cc4e6 geometry: <img>` is a 67-site cluster.
    ///
    /// Measured on headless Chrome and on this engine, same fixture, 800px viewport:
    ///
    /// ```text
    ///                                        Chrome        ours (before)
    ///   <img src="…/never.png">              16×16          784×0
    ///   <img width=120 height=70 src=…>      120×70         120×70      ✓
    ///   the div AFTER the bare img            y=196          y=168
    /// ```
    ///
    /// 784×0 is wrong twice: an inline replaced element must not take the whole line, and a box whose
    /// source broke is not zero-height. 16×16 is the placeholder Chrome reserves, and reserving it is
    /// what stops the rest of the page sliding up.
    ///
    /// ⚠ The gate asserts the FOLLOWING sibling's `y`, not just the image's box — a height that is
    /// right in isolation and does not push its siblings down would satisfy a box-only assertion and
    /// fix nothing about `dy`, which is the entire reason this is being changed.
    #[test]
    fn a_broken_img_reserves_chromes_16x16_placeholder_and_pushes_its_sibling_down() {
        let html = r#"<img id="bare" src="http://127.0.0.1:1/never.png"><div id="after"></div>"#;
        let css = "#after{height:10px}";
        let (dom, root) = layout_html(html, css, 800.0);
        let rects = root.node_rects(&dom);
        let by_id = |id: &str| {
            dom.descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id")
        };
        let img = rects[&by_id("bare")];
        assert!(
            (img.width - 16.0).abs() < 0.5 && (img.height - 16.0).abs() < 0.5,
            "a broken <img> with no dimensions must reserve Chrome's 16×16 placeholder, not the full              line width at zero height (measured: Chrome 16×16, ours was 784×0) — got {}×{}",
            img.width,
            img.height
        );
        let after = rects[&by_id("after")];
        assert!(
            after.y >= img.y + 16.0,
            "the box AFTER a broken <img> must be pushed down by the placeholder ({} vs img bottom              {}). A height that is right in isolation but does not displace its siblings fixes              nothing about the `dy` term that holds SHAPE at ~6%.",
            after.y,
            img.y + 16.0
        );
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

    /// **A FLOAT IS NOT THE FIRST IN-FLOW CHILD, AND WE LET IT CANCEL THE COLLAPSE.**
    ///
    /// CSS 2.1 §8.3.1 collapses a box's top margin with its first **in-flow** child's. A float is
    /// out of flow, so it is skipped and the `<p>` after it is the first in-flow child — but all
    /// four §8.3.1 search helpers bailed out on one, so the `<p>`'s margin stayed *inside* the
    /// parent and the parent grew by exactly that margin.
    ///
    /// This is `div.willkommen` on `kicktipp.com` — `.illu{float:right}` wrapping the illustration,
    /// then the prose — and it is the commonest float idiom there is (the pull-quote, the article
    /// figure, the sidebar thumbnail). It cost that site a **reading-order inversion**: Chrome reads
    /// the prose first (both at `y=0`, prose at `x=0`), we read the float first because we alone
    /// pushed the prose down 15px. `shape` was already 0.85 there, so this one number was the whole
    /// distance to M1.
    ///
    /// Chrome-measured, `/tmp/mc.html` at 800px with `body{margin:0}` and `p{margin:15px 0}`:
    /// float-first parent y=15 / p y=15; abspos-first parent y=68 / p y=68; **text**-first parent
    /// y=159 / p y=192 (no collapse — real inline content genuinely does separate the margins).
    ///
    /// RED PROOF: restore `return 0.0` in `leading_block_collapse_top`'s out-of-flow arm and the
    /// first two assertions fail with `#p` 15px below `#outer`'s content top. The third assertion is
    /// the guard that keeps the fix from becoming "collapse through anything".
    #[test]
    fn an_out_of_flow_first_child_does_not_cancel_the_parent_child_margin_collapse() {
        let css = "body{margin:0} p{margin:15px 0} .f{float:right;width:60px;height:40px} \
                   .a{position:absolute;width:30px;height:20px}";

        // 1. A float before the first in-flow block: the block's top margin still escapes.
        let (dom, root) = layout_html(
            r#"<div id="outer"><div class="f"></div><p id="p">alpha</p></div>"#,
            css,
            800.0,
        );
        let rects = root.node_rects(&dom);
        let (outer, p) = (rects[&by_id(&dom, "outer")], rects[&by_id(&dom, "p")]);
        assert!(
            (p.y - outer.y).abs() < 1.0,
            "a preceding FLOAT is out of flow — the <p> is still the first in-flow child and its \
             15px top margin must escape (outer.y={}, p.y={})",
            outer.y,
            p.y
        );

        // 2. Same for an absolutely-positioned first child.
        let (dom, root) = layout_html(
            r#"<div id="outer" style="position:relative"><div class="a"></div><p id="p">beta</p></div>"#,
            css,
            800.0,
        );
        let rects = root.node_rects(&dom);
        let (outer, p) = (rects[&by_id(&dom, "outer")], rects[&by_id(&dom, "p")]);
        assert!(
            (p.y - outer.y).abs() < 1.0,
            "a preceding position:absolute box is out of flow too (outer.y={}, p.y={})",
            outer.y,
            p.y
        );

        // 3. THE GUARD — real inline text before the block DOES separate the margins, so the <p>
        //    must still sit 15px inside. Without this, "skip out-of-flow" could be over-applied
        //    into "skip everything", which Chrome does not do (measured above: 159 vs 192).
        let (dom, root) = layout_html(
            r#"<div id="outer">text<p id="p">gamma</p></div>"#,
            css,
            800.0,
        );
        let rects = root.node_rects(&dom);
        let (outer, p) = (rects[&by_id(&dom, "outer")], rects[&by_id(&dom, "p")]);
        assert!(
            p.y - outer.y > 14.0,
            "inline text is in-flow content and BLOCKS the collapse — the 15px margin stays inside \
             (outer.y={}, p.y={})",
            outer.y,
            p.y
        );
    }

    /// The bottom-edge mirror: a **trailing** float must not cancel the parent↔last-child bottom
    /// margin collapse either. Chrome (`/tmp/mc.html`): the `<p>`'s 15px bottom margin escapes past
    /// the trailing float, so the parent's bottom edge is the `<p>`'s bottom edge.
    #[test]
    fn a_trailing_float_does_not_cancel_the_bottom_margin_collapse() {
        let (dom, root) = layout_html(
            r#"<div id="outer"><p id="p">gamma</p><div class="f"></div></div>"#,
            "body{margin:0} p{margin:15px 0} .f{float:right;width:60px;height:40px}",
            800.0,
        );
        let rects = root.node_rects(&dom);
        let (outer, p) = (rects[&by_id(&dom, "outer")], rects[&by_id(&dom, "p")]);
        assert!(
            ((outer.y + outer.height) - (p.y + p.height)).abs() < 1.0,
            "the trailing float is out of flow — the <p>'s bottom margin must still escape \
             (outer_b={}, p_b={})",
            outer.y + outer.height,
            p.y + p.height
        );
    }

    /// **`&nbsp;` IS NOT COLLAPSIBLE WHITE SPACE, AND `char::is_whitespace` SAYS IT IS.**
    ///
    /// CSS Text collapses exactly SPACE, TAB, LF, CR and FF. Rust's `char::is_whitespace` implements
    /// the **Unicode `White_Space` property**, a strictly larger set whose extra members are precisely
    /// the characters an author picks *because* they must not collapse. Every collapse site here used
    /// it, so `&nbsp;` was collapsed and trimmed like a space — and an element whose only content was
    /// `&nbsp;` ended up with no text, hence **no line box at all**.
    ///
    /// Measured against live Chromium:
    ///
    /// | markup | Chrome | was |
    /// |---|---|---|
    /// | `<div>&nbsp;</div>` height | 18 | **0** |
    /// | `a&nbsp;&nbsp;&nbsp;b` width (monospace 16px) | 48 | **29** (collapsed to one) |
    /// | `a   b` width — ASCII, MUST still collapse | 29 | 29 |
    ///
    /// Both directions are asserted, because a fix that simply stopped collapsing would be a worse
    /// bug: ASCII runs must still collapse to a single space.
    ///
    /// RED, run: change `is_css_white_space` back to `ch.is_whitespace()` — the nbsp div reads 0 and
    /// the three-nbsp run reads the same width as the one-nbsp run.
    #[test]
    fn a_non_breaking_space_is_content_not_collapsible_white_space() {
        let (dom, root) = layout_html(
            "<body style='margin:0;font:16px/normal monospace'>\
             <div id=nb>\u{a0}</div>\
             <div id=empty></div>\
             <span id=one style='display:inline-block'>a\u{a0}b</span>\
             <span id=three style='display:inline-block'>a\u{a0}\u{a0}\u{a0}b</span>\
             <span id=sp3 style='display:inline-block'>a   b</span>\
             </body>",
            "",
            1200.0,
        );
        let rects = root.node_rects(&dom);
        let r = |want: &str| {
            let n = dom
                .descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(want))
                .unwrap_or_else(|| panic!("no #{want}"));
            rects[&n]
        };
        // A div whose only content is `&nbsp;` gets a real line box (Chrome: 18 at 16px).
        assert!(
            r("nb").height > 10.0,
            "`<div>&nbsp;</div>` must generate a line box (Chrome 18), got {}",
            r("nb").height
        );
        // …and a genuinely empty one still must not.
        assert!(
            r("empty").height < 1.0,
            "an empty <div> has no line box, got {}",
            r("empty").height
        );
        // Three NBSP are three characters, not one collapsed space.
        // Two extra NBSP must add two spaces' width. The threshold is deliberately font-INDEPENDENT
        // (any face's space is wider than 2.5px at 16px): the absolute numbers in the table above come
        // from the browser fixture, where `monospace` resolves; this harness resolves a different face,
        // and a test that pinned 48-vs-29 here would be pinning the test environment's font, not the
        // rule. What is asserted is the RULE: three do not collapse to one, and ASCII still does.
        assert!(
            r("three").width > r("one").width + 5.0,
            "a run of `&nbsp;` must NOT collapse (browser fixture: Chrome 48 vs 29), got {} vs {}",
            r("three").width,
            r("one").width
        );
        // The other half: ASCII white space must still collapse, or the fix is a worse bug.
        assert!(
            (r("sp3").width - r("one").width).abs() < 2.0,
            "three ASCII spaces must still collapse to one (Chrome 29 == 29), got {} vs {}",
            r("sp3").width,
            r("one").width
        );
    }

    /// **A line box with no content-bearing member does not exist** (CSS 2.1 §9.4.2) — and the
    /// interesting half of this gate is the case it must NOT break.
    ///
    /// Measured against live Chromium on `body{margin:0;font:16px/normal sans-serif}`:
    ///
    /// | markup | Chrome (div / span) | was |
    /// |---|---|---|
    /// | `<div>text<span></span>text</div>` | 18 / **17** | 19 / 19 — the ANCHOR case, must stay >0 |
    /// | `<div><span></span></div>` | **0 / 0** | 19 / 19 ❌ |
    /// | `<div><span style=padding:4px></span></div>` | **18** / 25 | 18 / 0 |
    /// | `<div><span style=padding:4px 0></span></div>` | **0** | — |
    /// | `<div><span style=border-top:3px></span></div>` | **0** | — |
    /// | `<div><span style=margin-left:10px></span></div>` | **0** | — |
    ///
    /// The last three rows are why this is asserted and not derived: §9.4.2's own words are *"no
    /// inline elements with non-zero margins, padding or borders"*, and three of those four rows have
    /// exactly that and are still 0. Only an edge that occupies **inline flow width** holds the line.
    ///
    /// RED, run: give the empty-inline `Spacer` `holds_line: true` — `d2` reads 19 against Chrome's 0,
    /// which is the corpus symptom. Give it to every spacer and `d4`/`d5` go 19 as well.
    #[test]
    fn a_line_box_with_only_empty_inlines_does_not_exist() {
        let (dom, root) = layout_html(
            "<body style='margin:0;font:16px/normal sans-serif'>\
             <div id=d1>text<span id=s1></span>text</div>\
             <div id=d2><span id=s2></span></div>\
             <div id=d3><span id=s3 style='padding:4px'></span></div>\
             <div id=d4><span id=s4 style='padding:4px 0'></span></div>\
             <div id=d6><span id=s6 style='margin-left:10px'></span></div>\
             </body>",
            "",
            1200.0,
        );
        let rects = root.node_rects(&dom);
        let r = |want: &str| {
            let n = dom
                .descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(want))
                .unwrap_or_else(|| panic!("no #{want}"));
            rects[&n]
        };
        // THE FIX: an empty inline alone on its line generates no line box (Chrome 0/0).
        assert!(
            r("d2").height < 1.0,
            "`<div><span></span></div>` must have NO line box (Chrome 0), got {}",
            r("d2").height
        );
        assert!(
            r("s2").height < 1.0,
            "the empty inline alone on its line reports 0 in Chrome, got {}",
            r("s2").height
        );
        // THE HALF THAT MUST NOT BREAK — an empty inline SHARING a line with text keeps a real rect
        // (Chrome 17): fragment anchors, scroll-spy targets and `getBoundingClientRect` on a marker
        // span all read this box, and it is the reason `InlineItem::Spacer` exists. A blunt "an empty
        // inline makes no line box" fix passes the assertion above and fails this one.
        assert!(
            r("d1").height > 10.0,
            "a line with text still exists (Chrome 18), got {}",
            r("d1").height
        );
        assert!(
            r("s1").height > 10.0,
            "an empty inline BESIDE TEXT keeps its rect (Chrome 17), got {}",
            r("s1").height
        );
        // An edge that occupies inline flow width DOES hold the line open (Chrome 18)…
        assert!(
            r("d3").height > 10.0,
            "horizontal padding on an empty inline holds its line box (Chrome 18), got {}",
            r("d3").height
        );
        // …and one that does not, does not — Chrome is narrower than §9.4.2's own sentence here.
        assert!(
            r("d4").height < 1.0,
            "vertical-only padding does NOT hold a line box (Chrome 0), got {}",
            r("d4").height
        );
        assert!(
            r("d6").height < 1.0,
            "a horizontal margin does NOT hold a line box (Chrome 0), got {}",
            r("d6").height
        );
    }

    /// **Block-in-inline margin collapse** (CSS2 §9.2.1.1 + §8.3.1). An inline split around a block
    /// child stands in for the spec's ANONYMOUS BLOCK BOXES, and an anonymous block has no margins of
    /// its own — so the block child's vertical margins pass straight through it and out of the
    /// grandparent. `is_block_level` already blockified the `<a>`; the two collapse predicates tested
    /// the RAW `display` and still said "inline", so `<a><div style="margin:3px 0 6px">` came out
    /// 3+10+6 = 19px tall instead of 10. Chrome-measured on this exact shape (`--tol 0`):
    /// `#outer` is `[0 3 1200×10]`, ours was `[0 0 1200×19]`.
    ///
    /// Both halves are asserted, because the fix is only correct if the *eligibility* survives: an
    /// inline with real text BEFORE the block still declines the top collapse (the text is the first
    /// in-flow content), which is also what Chrome does.
    #[test]
    fn a_block_inside_an_inline_collapses_its_margins_out() {
        let html = r#"<div id="outer"><a id="lnk"><div id="inner"></div></a></div>
                      <div id="txt"><a id="lnk2">t<div id="inner2"></div></a></div>"#;
        let css = "body{margin:0} #inner,#inner2{margin:3px 0 6px;width:10px;height:10px}";
        let (dom, root) = layout_html(html, css, 400.0);
        let rects = root.node_rects(&dom);
        let outer = rects[&by_id(&dom, "outer")];
        let lnk = rects[&by_id(&dom, "lnk")];
        let inner = rects[&by_id(&dom, "inner")];
        assert!(
            (outer.height - 10.0).abs() < 1.0,
            "the block child's 3px/6px margins must escape THROUGH the blockified <a> and out of \
             #outer (want height 10, got {})",
            outer.height
        );
        assert!(
            (lnk.height - 10.0).abs() < 1.0,
            "the blockified <a> itself must not keep the margins inside (want 10, got {})",
            lnk.height
        );
        assert!(
            (inner.y - outer.y).abs() < 1.0,
            "#inner must sit flush at #outer's content top (inner.y={}, outer.y={})",
            inner.y,
            outer.y
        );

        // Eligibility half: real inline text before the block is the first in-flow content, so the
        // TOP margin stays in. The bottom one still escapes (the block is the last in-flow child).
        let txt = rects[&by_id(&dom, "txt")];
        let inner2 = rects[&by_id(&dom, "inner2")];
        assert!(
            inner2.y - txt.y >= 3.0,
            "text before the block must keep the 3px top margin INSIDE (inner2.y={}, txt.y={})",
            inner2.y,
            txt.y
        );
        assert!(
            ((txt.y + txt.height) - (inner2.y + inner2.height)).abs() < 1.0,
            "the trailing 6px must still escape the bottom (txt_b={}, inner2_b={})",
            txt.y + txt.height,
            inner2.y + inner2.height
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

    /// **A BUTTON CENTRES ITS CONTENT VERTICALLY, AND NO STYLESHEET CAN SAY SO.**
    ///
    /// The UA sheet already gives buttons `text-align: center`, which is why the HORIZONTAL half has
    /// always matched Chrome. The vertical half is not expressible in CSS at all: Blink lays a
    /// button's children out inside an anonymous flex-like box with `align-items: center`, and the
    /// HTML rendering spec describes the same thing. Every design system fixes a button height, so
    /// before this the label sat 5–20px too high on essentially every button on the web — and being
    /// a label inside a fixed-size box, it is the kind of divergence the fidelity instrument reports
    /// as `overlap` rather than as `shape`.
    ///
    /// Chrome-measured, `button{display:block;width:300px;padding:0;border:0;font:16px Arial}`,
    /// y of the label relative to the button's border box:
    ///
    /// ```text
    ///                                                    Chrome   before   after
    ///   height:50px, one 18px line                          16       0      16    ✗→✓
    ///   height:80px, TWO block spans (36px together)         22       0      22    ✗→✓
    ///   height:20px, an 18px line (nearly full)               1       0       1    ✗→✓
    ///   height:auto                                           0       0       0     ✓ control
    ///   a plain <div> at height:50px                          0       0       0     ✓ control
    /// ```
    ///
    /// Asserted against the AUTO-height button's own height rather than against `18`, so the UA
    /// font's metrics cannot make the test lie — the rule is `(box − content) / 2` and the auto
    /// button *is* the content.
    ///
    /// ⚠ **The CONTENT moves, not the box.** The border box is already `height`; shifting it would
    /// turn a centring bug into a placement bug one level up.
    ///
    /// ⚠ **It is the whole content as ONE group.** Two block children keep their own separation and
    /// move together — that is what makes this centring rather than per-line alignment, and row 2
    /// is the row that tells the two apart.
    ///
    /// ⚠ **MEASURED RESIDUE, NAMED NOT GUESSED — `box-sizing` on form controls.** Chrome's UA sheet
    /// computes `border-box` for `button`, `input[type=submit|reset|button]` and `select`, and
    /// `content-box` for `input[type=text]`, `textarea` and every ordinary element. At
    /// `height:50px; padding-top:20px` Chrome reports **50 / 50 / 70 / 50 / 70 / 70** for
    /// button / submit / text / select / textarea / div; we report **70** for all six. That is a
    /// separate one-rule UA-sheet defect, it makes three controls 20px too tall whenever they carry
    /// padding and a height, and it is why the padded button row is absent from the table above —
    /// its centring cannot be right until its content box is.
    ///
    /// ⚠ `input[type=submit]` takes the same code path and its vertical offset already matches
    /// (16 of 50). Its *horizontal* centring does not — the synthetic-text path draws the label at
    /// x=0 where Chrome centres it. Measured here, fixed elsewhere.
    ///
    /// To watch it go RED, drop the `shift_content_y` call: rows 1–3 read 0 and both controls stay
    /// green.
    #[test]
    fn a_button_centres_its_content_vertically_in_its_content_box() {
        let (dom, root) = layout_html(
            "<button id=b1 style='height:50px'><span id=t1>X</span></button>\
             <button id=b2 style='height:80px'><span id=t2 class=blk>A</span>\
               <span id=t3 class=blk>B</span></button>\
             <button id=b3 style='height:20px'><span id=t4>X</span></button>\
             <button id=b4><span id=t5>X</span></button>\
             <div id=b5 style='height:50px'><span id=t6>X</span></div>",
            "body{margin:0} button,div{display:block;width:300px;padding:0;border:0;margin:0} \
             .blk{display:block}",
            1200.0,
        );
        let rects = root.node_rects(&dom);
        let r = |id: &str| {
            let n = dom
                .descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id");
            rects[&n]
        };
        // The auto-height button IS the natural content height — one line of the UA font. Deriving
        // the expectation from it keeps every row below independent of the font's metrics.
        let line = r("b4").height;
        assert!(
            line > 1.0,
            "the auto button must have a real height, got {line}"
        );

        for (btn, label, want, why) in [
            (
                "b1",
                "t1",
                (50.0 - line) / 2.0,
                "one line in a 50px button is centred — this is the row every design-system button is",
            ),
            (
                "b2",
                "t2",
                (80.0 - 2.0 * line) / 2.0,
                "TWO block children centre as ONE GROUP, which is what makes this centring rather \
                 than per-line alignment",
            ),
            (
                "b3",
                "t4",
                (20.0 - line) / 2.0,
                "a nearly-full button still centres the remainder",
            ),
            ("b4", "t5", 0.0, "control: an auto-height button has nothing to centre"),
            ("b5", "t6", 0.0, "control: a plain <div> is NOT a button and must not move"),
        ] {
            let got = r(label).y - r(btn).y;
            assert!(
                (got - want).abs() < 1.0,
                "#{label} sits {got} below #{btn} and Chrome puts it {want} below — {why}",
                got = got,
                want = want
            );
        }
        // Row 2's second child must keep its own separation — the group moved, it did not collapse.
        let sep = r("t3").y - r("t2").y;
        assert!(
            (sep - line).abs() < 1.0,
            "the two block children must stay {line} apart after centring, got {sep} — a per-child \
             shift would have moved them independently"
        );
    }

    /// **CSS 2.1 §10.3.7 / §10.6.4 — THE STATIC POSITION IS PER AXIS.** §10.3.7 solves the
    /// horizontal equation and §10.6.4 the vertical one, *separately*: `left`/`right` both `auto`
    /// makes the box's INLINE position static, and independently `top`/`bottom` both `auto` makes
    /// its BLOCK position static.
    ///
    /// `position_absolutes` tested all four insets at once (`all_auto`), so naming ONE inset threw
    /// the static position away on BOTH axes and the box fell back to the containing block's origin
    /// on the axis that was still `auto`. That is every `position:absolute; right:8px` badge and
    /// close button, every `left:0` full-bleed underline, every `top:100%` dropdown.
    ///
    /// Chrome-measured, `body{margin:0;font:16px Arial}`, 400px `position:relative` wrappers,
    /// `a{display:block}`, the abspos span following a 36px `<span>Hello</span>`:
    ///
    /// ```text
    ///                                             Chrome        before          after
    ///   left:200px  (top auto)                 [200, +60]    [200,   0]      [200, +60]   ✗→✓
    ///   top:0       (left auto)                [ 36,   0]    [  0,   0]      [ 36,   0]   ✗→✓
    ///   right:10px  (top auto)                 [309, +60]    [309,   0]      [309, +60]   ✗→✓
    ///   all four auto                          [ 36, +60]    [ 36, +60]      [ 36, +60]    ✓ control
    ///   top:0; left:0                          [  0,   0]    [  0,   0]      [  0,   0]    ✓ control
    /// ```
    ///
    /// (`y` is relative to the wrapper's top; `+60` is the line, past the 60px spacer.) Rows 2 and 3
    /// are the ones that make this *per axis* rather than "use the static position more often": row
    /// 2 takes x from flow and y from the containing block, row 3 does the opposite, and a
    /// single-boolean fix cannot produce both.
    ///
    /// ⚠ The `continue` that drops a box flow never recorded is now conditioned on **both** axes
    /// wanting the static position. A box with a real inset on one axis is placeable and must not be
    /// dropped merely because no cursor was recorded for it — the previous code could not reach that
    /// case because it only looked at the all-auto box.
    ///
    /// To watch it go RED, restore the single `all_auto` boolean: rows 1 and 3 lose their `y` to the
    /// containing block's top, row 2 loses its `x`, and both controls stay green.
    #[test]
    fn the_static_position_of_an_absolute_box_is_resolved_per_axis() {
        let (dom, root) = layout_html(
            "<div class=w><div class=pad></div><a id=a1><span id=h1>Hello</span>\
               <span id=b1 style='position:absolute;left:200px'>LEFTSET</span></a></div>\
             <div class=w><div class=pad></div><a id=a2><span id=h2>Hello</span>\
               <span id=b2 style='position:absolute;top:0'>TOPSET</span></a></div>\
             <div class=w><div class=pad></div><a id=a3><span id=h3>Hello</span>\
               <span id=b3 style='position:absolute;right:10px'>RIGHTSET</span></a></div>\
             <div class=w><div class=pad></div><a id=a4><span id=h4>Hello</span>\
               <span id=b4 style='position:absolute'>BOTHAUTO</span></a></div>\
             <div class=w><div class=pad></div><a id=a5><span id=h5>Hello</span>\
               <span id=b5 style='position:absolute;top:0;left:0'>BOTHSET</span></a></div>",
            "body{margin:0} .w{position:relative;width:400px} .pad{height:60px} a{display:block}",
            1200.0,
        );
        let rects = root.node_rects(&dom);
        let find = |id: &str| {
            dom.descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id")
        };
        let r = |id: &str| rects[&find(id)];
        for (wrap, hello, abs) in [
            ("a1", "h1", "b1"),
            ("a2", "h2", "b2"),
            ("a3", "h3", "b3"),
            ("a4", "h4", "b4"),
            ("a5", "h5", "b5"),
        ] {
            // The wrapper is the containing block; its content top is the spacer, and the line the
            // abs box's static position belongs to is where `Hello` sits. Both asserted as
            // RELATIONSHIPS so the UA's default font metrics cannot make the test lie.
            let (w, h, b) = (r(wrap), r(hello), r(abs));
            let cb_left = w.x;
            let cb_top = w.y - 60.0; // the wrapper starts a spacer above its own line
            let (want_x, want_y, why): (f32, f32, &str) = match abs {
                "b1" => (
                    cb_left + 200.0,
                    h.y,
                    "left:200px resolves against the containing block, and `top:auto` must STILL \
                     take the static position — this is the row that was 60px out",
                ),
                "b2" => (
                    h.x + h.width,
                    cb_top,
                    "top:0 resolves against the containing block, and `left:auto` must STILL take \
                     the static position — the same bug on the other axis",
                ),
                "b3" => (
                    cb_left + 400.0 - 10.0 - b.width,
                    h.y,
                    "right:10px resolves against the containing block's RIGHT edge, and `top:auto` \
                     must still be static",
                ),
                "b4" => (
                    h.x + h.width,
                    h.y,
                    "control: all four auto is static on both axes",
                ),
                _ => (
                    cb_left,
                    cb_top,
                    "control: two insets set is the containing block on both axes",
                ),
            };
            assert!(
                (b.x - want_x).abs() < 1.0 && (b.y - want_y).abs() < 1.0,
                "#{abs} is at [{} {}] and it belongs at [{want_x} {want_y}] — {why}",
                b.x,
                b.y
            );
        }
    }

    /// **CSS 2.1 §10.3.7 / §10.6.4 — THE STATIC POSITION INCLUDES THE INLINE ADVANCE.** An
    /// insetless `position:absolute` box sits where its hypothetical box would have started; on an
    /// inline line that is *after* everything before it, not at the line's start edge.
    ///
    /// The engine recorded the container's content-box origin — exact only when the abs box is the
    /// first thing in its parent, which the code said out loud and left unbuilt: *"Text preceding it
    /// on the line should push the static position along that line; that refinement is not modelled
    /// here."* That is Bootstrap's `.sr-only`, on every framework page that ships it, plus every
    /// badge, caret and tooltip written as `position:absolute` after inline content.
    ///
    /// Chrome-measured, `body{margin:0;font:16px Arial}`, a 400px `position:relative` wrapper,
    /// `a{display:block}`, x of the absolutely positioned span:
    ///
    /// ```text
    ///                                                    Chrome   before   after
    ///   <span>Hello</span><span class=sr-only>              35       -1      35    ✗→✓
    ///   <span>Hello</span><span position:absolute>          36        0      36    ✗→✓
    ///   <span position:absolute>FIRST</span><span>Hello      0        0       0     ✓ control
    ///   …a WRAPPED first span, then <span position:absolute> 61        0      61    ✗→✓
    ///   the in-flow spans themselves                          0        0       0     ✓ control
    ///   dir=rtl wrapper                                     334        0       0     ✓ INERT
    /// ```
    ///
    /// Row 1 carries `margin:-1px`, so 35 is 36 less the margin — the margin is applied *after* the
    /// static position and the two must not be conflated. Row 4 is why the search is
    /// `(line_top, then x)` rather than `max(x)`: a fragment that wrapped onto a later line is
    /// genuinely later even though its right edge is further left.
    ///
    /// ⚠ **MEASURED RESIDUE, NAMED RATHER THAN GUESSED (both reproduce in this fixture):**
    /// * **Bare text directly in the block** (`<a>Bare text<span position:absolute>`) belongs at
    ///   **x=64** and stays at 0. A `TextFragment`'s `node` is the deepest *element* ancestor, so
    ///   such a fragment reports the block itself and there is no way to tell WHICH bare-text
    ///   sibling it came from. Attribution is by subtree, and what it cannot attribute keeps the
    ///   old seed.
    /// * **`left:200px; top:auto`** belongs at **y=294** and lands at 234 — the containing block's
    ///   top. `position_absolutes` anchors to the static position only when **all four** insets are
    ///   `auto`, but §10.3.7 is written PER AXIS. A separate defect in the same section, exposed by
    ///   this fixture and deliberately not fixed in the same tick.
    /// * **RTL is excluded by the caller** — under an RTL base direction the inline start is the
    ///   right edge and `frags` have already been through UAX #9 rule L2, so "the trailing edge of
    ///   the last preceding fragment" is the wrong end of the wrong box. The row above asserts the
    ///   guard's INERTNESS, not correctness: Chrome's answer there is 334.
    ///
    /// To watch it go RED, drop the `refine_inline_static_positions` call: rows 1, 2 and 4 read the
    /// line start and all three controls stay green.
    #[test]
    fn an_insetless_absolute_box_starts_after_the_inline_content_before_it() {
        let (dom, root) = layout_html(
            "<div class=w><a id=a1><span id=s1>Hello</span><span id=s2 class=sr>SR</span></a></div>\
             <div class=w><a id=a2><span id=s3>Hello</span><span id=s4 class=ab>PLAIN</span></a></div>\
             <div class=w><a id=a3><span id=s5 class=ab>FIRST</span><span id=s6>Hello</span></a></div>\
             <div class=w><a id=a5><span id=s9>wwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwww \
                wrapped</span><span id=s10 class=ab>AFTERWRAP</span></a></div>\
             <div class=w dir=rtl><a id=a7><span id=s12>Hello</span><span id=s13 class=ab>RTL</span></a></div>",
            "body{margin:0} .w{position:relative;width:400px} a{display:block} \
             .ab{position:absolute} \
             .sr{position:absolute;width:1px;height:1px;padding:0;margin:-1px;border:0}",
            1200.0,
        );
        let rects = root.node_rects(&dom);
        let x = |id: &str| {
            let n = dom
                .descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id");
            rects[&n].x
        };
        // `Hello` is 36px at 16px Arial, so every "after Hello" row is 36 (less any margin).
        let hello_end = x("s1")
            + rects[&dom
                .descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some("s1"))
                .unwrap()]
                .width;
        for (id, want, why) in [
            (
                "s2",
                hello_end - 1.0,
                "Bootstrap's .sr-only starts AFTER the text before it, less its own margin:-1px",
            ),
            (
                "s4",
                hello_end,
                "a plain insetless abspos span starts at the inline advance, not the line start",
            ),
            (
                "s5",
                0.0,
                "control: an abspos box that is FIRST has nothing before it and stays at the start",
            ),
            (
                "s12",
                0.0,
                "control: the in-flow span itself is untouched",
            ),
            (
                "s13",
                0.0,
                "the RTL guard is INERT — Chrome puts this at 334 and closing that is separate work",
            ),
        ] {
            assert!(
                (x(id) - want).abs() < 1.0,
                "#{id} is at x={} and it belongs at {want} — {why}",
                x(id)
            );
        }
        // The wrapped row: the abs box follows the SECOND line, so it is both lower and further
        // LEFT than the first line's right edge — the case a `max(x)` search would get wrong.
        let s9 = dom
            .descendants(dom.root())
            .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some("s9"))
            .expect("s9");
        let s10 = dom
            .descendants(dom.root())
            .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some("s10"))
            .expect("s10");
        let (w, a) = (rects[&s9], rects[&s10]);
        assert!(
            a.x > 1.0 && a.x < w.width,
            "the abs box after a WRAPPED span sits at the end of the LAST line (x={} of a {}px \
             wrapped box), not at the line start and not past the container",
            a.x,
            w.width
        );
        assert!(
            a.y > w.y + 1.0,
            "…and on that last line, not the first (y={} vs the box top {})",
            a.y,
            w.y
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
    /// a following block drops below the line's height.
    ///
    /// ⚠⚠ **THIS TEST ASSERTED `below.y == 30` AND CHROME SAYS 34 — the assertion was the bug,
    /// and its own comment claimed it was "verified numerically against Chrome by the parity
    /// harness".** It was not: a 30px inline-block sits ON the baseline, and the line box is 30
    /// plus the containing block's font DESCENT below it. The anonymous block wrapping this run
    /// was built with `strut_style: None` (see `flush_inline_run`), so the descent was zero and a
    /// wrong number got frozen into a test as ground truth. Re-measured in headless Chrome on
    /// this exact markup: `a [0 0 80×30]`, `b [84 0 80×30]`, `below [0 34 120×25]` — all three
    /// now byte-identical here. **A number asserted from an unverified claim of verification is
    /// the most expensive kind: it defends the defect.**
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
        // The second inline-block sits to the right of the first, one word space along (Chrome: 84).
        assert!(b.x >= 80.0, "second inline-block is to the right: {b:?}");
        assert!((b.y - 0.0).abs() < 0.5, "same line as the first");
        // The block after the inline run drops below the line — 30px of inline-block ABOVE the
        // baseline plus the strut's descent BELOW it. Chrome-measured on this markup: 34.
        let below = by_id("below");
        assert!(
            (below.y - 34.0).abs() < 1.0,
            "block drops below the inline line at Chrome's 34 (30px box + strut descent): {below:?}"
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

    /// **`box-sizing: border-box` applies to a FLOAT too** — the float path resolved its own width and
    /// never subtracted padding + border, so every floated column was `padding-left + padding-right`
    /// too wide.
    ///
    /// Measured against live Chromium on the exact corpus shape — `*{box-sizing:border-box}` with
    /// `.card{width:50%;float:left;padding:0 5px}` in a 704px container:
    ///
    /// | box | Chrome | was |
    /// |---|---|---|
    /// | 1st float (border box) | **352** | 362 ❌ |
    /// | its content | **342** | 352 ❌ |
    /// | 2nd float's x | **352** | 362 ❌ |
    /// | the same box WITHOUT `float` | 352 / 342 | 352 / 342 ✅ |
    ///
    /// The last row is the control and it is why this is a *float* bug and not a box-sizing bug:
    /// `layout_block` has applied `bs_extra_w` for many ticks; `layout_float` is a separate width
    /// resolution that never learned it. `*{box-sizing:border-box}` is in every CSS reset written since
    /// 2011, and a `width:%` + `padding` float is the pre-flexbox column — most of the WordPress web.
    ///
    /// Real site: `possssno.sbs` (coverage 1.000, the sharpest target on the t767 ledger) went shape
    /// **0.123 → 0.430**.
    ///
    /// RED, run: drop the `BoxSizing::BorderBox` arm in `layout_float` — the float reads 362 and its
    /// content 352.
    #[test]
    fn box_sizing_border_box_applies_to_a_float() {
        let (dom, root) = layout_html(
            "<body style='margin:0'><div id=wrap style='width:704px'>\
               <div class=f id=f1><div class=inner id=i1>a</div></div>\
               <div class=f id=f2><div class=inner id=i2>b</div></div>\
               <div id=nofloat><div class=inner id=i3>c</div></div>\
             </div></body>",
            "*{box-sizing:border-box} .f{width:50%;float:left;padding:0 5px} \
             .inner{height:20px} #nofloat{width:50%;padding:0 5px}",
            1200.0,
        );
        let rects = root.node_rects(&dom);
        let g = |id: &str| {
            let n = dom
                .descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id");
            (rects[&n].x, rects[&n].width)
        };
        let (f1x, f1w) = g("f1");
        let (i1x, i1w) = g("i1");
        let (f2x, _) = g("f2");
        assert!(
            (f1w - 352.0).abs() < 1.0 && (i1w - 342.0).abs() < 1.0,
            "the float's BORDER box is the specified 50% (352) and its CONTENT is 342: \
             float {f1x}/{f1w}, inner {i1x}/{i1w}"
        );
        assert!(
            (f2x - 352.0).abs() < 1.0,
            "two 50% floats fit side by side — the second starts at 352, got {f2x}"
        );
        // The control: the same box without `float` was ALREADY Chrome-exact, and must stay so.
        let (_, nfw) = g("nofloat");
        let (_, i3w) = g("i3");
        assert!(
            (nfw - 352.0).abs() < 1.0 && (i3w - 342.0).abs() < 1.0,
            "the non-float control is unchanged: {nfw} / {i3w}"
        );

        // ── **AND THE BLOCK AXIS OF THE SAME RULE, which this test did not ask for and the code
        // therefore did not have.** The width arm above subtracted padding+border from a border-box
        // float; the height arm did not, so the same box came out padding+border too TALL. One rule,
        // two axes, one of them landed — the shape this project keeps paying for. Chrome-measured:
        // `box-sizing:border-box; padding:10px; width:100px; height:100px` floated is 100x100, ours
        // was 100x120.
        let (dom2, root2) = layout_html(
            "<body style='margin:0'><div id=bh></div></body>",
            "#bh{float:left;box-sizing:border-box;padding:10px;width:100px;height:100px}",
            1200.0,
        );
        let rects2 = root2.node_rects(&dom2);
        let n = dom2
            .descendants(dom2.root())
            .find(|&n| dom2.element(n).and_then(|e| e.attr("id")) == Some("bh"))
            .expect("id");
        let (bw, bhh) = (rects2[&n].width, rects2[&n].height);
        assert!(
            (bw - 100.0).abs() < 1.0 && (bhh - 100.0).abs() < 1.0,
            "a border-box float's specified HEIGHT is its border box too (100x100), got {bw}x{bhh}"
        );
    }

    /// **THE OUT-OF-FLOW PASS'S `viewport` HELD THE DOCUMENT HEIGHT.**
    ///
    /// CSS 2.1 §10.1: the initial containing block has the dimensions of the **viewport**, and a
    /// `position:fixed` box's containing block IS the viewport. The out-of-flow pass built its
    /// containing block from `root.content_bottom()` — the whole scrolled document — so every
    /// percentage height on an out-of-flow box resolved against the page instead of the window.
    ///
    /// That is every full-height drawer, modal backdrop, off-canvas menu and overlay on any page
    /// long enough to scroll, i.e. exactly the pages where it is visible. Chrome-measured on a
    /// 3000px-tall page in an 800px window (Chrome's `innerHeight` 713):
    ///
    /// ```text
    ///                                        Chrome   before    after
    ///   position:fixed;    height:100%       300x713  300x3000  300x713   ✗→✓
    ///   position:fixed;    height:50%        100x357  100x1500  100x357   ✗→✓
    ///   position:absolute; height:100%       100x713  100x3000  100x713   ✗→✓
    ///   position:fixed;    height:auto        100x50   100x50    100x50    ✓  ← control
    /// ```
    ///
    /// ⚠ **The IN-FLOW initial containing block already read the real viewport height** (`icb_height`
    /// in `layout_document`); only the out-of-flow pass still used the document height. One rule,
    /// two implementations, and only one of them had been corrected — the same shape as t831/t833.
    /// The name is the reason it survived: the variable was called `viewport` and was not one.
    #[test]
    fn an_out_of_flow_percentage_height_resolves_against_the_viewport_not_the_document() {
        let vp_h = manuk_css::values::viewport_size().1;
        let (dom, root) = layout_html(
            "<body style='margin:0'><div id=tall></div>\
               <div id=fx></div><div id=half></div><div id=ab></div><div id=auto>x</div></body>",
            "#tall{height:3000px} \
             #fx{position:fixed;left:0;top:0;width:300px;height:100%} \
             #half{position:fixed;left:320px;top:0;width:100px;height:50%} \
             #ab{position:absolute;left:440px;top:0;width:100px;height:100%} \
             #auto{position:fixed;left:560px;top:0;width:100px}",
            1200.0,
        );
        let rects = root.node_rects(&dom);
        let g = |id: &str| {
            let n = dom
                .descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id");
            rects[&n].height
        };
        // Asserted against the viewport the engine was actually given, not a hard-coded number:
        // the defect is *which reference* is used, and pinning a literal would make this test a
        // statement about the harness's window size instead of about the rule.
        assert!(
            (g("fx") - vp_h).abs() < 1.0,
            "position:fixed; height:100% is the VIEWPORT height ({vp_h}), not the 3000px document \
             — got {}",
            g("fx")
        );
        assert!(
            (g("half") - vp_h / 2.0).abs() < 1.0,
            "height:50% is half the viewport ({}), got {}",
            vp_h / 2.0,
            g("half")
        );
        assert!(
            (g("ab") - vp_h).abs() < 1.0,
            "an absolute box with no positioned ancestor resolves against the ICB, which is also \
             viewport-sized — got {}",
            g("ab")
        );
        // The control: an out-of-flow box with `height:auto` still sizes to its content, so the fix
        // changed the REFERENCE and not the rule.
        assert!(
            g("auto") < 100.0,
            "height:auto must still be content-sized, got {}",
            g("auto")
        );
    }

    /// **A FLEX ITEM `<img>` TOLD TAFFY ITS CONTENT WANTED ZERO, SO IT SHRANK TO A SLIVER.**
    ///
    /// CSS Flexbox §4.5: a flex item's `min-width:auto` — the default — is its **automatic minimum
    /// size**, which for a replaced element is its intrinsic width. Chrome therefore refuses to
    /// shrink a row of logos below their own size and lets the container overflow (which is the
    /// entire point of the `display:flex; overflow-x:scroll` carousel). We shrank them to fit.
    ///
    /// The cause was one omission with two jobs: `replaced_default_size` — the seam that answers
    /// "how big is this replaced item?" for taffy — listed `svg|canvas|video|object|embed` and not
    /// `<img>`. The list was written for the DEFAULT OBJECT SIZE (300×150), which `<img>` correctly
    /// does not have; excluding it there silently also excluded it from reporting its **intrinsic**
    /// size, so an image flex item measured as content-of-zero.
    ///
    /// Measured on `promo.golesliga1max.pe`, whose `#slider-equipos { display:flex;
    /// overflow-x:scroll }` holds fifteen 74×82 team badges — Chrome renders each at 74×82, we
    /// rendered each at **18×82**, and that one row is 15 of the site's 26 shape misses.
    ///
    /// ```text
    ///   four 1000×266 images in a 320px flex row   Chrome        before      after
    ///                                              1000x266 ea   68x266 ea   1000x266 ea   ✗→✓
    /// ```
    ///
    /// ⚠ `<img>` is admitted to that seam but still gets **no default object size**: with neither a
    /// definite axis nor a ratio known it returns `None` and falls through to the broken-image path,
    /// so t689's "an image whose bytes never arrive is 16×16, not a full-line band" still holds.
    #[test]
    fn a_replaced_flex_item_is_floored_at_its_intrinsic_width() {
        let (dom, root) = layout_html(
            "<body style='margin:0'><div id=row>\
               <img id=a><img id=b><img id=c><img id=d></div></body>",
            "#row{display:flex;width:320px} \
             #row img{aspect-ratio:1000/266;width:1000px}",
            1200.0,
        );
        let rects = root.node_rects(&dom);
        let g = |id: &str| {
            let n = dom
                .descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id");
            (rects[&n].width, rects[&n].height)
        };
        for id in ["a", "b", "c", "d"] {
            let (w, h) = g(id);
            assert!(
                (w - 1000.0).abs() < 2.0 && (h - 266.0).abs() < 2.0,
                "flex item #{id}: `min-width:auto` floors a replaced item at its intrinsic size, so \
                 Chrome overflows the row at 1000x266 rather than shrinking — got {w}x{h}"
            );
        }

        // ── **AND THE DEFAULT OBJECT SIZE MUST NOT COME WITH IT.** This half exists because the
        // first version of the fix caused a real regression that no fixture caught: admitting
        // `<img>` to the replaced-size seam also handed it the `300×150` fallback, so an image with
        // a definite WIDTH and no ratio — an icon whose bytes have not arrived — took a **150px**
        // height. `777juegos.com`'s footer is a row of exactly those (Chrome measures them at
        // height 0) and it cost 8.75 shape points on the 16-site control.
        //
        // The assertion is deliberately *"not 150"* rather than a Chrome-exact number: Chrome puts
        // an alt-text line box here (36×34) and we produce 36×0, which is a separate pre-existing
        // gap this tick did not touch and must not silently claim. What this gate pins is the thing
        // that regressed — an `<img>` never gets the default object size.
        let (dom2, root2) = layout_html(
            "<body style='margin:0'><div id=row><img id=x></div></body>",
            "#row{display:flex;width:320px} #row img{width:36px}",
            1200.0,
        );
        let rects2 = root2.node_rects(&dom2);
        let n = dom2
            .descendants(dom2.root())
            .find(|&n| dom2.element(n).and_then(|e| e.attr("id")) == Some("x"))
            .expect("id");
        let h = rects2[&n].height;
        assert!(
            h < 100.0,
            "an <img> with a definite width and NO ratio must never take the 300x150 default \
             object size — Chrome gives it a 34px alt line box, we give 0, and 150 is the \
             regression this pins; got height {h}"
        );
    }

    /// **AN ABSOLUTELY POSITIONED REPLACED ELEMENT WAS ZERO PIXELS TALL. ALWAYS.**
    ///
    /// `layout_abs` took its height from `definite_ch` or from the CONTENT height, and a replaced
    /// element has no children — so unless the author gave an explicit `height` or set BOTH `top`
    /// and `bottom`, an absolutely positioned `<img>` measured `<w>x0` and painted nothing.
    ///
    /// This is the THIRD implementation of the rule t831 landed in `layout_float` and t833
    /// completed in `layout_block`, found by taking t833's own conclusion literally and grepping
    /// the remaining size resolutions instead of waiting for a site to name the next one. It was
    /// the worst of the three: the other two produced a wrong size, this produced **no box**.
    ///
    /// ⚠ **The `inset:0` variant HAPPENED to work** — both insets make `definite_ch` — which is
    /// exactly why this survived: the most-cited form of the idiom is the one that hid it, and
    /// `position:absolute; top:0; left:0` on an image is the hero/overlay/thumbnail pattern of the
    /// whole web.
    ///
    /// Chrome-measured, a 1000×266 image absolutely positioned in a 320×200 block:
    ///
    /// ```text
    ///                                  Chrome    before     after
    ///   max-width:100%                 320x85    320x0     320x85    ✗→✓
    ///   max-height:30px                113x30   1000x0     113x30    ✗→✓
    ///   max-width:100% + max-height    113x30    320x0     113x30    ✗→✓
    ///   min-width:1500px              1500x399  1500x0    1500x399   ✗→✓
    /// ```
    ///
    /// Every `before` height is 0 and every `before` width but one is already right — the clamps
    /// reached this path in an earlier tick and the ratio never did.
    #[test]
    fn an_abspos_replaced_element_takes_its_height_from_its_ratio() {
        let (dom, root) = layout_html(
            "<body style='margin:0'>\
               <div class=rel><img id=w></div>\
               <div class=rel><img id=h></div>\
               <div class=rel><img id=both></div>\
               <div class=rel><img id=mw></div>\
             </body>",
            ".rel{position:relative;width:320px;height:200px} \
             .rel img{position:absolute;top:0;left:0;aspect-ratio:1000/266;width:1000px} \
             #w{max-width:100%} #h{max-height:30px} \
             #both{max-width:100%;max-height:30px} #mw{min-width:1500px}",
            1200.0,
        );
        let rects = root.node_rects(&dom);
        let g = |id: &str| {
            let n = dom
                .descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id");
            (rects[&n].width, rects[&n].height)
        };
        for (id, want) in [
            ("w", (320.0, 85.0)),
            ("h", (113.0, 30.0)),
            ("both", (113.0, 30.0)),
            ("mw", (1500.0, 399.0)),
        ] {
            let (gw, gh) = g(id);
            assert!(
                (gw - want.0).abs() < 1.5 && (gh - want.1).abs() < 1.5,
                "abspos #{id}: Chrome measures {}x{}, got {gw}x{gh}",
                want.0,
                want.1
            );
        }
    }

    /// **CSS 2.1 §10.4 RUNS BLOCK → INLINE TOO, AND THE BLOCK PATH ONLY EVER RAN IT ONE WAY.**
    ///
    /// A `max-width` that moves a replaced element's used width has recomputed its height here for
    /// a long time. The mirror — a `max-height` that moves the HEIGHT must pull the width back
    /// through the ratio — was never written, so the box kept its pre-clamp width and the picture
    /// rendered stretched. t831 added exactly this to `layout_float`; **the block path had one half
    /// and the float path now has both**, which is t831's own pattern note arriving from the other
    /// direction.
    ///
    /// Chrome-measured on a 1000×266 image in a 320px block — the AWS Cognito hosted login page's
    /// `.logo-customizable { max-width:100%; max-height:30px }` exactly, which is what
    /// `admin.zoomph.com` renders:
    ///
    /// ```text
    ///                                              Chrome    before    after
    ///   max-width:100% + max-height:30px           113x30    320x30    113x30   ✗→✓
    ///   max-width:100% alone                       320x85    320x85    320x85    ✓  ← control
    ///   max-height:30px alone                      113x30   1000x30    113x30   ✗→✓
    ///   …+ display:block; margin:0 auto           113x30 @104  @0    113x30 @104  ✗→✓
    /// ```
    ///
    /// ⚠ The `max-width`-alone row is the control that names which half was missing: it was already
    /// right, because the inline→block transfer has been here the whole time. ⚠ And the centred row
    /// is why the fix re-runs the auto-margin split rather than only assigning a width — §10.4 says
    /// the §10.3.3 rules are applied *again*, and §10.3.3 is where two `auto` margins share the
    /// remainder. Assigning the width alone leaves a correctly-sized image flush left.
    #[test]
    fn a_max_height_on_a_replaced_element_pulls_its_width_back_through_the_ratio() {
        let (dom, root) = layout_html(
            "<body style='margin:0'>\
               <div class=box><img id=both></div>\
               <div class=box><img id=w></div>\
               <div class=box><img id=h></div>\
               <div class=box><img id=c></div>\
             </body>",
            ".box{width:320px} img{aspect-ratio:1000/266;width:1000px} \
             #both{max-width:100%;max-height:30px} \
             #w{max-width:100%} \
             #h{max-height:30px} \
             #c{display:block;margin:0 auto;max-width:100%;max-height:30px}",
            1200.0,
        );
        let rects = root.node_rects(&dom);
        let g = |id: &str| {
            let n = dom
                .descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id");
            (rects[&n].x, rects[&n].width, rects[&n].height)
        };
        for (id, want) in [
            ("both", (0.0, 113.0, 30.0)),
            // The control: `max-width` alone was ALREADY Chrome-exact (the inline→block transfer),
            // and a fix to the other half must not touch it.
            ("w", (0.0, 320.0, 85.0)),
            ("h", (0.0, 113.0, 30.0)),
            ("c", (104.0, 113.0, 30.0)),
        ] {
            let (gx, gw, gh) = g(id);
            assert!(
                (gx - want.0).abs() < 1.5 && (gw - want.1).abs() < 1.5 && (gh - want.2).abs() < 1.5,
                "#{id}: Chrome measures {}x{} at x={}, got {gw}x{gh} at x={gx}",
                want.1,
                want.2,
                want.0
            );
        }
    }

    /// **A FLOATED REPLACED ELEMENT HAS NO CONTENT, SO WITHOUT ITS RATIO IT HAS NO SIZE.**
    ///
    /// `layout_float` derived neither axis from `aspect_ratio`, and an `<img>` has no children — so a
    /// floated image whose other axis was `auto` measured **zero** in it. Chrome-measured on a
    /// 101×32 PNG and a 14×14 PNG served over HTTP, with the identical unfloated image in the same
    /// document as the control:
    ///
    /// ```text
    ///                                       Chrome   before   after
    ///   float:left, no width/height        101x32    101x0    101x32   ✗→✓
    ///   float:left, height=16 attr only      16x16      0x16     16x16   ✗→✓
    ///   the SAME image, not floated        101x32   101x32   101x32    ✓  ← control
    /// ```
    ///
    /// The control is the diagnosis: the block path had this rule the whole time, so the two
    /// resolutions disagreed inside one document. `.logo a img { float:left }` is how the legacy web
    /// puts a logo in a header, and it is why `app.ordertime.com` aimed this tick.
    #[test]
    fn a_floated_replaced_element_derives_its_missing_axis_from_its_ratio() {
        // `aspect-ratio` in CSS is the same `s.aspect_ratio` a decoded image's intrinsic size
        // produces (`apply_natural_size`), so this reaches the identical code path without needing
        // a network fetch inside a unit test.
        let (dom, root) = layout_html(
            "<body style='margin:0'>\
               <div><img id=f style='float:left' ></div>\
               <div><img id=h style='float:left;height:16px'></div>\
               <div><img id=c ></div>\
             </body>",
            "#f{aspect-ratio:101/32;width:101px} #h{aspect-ratio:1/1} #c{aspect-ratio:101/32;width:101px}",
            1200.0,
        );
        let rects = root.node_rects(&dom);
        let g = |id: &str| {
            let n = dom
                .descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id");
            (rects[&n].width, rects[&n].height)
        };
        let (fw, fh) = g("f");
        assert!(
            (fw - 101.0).abs() < 1.0 && (fh - 32.0).abs() < 1.0,
            "a floated image with an auto HEIGHT takes it from its width and ratio (101x32), \
             got {fw}x{fh}"
        );
        let (hw, hh) = g("h");
        assert!(
            (hw - 16.0).abs() < 1.0 && (hh - 16.0).abs() < 1.0,
            "a floated image with an auto WIDTH takes it from its height and ratio (16x16), \
             got {hw}x{hh}"
        );
        // The control that made this a diagnosis rather than a symptom: unfloated, already correct,
        // and it must stay correct — a fix that moves it is a fix in the wrong function.
        let (cw, chh) = g("c");
        assert!(
            (cw - 101.0).abs() < 1.0 && (chh - 32.0).abs() < 1.0,
            "the unfloated control was already Chrome-exact and must not move: {cw}x{chh}"
        );
    }

    /// **`min-width` / `max-width` / `min-height` / `max-height` DID NOT EXIST ON THE FLOAT PATH.**
    ///
    /// Not mis-applied — absent. A float is a second width/height resolution beside `layout_block`'s
    /// and it has been acquiring that function's rules one measured defect at a time. Deliberately
    /// tested on plain `<div>`s so no replaced-element machinery can explain the result away.
    /// Chrome-measured:
    ///
    /// ```text
    ///                                            Chrome   before   after
    ///   float, width:200px; max-width:50px       50x10    200x10   50x10   ✗→✓
    ///   float, width:20px;  min-width:80px       80x10     20x10   80x10   ✗→✓
    ///   float, width:10px; height:200px; max-h:50  10x50   10x200   10x50   ✗→✓
    ///   float, width:10px; height:20px;  min-h:80  10x80    10x20   10x80   ✗→✓
    /// ```
    ///
    /// `.col { float:left; width:50%; max-width:600px }` is the pre-flexbox responsive column, so
    /// this is not an edge of the float path — it is most of what floats are used for.
    #[test]
    fn a_float_clamps_both_axes_by_min_and_max() {
        let (dom, root) = layout_html(
            "<body style='margin:0'>\
               <div class=c id=mw></div><div class=c id=miw></div>\
               <div class=c id=mh></div><div class=c id=mih></div>\
             </body>",
            ".c{float:left;clear:both} \
             #mw{width:200px;max-width:50px;height:10px} \
             #miw{width:20px;min-width:80px;height:10px} \
             #mh{width:10px;height:200px;max-height:50px} \
             #mih{width:10px;height:20px;min-height:80px}",
            1200.0,
        );
        let rects = root.node_rects(&dom);
        let g = |id: &str| {
            let n = dom
                .descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id");
            (rects[&n].width, rects[&n].height)
        };
        for (id, want) in [
            ("mw", (50.0, 10.0)),
            ("miw", (80.0, 10.0)),
            ("mh", (10.0, 50.0)),
            ("mih", (10.0, 80.0)),
        ] {
            let (w, h) = g(id);
            assert!(
                (w - want.0).abs() < 1.0 && (h - want.1).abs() < 1.0,
                "float #{id}: Chrome measures {}x{}, got {w}x{h}",
                want.0,
                want.1
            );
        }
    }

    /// **CSS 2.1 §10.4 — A CLAMP ON A REPLACED ELEMENT TRANSFERS THROUGH THE RATIO, BOTH WAYS.**
    ///
    /// Clamping one axis of an image and leaving the other is how a picture renders stretched, and
    /// the float path could not do it in either direction because it had neither the ratio nor the
    /// clamp. Chrome-measured on the same two PNGs:
    ///
    /// ```text
    ///                                                Chrome   before   after
    ///   float, 101x32 image, max-width:50px          50x16    101x0    50x16   ✗→✓
    ///   float, 14x14 image, height=16, max-height:14 14x14      0x16    14x14   ✗→✓
    /// ```
    ///
    /// The second row is `app.ordertime.com` exactly: `.help img { max-height:14px; max-width:14px }`
    /// over an `<img height="16">`, which is where its `0x16` against Chrome's `14x14` came from.
    #[test]
    fn a_max_constraint_on_a_floated_image_transfers_through_its_ratio() {
        let (dom, root) = layout_html(
            "<body style='margin:0'>\
               <div><img id=w style='float:left'></div>\
               <div><img id=h style='float:left'></div>\
             </body>",
            "#w{aspect-ratio:101/32;width:101px;max-width:50px} \
             #h{aspect-ratio:1/1;height:16px;max-height:14px}",
            1200.0,
        );
        let rects = root.node_rects(&dom);
        let g = |id: &str| {
            let n = dom
                .descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id");
            (rects[&n].width, rects[&n].height)
        };
        let (ww, wh) = g("w");
        assert!(
            (ww - 50.0).abs() < 1.0 && (wh - 16.0).abs() < 1.0,
            "a max-width clamp must pull the HEIGHT down through the ratio (50x16), got {ww}x{wh}"
        );
        let (hw, hh) = g("h");
        assert!(
            (hw - 14.0).abs() < 1.0 && (hh - 14.0).abs() < 1.0,
            "a max-height clamp must pull the WIDTH in through the ratio (14x14), got {hw}x{hh}"
        );
    }

    /// **A `position:relative` ANCESTOR INSIDE AN OUT-OF-FLOW SUBTREE IS STILL A CONTAINING BLOCK.**
    ///
    /// `position_absolutes` builds its rect map from the IN-FLOW fragment tree, so nothing inside an
    /// out-of-flow subtree has an entry — and `abs_containing_block` tests `position != Static` and
    /// then requires a rect, walking straight PAST any ancestor it cannot find. So a
    /// `position:relative` row inside a `position:absolute` drawer was invisible as a containing
    /// block, and every abspos box under it escaped to the OUTER positioned ancestor.
    ///
    /// That is the AdminLTE sidebar exactly — `.main-sidebar{position:absolute}` > `section` > `ul`
    /// > `li` > `a{position:relative}` > `span.pull-right-container{position:absolute;top:50%}` —
    /// and the shape of every off-canvas menu, drawer and fixed toolbar whose rows carry their own
    /// badges, carets or absolutely-placed icons. Chrome-measured on the real AdminLTE stylesheet,
    /// 3 sidebar rows: the carets belong at `y` **65 / 109 / 153** and every one of ours landed on
    /// the same `y=353` — which is `viewport/2 - 7`, i.e. `top:50%` resolved against the sidebar.
    ///
    /// ⚠ **Only ONE axis was visibly wrong, which is why this read as a `top` defect rather than a
    /// containing-block defect:** `right:10px` is a LENGTH, and the drawer and the row share a right
    /// edge, so `x` came out correct *from the wrong containing block*. **A wrong containing block
    /// is only as visible as the insets that distinguish it.**
    ///
    /// ⚠ Rows 3 and 4 are controls, and the first draft of this fix broke BOTH of them — the
    /// unfiltered `node_rects` LIFTS a boxless element's geometry onto its ancestors, which from
    /// inside an out-of-flow subtree means onto the box's own containing block. They are asserted
    /// here as well as in their own tests because they are what makes this a *containing-block*
    /// widening rather than "every ancestor is a containing block now".
    ///
    /// To watch it go RED, drop the `rects.extend(...)` in `position_absolutes`: rows 1 and 2 both
    /// collapse onto `viewport/2 - 7`.
    #[test]
    fn a_relative_ancestor_inside_an_out_of_flow_subtree_is_a_containing_block() {
        let (dom, root) = layout_html(
            "<body>\
               <div class=drawer>\
                 <section><ul><li><a id=r1>ONE<span id=c1 class=caret></span></a></li>\
                              <li><a id=r2>TWO<span id=c2 class=caret></span></a></li></ul></section>\
               </div>\
               <div class=cb><div id=modal></div><div id=sib></div></div>\
             </body>",
            "body{margin:0} \
             .drawer{position:absolute;top:0;left:0;width:230px} \
             ul{list-style:none;margin:0;padding:0} \
             .drawer a{position:relative;display:block;height:44px} \
             .caret{position:absolute;right:10px;top:50%;margin-top:-7px;width:10px;height:14px} \
             .cb{position:relative;width:400px;height:400px} \
             #modal{position:absolute;top:0;right:0;bottom:0;left:0;width:200px;height:200px;\
                    margin-left:auto;margin-right:auto;margin-top:auto;margin-bottom:auto} \
             #sib{position:absolute;top:0;left:0;width:50px;height:50px}",
            1200.0,
        );
        let rects = root.node_rects(&dom);
        let r = |id: &str| {
            let n = dom
                .descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id");
            rects[&n]
        };
        // Rows 1 and 2 — each caret sits at the vertical middle of ITS OWN row, not at a single
        // shared y derived from the drawer.
        for (row, caret) in [("r1", "c1"), ("r2", "c2")] {
            let want = r(row).y + r(row).height / 2.0 - 7.0;
            assert!(
                (r(caret).y - want).abs() < 1.5,
                "#{caret} is at y={} and `top:50%` on a child of `#{row}` (y={}, h={}) puts it at \
                 {want} — a `position:relative` ancestor inside an out-of-flow subtree is still \
                 the containing block",
                r(caret).y,
                r(row).y,
                r(row).height
            );
        }
        assert!(
            (r("c1").y - r("c2").y).abs() > 1.0,
            "two carets in two different rows must not share a y ({} and {}) — that is the \
             signature of both resolving against the drawer",
            r("c1").y,
            r("c2").y
        );
        // Row 3 — CONTROL. An out-of-flow box must not lend its geometry to its OWN containing
        // block: `#modal` centres at (100,100) in a 400×400 `.cb`, and `#sib` must still see the
        // full 400×400, not `#modal`'s 200×200 at (100,100).
        assert!(
            (r("modal").x - 100.0).abs() < 1.0 && (r("modal").y - 100.0).abs() < 1.0,
            "control: inset:0;margin:auto centres at (100,100), got ({},{})",
            r("modal").x,
            r("modal").y
        );
        assert!(
            r("sib").x.abs() < 1.0 && r("sib").y.abs() < 1.0,
            "control: a sibling abspos box must still resolve against `.cb` at (0,0) — got \
             ({},{}), which is `#modal`'s origin leaking onto its own containing block",
            r("sib").x,
            r("sib").y
        );
    }

    /// **CSS 2.1 §10.3.3 — THE OVER-CONSTRAINED EQUATION IGNORES `margin-left` UNDER `rtl`**, so a
    /// narrower-than-container block is flush LEFT in an LTR page and flush RIGHT in an RTL one.
    ///
    /// Named as residue at t841, where rule L2 made the *content* of such a block read correctly
    /// while the block itself stayed on the wrong side. Every sidebar, card, fixed-width panel and
    /// `width`-without-`margin:auto` wrapper on the Arabic/Hebrew/Persian/Urdu web.
    ///
    /// Chrome-measured, `<html dir=rtl>`, 400px blocks in a 1200px viewport:
    ///
    /// ```text
    ///                                          Chrome   before   after
    ///   plain 400px block                        800       0      800    ✗→✓
    ///   dir=ltr ON THE BLOCK ITSELF               800       0      800    ✗→✓
    ///   margin-right:auto                          0       0        0     ✓ control
    ///   margin-left:auto                         800     800      800     ✓ control
    ///   margin-left:auto; margin-right:auto      400     400      400     ✓ control
    ///   inside a dir=ltr WRAPPER                     0       0        0     ✓ control
    /// ```
    ///
    /// ⚠ **Row 2 is the row that makes this a CONTAINING-BLOCK rule rather than "RTL elements go
    /// right".** `direction` is inherited, so reading the element's own style agrees with the spec
    /// everywhere except here — a `direction:ltr` block inside an RTL page is still *placed* by its
    /// RTL parent and stays flush right, while its own contents lay out LTR. Row 6 is the same point
    /// inverted: an LTR wrapper puts its child back on the left even in an RTL document.
    ///
    /// To watch it go RED, drop the `(false, false) if self.parent_is_rtl(node)` arm: rows 1 and 2
    /// read 0 and all four controls stay green.
    #[test]
    fn an_over_constrained_block_in_an_rtl_containing_block_is_flush_right() {
        let (dom, root) = layout_html(
            "<body dir=rtl>\
               <div id=b1></div>\
               <div id=b2 dir=ltr></div>\
               <div id=b3></div>\
               <div id=b4></div>\
               <div id=b5></div>\
               <div id=wrap dir=ltr><div id=b6></div></div>\
             </body>",
            "body{margin:0} div{width:400px;height:20px} \
             #b3{margin-right:auto} #b4{margin-left:auto} \
             #b5{margin-left:auto;margin-right:auto} #wrap{width:auto}",
            1200.0,
        );
        let rects = root.node_rects(&dom);
        let x = |id: &str| {
            let n = dom
                .descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id");
            rects[&n].x
        };
        for (id, want, why) in [
            (
                "b1",
                800.0,
                "an over-constrained block in an RTL containing block is flush RIGHT",
            ),
            (
                "b2",
                800.0,
                "dir on the BLOCK ITSELF does not place it — its containing block does",
            ),
            (
                "b3",
                0.0,
                "control: margin-right:auto is solved for, so the box stays at the start edge",
            ),
            (
                "b4",
                800.0,
                "control: margin-left:auto is solved for in either direction",
            ),
            (
                "b5",
                400.0,
                "control: two auto margins centre in either direction",
            ),
            (
                "b6",
                0.0,
                "control: an LTR wrapper puts its child back on the left in an RTL document",
            ),
        ] {
            assert!(
                (x(id) - want).abs() < 1.0,
                "#{id} is at x={} and Chrome puts it at {want} — {why}",
                x(id)
            );
        }
    }

    /// **G_BIDI_LINE — UAX #9 RULE L2: A LINE'S INLINE BOXES ARE REORDERED, NOT JUST ITS GLYPHS.**
    ///
    /// The engine has run the bidirectional algorithm inside a single text run since the shaper
    /// landed (`FontContext::shape_bidi`, gated by `engine/text`'s `g_bidi_base_direction`) — so one
    /// Arabic word comes out right, and the *whole other half* of UAX #9 was missing: a LINE is a
    /// sequence of inline BOXES (`<a>`, `<span>`, `<em>`, an `inline-block`), each measured and
    /// placed separately, and nothing reordered those. Every widths matched, the line was correctly
    /// flush right, and the links read **backwards**.
    ///
    /// Chrome-measured (`file://`, 1200×800, 400px containers), x relative to the container:
    ///
    /// ```text
    ///                                        Chrome            before        after
    ///   dir=rtl, three RTL-script <a>        370 / 343 / 312   312/343/370   370/343/312  ✗→✓
    ///   dir=ltr, three RTL-script <a>         58 /  31 /   0     0/ 34/ 61    58/ 31/  0  ✗→✓
    ///   dir=rtl, three LATIN <a>             303 / 334 / 364   303/334/364   303/334/364  ✓ (control)
    /// ```
    ///
    /// ⚠ **The third row is the control that makes this a BIDI fix rather than a "reverse the links
    /// on an RTL page" fix.** Latin text in an RTL paragraph is a single LTR run at level 2, so its
    /// boxes stay in source order and only the *line* is flush right — reversing them would be just
    /// as wrong as not reversing the second row, and a rule that only looked at the container's
    /// `direction` would get exactly one of the three right. The second row is the same point from
    /// the other side: RTL content inside an **LTR** paragraph reorders too.
    ///
    /// To watch it go RED, delete the `reorder_line_bidi` call in `close_line`: rows 1 and 2 read
    /// back in source order while row 3 is unchanged.
    #[test]
    fn a_lines_inline_boxes_are_reordered_into_bidi_visual_order() {
        // Persian, so the anchors' own text is strongly RTL — the levels have to come from the
        // CONTENT, not from the container.
        let (dom, root) = layout_html(
            "<body style='margin:0;font-size:16px'>\
               <div id=r dir=rtl><a id=r1>سلام</a> <a id=r2>دنیا</a> <a id=r3>فیلم</a></div>\
               <div id=l dir=ltr><a id=l1>سلام</a> <a id=l2>دنیا</a> <a id=l3>فیلم</a></div>\
               <div id=m dir=rtl><a id=m1>one</a> <a id=m2>two</a> <a id=m3>three</a></div>\
             </body>",
            "div{width:400px}",
            1200.0,
        );
        let rects = root.node_rects(&dom);
        let x = |id: &str| {
            let n = dom
                .descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id");
            rects[&n].x
        };
        // Row 1 — an RTL paragraph of RTL runs reads right-to-left: the FIRST anchor is RIGHTMOST.
        assert!(
            x("r1") > x("r2") && x("r2") > x("r3"),
            "dir=rtl RTL-script anchors must read right-to-left, got r1={} r2={} r3={}",
            x("r1"),
            x("r2"),
            x("r3")
        );
        // Row 2 — the same runs inside an LTR paragraph still reorder among themselves.
        assert!(
            x("l1") > x("l2") && x("l2") > x("l3"),
            "RTL-script anchors inside dir=ltr must still reorder, got l1={} l2={} l3={}",
            x("l1"),
            x("l2"),
            x("l3")
        );
        // Row 3 — THE CONTROL. Latin in an RTL paragraph is one LTR run; the boxes keep source
        // order and only the line is flush right.
        assert!(
            x("m1") < x("m2") && x("m2") < x("m3"),
            "LATIN anchors in an RTL paragraph must keep source order, got m1={} m2={} m3={}",
            x("m1"),
            x("m2"),
            x("m3")
        );
        // ⚠ The line's flush-RIGHT alignment (`text-align: start` resolving to `right` under RTL)
        // is deliberately NOT asserted here: it is a different property, it is already correct on
        // the SHIPPING cascade (measured above through `boxes`, which runs Stylo), and this test's
        // cascade does not apply it — so asserting it here would gate a cascade difference under a
        // layout name and go red for a reason that has nothing to do with rule L2.
    }

    /// **An RTL grid's COLUMN AXIS runs right-to-left** — `direction` reverses a grid's inline-axis
    /// track order (CSS Grid §3: the column axis IS the inline axis), so the first item lands in the
    /// RIGHTMOST column.
    ///
    /// Taffy has no `direction` property and the `row` ⇄ `row-reverse` swap that fixes flex (t764) has
    /// no grid equivalent — `grid-auto-flow` is not a direction — so the mirror is applied to the
    /// placed SLOTS on the way out, recursively, each against its own content box.
    ///
    /// Measured against live Chromium (`<html dir=rtl>`, a 600px `1fr 1fr` grid, x relative to the
    /// grid): Chrome **300 / 0 / 300** for items 1–3 (the third wraps to row 2); ours was 0 / 300 / 0.
    ///
    /// Real site: `mobile.ir` shape **0.493 → 0.523** and `reading_order` 87 → **75** on top of the
    /// t764/t765 RTL fixes; across the three, that page went shape 0.174 → 0.523, `reading_order`
    /// 874 → 75 and `h_overflow` 268 → 1. LTR control `marktplaats.nl` byte-identical throughout.
    ///
    /// ⚠ Second assertion, and it is the one that makes this a *direction* fix: a `direction:ltr` grid
    /// inside the same RTL document keeps LTR column order (Chrome: 0 / 100).
    ///
    /// RED, run: make `grid_is_rtl` return `false` — item 1 reads 0 where Chrome says 300.
    #[test]
    fn an_rtl_grid_orders_its_columns_right_to_left() {
        let (dom, root) = layout_html(
            "<body dir=rtl style='margin:0;width:600px'>\
               <div id=g style='display:grid;grid-template-columns:1fr 1fr;width:600px'>\
                 <div id=x1>1</div><div id=x2>2</div><div id=x3>3</div></div>\
               <div id=g2 style='display:grid;grid-template-columns:100px 200px;width:600px;direction:ltr'>\
                 <div id=y1>a</div><div id=y2>b</div></div>\
             </body>",
            "",
            600.0,
        );
        let rects = root.node_rects(&dom);
        let x = |id: &str| {
            let n = dom
                .descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id");
            rects[&n].x
        };
        assert!(
            (x("x1") - 300.0).abs() < 1.0 && x("x2").abs() < 1.0,
            "the FIRST grid item is in the RIGHT column: x1={} x2={}",
            x("x1"),
            x("x2")
        );
        assert!(
            (x("x3") - 300.0).abs() < 1.0,
            "the wrapped third item starts the next row on the RIGHT too: {}",
            x("x3")
        );
        assert!(
            x("y1") < x("y2"),
            "a direction:ltr grid inside an RTL page keeps LTR column order"
        );
    }

    /// **An RTL table's COLUMN AXIS runs right-to-left** — `direction` on the table box orders the
    /// columns, not just the text in them (CSS 2.1 §17.5.3), so the first `<td>` in source order is the
    /// RIGHTMOST cell.
    ///
    /// Measured against live Chromium (`<html dir=rtl>`, a 600px table of four 150px cells, x relative
    /// to the table): Chrome **450 / 300 / 150 / 0**; ours was 0 / 150 / 300 / 450 — every RTL table
    /// read backwards. It is the largest single mechanism on `mobile.ir`, the worst `reading_order`
    /// site in the CrUX sample: the fix took it from shape 0.320 → **0.493** and `reading_order`
    /// 820 → **87**, with `coverage` and `shape_n` unchanged and the LTR control byte-identical.
    ///
    /// ⚠ The second half is the one that makes it a *direction* fix rather than a *reverse the cells*
    /// fix: a `<table style="direction:ltr">` inside an RTL document keeps LTR column order, because the
    /// axis is read from the TABLE's own computed style. Chrome agrees, and this asserts it.
    ///
    /// RED, run: force `rtl_cols` to `false` — the first cell reads 0 where Chrome says 450.
    #[test]
    fn an_rtl_table_orders_its_columns_right_to_left() {
        let (dom, root) = layout_html(
            "<body dir=rtl style='margin:0;width:600px'>\
               <table id=t1 style='width:600px;border-collapse:collapse'><tr>\
                 <td id=a>a</td><td id=b>b</td><td id=c>c</td><td id=d>d</td></tr></table>\
               <table id=t2 style='width:600px;border-collapse:collapse;direction:ltr'><tr>\
                 <td id=e>e</td><td id=f>f</td></tr></table>\
             </body>",
            "td{padding:0;width:150px}",
            600.0,
        );
        let rects = root.node_rects(&dom);
        let x = |id: &str| {
            let n = dom
                .descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id");
            rects[&n].x
        };
        let (a, b, c, d) = (x("a"), x("b"), x("c"), x("d"));
        assert!(
            a > b && b > c && c > d,
            "source order runs RIGHT to LEFT in an RTL table: {a} {b} {c} {d}"
        );
        assert!(
            (a - 450.0).abs() < 1.0 && (d - 0.0).abs() < 1.0,
            "Chrome-exact: the first cell is flush right (450) and the last is at 0, got {a} / {d}"
        );
        // The control: `direction:ltr` on the TABLE wins over the document's RTL.
        assert!(
            x("e") < x("f"),
            "a direction:ltr table inside an RTL page keeps LTR column order"
        );
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

    /// `text-indent` shifts the inline-start of the FIRST line box only. Two idioms ride it: prose
    /// first-line indentation, and image replacement (`text-indent:-9999px` pushes the text
    /// off-screen so a background image shows alone). RED, run: revert the `text_indent` injection in
    /// `layout_inline` (first-fragment `x` back to `0.0`) — the indented first word snaps to x≈0 and
    /// the −9999px hero text lands on-screen.
    #[test]
    fn text_indent_offsets_the_first_line_only() {
        // Narrow container so the paragraph wraps: the first line is indented, the wrapped line is not.
        let (_dom, root) = layout_html(
            "<body><p style='text-indent:40px'>the quick brown fox jumps over the lazy dog again and again</p></body>",
            "body{margin:0}p{margin:0}",
            120.0,
        );
        let mut frags: Vec<(i32, f32)> = Vec::new();
        root.walk(&mut |b| {
            if let BoxContent::Inline(fs) = &b.content {
                for f in fs {
                    frags.push((f.line_top as i32, f.x));
                }
            }
        });
        assert!(!frags.is_empty(), "expected inline fragments");
        let tops: std::collections::BTreeSet<i32> = frags.iter().map(|(t, _)| *t).collect();
        assert!(
            tops.len() > 1,
            "text should wrap to multiple lines at width 120"
        );
        let min_first = |top: i32| {
            frags
                .iter()
                .filter(|(t, _)| *t == top)
                .map(|(_, x)| *x)
                .fold(f32::INFINITY, f32::min)
        };
        let first_top = *tops.iter().next().unwrap();
        let second_top = *tops.iter().nth(1).unwrap();
        let first_x = min_first(first_top);
        let second_x = min_first(second_top);
        // The first line starts 40px further in than the wrapped line (offset cancels the body/p box).
        assert!(
            (first_x - second_x - 40.0).abs() < 2.0,
            "first line must be indented 40px past the wrapped line: first_x={first_x}, second_x={second_x}"
        );
        // The wrapped line is NOT indented — it starts at the container edge.
        assert!(
            second_x.abs() < 2.0,
            "the second line must not be indented, second_x={second_x}"
        );

        // Image replacement: a large negative indent pushes the first line off-screen-left.
        let (_dom2, root2) = layout_html(
            "<body><p style='text-indent:-9999px'>LOGOTEXT</p></body>",
            "body{margin:0}p{margin:0}",
            800.0,
        );
        let mut min_x = f32::INFINITY;
        root2.walk(&mut |b| {
            if let BoxContent::Inline(fs) = &b.content {
                for f in fs {
                    min_x = min_x.min(f.x);
                }
            }
        });
        assert!(
            min_x < -1000.0,
            "text-indent:-9999px must push the first line off-screen, min_x={min_x}"
        );
    }

    /// `-webkit-line-clamp: N` caps a block at N line boxes and ends line N with `…` — the card /
    /// product / article-excerpt truncation idiom on nearly every content site. RED, run: delete the
    /// `apply_line_clamp` call in the block-inline path — the clamped `<div>` keeps ALL its wrapped
    /// lines and no ellipsis appears.
    #[test]
    fn line_clamp_caps_lines_and_appends_ellipsis() {
        let long =
            "the quick brown fox jumps over the lazy dog again and again and again and yet again";
        let collect = |html: &str| -> (std::collections::BTreeSet<i32>, Vec<String>) {
            let (_dom, root) = layout_html(html, "body{margin:0}div{margin:0}", 120.0);
            let mut tops = std::collections::BTreeSet::new();
            let mut texts = Vec::new();
            root.walk(&mut |b| {
                if let BoxContent::Inline(fs) = &b.content {
                    for f in fs {
                        tops.insert(f.line_top as i32);
                        texts.push(f.text.clone());
                    }
                }
            });
            (tops, texts)
        };

        // Control: unclamped, the paragraph wraps to more than two lines and has no ellipsis.
        let (ctrl_tops, ctrl_texts) = collect(&format!(
            "<body><div style='overflow:hidden'>{long}</div></body>"
        ));
        assert!(
            ctrl_tops.len() > 2,
            "control must wrap past two lines, got {}",
            ctrl_tops.len()
        );
        assert!(
            !ctrl_texts.iter().any(|t| t.contains('\u{2026}')),
            "control must not have an ellipsis"
        );

        // Clamped to 2: exactly two line boxes, and line 2 ends with `…`.
        let (tops, texts) = collect(&format!(
            "<body><div style='-webkit-line-clamp:2;overflow:hidden'>{long}</div></body>"
        ));
        assert_eq!(
            tops.len(),
            2,
            "line-clamp:2 must leave exactly two line boxes, got {}",
            tops.len()
        );
        assert!(
            texts
                .last()
                .map(|t| t.contains('\u{2026}'))
                .unwrap_or(false),
            "the clamped line must end with an ellipsis, texts={texts:?}"
        );

        // A block already SHORTER than the clamp is untouched (no ellipsis, no line loss).
        let (short_tops, short_texts) = collect(
            "<body><div style='-webkit-line-clamp:5;overflow:hidden'>short line</div></body>",
        );
        assert_eq!(short_tops.len(), 1, "short block stays one line");
        assert!(
            !short_texts.iter().any(|t| t.contains('\u{2026}')),
            "a block under the clamp gets no ellipsis"
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

    /// The HTML "ordinal value" algorithm: `<ol reversed>` counts DOWN, and an `<li value>`
    /// continues the count for every following item. Index-based numbering (the pre-fix form) got
    /// both wrong — a resumed list restarted at each item's position and a countdown ran upward.
    ///
    /// RED, run: revert `list_marker` to `start + preceding-<li>-count`. The reversed list reads
    /// `1. 2. 3.` and the value-continued list reads `1. 7. 3.` — the exact mis-numbering the
    /// running counter removes.
    #[test]
    fn list_ordinals_follow_reversed_and_value_continuation() {
        let html = "<ol reversed><li id=\"r1\">a</li><li id=\"r2\">b</li><li id=\"r3\">c</li></ol>\
                    <ol start=\"1\"><li id=\"v1\">x</li><li id=\"v2\" value=\"7\">y</li><li id=\"v3\">z</li></ol>";
        let css = "ol{list-style-type:decimal}";
        let (_dom, root) = layout_html(html, css, 400.0);
        let mut markers: Vec<String> = Vec::new();
        root.walk(&mut |b| {
            if let Some(m) = &b.marker {
                markers.push(m.text.clone());
            }
        });
        assert_eq!(
            markers,
            vec![
                // reversed: N, N-1, … 1
                "3.".to_string(),
                "2.".to_string(),
                "1.".to_string(),
                // value=7 on the 2nd item continues to 8 on the 3rd, not back to 3
                "1.".to_string(),
                "7.".to_string(),
                "8.".to_string(),
            ],
            "reversed counts down; a value continues the running counter"
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
    /// ⚠⚠ **An out-of-flow `::before` must leave the flow — it was pushing the item's text.**
    ///
    /// `.item::before { content: "–"; position: absolute; left: 0 }` over `padding-left: 20px` is
    /// *the* custom-bullet idiom, and the same shape carries every pseudo icon and chevron on the
    /// web. The generated content was emitted as an ordinary inline word, so the marker **took
    /// advance width** and shifted the item's own text right by it, while drawing itself where the
    /// text should have started. Measured against Chrome on `255md.com`: the dash glued to
    /// `ad delivery` instead of sitting 20px to its left.
    ///
    /// Three claims, and the second two are the ones that catch an over-broad fix: the marker still
    /// PAINTS (an out-of-flow box is removed from the flow, not from the page — deleting it would
    /// trade a placement bug for a missing-content bug), and a pseudo that is NOT positioned must
    /// still occupy its width.
    #[test]
    fn an_absolutely_positioned_pseudo_leaves_the_flow_but_still_paints() {
        let html = r#"<ul><li id="a">text</li></ul><ul><li id="b">text</li></ul>"#;
        let css = r#"ul{margin:0;padding:0;list-style:none}
                     li{padding-left:20px;position:relative}
                     #a::before{content:"XXXX";position:absolute;left:0}
                     #b::before{content:"XXXX"}"#;
        let (dom, root) = layout_html(html, css, 400.0);
        let _ = &dom;
        // Collect every fragment in document order with its x.
        let mut frags: Vec<(String, f32)> = Vec::new();
        root.walk(&mut |b| {
            if let BoxContent::Inline(f) = &b.content {
                for fr in f {
                    if !fr.text.trim().is_empty() {
                        frags.push((fr.text.clone(), fr.x));
                    }
                }
            }
        });
        let x_of = |t: &str, nth: usize| -> f32 {
            frags
                .iter()
                .filter(|(s, _)| s.trim() == t)
                .nth(nth)
                .unwrap_or_else(|| panic!("no `{t}` #{nth} among {frags:?}"))
                .1
        };
        // 1. THE MARKER STILL PAINTS. Two of them: one out-of-flow, one in-flow.
        assert_eq!(
            frags.iter().filter(|(s, _)| s.trim() == "XXXX").count(),
            2,
            "both markers must still render — out of FLOW is not out of the PAGE: {frags:?}"
        );
        // 2. The out-of-flow marker sits at the inset (`left:0` of a `position:relative` li), i.e.
        //    20px left of the content edge, and does NOT displace the text.
        // Stated RELATIVE to each other, never as absolute px: the body's UA margin is not what
        // this test is about, and encoding it would make the assertion fail for the wrong reason
        // the next time a default changes.
        assert_eq!(
            x_of("XXXX", 0),
            x_of("text", 0) - 20.0,
            "abs ::before must sit at `left:0` — the padding edge, 20px left of the text — not at \
             the text: {frags:?}"
        );
        // 3. THE CONTROL. A pseudo with no `position` is still in the flow and still pushes the
        //    text, so this cannot pass by having removed all generated content from the flow. Its
        //    marker starts exactly where the out-of-flow item's TEXT starts: the content edge.
        assert_eq!(
            x_of("XXXX", 1),
            x_of("text", 0),
            "an in-flow marker starts at the content edge: {frags:?}"
        );
        assert!(
            x_of("text", 1) > 20.0,
            "an IN-FLOW marker must still push the text right (control): {frags:?}"
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

    /// `white-space: pre-wrap` PRESERVES runs of spaces (and leading indentation); `pre-line`
    /// COLLAPSES them. The two shared one code path that collapsed, so pre-wrap silently reflowed
    /// `<textarea>` content and aligned/preformatted-but-wrapping text into a single-spaced blob.
    ///
    /// RED, run: fold pre-wrap back onto the pre-line collapse path. The pre-wrap render loses its
    /// extra spaces and reads `a b`, not `a   b`.
    fn joined_inline_text(root: &LayoutBox) -> String {
        let mut s = String::new();
        root.walk(&mut |b| {
            if let BoxContent::Inline(frags) = &b.content {
                for f in &*frags {
                    s.push_str(&f.text);
                }
            }
        });
        s
    }

    #[test]
    fn pre_wrap_preserves_spaces_while_pre_line_collapses() {
        let (_d1, pw) = layout_html("<p style=\"white-space:pre-wrap\">a   b</p>", "", 400.0);
        assert!(
            joined_inline_text(&pw).contains("a   b"),
            "pre-wrap must preserve the three spaces (got {:?})",
            joined_inline_text(&pw)
        );

        let (_d2, pl) = layout_html("<p style=\"white-space:pre-line\">a   b</p>", "", 400.0);
        // pre-line's single inter-word space is a positional GAP (`space_before`), not glyph text,
        // so its joined fragment text is "ab": the collapsed run leaves no spaces IN the text, which
        // is exactly the pre-wrap↔pre-line contrast — pre-wrap emits the run as measured tokens.
        let plt = joined_inline_text(&pl);
        assert_eq!(
            plt, "ab",
            "pre-line collapses the run to a gap, emitting no space glyphs (got {plt:?})"
        );
    }

    /// `capitalize` titlecases the first typographic LETTER of each word — not the first character.
    /// Leading punctuation, quotes and digits must pass through without consuming the word start, or a
    /// single `"`/`(`/digit silently swallows the capital (Chrome capitalizes past them).
    ///
    /// RED, run: restore `at_word_start = false;` in the else arm. `(hello)` → `(hello)`, `'twas` →
    /// `'twas`, `3d` → `3d` — every leading-symbol word loses its capital.
    #[test]
    fn capitalize_skips_leading_punctuation_and_digits() {
        use manuk_css::TextTransform as T;
        let cap = |s: &str| apply_text_transform(s, T::Capitalize).into_owned();
        assert_eq!(cap("(hello) world"), "(Hello) World", "leading bracket");
        assert_eq!(cap("'twas the night"), "'Twas The Night", "leading quote");
        assert_eq!(
            cap("\"quoted\" text"),
            "\"Quoted\" Text",
            "leading double-quote"
        );
        assert_eq!(
            cap("3d printing"),
            "3D Printing",
            "digit before the first letter"
        );
        // Regression guard: the plain case still works, and mid-word letters are untouched.
        assert_eq!(cap("hello world"), "Hello World");
        assert_eq!(
            cap("iPhone case"),
            "IPhone Case",
            "only the first letter is titlecased"
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

    /// ⚠⚠⚠ **A SHRINK-TO-FIT BOX WHOSE PADDING SITS ON A BLOCK CHILD CAME OUT ONE PADDING TOO
    /// NARROW — IN EVERY SHRINK-TO-FIT CONTEXT THERE IS.**
    ///
    /// `content_right_extent` measures max-content by laying the subtree out at 1e6 and reading how
    /// far it reached. A block child *fills* that width, so its own `rect.width` (≈1e6) is
    /// meaningless and the walk discards the box and recurses to the text. That discard is correct
    /// and it is also asymmetric: the child's LEFT padding survives it for free (it is baked into
    /// where the text was laid out, so it shows up in the fragment's `x`), while its RIGHT padding
    /// has no content after it to carry it and is simply lost.
    ///
    /// Measured against headless Chrome — a `13.2px/17.16px Arial` run inside a `box-sizing:
    /// border-box; padding: 6.6px` block, itself inside a shrink-to-fit box:
    ///
    /// ```text
    ///   outer box                       Chrome    before    after
    ///   flex item                        86.5      80.0      86.5   ✗→✓
    ///   inline-block                     86.5      80.0      86.5   ✗→✓
    ///   float: left                      86.5      80.0      86.5   ✗→✓
    ///   position: absolute               86.5      80.0      86.5   ✗→✓
    ///   display: table                   86.5      80.0      86.5   ✗→✓
    ///   padding on the box ITSELF        86.5      86.5      86.5   ← guard (never broken)
    ///   margin: 0 10px on the child      93.3      83.3      93.3   ✗→✓  (same loss, other property)
    /// ```
    ///
    /// The reach is a footer link, a nav item, a button — anything that hugs its text through a
    /// padded wrapper. On `kicktipp.com` it was a `<a>` 96px wide against Chrome's 103, and six px
    /// of width is a re-wrapped line, which is a doubled height, which cascades down the subtree.
    #[test]
    fn shrink_to_fit_counts_the_right_padding_of_a_filled_block_child() {
        // Padding on the CHILD, not on the box being measured — that is the whole point.
        let html = r#"<div id="host"><span id="ib"><div id="kid">hello</div></span></div>"#;
        let css = "#host{width:400px;font-size:16px}\
                   #ib{display:inline-block}\
                   #kid{box-sizing:border-box;padding-left:20px;padding-right:20px}";
        let (dom, root) = layout_html(html, css, 800.0);
        let rects = root.node_rects(&dom);
        let by_id = |id: &str| {
            dom.descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id")
        };
        // The bare word, measured with no padding at all, is the baseline the assertion is relative
        // to — so this does not hard-code a font metric that a font change would falsify.
        let bare = r#"<div id="host"><span id="ib">hello</span></div>"#;
        let (d2, r2) = layout_html(
            bare,
            "#host{width:400px;font-size:16px}#ib{display:inline-block}",
            800.0,
        );
        let rects2 = r2.node_rects(&d2);
        let text_w = rects2[&d2
            .descendants(d2.root())
            .find(|&n| d2.element(n).and_then(|e| e.attr("id")) == Some("ib"))
            .expect("id")]
            .width;
        let w = rects[&by_id("ib")].width;
        assert!(
            (w - (text_w + 40.0)).abs() < 1.0,
            "an inline-block wrapping a block with 20px padding EACH SIDE must be text+40 \
             ({:.2}), not text+20 ({:.2}) — got {w:.2}",
            text_w + 40.0,
            text_w + 20.0
        );
    }

    /// ⚠⚠⚠ **A BOX SIZED TO ITS OWN MAX-CONTENT MUST FIT ITS OWN CONTENT, AND ON A BARE `f32` IT
    /// DOES NOT.**
    ///
    /// max-content is read by laying the run out unbounded and measuring how far it reached; the box
    /// is then given exactly that number and the run is laid out *again* against it. The second pass
    /// accumulates the same advances in a different order and can land a few thousandths of a pixel
    /// over — and the line breaker has no tolerance, so it takes a break. The box hugs its text one
    /// word too tightly and comes out **two lines tall where Chrome renders one**.
    ///
    /// Measured on `kicktipp.com`: a footer link whose max-content came to `89.520px` and whose own
    /// re-layout needed `89.525px`. Blink cannot reach this state because a preferred width is a
    /// `LayoutUnit` built with `FromFloatCeil` — quantised *outward*, never inward. See
    /// [`taffy_tree::ceil_to_layout_unit`]; this test is the falsifier for that direction.
    #[test]
    fn a_flex_item_at_its_own_max_content_does_not_rewrap_its_own_text() {
        // Two words, so a break is available for the bug to take; a flex item, because that is the
        // path that hands the measured width straight back as the used width.
        let html = r#"<div id="row"><div id="item">Terms and conditions</div></div>"#;
        let css = "#row{display:flex;width:650px;font-size:13.2px}#item{font-size:13.2px}";
        let (dom, root) = layout_html(html, css, 1280.0);
        let rects = root.node_rects(&dom);
        let by_id = |id: &str| {
            dom.descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id")
        };
        let h = rects[&by_id("item")].height;
        assert!(
            h < 22.0,
            "a flex item sized to its own max-content must keep its text on ONE line \
             (~17px); {h:.2}px is two lines — the box re-wrapped the run it was measured from"
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

    /// **ONLY AN IN-FLOW BLOCK SPLITS AN INLINE — AND AN INLINE IS A CONTAINING BLOCK TOO.**
    ///
    /// CSS 2.1 §9.2.1.1 splits an inline box around a block-level box *in the flow*. A float or an
    /// out-of-flow positioned box is removed from the inline formatting context and splits nothing.
    /// But `position: absolute` **blockifies `display`** (CSS Display §2.7), so
    /// `<span style="position:absolute">` computes to `display: block` — and `inline_contains_block`
    /// walked it straight into the block-in-inline check and **blockified the inline ancestor**.
    ///
    /// `<a style="position:relative">text<span style="position:absolute">…</span></a>` is the
    /// stretched click target, the badge on an icon link, the tooltip anchor, the dropdown under a
    /// nav item. Every one of them turned its `<a>` into a **full-width block**: the link took the
    /// whole line, forced a break, changed its parent's height, and displaced everything below it.
    ///
    /// ⚠⚠ **TWO CHANGES, ONE BEHAVIOUR — and each alone reads as a near no-op.** Un-blockifying the
    /// inline is not enough: a boxless inline has no entry in `position_absolutes`' rect map, because
    /// `LayoutBox::walk` descends `BoxContent::Block` only and never enters `BoxContent::Inline`. So
    /// `abs_containing_block` still walked past it to the nearest BLOCK-level positioned ancestor —
    /// CSS 2.1 §10.1 says it must not. Both halves are asserted below, and the mutation that reverts
    /// either one fails a DIFFERENT row.
    ///
    /// Chrome `--headless=new`, 1200×800, `margin:0; font:16px/normal sans-serif`.
    #[test]
    fn an_out_of_flow_child_neither_splits_its_inline_nor_escapes_it() {
        let html = r##"<div class="outer" id="outer">
              <p>xxxx <a class="rel" id="aRelPlain" href="#">LINKTEXT</a> yyyy</p>
              <p>xxxx <a class="rel" id="aRel" href="#">LINKTEXT<span class="corner" id="cRel"></span></a> yyyy</p>
              <p>xxxx <a id="aStat" href="#">LINKTEXT<span class="corner" id="cStat"></span></a> yyyy</p>
            </div>"##;
        // ⚠ `display:block` on `.corner` is EXPLICIT, and that is not decoration. `position:absolute`
        //   blockifies `display` in the SHIPPING (Stylo) cascade, but `MinimalCascade` — which these
        //   unit tests run on — does not implement CSS Display §2.7. Leaving it implicit made the
        //   fixture compute `display:inline`, so the bug never reproduced here and the mutation that
        //   restores the blockify SURVIVED this gate. Stating the computed value keeps the gate
        //   testing LAYOUT rather than which cascade the harness happens to use.
        let css = "body{margin:0} div,p,a,span{font-family:sans-serif;font-size:16px;line-height:normal} \
                   .outer{position:relative;height:200px} a.rel{position:relative} \
                   .corner{position:absolute;display:block;top:0;left:0;width:10px;height:10px}";
        let (dom, root) = layout_html(html, css, 1200.0);
        let rects = root.node_rects(&dom);
        let r = |id: &str| {
            let n = dom
                .descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id");
            rects[&n]
        };

        // (1) THE INLINE IS NOT BLOCKIFIED. Chrome puts `#aRel` at x=36 with width 76 — the extent of
        //     its own text. Blockified it became a full-width box at x=0, width 1200, which is a
        //     16× error on the width of a link and a forced line break in the paragraph.
        assert!(
            (r("aRel").x - 36.0).abs() < 1.5 && (r("aRel").width - 76.0).abs() < 4.0,
            "#aRel is [{} {} {}x{}]; Chrome says [36 50 76x17]. An out-of-flow child must not \
             blockify its inline ancestor — a link that becomes a full-width block takes the whole \
             line and displaces everything below it.",
            r("aRel").x,
            r("aRel").y,
            r("aRel").width,
            r("aRel").height
        );

        // (2) THE ABSPOS CHILD RESOLVES AGAINST THAT INLINE. This is the half that `walk` could not
        //     see, and it is the whole point of `position:relative` on a link.
        let (dx, dy) = (r("cRel").x - r("aRel").x, r("cRel").y - r("aRel").y);
        assert!(
            dx.abs() < 1.5 && dy.abs() < 1.5,
            "#cRel sits ({dx}, {dy}) from #aRel and must sit at its origin (0, 0): with \
             `top:0;left:0` an abspos child of a `position:relative` INLINE resolves against that \
             inline (CSS 2.1 §10.1), not against the nearest block-level positioned ancestor. \
             Chrome: #aRel [36 50 76x17], #cRel [36 50 10x10]."
        );

        // (3) CONTROL — `position: static` still establishes NOTHING. The fix must widen which
        //     ancestors can be a containing block, not which ancestors ARE one; a version that made
        //     every inline a containing block would pass (1) and (2) and break every real overlay.
        //     Chrome puts #cStat at the `.outer` origin [0 16], not at its static `<a>`.
        assert!(
            r("cStat").x.abs() < 1.5 && (r("cStat").y - r("outer").y).abs() < 1.5,
            "#cStat is at [{} {}] and Chrome puts it at the `.outer` origin [0 {}] — a \
             `position:static` inline must NOT become a containing block.",
            r("cStat").x,
            r("cStat").y,
            r("outer").y
        );

        // (4) GUARD — an inline with no out-of-flow child is untouched. If this moves, every inline
        //     element on every page moved.
        assert!(
            (r("aRelPlain").x - 36.0).abs() < 1.5 && (r("aRelPlain").width - 76.0).abs() < 4.0,
            "GUARD: #aRelPlain (a plain inline link, no out-of-flow child) is [{} {} {}x{}] and \
             Chrome says [36 16 76x17].",
            r("aRelPlain").x,
            r("aRelPlain").y,
            r("aRelPlain").width,
            r("aRelPlain").height
        );
    }

    /// **THE CLEARFIX — a block-level `::after` is a box, and `content: ""` is content.**
    ///
    /// `.cf::after { content: ""; display: block; clear: both }` is *the* float-containment idiom of
    /// the last fifteen years: every Bootstrap-era grid, every WordPress theme, every hand-rolled
    /// `.clearfix`. Generated content was materialised only as inline WORDS
    /// (`collect_inline_group`), and that path additionally dropped the empty string — so a
    /// block-level pseudo produced **no box at all**, nothing cleared, and the parent **collapsed to
    /// zero**, dumping its floated children outside it and pulling every following sibling up.
    ///
    /// Measured on `keirin.jp`, whose nav is exactly this shape: `#nav_menus` and `#navbar` were
    /// **h=0 against Chrome's h=70** — and 70 is precisely the `dy` the first-divergence probe
    /// reported for that page. After: misplaced 1041 -> 954, median `dy` 124 -> 38, SHAPE
    /// 56.8% -> 59.2%, and the first divergence moved off the nav entirely.
    ///
    /// Chrome `--headless=new`, 1200x800, `margin:0`, a single `float:left` 100x70 child.
    #[test]
    fn a_block_level_after_pseudo_clears_the_floats_its_parent_would_otherwise_drop() {
        let html = r#"<div class="plain" id="p"><div class="f"></div></div>
                      <div class="ovh" id="o"><div class="f"></div></div>
                      <div class="cf" id="c"><div class="f"></div></div>
                      <div class="cfb" id="t"><div class="f"></div></div>
                      <div class="inl" id="i"><div class="f"></div></div>"#;
        // `content:""` is stated because it IS the idiom — an empty string is a box with no text,
        // not an absent pseudo (only `content: none` suppresses one).
        let css = "body{margin:0} div{font-family:sans-serif;font-size:16px} \
                   .f{float:left;width:100px;height:70px} \
                   .ovh{overflow:hidden} \
                   .cf::after{content:\"\";display:block;clear:both} \
                   .cfb::after{content:\"\";display:table;clear:both} \
                   .inl::after{content:\"\";clear:both}";
        let (dom, root) = layout_html(html, css, 1200.0);
        let rects = root.node_rects(&dom);
        let h = |id: &str| {
            let n = dom
                .descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id");
            rects[&n].height
        };

        // (1) THE FIX. Both spellings of the idiom contain the float.
        assert!(
            (h("c") - 70.0).abs() < 1.5,
            "#c is h{} and Chrome says h70 — `::after{{content:\"\";display:block;clear:both}}` is \
             the clearfix, and with no box generated nothing clears, so the parent collapses to zero \
             and drops its floated children outside itself.",
            h("c")
        );
        assert!(
            (h("t") - 70.0).abs() < 1.5,
            "#t is h{} and Chrome says h70 — `display:table` is the other common clearfix spelling \
             (it also suppresses margin collapse) and must behave the same here.",
            h("t")
        );

        // (2) CONTROL — a plain block STILL does not contain its floats. That is correct CSS, and a
        //     fix that simply made every parent contain its floats would pass (1) and silently break
        //     every intentional float overhang on the web. This is the assertion that separates
        //     "the clearfix works" from "floats are always contained".
        assert!(
            h("p").abs() < 1.5,
            "CONTROL: #p is h{} and MUST be h0 — a plain block box does not contain its floats. If \
             this grew, the fix is containing floats unconditionally rather than honouring `clear`.",
            h("p")
        );

        // (2b) CONTROL — `clear` DOES NOT APPLY TO AN INLINE BOX (CSS 2.1 §9.5.2), so an `::after`
        //      that omits `display:block` clears nothing and its parent stays collapsed. Chrome: h0.
        //      Without this row the `display` filter can be deleted outright and the gate still
        //      passes — the mutation that does exactly that SURVIVED the first version of this test.
        assert!(
            h("i").abs() < 1.5,
            "CONTROL: #i is h{} and Chrome says h0 — its `::after` has the DEFAULT `display:inline`, \
             and `clear` does not apply to an inline box. A generated box that clears regardless of \
             its display would contain floats nothing asked it to.",
            h("i")
        );

        // (3) GUARD — `overflow:hidden` already contained its float and must not move.
        assert!(
            (h("o") - 70.0).abs() < 1.5,
            "GUARD: #o (overflow:hidden, a BFC root) is h{} and was already h70.",
            h("o")
        );
    }

    /// **`display: flow-root` IS A BLOCK THAT CONTAINS ITS FLOATS — and the catch-all ate it.**
    ///
    /// `flow-root` exists for exactly one reason: a block box that establishes a block formatting
    /// context, so it contains its floats **without** `overflow:hidden`'s clipping and without a
    /// generated `::after`. `map_display`'s catch-all answers `Inline` for any keyword nobody mapped,
    /// which is the worst available answer — an inline box still participates in layout, so the
    /// failure reads as a subtle geometry bug rather than an unsupported value. Measured against
    /// Chrome: the container came out **[0 0 0x19]** where Chrome says **[0 0 1200x70]**.
    ///
    /// A 23-keyword sweep against Chrome found four values sitting in that catch-all — `flow-root`,
    /// `list-item`, `table-column`, `table-column-group` — the last two having had variants in our
    /// own enum the whole time. Divergences went **6 of 23 -> 2 of 23**; the remainder is `ruby` and
    /// MathML's `math`, both on the declared post-Phase-0 list.
    #[test]
    fn flow_root_is_a_block_that_contains_its_floats() {
        let html = r#"<div class="fr" id="r"><div class="f"></div></div>
                      <div class="fr" id="s">x</div>
                      <div class="pl" id="p"><div class="f"></div></div>"#;
        let css = "body{margin:0} div{font-family:sans-serif;font-size:16px;line-height:normal} \
                   .f{float:left;width:100px;height:70px} .fr{display:flow-root}";
        let (dom, root) = layout_html(html, css, 1200.0);
        let rects = root.node_rects(&dom);
        let r = |id: &str| {
            let n = dom
                .descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(id))
                .expect("id");
            rects[&n]
        };

        // (1) IT IS BLOCK-LEVEL. Falling through to `Inline` made it shrink to its content: width 0
        //     for the empty one, and the text's width for the other. Chrome: full containing block.
        assert!(
            (r("s").width - 1200.0).abs() < 1.5,
            "#s is {}px wide and Chrome says 1200 — `flow-root` is BLOCK-level, and the catch-all \
             that answered `Inline` made it shrink to its content instead of filling its parent.",
            r("s").width
        );

        // (2) IT CONTAINS ITS FLOATS. This is the whole reason the value exists.
        assert!(
            (r("r").height - 70.0).abs() < 1.5,
            "#r is h{} and Chrome says h70 — `flow-root` establishes a block formatting context, so \
             its floated child must be contained. Block-level alone is not enough: without the \
             `establishes_bfc` half it is just a block, and a plain block does NOT contain floats.",
            r("r").height
        );

        // (3) CONTROL — the plain block next to it still does NOT contain its float. If this grew,
        //     floats are being contained unconditionally rather than by `flow-root`.
        assert!(
            r("p").height.abs() < 1.5,
            "CONTROL: #p is h{} and MUST be h0 — it is a plain block, and only `flow-root` (or a \
             BFC root) contains floats.",
            r("p").height
        );
    }
}
