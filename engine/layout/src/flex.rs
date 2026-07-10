//! Flexbox layout via `taffy`.
//!
//! CLAUDE.md designates `taffy` as the flexbox/grid engine. This module is the
//! integration seam: block/inline layout stays from-scratch, but a `display:flex`
//! container hands its children's sizing to taffy and gets back main-axis slots.
//!
//! Scope note: a single-line flex **row** with fixed or grow-to-fill children,
//! which is the common real-world case. Wrapping, `align-items`, `column`
//! direction, and grid are the next steps — all expressed through the same taffy
//! tree, so they extend rather than replace this.

use manuk_css::{
    AlignItems as CssAlign, FlexDirection as CssDir, FlexWrap as CssWrap,
    JustifyContent as CssJustify, TrackSize,
};
use taffy::prelude::*;

/// A flex item's `flex-basis`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FlexBasis {
    Auto,
    Px(f32),
    /// Fraction in `0.0..=1.0`.
    Pct(f32),
}

/// One flex child's inputs.
#[derive(Clone, Copy, Debug)]
pub struct FlexItem {
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub grow: f32,
    pub shrink: f32,
    pub basis: FlexBasis,
    /// `align-self`; `None` defers to the container's `align-items`.
    pub align_self: Option<CssAlign>,
}

/// The flex container's configuration.
#[derive(Clone, Copy, Debug)]
pub struct FlexConfig {
    pub direction: CssDir,
    pub wrap: CssWrap,
    pub justify: CssJustify,
    pub align: CssAlign,
    pub row_gap: f32,
    pub column_gap: f32,
}

/// A resolved slot for a child. `x`/`y` are offsets from the container's content origin.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slot {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
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

/// Map manuk's `justify-content` to taffy's (taffy 0.12 exposes CSS keywords as UPPER_SNAKE
/// associated constants; `JustifyContent` is an alias of `AlignContent`).
fn map_justify(j: CssJustify) -> taffy::style::JustifyContent {
    use taffy::style::JustifyContent as T;
    match j {
        CssJustify::FlexStart => T::FLEX_START,
        CssJustify::FlexEnd => T::FLEX_END,
        CssJustify::Center => T::CENTER,
        CssJustify::SpaceBetween => T::SPACE_BETWEEN,
        CssJustify::SpaceAround => T::SPACE_AROUND,
        CssJustify::SpaceEvenly => T::SPACE_EVENLY,
    }
}

/// Map manuk's `align-items` to taffy's.
fn map_align(a: CssAlign) -> taffy::style::AlignItems {
    use taffy::style::AlignItems as T;
    match a {
        CssAlign::Stretch => T::STRETCH,
        CssAlign::FlexStart => T::FLEX_START,
        CssAlign::FlexEnd => T::FLEX_END,
        CssAlign::Center => T::CENTER,
        CssAlign::Baseline => T::BASELINE,
    }
}

/// Solve a flex layout in a container `container_width` px wide (and `container_height` px
/// tall when definite — needed for column direction, wrapping, and cross-axis alignment),
/// honoring direction, wrap, gap, justify-content, align-items, and per-item grow/shrink/
/// basis. Returns each item's 2D slot relative to the container's content origin.
pub fn solve_flex(
    container_width: f32,
    container_height: Option<f32>,
    items: &[FlexItem],
    cfg: &FlexConfig,
) -> Vec<Slot> {
    let mut tree: TaffyTree<()> = TaffyTree::new();

    let children: Vec<NodeId> = items
        .iter()
        .map(|it| {
            let basis = match it.basis {
                FlexBasis::Auto => auto(),
                FlexBasis::Px(p) => length(p),
                FlexBasis::Pct(f) => percent(f),
            };
            let style = Style {
                size: Size {
                    width: it.width.map(length).unwrap_or(auto()),
                    height: it.height.map(length).unwrap_or(auto()),
                },
                flex_grow: it.grow,
                flex_shrink: it.shrink,
                flex_basis: basis,
                align_self: it.align_self.map(map_align),
                ..Default::default()
            };
            tree.new_leaf(style).expect("taffy leaf")
        })
        .collect();

    let root = tree
        .new_with_children(
            Style {
                display: Display::Flex,
                flex_direction: map_direction(cfg.direction),
                flex_wrap: map_wrap(cfg.wrap),
                justify_content: Some(map_justify(cfg.justify)),
                align_items: Some(map_align(cfg.align)),
                gap: Size {
                    width: length(cfg.column_gap),
                    height: length(cfg.row_gap),
                },
                size: Size {
                    width: length(container_width),
                    height: container_height.map(length).unwrap_or(auto()),
                },
                ..Default::default()
            },
            &children,
        )
        .expect("taffy root");

    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(container_width),
            height: match container_height {
                Some(h) => AvailableSpace::Definite(h),
                None => AvailableSpace::MinContent,
            },
        },
    )
    .expect("taffy compute");

    children
        .iter()
        .map(|&c| {
            let l = tree.layout(c).expect("taffy layout");
            Slot {
                x: l.location.x,
                y: l.location.y,
                width: l.size.width,
                height: l.size.height,
            }
        })
        .collect()
}

/// Solve a CSS Grid layout via taffy. `cols`/`rows` are the track templates; items
/// auto-place into the grid in order. Returns each item's 2D slot.
pub fn solve_grid(
    container_width: f32,
    container_height: Option<f32>,
    items: &[FlexItem],
    cols: &[TrackSize],
    rows: &[TrackSize],
    row_gap: f32,
    column_gap: f32,
) -> Vec<Slot> {
    let mut tree: TaffyTree<()> = TaffyTree::new();

    let children: Vec<NodeId> = items
        .iter()
        .map(|it| {
            tree.new_leaf(Style {
                size: Size {
                    width: it.width.map(length).unwrap_or(auto()),
                    height: it.height.map(length).unwrap_or(auto()),
                },
                ..Default::default()
            })
            .expect("taffy grid leaf")
        })
        .collect();

    let track = |t: &TrackSize| match t {
        TrackSize::Px(p) => length(*p),
        TrackSize::Fr(f) => fr(*f),
        TrackSize::Percent(p) => percent(*p / 100.0),
        TrackSize::Auto => auto(),
    };
    let root = tree
        .new_with_children(
            Style {
                display: Display::Grid,
                grid_template_columns: cols.iter().map(track).collect(),
                grid_template_rows: rows.iter().map(track).collect(),
                gap: Size { width: length(column_gap), height: length(row_gap) },
                size: Size {
                    width: length(container_width),
                    height: container_height.map(length).unwrap_or(auto()),
                },
                ..Default::default()
            },
            &children,
        )
        .expect("taffy grid root");

    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(container_width),
            height: match container_height {
                Some(h) => AvailableSpace::Definite(h),
                None => AvailableSpace::MinContent,
            },
        },
    )
    .expect("taffy grid compute");

    children
        .iter()
        .map(|&c| {
            let l = tree.layout(c).expect("taffy grid layout");
            Slot { x: l.location.x, y: l.location.y, width: l.size.width, height: l.size.height }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(justify: CssJustify) -> FlexConfig {
        FlexConfig {
            direction: CssDir::Row,
            wrap: CssWrap::NoWrap,
            justify,
            align: CssAlign::Stretch,
            row_gap: 0.0,
            column_gap: 0.0,
        }
    }
    fn item(width: Option<f32>, grow: f32) -> FlexItem {
        FlexItem { width, height: Some(20.0), grow, shrink: 1.0, basis: FlexBasis::Auto, align_self: None }
    }
    fn row(w: f32, items: &[FlexItem], j: CssJustify) -> Vec<Slot> {
        solve_flex(w, None, items, &cfg(j))
    }

    #[test]
    fn two_grow_children_split_width() {
        let slots = row(300.0, &[item(None, 1.0), item(None, 1.0)], CssJustify::FlexStart);
        assert_eq!(slots.len(), 2);
        assert!((slots[0].width - 150.0).abs() < 1.0, "got {slots:?}");
        assert!((slots[1].width - 150.0).abs() < 1.0);
        assert!(slots[1].x >= slots[0].width - 1.0);
    }

    #[test]
    fn fixed_child_keeps_its_width() {
        let slots = row(300.0, &[item(Some(100.0), 0.0), item(None, 1.0)], CssJustify::FlexStart);
        assert!((slots[0].width - 100.0).abs() < 1.0);
        assert!((slots[1].width - 200.0).abs() < 1.0, "got {slots:?}");
    }

    #[test]
    fn justify_content_distributes_on_the_main_axis() {
        let two = [item(Some(100.0), 0.0), item(Some(100.0), 0.0)];
        let sb = row(600.0, &two, CssJustify::SpaceBetween);
        assert!(sb[0].x.abs() < 1.0, "first at left: {sb:?}");
        assert!((sb[1].x - 500.0).abs() < 1.0, "last at right edge: {sb:?}");
        let c = row(600.0, &two, CssJustify::Center);
        assert!((c[0].x - 200.0).abs() < 1.0, "centered start: {c:?}");
        let e = row(600.0, &two, CssJustify::FlexEnd);
        assert!((e[0].x - 400.0).abs() < 1.0, "right-aligned: {e:?}");
    }

    #[test]
    fn column_direction_stacks_and_gap_separates() {
        let col = FlexConfig { direction: CssDir::Column, row_gap: 10.0, ..cfg(CssJustify::FlexStart) };
        let items = [
            FlexItem { width: Some(80.0), height: Some(30.0), grow: 0.0, shrink: 1.0, basis: FlexBasis::Auto, align_self: None },
            FlexItem { width: Some(80.0), height: Some(40.0), grow: 0.0, shrink: 1.0, basis: FlexBasis::Auto, align_self: None },
        ];
        let slots = solve_flex(200.0, Some(300.0), &items, &col);
        assert!(slots[0].y.abs() < 1.0, "first at top: {slots:?}");
        // Second stacks below the first plus the 10px row-gap.
        assert!((slots[1].y - 40.0).abs() < 1.0, "second below with gap: {slots:?}");
    }

    #[test]
    fn wrap_pushes_the_overflowing_item_to_the_next_line() {
        let three: Vec<FlexItem> = (0..3)
            .map(|_| FlexItem { width: Some(100.0), height: Some(30.0), grow: 0.0, shrink: 0.0, basis: FlexBasis::Auto, align_self: None })
            .collect();
        let wrap = FlexConfig { wrap: CssWrap::Wrap, ..cfg(CssJustify::FlexStart) };
        let slots = solve_flex(250.0, None, &three, &wrap);
        // Two fit on the first line; the third wraps.
        assert!(slots[0].y.abs() < 1.0 && slots[1].y.abs() < 1.0, "{slots:?}");
        assert!(slots[2].y >= 30.0 - 1.0, "third wrapped to next line: {slots:?}");
        assert!(slots[2].x.abs() < 1.0, "third starts at the left: {slots:?}");
    }
}
