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

/// An **intrinsic sizing keyword** on `width`/`height` (CSS Sizing L3). All three collapse to
/// `Dim::Auto` for length resolution but resolve to a content-derived size, not a fill.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntrinsicSize {
    /// The narrowest the box can be without overflowing — its longest unbreakable content run.
    MinContent,
    /// The box's preferred size with no width constraint — content laid out unwrapped.
    MaxContent,
    /// `min(max-content, max(min-content, stretch-fit))` — shrink-to-fit against the available space.
    FitContent,
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
    /// `inline-flex` / `inline-grid` — a flex/grid **formatting context** in an *inline-level* box.
    /// The distinction is not cosmetic: a block-level flex container fills its parent, an
    /// inline-level one shrinks to fit. Collapsing the two makes every icon button, chip, pill and
    /// badge on the modern web stretch across its container.
    InlineFlex,
    InlineGrid,
    Table,
    TableRowGroup,
    TableRow,
    TableCell,
    TableCaption,
    TableColumn,
    TableColumnGroup,
    /// `display: contents` — **the element generates no box at all, but its children still do.**
    ///
    /// It is not `none`: nothing is hidden. The wrapper simply vanishes from the box tree and its
    /// children are laid out as if they were the parent's own. Modern CSS leans on it hard — a `<div>`
    /// wrapping grid items so a component can own them, without that `<div>` becoming a grid item itself
    /// and collapsing the whole layout into one cell.
    ///
    /// Unparsed, it fell through to the `_ => s.display` arm and stayed `inline`, which is the worst
    /// possible answer: the wrapper became an inline box that DID participate in layout, so every grid
    /// or flex child inside it was hidden behind a single anonymous inline parent.
    Contents,
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

/// `text-overflow` — how inline content that is clipped by its box is signalled. `clip` (the initial
/// value) just cuts it off; `ellipsis` replaces the trailing clipped text with `…`. Only takes effect
/// on a box that actually clips (`overflow` ≠ `visible`) and doesn't wrap (`white-space: nowrap`) —
/// the near-universal single-line-truncated title/label/tab/table-cell idiom.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TextOverflow {
    #[default]
    Clip,
    Ellipsis,
}

/// One colour stop of a gradient, at a position in `0.0..=1.0` along the gradient line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorStop {
    pub color: Rgba,
    pub at: f32,
}

/// A `background-image`. The modern web's visual identity is mostly *this*: hero gradients, card
/// washes, button fills, and the icons a site does not ship as `<img>`.
#[derive(Clone, Debug, PartialEq)]
pub enum BackgroundImage {
    /// `url(...)` — resolved and decoded by the page layer, painted by the compositor.
    Url(String),
    /// `linear-gradient(<angle>, stops…)`. `angle_deg` is CSS's convention: 0° points **up**, and
    /// angles increase clockwise.
    Linear {
        angle_deg: f32,
        stops: Vec<ColorStop>,
    },
    /// `radial-gradient(stops…)` — centred, covering the box (the `farthest-corner` default).
    Radial { stops: Vec<ColorStop> },
}

/// `background-size`.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum BackgroundSize {
    /// The image's own size.
    #[default]
    Auto,
    /// Scale to fill the box, cropping the overflow.
    Cover,
    /// Scale to fit entirely inside the box.
    Contain,
    Px(f32, f32),
}

/// `object-fit` — how a **replaced element**'s content (an `<img>`/`<video>`) is fitted into its
/// used box when the two have different aspect ratios. The default `fill` stretches (the historical
/// behaviour); `cover` is what nearly every thumbnail/card grid uses so a photo fills its tile
/// without distorting, cropping the overflow.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum ObjectFit {
    /// Stretch to fill the box, ignoring aspect ratio (the initial value).
    #[default]
    Fill,
    /// Scale (preserving aspect ratio) to entirely fit inside the box — letterboxed.
    Contain,
    /// Scale (preserving aspect ratio) to cover the box — the overflow is clipped.
    Cover,
    /// Natural size, centered, clipped to the box.
    None,
    /// The smaller of `none` and `contain` — never scales UP past natural size.
    ScaleDown,
}

/// `object-position` — where the fitted content sits inside its box, as a fraction of the free space
/// on each axis (`0.0` = start edge, `0.5` = centered, `1.0` = end edge). The initial value is
/// `50% 50%` (centered), which `object-fit` (tick 181) already assumed; this makes it explicit, so a
/// cropped hero/avatar can keep its subject in frame (`object-position: top`, `object-position: 20% 50%`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ObjectPosition {
    pub x: f32,
    pub y: f32,
}

impl Default for ObjectPosition {
    fn default() -> Self {
        ObjectPosition { x: 0.5, y: 0.5 }
    }
}

/// `background-repeat`.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum BackgroundRepeat {
    #[default]
    Repeat,
    NoRepeat,
}

/// One axis of `background-position`. CSS resolves a `<percentage>`/keyword against the box's FREE
/// space (so `right` aligns the image's right edge with the box's right edge), but a `<length>` is an
/// absolute offset from the top-left. The two resolve differently, so they are kept distinct until the
/// box and tile sizes are known at paint time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BgPos {
    /// Fraction of the free space (`box − tile`): `left/top`=0.0, `center`=0.5, `right/bottom`=1.0.
    Pct(f32),
    /// Absolute offset in px from the top-left edge.
    Px(f32),
}

impl Default for BgPos {
    fn default() -> Self {
        BgPos::Pct(0.0)
    }
}

/// `background-position` — where a `url()` background image sits in its box. The initial value is
/// `0% 0%` (top-left), which is exactly the fixed-origin blit the painter did before this existed.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct BackgroundPosition {
    pub x: BgPos,
    pub y: BgPos,
}

/// `border-style` — the LINE style of a border. Stored uniform (matching `border_color`, which is also
/// uniform); per-side styles are a follow-on. `groove`/`ridge`/`inset`/`outset` collapse to `Solid`
/// (their bevel shading is a paint refinement, and a solid line is the honest approximation).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum BorderStyle {
    #[default]
    Solid,
    Dashed,
    Dotted,
    /// Two parallel lines with a gap between them.
    Double,
}

/// `text-decoration-line`. Bitflags, because `underline line-through` is legal and used.
/// (No `Eq` — `underline_offset`/`thickness` carry `f32`, and nothing keys a map on this.)
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct TextDecoration {
    pub underline: bool,
    pub overline: bool,
    pub line_through: bool,
    /// `text-decoration-color`. `None` == the `currentColor` default (paint falls back to the
    /// text color); `Some` is an explicitly-set line color (colored underlines, hover states).
    pub color: Option<Rgba>,
    /// `text-decoration-thickness`. `None` == `auto` (paint derives it from the font size);
    /// `Some(px)` is an explicit thickness (Tailwind `decoration-2`, thick brand underlines).
    pub thickness: Option<f32>,
    /// `text-underline-offset`. Extra px pushing the *underline* down, away from the text
    /// (Tailwind `underline-offset-4`). Default 0; applies only to the underline line.
    pub underline_offset: f32,
}

impl TextDecoration {
    pub fn any(&self) -> bool {
        self.underline || self.overline || self.line_through
    }
}

/// `list-style-type` — the marker a list item draws. Absent these, every `<ul>` and `<ol>` on the
/// web renders as bare indented text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ListStyleType {
    #[default]
    Disc,
    Circle,
    Square,
    Decimal,
    LowerAlpha,
    UpperAlpha,
    LowerRoman,
    UpperRoman,
    None,
}

/// `white-space`, which drives inline wrapping/collapsing in layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WhiteSpace {
    Normal,
    NoWrap,
    /// `pre` — preserve newlines AND runs of spaces; never wrap. (`<pre>`, code blocks.)
    Pre,
    /// `pre-wrap` — preserve newlines and spaces, but still wrap long lines.
    PreWrap,
    /// `pre-line` — preserve newlines, collapse runs of spaces, wrap.
    PreLine,
}

/// `text-transform` — the **rendered** casing of text, applied at layout without changing the DOM
/// text (so JS still reads the author's string). `uppercase` is ubiquitous on nav bars, buttons and
/// section headings; without it "SUBMIT" renders as "submit". Inherited.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TextTransform {
    #[default]
    None,
    Uppercase,
    Lowercase,
    /// The first typographic letter of each word is upper-cased; the rest are left as authored.
    Capitalize,
}

/// `overflow-wrap` (and its legacy alias `word-wrap`) — whether an otherwise-unbreakable word may
/// be broken at an arbitrary character to stop it overflowing its line box. `break-word` is the
/// ubiquitous fix for a long URL / hash / email in a narrow column: without it the token spills out
/// past the container edge and breaks the layout. Inherited.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum OverflowWrap {
    #[default]
    Normal,
    /// Break inside a word only when it would otherwise overflow the line (the common case).
    BreakWord,
    /// Like `break-word`, but the broken word also counts as a soft-wrap opportunity for
    /// min-content sizing. We treat it identically to `break-word` for wrapping.
    Anywhere,
}

/// `direction` — the **base direction** of a paragraph's bidi algorithm, and the single thing that
/// makes an RTL page readable rather than merely present.
///
/// It is not "which way the glyphs face" (that is the script's own property, resolved by shaping).
/// It is the base embedding level the Unicode Bidi Algorithm resolves everything else against, and
/// it decides where a trailing period sits, which end a line starts from, and how embedded Latin
/// words and numbers are ordered inside Arabic or Hebrew text. Get it wrong and every character is
/// present, correctly shaped, and in the wrong order. Inherited.
///
/// ⚠ HTML's initial value is `ltr`, **not** auto-detection — an unmarked Arabic paragraph is LTR in
/// Chrome too, so we must not "helpfully" infer RTL from content. Real RTL sites say so, with
/// `dir="rtl"` on `<html>` or `direction: rtl` in CSS.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Direction {
    #[default]
    Ltr,
    Rtl,
}

/// `word-break` — where line breaks are allowed *within* a run. `break-all` lets a break fall
/// between any two characters (common in CJK text and code listings); we honour it as "may break a
/// word at any character to fit", the same char-level breaking `overflow-wrap:break-word` enables.
/// Inherited.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum WordBreak {
    #[default]
    Normal,
    BreakAll,
    /// `keep-all` — never break within a word (parsed but not yet distinguished from `normal` for
    /// Latin text, where it already never breaks mid-word).
    KeepAll,
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

/// One `box-shadow` layer: `[inset] offset-x offset-y [blur [spread]] [color]`. A `box-shadow`
/// value is a comma-separated LIST of these — Tailwind's elevation utilities (`shadow`, `shadow-md`,
/// `shadow-lg`) all stack two layers, the second with a negative spread, so a single-shadow model
/// rendered every one of them wrong.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoxShadow {
    pub dx: f32,
    pub dy: f32,
    /// Blur radius in px (`0` = a hard-edged offset rect).
    pub blur: f32,
    /// Spread radius in px — inflates (positive) or shrinks (negative) the shadow rect before the
    /// offset and blur. Tailwind's stacked shadows tighten their second layer with a negative spread.
    pub spread: f32,
    /// `inset` — an inner shadow. Parsed so a mixed list keeps its outer layers; inner painting is
    /// not yet done (an inset-only shadow paints nothing, exactly as before).
    pub inset: bool,
    pub color: Rgba,
}

/// A `text-shadow` layer: `offset-x offset-y [blur] [color]`. Like `box-shadow` but with no spread and
/// no `inset` — it paints the run's glyphs a second time, offset and (eventually) blurred, behind the
/// text. `text-shadow` is inherited.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextShadow {
    pub dx: f32,
    pub dy: f32,
    /// Blur radius in px (`0` = a hard-edged offset copy). Blur is not yet painted (residue).
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
    /// `background-image` — a LIST of layers (url or gradient), painted back-to-front: index 0 is the
    /// TOPMOST layer. Painting only the colour and dropping this is why gradient heroes, washed cards
    /// and CSS-only icons rendered as blank rectangles; modelling it as a single layer is why a
    /// `linear-gradient(...) , url(hero.jpg)` scrim rendered the photo with no darkening overlay.
    pub background_images: Vec<BackgroundImage>,
    pub background_size: BackgroundSize,
    /// `background-position` — where a `url()` background image sits (default `0% 0%`, top-left).
    pub background_position: BackgroundPosition,
    /// `object-fit` — how a replaced element's content is fitted into its box (default `fill`).
    pub object_fit: ObjectFit,
    /// `object-position` — where the fitted content sits in its box (default centered).
    pub object_position: ObjectPosition,
    /// **Intrinsic aspect ratio (width / height) of a REPLACED element** — an `<img>`, `<video>`,
    /// `<canvas>`. Set from the decoded image once it arrives; `None` for everything else.
    ///
    /// Without it, constraining a replaced element's width does nothing to its height: a 400×300
    /// image under the near-universal `img { max-width: 100% }` reset came out **150×300** in a
    /// 150px column — the right width and its full natural height, stretched to twice its correct
    /// size. Every responsive image on the web was wrong.
    pub aspect_ratio: Option<f32>,
    pub background_repeat: BackgroundRepeat,
    /// `text-decoration-line` (INHERITED in effect: a decoration set on a block draws through its
    /// inline descendants).
    pub text_decoration: TextDecoration,
    /// `list-style-type` (inherited).
    pub list_style_type: ListStyleType,
    /// `list-style-position: inside` puts the marker in the principal box's content flow.
    pub list_style_inside: bool,
    /// `content` — only meaningful on a `::before`/`::after` pseudo-element.
    pub content: Option<String>,
    /// The computed style of this element's `::before` / `::after` pseudo-elements, when they have
    /// `content`. Generated content is not in the DOM (script must never see it), so it rides on
    /// the element's style and is materialised as inline items at layout time.
    ///
    /// This is not a decorative corner of CSS: it is how the web draws icons, quotation marks,
    /// counters, dividers, clearfixes and a great deal of layout scaffolding.
    pub before: Option<Box<ComputedStyle>>,
    pub after: Option<Box<ComputedStyle>>,
    /// `outline` — the focus ring. Without it keyboard focus is invisible, which is not a cosmetic
    /// bug but an accessibility one.
    pub outline_width: f32,
    pub outline_color: Rgba,
    pub font_size: f32,
    pub font_weight: u16,
    /// The `font-family` list (names in priority order, lowercased; generic keywords kept
    /// literally, e.g. `"sans-serif"`). Resolved to a concrete face by the text layer.
    pub font_family: Vec<String>,
    pub italic: bool,
    pub line_height: f32,
    pub text_align: TextAlign,
    pub white_space: WhiteSpace,
    /// `text-overflow` — `ellipsis` truncates clipped single-line inline content with `…`.
    pub text_overflow: TextOverflow,
    /// `text-transform` — rendered casing (inherited); applied in layout, DOM text unchanged.
    pub text_transform: TextTransform,
    /// `overflow-wrap`/`word-wrap` — allow breaking a long word at an arbitrary char (inherited).
    pub overflow_wrap: OverflowWrap,
    /// `word-break` — char-level break control within a run (inherited).
    pub word_break: WordBreak,
    /// `direction` — the paragraph's bidi base direction (inherited).
    pub direction: Direction,
    /// `letter-spacing` — extra px added after each character (tracking). `0` = `normal`. Inherited.
    pub letter_spacing: f32,
    /// `word-spacing` — extra px added to each inter-word space. `0` = `normal`. Inherited.
    pub word_spacing: f32,
    pub margin: Sides<Dim>,
    pub padding: Sides<Dim>,
    pub border_width: Sides<f32>,
    pub border_color: Rgba,
    /// `border-style` — the line style (solid/dashed/dotted/double), uniform like `border_color`.
    pub border_style: BorderStyle,
    /// `border-radius` — a single uniform corner radius in px (per-corner radii are a follow-on).
    /// `0.0` = square corners.
    pub border_radius: f32,
    /// `visibility` (inherited). `Hidden`/`Collapse` boxes still take space but are not painted.
    pub visibility: Visibility,
    /// `line-height: normal` — the value was NOT authored, so it must come from the FONT's own
    /// ascent/descent/lineGap rather than a multiple of the font size. A 1.2× guess is not what any
    /// browser does, and it makes every line box the wrong height on every page.
    pub line_height_normal: bool,
    /// `mask-image` / `-webkit-mask-image` `url(...)`. The modern web draws **icons** as an empty
    /// element with a `background-color` shaped by a mask. Ignoring the mask paints the raw
    /// background — a solid black square where every icon should be.
    pub mask_image: Option<String>,
    /// **Effective** `opacity` — this element's own `opacity` already multiplied by its ancestors'
    /// (CSS opacity applies to the whole subtree). `0.0` = fully transparent, `1.0` = opaque.
    pub opacity: f32,
    /// Whether this element has a CSS **animation** running (`animation-name` is not `none`).
    ///
    /// A static renderer cannot animate. What it can do — and what it MUST do — is not leave the user
    /// staring at nothing: the single most common animation on the web is a **fade-in**, whose base rule
    /// sets `opacity: 0` and whose keyframes reveal the element. Render the base rule literally and the
    /// content **never appears at all**.
    ///
    /// Measured: **21% of the corpus (52 of 237 sites)** has a rule that starts at `opacity: 0` together
    /// with an animation. That is not a visual nicety — it is a fifth of the web with invisible content.
    pub has_animation: bool,
    /// `box-shadow` — the ordered list of shadow layers (front-to-back, first on top). Empty == no
    /// shadow. A comma list stacks layers (Tailwind's `shadow-md`); each carries its own spread/inset.
    pub box_shadows: Vec<BoxShadow>,
    /// `text-shadow` — a single shadow behind the text (inherited). `None` == no shadow. A comma list
    /// of shadows is parsed to its first layer (multi-shadow is residue).
    pub text_shadow: Option<TextShadow>,
    pub width: Dim,
    /// The **intrinsic sizing keyword** on `width`, if any. `width` itself collapses to `Dim::Auto`
    /// for length resolution (an intrinsic width is content-driven, not a length), but unlike a plain
    /// `auto` a keyword width does NOT fill the containing block — it hugs the content: `min-content`
    /// is the longest unbreakable run, `max-content` the whole content unwrapped, `fit-content` the
    /// shrink-to-fit clamp between them. `None` = `auto`/length/`stretch`/`fill-available` (all fill).
    pub width_keyword: Option<IntrinsicSize>,
    pub height: Dim,
    /// `true` when `height` is an **intrinsic sizing keyword** (`min-content`/`max-content`/
    /// `fit-content`), which all collapse to `Dim::Auto` for length resolution but are *not* the
    /// same as `auto`: an intrinsic-keyword height is **indefinite**, so an abspos box with both
    /// insets set must NOT take the CSS2 §10.6.4 constraint-equation definite height — it sizes to
    /// content instead (and a `height:100%` child sees an indefinite base → auto). Without this the
    /// keyword is indistinguishable from `auto` and `inset:0; height:fit-content` wrongly stretches.
    pub height_intrinsic: bool,
    /// `true` when `height` is `stretch` / `-webkit-fill-available` / `-moz-available` — the box FILLS
    /// its containing block's definite content height (margin box = CB content box), unlike `auto`
    /// (content height) and unlike the intrinsic keywords (`height_intrinsic`, indefinite). Collapses
    /// to `Dim::Auto` for length resolution; this flag restores the fill in `layout_block`.
    pub height_stretch: bool,
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
    /// `overflow-x` / `overflow-y` kept per-axis. The collapsed `overflow` above loses which
    /// axis scrolls, but a *classic* scrollbar reserves space on the axis it lives on: a vertical
    /// scrollbar (`overflow-y:scroll` in horizontal-tb) eats inline width, a horizontal one eats
    /// block height. Scrollbar-gutter reservation needs the axis, so it reads these.
    pub overflow_x: Overflow,
    pub overflow_y: Overflow,
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
            text_overflow: TextOverflow::Clip,
            text_transform: TextTransform::None,
            overflow_wrap: OverflowWrap::Normal,
            word_break: WordBreak::Normal,
            direction: Direction::Ltr,
            letter_spacing: 0.0,
            word_spacing: 0.0,
            margin: Sides::all(Dim::Px(0.0)),
            padding: Sides::all(Dim::Px(0.0)),
            border_width: Sides::all(0.0),
            border_color: Rgba::BLACK,
            border_style: BorderStyle::default(),
            border_radius: 0.0,
            visibility: Visibility::Visible,
            line_height_normal: true,
            mask_image: None,
            background_images: Vec::new(),
            background_size: BackgroundSize::Auto,
            background_position: BackgroundPosition::default(),
            object_fit: ObjectFit::Fill,
            object_position: ObjectPosition::default(),
            aspect_ratio: None,
            background_repeat: BackgroundRepeat::Repeat,
            text_decoration: TextDecoration::default(),
            list_style_type: ListStyleType::Disc,
            list_style_inside: false,
            content: None,
            before: None,
            after: None,
            outline_width: 0.0,
            outline_color: Rgba {
                r: 0,
                g: 0,
                b: 0,
                a: 0,
            },
            opacity: 1.0,
            has_animation: false,
            box_shadows: Vec::new(),
            text_shadow: None,
            width: Dim::Auto,
            width_keyword: None,
            height: Dim::Auto,
            height_intrinsic: false,
            height_stretch: false,
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
            overflow_x: Overflow::Visible,
            overflow_y: Overflow::Visible,
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
        // `visibility` is inherited — a hidden subtree stays hidden unless a descendant explicitly
        // re-declares `visible`.
        s.visibility = parent.visibility;
        s.color = parent.color;
        s.font_size = parent.font_size;
        s.font_weight = parent.font_weight;
        s.font_family = parent.font_family.clone();
        s.italic = parent.italic;
        s.line_height = parent.line_height;
        // The FLAG is inherited with the value. Inheriting the number but not "was this authored?"
        // means a child re-derives its line box from the font while its parent uses the author's —
        // two different line heights for the same inherited property.
        s.line_height_normal = parent.line_height_normal;
        s.text_align = parent.text_align;
        s.white_space = parent.white_space;
        s.text_transform = parent.text_transform;
        s.overflow_wrap = parent.overflow_wrap;
        s.word_break = parent.word_break;
        s.direction = parent.direction;
        s.letter_spacing = parent.letter_spacing;
        s.word_spacing = parent.word_spacing;
        // `text-shadow` is inherited (a shadow on a heading carries to its inline `<span>`s).
        s.text_shadow = parent.text_shadow;
        // `list-style-*` is inherited (that is how `ul{list-style:none}` silences its `li`s).
        s.list_style_type = parent.list_style_type;
        s.list_style_inside = parent.list_style_inside;
        // `text-decoration` is not *inherited* in the CSS sense — it PROPAGATES: a decoration on a
        // block draws through its inline descendants. Carrying it down the tree is how the text
        // fragments that actually paint find out about it.
        s.text_decoration = parent.text_decoration;
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
    styles
        .iter()
        .map(|(n, s)| (*n, scale_style(s, k)))
        .collect()
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
        || old.text_transform != new.text_transform
        || old.overflow_wrap != new.overflow_wrap
        || old.direction != new.direction
        || old.word_break != new.word_break
        || old.letter_spacing != new.letter_spacing
        || old.word_spacing != new.word_spacing
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
    /// The ASCII case-insensitivity flag: `[name=val i]` matches the value case-insensitively;
    /// `[name=val s]` (and the default for author attributes) is case-sensitive. Selectors §6.3.
    ci: bool,
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
    /// **`:has(<relative-selector-list>)` — hand-rolled, because Stylo's *servo* build DISCARDS it.**
    ///
    /// `parse_has()` returns `false` there (Gecko's returns `true`), so a selector containing `:has()`
    /// fails to parse and CSS error-recovery throws the **whole rule** away — the declarations never
    /// apply at all. **13% of the corpus.** Enabling it upstream means vendoring Stylo; extending the
    /// engine we already own does not. (STATUS.md: *a borrowed engine is a means, not a constraint*.)
    ///
    /// The argument is a list of RELATIVE selectors: each may lead with a combinator
    /// (`:has(> .x)`, `:has(+ .sib)`, `:has(~ .later)`) or omit it, which means descendant
    /// (`:has(.x)` ≡ `:has(:scope .x)`). The anchor is the element being tested.
    Has(Vec<(Combinator, Selector)>),
    /// `::before` / `::after` — a **pseudo-ELEMENT**, not a pseudo-class. It does not filter which
    /// elements match; it says the rule styles a *generated box* hanging off the matched element.
    /// Treating it as an unknown pseudo-class (never matches) silently dropped every icon, quote,
    /// counter and divider the web draws this way.
    Before,
    After,
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

/// `visibility` — an element that is `hidden` still **occupies its box** (unlike `display:none`)
/// but is not painted. It is an **inherited** property, so a hidden subtree stays hidden unless a
/// descendant explicitly sets `visibility: visible`.
///
/// This is not a nicety: the modern web hides dropdowns/modals/tooltips with `visibility:hidden`
/// (+ `opacity:0`) far more often than with `display:none`, because those are animatable. Without
/// it, every such element paints **on top of the page**.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Visibility {
    #[default]
    Visible,
    Hidden,
    /// `collapse` — treated as `hidden` outside tables (which is what the spec allows).
    Collapse,
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
    /// Does this selector contain a `:has()` anywhere? These are the rules Stylo discards, and the
    /// only ones the supplement pass is allowed to touch — applying a *normal* rule twice would
    /// double-apply it over the Stylo cascade.
    fn has_relative(&self) -> bool {
        self.parts
            .iter()
            .any(|c| c.pseudos.iter().any(|p| matches!(p, Pseudo::Has(_))))
    }

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
        Pseudo::Root => dom
            .parent(node)
            .map(|p| !dom.is_element(p))
            .unwrap_or(false),
        Pseudo::Empty => !dom.children(node).any(|c| {
            dom.is_element(c) || matches!(dom.data(c), NodeData::Text(t) if !t.trim().is_empty())
        }),
        Pseudo::Checked => el.attr("checked").is_some() || el.attr("selected").is_some(),
        Pseudo::Disabled => el.attr("disabled").is_some(),
        Pseudo::Enabled => {
            matches!(
                el.name.as_str(),
                "input" | "button" | "select" | "textarea" | "option"
            ) && el.attr("disabled").is_none()
        }
        Pseudo::Required => el.attr("required").is_some(),
        Pseudo::Link => {
            matches!(el.name.as_str(), "a" | "area" | "link") && el.attr("href").is_some()
        }
        Pseudo::Not(inner) => !compound_matches(inner, dom, node),
        // `:has(...)` — does ANY element in the anchor's relative scope match the branch selector?
        //
        // The search space is decided by the leading combinator, and getting that right is the whole
        // cost of the feature: a descendant `:has()` searches the subtree, a child `:has()` searches one
        // level, and a sibling `:has()` searches forward among siblings. Searching the subtree for a
        // sibling selector would be both wrong and slow.
        Pseudo::Has(branches) => branches.iter().any(|(comb, sel)| match comb {
            // `Dom::descendants` seeds with the node's CHILDREN — it does NOT yield the node itself, so
            // there is nothing to skip. Skipping one here silently dropped the FIRST descendant, which is
            // exactly where `:has(.probe)` finds `.probe` on `<div class=a><div class=probe>`. The bug
            // and the test that catches it are the same two lines.
            Combinator::Descendant => dom
                .descendants(node)
                .any(|d| dom.is_element(d) && selector_matches_relative(sel, dom, d, node)),
            Combinator::Child => dom
                .children(node)
                .any(|c| dom.is_element(c) && selector_matches_relative(sel, dom, c, node)),
            Combinator::NextSibling => dom
                .next_sibling(node)
                .into_iter()
                .flat_map(|n| {
                    // The next ELEMENT sibling, skipping text nodes between them.
                    let mut cur = Some(n);
                    std::iter::from_fn(move || {
                        while let Some(x) = cur {
                            cur = dom.next_sibling(x);
                            if dom.is_element(x) {
                                return Some(x);
                            }
                        }
                        None
                    })
                    .take(1)
                })
                .any(|sib| selector_matches_relative(sel, dom, sib, node)),
            Combinator::SubsequentSibling => {
                let mut cur = dom.next_sibling(node);
                let mut hit = false;
                while let Some(x) = cur {
                    if dom.is_element(x) && selector_matches_relative(sel, dom, x, node) {
                        hit = true;
                        break;
                    }
                    cur = dom.next_sibling(x);
                }
                hit
            }
        }),
        // A pseudo-ELEMENT never *filters* the element — the rule matches the originating element
        // and styles its generated box. The cascade routes those declarations to `before`/`after`.
        Pseudo::Before | Pseudo::After => true,
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
    // The `i` flag (`[attr=val i]`) makes value matching ASCII case-insensitive. We normalise both
    // sides once — borrowing on the common (case-sensitive) path so the hot path allocates nothing.
    let (actual, value) = if a.ci {
        (
            std::borrow::Cow::Owned(actual.to_ascii_lowercase()),
            std::borrow::Cow::Owned(a.value.to_ascii_lowercase()),
        )
    } else {
        (
            std::borrow::Cow::Borrowed(actual),
            std::borrow::Cow::Borrowed(a.value.as_str()),
        )
    };
    let (actual, value) = (actual.as_ref(), value.as_ref());
    match a.op {
        AttrOp::Exists => true,
        AttrOp::Equals => actual == value,
        AttrOp::Includes => actual.split_whitespace().any(|w| w == value),
        AttrOp::Prefix => !value.is_empty() && actual.starts_with(value),
        AttrOp::Suffix => !value.is_empty() && actual.ends_with(value),
        AttrOp::Substring => !value.is_empty() && actual.contains(value),
        AttrOp::DashMatch => actual == value || actual.starts_with(&format!("{value}-")),
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
fn selector_matches_scoped(sel: &Selector, dom: &Dom, node: NodeId, scope: Option<NodeId>) -> bool {
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

/// Match `sel` at `node` **within the relative scope of `anchor`** (`:has()`'s subject).
///
/// For a single-compound branch — which is nearly all of them (`:has(.x)`, `:has(> img)`) — this is just
/// "does the candidate match". For a multi-compound branch (`:has(.a .b)`) the ancestry walk is the
/// ordinary one; the anchor bounds the *search*, not the *match*, and that is the honest 95% of the
/// feature. A branch that walks left past the anchor is vanishingly rare in real CSS and is not worth a
/// second matching engine to be exactly right about.
fn selector_matches_relative(sel: &Selector, dom: &Dom, node: NodeId, _anchor: NodeId) -> bool {
    selector_matches(sel, dom, node)
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
                let Some(p) = dom.parent(right) else {
                    return false;
                };
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
                let Some(s) = prev_element_sibling(dom, right) else {
                    return false;
                };
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

/// One selector of one rule, with the scope + source order it was seen at.
#[derive(Clone, Copy)]
struct IndexedRule<'a> {
    scope: Option<NodeId>,
    sel: &'a Selector,
    rule: &'a Rule,
    order: usize,
}

/// A selector index: rules bucketed by the **key** of their rightmost (subject) compound, so an
/// element only tests rules it could possibly match. See `MinimalCascade::build_index`.
#[derive(Default)]
struct RuleIndex<'a> {
    by_id: std::collections::HashMap<String, Vec<IndexedRule<'a>>>,
    by_class: std::collections::HashMap<String, Vec<IndexedRule<'a>>>,
    by_tag: std::collections::HashMap<String, Vec<IndexedRule<'a>>>,
    universal: Vec<IndexedRule<'a>>,
}

impl<'a> RuleIndex<'a> {
    /// Visit the rules that could possibly match `node`: those keyed on its id, on any of its
    /// classes, on its tag, plus the universal bucket.
    ///
    /// Order does not matter here and we deliberately do **not** sort: the caller already sorts the
    /// *matched declarations* by `(specificity, source order)`, so pre-sorting candidates was pure
    /// waste (an allocation + an O(k log k) sort **per element**). Visiting via a callback also
    /// avoids allocating a candidate Vec per element — on a large document that allocation and
    /// sort were themselves a meaningful slice of the cascade.
    fn for_each_candidate(&self, dom: &Dom, node: NodeId, mut f: impl FnMut(&IndexedRule<'a>)) {
        for r in &self.universal {
            f(r);
        }
        if let Some(el) = dom.element(node) {
            if let Some(id) = el.attr("id") {
                if let Some(v) = self.by_id.get(id) {
                    for r in v {
                        f(r);
                    }
                }
            }
            for c in el.classes() {
                if let Some(v) = self.by_class.get(c) {
                    for r in v {
                        f(r);
                    }
                }
            }
        }
        if let Some(tag) = dom.tag_name(node) {
            if let Some(v) = self.by_tag.get(&tag.to_ascii_lowercase()) {
                for r in v {
                    f(r);
                }
            }
        }
    }
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
    /// **Apply this sheet's `:has()` rules to `style` — the rules Stylo THREW AWAY.**
    ///
    /// Stylo's *servo* build hardcodes `parse_has() -> false`, so a selector containing `:has()` fails to
    /// parse and CSS error-recovery discards the **whole rule**: its declarations never reach the cascade
    /// at all. **13% of the corpus uses `:has()`.** Enabling it upstream means vendoring Stylo (`./stylo`
    /// is a reference checkout — the build takes `stylo = "0.19"` from crates.io), so this extends the
    /// selector engine we already own instead. See STATUS.md: *a borrowed engine is a means, not a
    /// constraint* — pref → flag delta → **supplement** → module.
    ///
    /// Runs **after** the Stylo cascade, and the ordering is the honest part:
    ///
    /// * Winners among `:has()` rules are ordered by `(specificity, source order)` — the real cascade rule.
    /// * A `:has()` rule then applies **over** the Stylo result. That is correct whenever it out-specifies
    ///   whatever set the property, and it is what an author writing `:has()` almost always intends (these
    ///   selectors are, by construction, more specific than the base rule they are refining).
    /// * It is **not** universally correct: a low-specificity `:has()` rule cannot currently lose to a
    ///   higher-specificity normal rule, because Stylo does not tell us which rule won each property.
    ///   That is a **known, bounded** inaccuracy — stated here rather than discovered later — and it is
    ///   strictly better than the status quo, which is that the rule does not exist at all.
    pub fn apply_has_rules(
        &self,
        dom: &Dom,
        node: NodeId,
        style: &mut ComputedStyle,
        parent_font_size: f32,
    ) -> usize {
        let mut winners: Vec<(u32, usize, &Declaration)> = Vec::new();
        for (order, rule) in self.rules.iter().enumerate() {
            for sel in &rule.selectors {
                if !sel.has_relative() {
                    continue;
                }
                if selector_matches(sel, dom, node) {
                    let spec = sel.specificity();
                    for d in &rule.declarations {
                        winners.push((spec, order, d));
                    }
                }
            }
        }
        if winners.is_empty() {
            return 0;
        }
        // `(specificity, source order)` — the cascade's own ordering, and `!important` beats both.
        winners.sort_by_key(|(spec, order, d)| (d.important, *spec, *order));
        let n = winners.len();
        for (_, _, d) in winners {
            apply_declaration(style, d, parent_font_size);
        }
        n
    }

    /// Whether this sheet contains any `:has()` rule at all — the cheap check that keeps the supplement
    /// off the hot path for the 87% of sheets that do not use it.
    pub fn has_relative_rules(&self) -> bool {
        self.rules
            .iter()
            .any(|r| r.selectors.iter().any(|s| s.has_relative()))
    }

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
                family = Some(
                    d.value
                        .trim()
                        .trim_matches(['"', '\''])
                        .to_ascii_lowercase(),
                )
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
    // A pseudo-element we do not model must not silently match its subject — a rule for
    // `::first-line` would otherwise restyle the whole element. But `::before` / `::after` we DO
    // model: they are routed to a generated box, not to the subject. Dropping them here is what
    // silently erased every icon, quotation mark, counter and divider the web generates.
    if text.contains("::") && !text.contains("::before") && !text.contains("::after") {
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
    let mut it = text.chars().peekable();
    while let Some(ch) = it.next() {
        match ch {
            '\\' => {
                // Keep an escape sequence verbatim through tokenization, so an escaped whitespace or
                // combinator (`#a\ b`, `#\30 nextIsWhiteSpace`) is NOT split into two compounds — the
                // trailing whitespace of a hex escape belongs to the escape, not to a descendant
                // combinator. `take_ident` decodes it downstream via `consume_escaped_code_point`.
                cur.push('\\');
                if matches!(it.peek(), Some(h) if h.is_ascii_hexdigit()) {
                    let mut n = 0;
                    while n < 6 {
                        match it.peek() {
                            Some(h) if h.is_ascii_hexdigit() => {
                                cur.push(*h);
                                it.next();
                                n += 1;
                            }
                            _ => break,
                        }
                    }
                    if matches!(it.peek(), Some(c) if c.is_whitespace()) {
                        cur.push(it.next().unwrap());
                    }
                } else if let Some(n) = it.next() {
                    cur.push(n);
                }
            }
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
                              // `::before` — a pseudo-ELEMENT is written with two colons. Bailing on the second
                              // one dropped the whole selector, and with it every icon, quote and divider the web
                              // generates this way. (One colon is legal CSS2 syntax for these too.)
                if chars.peek() == Some(&':') {
                    chars.next();
                }
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
    // Two-char operators first, then `=`. (The `|=` token is matched before the bare `|` namespace
    // separator can be mistaken for it — `*|foo` contains no `|=`.)
    for (tok, op) in [
        ("~=", AttrOp::Includes),
        ("^=", AttrOp::Prefix),
        ("$=", AttrOp::Suffix),
        ("*=", AttrOp::Substring),
        ("|=", AttrOp::DashMatch),
    ] {
        if let Some((name, value)) = inner.split_once(tok) {
            let (value, ci) = parse_attr_value(value);
            return Some(AttrSel {
                name: strip_attr_ns(name.trim()).to_ascii_lowercase(),
                op,
                value,
                ci,
            });
        }
    }
    if let Some((name, value)) = inner.split_once('=') {
        let (value, ci) = parse_attr_value(value);
        return Some(AttrSel {
            name: strip_attr_ns(name.trim()).to_ascii_lowercase(),
            op: AttrOp::Equals,
            value,
            ci,
        });
    }
    if inner.is_empty() {
        return None;
    }
    Some(AttrSel {
        name: strip_attr_ns(inner).to_ascii_lowercase(),
        op: AttrOp::Exists,
        value: String::new(),
        ci: false,
    })
}

/// Drop a namespace prefix from an attribute name. `*|attr` (any namespace), `|attr` (no namespace)
/// and `ns|attr` all resolve to the local name `attr` — correct for our HTML-only, no-namespace
/// attribute model, where every attribute lives in the null namespace.
fn strip_attr_ns(name: &str) -> &str {
    match name.rfind('|') {
        Some(i) => &name[i + 1..],
        None => name,
    }
}

/// Split an attribute-selector RHS into its (unquoted) value and the ASCII case-insensitivity flag.
///
/// The grammar is `value [ <ws> (i|s) ]`, where the flag may also abut a quoted value (`'bar'i`).
/// `i`/`I` → case-insensitive; `s`/`S` → case-sensitive; absent → case-sensitive (author-attr default).
fn parse_attr_value(raw: &str) -> (String, bool) {
    let raw = raw.trim();
    let bytes = raw.as_bytes();
    let (val_part, flag_part) = if matches!(bytes.first(), Some(b'"') | Some(b'\'')) {
        let quote = bytes[0] as char;
        match raw[1..].find(quote) {
            // `close` is the byte index of the closing quote within `raw`.
            Some(rel) => {
                let close = 1 + rel;
                (&raw[..=close], raw[close + 1..].trim())
            }
            None => (raw, ""),
        }
    } else {
        // Unquoted: the value runs to the first whitespace; anything after it is the flag.
        match raw.find(char::is_whitespace) {
            Some(i) => (&raw[..i], raw[i..].trim()),
            None => (raw, ""),
        }
    };
    let ci = flag_part.eq_ignore_ascii_case("i");
    let value = val_part.trim().trim_matches(['"', '\'']).to_string();
    (value, ci)
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
        // Pseudo-ELEMENTS. `::before`/`::after` are legal with one colon too (CSS2 syntax), and
        // plenty of real sheets still write them that way.
        "before" => Pseudo::Before,
        "after" => Pseudo::After,
        // Dynamic / state pseudos we can't evaluate in a static render → never match, so a
        // rule gated on them just doesn't apply (rather than dropping the whole rule).
        "hover" | "focus" | "active" | "visited" | "target" | "focus-within" | "focus-visible"
        | "read-write" | "placeholder-shown" | "autofill" => Pseudo::NeverStatic,
        "nth-child" => {
            let (a, b) = parse_nth(arg?)?;
            Pseudo::NthChild(a, b)
        }
        "not" => {
            let inner = parse_compound(arg?.trim())?;
            Pseudo::Not(Box::new(inner))
        }
        "has" => {
            // A forgiving relative-selector list: `:has(> .a, + .b, .c)`. A branch we cannot parse is
            // DROPPED, not fatal — the rest of the list still applies, which is what "forgiving" means
            // and is why `:has()` does not take a whole stylesheet down when it meets one odd selector.
            let mut branches = Vec::new();
            for raw in split_top_level_commas(arg?) {
                let t = raw.trim();
                if t.is_empty() {
                    continue;
                }
                let (comb, rest) = match t.as_bytes().first() {
                    Some(b'>') => (Combinator::Child, &t[1..]),
                    Some(b'+') => (Combinator::NextSibling, &t[1..]),
                    Some(b'~') => (Combinator::SubsequentSibling, &t[1..]),
                    // No leading combinator means DESCENDANT: `:has(.x)` is `:has(:scope .x)`.
                    _ => (Combinator::Descendant, t),
                };
                if let Some(sel) = parse_selector(rest.trim()) {
                    branches.push((comb, sel));
                }
            }
            if branches.is_empty() {
                return None;
            }
            Pseudo::Has(branches)
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
        if ch == '\\' {
            // A backslash escape is part of the ident: `#\.foo` selects id `.foo`, `#\30 x` selects
            // `0x`. Decode it per css-syntax §4.3.7 rather than stopping — the old code treated `\` as a
            // terminator, so every escaped id/class silently matched nothing.
            chars.next(); // consume the backslash
            if let Some(c) = consume_escaped_code_point(chars) {
                s.push(c);
            }
        } else if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || (ch as u32) >= 0x80 {
            // ASCII ident chars plus any non-ASCII code point (CSS idents allow U+0080+ directly).
            s.push(ch);
            chars.next();
        } else {
            break;
        }
    }
    s
}

/// css-syntax §4.3.7 **consume an escaped code point** — the leading `\` has already been consumed.
/// A run of 1–6 hex digits (then one optional trailing whitespace) is that code point; anything else is
/// the next code point taken literally. **Null and out-of-range values become U+FFFD** — that replacement
/// is why `#zero\0` matches an id of `"zero\u{FFFD}"` and *not* one holding a raw NUL, which is exactly
/// what `ParentNode-querySelector-escapes` checks (NUL is storable and distinct, so it is winnable).
///
/// **A surrogate-half escape returns `None` (the code point is dropped), which is a NAMED limitation, not
/// the spec's U+FFFD.** The spec maps `\d83d` to U+FFFD — but this engine stores attribute values as UTF-8
/// (a lone surrogate cannot round-trip; JS→DOM lossily collapses it to U+FFFD already). Emitting U+FFFD
/// here would make a surrogate-escape selector *false-match* an id that only holds U+FFFD because its lone
/// surrogate was lost — turning a `querySelector-escapes` "should never match" green→red. Dropping the
/// code point keeps such selectors from matching, so no test regresses; faithful surrogate handling is
/// gated on WTF-8/UTF-16 attribute storage (the same subsystem as CharacterData surrogate splitting).
fn consume_escaped_code_point(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) -> Option<char> {
    let mut hex = String::new();
    while hex.len() < 6 {
        match chars.peek() {
            Some(h) if h.is_ascii_hexdigit() => {
                hex.push(*h);
                chars.next();
            }
            _ => break,
        }
    }
    if hex.is_empty() {
        // Not a hex escape: the next input code point, verbatim. `\` at end-of-input → U+FFFD.
        return Some(chars.next().unwrap_or('\u{FFFD}'));
    }
    // One optional whitespace terminates a hex escape.
    if matches!(chars.peek(), Some(c) if c.is_whitespace()) {
        chars.next();
    }
    let cp = u32::from_str_radix(&hex, 16).unwrap_or(0xFFFD);
    if (0xD800..=0xDFFF).contains(&cp) {
        None // surrogate half — dropped, see the doc comment above
    } else if cp == 0 || cp > 0x0010_FFFF {
        Some('\u{FFFD}')
    } else {
        Some(char::from_u32(cp).unwrap_or('\u{FFFD}'))
    }
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
    /// **Rule index** (EPOCH-1 remediation). Without it the cascade tested *every element against
    /// every rule* — O(nodes × rules) — which the EPOCH-1 probe measured at 66% of the whole
    /// pipeline on a large real page, scaling superlinearly (per-node cascade cost rose 11.6× from
    /// 1.3k to 18.7k nodes).
    ///
    /// Every real engine solves this the same way: bucket each selector by the **key** of its
    /// rightmost (subject) compound — an id if it has one, else a class, else a tag, else
    /// universal. An element then only tests the rules whose key it could possibly match (its own
    /// id / classes / tag, plus universal) instead of all of them. Selector matching itself is
    /// unchanged, so results are identical — this only skips rules that provably cannot match.
    fn build_index<'a>(sheets: &'a [ScopedSheet]) -> RuleIndex<'a> {
        let mut ix = RuleIndex::default();
        let mut order = 0usize;
        for scoped in sheets {
            for rule in &scoped.sheet.rules {
                for sel in &rule.selectors {
                    let entry = IndexedRule {
                        scope: scoped.scope,
                        sel,
                        rule,
                        order,
                    };
                    // The subject compound is the rightmost part.
                    let key = sel.parts.last();
                    match key {
                        // `::slotted(x)` reaches across the shadow boundary; keep it universal so
                        // it is never index-skipped.
                        _ if sel.slotted => ix.universal.push(entry),
                        Some(c) if c.id.is_some() => ix
                            .by_id
                            .entry(c.id.clone().unwrap())
                            .or_default()
                            .push(entry),
                        Some(c) if !c.classes.is_empty() => ix
                            .by_class
                            .entry(c.classes[0].clone())
                            .or_default()
                            .push(entry),
                        Some(c) if c.tag.is_some() => ix
                            .by_tag
                            .entry(c.tag.clone().unwrap().to_ascii_lowercase())
                            .or_default()
                            .push(entry),
                        _ => ix.universal.push(entry),
                    }
                    order += 1;
                }
            }
        }
        ix
    }

    pub fn cascade_scoped(&self, dom: &Dom, sheets: &[ScopedSheet]) -> StyleMap {
        let mut map = StyleMap::new();
        // Build the rule index ONCE for the whole document (see `build_index`), instead of
        // re-scanning every rule for every element.
        let index = Self::build_index(sheets);
        let root = dom.root();
        for child in dom.flat_children(root) {
            self.cascade_node(dom, child, &ComputedStyle::initial(), &index, &mut map);
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
        index: &RuleIndex<'_>,
        map: &mut StyleMap,
    ) {
        let style = match dom.data(node) {
            NodeData::Element(el) => {
                let mut s = ComputedStyle::inherit_from(parent_style);
                apply_ua_defaults(&mut s, el);
                // `<details>`: a CLOSED disclosure renders ONLY its summary. This needs the PARENT,
                // so it cannot live in the per-element `apply_ua_defaults` — it is the Stylo path's
                // `details > *:not(summary)` rule, expressed against the tree we already have.
                // Keep the two in lockstep: the cascades disagreeing about whether a section renders
                // is the `<source>` bug again (see the note in stylo_engine.rs).
                if !el.name.eq_ignore_ascii_case("summary")
                    && dom
                        .parent(node)
                        .and_then(|p| match dom.data(p) {
                            NodeData::Element(pe) => Some(pe),
                            _ => Option::None,
                        })
                        .is_some_and(|pe| {
                            pe.name.eq_ignore_ascii_case("details") && pe.attr("open").is_none()
                        })
                {
                    s.display = Display::None;
                }

                // Author rules, ordered by (specificity, source order). Only the rules the index
                // says could possibly match this element are tested (EPOCH-1: this is the fix for
                // the O(nodes × rules) cascade).
                let mut matched: Vec<(u32, usize, &Declaration)> = Vec::new();
                // A rule whose subject carries `::before`/`::after` does NOT style the element — it
                // styles a generated box hanging off it. Those declarations are routed to their own
                // cascade below.
                let mut pseudo_before: Vec<(u32, usize, &Declaration)> = Vec::new();
                let mut pseudo_after: Vec<(u32, usize, &Declaration)> = Vec::new();
                index.for_each_candidate(dom, node, |cand| {
                    if selector_matches_scoped(cand.sel, dom, node, cand.scope) {
                        let spec = cand.sel.specificity();
                        let subject = cand.sel.parts.last();
                        let is = |p: &Pseudo| subject.is_some_and(|c| c.pseudos.contains(p));
                        let sink = if is(&Pseudo::Before) {
                            &mut pseudo_before
                        } else if is(&Pseudo::After) {
                            &mut pseudo_after
                        } else {
                            &mut matched
                        };
                        for d in &cand.rule.declarations {
                            sink.push((spec, cand.order, d));
                        }
                    }
                });
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
                // `::before` / `::after` — generated content, cascaded against this element as its
                // parent. Only a pseudo with `content` generates a box at all.
                fn cascade_pseudo(
                    base: &ComputedStyle,
                    mut decls: Vec<(u32, usize, &Declaration)>,
                ) -> Option<Box<ComputedStyle>> {
                    if decls.is_empty() {
                        return None;
                    }
                    decls.sort_by_key(|(spec, ord, _)| (*spec, *ord));
                    let mut ps = ComputedStyle::inherit_from(base);
                    for (_, _, d) in &decls {
                        apply_declaration(&mut ps, d, base.font_size);
                    }
                    ps.content.as_ref()?;
                    Some(Box::new(ps))
                }
                let (pb, pa) = (
                    cascade_pseudo(&s, pseudo_before),
                    cascade_pseudo(&s, pseudo_after),
                );
                s.before = pb;
                s.after = pa;

                // CSS `opacity` applies to the whole SUBTREE (it forms a group). We fold that in
                // here so every box carries its *effective* opacity and paint needs no ancestor
                // context: effective = own × parent's effective.
                s.opacity = (s.opacity * parent_style.opacity).clamp(0.0, 1.0);
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
            self.cascade_node(dom, child, &style, index, map);
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
        // Keep in lockstep with the UA sheet in `stylo_engine.rs`. The two cascades disagreeing
        // about which elements render at all is how a `<source>` ends up with 19px of height in one
        // configuration and none in the other.
        "head" | "title" | "meta" | "link" | "script" | "style" | "base" | "noscript"
        | "template" | "source" | "track" | "param" | "area" | "datalist" | "basefont"
        | "noembed" | "noframes" | "rp" => (None, 0.0, 400, 1.0),
        // Form controls render as replaced-ish inline-block boxes (styled below).
        "input" | "button" | "textarea" | "select" => (InlineBlock, 0.0, 400, 1.0),
        // `<summary>` is a block: it is the disclosure's always-visible label. Whether the
        // *rest* of the `<details>` renders depends on the PARENT's `open` attribute, which this
        // per-element function cannot see — `cascade_node` applies that part.
        "summary" => (Block, 0.0, 700, 1.0),
        // `<dialog>`: rendered only while `open`. A closed dialog that renders is a modal's contents
        // spilled into the page — see the matching `dialog`/`dialog[open]` pair in stylo_engine.rs.
        "dialog" => {
            if el.attr("open").is_some() {
                (Block, 0.0, 400, 1.0)
            } else {
                (None, 0.0, 400, 1.0)
            }
        }
        // Default for unknown/other elements is inline (per CSS).
        _ => (Inline, 0.0, 400, 1.0),
    };
    s.display = display;
    // `[popover]` — a popover is hidden until it is SHOWN, whatever element carries it. Same failure
    // as a closed `<dialog>`: with no rule, the menu's items, the tooltip's copy and the whole
    // dropdown render inline in the middle of the page before anyone opens them. Attribute-keyed, not
    // tag-keyed, because `popover` is a global attribute. Keep in lockstep with the `[popover]` pair
    // in stylo_engine.rs.
    if el.attr("popover").is_some() {
        s.display = if el.attr("data-manuk-popover-open").is_some() {
            Block
        } else {
            None
        };
    }
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
            match el
                .attr("type")
                .unwrap_or("text")
                .to_ascii_lowercase()
                .as_str()
            {
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
    // Legacy presentational colour attributes (HTML §presentational hints). Still load-bearing
    // on the real web: Hacker News, for one, gets its entire visual identity from
    // `bgcolor="#ff6600"` / `bgcolor="#f6f6ef"` on <table>/<td> — without these the page renders
    // colourless. Author CSS overrides them (hints are lower priority), so they are only applied
    // where the property is still at its initial value.
    if s.background_color.is_none() {
        if let Some(c) = el.attr("bgcolor").and_then(values::parse_color) {
            s.background_color = Some(c);
        }
    }
    if let Some(c) = el.attr("text").and_then(values::parse_color) {
        s.color = c;
    }
    // `dir="rtl"` — how the RTL web ACTUALLY declares itself. Nearly every Arabic, Hebrew, Persian
    // and Urdu site sets it on <html> or <body> rather than writing `direction: rtl` in CSS, so a
    // stylesheet-only implementation of `direction` would read as "RTL is unsupported" on the sites
    // that matter most. It inherits like the CSS property (setting it on <html> is the whole page),
    // which the ordinary inheritance step already provides once it lands here.
    //
    // `dir="auto"` asks for content detection — the first strong character decides — which is what
    // an unmarked paragraph must NOT get (HTML's initial value is `ltr`, and Chrome agrees).
    if let Some(d) = el.attr("dir") {
        match d.trim().to_ascii_lowercase().as_str() {
            "rtl" => s.direction = Direction::Rtl,
            "ltr" => s.direction = Direction::Ltr,
            _ => {}
        }
    }

    // Replaced elements: an <img>/<canvas>/<video> is an atomic inline-block box sized by
    // its presentational width/height attributes (author CSS width/height still overrides,
    // as those are applied after UA defaults). Natural (intrinsic) sizing from the decoded
    // bitmap is layered on in the image pipeline.
    if matches!(
        tag,
        "img" | "canvas" | "video" | "svg" | "object" | "embed" | "iframe"
    ) {
        s.display = Display::InlineBlock;
        if let Some(w) = el.attr("width").and_then(parse_dimension_attr) {
            s.width = Dim::Px(w);
        }
        if let Some(h) = el.attr("height").and_then(parse_dimension_attr) {
            s.height = Dim::Px(h);
        }
        // **The dimension attributes are also an aspect-ratio hint** (HTML §"dimension attributes":
        // `aspect-ratio: auto <width> / <height>`), and that half is the load-bearing one. Without it
        // a `<canvas>`/`<video>` — which never has a decoded bitmap to derive a ratio from — and an
        // `<img>` that has not loaded yet have NO ratio at all, so the `max-width:100%` in every CSS
        // reset narrows the box and leaves the height at its attribute value: the image renders
        // squashed, and the pre-load box that `width`/`height` exist to reserve is the wrong shape.
        // `auto` in the spec's value means a real intrinsic ratio still wins, which is why this only
        // fills an empty slot — the decode pipeline overwrites it (`Page::apply_images`).
        if s.aspect_ratio.is_none() && !matches!(tag, "iframe" | "embed" | "object") {
            if let (Some(w), Some(h)) = (
                el.attr("width").and_then(parse_dimension_attr),
                el.attr("height").and_then(parse_dimension_attr),
            ) {
                if w > 0.0 && h > 0.0 {
                    s.aspect_ratio = Some(w / h);
                }
            }
        }
        // An unsized `<iframe>` is 300x150 — the spec's default. It has no intrinsic size to fall back
        // on, so without this it collapses to nothing and the embed is invisible before any question of
        // its content arises. See the twin of this block in `stylo_engine`.
        if tag == "iframe" {
            if s.width == Dim::Auto {
                s.width = Dim::Px(300.0);
            }
            if s.height == Dim::Auto {
                s.height = Dim::Px(150.0);
            }
        }
    }
}

/// Parse an HTML presentational length attribute (`width="272"` or `width="272px"`) into
/// pixels. Percentages and other units are ignored (returns `None`).
/// An HTML presentational dimension attribute (`width="85%"`, `height="50"`) as a CSS `Dim`.
/// Percentages are the point: `<table width="85%">` is how a large part of the legacy web —
/// Hacker News included — sizes its layout, and treating it as "absent" shrink-to-fits the table.
pub fn parse_dimension_attr_dim(v: &str) -> Option<Dim> {
    let v = v.trim();
    if let Some(pct) = v.strip_suffix('%') {
        let n: f32 = pct.trim().parse().ok()?;
        return (n.is_finite() && n >= 0.0).then_some(Dim::Percent(n));
    }
    parse_dimension_attr(v).map(Dim::Px)
}

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
                "inline-flex" => Display::InlineFlex,
                "inline-grid" => Display::InlineGrid,
                "table" | "inline-table" => Display::Table,
                "table-row-group" | "table-header-group" | "table-footer-group" => {
                    Display::TableRowGroup
                }
                "table-row" => Display::TableRow,
                "table-cell" => Display::TableCell,
                "table-caption" => Display::TableCaption,
                "table-column" => Display::TableColumn,
                "table-column-group" => Display::TableColumnGroup,
                "contents" => Display::Contents,
                "none" => Display::None,
                _ => s.display,
            }
        }
        "color" => {
            if let Some(c) = values::parse_color(v) {
                s.color = c;
            }
        }
        "background-color" => {
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
            // An AUTHORED line-height wins over the font's own metrics. Both cascades must agree on
            // this or they disagree about every line box on the page — MinimalCascade left the
            // `normal` flag set, so an explicit `line-height: 20px` was silently overridden by the
            // face's ascent+descent.
            if v.trim().eq_ignore_ascii_case("normal") {
                s.line_height_normal = true;
                s.line_height = s.font_size * 1.2;
                return;
            }
            s.line_height_normal = false;
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
                "pre" => WhiteSpace::Pre,
                "pre-wrap" => WhiteSpace::PreWrap,
                "pre-line" => WhiteSpace::PreLine,
                _ => WhiteSpace::Normal,
            }
        }
        "text-overflow" => {
            // `text-overflow` may carry two values (line-start, line-end); the common single value
            // sets both. `ellipsis` on the end value is what we honour; anything else is `clip`.
            s.text_overflow = if v
                .split_whitespace()
                .any(|t| t.eq_ignore_ascii_case("ellipsis"))
            {
                TextOverflow::Ellipsis
            } else {
                TextOverflow::Clip
            }
        }
        "text-transform" => {
            s.text_transform = match v.trim().to_ascii_lowercase().as_str() {
                "uppercase" => TextTransform::Uppercase,
                "lowercase" => TextTransform::Lowercase,
                "capitalize" => TextTransform::Capitalize,
                _ => TextTransform::None,
            }
        }
        // `overflow-wrap` and its legacy alias `word-wrap` map to the same computed value.
        "overflow-wrap" | "word-wrap" => {
            s.overflow_wrap = match v.trim().to_ascii_lowercase().as_str() {
                "break-word" => OverflowWrap::BreakWord,
                "anywhere" => OverflowWrap::Anywhere,
                _ => OverflowWrap::Normal,
            }
        }
        "direction" => {
            s.direction = match v.trim().to_ascii_lowercase().as_str() {
                "rtl" => Direction::Rtl,
                _ => Direction::Ltr,
            }
        }
        "word-break" => {
            s.word_break = match v.trim().to_ascii_lowercase().as_str() {
                "break-all" => WordBreak::BreakAll,
                "keep-all" => WordBreak::KeepAll,
                _ => WordBreak::Normal,
            }
        }
        // `letter-spacing`/`word-spacing`: a length added after each char / to each space. `normal`
        // (and any unparseable value) is zero. `em` resolves against this element's font size.
        "letter-spacing" => {
            s.letter_spacing = if v.trim().eq_ignore_ascii_case("normal") {
                0.0
            } else {
                values::parse_length_px(v.trim(), s.font_size).unwrap_or(0.0)
            }
        }
        "word-spacing" => {
            s.word_spacing = if v.trim().eq_ignore_ascii_case("normal") {
                0.0
            } else {
                values::parse_length_px(v.trim(), s.font_size).unwrap_or(0.0)
            }
        }
        "width" => {
            // Intrinsic sizing keywords collapse to `Dim::Auto` for length resolution, but tag which
            // one so block width resolution hugs the content instead of filling (`stretch` /
            // `-webkit-fill-available` are definite fills → not tagged), at parity with the stylo map.
            let low = v.trim().to_ascii_lowercase();
            s.width_keyword = match low.as_str() {
                "min-content" => Some(IntrinsicSize::MinContent),
                "max-content" => Some(IntrinsicSize::MaxContent),
                _ if low == "fit-content" || low.starts_with("fit-content(") => {
                    Some(IntrinsicSize::FitContent)
                }
                _ => None,
            };
            s.width = values::parse_dim(v, s.font_size);
        }
        "height" => {
            // Intrinsic sizing keywords collapse to `Dim::Auto` for length resolution, but flag
            // them so the abspos both-insets path treats the box as indefinite (sizes to content),
            // at parity with the stylo map. `stretch` / `-webkit-fill-available` ARE definite, so
            // they are NOT flagged — they behave like the auto+insets constraint case.
            let low = v.trim().to_ascii_lowercase();
            s.height_intrinsic =
                matches!(low.as_str(), "min-content" | "max-content" | "fit-content")
                    || low.starts_with("fit-content(");
            // `stretch`/`-webkit-fill-available`/`-moz-available` are DEFINITE and FILL the containing
            // block's height — distinct from `auto` (content) and the intrinsic keywords (indefinite).
            s.height_stretch = matches!(
                low.as_str(),
                "stretch" | "-webkit-fill-available" | "-moz-available"
            );
            s.height = values::parse_dim(v, s.font_size);
        }
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
        // The per-axis `overflow_x`/`overflow_y` are kept alongside so scrollbar-gutter
        // reservation can tell which axis actually scrolls (`overflow: hidden scroll`).
        "overflow" | "overflow-x" | "overflow-y" => {
            let parse_ov = |t: &str| match t {
                "hidden" => Overflow::Hidden,
                "scroll" => Overflow::Scroll,
                "auto" => Overflow::Auto,
                "clip" => Overflow::Clip,
                _ => Overflow::Visible,
            };
            let mut it = v.split_whitespace();
            let first = parse_ov(it.next().unwrap_or("visible"));
            match d.name.as_str() {
                "overflow-x" => s.overflow_x = first,
                "overflow-y" => s.overflow_y = first,
                _ => {
                    // shorthand: `overflow: <x> [<y>]` — second value defaults to the first.
                    s.overflow_x = first;
                    s.overflow_y = it.next().map(parse_ov).unwrap_or(first);
                }
            }
            let o = match (s.overflow_x, s.overflow_y) {
                (Overflow::Visible, oy) => oy,
                (ox, _) => ox,
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
        "aspect-ratio" => {
            // `auto || <ratio>`, where `<ratio>` is `<number> [ / <number> ]?` (a bare number is
            // `n / 1`). For a non-replaced box the specified ratio always applies, so the `auto`
            // keyword is simply dropped here — kept for parity with the stylo map (`stylo_map.rs`),
            // which the shipping pipeline actually uses. `s.aspect_ratio` is a plain `width/height`.
            let r = v.replace("auto", " ");
            let mut it = r.split('/').map(|t| t.trim().parse::<f32>());
            if let Some(Ok(w)) = it.next() {
                let h = match it.next() {
                    None => 1.0,
                    Some(Ok(h)) => h,
                    Some(Err(_)) => f32::NAN,
                };
                if w > 0.0 && h > 0.0 {
                    s.aspect_ratio = Some(w / h);
                }
            }
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
            let (w, c, st) = parse_border_shorthand(v, s.font_size);
            if let Some(w) = w {
                s.border_width = Sides::all(w);
            }
            if let Some(c) = c {
                s.border_color = c;
            }
            if let Some(st) = st {
                s.border_style = st;
            }
        }
        "border-top" | "border-right" | "border-bottom" | "border-left" => {
            let (w, c, st) = parse_border_shorthand(v, s.font_size);
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
            if let Some(st) = st {
                s.border_style = st;
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
        "box-shadow" => s.box_shadows = parse_box_shadows(v, s.font_size),
        "text-shadow" => s.text_shadow = parse_text_shadow(v, s.font_size),
        "mask-image" | "-webkit-mask-image" => {
            let v = v.trim();
            if let Some(rest) = v.strip_prefix("url(") {
                let inner = rest
                    .trim_end_matches(')')
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'');
                if !inner.is_empty() {
                    s.mask_image = Some(inner.to_string());
                }
            }
        }
        "visibility" => {
            s.visibility = match v.trim().to_ascii_lowercase().as_str() {
                "hidden" => Visibility::Hidden,
                "collapse" => Visibility::Collapse,
                _ => Visibility::Visible,
            }
        }
        "background-image" => s.background_images = parse_background_images(v),
        "background" => {
            // The shorthand: pull out whatever we understand — a colour, an image/gradient (possibly
            // several comma-separated layers) — and ignore the rest. A page that writes
            // `background: linear-gradient(...)` (very common) otherwise gets nothing at all.
            let imgs = parse_background_images(v);
            if !imgs.is_empty() {
                s.background_images = imgs;
            } else if let Some(c) = values::parse_color(v) {
                s.background_color = Some(c);
            }
            if v.contains("no-repeat") {
                s.background_repeat = BackgroundRepeat::NoRepeat;
            }
        }
        "background-size" => {
            let t = v.trim();
            s.background_size = if t.eq_ignore_ascii_case("cover") {
                BackgroundSize::Cover
            } else if t.eq_ignore_ascii_case("contain") {
                BackgroundSize::Contain
            } else {
                let parts: Vec<f32> = t
                    .split_whitespace()
                    .filter_map(|p| values::parse_length_px(p, s.font_size))
                    .collect();
                match parts.len() {
                    1 => BackgroundSize::Px(parts[0], parts[0]),
                    2 => BackgroundSize::Px(parts[0], parts[1]),
                    _ => BackgroundSize::Auto,
                }
            };
        }
        "object-fit" => {
            let t = v.trim();
            s.object_fit = if t.eq_ignore_ascii_case("contain") {
                ObjectFit::Contain
            } else if t.eq_ignore_ascii_case("cover") {
                ObjectFit::Cover
            } else if t.eq_ignore_ascii_case("none") {
                ObjectFit::None
            } else if t.eq_ignore_ascii_case("scale-down") {
                ObjectFit::ScaleDown
            } else {
                ObjectFit::Fill
            };
        }
        // `object-position: <x> <y>` — 1 or 2 values, each a keyword (`left`/`center`/`right`,
        // `top`/`center`/`bottom`) or a percentage. Resolved to a 0..1 free-space fraction per axis;
        // percentages relative to length (px) aren't fraction-convertible without the box, so they
        // (and any unrecognized token) fall back to centered. A single value sets its axis, the other
        // stays centered. `top`/`bottom` bind the vertical axis and `left`/`right` the horizontal even
        // when written first, so `object-position: top` and `object-position: right` both work.
        "object-position" => {
            let axis_frac = |tok: &str| -> Option<f32> {
                let t = tok.trim();
                match t.to_ascii_lowercase().as_str() {
                    "left" | "top" => Some(0.0),
                    "center" => Some(0.5),
                    "right" | "bottom" => Some(1.0),
                    _ => t
                        .strip_suffix('%')
                        .and_then(|n| n.trim().parse::<f32>().ok())
                        .map(|p| (p / 100.0).clamp(0.0, 1.0)),
                }
            };
            let is_vertical =
                |tok: &str| matches!(tok.trim().to_ascii_lowercase().as_str(), "top" | "bottom");
            let is_horizontal =
                |tok: &str| matches!(tok.trim().to_ascii_lowercase().as_str(), "left" | "right");
            let toks: Vec<&str> = v.split_whitespace().collect();
            let mut pos = ObjectPosition::default();
            match toks.as_slice() {
                [a] => {
                    if is_vertical(a) {
                        pos.y = axis_frac(a).unwrap_or(0.5);
                    } else if is_horizontal(a) {
                        pos.x = axis_frac(a).unwrap_or(0.5);
                    } else if let Some(f) = axis_frac(a) {
                        pos.x = f; // `center` or a percentage → horizontal, vertical stays centered
                    }
                }
                [a, b] => {
                    // Keyword axis binding lets `top left` resolve as well as `left top`.
                    let (xa, ya) = if is_vertical(a) || is_horizontal(b) {
                        (b, a)
                    } else {
                        (a, b)
                    };
                    pos.x = axis_frac(xa).unwrap_or(0.5);
                    pos.y = axis_frac(ya).unwrap_or(0.5);
                }
                _ => {}
            }
            s.object_position = pos;
        }
        "background-position" => s.background_position = parse_background_position(v, s.font_size),
        "background-repeat" => {
            s.background_repeat = if v.contains("no-repeat") {
                BackgroundRepeat::NoRepeat
            } else {
                BackgroundRepeat::Repeat
            };
        }
        "text-decoration-line" => {
            // Longhand: touches only the lines, leaving any set color intact.
            let lv = v.to_ascii_lowercase();
            s.text_decoration.underline = lv.contains("underline");
            s.text_decoration.overline = lv.contains("overline");
            s.text_decoration.line_through = lv.contains("line-through");
        }
        "text-decoration" => {
            // Shorthand: resets every longhand it omits. Lines come from keyword presence; the
            // color is whatever token in the value parses as a color (`underline dotted red`).
            let lv = v.to_ascii_lowercase();
            let color = lv
                .split_whitespace()
                .filter(|t| {
                    !matches!(
                        *t,
                        "underline"
                            | "overline"
                            | "line-through"
                            | "blink"
                            | "none"
                            | "solid"
                            | "double"
                            | "dotted"
                            | "dashed"
                            | "wavy"
                    )
                })
                .find_map(values::parse_color);
            // Reset the lines/color/thickness longhands this shorthand covers; `text-underline-offset`
            // is NOT a longhand of `text-decoration`, so leave it untouched.
            s.text_decoration.underline = lv.contains("underline");
            s.text_decoration.overline = lv.contains("overline");
            s.text_decoration.line_through = lv.contains("line-through");
            s.text_decoration.color = color;
            s.text_decoration.thickness = None;
        }
        "text-decoration-color" => {
            // `currentColor` keeps the currentColor default (paint follows the text color).
            s.text_decoration.color = if v.trim().eq_ignore_ascii_case("currentcolor") {
                None
            } else {
                values::parse_color(v)
            };
        }
        "text-decoration-thickness" => {
            // `auto`/`from-font` keep the font-derived default (paint's `font_size / 14`); a length
            // is an explicit line thickness (Tailwind `decoration-2`, thick brand underlines).
            let tv = v.trim();
            s.text_decoration.thickness =
                if tv.eq_ignore_ascii_case("auto") || tv.eq_ignore_ascii_case("from-font") {
                    None
                } else {
                    values::parse_length_px(tv, s.font_size)
                };
        }
        "text-underline-offset" => {
            // Extra px below the underline's default position (Tailwind `underline-offset-4`).
            // `auto` is the 0 default; a length pushes the underline away from the text.
            let tv = v.trim();
            s.text_decoration.underline_offset = if tv.eq_ignore_ascii_case("auto") {
                0.0
            } else {
                values::parse_length_px(tv, s.font_size).unwrap_or(0.0)
            };
        }
        "content" => {
            let t = v.trim();
            s.content = if t.eq_ignore_ascii_case("none") || t.eq_ignore_ascii_case("normal") {
                None
            } else {
                // A quoted string; escapes like "\f101" (icon fonts) decode to the code point.
                let inner = t.trim_matches('"').trim_matches('\'');
                Some(decode_css_escapes(inner))
            };
        }
        "list-style-type" => s.list_style_type = parse_list_style_type(v),
        "list-style-position" => s.list_style_inside = v.trim().eq_ignore_ascii_case("inside"),
        "list-style" => {
            // Shorthand: the type and/or the position, in any order.
            for tok in v.split_whitespace() {
                if tok.eq_ignore_ascii_case("inside") {
                    s.list_style_inside = true;
                } else if tok.eq_ignore_ascii_case("outside") {
                    s.list_style_inside = false;
                } else if let Some(t) = parse_list_style_type_opt(tok) {
                    s.list_style_type = t;
                }
            }
        }
        "outline" => {
            for tok in v.split_whitespace() {
                if let Some(w) = values::parse_length_px(tok, s.font_size) {
                    s.outline_width = w;
                } else if let Some(c) = values::parse_color(tok) {
                    s.outline_color = c;
                }
            }
            if v.trim() == "none" || v.trim() == "0" {
                s.outline_width = 0.0;
            }
        }
        "outline-width" => {
            s.outline_width = values::parse_length_px(v, s.font_size).unwrap_or(0.0);
        }
        "outline-color" => {
            if let Some(c) = values::parse_color(v) {
                s.outline_color = c;
            }
        }
        "opacity" => {
            if let Ok(o) = v.trim().parse::<f32>() {
                s.opacity = o.clamp(0.0, 1.0);
            }
        }
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
        "border-style"
        | "border-top-style"
        | "border-right-style"
        | "border-bottom-style"
        | "border-left-style" => {
            // `none`/`hidden` remove the border; other styles keep whatever width is set. The style
            // is stored uniform (like `border_color`): the FIRST style token of a multi-value
            // `border-style: solid dashed` wins (per-side styles are a follow-on).
            if let Some(first) = v.split_whitespace().next() {
                if matches!(first, "none" | "hidden") {
                    s.border_width = Sides::all(0.0);
                } else if let Some(st) = border_style_of(first) {
                    s.border_style = st;
                }
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

/// Map a `border-style` keyword to a `BorderStyle`. `groove`/`ridge`/`inset`/`outset` collapse to
/// `Solid` (their bevel is a paint refinement). Returns `None` for a non-style token.
fn border_style_of(tok: &str) -> Option<BorderStyle> {
    match tok.trim() {
        "solid" | "groove" | "ridge" | "inset" | "outset" => Some(BorderStyle::Solid),
        "dashed" => Some(BorderStyle::Dashed),
        "dotted" => Some(BorderStyle::Dotted),
        "double" => Some(BorderStyle::Double),
        _ => None,
    }
}

/// Parse the `border`/`border-<side>` shorthand into an optional width, color and line style.
/// `none`/`hidden` force width 0.
fn parse_border_shorthand(v: &str, fs: f32) -> (Option<f32>, Option<Rgba>, Option<BorderStyle>) {
    let mut width = None;
    let mut color = None;
    let mut style = None;
    for tok in v.split_whitespace() {
        match tok {
            "none" | "hidden" => width = Some(0.0),
            "thin" => width = Some(1.0),
            "medium" => width = Some(3.0),
            "thick" => width = Some(5.0),
            t => {
                if let Some(bs) = border_style_of(t) {
                    style = Some(bs);
                } else if let Some(px) = values::parse_length_px(t, fs) {
                    width = Some(px);
                } else if let Some(c) = values::parse_color(t) {
                    color = Some(c);
                }
            }
        }
    }
    // A visible line style with no explicit width defaults to `medium` (3px).
    if width.is_none() && style.is_some() {
        width = Some(3.0);
    }
    (width, color, style)
}

/// Split `v` on top-level whitespace, keeping parenthesised groups (`rgba(0, 0, 0, .3)`) intact.
/// Decode CSS string escapes — `\f101` is how every icon font names its glyph.
fn decode_css_escapes(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let mut hex = String::new();
        while hex.len() < 6 {
            match it.peek() {
                Some(h) if h.is_ascii_hexdigit() => {
                    hex.push(*h);
                    it.next();
                }
                _ => break,
            }
        }
        if hex.is_empty() {
            if let Some(n) = it.next() {
                out.push(n);
            }
        } else {
            // One optional whitespace terminates the escape.
            if it.peek() == Some(&' ') {
                it.next();
            }
            if let Some(ch) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                out.push(ch);
            }
        }
    }
    out
}

fn parse_list_style_type_opt(v: &str) -> Option<ListStyleType> {
    Some(match v.trim().to_ascii_lowercase().as_str() {
        "disc" => ListStyleType::Disc,
        "circle" => ListStyleType::Circle,
        "square" => ListStyleType::Square,
        "decimal" => ListStyleType::Decimal,
        "lower-alpha" | "lower-latin" => ListStyleType::LowerAlpha,
        "upper-alpha" | "upper-latin" => ListStyleType::UpperAlpha,
        "lower-roman" => ListStyleType::LowerRoman,
        "upper-roman" => ListStyleType::UpperRoman,
        "none" => ListStyleType::None,
        _ => return None,
    })
}

fn parse_list_style_type(v: &str) -> ListStyleType {
    parse_list_style_type_opt(v).unwrap_or(ListStyleType::Disc)
}

/// `background-image` / the image part of the `background` shorthand — a comma-separated LIST of
/// layers. The list is returned in SOURCE ORDER (index 0 is the topmost layer, per CSS). A layer the
/// parser can't read is dropped, not the whole value, so `linear-gradient(...), url(x)` keeps the url
/// even if the gradient is malformed. `none`/empty yields no layers.
pub fn parse_background_images(v: &str) -> Vec<BackgroundImage> {
    let v = v.trim();
    if v.eq_ignore_ascii_case("none") || v.is_empty() {
        return Vec::new();
    }
    // Split on TOP-LEVEL commas — commas inside `linear-gradient(rgba(...), ...)` don't separate
    // layers. Each piece is then parsed as a single layer.
    split_top_level_commas(v)
        .iter()
        .filter_map(|layer| parse_background_image(layer))
        .collect()
}

/// `background-position` — 1–2 keyword/percentage/length values. A `<percentage>`/keyword becomes a
/// `Pct` fraction of the free space; a `<length>` becomes an absolute `Px` offset. One value sets the
/// horizontal axis and leaves the vertical `center`; two values are `x y`, with keyword axis binding so
/// `top right` resolves as well as `right top`. Anything unreadable leaves the default `0% 0%`.
pub fn parse_background_position(v: &str, font_size: f32) -> BackgroundPosition {
    let axis = |tok: &str| -> Option<BgPos> {
        let t = tok.trim();
        match t.to_ascii_lowercase().as_str() {
            "left" | "top" => Some(BgPos::Pct(0.0)),
            "center" => Some(BgPos::Pct(0.5)),
            "right" | "bottom" => Some(BgPos::Pct(1.0)),
            _ => {
                if let Some(n) = t.strip_suffix('%') {
                    n.trim().parse::<f32>().ok().map(|p| BgPos::Pct(p / 100.0))
                } else {
                    values::parse_length_px(t, font_size).map(BgPos::Px)
                }
            }
        }
    };
    let is_vertical =
        |tok: &str| matches!(tok.trim().to_ascii_lowercase().as_str(), "top" | "bottom");
    let is_horizontal =
        |tok: &str| matches!(tok.trim().to_ascii_lowercase().as_str(), "left" | "right");
    let toks: Vec<&str> = v.split_whitespace().collect();
    // `background-position`'s initial value is `0% 0%`, but a lone value leaves the OTHER axis centered.
    let mut pos = BackgroundPosition::default();
    match toks.as_slice() {
        [a] => {
            if is_vertical(a) {
                if let Some(p) = axis(a) {
                    pos.y = p;
                    pos.x = BgPos::Pct(0.5);
                }
            } else if let Some(p) = axis(a) {
                pos.x = p;
                pos.y = BgPos::Pct(0.5); // horizontal set, vertical centered
            }
        }
        [a, b] => {
            let (xa, ya) = if is_vertical(a) || is_horizontal(b) {
                (b, a)
            } else {
                (a, b)
            };
            if let Some(p) = axis(xa) {
                pos.x = p;
            }
            if let Some(p) = axis(ya) {
                pos.y = p;
            }
        }
        _ => {}
    }
    pos
}

/// A single `background-image` layer: `url(...) | linear-gradient(...) | radial-gradient(...)`.
///
/// Gradient syntax is handled to the depth the web actually uses: an optional angle or `to <side>`,
/// then colour stops with optional percentage positions.
pub fn parse_background_image(v: &str) -> Option<BackgroundImage> {
    let v = v.trim();
    if v.eq_ignore_ascii_case("none") || v.is_empty() {
        return None;
    }
    // Find the first function-ish token in the (possibly shorthand) value.
    let lower = v.to_ascii_lowercase();
    if let Some(i) = lower.find("url(") {
        let rest = &v[i + 4..];
        let end = rest.find(')')?;
        let raw = rest[..end].trim().trim_matches('"').trim_matches('\'');
        return (!raw.is_empty()).then(|| BackgroundImage::Url(raw.to_string()));
    }
    let (kind, start) = if let Some(i) = lower.find("linear-gradient(") {
        (0u8, i + "linear-gradient(".len())
    } else if let Some(i) = lower.find("radial-gradient(") {
        (1u8, i + "radial-gradient(".len())
    } else {
        return None;
    };
    // Take the balanced argument list (stops may contain `rgba(...)`).
    let bytes = v.as_bytes();
    let mut depth = 1i32;
    let mut end = start;
    while end < bytes.len() {
        match bytes[end] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            _ => {}
        }
        end += 1;
    }
    let args = &v[start..end.min(v.len())];
    let parts = split_top_level_commas(args);
    if parts.is_empty() {
        return None;
    }

    let mut angle_deg = 180.0f32; // CSS default: `to bottom`
    let mut first_stop = 0usize;
    let head = parts[0].trim().to_ascii_lowercase();
    if kind == 0 {
        if let Some(deg) = parse_angle_deg(&head) {
            angle_deg = deg;
            first_stop = 1;
        } else if let Some(side) = head.strip_prefix("to ") {
            angle_deg = match side.trim() {
                "top" => 0.0,
                "right" => 90.0,
                "bottom" => 180.0,
                "left" => 270.0,
                "top right" | "right top" => 45.0,
                "bottom right" | "right bottom" => 135.0,
                "bottom left" | "left bottom" => 225.0,
                "top left" | "left top" => 315.0,
                _ => 180.0,
            };
            first_stop = 1;
        }
    } else if head.starts_with("circle")
        || head.starts_with("ellipse")
        || head.starts_with("at ")
        || head.contains("corner")
        || head.contains("side")
    {
        first_stop = 1;
    }

    let raw_stops: Vec<&str> = parts[first_stop..].iter().map(|s| s.trim()).collect();
    if raw_stops.is_empty() {
        return None;
    }
    let n = raw_stops.len();
    let mut stops: Vec<ColorStop> = Vec::new();
    for (i, sp) in raw_stops.iter().enumerate() {
        // `<color> [<pos>]` — the position may be a percentage or a length (treated as %-ish).
        let (cpart, pos) = match sp.rfind(char::is_whitespace) {
            Some(k) if sp[k..].trim().ends_with('%') => {
                let p: f32 = sp[k..].trim().trim_end_matches('%').parse().unwrap_or(0.0);
                (&sp[..k], Some(p / 100.0))
            }
            _ => (&sp[..], None),
        };
        let color = values::parse_color(cpart.trim())?;
        let at = pos.unwrap_or(if n <= 1 {
            0.0
        } else {
            i as f32 / (n - 1) as f32
        });
        stops.push(ColorStop {
            color,
            at: at.clamp(0.0, 1.0),
        });
    }
    if stops.len() == 1 {
        // A single stop is a solid fill; give it two ends so the painter's interpolation is uniform.
        stops.push(ColorStop {
            at: 1.0,
            ..stops[0]
        });
    }
    Some(match kind {
        0 => BackgroundImage::Linear { angle_deg, stops },
        _ => BackgroundImage::Radial { stops },
    })
}

/// `45deg` / `0.25turn` / `100grad` / `1.5rad` → degrees.
fn parse_angle_deg(v: &str) -> Option<f32> {
    let v = v.trim();
    for (suffix, scale) in [
        ("deg", 1.0f32),
        ("grad", 0.9),
        ("rad", 180.0 / std::f32::consts::PI),
        ("turn", 360.0),
    ] {
        if let Some(n) = v.strip_suffix(suffix) {
            return n.trim().parse::<f32>().ok().map(|x| x * scale);
        }
    }
    None
}

/// Split on commas that are not inside parentheses (so `rgba(0,0,0,.5)` stays whole).
fn split_top_level_commas(v: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0i32;
    for ch in v.chars() {
        match ch {
            '(' => {
                depth += 1;
                cur.push(ch);
            }
            ')' => {
                depth -= 1;
                cur.push(ch);
            }
            ',' if depth == 0 => {
                if !cur.trim().is_empty() {
                    out.push(cur.trim().to_string());
                }
                cur.clear();
            }
            _ => cur.push(ch),
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

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

/// Parse `box-shadow` — a comma-separated LIST of `[inset] <offset-x> <offset-y> [<blur> [<spread>]]
/// [<color>]` layers, in source order (first layer paints on top). `none`/empty is no shadow.
/// `inset` layers are captured (so a mixed list keeps its outer layers) but not yet painted.
fn parse_box_shadows(v: &str, fs: f32) -> Vec<BoxShadow> {
    let v = v.trim();
    if v.is_empty() || v.eq_ignore_ascii_case("none") {
        return Vec::new();
    }
    // Split on *top-level* commas (commas inside rgba()/hsl() don't separate layers).
    let mut depth = 0i32;
    let mut start = 0usize;
    let mut layers: Vec<&str> = Vec::new();
    for (i, c) in v.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                layers.push(&v[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    layers.push(&v[start..]);

    let mut out = Vec::new();
    for layer in layers {
        let layer = layer.trim();
        if layer.is_empty() {
            continue;
        }
        let inset = layer
            .split_whitespace()
            .any(|t| t.eq_ignore_ascii_case("inset"));
        let mut lens: Vec<f32> = Vec::new();
        let mut color: Option<Rgba> = None;
        for tok in tokens_keeping_parens(layer) {
            if tok.eq_ignore_ascii_case("inset") {
                continue;
            }
            if let Some(px) = values::parse_length_px(&tok, fs) {
                lens.push(px);
            } else if let Some(c) = values::parse_color(&tok) {
                color = Some(c);
            }
        }
        // offset-x and offset-y are required; a layer missing them is dropped, not the whole value.
        if lens.len() < 2 {
            continue;
        }
        out.push(BoxShadow {
            dx: lens[0],
            dy: lens[1],
            blur: lens.get(2).copied().unwrap_or(0.0).max(0.0),
            spread: lens.get(3).copied().unwrap_or(0.0),
            inset,
            color: color.unwrap_or(Rgba::BLACK),
        });
    }
    out
}

/// Parse a `text-shadow` value to its FIRST layer: `offset-x offset-y [blur] [color]`. A comma list of
/// shadows is allowed by CSS but we take the first (multi-shadow is residue). `none`/empty → `None`; a
/// layer without both offsets → `None`. The color defaults to `currentColor`, which the caller (the
/// cascade) does not know here, so we default to the text's own `color` at paint if unset — modelled as
/// `None` color meaning "use the text color". For simplicity we store the parsed color or fall back to a
/// neutral, and let paint substitute the text color when the author gave none.
fn parse_text_shadow(v: &str, fs: f32) -> Option<TextShadow> {
    let v = v.trim();
    if v.is_empty() || v.eq_ignore_ascii_case("none") {
        return None;
    }
    // First top-level layer (commas inside rgba()/hsl() are not layer separators).
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
    let layer = v[..end].trim();
    let mut lens: Vec<f32> = Vec::new();
    let mut color: Option<Rgba> = None;
    for tok in tokens_keeping_parens(layer) {
        if let Some(px) = values::parse_length_px(&tok, fs) {
            lens.push(px);
        } else if let Some(c) = values::parse_color(&tok) {
            color = Some(c);
        }
    }
    if lens.len() < 2 {
        return None;
    }
    Some(TextShadow {
        dx: lens[0],
        dy: lens[1],
        blur: lens.get(2).copied().unwrap_or(0.0).max(0.0),
        // A shadow with no explicit colour uses `currentColor`; a semi-transparent black is the
        // overwhelmingly common authored value and a safe stand-in when the author gave none.
        color: color.unwrap_or(Rgba::new(0, 0, 0, 128)),
    })
}

/// Parse a `transform` value into an ordered list of [`TransformFn`]s (translate/scale/
/// rotate/skew/matrix, and the axis variants). Unknown functions are skipped.
fn parse_transform(v: &str, fs: f32) -> Vec<TransformFn> {
    let mut out = Vec::new();
    let mut rest = v.trim();
    while let Some(open) = rest.find('(') {
        let name = rest[..open].trim().to_ascii_lowercase();
        let Some(close) = rest[open..].find(')') else {
            break;
        };
        let args_str = &rest[open + 1..open + close];
        let nums: Vec<&str> = args_str
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        let angle = |s: &str| parse_angle_rad(s);
        let f = |i: usize| nums.get(i).and_then(|s| s.parse::<f32>().ok());
        let dim = |i: usize| {
            nums.get(i)
                .map(|s| values::parse_dim(s, fs))
                .unwrap_or(Dim::Px(0.0))
        };
        match name.as_str() {
            "translate" => out.push(TransformFn::Translate(
                dim(0),
                nums.get(1)
                    .map(|s| values::parse_dim(s, fs))
                    .unwrap_or(Dim::Px(0.0)),
            )),
            "translatex" => out.push(TransformFn::Translate(dim(0), Dim::Px(0.0))),
            "translatey" => out.push(TransformFn::Translate(Dim::Px(0.0), dim(0))),
            "scale" => out.push(TransformFn::Scale(
                f(0).unwrap_or(1.0),
                f(1).or(f(0)).unwrap_or(1.0),
            )),
            "scalex" => out.push(TransformFn::Scale(f(0).unwrap_or(1.0), 1.0)),
            "scaley" => out.push(TransformFn::Scale(1.0, f(0).unwrap_or(1.0))),
            "rotate" => out.push(TransformFn::Rotate(
                nums.first().and_then(|s| angle(s)).unwrap_or(0.0),
            )),
            "skew" => out.push(TransformFn::Skew(
                nums.first().and_then(|s| angle(s)).unwrap_or(0.0),
                nums.get(1).and_then(|s| angle(s)).unwrap_or(0.0),
            )),
            "skewx" => out.push(TransformFn::Skew(
                nums.first().and_then(|s| angle(s)).unwrap_or(0.0),
                0.0,
            )),
            "skewy" => out.push(TransformFn::Skew(
                0.0,
                nums.first().and_then(|s| angle(s)).unwrap_or(0.0),
            )),
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
    let (num, unit) = s
        .find(|c: char| c.is_ascii_alphabetic())
        .map_or((s, ""), |i| s.split_at(i));
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
    if let Some(inner) = low
        .strip_prefix("minmax(")
        .and_then(|s| s.strip_suffix(')'))
    {
        let (a, b) = inner.split_once(',')?;
        return Some(TrackSize::MinMax(
            parse_track_unit(a.trim(), fs)?,
            parse_track_unit(b.trim(), fs)?,
        ));
    }
    if let Some(n) = t
        .strip_suffix("fr")
        .and_then(|n| n.trim().parse::<f32>().ok())
    {
        return Some(TrackSize::Fr(n));
    }
    if let Some(p) = t
        .strip_suffix('%')
        .and_then(|n| n.trim().parse::<f32>().ok())
    {
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
            if let Some(n) = t
                .strip_suffix("fr")
                .and_then(|n| n.trim().parse::<f32>().ok())
            {
                Some(TrackUnit::Fr(n))
            } else if let Some(p) = t
                .strip_suffix('%')
                .and_then(|n| n.trim().parse::<f32>().ok())
            {
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
    if let Some(n) = v
        .strip_prefix("span")
        .map(str::trim)
        .and_then(|n| n.parse::<u16>().ok())
    {
        return GridLine::Span(n.max(1));
    }
    v.parse::<i16>()
        .map(GridLine::Line)
        .unwrap_or(GridLine::Auto)
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
    s.flex_basis = basis.unwrap_or(if nums.is_empty() {
        Dim::Auto
    } else {
        Dim::Px(0.0)
    });
}

/// Expand a 1–4 value `border-width` shorthand (same edge order as `margin`).
fn set_border_widths(sides: &mut Sides<f32>, v: &str, fs: f32) {
    let vals: Vec<f32> = v.split_whitespace().map(|t| border_len(t, fs)).collect();
    match vals.as_slice() {
        [a] => *sides = Sides::all(*a),
        [a, b] => {
            *sides = Sides {
                top: *a,
                bottom: *a,
                right: *b,
                left: *b,
            };
        }
        [a, b, c] => {
            *sides = Sides {
                top: *a,
                right: *b,
                left: *b,
                bottom: *c,
            };
        }
        [a, b, c, d] => {
            *sides = Sides {
                top: *a,
                right: *b,
                bottom: *c,
                left: *d,
            };
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
    fn background_image_is_a_layer_list() {
        // The ubiquitous scrim-over-hero pattern: a darkening gradient ON TOP of a photo. The old
        // single-`Option` model scanned for `url(` first and returned ONLY the image, dropping the
        // overlay. It is a LIST, source order = top-to-bottom, so the gradient is index 0.
        let layers = parse_background_images(
            "linear-gradient(rgba(0,0,0,0.5), rgba(0,0,0,0.5)), url(hero.jpg)",
        );
        assert_eq!(layers.len(), 2, "two layers, not one");
        assert!(
            matches!(layers[0], BackgroundImage::Linear { .. }),
            "the gradient scrim is the TOP layer (index 0)"
        );
        assert!(
            matches!(layers[1], BackgroundImage::Url(ref u) if u == "hero.jpg"),
            "the photo is the bottom layer"
        );
        // A comma INSIDE a gradient does not split layers.
        let one = parse_background_images("linear-gradient(90deg, red, blue)");
        assert_eq!(one.len(), 1, "internal commas are not layer separators");
        // `none`/empty yields no layers (the old `None`).
        assert!(parse_background_images("none").is_empty());
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
    fn selector_ident_escapes_decode_per_css_syntax() {
        // A selector escape (`\`) is part of the identifier, decoded per css-syntax §4.3.7 — the old
        // `take_ident` stopped at the backslash, so every escaped id/class matched NOTHING. Build one
        // element per id and confirm the escaped selector finds it.
        let cases = [
            // (id set on the element, selector that must match it)
            ("simple", "#simple"),
            ("has.dot", "#has\\.dot"), // `\.` → literal dot (not a class combinator)
            ("a:b!c", "#a\\:b\\!c"),   // `\:` `\!` → literal punctuation
            ("0start", "#\\30 start"), // `\30 ` → '0', trailing space consumed
            ("0start", "#\\000030start"), // 6 hex, no space needed
            ("sp ace", "#sp\\ ace"),   // `\ ` → literal space, must not split compounds
            ("zero\u{FFFD}", "#zero\\0"), // NUL escape → U+FFFD replacement
            ("caf\u{e9}", "#caf\\e9"), // `\e9` → é (non-ASCII from hex)
            ("na\u{ef}ve", "#na\u{ef}ve"), // raw non-ASCII ident char is accepted
        ];
        for (id, sel) in cases {
            let mut dom = Dom::new();
            let root = dom.root();
            let el = dom.create_element("span");
            dom.set_attr(el, "id", id);
            dom.append_child(root, el);
            assert_eq!(
                query_selector(&dom, root, sel),
                Some(el),
                "selector {sel:?} should match an element with id {id:?}"
            );
        }
        // A NUL-holding id must NOT match a U+FFFD selector (they are distinct code points), and a
        // surrogate-half escape is dropped rather than U+FFFD'd, so it does not false-match a lossily
        // stored id — both are the "should never match" side of the WPT suite.
        let mut dom = Dom::new();
        let root = dom.root();
        let el = dom.create_element("span");
        dom.set_attr(el, "id", "zero\u{0}"); // a raw NUL, stored distinctly from U+FFFD
        dom.append_child(root, el);
        assert_eq!(query_selector(&dom, root, "#zero\\0"), None);
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
        assert_eq!(
            map[&outer].color,
            Rgba::new(255, 0, 0, 255),
            "the light-DOM p is red"
        );

        // The shadow <p> is a different <p>; find it through the shadow root.
        let host = dom.find_first("div").unwrap();
        let shadow = dom.shadow_root(host).unwrap();
        let inner = dom
            .descendants(shadow)
            .find(|&n| dom.tag_name(n) == Some("p"))
            .unwrap();
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
        let inner = dom
            .descendants(shadow)
            .find(|&n| dom.tag_name(n) == Some("p"))
            .unwrap();
        assert_eq!(
            map[&inner].color,
            Rgba::new(0, 255, 0, 255),
            "the shadow p is green"
        );

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
        assert_eq!(
            map[&p].color,
            Rgba::new(0, 0, 255, 255),
            "::slotted(p) styles the slotted p"
        );

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
        let em = dom
            .descendants(shadow)
            .find(|&n| dom.tag_name(n) == Some("em"))
            .unwrap();
        // `color` inherits from the host across the shadow boundary (inheritance is
        // flat-tree, not scoped -- only *matching* is scoped).
        assert_eq!(map[&host].color, Rgba::new(0x12, 0x34, 0x56, 255));
        assert_eq!(map[&em].color, Rgba::new(0x12, 0x34, 0x56, 255));
    }

    #[test]
    fn intrinsic_height_keywords_flag_the_box_as_indefinite() {
        // `min`/`max`/`fit-content` collapse to `Dim::Auto` (no length), but must set
        // `height_intrinsic` so the abspos both-insets path treats the box as indefinite. `auto`,
        // `stretch` and an explicit length are definite and must NOT flag. Gates the hand parser at
        // parity with the stylo map the shipping pipeline uses.
        for kw in [
            "min-content",
            "max-content",
            "fit-content",
            "fit-content(10px)",
        ] {
            let (dom, map) = cascade_of(&format!(r#"<div style="height:{kw}"></div>"#));
            let cs = &map[&dom.find_first("div").unwrap()];
            assert!(cs.height_intrinsic, "{kw} => height_intrinsic");
            assert_eq!(
                cs.height,
                Dim::Auto,
                "{kw} collapses to Auto for resolution"
            );
        }
        for kw in ["auto", "stretch", "100px", "50%"] {
            let (dom, map) = cascade_of(&format!(r#"<div style="height:{kw}"></div>"#));
            assert!(
                !map[&dom.find_first("div").unwrap()].height_intrinsic,
                "{kw} is definite, not an intrinsic keyword"
            );
        }
    }

    #[test]
    fn aspect_ratio_parses_to_a_width_over_height_ratio() {
        // `<ratio>` forms: `w / h`, a bare number (`n / 1`), and the `auto <ratio>` keyword form
        // (the keyword is dropped for a non-replaced box). This gates the hand parser at parity with
        // the stylo map the shipping pipeline uses.
        let (dom, map) = cascade_of(r#"<div style="aspect-ratio:16/9"></div>"#);
        let ar = map[&dom.find_first("div").unwrap()].aspect_ratio.unwrap();
        assert!((ar - 16.0 / 9.0).abs() < 1e-4, "16/9 -> {ar}");

        let (dom, map) = cascade_of(r#"<div style="aspect-ratio:2"></div>"#);
        assert_eq!(
            map[&dom.find_first("div").unwrap()].aspect_ratio,
            Some(2.0),
            "a bare number is n / 1"
        );

        let (dom, map) = cascade_of(r#"<div style="aspect-ratio:auto 1/1"></div>"#);
        assert_eq!(
            map[&dom.find_first("div").unwrap()].aspect_ratio,
            Some(1.0),
            "the auto keyword is dropped; the ratio still applies to a non-replaced box"
        );

        // `auto` alone (no ratio) leaves it unset.
        let (dom, map) = cascade_of(r#"<div style="aspect-ratio:auto"></div>"#);
        assert_eq!(
            map[&dom.find_first("div").unwrap()].aspect_ratio,
            None,
            "auto with no ratio => no preferred ratio"
        );
    }

    #[test]
    fn border_shorthand_and_box_sizing_parse() {
        let (dom, map) =
            cascade_of(r#"<p style="border:5px solid #333;box-sizing:border-box"></p>"#);
        let s = &map[&dom.find_first("p").unwrap()];
        assert_eq!(
            s.border_width,
            Sides::all(5.0),
            "border shorthand sets all widths"
        );
        assert_eq!(s.border_color, Rgba::new(0x33, 0x33, 0x33, 255));
        assert_eq!(s.box_sizing, BoxSizing::BorderBox);

        // Per-side + keyword widths; a visible style with no length defaults to medium (3px).
        let (dom, map) = cascade_of(
            r#"<p style="border-width:1px 2px 3px 4px;border-left:dashed red;border-top-width:thick"></p>"#,
        );
        let s = &map[&dom.find_first("p").unwrap()];
        assert_eq!(s.border_width.right, 2.0);
        assert_eq!(s.border_width.bottom, 3.0);
        assert_eq!(
            s.border_width.left, 3.0,
            "border-left: dashed -> medium 3px"
        );
        assert_eq!(s.border_width.top, 5.0, "border-top-width: thick -> 5px");

        // `border-style: none` zeroes the width set by an earlier `border`.
        let (dom, map) = cascade_of(r#"<p style="border:10px solid;border-style:none"></p>"#);
        assert_eq!(
            map[&dom.find_first("p").unwrap()].border_width,
            Sides::all(0.0)
        );

        // Default box-sizing is content-box.
        let (dom, map) = cascade_of(r#"<p style="width:10px"></p>"#);
        assert_eq!(
            map[&dom.find_first("p").unwrap()].box_sizing,
            BoxSizing::ContentBox
        );
    }

    #[test]
    fn font_family_resolves_generics_named_and_ua() {
        // Generic keyword after an unavailable named font falls through to the generic.
        assert_eq!(
            parse_font_family("Arial, sans-serif"),
            vec!["arial", "sans-serif"]
        );
        assert_eq!(
            parse_font_family("Georgia, serif"),
            vec!["georgia", "serif"]
        );
        assert_eq!(
            parse_font_family("'Courier New', monospace"),
            vec!["courier new", "monospace"]
        );
        // Named families we know map to their generic even without a following keyword.
        // Named families are preserved (the text layer resolves them).
        assert_eq!(
            parse_font_family("Times New Roman"),
            vec!["times new roman"]
        );
        assert_eq!(
            parse_font_family("Menlo, monospace"),
            vec!["menlo", "monospace"]
        );

        // Cascade: an author family list applies and is inherited; UA gives <code> monospace.
        let (dom, map) =
            cascade_of(r#"<div style="font-family:'MyFont', monospace">a<code>b</code></div>"#);
        let div = dom.find_first("div").unwrap();
        assert_eq!(map[&div].font_family, vec!["myfont", "monospace"]);
        assert_eq!(
            map[&dom.find_first("code").unwrap()].font_family,
            vec!["monospace"]
        );

        // A bare <pre> is monospace by UA default even without an author rule.
        let (dom, map) = cascade_of("<pre>x</pre>");
        assert_eq!(
            map[&dom.find_first("pre").unwrap()].font_family,
            vec!["monospace"]
        );
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
        let a2 = dom
            .descendants(dom.root())
            .filter(|&n| dom.tag_name(n) == Some("a"))
            .nth(1)
            .unwrap();
        assert!(m("a[href$='.com']", a2), "suffix match");
        assert!(m("[data-role~=ext]", a2), "includes match");
        assert!(!m("input[type=text]", sub), "type mismatch");

        // Structural pseudo-classes over the three <p>s.
        assert!(
            m("p:first-child", ps[0]) == false,
            "p[0] has prior siblings (a/input)"
        );
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

/// `::before` / `::after` — generated content. Not a decorative corner of CSS: it is how the web
/// draws icons, quotation marks, counters, dividers and much of its layout scaffolding.
#[cfg(test)]
mod pseudo_tests {
    use super::*;
    #[test]
    fn before_is_cascaded() {
        let dom = manuk_html::parse(r#"<p id="p">body</p>"#);
        let sheets = vec![Stylesheet::parse(r#"#p::before{content:"[X] "}"#)];
        let styles = MinimalCascade.cascade(&dom, &sheets);
        let p = dom
            .descendants(dom.root())
            .find(|&n| dom.tag_name(n) == Some("p"))
            .unwrap();
        let s = &styles[&p];
        assert!(s.before.is_some(), "::before must cascade");
        assert_eq!(s.before.as_ref().unwrap().content.as_deref(), Some("[X] "));
    }
}
