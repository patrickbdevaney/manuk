//! manuk-css — the style engine.
//!
//! CLAUDE.md names **Stylo** (Servo/Firefox's production CSS engine) as the reuse
//! target for CSS parsing + cascade. Stylo is heavy to build and drive, so it sits
//! behind the [`StyleEngine`] trait and the `stylo` cargo feature. The default
//! build ships [`MinimalCascade`] — a from-scratch cascade over a documented CSS
//! subset — so the whole workspace compiles, runs, and is testable without it.
//!
//! The subset is deliberately small (tag/id/class/`*` selectors, the descendant
//! combinator, and the box/text properties layout+paint consume). It is enough to
//! render real content; it is **not** a conformance target. Conformance is Stylo's
//! job, verified against the WPT `css/` suites (CLAUDE.md § verification).
//!
//! `cssparser` (the tokenizer Stylo itself is built on) is reused for robust
//! length/number tokenization; see [`values`].

use std::collections::HashMap;

use manuk_dom::{Dom, ElementData, NodeData, NodeId};

pub mod values;

pub use values::Rgba;

/// A resolved length in one of the forms layout understands. `em`/`rem` are
/// resolved to `Px` during the cascade (font sizes are known there); `%` and
/// `Auto` are resolved later against the containing block by layout.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Dim {
    Auto,
    Px(f32),
    Percent(f32),
    /// A `calc()` reduced to `px + pct% of the reference` — the common linear form.
    Calc {
        px: f32,
        pct: f32,
    },
}

impl Dim {
    /// Resolve to px against a containing-block reference length. `Auto` -> `auto_px`.
    pub fn resolve(self, reference: f32, auto_px: f32) -> f32 {
        match self {
            Dim::Auto => auto_px,
            Dim::Px(v) => v,
            Dim::Percent(p) => reference * p / 100.0,
            Dim::Calc { px, pct } => px + reference * pct / 100.0,
        }
    }
    pub fn is_auto(self) -> bool {
        matches!(self, Dim::Auto)
    }
}

/// The `display` outer type, subset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Display {
    Block,
    Inline,
    InlineBlock,
    Flex,
    Grid,
    Table,
    TableRowGroup,
    TableRow,
    TableCell,
    TableCaption,
    TableColumn,
    TableColumnGroup,
    None,
}

/// `table-layout` (CSS2 §17.5.2): fixed uses the first row / explicit widths; auto
/// sizes columns to content.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TableLayout {
    #[default]
    Auto,
    Fixed,
}

/// `float`, which pulls a box out of normal flow to one side (CSS2 §9.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Float {
    #[default]
    None,
    Left,
    Right,
}

/// `clear`, which pushes a box below preceding floats on the named side(s).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Clear {
    #[default]
    None,
    Left,
    Right,
    Both,
}

/// `position` (CSS2 §9.3 + CSS-Position sticky).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Position {
    #[default]
    Static,
    Relative,
    Absolute,
    Fixed,
    Sticky,
}

/// `overflow` — whether content is clipped to the box. We clip for every non-`visible`
/// value (scrolling of the clipped content is a follow-on); this is the visual-correctness
/// win real pages depend on (overflow:hidden containment, clearfix, avatars).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Overflow {
    #[default]
    Visible,
    Hidden,
    Scroll,
    Auto,
    Clip,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    Center,
    Right,
    Justify,
}

/// `white-space`, which drives inline wrapping/collapsing in layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WhiteSpace {
    Normal,
    NoWrap,
    Pre,
}

/// `box-sizing`: whether `width`/`height` size the content box (CSS default) or the
/// border box (padding + border counted inside the given dimension).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoxSizing {
    ContentBox,
    BorderBox,
}

/// `vertical-align` for inline-level boxes (the common keywords).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerticalAlign {
    Baseline,
    Top,
    Middle,
    Bottom,
    TextTop,
    TextBottom,
    Sub,
    Super,
}

/// `justify-content` — main-axis distribution of flex items.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JustifyContent {
    FlexStart,
    FlexEnd,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// `align-items` — cross-axis alignment of flex items.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlignItems {
    Stretch,
    FlexStart,
    FlexEnd,
    Center,
    Baseline,
}

/// `flex-direction` — the flex main axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlexDirection {
    Row,
    RowReverse,
    Column,
    ColumnReverse,
}

/// `flex-wrap` — whether flex items wrap onto multiple lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlexWrap {
    NoWrap,
    Wrap,
    WrapReverse,
}

/// A single `transform` function. Resolved to an affine matrix by layout (the `Translate`
/// dimensions may be percentages of the box's own size).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TransformFn {
    Translate(Dim, Dim),
    Scale(f32, f32),
    /// Rotation in radians.
    Rotate(f32),
    /// Skew angles (x, y) in radians.
    Skew(f32, f32),
    /// A raw `matrix(a,b,c,d,e,f)`.
    Matrix([f32; 6]),
}

/// A single grid track sizing unit (a `minmax()` bound or a plain track).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TrackUnit {
    Px(f32),
    Fr(f32),
    Percent(f32),
    Auto,
    MinContent,
    MaxContent,
}

/// One CSS Grid track size (`grid-template-columns`/`-rows` entry).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TrackSize {
    Px(f32),
    /// A flexible `fr` track.
    Fr(f32),
    Percent(f32),
    Auto,
    MinContent,
    MaxContent,
    /// `minmax(min, max)`.
    MinMax(TrackUnit, TrackUnit),
}

/// A grid item's placement on one axis (`grid-column` / `grid-row`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum GridLine {
    #[default]
    Auto,
    /// An explicit line number (1-based; negative counts from the end).
    Line(i16),
    /// `span N`.
    Span(u16),
}

/// A resolved `grid-template-areas` named cell region: 1-indexed grid-line ranges
/// `[start, end)` on each axis. Stylo pre-resolves the ASCII art into these rects.
#[derive(Clone, Debug, PartialEq)]
pub struct GridAreaRect {
    pub name: String,
    /// Row grid lines `(start, end)`, 1-indexed.
    pub row: (u16, u16),
    /// Column grid lines `(start, end)`, 1-indexed.
    pub col: (u16, u16),
}

/// Four-sided box values (margin, padding, border widths).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sides<T> {
    pub top: T,
    pub right: T,
    pub bottom: T,
    pub left: T,
}

/// A single **outer** `box-shadow`: `offset-x offset-y blur color`. `spread`, `inset`, and
/// multiple comma-separated shadows are follow-ons — this is the shape real pages overwhelmingly
/// use (a soft drop shadow under a card).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoxShadow {
    pub dx: f32,
    pub dy: f32,
    /// Blur radius in px (`0` = a hard-edged offset rect).
    pub blur: f32,
    pub color: Rgba,
}

impl<T: Copy> Sides<T> {
    pub fn all(v: T) -> Self {
        Sides {
            top: v,
            right: v,
            bottom: v,
            left: v,
        }
    }
}

/// Generic font family we can actually resolve (via fontdb's generic queries). Named
/// families in a `font-family` list that we can't map are skipped in favour of the first
/// recognizable generic; the property is inherited.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenericFamily {
    SansSerif,
    Serif,
    Monospace,
}

/// The fully-resolved style of one element, as consumed by layout and paint.
#[derive(Clone, Debug, PartialEq)]
pub struct ComputedStyle {
    pub display: Display,
    pub color: Rgba,
    pub background_color: Option<Rgba>,
    pub font_size: f32,
    pub font_weight: u16,
    /// The `font-family` list (names in priority order, lowercased; generic keywords kept
    /// literally, e.g. `"sans-serif"`). Resolved to a concrete face by the text layer.
    pub font_family: Vec<String>,
    pub italic: bool,
    pub line_height: f32,
    pub text_align: TextAlign,
    pub white_space: WhiteSpace,
    pub margin: Sides<Dim>,
    pub padding: Sides<Dim>,
    pub border_width: Sides<f32>,
    pub border_color: Rgba,
    /// `border-radius` — a single uniform corner radius in px (per-corner radii are a follow-on).
    /// `0.0` = square corners.
    pub border_radius: f32,
    /// `box-shadow` — the first outer shadow, if any (multiple shadows / `inset` / `spread` are
    /// follow-ons).
    pub box_shadow: Option<BoxShadow>,
    pub width: Dim,
    pub height: Dim,
    /// `min-*`/`max-*` sizing. `Dim::Auto` on a min means 0; on a max means "no limit".
    pub min_width: Dim,
    pub max_width: Dim,
    pub min_height: Dim,
    pub max_height: Dim,
    pub float: Float,
    pub clear: Clear,
    pub position: Position,
    /// `top`/`right`/`bottom`/`left` insets; `Dim::Auto` means "not set".
    pub inset: Sides<Dim>,
    /// `z-index`; `None` = `auto`.
    pub z_index: Option<i32>,
    /// `overflow` (the more-clipping of overflow-x/overflow-y). `Visible` = no clip; any
    /// other value clips descendants to this element's padding box.
    pub overflow: Overflow,
    pub table_layout: TableLayout,
    /// `border-spacing` (px) between table cells in the separated-borders model.
    pub border_spacing: f32,
    /// `border-collapse: collapse` — cells share borders (no border-spacing).
    pub border_collapse: bool,
    /// `box-sizing` — whether `width`/`height` measure the content box or the border box.
    pub box_sizing: BoxSizing,
    /// `justify-content` — flex main-axis distribution (only meaningful on a flex container).
    pub justify_content: JustifyContent,
    /// `align-items` — flex cross-axis alignment (only meaningful on a flex container).
    pub align_items: AlignItems,
    /// `flex-direction` (container).
    pub flex_direction: FlexDirection,
    /// `flex-wrap` (container).
    pub flex_wrap: FlexWrap,
    /// `row-gap` / `column-gap` (container), px.
    pub row_gap: f32,
    pub column_gap: f32,
    /// `flex-grow` / `flex-shrink` (item).
    pub flex_grow: f32,
    pub flex_shrink: f32,
    /// `flex-basis` (item); `Dim::Auto` = `auto`.
    pub flex_basis: Dim,
    /// `align-self` (item); `None` = `auto` (defer to the container's `align-items`).
    pub align_self: Option<AlignItems>,
    /// `transform` — an ordered list of transform functions (translate/scale/rotate/skew/
    /// matrix), resolved to an affine matrix at layout time (translate `%` is the box's own
    /// size). Empty = `none`.
    pub transform: Vec<TransformFn>,
    /// `vertical-align` — cross-axis alignment of an inline-level box on its line.
    pub vertical_align: VerticalAlign,
    /// `grid-template-columns` / `-rows` (container). Empty = none.
    pub grid_template_columns: Vec<TrackSize>,
    pub grid_template_rows: Vec<TrackSize>,
    /// `grid-column` / `grid-row` (item) start/end line placement.
    pub grid_column: (GridLine, GridLine),
    pub grid_row: (GridLine, GridLine),
    /// Container: `grid-template-areas` resolved to named line-rects.
    pub grid_template_areas: Vec<GridAreaRect>,
    /// Item: the named area this element is placed into (via `grid-area: name`).
    pub grid_area: Option<String>,
}

impl ComputedStyle {
    /// The CSS initial values, used as the root's starting point and for
    /// non-inherited resets.
    pub fn initial() -> Self {
        ComputedStyle {
            display: Display::Inline,
            color: Rgba::BLACK,
            background_color: None,
            font_size: 16.0,
            font_weight: 400,
            font_family: vec!["sans-serif".to_string()],
            italic: false,
            line_height: 16.0 * 1.2,
            text_align: TextAlign::Left,
            white_space: WhiteSpace::Normal,
            margin: Sides::all(Dim::Px(0.0)),
            padding: Sides::all(Dim::Px(0.0)),
            border_width: Sides::all(0.0),
            border_color: Rgba::BLACK,
            border_radius: 0.0,
            box_shadow: None,
            width: Dim::Auto,
            height: Dim::Auto,
            min_width: Dim::Auto,
            max_width: Dim::Auto,
            min_height: Dim::Auto,
            max_height: Dim::Auto,
            float: Float::None,
            clear: Clear::None,
            position: Position::Static,
            inset: Sides::all(Dim::Auto),
            z_index: None,
            overflow: Overflow::Visible,
            table_layout: TableLayout::Auto,
            border_spacing: 0.0,
            border_collapse: false,
            box_sizing: BoxSizing::ContentBox,
            justify_content: JustifyContent::FlexStart,
            align_items: AlignItems::Stretch,
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::NoWrap,
            row_gap: 0.0,
            column_gap: 0.0,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            flex_basis: Dim::Auto,
            align_self: None,
            transform: Vec::new(),
            vertical_align: VerticalAlign::Baseline,
            grid_template_columns: Vec::new(),
            grid_template_rows: Vec::new(),
            grid_column: (GridLine::Auto, GridLine::Auto),
            grid_row: (GridLine::Auto, GridLine::Auto),
            grid_template_areas: Vec::new(),
            grid_area: None,
        }
    }

    /// Produce a child's starting style: inherited properties flow down, everything
    /// else resets to initial. (CSS inheritance model.)
    fn inherit_from(parent: &ComputedStyle) -> Self {
        let mut s = ComputedStyle::initial();
        s.color = parent.color;
        s.font_size = parent.font_size;
        s.font_weight = parent.font_weight;
        s.font_family = parent.font_family.clone();
        s.italic = parent.italic;
        s.line_height = parent.line_height;
        s.text_align = parent.text_align;
        s.white_space = parent.white_space;
        s
    }
}

/// Map from DOM node to its computed style. Text nodes inherit their parent's.
pub type StyleMap = HashMap<NodeId, ComputedStyle>;

/// E1 **full-page zoom** — scale every *absolute* length in `style` by `k`.
///
/// Percentages and `auto` are deliberately left alone: they resolve against a
/// containing block that has itself been scaled, so scaling them too would compound.
/// This is what makes browser zoom *reflow* (and therefore stay crisp) rather than
/// magnify a bitmap: `font_size` grows, so glyphs are rasterized at the larger size.
pub fn scale_style(style: &ComputedStyle, k: f32) -> ComputedStyle {
    fn dim(d: Dim, k: f32) -> Dim {
        match d {
            Dim::Px(v) => Dim::Px(v * k),
            // Percent / Auto resolve against an already-scaled reference.
            other => other,
        }
    }
    fn sides_dim(s: Sides<Dim>, k: f32) -> Sides<Dim> {
        Sides {
            top: dim(s.top, k),
            right: dim(s.right, k),
            bottom: dim(s.bottom, k),
            left: dim(s.left, k),
        }
    }
    fn sides_px(s: Sides<f32>, k: f32) -> Sides<f32> {
        Sides {
            top: s.top * k,
            right: s.right * k,
            bottom: s.bottom * k,
            left: s.left * k,
        }
    }
    ComputedStyle {
        font_size: style.font_size * k,
        line_height: style.line_height * k,
        margin: sides_dim(style.margin, k),
        padding: sides_dim(style.padding, k),
        border_width: sides_px(style.border_width, k),
        width: dim(style.width, k),
        height: dim(style.height, k),
        inset: sides_dim(style.inset, k),
        border_spacing: style.border_spacing * k,
        ..style.clone()
    }
}

/// Scale a whole [`StyleMap`] for full-page zoom. Always derive from the *base* map;
/// scaling an already-scaled map compounds.
pub fn zoom_styles(styles: &StyleMap, k: f32) -> StyleMap {
    styles.iter().map(|(n, s)| (*n, scale_style(s, k))).collect()
}

/// How much work a style change forces (A2 incremental-layout damage taxonomy,
/// Servo's `RestyleDamage` idea). Ordered least→most expensive; a subtree's damage is
/// the max of its own and its children's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum RestyleDamage {
    /// Styles are identical — reuse the cached box and paint.
    #[default]
    None,
    /// Only paint-affecting properties changed (color/background/border-color/
    /// z-index) — reuse layout, repaint the box.
    Repaint,
    /// Geometry-affecting properties changed — re-lay-out this box (its box-tree
    /// structure is unchanged).
    Reflow,
    /// The generated box structure changes (`display` outer type) — rebuild the box.
    Rebuild,
}

/// Diff two computed styles into the [`RestyleDamage`] their change forces.
pub fn diff_style(old: &ComputedStyle, new: &ComputedStyle) -> RestyleDamage {
    if old == new {
        return RestyleDamage::None;
    }
    // A `display` outer-type change alters which boxes are generated.
    if old.display != new.display {
        return RestyleDamage::Rebuild;
    }
    // Geometry-affecting properties → re-lay-out this box.
    let reflow = old.width != new.width
        || old.height != new.height
        || old.margin != new.margin
        || old.padding != new.padding
        || old.border_width != new.border_width
        || old.font_size != new.font_size
        || old.font_weight != new.font_weight
        || old.font_family != new.font_family
        || old.italic != new.italic
        || old.line_height != new.line_height
        || old.text_align != new.text_align
        || old.white_space != new.white_space
        || old.float != new.float
        || old.clear != new.clear
        || old.position != new.position
        || old.inset != new.inset
        || old.table_layout != new.table_layout
        || old.border_spacing != new.border_spacing;
    if reflow {
        RestyleDamage::Reflow
    } else {
        // Everything remaining is paint-only (color/background/border-color/z-index).
        RestyleDamage::Repaint
    }
}

// ---------------------------------------------------------------------------
// Selectors
// ---------------------------------------------------------------------------

/// An attribute selector `[name]`, `[name=val]`, `[name~=val]`, etc.
#[derive(Clone, Debug, PartialEq)]
struct AttrSel {
    name: String,
    op: AttrOp,
    value: String,
}

#[derive(Clone, Debug, PartialEq)]
enum AttrOp {
    /// `[name]`
    Exists,
    /// `[name=val]`
    Equals,
    /// `[name~=val]` — whitespace-separated word list contains `val`.
    Includes,
    /// `[name^=val]`
    Prefix,
    /// `[name$=val]`
    Suffix,
    /// `[name*=val]`
    Substring,
    /// `[name|=val]` — equals `val` or starts with `val-`.
    DashMatch,
}

/// A simple pseudo-class we can evaluate. Dynamic pseudos that need interaction state we
/// don't have (`:hover`, `:focus`, …) are modelled as [`Pseudo::NeverStatic`] so a rule
/// gated on them simply doesn't apply to a static render (rather than dropping the rule).
#[derive(Clone, Debug, PartialEq)]
enum Pseudo {
    FirstChild,
    LastChild,
    OnlyChild,
    /// `:nth-child(an+b)` — coefficients `a`, `b` (1-based index among element siblings).
    NthChild(i32, i32),
    Root,
    Empty,
    Checked,
    Disabled,
    Enabled,
    Required,
    Link,
    /// `:not(<compound>)` — a single inner compound (no combinators).
    Not(Box<Compound>),
    /// `:hover`/`:focus`/`:active`/`:visited`/`:target`/… — never matches statically.
    NeverStatic,
}

/// How a compound relates to the compound on its **right** in a selector chain.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Combinator {
    Descendant,
    Child,
    NextSibling,
    SubsequentSibling,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Compound {
    universal: bool,
    tag: Option<String>,
    id: Option<String>,
    classes: Vec<String>,
    attrs: Vec<AttrSel>,
    pseudos: Vec<Pseudo>,
}

/// A selector chain; `parts[last]` is the subject (rightmost). `combinators[i]` links
/// `parts[i]` to `parts[i+1]` (so it has `parts.len() - 1` entries).
#[derive(Clone, Debug, PartialEq)]
struct Selector {
    parts: Vec<Compound>,
    combinators: Vec<Combinator>,
    /// N4 — `::slotted(<compound>)`. The subject compound is the *inner* selector, and it
    /// matches a **light-DOM** element assigned to a slot inside this sheet's shadow root.
    /// That is the one selector that deliberately reaches across the shadow boundary.
    slotted: bool,
}

impl Selector {
    /// (#id, #class/attr, #type) specificity, packed big-endian into a u32.
    fn specificity(&self) -> u32 {
        let (mut a, mut b, mut c) = (0u32, 0u32, 0u32);
        for p in &self.parts {
            if p.id.is_some() {
                a += 1;
            }
            // Classes, attribute selectors, and pseudo-classes are all class-level.
            b += (p.classes.len() + p.attrs.len() + p.pseudos.len()) as u32;
            if p.tag.is_some() {
                c += 1;
            }
        }
        (a.min(255) << 16) | (b.min(255) << 8) | c.min(255)
    }
}

/// The previous element sibling of `node` (skipping text/comment nodes), if any.
fn prev_element_sibling(dom: &Dom, node: NodeId) -> Option<NodeId> {
    let mut cur = dom.prev_sibling(node);
    while let Some(n) = cur {
        if dom.is_element(n) {
            return Some(n);
        }
        cur = dom.prev_sibling(n);
    }
    None
}

/// 1-based index of `node` among its element siblings, and the total element-sibling count.
fn element_sibling_position(dom: &Dom, node: NodeId) -> (usize, usize) {
    let Some(parent) = dom.parent(node) else {
        return (1, 1);
    };
    let mut index = 0;
    let mut total = 0;
    for c in dom.children(parent) {
        if dom.is_element(c) {
            total += 1;
            if c == node {
                index = total;
            }
        }
    }
    (index.max(1), total.max(1))
}

fn pseudo_matches(p: &Pseudo, dom: &Dom, node: NodeId) -> bool {
    let el = match dom.element(node) {
        Some(e) => e,
        None => return false,
    };
    match p {
        Pseudo::FirstChild => prev_element_sibling(dom, node).is_none(),
        Pseudo::LastChild => {
            let mut cur = dom.next_sibling(node);
            while let Some(n) = cur {
                if dom.is_element(n) {
                    return false;
                }
                cur = dom.next_sibling(n);
            }
            true
        }
        Pseudo::OnlyChild => {
            prev_element_sibling(dom, node).is_none()
                && pseudo_matches(&Pseudo::LastChild, dom, node)
        }
        Pseudo::NthChild(a, b) => {
            let (idx, _) = element_sibling_position(dom, node);
            let idx = idx as i32;
            // idx == a*n + b for some integer n >= 0.
            if *a == 0 {
                idx == *b
            } else {
                let n = (idx - b) / a;
                n >= 0 && a * n + b == idx
            }
        }
        Pseudo::Root => dom.parent(node).map(|p| !dom.is_element(p)).unwrap_or(false),
        Pseudo::Empty => !dom.children(node).any(|c| {
            dom.is_element(c) || matches!(dom.data(c), NodeData::Text(t) if !t.trim().is_empty())
        }),
        Pseudo::Checked => el.attr("checked").is_some() || el.attr("selected").is_some(),
        Pseudo::Disabled => el.attr("disabled").is_some(),
        Pseudo::Enabled => {
            matches!(el.name.as_str(), "input" | "button" | "select" | "textarea" | "option")
                && el.attr("disabled").is_none()
        }
        Pseudo::Required => el.attr("required").is_some(),
        Pseudo::Link => {
            matches!(el.name.as_str(), "a" | "area" | "link") && el.attr("href").is_some()
        }
        Pseudo::Not(inner) => !compound_matches(inner, dom, node),
        Pseudo::NeverStatic => false,
    }
}

fn attr_matches(a: &AttrSel, dom: &Dom, node: NodeId) -> bool {
    let Some(el) = dom.element(node) else {
        return false;
    };
    let Some(actual) = el.attr(&a.name) else {
        return false;
    };
    match a.op {
        AttrOp::Exists => true,
        AttrOp::Equals => actual == a.value,
        AttrOp::Includes => actual.split_whitespace().any(|w| w == a.value),
        AttrOp::Prefix => !a.value.is_empty() && actual.starts_with(&a.value),
        AttrOp::Suffix => !a.value.is_empty() && actual.ends_with(&a.value),
        AttrOp::Substring => !a.value.is_empty() && actual.contains(&a.value),
        AttrOp::DashMatch => actual == a.value || actual.starts_with(&format!("{}-", a.value)),
    }
}

fn compound_matches(c: &Compound, dom: &Dom, node: NodeId) -> bool {
    let Some(el) = dom.element(node) else {
        return false;
    };
    if let Some(tag) = &c.tag {
        if !el.name.eq_ignore_ascii_case(tag) {
            return false;
        }
    }
    if let Some(id) = &c.id {
        if el.id() != Some(id.as_str()) {
            return false;
        }
    }
    for class in &c.classes {
        if !el.has_class(class) {
            return false;
        }
    }
    for a in &c.attrs {
        if !attr_matches(a, dom, node) {
            return false;
        }
    }
    for p in &c.pseudos {
        if !pseudo_matches(p, dom, node) {
            return false;
        }
    }
    true
}

/// Does `node` match the CSS selector string `sel` (comma-separated list)? Reuses
/// the cascade's own selector engine, so `querySelector`-style APIs and the cascade
/// agree. Supports the documented subset (tag/id/class/`*` + descendant combinator).
/// N4 — a stylesheet plus the **tree scope** it belongs to.
///
/// `scope == None` is the document; `scope == Some(shadow_root)` is that shadow tree.
/// Encapsulation is exactly this: a sheet only sees elements in its own scope. The single
/// deliberate exception is `::slotted()`, which reaches out to the light-DOM nodes slotted
/// into the sheet's own shadow tree.
#[derive(Clone, Debug)]
pub struct ScopedSheet {
    pub scope: Option<NodeId>,
    pub sheet: Stylesheet,
}

/// Whether a sheet scoped to `scope` may style `node` at all (before selector matching).
fn scope_allows(dom: &Dom, node: NodeId, scope: Option<NodeId>) -> bool {
    dom.enclosing_shadow_root(node) == scope
}

/// `::slotted(x)` from shadow root `S` matches `node` when `node` is a light-DOM element
/// assigned to a slot **inside `S`**, and `x` matches it.
fn slotted_matches(dom: &Dom, node: NodeId, scope: Option<NodeId>, subject: &Compound) -> bool {
    let Some(shadow) = scope else {
        // `::slotted()` outside a shadow tree never matches anything.
        return false;
    };
    let Some(slot) = dom.assigned_slot(node) else {
        return false;
    };
    dom.enclosing_shadow_root(slot) == Some(shadow) && compound_matches(subject, dom, node)
}

/// Match `sel` against `node` for a sheet in `scope`.
fn selector_matches_scoped(
    sel: &Selector,
    dom: &Dom,
    node: NodeId,
    scope: Option<NodeId>,
) -> bool {
    if sel.slotted {
        let subject = sel.parts.last().expect("::slotted has one compound");
        return slotted_matches(dom, node, scope, subject);
    }
    scope_allows(dom, node, scope) && selector_matches(sel, dom, node)
}

pub fn matches_selector(dom: &Dom, node: NodeId, sel: &str) -> bool {
    dom.is_element(node)
        && parse_selector_list(sel)
            .iter()
            .any(|s| selector_matches(s, dom, node))
}

/// First element in document order within `root`'s subtree (excluding `root`)
/// matching `sel`, or `None`. The engine-shared analog of `Element.querySelector`.
pub fn query_selector(dom: &Dom, root: NodeId, sel: &str) -> Option<NodeId> {
    let sels = parse_selector_list(sel);
    if sels.is_empty() {
        return None;
    }
    dom.descendants(root)
        .find(|&n| dom.is_element(n) && sels.iter().any(|s| selector_matches(s, dom, n)))
}

/// All elements in document order within `root`'s subtree matching `sel`
/// (`Element.querySelectorAll`).
pub fn query_selector_all(dom: &Dom, root: NodeId, sel: &str) -> Vec<NodeId> {
    let sels = parse_selector_list(sel);
    if sels.is_empty() {
        return Vec::new();
    }
    dom.descendants(root)
        .filter(|&n| dom.is_element(n) && sels.iter().any(|s| selector_matches(s, dom, n)))
        .collect()
}

fn selector_matches(sel: &Selector, dom: &Dom, node: NodeId) -> bool {
    let Some((subject, left)) = sel.parts.split_last() else {
        return false;
    };
    if !compound_matches(subject, dom, node) {
        return false;
    }
    // Match the remaining compounds right-to-left, honouring each link's combinator.
    // `combinators[i]` links parts[i] to parts[i+1]; `right` tracks the node the
    // already-matched compound to our right landed on. Greedy (no backtracking) — correct
    // for the common selectors; a pathological descendant/sibling case could false-negative.
    let mut right = node;
    for i in (0..left.len()).rev() {
        let cand = &sel.parts[i];
        let comb = sel.combinators[i];
        match comb {
            Combinator::Child => {
                let Some(p) = dom.parent(right) else { return false };
                if !compound_matches(cand, dom, p) {
                    return false;
                }
                right = p;
            }
            Combinator::Descendant => {
                let mut cursor = dom.parent(right);
                loop {
                    let Some(anc) = cursor else { return false };
                    cursor = dom.parent(anc);
                    if compound_matches(cand, dom, anc) {
                        right = anc;
                        break;
                    }
                }
            }
            Combinator::NextSibling => {
                let Some(s) = prev_element_sibling(dom, right) else { return false };
                if !compound_matches(cand, dom, s) {
                    return false;
                }
                right = s;
            }
            Combinator::SubsequentSibling => {
                let mut cursor = prev_element_sibling(dom, right);
                loop {
                    let Some(sib) = cursor else { return false };
                    cursor = prev_element_sibling(dom, sib);
                    if compound_matches(cand, dom, sib) {
                        right = sib;
                        break;
                    }
                }
            }
        }
    }
    true
}

// ---------------------------------------------------------------------------
// Stylesheet parsing (subset)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Declaration {
    name: String,
    value: String,
    important: bool,
}

#[derive(Clone, Debug)]
struct Rule {
    selectors: Vec<Selector>,
    declarations: Vec<Declaration>,
}

/// An `@font-face` rule: the family name it defines and its candidate source URLs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontFace {
    /// `font-family` (lowercased, dequoted) — the name author CSS references.
    pub family: String,
    /// `src` `url(...)` candidates, in order.
    pub srcs: Vec<String>,
}

/// A parsed stylesheet (subset). Build one with [`Stylesheet::parse`].
#[derive(Clone, Debug, Default)]
pub struct Stylesheet {
    rules: Vec<Rule>,
    /// The original CSS source, retained so the Stylo engine can re-parse it with
    /// Stylo's own (spec-complete) parser. Empty for programmatically-built sheets.
    source: String,
    /// `@font-face` rules captured during parse (for web-font loading).
    font_faces: Vec<FontFace>,
}

impl Stylesheet {
    /// The raw CSS text this sheet was parsed from (for the Stylo cascade path).
    pub fn source(&self) -> &str {
        &self.source
    }

    /// The `@font-face` rules this sheet declares.
    pub fn font_faces(&self) -> &[FontFace] {
        &self.font_faces
    }
}

/// Parse an `@font-face` block body into a [`FontFace`] (`family` + `src` urls).
fn parse_font_face_block(block: &str) -> Option<FontFace> {
    let mut family = None;
    let mut srcs = Vec::new();
    for d in parse_declarations(block) {
        match d.name.as_str() {
            "font-family" => {
                family = Some(d.value.trim().trim_matches(['"', '\'']).to_ascii_lowercase())
            }
            "src" => {
                let mut rest = d.value.as_str();
                while let Some(p) = rest.find("url(") {
                    let after = &rest[p + 4..];
                    if let Some(close) = after.find(')') {
                        let url = after[..close].trim().trim_matches(['"', '\'']).to_string();
                        if !url.is_empty() {
                            srcs.push(url);
                        }
                        rest = &after[close + 1..];
                    } else {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    let family = family.filter(|f| !f.is_empty())?;
    (!srcs.is_empty()).then_some(FontFace { family, srcs })
}

impl Stylesheet {
    /// Parse CSS source into rules. Comments and `@`-rules are skipped; unknown
    /// selectors/properties are ignored rather than aborting the sheet (CSS's
    /// forward-compatible error recovery).
    pub fn parse(src: &str) -> Stylesheet {
        let src = strip_comments(src);
        let mut rules = Vec::new();
        let mut font_faces = Vec::new();
        let bytes = src.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if i >= bytes.len() {
                break;
            }
            // @-rules: capture @font-face (for web fonts); skip the rest of the subset.
            if bytes[i] == b'@' {
                let end = skip_at_rule(&src, i);
                let rest = &src[i..];
                if rest.len() >= 10 && rest[..10].eq_ignore_ascii_case("@font-face") {
                    if let Some(open) = rest.find('{') {
                        let block = &src[i + open + 1..end.saturating_sub(1)];
                        if let Some(ff) = parse_font_face_block(block) {
                            font_faces.push(ff);
                        }
                    }
                }
                i = end;
                continue;
            }
            // Read up to the opening brace: the selector list.
            let sel_start = i;
            while i < bytes.len() && bytes[i] != b'{' {
                i += 1;
            }
            if i >= bytes.len() {
                break;
            }
            let selector_text = &src[sel_start..i];
            i += 1; // consume '{'
            let decl_start = i;
            while i < bytes.len() && bytes[i] != b'}' {
                i += 1;
            }
            let decl_text = &src[decl_start..i.min(bytes.len())];
            if i < bytes.len() {
                i += 1; // consume '}'
            }

            let selectors = parse_selector_list(selector_text);
            if selectors.is_empty() {
                continue;
            }
            let declarations = parse_declarations(decl_text);
            if !declarations.is_empty() {
                rules.push(Rule {
                    selectors,
                    declarations,
                });
            }
        }
        Stylesheet {
            rules,
            source: src,
            font_faces,
        }
    }
}

fn strip_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let b = src.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if i + 1 < b.len() && b[i] == b'/' && b[i + 1] == b'*' {
            i += 2;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            i += 2;
        } else {
            out.push(b[i] as char);
            i += 1;
        }
    }
    out
}

fn skip_at_rule(src: &str, start: usize) -> usize {
    let b = src.as_bytes();
    let mut i = start;
    // Skip to ';' (statement at-rule) or a balanced '{...}' (block at-rule).
    while i < b.len() {
        match b[i] {
            b';' => return i + 1,
            b'{' => {
                let mut depth = 0;
                while i < b.len() {
                    match b[i] {
                        b'{' => depth += 1,
                        b'}' => {
                            depth -= 1;
                            if depth == 0 {
                                return i + 1;
                            }
                        }
                        _ => {}
                    }
                    i += 1;
                }
                return i;
            }
            _ => i += 1,
        }
    }
    i
}

fn parse_selector_list(text: &str) -> Vec<Selector> {
    text.split(',')
        .filter_map(|s| parse_selector(s.trim()))
        .collect()
}

fn parse_selector(text: &str) -> Option<Selector> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }

    // N4 — `::slotted(<compound>)`. Only the standalone form is supported (no ancestor
    // chain), which is what shadow stylesheets actually write. Anything else is dropped
    // rather than mis-matched.
    if let Some(rest) = text.strip_prefix("::slotted(") {
        let inner = rest.strip_suffix(')')?.trim();
        if inner.is_empty() {
            return None;
        }
        let compound = parse_compound(inner)?;
        return Some(Selector {
            parts: vec![compound],
            combinators: vec![],
            slotted: true,
        });
    }
    // A pseudo-element we do not model must not silently match its subject.
    if text.contains("::") {
        return None;
    }

    // Tokenize into an alternating compound/combinator sequence, respecting `[...]` and
    // `(...)` nesting (so `[a~=b]` and `:nth-child(2n+1)` don't split on `~`/`+`).
    enum Tok {
        Comp(String),
        Comb(Combinator),
    }
    let mut toks: Vec<Tok> = Vec::new();
    let mut cur = String::new();
    let mut depth = 0i32;
    let flush = |cur: &mut String, toks: &mut Vec<Tok>| {
        if !cur.trim().is_empty() {
            toks.push(Tok::Comp(cur.trim().to_string()));
        }
        cur.clear();
    };
    for ch in text.chars() {
        match ch {
            '[' | '(' => {
                depth += 1;
                cur.push(ch);
            }
            ']' | ')' => {
                depth -= 1;
                cur.push(ch);
            }
            '>' | '+' | '~' if depth == 0 => {
                flush(&mut cur, &mut toks);
                toks.push(Tok::Comb(match ch {
                    '>' => Combinator::Child,
                    '+' => Combinator::NextSibling,
                    _ => Combinator::SubsequentSibling,
                }));
            }
            c if c.is_whitespace() && depth == 0 => {
                flush(&mut cur, &mut toks);
                toks.push(Tok::Comb(Combinator::Descendant));
            }
            _ => cur.push(ch),
        }
    }
    flush(&mut cur, &mut toks);

    // Collapse adjacent combinators (a whitespace next to an explicit `>`/`+`/`~` yields
    // two in a row): keep the explicit one, drop the tentative descendant. Drop any
    // leading/trailing combinator.
    let mut norm: Vec<Tok> = Vec::new();
    for t in toks {
        match t {
            Tok::Comb(c) => match norm.last_mut() {
                Some(Tok::Comb(prev)) => {
                    if *prev == Combinator::Descendant {
                        *prev = c;
                    }
                }
                Some(Tok::Comp(_)) => norm.push(Tok::Comb(c)),
                None => {} // leading combinator — ignore
            },
            Tok::Comp(s) => norm.push(Tok::Comp(s)),
        }
    }
    if let Some(Tok::Comb(_)) = norm.last() {
        norm.pop();
    }

    let mut parts = Vec::new();
    let mut combinators = Vec::new();
    for t in norm {
        match t {
            Tok::Comp(s) => parts.push(parse_compound(&s)?),
            Tok::Comb(c) => combinators.push(c),
        }
    }
    if parts.is_empty() || combinators.len() + 1 != parts.len() {
        None
    } else {
        Some(Selector {
            parts,
            combinators,
            slotted: false,
        })
    }
}

fn parse_compound(token: &str) -> Option<Compound> {
    let mut c = Compound::default();
    let mut chars = token.chars().peekable();
    // Optional leading type or universal.
    if let Some(&ch) = chars.peek() {
        if ch == '*' {
            c.universal = true;
            chars.next();
        } else if ch.is_ascii_alphabetic() {
            let mut tag = String::new();
            while let Some(&ch) = chars.peek() {
                if matches!(ch, '.' | '#' | '[' | ':') {
                    break;
                }
                tag.push(ch);
                chars.next();
            }
            c.tag = Some(tag.to_ascii_lowercase());
        }
    }
    while let Some(&ch) = chars.peek() {
        match ch {
            '.' => {
                chars.next();
                let name = take_ident(&mut chars);
                if name.is_empty() {
                    return None;
                }
                c.classes.push(name);
            }
            '#' => {
                chars.next();
                let name = take_ident(&mut chars);
                if name.is_empty() {
                    return None;
                }
                c.id = Some(name);
            }
            '[' => {
                chars.next(); // consume '['
                let mut inner = String::new();
                let mut closed = false;
                for ch in chars.by_ref() {
                    if ch == ']' {
                        closed = true;
                        break;
                    }
                    inner.push(ch);
                }
                if !closed {
                    return None;
                }
                c.attrs.push(parse_attr(&inner)?);
            }
            ':' => {
                chars.next(); // consume ':'
                // Read the pseudo name, then an optional parenthesised argument.
                let name = take_ident(&mut chars);
                if name.is_empty() {
                    return None;
                }
                let mut arg = None;
                if chars.peek() == Some(&'(') {
                    chars.next();
                    let mut a = String::new();
                    let mut d = 1i32;
                    for ch in chars.by_ref() {
                        match ch {
                            '(' => d += 1,
                            ')' => {
                                d -= 1;
                                if d == 0 {
                                    break;
                                }
                            }
                            _ => {}
                        }
                        a.push(ch);
                    }
                    arg = Some(a);
                }
                c.pseudos.push(parse_pseudo(&name, arg.as_deref())?);
            }
            // Anything else is out of the supported grammar; drop the selector.
            _ => return None,
        }
    }
    Some(c)
}

/// Parse the inside of an attribute selector `[...]` (the text between the brackets).
fn parse_attr(inner: &str) -> Option<AttrSel> {
    let inner = inner.trim();
    // Two-char operators first, then `=`.
    for (tok, op) in [
        ("~=", AttrOp::Includes),
        ("^=", AttrOp::Prefix),
        ("$=", AttrOp::Suffix),
        ("*=", AttrOp::Substring),
        ("|=", AttrOp::DashMatch),
    ] {
        if let Some((name, value)) = inner.split_once(tok) {
            return Some(AttrSel {
                name: name.trim().to_ascii_lowercase(),
                op,
                value: clean_attr_value(value),
            });
        }
    }
    if let Some((name, value)) = inner.split_once('=') {
        return Some(AttrSel {
            name: name.trim().to_ascii_lowercase(),
            op: AttrOp::Equals,
            value: clean_attr_value(value),
        });
    }
    if inner.is_empty() {
        return None;
    }
    Some(AttrSel {
        name: inner.to_ascii_lowercase(),
        op: AttrOp::Exists,
        value: String::new(),
    })
}

/// Strip quotes and a trailing case-sensitivity flag (` i`/` s`) from an attr value.
fn clean_attr_value(v: &str) -> String {
    let v = v.trim();
    let v = v
        .strip_suffix(" i")
        .or_else(|| v.strip_suffix(" s"))
        .unwrap_or(v)
        .trim();
    v.trim_matches(['"', '\'']).to_string()
}

fn parse_pseudo(name: &str, arg: Option<&str>) -> Option<Pseudo> {
    Some(match name.to_ascii_lowercase().as_str() {
        "first-child" => Pseudo::FirstChild,
        "last-child" => Pseudo::LastChild,
        "only-child" => Pseudo::OnlyChild,
        "root" => Pseudo::Root,
        "empty" => Pseudo::Empty,
        "checked" => Pseudo::Checked,
        "disabled" => Pseudo::Disabled,
        "enabled" => Pseudo::Enabled,
        "required" => Pseudo::Required,
        "link" | "any-link" => Pseudo::Link,
        // Dynamic / state pseudos we can't evaluate in a static render → never match, so a
        // rule gated on them just doesn't apply (rather than dropping the whole rule).
        "hover" | "focus" | "active" | "visited" | "target" | "focus-within"
        | "focus-visible" | "read-write" | "placeholder-shown" | "autofill" => Pseudo::NeverStatic,
        "nth-child" => {
            let (a, b) = parse_nth(arg?)?;
            Pseudo::NthChild(a, b)
        }
        "not" => {
            let inner = parse_compound(arg?.trim())?;
            Pseudo::Not(Box::new(inner))
        }
        // Unknown pseudo → drop the selector (conservative: better than mis-applying).
        _ => return None,
    })
}

/// Parse an `:nth-child()` argument (`odd`, `even`, `N`, `an+b`, `-n+b`, `2n`) into `(a, b)`.
fn parse_nth(arg: &str) -> Option<(i32, i32)> {
    let s = arg.trim().to_ascii_lowercase().replace(' ', "");
    match s.as_str() {
        "odd" => return Some((2, 1)),
        "even" => return Some((2, 0)),
        _ => {}
    }
    if let Some(idx) = s.find('n') {
        let (a_str, rest) = s.split_at(idx);
        let b_str = &rest[1..]; // skip 'n'
        let a = match a_str {
            "" | "+" => 1,
            "-" => -1,
            n => n.parse().ok()?,
        };
        let b = if b_str.is_empty() {
            0
        } else {
            b_str.parse().ok()?
        };
        Some((a, b))
    } else {
        Some((0, s.parse().ok()?))
    }
}

fn take_ident(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut s = String::new();
    while let Some(&ch) = chars.peek() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            s.push(ch);
            chars.next();
        } else {
            break;
        }
    }
    s
}

fn parse_declarations(text: &str) -> Vec<Declaration> {
    let mut decls = Vec::new();
    for chunk in text.split(';') {
        let chunk = chunk.trim();
        if chunk.is_empty() {
            continue;
        }
        let Some((name, value)) = chunk.split_once(':') else {
            continue;
        };
        let name = name.trim().to_ascii_lowercase();
        let mut value = value.trim().to_string();
        let important = value.to_ascii_lowercase().ends_with("!important");
        if important {
            let cut = value.len() - "!important".len();
            value = value[..cut]
                .trim_end()
                .trim_end_matches('!')
                .trim()
                .to_string();
        }
        if name.is_empty() || value.is_empty() {
            continue;
        }
        decls.push(Declaration {
            name,
            value,
            important,
        });
    }
    decls
}

// ---------------------------------------------------------------------------
// The StyleEngine boundary + minimal cascade
// ---------------------------------------------------------------------------

/// The pluggable cascade boundary. `MinimalCascade` is the default; the `stylo`
/// feature provides a Stylo-backed implementation with the same signature.
pub trait StyleEngine {
    /// Compute a style for every node in `dom`, applying UA defaults, the given
    /// author `sheets`, and inline `style=""` attributes.
    fn cascade(&self, dom: &Dom, sheets: &[Stylesheet]) -> StyleMap;
}

/// From-scratch cascade over the documented subset. See module docs.
#[derive(Debug, Default, Clone, Copy)]
pub struct MinimalCascade;

impl StyleEngine for MinimalCascade {
    fn cascade(&self, dom: &Dom, sheets: &[Stylesheet]) -> StyleMap {
        // Document-scoped sheets, plus every shadow root's own `<style>` elements.
        let mut scoped: Vec<ScopedSheet> = sheets
            .iter()
            .cloned()
            .map(|sheet| ScopedSheet { scope: None, sheet })
            .collect();
        scoped.extend(MinimalCascade::collect_shadow_stylesheets(dom));
        self.cascade_scoped(dom, &scoped)
    }
}

impl MinimalCascade {
    /// Gather author stylesheets embedded in the document's `<style>` elements.
    ///
    /// Shadow roots are **not** descendants of the document root, so their `<style>`
    /// elements are correctly excluded here — they are collected by
    /// [`collect_shadow_stylesheets`](Self::collect_shadow_stylesheets) with their scope.
    pub fn collect_style_elements(dom: &Dom) -> Vec<Stylesheet> {
        dom.descendants(dom.root())
            .filter(|&n| dom.tag_name(n) == Some("style"))
            .map(|n| Stylesheet::parse(&dom.text_content(n)))
            .collect()
    }

    /// N4 — every shadow root's `<style>` elements, each tagged with its scope.
    pub fn collect_shadow_stylesheets(dom: &Dom) -> Vec<ScopedSheet> {
        let mut out = Vec::new();
        for sr in dom.all_shadow_roots() {
            for n in dom.descendants(sr) {
                if dom.tag_name(n) == Some("style") {
                    out.push(ScopedSheet {
                        scope: Some(sr),
                        sheet: Stylesheet::parse(&dom.text_content(n)),
                    });
                }
            }
        }
        out
    }

    /// N4 — cascade over the **flat tree** with tree-scoped matching.
    ///
    /// Walking the flat tree is what makes shadow content styled and laid out at all, and
    /// it is also what makes inheritance correct: a slotted element inherits from the
    /// slot's flat-tree ancestors, not from its node-tree parent.
    pub fn cascade_scoped(&self, dom: &Dom, sheets: &[ScopedSheet]) -> StyleMap {
        let mut map = StyleMap::new();
        let root = dom.root();
        for child in dom.flat_children(root) {
            self.cascade_node(dom, child, &ComputedStyle::initial(), sheets, &mut map);
        }
        map
    }

    // `self` (a unit struct) threads through the recursion for call-site symmetry
    // with the public `cascade`; not a real parameter smell.
    #[allow(clippy::only_used_in_recursion)]
    fn cascade_node(
        &self,
        dom: &Dom,
        node: NodeId,
        parent_style: &ComputedStyle,
        sheets: &[ScopedSheet],
        map: &mut StyleMap,
    ) {
        let style = match dom.data(node) {
            NodeData::Element(el) => {
                let mut s = ComputedStyle::inherit_from(parent_style);
                apply_ua_defaults(&mut s, el);

                // Author rules, ordered by (specificity, source order).
                let mut matched: Vec<(u32, usize, &Declaration)> = Vec::new();
                let mut order = 0usize;
                for scoped in sheets {
                    for rule in &scoped.sheet.rules {
                        if let Some(spec) = rule
                            .selectors
                            .iter()
                            .filter(|sel| selector_matches_scoped(sel, dom, node, scoped.scope))
                            .map(|sel| sel.specificity())
                            .max()
                        {
                            for d in &rule.declarations {
                                matched.push((spec, order, d));
                                order += 1;
                            }
                        }
                    }
                }
                // Inline style has the highest weight.
                let inline = el.attr("style").map(parse_declarations).unwrap_or_default();

                matched.sort_by_key(|(spec, ord, _)| (*spec, *ord));
                let parent_fs = parent_style.font_size;
                for (_, _, d) in &matched {
                    apply_declaration(&mut s, d, parent_fs);
                }
                for d in &inline {
                    apply_declaration(&mut s, d, parent_fs);
                }
                // !important pass (author important beats normal), applied last.
                for (_, _, d) in matched.iter().filter(|(_, _, d)| d.important) {
                    apply_declaration(&mut s, d, parent_fs);
                }
                s
            }
            // Text/comment/doctype inherit their parent's computed style.
            _ => ComputedStyle::inherit_from(parent_style),
        };

        map.insert(node, style.clone());
        // Recurse over the FLAT tree: shadow content is styled, slotted light-DOM nodes
        // are visited once (through their slot), and unslotted light children are skipped
        // because they do not render.
        for child in dom.flat_children(node) {
            self.cascade_node(dom, child, &style, sheets, map);
        }
    }
}

/// The user-agent default stylesheet, reduced to what the layout slice needs:
/// which elements are block vs inline vs display:none, and their default margins.
fn apply_ua_defaults(s: &mut ComputedStyle, el: &ElementData) {
    use Display::*;
    let tag = el.name.as_str();
    let (display, top_bottom_em, weight, scale): (Display, f32, u16, f32) = match tag {
        "html" | "body" | "div" | "section" | "article" | "header" | "footer" | "nav" | "main"
        | "aside" | "figure" | "figcaption" | "address" => (Block, 0.0, 400, 1.0),
        "p" | "blockquote" => (Block, 1.0, 400, 1.0),
        "h1" => (Block, 0.67, 700, 2.0),
        "h2" => (Block, 0.75, 700, 1.5),
        "h3" => (Block, 0.83, 700, 1.17),
        "h4" => (Block, 1.12, 700, 1.0),
        "h5" => (Block, 1.5, 700, 0.83),
        "h6" => (Block, 1.67, 700, 0.75),
        "ul" | "ol" => (Block, 1.0, 400, 1.0),
        "li" | "dd" | "dt" => (Block, 0.0, 400, 1.0),
        "pre" => (Block, 1.0, 400, 1.0),
        "hr" => (Block, 0.5, 400, 1.0),
        "b" | "strong" => (Inline, 0.0, 700, 1.0),
        "table" => (Table, 0.0, 400, 1.0),
        "thead" | "tbody" | "tfoot" => (TableRowGroup, 0.0, 400, 1.0),
        "tr" => (TableRow, 0.0, 400, 1.0),
        "td" => (TableCell, 0.0, 400, 1.0),
        "th" => (TableCell, 0.0, 700, 1.0),
        "caption" => (TableCaption, 0.0, 400, 1.0),
        "colgroup" => (TableColumnGroup, 0.0, 400, 1.0),
        "col" => (TableColumn, 0.0, 400, 1.0),
        "head" | "title" | "meta" | "link" | "script" | "style" | "base" | "noscript" => {
            (None, 0.0, 400, 1.0)
        }
        // Form controls render as replaced-ish inline-block boxes (styled below).
        "input" | "button" | "textarea" | "select" => (InlineBlock, 0.0, 400, 1.0),
        // Default for unknown/other elements is inline (per CSS).
        _ => (Inline, 0.0, 400, 1.0),
    };
    s.display = display;
    // Form-control default appearance (UA stylesheet): a bordered, padded box. A text input
    // gets a default width; buttons hug their label. This is what makes fields visible.
    if matches!(tag, "input" | "button" | "textarea" | "select") {
        s.border_width = Sides::all(1.0);
        s.border_color = Rgba::new(118, 118, 118, 255);
        s.padding = Sides {
            top: Dim::Px(2.0),
            bottom: Dim::Px(3.0),
            left: Dim::Px(6.0),
            right: Dim::Px(6.0),
        };
        s.box_sizing = BoxSizing::BorderBox;
        if matches!(tag, "button") {
            s.background_color = Some(Rgba::new(239, 239, 239, 255));
            s.padding.left = Dim::Px(10.0);
            s.padding.right = Dim::Px(10.0);
        } else {
            s.background_color = Some(Rgba::WHITE);
        }
        if tag == "textarea" {
            s.width = Dim::Px(180.0);
            s.height = Dim::Px(48.0);
        }
        if tag == "input" {
            match el.attr("type").unwrap_or("text").to_ascii_lowercase().as_str() {
                // Button-like inputs hug their label (like <button>).
                "submit" | "reset" | "button" | "file" => {
                    s.background_color = Some(Rgba::new(239, 239, 239, 255));
                    s.padding.left = Dim::Px(10.0);
                    s.padding.right = Dim::Px(10.0);
                }
                // Checkbox / radio: a small square. A checked one is filled so its state is
                // visible (a full round/check mark needs border-radius/glyph rendering).
                "checkbox" | "radio" => {
                    s.width = Dim::Px(13.0);
                    s.height = Dim::Px(13.0);
                    s.padding = Sides::all(Dim::Px(0.0));
                    if el.attr("checked").is_some() {
                        s.background_color = Some(Rgba::new(60, 110, 220, 255));
                    }
                }
                "hidden" => s.display = None,
                // Text-like inputs get a default field width.
                _ => s.width = Dim::Px(180.0),
            }
        }
    }
    if weight != 400 {
        s.font_weight = weight;
    }
    if scale != 1.0 {
        s.font_size *= scale;
        s.line_height = s.font_size * 1.2;
    }
    if tag == "body" {
        s.margin = Sides::all(Dim::Px(8.0));
    } else if top_bottom_em != 0.0 {
        let m = Dim::Px(top_bottom_em * s.font_size);
        s.margin.top = m;
        s.margin.bottom = m;
    }
    if tag == "pre" {
        s.white_space = WhiteSpace::Pre;
    }
    // UA default: monospace for the code/teletype families.
    if matches!(tag, "pre" | "code" | "kbd" | "samp" | "tt" | "var") {
        s.font_family = vec!["monospace".to_string()];
    }
    if matches!(tag, "ul" | "ol") {
        s.padding.left = Dim::Px(40.0);
    }
    // UA default: table cells have 1px padding (Chrome/Firefox), which affects row heights.
    if matches!(tag, "td" | "th") {
        s.padding = Sides::all(Dim::Px(1.0));
    }
    // Replaced elements: an <img>/<canvas>/<video> is an atomic inline-block box sized by
    // its presentational width/height attributes (author CSS width/height still overrides,
    // as those are applied after UA defaults). Natural (intrinsic) sizing from the decoded
    // bitmap is layered on in the image pipeline.
    if matches!(tag, "img" | "canvas" | "video" | "svg" | "object" | "embed") {
        s.display = Display::InlineBlock;
        if let Some(w) = el.attr("width").and_then(parse_dimension_attr) {
            s.width = Dim::Px(w);
        }
        if let Some(h) = el.attr("height").and_then(parse_dimension_attr) {
            s.height = Dim::Px(h);
        }
    }
}

/// Parse an HTML presentational length attribute (`width="272"` or `width="272px"`) into
/// pixels. Percentages and other units are ignored (returns `None`).
fn parse_dimension_attr(v: &str) -> Option<f32> {
    let v = v.trim().trim_end_matches("px").trim();
    let n: f32 = v.parse().ok()?;
    if n.is_finite() && n >= 0.0 {
        Some(n)
    } else {
        None
    }
}

/// Apply one declaration onto a computed style. Unknown properties/values are
/// silently ignored (CSS error recovery). `parent_fs` resolves `em`/`%` fonts.
fn apply_declaration(s: &mut ComputedStyle, d: &Declaration, parent_fs: f32) {
    let v = d.value.trim();
    match d.name.as_str() {
        "display" => {
            s.display = match v {
                "block" => Display::Block,
                "inline" => Display::Inline,
                "inline-block" => Display::InlineBlock,
                "flex" => Display::Flex,
                "grid" => Display::Grid,
                "table" | "inline-table" => Display::Table,
                "table-row-group" | "table-header-group" | "table-footer-group" => {
                    Display::TableRowGroup
                }
                "table-row" => Display::TableRow,
                "table-cell" => Display::TableCell,
                "table-caption" => Display::TableCaption,
                "table-column" => Display::TableColumn,
                "table-column-group" => Display::TableColumnGroup,
                "none" => Display::None,
                _ => s.display,
            }
        }
        "color" => {
            if let Some(c) = values::parse_color(v) {
                s.color = c;
            }
        }
        "background-color" | "background" => {
            if let Some(c) = values::parse_color(v) {
                s.background_color = Some(c);
            }
        }
        "font-size" => {
            s.font_size = values::resolve_font_size(v, parent_fs).unwrap_or(s.font_size);
            s.line_height = s.font_size * 1.2;
        }
        "font-weight" => {
            s.font_weight = match v {
                "bold" | "bolder" => 700,
                "normal" => 400,
                "lighter" => 300,
                n => n.parse().unwrap_or(s.font_weight),
            }
        }
        "font-style" => s.italic = v == "italic" || v == "oblique",
        "font-family" => {
            let list = parse_font_family(v);
            if !list.is_empty() {
                s.font_family = list;
            }
        }
        "line-height" => {
            if let Ok(n) = v.parse::<f32>() {
                s.line_height = n * s.font_size; // unitless multiplier
            } else if let Some(px) = values::parse_length_px(v, s.font_size) {
                s.line_height = px;
            } else if v == "normal" {
                s.line_height = s.font_size * 1.2;
            }
        }
        "text-align" => {
            s.text_align = match v {
                "center" => TextAlign::Center,
                "right" => TextAlign::Right,
                "justify" => TextAlign::Justify,
                _ => TextAlign::Left,
            }
        }
        "white-space" => {
            s.white_space = match v {
                "nowrap" => WhiteSpace::NoWrap,
                "pre" | "pre-wrap" | "pre-line" => WhiteSpace::Pre,
                _ => WhiteSpace::Normal,
            }
        }
        "width" => s.width = values::parse_dim(v, s.font_size),
        "height" => s.height = values::parse_dim(v, s.font_size),
        "min-width" => s.min_width = values::parse_dim(v, s.font_size),
        "max-width" => s.max_width = values::parse_dim(v, s.font_size),
        "min-height" => s.min_height = values::parse_dim(v, s.font_size),
        "max-height" => s.max_height = values::parse_dim(v, s.font_size),
        "margin" => set_shorthand(&mut s.margin, v, s.font_size, true),
        "margin-top" => s.margin.top = values::parse_dim(v, s.font_size),
        "margin-right" => s.margin.right = values::parse_dim(v, s.font_size),
        "margin-bottom" => s.margin.bottom = values::parse_dim(v, s.font_size),
        "margin-left" => s.margin.left = values::parse_dim(v, s.font_size),
        "padding" => set_shorthand(&mut s.padding, v, s.font_size, false),
        "padding-top" => s.padding.top = values::parse_dim(v, s.font_size),
        "padding-right" => s.padding.right = values::parse_dim(v, s.font_size),
        "padding-bottom" => s.padding.bottom = values::parse_dim(v, s.font_size),
        "padding-left" => s.padding.left = values::parse_dim(v, s.font_size),
        "float" => {
            s.float = match v {
                "left" => Float::Left,
                "right" => Float::Right,
                _ => Float::None,
            }
        }
        "clear" => {
            s.clear = match v {
                "left" => Clear::Left,
                "right" => Clear::Right,
                "both" => Clear::Both,
                _ => Clear::None,
            }
        }
        "position" => {
            s.position = match v {
                "relative" => Position::Relative,
                "absolute" => Position::Absolute,
                "fixed" => Position::Fixed,
                "sticky" => Position::Sticky,
                _ => Position::Static,
            }
        }
        "top" => s.inset.top = values::parse_dim(v, s.font_size),
        "right" => s.inset.right = values::parse_dim(v, s.font_size),
        "bottom" => s.inset.bottom = values::parse_dim(v, s.font_size),
        "left" => s.inset.left = values::parse_dim(v, s.font_size),
        "z-index" => s.z_index = if v == "auto" { None } else { v.parse().ok() },
        // overflow shorthand + longhands: we clip the box for any non-visible value, and
        // take the more-clipping of x/y (a single clip rect, no independent-axis scroll).
        "overflow" | "overflow-x" | "overflow-y" => {
            let o = match v.split_whitespace().next().unwrap_or("visible") {
                "hidden" => Overflow::Hidden,
                "scroll" => Overflow::Scroll,
                "auto" => Overflow::Auto,
                "clip" => Overflow::Clip,
                _ => Overflow::Visible,
            };
            if o != Overflow::Visible {
                s.overflow = o;
            }
        }
        "table-layout" => {
            s.table_layout = match v {
                "fixed" => TableLayout::Fixed,
                _ => TableLayout::Auto,
            }
        }
        "border-collapse" => s.border_collapse = v.trim() == "collapse",
        "border-spacing" => {
            // Only the first (horizontal) length is used in this slice.
            if let Some(px) = v
                .split_whitespace()
                .next()
                .and_then(|t| values::parse_length_px(t, s.font_size))
            {
                s.border_spacing = px;
            }
        }
        "box-sizing" => {
            s.box_sizing = if v.trim() == "border-box" {
                BoxSizing::BorderBox
            } else {
                BoxSizing::ContentBox
            };
        }
        "justify-content" => {
            s.justify_content = match v.trim() {
                "center" => JustifyContent::Center,
                "flex-end" | "end" | "right" => JustifyContent::FlexEnd,
                "space-between" => JustifyContent::SpaceBetween,
                "space-around" => JustifyContent::SpaceAround,
                "space-evenly" => JustifyContent::SpaceEvenly,
                _ => JustifyContent::FlexStart,
            };
        }
        "align-items" => {
            s.align_items = match v.trim() {
                "center" => AlignItems::Center,
                "flex-end" | "end" => AlignItems::FlexEnd,
                "flex-start" | "start" => AlignItems::FlexStart,
                "baseline" => AlignItems::Baseline,
                _ => AlignItems::Stretch,
            };
        }
        "flex-direction" => {
            s.flex_direction = match v.trim() {
                "column" => FlexDirection::Column,
                "column-reverse" => FlexDirection::ColumnReverse,
                "row-reverse" => FlexDirection::RowReverse,
                _ => FlexDirection::Row,
            };
        }
        "flex-wrap" => {
            s.flex_wrap = match v.trim() {
                "wrap" => FlexWrap::Wrap,
                "wrap-reverse" => FlexWrap::WrapReverse,
                _ => FlexWrap::NoWrap,
            };
        }
        "gap" => {
            // `gap: <row> [<column>]`.
            let parts: Vec<f32> = v
                .split_whitespace()
                .filter_map(|t| values::parse_length_px(t, s.font_size))
                .collect();
            match parts.as_slice() {
                [r] => {
                    s.row_gap = *r;
                    s.column_gap = *r;
                }
                [r, c] => {
                    s.row_gap = *r;
                    s.column_gap = *c;
                }
                _ => {}
            }
        }
        "row-gap" => {
            if let Some(px) = values::parse_length_px(v.trim(), s.font_size) {
                s.row_gap = px;
            }
        }
        "column-gap" => {
            if let Some(px) = values::parse_length_px(v.trim(), s.font_size) {
                s.column_gap = px;
            }
        }
        "align-self" => {
            s.align_self = match v.trim() {
                "auto" => None,
                "center" => Some(AlignItems::Center),
                "flex-end" | "end" => Some(AlignItems::FlexEnd),
                "flex-start" | "start" => Some(AlignItems::FlexStart),
                "baseline" => Some(AlignItems::Baseline),
                "stretch" => Some(AlignItems::Stretch),
                _ => None,
            };
        }
        "flex-grow" => s.flex_grow = v.trim().parse().unwrap_or(0.0),
        "flex-shrink" => s.flex_shrink = v.trim().parse().unwrap_or(1.0),
        "flex-basis" => s.flex_basis = values::parse_dim(v, s.font_size),
        "flex" => parse_flex_shorthand(s, v),
        "order" => {} // parsed but not yet used in layout
        "grid-template-columns" => s.grid_template_columns = parse_track_list(v, s.font_size),
        "grid-template-rows" => s.grid_template_rows = parse_track_list(v, s.font_size),
        "grid-column" => s.grid_column = parse_grid_line_shorthand(v),
        "grid-row" => s.grid_row = parse_grid_line_shorthand(v),
        "grid-column-start" => s.grid_column.0 = parse_grid_line(v),
        "grid-column-end" => s.grid_column.1 = parse_grid_line(v),
        "grid-row-start" => s.grid_row.0 = parse_grid_line(v),
        "grid-row-end" => s.grid_row.1 = parse_grid_line(v),
        "transform" => s.transform = parse_transform(v, s.font_size),
        "vertical-align" => {
            s.vertical_align = match v.trim() {
                "top" => VerticalAlign::Top,
                "middle" => VerticalAlign::Middle,
                "bottom" => VerticalAlign::Bottom,
                "text-top" => VerticalAlign::TextTop,
                "text-bottom" => VerticalAlign::TextBottom,
                "sub" => VerticalAlign::Sub,
                "super" => VerticalAlign::Super,
                _ => VerticalAlign::Baseline,
            };
        }
        // The `border` family. Widths feed the box model; the color feeds paint; the line
        // style is not tracked (only presence, since `none`/`hidden` zero the width).
        "border" => {
            let (w, c) = parse_border_shorthand(v, s.font_size);
            if let Some(w) = w {
                s.border_width = Sides::all(w);
            }
            if let Some(c) = c {
                s.border_color = c;
            }
        }
        "border-top" | "border-right" | "border-bottom" | "border-left" => {
            let (w, c) = parse_border_shorthand(v, s.font_size);
            if let Some(w) = w {
                match d.name.as_str() {
                    "border-top" => s.border_width.top = w,
                    "border-right" => s.border_width.right = w,
                    "border-bottom" => s.border_width.bottom = w,
                    _ => s.border_width.left = w,
                }
            }
            if let Some(c) = c {
                s.border_color = c;
            }
        }
        "border-radius" => {
            // MVP: a single uniform radius. `border-radius: 8px` / `8px 8px` → take the first
            // length (per-corner + elliptical `/` radii are a follow-on).
            if let Some(first) = v.split_whitespace().next() {
                if let Dim::Px(px) = values::parse_dim(first, s.font_size) {
                    s.border_radius = px.max(0.0);
                }
            }
        }
        "box-shadow" => s.box_shadow = parse_box_shadow(v, s.font_size),
        "border-width" => set_border_widths(&mut s.border_width, v, s.font_size),
        "border-top-width" => s.border_width.top = border_len(v, s.font_size),
        "border-right-width" => s.border_width.right = border_len(v, s.font_size),
        "border-bottom-width" => s.border_width.bottom = border_len(v, s.font_size),
        "border-left-width" => s.border_width.left = border_len(v, s.font_size),
        "border-color" => {
            if let Some(c) = values::parse_color(v) {
                s.border_color = c;
            }
        }
        "border-style" => {
            // `none`/`hidden` remove the border; other styles keep whatever width is set.
            if matches!(v.trim(), "none" | "hidden") {
                s.border_width = Sides::all(0.0);
            }
        }
        _ => {}
    }
}

/// A `border-width` keyword or length to px. `thin`/`medium`/`thick` per CSS2 §8.
fn border_len(tok: &str, fs: f32) -> f32 {
    match tok.trim() {
        "thin" => 1.0,
        "medium" => 3.0,
        "thick" => 5.0,
        t => values::parse_length_px(t, fs).unwrap_or(0.0),
    }
}

/// Resolve a `font-family` list to a generic family we can render. Walks the prioritized
/// list and returns the first token we recognize — a generic keyword, or a well-known
/// named family mapped to its generic (so `"Courier New"` → monospace, `Georgia` → serif).
/// Named families we don't know are skipped (we can't load them), falling through to the
/// next candidate; `None` if nothing is recognized (caller keeps the inherited family).
/// Parse a `font-family` value into the priority list of family names (lowercased,
/// dequoted). Generic keywords are kept literally (e.g. `"sans-serif"`); named families
/// are preserved so the text layer can resolve them to installed / `@font-face` faces.
fn parse_font_family(v: &str) -> Vec<String> {
    v.split(',')
        .map(|raw| raw.trim().trim_matches(['"', '\'']).to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Parse the `border`/`border-<side>` shorthand into an optional width and color. The line
/// style is consumed but not stored; `none`/`hidden` force width 0.
fn parse_border_shorthand(v: &str, fs: f32) -> (Option<f32>, Option<Rgba>) {
    let mut width = None;
    let mut color = None;
    let mut saw_visible_style = false;
    for tok in v.split_whitespace() {
        match tok {
            "none" | "hidden" => width = Some(0.0),
            "solid" | "dashed" | "dotted" | "double" | "groove" | "ridge" | "inset"
            | "outset" => saw_visible_style = true,
            "thin" => width = Some(1.0),
            "medium" => width = Some(3.0),
            "thick" => width = Some(5.0),
            t => {
                if let Some(px) = values::parse_length_px(t, fs) {
                    width = Some(px);
                } else if let Some(c) = values::parse_color(t) {
                    color = Some(c);
                }
            }
        }
    }
    // A visible line style with no explicit width defaults to `medium` (3px).
    if width.is_none() && saw_visible_style {
        width = Some(3.0);
    }
    (width, color)
}

/// Split `v` on top-level whitespace, keeping parenthesised groups (`rgba(0, 0, 0, .3)`) intact.
fn tokens_keeping_parens(v: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0i32;
    for c in v.chars() {
        match c {
            '(' => {
                depth += 1;
                cur.push(c);
            }
            ')' => {
                depth -= 1;
                cur.push(c);
            }
            c if c.is_whitespace() && depth == 0 => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Parse `box-shadow: <offset-x> <offset-y> [<blur>] [<color>]` — the first **outer** shadow.
/// `none`, `inset`, and shadows past the first comma are not painted (documented follow-ons), and
/// yield `None` rather than a wrong shadow.
fn parse_box_shadow(v: &str, fs: f32) -> Option<BoxShadow> {
    let v = v.trim();
    if v.is_empty() || v.eq_ignore_ascii_case("none") {
        return None;
    }
    // Only the first shadow: split on the first *top-level* comma (commas inside rgba() don't count).
    let mut depth = 0i32;
    let mut end = v.len();
    for (i, c) in v.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                end = i;
                break;
            }
            _ => {}
        }
    }
    let first = &v[..end];
    if first.split_whitespace().any(|t| t.eq_ignore_ascii_case("inset")) {
        return None; // inset shadows aren't painted yet
    }

    let mut lens: Vec<f32> = Vec::new();
    let mut color: Option<Rgba> = None;
    for tok in tokens_keeping_parens(first) {
        if let Some(px) = values::parse_length_px(&tok, fs) {
            lens.push(px);
        } else if let Some(c) = values::parse_color(&tok) {
            color = Some(c);
        }
    }
    // offset-x and offset-y are required.
    if lens.len() < 2 {
        return None;
    }
    Some(BoxShadow {
        dx: lens[0],
        dy: lens[1],
        blur: lens.get(2).copied().unwrap_or(0.0).max(0.0),
        color: color.unwrap_or(Rgba::BLACK),
    })
}

/// Parse a `transform` value into an ordered list of [`TransformFn`]s (translate/scale/
/// rotate/skew/matrix, and the axis variants). Unknown functions are skipped.
fn parse_transform(v: &str, fs: f32) -> Vec<TransformFn> {
    let mut out = Vec::new();
    let mut rest = v.trim();
    while let Some(open) = rest.find('(') {
        let name = rest[..open].trim().to_ascii_lowercase();
        let Some(close) = rest[open..].find(')') else { break };
        let args_str = &rest[open + 1..open + close];
        let nums: Vec<&str> = args_str.split(',').map(str::trim).filter(|s| !s.is_empty()).collect();
        let angle = |s: &str| parse_angle_rad(s);
        let f = |i: usize| nums.get(i).and_then(|s| s.parse::<f32>().ok());
        let dim = |i: usize| nums.get(i).map(|s| values::parse_dim(s, fs)).unwrap_or(Dim::Px(0.0));
        match name.as_str() {
            "translate" => out.push(TransformFn::Translate(dim(0), nums.get(1).map(|s| values::parse_dim(s, fs)).unwrap_or(Dim::Px(0.0)))),
            "translatex" => out.push(TransformFn::Translate(dim(0), Dim::Px(0.0))),
            "translatey" => out.push(TransformFn::Translate(Dim::Px(0.0), dim(0))),
            "scale" => out.push(TransformFn::Scale(f(0).unwrap_or(1.0), f(1).or(f(0)).unwrap_or(1.0))),
            "scalex" => out.push(TransformFn::Scale(f(0).unwrap_or(1.0), 1.0)),
            "scaley" => out.push(TransformFn::Scale(1.0, f(0).unwrap_or(1.0))),
            "rotate" => out.push(TransformFn::Rotate(nums.first().and_then(|s| angle(s)).unwrap_or(0.0))),
            "skew" => out.push(TransformFn::Skew(
                nums.first().and_then(|s| angle(s)).unwrap_or(0.0),
                nums.get(1).and_then(|s| angle(s)).unwrap_or(0.0),
            )),
            "skewx" => out.push(TransformFn::Skew(nums.first().and_then(|s| angle(s)).unwrap_or(0.0), 0.0)),
            "skewy" => out.push(TransformFn::Skew(0.0, nums.first().and_then(|s| angle(s)).unwrap_or(0.0))),
            "matrix" => {
                if nums.len() == 6 {
                    let mut m = [0.0f32; 6];
                    let mut ok = true;
                    for (k, n) in nums.iter().enumerate() {
                        match n.parse::<f32>() {
                            Ok(val) => m[k] = val,
                            Err(_) => ok = false,
                        }
                    }
                    if ok {
                        out.push(TransformFn::Matrix(m));
                    }
                }
            }
            _ => {}
        }
        rest = &rest[open + close + 1..];
    }
    out
}

/// Parse an `<angle>` (`deg`/`rad`/`grad`/`turn`, default deg) to radians.
fn parse_angle_rad(s: &str) -> Option<f32> {
    let s = s.trim();
    let (num, unit) = s.find(|c: char| c.is_ascii_alphabetic()).map_or((s, ""), |i| s.split_at(i));
    let n: f32 = num.trim().parse().ok()?;
    Some(match unit.to_ascii_lowercase().as_str() {
        "rad" => n,
        "grad" => n * std::f32::consts::PI / 200.0,
        "turn" => n * std::f32::consts::TAU,
        _ => n * std::f32::consts::PI / 180.0, // deg (default)
    })
}

/// Parse a `grid-template-columns`/`-rows` track list, expanding a single-track
/// `repeat(N, <track>)`. Line names and `minmax()` are not modeled.
fn parse_track_list(v: &str, fs: f32) -> Vec<TrackSize> {
    split_tracks_top_level(&expand_grid_repeat(v))
        .into_iter()
        .filter_map(|t| parse_track(&t, fs))
        .collect()
}

/// Split a track list on whitespace, keeping parenthesized groups (`minmax(a, b)`) intact.
fn split_tracks_top_level(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0i32;
    for ch in s.chars() {
        match ch {
            '(' => {
                depth += 1;
                cur.push(ch);
            }
            ')' => {
                depth -= 1;
                cur.push(ch);
            }
            c if c.is_whitespace() && depth == 0 => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn parse_track(t: &str, fs: f32) -> Option<TrackSize> {
    let t = t.trim();
    let low = t.to_ascii_lowercase();
    if low == "auto" {
        return Some(TrackSize::Auto);
    }
    if low == "min-content" {
        return Some(TrackSize::MinContent);
    }
    if low == "max-content" {
        return Some(TrackSize::MaxContent);
    }
    if let Some(inner) = low.strip_prefix("minmax(").and_then(|s| s.strip_suffix(')')) {
        let (a, b) = inner.split_once(',')?;
        return Some(TrackSize::MinMax(parse_track_unit(a.trim(), fs)?, parse_track_unit(b.trim(), fs)?));
    }
    if let Some(n) = t.strip_suffix("fr").and_then(|n| n.trim().parse::<f32>().ok()) {
        return Some(TrackSize::Fr(n));
    }
    if let Some(p) = t.strip_suffix('%').and_then(|n| n.trim().parse::<f32>().ok()) {
        return Some(TrackSize::Percent(p));
    }
    values::parse_length_px(t, fs).map(TrackSize::Px)
}

fn parse_track_unit(t: &str, fs: f32) -> Option<TrackUnit> {
    let low = t.to_ascii_lowercase();
    match low.as_str() {
        "auto" => Some(TrackUnit::Auto),
        "min-content" => Some(TrackUnit::MinContent),
        "max-content" => Some(TrackUnit::MaxContent),
        _ => {
            if let Some(n) = t.strip_suffix("fr").and_then(|n| n.trim().parse::<f32>().ok()) {
                Some(TrackUnit::Fr(n))
            } else if let Some(p) = t.strip_suffix('%').and_then(|n| n.trim().parse::<f32>().ok()) {
                Some(TrackUnit::Percent(p))
            } else {
                values::parse_length_px(t, fs).map(TrackUnit::Px)
            }
        }
    }
}

/// Parse a `grid-column`/`grid-row` shorthand (`<start> [/ <end>]`).
fn parse_grid_line_shorthand(v: &str) -> (GridLine, GridLine) {
    match v.split_once('/') {
        Some((a, b)) => (parse_grid_line(a), parse_grid_line(b)),
        None => (parse_grid_line(v), GridLine::Auto),
    }
}

/// Parse one grid line: `auto`, a line number, or `span N`.
fn parse_grid_line(v: &str) -> GridLine {
    let v = v.trim();
    if v.eq_ignore_ascii_case("auto") || v.is_empty() {
        return GridLine::Auto;
    }
    if let Some(n) = v.strip_prefix("span").map(str::trim).and_then(|n| n.parse::<u16>().ok()) {
        return GridLine::Span(n.max(1));
    }
    v.parse::<i16>().map(GridLine::Line).unwrap_or(GridLine::Auto)
}

/// Expand `repeat(N, <single-track>)` occurrences into N copies of the track.
fn expand_grid_repeat(v: &str) -> String {
    let mut out = String::new();
    let mut rest = v;
    while let Some(idx) = rest.to_ascii_lowercase().find("repeat(") {
        out.push_str(&rest[..idx]);
        let after = &rest[idx + 7..];
        let Some(end) = after.find(')') else { break };
        if let Some((n, track)) = after[..end].split_once(',') {
            if let Ok(count) = n.trim().parse::<usize>() {
                for i in 0..count {
                    if i > 0 || !out.ends_with(' ') {
                        out.push(' ');
                    }
                    out.push_str(track.trim());
                }
            }
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

/// Parse the `flex` shorthand (`flex: <grow> <shrink>? <basis>?`, plus the `none`/`auto`/
/// `initial` keywords). A bare number is grow (then shrink); a length/percent/`auto` is basis.
/// A single number defaults basis to `0` (the common `flex: 1` case), matching CSS.
fn parse_flex_shorthand(s: &mut ComputedStyle, v: &str) {
    match v.trim() {
        "none" => {
            s.flex_grow = 0.0;
            s.flex_shrink = 0.0;
            s.flex_basis = Dim::Auto;
            return;
        }
        "auto" => {
            s.flex_grow = 1.0;
            s.flex_shrink = 1.0;
            s.flex_basis = Dim::Auto;
            return;
        }
        "initial" => {
            s.flex_grow = 0.0;
            s.flex_shrink = 1.0;
            s.flex_basis = Dim::Auto;
            return;
        }
        _ => {}
    }
    let mut nums = Vec::new();
    let mut basis = None;
    for t in v.split_whitespace() {
        if let Ok(n) = t.parse::<f32>() {
            nums.push(n);
        } else {
            basis = Some(values::parse_dim(t, s.font_size));
        }
    }
    match nums.as_slice() {
        [g] => {
            s.flex_grow = *g;
            s.flex_shrink = 1.0;
        }
        [g, sh] => {
            s.flex_grow = *g;
            s.flex_shrink = *sh;
        }
        _ => {}
    }
    // An explicit basis wins; otherwise a numeric `flex` sets basis 0 (not auto).
    s.flex_basis = basis.unwrap_or(if nums.is_empty() { Dim::Auto } else { Dim::Px(0.0) });
}

/// Expand a 1–4 value `border-width` shorthand (same edge order as `margin`).
fn set_border_widths(sides: &mut Sides<f32>, v: &str, fs: f32) {
    let vals: Vec<f32> = v.split_whitespace().map(|t| border_len(t, fs)).collect();
    match vals.as_slice() {
        [a] => *sides = Sides::all(*a),
        [a, b] => {
            *sides = Sides { top: *a, bottom: *a, right: *b, left: *b };
        }
        [a, b, c] => {
            *sides = Sides { top: *a, right: *b, left: *b, bottom: *c };
        }
        [a, b, c, d] => {
            *sides = Sides { top: *a, right: *b, bottom: *c, left: *d };
        }
        _ => {}
    }
}

/// Expand a 1–4 value `margin`/`padding` shorthand.
fn set_shorthand(sides: &mut Sides<Dim>, v: &str, fs: f32, allow_auto: bool) {
    let vals: Vec<Dim> = v
        .split_whitespace()
        .map(|t| {
            let d = values::parse_dim(t, fs);
            if !allow_auto && d.is_auto() {
                Dim::Px(0.0)
            } else {
                d
            }
        })
        .collect();
    match vals.as_slice() {
        [a] => *sides = Sides::all(*a),
        [a, b] => {
            *sides = Sides {
                top: *a,
                bottom: *a,
                right: *b,
                left: *b,
            }
        }
        [a, b, c] => {
            *sides = Sides {
                top: *a,
                right: *b,
                left: *b,
                bottom: *c,
            }
        }
        [a, b, c, d, ..] => {
            *sides = Sides {
                top: *a,
                right: *b,
                bottom: *c,
                left: *d,
            }
        }
        [] => {}
    }
}

#[cfg(feature = "stylo")]
pub mod stylo_engine;

/// D2 Step-0 probe: drive real Stylo (Device + parser + Stylist) end to end.
#[cfg(feature = "stylo")]
pub mod stylo_probe;

/// D2 impedance resolution: the per-element `AtomicRefCell<ElementData>` store + the
/// `(&Dom, NodeId)` handle the Stylo DOM trait wall attaches to.
#[cfg(feature = "stylo")]
pub mod stylo_dom;

/// D2 back-half: mapping Stylo's `ComputedValues` onto [`ComputedStyle`]. Scalar subset
/// landed + tested against Stylo's initial values; the geometric properties follow per
/// `docs/parity/STYLO-CASCADE-PLAN.md`.
#[cfg(feature = "stylo")]
pub mod stylo_map;

/// D2: the Stylo DOM trait wall (`TDocument`/`TNode`/`TShadowRoot`/`TElement`) that lets
/// the cascade name a `TElement` type; matching still uses the real `selectors::Element`.
#[cfg(feature = "stylo")]
pub mod stylo_traits;

#[cfg(test)]
mod tests {
    use super::*;

    fn build_dom() -> Dom {
        let mut dom = Dom::new();
        let body = dom.create_element("body");
        let p = dom.create_element("p");
        dom.set_attr(p, "class", "lead");
        let span = dom.create_element("span");
        dom.set_attr(span, "id", "x");
        let t = dom.create_text("hi");
        dom.append_child(dom.root(), body);
        dom.append_child(body, p);
        dom.append_child(p, span);
        dom.append_child(span, t);
        dom
    }

    fn styled(css: &str) -> (Dom, StyleMap) {
        let dom = build_dom();
        let sheets = vec![Stylesheet::parse(css)];
        let map = MinimalCascade.cascade(&dom, &sheets);
        (dom, map)
    }

    #[test]
    fn ua_defaults_and_inheritance() {
        let (dom, map) = styled("");
        let p = dom.find_first("p").unwrap();
        assert_eq!(map[&p].display, Display::Block);
        assert_eq!(map[&p].color, Rgba::BLACK);
        // p default margins are 1em = 16px top/bottom.
        assert_eq!(map[&p].margin.top, Dim::Px(16.0));
    }

    #[test]
    fn author_rules_cascade_by_specificity() {
        let css = "p { color: red } .lead { color: green } #x { color: blue }";
        let (dom, map) = styled(css);
        let p = dom.find_first("p").unwrap();
        let span = dom.find_first("span").unwrap();
        // .lead (0,1,0) beats p (0,0,1).
        assert_eq!(map[&p].color, Rgba::new(0, 128, 0, 255));
        // #x id selector wins on the span.
        assert_eq!(map[&span].color, Rgba::new(0, 0, 255, 255));
    }

    #[test]
    fn descendant_combinator() {
        let css = "body span { color: red }";
        let (dom, map) = styled(css);
        let span = dom.find_first("span").unwrap();
        assert_eq!(map[&span].color, Rgba::new(255, 0, 0, 255));
    }

    #[test]
    fn float_clear_position_insets_parse() {
        let (dom, map) = styled(
            "p { float: right; clear: both; position: absolute; top: 10px; left: 5%; z-index: 3 }",
        );
        let p = dom.find_first("p").unwrap();
        let s = &map[&p];
        assert_eq!(s.float, Float::Right);
        assert_eq!(s.clear, Clear::Both);
        assert_eq!(s.position, Position::Absolute);
        assert_eq!(s.inset.top, Dim::Px(10.0));
        assert_eq!(s.inset.left, Dim::Percent(5.0));
        assert_eq!(s.inset.right, Dim::Auto); // unset stays auto
        assert_eq!(s.z_index, Some(3));
    }

    #[test]
    fn restyle_damage_classifies_changes() {
        let base = ComputedStyle::initial();

        // Identical → None.
        assert_eq!(diff_style(&base, &base.clone()), RestyleDamage::None);

        // color-only → Repaint.
        let mut paint = base.clone();
        paint.color = Rgba::new(1, 2, 3, 255);
        assert_eq!(diff_style(&base, &paint), RestyleDamage::Repaint);

        // width change → Reflow.
        let mut reflow = base.clone();
        reflow.width = Dim::Px(100.0);
        assert_eq!(diff_style(&base, &reflow), RestyleDamage::Reflow);

        // display change → Rebuild (and it dominates a simultaneous color change).
        let mut rebuild = base.clone();
        rebuild.display = Display::Flex;
        rebuild.color = Rgba::new(9, 9, 9, 255);
        assert_eq!(diff_style(&base, &rebuild), RestyleDamage::Rebuild);

        // Damage is ordered least→most expensive.
        assert!(RestyleDamage::None < RestyleDamage::Repaint);
        assert!(RestyleDamage::Repaint < RestyleDamage::Reflow);
        assert!(RestyleDamage::Reflow < RestyleDamage::Rebuild);
    }

    #[test]
    fn query_selector_reuses_the_cascade_engine() {
        // <body><p class=lead>…<span id=x></span></p></body> from build_dom().
        let dom = build_dom();
        let root = dom.root();
        let span = dom.find_first("span").unwrap();
        let p = dom.find_first("p").unwrap();
        assert_eq!(query_selector(&dom, root, "span"), Some(span));
        assert_eq!(query_selector(&dom, root, "#x"), Some(span));
        assert_eq!(query_selector(&dom, root, "body p"), Some(p));
        assert_eq!(query_selector(&dom, root, ".nope"), None);
        assert!(matches_selector(&dom, span, "span"));
        assert_eq!(query_selector_all(&dom, root, "span").len(), 1);
    }

    #[test]
    fn table_display_and_properties_parse() {
        let (dom, map) = styled("p { display: table; table-layout: fixed; border-spacing: 4px }");
        let p = dom.find_first("p").unwrap();
        let s = &map[&p];
        assert_eq!(s.display, Display::Table);
        assert_eq!(s.table_layout, TableLayout::Fixed);
        assert_eq!(s.border_spacing, 4.0);
    }

    #[test]
    fn table_ua_defaults() {
        // Build a tiny table DOM and confirm UA display defaults.
        let mut dom = Dom::new();
        let root = dom.root();
        let table = dom.create_element("table");
        let tr = dom.create_element("tr");
        let td = dom.create_element("td");
        let th = dom.create_element("th");
        dom.append_child(root, table);
        dom.append_child(table, tr);
        dom.append_child(tr, td);
        dom.append_child(tr, th);
        let map = MinimalCascade.cascade(&dom, &[]);
        assert_eq!(map[&table].display, Display::Table);
        assert_eq!(map[&tr].display, Display::TableRow);
        assert_eq!(map[&td].display, Display::TableCell);
        assert_eq!(map[&th].display, Display::TableCell);
        assert_eq!(map[&th].font_weight, 700, "th is bold by default");
    }

    #[test]
    fn inline_style_wins() {
        let mut dom = build_dom();
        let p = dom.find_first("p").unwrap();
        dom.set_attr(p, "style", "color: rgb(1,2,3); width: 50%");
        let map = MinimalCascade.cascade(&dom, &[Stylesheet::parse("p{color:red}")]);
        assert_eq!(map[&p].color, Rgba::new(1, 2, 3, 255));
        assert_eq!(map[&p].width, Dim::Percent(50.0));
    }
}

#[cfg(test)]
mod shadow_scoping_tests {
    use super::*;

    fn cascade_of(html: &str) -> (manuk_dom::Dom, StyleMap) {
        let dom = manuk_html::parse(html);
        let sheets = MinimalCascade::collect_style_elements(&dom);
        let map = MinimalCascade.cascade(&dom, &sheets);
        (dom, map)
    }

    /// N4's headline acceptance, direction 1: a **document** rule must not reach inside a
    /// shadow root. `p { color: red }` in the light DOM must not paint the shadow's `<p>`.
    #[test]
    fn a_document_rule_does_not_match_inside_a_shadow_root() {
        let (dom, map) = cascade_of(
            r#"<style>p { color: #ff0000 }</style>
               <div id="host"><template shadowrootmode="open"><p id="inner">shadow</p></template></div>
               <p id="outer">light</p>"#,
        );
        let outer = dom.find_first("p").expect("light-DOM p");
        assert_eq!(dom.element(outer).unwrap().attr("id"), Some("outer"));
        assert_eq!(map[&outer].color, Rgba::new(255, 0, 0, 255), "the light-DOM p is red");

        // The shadow <p> is a different <p>; find it through the shadow root.
        let host = dom.find_first("div").unwrap();
        let shadow = dom.shadow_root(host).unwrap();
        let inner = dom.descendants(shadow).find(|&n| dom.tag_name(n) == Some("p")).unwrap();
        assert_ne!(inner, outer);
        assert_ne!(
            map[&inner].color,
            Rgba::new(255, 0, 0, 255),
            "a document rule must NOT cross the shadow boundary"
        );
    }

    /// Direction 2: a rule **inside** a shadow root must not escape it.
    #[test]
    fn a_shadow_rule_does_not_match_a_light_dom_element() {
        let (dom, map) = cascade_of(
            r#"<div id="host">
                 <template shadowrootmode="open">
                   <style>p { color: #00ff00 }</style>
                   <p id="inner">shadow</p>
                 </template>
               </div>
               <p id="outer">light</p>"#,
        );
        let host = dom.find_first("div").unwrap();
        let shadow = dom.shadow_root(host).unwrap();
        let inner = dom.descendants(shadow).find(|&n| dom.tag_name(n) == Some("p")).unwrap();
        assert_eq!(map[&inner].color, Rgba::new(0, 255, 0, 255), "the shadow p is green");

        // The light-DOM <p> is the one that is NOT inside the shadow root.
        let outer = dom
            .descendants(dom.root())
            .find(|&n| dom.tag_name(n) == Some("p"))
            .unwrap();
        assert_ne!(outer, inner);
        assert_ne!(
            map[&outer].color,
            Rgba::new(0, 255, 0, 255),
            "a shadow rule must NOT escape the shadow boundary"
        );
    }

    /// `::slotted(p)` is the one selector that deliberately reaches across the boundary:
    /// from inside the shadow tree, it styles the **light-DOM** nodes slotted into it.
    #[test]
    fn slotted_matches_a_slotted_light_dom_element() {
        let (dom, map) = cascade_of(
            r#"<div id="host">
                 <template shadowrootmode="open">
                   <style>::slotted(p) { color: #0000ff }</style>
                   <slot></slot>
                 </template>
                 <p id="slotted">light</p>
                 <span id="also">span</span>
               </div>"#,
        );
        let p = dom.find_first("p").unwrap();
        assert_eq!(map[&p].color, Rgba::new(0, 0, 255, 255), "::slotted(p) styles the slotted p");

        // ...but not the slotted <span>: the compound must still match.
        let span = dom.find_first("span").unwrap();
        assert_ne!(map[&span].color, Rgba::new(0, 0, 255, 255));
    }

    /// `::slotted()` must not match an element that is not slotted at all, and a
    /// document-level `::slotted()` matches nothing.
    #[test]
    fn slotted_does_not_match_unslotted_or_document_elements() {
        let (dom, map) = cascade_of(
            r#"<style>::slotted(p) { color: #0000ff }</style>
               <p id="plain">nobody slots me</p>"#,
        );
        let p = dom.find_first("p").unwrap();
        assert_ne!(
            map[&p].color,
            Rgba::new(0, 0, 255, 255),
            "::slotted() outside a shadow tree matches nothing"
        );
    }

    /// An unmodelled pseudo-element must not silently match its subject — dropping the
    /// rule is right; applying it to the bare `p` is not.
    #[test]
    fn an_unmodelled_pseudo_element_selector_is_dropped_not_mismatched() {
        let (dom, map) = cascade_of(
            r#"<style>p::before { color: #ff0000 } p::first-line { color: #ff0000 }</style>
               <p>x</p>"#,
        );
        let p = dom.find_first("p").unwrap();
        assert_ne!(map[&p].color, Rgba::new(255, 0, 0, 255));
    }

    /// Shadow content is styled at all — it is reached through the flat tree, and it
    /// inherits from its flat-tree ancestors.
    #[test]
    fn shadow_content_is_styled_and_inherits_through_the_flat_tree() {
        let (dom, map) = cascade_of(
            r#"<style>#host { color: #123456 }</style>
               <div id="host"><template shadowrootmode="open"><em id="deep">x</em></template></div>"#,
        );
        let host = dom.find_first("div").unwrap();
        let shadow = dom.shadow_root(host).unwrap();
        let em = dom.descendants(shadow).find(|&n| dom.tag_name(n) == Some("em")).unwrap();
        // `color` inherits from the host across the shadow boundary (inheritance is
        // flat-tree, not scoped -- only *matching* is scoped).
        assert_eq!(map[&host].color, Rgba::new(0x12, 0x34, 0x56, 255));
        assert_eq!(map[&em].color, Rgba::new(0x12, 0x34, 0x56, 255));
    }

    #[test]
    fn border_shorthand_and_box_sizing_parse() {
        let (dom, map) = cascade_of(
            r#"<p style="border:5px solid #333;box-sizing:border-box"></p>"#,
        );
        let s = &map[&dom.find_first("p").unwrap()];
        assert_eq!(s.border_width, Sides::all(5.0), "border shorthand sets all widths");
        assert_eq!(s.border_color, Rgba::new(0x33, 0x33, 0x33, 255));
        assert_eq!(s.box_sizing, BoxSizing::BorderBox);

        // Per-side + keyword widths; a visible style with no length defaults to medium (3px).
        let (dom, map) = cascade_of(
            r#"<p style="border-width:1px 2px 3px 4px;border-left:dashed red;border-top-width:thick"></p>"#,
        );
        let s = &map[&dom.find_first("p").unwrap()];
        assert_eq!(s.border_width.right, 2.0);
        assert_eq!(s.border_width.bottom, 3.0);
        assert_eq!(s.border_width.left, 3.0, "border-left: dashed -> medium 3px");
        assert_eq!(s.border_width.top, 5.0, "border-top-width: thick -> 5px");

        // `border-style: none` zeroes the width set by an earlier `border`.
        let (dom, map) = cascade_of(r#"<p style="border:10px solid;border-style:none"></p>"#);
        assert_eq!(map[&dom.find_first("p").unwrap()].border_width, Sides::all(0.0));

        // Default box-sizing is content-box.
        let (dom, map) = cascade_of(r#"<p style="width:10px"></p>"#);
        assert_eq!(map[&dom.find_first("p").unwrap()].box_sizing, BoxSizing::ContentBox);
    }

    #[test]
    fn font_family_resolves_generics_named_and_ua() {
        // Generic keyword after an unavailable named font falls through to the generic.
        assert_eq!(parse_font_family("Arial, sans-serif"), vec!["arial", "sans-serif"]);
        assert_eq!(parse_font_family("Georgia, serif"), vec!["georgia", "serif"]);
        assert_eq!(parse_font_family("'Courier New', monospace"), vec!["courier new", "monospace"]);
        // Named families we know map to their generic even without a following keyword.
        // Named families are preserved (the text layer resolves them).
        assert_eq!(parse_font_family("Times New Roman"), vec!["times new roman"]);
        assert_eq!(parse_font_family("Menlo, monospace"), vec!["menlo", "monospace"]);

        // Cascade: an author family list applies and is inherited; UA gives <code> monospace.
        let (dom, map) = cascade_of(
            r#"<div style="font-family:'MyFont', monospace">a<code>b</code></div>"#,
        );
        let div = dom.find_first("div").unwrap();
        assert_eq!(map[&div].font_family, vec!["myfont", "monospace"]);
        assert_eq!(map[&dom.find_first("code").unwrap()].font_family, vec!["monospace"]);

        // A bare <pre> is monospace by UA default even without an author rule.
        let (dom, map) = cascade_of("<pre>x</pre>");
        assert_eq!(map[&dom.find_first("pre").unwrap()].font_family, vec!["monospace"]);
    }

    #[test]
    fn extended_selectors_match() {
        use manuk_html::parse;
        let html = r#"
          <div class="nav">
            <a href="/x" class="item">one</a>
            <input type="submit" disabled>
            <a href="https://e.com" data-role="ext">two</a>
            <p>alpha</p><p>beta</p><p>gamma</p>
          </div>"#;
        let dom = parse(html);
        let a1 = dom.find_first("a").unwrap();
        let sub = dom.find_first("input").unwrap();
        // Collect the <p>s in order.
        let ps: Vec<_> = dom
            .descendants(dom.root())
            .filter(|&n| dom.tag_name(n) == Some("p"))
            .collect();
        let m = |sel: &str, node| matches_selector(&dom, node, sel);

        // Child vs descendant combinator.
        assert!(m(".nav > a", a1), "direct child a");
        assert!(m("div a", a1), "descendant a");
        assert!(!m("p > a", a1), "a is not a child of p");

        // Attribute selectors.
        assert!(m("[href]", a1));
        assert!(m("input[type=submit]", sub));
        assert!(m("a[href^='/']", a1), "prefix match");
        let a2 = dom.descendants(dom.root())
            .filter(|&n| dom.tag_name(n) == Some("a")).nth(1).unwrap();
        assert!(m("a[href$='.com']", a2), "suffix match");
        assert!(m("[data-role~=ext]", a2), "includes match");
        assert!(!m("input[type=text]", sub), "type mismatch");

        // Structural pseudo-classes over the three <p>s.
        assert!(m("p:first-child", ps[0]) == false, "p[0] has prior siblings (a/input)");
        assert!(m("p:last-child", ps[2]), "gamma is last child");
        assert!(m("p:nth-child(4)", ps[0]), "alpha is the 4th child element");
        // alpha=4th, beta=5th, gamma=6th among element children.
        assert!(m(":nth-child(odd)", ps[1]), "beta (5th) is odd");
        assert!(m(":nth-child(even)", ps[2]), "gamma (6th) is even");
        assert!(!m(":nth-child(odd)", ps[2]), "gamma (6th) is not odd");
        assert!(m(":not(a)", ps[0]), ":not(a) matches p");

        // State + dynamic pseudos.
        assert!(m("input:disabled", sub));
        assert!(!m("input:enabled", sub));
        assert!(!m("a:hover", a1), ":hover never matches in a static render");
        assert!(!m("a:hover", a2));

        // Sibling combinators.
        assert!(m("p + p", ps[1]), "beta follows a p");
        assert!(!m("p + p", ps[0]), "alpha has no preceding p sibling");
        assert!(m("a ~ p", ps[2]), "gamma has a preceding a sibling");
    }
}
