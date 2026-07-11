//! Flex/grid slot type.
//!
//! Flex and grid layout now run through the unified taffy tree ([`crate::taffy_tree`]),
//! which lays out a container and its directly-nested flex/grid descendants in one tree.
//! This module retains only [`Slot`] — the resolved per-child rectangle that
//! `place_taffy_slots` consumes. (The previous per-container `solve_flex`/`solve_grid` +
//! hand-built item lists were superseded by `taffy_tree::solve_subtree`.)

/// A resolved slot for a child. `x`/`y` are offsets from the container's content origin.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slot {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
