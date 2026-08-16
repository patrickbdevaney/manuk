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
        // taffy implements the §7.2.2 clamp itself; it was simply never asked, because the cascade
        // handed it `Auto`.
        CssTrackSize::FitContent(p) => fit_content(length(*p)),
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
    // ⚠⚠⚠ **CSS 2.1 §10.4's TWO CONFLICT ARMS — WHERE A MIN ON ONE AXIS AND A MAX ON THE OTHER
    // CANNOT BOTH BE MET WITH THE RATIO INTACT, AND THE SPEC SAYS THE RATIO LOSES.**
    //
    // taffy synthesises a missing min axis from the ratio (`Size::maybe_apply_aspect_ratio`, applied
    // to `min_size` in both `compute/leaf.rs` and `compute/flexbox.rs`) and that is CORRECT for the
    // single-violation arms — it is exactly what makes `min-width:600px` on a 480×474 image come out
    // 600×592.5, Chrome's answer. What taffy does not do is cap that synthesised minimum by the
    // OTHER axis's maximum, so `min-width:600px; max-height:100px` synthesises `min-height:592.5px`,
    // the min beats the max in the clamp, and the box is **493px too tall**. §10.4's table names the
    // case explicitly:
    //
    // ```text
    //   (w < min-w) and (h > max-h)   ->   (min-w, max-h)
    //   (w > max-w) and (h < min-h)   ->   (max-w, min-h)
    // ```
    //
    // Both bounds win and the ratio is abandoned. Capping the SYNTHESISED minimum is the smallest
    // form of that: taffy's clamp then lands on `(min-w, max-h)` by itself, and the eight arms it
    // already gets right are untouched because the cap only bites when `min/r` exceeds the max.
    //
    // Chrome-measured through five formatting contexts (480×474 image in a 230px column):
    //
    // ```text
    //                                       CHROME     block/float/abspos   flex        grid
    //   min-width:600px + max-height:100px  600x100    600x100  ✓           600x593 ✗   600x593 ✗
    //   max-width:150px + min-height:800px  150x800    150x800  ✓           810x800 ✗   150x800 ✓
    // ```
    //
    // ⚠ The block/float/abspos copies of §10.4 were already right (t1157 priced eight of ten arms
    // Chrome-exact); this is the fourth formatting context, which is taffy's, and the only one that
    // could not be fixed by writing the rule a fourth time — the number is produced inside the
    // dependency, so the fix has to be in what the dependency is TOLD. t1158 refuted the two
    // obvious sites (`clamp_replaced_intrinsic`, and withholding the ratio from taffy) by measuring
    // both INERT; the slot itself was already 600×592.5 before our block path ever saw it.
    //
    // ⚠ **PX BOUNDS ONLY, and that is a real limit rather than an oversight.** This seam has no
    // percentage basis — `dimension()` hands taffy the percentage to resolve later — so a
    // conflicting `min-width:80%`/`max-height:100px` pair is not detectable here and keeps taffy's
    // answer. Naming it beats a wrong basis guessed at.
    let conflict_min = |ratio: f32| -> (Option<f32>, Option<f32>) {
        let px = |d: manuk_css::Dim| match d {
            manuk_css::Dim::Px(p) => Some(p.max(0.0)),
            _ => None,
        };
        // A `max-*` that is an intrinsic KEYWORD is content-derived and not a length; leave those to
        // taffy rather than pretending a number was available.
        let max_w = px(cs.max_width).filter(|_| cs.max_width_keyword.is_none());
        let max_h = px(cs.max_height).filter(|_| cs.max_height_keyword.is_none());
        // Only the axis taffy would SYNTHESISE (the one whose min is `auto`) can be over-tight, and
        // only while the automatic-minimum zero above has not already made it definite.
        let synth_h = matches!(cs.min_height, manuk_css::Dim::Auto)
            && cs.overflow_y == manuk_css::Overflow::Visible;
        let synth_w = matches!(cs.min_width, manuk_css::Dim::Auto)
            && cs.overflow_x == manuk_css::Overflow::Visible;
        let h = match (synth_h, px(cs.min_width), max_h) {
            (true, Some(mw), Some(mh)) if mw / ratio > mh => Some(mh),
            _ => None,
        };
        let w = match (synth_w, px(cs.min_height), max_w) {
            (true, Some(mh), Some(mw)) if mh * ratio > mw => Some(mw),
            _ => None,
        };
        (w, h)
    };
    let (conflict_min_w, conflict_min_h) = match cs.aspect_ratio.filter(|r| *r > 0.0) {
        Some(r) => conflict_min(r),
        None => (None, None),
    };
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
        // ⚠⚠⚠ **A NATURAL SIZE IS NOT A SPECIFIED SIZE.** `manuk_css::fill_natural_size` writes a
        // decoded bitmap's own pixel size into `width`/`height` and MARKS the axes it filled — the
        // marks exist because those two are an INTRINSIC size wearing a declared size's type. Handed
        // to taffy as `Dimension::length` it is a definite main size, so `align-items: stretch` has
        // nothing to override and an image keeps its natural box: a 480×474 image in a
        // `height:288px` flex row came out **480×474**, overflowing, against Chrome's **292×288**.
        //
        // Dropping the marked axes to `auto` loses nothing: the measure seam still returns the
        // natural box as the item's CONTENT size, so an auto-height row is unchanged. It only stops
        // the natural box from outranking the formatting context. ⚠ On its own this changes NOTHING
        // (measured, t1123) — the measure seam reads the same `Dim::Px` out of the STYLE and hands
        // taffy the old answer anyway. It is half of a pair; the other half is the ratio transfer in
        // `layout_flex_or_grid`'s measure closure.
        size: Size {
            width: if cs.width_is_natural {
                auto()
            } else {
                dimension(cs.width, calc)
            },
            height: if cs.height_is_natural {
                auto()
            } else {
                dimension(cs.height, calc)
            },
        },
        // ⚠⚠⚠ **A NON-`visible` OVERFLOW ZEROES THE AUTOMATIC MINIMUM SIZE, AND THAT IS THE MOST
        // WIDELY-USED ESCAPE HATCH ON THE FLEX WEB.**
        //
        // CSS Box Sizing §5.1 / Flexbox §4.5: `min-width: auto` on a flex or grid item resolves to
        // the item's min-content size — **but only while the item's overflow in that axis is
        // `visible`**. `overflow: hidden` (or `scroll`/`auto`/`clip`) resolves it to **zero**, which
        // is precisely why `.item { overflow: hidden }` is the canonical fix for "my flex row will
        // not shrink" and appears in every truncating sidebar, breadcrumb, chat list and
        // table-shaped flex row on the web. Measured against Chrome, a 200px flex row whose item
        // holds a 337px `white-space: nowrap` string:
        //
        // ```text
        //                                                Chrome     before     after
        //   flex item, nowrap                            337.16     337        337     <- control
        //   flex item, nowrap, min-width: 0              200        200        200     <- control
        //   flex item, nowrap, overflow: hidden          200        337        200
        // ```
        //
        // ⚠ **`min-width: 0` was already correct, and that mirror is what makes this a branch rather
        // than the algorithm.** An author who writes the explicit zero got the right answer here for
        // as long as taffy has been in the tree; the author who writes `overflow: hidden` — which is
        // the more common of the two, 69.0% of the burndown corpus declares BOTH `display:flex` and
        // an `overflow:hidden` against 46.2% for `min-width:0` — did not.
        //
        // Per-axis, because the property is: `overflow-x` governs the inline minimum and
        // `overflow-y` the block one. Applying it to every box, not just flex/grid items, is safe:
        // a block box's automatic minimum is already zero, so this can only ever agree with it.
        min_size: Size {
            width: if let Some(w) = conflict_min_w {
                length(w)
            } else if cs.overflow_x != manuk_css::Overflow::Visible
                && matches!(cs.min_width, manuk_css::Dim::Auto)
            {
                length(0.0)
            } else {
                dimension(cs.min_width, calc)
            },
            height: if let Some(h) = conflict_min_h {
                length(h)
            } else if cs.overflow_y != manuk_css::Overflow::Visible
                && matches!(cs.min_height, manuk_css::Dim::Auto)
            {
                length(0.0)
            } else {
                dimension(cs.min_height, calc)
            },
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
        // A gap is a `LengthPercentage` in taffy too, so a percentage crosses intact and is
        // resolved against the container's inner size on that axis — which is the basis CSS asks
        // for and the reason the cascade must NOT resolve it (measured: `column-gap: 10%` of a
        // 300px grid is 30px; of the same grid with `padding: 0 50px` it is 20px, the CONTENT box).
        gap: Size {
            width: lp(cs.column_gap, calc),
            height: lp(cs.row_gap, calc),
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
/// A Manuk-measured leaf's answer: its content size, and its FIRST-LINE BASELINE from that box's
/// content-box top (`None` = it has no line box, and taffy's own bottom-edge fallback is correct).
type MeasureFn<'m> =
    dyn FnMut(DomNodeId, Size<Option<f32>>, Size<AvailableSpace>) -> (Size<f32>, Option<f32>) + 'm;

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

/// **The USED track sizes of one grid container — the value CSSOM §5.1 says
/// `grid-template-columns`/`-rows` resolve to, and the one number taffy computed on every layout and
/// then threw away at a trait boundary.**
///
/// `grid-template-*` is one of the few properties whose *resolved* value is the **used** value rather
/// than the computed one: Chrome answers a 900px `repeat(3, 1fr)` grid with `"300px 300px 300px"`,
/// not with the author's `repeat(3, 1fr)`. t1269 measured that we had only the specified tracks and
/// **withheld the property entirely** rather than publish a wrong answer of the right type. This is
/// the value that makes publishing it correct.
///
/// The sizes are the **tracks only** — taffy's `GridTrack` vector interleaves gutter pseudo-tracks
/// between the real ones, and `DetailedGridTracksInfo::sizes` has already filtered them out
/// (`gutters` carries them separately, and `gap` is published from the cascade). Implicit tracks are
/// included in document order exactly as Chrome includes them: an item auto-placed into row 4 of a
/// two-row template makes `gridTemplateRows` report four sizes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GridTracks {
    /// Used size, in px, of each column track — inline axis, in visual order.
    pub columns: Vec<f32>,
    /// Used size, in px, of each row track — block axis, top to bottom.
    pub rows: Vec<f32>,
}

struct TNode {
    dom: DomNodeId,
    style: Style,
    children: Vec<TId>,
    cache: Cache,
    /// The **exact** measure memo that sits in front of `cache`'s nine slots — see [`MeasureMemo`].
    memo: MeasureMemo,
    layout: Layout,
    /// Flex/grid container (taffy lays out its children) vs. Manuk-measured leaf.
    container: bool,
}

/// ⚠⚠⚠ **TAFFY'S PER-NODE CACHE HAS NINE SLOTS AND ONE OF THEM ABSORBS EVERY DEFINITE WIDTH — SO A
/// CONTAINER THAT SIZES ITS ITEMS AT SEVERAL WIDTHS EVICTS ITS OWN ANSWER ON EVERY PROBE.**
///
/// `Cache::compute_cache_slot` (taffy 0.12.1 `tree/cache.rs`) buckets a `ComputeSize` result by
/// *shape* of the request, not by its value: with neither dimension known, **slot 5 is
/// `(MaxContent | Definite(_), MaxContent | Definite(_))`** — every definite available width lands
/// there and overwrites the last one. Taffy states the assumption in its own comment: *"a node will
/// generally be sized under one or the other but not both."* On a real page that assumption fails,
/// and when it fails the cache does not merely stop helping — it turns each nesting level into a
/// multiplier, because a missed parent re-solves its whole subtree.
///
/// Measured on `morikoshi.net` (4,437 nodes, ten flex/grid containers, 48.7 s to load, the
/// `timeout-150s` cohort that IS the M1 scorability cap). One container, `NodeId(1441)`:
///
/// ```text
///                       probes   distinct leaves   cache_hit   cache_miss   per-leaf lookups
///   taffy's 9 slots    294,991               357     263,325      374,759              1,644
/// ```
///
/// ⚠⚠⚠ **1,644 lookups per leaf against 21 DISTINCT INPUTS.** The tree is asked the same twenty-one
/// questions seventy-eight times each and answers `None` every time. That is not an algorithm asking
/// a lot of different questions — it is a memo that does not retain, and no per-document total could
/// ever have said so (t1258-1259 built the ledger that names the container; this is what it named).
///
/// ⚠ **Scope, deliberately narrow: `ComputeSize` only.** `PerformLayout` keeps taffy's single-entry
/// `final_layout_entry` **unchanged**, and that is a correctness requirement rather than caution. A
/// `PerformLayout` run *writes* its descendants' `layout` fields; taffy's one slot means a repeat of
/// an earlier key always re-runs and re-writes them. Remembering more `PerformLayout` results would
/// let a stale hit skip those writes after an intervening different-key run had overwritten the
/// subtree — geometry corruption bought with a speed-up. `ComputeSize` writes nothing (taffy's
/// flexbox/grid return before final placement), which is exactly why it is the safe half.
///
/// The match predicate is taffy's own, reproduced rather than invented: same packed
/// known-dimensions/available-space key, same x-axis parent size. Only the *storage* differs — an
/// exact list instead of nine buckets — so a hit here is a hit taffy would also have served had its
/// slot not been overwritten. This is a **supplement, not a fork** (`STATUS.md` option 3): `CacheTree`
/// is the extension point taffy publishes for exactly this, and the fork surface stays empty.
#[derive(Default)]
struct MeasureMemo {
    /// `(packed key, x-axis parent size, measured outer size)`, newest last.
    entries: Vec<(u64, u32, Size<f32>)>,
}

/// Past this many distinct measure requests for ONE node the memo stops growing and behaves like
/// taffy's fixed cache again. A bound, not a tuning knob: the observed distinct-input count on the
/// pathological container is **21**, so this is an order of magnitude of headroom, and an unbounded
/// per-node `Vec` on a hostile page is a memory bug wearing a performance fix's clothes.
const MEASURE_MEMO_CAP: usize = 64;

impl MeasureMemo {
    /// Taffy's `mixed_cache_key`, for one axis: a known dimension if there is one, otherwise the
    /// available space. Reproduced from `tree/cache.rs` — the bit patterns are the contract.
    fn axis_key(kd: Option<f32>, avail: AvailableSpace) -> u32 {
        match kd {
            Some(v) => v.to_bits(),
            None => match avail {
                // Negated, exactly as taffy does, so a definite value can never collide with the
                // `INFINITY`/`NEG_INFINITY` bit patterns the two keywords use.
                AvailableSpace::Definite(v) => (-v).to_bits(),
                AvailableSpace::MinContent => f32::NEG_INFINITY.to_bits(),
                AvailableSpace::MaxContent => f32::INFINITY.to_bits(),
            },
        }
    }

    fn key(inputs: &LayoutInput) -> (u64, u32) {
        let kd = inputs.known_dimensions;
        let av = inputs.available_space;
        let packed = (u64::from(Self::axis_key(kd.width, av.width)) << 32)
            | u64::from(Self::axis_key(kd.height, av.height));
        // Taffy compares only the WIDTH of `parent_size` here (`x_axis_parent_size`), with the bit
        // it packs the requested axis into masked off. Same predicate, same mask.
        let parent_w = match inputs.parent_size.width {
            Some(v) => v.to_bits(),
            None => f32::INFINITY.to_bits(),
        } & !(1u32 << 31);
        (packed, parent_w)
    }

    fn get(&self, inputs: &LayoutInput) -> Option<Size<f32>> {
        let (packed, parent_w) = Self::key(inputs);
        self.entries
            .iter()
            .find(|(k, p, _)| *k == packed && *p == parent_w)
            .map(|(_, _, size)| *size)
    }

    fn store(&mut self, inputs: &LayoutInput, size: Size<f32>) {
        let (packed, parent_w) = Self::key(inputs);
        if let Some(e) = self
            .entries
            .iter_mut()
            .find(|(k, p, _)| *k == packed && *p == parent_w)
        {
            e.2 = size;
        } else if self.entries.len() < MEASURE_MEMO_CAP {
            self.entries.push((packed, parent_w, size));
        }
    }

    fn clear(&mut self) {
        self.entries.clear();
    }
}

/// A unified taffy tree spanning one flex/grid container and its directly-nested flex/grid
/// descendants. Block/inline/float/table children are leaves measured back into Manuk.
pub struct TaffyDom<'m> {
    nodes: Vec<TNode>,
    measure: Box<MeasureFn<'m>>,
    /// Mixed `calc(px + pct%)` terms, indexed by the handle encoded into each calc `Dimension`.
    /// taffy hands the handle back to [`Self::resolve_calc_value`] with the definite basis.
    calc: Vec<(f32, f32)>,
    /// **THE ROOT-SUPPRESSION FLAG — the one node whose intrinsic keyword must NOT be resolved in
    /// this tree, and the reason [`Self::resolve_intrinsic_inline`] can run for a CONTAINER at all.**
    ///
    /// Measuring a container's intrinsic width answers by building a *second* `TaffyDom` rooted at
    /// that same node ([`max_content_width`] / [`solve_subtree`], both reached through the measure
    /// callback). Without this, the nested build's `add` would arrive back at the resolver on the
    /// node it is already inside and recurse without bound — a Bar 0 crash, not a wrong number.
    /// Suppressing it at the ROOT is what makes the nested build terminate: the root's own width is
    /// not this tree's business anyway, because the block path that wraps the subtree already
    /// resolved it (`shrink_to_fit`) and hands it in as `container_width`.
    built_for: DomNodeId,
    /// **THE ROOT'S OWN PADDING, KEPT BECAUSE ONE CONSUMER STILL NEEDS IT: a GRID's out-of-flow
    /// static-position AREA.** [`Self::build`] zeroes the root's frame (Manuk applies it around the
    /// subtree), and for every in-flow answer that is right. But taffy's grid gives an
    /// auto-placed `position:absolute` child the rect `border.top … border_box.height −
    /// border.bottom` — **the padding box**, exactly as Grid §9 says — and with the padding zeroed
    /// that rect collapses onto the CONTENT box. Handing the real padding back on a grid root (see
    /// [`solve_subtree`]) is what makes taffy's already-correct rule visible.
    root_padding: Rect<LengthPercentage>,
    /// **The used track sizes taffy hands back through `LayoutGridContainer::set_detailed_grid_info`,
    /// which is a no-op by default and was therefore invisible.** Keyed by DOM node because the
    /// taffy ids are private to this tree and every consumer upstream speaks `DomNodeId`.
    ///
    /// ⚠ **THE WRITE POLARITY IS LAST-WINS, AND THAT IS A READING OF TAFFY, NOT A GUESS.**
    /// `compute_grid_layout` returns at `run_mode == RunMode::ComputeSize` (taffy 0.12.1
    /// `compute/grid/mod.rs:543`) **before** it reaches the `set_detailed_grid_info` call at 703 — so
    /// an intrinsic *sizing* request against a node in THIS tree never writes here at all, and every
    /// entry is from a real `PerformLayout`. The remaining probe hazard is one level out: a
    /// max-content measure builds a **second** `TaffyDom` and runs a full `compute_root_layout` on it
    /// at a huge available width, which does write. That one is caught by `Ctx::intrinsic_probe` at
    /// the point of RECORDING (see `layout_flex_or_grid`) — the same flag `record_transform` gates on
    /// after t1120 poisoned `pre_transform_rect` permanently by being first-write-wins. ⚠ That guard
    /// measured INERT (removing it changes no published value, because a flex container measures its
    /// items before it lays them out, so the real pass writes last anyway); the ordering is what
    /// carries this, and the guard is defence in depth. Recorded at the recording site.
    grid_tracks: Vec<(DomNodeId, GridTracks)>,
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
            built_for: container,
            root_padding: Rect::zero(),
            grid_tracks: Vec::new(),
        };
        let root = tree.add(dom, styles, container);
        // The container's own margin/padding/border/inset are applied by Manuk's block
        // layout around this subtree; the tree just positions children from the content
        // origin, so zero them on the root and pin it in flow.
        let r: usize = root.into();
        tree.root_padding = tree.nodes[r].style.padding;
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
    /// ⚠⚠⚠ **AN ITEM THAT IS ITSELF A FLEX/GRID CONTAINER IS NOW RESOLVED HERE TOO, AND THE THING
    /// THAT MAKES THAT SAFE IS [`TaffyDom::built_for`].** Until t1163 the call site read
    /// `if !container`, because the measure callback answers a container's intrinsic width by
    /// building a *second* `TaffyDom` rooted at that node, whose `add` would reach this function
    /// again on the same node and recurse without bound. The root-suppression flag breaks exactly
    /// that cycle — the nested build declines to resolve its OWN root — so the guard here narrows
    /// from *"never for a container"* to *"never for the node this tree was built for"*.
    ///
    /// ⚠⚠ **THE SYMPTOM DID NOT LOOK LIKE A MISSING INPUT, IT LOOKED LIKE A GRID BUG (t1162).** A
    /// grid item's default `justify-items: stretch` fills the track on the INLINE axis, which is
    /// *correct* for `width: auto`. With the keyword dropped, `width: min-content` arrived
    /// indistinguishable from `auto`, and the grid dutifully stretched a box that should never have
    /// been stretchable — 230 against Chrome's 110. A flex parent stretches only the CROSS axis,
    /// which is why the same subjects were already exact there and why fifteen cells of one battery
    /// pointed at a correct implementation.
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
            .0
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
        // A container's intrinsic keyword is resolvable here as of t1163; the ROOT's is not, and
        // `built_for` says why — the measure that would answer it builds a tree rooted at this very
        // node, so resolving it at the root is unbounded recursion rather than a number.
        if dom.is_element(node) && node != self.built_for {
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
            memo: MeasureMemo::default(),
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

    /// **A FLEX OR GRID CONTAINER BEING *SIZED* ANSWERS WITH ITS MAX-CONTENT WIDTH, AND THE
    /// FIT-CONTENT CLAMP ABOVE IT IS NOBODY'S.**
    ///
    /// `determine_hypothetical_cross_size` hands a child the container's inner cross size as
    /// `AvailableSpace::Definite` and takes whatever comes back (taffy 0.12.1
    /// `compute/flexbox.rs:1403-1426`). For a Manuk-measured leaf that is correct — `shrink_to_fit`
    /// applies `pref.min(avail.max(min_content))` on the way out. For a child that is ITSELF a flex
    /// container, `determine_container_main_size` (`:955-981`) sums the items' **flex base sizes**
    /// and returns that, with no clamp to the definite available space it was given, so the answer
    /// comes back at max-content and the parent uses it verbatim as the hypothetical cross size.
    ///
    /// The visible result is `hnhbkis.edu.in`'s logo card: `<div class="h-72 flex items-center
    /// justify-center"><img class="max-h-72 max-w-full"></div>` inside `flex flex-col items-center`
    /// — the frame reported the image's natural **480px** inside a **230px** card, `align-items:
    /// center` centred the overflow, and the box landed 125px off the left edge of its own parent
    /// and 21px outside the viewport. Chrome-measured, on the reduction with the classes expanded:
    ///
    /// ```text
    ///                                                       CHROME   BEFORE    AFTER
    ///   col + items-center · frame · img max-w-full           230      480      230   <- the site
    ///   col + items-center · frame · nowrap text (fits)       204      204      204   CTRL
    ///   row parent        · frame · img max-w-full            230      480      230
    ///   row parent        · frame flex-shrink:0 · plain img   480      480      480   CTRL
    /// ```
    ///
    /// ⚠⚠⚠ **THE `flex-shrink: 0` CONTROL IS PRESERVED BY THE MIN-CONTENT TERM, NOT BY A SCOPE
    /// TEST, AND THAT IS WHY THIS IS SAFE TO APPLY EVERYWHERE.** The clamp is the fit-content
    /// formula in full — `min(max-content, max(min-content, available))` — so a container that
    /// genuinely CANNOT be narrower than the space it was offered keeps its overflow: an image with
    /// no `max-width` has a min-content of 480 and the `max()` returns 480 unchanged. Writing it as
    /// a bare `min(width, available)` would pass three of those four rows and silently un-break the
    /// web's most common overflow.
    ///
    /// ⚠ Inline axis only. A flex container's block size is its content height, not a fit-content
    /// size, and the same clamp on the cross-block axis would flatten every column that is taller
    /// than its offered space. ⚠ And only where the width is `auto` and unknown: with a definite
    /// width there is nothing to fit, and in `PerformLayout` the size has already been decided by
    /// the caller — re-clamping there would fight it.
    ///
    /// This is a **supplement, not a patch** (`STATUS.md`, option 3): the fork surface stays empty
    /// and a taffy bump cannot silently revert it, because the gate above owns the numbers.
    fn fit_content_inline(
        &mut self,
        node_id: TId,
        inputs: LayoutInput,
        width: f32,
        mut run: impl FnMut(&mut Self, LayoutInput) -> LayoutOutput,
    ) -> f32 {
        let idx: usize = node_id.into();
        if inputs.run_mode != taffy::RunMode::ComputeSize
            || inputs.known_dimensions.width.is_some()
            || !self.nodes[idx].style.size.width.is_auto()
        {
            return width;
        }
        let AvailableSpace::Definite(avail) = inputs.available_space.width else {
            return width;
        };
        if !(width > avail) {
            return width;
        }
        let min_content = run(
            self,
            LayoutInput {
                available_space: Size {
                    width: AvailableSpace::MinContent,
                    ..inputs.available_space
                },
                ..inputs
            },
        )
        .size
        .width;
        width.min(avail.max(min_content))
    }

    fn dispatch(&mut self, node_id: TId, inputs: LayoutInput) -> LayoutOutput {
        let idx: usize = node_id.into();
        if self.nodes[idx].container {
            let run = |tree: &mut Self, inputs| match tree.nodes[idx].style.display {
                Display::Grid => compute_grid_layout(tree, node_id, inputs),
                _ => compute_flexbox_layout(tree, node_id, inputs),
            };
            let mut out = run(self, inputs);
            out.size.width = self.fit_content_inline(node_id, inputs, out.size.width, run);
            out
        } else {
            // Manuk-measured leaf: content-size via the callback into block/inline layout.
            let style = self.nodes[idx].style.clone();
            let dom_node = self.nodes[idx].dom;
            let measure = &mut self.measure;
            let mut baseline: Option<f32> = None;
            let out = compute_leaf_layout(
                inputs,
                &style,
                |_, _| 0.0,
                |known, avail| {
                    let (size, b) = measure(dom_node, known, avail);
                    baseline = b;
                    size
                },
            );
            // ── ⚠⚠⚠ **`compute_leaf_layout` ALWAYS REPORTS `first_baselines: Point::NONE`, AND
            //    TAFFY'S FALLBACK FOR THAT IS THE BOX'S BOTTOM EDGE**
            //    (`first_baselines.y.unwrap_or(size.height)`, `compute/flexbox.rs`). Every leaf in
            //    this tree is Manuk-measured, so `align-items: baseline` was silently `end` in BOTH
            //    formatting contexts — a 32px item beside a 16px one put the small item's top at
            //    `big_height - small_height` instead of at the shared baseline, 18 against Chrome's
            //    15. The error hides completely whenever the items are the same height, which is
            //    most of them, which is why a 40-row grid battery found this one row and no other.
            //
            //    The measure returns the baseline from the leaf's CONTENT-box top and taffy's is
            //    from the BORDER-box top, so the frame is added back here — the one thing that
            //    cannot be read off the returned `LayoutOutput`.
            let frame_top = taffy::ResolveOrZero::<Option<f32>, f32>::resolve_or_zero(
                style.border.top,
                None,
                |_, _| 0.0,
            ) + taffy::ResolveOrZero::<Option<f32>, f32>::resolve_or_zero(
                style.padding.top,
                None,
                |_, _| 0.0,
            );
            match baseline {
                Some(b) => LayoutOutput {
                    first_baselines: taffy::Point {
                        x: None,
                        y: Some(b + frame_top),
                    },
                    ..out
                },
                None => out,
            }
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
        let n = &self.nodes[usize::from(node_id)];
        // `ComputeSize` is served by the exact memo (see `MeasureMemo` for why only this run mode);
        // everything else is taffy's cache verbatim.
        if inputs.run_mode == taffy::RunMode::ComputeSize {
            if let Some(size) = n.memo.get(inputs) {
                return Some(LayoutOutput::from_outer_size(size));
            }
        }
        n.cache.get(inputs)
    }
    fn cache_store(&mut self, node_id: TId, inputs: &LayoutInput, output: LayoutOutput) {
        let n = &mut self.nodes[usize::from(node_id)];
        if inputs.run_mode == taffy::RunMode::ComputeSize {
            n.memo.store(inputs, output.size);
        }
        n.cache.store(inputs, output);
    }
    fn cache_clear(&mut self, node_id: TId) {
        let n = &mut self.nodes[usize::from(node_id)];
        n.memo.clear();
        n.cache.clear();
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

    /// ⚠⚠⚠ **THE DEFAULT BODY OF THIS METHOD IS A NO-OP, WHICH IS WHY A NUMBER THAT WAS COMPUTED ON
    /// EVERY SINGLE LAYOUT WAS UNREACHABLE FOR THE WHOLE LIFE OF THE GRID IMPLEMENTATION.**
    ///
    /// taffy resolves the used size of every grid track — it has to, that *is* grid layout — and then
    /// offers it here. Not overriding it is not "the feature is off"; the work happened and the
    /// result was discarded one frame from the caller. `detailed_layout_info` is in taffy 0.12's
    /// `default` feature set, so nothing had to be enabled: only this body had to exist.
    ///
    /// That is the same shape as t1267's `element.animate()` (the method WORKED while its feature
    /// detect was false) read from the other side: here the capability was *present in the
    /// dependency* and absent from us because a hook nobody wrote defaults to silence.
    fn set_detailed_grid_info(&mut self, node_id: TId, info: taffy::DetailedGridInfo) {
        let dom = self.nodes[usize::from(node_id)].dom;
        self.grid_tracks.push((
            dom,
            GridTracks {
                columns: info.columns.sizes,
                rows: info.rows.sizes,
            },
        ));
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
    measure: impl FnMut(DomNodeId, Size<Option<f32>>, Size<AvailableSpace>) -> (Size<f32>, Option<f32>)
        + 'm,
) -> (Vec<Placed>, f32, Vec<(DomNodeId, GridTracks)>) {
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
    let mut placed: Vec<Placed> = child_ids.iter().map(|&c| tree.placed(c)).collect();
    // `content_box_height`, not `size.height`, and **the two are equal here** — `build` zeroes the
    // root's margin/padding/border because Manuk applies the container's own frame around the
    // content origin it passes in. This is the defensive form rather than a fix for a live bug: the
    // caller's `cy` IS the content origin and it adds the frame back itself, so if that zeroing is
    // ever removed a border-box height would silently double-count the padding. The claim this
    // spelling makes — *"the content box"* — stays true either way.
    //
    // ⚠ It is read HERE, before the §9.1 re-solve below, because that pass deliberately perturbs
    // the root's box and must not be allowed to answer any question but its own.
    let content_h = tree.nodes[r].layout.content_box_height();
    // ⚠ **READ HERE, FOR THE SAME REASON `content_h` IS READ HERE — the §9.1 re-solve below
    // deliberately perturbs the root's box** (it zeroes the min/max size and hands the real padding
    // back so an abspos child's static position lands in the PADDING box), and a grid re-solved
    // under a different box answers a different track question. `set_detailed_grid_info` fires again
    // on that pass, so taking the snapshot *after* it would publish the perturbation.
    //
    // The vector is drained rather than cloned: the tree is dropped a few lines later and every
    // entry is wanted.
    let tracks = std::mem::take(&mut tree.grid_tracks);
    restate_grid_abspos_in_the_padding_box(
        &mut tree,
        root,
        styles,
        container,
        container_width,
        content_h,
        &child_ids,
        &mut placed,
    );
    (placed, content_h, tracks)
}

/// ⚠⚠⚠ **GRID §9 HAS TWO SECTIONS AND THEY GIVE DIFFERENT ANSWERS — the discriminator is not
/// `display`, it is whether the grid is ALSO the child's CONTAINING BLOCK.** An abspos child of a
/// grid with no definite grid position takes its static position from *"a grid area whose edges
/// coincide with"*: the **padding** edges when the grid is the containing block (§9.1), the
/// **content** edges when the grid is merely the parent (§9.2). Measured against live Chromium, one
/// variable per row, with flex as the control (t1175):
///
/// ```text
///     display  position    Chrome     which box
///     grid     static      36,97      CONTENT edge
///     grid     relative    23,23      PADDING edge   ← the only row that moves
///     flex     static      36,97      CONTENT edge
///     flex     relative    36,97      CONTENT edge   ← `position` does not flip flex (§4.1)
/// ```
///
/// **Taffy already implements §9.1** — `compute/grid/mod.rs` gives an abspos child with
/// unresolvable track indexes the rect `border.top … container_border_box.height − border.bottom`,
/// which IS the padding box. It was invisible because [`TaffyDom::build`] zeroes the root's frame,
/// collapsing that rect onto the content box; by that accident every §9.2 page was already right.
///
/// ⚠⚠⚠ **THIS RUNS AS A SECOND SOLVE, AND THE REASON IS A MEASURED REGRESSION.** The obvious
/// spelling — hand the padding back before the one and only `compute_root_layout` — is wrong in a
/// way no reasoning about coordinates predicts: taffy adds the padding to `min_size` as a
/// box-sizing adjustment (`grid/mod.rs`), so `min-height:100px` on a padded grid grew the container
/// **and its row** by the padding (`grid-box-sizing-001`: 144 → 168, the item 100 → 124, −2
/// `css/css-grid` subtests). The root's box participates in sizing; it must not be perturbed by a
/// question about where one out-of-flow child sits.
///
/// So pass 1 above stays the sole authority for every size and every in-flow position — it is not
/// re-read here — and this pass exists only to ask taffy *"align this child in the padding box
/// instead"*. Both axes are pinned to the size pass 1 already resolved and `min`/`max` are cleared,
/// so nothing in pass 2 can size anything: the tracks are the same tracks, and only the slots of
/// `position:absolute` children are copied out, shifted back into content-origin coordinates.
///
/// ⚠⚠ **TWO EARLIER SHAPES OF THIS FIX WERE REFUSED BY THE RATCHET, AND BOTH LOST THE SAME EIGHT
/// `-large-border-padding` REFTESTS** — t1174 as a post-hoc subtraction of the padding, and t1175's
/// first cut as the padding box for EVERY grid root. Those eight are §9.2 (their grids are
/// `position: static`) and their references are ordinary in-flow items, so they assert the content
/// box directly. **t1173's control table varied `display` while holding `position: relative` fixed,
/// and read the whole effect onto the variable it happened to be varying.**
///
/// To watch it go RED: drop the `is_abs_containing_block` term (the §9.2 rows move to the padding
/// edge and those eight reftests fail), or skip the call entirely (the §9.1 row stays at the
/// content edge).
#[allow(clippy::too_many_arguments)]
fn restate_grid_abspos_in_the_padding_box(
    tree: &mut TaffyDom<'_>,
    root: TId,
    styles: &StyleMap,
    container: DomNodeId,
    container_width: f32,
    content_h: f32,
    child_ids: &[TId],
    placed: &mut [Placed],
) {
    let r: usize = root.into();
    let pad = tree.root_padding;
    if !matches!(tree.nodes[r].style.display, taffy::Display::Grid)
        || !crate::is_abs_containing_block(&styles[&container])
        || pad == Rect::zero()
    {
        return;
    }
    let out_of_flow: Vec<usize> = child_ids
        .iter()
        .enumerate()
        .filter(|(_, &c)| {
            tree.nodes[usize::from(c)].style.position == taffy::style::Position::Absolute
        })
        .map(|(i, _)| i)
        .collect();
    if out_of_flow.is_empty() {
        return;
    }
    for n in tree.nodes.iter_mut() {
        n.cache.clear();
    }
    tree.nodes[r].style.padding = pad;
    tree.nodes[r].style.box_sizing = taffy::BoxSizing::ContentBox;
    tree.nodes[r].style.min_size = Size {
        width: auto(),
        height: auto(),
    };
    tree.nodes[r].style.max_size = Size {
        width: auto(),
        height: auto(),
    };
    tree.nodes[r].style.size = Size {
        width: length(container_width),
        height: length(content_h.max(0.0)),
    };
    compute_root_layout(
        tree,
        root,
        Size {
            width: AvailableSpace::Definite(container_width),
            height: AvailableSpace::Definite(content_h.max(0.0)),
        },
    );
    // Taffy's OWN resolved padding, not the style's, so a percentage cancels exactly against the
    // shift it produced.
    let resolved = tree.nodes[r].layout.padding;
    for i in out_of_flow {
        let mut p = tree.placed(child_ids[i]);
        p.slot.x -= resolved.left;
        p.slot.y -= resolved.top;
        placed[i] = p;
    }
}

/// ⚠⚠⚠ **GRID §9.1 REPLACES THE CONTAINING BLOCK ITSELF, AND THE OUT-OF-FLOW PASS ONLY EVER KNEW
/// THE PADDING BOX.**
///
/// > *"If an absolutely positioned element's containing block is generated by a grid container, the
/// > containing block corresponds to the grid area determined by its grid-placement properties."*
/// > — CSS Grid §9.1
///
/// Note the subject: *an absolutely positioned element whose containing block is generated by a
/// grid container* — **not** *a child of one*. A `position:absolute` box nested two levels down
/// inside a grid item gets the grid AREA as its containing block just the same, and
/// [`crate::LayoutTree::abs_containing_block`] returned the grid's **padding box** for every one of
/// them. Measured against the WPT `css/css-grid/abspos` corpus, where the whole
/// `positioned-grid-descendants-*` family is exactly this shape:
///
/// ```text
///   width  expected 505 · 445 · 305 · 145 · 5 · 0 …   got 510 for EVERY ONE
///   height expected 340 · 200 · 190 ·  90 · 30 · 0 …  got 360 for EVERY ONE
/// ```
///
/// 510 is `570 − left − right` and 360 is `430 − top − bottom` — the padding box of the grid, minus
/// the insets, whatever the placement said. One number for nine different expected answers is the
/// signature of a containing block that never varied.
///
/// **Taffy already implements this rule and we could not reach its answer.** `compute/grid/mod.rs`
/// builds exactly this rect for an abspos child (`rows[index].offset … columns[index].offset`, with
/// the padding edge as the fallback for an `auto` line) — including named lines, spans, RTL column
/// order and implicit tracks. Reimplementing that from the track list would be a second
/// implementation of a rule we already ship, which is the failure mode this codebase keeps naming.
///
/// So this asks taffy the question instead of answering it: solve the grid one more time with a
/// **phantom** `position:absolute` child carrying the probe's grid placement and `inset: 0` on all
/// four sides. Both insets present on an axis makes taffy derive the size as
/// `grid_area − start − end`, so the phantom's layout **is** the grid area, in the grid's own
/// coordinates. Nothing else is read from the solve.
///
/// ⚠⚠ The root is pinned and its padding handed back for the same reason
/// [`restate_grid_abspos_in_the_padding_box`] does it, and the reason is the same measured trap:
/// taffy folds padding into `min_size` as a box-sizing adjustment, so restoring it on an
/// unpinned root would grow the container and its tracks. Pinned to the size pass 1 already
/// resolved, with `min`/`max` cleared, nothing in this solve can size anything.
///
/// ⚠ The returned slot is relative to the grid's **padding box** origin — the padding is restored
/// and the border zeroed, so taffy's own `border.top`/`border_box.height − border.bottom` fallback
/// for an `auto` line lands on the padding edge, which is what §9.1 asks for.
pub fn grid_area_containing_block<'m>(
    dom: &Dom,
    styles: &StyleMap,
    container: DomNodeId,
    container_width: f32,
    content_h: f32,
    probe: DomNodeId,
    measure: impl FnMut(DomNodeId, Size<Option<f32>>, Size<AvailableSpace>) -> (Size<f32>, Option<f32>)
        + 'm,
) -> Option<Slot> {
    let (mut tree, root) = TaffyDom::build(dom, styles, container, Box::new(measure));
    let r: usize = root.into();
    if !matches!(tree.nodes[r].style.display, taffy::Display::Grid) {
        return None;
    }
    let cs = &styles[&probe];
    let phantom = Style {
        position: taffy::style::Position::Absolute,
        inset: Rect {
            left: length(0.0),
            right: length(0.0),
            top: length(0.0),
            bottom: length(0.0),
        },
        grid_row: grid_line(cs.grid_row),
        grid_column: grid_line(cs.grid_column),
        ..Style::DEFAULT
    };
    let pid = TId::from(tree.nodes.len());
    tree.nodes.push(TNode {
        // The phantom stands in for the probe, so the measure callback it may reach is asked about
        // the same element the caller is positioning — never a synthetic node with no style.
        dom: probe,
        style: phantom,
        children: Vec::new(),
        cache: Cache::new(),
        memo: MeasureMemo::default(),
        layout: Layout::new(),
        container: false,
    });
    tree.nodes[r].children.push(pid);
    let pad = tree.root_padding;
    tree.nodes[r].style.padding = pad;
    tree.nodes[r].style.box_sizing = taffy::BoxSizing::ContentBox;
    tree.nodes[r].style.min_size = Size {
        width: auto(),
        height: auto(),
    };
    tree.nodes[r].style.max_size = Size {
        width: auto(),
        height: auto(),
    };
    tree.nodes[r].style.size = Size {
        width: length(container_width),
        height: length(content_h.max(0.0)),
    };
    compute_root_layout(
        &mut tree,
        root,
        Size {
            width: AvailableSpace::Definite(container_width),
            height: AvailableSpace::Definite(content_h.max(0.0)),
        },
    );
    let l = tree.nodes[usize::from(pid)].layout;
    Some(Slot {
        x: l.location.x,
        y: l.location.y,
        width: l.size.width,
        height: l.size.height,
    })
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
    measure: impl FnMut(DomNodeId, Size<Option<f32>>, Size<AvailableSpace>) -> (Size<f32>, Option<f32>)
        + 'm,
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
        cs.column_gap = Dim::Px(8.0);
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

        let (placed, _solved_h, _tracks) =
            solve_subtree(&dom, &styles, container, 1000.0, None, |_n, _k, _a| {
                (
                    Size {
                        width: 0.0,
                        height: 0.0,
                    },
                    None,
                )
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
        let (placed, _solved_h, _tracks) =
            solve_subtree(&dom, &styles, container, 300.0, None, |_n, _k, _a| {
                (
                    Size {
                        width: 0.0,
                        height: 0.0,
                    },
                    None,
                )
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

        let (placed, _solved_h, _tracks) =
            solve_subtree(&dom, &styles, container, 600.0, None, |_n, _k, _a| {
                (
                    Size {
                        width: 0.0,
                        height: 0.0,
                    },
                    None,
                )
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
        let (placed, _solved_h, _tracks) =
            solve_subtree(&dom, &styles, container, 600.0, None, |_n, _k, _a| {
                (
                    Size {
                        width: 0.0,
                        height: 0.0,
                    },
                    None,
                )
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
