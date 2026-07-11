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
    AlignItems as CssAlign, BoxSizing, ComputedStyle, Dim, Display as CssDisplay,
    FlexDirection as CssDir, FlexWrap as CssWrap, GridLine as CssGridLine, JustifyContent as CssJustify,
    Position as CssPosition, TrackSize as CssTrackSize, TrackUnit,
};
use taffy::prelude::*;
use taffy::style::{BoxSizing as TaffyBoxSizing, Position as TaffyPosition};

/// `Dim` → taffy `Dimension` (`auto` / length / percentage; a mixed `calc` collapses to its
/// dominant part — taffy's own calc plumbing is not wired, a documented v1 simplification).
fn dimension(d: Dim) -> Dimension {
    match d {
        Dim::Auto => auto(),
        Dim::Px(p) => length(p),
        Dim::Percent(p) => percent(p / 100.0),
        Dim::Calc { px, pct } => {
            if px != 0.0 {
                length(px)
            } else {
                percent(pct / 100.0)
            }
        }
    }
}

/// `Dim` → taffy `LengthPercentageAuto` (margins / insets).
fn lp_auto(d: Dim) -> LengthPercentageAuto {
    match d {
        Dim::Auto => auto(),
        Dim::Px(p) => length(p),
        Dim::Percent(p) => percent(p / 100.0),
        Dim::Calc { px, pct } => {
            if px != 0.0 {
                length(px)
            } else {
                percent(pct / 100.0)
            }
        }
    }
}

/// `Dim` → taffy `LengthPercentage` (padding; `auto` is invalid → 0).
fn lp(d: Dim) -> LengthPercentage {
    match d {
        Dim::Auto => length(0.0),
        Dim::Px(p) => length(p),
        Dim::Percent(p) => percent(p / 100.0),
        Dim::Calc { px, pct } => {
            if px != 0.0 {
                length(px)
            } else {
                percent(pct / 100.0)
            }
        }
    }
}

fn map_display(d: CssDisplay) -> Display {
    match d {
        CssDisplay::Flex => Display::Flex,
        CssDisplay::Grid => Display::Grid,
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

fn map_direction(d: CssDir) -> FlexDirection {
    match d {
        CssDir::Row => FlexDirection::Row,
        CssDir::RowReverse => FlexDirection::RowReverse,
        CssDir::Column => FlexDirection::Column,
        CssDir::ColumnReverse => FlexDirection::ColumnReverse,
    }
}

fn map_wrap(w: CssWrap) -> FlexWrap {
    match w {
        CssWrap::NoWrap => FlexWrap::NoWrap,
        CssWrap::Wrap => FlexWrap::Wrap,
        CssWrap::WrapReverse => FlexWrap::WrapReverse,
    }
}

fn map_justify(j: CssJustify) -> JustifyContent {
    match j {
        CssJustify::FlexStart => JustifyContent::FLEX_START,
        CssJustify::FlexEnd => JustifyContent::FLEX_END,
        CssJustify::Center => JustifyContent::CENTER,
        CssJustify::SpaceBetween => JustifyContent::SPACE_BETWEEN,
        CssJustify::SpaceAround => JustifyContent::SPACE_AROUND,
        CssJustify::SpaceEvenly => JustifyContent::SPACE_EVENLY,
    }
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

fn grid_line(pair: (CssGridLine, CssGridLine)) -> Line<GridPlacement> {
    fn one(l: CssGridLine) -> GridPlacement {
        match l {
            CssGridLine::Auto => GridPlacement::Auto,
            CssGridLine::Line(n) => line(n as i16),
            CssGridLine::Span(n) => span(n),
        }
    }
    Line { start: one(pair.0), end: one(pair.1) }
}

/// Map a Manuk [`ComputedStyle`] onto a taffy [`Style`], covering the box model + flex + grid
/// properties taffy needs to lay out a flex/grid container and its items. Inline/float/table
/// specifics stay with Manuk (this node is a leaf to taffy in those cases).
pub fn to_taffy_style(cs: &ComputedStyle) -> Style {
    Style {
        display: map_display(cs.display),
        box_sizing: if cs.box_sizing == BoxSizing::BorderBox {
            TaffyBoxSizing::BorderBox
        } else {
            TaffyBoxSizing::ContentBox
        },
        position: map_position(cs.position),
        inset: Rect {
            left: lp_auto(cs.inset.left),
            right: lp_auto(cs.inset.right),
            top: lp_auto(cs.inset.top),
            bottom: lp_auto(cs.inset.bottom),
        },
        size: Size { width: dimension(cs.width), height: dimension(cs.height) },
        min_size: Size { width: dimension(cs.min_width), height: dimension(cs.min_height) },
        max_size: Size { width: dimension(cs.max_width), height: dimension(cs.max_height) },
        margin: Rect {
            left: lp_auto(cs.margin.left),
            right: lp_auto(cs.margin.right),
            top: lp_auto(cs.margin.top),
            bottom: lp_auto(cs.margin.bottom),
        },
        padding: Rect {
            left: lp(cs.padding.left),
            right: lp(cs.padding.right),
            top: lp(cs.padding.top),
            bottom: lp(cs.padding.bottom),
        },
        border: Rect {
            left: length(cs.border_width.left),
            right: length(cs.border_width.right),
            top: length(cs.border_width.top),
            bottom: length(cs.border_width.bottom),
        },
        align_items: Some(map_align(cs.align_items)),
        align_self: cs.align_self.map(map_align),
        justify_content: Some(map_justify(cs.justify_content)),
        gap: Size { width: length(cs.column_gap), height: length(cs.row_gap) },
        flex_direction: map_direction(cs.flex_direction),
        flex_wrap: map_wrap(cs.flex_wrap),
        flex_grow: cs.flex_grow,
        flex_shrink: cs.flex_shrink,
        flex_basis: dimension(cs.flex_basis),
        grid_template_columns: cs.grid_template_columns.iter().map(|t| GridTemplateComponent::Single(track(t))).collect(),
        grid_template_rows: cs.grid_template_rows.iter().map(|t| GridTemplateComponent::Single(track(t))).collect(),
        grid_column: grid_line(cs.grid_column),
        grid_row: grid_line(cs.grid_row),
        ..Default::default()
    }
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
        let t = to_taffy_style(&cs);
        assert_eq!(t.display, Display::Flex);
        assert_eq!(t.flex_direction, FlexDirection::Column);
        assert_eq!(t.size.width, length(600.0));
        assert_eq!(t.gap.width, length(8.0));
    }

    #[test]
    fn maps_item_grow_and_auto_size() {
        let mut cs = ComputedStyle::initial();
        cs.flex_grow = 1.0;
        let t = to_taffy_style(&cs);
        assert_eq!(t.flex_grow, 1.0);
        assert_eq!(t.size.width, auto());
    }
}
