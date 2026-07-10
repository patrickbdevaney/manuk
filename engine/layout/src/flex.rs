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

use taffy::prelude::*;

/// One flex child's inputs. `width`/`height` are definite px if the style set them;
/// `grow` distributes leftover main-axis space (auto-width children get `1.0`).
#[derive(Clone, Copy, Debug)]
pub struct FlexItem {
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub grow: f32,
}

/// A resolved slot for a child on the flex main axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slot {
    pub x: f32,
    pub width: f32,
    pub height: f32,
}

/// Solve a single-row flex layout in a container of `container_width` px.
pub fn solve_row(container_width: f32, items: &[FlexItem]) -> Vec<Slot> {
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
        let slots = solve_row(300.0, &items);
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
        let slots = solve_row(300.0, &items);
        assert!((slots[0].width - 100.0).abs() < 1.0);
        // The grow child takes the remaining 200.
        assert!((slots[1].width - 200.0).abs() < 1.0, "got {slots:?}");
    }
}
