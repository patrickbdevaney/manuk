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
    BoxSizing, Clear, ComputedStyle, Dim, Display, Float, Position, Rgba, StyleMap, TextAlign,
    VerticalAlign, WhiteSpace,
};
use manuk_dom::{Dom, NodeData, NodeId};
use manuk_text::{FontContext, FontFamily, FontKey};

pub mod flex;
mod taffy_tree;

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
}

impl TextFragment {
    /// This run's box: `line_height` tall, anchored at the line top.
    pub fn rect(&self) -> Rect {
        Rect {
            x: self.x,
            y: self.line_top,
            width: self.width,
            height: self.style.line_height,
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
    /// The DOM node this box came from, if any (anonymous boxes are `None`).
    pub node: Option<NodeId>,
    pub content: BoxContent,
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
    pub fn node_rects(&self, dom: &Dom) -> std::collections::HashMap<NodeId, Rect> {
        fn add(map: &mut std::collections::HashMap<NodeId, Rect>, node: NodeId, rect: Rect) {
            map.entry(node)
                .and_modify(|r| *r = r.union(&rect))
                .or_insert(rect);
        }
        let mut map: std::collections::HashMap<NodeId, Rect> = std::collections::HashMap::new();
        self.walk(&mut |b| {
            if let Some(node) = b.node {
                add(&mut map, node, b.rect);
            }
            if let BoxContent::Inline(frags) = &b.content {
                for f in frags {
                    let Some(owner) = f.node else { continue };
                    let rect = f.rect();
                    add(&mut map, owner, rect);
                    let mut cur = dom.parent(owner);
                    while let Some(p) = cur {
                        if dom.is_element(p) {
                            add(&mut map, p, rect);
                        }
                        cur = dom.parent(p);
                    }
                }
            }
        });
        map
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

    /// Shift this box and its whole subtree by `(dx, dy)` (absolute coords). Used to
    /// re-origin a float's content once its final position is known.
    fn translate(&mut self, dx: f32, dy: f32) {
        self.rect.x += dx;
        self.rect.y += dy;
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
        let maxx = corners.iter().map(|p| p.0).fold(f32::NEG_INFINITY, f32::max);
        let miny = corners.iter().map(|p| p.1).fold(f32::INFINITY, f32::min);
        let maxy = corners.iter().map(|p| p.1).fold(f32::NEG_INFINITY, f32::max);
        self.rect = Rect { x: minx, y: miny, width: maxx - minx, height: maxy - miny };
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
}

/// Lay out a whole document into a fragment tree, given a viewport width in px.
///
/// The root box is `<body>` (falling back to `<html>` or the first element), laid
/// out in an initial containing block of `viewport_width`.
pub fn layout_document(
    dom: &Dom,
    styles: &StyleMap,
    fonts: &FontContext,
    viewport_width: f32,
) -> LayoutBox {
    let ctx = Ctx { dom, styles, fonts, measure_cache: RefCell::new(HashMap::new()) };
    let root_el = dom
        .find_first("body")
        .or_else(|| dom.find_first("html"))
        .or_else(|| dom.children(dom.root()).find(|&n| dom.is_element(n)));

    match root_el {
        Some(el) => {
            // The initial containing block is itself a BFC root; `layout_block` gives
            // the root element its own context, so this outer one is just a seed.
            let mut floats = FloatContext::new(0.0, viewport_width);
            let mut root = ctx
                .layout_block(el, viewport_width, None, 0.0, 0.0, 0.0, &mut floats)
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
            node: None,
            content: BoxContent::Block(vec![]),
        },
    }
}

/// Is `node` a block-level box in its parent's formatting context?
/// Compose a `transform` function list into an **absolute** affine matrix applied around
/// `origin` (the transform-origin, default the box center). `w`/`h` resolve `translate` `%`.
fn resolve_transform(fns: &[manuk_css::TransformFn], w: f32, h: f32, origin: (f32, f32)) -> [f32; 6] {
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
        })
    }
}

/// The synthetic text a form control renders (its value / label), or `None` for controls
/// that render no text (`<button>` uses its real children; checkbox/radio are boxes). A
/// text input returns `Some("")` when empty so it still lays out with a line's height.
fn form_control_text(dom: &Dom, node: NodeId) -> Option<String> {
    let el = dom.element(node)?;
    match dom.tag_name(node)? {
        "input" => match el.attr("type").unwrap_or("text").to_ascii_lowercase().as_str() {
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
            selected.or(first).map(|opt| dom.text_content(opt).trim().to_string())
        }
        _ => None,
    }
}

fn is_block_level(dom: &Dom, styles: &StyleMap, node: NodeId) -> bool {
    if let NodeData::Element(_) = dom.data(node) {
        matches!(
            styles.get(&node).map(|s| s.display),
            Some(Display::Block | Display::Flex | Display::Grid | Display::Table)
        )
    } else {
        false
    }
}

fn is_rendered(dom: &Dom, styles: &StyleMap, node: NodeId) -> bool {
    match dom.data(node) {
        NodeData::Element(_) => {
            !matches!(styles.get(&node).map(|s| s.display), Some(Display::None))
        }
        NodeData::Text(_) => true,
        _ => false,
    }
}

fn text_style(cs: &ComputedStyle, fonts: &FontContext) -> TextStyle {
    TextStyle {
        font_key: FontKey {
            // Resolve the CSS font-family list to a concrete face (installed or
            // `@font-face`-registered), falling back through generics.
            family: fonts.resolve_family(&cs.font_family),
            bold: cs.font_weight >= 600,
            italic: cs.italic,
        },
        font_size: cs.font_size,
        color: cs.color,
        line_height: cs.line_height,
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

/// Is this box positioned out of normal flow (absolute/fixed)? Such boxes are
/// collected and laid out in a later pass (D1 sub-feature 2).
fn is_out_of_flow_positioned(s: &ComputedStyle) -> bool {
    matches!(s.position, Position::Absolute | Position::Fixed)
}

/// Does this element establish a new block formatting context (CSS2 §9.4.1)? Such a
/// box does not share its parent's float context — its own floats stay inside and it
/// does not overlap outer floats. (`overflow` is not modeled yet.)
fn establishes_bfc(s: &ComputedStyle) -> bool {
    is_float(s)
        || is_out_of_flow_positioned(s)
        || matches!(
            s.display,
            Display::Flex | Display::Grid | Display::InlineBlock
        )
}

/// The max right extent of already-laid-out content (used for shrink-to-fit).
fn content_right_extent(content: &BoxContent, fonts: &FontContext) -> f32 {
    let mut max_r = 0.0f32;
    fn visit(b: &LayoutBox, fonts: &FontContext, max_r: &mut f32) {
        *max_r = max_r.max(b.rect.x + b.rect.width);
        match &b.content {
            BoxContent::Block(kids) => {
                for k in kids {
                    visit(k, fonts, max_r);
                }
            }
            BoxContent::Inline(frags) => {
                for f in frags {
                    let w = fonts.measure(&f.text, f.style.font_key, f.style.font_size);
                    *max_r = max_r.max(f.x + w);
                }
            }
        }
    }
    match content {
        BoxContent::Block(kids) => {
            for k in kids {
                visit(k, fonts, &mut max_r);
            }
        }
        BoxContent::Inline(frags) => {
            for f in frags {
                let w = fonts.measure(&f.text, f.style.font_key, f.style.font_size);
                max_r = max_r.max(f.x + w);
            }
        }
    }
    max_r
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

impl Ctx<'_> {
    /// Lay out a block box in a containing block of `cw` px. `y` is the border-bottom
    /// edge of the preceding in-flow sibling (or the container's content-top for the
    /// first child); `prev_margin` is that sibling's trailing collapsible margin (0
    /// if none). The block's top margin collapses with `prev_margin` to decide its
    /// border-box top. Returns the positioned box and its own top/bottom margins.
    ///
    /// Simplification (documented, CLAUDE.md § verification): parent↔first/last-child
    /// margin collapsing is not yet modeled — only adjacent-sibling collapsing is.
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
        let s = self.styles[&node].clone();

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

        // Resolve width. `auto` fills the available inline space — except an inline-block
        // (an atomic inline box) shrinks to fit its content, so a `<button>` hugs its label.
        let extra = ml + mr + pl + pr + bl + br;
        let mut width = match s.width {
            Dim::Auto if s.display == Display::InlineBlock => {
                self.shrink_to_fit(node, (cw - extra).max(0.0))
            }
            Dim::Auto => (cw - extra).max(0.0),
            other => other.resolve(cw, (cw - extra).max(0.0)),
        };
        // `box-sizing:border-box` — the specified width is the border box, so the content
        // width is that minus padding + border. (`auto` already resolves to content width.)
        let bs_extra_w = if s.box_sizing == BoxSizing::BorderBox { pl + pr + bl + br } else { 0.0 };
        if s.box_sizing == BoxSizing::BorderBox && s.width != Dim::Auto {
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
        if max_w.is_finite() {
            width = width.min(max_w);
        }
        width = width.max(min_w);

        // Horizontal auto-margin centering when width is definite. Only the left
        // margin shifts the box; the right margin absorbs the remainder implicitly.
        if s.width != Dim::Auto {
            let leftover = cw - (width + pl + pr + bl + br);
            match (s.margin.left.is_auto(), s.margin.right.is_auto()) {
                (true, true) => ml = (leftover / 2.0).max(0.0),
                (true, false) => ml = (leftover - mr).max(0.0),
                _ => {}
            }
        }
        let _ = mr; // right margin does not affect downstream positioning here

        let border_x = x + ml;
        // Collapse this block's top margin with the preceding sibling's trailing
        // margin to place the border-box top.
        let border_y = y + collapse_margins(prev_margin, mt);
        let content_x = border_x + bl + pl;
        let content_y = border_y + bt + pt;

        // This block's own **definite** content height, if any — the reference a
        // percentage-height *child* resolves against (CSS2 §10.5). Computed before laying
        // out children so their `height:%` works; `None` (auto height) means a percent-height
        // child falls back to its content height.
        let bs_extra_h = if s.box_sizing == BoxSizing::BorderBox { pt + pb + bt + bb } else { 0.0 };
        let own_definite_h: Option<f32> = match s.height {
            Dim::Px(p) => Some((p - bs_extra_h).max(0.0)),
            Dim::Percent(pct) => pch.map(|h| (h * pct / 100.0 - bs_extra_h).max(0.0)),
            Dim::Calc { .. } => pch.map(|h| (s.height.resolve(h, 0.0) - bs_extra_h).max(0.0)),
            Dim::Auto => None,
        };

        // A BFC root gets a fresh float context spanning its own content box; a plain
        // block shares its parent's so floats affect content across nested blocks.
        let mut own_bfc;
        let (content, content_height) = if establishes_bfc(&s) {
            own_bfc = FloatContext::new(content_x, content_x + width);
            let (c, h) = self.layout_children(node, content_x, content_y, width, own_definite_h, &mut own_bfc);
            // A BFC root grows to contain its floats (CSS2 §10.6.7 auto-height case).
            let float_h = (own_bfc.lowest_bottom() - content_y).max(0.0);
            (c, h.max(float_h))
        } else {
            self.layout_children(node, content_x, content_y, width, own_definite_h, floats)
        };
        let mut content_height = own_definite_h.unwrap_or(content_height);
        // min-height / max-height clamp (content-box).
        let min_h = (s.min_height.resolve(pch.unwrap_or(0.0), 0.0) - bs_extra_h).max(0.0);
        let max_h = match s.max_height {
            Dim::Auto => f32::INFINITY,
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

        let mut boxx = LayoutBox {
            rect,
            background: s.background_color,
            border: border_of(&s),
            node: Some(node),
            content,
        };

        // `position: relative` offsets the box (and its subtree) visually without
        // affecting the flow. `left`/`top` win over `right`/`bottom`; percentages
        // resolve against the containing block (width for x; height unknown here, so
        // percentage y resolves against 0 — documented).
        if s.position == Position::Relative {
            let dx = if !s.inset.left.is_auto() {
                s.inset.left.resolve(cw, 0.0)
            } else if !s.inset.right.is_auto() {
                -s.inset.right.resolve(cw, 0.0)
            } else {
                0.0
            };
            let dy = if !s.inset.top.is_auto() {
                s.inset.top.resolve(0.0, 0.0)
            } else if !s.inset.bottom.is_auto() {
                -s.inset.bottom.resolve(0.0, 0.0)
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
            margin_top: mt,
            margin_bottom: mb,
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
        let display = self.styles[&node].display;

        // Form controls render their *value*/label as synthetic text (an `<input>` has no
        // child nodes; a `<button>` uses its real children so it is not handled here).
        if let Some(text) = form_control_text(self.dom, node) {
            let style = text_style(&self.styles[&node], self.fonts);
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
            }];
            let (frags, _atomics, h) = self.layout_inline(items, cx, cy, cw, TextAlign::Left, floats);
            return (BoxContent::Inline(frags), h);
        }

        // N4: the FLAT tree — a shadow host lays out its shadow content, and a `<slot>`
        // lays out the light-DOM nodes assigned to it.
        let kids: Vec<NodeId> = self
            .dom
            .flat_children(node)
            .into_iter()
            .filter(|&k| is_rendered(self.dom, self.styles, k))
            .collect();

        // Flex/grid containers route through taffy.
        if display == Display::Flex {
            return self.layout_flex(node, cx, cy, cw, &kids);
        }
        if display == Display::Grid {
            return self.layout_grid(node, cx, cy, cw, &kids);
        }

        // Floated / out-of-flow children never count toward the "has block" decision.
        let flow_kids: Vec<NodeId> = kids
            .iter()
            .copied()
            .filter(|&k| {
                let s = &self.styles[&k];
                !is_float(s) && !is_out_of_flow_positioned(s)
            })
            .collect();
        let has_block = flow_kids
            .iter()
            .any(|&k| is_block_level(self.dom, self.styles, k));

        if !has_block && !kids.iter().any(|&k| is_float(&self.styles[&k])) {
            // Pure inline formatting context (no floats to flow around).
            let items = self.collect_inline_group(&flow_kids, cw);
            let align = self.styles[&node].text_align;
            let (frags, atomics, h) = self.layout_inline(items, cx, cy, cw, align, floats);
            if atomics.is_empty() {
                return (BoxContent::Inline(frags), h);
            }
            // Inline-blocks present: the anonymous line box (text) and the atomic boxes
            // become siblings so both reach the fragment tree.
            let mut boxes = Vec::new();
            if !frags.is_empty() {
                boxes.push(LayoutBox {
                    rect: Rect { x: cx, y: cy, width: cw, height: h },
                    background: None,
                    border: None,
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

        for &k in &kids {
            let ks = &self.styles[&k];
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
                // Absolutely/fixed positioned: skipped here, handled in D1 sub-feature
                // 2. Leaving them out keeps normal flow correct in the meantime.
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
                let r = self.layout_block(k, cw, pch, cx, cur_y, prev_margin, floats);
                // Stack against the normal-flow bottom (relative shifts are visual).
                cur_y = r.flow_bottom;
                prev_margin = r.margin_bottom;
                boxes.push(r.boxx);
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
        let s = self.styles[&node].clone();
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
            Dim::Auto => self.shrink_to_fit(node, avail),
            other => other.resolve(cw, avail).max(0.0),
        };

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

    /// Shrink-to-fit width (CSS2 §10.3.5, approximated as `min(max-content, avail)`):
    /// lay the content out unconstrained to get its preferred width, then clamp.
    fn shrink_to_fit(&self, node: NodeId, avail: f32) -> f32 {
        let mut fc = FloatContext::new(0.0, 1.0e6);
        let (content, _h) = self.layout_children(node, 0.0, 0.0, 1.0e6, None, &mut fc);
        let pref = content_right_extent(&content, self.fonts);
        pref.min(avail).max(0.0)
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
        let (_content, height) = self.layout_children(node, 0.0, 0.0, width.max(0.0), None, &mut fc);
        let result = (width, height);
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
        let s = self.styles[&node].clone();
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
        let spacing = if s.border_collapse { 0.0 } else { s.border_spacing };
        let rows = self.collect_table_rows(node);

        // Placement grid: each cell claims the next free slot in its row, spanning
        // colspan columns × rowspan rows and marking those slots occupied (so cells below a
        // rowspan and to the right of a colspan shift over). CSS2 §17.5.
        let mut placed: Vec<PlacedCell> = Vec::new();
        let mut occ: Vec<Vec<bool>> = Vec::new();
        let mut ncols = 0usize;
        for (r, row) in rows.iter().enumerate() {
            let mut col = 0usize;
            for &cell in row {
                while occ.get(r).and_then(|o| o.get(col)).copied().unwrap_or(false) {
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
                placed.push(PlacedCell { cell, row: r, col, colspan: cs, rowspan: rs });
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

        let widths = if ncols == 0 {
            Vec::new()
        } else if s.table_layout == manuk_css::TableLayout::Fixed {
            self.fixed_col_widths(&rows, ncols, avail_cols)
        } else {
            self.auto_col_widths(&rows, ncols, avail_cols, table_specified.is_some())
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
            row_boxes.push(LayoutBox {
                rect: Rect { x: content_x, y: row_y[r], width: content_w, height: row_h[r] },
                background: None,
                border: None,
                node: None,
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
            node: Some(node),
            content: BoxContent::Block(row_boxes),
        };
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
    fn collect_table_rows(&self, table: NodeId) -> Vec<Vec<NodeId>> {
        let mut rows = Vec::new();
        for child in self.dom.children(table) {
            if !is_rendered(self.dom, self.styles, child) || !self.dom.is_element(child) {
                continue;
            }
            match self.styles[&child].display {
                Display::TableRow => rows.push(self.collect_cells(child)),
                Display::TableRowGroup => {
                    for gr in self.dom.children(child) {
                        if is_rendered(self.dom, self.styles, gr)
                            && self.dom.is_element(gr)
                            && self.styles[&gr].display == Display::TableRow
                        {
                            rows.push(self.collect_cells(gr));
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
                    && self.styles[&c].display == Display::TableCell
            })
            .collect()
    }

    /// A cell's intrinsic `(min-content, max-content)` border-box widths.
    fn cell_intrinsic(&self, cell: NodeId) -> (f32, f32) {
        let s = &self.styles[&cell];
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
        let max = content_right_extent(&cmax, self.fonts);
        let mut fc_min = FloatContext::new(0.0, 0.0);
        let (cmin, _) = self.layout_children(cell, 0.0, 0.0, 0.0, None, &mut fc_min);
        let min = content_right_extent(&cmin, self.fonts);
        (min + frame, max + frame)
    }

    /// Auto table layout (CSS2 §17.5.2.2): distribute `avail` across columns using
    /// per-column min/max content widths.
    fn auto_col_widths(
        &self,
        rows: &[Vec<NodeId>],
        ncols: usize,
        avail: f32,
        table_has_width: bool,
    ) -> Vec<f32> {
        let mut col_min = vec![0.0f32; ncols];
        let mut col_max = vec![0.0f32; ncols];
        for row in rows {
            for (c, &cell) in row.iter().enumerate() {
                if c >= ncols {
                    break;
                }
                let (mn, mx) = self.cell_intrinsic(cell);
                col_min[c] = col_min[c].max(mn);
                col_max[c] = col_max[c].max(mx);
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
                set[c] = match self.styles[&cell].width {
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
        let s = self.styles[&cell].clone();
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
            let s = &self.styles[&node];
            // A box with no inset on any edge is at its *static position* (its
            // would-be in-flow spot). Computing that needs in-flow tracking we don't
            // have yet, so such boxes are left unplaced (documented) rather than
            // dropped at the CB origin, which would render them in the wrong place.
            if s.inset.left.is_auto()
                && s.inset.right.is_auto()
                && s.inset.top.is_auto()
                && s.inset.bottom.is_auto()
            {
                continue;
            }
            let cb = if s.position == Position::Fixed {
                viewport
            } else {
                self.abs_containing_block(node, &rects, viewport)
            };
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
                        node: None,
                        content: BoxContent::Inline(std::mem::take(frags)),
                    });
                }
                kids.extend(new_boxes);
                root.content = BoxContent::Block(kids);
            }
        }
    }

    /// Collect rendered `absolute`/`fixed` element nodes in `node`'s subtree, DOM
    /// pre-order.
    fn collect_positioned(&self, node: NodeId, out: &mut Vec<NodeId>) {
        for k in self.dom.children(node) {
            if !is_rendered(self.dom, self.styles, k) {
                continue;
            }
            if self.dom.is_element(k) && is_out_of_flow_positioned(&self.styles[&k]) {
                out.push(k);
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
                let s = &self.styles[&anc];
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
        let s = self.styles[&node].clone();
        let cw = cb.width;
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

        let left = (!s.inset.left.is_auto()).then(|| s.inset.left.resolve(cw, 0.0));
        let right = (!s.inset.right.is_auto()).then(|| s.inset.right.resolve(cw, 0.0));
        let top = (!s.inset.top.is_auto()).then(|| s.inset.top.resolve(cb.height, 0.0));
        let bottom = (!s.inset.bottom.is_auto()).then(|| s.inset.bottom.resolve(cb.height, 0.0));

        let frame = ml + mr + pl + pr + bl + br;
        // Width: definite wins; else if both left+right are set the box stretches to
        // fill between them; else shrink-to-fit.
        let content_w = match s.width {
            Dim::Auto => match (left, right) {
                (Some(l), Some(r)) => (cw - l - r - frame).max(0.0),
                _ => self.shrink_to_fit(node, (cw - frame).max(0.0)),
            },
            other => other.resolve(cw, (cw - frame).max(0.0)).max(0.0),
        };

        // Lay out content at a provisional origin, then re-origin once placed.
        let mut inner = FloatContext::new(0.0, content_w);
        let (content, ch) = self.layout_children(node, 0.0, 0.0, content_w, None, &mut inner);
        let frame_v = mt + mb + pt + pb + bt + bb;
        // Height: definite wins; else if both top+bottom are set the box stretches to fill
        // between them; else it is content height (CSS2 §10.6.4).
        let content_height = match s.height {
            Dim::Auto => match (top, bottom) {
                (Some(t), Some(b)) => (cb.height - t - b - frame_v).max(0.0),
                _ => ch.max(inner.lowest_bottom().max(0.0)),
            },
            other => other.resolve(cb.height, ch),
        };

        let border_box_w = bl + pl + content_w + pr + br;
        let border_box_h = bt + pt + content_height + pb + bb;

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
        let items = self.collect_inline_group(run, cw);
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
            node: None,
            content: BoxContent::Inline(frags),
        });
        // Inline-block atomic boxes are already absolutely positioned; add them as siblings.
        boxes.extend(atomics);
        (start + h, 0.0)
    }

    /// Lay out flex children as a row using taffy for main-axis sizing/positioning.
    /// Each child is then laid out as a block within its taffy-assigned slot.
    fn layout_flex(&self, node: NodeId, cx: f32, cy: f32, cw: f32, kids: &[NodeId]) -> (BoxContent, f32) {
        self.layout_flex_or_grid(node, cx, cy, cw, kids)
    }

    /// Lay out a `display:grid` container via taffy, then place each item at its grid slot.
    fn layout_grid(&self, node: NodeId, cx: f32, cy: f32, cw: f32, kids: &[NodeId]) -> (BoxContent, f32) {
        self.layout_flex_or_grid(node, cx, cy, cw, kids)
    }

    /// Shared flex/grid layout via the unified taffy tree ([`taffy_tree::solve_subtree`]): the
    /// container and its directly-nested flex/grid descendants are solved in one tree, with
    /// block/inline/float/table children content-measured back into Manuk. Returns the
    /// container's child slots, then places each child (as a block within its slot).
    fn layout_flex_or_grid(&self, node: NodeId, cx: f32, cy: f32, cw: f32, kids: &[NodeId]) -> (BoxContent, f32) {
        let block_kids: Vec<NodeId> = kids.iter().copied().filter(|&k| self.dom.is_element(k)).collect();
        if block_kids.is_empty() {
            return (BoxContent::Block(vec![]), 0.0);
        }
        let container_h = match self.styles[&node].height {
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
                let aw = known.width.or(match avail.width {
                    taffy::AvailableSpace::Definite(w) => Some(w),
                    _ => None,
                });
                let (w, h) = self.measure_intrinsic(dn, aw);
                taffy::Size { width: known.width.unwrap_or(w), height: known.height.unwrap_or(h) }
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
            let s = &self.styles[&p.dom];
            let boxx = LayoutBox {
                rect: Rect { x: abs_x, y: abs_y, width: p.slot.width, height: p.slot.height },
                background: s.background_color,
                border: border_of(s),
                node: Some(p.dom),
                content: BoxContent::Block(children),
            };
            (boxx, p.slot.y + p.slot.height)
        } else {
            let mut item_floats = FloatContext::new(abs_x, abs_x + p.slot.width);
            let r = self.layout_block(p.dom, p.slot.width, Some(p.slot.height), abs_x, abs_y, 0.0, &mut item_floats);
            let mut boxx = r.boxx;
            // Taffy sized the item (grow/stretch/track height); when its own height is `auto`,
            // adopt taffy's slot height so it fills its flex line / grid cell.
            if self.styles[&p.dom].height == Dim::Auto && p.slot.height > boxx.rect.height {
                boxx.rect.height = p.slot.height;
            }
            let bottom = p.slot.y + r.margin_top + boxx.rect.height + r.margin_bottom;
            (boxx, bottom)
        }
    }

    /// Collect inline tokens (words) from a run of inline-level siblings, tracking
    /// inter-word spacing.
    fn collect_inline_group(&self, nodes: &[NodeId], cw: f32) -> Vec<InlineItem> {
        let mut out = Vec::new();
        let mut pending_space = false;
        let mut first = true;
        for &n in nodes {
            self.collect_inline_node(n, &mut out, &mut pending_space, &mut first, None, cw);
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
            NodeData::Text(t) => {
                let cs = &self.styles[&node];
                let style = text_style(cs, self.fonts);
                // `white-space` is inherited, so the text node carries it. `nowrap` and `pre`
                // both suppress wrapping between words.
                let no_wrap = matches!(cs.white_space, WhiteSpace::NoWrap | WhiteSpace::Pre);
                let mut buf = String::new();
                for ch in t.chars() {
                    if ch.is_whitespace() {
                        if !buf.is_empty() {
                            push_word(out, &mut buf, style, pending_space, first, owner, no_wrap);
                        }
                        *pending_space = true;
                    } else {
                        buf.push(ch);
                    }
                }
                if !buf.is_empty() {
                    push_word(out, &mut buf, style, pending_space, first, owner, no_wrap);
                }
            }
            NodeData::Element(_) => {
                let disp = self.styles.get(&node).map(|s| s.display);
                if disp == Some(Display::None) {
                    return;
                }
                // An `inline-block` (or inline-flex/grid) is an *atomic* inline box: lay it
                // out as a block right here and flow it like a word, rather than recursing
                // into its children as inline text.
                if matches!(disp, Some(Display::InlineBlock | Display::Flex | Display::Grid)) {
                    let s = &self.styles[&node];
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
                    });
                    *first = false;
                    *pending_space = false;
                    return;
                }
                // An inline element's horizontal padding + border occupies space in the flow
                // and extends its geometry — emit edge spacers around its content.
                let s = &self.styles[&node];
                let pad_l = s.padding.left.resolve(cw, 0.0) + s.border_width.left;
                let pad_r = s.padding.right.resolve(cw, 0.0) + s.border_width.right;
                if pad_l > 0.0 {
                    out.push(InlineItem::Spacer {
                        width: pad_l,
                        node: Some(node),
                        space_before: *pending_space && !*first,
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
                    out.push(InlineItem::Spacer { width: pad_r, node: Some(node), space_before: false });
                    *pending_space = false;
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
    fn layout_inline(
        &self,
        items: Vec<InlineItem>,
        cx: f32,
        cy: f32,
        cw: f32,
        align: TextAlign,
        floats: &FloatContext,
    ) -> (Vec<TextFragment>, Vec<LayoutBox>, f32) {
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
            // Per-item main-axis advance, leading space, cross-axis height, and the LineFrag
            // builder (positioned once the line's x is known).
            let (advance, space_w, est_h, no_wrap, make_frag): (f32, f32, f32, bool, Box<dyn FnOnce(f32) -> LineFrag>) =
                match item {
                    InlineItem::Word { text, style, space_before, node, no_wrap } => {
                        let key = style.font_key;
                        let size = style.font_size;
                        let lm = self.fonts.line_metrics(key, size);
                        let word_w = self.fonts.measure(&text, key, size);
                        let space_w = if space_before { self.fonts.measure(" ", key, size) } else { 0.0 };
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
                                atomic: None,
                                atomic_h: 0.0,
                                valign: VerticalAlign::Baseline,
                            }),
                        )
                    }
                    InlineItem::Atomic { box_, advance, height, space_before, valign } => {
                        // Whitespace around an atomic uses the default text space width.
                        let key = FontKey { family: FontFamily::SansSerif, bold: false, italic: false };
                        let space_w = if space_before { self.fonts.measure(" ", key, 16.0) } else { 0.0 };
                        (
                            advance,
                            space_w,
                            height,
                            false,
                            Box::new(move |x: f32| LineFrag {
                                x,
                                width: advance,
                                text: String::new(),
                                style: TextStyle { font_key: key, font_size: 16.0, color: Rgba::BLACK, line_height: height },
                                // Treated as all-ascent so text on the same line shares the top.
                                ascent: height,
                                descent: 0.0,
                                node: None,
                                atomic: Some(box_),
                                atomic_h: height,
                                valign,
                            }),
                        )
                    }
                    InlineItem::Spacer { width, node, space_before } => {
                        // Inline padding/border: occupies `width`, paints nothing, but its
                        // (empty-text) fragment carries the owning element's geometry.
                        let key = FontKey { family: FontFamily::SansSerif, bold: false, italic: false };
                        let space_w = if space_before { self.fonts.measure(" ", key, 16.0) } else { 0.0 };
                        (
                            width,
                            space_w,
                            0.0,
                            true, // padding never introduces a break within its element
                            Box::new(move |x: f32| LineFrag {
                                x,
                                width,
                                text: String::new(),
                                style: TextStyle { font_key: key, font_size: 16.0, color: Rgba::BLACK, line_height: 0.0 },
                                ascent: 0.0,
                                descent: 0.0,
                                node,
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
                y = close_line(&mut frags, &mut atomic_boxes, &mut cur, y, line_left, line_avail, align, self.fonts);
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
            y = close_line(&mut frags, &mut atomic_boxes, &mut cur, y, line_left, line_avail, align, self.fonts);
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
    fonts: &FontContext,
) -> f32 {
    let ascent = line.iter().map(|f| f.ascent).fold(0.0, f32::max);
    let descent = line.iter().map(|f| f.descent).fold(0.0, f32::max);
    let pref = line.iter().map(|f| f.style.line_height).fold(0.0, f32::max);
    // An inline-block's margin-box height participates in the line height.
    let tallest_atomic = line.iter().map(|f| f.atomic_h).fold(0.0, f32::max);
    let content_h = (ascent + descent).max(tallest_atomic);
    let line_h = pref.max(content_h);
    let leading = ((line_h - (ascent + descent)) / 2.0).max(0.0);
    let baseline = y + leading + ascent;

    let line_width = line
        .last()
        .map(|f| {
            if f.atomic.is_some() {
                f.x + f.width
            } else {
                f.x + fonts.measure(&f.text, f.style.font_key, f.style.font_size)
            }
        })
        .unwrap_or(0.0);
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
            frags.push(TextFragment {
                x: fx,
                line_top: y,
                baseline,
                width: f.width,
                text: f.text,
                style: f.style,
                node: f.node,
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
    },
    /// An `inline-block`: `advance` is its margin-box main-axis size; `box_` is its already
    /// laid-out block box (positioned at the origin, translated into place at line close).
    Atomic {
        box_: Box<LayoutBox>,
        advance: f32,
        height: f32,
        space_before: bool,
        valign: VerticalAlign,
    },
    /// Horizontal padding/border of an inline element (`<span style="padding:0 15px">`):
    /// occupies `width` in the flow and extends the owning element's geometry, but paints
    /// nothing itself.
    Spacer {
        width: f32,
        node: Option<NodeId>,
        space_before: bool,
    },
}


fn push_word(
    out: &mut Vec<InlineItem>,
    buf: &mut String,
    style: TextStyle,
    pending_space: &mut bool,
    first: &mut bool,
    node: Option<NodeId>,
    no_wrap: bool,
) {
    out.push(InlineItem::Word {
        text: std::mem::take(buf),
        style,
        space_before: *pending_space && !*first,
        node,
        no_wrap,
    });
    *first = false;
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
            let n = dom.descendants(dom.root()).find(|&n| dom.element(n).and_then(|e| e.id()) == Some(id)).unwrap();
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
        assert!((below.y - 30.0).abs() < 1.0, "block drops below the inline line: {below:?}");
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
        assert!(ar.width > 0.0 && ar.height > 0.0, "degenerate <a> rect: {ar:?}");

        // The <a> is strictly narrower than its containing <p> block box, and sits
        // inside it — i.e. it is a genuine sub-rect, not the parent's box copied.
        let pr = rects.get(&p).unwrap();
        assert!(ar.width < pr.width, "a={ar:?} should be narrower than p={pr:?}");
        assert!(ar.x >= pr.x && ar.right() <= pr.right() + 0.01);

        // "before" precedes the link on the same line, so the link starts to its right.
        assert!(ar.x > pr.x, "link should not start at the paragraph's left edge");
    }

    /// A run is unioned into its element ancestors, so `<a><em>x</em></a>` gives the
    /// `<a>` a rect too — not only the innermost `<em>`.
    #[test]
    fn node_rects_propagates_runs_to_element_ancestors() {
        let (dom, root) = layout_html("<body><p><a href='/x'><em>hi</em></a></p></body>", "", 800.0);
        let rects = root.node_rects(&dom);
        let a = dom.find_first("a").unwrap();
        let em = dom.find_first("em").unwrap();
        let ar = rects.get(&a).expect("<a> gets geometry from its descendant run");
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
}
