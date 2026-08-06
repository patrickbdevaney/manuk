//! Blitz-model unified Taffy tree (research #7).
//!
//! `solve_flex`/`solve_grid` build a throwaway [`taffy::TaffyTree`] per flex/grid container
//! and measure children back into block/inline via a closure. That is correct but a
//! *directly*-nested flex/grid re-solves in its own fresh tree. This module implements
//! taffy's low-level trait API ([`LayoutPartialTree`] et al.) over Manuk's arena DOM so a
//! flex/grid container **and its directly-nested flex/grid descendants share one tree +
//! cache**. Block / inline / float / table nodes stay Manuk-measured leaves (taffy can't do
//! those, and they carry the WPT parity gate), sized through [`compute_leaf_layout`].
//!
//! This is the `ComputedStyle → taffy::Style` mapping — the shared foundation. The tree
//! wrapper, trait impls, and geometry extraction build on it in this module.

use manuk_css::{
    AlignItems as CssAlign, BoxSizing, ComputedStyle, Dim, Direction as CssDirection,
    Display as CssDisplay, FlexDirection as CssDir, FlexWrap as CssWrap,
    GridAutoFlow as CssGridAutoFlow, GridLine as CssGridLine, IntrinsicSize,
    JustifyContent as CssJustify, Position as CssPosition, TrackComponent as CssTrackComponent,
    TrackSize as CssTrackSize, TrackUnit,
};
use taffy::prelude::*;
use taffy::style::{
    BoxSizing as TaffyBoxSizing, CheapCloneStr, Dimension, GridTemplateRepetition,
    LengthPercentage, LengthPercentageAuto, Position as TaffyPosition, RepetitionCount,
};

/// Register a mixed `calc(px + pct%)` into the tree's calc table and encode its index as the
/// opaque `*const ()` handle taffy round-trips back to [`TaffyDom::resolve_calc_value`].
///
/// taffy requires the handle to be non-null and 8-byte aligned (low 3 bits = 0), so the index
/// is stored as `(idx + 1) << 3`. It is an *index*, not a real address — the `Vec` may realloc
/// freely without invalidating it. The `+1` keeps index 0 off the forbidden null value.
fn reg_calc(calc: &mut Vec<(f32, f32)>, px: f32, pct: f32) -> *const () {
    let idx = calc.len();
    calc.push((px, pct));
    (((idx + 1) << 3) as usize) as *const ()
}

/// Split a `calc(px + pct%)` into taffy's length / percent fast paths, or a genuine calc handle
/// when BOTH terms are present. A single-term calc (`calc(50%)`, `calc(20px)`) needs no handle.
/// Only the mixed case (`calc(100% - 250px)`) requires taffy's calc plumbing, which resolves the
/// two terms against the definite basis at layout time — the same `px + pct% · basis` the block
/// path already does via [`Dim::resolve`].
macro_rules! dim_impl {
    ($ty:ty, $auto:expr) => {
        |d: Dim, calc: &mut Vec<(f32, f32)>| -> $ty {
            match d {
                Dim::Auto => $auto,
                Dim::Px(p) => length(p),
                Dim::Percent(p) => percent(p / 100.0),
                Dim::Calc { px, pct } => {
                    if px != 0.0 && pct != 0.0 {
                        <$ty>::calc(reg_calc(calc, px, pct))
                    } else if px != 0.0 {
                        length(px)
                    } else {
                        percent(pct / 100.0)
                    }
                }
            }
        }
    };
}

/// `Dim` → taffy `Dimension` (`auto` / length / percentage / calc), sizes + flex-basis.
fn dimension(d: Dim, calc: &mut Vec<(f32, f32)>) -> Dimension {
    dim_impl!(Dimension, auto())(d, calc)
}

/// `Dim` → taffy `LengthPercentageAuto` (margins / insets).
fn lp_auto(d: Dim, calc: &mut Vec<(f32, f32)>) -> LengthPercentageAuto {
    dim_impl!(LengthPercentageAuto, auto())(d, calc)
}

/// `Dim` → taffy `LengthPercentage` (padding; `auto` is invalid → 0).
fn lp(d: Dim, calc: &mut Vec<(f32, f32)>) -> LengthPercentage {
    dim_impl!(LengthPercentage, length(0.0))(d, calc)
}

fn map_display(d: CssDisplay) -> Display {
    match d {
        // Inline-level flex/grid boxes run the SAME formatting algorithm; they differ only in how
        // their parent sizes them (shrink-to-fit), which `layout_block` handles.
        CssDisplay::Flex | CssDisplay::InlineFlex => Display::Flex,
        CssDisplay::Grid | CssDisplay::InlineGrid => Display::Grid,
        CssDisplay::None => Display::None,
        // Everything else is a Manuk-measured leaf; taffy treats it as a block-level box.
        _ => Display::Block,
    }
}

fn map_position(p: CssPosition) -> TaffyPosition {
    match p {
        CssPosition::Absolute | CssPosition::Fixed => TaffyPosition::Absolute,
        _ => TaffyPosition::Relative,
    }
}

/// **`row` is a LOGICAL direction, and taffy only speaks physical.** The flex main axis for `row` runs
/// along the *inline* axis, which under `direction: rtl` points RIGHT-TO-LEFT (CSS Flexbox §5.1) — so an
/// RTL flex row starts at the container's right edge and `justify-content: flex-start` packs to the
/// right. Taffy has no `direction` property at all, so the mapping has to carry it: RTL swaps
/// `row` ⇄ `row-reverse`, which produces exactly that geometry.
///
/// Measured against live Chromium (`<html dir=rtl>`, a 600px flex row of three 100px items, x relative
/// to the container): Chrome **500 / 400 / 300**, ours was **0 / 100 / 200** — every RTL nav bar, toolbar
/// and card row ran backwards, which is the `reading_order` invariant firing on `mobile.ir` (874) and
/// `ta3lemkonline.com` (817).
///
/// `column`/`column-reverse` are unchanged: their main axis is the BLOCK axis, which `direction` does not
/// flip. (RTL does flip a column's *cross*-axis start edge, which taffy cannot express — recorded in
/// `CONSTELLATION.tsv` rather than approximated here.)
fn map_direction(d: CssDir, rtl: bool) -> FlexDirection {
    match (d, rtl) {
        (CssDir::Row, false) | (CssDir::RowReverse, true) => FlexDirection::Row,
        (CssDir::RowReverse, false) | (CssDir::Row, true) => FlexDirection::RowReverse,
        (CssDir::Column, _) => FlexDirection::Column,
        (CssDir::ColumnReverse, _) => FlexDirection::ColumnReverse,
    }
}

fn map_wrap(w: CssWrap) -> FlexWrap {
    match w {
        CssWrap::NoWrap => FlexWrap::NoWrap,
        CssWrap::Wrap => FlexWrap::Wrap,
        CssWrap::WrapReverse => FlexWrap::WrapReverse,
    }
}

/// `None` is not "unset" to taffy — it is the CSS initial value `normal`, and taffy resolves it
/// PER FORMATTING CONTEXT exactly as the spec asks: flexbox falls back to `FLEX_START`, grid falls
/// back to `STRETCH` (which is what gates CSS Grid §11.8 "Stretch auto Tracks"). Handing taffy a
/// concrete `FLEX_START` instead — which is what we did while the CSS enum had no `Normal` — is
/// therefore not a harmless normalisation: it silently disables free-space distribution for every
/// `auto` grid track, on every grid, whether or not the author wrote `justify-content` at all.
fn map_justify(j: CssJustify) -> Option<JustifyContent> {
    Some(match j {
        CssJustify::Normal => return None,
        CssJustify::FlexStart => JustifyContent::FLEX_START,
        CssJustify::FlexEnd => JustifyContent::FLEX_END,
        CssJustify::Center => JustifyContent::CENTER,
        CssJustify::SpaceBetween => JustifyContent::SPACE_BETWEEN,
        CssJustify::SpaceAround => JustifyContent::SPACE_AROUND,
        CssJustify::SpaceEvenly => JustifyContent::SPACE_EVENLY,
    })
}

fn map_align(a: CssAlign) -> AlignItems {
    match a {
        CssAlign::Stretch => AlignItems::STRETCH,
        CssAlign::FlexStart => AlignItems::FLEX_START,
        CssAlign::FlexEnd => AlignItems::FLEX_END,
        CssAlign::Center => AlignItems::CENTER,
        CssAlign::Baseline => AlignItems::BASELINE,
    }
}

fn track_min(u: TrackUnit) -> MinTrackSizingFunction {
    match u {
        TrackUnit::Px(p) => length(p),
        TrackUnit::Percent(p) => percent(p / 100.0),
        TrackUnit::Auto | TrackUnit::Fr(_) => auto(),
        TrackUnit::MinContent => min_content(),
        TrackUnit::MaxContent => max_content(),
    }
}

fn track_max(u: TrackUnit) -> MaxTrackSizingFunction {
    match u {
        TrackUnit::Px(p) => length(p),
        TrackUnit::Percent(p) => percent(p / 100.0),
        TrackUnit::Fr(f) => fr(f),
        TrackUnit::Auto => auto(),
        TrackUnit::MinContent => min_content(),
        TrackUnit::MaxContent => max_content(),
    }
}

fn track(t: &CssTrackSize) -> TrackSizingFunction {
    match t {
        CssTrackSize::Px(p) => length(*p),
        CssTrackSize::Fr(f) => fr(*f),
        CssTrackSize::Percent(p) => percent(*p / 100.0),
        CssTrackSize::Auto => auto(),
        CssTrackSize::MinContent => min_content(),
        CssTrackSize::MaxContent => max_content(),
        CssTrackSize::MinMax(lo, hi) => minmax(track_min(*lo), track_max(*hi)),
    }
}

/// One `grid-template-columns`/`-rows` component → taffy.
///
/// The `AutoRepeat` arm is the point: taffy computes the repetition count itself (CSS Grid §7.2.3.1
/// — the largest N whose tracks plus gutters fit the container's inline size), and `AutoFit`
/// additionally collapses the repetitions that end up empty. Both are decisions that need the
/// container's resolved size, so both belong here and not in either cascade. We carry no line names,
/// so the `line_names` vector is empty rather than absent — taffy tolerates a short list.
///
/// Generic over taffy's custom-ident string because `Style`'s own parameter is defaulted and its
/// concrete type (`taffy::util::sys::DefaultCheapStr`) is crate-private; inference resolves it at
/// the call site rather than us hard-coding a `String` that a taffy bump could change.
fn template_component<S: CheapCloneStr>(c: &CssTrackComponent) -> GridTemplateComponent<S> {
    match c {
        CssTrackComponent::Single(t) => GridTemplateComponent::Single(track(t)),
        CssTrackComponent::AutoRepeat { fit, tracks } => {
            GridTemplateComponent::Repeat(GridTemplateRepetition {
                count: if *fit {
                    RepetitionCount::AutoFit
                } else {
                    RepetitionCount::AutoFill
                },
                tracks: tracks.iter().map(track).collect(),
                line_names: Vec::new(),
            })
        }
    }
}

fn grid_line(pair: (CssGridLine, CssGridLine)) -> Line<GridPlacement> {
    fn one(l: CssGridLine) -> GridPlacement {
        match l {
            CssGridLine::Auto => GridPlacement::Auto,
            CssGridLine::Line(n) => line(n as i16),
            CssGridLine::Span(n) => span(n),
        }
    }
    Line {
        start: one(pair.0),
        end: one(pair.1),
    }
}

/// Map a Manuk [`ComputedStyle`] onto a taffy [`Style`], covering the box model + flex + grid
/// properties taffy needs to lay out a flex/grid container and its items. Inline/float/table
/// specifics stay with Manuk (this node is a leaf to taffy in those cases).
pub fn to_taffy_style(cs: &ComputedStyle, calc: &mut Vec<(f32, f32)>) -> Style {
    Style {
        display: map_display(cs.display),
        box_sizing: if cs.box_sizing == BoxSizing::BorderBox {
            TaffyBoxSizing::BorderBox
        } else {
            TaffyBoxSizing::ContentBox
        },
        position: map_position(cs.position),
        inset: Rect {
            left: lp_auto(cs.inset.left, calc),
            right: lp_auto(cs.inset.right, calc),
            top: lp_auto(cs.inset.top, calc),
            bottom: lp_auto(cs.inset.bottom, calc),
        },
        size: Size {
            width: dimension(cs.width, calc),
            height: dimension(cs.height, calc),
        },
        min_size: Size {
            width: dimension(cs.min_width, calc),
            height: dimension(cs.min_height, calc),
        },
        max_size: Size {
            width: dimension(cs.max_width, calc),
            height: dimension(cs.max_height, calc),
        },
        margin: Rect {
            left: lp_auto(cs.margin.left, calc),
            right: lp_auto(cs.margin.right, calc),
            top: lp_auto(cs.margin.top, calc),
            bottom: lp_auto(cs.margin.bottom, calc),
        },
        padding: Rect {
            left: lp(cs.padding.left, calc),
            right: lp(cs.padding.right, calc),
            top: lp(cs.padding.top, calc),
            bottom: lp(cs.padding.bottom, calc),
        },
        border: Rect {
            left: length(cs.border_width.left),
            right: length(cs.border_width.right),
            top: length(cs.border_width.top),
            bottom: length(cs.border_width.bottom),
        },
        align_items: Some(map_align(cs.align_items)),
        // `justify-items` — the container-level default for a grid item's INLINE-axis alignment.
        // The block-axis twin above has been here since flex landed; this line had no partner, so a
        // grid declaring `justify-items: end` put every item at the start of its track (Chrome
        // x=140 in a 200px track, ours x=0) — the same absence t980 found one level down in
        // `justify-self`, one level up.
        justify_items: Some(map_align(cs.justify_items)),
        align_self: cs.align_self.map(map_align),
        justify_self: cs.justify_self.map(map_align),
        justify_content: map_justify(cs.justify_content),
        // `align-content` — cross-axis distribution of a wrapped flex container's LINES and of a
        // grid's ROWS. `map_justify` is shared with the line above (taffy's `AlignContent` and
        // `JustifyContent` are the same type), and `Normal` deliberately maps to `None` so taffy's
        // own default — stretch on this axis — stands, which is why the initial value was right the
        // whole time and every declared one was wrong.
        align_content: map_justify(cs.align_content),
        gap: Size {
            width: length(cs.column_gap),
            height: length(cs.row_gap),
        },
        flex_direction: map_direction(cs.flex_direction, cs.direction == CssDirection::Rtl),
        flex_wrap: map_wrap(cs.flex_wrap),
        flex_grow: cs.flex_grow,
        flex_shrink: cs.flex_shrink,
        flex_basis: dimension(cs.flex_basis, calc),
        grid_template_columns: cs
            .grid_template_columns
            .iter()
            .map(template_component)
            .collect(),
        grid_template_rows: cs
            .grid_template_rows
            .iter()
            .map(template_component)
            .collect(),
        // The IMPLICIT tracks, and the axis auto-placement advances along. `grid_template_*` above
        // has been mapped since grid landed; these three fields exist on taffy's `Style` and nothing
        // ever wrote them, so a grid with more items than its template holds put every overflow item
        // in a new ROW of CONTENT height — even when the author said `grid-auto-flow: column` or
        // sized the implicit tracks. Empty lists are `auto`, which is both CSS's initial value and
        // taffy's default, so an undeclared grid is unchanged.
        grid_auto_rows: cs.grid_auto_rows.iter().map(track).collect(),
        grid_auto_columns: cs.grid_auto_columns.iter().map(track).collect(),
        grid_auto_flow: match cs.grid_auto_flow {
            CssGridAutoFlow::Row => GridAutoFlow::Row,
            CssGridAutoFlow::Column => GridAutoFlow::Column,
            CssGridAutoFlow::RowDense => GridAutoFlow::RowDense,
            CssGridAutoFlow::ColumnDense => GridAutoFlow::ColumnDense,
        },
        grid_column: grid_line(cs.grid_column),
        grid_row: grid_line(cs.grid_row),
        // **The intrinsic ratio has to cross into taffy, or it does not exist inside flex and grid.**
        // The block path derives an `auto` axis from the other one through `cs.aspect_ratio`, but a
        // flex or grid item's size is taffy's to decide, and taffy was never told the ratio — so an
        // image given only a `height` came out `0` wide: present in the tree, laid out, invisible.
        // This is the same value the block path uses (an `aspect-ratio` declaration, or the natural
        // ratio of a decoded image / the `width`+`height` attribute pair), so all three formatting
        // contexts now transfer a definite axis through the ratio the same way.
        aspect_ratio: cs.aspect_ratio,
        ..Default::default()
    }
}

use crate::flex::Slot;
use manuk_css::StyleMap;
use manuk_dom::{Dom, NodeId as DomNodeId};
use taffy::{
    compute_cached_layout, compute_flexbox_layout, compute_grid_layout, compute_leaf_layout,
    compute_root_layout, Cache, CacheTree, Layout, LayoutFlexboxContainer, LayoutGridContainer,
    LayoutInput, LayoutOutput, LayoutPartialTree, NodeId as TId, RoundTree, TraversePartialTree,
    TraverseTree,
};

/// A callback that content-measures a Manuk-leaf DOM node (block/inline/table/float) for
/// the taffy tree — `(dom_node, known_dims, available_space) -> size`.
type MeasureFn<'m> =
    dyn FnMut(DomNodeId, Size<Option<f32>>, Size<AvailableSpace>) -> Size<f32> + 'm;

/// A node placed by the unified taffy tree: its DOM node, its taffy-assigned rectangle
/// (`slot`, relative to its parent's border box), whether it is a flex/grid **container**
/// (its `children` were positioned by taffy in this same tree — extract them directly, no
/// re-solve) or a Manuk-measured **leaf** (`children` empty — lay its content out via block/
/// inline at the assigned rect).
pub struct Placed {
    pub dom: DomNodeId,
    pub slot: Slot,
    pub container: bool,
    pub children: Vec<Placed>,
}

struct TNode {
    dom: DomNodeId,
    style: Style,
    children: Vec<TId>,
    cache: Cache,
    layout: Layout,
    /// Flex/grid container (taffy lays out its children) vs. Manuk-measured leaf.
    container: bool,
}

/// A unified taffy tree spanning one flex/grid container and its directly-nested flex/grid
/// descendants. Block/inline/float/table children are leaves measured back into Manuk.
pub struct TaffyDom<'m> {
    nodes: Vec<TNode>,
    measure: Box<MeasureFn<'m>>,
    /// Mixed `calc(px + pct%)` terms, indexed by the handle encoded into each calc `Dimension`.
    /// taffy hands the handle back to [`Self::resolve_calc_value`] with the definite basis.
    calc: Vec<(f32, f32)>,
}

impl<'m> TaffyDom<'m> {
    /// Build the tree for `container` (a flex/grid DOM node) and its subtree, mapping styles
    /// and recursing through nested flex/grid. Returns the tree and the container's taffy id.
    fn build(
        dom: &Dom,
        styles: &StyleMap,
        container: DomNodeId,
        measure: Box<MeasureFn<'m>>,
    ) -> (Self, TId) {
        let mut tree = TaffyDom {
            nodes: Vec::new(),
            measure,
            calc: Vec::new(),
        };
        let root = tree.add(dom, styles, container);
        // The container's own margin/padding/border/inset are applied by Manuk's block
        // layout around this subtree; the tree just positions children from the content
        // origin, so zero them on the root and pin it in flow.
        let r: usize = root.into();
        tree.nodes[r].style.margin = Rect::zero();
        tree.nodes[r].style.padding = Rect::zero();
        tree.nodes[r].style.border = Rect::zero();
        tree.nodes[r].style.inset = Rect::auto();
        tree.nodes[r].style.position = taffy::style::Position::Relative;
        (tree, root)
    }

    /// **An intrinsic sizing keyword is UNREPRESENTABLE in taffy, so it has to be RESOLVED to px
    /// before the style is built — otherwise it is invisible inside flex and grid.**
    ///
    /// `to_taffy_style` maps `cs.width` through `dimension()`, and a `min-content` / `max-content`
    /// width is stored as `Dim::Auto` plus a keyword sidecar (`width_keyword`, and since t930
    /// `min_width_keyword` / `max_width_keyword`). The sidecar never crossed into taffy, so every
    /// one of them silently became `Dimension::Auto` — *"size me from my flex basis"*, which is a
    /// different, valid answer. t930 fixed this on the block path and left the flex/grid half open;
    /// measured against Chrome, the gap was wider than the note recorded — plain `width:min-content`
    /// is wrong on a flex item too, and on a grid item, not just the four min/max properties.
    ///
    /// ```text
    ///   "hello there world", 16px serif: min-content 37.33 · max-content 109.30 · 400px container
    ///                                                  Chrome   before   after
    ///     flex item  width:min-content                  37.33      109      37
    ///     flex item  max-width:min-content              37.33      109      37
    ///     flex item  min-width:max-content   (20px CB) 109.30       37     109
    ///     flex item  flex:1; max-width:min-content      37.33      400      37
    ///     flex item  flex:1; max-width:max-content     109.30      400     109
    ///     grid item  width:min-content                  37.33      400      37
    ///     grid item  max-width:min-content              37.33      400      37
    /// ```
    ///
    /// Taffy 0.12 *can* hold a `CompactLength::min_content()`, but its `Dimension` validates as
    /// `LENGTH | PERCENT | AUTO` only, so handing one to the flexbox algorithm is a tag it never
    /// reads — a dependency asked a question it does not answer. Resolving to px instead uses the
    /// measure callback that is already threaded through this tree for exactly this purpose, and it
    /// is the SAME question the block path asks (`min_content_width` / `max_content_width` both
    /// bottom out in `measure_intrinsic`), so the two formatting contexts cannot drift apart.
    ///
    /// ⚠ **`fit-content` is deliberately LEFT as `Dimension::Auto`, and that is a measurement, not
    /// an omission.** `fit-content` is `min(max-content, max(min-content, stretch-fit))`, and the
    /// stretch-fit inside a flex line is not known at style-build time. Taffy's `auto` + `flex-basis:
    /// auto` + `flex-shrink` *is* that clamp: measured Chrome-exact in a wide container (109.30) and
    /// in a narrow one (37.33), on `width`, `min-width` and `max-width` alike. Resolving it here
    /// would replace a correct answer with a guess.
    ///
    /// ⚠ **`box-sizing` has NO effect on an intrinsic keyword** — measured, because the grammar
    /// invites the opposite assumption. With `padding: 0 10px`, Chrome gives a **57.33** border box
    /// under `content-box` AND under `border-box`. Taffy subtracts the frame from `size` under
    /// border-box, so the frame is added back there to land on the same border box either way.
    ///
    /// ⚠ **NOT DONE, named with its number: a flex/grid item that is ITSELF a flex/grid CONTAINER.**
    /// `display:flex; width:min-content` nested in a flex row measures **109.30** against Chrome's
    /// 37.33. Resolving it here would re-enter: the measure callback answers a container's intrinsic
    /// width by building a *second* `TaffyDom` for that node, whose `add` would reach this function
    /// again on the same node and recurse without bound — a Bar-0 crash, not a wrong number. It
    /// needs a root-suppression flag on the nested build, which is a different mechanism; the
    /// `container` guard at the call site is what keeps the recursion profile unchanged.
    fn resolve_intrinsic_inline(&mut self, cs: &ComputedStyle, node: DomNodeId, style: &mut Style) {
        if cs.width_keyword.is_none()
            && cs.min_width_keyword.is_none()
            && cs.max_width_keyword.is_none()
        {
            return;
        }
        // Percentage padding resolves against a containing-block width that does not exist during an
        // intrinsic measure; `px_right_insets` on the block path treats it as 0 for the same reason.
        let frame = if cs.box_sizing == BoxSizing::BorderBox {
            cs.padding.left.resolve(0.0, 0.0).max(0.0)
                + cs.padding.right.resolve(0.0, 0.0).max(0.0)
                + cs.border_width.left.max(0.0)
                + cs.border_width.right.max(0.0)
        } else {
            0.0
        };
        let measure = &mut self.measure;
        let mut probe = |width: AvailableSpace| -> f32 {
            measure(
                node,
                Size {
                    width: None,
                    height: None,
                },
                Size {
                    width,
                    height: AvailableSpace::MaxContent,
                },
            )
            .width
                + frame
        };
        let mut px = |k: IntrinsicSize| -> Option<f32> {
            let width = match k {
                IntrinsicSize::MinContent => AvailableSpace::MinContent,
                IntrinsicSize::MaxContent => AvailableSpace::MaxContent,
                // ⚠⚠⚠ **`fit-content` CANNOT BE RESOLVED TO A LENGTH HERE, AND THAT IS WHY THIS ARM
                // USED TO GIVE UP.** Its definition is
                //
                //     fit-content = min(max-content, max(min-content, stretch))
                //
                // and the `stretch` term is *the space the formatting context is about to hand this
                // box* — a number this function does not have and cannot ask for without re-entering
                // the measure it is inside. So it is not resolved here; it is expressed below as the
                // BOUNDS it is defined by, with taffy's own `auto` supplying the middle term. The
                // `None` stays because there is no length to return, not because there is nothing
                // to do.
                IntrinsicSize::FitContent => return None,
            };
            Some(probe(width))
        };
        if let Some(v) = cs.width_keyword.and_then(&mut px) {
            style.size.width = length(v);
        }
        if let Some(v) = cs.min_width_keyword.and_then(&mut px) {
            style.min_size.width = length(v);
        }
        if let Some(v) = cs.max_width_keyword.and_then(&mut px) {
            style.max_size.width = length(v);
        }

        // ── `width: fit-content` — the shrink-to-fit idiom, and the one keyword the taffy path
        //    dropped while the block path handled it in six places.
        //
        // Leaving `size.width` at `auto` IS the `stretch` term of the definition: taffy offers the
        // box its available space, and clamping that between the two content bounds yields exactly
        // `clamp(min-content, available, max-content)` — which is `fit-content`, evaluated by the
        // one participant that knows the available width.
        //
        // Measured: `<div style="width:fit-content">abc</div>` in a 200px grid track is **29px** in
        // Chrome and was **200** here — the box filled its track, which is the opposite of what the
        // declaration asks for. `width:fit-content` is how a modern page sizes a badge, a pill, a
        // tooltip or a button to its label.
        if cs.width_keyword == Some(IntrinsicSize::FitContent) {
            let min_c = probe(AvailableSpace::MinContent);
            let max_c = probe(AvailableSpace::MaxContent);
            // Compose with an author `min-width`/`max-width` rather than overwriting it: an explicit
            // bound is a separate constraint that still applies on top of the computed width. Only a
            // definite LENGTH can be compared here — a percentage bound resolves against a
            // containing block this function does not know, so it is left alone rather than guessed
            // at (which would replace a right-in-the-common-case answer with a wrong one).
            let as_len = |d: Dimension| -> Option<f32> {
                let raw = d.into_raw();
                (raw.tag() == taffy::style::CompactLength::LENGTH_TAG).then(|| raw.value())
            };
            let author_min = as_len(style.min_size.width);
            let author_max = as_len(style.max_size.width);
            // ⚠⚠ **ORDER, AND IT IS THE ONE THING A ONE-ROW FIXTURE GETS WRONG.** The `min-content`
            // term lives INSIDE `fit-content`; `max-width` clamps the RESULT. Taffy resolves
            // min-over-max, so pushing `min-content` in as a floor and the author's `max-width` in
            // as a ceiling makes the floor outrank the ceiling — measured, `width:fit-content;
            // max-width:20px` around a 29px word read **29** against Chrome's **20**. The synthetic
            // floor is therefore clamped by the ceiling first, and only the author's own `min-width`
            // is allowed to win over it afterwards, which is the CSS 2.1 §10.4 order.
            let ceiling = match author_max {
                Some(a) => max_c.min(a),
                None => max_c,
            };
            let floor = min_c.min(ceiling);
            let floor = match author_min {
                Some(a) => floor.max(a),
                None => floor,
            };
            style.max_size.width = length(ceiling);
            style.min_size.width = length(floor);
        }
    }

    fn add(&mut self, dom: &Dom, styles: &StyleMap, node: DomNodeId) -> TId {
        let cs = &styles[&node];
        let mut style = to_taffy_style(cs, &mut self.calc);
        let mut container = matches!(
            cs.display,
            CssDisplay::Flex | CssDisplay::Grid | CssDisplay::InlineFlex | CssDisplay::InlineGrid
        );
        // An ANONYMOUS flex/grid item (a bare text run — see `flex_items`) takes its inherited
        // properties from the parent and every OTHER property at its initial value. We cannot read
        // that off the stored style, because THE TWO CASCADES DISAGREE about what a text node
        // holds: `MinimalCascade` stores `inherit_from(parent)` (non-inherited props already
        // initial), while the Stylo path stores a full CLONE of the parent's computed style. Under
        // Stylo the clone carries `display:flex` — so the anonymous item would be treated as a flex
        // CONTAINER, recurse into a text node's (empty) child list, and collapse to a zero box,
        // which is the original bug wearing a different hat. It would also inherit the parent's
        // width, padding and margin and apply them a second time.
        //
        // So synthesise the contract here instead of trusting either cascade. Cheap, and it cannot
        // drift when the two cascades next diverge.
        if !dom.is_element(node) {
            container = false;
            style = Style {
                display: Display::Block,
                ..Style::DEFAULT
            };
        }
        if !container && dom.is_element(node) {
            self.resolve_intrinsic_inline(cs, node, &mut style);
        }
        let children: Vec<TId> = if container {
            // The FLAT tree, exactly as the block path does — a shadow host that is also a flex or
            // grid container must lay out its shadow content, not its light children.
            // `display: none` is not "lay it out and give it no room" — it means the element and its
            // subtree **generate no boxes at all**. Adding them to the tree anyway let taffy hand them a
            // zero slot while our extraction still measured and materialised their content: a `<script>`
            // inside a flex `<body>` painted its own source code down the page.
            //
            // And `display: contents` is the mirror image: the wrapper generates no box, **but its
            // children do — as items of THIS container.** A grid whose items are wrapped in a
            // `display: contents` div (which is the entire reason the property exists, and what every
            // component framework emits) must see the three children, not one wrapper. Otherwise the grid
            // gets a single item and collapses into one cell, with everything present and in the wrong
            // place.
            flex_items(dom, styles, node, 0)
                .into_iter()
                .map(|c| self.add(dom, styles, c))
                .collect()
        } else {
            Vec::new()
        };
        // grid-template-areas: resolve each child's `grid-area: name` against this
        // container's named rects into explicit line placement. Our taffy path exposes no
        // ASCII-art areas API, so we pre-resolve names to lines here (the container has the
        // rects; the child carries the area name).
        if container && !cs.grid_template_areas.is_empty() {
            for &child in &children {
                let cdom = self.nodes[usize::from(child)].dom;
                if let Some(name) = styles[&cdom].grid_area.clone() {
                    if let Some(r) = cs.grid_template_areas.iter().find(|a| a.name == name) {
                        let n = &mut self.nodes[usize::from(child)];
                        n.style.grid_row = Line {
                            start: line(r.row.0 as i16),
                            end: line(r.row.1 as i16),
                        };
                        n.style.grid_column = Line {
                            start: line(r.col.0 as i16),
                            end: line(r.col.1 as i16),
                        };
                    }
                }
            }
        }
        let id = self.nodes.len();
        self.nodes.push(TNode {
            dom: node,
            style,
            children,
            cache: Cache::new(),
            layout: Layout::new(),
            container,
        });
        TId::from(id)
    }

    /// Recursively snapshot the placed geometry of `tid` and its subtree from the computed
    /// tree (each node's taffy `layout`), so callers can extract the whole positioned
    /// flex/grid subtree without re-solving nested containers.
    fn placed(&self, tid: TId) -> Placed {
        let n = &self.nodes[usize::from(tid)];
        let l = n.layout;
        Placed {
            dom: n.dom,
            slot: Slot {
                x: l.location.x,
                y: l.location.y,
                width: l.size.width,
                height: l.size.height,
            },
            container: n.container,
            children: n.children.iter().map(|&c| self.placed(c)).collect(),
        }
    }

    fn dispatch(&mut self, node_id: TId, inputs: LayoutInput) -> LayoutOutput {
        let idx: usize = node_id.into();
        if self.nodes[idx].container {
            match self.nodes[idx].style.display {
                Display::Grid => compute_grid_layout(self, node_id, inputs),
                _ => compute_flexbox_layout(self, node_id, inputs),
            }
        } else {
            // Manuk-measured leaf: content-size via the callback into block/inline layout.
            let style = self.nodes[idx].style.clone();
            let dom_node = self.nodes[idx].dom;
            let measure = &mut self.measure;
            compute_leaf_layout(
                inputs,
                &style,
                |_, _| 0.0,
                |known, avail| measure(dom_node, known, avail),
            )
        }
    }
}

/// The flex/grid items of `node`, with every `display: contents` wrapper dissolved.
///
/// Recursive, because `contents` inside `contents` is legal and a component tree produces exactly that.
/// Depth-bounded, because a stack overflow in layout is a Bar 0 crash and this is precisely the property
/// a hostile page would nest ten thousand deep.
fn flex_items(
    dom: &Dom,
    styles: &StyleMap,
    node: manuk_dom::NodeId,
    depth: u32,
) -> Vec<manuk_dom::NodeId> {
    if depth > 64 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for c in dom.flat_children(node) {
        if !dom.is_element(c) {
            // **ANONYMOUS FLEX/GRID ITEMS** (Flexbox §4, Grid §6). Text sitting DIRECTLY inside a
            // flex or grid container is not stray content to be skipped — each contiguous run of it
            // is wrapped in an anonymous block-level item. Filtering to elements dropped that text
            // out of layout entirely: `<a style="display:flex">Recent changes</a>` (Wikipedia's
            // whole navigation, and every icon+label button on the modern web) measured 2×2 px, so
            // the container collapsed to its longest WORD and every label wrapped.
            //
            // The text node itself is the item. That works because its computed style is
            // `inherit_from(parent)` — every non-inherited property is already at its initial value,
            // which is exactly the anonymous box's contract — and `map_display` sends `inline`
            // through as a Manuk-measured block leaf.
            //
            // White-space-only runs are NOT wrapped (the spec says so explicitly); otherwise the
            // newline between two flex children would become a third item and take up a slot.
            if matches!(dom.data(c), manuk_dom::NodeData::Text(t) if !t.trim().is_empty())
                && styles.contains_key(&c)
            {
                out.push(c);
            }
            continue;
        }
        match styles.get(&c).map(|s| s.display) {
            None | Some(CssDisplay::None) => {}
            Some(CssDisplay::Contents) => out.extend(flex_items(dom, styles, c, depth + 1)),
            Some(_) => out.push(c),
        }
    }
    // ── **`order` — THE ITEMS ARE LAID OUT IN ORDER-MODIFIED DOCUMENT ORDER** (Flexbox §5.4, Grid
    // §6.3). taffy has no `order` field, so the sort has to happen here, on the way in.
    //
    // Ignoring it is not a missing feature that degrades gracefully — it is a READING-ORDER defect,
    // the jarring dimension this corpus is worst at (14.5% of in-scope sites clean at t786). A
    // responsive layout that writes `order: -1` to pull the image above the copy on mobile, or
    // `order: 2` to send the sidebar after the article on desktop, renders with its blocks in the
    // wrong sequence and every pairwise comparison against Chrome disagrees.
    //
    // ⚠ **STABLE sort, and that is the whole specification of the tie.** Equal `order` — which is
    // every item on most pages, since the initial value is 0 — must keep DOCUMENT order. An unstable
    // sort would shuffle ordinary flex rows for no reason at all, on every page, which is a far worse
    // bug than the one being fixed.
    //
    // The DOM, the accessibility tree and sequential focus are untouched: `order` is visual only, by
    // design, and reordering them here would turn a layout fix into an a11y regression.
    if out
        .iter()
        .any(|n| styles.get(n).is_some_and(|s| s.order != 0))
    {
        out.sort_by_key(|n| styles.get(n).map(|s| s.order).unwrap_or(0));
    }
    out
}

impl TraversePartialTree for TaffyDom<'_> {
    type ChildIter<'a>
        = std::iter::Copied<std::slice::Iter<'a, TId>>
    where
        Self: 'a;
    fn child_ids(&self, node_id: TId) -> Self::ChildIter<'_> {
        self.nodes[usize::from(node_id)].children.iter().copied()
    }
    fn child_count(&self, node_id: TId) -> usize {
        self.nodes[usize::from(node_id)].children.len()
    }
    fn get_child_id(&self, node_id: TId, index: usize) -> TId {
        self.nodes[usize::from(node_id)].children[index]
    }
}
impl TraverseTree for TaffyDom<'_> {}

impl LayoutPartialTree for TaffyDom<'_> {
    type CoreContainerStyle<'a>
        = &'a Style
    where
        Self: 'a;
    type CustomIdent = String;
    fn get_core_container_style(&self, node_id: TId) -> &Style {
        &self.nodes[usize::from(node_id)].style
    }
    /// Resolve a `calc(px + pct%)` handle against the definite `basis` taffy supplies — the same
    /// linear form [`Dim::resolve`] computes on the block path, so flex/grid items agree with
    /// block ones on e.g. `width: calc(100% - 250px)`. The handle is `(idx + 1) << 3`.
    fn resolve_calc_value(&self, val: *const (), basis: f32) -> f32 {
        let idx = ((val as usize) >> 3).wrapping_sub(1);
        match self.calc.get(idx) {
            Some(&(px, pct)) => px + basis * pct / 100.0,
            None => 0.0,
        }
    }
    fn set_unrounded_layout(&mut self, node_id: TId, layout: &Layout) {
        self.nodes[usize::from(node_id)].layout = *layout;
    }
    fn compute_child_layout(&mut self, node_id: TId, inputs: LayoutInput) -> LayoutOutput {
        compute_cached_layout(self, node_id, inputs, |tree, id, inputs| {
            tree.dispatch(id, inputs)
        })
    }
}

impl CacheTree for TaffyDom<'_> {
    fn cache_get(&self, node_id: TId, inputs: &LayoutInput) -> Option<LayoutOutput> {
        self.nodes[usize::from(node_id)].cache.get(inputs)
    }
    fn cache_store(&mut self, node_id: TId, inputs: &LayoutInput, output: LayoutOutput) {
        self.nodes[usize::from(node_id)].cache.store(inputs, output);
    }
    fn cache_clear(&mut self, node_id: TId) {
        self.nodes[usize::from(node_id)].cache.clear();
    }
}

impl RoundTree for TaffyDom<'_> {
    fn get_unrounded_layout(&self, node_id: TId) -> Layout {
        self.nodes[usize::from(node_id)].layout
    }
    fn set_final_layout(&mut self, node_id: TId, layout: &Layout) {
        self.nodes[usize::from(node_id)].layout = *layout;
    }
}

impl LayoutFlexboxContainer for TaffyDom<'_> {
    type FlexboxContainerStyle<'a>
        = &'a Style
    where
        Self: 'a;
    type FlexboxItemStyle<'a>
        = &'a Style
    where
        Self: 'a;
    fn get_flexbox_container_style(&self, node_id: TId) -> &Style {
        &self.nodes[usize::from(node_id)].style
    }
    fn get_flexbox_child_style(&self, child_node_id: TId) -> &Style {
        &self.nodes[usize::from(child_node_id)].style
    }
}

impl LayoutGridContainer for TaffyDom<'_> {
    type GridContainerStyle<'a>
        = &'a Style
    where
        Self: 'a;
    type GridItemStyle<'a>
        = &'a Style
    where
        Self: 'a;
    fn get_grid_container_style(&self, node_id: TId) -> &Style {
        &self.nodes[usize::from(node_id)].style
    }
    fn get_grid_child_style(&self, child_node_id: TId) -> &Style {
        &self.nodes[usize::from(child_node_id)].style
    }
}

/// Chrome's `LayoutUnit` — 1/64 CSS px. **Every length Blink computes is quantised to this
/// grid**, and that quantisation is not a detail: it is what makes `66.66666667% + 33.33333333%`
/// of a 1200px row come to exactly 1200 instead of a hair over.
const LAYOUT_UNIT: f32 = 64.0;

fn snap_to_layout_unit(v: f32) -> f32 {
    (v * LAYOUT_UNIT).round() / LAYOUT_UNIT
}

/// The same grid, rounded **UP** — which is the only direction an *intrinsic* width may be snapped.
///
/// ⚠⚠⚠ **A BOX SIZED TO ITS OWN MAX-CONTENT MUST FIT ITS OWN CONTENT, AND ON A BARE `f32` IT DOES
/// NOT.** max-content is measured by laying the run out at an unbounded width and reading how far it
/// reached; the box is then given exactly that width and the run is laid out *again* against it. The
/// second pass accumulates the same fragment advances in a different order, so the total can land a
/// few thousandths of a pixel **over** the width it produced — and the line breaker, which has no
/// tolerance, takes a break. The box hugs its text one word too tightly, the line count goes up, and
/// the height error cascades down the whole subtree. Measured on `kicktipp.com`: a footer link whose
/// max-content came to `89.520px` and whose own re-layout needed `89.525px`, so a box Chrome renders
/// one line tall came out two.
///
/// Blink does not have this failure mode because a preferred width is a `LayoutUnit` built with
/// `FromFloatCeil` — the quantisation is *outward*, so the box is never smaller than the content it
/// was measured from. One 1/64px of slack, deliberately spent. Same reasoning as
/// [`snap_to_layout_unit`], opposite rounding, and the direction is the whole point.
pub(crate) fn ceil_to_layout_unit(v: f32) -> f32 {
    if v.is_finite() {
        (v * LAYOUT_UNIT).ceil() / LAYOUT_UNIT
    } else {
        v
    }
}

/// ⚠⚠⚠ **A SUB-PIXEL FLOAT EXCESS BREAKS A FLEX LINE, AND BOOTSTRAP'S COLUMNS ARE WRITTEN IN
/// EXACTLY THE PERCENTAGES THAT TRIGGER IT.**
///
/// taffy collects flex items into lines with a bare `>` and no tolerance
/// (`taffy-0.12.1/src/compute/flexbox.rs:930`):
///
/// ```text
/// line_length += child.hypothetical_outer_size.main(constants.dir) + gap_contribution;
/// line_length > main_axis_available_space && idx != 0
/// ```
///
/// `width: 66.66666667%` is not representable in binary. As `f32` it resolves against a 1200px
/// row to `800.00004`, its `33.33333333%` sibling to `400.00002`, and the pair sums to
/// `1200.00006` — **six hundred-thousandths of a pixel over**, which is enough. The second column
/// starts a new line and the two cards stack. Chrome never sees it: Blink quantises each resolved
/// length to `LayoutUnit` (1/64 px) first, so the same pair is exactly `800 + 400 = 1200` and fits.
///
/// Measured against headless Chrome on a 1200px `flex-wrap: wrap` row, `[x y]` of the second item:
///
/// ```text
///   width pair                        Chrome      before      after
///   50% + 50%                        [600  0]    [600  0]   unchanged ✓ (exact in binary)
///   75% + 25%                        [900  0]    [900  0]   unchanged ✓ (exact in binary)
///   66.6667% + 33.3333%              [800  0]    [800  0]   unchanged ✓ (sums UNDER 100)
///   66.66666667% + 33.33333333%      [800  0]    [0   20]   [800  0]  ✗→✓  ← Bootstrap 5
///   66.666667% + 33.333333%          [800  0]    [0   20]   [800  0]  ✗→✓
///   33.33333333% × 3 (3rd item)      [800  0]    [0   20]   [800  0]  ✗→✓
/// ```
///
/// **THE REACH is every Bootstrap grid on the web.** `.col-8`/`.col-4` ship literally as
/// `width: 66.66666667%` / `33.33333333%`, so a two-column Bootstrap 5 row STACKED instead of
/// sitting side by side — and because the cards' own overlays are absolutely positioned, they then
/// landed on top of each other rather than merely flowing wrong.
///
/// ⚠ **BOUND, stated rather than glossed:** this snaps the percentage main-axis widths of the
/// container's DIRECT children, because that is the only place the containing-block width is known
/// before taffy runs. A flex container nested *inside* a flex item has a content width that taffy
/// itself decides, so its own children keep raw `f32` resolution and can still lose the line-break
/// by a sub-pixel. Fixing that needs the quantisation inside taffy's resolver, which is not ours to
/// patch. The gate asserts the direct-child case and records the nested one as unfixed.
fn snap_row_item_percent_widths(tree: &mut TaffyDom, root: TId, container_width: f32) {
    let r: usize = root.into();
    // Line breaking is a MAIN-axis question, so this is only the width axis and only for a `row`
    // container. A `column` container breaks on height, where the container's own main size is
    // usually indefinite and there is no definite base to snap against in the first place.
    if !matches!(tree.nodes[r].style.display, taffy::Display::Flex)
        || !matches!(
            tree.nodes[r].style.flex_direction,
            FlexDirection::Row | FlexDirection::RowReverse
        )
    {
        return;
    }
    for c in tree.nodes[r].children.clone() {
        let i: usize = c.into();
        let raw = tree.nodes[i].style.size.width.into_raw();
        if raw.tag() == taffy::style::CompactLength::PERCENT_TAG {
            let px = snap_to_layout_unit(raw.value() * container_width);
            tree.nodes[i].style.size.width = length(px);
        }
        // ⚠ **`flex-basis` IS THE MAIN SIZE TOO, and leaving it out left the bug half-fixed.**
        // `flex: 0 0 66.666667%` (Bootstrap 4's column, and the `flex-basis` longhand behind it) never
        // touches `width` at all — the hypothetical main size comes from the BASIS. Measured against
        // Chrome on a 1200px row, the pair was the right WIDTHS (800/400) on the WRONG LINES, which is
        // the same defect wearing the one property t817 did not cover.
        let rb = tree.nodes[i].style.flex_basis.into_raw();
        if rb.tag() == taffy::style::CompactLength::PERCENT_TAG {
            let px = snap_to_layout_unit(rb.value() * container_width);
            tree.nodes[i].style.flex_basis = length(px);
        }
    }
}

/// Lay out a flex/grid `container` and its directly-nested flex/grid descendants in one
/// unified taffy tree, measuring block/inline/float/table leaves via `measure`. Returns the
/// container's direct children as [`Placed`] subtrees (positions relative to the content
/// origin) — a container child carries its whole positioned subtree so the caller extracts
/// it directly instead of re-solving — **and the container's own resolved content height**.
///
/// ⚠⚠⚠ **That second return value used to be thrown away, and the caller reconstructed the
/// container's height from the bottom edge of its lowest child.** For flex those two agree, which is
/// why it survived. For a GRID they are different questions: a grid container's block size is the
/// sum of its resolved ROW TRACKS plus the row gaps, and a track has a size whether or not anything
/// fills it. Measured against headless Chrome:
///
/// ```text
///                                                       Chrome   child-extent   tracks
///    grid-template-rows:100px, one 40px item              100         40          100
///    grid-template-rows:20px,  one 40px item               20         40           20
///    grid-template-rows:40px 100px, two 40px items        140         80          140
///    grid-template-rows:40px 70px,  ONE item              110         40          110
/// ```
///
/// The second row is the one that settles the shape of the fix: Chrome's container is **shorter**
/// than its own content — the item overflows a track too small for it — so this cannot be a
/// `max(child_extent, tracks)` that only ever grows. It has to be taffy's answer, and taffy already
/// computed it.
pub fn solve_subtree<'m>(
    dom: &Dom,
    styles: &StyleMap,
    container: DomNodeId,
    container_width: f32,
    container_height: Option<f32>,
    measure: impl FnMut(DomNodeId, Size<Option<f32>>, Size<AvailableSpace>) -> Size<f32> + 'm,
) -> (Vec<Placed>, f32) {
    let (mut tree, root) = TaffyDom::build(dom, styles, container, Box::new(measure));
    // Pin the root to the given content size (Manuk resolved width; height when definite).
    let r: usize = root.into();
    tree.nodes[r].style.size = Size {
        width: length(container_width),
        height: container_height.map(length).unwrap_or(auto()),
    };
    snap_row_item_percent_widths(&mut tree, root, container_width);
    // **An INDEFINITE main size is INFINITE available main space, never zero.** For a `column`
    // flex container the block axis is the MAIN axis, and the available main space is what
    // `flex-wrap: wrap` breaks lines against (CSS Flexbox §9.3.5). Passing `MinContent` there says
    // *"be as short as you can"*, so every item taller than nothing started a new flex line — a
    // vertical stack came out as N side-by-side COLUMNS, each `1/N` of the width.
    //
    // That is not a corner case: `display:flex; flex-direction:column; flex-wrap:wrap` on the page
    // root is a stock design-system idiom (`.hz-Page-body` on marktplaats.nl, where it put the
    // header, the nav bar, the page body and the footer in four 1200px-wide columns and drove
    // `page-wrapper` to `x=2201` — h_overflow 742). Chrome, measured on the reduced fixture, does
    // NOT wrap: an auto-height column container has an indefinite main size, so all items share one
    // line and `min-height` only floors the result.
    //
    // The CROSS axis keeps `MinContent` — for a `row` container the height is the cross axis, it
    // does not decide line breaking, and it is content-sized either way.
    let vertical_main = matches!(tree.nodes[r].style.display, taffy::Display::Flex)
        && matches!(
            tree.nodes[r].style.flex_direction,
            FlexDirection::Column | FlexDirection::ColumnReverse
        );
    compute_root_layout(
        &mut tree,
        root,
        Size {
            width: AvailableSpace::Definite(container_width),
            height: match container_height {
                Some(h) => AvailableSpace::Definite(h),
                None if vertical_main => AvailableSpace::MaxContent,
                None => AvailableSpace::MinContent,
            },
        },
    );
    let child_ids: Vec<TId> = tree.nodes[r].children.clone();
    let placed: Vec<Placed> = child_ids.iter().map(|&c| tree.placed(c)).collect();
    // `content_box_height`, not `size.height`, and the two are **equal here** — `build` zeroes the
    // root's margin/padding/border because Manuk applies the container's own frame around the
    // content origin it passes in. This is the defensive form rather than a fix for a live bug: the
    // caller's `cy` IS the content origin and it adds the frame back itself, so if that zeroing is
    // ever removed a border-box height would silently double-count the padding. The claim this
    // spelling makes — *"the content box"* — stays true either way.
    (placed, tree.nodes[r].layout.content_box_height())
}

/// The **max-content width** of a flex/grid container, asked of taffy directly.
///
/// Do NOT compute this by laying the container out at a huge available width and reading the right
/// edge of its content: `max-width` clamps the container back down, and `justify-content: center`
/// then pushes the content to the middle of *that*, so the "extent" you read back is
/// `(max-width + content) / 2` — a number with no meaning. Wikipedia's 32px icon button measured
/// 234px that way, which overflowed the header's flex line and wrapped its search bar onto a second
/// row, displacing every element on the page below it.
///
/// Taffy already knows how to size a flex/grid container to its content; ask it.
pub fn max_content_width<'m>(
    dom: &Dom,
    styles: &StyleMap,
    container: DomNodeId,
    measure: impl FnMut(DomNodeId, Size<Option<f32>>, Size<AvailableSpace>) -> Size<f32> + 'm,
) -> f32 {
    let (mut tree, root) = TaffyDom::build(dom, styles, container, Box::new(measure));
    let r: usize = root.into();
    // Auto-size the root (do not pin it), then solve against MAX-CONTENT available space.
    tree.nodes[r].style.size = Size {
        width: auto(),
        height: auto(),
    };
    compute_root_layout(
        &mut tree,
        root,
        Size {
            width: AvailableSpace::MaxContent,
            height: AvailableSpace::MaxContent,
        },
    );
    tree.nodes[r].layout.size.width
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_flex_container_style() {
        let mut cs = ComputedStyle::initial();
        cs.display = CssDisplay::Flex;
        cs.width = Dim::Px(600.0);
        cs.flex_direction = CssDir::Column;
        cs.column_gap = 8.0;
        let t = to_taffy_style(&cs, &mut Vec::new());
        assert_eq!(t.display, Display::Flex);
        assert_eq!(t.flex_direction, FlexDirection::Column);
        assert_eq!(t.size.width, length(600.0));
        assert_eq!(t.gap.width, length(8.0));
    }

    #[test]
    fn maps_item_grow_and_auto_size() {
        let mut cs = ComputedStyle::initial();
        cs.flex_grow = 1.0;
        let t = to_taffy_style(&cs, &mut Vec::new());
        assert_eq!(t.flex_grow, 1.0);
        assert_eq!(t.size.width, auto());
    }

    /// The daily-driver `calc()` bar: a flex-item sidebar `width: calc(100% - 250px)` in a
    /// 1000px flex row must resolve to 750px, NOT the old collapse-to-one-term (-250px → 0).
    /// Falsifiable: reverting the calc plumbing drops the sidebar to 0 and this fails.
    #[test]
    fn flex_item_calc_width_mixes_px_and_percent() {
        use manuk_dom::Dom;
        use std::collections::HashMap;

        let mut dom = Dom::new();
        let container = dom.create_element("div");
        dom.append_child(dom.root(), container);
        let sidebar = dom.create_element("div");
        dom.append_child(container, sidebar);

        let mut styles: HashMap<_, _> = HashMap::new();
        let mut cc = ComputedStyle::initial();
        cc.display = CssDisplay::Flex;
        cc.width = Dim::Px(1000.0);
        styles.insert(container, cc);
        let mut cs = ComputedStyle::initial();
        cs.display = CssDisplay::Block;
        // calc(100% - 250px): px = -250, pct = 100 (percentages stored 0–100 in Dim).
        cs.width = Dim::Calc {
            px: -250.0,
            pct: 100.0,
        };
        cs.flex_shrink = 0.0; // don't let flex shrink the item below its basis
        styles.insert(sidebar, cs);

        let (placed, _solved_h) =
            solve_subtree(&dom, &styles, container, 1000.0, None, |_n, _k, _a| Size {
                width: 0.0,
                height: 0.0,
            });
        assert_eq!(placed.len(), 1);
        assert!(
            (placed[0].slot.width - 750.0).abs() < 1.0,
            "calc(100% - 250px) of 1000px should be 750px, got {}",
            placed[0].slot.width
        );
    }

    #[test]
    fn solve_subtree_lays_out_flex_row() {
        use manuk_dom::Dom;
        use std::collections::HashMap;

        // A 300px flex row with two grow:1 children → 150/150 split.
        let mut dom = Dom::new();
        let container = dom.create_element("div");
        dom.append_child(dom.root(), container);
        let a = dom.create_element("div");
        let b = dom.create_element("div");
        dom.append_child(container, a);
        dom.append_child(container, b);

        let mut styles: HashMap<_, _> = HashMap::new();
        let mut cc = ComputedStyle::initial();
        cc.display = CssDisplay::Flex;
        cc.width = Dim::Px(300.0);
        styles.insert(container, cc);
        for &child in &[a, b] {
            let mut cs = ComputedStyle::initial();
            cs.display = CssDisplay::Block;
            cs.flex_grow = 1.0;
            styles.insert(child, cs);
        }

        // Leaves measure to zero content (only grow matters here).
        let (placed, _solved_h) =
            solve_subtree(&dom, &styles, container, 300.0, None, |_n, _k, _a| Size {
                width: 0.0,
                height: 0.0,
            });
        assert_eq!(placed.len(), 2);
        let s0 = placed[0].slot;
        let s1 = placed[1].slot;
        assert!((s0.width - 150.0).abs() < 1.0, "got {s0:?}");
        assert!((s1.width - 150.0).abs() < 1.0, "got {s1:?}");
        assert!(s1.x >= s0.width - 1.0, "second is to the right");
        assert!(!placed[0].container, "block child is a leaf");
    }

    /// **An RTL flex ROW runs right-to-left** — `row` is a LOGICAL direction and taffy speaks only
    /// physical, so `direction: rtl` has to reach it as `row-reverse` (CSS Flexbox §5.1).
    ///
    /// Measured against live Chromium (`<html dir=rtl>`, a 600px flex row of three 100px items, x
    /// relative to the container):
    ///
    /// | item | Chrome | was |
    /// |---|---|---|
    /// | 1st | `500` | `0` ❌ |
    /// | 2nd | `400` | `100` ❌ |
    /// | 3rd | `300` | `200` ❌ |
    ///
    /// Every RTL nav bar, toolbar, breadcrumb and card row ran backwards. On `mobile.ir` the fix moved
    /// shape **0.174 → 0.320**, `h_overflow` **268 → 1** and `reading_order` 874 → 820, with coverage
    /// and `shape_n` unchanged; `marktplaats.nl` (LTR) was byte-identical.
    ///
    /// RED, run: drop the `rtl` argument from `map_direction` — the items read 0 / 100 / 200.
    #[test]
    fn an_rtl_flex_row_runs_right_to_left() {
        use manuk_dom::Dom;
        use std::collections::HashMap;

        let mut dom = Dom::new();
        let container = dom.create_element("div");
        dom.append_child(dom.root(), container);
        let kids: Vec<_> = (0..3)
            .map(|_| {
                let k = dom.create_element("div");
                dom.append_child(container, k);
                k
            })
            .collect();

        let mut styles: HashMap<_, _> = HashMap::new();
        let mut cc = ComputedStyle::initial();
        cc.display = CssDisplay::Flex;
        cc.direction = CssDirection::Rtl;
        cc.width = Dim::Px(600.0);
        styles.insert(container, cc);
        for &k in &kids {
            let mut cs = ComputedStyle::initial();
            cs.display = CssDisplay::Block;
            cs.direction = CssDirection::Rtl;
            cs.width = Dim::Px(100.0);
            cs.flex_shrink = 0.0;
            styles.insert(k, cs);
        }

        let (placed, _solved_h) =
            solve_subtree(&dom, &styles, container, 600.0, None, |_n, _k, _a| Size {
                width: 0.0,
                height: 0.0,
            });
        assert_eq!(placed.len(), 3);
        let xs: Vec<f32> = placed.iter().map(|p| p.slot.x).collect();
        assert!(
            (xs[0] - 500.0).abs() < 1.0
                && (xs[1] - 400.0).abs() < 1.0
                && (xs[2] - 300.0).abs() < 1.0,
            "the FIRST item is at the RIGHT edge and the row packs leftwards: {xs:?}"
        );
    }

    /// **`flex-wrap: wrap` on an auto-height COLUMN container must not wrap** — an indefinite main
    /// size is INFINITE available main space, not zero. Measured against live Chromium on
    /// `#c{display:flex;flex-direction:column;flex-wrap:wrap;min-height:100vh;width:1200px}` with
    /// 200 / 900 / 150-tall children:
    ///
    /// | box | Chrome | was |
    /// |---|---|---|
    /// | `#c` | `1200×1250` | `1200×900` ❌ |
    /// | `#a` | `[0 0 1200×200]` | `[0 0 400×200]` ❌ |
    /// | `#b` | `[0 200 1200×900]` | `[400 0 400×900]` ❌ |
    /// | `#d` | `[0 1100 1200×150]` | `[800 0 400×150]` ❌ |
    ///
    /// Every item started its own flex LINE, so a vertical stack came out as three side-by-side
    /// columns each a third of the width. On marktplaats.nl (`.hz-Page-body`, a stock design-system
    /// rule) that put the header, nav bar, page body and footer in four 1200px columns and drove
    /// `#page-wrapper` to `x=2201`.
    ///
    /// RED, run: restore `AvailableSpace::MinContent` for the indefinite-height case and both items
    /// land on x=0/x=600 in one row of two lines instead of stacking.
    ///
    /// The nowrap column, the row container and the DEFINITE-height column (which *must* still wrap,
    /// Chrome-verified) are unaffected — the control ran with the change stashed and only the
    /// wrap+auto-height column moved.
    #[test]
    fn auto_height_column_flex_does_not_wrap() {
        use manuk_dom::Dom;
        use std::collections::HashMap;

        let mut dom = Dom::new();
        let container = dom.create_element("div");
        dom.append_child(dom.root(), container);
        let a = dom.create_element("div");
        let b = dom.create_element("div");
        dom.append_child(container, a);
        dom.append_child(container, b);

        let mut styles: HashMap<_, _> = HashMap::new();
        let mut cc = ComputedStyle::initial();
        cc.display = CssDisplay::Flex;
        cc.flex_direction = CssDir::Column;
        cc.flex_wrap = CssWrap::Wrap;
        cc.width = Dim::Px(600.0);
        styles.insert(container, cc);
        for (child, h) in [(a, 100.0), (b, 400.0)] {
            let mut cs = ComputedStyle::initial();
            cs.display = CssDisplay::Block;
            cs.height = Dim::Px(h);
            styles.insert(child, cs);
        }

        // Height `None` = the container's own height is indefinite, which is the whole point.
        let (placed, _solved_h) =
            solve_subtree(&dom, &styles, container, 600.0, None, |_n, _k, _a| Size {
                width: 0.0,
                height: 0.0,
            });
        assert_eq!(placed.len(), 2);
        let (s0, s1) = (placed[0].slot, placed[1].slot);
        assert!(
            (s1.y - 100.0).abs() < 1.0 && s1.x.abs() < 1.0,
            "second item stacks BELOW the first, not beside it: {s1:?}"
        );
        assert!(
            (s0.width - 600.0).abs() < 1.0 && (s1.width - 600.0).abs() < 1.0,
            "one line means both items get the full cross size: {s0:?} {s1:?}"
        );
    }
}
