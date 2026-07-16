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
    FlexDirection as CssDir, FlexWrap as CssWrap, GridLine as CssGridLine,
    JustifyContent as CssJustify, Position as CssPosition, TrackSize as CssTrackSize, TrackUnit,
};
use taffy::prelude::*;
use taffy::style::{
    BoxSizing as TaffyBoxSizing, Dimension, LengthPercentage, LengthPercentageAuto,
    Position as TaffyPosition,
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
        align_self: cs.align_self.map(map_align),
        justify_content: Some(map_justify(cs.justify_content)),
        gap: Size {
            width: length(cs.column_gap),
            height: length(cs.row_gap),
        },
        flex_direction: map_direction(cs.flex_direction),
        flex_wrap: map_wrap(cs.flex_wrap),
        flex_grow: cs.flex_grow,
        flex_shrink: cs.flex_shrink,
        flex_basis: dimension(cs.flex_basis, calc),
        grid_template_columns: cs
            .grid_template_columns
            .iter()
            .map(|t| GridTemplateComponent::Single(track(t)))
            .collect(),
        grid_template_rows: cs
            .grid_template_rows
            .iter()
            .map(|t| GridTemplateComponent::Single(track(t)))
            .collect(),
        grid_column: grid_line(cs.grid_column),
        grid_row: grid_line(cs.grid_row),
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

    fn add(&mut self, dom: &Dom, styles: &StyleMap, node: DomNodeId) -> TId {
        let cs = &styles[&node];
        let style = to_taffy_style(cs, &mut self.calc);
        let container = matches!(
            cs.display,
            CssDisplay::Flex | CssDisplay::Grid | CssDisplay::InlineFlex | CssDisplay::InlineGrid
        );
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
            continue;
        }
        match styles.get(&c).map(|s| s.display) {
            None | Some(CssDisplay::None) => {}
            Some(CssDisplay::Contents) => out.extend(flex_items(dom, styles, c, depth + 1)),
            Some(_) => out.push(c),
        }
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

/// Lay out a flex/grid `container` and its directly-nested flex/grid descendants in one
/// unified taffy tree, measuring block/inline/float/table leaves via `measure`. Returns the
/// container's direct children as [`Placed`] subtrees (positions relative to the content
/// origin) — a container child carries its whole positioned subtree so the caller extracts
/// it directly instead of re-solving.
pub fn solve_subtree<'m>(
    dom: &Dom,
    styles: &StyleMap,
    container: DomNodeId,
    container_width: f32,
    container_height: Option<f32>,
    measure: impl FnMut(DomNodeId, Size<Option<f32>>, Size<AvailableSpace>) -> Size<f32> + 'm,
) -> Vec<Placed> {
    let (mut tree, root) = TaffyDom::build(dom, styles, container, Box::new(measure));
    // Pin the root to the given content size (Manuk resolved width; height when definite).
    let r: usize = root.into();
    tree.nodes[r].style.size = Size {
        width: length(container_width),
        height: container_height.map(length).unwrap_or(auto()),
    };
    compute_root_layout(
        &mut tree,
        root,
        Size {
            width: AvailableSpace::Definite(container_width),
            height: match container_height {
                Some(h) => AvailableSpace::Definite(h),
                None => AvailableSpace::MinContent,
            },
        },
    );
    let child_ids: Vec<TId> = tree.nodes[r].children.clone();
    child_ids.iter().map(|&c| tree.placed(c)).collect()
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

        let placed = solve_subtree(&dom, &styles, container, 1000.0, None, |_n, _k, _a| Size {
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
        let placed = solve_subtree(&dom, &styles, container, 300.0, None, |_n, _k, _a| Size {
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
}
