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

/// **Selector SYNTAX validation** — a question separate from "can we match it", and the reason
/// `querySelectorAll('[')` returned an empty list instead of throwing `SyntaxError`.
pub mod selector_syntax;
pub use selector_syntax::selector_syntax_error;

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
    /// `display: flow-root` — a block box that **establishes a block formatting context**. The only
    /// difference from `Block` is that difference, and it is the whole reason the value exists: it is
    /// the modern, explicit *"contain my floats"* with none of the side effects of the alternatives
    /// (`overflow:hidden` also clips; a `::after` clearfix needs a generated box). Treated as
    /// block-level everywhere `Block` is, and listed in `establishes_bfc`.
    FlowRoot,
    TableRowGroup,
    /// `display: table-header-group` (`<thead>`) and `table-footer-group` (`<tfoot>`).
    ///
    /// ⚠ These used to fold into `TableRowGroup`, and the fold was not a simplification — it
    /// discarded the ONLY thing that distinguishes them. CSS Tables lays row groups out as
    /// **header → body → footer regardless of source order**, so `<tfoot>` written before `<tbody>`
    /// (the classic HTML4 idiom, and still everywhere in legacy markup) rendered at the TOP of the
    /// table. Three variants because the ORDER is the semantics.
    TableHeaderGroup,
    TableFooterGroup,
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

/// `pointer-events` (inherited). `None` makes the element (and, by inheritance, its subtree unless a
/// descendant sets `auto`) transparent to hit-testing: `elementFromPoint` and click dispatch pass
/// *through* it to whatever is behind. The canonical use is a full-bleed decorative overlay that must
/// not swallow clicks meant for the content beneath it — get this wrong and an agent (or the page's own
/// script) clicking a button under such an overlay hits the overlay and the button never fires.
/// Stylo's servo build models only these two values; the SVG-only keywords are `cfg(gecko)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PointerEvents {
    #[default]
    Auto,
    None,
}

/// `user-select` — whether the user can select an element's text, and how a click-drag grows the
/// selection. Toolbars, buttons, drag-handles and code-copy widgets set `user-select: none`
/// ubiquitously so a double-click drag on the chrome does not select label text; editors set
/// `user-select: all` on atomic tokens. Stylo's servo build parses it only once
/// `layout.unimplemented` is flipped (see `cascade_via_stylo`); its four values are the full set.
/// We resolve the COMPUTED value so `getComputedStyle(el).userSelect` reads correctly (the CSSOM
/// value feature-detection reads back); the *geometry* of a user mouse-drag selection remains a
/// layout/hit-test concern we do not model, exactly like the note on `Selection` in the JS layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum UserSelect {
    #[default]
    Auto,
    Text,
    None,
    All,
}

/// `color-scheme` (inherited) — which system-appearance the element opts into. A page that declares
/// `color-scheme: dark` (or a `<meta name="color-scheme" content="dark">`) is telling the UA to
/// render its *default* surfaces dark: form controls, scrollbars, and — the case this engine
/// models — the **canvas background** behind content shorter than the viewport. Without it a
/// dark-only page paints its content on a correct dark box floating in a WHITE void. Stylo's servo
/// build parses it only with `layout.unimplemented` on (see `cascade_via_stylo`). We resolve the
/// keyword set to the four cases that decide the canvas default; `LightDark` (both listed) defers to
/// the user preference, which defaults light.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ColorScheme {
    #[default]
    Normal,
    Light,
    Dark,
    /// Both `light` and `dark` listed — used scheme follows the user's `prefers-color-scheme`.
    LightDark,
}

impl ColorScheme {
    /// The *used* scheme is dark only when the page supports dark and NOT light — a dark-only page
    /// is rendered dark regardless of the OS preference (Chrome's behaviour). A `light dark` page
    /// defers to the preference, which defaults light here, so it is not treated as dark.
    pub fn is_dark(self) -> bool {
        matches!(self, ColorScheme::Dark)
    }
}

/// `scrollbar-width` (CSS Scrollbars, Baseline 2024) — how much room a classic scrollbar takes.
/// Dark-mode and compact UIs set `scrollbar-width: thin` on scroll containers; `none` hides the
/// scrollbar entirely (a custom overlay draws its own). It is `engine="gecko"` in stylo 0.19 (absent
/// from the servo build), so it is recovered from [`MinimalCascade`] like `-webkit-line-clamp`. We
/// resolve the COMPUTED keyword so `getComputedStyle(el).scrollbarWidth` reflects the stylesheet; the
/// visible-scrollbar geometry is a paint concern this engine does not model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ScrollbarWidth {
    #[default]
    Auto,
    Thin,
    None,
}

/// `scrollbar-color` (CSS Scrollbars, Baseline 2024) — the thumb/track colours a page themes its
/// scrollbars with (`scrollbar-color: #888 #222` on a dark UI). `auto` is the UA default. It is
/// `engine="gecko"` in stylo 0.19 (absent from the servo build), so it is recovered from
/// [`MinimalCascade`]. We resolve the two colours to rgba so `getComputedStyle(el).scrollbarColor`
/// reports what the stylesheet set, the value dark-mode themers feature-detect; painting the scrollbar
/// itself is out of scope, like `user-select`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScrollbarColor {
    Auto,
    /// `<color>{2}` — thumb (first) then track (second).
    Colors {
        thumb: Rgba,
        track: Rgba,
    },
}

impl Default for ScrollbarColor {
    fn default() -> Self {
        ScrollbarColor::Auto
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    Center,
    Right,
    Justify,
    /// `start` (the INITIAL value) / `end` — LOGICAL alignments that resolve to left/right against the
    /// element's `direction`. Kept distinct so the cascade can resolve them once direction is known
    /// (the shipping path recovers direction from MinimalCascade *after* the Stylo map runs); they are
    /// resolved to a physical value before layout ever sees them, so `start` right-aligns an RTL
    /// paragraph — the default for the entire Arabic/Hebrew/Persian web.
    Start,
    End,
}

impl TextAlign {
    /// Resolve a logical `start`/`end` to a physical `Left`/`Right` against `rtl`; physical values
    /// (and `Justify`) pass through unchanged. `start` is left in LTR and right in RTL.
    pub fn resolve_physical(self, rtl: bool) -> TextAlign {
        match self {
            TextAlign::Start => {
                if rtl {
                    TextAlign::Right
                } else {
                    TextAlign::Left
                }
            }
            TextAlign::End => {
                if rtl {
                    TextAlign::Left
                } else {
                    TextAlign::Right
                }
            }
            other => other,
        }
    }
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

/// `scroll-snap-type` — which axes a scroll container snaps on.
///
/// The `mandatory`/`proximity` strictness is deliberately NOT modelled: `proximity` lets the UA
/// decide, and "snap to the nearest point" is a conforming choice for both. Modelling the axis is
/// what decides whether a carousel lands on a slide; modelling the strictness would only change
/// *how often*, and guessing a threshold would be inventing behaviour rather than implementing it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ScrollSnapAxis {
    #[default]
    None,
    X,
    Y,
    Both,
}

impl ScrollSnapAxis {
    pub fn snaps_x(self) -> bool {
        matches!(self, ScrollSnapAxis::X | ScrollSnapAxis::Both)
    }
    pub fn snaps_y(self) -> bool {
        matches!(self, ScrollSnapAxis::Y | ScrollSnapAxis::Both)
    }
}

/// `scroll-snap-align` — where a child aligns inside the container's snapport when it is snapped to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ScrollSnapAlign {
    #[default]
    None,
    Start,
    Center,
    End,
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

/// `border-style` — the LINE style of ONE side of a border. `groove`/`ridge`/`inset`/`outset`
/// collapse to `Solid` (their bevel shading is a paint refinement, and a solid line is the honest
/// approximation).
///
/// ⚠ Held per side in a [`Sides<BorderStyle>`] since t1079. It was a scalar for 1,078 ticks, beside
/// a per-side `border_width`, so `border-bottom-style: dashed` painted solid whenever the top said
/// solid — see the field's own note.
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

/// `writing-mode` — **which way the INLINE axis runs**, and therefore which physical axis every
/// box's "length" is measured along. Inherited.
///
/// ⚠⚠⚠ **THIS IS NOT A TEXT-RENDERING SWITCH, IT IS A COORDINATE SYSTEM.** In `horizontal-tb` the
/// inline axis is horizontal and the block axis runs down, which is the only geometry this engine
/// had ever modelled — every `width` was an inline size and every `height` was a block size, in one
/// fixed pairing that nothing could vary. In a vertical mode the pairing **swaps**: the inline axis
/// runs down the page, the block axis runs sideways, and a box's `width` is now its *block* size
/// while its `height` is its *inline* size. Stylo already resolves the logical spellings
/// (`inline-size`, `margin-block-start`, …) onto the physical ones against this value, so by the
/// time a `ComputedStyle` exists the property is invisible in every field except this one — which
/// is exactly why its absence was silent rather than loud.
///
/// Measured in Chrome, `<div style="width:400px;writing-mode:vertical-rl"><div>x</div></div>` at
/// 16px/20px monospace — the container is 400 **wide by 10 tall**, and the child sits at
/// `[380 0 20x10]`: 20px of *block* size (the line height) hugging the RIGHT edge, 10px of *inline*
/// size (the glyph advance) running down. Laid out as `horizontal-tb` the same markup gives
/// `[0 0 400x20]`. Nothing about that is a near miss.
///
/// `sideways-rl`/`sideways-lr` are parsed and treated as their `vertical-*` counterparts: they
/// differ only in glyph orientation (all glyphs rotated, never upright), not in box geometry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum WritingMode {
    /// The initial value — inline runs left-to-right (or right-to-left), blocks stack downwards.
    #[default]
    HorizontalTb,
    /// Inline runs top-to-bottom, blocks stack **right to left** (Japanese/Chinese vertical text).
    VerticalRl,
    /// Inline runs top-to-bottom, blocks stack **left to right** (Mongolian).
    VerticalLr,
    /// `sideways-rl` — geometry as [`Self::VerticalRl`], glyphs always rotated.
    SidewaysRl,
    /// `sideways-lr` — geometry as [`Self::VerticalLr`], glyphs always rotated (upwards).
    SidewaysLr,
}

impl WritingMode {
    /// Is the inline axis VERTICAL — i.e. is this box's `width` a block size?
    pub fn is_vertical(self) -> bool {
        !matches!(self, WritingMode::HorizontalTb)
    }

    /// Do blocks stack towards the LEFT (`vertical-rl`/`sideways-rl`)? The block-start edge is then
    /// the box's RIGHT edge, which is what makes the first child of a `vertical-rl` container sit
    /// flush against the right side.
    pub fn is_rl(self) -> bool {
        matches!(self, WritingMode::VerticalRl | WritingMode::SidewaysRl)
    }

    /// The CSSOM serialization — what `getComputedStyle(el).writingMode` must answer.
    pub fn as_css(self) -> &'static str {
        match self {
            WritingMode::HorizontalTb => "horizontal-tb",
            WritingMode::VerticalRl => "vertical-rl",
            WritingMode::VerticalLr => "vertical-lr",
            WritingMode::SidewaysRl => "sideways-rl",
            WritingMode::SidewaysLr => "sideways-lr",
        }
    }
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

/// `tab-size` — **the distance between tab stops in a preserved-whitespace run**, and the reason it
/// is an enum rather than a number is that CSS gives it two incompatible units.
///
/// A `<number>` is a count of **space advances** in whatever font the run is set in, so it is not a
/// length until a font is in hand; a `<length>` is absolute. Collapsing the first into the second at
/// parse time would bake in the parse-time font size and be wrong for every element that inherits it
/// (which is all of them — the property is inherited, and it is set on `body`/`pre` far more often
/// than on the element that renders the tab).
///
/// The initial value is `8`, which is what an unstyled `<pre>` on the open web is laid out with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TabSize {
    /// A count of space advances — resolved against the run's own font at layout time.
    Spaces(f32),
    /// An absolute length in px.
    Px(f32),
}

impl Default for TabSize {
    fn default() -> Self {
        TabSize::Spaces(8.0)
    }
}

/// `box-sizing`: whether `width`/`height` size the content box (CSS default) or the
/// border box (padding + border counted inside the given dimension).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoxSizing {
    ContentBox,
    BorderBox,
}

/// `vertical-align` for inline-level boxes (the common keywords).
// `Eq` is dropped because the length/percentage forms carry an `f32` (t922). Nothing compares a
// `VerticalAlign` for total equality; `PartialEq` is what the matches and tests use.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VerticalAlign {
    Baseline,
    Top,
    Middle,
    Bottom,
    TextTop,
    TextBottom,
    Sub,
    Super,
    /// `vertical-align: <length>` — an explicit raise (positive) or drop (negative) in px, already
    /// resolved from `em`/`rem` against this element's font size at parse time.
    Length(f32),
    /// `vertical-align: <percentage>` — of THIS element's own `line-height` (CSS 2.1 §10.8.1), which
    /// is why it is kept as a ratio and resolved in layout rather than here.
    Percent(f32),
}

/// `justify-content` — main-axis distribution of flex items, inline-axis distribution of grid tracks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JustifyContent {
    /// The INITIAL value, and it is **not a synonym for `flex-start`** — that conflation is what this
    /// variant exists to undo. `normal` behaves as `flex-start` in a FLEX container but as `stretch` in
    /// a GRID one, and the grid half is load-bearing: CSS Grid §11.8 "Stretch auto Tracks" only runs
    /// when the axis is stretch-aligned, so an `auto` track absorbs the container's free space *only*
    /// under `normal`. With `normal` folded into `FlexStart` every grid we built skipped that step and
    /// content-sized its implied tracks — a 600px container with `grid-template-areas:"l r"` produced
    /// 88px/133px columns where Chromium gives 289px/291px.
    Normal,
    FlexStart,
    FlexEnd,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// **`appearance` — whether this element is drawn as a NATIVE WIDGET.**
///
/// Two values, because this engine has exactly two behaviours: it draws the platform control or it
/// does not. CSS UI 4 has a dozen more keywords (`textfield`, `menulist-button`, `button`, …); every
/// one of them that is VALID computes to `Auto` here, which is what WPT's own
/// `appearance-cssom-001` accepts for the compat set (`[value, "auto"]`) and is the honest answer
/// for an engine that does not draw a text field differently from a button.
///
/// ⚠ The SPECIFIED value is preserved separately by the CSSOM (`el.style.appearance` echoes what the
/// author wrote); this is the COMPUTED one, and it says only what the engine will actually do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Appearance {
    /// No native widget — the element is drawn from its own box, border and background alone.
    None,
    /// The platform control. The UA default for form controls; the initial value everywhere else
    /// is `None`.
    Auto,
}

/// `align-items` — cross-axis alignment of flex items.
///
/// ⚠⚠⚠ **`Normal` IS NOT `Stretch`, AND THE TWO WERE ONE VARIANT.** They behave identically in
/// flexbox and identically in grid *for ordinary boxes*, which is why conflating them survived —
/// but a grid item that is a REPLACED element with an intrinsic size aligns as `start` under
/// `normal` and stretches under an explicit `stretch`. Chrome-measured, a 16x16 `<img>` in a 40x40
/// grid: `align-items:normal` → **16x16**, `align-items:stretch` → **40x40**. With one variant
/// there is no way to ask the question, so every avatar, logo and icon in a grid cell was inflated
/// to the cell. `normal` is also the value `getComputedStyle` must report for the initial value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlignItems {
    /// The CSS initial value. Behaves as `Stretch` everywhere except a replaced grid item with an
    /// intrinsic size, where it behaves as `start` — see `taffy_tree`'s grid-item pass.
    Normal,
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
    /// ⚠⚠⚠ **`fit-content(<length>)`, which used to COLLAPSE TO `Auto` and therefore STRETCHED.**
    ///
    /// Grid §7.2.2 defines it as `minmax(auto, max-content)` clamped by the argument — i.e.
    /// `min(max-content, max(min-content, <length>))`. Mapping it to `Auto` gave the opposite
    /// behaviour on the only axis anyone uses it for: an `auto` track absorbs free space, so
    /// `fit-content(50px)` on a 400px grid produced a **400px** track where every browser gives 50.
    /// It is the "clamp this column, but no wider than its content" idiom — a sidebar, a label
    /// column, a truncating table cell — and it did the reverse.
    ///
    /// Carries the clamp in px; a percentage argument resolves at cascade time like any other
    /// length, and an argument that cannot be resolved falls back to `Auto` (the old behaviour)
    /// rather than guessing.
    FitContent(f32),
}

/// One component of a `grid-template-columns` / `-rows` list.
///
/// **An `auto-fill` / `auto-fit` `repeat()` cannot be expanded by a cascade**, and that is the whole
/// reason this type exists rather than a flat `Vec<TrackSize>`. The repetition count is defined by
/// CSS Grid §7.2.3.1 as the largest N whose tracks plus gutters still fit **the grid container's
/// resolved inline size** — a number only layout knows. Collapsing the repeat to a fixed count at
/// parse time (we used `1`) turns the responsive-card idiom
/// `repeat(auto-fill, minmax(18em, 1fr))` into a single full-width column on every site that uses it.
#[derive(Clone, Debug, PartialEq)]
pub enum TrackComponent {
    /// A single, non-repeated track — or one expansion of an integer `repeat(N, …)`, which the
    /// cascade *can* expand because N is literal.
    Single(TrackSize),
    /// `repeat(auto-fill | auto-fit, <track-list>)`, carried through to layout intact.
    AutoRepeat {
        /// `true` for `auto-fit`, whose distinguishing behaviour is that repetitions which end up
        /// **empty collapse to zero** (their gutters with them), so 2 items in a 3-track grid span
        /// the container instead of huddling in the first two tracks. `auto-fill` keeps them.
        fit: bool,
        /// The track sizes inside the `repeat()`, repeated as a group.
        tracks: Vec<TrackSize>,
    },
}

/// `grid-auto-flow` — the axis the **auto-placement algorithm** advances along, plus whether it may
/// go backwards to back-fill holes.
///
/// The two halves are orthogonal and the CSS grammar lets them appear in either order
/// (`column dense` == `dense column`), which is why this is one enum of four states rather than an
/// axis plus a flag: taffy models it the same way, so the mapping is total and no state can be
/// dropped in translation.
///
/// **`Row` is the initial value and it is why nothing noticed this was missing** — every grid that
/// never declares the property flows exactly as a defaulted field does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum GridAutoFlow {
    /// Fill each row in turn, adding implicit **rows** as needed. The initial value.
    #[default]
    Row,
    /// Fill each column in turn, adding implicit **columns** as needed.
    Column,
    /// `row dense` — row flow, but an item smaller than a hole left earlier may move **back** into it.
    RowDense,
    /// `column dense`.
    ColumnDense,
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

/// One `filter` function, computed. The list is applied **in source order**: `filter: grayscale(1)
/// blur(2px)` desaturates first and blurs the desaturated result, which is not the same picture as
/// the reverse.
///
/// The variants are exactly Filter Effects 1's shorthand functions minus `url()` (an SVG filter
/// reference, which needs an SVG filter graph and is not representable in Stylo's servo build
/// either). `backdrop-filter` is deliberately NOT modelled here: it filters what is painted
/// *behind* the element, a different input, and it is still an honest "no".
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FilterOp {
    /// `blur(<length>)` — the Gaussian standard deviation in px.
    Blur(f32),
    /// `brightness(<number|percentage>)` — 1.0 is the identity.
    Brightness(f32),
    /// `contrast(<number|percentage>)` — 1.0 is the identity.
    Contrast(f32),
    /// `grayscale(<number|percentage>)` — 0..1, 1 = fully desaturated.
    Grayscale(f32),
    /// `hue-rotate(<angle>)` — degrees.
    HueRotate(f32),
    /// `invert(<number|percentage>)` — 0..1.
    Invert(f32),
    /// `opacity(<number|percentage>)` — 0..1, multiplies alpha.
    Opacity(f32),
    /// `saturate(<number|percentage>)` — 1.0 is the identity.
    Saturate(f32),
    /// `sepia(<number|percentage>)` — 0..1.
    Sepia(f32),
    /// `drop-shadow(<offset-x> <offset-y> [<blur>] [<color>])` — a shadow of the element's **alpha
    /// silhouette**, not of its box. That is what separates it from `box-shadow` and why icons and
    /// cut-out PNGs use it.
    DropShadow {
        dx: f32,
        dy: f32,
        blur: f32,
        color: Rgba,
    },
}

/// A `<shape-radius>` — the radius of a `circle()`/`ellipse()` clip.
///
/// A percentage here does **not** resolve against a box side. `circle(50%)` resolves against
/// `sqrt(w² + h²) / √2`, the CSS "reference box diagonal", which is why the radius carries its own
/// type instead of reusing [`Dim`]'s resolver: handing it a width would make every non-square
/// avatar the wrong size, and in the direction that clips the face.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ShapeRadius {
    Len(Dim),
    /// Distance to the nearest side of the reference box — the default for `circle()`.
    ClosestSide,
    FarthestSide,
}

/// A `clip-path` basic shape. Coordinates are relative to the element's **border box**.
///
/// `path()`, `shape()` and `url(#svgclip)` are deliberately absent: they need an SVG path/filter
/// graph, and modelling them here as a variant nothing draws is exactly the "parses, never renders"
/// lie this engine spent two ticks removing. A `clip-path` we cannot draw stays `None`, which
/// renders the element unclipped — visibly wrong, but not silently wrong.
#[derive(Clone, Debug, PartialEq)]
pub enum ClipShape {
    /// `inset(top right bottom left round <radius>)` — insets from each edge of the reference box.
    Inset {
        top: Dim,
        right: Dim,
        bottom: Dim,
        left: Dim,
        /// A uniform corner radius (`round`), px only — the elliptical/per-corner forms are residue.
        round: f32,
    },
    Circle {
        cx: Dim,
        cy: Dim,
        r: ShapeRadius,
    },
    Ellipse {
        cx: Dim,
        cy: Dim,
        rx: ShapeRadius,
        ry: ShapeRadius,
    },
    /// `polygon(<fill-rule>? x y, …)` — the diagonal section dividers and angled banners.
    Polygon {
        /// `evenodd` rather than the default `nonzero`.
        even_odd: bool,
        points: Vec<(Dim, Dim)>,
    },
}

/// `mix-blend-mode` — how an element composites against its **backdrop** (what is already painted
/// beneath it), rather than simply covering it.
///
/// Every one of these is a real formula from Compositing and Blending 1, and `tiny-skia` implements
/// all of them — which is the whole reason this capability is cheap: the work was never the maths,
/// it was having an offscreen group to composite *from* (tick 592). `plus-lighter` is the one CSS
/// keyword with no `tiny-skia` counterpart and is mapped honestly to `Normal`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BlendMode {
    #[default]
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    HardLight,
    SoftLight,
    Difference,
    Exclusion,
    Hue,
    Saturation,
    Color,
    Luminosity,
}

impl BlendMode {
    /// `true` for every mode that actually needs the backdrop — i.e. everything but `normal`.
    /// An element with `normal` must stay on the cheap direct-to-canvas paint path.
    pub fn is_blending(self) -> bool {
        self != BlendMode::Normal
    }
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

/// Split a `content` value into its terms, resolving nothing that needs the document.
///
/// The tokenizer is deliberately small and string-aware: quotes may contain commas, parentheses and
/// whitespace, so it cannot be a `split_whitespace`. Anything that is neither a quoted string nor a
/// `counter(...)` is skipped — `attr()` is handled by the Stylo path, which has the element.
pub fn parse_content_parts(v: &str) -> Vec<ContentPart> {
    let mut out = Vec::new();
    let b: Vec<char> = v.chars().collect();
    let mut i = 0usize;
    while i < b.len() {
        let c = b[i];
        if c == '"' || c == '\'' {
            let quote = c;
            let mut lit = String::new();
            i += 1;
            while i < b.len() && b[i] != quote {
                lit.push(b[i]);
                i += 1;
            }
            i += 1; // the closing quote
            out.push(ContentPart::Text(decode_css_escapes(&lit)));
        } else if b[i..].starts_with(&['c', 'o', 'u', 'n', 't', 'e', 'r']) {
            // `counter(` or `counters(` — take the name, ignore a style/separator argument.
            let Some(open) = b[i..].iter().position(|&x| x == '(') else {
                break;
            };
            let Some(close) = b[i + open..].iter().position(|&x| x == ')') else {
                break;
            };
            let inner: String = b[i + open + 1..i + open + close].iter().collect();
            let name = inner
                .split(',')
                .next()
                .unwrap_or("")
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string();
            if !name.is_empty() {
                out.push(ContentPart::Counter(name));
            }
            i += open + close + 1;
        } else {
            i += 1;
        }
    }
    // A bare unquoted value (the old lenient behaviour) rather than nothing at all.
    if out.is_empty() {
        let inner = v.trim().trim_matches('"').trim_matches('\'');
        if !inner.is_empty() {
            out.push(ContentPart::Text(decode_css_escapes(inner)));
        }
    }
    out
}

/// `counter-reset` / `counter-increment`: a list of `<name> [<integer>]` pairs. `dflt` is the
/// implied integer — **0 for reset and 1 for increment**, which is the difference between the two
/// properties and the only difference.
pub fn parse_counter_list(v: &str, dflt: i32) -> Vec<(String, i32)> {
    let mut out: Vec<(String, i32)> = Vec::new();
    for tok in v.split_whitespace() {
        if tok.eq_ignore_ascii_case("none") {
            return Vec::new();
        }
        match tok.parse::<i32>() {
            Ok(n) => {
                if let Some(last) = out.last_mut() {
                    last.1 = n;
                }
            }
            Err(_) => out.push((tok.to_string(), dflt)),
        }
    }
    out
}

/// One term of a `content` value on a `::before`/`::after`.
///
/// `content` is a LIST — `content: "S" counter(sec) ". "` is three terms — and the list has to
/// survive the cascade unflattened because [`ContentPart::Counter`]'s value is a function of
/// document order, which the cascade does not know. See [`ComputedStyle::content`].
#[derive(Debug, Clone, PartialEq)]
pub enum ContentPart {
    /// A literal string, or an `attr()` already resolved against the element (that one CAN be
    /// resolved in the cascade — the attribute is right there on the element).
    Text(String),
    /// `counter(name)` / `counter(name, <list-style-type>)` — resolved by layout's document-order
    /// walk, never here.
    Counter(String),
}

impl ContentPart {
    /// The literal text of this term, or `None` for a term whose value layout has to supply.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            ContentPart::Text(t) => Some(t),
            ContentPart::Counter(_) => None,
        }
    }
}

/// The fully-resolved style of one element, as consumed by layout and paint.
#[derive(Clone, Debug, PartialEq)]
pub struct ComputedStyle {
    pub display: Display,
    pub color: Rgba,
    /// **The computed `color` as CSS Color 4 says to SERIALIZE it — but only when `color` is not a
    /// legacy sRGB colour.**
    ///
    /// ⚠⚠⚠ [`Self::color`] is an `Rgba { r, g, b, a: u8 }`, which is the right type for painting and
    /// the wrong one for reporting. CSS Color 4 splits serialization in two: a *legacy* sRGB colour
    /// (hex, named, `rgb()`, `hsl()`, `hwb()`) serializes as `rgb()`/`rgba()` with 0–255 integer
    /// channels, and **everything else keeps its own function and its own 0–1 float channels** —
    /// `color(display-p3 …)`, `lab()`, `oklch()`, `color-mix()`, and every relative `rgb(from …)`.
    /// Quantising those to 8-bit sRGB is lossy for a wide-gamut colour and simply has no spelling:
    /// `color(display-p3 1 0 0)` is outside sRGB entirely.
    ///
    /// So the space had to survive the cascade→style boundary for the CSSOM, and it does it as the
    /// finished string rather than as a second colour type: the value is **borrowed whole** from
    /// Stylo's `impl ToCss for AbsoluteColor`, which already implements every branch of the spec's
    /// serialization including its alpha rule. Nothing here re-derives it.
    ///
    /// ⚠ `None` for a legacy colour, on purpose — that keeps every hex/named/`rgb()`/`hsl()` on the
    /// open web on the byte-for-byte answer it already had, including the alpha serialization fitted
    /// against Chrome at t1205, instead of routing them through a second implementation.
    pub color_css: Option<String>,
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
    /// **Was this axis filled in from the element's NATURAL size rather than specified?**
    ///
    /// `apply_natural_size` writes a decoded image's (or a `<canvas>`'s) own pixel size into
    /// `width`/`height` when — and only when — the axis is `auto`. After it runs, a `Dim::Px` no
    /// longer says where the number came from, and **the difference is observable**. Chrome, on one
    /// bitmap and one clamp, with only the dimension attributes differing:
    ///
    /// ```text
    ///   <img              style="max-height:30px">   1000x266 bitmap   112.78 x 30   ratio TRANSFERS
    ///   <img w=1000 h=266 style="max-height:30px">   same bitmap       1000   x 30   it does NOT
    /// ```
    ///
    /// A natural axis behaves as `auto` — the ratio may rewrite it. A *specified* axis (author CSS
    /// or a dimension attribute, which is a presentational hint for the same property) is a value
    /// the page asked for, and no clamp on the other axis may overwrite it. Without this flag the
    /// two are one state and one of the rows above must be wrong.
    pub width_is_natural: bool,
    /// The block-axis twin of `width_is_natural` — see there.
    pub height_is_natural: bool,
    pub background_repeat: BackgroundRepeat,
    /// `text-decoration-line` (INHERITED in effect: a decoration set on a block draws through its
    /// inline descendants).
    pub text_decoration: TextDecoration,
    /// `list-style-type` (inherited).
    pub list_style_type: ListStyleType,
    /// `list-style-position: inside` puts the marker in the principal box's content flow.
    pub list_style_inside: bool,
    /// `content` — only meaningful on a `::before`/`::after` pseudo-element.
    ///
    /// ⚠⚠⚠ **THE COUNTER TERMS SURVIVE HERE UNRESOLVED, AND THAT IS THE WHOLE POINT OF THE TYPE.**
    /// This used to be an `Option<String>`, flattened in the cascade — and a string cannot hold a
    /// counter, because a counter's value depends on **document order** and is not knowable when
    /// the element's style is computed. So `content: "S" counter(sec) ". "` came out as `"S. "`:
    /// the strings concatenated correctly and the counter silently evaporated at a `_ => {}`
    /// (t1095). Keeping the list unflattened lets layout's document-order walk fill the counters in.
    ///
    /// Use [`ComputedStyle::content_text`] where the counter-free text is what is wanted.
    pub content: Option<Vec<ContentPart>>,
    /// `counter-reset: name [n]` — sets the counter to `n` (default 0) at this element.
    pub counter_reset: Vec<(String, i32)>,
    /// `counter-increment: name [n]` — adds `n` (default 1) at this element, AFTER any reset.
    pub counter_increment: Vec<(String, i32)>,
    /// The computed style of this element's `::before` / `::after` pseudo-elements, when they have
    /// `content`. Generated content is not in the DOM (script must never see it), so it rides on
    /// the element's style and is materialised as inline items at layout time.
    ///
    /// This is not a decorative corner of CSS: it is how the web draws icons, quotation marks,
    /// counters, dividers, clearfixes and a great deal of layout scaffolding.
    pub before: Option<Box<ComputedStyle>>,
    pub after: Option<Box<ComputedStyle>>,
    /// The computed style of this element's `::first-letter` pseudo-element (CSS 2.1 §5.12.1).
    ///
    /// ⚠ **It is NOT generated content, and that is the whole difference from [`Self::before`].**
    /// `::before`/`::after` invent a box around text the author wrote in `content`; `::first-letter`
    /// re-styles a *range of text that is already in the DOM*, and the range is not known until the
    /// inline items of the first line have been collected. So there is nothing to check `content`
    /// for here — a `::first-letter` rule that sets only `color` still generates a box — and the
    /// range is resolved in layout, where the words are.
    pub first_letter: Option<Box<ComputedStyle>>,
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
    /// `text-indent` — inline-start indent of the **first line box only** (inherited). A length or
    /// %-of-containing-block, stored as `Dim` and resolved at layout against the container width.
    /// The image-replacement idiom (`text-indent:-9999px` / `text-indent:100%`) rides this: a large
    /// negative or 100% value pushes the first line off-screen so the background image shows alone.
    pub text_indent: Dim,
    pub white_space: WhiteSpace,
    /// `text-overflow` — `ellipsis` truncates clipped single-line inline content with `…`.
    pub text_overflow: TextOverflow,
    /// `-webkit-line-clamp: <n>` — cap a block at N line boxes, dropping the rest and appending `…`
    /// to line N. The container half of the ubiquitous card/product/article-excerpt truncation idiom
    /// (`display:-webkit-box; -webkit-box-orient:vertical; -webkit-line-clamp:N; overflow:hidden`).
    /// `None` = unclamped (initial); a non-inherited box property, so it never leaks to descendants.
    pub line_clamp: Option<u16>,
    /// **The blockified display an author asked for with `display: -webkit-box`** — `Some(Block)`
    /// for `-webkit-box`, `Some(InlineBlock)` for `-webkit-inline-box`, `None` for every other
    /// (including a later `display` declaration that wins the cascade and clears it).
    ///
    /// This field exists ONLY so the shipping Stylo path can recover the value: both keywords are
    /// `#[cfg(feature = "gecko")]` in stylo 0.19's display parser, so the servo build rejects the
    /// whole declaration and the element keeps its default `inline` — which is what made the
    /// `line_clamp` recovery above a dead letter, since the clamp only ever fires on a block.
    /// Recording the *decision* separately (rather than reading `display` back out) is what keeps
    /// the merge from overriding a display Stylo resolved correctly.
    pub legacy_webkit_box: Option<Display>,
    /// `scroll-snap-type` on a scroll container; `scroll-snap-align` on its children.
    pub scroll_snap_type: ScrollSnapAxis,
    pub scroll_snap_align: ScrollSnapAlign,
    /// `text-transform` — rendered casing (inherited); applied in layout, DOM text unchanged.
    pub text_transform: TextTransform,
    /// `overflow-wrap`/`word-wrap` — allow breaking a long word at an arbitrary char (inherited).
    pub overflow_wrap: OverflowWrap,
    /// `word-break` — char-level break control within a run (inherited).
    pub word_break: WordBreak,
    /// `direction` — the paragraph's bidi base direction (inherited).
    pub direction: Direction,
    /// `writing-mode` — which physical axis the inline direction runs along (inherited). See
    /// [`WritingMode`]: in a vertical mode `width` is a BLOCK size and `height` is an INLINE size,
    /// and layout transposes the whole subtree rather than reinterpreting every field at its use
    /// site.
    pub writing_mode: WritingMode,
    /// `letter-spacing` — extra px added after each character (tracking). `0` = `normal`. Inherited.
    pub letter_spacing: f32,
    /// `word-spacing` — extra px added to each inter-word space. `0` = `normal`. Inherited.
    pub word_spacing: f32,
    /// `tab-size` — the distance between tab stops in a preserved-whitespace run. Inherited.
    pub tab_size: TabSize,
    /// `transform-origin` — the point a `transform` is applied ABOUT, as `(x, y)` resolved against
    /// the border box. Initial `50% 50%` (the centre), which is what the engine hard-coded at three
    /// call sites before this existed. NOT inherited.
    pub transform_origin: (Dim, Dim),
    pub margin: Sides<Dim>,
    pub padding: Sides<Dim>,
    pub border_width: Sides<f32>,
    /// `border-*-color`, **per side**.
    ///
    /// ⚠⚠⚠ **This was a single `Rgba` for 1,078 ticks, sitting beside a per-side `border_width`**,
    /// and `stylo_map` filled it from `clone_border_top_color()` — so every box on the web painted
    /// all four of its edges in its TOP edge's colour. The idioms that breaks are not exotic: the
    /// `border-left: 3px solid <brand>` accent bar on a card or blockquote, the `border-bottom`
    /// rule under a heading or active tab, and every table with a coloured horizontal rule. It was
    /// found from the other end — CSS 2.1's `*-applies-to-NNN` family, 483 of the suite's 3,022
    /// remaining failures, whose REFERENCE files distinguish a box's top edge from its bottom by
    /// giving them different colours.
    pub border_color: Sides<Rgba>,
    /// `border-*-style`, per side, for the same reason and found by the same probe.
    pub border_style: Sides<BorderStyle>,
    /// `border-radius` — a single uniform corner radius in px (per-corner radii are a follow-on).
    /// `0.0` = square corners.
    pub border_radius: f32,
    /// `visibility` (inherited). `Hidden`/`Collapse` boxes still take space but are not painted.
    pub visibility: Visibility,
    /// `pointer-events` (inherited). `None` = transparent to hit-testing (`elementFromPoint`/click
    /// dispatch pass through). See [`PointerEvents`].
    pub pointer_events: PointerEvents,
    /// `user-select` — the computed selectability keyword, resolved so `getComputedStyle` reflects
    /// it. See [`UserSelect`].
    pub user_select: UserSelect,
    /// `color-scheme` (inherited) — decides the dark/light canvas default and reflects through
    /// `getComputedStyle`. See [`ColorScheme`].
    pub color_scheme: ColorScheme,
    /// `scrollbar-width` — the computed thin/none/auto keyword, resolved so `getComputedStyle`
    /// reflects it. See [`ScrollbarWidth`].
    pub scrollbar_width: ScrollbarWidth,
    /// `scrollbar-color` (inherited) — the computed thumb/track colours, resolved to absolute rgba
    /// so `getComputedStyle` reflects them. See [`ScrollbarColor`].
    pub scrollbar_color: ScrollbarColor,
    /// `field-sizing: content` (Baseline June 2026) — the form control sizes from its CONTENT,
    /// so the UA's fixed-size presentational hints (`<textarea cols>` above all) must stand down
    /// and let intrinsic sizing run. `false` = `fixed`, the initial value.
    pub field_sizing_content: bool,
    /// `appearance: none` (or `-webkit-appearance: none`) — the author has taken the NATIVE WIDGET
    /// off this control.
    ///
    /// **This engine draws no native widget, so for a long time reading this property would have
    /// been theatre** — `G_APPEARANCE_NONE` says exactly that, measured: our controls are drawn by
    /// ordinary UA *CSS* at lowest specificity, which an author rule already beats, so
    /// `appearance: none` was a visual no-op with nothing to switch off.
    ///
    /// It has one reader now, and it is geometric rather than visual: a `<select>` must RESERVE room
    /// for the dropdown arrow in its intrinsic width, and `appearance: none` is precisely the
    /// declaration that says there is no arrow to reserve for. Chrome, measured: 159px with the
    /// widget, **139px with `appearance: none`** on the same option text.
    ///
    /// `clone_appearance()` is `engine="gecko"` in stylo 0.19 (compile-probed at t788: *no method
    /// named `clone_appearance` found for `&style::properties::ComputedValues`*), so this is
    /// recovered from `MinimalCascade` and merged in `stylo_engine` — the same fence as
    /// `scrollbar-width` and `-webkit-line-clamp`.
    pub appearance: Appearance,
    /// **The display this element would have had IN FLOW** — i.e. before out-of-flow blockification.
    ///
    /// ⚠⚠⚠ CSS 2.1 §10.6.4 defines an out-of-flow box's STATIC POSITION as where *"a hypothetical
    /// box … if its `position` property had been `static`"* would have gone — and that hypothetical
    /// box has the element's SPECIFIED display, not the blockified one. The distinction is not
    /// academic; it is the whole answer. Chrome-measured, an absolutely positioned box placed after
    /// inline text in a 400px block (`16px/20px monospace`):
    ///
    /// ```text
    ///   <div  style="position:absolute">     @0,20    ← a new line: it is BLOCK-level in flow
    ///   <span style="position:absolute">     @19,0    ← after the text: INLINE-level in flow
    ///   <div  style="position:absolute;display:inline-block">   @19,0
    /// ```
    ///
    /// ⚠ `display` itself cannot answer this: `position:absolute` BLOCKIFIES, so `getComputedStyle`
    /// reports `block` for ALL THREE — measured, in Chrome and here. This field is the value before
    /// that, which the MinimalCascade has naturally (it does not blockify) and Stylo's `clone_display`
    /// does not.
    pub display_in_flow: Display,
    /// `order` — a flex/grid item's VISUAL order among its siblings (initial `0`, may be negative).
    ///
    /// Only the visual order: the DOM, the accessibility tree and sequential focus keep source order,
    /// which is exactly why the spec cautions against carrying meaning in it — and exactly why an
    /// engine that ignores it reads as a READING-ORDER divergence rather than as a missing property.
    pub order: i32,
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
    /// `filter` — the ordered function list applied to this element **and its subtree**, as a group.
    /// Empty == `none`. This is the element's OWN value; paint composes it with its ancestors' (a
    /// filter forms a containing block and applies to everything inside it), which is why it is not
    /// folded here the way `opacity` is.
    ///
    /// It is on **51.9% of page loads** (Blink use counters, surface audit #32) and, unlike most
    /// visual effects, has no cascade-level fallback: a page that asks for a blur and gets a sharp
    /// image is not degraded, it is wrong — the frosted bar it drew its text over is now opaque.
    pub filter: Vec<FilterOp>,
    /// `clip-path` — the element's own shape, or `None` for `none`/`url()`/`path()`/`shape()`.
    /// Coordinates resolve against this element's **border box**, so paint carries the box along
    /// with the shape. Like [`Self::filter`] it clips the element AND its subtree as a group.
    ///
    /// 43.8% of page loads (Blink use counters, surface audit #32).
    pub clip_path: Option<ClipShape>,
    /// `mix-blend-mode` — 12.9% of page loads (Blink use counters, surface audit #32). The gradient
    /// scrim over a hero image, the duotone photo treatment, and `difference` text that stays legible
    /// over anything are all this property; without it the overlay simply covers what it was meant to
    /// tint.
    pub mix_blend_mode: BlendMode,
    /// `backdrop-filter` — the SAME function list as [`Self::filter`], applied to a different input:
    /// what is already painted **behind** the element, seen through its own box. That difference is
    /// the whole reason it stayed an honest "no" for three ticks after `filter` landed.
    ///
    /// 34.3% of page loads (Blink use counters, surface audit #32) — the frosted sticky header, the
    /// glassmorphic modal, the blurred sheet behind a dialog. It is also the single costliest
    /// property to lie about: a page that is told yes drops the opaque background it shipped for
    /// engines that cannot blur, and its text lands unreadable over a photograph.
    pub backdrop_filter: Vec<FilterOp>,
    pub width: Dim,
    /// The **intrinsic sizing keyword** on `width`, if any. `width` itself collapses to `Dim::Auto`
    /// for length resolution (an intrinsic width is content-driven, not a length), but unlike a plain
    /// `auto` a keyword width does NOT fill the containing block — it hugs the content: `min-content`
    /// is the longest unbreakable run, `max-content` the whole content unwrapped, `fit-content` the
    /// shrink-to-fit clamp between them. `None` = `auto`/length/`stretch`/`fill-available` (all fill).
    pub width_keyword: Option<IntrinsicSize>,
    /// `true` when `width` is `stretch` / `-webkit-fill-available` / `-moz-available` — the inline
    /// mirror of [`Self::height_stretch`]. It collapses to `Dim::Auto`, and for a plain block box
    /// that is already the right answer, which is exactly why its absence hid: `auto` fills there
    /// too. It is **every other box** that diverges — a float, an abspos, an inline-block, a form
    /// control and a replaced element all *shrink to fit* on `auto` and *fill* on `stretch`. Without
    /// this flag a `width: stretch` `<canvas>` or floated card collapses to its content instead of
    /// filling its column.
    pub width_stretch: bool,
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
    /// ⚠⚠⚠ **The intrinsic sizing keywords on the four min/max properties — the sidecar
    /// [`Self::width_keyword`] has always had and these never did.**
    ///
    /// `min-content` / `max-content` / `fit-content` are legal on `min-width`, `max-width`,
    /// `min-height` and `max-height` exactly as they are on `width`/`height` (CSS Sizing L3 §5), and
    /// both cascades collapsed them to `Dim::Auto` — which the clamp reads as **0** on a min and as
    /// **no limit** on a max. So the declaration parsed to a different, valid value and vanished:
    /// `max-width: min-content` left the box filling its container, `min-width: max-content` let it
    /// crush below its content. A wrong answer of the right type, the shape this project rates most
    /// dangerous, and the same defect `vertical-align: <length>` had at t922.
    ///
    /// `None` = `auto`/`none`/a length/`stretch` (all of which resolve as a plain [`Dim`]).
    pub min_width_keyword: Option<IntrinsicSize>,
    pub max_width_keyword: Option<IntrinsicSize>,
    /// The block-axis pair. All three keywords name the **content height** for a block box (its
    /// min-content and max-content block sizes are the same thing), so layout does not need to
    /// distinguish them — but the *specified* keyword is kept rather than a `bool`, because
    /// `getComputedStyle` must serialise back the keyword the author wrote.
    pub min_height_keyword: Option<IntrinsicSize>,
    pub max_height_keyword: Option<IntrinsicSize>,
    /// **`stretch` on the block-axis MIN/MAX pair** — `min-height`/`max-height` and their logical
    /// spellings `min-block-size`/`max-block-size`.
    ///
    /// Its own flag for the same reason [`Self::height_stretch`] is: `stretch` collapses to
    /// `Dim::Auto` for length resolution, and on a *max* that reads as "no limit" while on a *min*
    /// it reads as zero — so without the flag the declaration is not representable at all and the
    /// clamp silently does nothing. The `stretch` value here means *the stretch-fit block size*:
    /// the containing block's definite content height less this box's own margins, border and
    /// padding — the same quantity `height: stretch` resolves to, which is why layout computes it
    /// once (`specified_definite_h`'s stretch arm) and both readers share it.
    ///
    /// ⚠ **Inline axis deliberately NOT added in the same tick.** `min-width`/`max-width: stretch`
    /// is the exact mirror and every failing test in `css/css-sizing/stretch` exercises the BLOCK
    /// axis, so the inline half would be an unmeasured change riding along with a measured one.
    /// Named as residue rather than half-built.
    pub min_height_stretch: bool,
    pub max_height_stretch: bool,
    /// The INLINE-axis mirror — `min-width`/`max-width` and `min-inline-size`/`max-inline-size`.
    ///
    /// Chrome-measured (t1250, a 120px containing block, the child carrying 2+3px inline margins,
    /// 3px border and 2px padding → stretch-fit border box **115**):
    ///
    /// ```text
    ///                                          Chrome   before    after
    ///   max-inline-size:stretch; width:500px    115      510   ✗   115  ✓
    ///   min-inline-size:stretch; float:left     115       10   ✗   115  ✓
    ///   min-inline-size:stretch (plain block)   115      115   ✓   115  ✓   <- auto already fills
    ///   max-inline-size:stretch; width:20px      30       30   ✓    30  ✓   <- CONTROL, caps only
    /// ```
    ///
    /// The two that were wrong are exactly the two t219 named when it built `width_stretch`:
    /// *"`stretch` only differs from `auto` for the boxes that shrink-to-fit"* — a **float** is one
    /// of those, and a **max** has no `auto` behaviour to be accidentally right about.
    pub min_width_stretch: bool,
    pub max_width_stretch: bool,
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
    /// `border-spacing` (px) between table cells in the separated-borders model — the HORIZONTAL
    /// component. `border-spacing` takes one or TWO lengths and the second is the vertical one
    /// (`border_spacing_v`); a single value sets both.
    pub border_spacing: f32,
    /// The VERTICAL half of `border-spacing`. Kept separate because a table with
    /// `border-spacing: 10px 20px` insets its ROWS by 20 and its COLUMNS by 10, and folding the two
    /// made the row inset silently equal to the column one (t925: Chrome 64, ours 44).
    pub border_spacing_v: f32,
    /// `border-collapse: collapse` — cells share borders (no border-spacing).
    pub border_collapse: bool,
    /// `box-sizing` — whether `width`/`height` measure the content box or the border box.
    pub box_sizing: BoxSizing,
    /// `justify-content` — flex main-axis distribution (only meaningful on a flex container).
    pub justify_content: JustifyContent,
    /// `align-content` — distribution along the CROSS axis of a multi-line flex container, and along
    /// the BLOCK axis (the rows) of a grid. The container-level twin of [`Self::justify_content`],
    /// and the half that was missing: a wrapped flex container asking for `align-content: flex-end`
    /// laid its lines out from the top, and a grid asking for `align-content: center` left its rows
    /// at the start of the box. It shares [`JustifyContent`]'s value set — including `Normal`, which
    /// is `stretch` on this axis for BOTH formatting contexts, which is why the initial value has
    /// always looked correct.
    pub align_content: JustifyContent,
    /// `align-items` — flex cross-axis alignment (only meaningful on a flex container).
    pub align_items: AlignItems,
    /// `justify-items` — the INLINE-axis alignment a grid container hands to every item that does not
    /// override it with `justify-self`. The container-level twin of [`Self::align_items`] and the
    /// inline-axis partner of [`Self::justify_self`] (t980). Does not apply to flex containers, where
    /// the inline axis is distributed by `justify-content` instead.
    pub justify_items: AlignItems,
    /// `flex-direction` (container).
    pub flex_direction: FlexDirection,
    /// `flex-wrap` (container).
    pub flex_wrap: FlexWrap,
    /// `row-gap` / `column-gap` (container).
    ///
    /// ⚠ **`Dim`, not `f32`, and the widening IS the fix.** A bare px float has nowhere to put a
    /// PERCENTAGE, so `column-gap: 10%` was not an unhandled value — it was a value the field could
    /// not represent, and it silently became `0`. A gap percentage resolves against the container's
    /// **content-box size on that axis** (measured against Chrome: 10% of a 300px grid is 30px, and
    /// of the same grid with `padding: 0 50px` it is 20px), which is a basis only layout knows —
    /// exactly why it has to survive the cascade as a percentage rather than being resolved here.
    pub row_gap: Dim,
    pub column_gap: Dim,
    /// `flex-grow` / `flex-shrink` (item).
    pub flex_grow: f32,
    pub flex_shrink: f32,
    /// `flex-basis` (item); `Dim::Auto` = `auto`.
    pub flex_basis: Dim,
    /// `align-self` (item); `None` = `auto` (defer to the container's `align-items`).
    pub align_self: Option<AlignItems>,
    /// `justify-self` — a GRID item's own INLINE-axis alignment within its track, overriding the
    /// container's `justify-items`. `None` is `auto` (defer to the container). The align-axis twin
    /// of [`Self::align_self`], and it was the missing half: `align-self` reached taffy and this
    /// did not, so a `justify-self: end` item sat at the START of its track.
    pub justify_self: Option<AlignItems>,
    /// `transform` — an ordered list of transform functions (translate/scale/rotate/skew/
    /// matrix), resolved to an affine matrix at layout time (translate `%` is the box's own
    /// size). Empty = `none`.
    pub transform: Vec<TransformFn>,
    /// ⚠⚠⚠ **`translate` / `rotate` / `scale` — the INDIVIDUAL transform properties, and neither
    /// cascade parsed them.** CSS Transforms 2 splits the three commonest transform functions into
    /// properties of their own so an author (and every animation library) can set one without
    /// clobbering the others — `element.style.translate = '30px 15px'` does not destroy a `rotate`
    /// the stylesheet set. This engine matched only `"transform"`, so all three were **absent**
    /// rather than wrong, and the element sat untransformed. **19.3% of the burndown corpus declares
    /// at least one** (`rotate:` 12.9%, `scale:` 8.8%, `translate:` 3.5%, 171 sites fetched with
    /// their linked stylesheets).
    ///
    /// Kept as their own fields rather than folded into `transform`, for two reasons: the spec fixes
    /// the composition order (**translate, then rotate, then scale, then the `transform` list**)
    /// regardless of declaration order, which a single Vec appended to at parse time cannot honour;
    /// and `getComputedStyle(el).transform` must keep reporting the `transform` property alone.
    /// [`ComputedStyle::effective_transform`] is what layout asks for.
    pub translate: Option<(Dim, Dim)>,
    /// The RESOLVED 2D operation, not a bare angle: a rotation about x or y projects to a scale on
    /// the other axis (see [`axis_rotation_2d`]), so the field has to be able to hold either.
    pub rotate: Option<TransformFn>,
    pub scale: Option<(f32, f32)>,
    /// `vertical-align` — cross-axis alignment of an inline-level box on its line.
    pub vertical_align: VerticalAlign,
    /// `grid-template-columns` / `-rows` (container). Empty = none. A [`TrackComponent`] rather than
    /// a bare [`TrackSize`] because `repeat(auto-fill|auto-fit, …)` has no fixed count until layout
    /// knows the container's size.
    pub grid_template_columns: Vec<TrackComponent>,
    pub grid_template_rows: Vec<TrackComponent>,
    /// ⚠⚠⚠ **THE `<line-names>` OF THOSE TEMPLATES — one entry per grid LINE, i.e. tracks + 1.**
    ///
    /// `[a] repeat(4, [b] 200px [c]) [d]` names five lines, and the resolved value Chrome answers
    /// with is `[a b] 200px [c b] 200px [c b] 200px [c b] 200px [c d]` — the names are part of the
    /// serialization, not decoration. Until tick 1289 they were read by Stylo, carried through the
    /// cascade, and then **dropped at the map boundary**: `template_to_tracks` never looked at
    /// `TrackList::line_names`, so every track size came out right and every name vanished. A grid
    /// library reading its own template back got a list it could not match to its named areas.
    ///
    /// An integer `repeat(N, …)` is expanded here in **lockstep** with the sizes in
    /// [`Self::grid_template_columns`] — the two lists are consumed together by one interleave, and
    /// a length that drifts from the size list produces a misaligned name group, which is a wrong
    /// answer of the right type and strictly worse than the missing names it replaced.
    ///
    /// Empty (rather than a vector of empty vectors) when the template declares no names at all,
    /// which is the overwhelmingly common case and costs nothing.
    pub grid_template_columns_line_names: Vec<Vec<String>>,
    pub grid_template_rows_line_names: Vec<Vec<String>>,
    /// `grid-auto-rows` / `grid-auto-columns` (container) — the sizes given to the **implicit**
    /// tracks the auto-placement algorithm creates when there are more items than the explicit
    /// template has room for. A plain `<track-size>+` list with no `repeat()`, because the grammar
    /// forbids one here: the list is **cycled** over the implicit tracks instead, so
    /// `grid-auto-rows: 80px 20px` makes the implicit rows 80, 20, 80, 20… Empty = `auto`.
    pub grid_auto_rows: Vec<TrackSize>,
    pub grid_auto_columns: Vec<TrackSize>,
    /// Does a **non-`position`** property on this element make it the containing block for its
    /// out-of-flow (`absolute` / `fixed`) descendants?
    ///
    /// ⚠ This covers `will-change` (when it names a property that would create one), `contain`
    /// (`layout` / `paint` / `strict` / `content`) and `perspective`. **`transform`, `filter` and
    /// `backdrop-filter` are deliberately NOT folded in here** — they have their own fields and
    /// layout reads them directly, so mirroring them into a boolean would be two sources of one
    /// truth and the classic way they drift apart.
    ///
    /// It is a `bool` rather than the property values because that is *all layout needs*, and the
    /// alternative — carrying a `will-change` string list on every `ComputedStyle` — is the
    /// per-node allocation the custom-property field already documents as a measured mistake.
    /// ⚠ The cost of the boolean is that `getComputedStyle().willChange` cannot be served from it;
    /// we do not publish that property today, and the day we do it needs the list, not this flag.
    pub establishes_containing_block: bool,
    /// `grid-auto-flow` (container) — which axis auto-placement advances along, and whether it
    /// back-fills holes (`dense`).
    pub grid_auto_flow: GridAutoFlow,
    /// `grid-column` / `grid-row` (item) start/end line placement.
    pub grid_column: (GridLine, GridLine),
    pub grid_row: (GridLine, GridLine),
    /// Container: `grid-template-areas` resolved to named line-rects.
    pub grid_template_areas: Vec<GridAreaRect>,
    /// Item: the named area this element is placed into (via `grid-area: name`).
    pub grid_area: Option<String>,
    /// Computed CSS **custom properties** (`--foo`) resolved through the cascade, as `(name, value)`
    /// where `name` includes the leading `--`. This is what `getComputedStyle(el).getPropertyValue(
    /// '--foo')` returns — the design-token read every theming system, chart library and CSS-in-JS
    /// runtime does. Empty when the element defines/inherits no custom properties.
    /// **`Arc<str>`, not `String`, and that is a measured decision rather than a style one.** A page
    /// with a design-token sheet has hundreds of custom properties on `:root`, and they INHERIT — so
    /// every element's computed style carries a copy of essentially all of them. Measured on
    /// wix.com (575 distinct custom properties, 10,424 elements): **1.44 million entries per
    /// cascade**, which as owned `String`s was 2.9 million heap allocations per cascade and ~67% of
    /// the whole cascade's wall time. The set of distinct strings is tiny; the number of copies is
    /// enormous. Interning collapses the allocations to one per distinct string and makes cloning a
    /// `ComputedStyle` — which the recovery loop does per node — a refcount bump.
    pub custom_properties: Vec<(std::sync::Arc<str>, std::sync::Arc<str>)>,
}

impl ComputedStyle {
    /// The counter-free text of a pseudo's `content`, for callers that only want the string.
    ///
    /// ⚠ Returns the terms it CAN resolve and skips the counters, because a counter's value is not
    /// knowable here — see [`ComputedStyle::content`]. A caller that needs the rendered text must
    /// go through layout, which is the only place document order exists.
    pub fn content_text(&self) -> Option<String> {
        self.content
            .as_ref()
            .map(|parts| parts.iter().filter_map(|p| p.as_text()).collect())
    }

    /// **The transform layout must actually apply**, in the order CSS Transforms 2 §3 fixes:
    /// `translate`, then `rotate`, then `scale`, then the `transform` list — **whatever order the
    /// declarations appeared in**. That ordering rule is the whole reason these are four fields and
    /// not one appended Vec.
    ///
    /// Returns a borrow of `transform` in the overwhelmingly common case (no individual property
    /// set), so the pages that do not use them allocate nothing.
    pub fn effective_transform(&self) -> std::borrow::Cow<'_, [TransformFn]> {
        if self.translate.is_none() && self.rotate.is_none() && self.scale.is_none() {
            return std::borrow::Cow::Borrowed(&self.transform);
        }
        let mut out = Vec::with_capacity(self.transform.len() + 3);
        if let Some((x, y)) = self.translate {
            out.push(TransformFn::Translate(x, y));
        }
        if let Some(r) = self.rotate {
            out.push(r);
        }
        if let Some((x, y)) = self.scale {
            out.push(TransformFn::Scale(x, y));
        }
        out.extend_from_slice(&self.transform);
        std::borrow::Cow::Owned(out)
    }

    /// Does this box carry a transform at all — from `transform` or from any of the three
    /// individual properties? The grouping-property question (`does it establish a containing block
    /// for out-of-flow descendants`) is asked of all four, not just the list.
    pub fn has_transform(&self) -> bool {
        !self.transform.is_empty()
            || self.translate.is_some()
            || self.rotate.is_some()
            || self.scale.is_some()
    }

    /// The CSS initial values, used as the root's starting point and for
    /// non-inherited resets.
    pub fn initial() -> Self {
        ComputedStyle {
            display: Display::Inline,
            color: Rgba::BLACK,
            color_css: None,
            background_color: None,
            font_size: 16.0,
            font_weight: 400,
            font_family: vec!["sans-serif".to_string()],
            italic: false,
            line_height: 16.0 * 1.2,
            text_align: TextAlign::Left,
            text_indent: Dim::Px(0.0),
            white_space: WhiteSpace::Normal,
            text_overflow: TextOverflow::Clip,
            line_clamp: None,
            legacy_webkit_box: None,
            scroll_snap_type: ScrollSnapAxis::None,
            scroll_snap_align: ScrollSnapAlign::None,
            text_transform: TextTransform::None,
            overflow_wrap: OverflowWrap::Normal,
            word_break: WordBreak::Normal,
            direction: Direction::Ltr,
            writing_mode: WritingMode::HorizontalTb,
            letter_spacing: 0.0,
            word_spacing: 0.0,
            tab_size: TabSize::default(),
            transform_origin: (Dim::Percent(50.0), Dim::Percent(50.0)),
            margin: Sides::all(Dim::Px(0.0)),
            padding: Sides::all(Dim::Px(0.0)),
            border_width: Sides::all(0.0),
            border_color: Sides::all(Rgba::BLACK),
            border_style: Sides::all(BorderStyle::default()),
            border_radius: 0.0,
            visibility: Visibility::Visible,
            pointer_events: PointerEvents::Auto,
            user_select: UserSelect::Auto,
            color_scheme: ColorScheme::Normal,
            scrollbar_width: ScrollbarWidth::Auto,
            scrollbar_color: ScrollbarColor::Auto,
            field_sizing_content: false,
            appearance: Appearance::None,
            display_in_flow: Display::Inline,
            order: 0,
            line_height_normal: true,
            mask_image: None,
            background_images: Vec::new(),
            background_size: BackgroundSize::Auto,
            background_position: BackgroundPosition::default(),
            object_fit: ObjectFit::Fill,
            object_position: ObjectPosition::default(),
            aspect_ratio: None,
            width_is_natural: false,
            height_is_natural: false,
            background_repeat: BackgroundRepeat::Repeat,
            text_decoration: TextDecoration::default(),
            list_style_type: ListStyleType::Disc,
            list_style_inside: false,
            content: None,
            counter_reset: Vec::new(),
            counter_increment: Vec::new(),
            before: None,
            after: None,
            first_letter: None,
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
            filter: Vec::new(),
            clip_path: None,
            mix_blend_mode: BlendMode::Normal,
            backdrop_filter: Vec::new(),
            width: Dim::Auto,
            width_keyword: None,
            width_stretch: false,
            height: Dim::Auto,
            height_intrinsic: false,
            height_stretch: false,
            min_width: Dim::Auto,
            max_width: Dim::Auto,
            min_height: Dim::Auto,
            max_height: Dim::Auto,
            min_width_keyword: None,
            max_width_keyword: None,
            min_height_keyword: None,
            min_height_stretch: false,
            max_height_stretch: false,
            min_width_stretch: false,
            max_width_stretch: false,
            max_height_keyword: None,
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
            border_spacing_v: 0.0,
            border_collapse: false,
            box_sizing: BoxSizing::ContentBox,
            justify_content: JustifyContent::Normal,
            align_content: JustifyContent::Normal,
            align_items: AlignItems::Normal,
            justify_items: AlignItems::Normal,
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::NoWrap,
            row_gap: Dim::Px(0.0),
            column_gap: Dim::Px(0.0),
            flex_grow: 0.0,
            flex_shrink: 1.0,
            flex_basis: Dim::Auto,
            align_self: None,
            justify_self: None,
            transform: Vec::new(),
            translate: None,
            rotate: None,
            scale: None,
            vertical_align: VerticalAlign::Baseline,
            grid_template_columns: Vec::new(),
            grid_template_rows: Vec::new(),
            grid_template_columns_line_names: Vec::new(),
            grid_template_rows_line_names: Vec::new(),
            grid_auto_rows: Vec::new(),
            grid_auto_columns: Vec::new(),
            grid_auto_flow: GridAutoFlow::Row,
            establishes_containing_block: false,
            grid_column: (GridLine::Auto, GridLine::Auto),
            grid_row: (GridLine::Auto, GridLine::Auto),
            grid_template_areas: Vec::new(),
            grid_area: None,
            custom_properties: Vec::new(),
        }
    }

    /// **The style `getComputedStyle(el, '::before')` must report when the pseudo generates NO
    /// box** — inherited properties from the originating element, everything else initial.
    ///
    /// Chrome never answers that call with `null` and never answers it with the *element's* style:
    /// on a `<div style="width:200px">` with no `::before` rule at all it reports
    /// `content:none · display:inline · width:auto`, and it reports the div's colour and font-size
    /// because those inherit. Handing back the element's own style instead would report
    /// `display:block · width:200px` — **a wrong answer of the right type**, which is the shape
    /// this project has already paid for twice (t733, t1096).
    ///
    /// It is `inherit_from` under a name that says what it is for: the cascade drops a pseudo with
    /// no `content` (it generates no box, so layout has nothing to do), and this is what the CSSOM
    /// still owes a caller in that case.
    pub fn absent_pseudo_of(origin: &ComputedStyle) -> Self {
        Self::inherit_from(origin)
    }

    /// **The style of an ANONYMOUS box generated by `parent`** — CSS 2.1 §9.2.1.1 / §17.2.1.
    ///
    /// An anonymous box has no element, so it has no declarations of its own: every inheritable
    /// property comes from the box that generated it and every other property is at its initial
    /// value. That second half is the load-bearing one and it is why this is not
    /// `parent.clone()`: an anonymous table generated around a misparented `table-cell` must not
    /// inherit the container's `width`, `height`, `margin`, `padding`, `border` or `background` —
    /// it would paint the container's background twice and take its declared width as its own.
    ///
    /// Shares `inherit_from` with the cascade rather than re-listing the inherited set, because a
    /// second list is a second thing to keep in step (`absent_pseudo_of` is the third caller).
    pub fn anonymous_from(parent: &ComputedStyle) -> Self {
        Self::inherit_from(parent)
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
        s.text_indent = parent.text_indent;
        s.white_space = parent.white_space;
        s.text_transform = parent.text_transform;
        s.overflow_wrap = parent.overflow_wrap;
        s.word_break = parent.word_break;
        s.direction = parent.direction;
        // `writing-mode` is inherited — that is how `html { writing-mode: vertical-rl }` turns a
        // whole Japanese document vertical without naming a single descendant.
        s.writing_mode = parent.writing_mode;
        s.letter_spacing = parent.letter_spacing;
        s.word_spacing = parent.word_spacing;
        s.tab_size = parent.tab_size;
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
        border_spacing_v: style.border_spacing_v * k,
        text_indent: dim(style.text_indent, k),
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
        // `writing-mode` transposes the whole subtree's geometry — the largest reflow-forcing
        // change there is.
        || old.writing_mode != new.writing_mode
        || old.word_break != new.word_break
        || old.letter_spacing != new.letter_spacing
        || old.word_spacing != new.word_spacing
        || old.tab_size != new.tab_size
        || old.transform_origin != new.transform_origin
        || old.float != new.float
        || old.clear != new.clear
        || old.position != new.position
        || old.inset != new.inset
        || old.table_layout != new.table_layout
        || old.border_spacing != new.border_spacing
        || old.border_spacing_v != new.border_spacing_v;
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
    /// **`:nth-last-child(an+b)` — the same index counted from the END.**
    ///
    /// Absent, it fell through to `_ => return None` and took the **whole selector** with it, so
    /// `querySelectorAll('li:nth-last-child(3n)')` answered `[]` rather than a wrong set. The same
    /// silent-empty shape as `:is()` at t1194.
    NthLastChild(i32, i32),
    /// **The `-of-type` family**, whose index counts only siblings of the SAME element type rather
    /// than all element siblings. `:first-of-type` is not `:first-child` — `<p>x<em>a</em>…` has an
    /// `<em>` that is first of its type and is nobody's first child, and the two answers differ on
    /// most real markup.
    FirstOfType,
    LastOfType,
    OnlyOfType,
    NthOfType(i32, i32),
    NthLastOfType(i32, i32),
    Root,
    Empty,
    Checked,
    Disabled,
    Enabled,
    Required,
    /// `:read-only` / `:read-write` — the mutability pseudo-classes. An `<input>`/`<textarea>` WITHOUT
    /// a `readonly` attribute is `:read-write`; everything else (a readonly control, and every
    /// non-editable element like a `<p>` or `<div>`) is `:read-only`. The shipping STYLE cascade already
    /// resolves both (Stylo's `NonTSPseudoClass::ReadOnly`/`ReadWrite`), so `input:read-only { … }`
    /// renders; this makes the querySelector/`matches` engine agree, so a form library that does
    /// `querySelectorAll('input:read-write')` finds the editable fields. The rule mirrors `stylo_dom`
    /// exactly (own `readonly` attribute + input/textarea tag). `contenteditable` making an arbitrary
    /// element `:read-write` is not tracked in either engine — the one unmodelled edge, kept identical
    /// on both sides so cascade and querySelector never disagree.
    ReadOnly,
    ReadWrite,
    /// `:muted` — a media element (`<video>`/`<audio>`) that is muted. We match the `muted` content
    /// attribute (initial mute state); the live `.muted` IDL property is not tracked here, exactly as
    /// [`Pseudo::Checked`] matches the `checked` attribute and not the runtime property. querySelector
    /// only: the *servo* Stylo build has no `Muted` variant in `NonTSPseudoClass` (it is gecko-only),
    /// so `video:muted { … }` cannot cascade without vendoring Stylo — the same fence as `:has()`.
    Muted,
    /// `:open` (Baseline 2026) — a `<details>`/`<dialog>` in its open state, matched via the `open`
    /// content attribute (as [`Pseudo::Checked`] matches `checked`). The shipping STYLE cascade already
    /// handles `:open` (Stylo's `NonTSPseudoClass::Open`), so `details:open { … }` renders; this makes
    /// the querySelector/`matches` engine agree, so a disclosure-widget library that does
    /// `querySelectorAll('details:open')` finds the open ones. (`<select>`'s open state is UI-only, not
    /// an attribute, so it is out of reach here — same fence as `:checked`'s runtime-property gap.)
    Open,
    Link,
    /// `:not(<compound>)` — a single inner compound (no combinators).
    /// `:not(...)` — a COMPLEX selector list; the element matches when **none** of them do.
    ///
    /// A list, not a single compound: `:not(.a, .b)` is Selectors 4 and Baseline, and `:not(.a .b)`
    /// takes a complex member. Holding one `Compound` made both unparsable, which dropped the whole
    /// selector rather than the pseudo.
    Not(Vec<Selector>),
    /// `:is(...)` / `:where(...)` — a forgiving list of COMPLEX selectors, matched with the
    /// element under test as the subject. `:where()` shares this variant because the two differ
    /// only in specificity, which this matcher never consults (see the parse site).
    Is(Vec<Selector>),
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

/// 1-based index of `node` among its element siblings **of the same type**, and the total count of
/// those. The `-of-type` twin of [`element_sibling_position`], and the only difference is the tag
/// filter — which is exactly the difference `:first-child` and `:first-of-type` disagree on.
///
/// The "type" is the local name, as the Dom stores it. A namespace-aware comparison would need
/// namespaces on `Element`, which this arena does not carry per-node; HTML content is all one
/// namespace, so the distinction only bites on inline SVG/MathML with colliding local names.
fn type_sibling_position(dom: &Dom, node: NodeId) -> (usize, usize) {
    let Some(name) = dom.element(node).map(|e| e.name.clone()) else {
        return (1, 1);
    };
    let Some(parent) = dom.parent(node) else {
        return (1, 1);
    };
    let mut index = 0;
    let mut total = 0;
    for c in dom.children(parent) {
        if dom.element(c).map(|e| e.name == name).unwrap_or(false) {
            total += 1;
            if c == node {
                index = total;
            }
        }
    }
    (index.max(1), total.max(1))
}

/// **`idx == a*n + b` for some integer `n >= 0`** — the whole of `An+B` matching, in one place.
///
/// Written once because four pseudo-classes need it and a copy that drifts is how
/// `:nth-child` and `:nth-of-type` end up disagreeing about `-n+3`. The `a == 0` case is a plain
/// equality (`:nth-child(3)`); otherwise the division is checked by re-multiplying, so a
/// non-divisible index fails rather than rounding into a match.
fn nth_matches(a: i32, b: i32, idx: i32) -> bool {
    if a == 0 {
        idx == b
    } else {
        let n = (idx - b) / a;
        n >= 0 && a * n + b == idx
    }
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

/// Whether `node` is a form control that is *actually* disabled — by its own `disabled` attribute OR by
/// an ancestor `<fieldset disabled>` (the idiomatic bulk-disable of a whole form section). This is what
/// `:disabled` matches and the negation of what `:enabled` matches. The same rule the focus path uses
/// (`Page::is_disabled`), so the live cascade, `querySelector`, and focusability all agree — an
/// `input:disabled { opacity:.5 }` rule now greys out a control disabled via its fieldset, not just one
/// with its own attribute. (The first-`<legend>` exception — controls in a disabled fieldset's first
/// legend stay enabled — is a niche edge we do not model, matching `is_disabled`.)
pub(crate) fn is_disabled_control(dom: &Dom, node: NodeId) -> bool {
    let Some(el) = dom.element(node) else {
        return false;
    };
    if !matches!(
        el.name.as_str(),
        "input" | "select" | "textarea" | "button" | "fieldset" | "optgroup" | "option"
    ) {
        return false;
    }
    if el.attr("disabled").is_some() {
        return true;
    }
    let mut cur = dom.parent(node);
    while let Some(n) = cur {
        if dom
            .element(n)
            .is_some_and(|e| e.name == "fieldset" && e.attr("disabled").is_some())
        {
            return true;
        }
        cur = dom.parent(n);
    }
    false
}

/// Is `node` editable via `contenteditable`? Walks self → ancestors: the nearest element with an
/// EXPLICIT contenteditable state wins — `""`/`true`/`plaintext-only` ⇒ editable, `false` ⇒ not; the
/// attribute absent or `inherit`/unknown keeps walking up. Mirrors the JS `isContentEditable` shim
/// (`event_loop.rs`, tick 456), minus `document.designMode` — a JS runtime property the cascade cannot
/// see (the one unmodelled edge, and a rare one). This is what makes `:read-write`/`:read-only` agree
/// with `el.isContentEditable`: a `<div contenteditable>` is `:read-write`, not `:read-only`.
pub(crate) fn is_contenteditable(dom: &Dom, node: NodeId) -> bool {
    let mut cur = Some(node);
    while let Some(n) = cur {
        if let Some(el) = dom.element(n) {
            if let Some(v) = el.attr("contenteditable") {
                let v = v.trim().to_ascii_lowercase();
                if v.is_empty() || v == "true" || v == "plaintext-only" {
                    return true;
                }
                if v == "false" {
                    return false;
                }
                // `inherit` or an invalid value → not an explicit state; keep walking up.
            }
        }
        cur = dom.parent(n);
    }
    false
}

// The `:has()` memo for ONE cascade pass — see `HasMemoScope`, and the `Pseudo::Has` arm of
// `pseudo_matches` for why it exists (a Bar 0 hang, not an optimisation).
//
// `None` means **no scope is open**, which is the default and the safe state: every `:has()` is
// computed. A cache that is ambient would answer from a DOM that has since changed; this one only
// exists while a caller has promised not to mutate.
thread_local! {
    static HAS_MEMO: std::cell::RefCell<Option<std::collections::HashMap<(usize, NodeId), bool>>> =
        const { std::cell::RefCell::new(None) };
}

/// Opens the `:has()` memo for as long as this value lives. **The contract is that the DOM does not
/// change while it is open** — which is exactly true of a cascade pass, and is why the scope is a
/// guard the cascade creates rather than a cache the matcher owns.
///
/// Nested scopes are safe: an inner one finds a memo already open and leaves it alone, so the
/// outermost scope owns the lifetime. Without that, an inner `Drop` would close the cache the outer
/// pass was still using.
pub struct HasMemoScope {
    owner: bool,
}

impl HasMemoScope {
    /// Open the memo (or join the one already open).
    pub fn new() -> Self {
        let owner = HAS_MEMO.with(|m| {
            let mut m = m.borrow_mut();
            if m.is_none() {
                *m = Some(std::collections::HashMap::new());
                true
            } else {
                false
            }
        });
        Self { owner }
    }
}

impl Default for HasMemoScope {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for HasMemoScope {
    fn drop(&mut self) {
        if self.owner {
            HAS_MEMO.with(|m| *m.borrow_mut() = None);
        }
    }
}

// **The count of UNCACHED `:has()` evaluations** — the gate's readout, and the reason the gate is a
// counter rather than a stopwatch. A timing assertion on a shared CI box is a flake; "how many times
// did the expensive thing actually run" is the quantity the fix is about, and it is exact.
thread_local! {
    static HAS_EVALS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// How many `:has()` branch searches have actually run on this thread. Test-facing.
pub fn has_evaluations() -> u64 {
    HAS_EVALS.with(|c| c.get())
}

/// Reset the `:has()` evaluation counter. Test-facing.
pub fn reset_has_evaluations() {
    HAS_EVALS.with(|c| c.set(0));
}

/// Read a memoised `:has()` answer, if a scope is open and it has been computed.
///
/// ⚠ The borrow is released before the caller computes anything — a `:has()` branch can contain
/// another `:has()`, so holding it across the evaluation would be a re-entrant `RefCell` panic
/// (a Bar 0 crash introduced by the fix for a Bar 0 hang).
fn has_memo_get(key: (usize, NodeId)) -> Option<bool> {
    HAS_MEMO.with(|m| m.borrow().as_ref().and_then(|c| c.get(&key).copied()))
}

/// Record a `:has()` answer. A no-op when no scope is open.
fn has_memo_put(key: (usize, NodeId), value: bool) {
    HAS_MEMO.with(|m| {
        if let Some(c) = m.borrow_mut().as_mut() {
            c.insert(key, value);
        }
    });
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
            nth_matches(*a, *b, idx as i32)
        }
        Pseudo::NthLastChild(a, b) => {
            let (idx, total) = element_sibling_position(dom, node);
            nth_matches(*a, *b, (total - idx + 1) as i32)
        }
        Pseudo::FirstOfType => type_sibling_position(dom, node).0 == 1,
        Pseudo::LastOfType => {
            let (idx, total) = type_sibling_position(dom, node);
            idx == total
        }
        Pseudo::OnlyOfType => type_sibling_position(dom, node).1 == 1,
        Pseudo::NthOfType(a, b) => {
            let (idx, _) = type_sibling_position(dom, node);
            nth_matches(*a, *b, idx as i32)
        }
        Pseudo::NthLastOfType(a, b) => {
            let (idx, total) = type_sibling_position(dom, node);
            nth_matches(*a, *b, (total - idx + 1) as i32)
        }
        Pseudo::Root => dom
            .parent(node)
            .map(|p| !dom.is_element(p))
            .unwrap_or(false),
        Pseudo::Empty => !dom.children(node).any(|c| {
            dom.is_element(c) || matches!(dom.data(c), NodeData::Text(t) if !t.trim().is_empty())
        }),
        Pseudo::Checked => el.attr("checked").is_some() || el.attr("selected").is_some(),
        Pseudo::Disabled => is_disabled_control(dom, node),
        Pseudo::Open => el.attr("open").is_some(),
        Pseudo::Enabled => {
            matches!(
                el.name.as_str(),
                "input" | "button" | "select" | "textarea" | "option"
            ) && !is_disabled_control(dom, node)
        }
        Pseudo::Required => el.attr("required").is_some(),
        // Mirror `stylo_dom.rs` so the two engines agree: `:read-only` is a readonly input/textarea OR
        // any non-editable element; `:read-write` is an input/textarea without `readonly`.
        // `:read-write` = an editable input/textarea, OR a `contenteditable` host (t456's editability,
        // now visible to the cascade). `:read-only` is its exact complement — a plain <div> is
        // `:read-only`, a `<div contenteditable>` is `:read-write`.
        Pseudo::ReadWrite => {
            (matches!(el.name.as_str(), "input" | "textarea") && el.attr("readonly").is_none())
                || is_contenteditable(dom, node)
        }
        Pseudo::ReadOnly => !pseudo_matches(&Pseudo::ReadWrite, dom, node),
        Pseudo::Muted => {
            matches!(el.name.as_str(), "video" | "audio") && el.attr("muted").is_some()
        }
        Pseudo::Link => {
            matches!(el.name.as_str(), "a" | "area" | "link") && el.attr("href").is_some()
        }
        Pseudo::Not(list) => !list.iter().any(|s| selector_matches(s, dom, node)),
        // Any member matching, with THIS node as the subject, is a match.
        Pseudo::Is(list) => list.iter().any(|s| selector_matches(s, dom, node)),
        // `:has(...)` — does ANY element in the anchor's relative scope match the branch selector?
        //
        // ⚠⚠⚠ **`:has()` WAS QUADRATIC, AND ONE WPT TEST IS NAMED AFTER EXACTLY THAT.**
        // `css/selectors/invalidation/has-complexity.html` — *":has() invalidation should not be
        // O(n^2)"* — builds 75,000 elements under one `<main>` and asserts the page still responds.
        // It did not: the cascade visits every node, `main:has(span) span` makes every one of those
        // spans walk up to `<main>` and re-run the subtree search, so the work is
        // `nodes × subtree`. Measured on the ladder below, each doubling costs **4×**:
        //
        // ```text
        //     n     250    500   1000   2000   4000        75000 (the test)
        //    ms      41    133    551   2074   8176        ~48 MINUTES, extrapolated
        // ```
        //
        // The runner reported it as `CRASH (killed by a signal)` — the watchdog killing a page that
        // had stopped responding. **That is Bar 0**, not a conformance miss: any real page with a
        // `:has()` rule and a few thousand elements froze the tab.
        //
        // The answer is the same one Chrome and WebKit reach: **the `:has()` question is asked of the
        // ANCHOR, and the anchor is asked the same question over and over.** `main:has(span)` has one
        // answer for `<main>`, and it was being recomputed once per span. Memoising `(this exact
        // :has() pseudo, this node) -> bool` for the duration of one cascade collapses
        // `nodes × subtree` back to `subtree`, because the second and later askers are a hash lookup.
        //
        // ⚠ **THE MEMO IS SCOPED, NOT AMBIENT, AND THAT IS THE WHOLE OF ITS SAFETY.** It is only
        // open inside a [`HasMemoScope`], which a cascade pass opens over a DOM it does not mutate
        // and closes on drop. With no scope open there is no cache and every call computes — so a
        // caller that mutates between queries (`querySelectorAll` from script, say) cannot read a
        // stale answer, because it never had one to read.
        Pseudo::Has(branches) => {
            // Key on the branch list's own allocation: two different `:has()` pseudos are two
            // different `Vec`s. An EMPTY branch list is the one case that can collide across
            // pseudos, and it is harmless — `.any()` over nothing is `false` for all of them.
            let key = (branches.as_ptr() as usize, node);
            if let Some(hit) = has_memo_get(key) {
                return hit;
            }
            let out = has_branches_match(branches, dom, node);
            has_memo_put(key, out);
            out
        }
        // A pseudo-ELEMENT never *filters* the element — the rule matches the originating element
        // and styles its generated box. The cascade routes those declarations to `before`/`after`.
        Pseudo::Before | Pseudo::After => true,
        Pseudo::NeverStatic => false,
    }
}

/// The uncached `:has()` evaluation — the search space is decided by the leading combinator, and
/// getting that right is the whole cost of the feature: a descendant `:has()` searches the subtree,
/// a child `:has()` searches one level, and a sibling `:has()` searches forward among siblings.
/// Searching the subtree for a sibling selector would be both wrong and slow.
fn has_branches_match(branches: &[(Combinator, Selector)], dom: &Dom, node: NodeId) -> bool {
    HAS_EVALS.with(|c| c.set(c.get() + 1));
    branches.iter().any(|(comb, sel)| match comb {
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
    })
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
    /// The `@media` conditions this rule is nested inside, outermost first — evaluated at
    /// **cascade** time, not parse time, so a resize re-decides them. Empty = unconditional.
    ///
    /// A list rather than one string because nesting is *conjunction*, and stitching two
    /// preludes into one string would have to invent a syntax CSS does not have (a media type
    /// cannot be parenthesised, so `(screen) and (min-width:0)` is not a valid query).
    media: Vec<String>,
}

impl Rule {
    /// Do every enclosing `@media` condition hold right now? Evaluated per cascade, so a
    /// viewport change re-decides it without reparsing.
    fn media_applies(&self) -> bool {
        self.media.iter().all(|q| media_matches(q))
    }
}

/// One `:has()` selector lifted out of the stylesheets, with everything the per-element pass needs.
///
/// **This type exists because the filtering was per-ELEMENT and depends only on the STYLESHEET.**
/// `apply_has_rules` used to re-walk every rule of every `:has()`-carrying sheet for every element,
/// re-evaluating each rule's `@media` and re-asking each selector whether it was relative — work whose
/// answer cannot change between elements. Third instance of the same defect class as t572's
/// `cascade_pseudo` and t573's `property_at(i)`: *work that depends only on the stylesheet, done once
/// per element.*
///
/// **Measured, and the interesting part is WHICH n drives it.** Quadrupling the rules *within* a sheet
/// (600 → 2,400, same elements) moved the cascade barely at all — the inner scan short-circuits on
/// `has_relative()` and costs ~0.2 ns an iteration. Multiplying the *sheets* is what costs: on 60 sheets
/// × 18,125 elements, adding one `:has()` rule per sheet moved the cascade **19.7 → 22.7 ms, 20.7 → 24.3,
/// 21.9 → 23.8 — about +14%**, because the per-element loop runs `for sh in &has_sheets` and pays the
/// whole scan again for each. A first attempt at this measurement varied the wrong n and showed nothing.
pub struct RelativeRule<'a> {
    sel: &'a Selector,
    spec: u32,
    order: usize,
    decls: &'a [Declaration],
}

/// Lift every `:has()` selector out of `sheets` **once**, in global source order.
///
/// Cheap and done per cascade rather than per element; the returned slice is what
/// [`apply_relative_rules`] walks. Sheets are indexed by a large stride so a later sheet's rules always
/// sort after an earlier sheet's, which is the document order the cascade tie-break assumes.
pub fn collect_relative_rules<'a>(sheets: &[&'a Stylesheet]) -> Vec<RelativeRule<'a>> {
    let mut out = Vec::new();
    for (si, sh) in sheets.iter().enumerate() {
        sh.relative_rules(si * 1_000_000, &mut out);
    }
    out
}

/// Apply the pre-collected `:has()` rules that match `node`.
///
/// The per-element cost is now proportional to the number of `:has()` **selectors on the page** — which
/// is small, and is the number that should have governed it all along — rather than to
/// `elements × sheets × rules`.
pub fn apply_relative_rules(
    index: &[RelativeRule<'_>],
    dom: &Dom,
    node: NodeId,
    style: &mut ComputedStyle,
    parent_font_size: f32,
) -> usize {
    let mut winners: Vec<(u32, usize, &Declaration)> = Vec::new();
    for r in index {
        if selector_matches(r.sel, dom, node) {
            for d in r.decls {
                winners.push((r.spec, r.order, d));
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

/// An `@font-face` rule: the family name it defines and its candidate source URLs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontFace {
    /// `font-family` (lowercased, dequoted) — the name author CSS references.
    pub family: String,
    /// `src` `url(...)` candidates, in order.
    pub srcs: Vec<String>,
    /// **`unicode-range`, as inclusive codepoint spans — `None` means the spec's default `U+0-10FFFF`.**
    ///
    /// ⚠⚠⚠ **THIS IS THE DESCRIPTOR THAT SAYS *WHICH* FACE, AND WITHOUT IT A FAMILY IS A BAG.** The
    /// inlined Google-Fonts block is the commonest webfont delivery on the web and it is subsetted by
    /// codepoint: `www.kuechenmomente.de` ships **100 `@font-face` rules all named `Raleway`** —
    /// weights {400,700} × styles {normal,italic} × ~13 subsets — with **Cyrillic and Vietnamese
    /// first in source order** and Latin further down. Registered without their ranges they all land
    /// in one family list where `FontContext::face_id` selects on weight and style alone, so a
    /// Cyrillic subset and the Latin subset are indistinguishable and a face chosen for 400/normal
    /// may have no Latin glyph to shape with. Measured (t1153): our `Raleway/18` advance was **240**
    /// against Chrome's **166** — every text box on the page 45% too wide, re-wrapping prose and
    /// arriving downstream as `dy`, where it was scored as *shape*.
    ///
    /// ⚠ **The performance half is the same field.** With no range there is no reason not to fetch
    /// all hundred subsets; a page Chrome serves with ONE woff2 costs us a hundred requests against a
    /// render deadline. `unicode-range` is how the right face is chosen *and* how the other
    /// ninety-nine are never asked for.
    pub unicode_range: Option<Vec<(u32, u32)>>,
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
        apply_relative_rules(
            &collect_relative_rules(std::slice::from_ref(&self)),
            dom,
            node,
            style,
            parent_font_size,
        )
    }

    /// Whether this sheet contains any `:has()` rule at all — the cheap check that keeps the supplement
    /// off the hot path for the 87% of sheets that do not use it.
    pub fn has_relative_rules(&self) -> bool {
        self.rules
            .iter()
            .any(|r| r.selectors.iter().any(|s| s.has_relative()))
    }

    /// The sheet's `:has()` selectors, with the declarations they carry — the per-sheet half of
    /// [`collect_relative_rules`]. `order` is offset by `base` so source order stays global across
    /// sheets, which is what the cascade tie-break needs.
    fn relative_rules<'a>(&'a self, base: usize, out: &mut Vec<RelativeRule<'a>>) {
        for (i, rule) in self.rules.iter().enumerate() {
            if !rule.media_applies() {
                continue;
            }
            for sel in &rule.selectors {
                if sel.has_relative() {
                    out.push(RelativeRule {
                        sel,
                        spec: sel.specificity(),
                        order: base + i,
                        decls: &rule.declarations,
                    });
                }
            }
        }
    }

    /// The raw CSS text this sheet was parsed from (for the Stylo cascade path).
    pub fn source(&self) -> &str {
        &self.source
    }

    /// The `@font-face` rules this sheet declares.
    pub fn font_faces(&self) -> &[FontFace] {
        &self.font_faces
    }

    /// Every `@import` URL this sheet declares, in source order.
    ///
    /// **We never fetched these, and it costs whole stylesheets.** `@import
    /// url(https://fonts.googleapis.com/css?family=Lora:400,700)` inside an external sheet is how a very
    /// large share of the web delivers its fonts — and how CSS architectures split a design system into
    /// partials behind one entry point. Dropping the import drops **every rule and every `@font-face` in
    /// the imported sheet**, silently.
    ///
    /// Measured at t563/t564 on `martinfowler.com`: its `home.css` `@import`s Open Sans, Inconsolata and
    /// Lora from Google Fonts, so Chromium resolved `{Lora/13}` where we fell back to `{serif/13}` — and
    /// that one substitution made a `<p>` **293px wide in Chromium and 619px in ours**, a different wrap
    /// width cascading through everything below it. The font was the visible symptom; the unfetched
    /// stylesheet is the cause.
    ///
    /// Returns the raw URL text (relative or absolute) exactly as authored — resolution against the
    /// sheet's own base URL is the caller's job, because only the caller knows it.
    pub fn imports(&self) -> Vec<String> {
        let mut out = Vec::new();
        let b = self.source.as_bytes();
        let mut i = 0;
        while let Some(pos) = self.source[i..].find("@import") {
            let mut j = i + pos + "@import".len();
            // `@import <url> [media];` — take everything up to the terminating `;` (or `{`, which means
            // this is not a well-formed import and the sheet's error recovery should skip it).
            let start = j;
            while j < b.len() && b[j] != b';' && b[j] != b'{' {
                j += 1;
            }
            if j < b.len() && b[j] == b';' {
                let spec = self.source[start..j].trim();
                if let Some(u) = import_url(spec) {
                    out.push(u);
                }
            }
            i = (j + 1).min(b.len());
            if i >= b.len() {
                break;
            }
        }
        out
    }
}

/// Parse an `@font-face` block body into a [`FontFace`] (`family` + `src` urls).
fn parse_font_face_block(block: &str) -> Option<FontFace> {
    let mut family = None;
    let mut srcs = Vec::new();
    let mut unicode_range = None;
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
            "unicode-range" => unicode_range = parse_unicode_range(&d.value),
            _ => {}
        }
    }
    let family = family.filter(|f| !f.is_empty())?;
    (!srcs.is_empty()).then_some(FontFace {
        family,
        srcs,
        unicode_range,
    })
}

/// Parse a `unicode-range` descriptor into inclusive codepoint spans.
///
/// Three forms, all of which the Google-Fonts block uses (CSS Fonts §4.5):
///
/// ```text
///   U+0-7F                single span, hex, case-insensitive
///   U+0460-052F           an explicit range
///   U+4??                 a WILDCARD range — `?` spans 0..=F in that digit, so U+400-4FF
/// ```
///
/// ⚠ **An unparseable component makes the WHOLE descriptor invalid** (CSS Fonts §4.5: the value is
/// a comma-separated list and a syntax error invalidates the declaration), which returns `None` and
/// therefore means *"all codepoints"* at the call site — the conservative direction. Dropping just
/// the bad component would silently NARROW a face's coverage and could hide a face the page needs,
/// which is the failure this whole field exists to prevent.
fn parse_unicode_range(v: &str) -> Option<Vec<(u32, u32)>> {
    let mut out = Vec::new();
    for part in v.split(',') {
        let p = part.trim();
        let body = p
            .strip_prefix("U+")
            .or_else(|| p.strip_prefix("u+"))
            .or_else(|| p.strip_prefix("U-"))
            .or_else(|| p.strip_prefix("u-"))?;
        if let Some((lo, hi)) = body.split_once('-') {
            let lo = u32::from_str_radix(lo.trim(), 16).ok()?;
            let hi = u32::from_str_radix(hi.trim(), 16).ok()?;
            if lo > hi {
                return None;
            }
            out.push((lo, hi));
        } else if body.contains('?') {
            // A wildcard is a range: every `?` is 0 at the low end and F at the high end. The digits
            // before the first `?` must be literal hex, and a `?` may not be followed by a non-`?`.
            let (head, tail) = body.split_at(body.find('?')?);
            if tail.chars().any(|c| c != '?') || body.len() > 6 {
                return None;
            }
            let lo = u32::from_str_radix(&format!("{head}{}", "0".repeat(tail.len())), 16).ok()?;
            let hi = u32::from_str_radix(&format!("{head}{}", "F".repeat(tail.len())), 16).ok()?;
            out.push((lo, hi));
        } else {
            let c = u32::from_str_radix(body.trim(), 16).ok()?;
            out.push((c, c));
        }
    }
    (!out.is_empty()).then_some(out)
}

/// Pull the URL out of an `@import` prelude: `url("a.css")`, `url(a.css)`, `'a.css'`, `"a.css"`,
/// optionally followed by a media query list (which we ignore here — a conditional import still needs
/// fetching; the enclosing `@media` decides application, not delivery).
fn import_url(spec: &str) -> Option<String> {
    let spec = spec.trim();
    let inner = if let Some(rest) = spec
        .strip_prefix("url(")
        .or_else(|| spec.strip_prefix("URL("))
    {
        // `url(` … `)` — the media list, if any, follows the closing paren.
        let end = rest.find(')')?;
        &rest[..end]
    } else {
        // A bare string form: take the quoted run, ignoring any trailing media list.
        let q = spec.chars().next()?;
        if q != '"' && q != '\'' {
            return None;
        }
        let rest = &spec[1..];
        let end = rest.find(q)?;
        &rest[..end]
    };
    let u = inner.trim().trim_matches(['"', '\'']).trim();
    (!u.is_empty()).then(|| u.to_string())
}

impl Stylesheet {
    /// Parse CSS source into rules. Comments and `@`-rules are skipped; unknown
    /// selectors/properties are ignored rather than aborting the sheet (CSS's
    /// forward-compatible error recovery).
    pub fn parse(src: &str) -> Stylesheet {
        let src = strip_cdata(src);
        let src = strip_comments(&src);
        let mut rules = Vec::new();
        let mut font_faces = Vec::new();
        parse_rules_into(&src, &[], &mut rules, &mut font_faces);
        Stylesheet {
            rules,
            source: src,
            font_faces,
        }
    }
}

/// Parse `src` into `rules`, tagging every rule with `media` (the enclosing `@media`
/// condition, if any).
///
/// **`@media` is DESCENDED INTO, not skipped.** It used to be skipped along with every other
/// at-rule, which silently deleted every rule inside it. Under the Stylo cascade that was
/// invisible for most properties — Stylo re-parses the source itself — but a dozen properties
/// (`visibility`, `background-image`/`-size`/`-position`, `mask-image`, `border-style`,
/// `text-shadow`, `object-fit`/`-position`, `vertical-align`, …) are recovered from *this*
/// cascade because Stylo's servo build does not expose them. So `@media (max-width: 700px) {
/// .panel { visibility: hidden } }` computed `visible`, and every conditional `<link
/// media="…">` sheet — which the page pipeline wraps in `@media` precisely so the cascade
/// decides — lost the same dozen properties wholesale.
fn parse_rules_into(
    src: &str,
    media: &[String],
    rules: &mut Vec<Rule>,
    font_faces: &mut Vec<FontFace>,
) {
    let bytes = src.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        // @-rules: capture @font-face (for web fonts), DESCEND into @media, skip the rest.
        if bytes[i] == b'@' {
            let end = skip_at_rule(src, i);
            let rest = &src[i..];
            // The block's own `{`, if it has one. A STATEMENT at-rule (`@layer a, b;`,
            // `@import …;`) ends at the `;`, so a bare `rest.find('{')` would find the brace of
            // some LATER rule and slice past `end`. Anything at or after `end` is not ours.
            let block_open = rest.find('{').filter(|o| i + o < end);
            // `get(..n)` and not `[..n]`: the byte-length guard alone let a multi-byte at-rule
            // name (`@媒体…` — netlify.com's sheet, found by the tick-380 oracle run) land the
            // slice mid-character and PANIC the engine. `get` returns None on a non-boundary,
            // which is exactly "not this keyword" — the rule is skipped like any unknown at-rule.
            let at_kw =
                |n: usize, kw: &str| rest.get(..n).is_some_and(|p| p.eq_ignore_ascii_case(kw));
            if at_kw(10, "@font-face") {
                if let Some(open) = block_open {
                    let block = &src[i + open + 1..end.saturating_sub(1)];
                    if let Some(ff) = parse_font_face_block(block) {
                        font_faces.push(ff);
                    }
                }
            } else if at_kw(9, "@supports") {
                // `@supports` is the same defect as `@media` with a different at-keyword: the
                // block was skipped, so the twelve properties recovered from this cascade were
                // exempt from it. The condition is evaluated, never assumed — a false `@supports`
                // is a fallback the author wrote for browsers that do not support the thing.
                if let Some(open) = block_open {
                    if supports_condition_matches(rest[9..open].trim()) {
                        let body = &src[i + open + 1..end.saturating_sub(1)];
                        parse_rules_into(body, media, rules, font_faces);
                    }
                }
            } else if at_kw(6, "@layer") {
                // `@layer name { … }` — descend. Layered rules should LOSE to unlayered ones at
                // equal specificity, which this cascade cannot yet express, so this is knowingly
                // approximate. It is still strictly closer than the previous behaviour, which was
                // to delete the contents outright. `@layer a, b;` (the statement form, no block)
                // has no rules to descend into and is skipped by the same code.
                if let Some(open) = block_open {
                    let body = &src[i + open + 1..end.saturating_sub(1)];
                    parse_rules_into(body, media, rules, font_faces);
                }
            } else if at_kw(6, "@media") {
                if let Some(open) = block_open {
                    let prelude = rest[6..open].trim();
                    let body = &src[i + open + 1..end.saturating_sub(1)];
                    // Nesting is conjunction: an inner block applies only when both hold.
                    let mut inner = media.to_vec();
                    inner.push(prelude.to_string());
                    parse_rules_into(body, &inner, rules, font_faces);
                }
            }
            i = end;
            continue;
        }
        {
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
                    media: media.to_vec(),
                });
            }
        }
    }
}

/// Evaluate an `@supports` condition — **by actually trying the declaration**.
///
/// The honest way to answer "do you support `display: grid`?" is not a hand-maintained list of
/// property names, which is a second source of truth that goes stale the moment a property is
/// implemented or removed. It is to parse the declaration, apply it to a default
/// [`ComputedStyle`], and see whether anything changed. A property this cascade does not implement,
/// or a value it does not recognise, leaves the style untouched.
///
/// **The probe is conservative by construction, and that is the safe direction.** A declaration
/// whose value happens to equal the initial value (`@supports (display: block)`) reads as
/// unsupported, so the block does not apply — which is exactly what happened before this function
/// existed. It can only ever be as wrong as the old behaviour, never newly wrong.
///
/// Supports `not`, `and`, `or`, nested parens, and `selector(…)` (answered by whether our own
/// selector parser accepts it). An unparseable condition is **false**, matching `media_matches`
/// and CSS's own error handling: never guess in the direction of applying a stylesheet.
fn supports_condition_matches(cond: &str) -> bool {
    let c = cond.trim();
    if c.is_empty() {
        return false;
    }
    if let Some(rest) = c.strip_prefix("not ").or_else(|| c.strip_prefix("not(")) {
        let inner = if c.starts_with("not(") { &c[3..] } else { rest };
        return !supports_condition_matches(inner);
    }
    // Top-level `and` / `or`, split at paren depth 0. CSS forbids mixing them without parens, so
    // whichever appears first decides how the whole level combines.
    let b = c.as_bytes();
    let (mut depth, mut i) = (0i32, 0usize);
    let mut parts: Vec<&str> = Vec::new();
    let mut op: Option<bool> = None; // Some(true) = and, Some(false) = or
    let mut start = 0usize;
    while i < b.len() {
        match b[i] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            _ => {}
        }
        if depth == 0 {
            let and = c[i..].starts_with(" and ");
            let or = c[i..].starts_with(" or ");
            if (and && op != Some(false)) || (or && op != Some(true)) {
                op.get_or_insert(and);
                parts.push(&c[start..i]);
                i += if and { 5 } else { 4 };
                start = i;
                continue;
            }
        }
        i += 1;
    }
    if let Some(is_and) = op {
        parts.push(&c[start..]);
        return if is_and {
            parts.iter().all(|p| supports_condition_matches(p))
        } else {
            parts.iter().any(|p| supports_condition_matches(p))
        };
    }
    // A single term: `selector(…)`, a parenthesised group, or a bare `prop: value`.
    if let Some(sel) = c
        .strip_prefix("selector(")
        .and_then(|s| s.strip_suffix(')'))
    {
        return !parse_selector_list(sel).is_empty();
    }
    if let Some(inner) = c.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
        // A nested group, or the declaration itself. Try it as a group first only if it still
        // looks like a condition; otherwise fall through to the declaration probe.
        if inner.contains(" and ") || inner.contains(" or ") || inner.starts_with("not ") {
            return supports_condition_matches(inner);
        }
        return declaration_is_supported(inner);
    }
    declaration_is_supported(c)
}

/// Does this cascade actually implement `prop: value`? Applies it to a default style and checks
/// whether anything moved. See [`supports_condition_matches`] for why this beats a name list.
fn declaration_is_supported(decl: &str) -> bool {
    let decls = parse_declarations(decl);
    if decls.is_empty() {
        return false;
    }
    let base = ComputedStyle::initial();
    let mut probe = base.clone();
    for d in &decls {
        apply_declaration(&mut probe, d, 16.0);
    }
    probe != base
}

/// Media Queries Level 4 evaluates in **FOUR** states, and collapsing them to a `bool` inverts
/// whole stylesheets.
///
/// The two extra states are not pedantry — they are the only way `not` can be right:
///
/// * **`Unknown`** is `<general-enclosed>`: a syntactically well-formed `( … )` block whose feature
///   this UA does not recognise. MQ4 §3.2 gives it Kleene logic — `not unknown` is **`Unknown`**,
///   *not* `True`. A three-state evaluator that folds `Unknown` into `False` and then negates
///   answers **`true`** for `@media not (some-2029-feature)` and applies a sheet written for a
///   browser we are not.
/// * **`Invalid`** is a grammar failure. MQ4 says such a query *"must be replaced with `not all`"* —
///   and the replacement happens at the **whole-query** level, so it survives an enclosing `not`.
///   `not )` is false; it is not "the negation of a false thing".
///
/// Both collapse to `false` at the top, which is why the old `bool` evaluator looked right on every
/// query that contained no `not`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mq {
    True,
    False,
    /// `<general-enclosed>` — well-formed, unrecognised. Kleene's third value.
    Unknown,
    /// A grammar failure. Absorbing: it survives `not`, `and` and `or` alike.
    Invalid,
}

impl Mq {
    fn of(b: bool) -> Mq {
        if b {
            Mq::True
        } else {
            Mq::False
        }
    }
    fn not(self) -> Mq {
        match self {
            Mq::True => Mq::False,
            Mq::False => Mq::True,
            other => other, // Unknown and Invalid both negate to themselves.
        }
    }
    fn and(self, o: Mq) -> Mq {
        match (self, o) {
            (Mq::Invalid, _) | (_, Mq::Invalid) => Mq::Invalid,
            (Mq::False, _) | (_, Mq::False) => Mq::False,
            (Mq::Unknown, _) | (_, Mq::Unknown) => Mq::Unknown,
            _ => Mq::True,
        }
    }
    fn or(self, o: Mq) -> Mq {
        match (self, o) {
            (Mq::Invalid, _) | (_, Mq::Invalid) => Mq::Invalid,
            (Mq::True, _) | (_, Mq::True) => Mq::True,
            (Mq::Unknown, _) | (_, Mq::Unknown) => Mq::Unknown,
            _ => Mq::False,
        }
    }
}

/// Evaluate a `@media` prelude — a `<media-query-list>` — against the current viewport.
///
/// **Public because `window.matchMedia` must give the same answer.** A page that branches in JS on
/// `matchMedia('(max-width: 700px)')` and in CSS on the identical query and gets two different
/// answers renders a layout no designer ever specified. There was a second, independent evaluator
/// in the JS prelude with its own feature table and an `unknown → true` default, i.e. the exact
/// opposite of this one's `unknown → false`; both are now this function.
///
/// A `<media-query-list>` is a comma-separated list of `<media-query>`, and a `<media-query>` is
/// **either** a bare `<media-condition>` **or** `[not | only]? <media-type> [and <condition>]?`.
/// That distinction is load-bearing: `not print` is a *query* and is TRUE on a screen, while inside
/// a `sizes` attribute — which takes a `<media-condition>`, see [`media_condition_matches`] — the
/// same text is a grammar error and is FALSE. One string, two answers, and the difference is which
/// production asked.
pub fn media_matches(query: &str) -> bool {
    split_top_level(query, ",")
        .into_iter()
        .any(|q| !q.trim().is_empty() && eval_media_query(&q.to_ascii_lowercase()) == Mq::True)
}

/// Evaluate a `<media-condition>` — the production `sizes` and `@supports`-style contexts use.
///
/// **A media CONDITION cannot contain a media TYPE.** `sizes="not print 100vw, 1px"` must resolve to
/// `1px`, because `not print` is not a condition at all and the whole `<source-size>` is discarded —
/// where the identical text in `@media` is a perfectly good query that matches. Routing `sizes`
/// through [`media_matches`] answered `100vw`, i.e. picked a different bitmap for the same page.
pub fn media_condition_matches(cond: &str) -> bool {
    eval_condition(&cond.trim().to_ascii_lowercase()) == Mq::True
}

/// `<media-query> = <media-condition> | [ not | only ]? <media-type> [ and <media-condition> ]?`
fn eval_media_query(q: &str) -> Mq {
    let q = q.trim();
    if q.is_empty() {
        return Mq::Invalid;
    }
    // A leading `(` can only start a condition. A leading `not (` is `<media-not>`, which is also
    // a condition — the type form's `not` is followed by an identifier, never a paren.
    if q.starts_with('(') {
        return eval_condition(q);
    }
    if let Some(rest) = q.strip_prefix("not ") {
        let rest = rest.trim();
        if rest.starts_with('(') {
            return eval_condition(q);
        }
        // `not` here negates the result of the WHOLE query (MQ4 §2.1), not just the type.
        return eval_type_query(rest).not();
    }
    // `only screen` is a legacy cloaking prefix for CSS2 UAs; it has no effect on the result.
    if let Some(rest) = q.strip_prefix("only ") {
        return eval_type_query(rest.trim());
    }
    eval_type_query(q)
}

/// `<media-type> [ and <media-condition-without-or> ]?`
fn eval_type_query(q: &str) -> Mq {
    let parts = split_top_level(q, " and ");
    let ty = parts[0].trim();
    // `not`/`only`/`and`/`or`/`layer` are excluded from `<media-type>` by the grammar itself, so
    // `not not` is a syntax error rather than a double negative.
    if !is_css_ident(ty) || matches!(ty, "not" | "only" | "and" | "or" | "layer") {
        return Mq::Invalid;
    }
    // An UNKNOWN media type is valid syntax that simply never matches — `not tty` is TRUE. That is
    // why this is `False` and not `Unknown`: unknown *types* do not get Kleene treatment, only
    // unknown *features* do.
    let mut result = Mq::of(matches!(ty, "all" | "screen"));
    for p in &parts[1..] {
        result = result.and(eval_in_parens(p));
    }
    result
}

/// `<media-condition> = <media-not> | <media-in-parens> [ <media-and>* | <media-or>* ]`
///
/// The grammar deliberately forbids **mixing** `and` and `or` at one level without parentheses,
/// because `a and b or c` has no agreed precedence on the web. Mixing is a syntax error, not a
/// guess.
fn eval_condition(c: &str) -> Mq {
    let c = c.trim();
    if c.is_empty() {
        return Mq::Invalid;
    }
    let ors = split_top_level(c, " or ");
    let ands = split_top_level(c, " and ");
    match (ors.len() > 1, ands.len() > 1) {
        (true, true) => Mq::Invalid,
        (true, false) => ors
            .iter()
            .map(|p| eval_in_parens(p))
            .fold(Mq::False, Mq::or),
        (false, true) => ands
            .iter()
            .map(|p| eval_in_parens(p))
            .fold(Mq::True, Mq::and),
        (false, false) => match c.strip_prefix("not ") {
            Some(rest) => eval_in_parens(rest).not(),
            None => eval_in_parens(c),
        },
    }
}

/// `<media-in-parens> = ( <media-condition> ) | <media-feature> | <general-enclosed>`
fn eval_in_parens(s: &str) -> Mq {
    let s = s.trim();
    let Some(inner) = strip_outer_parens(s) else {
        // `<general-enclosed>` also covers a function-token block, `ident( … )`. Anything else
        // here — a bare word, a stray `)`, a `!` — never matched the grammar at all.
        return if is_enclosed_function(s) {
            Mq::Unknown
        } else {
            Mq::Invalid
        };
    };
    let inner = inner.trim();
    if inner.is_empty() {
        return Mq::Invalid;
    }
    // A nested condition rather than a feature.
    if inner.starts_with('(')
        || inner.starts_with("not ")
        || split_top_level(inner, " and ").len() > 1
        || split_top_level(inner, " or ").len() > 1
    {
        return eval_condition(inner);
    }
    eval_feature(inner)
}

/// Split on a top-level separator — parens may contain the same text inside a value, so track depth.
fn split_top_level<'a>(q: &'a str, sep: &str) -> Vec<&'a str> {
    let b = q.as_bytes();
    let (mut out, mut depth, mut start, mut i) = (Vec::new(), 0i32, 0usize, 0usize);
    while i < b.len() {
        match b[i] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            _ => {}
        }
        if depth == 0 && q[i..].starts_with(sep) {
            out.push(&q[start..i]);
            i += sep.len();
            start = i;
            continue;
        }
        i += 1;
    }
    out.push(&q[start..]);
    out
}

/// `(a)` → `a`, but **only when the closing paren is the one that opened it**. `(a) or (b)` is not
/// a parenthesised block, and a naive `strip_prefix('(') + strip_suffix(')')` reads it as the
/// nonsense `a) or (b` — which is how `or` used to evaluate to a failed feature lookup.
fn strip_outer_parens(s: &str) -> Option<&str> {
    let b = s.as_bytes();
    if b.first() != Some(&b'(') {
        return None;
    }
    let mut depth = 0i32;
    for (i, c) in b.iter().enumerate() {
        match c {
            b'(' => depth += 1,
            b')' => depth -= 1,
            _ => {}
        }
        if depth == 0 {
            return if i + 1 == b.len() {
                Some(&s[1..i])
            } else {
                None
            };
        }
    }
    None
}

/// `unknown-general-enclosed(foo)` — a function block, which the grammar admits as unrecognised
/// rather than malformed.
fn is_enclosed_function(s: &str) -> bool {
    let Some(open) = s.find('(') else {
        return false;
    };
    is_css_ident(&s[..open]) && strip_outer_parens(&s[open..]).is_some()
}

fn is_css_ident(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if first.is_ascii_digit() {
        return false;
    }
    s.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || !c.is_ascii())
}

/// A single `<media-feature>`, in either of its **two grammars**.
///
/// `( feature: value )` asks *"is the value this?"*. `( feature )` — the **boolean context**, MQ4
/// §2.4 — asks *"is the feature ENGAGED?"*, and those are different questions with different
/// answers. Treating them as one inverted five features at once; see the boolean block below.
///
/// Returns `Unknown` — never `False` — for anything unrecognised or out of range, so that an
/// enclosing `not` cannot turn our ignorance into a positive match.
fn eval_feature(feature: &str) -> Mq {
    // ⚠ The ICB, not the window: `@media (width: 185px)` matches in a 200px frame whose root
    // element reserves a scrollbar, and a breakpoint that disagrees with the `vw` beside it is the
    // same defect `matchMedia` vs `@media` was. One source (`values::icb_size`) for both.
    let (vw, vh) = crate::values::icb_size();
    // Range syntax (`width >= 600px`) is normalised to the `min-`/`max-` prefix form, which is
    // the one the rest of this function speaks. `<`/`>` map to the same comparison here: the
    // half-pixel difference between `<` and `<=` never decides a real breakpoint.
    let (name, value) = if let Some((n, v)) = feature.split_once(">=").or(feature.split_once('>')) {
        (format!("min-{}", n.trim()), v.trim().to_string())
    } else if let Some((n, v)) = feature.split_once("<=").or(feature.split_once('<')) {
        (format!("max-{}", n.trim()), v.trim().to_string())
    } else if let Some((n, v)) = feature.split_once(':') {
        (n.trim().to_string(), v.trim().to_string())
    } else {
        // ── THE BOOLEAN CONTEXT, and it asks a DIFFERENT QUESTION than the value form.
        //
        // `( feature )` with no value means **"is this feature ENGAGED?"** — MQ4 §2.4 — and the
        // answer is *not* "does its default value match". Treating the two as the same question
        // inverted five features at once, and the loudest of them is the near-universal
        //
        //     @media (prefers-reduced-motion) { * { animation: none !important } }
        //
        // which we answered TRUE — so every animation on the page was disabled, on a browser that
        // has no reduced-motion preference at all. The `(prefers-reduced-motion: reduce)` spelling
        // was always right, which is exactly why this hid: the common form works.
        return match feature.trim() {
            // ⭐ Features whose value set contains NO "false" value match unconditionally in a
            // boolean context. `(orientation)` is not asking whether we are landscape.
            "width" | "height" | "orientation" | "display-mode" | "prefers-color-scheme" => {
                Mq::True
            }
            // The "false" value is `no-preference`/`none`, and that is precisely what we are.
            "prefers-reduced-motion"
            | "prefers-reduced-transparency"
            | "prefers-contrast"
            | "forced-colors"
            | "inverted-colors" => Mq::False,
            // Engaged: a desktop browser with a fine pointer, hover, 8-bit colour and JS enabled.
            "hover" | "any-hover" | "pointer" | "any-pointer" | "color" | "any-color"
            | "scripting" => Mq::True,
            // ⚠ A `min-`/`max-` prefix is a RANGE, and a range with no value is not a boolean
            // feature — it never matched the grammar, so it is `<general-enclosed>`.
            _ => Mq::Unknown,
        };
    };
    // ⚠ Past this point the VALUE form is in hand, so an EMPTY value is a colon with nothing after
    // it — `(hover: )` — which never matched the grammar. It is not the boolean form; that was
    // answered above and returned.
    if value.is_empty() {
        return Mq::Unknown;
    }
    // Media-query lengths resolve `em`/`rem` against the INITIAL font size, never the element's.
    let px = |v: &str| -> Option<f32> {
        let v = v.trim();
        if let Some(n) = v.strip_suffix("px") {
            n.trim().parse::<f32>().ok()
        } else if let Some(n) = v.strip_suffix("rem").or(v.strip_suffix("em")) {
            n.trim().parse::<f32>().ok().map(|n| n * 16.0)
        } else {
            // A unitless number is a length only when it is zero.
            v.parse::<f32>().ok().filter(|n| *n == 0.0)
        }
    };
    // A NEGATIVE length is not a false match, it is an invalid `<media-feature>` — and an invalid
    // feature is `<general-enclosed>`, i.e. Unknown. `not (min-width: -1px)` must not be true.
    let len = |v: &str| -> Option<f32> { px(v).filter(|n| *n >= 0.0) };
    // A known feature whose value does not parse is Unknown, not False, for the same reason.
    let cmp = |v: Option<f32>, f: &dyn Fn(f32) -> bool| match v {
        Some(v) => Mq::of(f(v)),
        None => Mq::Unknown,
    };
    // A keyword feature given a value OUTSIDE its own value set is invalid — `<general-enclosed>`,
    // Unknown — not merely a value we do not happen to be. The distinction only shows up under
    // `not`: `not (orientation: sideways)` must be FALSE, and answering `False` here would negate
    // it to true. This is the same rule `len` applies to an out-of-range length, in keyword form.
    let kw = |ours: &str, allowed: &[&str]| -> Mq {
        if allowed.contains(&value.as_str()) {
            Mq::of(value == ours)
        } else {
            Mq::Unknown
        }
    };
    match name.as_str() {
        "min-width" => cmp(len(&value), &|v| vw >= v),
        "max-width" => cmp(len(&value), &|v| vw <= v),
        "width" => cmp(len(&value), &|v| (vw - v).abs() < 0.5),
        "min-height" => cmp(len(&value), &|v| vh >= v),
        "max-height" => cmp(len(&value), &|v| vh <= v),
        "height" => cmp(len(&value), &|v| (vh - v).abs() < 0.5),
        "orientation" => kw(
            if vw >= vh { "landscape" } else { "portrait" },
            &["portrait", "landscape"],
        ),
        // We are a real, light-scheme, non-reduced-motion desktop browser with a fine pointer
        // and hover. These answers must agree with what `window.matchMedia` tells the page —
        // a browser is allowed to be unusual, it is not allowed to disagree with itself.
        "prefers-color-scheme" => kw("light", &["light", "dark"]),
        "prefers-reduced-motion" => kw("no-preference", &["no-preference", "reduce"]),
        "prefers-reduced-transparency" => kw("no-preference", &["no-preference", "reduce"]),
        "prefers-contrast" => kw(
            "no-preference",
            &["no-preference", "more", "less", "custom"],
        ),
        "forced-colors" => kw("none", &["none", "active"]),
        "inverted-colors" => kw("none", &["none", "inverted"]),
        "hover" | "any-hover" => kw("hover", &["none", "hover"]),
        "pointer" | "any-pointer" => kw("fine", &["none", "coarse", "fine"]),
        "color" | "any-color" => cmp(
            value.trim().parse::<f32>().ok().filter(|v| *v >= 0.0),
            &|v| v == 8.0,
        ),
        "display-mode" => kw(
            "browser",
            &[
                "fullscreen",
                "standalone",
                "minimal-ui",
                "browser",
                "window-controls-overlay",
                "picture-in-picture",
            ],
        ),
        "scripting" => kw("enabled", &["none", "initial-only", "enabled"]),
        // Unrecognised feature → `<general-enclosed>` → Unknown, which is `false` at the top level
        // and stays `false` under `not`. Never guess in the direction of applying a sheet.
        _ => Mq::Unknown,
    }
}

/// ⚠⚠⚠ **THIS FUNCTION MOJIBAKE'D EVERY STYLESHEET IN THE ENGINE, and it did it in one character.**
///
/// It used to walk the source as BYTES and emit `out.push(b[i] as char)`. For ASCII that is the
/// identity. For anything else it widens **each UTF-8 byte into its own Latin-1 code point**, so
/// `–` (U+2013, bytes `E2 80 93`) came out as the three characters `â€“`.
///
/// The blast radius is the whole cascade, because this runs on the way IN: `Stylesheet::parse`
/// stores the result as `source`, and `source` is the string handed verbatim to
/// `StyloStylesheet::from_str`. **Stylo never saw a correctly-decoded stylesheet.** Measured on
/// `255md.com`, whose list markers are `li::before { content: "–" }` — we drew `â` glued to each
/// item where Chrome draws an en dash. The same corruption reaches:
///
/// * every non-ASCII `content:` string — arrows, bullets, quotes, checkmarks, currency, the icon
///   glyphs half the web puts in `::before`;
/// * **`font-family` names written in their own script** — `font-family: "微软雅黑"`,
///   `"ヒラギノ角ゴ"`, `"맑은 고딕"`. A mangled family name matches no font, so the whole CJK
///   font stack silently falls through to a default. That is a large share of the CrUX tail.
/// * `quotes:`, non-ASCII identifiers, and any `url()` with a non-ASCII path.
///
/// The escape form was never affected — `content: "\2013"` is pure ASCII and always worked — which
/// is exactly why this survived: the bug is invisible to any test written in ASCII, and every test
/// in this file was.
///
/// Scanning for the delimiters as bytes is still correct and is kept: `/` and `*` are ASCII, and a
/// UTF-8 continuation byte is always ≥ `0x80`, so no multi-byte character can contain either one.
/// The only thing that had to change is what gets COPIED — the whole character, not one byte of it.
/// Strip the XHTML `<![CDATA[ … ]]>` wrapper an inline `<style>` may carry.
///
/// ⚠⚠⚠ **A WRAPPED SHEET WAS DROPPED IN ITS ENTIRETY — every rule, not just the first.** In XHTML,
/// `<style type="text/css"><![CDATA[ … ]]></style>` is the standard way to keep `<` and `&` out of
/// the XML parser's way, and it is what the CSS 2.1 conformance suite is written in: **2,191 of its
/// 10,501 files use it**, tests and references alike. Our parser met `<![CDATA[` where a selector
/// belonged and bailed on the whole sheet, so those pages rendered completely unstyled.
///
/// Chrome-measured on a three-rule sheet: Chrome applies **all three** and we applied none. The
/// numbers matter because they rule out the other plausible reading — CSS error recovery would drop
/// only the first rule and keep the rest, and Chrome keeps the first one too.
///
/// ⚠ It is a WRAPPER, so only a leading `<![CDATA[` and a trailing `]]>` are removed, and only when
/// they are the outermost non-whitespace tokens. A `]]>` sitting inside a string or a URL is content
/// and is left alone; a sheet without the wrapper is returned untouched and pays one `trim_start`.
fn strip_cdata(src: &str) -> String {
    let t = src.trim_start();
    let Some(rest) = t.strip_prefix("<![CDATA[") else {
        return src.to_string();
    };
    // The closing marker is the LAST one, so a `]]>` earlier in the sheet cannot truncate it.
    match rest.rfind("]]>") {
        Some(i) => rest[..i].to_string(),
        // An unterminated wrapper is still a wrapper: dropping the opener recovers every rule,
        // which is strictly better than dropping the sheet.
        None => rest.to_string(),
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
            // Advance over the lead byte and any continuation bytes, then copy the character
            // whole. `i` therefore always lands on a char boundary, so the slice cannot panic.
            let start = i;
            i += 1;
            while i < b.len() && (b[i] & 0xC0) == 0x80 {
                i += 1;
            }
            out.push_str(&src[start..i]);
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

/// Split a selector list on its TOP-LEVEL commas and parse each branch.
///
/// ⚠⚠⚠ **This was `text.split(',')` — a naive split that does not see parentheses — and it is the
/// root cause `:is()` merely exposed.** A comma inside a functional pseudo is an argument
/// separator, not a list separator, so every one of these was cut in half:
///
/// ```text
///   .a :is(.b, .c)        →  ".a :is(.b"   +  ".c)"
///   p:has(> img, > svg)   →  "p:has(> img" +  "> svg)"
///   :not(.a, .b)          →  ":not(.a"     +  ".b)"
/// ```
///
/// The first fragment has an unbalanced `(` and parses as though the list held only its first
/// member; the second is garbage and is dropped. So the selector did not fail loudly — it
/// **quietly matched a subset**, which is the worst of the three possible outcomes and is why it
/// survived: `:is(.b, .c)` returned the `.b` elements and looked like it worked.
///
/// `split_top_level_commas` — already in this file, already used by the `:has()` arm — is
/// parenthesis-aware and is what this should always have called.
fn parse_selector_list(text: &str) -> Vec<Selector> {
    split_top_level_commas(text)
        .iter()
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
        "open" => Pseudo::Open,
        "enabled" => Pseudo::Enabled,
        "required" => Pseudo::Required,
        "read-only" => Pseudo::ReadOnly,
        "read-write" => Pseudo::ReadWrite,
        "muted" => Pseudo::Muted,
        "link" | "any-link" => Pseudo::Link,
        // Pseudo-ELEMENTS. `::before`/`::after` are legal with one colon too (CSS2 syntax), and
        // plenty of real sheets still write them that way.
        "before" => Pseudo::Before,
        "after" => Pseudo::After,
        // Dynamic / state pseudos we can't evaluate in a static render → never match, so a
        // rule gated on them just doesn't apply (rather than dropping the whole rule).
        "hover" | "focus" | "active" | "visited" | "target" | "focus-within" | "focus-visible"
        | "placeholder-shown" | "autofill" => Pseudo::NeverStatic,
        "first-of-type" => Pseudo::FirstOfType,
        "last-of-type" => Pseudo::LastOfType,
        "only-of-type" => Pseudo::OnlyOfType,
        "nth-child" => {
            let (a, b) = parse_nth(arg?)?;
            Pseudo::NthChild(a, b)
        }
        // **The other four An+B pseudos, absent since this parser was written.** `:nth-child` was
        // here alone, so `:nth-last-child`, `:nth-of-type` and `:nth-last-of-type` hit the
        // `_ => return None` arm at the bottom — which drops the ENTIRE selector, not the pseudo.
        // Measured before the fix: `em:nth-of-type(3)` → 0 matches (Chrome 1),
        // `li:nth-last-child(3n)` → 0 (Chrome 2), `#p :last-of-type` → 0 (Chrome 2), while
        // `li:nth-child(2n)` was correct at 3. An empty answer from a valid selector is the
        // hardest failure to notice, because it looks exactly like a page with nothing to match.
        "nth-last-child" => {
            let (a, b) = parse_nth(arg?)?;
            Pseudo::NthLastChild(a, b)
        }
        "nth-of-type" => {
            let (a, b) = parse_nth(arg?)?;
            Pseudo::NthOfType(a, b)
        }
        "nth-last-of-type" => {
            let (a, b) = parse_nth(arg?)?;
            Pseudo::NthLastOfType(a, b)
        }
        // ⚠ `:not()` is NOT forgiving — unlike `:is()`/`:has()`. Selectors 4 is explicit: an
        // invalid member makes the whole `:not()` invalid, because dropping one would INVERT the
        // meaning of the rest (`:not(.a, ??)` silently becoming `:not(.a)` matches strictly more,
        // not less). So this fails closed on any unparsable member, and `:is()` above does not.
        "not" => {
            let mut list = Vec::new();
            for raw in split_top_level_commas(arg?) {
                list.push(parse_selector(raw.trim())?);
            }
            if list.is_empty() {
                return None;
            }
            Pseudo::Not(list)
        }
        // **`:is()` / `:where()` — a FORGIVING list of COMPLEX selectors, matched with this element
        // as the subject.**
        //
        // Both fell through to the `_ => return None` arm below, which drops the WHOLE selector —
        // so `document.querySelectorAll('.a :is(.b, .c)')` returned **nothing at all**, not a
        // partial answer. They are Baseline CSS and the standard way every modern stylesheet and
        // component library writes a grouped rule (`.card :is(h1, h2, h3)`), so the silence was
        // broad.
        //
        // The list members are COMPLEX, not compound (`:is(.e + .f, .g > .b)` is legal), which is
        // why this reuses `parse_selector`/`selector_matches` rather than the compound pair `:not`
        // uses — matching a complex selector with a given node as the subject is exactly what
        // `selector_matches` already does.
        //
        // FORGIVING means an unparsable member is DROPPED and the rest still apply — `:is(.a, 123)`
        // matches `.a`, and only an entirely unusable list makes the selector fail. That is the same
        // rule `:has()` implements one arm down, and it is why `:is()` cannot take a stylesheet with
        // it the way an unknown pseudo does.
        //
        // ⚠ `:where()` is IDENTICAL here on purpose. The two differ only in SPECIFICITY — `:where()`
        // contributes zero — and this matcher answers *"does it match"* for
        // `querySelector`/`matches`/`closest`, where specificity is not consulted. The live cascade
        // is Stylo's and computes specificity itself, so nothing downstream of this needs the
        // distinction. Collapsing them anywhere specificity IS read would be wrong.
        "is" | "where" | "matches" | "-webkit-any" | "-moz-any" => {
            let mut list = Vec::new();
            for raw in split_top_level_commas(arg?) {
                if let Some(sel) = parse_selector(raw.trim()) {
                    list.push(sel);
                }
            }
            if list.is_empty() {
                return None;
            }
            Pseudo::Is(list)
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

/// **Split a declaration block on `;` — but NOT on a `;` inside `url()`, a function, or a string.**
///
/// ⚠⚠⚠ **A `data:` URI CONTAINS A SEMICOLON, AND THE NAIVE `text.split(';')` CUT EVERY ONE IN
/// HALF.** `src: url(data:font/ttf;base64,AAAA…) format("truetype")` becomes the two fragments
/// `src: url(data:font/ttf` and `base64,AAAA…) format("truetype")`; the first has an unterminated
/// `url(` so [`parse_font_face_block`] finds no source and **drops the whole `@font-face`**, and the
/// second is not a declaration at all.
///
/// Measured against Chrome on a `file://` fixture, one 147KB TrueType face declared three ways and
/// used at `font-family: <face>, monospace` so a failure falls back visibly:
///
/// ```text
///                                          chrome    before   after
///   src: url(go.ttf)                CTRL    126.56     127      127
///   src: url("go.ttf")              CTRL    126.56     127      127
///   src: url(data:font/ttf;base64,…)        126.56     145      127
///   font-family: monospace          CTRL    144.5      145      145
///   font-family: NoSuchFace,monospace CTRL  144.5      145      145
/// ```
///
/// **The two `file://` rows are what identify the mechanism.** A web font declared with an ordinary
/// URL has always loaded, so the row this defect lives in was carried as *"web fonts — partial"* and
/// a probe that used only a `data:` URI would have concluded that web fonts do not work at all. The
/// discriminator is the pair, and the residual 0.44px on the working rows is the ordinary sub-pixel
/// advance gap the monospace control shows too.
///
/// Priced on the burndown corpus: a `data:` payload appears in an `@font-face` `src` on **17 of the
/// 166 pages that use `@font-face` at all (10%)**, and a `;`-bearing `data:` URI appears inside some
/// CSS `url()` on **89 of 761 files** — 1053 of those are `data:image/svg+xml;`, the icons, chevrons
/// and checkmarks a modern stylesheet inlines.
///
/// ⚠ The shipping cascade is Stylo, which parses declarations correctly, so the BACKGROUND-image
/// half of that population is not affected on the live path. The `@font-face` half is: face
/// harvesting runs through *this* parser (`Stylesheet::parse(&css).font_faces()`) whichever engine
/// computes the styles, which is exactly why the measured failure is a font and not a background.
///
/// Nesting depth is tracked over `(`/`)` and quotes over `"`/`'`, because a `;` can also appear
/// inside a quoted string (`content: "a;b"`) and inside `calc()`-shaped functions that carry one.
fn split_declarations(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    let mut prev_escape = false;
    for c in text.chars() {
        match quote {
            Some(q) => {
                cur.push(c);
                if prev_escape {
                    prev_escape = false;
                } else if c == '\\' {
                    prev_escape = true;
                } else if c == q {
                    quote = None;
                }
            }
            None => match c {
                '"' | '\'' => {
                    quote = Some(c);
                    cur.push(c);
                }
                '(' => {
                    depth += 1;
                    cur.push(c);
                }
                ')' => {
                    depth = depth.saturating_sub(1);
                    cur.push(c);
                }
                ';' if depth == 0 => out.push(std::mem::take(&mut cur)),
                _ => cur.push(c),
            },
        }
    }
    out.push(cur);
    out
}

fn parse_declarations(text: &str) -> Vec<Declaration> {
    let mut decls = Vec::new();
    for chunk in split_declarations(text) {
        let chunk = chunk.as_str();
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
                if !rule.media_applies() {
                    continue;
                }
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
        // The `:has()` memo, open for exactly this pass — see `HasMemoScope`. The pass reads the DOM
        // and writes only styles, which is the promise the guard is asserting. The Stylo path opens
        // its own around the equivalent loop (`stylo_engine.rs`); one rule, and this time BOTH
        // implementations got it in the same tick.
        let _has_memo = HasMemoScope::new();
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

                // ⚠⚠⚠ **CHROME'S `<td>` COMPUTES `vertical-align: middle`, NOT `baseline`, AND THE
                // RULE THAT SAYS SO IS ON THE ROW GROUP.** Blink's UA sheet is
                //
                // ```css
                //   thead, tbody, tfoot { vertical-align: middle }
                //   tr, td, th         { vertical-align: inherit }
                // ```
                //
                // and the second line is doing real work: `vertical-align` is NOT an inherited
                // property, so `inherit` is the only way the row group's value reaches a cell. A
                // default HTML table therefore CENTRES every cell's content vertically and never
                // forms a baseline group at all — which is the opposite of what an engine that
                // leaves cells at the initial `baseline` does. Measured (16px/20px monospace, a
                // 40px block beside a one-line text cell), `getComputedStyle(td).verticalAlign` and
                // the text's offset from the top of the table:
                //
                // ```text
                //                                          Chrome        before
                //   a plain <td>                        middle  dy=10   baseline  dy=0
                //   …in a 60px row                      middle  dy=20   baseline  dy=0
                //   <tbody style="vertical-align:top">   top    dy=0     baseline  dy=0
                //   <tr style="vertical-align:bottom">   bottom dy=20    baseline  dy=0
                //   a <th>                              middle  dy=10   baseline  dy=0
                //   <div style="display:table-cell">    baseline dy=25  baseline  dy=0
                // ```
                //
                // The last row is why this is keyed on the TAG and not on `Display::TableCell`: the
                // rule is a UA *declaration* on `td`/`th`, so a `div { display: table-cell }` keeps
                // the initial `baseline` in Chrome. Matching on the computed display would have
                // moved that div too, and been wrong.
                //
                // It lives HERE rather than in `apply_ua_defaults` for the same reason `<details>`
                // above does: `inherit` needs the parent, and the per-element helper has no tree.
                // And it must live in THIS cascade at all — not the Stylo sheet — because
                // `vertical-align` is one of the properties `stylo_engine.rs` RECOVERS from
                // MinimalCascade wholesale (stylo 0.19 exposes no computed longhand for it), so a
                // rule written only in the Stylo UA sheet is overwritten by this map and inert.
                match el.name.to_ascii_lowercase().as_str() {
                    "thead" | "tbody" | "tfoot" => s.vertical_align = VerticalAlign::Middle,
                    "tr" | "td" | "th" => s.vertical_align = parent_style.vertical_align,
                    _ => {}
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
                // ⚠ **THE IN-FLOW DISPLAY IS SNAPSHOT HERE — after every declaration and before any
                // out-of-flow blockification.** This cascade does not blockify, so `s.display` at
                // this point IS the specified value; the field exists because the STYLO path's
                // `clone_display` returns the blockified one and the static-position rule needs the
                // other. See `ComputedStyle::display_in_flow`.
                s.display_in_flow = s.display;
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
    // ⚠ **`appearance: auto` IS A UA RULE, NOT AN INITIAL VALUE.** A `<div>` computes `none` and a
    // `<button>` computes `auto` — Chrome-measured on both — so the default is keyed on the tag.
    // It is set BEFORE the author declarations are applied, which is what lets `appearance: none`
    // on a control, and `appearance: auto` on a div, both take effect.
    if tag_has_native_appearance(tag) {
        s.appearance = Appearance::Auto;
    }
    let (display, top_bottom_em, weight, scale): (Display, f32, u16, f32) = match tag {
        "html" | "body" | "div" | "section" | "article" | "header" | "footer" | "nav" | "main"
        | "aside" | "figure" | "figcaption" | "address" => (Block, 0.0, 400, 1.0),
        // ⚠⚠⚠ **THE TWIN SHEETS HAD DRIFTED AGAIN, AND THIS TIME IT IS THIS ONE THAT IS SHORT.**
        // `stylo_engine.rs` — the SHIPPING cascade — says
        // `form, fieldset, table, caption, center, menu, dl { display: block }` and
        // `summary { display: block }`; this list carried the table family and **none of the rest**,
        // so a `<form>` was laid out as a *boxless inline* everywhere `MinimalCascade` runs — which
        // is `manuk-agent`, the crate whose whole job is clicking things.
        //
        // The cost was not cosmetic. `<form><button>Go</button></form>` gave the form the button's
        // lifted box, and once an inline reports its own content area (t853) the form's box became
        // *smaller* than the button it contains — so `A11yNode::hit_test`, which resolves ties by
        // smallest-area-wins, handed the agent's coordinate click to the **form**. A wrapper
        // element the author never meant to be clickable swallowed the click on the control inside
        // it.
        //
        // This is the t851 pattern one turn further on: two hand-maintained UA sheets, each
        // concealing the other's gap from whichever test you happened to write, and the drift only
        // becomes visible when something starts *depending* on the answer. Margins are left to the
        // metrics that already exist (`dl`/`menu` take 1em there, the rest take none), so this is a
        // `display` correction and nothing else.
        // (`summary` is already blocked below, with its bold weight; `table`/`caption` carry
        // their own inner display.)
        "form" | "fieldset" | "center" | "menu" | "dl" => (Block, 0.0, 400, 1.0),
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
        // ⚠⚠⚠ **`sup`/`sub` LIVE IN BOTH UA SHEETS OR IN NEITHER (t923).** `stylo_engine.rs` gained
        // `sup { vertical-align: super; font-size: smaller }` at t914 and this sheet did not — and
        // because `vertical_align` is one of the handful of properties RECOVERED from MinimalCascade
        // into the Stylo map (`stylo_engine.rs`, the "no computed longhand accessor in stylo 0.19"
        // block), MinimalCascade's `Baseline` was written straight over Stylo's correct `super`.
        //
        // The tell was exact and took one fixture to see: `<span style="font-size:13.333px;
        // vertical-align:super">` grows its line to Chrome's 27, and `<sup>` — the same font size,
        // the same raise, from the UA sheet — stayed at 24. The FONT SIZE arrived (the box is 36x15
        // in both engines, byte-exact); only the alignment was lost, because only the alignment
        // takes the recovery path.
        //
        // `0.8333` is `smaller` on Chrome's scale at 16px (13.333px), which is what both engines
        // already produce for the box; it is spelled here so the two sheets agree by construction
        // rather than by coincidence.
        "sup" | "sub" => (Inline, 0.0, 400, 0.8333),
        "table" => (Table, 0.0, 400, 1.0),
        "thead" => (TableHeaderGroup, 0.0, 400, 1.0),
        "tfoot" => (TableFooterGroup, 0.0, 400, 1.0),
        "tbody" => (TableRowGroup, 0.0, 400, 1.0),
        "tr" => (TableRow, 0.0, 400, 1.0),
        "td" => (TableCell, 0.0, 400, 1.0),
        "th" => (TableCell, 0.0, 700, 1.0),
        "caption" => (TableCaption, 0.0, 400, 1.0),
        "colgroup" => (TableColumnGroup, 0.0, 400, 1.0),
        "col" => (TableColumn, 0.0, 400, 1.0),
        // Keep in lockstep with the UA sheet in `stylo_engine.rs`. The two cascades disagreeing
        // about which elements render at all is how a `<source>` ends up with 19px of height in one
        // configuration and none in the other.
        // ⚠⚠ `source`, `track`, `area` and `noscript` were REMOVED from this list (t809): Chrome
        // computes `inline` for all four, and they generate no box for a STRUCTURAL reason (their
        // parent consumes them), which now lives in `layout::never_rendered`. `param`, `datalist`,
        // `template` and `rp` really are `display: none` in Chrome and stay. Both cascades were
        // changed in the same tick, which is what the lockstep note above is for.
        "head" | "title" | "meta" | "link" | "script" | "style" | "base" | "template" | "param"
        | "datalist" | "basefont" | "noembed" | "noframes" | "rp" => (None, 0.0, 400, 1.0),
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
    // `hidden` global attribute — https://html.spec.whatwg.org/#hidden-elements. Any element carrying
    // the boolean `hidden` attribute is NOT rendered. Attribute-keyed, not tag-keyed, because `hidden`
    // is global. The `until-found` value is the one exception: the spec renders it with
    // `content-visibility: hidden` (collapsed but findable), which we do not support yet, so we leave
    // it visible rather than falsely collapse content we could not later reveal. Keep in lockstep with
    // the `[hidden]:not([hidden="until-found"])` rule in stylo_engine.rs.
    if el
        .attr("hidden")
        .is_some_and(|v| !v.eq_ignore_ascii_case("until-found"))
    {
        s.display = None;
    }
    // Form-control default appearance (UA stylesheet): a bordered, padded box. A text input
    // gets a default width; buttons hug their label. This is what makes fields visible.
    if matches!(tag, "input" | "button" | "textarea" | "select") {
        s.border_width = Sides::all(1.0);
        s.border_color = Sides::all(Rgba::new(118, 118, 118, 255));
        s.padding = Sides {
            top: Dim::Px(2.0),
            bottom: Dim::Px(3.0),
            left: Dim::Px(6.0),
            right: Dim::Px(6.0),
        };
        // **BUTTONS AND `<select>` ARE `border-box`; TEXT FIELDS AND `<textarea>` ARE NOT** — and
        // this cascade had it the other way round, applying `border-box` to all four tags. Chrome's
        // UA sheet draws the line where the controls that look most alike end up on opposite sides
        // of it. Measured at `height:50px; padding-top:20px`, used border-box height:
        //
        //     button  submit  text  select  textarea  div
        //       50      50     70     50       70      70     Chrome
        //
        // Kept in lockstep with the `box-sizing: border-box` rule in `stylo_engine.rs`, which is the
        // SHIPPING cascade and did not have this rule at all. The two sheets were wrong in opposite
        // directions, which is exactly what a hand-maintained pair of stylesheets does.
        if matches!(tag, "button" | "select")
            || (tag == "input"
                && el.attr("type").is_some_and(|t| {
                    matches!(
                        t.to_ascii_lowercase().as_str(),
                        "submit" | "reset" | "button"
                    )
                }))
        {
            s.box_sizing = BoxSizing::BorderBox;
        }
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
                //
                // **`border-box` and the per-type margin are Chrome's, and they are the geometry
                // half — kept in lockstep with the `input[type=checkbox], input[type=radio]` rule in
                // `stylo_engine.rs`, which is the SHIPPING cascade.** 13px is the OUTER box there,
                // so under `content-box` the 1px border we draw (Chrome draws none — it paints the
                // control natively) made it 15x15. The margins are `3px 3px 3px 4px` / `3px 3px 0
                // 5px`, asymmetric and per-type, and they are what puts the label beside the box.
                ty @ ("checkbox" | "radio") => {
                    let radio = ty == "radio";
                    s.width = Dim::Px(13.0);
                    s.height = Dim::Px(13.0);
                    s.padding = Sides::all(Dim::Px(0.0));
                    s.box_sizing = BoxSizing::BorderBox;
                    s.margin = Sides {
                        top: Dim::Px(3.0),
                        right: Dim::Px(3.0),
                        bottom: Dim::Px(if radio { 0.0 } else { 3.0 }),
                        left: Dim::Px(if radio { 5.0 } else { 4.0 }),
                    };
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
    // The alignment half of the `sup`/`sub` row above — the size half rides on `scale`.
    match tag {
        "sup" => s.vertical_align = VerticalAlign::Super,
        "sub" => s.vertical_align = VerticalAlign::Sub,
        _ => {}
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

    // Replaced elements: an <img>/<canvas>/<video> is an ATOMIC INLINE box sized by its
    // presentational width/height attributes (author CSS width/height still overrides, as
    // those are applied after UA defaults). Computed display stays `inline` — the spec's and
    // Chrome's value (tick 384; this used to force `inline-block`, and 81 corpus sites showed
    // the divergence on <img> alone). Layout treats an inline replaced box atomically, which
    // is the behavior the old mutation was standing in for. Natural (intrinsic) sizing from
    // the decoded bitmap is layered on in the image pipeline.
    if matches!(
        tag,
        "img" | "canvas" | "video" | "svg" | "object" | "embed" | "iframe"
    ) {
        // A presentational hint may only fill a genuinely ABSENT width. `width: stretch` and the
        // intrinsic keywords compute to `Dim::Auto`, so they look absent — and `<canvas width="40">`
        // would beat the author's `width: stretch` and keep hugging its 40px. The flags tell "no
        // width specified" apart from "a width specified that resolves later". Twin of the guard in
        // `stylo_engine::apply_presentational_hints`.
        // ⚠ `canvas` is excluded: its attributes are the output BITMAP (the natural size), not the
        // CSS dimension properties. Twin of the exclusion in
        // `stylo_engine::presentational_hint_block` — the ratio half below still applies to it.
        if tag != "canvas" && !s.width_stretch && s.width_keyword.is_none() {
            if let Some(w) = el.attr("width").and_then(parse_dimension_attr) {
                s.width = Dim::Px(w);
            }
        }
        if tag != "canvas" && !s.height_stretch && !s.height_intrinsic {
            if let Some(h) = el.attr("height").and_then(parse_dimension_attr) {
                s.height = Dim::Px(h);
            }
        }
        // **The dimension attributes are also an aspect-ratio hint** (HTML §"dimension attributes":
        // `aspect-ratio: auto <width> / <height>`), and that half is the load-bearing one. Without it
        // a `<canvas>`/`<video>` — which never has a decoded bitmap to derive a ratio from — and an
        // `<img>` that has not loaded yet have NO ratio at all, so the `max-width:100%` in every CSS
        // reset narrows the box and leaves the height at its attribute value: the image renders
        // squashed, and the pre-load box that `width`/`height` exist to reserve is the wrong shape.
        // `auto` in the spec's value means a real intrinsic ratio still wins, which is why this only
        // fills an empty slot — the decode pipeline overwrites it (`Page::apply_images`).
        // `viewBox` gives an `<svg>` an intrinsic RATIO with no dimension attributes at all (SVG2;
        // the icon idiom). Twin of the block in `stylo_engine::apply_presentational_hints`.
        if tag == "svg" && s.aspect_ratio.is_none() {
            if let Some(vb) = el.attr("viewBox").or_else(|| el.attr("viewbox")) {
                let n: Vec<f32> = vb
                    .split(|c: char| c == ',' || c.is_ascii_whitespace())
                    .filter(|t| !t.is_empty())
                    .filter_map(|t| t.parse().ok())
                    .collect();
                if n.len() == 4 && n[2] > 0.0 && n[3] > 0.0 {
                    s.aspect_ratio = Some(n[2] / n[3]);
                }
            }
        }
        // A `<canvas>`'s attributes are its NATURAL size (the output bitmap), not the CSS dimension
        // properties — which is why they are excluded from the fill above. Twin of the block in
        // `stylo_engine::apply_presentational_hints`; `fill_natural_size` is the shared producer.
        if tag == "canvas" {
            if let (Some(w), Some(h)) = (
                el.attr("width").and_then(parse_dimension_attr),
                el.attr("height").and_then(parse_dimension_attr),
            ) {
                if w > 0.0 && h > 0.0 {
                    fill_natural_size(s, w, h);
                }
            }
        }
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
        // ⚠ The `<iframe>` 300×150 default was written here as a COMPUTED value and is gone for the
        // reason spelled out at its twin in `stylo_engine`: 300×150 is the **default object size**,
        // a used value, and forcing `auto` into a definite length here deleted the fact flex
        // stretch and `aspect-ratio` both read. `layout`'s `default_object_tag` already lists
        // `iframe`. Removed in both cascades together so the two do not drift.
    }
}

/// Parse an HTML presentational length attribute (`width="272"` or `width="272px"`) into
/// pixels. Percentages and other units are ignored (returns `None`).
/// **Fill a replaced element's NATURAL size into the axes nobody specified**, and mark them.
///
/// The one producer of `width_is_natural` / `height_is_natural` — a decoded bitmap's own pixel size
/// (`manuk-page`) and a `<canvas>`'s dimension attributes (the cascade) are the same thing wearing
/// two hats, and the rule that consumes the mark (CSS's ratio transfer) must not be able to tell
/// them apart.
///
/// ⚠ **A height is left `auto` whenever a ratio exists, and that is not an omission.** The used
/// height of a replaced element with a ratio comes from its *used* width — which is not known here,
/// because it has not been clamped yet. Writing the natural height in would pin it to the
/// pre-clamp value, which is exactly the squashed-image bug: `<canvas width="800" height="400">`
/// under `max-width:100%` in a 400px column is 400x200 in Chrome, and 400x400 if this fills the
/// height.
pub fn fill_natural_size(s: &mut ComputedStyle, nw: f32, nh: f32) {
    if nw > 0.0 && nh > 0.0 {
        s.aspect_ratio = Some(nw / nh);
    }
    let width_absent = s.width == Dim::Auto && !s.width_stretch && s.width_keyword.is_none();
    if width_absent && (s.height == Dim::Auto || s.aspect_ratio.is_none()) {
        s.width = Dim::Px(nw);
        s.width_is_natural = true;
    }
    if s.height == Dim::Auto && !s.height_stretch && !s.height_intrinsic && s.aspect_ratio.is_none()
    {
        s.height = Dim::Px(nh);
        s.height_is_natural = true;
    }
}

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

/// Which **intrinsic sizing keyword** a declared value names, if any — `min-content`,
/// `max-content`, `fit-content` or `fit-content(<length>)` (treated as plain `fit-content`).
///
/// ONE function for all seven sizing properties (`width`, `height` and the four min/max), because
/// the defect t930 fixed was exactly that `width` had this logic inline and the min/max properties
/// had none: the keyword fell through `parse_dim` to `Dim::Auto` and meant *0* on a min and *no
/// limit* on a max. Duplicating the match is how the next property gets missed the same way.
/// `stretch` / `-webkit-fill-available` / `-moz-available` are deliberately NOT here — they are
/// definite fills, not content-derived sizes, and carry their own flags.
fn intrinsic_kw(v: &str) -> Option<IntrinsicSize> {
    let low = v.trim().to_ascii_lowercase();
    match low.as_str() {
        "min-content" => Some(IntrinsicSize::MinContent),
        "max-content" => Some(IntrinsicSize::MaxContent),
        _ if low == "fit-content" || low.starts_with("fit-content(") => {
            Some(IntrinsicSize::FitContent)
        }
        _ => None,
    }
}

/// [`intrinsic_kw`] for the four **min/max** sizing properties, where the functional
/// `fit-content(<length>)` form is **not valid** and must fall back to the initial value.
///
/// Chrome-measured rather than inferred from the grammar — `<div style="min-width:fit-content(50px);
/// max-width:fit-content(50px); max-height:fit-content(50px)">` reads back `0px` / `none` / `none`,
/// i.e. the declaration was dropped. Accepting it here would have been a *more* permissive parser
/// that renders a box Chrome does not.
/// **CSS Display L3's TWO-VALUE `display`, rewritten to the legacy keyword it is a synonym for.**
///
/// `display: <display-outside> <display-inside>` (either order, plus an optional `list-item`) is not
/// a new set of layout modes — it is the *existing* modes, spelled as the pair they always were.
/// `inline flow-root` **is** `inline-block`; `block flow` **is** `block`. So this canonicalises and
/// lets the single-keyword table do the mapping, rather than growing a second table that can drift.
///
/// ⚠⚠⚠ **THE FIRST BATTERY I WROTE FOR THIS AGREED WITH CHROME ON 7 OF 8 ROWS WHILE THE FEATURE WAS
/// COMPLETELY UNIMPLEMENTED.** An unrecognised `display` is an invalid declaration and leaves the
/// element at its *previous* value — so `display: block flow` on a `<div>` is 400px wide whether it
/// parsed or not, and `block flex` on a `<div>` measured the same in a fixture that gave it an
/// explicit width. **Every row has to be an element whose DEFAULT display differs from the one the
/// pair asks for**, or the battery is measuring the UA stylesheet:
///
/// ```text
///                                           Chrome    before    after
///   <div  display:inline flow>x               8x17    400x18     8x17
///   <span display:block flow>                400x20     0x0     400x20
///   <span display:block flex>                400x20     0x0     400x20
///   <div  display:inline flex>                50x20   400x20     50x20
///   <div  display:inline grid>                50x20   400x20     50x20
///   <span display:block flow-root>           400x50     0x0     400x50
///   <div  display:inline flow-root>           50x20   400x20     50x20
///   <div  display:block flow list-item>      400x20   400x20    400x20   <- agrees by ACCIDENT
/// ```
///
/// ⚠⚠ **`display: inline table` is routed to the existing `inline-table` keyword rather than given a
/// new mode**, which is the same discipline: giving the *pair* a behaviour the *keyword* does not
/// have is how two spellings of one value drift apart. ⚠ A first draft of this comment claimed that
/// left a measured 50x20-vs-400x20 gap against Chrome; **it does not, and the claim was written
/// before the row was re-run.** `inline table` reads **50x20, Chrome-exact**, because the shared
/// `Display::Table` already shrink-wraps. Whether `table` and `inline-table` diverge anywhere *else*
/// is a separate question this battery does not ask and does not answer.
fn two_value_display_to_legacy(v: &str) -> String {
    let parts: Vec<&str> = v.split_ascii_whitespace().collect();
    if parts.len() < 2 {
        return v.to_string();
    }
    let (mut outside, mut inside, mut list_item) = (None, None, false);
    for p in &parts {
        match *p {
            "block" | "inline" => {
                if outside.replace(*p).is_some() {
                    return v.to_string(); // two outsides — invalid
                }
            }
            "flow" | "flow-root" | "table" | "flex" | "grid" | "ruby" => {
                if inside.replace(*p).is_some() {
                    return v.to_string();
                }
            }
            "list-item" => {
                if list_item {
                    return v.to_string();
                }
                list_item = true;
            }
            // Anything else makes the whole declaration invalid; hand it back unchanged so the
            // caller's `_ => None` arm rejects it and the previous value stands.
            _ => return v.to_string(),
        }
    }
    // `list-item` may only pair with `flow` / `flow-root`, and defaults to `block flow`.
    if list_item && !matches!(inside, None | Some("flow") | Some("flow-root")) {
        return v.to_string();
    }
    let outside = outside.unwrap_or("block");
    let inside = inside.unwrap_or("flow");
    // `list-item` is block-level and its marker is generated elsewhere — same answer the
    // single-keyword arm gives, which is the point of routing through it.
    if list_item {
        return "list-item".to_string();
    }
    match (outside, inside) {
        ("block", "flow") => "block",
        ("block", "flow-root") => "flow-root",
        ("inline", "flow") => "inline",
        ("inline", "flow-root") => "inline-block",
        ("block", "flex") => "flex",
        ("inline", "flex") => "inline-flex",
        ("block", "grid") => "grid",
        ("inline", "grid") => "inline-grid",
        ("block", "table") => "table",
        ("inline", "table") => "inline-table",
        // `ruby` has no single-keyword equivalent in this engine; leave it unrecognised rather
        // than inventing one.
        _ => return v.to_string(),
    }
    .to_string()
}

/// **A NEGATIVE LENGTH ON A SIZE PROPERTY IS A PARSE ERROR, NOT A ZERO.**
///
/// `width`, `height` and the four min/max sizing properties take `<length-percentage [0,∞]>`. A
/// declaration outside that range is **invalid and dropped**, which is a different observable from
/// clamping it: the cascade falls back to whatever was already there. Chrome, in a 400px container:
///
/// ```text
///                                       Chrome   clamp-to-0 would give
///   width:-5px                            400            0
///   width:200px; width:-5px               200            0   <- the DECISIVE row
///   width:-5%                             400            0
///   width:200px; max-width:-5px           200            0
///   min-width:-5px; width:50px             50           50   <- CONTROL, agrees by ACCIDENT
///   width:calc(50% - 300px)                 0            0   <- CONTROL: calc is NOT a parse error
/// ```
///
/// ⚠⚠⚠ **`width:200px; width:-5px` is what locates the fix.** Reinterpreting a negative width as
/// `auto` down in layout would answer 400 on the first row and **400 on the second**, where Chrome
/// says 200. Only *not applying the declaration* leaves the earlier one standing, so this belongs
/// here — at the point of application — and nowhere else.
///
/// ⚠⚠ **`min-width` was already right, and for the wrong reason: its initial value IS 0**, so
/// clamping a negative to zero and dropping the declaration agree at exactly one point. `max-width`
/// initialises to `none`, so the same clamp takes the box to **zero width**. A battery that checked
/// the min half and inferred the max half would have cleared this.
///
/// ⚠ **`calc()` is deliberately NOT rejected.** A negative *result* is allowed at computed-value
/// time and clamped to 0 at used-value time — `width:calc(50% - 300px)` is 0 in Chrome, not `auto`.
/// Rejecting `Dim::Calc` here would break that row, which is why it is in the battery as a control.
fn is_negative_size(d: Dim) -> bool {
    matches!(d, Dim::Px(v) | Dim::Percent(v) if v < 0.0)
}

fn intrinsic_kw_bare(v: &str) -> Option<IntrinsicSize> {
    match v.trim().to_ascii_lowercase().as_str() {
        "min-content" => Some(IntrinsicSize::MinContent),
        "max-content" => Some(IntrinsicSize::MaxContent),
        "fit-content" => Some(IntrinsicSize::FitContent),
        _ => None,
    }
}

/// Apply one declaration onto a computed style. Unknown properties/values are
/// silently ignored (CSS error recovery). `parent_fs` resolves `em`/`%` fonts.
fn apply_declaration(s: &mut ComputedStyle, d: &Declaration, parent_fs: f32) {
    let v = d.value.trim();
    match d.name.as_str() {
        "display" => {
            // ⚠ **`-webkit-box` is not a vendor-prefixed curiosity, it is how the web clamps text.**
            // `display:-webkit-box; -webkit-box-orient:vertical; -webkit-line-clamp:N;
            // overflow:hidden` is THE card/excerpt truncation idiom, and the display keyword is the
            // half that makes the box a block — without it the clamp we already implement never runs
            // (`line_clamp` is applied in the block-inline path). Chrome computes the clamped case to
            // `flow-root` and the bare case to `-webkit-box`; both are BLOCK-LEVEL, which is the part
            // that decides layout, so both map to `Block` here.
            //
            // The legacy flex-container half — `-webkit-box-orient: horizontal` laying element
            // children out in a ROW — is deliberately NOT implemented (recorded in
            // `CONSTELLATION.tsv`): the dominant idiom is text-only or `orient: vertical`, and the
            // pre-fix behaviour for the row case was `inline`, which stacked them anyway.
            // The second element is the `-webkit-box` MARKER: set by the two legacy keywords, and
            // CLEARED by any other recognised value, so a later `display:flex` that wins the cascade
            // also wins the recovery. An UNRECOGNISED value is an invalid declaration and leaves both
            // the display and the marker exactly as they were.
            // ⚠⚠⚠ **CSS DISPLAY L3'S TWO-VALUE SYNTAX, CANONICALISED TO THE LEGACY KEYWORD RATHER
            // THAN GIVEN ITS OWN MAPPING TABLE.** `display: inline flow-root` IS `inline-block`;
            // they are two spellings of one computed value, and the moment this arm grows a second
            // copy of the table below, the two spellings can drift apart. Rewriting the value and
            // falling through is the whole implementation.
            let v = &two_value_display_to_legacy(v);
            let v: &str = v;
            let parsed = match v {
                "-webkit-box" => Some((Display::Block, Some(Display::Block))),
                "-webkit-inline-box" => Some((Display::InlineBlock, Some(Display::InlineBlock))),
                "block" => Some((Display::Block, None)),
                "inline" => Some((Display::Inline, None)),
                "inline-block" => Some((Display::InlineBlock, None)),
                "flex" => Some((Display::Flex, None)),
                "grid" => Some((Display::Grid, None)),
                "inline-flex" => Some((Display::InlineFlex, None)),
                "inline-grid" => Some((Display::InlineGrid, None)),
                "table" | "inline-table" => Some((Display::Table, None)),
                "table-row-group" => Some((Display::TableRowGroup, None)),
                "table-header-group" => Some((Display::TableHeaderGroup, None)),
                "table-footer-group" => Some((Display::TableFooterGroup, None)),
                "table-row" => Some((Display::TableRow, None)),
                "table-cell" => Some((Display::TableCell, None)),
                "table-caption" => Some((Display::TableCaption, None)),
                "table-column" => Some((Display::TableColumn, None)),
                "table-column-group" => Some((Display::TableColumnGroup, None)),
                "flow-root" => Some((Display::FlowRoot, None)),
                // `list-item` is block-level; the marker is generated elsewhere. Both cascades must
                // agree here — a keyword one of them knows and the other does not is the
                // two-cascades trap, and it produces a divergence nobody can reproduce.
                "list-item" => Some((Display::Block, None)),
                "contents" => Some((Display::Contents, None)),
                "none" => Some((Display::None, None)),
                _ => None,
            };
            if let Some((d, legacy)) = parsed {
                s.display = d;
                s.legacy_webkit_box = legacy;
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
                "start" => TextAlign::Start,
                "end" => TextAlign::End,
                _ => TextAlign::Left,
            }
        }
        "text-indent" => {
            // Length or %-of-containing-block; stored as `Dim`, resolved at layout. The `hanging`
            // and `each_line` keywords are accepted-and-ignored (rare; the length is what indents).
            let first = v.split_whitespace().next().unwrap_or("");
            s.text_indent = values::parse_dim(first, s.font_size);
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
        "-webkit-line-clamp" | "line-clamp" => {
            // `<integer>` (≥1) clamps to that many lines; `none`/`0`/garbage → unclamped. The full
            // `line-clamp` shorthand also carries `<block-ellipsis>`/`continue`, but the authored form
            // on the web is overwhelmingly a bare integer, which is all layout consumes.
            let t = v.trim();
            s.line_clamp = if t.eq_ignore_ascii_case("none") {
                None
            } else {
                t.parse::<u16>().ok().filter(|&n| n >= 1)
            };
        }
        "scroll-snap-type" => {
            // `x mandatory` / `y proximity` / `both mandatory` / `none`. The axis is the first
            // token; the strictness token is accepted and ignored (see `ScrollSnapAxis`).
            let lower = v.trim().to_ascii_lowercase();
            let axis = lower.split_whitespace().next().unwrap_or("");
            s.scroll_snap_type = match axis {
                "x" | "inline" => ScrollSnapAxis::X,
                "y" | "block" => ScrollSnapAxis::Y,
                "both" => ScrollSnapAxis::Both,
                "none" => ScrollSnapAxis::None,
                // INVALID → drop the declaration (CSS 2.1 §4.2), do not reset to the initial value.
                _ => return,
            }
        }
        "scroll-snap-align" => {
            // One value sets both axes; two set block then inline. We snap on whichever axis the
            // CONTAINER declares, so a single alignment is all that is consulted — taking the first
            // token is correct for `start`/`center`/`end` and for the doubled `start start`.
            let lower = v.trim().to_ascii_lowercase();
            let first = lower.split_whitespace().next().unwrap_or("");
            s.scroll_snap_align = match first {
                "start" => ScrollSnapAlign::Start,
                "center" => ScrollSnapAlign::Center,
                "end" => ScrollSnapAlign::End,
                "none" => ScrollSnapAlign::None,
                // INVALID → drop the declaration (CSS 2.1 §4.2).
                _ => return,
            }
        }
        // ⚠⚠⚠ **`_ => Initial` APPLIES AN INVALID DECLARATION; CSS 2.1 §4.2 SAYS TO IGNORE IT.** The
        // difference only shows when a valid declaration came first — `text-transform: uppercase;
        // text-transform: banana` must stay UPPERCASE, and a `_ => TextTransform::None` arm quietly
        // resets it. Measured against live Chromium on `<span>wwwww</span>` at 16px, with the three
        // control rows that make this a rule about *dropping* rather than about garbage:
        //
        // ```text
        //                               CHROME    before    after
        //     uppercase; banana          75.52      58        76     ✗→✓
        //     uppercase                  75.52      76        76     ✓ we DO apply it
        //     banana only                57.78      58        58     ✓ only-invalid → initial
        //     uppercase; none            57.78      58        58     ✓ a VALID override still wins
        // ```
        //
        // The last two rows are the ones a careless fix breaks: making the property *sticky* would
        // keep `uppercase` across row four, and treating unknown-only as *inherit* would break row
        // three. Leaving the field untouched is the only shape that satisfies all four, and it is
        // also what "ignore the declaration" literally means.
        //
        // ⚠⚠ **THE SCOPE IS ARCHITECTURAL, NOT A HEDGE.** The shipping cascade is Stylo, which drops
        // invalid declarations correctly on its own. This applies to the properties `stylo_engine`
        // RECOVERS from here because Stylo's servo build cannot express them — those arms decide the
        // shipping answer, and those are the arms fixed. See `NONE_IS_A_REAL_KEYWORD` below for the
        // one case that still assigns on the fall-through, and why.
        "text-transform" => {
            s.text_transform = match v.trim().to_ascii_lowercase().as_str() {
                "uppercase" => TextTransform::Uppercase,
                "lowercase" => TextTransform::Lowercase,
                "capitalize" => TextTransform::Capitalize,
                "none" => TextTransform::None,
                // Not a keyword this property takes → the declaration is INVALID and is dropped.
                _ => return,
            }
        }
        // `scrollbar-width`/`scrollbar-color` are `engine="gecko"` in stylo 0.19 (dropped from the
        // servo build entirely, like `-webkit-line-clamp`), so they are recovered here and merged in
        // `stylo_engine`. We resolve only the COMPUTED value the CSSOM reports; painting a themed
        // scrollbar is out of scope.
        "scrollbar-width" => {
            s.scrollbar_width = match v.trim().to_ascii_lowercase().as_str() {
                "thin" => ScrollbarWidth::Thin,
                "none" => ScrollbarWidth::None,
                "auto" => ScrollbarWidth::Auto,
                // INVALID → drop the declaration (CSS 2.1 §4.2).
                _ => return,
            }
        }
        "scrollbar-color" => {
            // `auto` | `<thumb-color> <track-color>`. Split at the first space at paren depth 0 so the
            // commas/spaces inside `rgb(…)` do not fool the token boundary; a malformed pair → `auto`.
            let t = v.trim();
            s.scrollbar_color = if t.eq_ignore_ascii_case("auto") {
                ScrollbarColor::Auto
            } else {
                let mut depth = 0i32;
                let mut split = None;
                for (i, c) in t.char_indices() {
                    match c {
                        '(' => depth += 1,
                        ')' => depth -= 1,
                        c if c.is_whitespace() && depth == 0 => {
                            split = Some(i);
                            break;
                        }
                        _ => {}
                    }
                }
                split
                    .map(|i| (t[..i].trim(), t[i..].trim()))
                    .and_then(
                        |(a, b)| match (values::parse_color(a), values::parse_color(b)) {
                            (Some(thumb), Some(track)) => {
                                Some(ScrollbarColor::Colors { thumb, track })
                            }
                            _ => None,
                        },
                    )
                    .unwrap_or(ScrollbarColor::Auto)
            };
        }
        // `overflow-wrap` and its legacy alias `word-wrap` map to the same computed value.
        "overflow-wrap" | "word-wrap" => {
            s.overflow_wrap = match v.trim().to_ascii_lowercase().as_str() {
                "break-word" => OverflowWrap::BreakWord,
                "anywhere" => OverflowWrap::Anywhere,
                "normal" => OverflowWrap::Normal,
                // INVALID → drop the declaration (CSS 2.1 §4.2).
                _ => return,
            }
        }
        "direction" => {
            s.direction = match v.trim().to_ascii_lowercase().as_str() {
                "rtl" => Direction::Rtl,
                "ltr" => Direction::Ltr,
                // INVALID → drop the declaration (CSS 2.1 §4.2).
                _ => return,
            }
        }
        "word-break" => {
            s.word_break = match v.trim().to_ascii_lowercase().as_str() {
                "break-all" => WordBreak::BreakAll,
                "keep-all" => WordBreak::KeepAll,
                "normal" => WordBreak::Normal,
                // INVALID → drop the declaration (CSS 2.1 §4.2).
                _ => return,
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
        // `tab-size` (and its `-moz-` alias, which is still what a lot of shipped CSS writes): a
        // BARE NUMBER is a count of space advances, anything with a unit is a length. The two are
        // kept apart all the way to layout — see `TabSize`. A negative or unparseable value keeps
        // the initial 8, which is what an unstyled `<pre>` renders with.
        "tab-size" | "-moz-tab-size" => {
            let t = v.trim();
            s.tab_size = if let Ok(n) = t.parse::<f32>() {
                if n >= 0.0 {
                    TabSize::Spaces(n)
                } else {
                    TabSize::default()
                }
            } else {
                match values::parse_length_px(t, s.font_size) {
                    Some(px) if px >= 0.0 => TabSize::Px(px),
                    _ => TabSize::default(),
                }
            }
        }
        // `transform-origin: <x> <y>` — the point a transform is applied about, for the
        // **MinimalCascade** (JS-less / headless fallback) path; the shipping cascade takes Stylo's
        // own computed value in `stylo_map.rs`. Kept in step with it deliberately: t975 found the
        // same property fixed in one path and not the other, and the two answering differently is
        // its own class of bug.
        //
        // ⚠ `top`/`bottom` name the Y axis WHEREVER they appear, so `top left` is as valid as
        // `left top`; reading the two words positionally silently swaps that pair. A third (z)
        // component is accepted and ignored — it only matters under a perspective context.
        "transform-origin" => {
            let t = v.trim().to_ascii_lowercase();
            let kw = |w: &str| -> Option<Dim> {
                match w {
                    "left" | "top" => Some(Dim::Percent(0.0)),
                    "center" => Some(Dim::Percent(50.0)),
                    "right" | "bottom" => Some(Dim::Percent(100.0)),
                    _ => None,
                }
            };
            let axis_of = |w: &str| match w {
                "left" | "right" => Some(0usize),
                "top" | "bottom" => Some(1usize),
                _ => None,
            };
            let mut xy: [Option<Dim>; 2] = [None, None];
            let mut next = 0usize;
            for w in t.split_whitespace().take(3) {
                // `parse_dim` answers `Auto` for anything it cannot read, and `transform-origin`
                // never takes `auto` — so `Auto` here means "not a length", not a value.
                let val = kw(w).or_else(|| match values::parse_dim(w, s.font_size) {
                    Dim::Auto => None,
                    d => Some(d),
                });
                let Some(val) = val else { continue };
                let slot = axis_of(w).unwrap_or_else(|| next.min(1));
                if xy[slot].is_none() {
                    xy[slot] = Some(val);
                }
                next = slot + 1;
            }
            s.transform_origin = (
                xy[0].unwrap_or(Dim::Percent(50.0)),
                xy[1].unwrap_or(Dim::Percent(50.0)),
            );
        }
        "width" => {
            // Intrinsic sizing keywords collapse to `Dim::Auto` for length resolution, but tag which
            // one so block width resolution hugs the content instead of filling (`stretch` /
            // `-webkit-fill-available` are definite fills → not tagged), at parity with the stylo map.
            let low = v.trim().to_ascii_lowercase();
            s.width_keyword = intrinsic_kw(v);
            // The inline mirror of `height_stretch`: DEFINITE, and it FILLS — which only differs
            // from `auto` for the boxes that shrink-to-fit (float / abspos / inline-block / replaced
            // / form control), and that is precisely where it matters.
            s.width_stretch = matches!(
                low.as_str(),
                "stretch" | "-webkit-fill-available" | "-moz-available"
            );
            // A negative length/percentage makes the DECLARATION invalid — see `is_negative_size`.
            let d = values::parse_dim(v, s.font_size);
            if !is_negative_size(d) {
                s.width = d;
            }
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
            let d = values::parse_dim(v, s.font_size);
            if !is_negative_size(d) {
                s.height = d;
            }
        }
        // The four min/max sizing properties take the same intrinsic keywords `width`/`height` do,
        // and until t930 they were parsed with `parse_dim` alone — which answers `Dim::Auto` for a
        // keyword it does not know, i.e. **0 on a min and no-limit on a max**. Tag the keyword
        // beside the `Dim` exactly as the `width` arm above does, at parity with the stylo map.
        "min-width" => {
            let low = v.trim().to_ascii_lowercase();
            s.min_width_stretch = matches!(
                low.as_str(),
                "stretch" | "-webkit-fill-available" | "-moz-available"
            );
            let d = values::parse_dim(v, s.font_size);
            if !is_negative_size(d) {
                s.min_width_keyword = intrinsic_kw_bare(v);
                s.min_width = d;
            }
        }
        "max-width" => {
            let low = v.trim().to_ascii_lowercase();
            s.max_width_stretch = matches!(
                low.as_str(),
                "stretch" | "-webkit-fill-available" | "-moz-available"
            );
            let d = values::parse_dim(v, s.font_size);
            if !is_negative_size(d) {
                s.max_width_keyword = intrinsic_kw_bare(v);
                s.max_width = d;
            }
        }
        // ⚠ The `stretch` flag is set OUTSIDE the `is_negative_size` guard's effect on the `Dim`,
        // because `stretch` parses to `Dim::Auto` (not a negative length) and the guard passes — but
        // it is set from the same keyword text, so a bad value cannot turn it on. Twin of the
        // `height` arm above and of `stylo_map`'s `size_is_stretch`/`maxsize_is_stretch`.
        "min-height" => {
            let low = v.trim().to_ascii_lowercase();
            s.min_height_stretch = matches!(
                low.as_str(),
                "stretch" | "-webkit-fill-available" | "-moz-available"
            );
            let d = values::parse_dim(v, s.font_size);
            if !is_negative_size(d) {
                s.min_height_keyword = intrinsic_kw_bare(v);
                s.min_height = d;
            }
        }
        "max-height" => {
            let low = v.trim().to_ascii_lowercase();
            s.max_height_stretch = matches!(
                low.as_str(),
                "stretch" | "-webkit-fill-available" | "-moz-available"
            );
            let d = values::parse_dim(v, s.font_size);
            if !is_negative_size(d) {
                s.max_height_keyword = intrinsic_kw_bare(v);
                s.max_height = d;
            }
        }
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
            // ⚠⚠⚠ **TWO LENGTHS, AND THE SECOND ONE WAS DROPPED (t925).** The comment this replaces
            // said *"Only the first (horizontal) length is used in this slice"* — accurate, and the
            // consequence was that `border-spacing: 10px 20px` inset the ROWS by 10 instead of 20:
            // Chrome makes that table **64** tall and we made it 44. One value still sets both, per
            // the shorthand.
            let mut it = v
                .split_whitespace()
                .filter_map(|t| values::parse_length_px(t, s.font_size));
            if let Some(h) = it.next() {
                s.border_spacing = h;
                s.border_spacing_v = it.next().unwrap_or(h);
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
        // ── The four container-level alignment longhands. They are parsed by TWO shared helpers
        //    (`values::parse_content_distribution` / `parse_item_alignment`) rather than by four
        //    hand-written match blocks, because the hand-written form is exactly how `align-content`
        //    and `justify-items` came to be missing: a reader who sees `justify-content` handled
        //    directly above `align-items` reads the pair as coverage of the family. One helper per
        //    axis-independent VALUE SET makes an absent property a missing call, not a missing arm.
        "justify-content" => {
            s.justify_content = values::parse_content_distribution(v, values::AlignAxis::Inline)
        }
        "align-content" => {
            s.align_content = values::parse_content_distribution(v, values::AlignAxis::Block)
        }
        "align-items" => s.align_items = values::parse_item_alignment(v, values::AlignAxis::Block),
        "justify-items" => {
            s.justify_items = values::parse_item_alignment(v, values::AlignAxis::Inline)
        }
        // The `place-*` shorthands set BOTH axes, ALIGN first. `place-content: center` is one token
        // for `align-content: center; justify-content: center`; two tokens set align then justify.
        "place-content" => {
            let mut it = v.split_whitespace();
            let a = it.next().unwrap_or("normal");
            let j = it.next().unwrap_or(a);
            s.align_content = values::parse_content_distribution(a, values::AlignAxis::Block);
            s.justify_content = values::parse_content_distribution(j, values::AlignAxis::Inline);
        }
        "place-items" => {
            let mut it = v.split_whitespace();
            let a = it.next().unwrap_or("normal");
            let j = it.next().unwrap_or(a);
            s.align_items = values::parse_item_alignment(a, values::AlignAxis::Block);
            s.justify_items = values::parse_item_alignment(j, values::AlignAxis::Inline);
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
            // `gap: <row> [<column>]`. `parse_dim` rather than `parse_length_px` so a PERCENTAGE
            // survives to layout, which is the only participant that knows the basis.
            let parts: Vec<Dim> = v
                .split_whitespace()
                .map(|t| values::parse_dim(t, s.font_size))
                .filter(|d| !matches!(d, Dim::Auto))
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
            let d = values::parse_dim(v.trim(), s.font_size);
            if !matches!(d, Dim::Auto) {
                s.row_gap = d;
            }
        }
        "column-gap" => {
            let d = values::parse_dim(v.trim(), s.font_size);
            if !matches!(d, Dim::Auto) {
                s.column_gap = d;
            }
        }
        // `justify-self` — the INLINE-axis twin of `align-self` for a grid item. Same keyword set:
        // the flex spellings (`flex-start`/`flex-end`) are accepted alongside the logical ones
        // because authors write both, and a grid item styled by a flex-era design token is common.
        "justify-self" => {
            s.justify_self = match v.trim() {
                "auto" => None,
                "center" => Some(AlignItems::Center),
                "flex-end" | "end" | "right" => Some(AlignItems::FlexEnd),
                "flex-start" | "start" | "left" => Some(AlignItems::FlexStart),
                "baseline" => Some(AlignItems::Baseline),
                "stretch" => Some(AlignItems::Stretch),
                "normal" => Some(AlignItems::Normal),
                _ => None,
            };
        }
        "align-self" => {
            s.align_self = match v.trim() {
                "auto" => None,
                "center" => Some(AlignItems::Center),
                "flex-end" | "end" => Some(AlignItems::FlexEnd),
                "flex-start" | "start" => Some(AlignItems::FlexStart),
                "baseline" => Some(AlignItems::Baseline),
                "stretch" => Some(AlignItems::Stretch),
                "normal" => Some(AlignItems::Normal),
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
        "grid-auto-rows" => s.grid_auto_rows = parse_auto_track_list(v, s.font_size),
        "grid-auto-columns" => s.grid_auto_columns = parse_auto_track_list(v, s.font_size),
        // `will-change` / `contain` / `perspective` — the three ways to become a containing block
        // for out-of-flow descendants WITHOUT being positioned and without a transform. Only the
        // one bit layout needs is kept; see the field's own doc for why it is not the value list.
        //
        // ⚠ The negative half is measured, not assumed: `will-change: opacity` does NOT create one
        // (Chrome puts the fixed child back on the viewport, -364 from the wrapper), and neither do
        // `contain: style` or `contain: size`. A predicate written as "any will-change" or "any
        // contain" would pass every positive row in the fixture and be wrong about all three.
        "will-change" => {
            s.establishes_containing_block |= v.split(',').any(|f| {
                matches!(
                    f.trim().to_ascii_lowercase().as_str(),
                    "transform"
                        | "perspective"
                        | "filter"
                        | "backdrop-filter"
                        | "rotate"
                        | "scale"
                        | "translate"
                )
            });
        }
        "contain" => {
            s.establishes_containing_block |= v.split_ascii_whitespace().any(|f| {
                matches!(
                    f.trim().to_ascii_lowercase().as_str(),
                    "layout" | "paint" | "strict" | "content"
                )
            });
        }
        "perspective" => {
            s.establishes_containing_block |= v.trim().to_ascii_lowercase() != "none";
        }
        "grid-auto-flow" => {
            if let Some(f) = parse_grid_auto_flow(v) {
                s.grid_auto_flow = f;
            }
        }
        "grid-column" => s.grid_column = parse_grid_line_shorthand(v),
        "grid-row" => s.grid_row = parse_grid_line_shorthand(v),
        "grid-column-start" => s.grid_column.0 = parse_grid_line(v),
        "grid-column-end" => s.grid_column.1 = parse_grid_line(v),
        "grid-row-start" => s.grid_row.0 = parse_grid_line(v),
        "grid-row-end" => s.grid_row.1 = parse_grid_line(v),
        "transform" => s.transform = parse_transform(v, s.font_size),
        // The individual transform properties. `none` is the initial value and must CLEAR a value
        // an earlier rule set, so each arm assigns rather than only assigning on success.
        "translate" => {
            s.translate = (v.trim().to_ascii_lowercase() != "none").then(|| {
                let mut p = v.split_ascii_whitespace();
                let x = p
                    .next()
                    .map(|t| values::parse_dim(t, s.font_size))
                    .unwrap_or(Dim::Px(0.0));
                // A one-value `translate` leaves y at 0 — NOT at x. `translate: 30px` is
                // `translate(30px, 0)`, the same shorthand rule the function has.
                let y = p
                    .next()
                    .map(|t| values::parse_dim(t, s.font_size))
                    .unwrap_or(Dim::Px(0.0));
                (x, y)
            })
        }
        "rotate" => {
            // `rotate: x|y|z <angle>` and `rotate: <x> <y> <z> <angle>` are the 3D spellings; only
            // a rotation about z has a 2D effect, and taking the angle off `z 45deg` while ignoring
            // `x 45deg` is the same exact-projection rule `stylo_map` applies to `rotate3d`.
            let t = v.trim().to_ascii_lowercase();
            s.rotate = if t == "none" || t.is_empty() {
                None
            } else {
                let parts: Vec<&str> = t.split_ascii_whitespace().collect();
                let axis_angle = match parts.as_slice() {
                    // A bare angle is a rotation about z.
                    [a] => parse_angle_rad(a).map(|r| (0.0, 0.0, 1.0, r)),
                    ["x", a] => parse_angle_rad(a).map(|r| (1.0, 0.0, 0.0, r)),
                    ["y", a] => parse_angle_rad(a).map(|r| (0.0, 1.0, 0.0, r)),
                    ["z", a] => parse_angle_rad(a).map(|r| (0.0, 0.0, 1.0, r)),
                    [x, y, z, a] => parse_angle_rad(a).map(|r| {
                        (
                            x.parse::<f32>().unwrap_or(0.0),
                            y.parse::<f32>().unwrap_or(0.0),
                            z.parse::<f32>().unwrap_or(0.0),
                            r,
                        )
                    }),
                    _ => None,
                };
                axis_angle.and_then(|(x, y, z, r)| axis_rotation_2d(x, y, z, r))
            };
        }
        "scale" => {
            let t = v.trim().to_ascii_lowercase();
            s.scale = if t == "none" || t.is_empty() {
                None
            } else {
                let n: Vec<f32> = t
                    .split_ascii_whitespace()
                    .map(|p| {
                        p.strip_suffix('%')
                            .and_then(|q| q.parse::<f32>().ok().map(|v| v / 100.0))
                            .or_else(|| p.parse::<f32>().ok())
                            .unwrap_or(1.0)
                    })
                    .collect();
                match n.as_slice() {
                    // A one-value `scale` is UNIFORM — the opposite of `translate`'s rule, which is
                    // why both are written out here rather than shared.
                    [x] => Some((*x, *x)),
                    [x, y] | [x, y, _] => Some((*x, *y)),
                    _ => None,
                }
            };
        }
        "vertical-align" => {
            s.vertical_align = match v.trim() {
                "top" => VerticalAlign::Top,
                "middle" => VerticalAlign::Middle,
                "bottom" => VerticalAlign::Bottom,
                "text-top" => VerticalAlign::TextTop,
                "text-bottom" => VerticalAlign::TextBottom,
                "sub" => VerticalAlign::Sub,
                "super" => VerticalAlign::Super,
                // ⚠⚠⚠ **THE LENGTH AND PERCENTAGE FORMS WERE UNREPRESENTABLE, SO THEY PARSED TO
                // `baseline` AND VANISHED (t922).** `vertical-align: -2px` is the standard idiom for
                // nudging an inline icon or badge against its label, and `50%` is how a raised
                // marker is expressed relative to its own line. Chrome-measured on 16px/1.5 text:
                // `10px` and `-10px` each grow the line to **34**, and `50%` to **36** — against 24
                // for all three before.
                other => {
                    let t = other.trim();
                    if let Some(pct) = t.strip_suffix('%') {
                        pct.trim()
                            .parse::<f32>()
                            .map(|p| VerticalAlign::Percent(p / 100.0))
                            .unwrap_or(VerticalAlign::Baseline)
                    } else if let Some(px) = values::parse_length_px(t, s.font_size) {
                        VerticalAlign::Length(px)
                    } else {
                        VerticalAlign::Baseline
                    }
                }
            };
        }
        // The `border` family. Widths feed the box model; the colour and the line style feed paint,
        // and both are **per side** — a shorthand that names one side must not repaint the other
        // three, which is exactly what it did until t1079.
        "border" => {
            let (w, c, st) = parse_border_shorthand(v, s.font_size);
            if let Some(w) = w {
                s.border_width = Sides::all(w);
            }
            if let Some(c) = c {
                s.border_color = Sides::all(c);
            }
            if let Some(st) = st {
                s.border_style = Sides::all(st);
            }
        }
        "border-top" | "border-right" | "border-bottom" | "border-left" => {
            let (w, c, st) = parse_border_shorthand(v, s.font_size);
            let side = d.name.as_str();
            if let Some(w) = w {
                *side_mut(&mut s.border_width, side) = w;
            }
            if let Some(c) = c {
                *side_mut(&mut s.border_color, side) = c;
            }
            if let Some(st) = st {
                *side_mut(&mut s.border_style, side) = st;
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
        "filter" | "-webkit-filter" => s.filter = parse_filters(v, s.font_size),
        "backdrop-filter" | "-webkit-backdrop-filter" => {
            s.backdrop_filter = parse_filters(v, s.font_size)
        }
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
        // `field-sizing` (Baseline June 2026). Stylo 0.19 predates it, so this cascade is the
        // property's only source — same recovered-property route as `visibility` above.
        "field-sizing" => {
            s.field_sizing_content = v.trim().eq_ignore_ascii_case("content");
        }
        // `appearance` / `-webkit-appearance` — `engine="gecko"` in stylo 0.19, so this cascade is
        // the property's only source. Only `none` is consumed: it is the one value with a geometric
        // consequence here (a `<select>` stops reserving the dropdown arrow's width), and inventing
        // behaviour for the other keywords would be modelling widgets this engine does not draw.
        // The `-webkit-` alias is not decoration — it is what the majority of shipped CSS writes,
        // usually in the same declaration block as the unprefixed one.
        "appearance" | "-webkit-appearance" => {
            // ⚠ An INVALID keyword leaves the property alone — CSS drops a declaration it cannot
            // parse, and `appearance-cssom-001` asserts exactly that (`bogus-button` must read back
            // as the empty string, not as itself). `None` from the mapper means "not a value".
            if let Some(a) = appearance_from_keyword(v.trim()) {
                s.appearance = a;
            }
        }
        // `order` — kept in lockstep with the Stylo path (`clone_order`), because the two cascades
        // disagreeing on a property is how a `<source>` got 19px of height in one configuration and
        // none in the other.
        "order" => {
            if let Ok(n) = v.trim().parse::<i32>() {
                s.order = n;
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
                Some(parse_content_parts(t))
            };
        }
        "counter-reset" => s.counter_reset = parse_counter_list(v, 0),
        "counter-increment" => s.counter_increment = parse_counter_list(v, 1),
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
        // `border-color` takes the CSS box-side shorthand's 1-4 value form, exactly as
        // `border-width` does — `border-color: red blue` is red on the block axis and blue on the
        // inline one, and collapsing it to the first token painted three sides the wrong colour.
        "border-color" => {
            let toks: Vec<&str> = v.split_whitespace().collect();
            if let Some(sides) = expand_box_sides(&toks, values::parse_color) {
                s.border_color = sides;
            }
        }
        "border-top-color" => set_side_color(&mut s.border_color.top, v),
        "border-right-color" => set_side_color(&mut s.border_color.right, v),
        "border-bottom-color" => set_side_color(&mut s.border_color.bottom, v),
        "border-left-color" => set_side_color(&mut s.border_color.left, v),
        // `none`/`hidden` remove THAT side's border; other styles keep whatever width is set.
        //
        // ⚠ **`border-left-style: none` used to zero all four widths.** The `Sides::all(0.0)` below
        // was written when the style was a scalar, and it made a single-side reset delete the box's
        // whole border — the shape `border: 1px solid; border-right-style: none` takes on every
        // segmented control and button group on the web.
        "border-style" => {
            let toks: Vec<&str> = v.split_whitespace().collect();
            if let Some(sides) = expand_sides_str(&toks) {
                for (name, tok) in [
                    ("border-top", sides.top),
                    ("border-right", sides.right),
                    ("border-bottom", sides.bottom),
                    ("border-left", sides.left),
                ] {
                    set_side_style(s, name, tok);
                }
            }
        }
        "border-top-style" | "border-right-style" | "border-bottom-style" | "border-left-style" => {
            if let Some(first) = v.split_whitespace().next() {
                let side = d.name.as_str().trim_end_matches("-style");
                set_side_style(s, side, first);
            }
        }
        _ => {}
    }
}

/// One side of a [`Sides`], selected by the CSS property name that names it (`border-top`,
/// `padding-left`, …). The name is matched on its LAST segment, so `border-top` and
/// `border-top-style` both select `top`.
fn side_mut<'a, T>(sides: &'a mut Sides<T>, prop: &str) -> &'a mut T {
    if prop.contains("-right") {
        &mut sides.right
    } else if prop.contains("-bottom") {
        &mut sides.bottom
    } else if prop.contains("-left") {
        &mut sides.left
    } else {
        &mut sides.top
    }
}

/// **CSS's 1-to-4-value box-side shorthand** (`<all>` · `<block> <inline>` · `<top> <inline>
/// <bottom>` · `<top> <right> <bottom> <left>`), applied to any parsed value type.
///
/// Returns `None` when the list is empty, over-long, or any token fails to parse — a partially
/// understood `border-color: red notacolor` must leave the whole declaration alone rather than
/// paint half of it, because a declaration with an invalid component is invalid as a whole
/// (CSS Syntax); applying the parseable half is a silent, asymmetric render.
fn expand_box_sides<T: Copy, F: Fn(&str) -> Option<T>>(toks: &[&str], f: F) -> Option<Sides<T>> {
    let s = expand_sides_str(toks)?;
    Some(Sides {
        top: f(s.top)?,
        right: f(s.right)?,
        bottom: f(s.bottom)?,
        left: f(s.left)?,
    })
}

/// [`expand_box_sides`]'s positional half, before any value is parsed — kept separate because a
/// `border-style` list is consumed as raw tokens (`none` is not a `BorderStyle`, it is a width of
/// zero), and the 1-to-4 rule must not be written twice.
fn expand_sides_str<'a>(toks: &[&'a str]) -> Option<Sides<&'a str>> {
    Some(match toks {
        [a] => Sides::all(*a),
        [a, b] => Sides {
            top: a,
            bottom: a,
            right: b,
            left: b,
        },
        [a, b, c] => Sides {
            top: a,
            right: b,
            left: b,
            bottom: c,
        },
        [a, b, c, d] => Sides {
            top: a,
            right: b,
            bottom: c,
            left: d,
        },
        _ => return None,
    })
}

/// `border-<side>-color: <color>` — a no-op on an unparseable value, which is what an invalid
/// declaration must be.
fn set_side_color(slot: &mut Rgba, v: &str) {
    if let Some(c) = values::parse_color(v) {
        *slot = c;
    }
}

/// `border-<side>-style: <style>` on `side` (`border-top`, `border-right`, …).
///
/// `none`/`hidden` zero **that side's** width and nothing else — CSS 2.1 §8.5.3: a border with
/// `border-style: none` has a used width of zero whatever `border-width` says.
fn set_side_style(s: &mut ComputedStyle, side: &str, tok: &str) {
    if matches!(tok, "none" | "hidden") {
        *side_mut(&mut s.border_width, side) = 0.0;
    } else if let Some(st) = border_style_of(tok) {
        *side_mut(&mut s.border_style, side) = st;
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

/// Parse a `filter` value into its ordered function list.
///
/// A malformed *function* is dropped on its own rather than voiding the whole declaration. That is a
/// deliberate divergence from CSS's all-or-nothing declaration parsing, and it is the safer failure
/// here: the list is applied as a pipeline, so keeping the eight functions we understood and dropping
/// the one we did not renders closer to the author's intent than rendering nothing at all.
fn parse_filters(v: &str, fs: f32) -> Vec<FilterOp> {
    let v = v.trim();
    if v.is_empty() || v.eq_ignore_ascii_case("none") {
        return Vec::new();
    }
    let b = v.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < b.len() {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        let start = i;
        while i < b.len() && b[i] != b'(' && !b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= b.len() || b[i] != b'(' {
            break; // a bare ident (or `url(...)`-less garbage) — nothing further is parseable
        }
        let name = &v[start..i];
        // Walk to the MATCHING close paren: `drop-shadow(0 1px 2px rgba(0,0,0,.4))` nests.
        let args_start = i + 1;
        let mut depth = 0i32;
        while i < b.len() {
            match b[i] {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        if i >= b.len() {
            break; // unbalanced
        }
        let args = &v[args_start..i];
        i += 1;
        if let Some(op) = parse_filter_fn(name, args, fs) {
            out.push(op);
        }
    }
    out
}

/// `<number> | <percentage>` as a plain factor — `0.5` and `50%` are the same amount.
fn parse_amount(s: &str) -> Option<f32> {
    let s = s.trim();
    if let Some(p) = s.strip_suffix('%') {
        return p.trim().parse::<f32>().ok().map(|n| n / 100.0);
    }
    s.parse::<f32>().ok()
}

/// One `name(args)` chunk. An omitted argument means **1** for every amount filter (`grayscale()`
/// == `grayscale(1)`), which is why the defaults are not 0.
fn parse_filter_fn(name: &str, args: &str, fs: f32) -> Option<FilterOp> {
    let n = name
        .trim()
        .trim_start_matches("-webkit-")
        .to_ascii_lowercase();
    let a = args.trim();
    let amount = || {
        if a.is_empty() {
            1.0
        } else {
            parse_amount(a).unwrap_or(1.0)
        }
    };
    Some(match n.as_str() {
        "blur" => FilterOp::Blur(values::parse_length_px(a, fs).unwrap_or(0.0).max(0.0)),
        "brightness" => FilterOp::Brightness(amount().max(0.0)),
        "contrast" => FilterOp::Contrast(amount().max(0.0)),
        "grayscale" => FilterOp::Grayscale(amount().clamp(0.0, 1.0)),
        "hue-rotate" => FilterOp::HueRotate(parse_angle_deg(a).unwrap_or(0.0)),
        "invert" => FilterOp::Invert(amount().clamp(0.0, 1.0)),
        "opacity" => FilterOp::Opacity(amount().clamp(0.0, 1.0)),
        "saturate" => FilterOp::Saturate(amount().max(0.0)),
        "sepia" => FilterOp::Sepia(amount().clamp(0.0, 1.0)),
        "drop-shadow" => {
            let mut lens: Vec<f32> = Vec::new();
            let mut color: Option<Rgba> = None;
            for tok in tokens_keeping_parens(a) {
                if let Some(px) = values::parse_length_px(&tok, fs) {
                    lens.push(px);
                } else if let Some(c) = values::parse_color(&tok) {
                    color = Some(c);
                }
            }
            if lens.len() < 2 {
                return None; // both offsets are required
            }
            FilterOp::DropShadow {
                dx: lens[0],
                dy: lens[1],
                blur: lens.get(2).copied().unwrap_or(0.0).max(0.0),
                color: color.unwrap_or(Rgba::BLACK),
            }
        }
        _ => return None,
    })
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
            // ⚠⚠⚠ **THE 3D FUNCTIONS WERE FALLING INTO `_ => {}` AND BEING SILENTLY DROPPED**, and
            // `translate3d(x, y, 0)` is not an exotic spelling — it is *the* idiom for putting an
            // element on its own compositor layer, which is how every animation library, carousel,
            // drawer and sticky header on the modern web writes a translation. Dropped, the element
            // is left at its **untransformed** position, which is the largest possible error for the
            // property. Measured against Chrome, a 100×40 box:
            //
            // ```text
            //   transform: translate3d(20px,10px,0)    Chrome [20, 720]    before [0, 710]
            // ```
            //
            // The 2D projection of each of these is exact, not an approximation: with no
            // `perspective` in force, `z` contributes nothing to the on-screen position, so the
            // x/y terms of the 3D function ARE its rendered effect.
            //
            // ⚠ `rotate3d` is deliberately handled **only about the z axis**. A rotation about x or
            // y projects to a foreshortening that this 2D pipeline cannot express, and inventing one
            // would be a wrong answer of the right type; `rotate3d(0,0,1,θ)` — which is what a page
            // that wants a plain rotation on its own layer writes — is exactly `rotate(θ)`.
            "translate3d" => out.push(TransformFn::Translate(
                dim(0),
                nums.get(1)
                    .map(|s| values::parse_dim(s, fs))
                    .unwrap_or(Dim::Px(0.0)),
            )),
            // `translateZ`/`perspective` have no 2D effect without a perspective context. Matching
            // them explicitly (rather than letting them fall through) says the omission is a
            // DECISION — the `_` arm is where the bug above lived.
            "translatez" | "perspective" => {}
            "scale3d" => out.push(TransformFn::Scale(f(0).unwrap_or(1.0), f(1).unwrap_or(1.0))),
            "scalez" => {}
            "rotatez" => out.push(TransformFn::Rotate(
                nums.first().and_then(|s| angle(s)).unwrap_or(0.0),
            )),
            // `rotateX`/`rotateY` project to a SCALE on the other axis, exactly — see
            // `axis_rotation_2d`. They used to fall into `_ => {}` and be dropped.
            "rotatex" => {
                if let Some(t) = nums
                    .first()
                    .and_then(|s| angle(s))
                    .and_then(|a| axis_rotation_2d(1.0, 0.0, 0.0, a))
                {
                    out.push(t);
                }
            }
            "rotatey" => {
                if let Some(t) = nums
                    .first()
                    .and_then(|s| angle(s))
                    .and_then(|a| axis_rotation_2d(0.0, 1.0, 0.0, a))
                {
                    out.push(t);
                }
            }
            "rotate3d" => {
                let (x, y, z) = (
                    f(0).unwrap_or(0.0),
                    f(1).unwrap_or(0.0),
                    f(2).unwrap_or(0.0),
                );
                if let Some(t) = nums
                    .get(3)
                    .and_then(|s| angle(s))
                    .and_then(|a| axis_rotation_2d(x, y, z, a))
                {
                    out.push(t);
                }
            }
            // `matrix3d` is a 4×4 in column-major order; its 2D projection takes the four linear
            // terms and the two translations — m11 m12 m21 m22 m41 m42, i.e. indices 0 1 4 5 12 13.
            "matrix3d" => {
                if nums.len() == 16 {
                    let v: Option<Vec<f32>> = nums.iter().map(|n| n.parse::<f32>().ok()).collect();
                    if let Some(v) = v {
                        out.push(TransformFn::Matrix([v[0], v[1], v[4], v[5], v[12], v[13]]));
                    }
                }
            }
            _ => {}
        }
        rest = &rest[open + close + 1..];
    }
    out
}

/// ⚠⚠⚠ **A ROTATION ABOUT X OR Y IS A SCALE ON THE OTHER AXIS, AND THIS REPO SAID IT WAS
/// INEXPRESSIBLE.**
///
/// `stylo_map.rs` and `parse_transform` both carried the note *"a rotation about x or y foreshortens,
/// which a 2D pipeline cannot express, and inventing one would be a wrong answer of the right type"*,
/// and dropped every such rotation on the floor. Measured against Chrome (headless, a 100×40 box,
/// `transform-origin: 0 0`), it is not inexpressible — with no `perspective` in force the
/// orthographic projection of the rotation **is exactly** a scale by `cos θ` on the perpendicular
/// axis:
///
/// ```text
///                              Chrome              ours (before)      cos θ x the axis
///   rotateX(45deg)           100 x 28.28          100 x 40           40 x cos45  = 28.28
///   rotateY(45deg)            70.71 x 40          100 x 40          100 x cos45  = 70.71
///   rotate3d(0,1,0,60deg)      50 x 40            100 x 40          100 x cos60  = 50
///   rotateX(90deg)           100 x 0              100 x 40           40 x cos90  = 0
///   rotateX(120deg)          100 x 20  (y = -20)  100 x 40           40 x cos120 = -20
/// ```
///
/// `rotateX(120deg)` is the row that makes it precise: `cos` is NEGATIVE past 90°, the box flips
/// through its origin, and Chrome reports the flipped position — which a `Scale(1, cos θ)` gives for
/// free because the box's rect comes from mapping its corners. A rule written as `abs(cos θ)` would
/// pass every row below 90° and be wrong above it.
///
/// **A rotation about a genuinely mixed axis is still `None`** and that part of the old note stands:
/// `rotate3d(1,1,0,45deg)` measures 91.21 × 48.79, which is not a scale on either axis. The
/// difference is that the exclusion is now the narrow case rather than the whole family.
pub(crate) fn axis_rotation_2d(x: f32, y: f32, z: f32, rad: f32) -> Option<TransformFn> {
    match (x != 0.0, y != 0.0, z != 0.0) {
        (false, false, true) => Some(TransformFn::Rotate(rad)),
        (true, false, false) => Some(TransformFn::Scale(1.0, rad.cos())),
        (false, true, false) => Some(TransformFn::Scale(rad.cos(), 1.0)),
        // Either no axis at all, or two — the genuinely 3D case, which an affine 2D map cannot hold.
        _ => None,
    }
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

/// Parse a `grid-template-columns`/`-rows` track list.
///
/// An integer `repeat(N, …)` is expanded here, because N is literal. An `auto-fill`/`auto-fit`
/// `repeat()` is **kept intact** as a [`TrackComponent::AutoRepeat`] — its count is the largest that
/// fits the container (CSS Grid §7.2.3.1), which the cascade cannot know.
///
/// This replaces a **string** rewrite (`expand_grid_repeat`) that scanned for the first `)` after
/// `repeat(`. For `repeat(auto-fill, minmax(180px,1fr))` that `)` closes `minmax(`, so the rewrite
/// parsed `"auto-fill"` as the count, failed, emitted nothing, and left a stray `)` behind for the
/// track parser to discard. Parsing the nesting instead of pattern-matching the text is what makes
/// `minmax()` inside `repeat()` — i.e. every responsive card grid on the web — survive at all.
/// Line names are still not modeled.
fn parse_track_list(v: &str, fs: f32) -> Vec<TrackComponent> {
    let mut out = Vec::new();
    for tok in split_tracks_top_level(v) {
        let low = tok.to_ascii_lowercase();
        let Some(inner) = low
            .strip_prefix("repeat(")
            .and_then(|s| s.strip_suffix(')'))
        else {
            if let Some(t) = parse_track(&tok, fs) {
                out.push(TrackComponent::Single(t));
            }
            continue;
        };
        // `repeat(<count>, <track-list>)` — the FIRST top-level comma separates them; any comma
        // after it belongs to a `minmax(a, b)` and must not be split on.
        let Some((count, tracks)) = inner.split_once(',') else {
            continue;
        };
        let tracks: Vec<TrackSize> = split_tracks_top_level(tracks)
            .iter()
            .filter_map(|t| parse_track(t, fs))
            .collect();
        if tracks.is_empty() {
            continue;
        }
        match count.trim() {
            "auto-fill" => out.push(TrackComponent::AutoRepeat { fit: false, tracks }),
            "auto-fit" => out.push(TrackComponent::AutoRepeat { fit: true, tracks }),
            n => {
                // A literal count. Bounded: `repeat(100000, 1fr)` is legal CSS and would otherwise
                // let one declaration allocate an unbounded track list — Bar 0 outranks fidelity to
                // a track list no page can see.
                let Ok(n) = n.parse::<usize>() else { continue };
                for _ in 0..n.min(1000) {
                    out.extend(tracks.iter().copied().map(TrackComponent::Single));
                }
            }
        }
    }
    out
}

/// `grid-auto-rows` / `grid-auto-columns` — a plain `<track-size>+` list.
///
/// Deliberately **not** `parse_track_list`: the two grammars differ in one way that matters, and
/// sharing the richer parser would silently accept the difference. `repeat()` is legal in a
/// *template* and forbidden in an *auto* track list, because the auto list has no length of its own
/// — it is **cycled** over however many implicit tracks placement ends up creating. Feeding
/// `repeat(auto-fill, …)` through here would produce a `TrackComponent` this field cannot hold, so
/// the narrower parser is the honest one: anything that is not a `<track-size>` is dropped, which is
/// what "invalid at computed-value time" means for a list-valued property.
fn parse_auto_track_list(v: &str, fs: f32) -> Vec<TrackSize> {
    split_tracks_top_level(v)
        .iter()
        .filter_map(|t| parse_track(t, fs))
        .collect()
}

/// `grid-auto-flow: [ row | column ] || dense`.
///
/// `||` in the CSS grammar means *one or both, in any order*, so `column dense` and `dense column`
/// are the same declaration and `dense` alone is legal (it means `row dense`). Returns `None` for a
/// declaration that names neither, leaving the cascaded value untouched rather than resetting it to
/// the initial value — an invalid declaration is ignored, not applied.
fn parse_grid_auto_flow(v: &str) -> Option<GridAutoFlow> {
    let (mut column, mut dense, mut seen) = (false, false, false);
    for word in v.split_ascii_whitespace() {
        match word.to_ascii_lowercase().as_str() {
            "row" => seen = true,
            "column" => {
                column = true;
                seen = true;
            }
            "dense" => {
                dense = true;
                seen = true;
            }
            _ => return None,
        }
    }
    if !seen {
        return None;
    }
    Some(match (column, dense) {
        (false, false) => GridAutoFlow::Row,
        (false, true) => GridAutoFlow::RowDense,
        (true, false) => GridAutoFlow::Column,
        (true, true) => GridAutoFlow::ColumnDense,
    })
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

/// `@keyframes` sampled to an INTERPOLATED computed value — the engine's first interpolation of
/// anything. See the module doc for what it cost to not have it.
#[cfg(feature = "stylo")]
pub mod animation;

/// A running CSS TRANSITION sampled to an INTERPOLATED computed value. The sibling of
/// [`animation`], and the one that needs a MEMORY: a transition's `from` endpoint is what the last
/// cascade published, and appears in no rule anywhere.
#[cfg(feature = "stylo")]
pub mod transition;

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

    /// # An INVALID declaration is IGNORED — it must not overwrite the valid one before it
    ///
    /// CSS 2.1 §4.2: *"User agents must ignore a declaration with an illegal value."* Every keyword
    /// arm in [`apply_declaration`] was written as `match { "a" => A, "b" => B, _ => Initial }`, so
    /// an unrecognised value was **applied** as the initial value — which is only invisible while
    /// nothing valid came first.
    ///
    /// This matters on the SHIPPING path and not merely here, because Stylo's servo build cannot
    /// express these properties and `stylo_engine` recovers them from this cascade. Measured against
    /// live Chromium, `<span style="display:inline-block">wwwww</span>` at 16px proportional:
    ///
    /// ```text
    ///                               CHROME    before    after
    ///     uppercase; banana          75.52      58        76     <- THE DEFECT
    ///     uppercase                  75.52      76        76     <- we DO apply text-transform
    ///     banana only                57.78      58        58     <- only-invalid IS the initial
    ///     uppercase; none            57.78      58        58     <- a VALID override still wins
    /// ```
    ///
    /// ⚠⚠ **THE LAST TWO ROWS ARE WHAT MAKE THIS A RULE ABOUT *DROPPING*.** A fix that made the
    /// property sticky satisfies row one and breaks row four; one that made an unknown-only value
    /// inherit satisfies row one and breaks row three. Leaving the field untouched is the only shape
    /// that satisfies all four — and it is what "ignore the declaration" literally means.
    ///
    /// ⚠ Each arm therefore had to gain the keyword it was previously falling through to
    /// (`"none" => TextTransform::None`), because that keyword is REAL and must still be honoured.
    /// Dropping the fall-through without adding it would turn `text-transform: none` into a no-op.
    ///
    /// To watch it go RED: restore any `_ => <Initial>` arm — row one reads the initial value again.
    #[test]
    fn an_invalid_declaration_is_dropped_and_does_not_override_the_valid_one_before_it() {
        let one = |decl: &str| {
            let (dom, map) = styled(&format!("span {{ {decl} }}"));
            let n = query_selector_all(&dom, dom.root(), "span")[0];
            map.get(&n).cloned().unwrap_or_else(ComputedStyle::initial)
        };

        // ── text-transform: the row measured against Chrome, with its three controls.
        assert_eq!(
            one("text-transform: uppercase; text-transform: banana").text_transform,
            TextTransform::Uppercase,
            "an INVALID value must be ignored, leaving the valid declaration standing"
        );
        assert_eq!(
            one("text-transform: uppercase").text_transform,
            TextTransform::Uppercase,
            "CONTROL: the property is applied at all"
        );
        assert_eq!(
            one("text-transform: banana").text_transform,
            TextTransform::None,
            "CONTROL: an only-invalid declaration leaves the INITIAL value, not a sticky one"
        );
        assert_eq!(
            one("text-transform: uppercase; text-transform: none").text_transform,
            TextTransform::None,
            "CONTROL: a VALID later declaration still wins — `none` is a real keyword, not a fallback"
        );

        // ── the sibling arms, same shape, same rule. Each carries its own valid-override control so
        // that adding the previously-fallen-through keyword is proven and not assumed.
        assert_eq!(
            one("word-break: break-all; word-break: banana").word_break,
            WordBreak::BreakAll
        );
        assert_eq!(
            one("word-break: break-all; word-break: normal").word_break,
            WordBreak::Normal,
            "CONTROL: `normal` is a real keyword"
        );
        assert_eq!(
            one("overflow-wrap: break-word; overflow-wrap: banana").overflow_wrap,
            OverflowWrap::BreakWord
        );
        assert_eq!(
            one("overflow-wrap: break-word; overflow-wrap: normal").overflow_wrap,
            OverflowWrap::Normal,
            "CONTROL: `normal` is a real keyword"
        );
        assert_eq!(
            one("scroll-snap-align: center; scroll-snap-align: banana").scroll_snap_align,
            ScrollSnapAlign::Center
        );
        assert_eq!(
            one("scroll-snap-align: center; scroll-snap-align: none").scroll_snap_align,
            ScrollSnapAlign::None,
            "CONTROL: `none` is a real keyword"
        );
        assert_eq!(
            one("scroll-snap-type: x mandatory; scroll-snap-type: banana").scroll_snap_type,
            ScrollSnapAxis::X
        );
        assert_eq!(
            one("scroll-snap-type: x mandatory; scroll-snap-type: none").scroll_snap_type,
            ScrollSnapAxis::None,
            "CONTROL: `none` is a real keyword"
        );
        assert_eq!(
            one("scrollbar-width: thin; scrollbar-width: banana").scrollbar_width,
            ScrollbarWidth::Thin
        );
        assert_eq!(
            one("scrollbar-width: thin; scrollbar-width: auto").scrollbar_width,
            ScrollbarWidth::Auto,
            "CONTROL: `auto` is a real keyword"
        );
        assert_eq!(
            one("direction: rtl; direction: banana").direction,
            Direction::Rtl
        );
        assert_eq!(
            one("direction: rtl; direction: ltr").direction,
            Direction::Ltr,
            "CONTROL: `ltr` is a real keyword"
        );
    }

    /// **The four container-level Box-Alignment longhands, on the MINIMAL cascade.**
    ///
    /// Two of them (`align-content`, `justify-items`) did not exist anywhere in the engine until
    /// t981, and the reason they survived is that their axis-twins sat directly beside them and were
    /// right. The four are now parsed by two shared helpers, so this asserts what those helpers
    /// cannot express: that all four call sites exist, that the `place-*` shorthands reach BOTH
    /// halves, and that the ALIGN axis rejects the `left`/`right` keywords the INLINE axis accepts.
    ///
    /// The shipping cascade is Stylo (`live-cascade-is-stylo-not-minimal`) and its half is gated by
    /// `G_CONTAINER_ALIGNMENT`; this covers the JS-less/headless fallback, which has to agree.
    #[test]
    fn the_four_container_alignment_longhands_all_parse() {
        let one = |decl: &str| {
            let (dom, map) = styled(&format!("span {{ {decl} }}"));
            let n = query_selector_all(&dom, dom.root(), "span")[0];
            map.get(&n).cloned().unwrap_or_else(ComputedStyle::initial)
        };

        // Each longhand reaches its OWN field and leaves the other three at their initial value —
        // the property-crosstalk a copy-pasted arm would introduce.
        let s = one("align-content: space-between");
        assert_eq!(s.align_content, JustifyContent::SpaceBetween);
        assert_eq!(s.justify_content, JustifyContent::Normal);
        let s = one("justify-content: space-between");
        assert_eq!(s.justify_content, JustifyContent::SpaceBetween);
        assert_eq!(s.align_content, JustifyContent::Normal);
        // ⚠ The untouched twin reads `Normal`, NOT `Stretch` — that is the CSS initial value, and
        // the two stopped being one variant at t1345 because a replaced grid item aligns as `start`
        // under `normal` and inflates to its cell under an explicit `stretch`.
        let s = one("justify-items: center");
        assert_eq!(s.justify_items, AlignItems::Center);
        assert_eq!(s.align_items, AlignItems::Normal);
        let s = one("align-items: center");
        assert_eq!(s.align_items, AlignItems::Center);
        assert_eq!(s.justify_items, AlignItems::Normal);
        // …and the keyword still parses to its own variant, which is what makes the split testable.
        assert_eq!(one("align-items: stretch").align_items, AlignItems::Stretch);
        assert_eq!(one("align-items: normal").align_items, AlignItems::Normal);

        // `place-*` sets ALIGN first, then JUSTIFY; one token sets both.
        let s = one("place-content: center end");
        assert_eq!(s.align_content, JustifyContent::Center);
        assert_eq!(s.justify_content, JustifyContent::FlexEnd);
        let s = one("place-items: end");
        assert_eq!(s.align_items, AlignItems::FlexEnd);
        assert_eq!(s.justify_items, AlignItems::FlexEnd);

        // `left`/`right` are inline-axis keywords. Legal on `justify-*`; on `align-*` the whole
        // declaration is invalid, so the initial value must stand rather than the parser inventing
        // an end-alignment out of a keyword the axis cannot use.
        assert_eq!(
            one("justify-content: right").justify_content,
            JustifyContent::FlexEnd
        );
        assert_eq!(
            one("align-content: right").align_content,
            JustifyContent::Normal
        );
        assert_eq!(
            one("justify-items: right").justify_items,
            AlignItems::FlexEnd
        );
        assert_eq!(one("align-items: right").align_items, AlignItems::Normal);

        // `normal` and `stretch` both mean "stretch" on these axes and share one representation —
        // folding either into `flex-start` is what disabled CSS Grid §11.8 once already.
        assert_eq!(
            one("align-content: stretch").align_content,
            JustifyContent::Normal
        );
        assert_eq!(
            one("align-content: normal").align_content,
            JustifyContent::Normal
        );
    }

    /// `will-change` / `contain` / `perspective` as containing-block creators, on the minimal
    /// (JS-less) cascade. The Stylo path is gated by `G_TRANSFORM_CONTAINING_BLOCK`; this is the
    /// fallback, and it carries the NEGATIVE half explicitly because that is the half a predicate
    /// written from the property NAME rather than from its VALUES gets wrong.
    #[test]
    fn will_change_contain_and_perspective_create_a_containing_block_only_for_the_right_values() {
        let cb = |decl: &str| {
            let (dom, map) = styled(&format!("span {{ {decl} }}"));
            let n = query_selector_all(&dom, dom.root(), "span")[0];
            map.get(&n)
                .cloned()
                .unwrap_or_else(ComputedStyle::initial)
                .establishes_containing_block
        };

        for decl in [
            "will-change: transform",
            "will-change: filter",
            "will-change: perspective",
            "will-change: backdrop-filter",
            "will-change: top, transform",
            "will-change: TRANSFORM",
            "contain: layout",
            "contain: paint",
            "contain: strict",
            "contain: content",
            "contain: size layout",
            "perspective: 100px",
        ] {
            assert!(cb(decl), "`{decl}` must create a containing block");
        }

        // ⚠ THE NEGATIVE HALF, and every one of these is Chrome-measured rather than reasoned from
        // the grammar. `will-change: opacity` creates a STACKING CONTEXT — a different thing the
        // same property also does — and `contain: style`/`size` are containment of other kinds.
        // A predicate written as "any will-change" or "any contain" passes all twelve rows above
        // and is wrong about all of these.
        for decl in [
            "will-change: opacity",
            "will-change: auto",
            "will-change: scroll-position",
            "will-change: z-index",
            "contain: style",
            "contain: size",
            "contain: none",
            "perspective: none",
        ] {
            assert!(!cb(decl), "`{decl}` must NOT create a containing block");
        }

        // And the base case: nothing declared.
        assert!(!ComputedStyle::initial().establishes_containing_block);
    }

    /// `gap` on the **minimal (JS-less) cascade**, which is where the shorthand's own expansion
    /// lives — under Stylo the shorthand is expanded before we ever see it, so `G_PERCENTAGE_GAP`
    /// structurally cannot test the order of the two halves. This is that gate's missing half, and
    /// it is here rather than there for the reason t976 recorded: a RED proof aimed at the wrong
    /// cascade cannot fire.
    #[test]
    fn gap_carries_percentages_and_the_shorthand_sets_row_first() {
        let one = |decl: &str| {
            let (dom, map) = styled(&format!("span {{ {decl} }}"));
            let n = query_selector_all(&dom, dom.root(), "span")[0];
            map.get(&n).cloned().unwrap_or_else(ComputedStyle::initial)
        };

        // A PERCENTAGE survives the cascade instead of collapsing to 0 — the whole point of the
        // `f32 → Dim` widening, since only layout knows the basis.
        assert_eq!(one("column-gap: 10%").column_gap, Dim::Percent(10.0));
        assert_eq!(one("row-gap: 25%").row_gap, Dim::Percent(25.0));
        // ...and a plain length still lands, which a percentage-only fix would break.
        assert_eq!(one("column-gap: 30px").column_gap, Dim::Px(30.0));

        // Each longhand reaches its OWN axis and leaves the other at the initial value.
        assert_eq!(one("column-gap: 10%").row_gap, Dim::Px(0.0));
        assert_eq!(one("row-gap: 10%").column_gap, Dim::Px(0.0));

        // ⚠ `gap: <row> <column>` — BLOCK axis first, which is the opposite order to most
        // two-value shorthands people reach for by analogy (`margin` is top/right). Two DIFFERENT
        // percentages, so a swap cannot hide.
        let s = one("gap: 10% 20%");
        assert_eq!(s.row_gap, Dim::Percent(10.0));
        assert_eq!(s.column_gap, Dim::Percent(20.0));
        // One token sets both.
        let s = one("gap: 12px");
        assert_eq!((s.row_gap, s.column_gap), (Dim::Px(12.0), Dim::Px(12.0)));

        // The initial value is zero on both axes and must stay so through the type change.
        let s = ComputedStyle::initial();
        assert_eq!((s.row_gap, s.column_gap), (Dim::Px(0.0), Dim::Px(0.0)));
    }

    /// The three **implicit-track** properties on the minimal (JS-less) cascade. The gate
    /// `g_grid_implicit_tracks` proves the Stylo path; this proves the fallback, and the two
    /// grammar facts that a shared parser would have flattened.
    #[test]
    fn grid_implicit_track_properties_parse_on_the_minimal_cascade() {
        let one = |decl: &str| {
            let (dom, map) = styled(&format!("span {{ {decl} }}"));
            let n = query_selector_all(&dom, dom.root(), "span")[0];
            map.get(&n).cloned().unwrap_or_else(ComputedStyle::initial)
        };

        // `[ row | column ] || dense` — both orders, and `dense` alone meaning `row dense`.
        assert_eq!(one("grid-auto-flow: row").grid_auto_flow, GridAutoFlow::Row);
        assert_eq!(
            one("grid-auto-flow: column").grid_auto_flow,
            GridAutoFlow::Column
        );
        assert_eq!(
            one("grid-auto-flow: column dense").grid_auto_flow,
            GridAutoFlow::ColumnDense
        );
        assert_eq!(
            one("grid-auto-flow: dense column").grid_auto_flow,
            GridAutoFlow::ColumnDense
        );
        assert_eq!(
            one("grid-auto-flow: dense").grid_auto_flow,
            GridAutoFlow::RowDense
        );
        // An unrecognised keyword invalidates the DECLARATION, which is ignored — the cascaded
        // value stands. It must not reset to the initial value, which is what returning
        // `GridAutoFlow::Row` on a parse failure would do.
        assert_eq!(
            one("grid-auto-flow: column; grid-auto-flow: sideways").grid_auto_flow,
            GridAutoFlow::Column
        );

        // The auto track list is a `<track-size>+` that CYCLES; it holds several values and the
        // count is meaningful, so the length is asserted as well as the contents.
        assert_eq!(
            one("grid-auto-rows: 80px 20px").grid_auto_rows,
            vec![TrackSize::Px(80.0), TrackSize::Px(20.0)]
        );
        assert_eq!(
            one("grid-auto-columns: minmax(10px, 1fr)").grid_auto_columns,
            vec![TrackSize::MinMax(TrackUnit::Px(10.0), TrackUnit::Fr(1.0))]
        );
        // Each reaches its OWN axis and leaves the other empty — the crosstalk a copied arm brings.
        assert!(one("grid-auto-rows: 80px").grid_auto_columns.is_empty());
        assert!(one("grid-auto-columns: 80px").grid_auto_rows.is_empty());

        // ⚠ `repeat()` is legal in `grid-template-*` and FORBIDDEN in an auto track list — the list
        // has no length of its own, it is cycled. This is the one grammar difference that made the
        // two properties get two parsers instead of sharing the richer one.
        assert!(one("grid-auto-rows: repeat(auto-fill, 80px)")
            .grid_auto_rows
            .is_empty());
        assert_eq!(
            one("grid-template-rows: repeat(auto-fill, 80px)")
                .grid_template_rows
                .len(),
            1,
            "the TEMPLATE parser must still accept the repeat the AUTO parser rejects"
        );
    }

    /// **A real-site crash (tick 380 oracle run: netlify.com).** An at-rule whose name holds
    /// multi-byte UTF-8 — `@media` written in CJK, an emoji custom at-rule, any hostile bytes —
    /// hit `rest[..6]` / `rest[..9]` / `rest[..10]` prefix slices guarded only by BYTE length, so
    /// the slice landed mid-character and panicked the whole engine. A browser must never panic
    /// on bytes the network hands it: unknown at-rules are SKIPPED, whatever they are named.
    /// The three strings place a slice index inside a 3-byte char (bytes 6 and 9) and a 4-byte
    /// char (byte 10) so every guarded prefix length is crossed mid-character at least once.
    #[test]
    fn multibyte_at_rule_names_never_panic() {
        for css in [
            "@媒体查询 { .a { color: red } }", // 3-byte chars: bytes 6 and 9 mid-char
            "@🦀🦀🦀 { }",                     // 4-byte chars: byte 10 mid-char
            "@é;",                             // 2-byte char, statement form
        ] {
            let sheet = Stylesheet::parse(css);
            assert!(
                sheet.rules.is_empty(),
                "unknown at-rule must be skipped, not styled"
            );
        }
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
        assert_eq!(s.border_color, Sides::all(Rgba::new(0x33, 0x33, 0x33, 255)));
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

    /// **A border has FOUR colours and FOUR styles** (t1079).
    ///
    /// This lives here rather than only in `G_BORDER_SIDES` because the page gate cannot see it:
    /// the shipping cascade is Stylo, which owns border colour and width outright, so a
    /// MinimalCascade regression in either is invisible through `Page::load`. MinimalCascade is
    /// still the cascade the layout batteries run on, and it is where the two shorthands and the
    /// four longhands are parsed.
    #[test]
    fn border_colour_and_style_are_per_side() {
        // The 1-to-4-value box-side shorthand: two values are block axis, then inline axis.
        let (dom, map) = cascade_of(r#"<p style="border-color:red blue"></p>"#);
        let s = &map[&dom.find_first("p").unwrap()];
        assert_eq!(s.border_color.top, Rgba::new(255, 0, 0, 255));
        assert_eq!(s.border_color.bottom, Rgba::new(255, 0, 0, 255));
        assert_eq!(
            s.border_color.left,
            Rgba::new(0, 0, 255, 255),
            "`border-color: red blue` used to collapse to its FIRST token"
        );
        assert_eq!(s.border_color.right, Rgba::new(0, 0, 255, 255));

        // The four longhands — which had no arm at all before t1079.
        let (dom, map) = cascade_of(
            r#"<p style="border-top-color:red;border-right-color:lime;border-bottom-color:blue;border-left-color:black"></p>"#,
        );
        let s = &map[&dom.find_first("p").unwrap()];
        assert_eq!(s.border_color.top, Rgba::new(255, 0, 0, 255));
        assert_eq!(s.border_color.right, Rgba::new(0, 255, 0, 255));
        assert_eq!(s.border_color.bottom, Rgba::new(0, 0, 255, 255));
        assert_eq!(s.border_color.left, Rgba::new(0, 0, 0, 255));

        // The per-side shorthand must not repaint the other three sides.
        let (dom, map) =
            cascade_of(r#"<p style="border:1px solid red;border-bottom:1px dashed blue"></p>"#);
        let s = &map[&dom.find_first("p").unwrap()];
        assert_eq!(s.border_color.top, Rgba::new(255, 0, 0, 255));
        assert_eq!(s.border_color.bottom, Rgba::new(0, 0, 255, 255));
        assert_eq!(s.border_style.top, BorderStyle::Solid);
        assert_eq!(s.border_style.bottom, BorderStyle::Dashed);

        // ⚠ `border-<side>-style: none` zeroes THAT side's width and no other. It used to zero all
        // four, so `border: 1px solid; border-right-style: none` deleted the whole border — the
        // shape every segmented control and button group is built from.
        let (dom, map) = cascade_of(r#"<p style="border:10px solid;border-right-style:none"></p>"#);
        let s = &map[&dom.find_first("p").unwrap()];
        assert_eq!(s.border_width.right, 0.0);
        assert_eq!(s.border_width.top, 10.0, "the other three edges SURVIVE it");
        assert_eq!(s.border_width.bottom, 10.0);
        assert_eq!(s.border_width.left, 10.0);

        // …and the 1-to-4 form of `border-style` reaches all four sides, `none` included.
        let (dom, map) =
            cascade_of(r#"<p style="border:10px solid;border-style:dashed none"></p>"#);
        let s = &map[&dom.find_first("p").unwrap()];
        assert_eq!(s.border_style.top, BorderStyle::Dashed);
        assert_eq!(s.border_width.left, 0.0);
        assert_eq!(s.border_width.top, 10.0);
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

    /// # G_HAS_LINEAR — `:has()` was QUADRATIC, and one WPT test is named after exactly that
    ///
    /// `css/selectors/invalidation/has-complexity.html` — *":has() invalidation should not be
    /// O(n^2)"* — builds 75,000 elements under one `<main>` and asserts the page still responds.
    /// The runner reported it as **`CRASH (killed by a signal)`**: the watchdog killing a page that
    /// had stopped responding. **That is Bar 0**, and it was found by refreshing
    /// `docs/loop/WPT-AREAS.tsv`, which had been frozen since Jul 16 — the primary metric's own
    /// source hiding a crash for a month.
    ///
    /// **The mechanism.** The cascade visits every node, and `main:has(span) span` sends every one
    /// of those spans up to the single `<main>` to re-run the subtree search. The work is
    /// `nodes × subtree`, and the ladder is unambiguous — each doubling of `n` cost **4×**:
    ///
    /// ```text
    ///      n      250    500   1000   2000   4000      75000 (the test)
    ///     BEFORE   41    133    551   2074   8176 ms   ~48 MINUTES, extrapolated
    ///     AFTER     8      9     20     36     68 ms   (linear: 8000->137, 16000->291, 25000->452)
    /// ```
    ///
    /// **The answer is that the `:has()` question is asked of the ANCHOR, and the anchor is asked
    /// the same question over and over.** `main:has(span)` has ONE answer for `<main>`; it was being
    /// recomputed once per span. Memoising `(this exact :has() pseudo, this node) -> bool` for the
    /// duration of one cascade collapses `nodes × subtree` to `subtree`.
    ///
    /// ⚠⚠ **THE GATE IS A COUNTER, NOT A STOPWATCH.** A timing assertion on a shared box is a flake,
    /// and this loop has been burned by machine-dependent numbers before. *"How many times did the
    /// expensive thing actually run"* is the quantity the fix is about, and it is exact: the count of
    /// uncached `:has()` branch searches must NOT grow with the number of spans.
    ///
    /// ⚠⚠⚠ **THE BAR 0 IS NOT CLOSED, AND SAYING OTHERWISE WOULD BE THE EASY LIE.** The WPT test
    /// still crashes after this fix, because a SECOND quadratic dominates it and it is in a different
    /// subsystem: `Page::relayout` *"recascades only when the node count outgrew the style map"*
    /// (`engine/page/src/lib.rs:6167`), so each of the test's 75,000 `appendChild` calls triggers a
    /// FULL re-cascade — `appends × nodes`. This fix makes each of those cascades linear instead of
    /// quadratic; it does not make there be fewer of them. Incremental style invalidation is the
    /// mechanism that closes it, and it is a different tick.
    ///
    /// ⚠ **THE MEMO IS SCOPED, NOT AMBIENT, AND THAT IS THE WHOLE OF ITS SAFETY.** It exists only
    /// inside a [`HasMemoScope`], which a cascade pass opens over a DOM it does not mutate and closes
    /// on drop. With no scope open there is no cache and every call computes — so a caller that
    /// mutates between queries (`querySelectorAll` from script) cannot read a stale answer, because
    /// it never had one. Both cascade implementations open one in this tick (`MinimalCascade::
    /// cascade_scoped` and `stylo_engine`'s `:has()` loop) — the *one rule, N implementations* trap
    /// this repo has paid for at t720, t1027, t1131 and t1134, avoided by fixing both at once.
    ///
    /// ⚠⚠⚠ **THE FIRST DRAFT OF THIS GATE WAS BLIND TO ITS OWN SUBJECT AND REPORTED THE BUG FIXED
    /// WITH THE FIX REMOVED.** It carried only `main:has(...) .subject` rules — and the rule index
    /// buckets by the RIGHTMOST compound, so exactly ONE element (`.subject`) ever asked the
    /// question. Three evaluations either way, memo or no memo. The rule that creates the quadratic
    /// is the one whose SUBJECT is the repeated element — `main:has(span) span`, which sends all
    /// 2,000 spans up to the same anchor. It is in the fixture for that reason and must not be
    /// removed to "simplify" it.
    ///
    /// To watch it go RED: delete the `HasMemoScope::new()` line in `cascade_scoped`.
    #[test]
    fn has_is_evaluated_once_per_anchor_not_once_per_element() {
        const CSS: &str = r#"
div, main { color: grey }
main:has(span) .subject { color: red }
main:has(span + span) .subject { color: green }
main:has(div div span) .subject { color: purple }
main:has(span) span { color: black }
"#;
        let run = |n: usize| -> (u64, usize, Option<String>) {
            let spans = "<span></span>".repeat(n);
            let html = format!(
                "<main><div id=container>{spans}</div><div id=subject class=subject></div></main>"
            );
            let dom = manuk_html::parse(&html);
            let sheets = vec![Stylesheet::parse(CSS)];
            reset_has_evaluations();
            let map = MinimalCascade.cascade(&dom, &sheets);
            // The SUBJECT's colour is the correctness half — a memo that returns the wrong answer
            // would be a much worse bug than the one it fixes, and a pure speed assertion cannot see
            // it. `main:has(span + span)` wins over `main:has(span)` on source order.
            let subject = dom
                .descendants(dom.root())
                .find(|&x| dom.element(x).and_then(|e| e.attr("id")) == Some("subject"));
            let color = subject
                .and_then(|x| map.get(&x))
                .map(|s| format!("{:?}", s.color));
            (has_evaluations(), map.len(), color)
        };
        let (e_small, n_small, c_small) = run(50);
        let (e_large, n_large, c_large) = run(2000);

        // CORRECTNESS FIRST — the memo must not change a single answer, at either size.
        assert_eq!(
            c_small, c_large,
            "the subject's colour must not depend on how many spans are present"
        );
        assert!(
            c_small.is_some(),
            "the subject must be styled at all (the fixture must be able to express its subject)"
        );
        assert_eq!(
            n_small, 56,
            "50 spans + main + container + subject + root-ish"
        );
        assert_eq!(n_large, 2006);

        // THE SUBJECT — evaluations must not grow with the element count. 40× the spans must not
        // cost 40× the searches; before the memo it cost MORE than 40× (the searches themselves got
        // longer too, which is the second factor of the quadratic).
        assert!(
            e_large <= e_small * 2,
            "`:has()` must be evaluated per ANCHOR, not per element: {e_small} evaluations at 50 \
             spans but {e_large} at 2000. A memo that is not open makes this ratio ~40x."
        );
        // ...and the absolute count is small, so the assertion above cannot be satisfied by both
        // numbers being huge.
        assert!(
            e_large < 200,
            "3 `:has()` rules over a handful of anchors is a handful of searches, not {e_large}"
        );
    }

    /// **AN XHTML `<style><![CDATA[ … ]]></style>` SHEET WAS DROPPED IN ITS ENTIRETY.**
    ///
    /// Every rule, not just the first — which is what separates this from ordinary CSS error
    /// recovery, and what makes the page render completely unstyled rather than slightly wrong.
    /// It is the standard XHTML idiom and **2,191 of the CSS 2.1 conformance suite's 10,501 files
    /// use it**, tests and references alike; Chrome applies all three rules of the fixture below.
    #[test]
    fn a_cdata_wrapped_stylesheet_is_parsed_in_full() {
        let wrapped = Stylesheet::parse(
            "<![CDATA[\n#a { width: 100px }\n#b { width: 70px }\n#c { width: 40px }\n]]>",
        );
        let plain =
            Stylesheet::parse("#a { width: 100px }\n#b { width: 70px }\n#c { width: 40px }");
        // ⚠ THREE rules, not one: CSS error recovery would drop only the first and keep the rest,
        // so a one-rule fixture cannot tell "the wrapper is stripped" from "the parser recovered".
        assert_eq!(
            wrapped.rules.len(),
            3,
            "a CDATA-wrapped sheet must parse every rule, got {}",
            wrapped.rules.len()
        );
        assert_eq!(
            wrapped.rules.len(),
            plain.rules.len(),
            "wrapped and unwrapped must agree"
        );
        // ── NEGATIVE 1: a sheet WITHOUT the wrapper is untouched (the common case).
        assert_eq!(Stylesheet::parse("#a{color:red}").rules.len(), 1);
        // ── NEGATIVE 2: `]]>` is only a terminator at the END. A sheet whose content contains one
        //    inside a string must not be truncated at it.
        let inner = Stylesheet::parse(
            "<![CDATA[\n#a { content: \"]]>\" }\n#b { width: 1px }\n#c { width: 2px }\n]]>",
        );
        assert_eq!(
            inner.rules.len(),
            3,
            "an inner `]]>` must not truncate the sheet, got {}",
            inner.rules.len()
        );
        // ── NEGATIVE 3: an UNTERMINATED wrapper still yields its rules — dropping the opener alone
        //    recovers everything, which is strictly better than dropping the sheet.
        assert_eq!(
            Stylesheet::parse("<![CDATA[\n#a { width: 1px }\n#b { width: 2px }")
                .rules
                .len(),
            2
        );
    }

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
        assert_eq!(
            s.before.as_ref().unwrap().content.as_deref(),
            Some(&[ContentPart::Text("[X] ".into())][..]),
            "a counter-free `content` is ONE Text part — the list type exists for counters, and a \
             value without one must not fragment"
        );
    }

    /// **`@import` URLs must be extracted, because an unfetched import drops a whole stylesheet.**
    ///
    /// Measured at t563/t564: `martinfowler.com`'s `home.css` `@import`s Open Sans, Inconsolata and Lora
    /// from Google Fonts. Chromium resolved `{Lora/13}`; we fell back to `{serif/13}`, and that one
    /// substitution made a `<p>` **293px wide in Chromium and 619px in ours**. The font was the visible
    /// symptom; the unfetched sheet was the cause.
    #[test]
    fn imports_are_extracted_in_every_authored_form() {
        let sheet = Stylesheet::parse(
            r#"
            @import url(https://fonts.googleapis.com/css?family=Lora:400,400i,700,700i);
            @import url("partials/tokens.css");
            @import 'partials/layout.css';
            @import url(print.css) print;
            @charset "utf-8";
            body { color: red }
            "#,
        );
        let got = sheet.imports();
        assert_eq!(
            got,
            vec![
                "https://fonts.googleapis.com/css?family=Lora:400,400i,700,700i".to_string(),
                "partials/tokens.css".to_string(),
                "partials/layout.css".to_string(),
                "print.css".to_string(),
            ],
            "all four authored forms, in source order, and NOT the @charset — a media list after \
             `url(...)` must not swallow the URL, and a conditional import still needs FETCHING (the \
             enclosing @media decides application, not delivery)"
        );
        // A sheet with no imports yields none, and ordinary rules still parse alongside them.
        assert!(Stylesheet::parse("body { color: red }")
            .imports()
            .is_empty());
        assert!(
            !sheet.rules.is_empty(),
            "the imports must not eat the rest of the sheet"
        );
    }

    /// **ONE HUNDRED `@font-face` RULES FOR ONE FAMILY, AND `unicode-range` TOLD THEM APART.**
    ///
    /// The inlined Google-Fonts block is the commonest webfont delivery on the web and it is
    /// **subsetted by codepoint**. `www.kuechenmomente.de` ships 170 `@font-face` rules, **100 of
    /// them named `Raleway`** — weights {400,700} × styles {normal,italic} × ~13 subsets — with the
    /// **Cyrillic and Vietnamese blocks FIRST in source order** and Latin further down. The
    /// descriptor was not parsed and had nowhere to live, so all hundred were fetched and registered
    /// under one name, into a family list `FontContext::face_id` searches by weight and style alone.
    /// A Cyrillic subset and the Latin subset are indistinguishable to that search.
    ///
    /// Measured (t1153, the face-advance probe, one fixed string in the element's own resolved font):
    ///
    /// ```text
    ///                          declared          CHROME   OURS
    ///   kuechenmomente.de      Raleway/18          166     240     +45%
    ///   jatekshop.eu           fira_sansbook/14    129     140     +8.5%
    ///   lyreco.com             Lyreco Renner/18    174     184     +5.7%
    ///   ──────────────────────────────────────────────────────────────
    ///   kuechenmomente.de      -apple-system/10    102     102      0    <- CONTROL, both fall back
    /// ```
    ///
    /// Every text box on those pages was that much too wide, which re-wraps prose, changes the line
    /// count, and arrives downstream as `dy` — where it was scored as *shape*, for hundreds of ticks.
    ///
    /// ⚠ **The wildcard row is not decoration.** `U+4??` is a RANGE (`U+400-4FF`), and reading it as
    /// a literal would make the commonest short form of the descriptor unparseable — which, per the
    /// invalid-value rule below, would silently restore the old behaviour for exactly the faces that
    /// use it.
    ///
    /// ⚠⚠ **AN UNPARSEABLE COMPONENT INVALIDATES THE WHOLE DESCRIPTOR, AND THAT DIRECTION IS THE
    /// SAFE ONE.** `None` means *"all codepoints"* at the call site, so a descriptor we cannot read
    /// makes the face a candidate rather than excluding it. Dropping just the bad component would
    /// NARROW a face's coverage on a guess and could hide the one face a page needs — the exact
    /// failure this field exists to prevent.
    #[test]
    fn a_unicode_range_subset_is_parsed_including_its_wildcard_form() {
        let ff = |css: &str| parse_font_face_block(css).expect("@font-face");

        // The real Google-Fonts Cyrillic subset, byte-for-byte off kuechenmomente.de.
        let cyr = ff(
            "font-family:'Raleway'; font-style:italic; font-weight:400; \
             src: url(x.woff2) format('woff2'); \
             unicode-range: U+0460-052F, U+1C80-1C8A, U+20B4, U+2DE0-2DFF, U+A640-A69F, U+FE2E-FE2F;",
        );
        let r = cyr.unicode_range.expect("the Cyrillic subset has a range");
        assert_eq!(r.len(), 6, "six components, one of them a bare codepoint");
        assert!(r.contains(&(0x0460, 0x052F)), "an explicit range");
        assert!(
            r.contains(&(0x20B4, 0x20B4)),
            "a BARE codepoint is a 1-wide range"
        );
        let covers = |r: &[(u32, u32)], c: char| {
            r.iter()
                .any(|&(lo, hi)| (c as u32) >= lo && (c as u32) <= hi)
        };
        assert!(
            !covers(&r, 'a'),
            "the Cyrillic-EXT subset must NOT claim Latin 'a'"
        );
        // ⚠ U+0450 is NOT in this subset and an earlier draft of this gate asserted it was: these
        // ranges are Cyrillic **Extended**, and basic Cyrillic ships as its own `U+0400-045F` block.
        // The mistake is left recorded because it is the same one the ENGINE makes — treating a
        // family's subsets as interchangeable — one level up, in the test.
        assert!(
            !covers(&r, 'ѐ'),
            "U+0450 is basic Cyrillic and lives in a DIFFERENT subset"
        );
        assert!(
            covers(&r, '\u{0460}'),
            "…and U+0460 is the first codepoint this one does claim"
        );

        // The wildcard form, which is a RANGE and not a literal.
        let w = ff("font-family:W; src:url(x.woff2); unicode-range: U+4??;")
            .unicode_range
            .expect("a wildcard range");
        assert_eq!(w, vec![(0x400, 0x4FF)], "U+4?? is U+400-4FF");

        // A latin subset covers the text these pages are actually made of.
        let lat =
            ff("font-family:L; src:url(x.woff2); unicode-range: U+0000-00FF, U+0131, U+2000-206F;")
                .unicode_range
                .expect("a latin range");
        assert!(covers(&lat, 'a') && covers(&lat, 'ü'), "latin-1 is covered");
        assert!(!covers(&lat, 'ѐ'), "…and Cyrillic is not");

        // ── ABSENCE and INVALIDITY both mean "all codepoints", and they must not be confused with
        // an EMPTY range, which would mean "no codepoints" and would hide the face forever.
        assert_eq!(
            ff("font-family:N; src:url(x.woff2);").unicode_range,
            None,
            "no descriptor at all = the spec's default U+0-10FFFF, expressed as absence"
        );
        assert_eq!(
            ff("font-family:B; src:url(x.woff2); unicode-range: U+0-FF, notahex;").unicode_range,
            None,
            "one bad component invalidates the WHOLE descriptor — the face stays a candidate"
        );
        assert_eq!(
            ff("font-family:R; src:url(x.woff2); unicode-range: U+FF-00;").unicode_range,
            None,
            "a reversed range is invalid, not an empty set"
        );
    }

    /// **A `data:` URI contains a SEMICOLON, and the declaration splitter cut every one in half.**
    ///
    /// `src: url(data:font/ttf;base64,AAAA) format("truetype")` split into `src: url(data:font/ttf`
    /// and `base64,AAAA) format("truetype")`. The first has an unterminated `url(`, so
    /// `parse_font_face_block` finds no source and **drops the whole `@font-face`** — and face
    /// harvesting runs through this parser whichever engine computes the styles, so the failure is
    /// live on the shipping (Stylo) path.
    ///
    /// Chrome-measured on a `file://` fixture, one 147KB TrueType face declared three ways, used as
    /// `font-family: <face>, monospace` so a failure falls back visibly:
    ///
    /// ```text
    ///                                          chrome    before   after
    ///   src: url(go.ttf)                CTRL    126.56     127      127
    ///   src: url(data:font/ttf;base64,…)        126.56     145      127
    ///   font-family: monospace          CTRL    144.5      145      145
    /// ```
    ///
    /// ⚠ **The `url(go.ttf)` control is what names the organ.** An ordinary web font has always
    /// loaded, so a probe whose only web font was a `data:` URI would have concluded that web fonts
    /// do not work at all — the map row was `partial` and that is very nearly what it says.
    ///
    /// Priced on the corpus: a `data:` payload in an `@font-face` `src` on **17 of the 166 pages
    /// using `@font-face` (10%)**, and a `;`-bearing `data:` URI inside some CSS `url()` on **89 of
    /// 761 files**.
    ///
    /// RED, run: restore `text.split(';')` in `parse_declarations` — the `data:` face yields no
    /// `srcs` and the whole rule is dropped.
    #[test]
    fn a_semicolon_inside_a_url_does_not_split_the_declaration() {
        // The subject: a `data:` URI's `;base64,` must not end the declaration.
        let ff = parse_font_face_block(
            r#"font-family:"Probe"; src: url(data:font/ttf;base64,AAECAwQ=) format("truetype");"#,
        )
        .expect("the @font-face must survive its own data: URI");
        assert_eq!(ff.family, "probe");
        assert_eq!(
            ff.srcs,
            vec!["data:font/ttf;base64,AAECAwQ=".to_string()],
            "the src is the WHOLE data URI, semicolon and all"
        );

        // ── CONTROL 1: an ordinary URL was always fine and must stay byte-identical.
        let plain =
            parse_font_face_block(r#"font-family:"P"; src: url(go.ttf) format("truetype");"#)
                .expect("plain url");
        assert_eq!(plain.srcs, vec!["go.ttf".to_string()]);

        // ── CONTROL 2: a real `;` between declarations still separates them. This is the over-fix
        //    boundary — a splitter that stopped honouring `;` entirely would pass every row above.
        let d = parse_declarations("color: red; background: blue url(a.png); margin: 0");
        assert_eq!(d.len(), 3, "three declarations, got {d:?}");
        assert_eq!(d[0].name, "color");
        assert_eq!(d[1].value, "blue url(a.png)");
        assert_eq!(d[2].name, "margin");

        // ── CONTROL 3: a `;` inside a QUOTED string is not a separator either.
        let q = parse_declarations(r#"content: "a;b"; color: red"#);
        assert_eq!(q.len(), 2, "a quoted semicolon must not split, got {q:?}");
        assert_eq!(q[0].value, r#""a;b""#);

        // ── CONTROL 4: `!important` and trailing/empty chunks still behave.
        let i = parse_declarations("color: red !important;;");
        assert_eq!(i.len(), 1);
        assert!(i[0].important);
    }
}

/// **Serialize one `font-family` name the way CSSOM does: as a SEQUENCE OF IDENTIFIERS when it can
/// be, as a STRING when it cannot.**
///
/// The computed `font-family` was `cs.font_family.join(", ")` — the names, bare, never quoted, so a
/// name that genuinely needs quotes (`21st Century`) came back as something that would re-parse as
/// something else.
///
/// ⚠⚠⚠ **AND THE RULE IS A SEQUENCE, NOT A SINGLE IDENTIFIER — CHROME AND THE SPEC DISAGREE HERE,
/// AND THIS FOLLOWS THE SPEC.** The first implementation quoted anything that was not ONE identifier,
/// which is what headless Chrome does (`font-family: Times New Roman` → `"Times New Roman"`). It cost
/// **three net subtests**, because WPT encodes the CSSOM rule instead:
///
/// ```text
///   css/css-fonts/parsing/font-family-computed.html
///     '"New Century Schoolbook", serif'  →  New Century Schoolbook, serif     UNQUOTED
///     '"21st Century", fantasy'          →  "21st Century", fantasy           quoted
/// ```
///
/// `New Century Schoolbook` is three valid identifiers, so it serializes as three identifiers even
/// though it was WRITTEN as a string; `21st Century` cannot, because `21st` starts with a digit.
/// The quoting in the SOURCE is not the question — the question is whether the NAME can be spelled
/// as identifiers.
///
/// ⚠ This is one of the few places this engine deliberately does NOT match measured Chrome. Chrome
/// quotes every multi-word family; the spec, and WPT, do not. It is a pure serialization detail with
/// no capability behind it, so the ratchet's metric wins over the oracle — and it is written down
/// here so the next reader who probes Chrome and finds a "divergence" does not "fix" it back.
///
/// ⚠ Non-ASCII is an ordinary identifier character (`素象` is unquoted), which is why this tests
/// `>= U+0080` rather than an ASCII allow-list. A CSS-WIDE KEYWORD (`inherit`, `initial`, …) stays
/// quoted whatever it looks like: unquoted it would not be a family name at all.
///
/// ⚠⚠ **Stylo already does this for the INLINE path** (`el.style.fontFamily` round-trips exactly,
/// measured) — this exists because the COMPUTED path never went through Stylo's serializer, not
/// because the rule was missing from the engine.
pub fn serialize_font_family_name(name: &str) -> String {
    let is_start = |c: char| c.is_ascii_alphabetic() || c == '_' || c >= '\u{80}';
    let is_body = |c: char| is_start(c) || c.is_ascii_digit() || c == '-';
    let is_ident = |part: &str| {
        let mut chars = part.chars();
        match chars.next() {
            None => false,
            Some(c0) => {
                let head_ok = if c0 == '-' {
                    matches!(chars.clone().next(), Some(c1) if is_start(c1) || c1 == '-')
                } else {
                    is_start(c0)
                };
                head_ok && part.chars().all(is_body)
            }
        }
    };
    // A CSS-wide keyword can never appear unquoted as a family name — it would be the keyword.
    let is_css_wide = matches!(
        name.to_ascii_lowercase().as_str(),
        "inherit" | "initial" | "unset" | "revert" | "revert-layer" | "default"
    );
    // Every space-separated part must be an identifier.
    //
    // ⚠ That single condition also settles the spacing, and the redundant belt-and-braces check
    // that was here first could not be made to go RED: `"a  b".split(' ')` yields `["a", "", "b"]`
    // and the EMPTY part is not an identifier, so a doubled space is rejected by this line alone.
    // The same for a leading or trailing space. An extra `join(" ") == name` guard looked like
    // defence and was an unfalsifiable branch — a green that cannot go red measured nothing.
    let seq_ok = !name.is_empty() && !is_css_wide && name.split(' ').all(is_ident);
    if seq_ok {
        return name.to_string();
    }
    // A CSS string: `"` and `\` are the only characters that must be escaped.
    let escaped: String = name
        .chars()
        .flat_map(|c| match c {
            '"' | '\\' => vec!['\\', c],
            other => vec![other],
        })
        .collect();
    format!("\"{escaped}\"")
}

/// The whole computed `font-family` list, each name serialized by
/// [`serialize_font_family_name`], comma-separated as CSSOM asks.
pub fn serialize_font_family_list(names: &[String]) -> String {
    names
        .iter()
        .map(|n| serialize_font_family_name(n))
        .collect::<Vec<_>>()
        .join(", ")
}

/// **One `appearance` keyword → what this engine will actually DO, or `None` if the value is
/// invalid and the declaration must be dropped.**
///
/// CSS UI 4 splits the keywords three ways and this collapses two of them:
///
/// * `none` → [`Appearance::None`];
/// * `auto`, `<compat-special>` (`textfield`, `menulist-button`) and `<compat-auto>` (`button`,
///   `checkbox`, `listbox`, `menulist`, `meter`, `progress-bar`, `radio`, `searchfield`,
///   `textarea`) → [`Appearance::Auto`], because this engine draws one native control or none —
///   which is what WPT's `appearance-cssom-001` accepts for the compat set (`[value, "auto"]`);
/// * anything else — and the list of legacy `-moz-`/`-webkit-` widget names is long — is INVALID,
///   and an invalid declaration is dropped rather than coerced.
///
/// ⚠ The `<compat-special>` pair is the honest cost of the collapse: WPT expects those two to
/// compute to THEMSELVES, and they compute to `auto` here. Two values, named rather than faked.
pub fn appearance_from_keyword(v: &str) -> Option<Appearance> {
    let k = v.trim().to_ascii_lowercase();
    match k.as_str() {
        "none" => Some(Appearance::None),
        "auto" | "textfield" | "menulist-button" | "button" | "checkbox" | "listbox"
        | "menulist" | "meter" | "progress-bar" | "radio" | "searchfield" | "textarea" => {
            Some(Appearance::Auto)
        }
        _ => None,
    }
}

/// The elements this engine draws a NATIVE CONTROL for — the UA sheet's `appearance: auto`.
///
/// ⚠ Chrome-measured: `<button>`, `<input>` (every type probed), `<select>` and `<textarea>` all
/// compute `auto`; a `<div>` computes `none`. That asymmetry is the whole reason this is a
/// tag-keyed default and not a global initial value.
pub fn tag_has_native_appearance(tag: &str) -> bool {
    matches!(
        tag,
        "button" | "input" | "select" | "textarea" | "meter" | "progress"
    )
}
