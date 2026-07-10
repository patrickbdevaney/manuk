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

use manuk_css::{AlignItems as CssAlign, JustifyContent as CssJustify};
use taffy::prelude::*;

/// One flex child's inputs. `width`/`height` are definite px if the style set them;
/// `grow` distributes leftover main-axis space (auto-width children get `1.0`).
#[derive(Clone, Copy, Debug)]
pub struct FlexItem {
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub grow: f32,
}

/// A resolved slot for a child. `x`/`y` are offsets from the container's content origin.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slot {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
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

/// Solve a single-row flex layout in a container of `container_width` px, honoring
/// `justify-content` (main axis) and `align-items` (cross axis).
pub fn solve_row(
    container_width: f32,
    items: &[FlexItem],
    justify: CssJustify,
    align: CssAlign,
) -> Vec<Slot> {
    let mut tree: TaffyTree<()> = TaffyTree::new();

    let children: Vec<NodeId> = items
        .iter()
        .map(|it| {
            let style = Style {
                size: Size {
                    width: it.width.map(length).unwrap_or(auto()),
                    height: it.height.map(length).unwrap_or(auto()),
                },
                flex_grow: it.grow,
                ..Default::default()
            };
            tree.new_leaf(style).expect("taffy leaf")
        })
        .collect();

    let root = tree
        .new_with_children(
            Style {
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                justify_content: Some(map_justify(justify)),
                align_items: Some(map_align(align)),
                size: Size {
                    width: length(container_width),
                    height: auto(),
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
            height: AvailableSpace::MinContent,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_grow_children_split_width() {
        let items = [
            FlexItem {
                width: None,
                height: Some(20.0),
                grow: 1.0,
            },
            FlexItem {
                width: None,
                height: Some(20.0),
                grow: 1.0,
            },
        ];
        let slots = solve_row(300.0, &items, CssJustify::FlexStart, CssAlign::Stretch);
        assert_eq!(slots.len(), 2);
        // Each grow:1 child gets half.
        assert!((slots[0].width - 150.0).abs() < 1.0, "got {slots:?}");
        assert!((slots[1].width - 150.0).abs() < 1.0);
        // Second is placed after the first on the main axis.
        assert!(slots[1].x >= slots[0].width - 1.0);
    }

    #[test]
    fn fixed_child_keeps_its_width() {
        let items = [
            FlexItem {
                width: Some(100.0),
                height: Some(20.0),
                grow: 0.0,
            },
            FlexItem {
                width: None,
                height: Some(20.0),
                grow: 1.0,
            },
        ];
        let slots = solve_row(300.0, &items, CssJustify::FlexStart, CssAlign::Stretch);
        assert!((slots[0].width - 100.0).abs() < 1.0);
        // The grow child takes the remaining 200.
        assert!((slots[1].width - 200.0).abs() < 1.0, "got {slots:?}");
    }

    #[test]
    fn justify_content_distributes_on_the_main_axis() {
        let two = |w: f32| FlexItem { width: Some(w), height: Some(40.0), grow: 0.0 };
        let items = [two(100.0), two(100.0)];

        // space-between pins the first to the left and the last to the right edge.
        let sb = solve_row(600.0, &items, CssJustify::SpaceBetween, CssAlign::Stretch);
        assert!(sb[0].x.abs() < 1.0, "first at left: {sb:?}");
        assert!((sb[1].x - 500.0).abs() < 1.0, "last at right edge: {sb:?}");

        // center centers the pair: leading space = (600 - 200)/2 = 200.
        let c = solve_row(600.0, &[two(100.0), two(100.0)], CssJustify::Center, CssAlign::Stretch);
        assert!((c[0].x - 200.0).abs() < 1.0, "centered start: {c:?}");

        // flex-end pushes to the right: first at 600 - 200 = 400.
        let e = solve_row(600.0, &[two(100.0), two(100.0)], CssJustify::FlexEnd, CssAlign::Stretch);
        assert!((e[0].x - 400.0).abs() < 1.0, "right-aligned: {e:?}");
    }
}
